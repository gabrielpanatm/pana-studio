use std::{fs, path::Path};

use tauri::AppHandle;

use crate::{
    app_home::{app_config_path, project_app_config_path, projects_config_dir},
    commands::config::model::{GlobalAppConfig, ProjectAppConfig, ProjectAppConfigInput},
    kernel::write_authority::{
        WriteAuthority, WriteCategory, WriteIntent, WriteOperationKind, WriteOwner, WritePolicy,
        WriteTarget,
    },
};

pub(crate) fn read_project_app_config_for_root(
    app: &AppHandle,
    root: &Path,
) -> Result<ProjectAppConfig, String> {
    let project_path = canonical_project_path(root);
    let path = project_app_config_path(app, &project_path)?;
    if !path.exists() {
        return Ok(default_project_app_config(project_path));
    }

    let source = fs::read_to_string(&path)
        .map_err(|e| format!("Nu am putut citi configurația locală Pană Studio: {}", e))?;
    let mut config: ProjectAppConfig = serde_json::from_str(&source)
        .map_err(|e| format!("Configurația locală Pană Studio este invalidă: {}", e))?;
    config.project_path = project_path;
    Ok(config)
}

pub(super) fn project_app_config_from_input(
    root: &Path,
    config: ProjectAppConfigInput,
) -> ProjectAppConfig {
    ProjectAppConfig {
        project_path: canonical_project_path(root),
        cachebust_assets: config.cachebust_assets,
    }
}

pub(super) fn write_project_app_config_for_root(
    app: &AppHandle,
    _root: &Path,
    stored: ProjectAppConfig,
) -> Result<ProjectAppConfig, String> {
    let project_path = stored.project_path.clone();
    let global_path = app_config_path(app)?;
    if !global_path.exists() {
        let global = serde_json::to_string_pretty(&GlobalAppConfig::default())
            .map_err(|e| format!("Nu am putut serializa config-ul Pană Studio: {}", e))?;
        let boundary = global_path
            .parent()
            .ok_or_else(|| "Config-ul Pană Studio nu are folder părinte.".to_string())?
            .to_path_buf();
        write_internal_config(
            app,
            global_path,
            boundary,
            "config/config.json",
            "Scriere config global Pană Studio",
            format!("{}\n", global),
        )?;
    }

    let body = serde_json::to_string_pretty(&stored)
        .map_err(|e| format!("Nu am putut serializa config-ul proiectului: {}", e))?;
    let projects_root = projects_config_dir(app)?;
    let project_config_path = project_app_config_path(app, &project_path)?;
    let project_config_label = format!(
        "config/projects/{}",
        project_config_path
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .unwrap_or("project.json")
    );
    write_internal_config(
        app,
        project_config_path,
        projects_root,
        project_config_label,
        "Scriere config local proiect Pană Studio",
        format!("{}\n", body),
    )?;
    Ok(stored)
}

fn default_project_app_config(project_path: String) -> ProjectAppConfig {
    ProjectAppConfig {
        project_path,
        cachebust_assets: false,
    }
}

fn write_internal_config(
    app: &AppHandle,
    path: impl Into<std::path::PathBuf>,
    boundary: impl Into<std::path::PathBuf>,
    public_label: impl Into<String>,
    description: impl Into<String>,
    contents: String,
) -> Result<(), String> {
    let target = WriteTarget::new(path, boundary, public_label);
    let intent = WriteIntent::new(
        WriteCategory::InternalAppWrite,
        WriteOwner::AppConfig,
        WriteOperationKind::WriteText,
        target,
        WritePolicy::internal_atomic(),
        description,
    );
    WriteAuthority::new(app)
        .write_text(intent, &contents)
        .map_err(|error| error.into_terminal_diagnostic())
        .map(|_| ())
}

fn canonical_project_path(root: &Path) -> String {
    fs::canonicalize(root)
        .unwrap_or_else(|_| root.to_path_buf())
        .to_string_lossy()
        .to_string()
}
