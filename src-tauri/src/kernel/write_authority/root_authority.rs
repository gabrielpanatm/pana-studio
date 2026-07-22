use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard,
    },
};

use super::{
    capability,
    model::{WriteCategory, WriteIntent, WriteOwner, WriteTarget},
    recovery::{RecoveryCoordinator, WriteAuthorityRecoveryScan},
};

static SCOPED_AUTHORITY_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct FilesystemIdentity {
    pub device: u64,
    pub inode: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum DirectoryAuthorityScope {
    ApplicationConfig,
    ApplicationData,
    ApplicationCache,
    ApplicationLogs,
    ApplicationPreviewCache,
    ApplicationWriteAuthorityWal,
    RecoveryTarget,
    ProjectRoot,
    ProjectBootstrap { lease_id: u64 },
    ExternalCodex { lease_id: u64 },
}

#[derive(Clone, Debug)]
pub(super) struct DirectoryAuthority {
    inner: Arc<DirectoryAuthorityInner>,
}

#[derive(Debug)]
struct DirectoryAuthorityInner {
    root_path: PathBuf,
    identity: FilesystemIdentity,
    scope: DirectoryAuthorityScope,
    #[cfg(target_os = "linux")]
    directory: rustix::fd::OwnedFd,
}

impl DirectoryAuthority {
    #[cfg(target_os = "linux")]
    pub(super) fn from_opened_directory(
        root_path: PathBuf,
        identity: FilesystemIdentity,
        scope: DirectoryAuthorityScope,
        directory: rustix::fd::OwnedFd,
    ) -> Self {
        Self {
            inner: Arc::new(DirectoryAuthorityInner {
                root_path,
                identity,
                scope,
                directory,
            }),
        }
    }

    pub(super) fn root_path(&self) -> &Path {
        &self.inner.root_path
    }

    pub(super) fn identity(&self) -> FilesystemIdentity {
        self.inner.identity
    }

    pub(super) fn scope(&self) -> &DirectoryAuthorityScope {
        &self.inner.scope
    }

    #[cfg(target_os = "linux")]
    pub(super) fn directory(&self) -> &rustix::fd::OwnedFd {
        &self.inner.directory
    }

    pub(super) fn same_authority(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationAuthorityPaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub log_dir: PathBuf,
    pub projects_config_dir: PathBuf,
    pub mcp_dir: PathBuf,
    pub sessions_dir: PathBuf,
    pub kernel_dir: PathBuf,
    pub write_authority_wal_dir: PathBuf,
    pub scratch_dir: PathBuf,
    pub preview_cache_dir: PathBuf,
    pub app_logs_dir: PathBuf,
}

#[derive(Clone, Debug)]
struct ApplicationAuthorities {
    paths: ApplicationAuthorityPaths,
    config: DirectoryAuthority,
    data: DirectoryAuthority,
    cache: DirectoryAuthority,
    logs: DirectoryAuthority,
    preview_cache: DirectoryAuthority,
    write_authority_wal: DirectoryAuthority,
}

impl ApplicationAuthorities {
    fn capture(paths: ApplicationAuthorityPaths) -> Result<Self, String> {
        let config = capability::bootstrap_directory_authority(
            &paths.config_dir,
            "application-home/config",
            DirectoryAuthorityScope::ApplicationConfig,
        )?;
        let data = capability::bootstrap_directory_authority(
            &paths.data_dir,
            "application-home/data",
            DirectoryAuthorityScope::ApplicationData,
        )?;
        let cache = capability::bootstrap_directory_authority(
            &paths.cache_dir,
            "application-home/cache",
            DirectoryAuthorityScope::ApplicationCache,
        )?;
        let logs = capability::bootstrap_directory_authority(
            &paths.log_dir,
            "application-home/logs",
            DirectoryAuthorityScope::ApplicationLogs,
        )?;

        for (authority, path, label) in [
            (
                &config,
                &paths.projects_config_dir,
                "application-home/config/projects",
            ),
            (&config, &paths.mcp_dir, "application-home/config/mcp"),
            (&data, &paths.sessions_dir, "application-home/data/sessions"),
            (&data, &paths.kernel_dir, "application-home/data/kernel"),
            (
                &data,
                &paths.write_authority_wal_dir,
                "application-home/data/kernel/write-authority-wal",
            ),
            (&cache, &paths.scratch_dir, "application-home/cache/scratch"),
            (
                &cache,
                &paths.preview_cache_dir,
                "application-home/cache/preview",
            ),
            (&logs, &paths.app_logs_dir, "application-home/logs/app"),
        ] {
            capability::create_directory_from_authority(authority, path, label)?;
        }

        let preview_cache = capability::capture_descendant_authority(
            &cache,
            &paths.preview_cache_dir,
            "application-home/cache/preview",
            DirectoryAuthorityScope::ApplicationPreviewCache,
        )?;
        let write_authority_wal = capability::capture_descendant_authority(
            &data,
            &paths.write_authority_wal_dir,
            "application-home/data/kernel/write-authority-wal",
            DirectoryAuthorityScope::ApplicationWriteAuthorityWal,
        )?;

        Ok(Self {
            paths,
            config,
            data,
            cache,
            logs,
            preview_cache,
            write_authority_wal,
        })
    }

    fn verify_path_bindings(&self) -> Result<(), String> {
        for authority in [
            &self.config,
            &self.data,
            &self.cache,
            &self.logs,
            &self.preview_cache,
            &self.write_authority_wal,
        ] {
            capability::verify_directory_authority_path(authority)?;
        }
        Ok(())
    }

    fn authority_for_internal_path(&self, path: &Path) -> Option<DirectoryAuthority> {
        [&self.config, &self.data, &self.cache, &self.logs]
            .into_iter()
            .find(|authority| path.starts_with(authority.root_path()))
            .cloned()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PendingProjectAuthority {
    runtime_session_id: String,
    project_root: PathBuf,
    project: DirectoryAuthority,
}

impl PendingProjectAuthority {
    pub(crate) fn verify_path_binding(&self) -> Result<(), String> {
        capability::verify_directory_authority_path(&self.project)
    }
}

#[derive(Clone, Debug)]
struct ActiveProjectAuthority {
    runtime_session_id: String,
    generation: u64,
    project_root: PathBuf,
    project: DirectoryAuthority,
}

#[derive(Debug, Default)]
struct ProjectAuthorityCatalog {
    active: Option<ActiveProjectAuthority>,
}

/// Runtime-only authority catalog. DTO snapshots keep paths and serializable
/// fingerprints; this state keeps the directory handles that actually grant
/// filesystem authority for the lifetime of the process/session.
#[derive(Debug, Default)]
pub struct WriteAuthorityRuntime {
    application: OnceLock<ApplicationAuthorities>,
    application_bootstrap_gate: Mutex<()>,
    recovery: OnceLock<RecoveryCoordinator>,
    recovery_bootstrap_gate: Mutex<()>,
    project: RwLock<ProjectAuthorityCatalog>,
    next_project_generation: AtomicU64,
}

impl WriteAuthorityRuntime {
    pub fn install_application_home(&self, paths: ApplicationAuthorityPaths) -> Result<(), String> {
        let _bootstrap = self.application_bootstrap_gate.lock().map_err(|_| {
            "WriteAuthority Application Home bootstrap gate este otrăvit.".to_string()
        })?;
        if let Some(installed) = self.application.get() {
            if installed.paths == paths {
                return installed.verify_path_bindings();
            }
            return Err(
                "WriteAuthorityRuntime a refuzat schimbarea Application Home după bootstrap."
                    .to_string(),
            );
        }
        let authorities = ApplicationAuthorities::capture(paths)?;
        self.application.set(authorities).map_err(|_| {
            "WriteAuthorityRuntime nu a putut publica Application Home o singură dată.".to_string()
        })
    }

    pub fn application_paths(&self) -> Option<ApplicationAuthorityPaths> {
        self.application.get().map(|value| value.paths.clone())
    }

    pub(crate) fn boot_recovery(&self) -> Result<WriteAuthorityRecoveryScan, String> {
        let _bootstrap = self
            .recovery_bootstrap_gate
            .lock()
            .map_err(|_| "WriteAuthority recovery bootstrap gate este otrăvit.".to_string())?;
        if self.recovery.get().is_none() {
            let application = self.application.get().ok_or_else(|| {
                "WriteAuthority recovery nu poate porni înainte de Application Home.".to_string()
            })?;
            let coordinator =
                RecoveryCoordinator::bootstrap(application.write_authority_wal.clone())?;
            self.recovery.set(coordinator).map_err(|_| {
                "WriteAuthority recovery coordinator nu a putut fi publicat one-shot.".to_string()
            })?;
        }
        self.recovery
            .get()
            .ok_or_else(|| {
                "WriteAuthority recovery coordinator lipsește după bootstrap.".to_string()
            })?
            .snapshot()
    }

    pub(crate) fn recovery_scan(&self) -> Result<WriteAuthorityRecoveryScan, String> {
        if self.recovery.get().is_none() {
            return self.boot_recovery();
        }
        self.recovery
            .get()
            .ok_or_else(|| "WriteAuthority recovery coordinator lipsește.".to_string())?
            .rescan_and_recover_exclusive()
    }

    pub(crate) fn resolve_recovery(
        &self,
        input: super::recovery::WriteAuthorityRecoveryResolutionInput,
    ) -> Result<super::recovery::WriteAuthorityRecoveryResolutionReceipt, String> {
        self.ensure_recovery()?.resolve_operator_exclusive(input)
    }

    pub(crate) fn require_recovery_clean(&self) -> Result<(), String> {
        self.ensure_recovery()?.require_clean()
    }

    pub(super) fn recovery_coordinator(&self) -> Result<&RecoveryCoordinator, String> {
        self.ensure_recovery()
    }

    fn ensure_recovery(&self) -> Result<&RecoveryCoordinator, String> {
        if self.recovery.get().is_none() {
            self.boot_recovery()?;
        }
        self.recovery
            .get()
            .ok_or_else(|| "WriteAuthority recovery coordinator nu este instalat.".to_string())
    }

    pub(crate) fn capture_pending_project(
        &self,
        runtime_session_id: impl Into<String>,
        project_root: impl Into<PathBuf>,
        expected_device: u64,
        expected_inode: u64,
    ) -> Result<PendingProjectAuthority, String> {
        let project_root = project_root.into();
        let project = capability::capture_directory_authority(
            &project_root,
            "project-session/root",
            DirectoryAuthorityScope::ProjectRoot,
        )?;
        let expected = FilesystemIdentity {
            device: expected_device,
            inode: expected_inode,
        };
        if project.identity() != expected {
            return Err(format!(
                "ProjectSession authority a refuzat root-ul {}: fingerprintul sesiunii este dev={} ino={}, dar handle-ul capturat este dev={} ino={}.",
                project_root.display(),
                expected.device,
                expected.inode,
                project.identity().device,
                project.identity().inode
            ));
        }
        Ok(PendingProjectAuthority {
            runtime_session_id: runtime_session_id.into(),
            project_root,
            project,
        })
    }

    pub(crate) fn project_publication(&self) -> Result<ProjectPublicationGuard<'_>, String> {
        self.project
            .write()
            .map(|catalog| ProjectPublicationGuard {
                runtime: self,
                catalog,
            })
            .map_err(|_| "WriteAuthority project publication gate este otrăvit.".to_string())
    }

    pub(crate) fn acquire_active_project_read_lease_for_session(
        &self,
        expected_root: &Path,
        expected_runtime_session_id: &str,
    ) -> Result<ActiveProjectReadLease<'_>, String> {
        self.acquire_active_project_read_lease_inner(
            expected_root,
            Some(expected_runtime_session_id),
        )
    }

    fn acquire_active_project_read_lease_inner(
        &self,
        expected_root: &Path,
        expected_runtime_session_id: Option<&str>,
    ) -> Result<ActiveProjectReadLease<'_>, String> {
        let catalog = self
            .project
            .read()
            .map_err(|_| "WriteAuthority project effect gate este otrăvit.".to_string())?;
        let active = catalog.active.as_ref().ok_or_else(|| {
            "WriteAuthority preview sync cere ProjectSession authority activă.".to_string()
        })?;
        if active.project_root != expected_root {
            return Err(format!(
                "WriteAuthority preview sync a refuzat project switch-ul: expected {}, active {}.",
                expected_root.display(),
                active.project_root.display()
            ));
        }
        if let Some(expected_runtime_session_id) = expected_runtime_session_id {
            if active.runtime_session_id != expected_runtime_session_id {
                return Err(format!(
                    "WriteAuthority read lease a refuzat aceeași rădăcină redeschisă: requestul aparține runtime session {}, dar authority activă este {} generation {}.",
                    expected_runtime_session_id, active.runtime_session_id, active.generation
                ));
            }
        }
        capability::verify_directory_authority_path(&active.project)?;
        Ok(ActiveProjectReadLease { catalog })
    }

    pub(super) fn acquire_write_lease<'a>(
        &'a self,
        intent: &WriteIntent,
    ) -> Result<RuntimeWriteLease<'a>, String> {
        let selector = selector_for_intent(intent);
        match selector {
            AuthoritySelector::Application => {
                let application = self.application.get().ok_or_else(|| {
                    "WriteAuthorityRuntime nu are Application Home instalat.".to_string()
                })?;
                Ok(RuntimeWriteLease {
                    application,
                    project: None,
                    selector,
                })
            }
            AuthoritySelector::PreviewCache => {
                let application = self.application.get().ok_or_else(|| {
                    "WriteAuthorityRuntime nu are Application Home instalat.".to_string()
                })?;
                Ok(RuntimeWriteLease {
                    application,
                    project: None,
                    selector,
                })
            }
            AuthoritySelector::ActiveProject => {
                let application = self.application.get().ok_or_else(|| {
                    "WriteAuthorityRuntime nu are Application Home instalat.".to_string()
                })?;
                let project = self
                    .project
                    .read()
                    .map_err(|_| "WriteAuthority project effect gate este otrăvit.".to_string())?;
                if project.active.is_none() {
                    return Err(
                        "WriteAuthority a refuzat mutația: nu există ProjectSession authority activă."
                            .to_string(),
                    );
                }
                Ok(RuntimeWriteLease {
                    application,
                    project: Some(project),
                    selector,
                })
            }
            AuthoritySelector::ProjectBootstrap | AuthoritySelector::ExternalCodex => {
                let application = self.application.get().ok_or_else(|| {
                    "WriteAuthorityRuntime nu are Application Home instalat.".to_string()
                })?;
                Ok(RuntimeWriteLease {
                    application,
                    project: None,
                    selector,
                })
            }
        }
    }

    pub(super) fn observability_authority(
        &self,
        path: &Path,
    ) -> Result<DirectoryAuthority, String> {
        let application = self.application.get().ok_or_else(|| {
            "WriteAuthorityRuntime nu are Application Home instalat pentru observability."
                .to_string()
        })?;
        if !path.starts_with(application.logs.root_path()) {
            return Err(format!(
                "Observability path {} nu aparține ApplicationHome.logs.",
                path.display()
            ));
        }
        Ok(application.logs.clone())
    }
}

