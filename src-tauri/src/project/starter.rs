use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use tauri::{AppHandle, Runtime};

use crate::kernel::write_authority::{
    ProjectBootstrapLease, WriteAuthority, WriteCategory, WriteIntent, WriteOperationKind,
    WriteOwner, WritePolicy,
};

use super::init::starter_resource_candidates;

pub fn apply_starter<R: Runtime>(
    app: &AppHandle<R>,
    bootstrap: &ProjectBootstrapLease,
    project_root: &Path,
    starter_name: &str,
) -> Result<(), String> {
    let starter_root = resolve_starter_root(app, starter_name)?;
    copy_dir_recursive(
        app,
        &starter_root,
        project_root,
        bootstrap,
        project_root,
        &format!("starter/{starter_name}"),
    )
}

fn resolve_starter_root<R: Runtime>(
    app: &AppHandle<R>,
    starter_name: &str,
) -> Result<PathBuf, String> {
    starter_resource_candidates(app, starter_name)
        .into_iter()
        .find(|candidate| candidate.is_dir())
        .ok_or_else(|| {
            format!(
                "Nu am găsit starterul `{}` în resursele aplicației.",
                starter_name
            )
        })
}

fn copy_dir_recursive<R: Runtime>(
    app: &AppHandle<R>,
    source: &Path,
    destination: &Path,
    bootstrap: &ProjectBootstrapLease,
    label_boundary: &Path,
    label_root: &str,
) -> Result<(), String> {
    // `bootstrap.root()` este authority root deja existent și sigilat de
    // ProjectBootstrapLease. Directory direct operează exclusiv pe un leaf;
    // numai descendenții sunt publicați prin CreateDirectory.
    if destination != bootstrap.root() {
        create_project_initializer_directory(
            app,
            bootstrap,
            label_boundary,
            destination,
            label_root,
        )?;
    }

    for entry in fs::read_dir(source)
        .map_err(|error| format!("Nu am putut citi starterul {}: {}", source.display(), error))?
    {
        let entry =
            entry.map_err(|error| format!("Nu am putut citi o intrare starter: {}", error))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("Nu am putut citi tipul intrării starter: {}", error))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(
                app,
                &source_path,
                &destination_path,
                bootstrap,
                label_boundary,
                label_root,
            )?;
        } else if file_type.is_file() {
            copy_project_initializer_file(
                app,
                bootstrap,
                label_boundary,
                &source_path,
                &destination_path,
                label_root,
            )?;
        } else {
            return Err(format!(
                "Intrare starter nesuportată: {}",
                source_path.display()
            ));
        }
    }

    Ok(())
}

fn create_project_initializer_directory<R: Runtime>(
    app: &AppHandle<R>,
    bootstrap: &ProjectBootstrapLease,
    label_boundary: &Path,
    target: &Path,
    label_root: &str,
) -> Result<(), String> {
    let intent = project_initializer_intent(
        bootstrap,
        label_boundary,
        target,
        label_root,
        WriteOperationKind::CreateDirectory,
        "Project initializer directory",
    )?;
    WriteAuthority::new(app)
        .create_directory_all(intent)
        .map_err(|error| error.into_terminal_diagnostic())?;
    Ok(())
}

fn copy_project_initializer_file<R: Runtime>(
    app: &AppHandle<R>,
    bootstrap: &ProjectBootstrapLease,
    label_boundary: &Path,
    source: &Path,
    target: &Path,
    label_root: &str,
) -> Result<(), String> {
    match fs::symlink_metadata(target) {
        Ok(_) => {
            return Err(format!(
                "Initializer proiect blocat: destinația {} există deja.",
                initializer_label(label_boundary, target, label_root)
            ));
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "Nu am putut verifica destinația {} înainte de copiere: {}",
                target.display(),
                error
            ));
        }
    }

    let intent = project_initializer_intent(
        bootstrap,
        label_boundary,
        target,
        label_root,
        WriteOperationKind::Copy,
        "Project initializer resource copy",
    )?;
    WriteAuthority::new(app)
        .copy_file(intent, source)
        .map_err(|error| error.into_terminal_diagnostic())?;
    Ok(())
}

