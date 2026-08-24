use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Mutex,
};

use tauri::{AppHandle, Runtime};

use crate::kernel::{
    project_session::ProjectSessionSnapshot,
    write_authority::{
        WriteAuthority, WriteCategory, WriteIntent, WriteOperationKind, WriteOwner, WritePolicy,
        WriteTarget,
    },
};

use super::model::{
    WorkbenchDocumentPresentation, WorkbenchSnapshot, WorkbenchSplit, WorkbenchSurface,
    WORKBENCH_SCHEMA_VERSION,
};

const WORKBENCH_STATE_FILE: &str = "workbench.json";
const WORKBENCH_STATE_MAX_BYTES: u64 = 256 * 1024;
static WORKBENCH_PERSISTENCE_LOCK: Mutex<()> = Mutex::new(());

pub fn read_persisted_workbench(
    session: &ProjectSessionSnapshot,
) -> Result<Option<WorkbenchSnapshot>, String> {
    let path = workbench_state_path(session)?;
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "Workbench nu a putut verifica proiecția persistentă: {error}"
            ))
        }
    };
    if metadata.file_type().is_symlink() {
        return Err(
            "Workbench a refuzat proiecția persistentă: workbench.json este symlink.".to_string(),
        );
    }
    if !metadata.is_file() {
        return Err(
            "Workbench a refuzat proiecția persistentă: workbench.json nu este fișier.".to_string(),
        );
    }
    if metadata.len() > WORKBENCH_STATE_MAX_BYTES {
        return Err(format!(
            "Workbench a refuzat proiecția persistentă de {} bytes; limita este {}.",
            metadata.len(),
            WORKBENCH_STATE_MAX_BYTES
        ));
    }
    let source = fs::read_to_string(&path)
        .map_err(|error| format!("Workbench nu a putut citi proiecția persistentă: {error}"))?;
    let mut snapshot = serde_json::from_str::<WorkbenchSnapshot>(&source)
        .map_err(|error| format!("Workbench nu a putut decoda proiecția persistentă: {error}"))?;
    if snapshot.schema_version == 1 {
        // Schema 1 did not persist document presentation. Preserve only the
        // minimum compatibility needed to load its visual/split layout; the
        // authoritative ProjectScan/FileBuffer language immediately replaces
        // this migration hint before the snapshot is published.
        let visual_layout = snapshot.split != WorkbenchSplit::None;
        for group in &mut snapshot.groups {
            for document in &mut group.documents {
                document.presentation =
                    if visual_layout || document.surface == WorkbenchSurface::Visual {
                        WorkbenchDocumentPresentation::Html
                    } else {
                        WorkbenchDocumentPresentation::CodeOnly
                    };
            }
        }
        snapshot.schema_version = 2;
    }
    if snapshot.schema_version == 2 {
        // Schema 3 adds the project_settings activity. Schema 2 snapshots do
        // not require a data rewrite; advancing the explicit schema keeps the
        // persisted contract auditable instead of silently accepting an old
        // projection shape forever.
        snapshot.schema_version = WORKBENCH_SCHEMA_VERSION;
    }
    require_persisted_identity(session, &snapshot)?;
    Ok(Some(snapshot))
}

pub fn persist_workbench<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    snapshot: &WorkbenchSnapshot,
) -> Result<(), String> {
    let _guard = WORKBENCH_PERSISTENCE_LOCK
        .lock()
        .map_err(|_| "Workbench persistence mutex este compromis.".to_string())?;
    persist_workbench_unlocked(app, session, snapshot)
}

pub fn persist_latest_workbench<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    snapshot: &WorkbenchSnapshot,
) -> Result<(), String> {
    let _guard = WORKBENCH_PERSISTENCE_LOCK
        .lock()
        .map_err(|_| "Workbench persistence mutex este compromis.".to_string())?;
    if read_persisted_workbench(session)?
        .is_some_and(|persisted| persisted.revision >= snapshot.revision)
    {
        return Ok(());
    }
    persist_workbench_unlocked(app, session, snapshot)
}

fn persist_workbench_unlocked<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    snapshot: &WorkbenchSnapshot,
) -> Result<(), String> {
    require_persisted_identity(session, snapshot)?;
    if snapshot.runtime_session_id != session.runtime_instance_id() {
        return Err(format!(
            "Workbench a refuzat persistența pentru runtime session {}: sesiunea activă este {}.",
            snapshot.runtime_session_id,
            session.runtime_instance_id()
        ));
    }
    let source = serde_json::to_string_pretty(snapshot)
        .map_err(|error| format!("Workbench nu a putut serializa proiecția: {error}"))?;
    if source.len() as u64 > WORKBENCH_STATE_MAX_BYTES {
        return Err(format!(
            "Workbench a refuzat proiecția de {} bytes; limita este {}.",
            source.len(),
            WORKBENCH_STATE_MAX_BYTES
        ));
    }
    let path = workbench_state_path(session)?;
    let boundary = PathBuf::from(&session.session_dir);
    let intent = WriteIntent::new(
        WriteCategory::InternalAppWrite,
        WriteOwner::Workbench,
        WriteOperationKind::WriteText,
        WriteTarget::new(
            path,
            boundary,
            format!("sessions/{}/workbench.json", session.id),
        ),
        WritePolicy::workbench_projection_atomic(),
        "Persistă proiecția Workbench pentru ProjectSession.",
    );
    WriteAuthority::new(app)
        .write_text(intent, &format!("{source}\n"))
        .map_err(|error| error.into_terminal_diagnostic())
        .map(|_| ())
}

