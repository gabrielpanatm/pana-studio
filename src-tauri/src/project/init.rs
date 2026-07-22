use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager, Runtime};

use crate::kernel::write_authority::{
    ProjectBootstrapLease, WriteAuthority, WriteCategory, WriteIntent, WriteOperationKind,
    WriteOwner, WritePolicy,
};

use super::{starter::apply_starter, zola_config::normalize_zola_config_after_init};

pub fn init_project_with_starter<R: Runtime>(
    app: &AppHandle<R>,
    root: &Path,
) -> Result<String, String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("Nu am putut rezolva folderul: {error}"))?;
    ensure_empty_directory(&root)?;
    let bootstrap = ProjectBootstrapLease::capture(&root)?;
    bootstrap.verify_path_binding()?;

    // The selected directory is already the canonical Zola root. Every file
    // is published through the descriptor-bound bootstrap authority; no
    // transient CLI scaffold is created and then deleted.
    let publication = apply_starter(app, &bootstrap, &root, "pana-basic")
        .and_then(|()| normalize_zola_config_after_init(app, &bootstrap, &root));
    if let Err(error) = publication {
        let rollback = rollback_project_initialization(app, &bootstrap, &root);
        return Err(match rollback {
            Ok(()) => format!(
                "Inițializarea proiectului a eșuat și publicația parțială a fost retrasă: {error}"
            ),
            Err(rollback_error) => format!(
                "Inițializarea proiectului a eșuat ({error}), iar rollback-ul WriteAuthority necesită atenție: {rollback_error}"
            ),
        });
    }
    bootstrap.verify_path_binding()?;

    Ok("OK Proiect Zola inițializat direct cu starterul Pană Studio, exclusiv prin WriteAuthority."
        .to_string())
}

fn rollback_project_initialization<R: Runtime>(
    app: &AppHandle<R>,
    bootstrap: &ProjectBootstrapLease,
    root: &Path,
) -> Result<(), String> {
    let mut entries = root
        .read_dir()
        .map_err(|error| format!("Rollback-ul nu a putut enumera proiectul: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Rollback-ul nu a putut citi o intrare: {error}"))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let target = entry.path();
        let metadata = std::fs::symlink_metadata(&target).map_err(|error| {
            format!(
                "Rollback-ul nu a putut inspecta {}: {error}",
                target.display()
            )
        })?;
        let operation = if metadata.is_dir() && !metadata.file_type().is_symlink() {
            WriteOperationKind::RemoveDirectoryTree
        } else {
            WriteOperationKind::RemoveFile
        };
        let label = format!(
            "starter/pana-basic/rollback/{}",
            entry.file_name().to_string_lossy()
        );
        let intent = WriteIntent::new(
            WriteCategory::ProjectSourceWrite,
            WriteOwner::ProjectInitializer,
            operation,
            bootstrap.target(&target, label)?,
            WritePolicy::project_creation_lifecycle(),
            "Project initializer transactional rollback",
        );
        let _receipt = match operation {
            WriteOperationKind::RemoveDirectoryTree => WriteAuthority::new(app)
                .remove_directory_tree_if_exists(intent)
                .map_err(|error| error.into_terminal_diagnostic())?,
            WriteOperationKind::RemoveFile => WriteAuthority::new(app)
                .remove_file_if_exists(intent)
                .map_err(|error| error.into_terminal_diagnostic())?,
            _ => unreachable!(),
        };
    }
    bootstrap.verify_path_binding()
}

fn ensure_empty_directory(root: &Path) -> Result<(), String> {
    if !root.is_dir() {
        return Err(format!("Path-ul nu este folder: {}", root.display()));
    }
    let has_entries = root
        .read_dir()
        .map_err(|error| format!("Nu am putut citi folderul: {error}"))?
        .next()
        .is_some();
    if has_entries {
        return Err(
            "Inițializarea este permisă doar într-un dosar gol. Alege un dosar gol sau un proiect Zola existent."
                .to_string(),
        );
    }
    Ok(())
}

pub fn starter_resource_candidates<R: Runtime>(
    app: &AppHandle<R>,
    starter_name: &str,
) -> Vec<PathBuf> {
    let mut candidates = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("starters")
        .join(starter_name)];
    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("resources/starters").join(starter_name));
        candidates.push(resource_dir.join("starters").join(starter_name));
        candidates.push(
            resource_dir
                .join("src-tauri/resources/starters")
                .join(starter_name),
        );
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_home::{ensure_app_home, TEST_APP_ENV_LOCK};
    use std::{
        env, fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn project_initialization_creates_the_embedded_zola_site_directly() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = temp_dir("direct-zola-root");
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let app_handle = app.handle().clone();
        ensure_app_home(&app_handle).unwrap();

        let log = init_project_with_starter(&app_handle, &project).unwrap();

        assert!(log.contains("WriteAuthority"));
        assert!(project.join("zola.toml").is_file());
        assert!(project.join("content/_index.md").is_file());
        assert!(project.join("templates/index.html").is_file());
        assert!(!project.join("sursa").exists());
        assert!(!project.join("export").exists());
        cleanup(root);
    }

    #[test]
    fn project_initialization_refuses_a_non_empty_directory_without_changes() {
        let root = temp_dir("non-empty");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("keep.txt"), "keep").unwrap();
        assert!(ensure_empty_directory(&root).is_err());
        assert_eq!(fs::read_to_string(root.join("keep.txt")).unwrap(), "keep");
        cleanup(root);
    }

    #[test]
    fn transactional_rollback_removes_every_initializer_publication() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let fixture = temp_dir("rollback");
        let root = fixture.join("project");
        fs::create_dir_all(root.join("content")).unwrap();
        fs::write(root.join("content/_index.md"), "partial").unwrap();
        fs::write(root.join("zola.toml"), "base_url = '/'\n").unwrap();
        let _env_guard = TestEnvGuard::from_root(&fixture.join("app-home"));
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let app_handle = app.handle().clone();
        ensure_app_home(&app_handle).unwrap();
        let bootstrap = ProjectBootstrapLease::capture(&root).unwrap();

        rollback_project_initialization(&app_handle, &bootstrap, &root).unwrap();

        assert!(fs::read_dir(&root).unwrap().next().is_none());
        cleanup(fixture);
    }

    fn temp_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pana-init-{label}-{}-{stamp}", std::process::id()))
    }

    fn cleanup(path: PathBuf) {
        let _ = fs::remove_dir_all(path);
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
                .collect();
            for (key, path) in bindings {
                env::set_var(key, path);
            }
            Self { previous_values }
        }
    }

    impl Drop for TestEnvGuard {
        fn drop(&mut self) {
            for (key, value) in &self.previous_values {
                match value {
                    Some(value) => env::set_var(key, value),
                    None => env::remove_var(key),
                }
            }
        }
    }
}
