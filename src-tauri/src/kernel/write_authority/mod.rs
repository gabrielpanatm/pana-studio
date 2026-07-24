use std::{
    fmt,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::Serialize;

mod authority;
mod boundary;
mod capability;
#[cfg(test)]
mod compliance;
mod model;
mod operation;
mod recovery;
mod registry;
mod root_authority;
#[cfg(test)]
pub(crate) mod test_support;
mod tree_fingerprint;

pub use authority::WriteAuthority;
pub use model::{
    ConflictPolicy, ExpectedLeaf, ExpectedLeafVersion, RecoveryPolicy, WriteAtomicity,
    WriteAuthorityError, WriteCategory, WriteIntent, WriteOperationKind, WriteOwner, WritePolicy,
    WriteReceipt, WriteRecoveryReceipt, WriteRejection, WriteTarget,
};
pub use recovery::{
    WalPhase, WriteAuthorityRecoveryClassification, WriteAuthorityRecoveryItem,
    WriteAuthorityRecoveryResolutionAction, WriteAuthorityRecoveryResolutionInput,
    WriteAuthorityRecoveryResolutionReceipt, WriteAuthorityRecoveryScan,
};
pub use registry::{known_write_declarations, WriteDeclaration};
pub(crate) use root_authority::{ActiveProjectReadLease, CodexConfigLease, ProjectBootstrapLease};
pub use root_authority::{ApplicationAuthorityPaths, WriteAuthorityRuntime};

static ZOLA_ARTIFACT_PUBLICATION_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static COMPONENT_VALIDATION_SANDBOX_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Sealed, descriptor-backed authority for one configured Zola output name
/// and its private sibling generations. It is intentionally separate from
/// ProjectSourceWrite because valid Zola `output_dir` values may be outside
/// the selected project root.
pub(crate) struct ZolaArtifactPublicationLease {
    authority: root_authority::DirectoryAuthority,
    artifact_root: PathBuf,
    lease_id: u64,
}

impl ZolaArtifactPublicationLease {
    pub(crate) fn capture(artifact_root: &Path) -> Result<Self, String> {
        let parent = artifact_root.parent().ok_or_else(|| {
            format!(
                "Artifactul {} nu are un director părinte sigur.",
                artifact_root.display()
            )
        })?;
        let lease_id = ZOLA_ARTIFACT_PUBLICATION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let authority = capability::bootstrap_directory_authority(
            parent,
            "zola-artifact/parent",
            root_authority::DirectoryAuthorityScope::ZolaArtifactPublication { lease_id },
        )?;
        Ok(Self {
            authority,
            artifact_root: artifact_root.to_path_buf(),
            lease_id,
        })
    }

    pub(crate) fn verify_path_binding(&self) -> Result<(), String> {
        capability::verify_directory_authority_path(&self.authority)
    }

    pub(crate) fn publish_private_generation(
        &self,
        staging_root: &Path,
    ) -> Result<Option<String>, CapabilityMaintenanceError> {
        let source = self.private_generation_target(staging_root)?;
        let destination = WriteTarget::new(
            &self.artifact_root,
            self.authority.root_path(),
            "zola-artifact/public",
        )
        .bind_authority(self.authority.clone())?;
        let effect = capability::publish_rebuildable_directory(&source, &destination)?;
        require_durable_maintenance_effect(effect)?;

        // With EXCHANGE the old complete artifact now owns the private staged
        // name. Without a previous artifact the staged name is already absent.
        match self.discard_private_generation(staging_root) {
            Ok(_) => Ok(None),
            Err(error) => Ok(Some(format!(
                "Artifactul nou este publicat, dar cleanup-ul generației precedente cere intervenție: {error}"
            ))),
        }
    }

    pub(crate) fn discard_private_generation(
        &self,
        staging_root: &Path,
    ) -> Result<bool, CapabilityMaintenanceError> {
        let target = self.private_generation_target(staging_root)?;
        let operation_id = format!("zola-artifact-cleanup-{}", self.lease_id);
        let effect = capability::remove_rebuildable_directory_if_exists(&target, &operation_id)?;
        require_durable_maintenance_effect(effect).map(|effect| effect.changed)
    }

    fn private_generation_target(
        &self,
        path: &Path,
    ) -> Result<WriteTarget, CapabilityMaintenanceError> {
        if path.parent() != self.artifact_root.parent()
            || !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(".pana-studio-build-staging-"))
        {
            return Err("Authority Zola a refuzat o generație care nu este sibling privat al artifactului configurat.".into());
        }
        WriteTarget::new(
            path,
            self.authority.root_path(),
            "zola-artifact/private-generation",
        )
        .bind_authority(self.authority.clone())
        .map_err(Into::into)
    }
}