pub(crate) struct ProjectPublicationGuard<'a> {
    runtime: &'a WriteAuthorityRuntime,
    catalog: RwLockWriteGuard<'a, ProjectAuthorityCatalog>,
}

impl ProjectPublicationGuard<'_> {
    pub(crate) fn publish(&mut self, pending: PendingProjectAuthority) -> Result<u64, String> {
        capability::verify_directory_authority_path(&pending.project)?;
        let generation = self
            .runtime
            .next_project_generation
            .fetch_add(1, Ordering::SeqCst)
            .saturating_add(1);
        self.catalog.active = Some(ActiveProjectAuthority {
            runtime_session_id: pending.runtime_session_id,
            generation,
            project_root: pending.project_root,
            project: pending.project,
        });
        Ok(generation)
    }

    pub(crate) fn revoke(&mut self) {
        self.runtime
            .next_project_generation
            .fetch_add(1, Ordering::SeqCst);
        self.catalog.active = None;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthoritySelector {
    Application,
    ActiveProject,
    PreviewCache,
    ProjectBootstrap,
    ExternalCodex,
}

pub(super) struct RuntimeWriteLease<'a> {
    application: &'a ApplicationAuthorities,
    project: Option<RwLockReadGuard<'a, ProjectAuthorityCatalog>>,
    selector: AuthoritySelector,
}