fn project_initializer_intent(
    bootstrap: &ProjectBootstrapLease,
    label_boundary: &Path,
    target: &Path,
    label_root: &str,
    operation: WriteOperationKind,
    description: &str,
) -> Result<WriteIntent, String> {
    Ok(WriteIntent::new(
        WriteCategory::ProjectSourceWrite,
        WriteOwner::ProjectInitializer,
        operation,
        bootstrap.target(
            target.to_path_buf(),
            initializer_label(label_boundary, target, label_root),
        )?,
        WritePolicy::project_creation_lifecycle(),
        description,
    ))
}

fn initializer_label(boundary_root: &Path, target: &Path, label_root: &str) -> String {
    target
        .strip_prefix(boundary_root)
        .ok()
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .filter(|relative| !relative.is_empty())
        .map(|relative| format!("{label_root}/{relative}"))
        .unwrap_or_else(|| format!("{label_root}/root"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_starter_is_a_direct_zola_root_with_native_output_default() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/starters/pana-basic");
        assert!(root.join("zola.toml").is_file());
        assert!(root.join("content").is_dir());
        assert!(root.join("templates").is_dir());
        assert!(!root.join("sursa").exists());
        assert!(!root.join("export").exists());

        let config = fs::read_to_string(root.join("zola.toml")).unwrap();
        assert!(!config
            .lines()
            .any(|line| line.trim_start().starts_with("output_dir")));
        let gitignore = fs::read_to_string(root.join(".gitignore")).unwrap();
        assert!(gitignore.lines().any(|line| line == "/public/"));
        assert!(gitignore.lines().any(|line| line == ".env"));
    }

    #[test]
    fn project_initializer_directory_intents_have_target_specific_labels() {
        let root = std::env::temp_dir().join(format!("pana-project-intent-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let bootstrap = ProjectBootstrapLease::capture(&root).unwrap();

        let root_intent = project_initializer_intent(
            &bootstrap,
            &root,
            &root,
            "starter/pana-basic",
            WriteOperationKind::CreateDirectory,
            "directory",
        )
        .unwrap();
        assert_eq!(root_intent.target.public_label, "starter/pana-basic/root");

        let source_intent = project_initializer_intent(
            &bootstrap,
            &root,
            &root.join("templates"),
            "starter/pana-basic",
            WriteOperationKind::CreateDirectory,
            "directory",
        )
        .unwrap();
        assert_eq!(
            source_intent.target.public_label,
            "starter/pana-basic/templates"
        );
        assert_eq!(source_intent.category, WriteCategory::ProjectSourceWrite);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn starter_recursion_reuses_bootstrap_root_and_creates_only_descendants() {
        let _env_lock = crate::app_home::TEST_APP_ENV_LOCK.lock().unwrap();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixture =
            std::env::temp_dir().join(format!("pana-starter-root-{}-{nonce}", std::process::id()));
        let source = fixture.join("template");
        let destination = fixture.join("project");
        fs::create_dir_all(source.join("templates")).unwrap();
        fs::write(source.join("templates/index.html"), b"<main></main>").unwrap();
        fs::create_dir_all(&destination).unwrap();

        let _env = TestEnvGuard::from_root(&fixture.join("app-home"));
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");
        crate::app_home::ensure_app_home(app.handle()).unwrap();
        let bootstrap = ProjectBootstrapLease::capture(&destination).unwrap();

        copy_dir_recursive(
            app.handle(),
            &source,
            &destination,
            &bootstrap,
            &destination,
            "starter/pana-basic",
        )
        .unwrap();

        assert_eq!(
            fs::read(destination.join("templates/index.html")).unwrap(),
            b"<main></main>"
        );
        drop(app);
        fs::remove_dir_all(fixture).unwrap();
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
                .map(|(key, _)| (*key, std::env::var(key).ok()))
                .collect::<Vec<_>>();
            for (key, path) in bindings {
                std::env::set_var(key, path);
            }
            Self { previous_values }
        }
    }

    impl Drop for TestEnvGuard {
        fn drop(&mut self) {
            for (key, value) in &self.previous_values {
                if let Some(value) = value {
                    std::env::set_var(key, value);
                } else {
                    std::env::remove_var(key);
                }
            }
        }
    }
}