/// Capability sigilat pentru proiecția temporară, completă, validată de Zola
/// înainte ca o mutație semantică de componentă să intre în ProjectWorkspace.
/// Sandbox-ul nu este o a doua autoritate de proiect: este rebuildable,
/// create-only și poate fi consumat doar de validarea componentelor.
pub(crate) struct ComponentValidationSandboxLease {
    parent_authority: root_authority::DirectoryAuthority,
    sandbox_authority: Option<root_authority::DirectoryAuthority>,
    stable_directory: Option<CapabilitySubprocessDirectory>,
    sandbox_root: PathBuf,
    lease_id: u64,
}

impl ComponentValidationSandboxLease {
    pub(crate) fn capture(sandbox_root: &Path) -> Result<Self, String> {
        let parent = sandbox_root.parent().ok_or_else(|| {
            format!(
                "Sandbox-ul de validare {} nu are un director părinte sigur.",
                sandbox_root.display()
            )
        })?;
        let lease_id = COMPONENT_VALIDATION_SANDBOX_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let scope = root_authority::DirectoryAuthorityScope::ComponentValidation { lease_id };
        let parent_authority = capability::bootstrap_directory_authority(
            parent,
            "component-validation/parent",
            scope.clone(),
        )?;
        let create_target = WriteTarget::new(
            sandbox_root,
            parent_authority.root_path(),
            "component-validation/sandbox",
        )
        .with_expected_absent()
        .bind_authority(parent_authority.clone())?;
        let effect = capability::create_component_validation_directory(&create_target)?;
        require_durable_maintenance_effect(effect).map_err(|error| error.to_string())?;

        let sandbox_authority = match capability::capture_descendant_authority(
            &parent_authority,
            sandbox_root,
            "component-validation/root",
            scope,
        ) {
            Ok(authority) => authority,
            Err(error) => {
                return Err(cleanup_failed_component_validation_capture(
                    &parent_authority,
                    sandbox_root,
                    lease_id,
                    error,
                ));
            }
        };
        let stable_directory = match capability::capture_directory_lease_from_authority(
            &sandbox_authority,
            sandbox_root,
            "component-validation/stable-root",
        ) {
            Ok(directory) => directory,
            Err(error) => {
                drop(sandbox_authority);
                return Err(cleanup_failed_component_validation_capture(
                    &parent_authority,
                    sandbox_root,
                    lease_id,
                    error,
                ));
            }
        };

        Ok(Self {
            parent_authority,
            sandbox_authority: Some(sandbox_authority),
            stable_directory: Some(CapabilitySubprocessDirectory {
                inner: stable_directory,
            }),
            sandbox_root: sandbox_root.to_path_buf(),
            lease_id,
        })
    }

    pub(crate) fn current_dir_path(&self) -> PathBuf {
        self.stable_directory
            .as_ref()
            .expect("lease-ul viu păstrează directorul stabil")
            .current_dir_path()
    }