impl RuntimeWriteLease<'_> {
    pub(super) fn bind_target(&self, target: &WriteTarget) -> Result<WriteTarget, String> {
        let authority = match self.selector {
            AuthoritySelector::Application => self
                .application
                .authority_for_internal_path(&target.path)
                .ok_or_else(|| {
                    format!(
                        "WriteAuthority a refuzat {}: target-ul nu aparține Application Home instalat.",
                        target.public_label
                    )
                })?,
            AuthoritySelector::PreviewCache => {
                if !target.path.starts_with(self.application.preview_cache.root_path()) {
                    return Err(format!(
                        "WriteAuthority a refuzat {}: preview-ul nu aparține ApplicationHome.preview_cache_dir.",
                        target.public_label
                    ));
                }
                self.application.preview_cache.clone()
            }
            AuthoritySelector::ActiveProject => {
                let active = self.active_project()?;
                require_expected_runtime_session(target, active)?;
                if !target.path.starts_with(&active.project_root) {
                    return Err(format!(
                        "WriteAuthority a refuzat {}: target-ul nu aparține ProjectSession {} generation {}.",
                        target.public_label, active.runtime_session_id, active.generation
                    ));
                }
                active.project.clone()
            }
            AuthoritySelector::ProjectBootstrap => {
                let authority = target.authority().ok_or_else(|| {
                    "ProjectInitializer cere un ProjectBootstrapLease sigilat.".to_string()
                })?;
                if !matches!(
                    authority.scope(),
                    DirectoryAuthorityScope::ProjectBootstrap { .. }
                ) {
                    return Err(
                        "ProjectInitializer a primit un grant cu scop incompatibil.".to_string()
                    );
                }
                authority.clone()
            }
            AuthoritySelector::ExternalCodex => {
                let authority = target.authority().ok_or_else(|| {
                    "ExternalIntegrationWrite cere un CodexConfigLease sigilat.".to_string()
                })?;
                if !matches!(
                    authority.scope(),
                    DirectoryAuthorityScope::ExternalCodex { .. }
                ) {
                    return Err(
                        "ExternalIntegrationWrite a primit un grant cu scop incompatibil."
                            .to_string(),
                    );
                }
                authority.clone()
            }
        };
        target.clone().bind_authority(authority)
    }

    fn active_project(&self) -> Result<&ActiveProjectAuthority, String> {
        self.project
            .as_ref()
            .and_then(|catalog| catalog.active.as_ref())
            .ok_or_else(|| "ProjectSession authority nu mai este activă.".to_string())
    }
}

