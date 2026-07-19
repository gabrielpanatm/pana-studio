use std::path::Path;

use tauri::{AppHandle, Runtime};

use crate::kernel::write_authority::{
    ProjectBootstrapLease, WriteAuthority, WriteCategory, WriteIntent, WriteOperationKind,
    WriteOwner, WritePolicy,
};

pub fn normalize_zola_config_after_init<R: Runtime>(
    app: &AppHandle<R>,
    bootstrap: &ProjectBootstrapLease,
    root: &Path,
) -> Result<(), String> {
    let zola = root.join("zola.toml");
    let config = root.join("config.toml");

    if zola.is_file() && config.is_file() {
        let intent = WriteIntent::new(
            WriteCategory::ProjectSourceWrite,
            WriteOwner::ProjectInitializer,
            WriteOperationKind::RemoveFile,
            bootstrap.target(config.clone(), "zola-init-config/config.toml")?,
            WritePolicy::project_creation_lifecycle(),
            "Project initializer Zola config normalization",
        );
        WriteAuthority::new(app)
            .remove_file_if_exists(intent)
            .map_err(|error| error.into_terminal_diagnostic())?;
    }

    if !zola.is_file() && !config.is_file() {
        return Err("Inițializarea nu a creat zola.toml sau config.toml.".to_string());
    }

    Ok(())
}