    pub(crate) fn write_bytes(&self, relative_path: &Path, bytes: &[u8]) -> Result<(), String> {
        let relative_path = validation_relative_path(relative_path)?;
        let target = self.create_only_target(relative_path, "component-validation/projected")?;
        let effect = capability::atomic_write(
            &target,
            bytes,
            capability::CapabilityReplacePolicy::CreateNew,
        )?;
        require_durable_maintenance_effect(effect)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub(crate) fn copy_regular_file(
        &self,
        source: &Path,
        relative_path: &Path,
    ) -> Result<(), String> {
        let relative_path = validation_relative_path(relative_path)?;
        let target = self.create_only_target(relative_path, "component-validation/copied")?;
        let effect = capability::copy_rebuildable_file(&target, source)?;
        require_durable_maintenance_effect(effect)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub(crate) fn discard(mut self) -> Result<(), String> {
        self.stable_directory.take();
        self.sandbox_authority.take();
        discard_component_validation_sandbox(
            &self.parent_authority,
            &self.sandbox_root,
            self.lease_id,
        )
    }

    fn create_only_target(
        &self,
        relative_path: &Path,
        public_label: &str,
    ) -> Result<WriteTarget, String> {
        let authority = self
            .sandbox_authority
            .as_ref()
            .ok_or_else(|| "Sandbox-ul de validare a fost deja închis.".to_string())?;
        WriteTarget::new(
            self.sandbox_root.join(relative_path),
            authority.root_path(),
            public_label,
        )
        .with_expected_absent()
        .bind_authority(authority.clone())
    }
}

fn validation_relative_path(path: &Path) -> Result<&Path, String> {
    if path.as_os_str().is_empty()
        || path.components().any(
            |component| !matches!(component, Component::Normal(segment) if !segment.is_empty()),
        )
    {
        return Err(format!(
            "Sandbox-ul de validare a refuzat path-ul relativ nesigur {}.",
            path.display()
        ));
    }
    Ok(path)
}

fn cleanup_failed_component_validation_capture(
    parent_authority: &root_authority::DirectoryAuthority,
    sandbox_root: &Path,
    lease_id: u64,
    capture_error: String,
) -> String {
    match discard_component_validation_sandbox(parent_authority, sandbox_root, lease_id) {
        Ok(()) => capture_error,
        Err(cleanup_error) => format!(
            "{capture_error} În plus, sandbox-ul incomplet nu a putut fi eliminat: {cleanup_error}"
        ),
    }
}

fn discard_component_validation_sandbox(
    parent_authority: &root_authority::DirectoryAuthority,
    sandbox_root: &Path,
    lease_id: u64,
) -> Result<(), String> {
    let target = WriteTarget::new(
        sandbox_root,
        parent_authority.root_path(),
        "component-validation/cleanup",
    )
    .bind_authority(parent_authority.clone())?;
    let operation_id = format!("component-validation-cleanup-{lease_id}");
    let effect = capability::remove_rebuildable_directory_if_exists(&target, &operation_id)?;
    require_durable_maintenance_effect(effect)
        .map(|_| ())
        .map_err(|error| error.to_string())
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub(crate) enum CapabilityMaintenanceError {
    Rejected(WriteRejection),
    RecoveryRequired(CapabilityMaintenanceRecovery),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CapabilityMaintenanceRecovery {
    pub changed: bool,
    pub bytes_written: u64,
    pub diagnostic: String,
    retry_forbidden: bool,
}

#[cfg(test)]
impl CapabilityMaintenanceRecovery {
    pub const fn retry_forbidden(&self) -> bool {
        self.retry_forbidden
    }
}

impl CapabilityMaintenanceError {
    pub fn into_terminal_diagnostic(self) -> String {
        self.to_string()
    }
}

impl From<String> for CapabilityMaintenanceError {
    fn from(diagnostic: String) -> Self {
        Self::Rejected(WriteRejection::new(diagnostic))
    }
}

impl From<&str> for CapabilityMaintenanceError {
    fn from(diagnostic: &str) -> Self {
        Self::Rejected(WriteRejection::new(diagnostic))
    }
}

impl fmt::Display for CapabilityMaintenanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(rejection) => formatter.write_str(&rejection.diagnostic),
            Self::RecoveryRequired(recovery) => write!(
                formatter,
                "CAPABILITY_MAINTENANCE_RECOVERY_REQUIRED: {} Nu repeta operația automat.",
                recovery.diagnostic
            ),
        }
    }
}

impl std::error::Error for CapabilityMaintenanceError {}

fn require_durable_maintenance_effect(
    effect: capability::CapabilityEffect,
) -> Result<capability::CapabilityEffect, CapabilityMaintenanceError> {
    if effect.recovery_required {
        return Err(CapabilityMaintenanceError::RecoveryRequired(
            CapabilityMaintenanceRecovery {
                changed: effect.changed,
                bytes_written: effect.bytes_written,
                diagnostic: effect.diagnostic.unwrap_or_else(|| {
                    "Efectul maintenance este vizibil, dar durabilitatea lui este incertă."
                        .to_string()
                }),
                retry_forbidden: true,
            },
        ));
    }
    Ok(effect)
}

pub(crate) fn capability_append_observability_file(
    runtime: Option<&WriteAuthorityRuntime>,
    path: &Path,
    boundary_root: &Path,
    public_label: &str,
    bytes: &[u8],
) -> Result<u64, CapabilityMaintenanceError> {
    let target = observability_target(runtime, path, boundary_root, public_label)?;
    let effect = capability::append(&target, bytes)?;
    require_durable_maintenance_effect(effect).map(|effect| effect.bytes_written)
}

pub(crate) enum CapabilityMaintenanceLockMode {
    Shared,
    Exclusive,
}

pub(crate) struct CapabilityMaintenanceLock {
    _inner: capability::CapabilityFileLock,
}

/// Held directory capability used only to hand a stable working directory to
/// a trusted subprocess. The child receives `.` after `current_dir` resolves
/// this descriptor-backed path, never the original mutable pathname.
pub(crate) struct CapabilitySubprocessDirectory {
    inner: capability::CapabilityDirectoryLease,
}

impl CapabilitySubprocessDirectory {
    pub fn current_dir_path(&self) -> std::path::PathBuf {
        self.inner.current_dir_path()
    }
}

pub(crate) fn capability_open_regular_file_readonly_no_follow(
    path: &Path,
    public_label: &str,
) -> Result<std::fs::File, String> {
    capability::open_regular_file_readonly_no_follow(path, public_label)
}

pub(crate) fn capability_open_optional_regular_file_readonly_no_follow(
    path: &Path,
    public_label: &str,
) -> Result<Option<std::fs::File>, String> {
    capability::open_optional_regular_file_readonly_no_follow(path, public_label)
}

pub(crate) fn capability_lock_observability_file(
    runtime: Option<&WriteAuthorityRuntime>,
    path: &Path,
    boundary_root: &Path,
    public_label: &str,
    mode: CapabilityMaintenanceLockMode,
) -> Result<CapabilityMaintenanceLock, CapabilityMaintenanceError> {
    let target = observability_target(runtime, path, boundary_root, public_label)?;
    let mode = match mode {
        CapabilityMaintenanceLockMode::Shared => capability::CapabilityLockMode::Shared,
        CapabilityMaintenanceLockMode::Exclusive => capability::CapabilityLockMode::Exclusive,
    };
    capability::lock_file(&target, mode)
        .map(|inner| CapabilityMaintenanceLock { _inner: inner })
        .map_err(Into::into)
}

pub(crate) fn capability_remove_observability_file(
    runtime: Option<&WriteAuthorityRuntime>,
    path: &Path,
    boundary_root: &Path,
    public_label: &str,
) -> Result<bool, CapabilityMaintenanceError> {
    let target = observability_target(runtime, path, boundary_root, public_label)?;
    let effect = capability::remove_file_if_exists_maintenance(&target)?;
    require_durable_maintenance_effect(effect).map(|effect| effect.changed)
}

pub(crate) fn capability_rename_observability_file(
    runtime: Option<&WriteAuthorityRuntime>,
    source: &Path,
    destination: &Path,
    boundary_root: &Path,
    source_label: &str,
    destination_label: &str,
) -> Result<(), CapabilityMaintenanceError> {
    let source = observability_target(runtime, source, boundary_root, source_label)?;
    let destination = observability_target(runtime, destination, boundary_root, destination_label)?;
    let effect = capability::rename_noreplace(&source, &destination)?;
    require_durable_maintenance_effect(effect).map(|_| ())
}

fn observability_target(
    runtime: Option<&WriteAuthorityRuntime>,
    path: &Path,
    boundary_root: &Path,
    public_label: &str,
) -> Result<WriteTarget, CapabilityMaintenanceError> {
    if let Some(runtime) = runtime {
        let authority = runtime.observability_authority(path)?;
        return WriteTarget::new(path, boundary_root, public_label)
            .bind_authority(authority)
            .map_err(Into::into);
    }
    #[cfg(test)]
    {
        return Ok(WriteTarget::new(path, boundary_root, public_label));
    }
    #[cfg(not(test))]
    Err("Observability capability cere WriteAuthorityRuntime instalat.".into())
}

#[cfg(test)]
mod maintenance_tests {
    use std::{
        fs,
        path::Path,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        require_durable_maintenance_effect, CapabilityMaintenanceError,
        CapabilityMaintenanceRecovery, ComponentValidationSandboxLease,
    };
    use crate::kernel::write_authority::capability::CapabilityEffect;