fn require_expected_runtime_session(
    target: &WriteTarget,
    active: &ActiveProjectAuthority,
) -> Result<(), String> {
    let Some(expected) = target.expected_runtime_session_id.as_deref() else {
        return Ok(());
    };
    if expected == active.runtime_session_id {
        return Ok(());
    }
    Err(format!(
        "WriteAuthority a refuzat {}: requestul aparține runtime session {}, dar authority activă pentru același root este {} generation {}.",
        target.public_label, expected, active.runtime_session_id, active.generation
    ))
}

fn selector_for_intent(intent: &WriteIntent) -> AuthoritySelector {
    match intent.category {
        WriteCategory::InternalAppWrite => AuthoritySelector::Application,
        WriteCategory::PreviewWorkspaceWrite => AuthoritySelector::PreviewCache,
        WriteCategory::BuildOutputWrite => AuthoritySelector::ActiveProject,
        WriteCategory::ExternalIntegrationWrite => AuthoritySelector::ExternalCodex,
        WriteCategory::ProjectSourceWrite if intent.owner == WriteOwner::ProjectInitializer => {
            AuthoritySelector::ProjectBootstrap
        }
        WriteCategory::ProjectSourceWrite => AuthoritySelector::ActiveProject,
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ProjectBootstrapLease {
    authority: DirectoryAuthority,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ActiveProjectFileReadSnapshot {
    pub(crate) bytes: Vec<u8>,
    pub(crate) version_token: String,
}

pub(crate) struct ActiveProjectReadLease<'a> {
    catalog: RwLockReadGuard<'a, ProjectAuthorityCatalog>,
}

impl ActiveProjectReadLease<'_> {
    /// Captures a stable, descriptor-backed working directory inside the
    /// active ProjectSession authority for a trusted subprocess.
    ///
    /// The returned lease must remain alive until the child has been spawned.
    /// Project publication is also kept behind this read lease, so reopening
    /// the same lexical root cannot redirect the subprocess to another
    /// ProjectSession generation.
    pub(crate) fn capture_subprocess_directory(
        &self,
        relative_path: &Path,
        public_label: &str,
    ) -> Result<super::CapabilitySubprocessDirectory, String> {
        let active = self.catalog.active.as_ref().ok_or_else(|| {
            "WriteAuthority subprocess nu mai are ProjectSession authority activă.".to_string()
        })?;
        let path = active.project_root.join(relative_path);
        capability::capture_directory_lease_from_authority(&active.project, &path, public_label)
            .map(|inner| super::CapabilitySubprocessDirectory { inner })
    }

    pub(crate) fn read_bounded_regular_file(
        &self,
        relative_path: &Path,
        max_bytes: u64,
        public_label: &str,
    ) -> Result<Option<ActiveProjectFileReadSnapshot>, String> {
        let active = self.catalog.active.as_ref().ok_or_else(|| {
            "WriteAuthority bounded read nu mai are ProjectSession authority activă.".to_string()
        })?;
        let path = active.project_root.join(relative_path);
        capability::read_bounded_regular_file_from_authority(
            &active.project,
            &path,
            public_label,
            max_bytes,
        )
        .map(|snapshot| {
            snapshot.map(|snapshot| ActiveProjectFileReadSnapshot {
                bytes: snapshot.bytes,
                version_token: snapshot.version_token,
            })
        })
    }
}

impl ProjectBootstrapLease {
    pub(crate) fn capture(root: &Path) -> Result<Self, String> {
        let lease_id = SCOPED_AUTHORITY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let authority = capability::capture_directory_authority(
            root,
            "project-bootstrap/root",
            DirectoryAuthorityScope::ProjectBootstrap { lease_id },
        )?;
        Ok(Self { authority })
    }

    pub(crate) fn root(&self) -> &Path {
        self.authority.root_path()
    }

    pub(crate) fn target(
        &self,
        path: impl Into<PathBuf>,
        public_label: impl Into<String>,
    ) -> Result<WriteTarget, String> {
        WriteTarget::new(path, self.root(), public_label).bind_authority(self.authority.clone())
    }

    pub(crate) fn verify_path_binding(&self) -> Result<(), String> {
        capability::verify_directory_authority_path(&self.authority)
    }

    pub(super) fn authority(&self) -> &DirectoryAuthority {
        &self.authority
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CodexConfigLease {
    authority: DirectoryAuthority,
}

impl CodexConfigLease {
    pub(crate) fn capture(codex_dir: &Path) -> Result<Self, String> {
        let lease_id = SCOPED_AUTHORITY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let authority = capability::capture_directory_authority(
            codex_dir,
            "external-codex/root",
            DirectoryAuthorityScope::ExternalCodex { lease_id },
        )?;
        Ok(Self { authority })
    }

    pub(crate) fn target(
        &self,
        path: impl Into<PathBuf>,
        public_label: impl Into<String>,
    ) -> Result<WriteTarget, String> {
        WriteTarget::new(path, self.authority.root_path(), public_label)
            .bind_authority(self.authority.clone())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::{
        fs,
        os::unix::fs::MetadataExt,
        path::Path,
        sync::{mpsc, Arc},
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use crate::kernel::write_authority::{
        WriteCategory, WriteIntent, WriteOperationKind, WriteOwner, WritePolicy, WriteTarget,
    };

    #[test]
    fn project_publication_waits_for_in_flight_write_lease() {
        let root = unique_test_dir("project-publication-gate");
        let project_a = root.join("project-a");
        let project_b = root.join("project-b");
        fs::create_dir_all(project_a.join("sursa")).unwrap();
        fs::create_dir_all(project_b.join("sursa")).unwrap();

        let runtime = Arc::new(WriteAuthorityRuntime::default());
        let application = application_paths(&root.join("app-home"));
        runtime
            .install_application_home(application.clone())
            .unwrap();
        let session_a = application.sessions_dir.join("session-a");
        let session_b = application.sessions_dir.join("session-b");
        fs::create_dir_all(&session_a).unwrap();
        fs::create_dir_all(&session_b).unwrap();

        let pending_a = pending(&runtime, "session-a/runtime", &project_a, &session_a);
        let pending_b = pending(&runtime, "session-b/runtime", &project_b, &session_b);
        runtime
            .project_publication()
            .unwrap()
            .publish(pending_a)
            .unwrap();

        let intent_a = project_intent(&project_a, "sursa/a.txt");
        let lease_a = runtime.acquire_write_lease(&intent_a).unwrap();
        lease_a.bind_target(&intent_a.target).unwrap();

        let (started_tx, started_rx) = mpsc::channel();
        let (published_tx, published_rx) = mpsc::channel();
        let writer_runtime = Arc::clone(&runtime);
        let publisher = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            let generation = writer_runtime
                .project_publication()
                .unwrap()
                .publish(pending_b)
                .unwrap();
            published_tx.send(generation).unwrap();
        });

        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(matches!(
            published_rx.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));

        drop(lease_a);
        let generation_b = published_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        publisher.join().unwrap();
        assert!(generation_b >= 2);

        let current_lease = runtime.acquire_write_lease(&intent_a).unwrap();
        let stale_error = current_lease.bind_target(&intent_a.target).unwrap_err();
        assert!(stale_error.contains("session-b/runtime"));
        let intent_b = project_intent(&project_b, "sursa/b.txt");
        current_lease.bind_target(&intent_b.target).unwrap();
        drop(current_lease);
        drop(runtime);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn same_root_reopen_rejects_target_bound_to_stale_runtime_session() {
        let root = unique_test_dir("same-root-stale-runtime-session");
        let project = root.join("project");
        fs::create_dir_all(project.join("sursa")).unwrap();

        let runtime = WriteAuthorityRuntime::default();
        let application = application_paths(&root.join("app-home"));
        runtime
            .install_application_home(application.clone())
            .unwrap();
        let session_a = application.sessions_dir.join("session-a");
        let session_b = application.sessions_dir.join("session-b");
        fs::create_dir_all(&session_a).unwrap();
        fs::create_dir_all(&session_b).unwrap();

        runtime
            .project_publication()
            .unwrap()
            .publish(pending(
                &runtime,
                "same-root/runtime-a",
                &project,
                &session_a,
            ))
            .unwrap();

        let stale_intent = project_intent(&project, "sursa/stale.txt");
        assert_eq!(
            stale_intent.target.expected_runtime_session_id, None,
            "WriteTarget must remain backwards-compatible unless a caller opts into session CAS"
        );
        let mut stale_intent = stale_intent;
        stale_intent.target = stale_intent
            .target
            .with_expected_runtime_session_id("same-root/runtime-a");

        runtime
            .project_publication()
            .unwrap()
            .publish(pending(
                &runtime,
                "same-root/runtime-b",
                &project,
                &session_b,
            ))
            .unwrap();

        let lease = runtime.acquire_write_lease(&stale_intent).unwrap();
        let error = lease.bind_target(&stale_intent.target).unwrap_err();
        assert!(error.contains("same-root/runtime-a"));
        assert!(error.contains("same-root/runtime-b"));

        let current_intent = WriteIntent {
            target: project_intent(&project, "sursa/current.txt")
                .target
                .with_expected_runtime_session_id("same-root/runtime-b"),
            ..project_intent(&project, "sursa/current.txt")
        };
        lease.bind_target(&current_intent.target).unwrap();
        drop(lease);
        drop(runtime);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn same_root_reopen_rejects_session_bound_read_lease() {
        let root = unique_test_dir("same-root-stale-read-lease");
        let project = root.join("project");
        fs::create_dir_all(project.join("sursa")).unwrap();

        let runtime = WriteAuthorityRuntime::default();
        let application = application_paths(&root.join("app-home"));
        runtime
            .install_application_home(application.clone())
            .unwrap();
        let session_a = application.sessions_dir.join("session-a");
        let session_b = application.sessions_dir.join("session-b");
        fs::create_dir_all(&session_a).unwrap();
        fs::create_dir_all(&session_b).unwrap();
        runtime
            .project_publication()
            .unwrap()
            .publish(pending(
                &runtime,
                "same-root/runtime-a",
                &project,
                &session_a,
            ))
            .unwrap();
        runtime
            .acquire_active_project_read_lease_for_session(&project, "same-root/runtime-a")
            .unwrap();

        runtime
            .project_publication()
            .unwrap()
            .publish(pending(
                &runtime,
                "same-root/runtime-b",
                &project,
                &session_b,
            ))
            .unwrap();

        let error = match runtime
            .acquire_active_project_read_lease_for_session(&project, "same-root/runtime-a")
        {
            Ok(_) => panic!("read lease stale trebuia refuzat"),
            Err(error) => error,
        };
        assert!(error.contains("same-root/runtime-a"));
        assert!(error.contains("same-root/runtime-b"));
        runtime
            .acquire_active_project_read_lease_for_session(&project, "same-root/runtime-b")
            .unwrap();

        drop(runtime);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn active_project_read_lease_reads_bounded_and_rejects_symlink_paths() {
        let root = unique_test_dir("project-bounded-read");
        let project = root.join("project");
        let assets = project.join("assets");
        fs::create_dir_all(&assets).unwrap();
        fs::write(assets.join("image.bin"), b"bounded-image").unwrap();
        fs::write(root.join("outside.bin"), b"outside").unwrap();

        let runtime = WriteAuthorityRuntime::default();
        let application = application_paths(&root.join("app-home"));
        runtime
            .install_application_home(application.clone())
            .unwrap();
        let session = application.sessions_dir.join("session-a");
        fs::create_dir_all(&session).unwrap();
        runtime
            .project_publication()
            .unwrap()
            .publish(pending(
                &runtime,
                "bounded-read/runtime-a",
                &project,
                &session,
            ))
            .unwrap();

        let lease = runtime
            .acquire_active_project_read_lease_for_session(&project, "bounded-read/runtime-a")
            .unwrap();
        let snapshot = lease
            .read_bounded_regular_file(
                Path::new("assets/image.bin"),
                64,
                "test/project-bounded-read",
            )
            .unwrap()
            .unwrap();
        assert_eq!(snapshot.bytes, b"bounded-image");
        assert!(!snapshot.version_token.is_empty());

        let oversized = lease
            .read_bounded_regular_file(
                Path::new("assets/image.bin"),
                4,
                "test/project-bounded-read-oversized",
            )
            .unwrap_err();
        assert!(oversized.contains("depășește limita"));
        assert!(lease
            .read_bounded_regular_file(
                Path::new("assets/missing.bin"),
                64,
                "test/project-bounded-read-missing",
            )
            .unwrap()
            .is_none());

        std::os::unix::fs::symlink(root.join("outside.bin"), assets.join("leaf-link.bin")).unwrap();
        let leaf_error = lease
            .read_bounded_regular_file(
                Path::new("assets/leaf-link.bin"),
                64,
                "test/project-bounded-read-leaf-symlink",
            )
            .unwrap_err();
        assert!(leaf_error.contains("fără symlink"));

        std::os::unix::fs::symlink(&assets, project.join("assets-link")).unwrap();
        let ancestor_error = lease
            .read_bounded_regular_file(
                Path::new("assets-link/image.bin"),
                64,
                "test/project-bounded-read-ancestor-symlink",
            )
            .unwrap_err();
        assert!(ancestor_error.contains("fără symlink"));

        drop(lease);
        drop(runtime);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn active_project_bounded_read_rejects_leaf_swap_after_open() {
        let root = unique_test_dir("project-bounded-read-leaf-swap");
        let project = root.join("project");
        let assets = project.join("assets");
        fs::create_dir_all(&assets).unwrap();
        let target = assets.join("image.bin");
        let held = assets.join("image-held.bin");
        let outside = root.join("outside.bin");
        fs::write(&target, b"authority-bytes").unwrap();
        fs::write(&outside, b"outside-bytes").unwrap();

        let runtime = WriteAuthorityRuntime::default();
        let application = application_paths(&root.join("app-home"));
        runtime
            .install_application_home(application.clone())
            .unwrap();
        let session = application.sessions_dir.join("session-a");
        fs::create_dir_all(&session).unwrap();
        runtime
            .project_publication()
            .unwrap()
            .publish(pending(
                &runtime,
                "bounded-read-swap/runtime-a",
                &project,
                &session,
            ))
            .unwrap();
        let lease = runtime
            .acquire_active_project_read_lease_for_session(&project, "bounded-read-swap/runtime-a")
            .unwrap();

        let hook_target = target.clone();
        let hook_held = held.clone();
        let hook_outside = outside.clone();
        let result = capability::with_after_bounded_read_leaf_opened_hook_for_test(
            move || {
                fs::rename(&hook_target, &hook_held).unwrap();
                std::os::unix::fs::symlink(&hook_outside, &hook_target).unwrap();
            },
            || {
                lease.read_bounded_regular_file(
                    Path::new("assets/image.bin"),
                    64,
                    "test/project-bounded-read-leaf-swap",
                )
            },
        );
        let error = result.unwrap_err();
        assert!(error.contains("s-a schimbat"), "{error}");

        fs::remove_file(&target).unwrap();
        fs::rename(&held, &target).unwrap();
        drop(lease);
        drop(runtime);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn application_home_authority_cannot_be_rebound_after_bootstrap() {
        let root = unique_test_dir("application-home-single-install");
        let first = application_paths(&root.join("first"));
        let second = application_paths(&root.join("second"));
        let runtime = WriteAuthorityRuntime::default();

        runtime.install_application_home(first.clone()).unwrap();
        let error = runtime.install_application_home(second).unwrap_err();

        assert!(error.contains("schimbarea Application Home"));
        assert_eq!(runtime.application_paths(), Some(first));
        let held_data = root.join("first/data-held");
        fs::rename(root.join("first/data"), &held_data).unwrap();
        fs::create_dir_all(root.join("first/data")).unwrap();
        let replacement_error = runtime
            .install_application_home(application_paths(&root.join("first")))
            .unwrap_err();
        assert!(replacement_error.contains("înlocuit"));
        drop(runtime);
        fs::remove_dir_all(root).unwrap();
    }

    fn pending(
        runtime: &WriteAuthorityRuntime,
        runtime_session_id: &str,
        project_root: &Path,
        _session_dir: &Path,
    ) -> PendingProjectAuthority {
        let metadata = fs::metadata(project_root).unwrap();
        runtime
            .capture_pending_project(
                runtime_session_id,
                project_root,
                metadata.dev(),
                metadata.ino(),
            )
            .unwrap()
    }

    fn project_intent(project_root: &Path, relative: &str) -> WriteIntent {
        WriteIntent::new(
            WriteCategory::ProjectSourceWrite,
            WriteOwner::ProjectWorkspace,
            WriteOperationKind::WriteText,
            WriteTarget::new(
                project_root.join(relative),
                project_root,
                format!("test/{relative}"),
            )
            .with_expected_absent(),
            WritePolicy::project_workspace_write(),
            "Project publication gate test.",
        )
    }

    fn application_paths(root: &Path) -> ApplicationAuthorityPaths {
        ApplicationAuthorityPaths {
            config_dir: root.join("config"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            log_dir: root.join("logs"),
            projects_config_dir: root.join("config/projects"),
            mcp_dir: root.join("config/mcp"),
            sessions_dir: root.join("data/sessions"),
            kernel_dir: root.join("data/kernel"),
            write_authority_wal_dir: root.join("data/kernel/write-authority-wal"),
            scratch_dir: root.join("cache/scratch"),
            preview_cache_dir: root.join("cache/preview"),
            app_logs_dir: root.join("logs/app"),
        }
    }

    fn unique_test_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "pana-write-authority-{label}-{}-{nanos}",
            std::process::id()
        ))
    }
}
