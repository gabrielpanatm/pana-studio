use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use tauri::{AppHandle, Manager, Runtime};

use crate::kernel::write_authority::{
    capability_capture_subprocess_directory, ProjectBootstrapLease, WriteAuthority, WriteCategory,
    WriteIntent, WriteOperationKind, WriteOwner, WritePolicy,
};

use super::{
    starter::{apply_project_template, apply_starter},
    zola_config::normalize_zola_config_after_init,
};

const ZOLA_INIT_TIMEOUT: Duration = Duration::from_secs(30);

pub fn init_project_with_starter<R: Runtime>(
    app: &AppHandle<R>,
    zola_binary: &Path,
    root: &Path,
) -> Result<String, String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("Nu am putut rezolva folderul: {}", error))?;
    ensure_empty_directory(&root)?;
    let bootstrap = ProjectBootstrapLease::capture(&root)?;
    bootstrap.verify_path_binding()?;

    apply_project_template(app, &bootstrap)?;

    let zola_root = root.join("sursa");
    create_project_initializer_directory(
        app,
        &bootstrap,
        &zola_root,
        "project-template/sursa",
        "Project initializer Zola root",
    )?;
    ensure_empty_directory(&zola_root)?;

    let mut log = run_zola_init(zola_binary, &bootstrap, &zola_root)?;
    remove_zola_init_scaffold(app, &bootstrap, &zola_root)?;
    apply_starter(app, &bootstrap, &zola_root, "pana-basic")?;
    normalize_zola_config_after_init(app, &bootstrap, &zola_root)?;

    if !log.is_empty() {
        log.push('\n');
    }
    log.push_str("Template proiect Pană Studio și starter Zola aplicate.");
    Ok(log)
}

fn ensure_empty_directory(root: &Path) -> Result<(), String> {
    if !root.is_dir() {
        return Err(format!("Path-ul nu este folder: {}", root.display()));
    }

    let has_entries = root
        .read_dir()
        .map_err(|error| format!("Nu am putut citi folderul: {}", error))?
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

fn run_zola_init(
    binary: &Path,
    bootstrap: &ProjectBootstrapLease,
    root: &Path,
) -> Result<String, String> {
    let directory =
        capability_capture_subprocess_directory(bootstrap, root, "project-initializer/zola-cwd")
            .map_err(|error| {
                format!(
                    "Nu am putut captura directorul stabil pentru zola init: {}",
                    error.into_terminal_diagnostic()
                )
            })?;
    directory.require_empty().map_err(|error| {
        format!(
            "zola init a fost blocat deoarece directorul capturat nu mai este gol: {}",
            error.into_terminal_diagnostic()
        )
    })?;
    let descriptor_current_dir = directory.current_dir_path();
    let spawn_result = Command::new(binary)
        .arg("init")
        .arg(".")
        .current_dir(&descriptor_current_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    // `Command::spawn` resolves current_dir in the child before exec. Keep the
    // descriptor alive through that handshake, then the child can continue on
    // the captured inode with `.` even if the original pathname is replaced.
    drop(directory);
    let mut child =
        spawn_result.map_err(|error| format!("Nu am putut porni zola init: {}", error))?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(b"\n\n\n\n\n");
    }

    let deadline = Instant::now() + ZOLA_INIT_TIMEOUT;
    loop {
        if child
            .try_wait()
            .map_err(|error| format!("Nu am putut verifica zola init: {}", error))?
            .is_some()
        {
            break;
        }

        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "zola init nu a terminat în {} secunde și a fost oprit.",
                ZOLA_INIT_TIMEOUT.as_secs()
            ));
        }

        std::thread::sleep(Duration::from_millis(100));
    }

    let output = child
        .wait_with_output()
        .map_err(|error| format!("zola init a eșuat: {}", error))?;
    let log = command_log(&output.stdout, &output.stderr);

    if output.status.success() {
        Ok(log)
    } else {
        Err(format!("Eroare zola init:\n{}", log))
    }
}

