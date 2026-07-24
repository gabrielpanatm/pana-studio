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

pub fn activate_theme_after_init<R: Runtime>(
    app: &AppHandle<R>,
    bootstrap: &ProjectBootstrapLease,
    root: &Path,
    theme_id: &str,
) -> Result<(), String> {
    let config = ["zola.toml", "config.toml"]
        .into_iter()
        .map(|name| root.join(name))
        .find(|path| path.is_file())
        .ok_or_else(|| "Inițializarea nu conține o configurație Zola.".to_string())?;
    let source = std::fs::read_to_string(&config)
        .map_err(|error| format!("Nu am putut citi configurația inițială Zola: {error}"))?;
    let patched = crate::zola_theme::set_active_theme_in_source(&source, theme_id)?;
    let intent = WriteIntent::new(
        WriteCategory::ProjectSourceWrite,
        WriteOwner::ProjectInitializer,
        WriteOperationKind::WriteText,
        bootstrap.target(config, "theme-pack/activation/zola-config")?,
        WritePolicy::project_creation_write(),
        "Project initializer selected theme activation",
    );
    WriteAuthority::new(app)
        .write_text(intent, &patched)
        .map_err(|error| error.into_terminal_diagnostic())?;
    Ok(())
}