fn workbench_state_path(session: &ProjectSessionSnapshot) -> Result<PathBuf, String> {
    let boundary = Path::new(&session.session_dir);
    if !boundary.is_absolute() {
        return Err("Workbench cere un director ProjectSession absolut.".to_string());
    }
    Ok(boundary.join(WORKBENCH_STATE_FILE))
}

fn require_persisted_identity(
    session: &ProjectSessionSnapshot,
    snapshot: &WorkbenchSnapshot,
) -> Result<(), String> {
    if snapshot.schema_version != WORKBENCH_SCHEMA_VERSION {
        return Err(format!(
            "Workbench schema {} nu este compatibilă cu schema {}.",
            snapshot.schema_version, WORKBENCH_SCHEMA_VERSION
        ));
    }
    if snapshot.project_session_id != session.id || snapshot.project_root != session.project_root {
        return Err(format!(
            "Workbench proiecția persistentă aparține altei sesiuni/proiect: {}/{}.",
            snapshot.project_session_id, snapshot.project_root
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{env, fs, path::Path, time::SystemTime};

    use tauri::Manager;

    use crate::{
        app_home::{ensure_app_home, project_session_dir, TEST_APP_ENV_LOCK},
        kernel::{
            project_session::{ProjectRootFingerprint, ProjectSessionScanSummary},
            workbench::{
                WorkbenchDocumentPresentation, WorkbenchGroupId, WorkbenchIdentity,
                WorkbenchIntent, WorkbenchRuntime, WorkbenchSplit, WorkbenchSurface,
            },
            write_authority::WriteAuthorityRuntime,
        },
    };

    use super::*;

    #[test]
    fn persisted_workbench_roundtrip_uses_write_authority() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("workbench-storage");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build");
        let app_home = ensure_app_home(app.handle()).unwrap();
        app.state::<WriteAuthorityRuntime>()
            .boot_recovery()
            .unwrap();
        let session = test_session(app.handle(), "/project/workbench");
        fs::create_dir_all(&session.session_dir).unwrap();
        let snapshot = WorkbenchRuntime::default().read(&session).unwrap();

        persist_workbench(app.handle(), &session, &snapshot).unwrap();
        let restored = read_persisted_workbench(&session).unwrap().unwrap();

        assert_eq!(restored, snapshot);
        assert!(PathBuf::from(&session.session_dir)
            .join(WORKBENCH_STATE_FILE)
            .is_file());
        assert_eq!(
            PathBuf::from(app_home.sessions_dir).join(&session.id),
            PathBuf::from(&session.session_dir)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn schema_one_visual_split_is_loadable_before_authoritative_reconciliation() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("workbench-storage-v1");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build");
        let session = test_session(app.handle(), "/project/workbench-v1");
        fs::create_dir_all(&session.session_dir).unwrap();
        let runtime = WorkbenchRuntime::default();
        let before = runtime.read(&session).unwrap();
        let snapshot = runtime
            .apply(
                &session,
                &WorkbenchIdentity {
                    expected_project_root: before.project_root.clone(),
                    expected_runtime_session_id: before.runtime_session_id.clone(),
                    expected_revision: before.revision,
                },
                WorkbenchIntent::ConfigureSynchronizedSplit {
                    split: WorkbenchSplit::Vertical,
                    relative_path: "templates/index.html".to_string(),
                    secondary_surface: WorkbenchSurface::Code,
                    presentation: WorkbenchDocumentPresentation::Html,
                },
            )
            .unwrap()
            .snapshot;
        let mut legacy = serde_json::to_value(snapshot).unwrap();
        legacy["schemaVersion"] = serde_json::json!(1);
        for group in legacy["groups"].as_array_mut().unwrap() {
            for document in group["documents"].as_array_mut().unwrap() {
                document.as_object_mut().unwrap().remove("presentation");
            }
        }
        fs::write(
            PathBuf::from(&session.session_dir).join(WORKBENCH_STATE_FILE),
            format!("{}\n", serde_json::to_string_pretty(&legacy).unwrap()),
        )
        .unwrap();

        let restored = read_persisted_workbench(&session).unwrap().unwrap();

        assert_eq!(restored.schema_version, WORKBENCH_SCHEMA_VERSION);
        assert_eq!(restored.split, WorkbenchSplit::Vertical);
        assert!(restored.groups.iter().all(|group| group
            .documents
            .iter()
            .all(|document| document.presentation == WorkbenchDocumentPresentation::Html)));
        assert_eq!(restored.active_group_id, WorkbenchGroupId::Secondary);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn schema_two_snapshot_advances_to_the_project_settings_schema() {
        let session = detached_test_session(
            "workbench-v2",
            "/project/workbench-v2",
            "/tmp/project-workbench-v2",
        );
        fs::create_dir_all(&session.session_dir).unwrap();
        let mut legacy = WorkbenchRuntime::default().read(&session).unwrap();
        legacy.schema_version = 2;
        fs::write(
            PathBuf::from(&session.session_dir).join(WORKBENCH_STATE_FILE),
            format!("{}\n", serde_json::to_string_pretty(&legacy).unwrap()),
        )
        .unwrap();

        let restored = read_persisted_workbench(&session).unwrap().unwrap();

        assert_eq!(restored.schema_version, WORKBENCH_SCHEMA_VERSION);
        fs::remove_dir_all(&session.session_dir).unwrap();
    }

    #[test]
    fn persisted_workbench_rejects_foreign_project_identity() {
        let session = detached_test_session("project-a", "/project/a", "/tmp/project-a");
        let foreign = detached_test_session("project-b", "/project/b", "/tmp/project-b");
        let snapshot = WorkbenchRuntime::default().read(&foreign).unwrap();

        let error = require_persisted_identity(&session, &snapshot).unwrap_err();

        assert!(error.contains("altei sesiuni/proiect"));
    }

    #[test]
    fn latest_persistence_never_overwrites_a_newer_revision() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("workbench-latest-persistence");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build");
        ensure_app_home(app.handle()).unwrap();
        app.state::<WriteAuthorityRuntime>()
            .boot_recovery()
            .unwrap();
        let session = test_session(app.handle(), "/project/workbench-latest");
        fs::create_dir_all(&session.session_dir).unwrap();
        let mut newer = WorkbenchRuntime::default().read(&session).unwrap();
        newer.revision = 2;
        let mut stale = newer.clone();
        stale.revision = 1;

        persist_latest_workbench(app.handle(), &session, &newer).unwrap();
        persist_latest_workbench(app.handle(), &session, &stale).unwrap();

        let restored = read_persisted_workbench(&session).unwrap().unwrap();
        assert_eq!(restored.revision, newer.revision);
        fs::remove_dir_all(root).unwrap();
    }

    fn test_session<R: Runtime>(app: &AppHandle<R>, project_root: &str) -> ProjectSessionSnapshot {
        let id = crate::app_home::project_session_id(project_root);
        let session_dir = project_session_dir(app, project_root).unwrap();
        detached_test_session(&id, project_root, &session_dir.to_string_lossy())
    }

    fn detached_test_session(
        id: &str,
        project_root: &str,
        session_dir: &str,
    ) -> ProjectSessionSnapshot {
        ProjectSessionSnapshot {
            schema_version: 1,
            id: id.to_string(),
            project_root: project_root.to_string(),
            zola_root: project_root.to_string(),
            session_dir: session_dir.to_string(),
            manifest_path: PathBuf::from(session_dir)
                .join("manifest.json")
                .to_string_lossy()
                .into_owned(),
            opened_at_ms: 10,
            last_seen_at_ms: 10,
            root_fingerprint: ProjectRootFingerprint {
                canonical_path: project_root.to_string(),
                modified_ms: 0,
                size: 0,
                readonly: false,
                unix_device: None,
                unix_inode: None,
            },
            scan_summary: ProjectSessionScanSummary {
                active_theme: None,
                file_count: 1,
                directory_count: 1,
            },
        }
    }

    fn unique_test_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!("pana-studio-{name}-{stamp}"))
    }

    struct TestEnvGuard {
        previous_values: Vec<(&'static str, Option<String>)>,
    }

    impl TestEnvGuard {
        fn from_root(root: &Path) -> Self {
            let bindings = [
                ("XDG_CONFIG_HOME", root.join("config")),
                ("XDG_DATA_HOME", root.join("data")),
                ("XDG_CACHE_HOME", root.join("cache")),
                ("XDG_STATE_HOME", root.join("state")),
            ];
            let previous_values = bindings
                .iter()
                .map(|(key, _)| (*key, env::var(key).ok()))
                .collect::<Vec<_>>();
            for (key, path) in bindings {
                env::set_var(key, path);
            }
            Self { previous_values }
        }
    }

    impl Drop for TestEnvGuard {
        fn drop(&mut self) {
            for (key, value) in self.previous_values.drain(..) {
                match value {
                    Some(previous) => env::set_var(key, previous),
                    None => env::remove_var(key),
                }
            }
        }
    }
}