fn remove_zola_init_scaffold<R: Runtime>(
    app: &AppHandle<R>,
    bootstrap: &ProjectBootstrapLease,
    root: &Path,
) -> Result<(), String> {
    for directory in ["content", "templates", "sass", "static", "themes"] {
        let path = root.join(directory);
        if path.exists() {
            remove_project_initializer_directory_tree(
                app,
                bootstrap,
                &path,
                &format!("zola-init-scaffold/{directory}"),
                "Project initializer Zola scaffold cleanup",
            )?;
        }
    }

    for file in ["zola.toml", "config.toml"] {
        let path = root.join(file);
        remove_project_initializer_file_if_exists(
            app,
            bootstrap,
            &path,
            &format!("zola-init-scaffold/{file}"),
            "Project initializer Zola scaffold config cleanup",
        )?;
    }

    Ok(())
}

fn create_project_initializer_directory<R: Runtime>(
    app: &AppHandle<R>,
    bootstrap: &ProjectBootstrapLease,
    target: &Path,
    label: &str,
    description: &str,
) -> Result<(), String> {
    let intent = project_initializer_intent(
        bootstrap,
        target,
        label,
        WriteOperationKind::CreateDirectory,
        description,
    )?;
    WriteAuthority::new(app)
        .create_directory_all(intent)
        .map_err(|error| error.into_terminal_diagnostic())?;
    Ok(())
}

fn remove_project_initializer_directory_tree<R: Runtime>(
    app: &AppHandle<R>,
    bootstrap: &ProjectBootstrapLease,
    target: &Path,
    label: &str,
    description: &str,
) -> Result<(), String> {
    let intent = project_initializer_intent(
        bootstrap,
        target,
        label,
        WriteOperationKind::RemoveDirectoryTree,
        description,
    )?;
    WriteAuthority::new(app)
        .remove_directory_tree_if_exists(intent)
        .map_err(|error| error.into_terminal_diagnostic())?;
    Ok(())
}

fn remove_project_initializer_file_if_exists<R: Runtime>(
    app: &AppHandle<R>,
    bootstrap: &ProjectBootstrapLease,
    target: &Path,
    label: &str,
    description: &str,
) -> Result<(), String> {
    let intent = project_initializer_intent(
        bootstrap,
        target,
        label,
        WriteOperationKind::RemoveFile,
        description,
    )?;
    WriteAuthority::new(app)
        .remove_file_if_exists(intent)
        .map_err(|error| error.into_terminal_diagnostic())?;
    Ok(())
}

fn project_initializer_intent(
    bootstrap: &ProjectBootstrapLease,
    target: &Path,
    label: &str,
    operation: WriteOperationKind,
    description: &str,
) -> Result<WriteIntent, String> {
    Ok(WriteIntent::new(
        WriteCategory::ProjectSourceWrite,
        WriteOwner::ProjectInitializer,
        operation,
        bootstrap.target(target.to_path_buf(), label)?,
        WritePolicy::project_creation_lifecycle(),
        description,
    ))
}