    #[test]
    fn maintenance_adapter_never_flattens_recovery_required_effect() {
        let effect = CapabilityEffect {
            changed: true,
            bytes_written: 12,
            recovery_required: true,
            diagnostic: Some("directory fsync failed".to_string()),
        };

        let error = require_durable_maintenance_effect(effect).unwrap_err();
        let CapabilityMaintenanceError::RecoveryRequired(recovery) = error else {
            panic!("maintenance recovery must not become a zero-effect rejection");
        };
        assert_eq!(
            recovery,
            CapabilityMaintenanceRecovery {
                changed: true,
                bytes_written: 12,
                diagnostic: "directory fsync failed".to_string(),
                retry_forbidden: true,
            }
        );
        assert!(recovery.retry_forbidden());
    }

    #[test]
    fn maintenance_adapter_returns_only_durable_effects_as_ok() {
        let effect = CapabilityEffect {
            changed: false,
            bytes_written: 0,
            recovery_required: false,
            diagnostic: None,
        };

        assert_eq!(
            require_durable_maintenance_effect(effect).unwrap().changed,
            false
        );
    }

    #[test]
    fn component_validation_sandbox_streams_files_and_is_removed_explicitly() {
        let parent = validation_test_root("lifecycle");
        fs::create_dir_all(&parent).unwrap();
        let source = parent.join("source.bin");
        fs::write(&source, [0_u8, 1, 2, 3, 255]).unwrap();
        let sandbox_root = parent.join("sandbox");

        let sandbox = ComponentValidationSandboxLease::capture(&sandbox_root).unwrap();
        sandbox
            .write_bytes(Path::new("templates/card.html"), b"<article />")
            .unwrap();
        sandbox
            .copy_regular_file(&source, Path::new("static/card.bin"))
            .unwrap();
        let stable_root = sandbox.current_dir_path();
        assert_eq!(
            fs::read_to_string(stable_root.join("templates/card.html")).unwrap(),
            "<article />"
        );
        assert_eq!(
            fs::read(stable_root.join("static/card.bin")).unwrap(),
            [0_u8, 1, 2, 3, 255]
        );

        sandbox.discard().unwrap();
        assert!(!sandbox_root.exists());
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn component_validation_sandbox_never_adopts_an_existing_directory() {
        let parent = validation_test_root("collision");
        let sandbox_root = parent.join("sandbox");
        fs::create_dir_all(&sandbox_root).unwrap();
        fs::write(sandbox_root.join("owner.txt"), b"external").unwrap();

        let error = ComponentValidationSandboxLease::capture(&sandbox_root)
            .err()
            .expect("un sandbox existent trebuie refuzat");
        assert!(error.contains("reutilizarea unui sandbox"));
        assert_eq!(
            fs::read_to_string(sandbox_root.join("owner.txt")).unwrap(),
            "external"
        );
        fs::remove_dir_all(parent).unwrap();
    }

    fn validation_test_root(label: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "pana-component-validation-{label}-{}-{nonce}",
            std::process::id()
        ))
    }
}