fn command_log(stdout: &[u8], stderr: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);
    format!("{}{}", stdout, stderr).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::{remove_zola_init_scaffold, run_zola_init};
    use crate::app_home::{ensure_app_home, TEST_APP_ENV_LOCK};
    use crate::kernel::write_authority::{
        test_support::with_before_remove_tree_traversal_hook_for_test, ProjectBootstrapLease,
    };
    use std::{
        env, fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("pana-studio-{name}-{stamp}"))
    }

    #[test]
    fn removes_default_zola_scaffold_before_starter_overlay() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = temp_dir("zola-scaffold");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");
        let app_handle = app.handle().clone();
        ensure_app_home(&app_handle).expect("test app home should be available");

        fs::create_dir_all(root.join("templates")).expect("create templates");
        fs::create_dir_all(root.join("themes")).expect("create themes");
        fs::create_dir_all(root.join("content")).expect("create content");
        fs::write(root.join("templates/index.html"), "").expect("write template");
        fs::write(root.join("themes/README.md"), "").expect("write theme readme");
        fs::write(root.join("zola.toml"), "base_url = \"http://example.test\"")
            .expect("write zola config");
        fs::write(
            root.join("config.toml"),
            "base_url = \"http://example.test\"",
        )
        .expect("write legacy config");

        let bootstrap = ProjectBootstrapLease::capture(&root).expect("capture bootstrap root");
        remove_zola_init_scaffold(&app_handle, &bootstrap, &root).expect("remove scaffold");

        assert!(!root.join("templates").exists());
        assert!(!root.join("themes").exists());
        assert!(!root.join("content").exists());
        assert!(!root.join("zola.toml").exists());
        assert!(!root.join("config.toml").exists());

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn scaffold_cleanup_stops_on_remove_tree_recovery_without_later_cleanup() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = temp_dir("zola-scaffold-tree-recovery");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");
        let app_handle = app.handle().clone();
        ensure_app_home(&app_handle).expect("test app home should be available");

        fs::create_dir_all(root.join("content/nested")).expect("create content");
        fs::write(root.join("content/nested/page.md"), "planned").expect("write content");
        fs::create_dir_all(root.join("templates")).expect("create templates");
        fs::write(root.join("templates/index.html"), "later").expect("write template");
        let bootstrap = ProjectBootstrapLease::capture(&root).expect("capture bootstrap root");
        let hook_root = root.clone();

        let result = with_before_remove_tree_traversal_hook_for_test(
            move || {
                let quarantine = fs::read_dir(&hook_root)
                    .unwrap()
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .find(|path| {
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .is_some_and(|name| name.contains("remove-tree-quarantine"))
                    })
                    .expect("content quarantine should exist");
                fs::write(quarantine.join("competitor.md"), "competitor").unwrap();
            },
            || remove_zola_init_scaffold(&app_handle, &bootstrap, &root),
        );

        let error = result.unwrap_err();
        assert!(error.to_lowercase().contains("recovery"), "{error}");
        assert!(root.join("templates/index.html").exists());
        let quarantine = fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains("remove-tree-quarantine"))
            })
            .expect("content quarantine should remain hot");
        assert_eq!(
            fs::read_to_string(quarantine.join("competitor.md")).unwrap(),
            "competitor"
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn zola_init_uses_captured_directory_as_cwd_and_dot_argument() {
        let root = temp_dir("zola-captured-cwd");
        let project = root.join("project");
        let fake_zola = root.join("fake-zola.sh");
        fs::create_dir_all(&project).expect("create project root");
        fs::write(
            &fake_zola,
            "#!/bin/sh\nprintf '%s|%s' \"$1\" \"$2\" > invocation.txt\n",
        )
        .expect("write fake zola");
        let mut permissions = fs::metadata(&fake_zola)
            .expect("fake zola metadata")
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&fake_zola, permissions).expect("make fake zola executable");

        let bootstrap = ProjectBootstrapLease::capture(&project).expect("capture bootstrap root");
        run_zola_init(&fake_zola, &bootstrap, &project).expect("run fake zola");

        assert_eq!(
            fs::read_to_string(project.join("invocation.txt")).expect("read invocation"),
            "init|."
        );
        fs::remove_dir_all(root).expect("cleanup");
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
            for (key, value) in &self.previous_values {
                if let Some(value) = value {
                    env::set_var(key, value);
                } else {
                    env::remove_var(key);
                }
            }
        }
    }
}

pub fn starter_resource_candidates<R: Runtime>(
    app: &AppHandle<R>,
    starter_name: &str,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("starters")
            .join(starter_name),
    );

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(
            resource_dir
                .join("resources")
                .join("starters")
                .join(starter_name),
        );
        candidates.push(resource_dir.join("starters").join(starter_name));
        candidates.push(
            resource_dir
                .join("src-tauri")
                .join("resources")
                .join("starters")
                .join(starter_name),
        );
    }

    candidates
}
