use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime};

use crate::kernel::write_authority::{ApplicationAuthorityPaths, WriteAuthorityRuntime};

const APP_HOME_SCHEMA_VERSION: u32 = 1;

#[cfg(test)]
pub(crate) static TEST_APP_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppHomeSnapshot {
    pub schema_version: u32,
    pub identifier: String,
    pub config_dir: String,
    pub data_dir: String,
    pub cache_dir: String,
    pub log_dir: String,
    pub temp_dir: String,
    pub projects_config_dir: String,
    pub mcp_dir: String,
    pub sessions_dir: String,
    pub kernel_dir: String,
    pub write_authority_wal_dir: String,
    pub scratch_dir: String,
    pub preview_cache_dir: String,
    pub app_logs_dir: String,
}

pub fn ensure_app_home<R: Runtime>(app: &AppHandle<R>) -> Result<AppHomeSnapshot, String> {
    let snapshot = app_home_snapshot(app)?;
    if app.try_state::<WriteAuthorityRuntime>().is_none() {
        app.manage(WriteAuthorityRuntime::default());
    }
    app.state::<WriteAuthorityRuntime>()
        .install_application_home(application_authority_paths(&snapshot))?;
    Ok(snapshot)
}

pub fn app_home_snapshot<R: Runtime>(app: &AppHandle<R>) -> Result<AppHomeSnapshot, String> {
    let identifier = app.config().identifier.clone();
    if let Some(paths) = app
        .try_state::<WriteAuthorityRuntime>()
        .and_then(|runtime| runtime.application_paths())
    {
        let temp_dir = app.path().temp_dir().map_err(|error| {
            format!(
                "Nu am putut localiza folderul temporar Pană Studio: {}",
                error
            )
        })?;
        return Ok(snapshot_from_authority_paths(identifier, paths, temp_dir));
    }
    let config_dir = app.path().app_config_dir().map_err(|error| {
        format!(
            "Nu am putut localiza folderul de config Pană Studio: {}",
            error
        )
    })?;
    let data_dir = app.path().app_data_dir().map_err(|error| {
        format!(
            "Nu am putut localiza folderul de date Pană Studio: {}",
            error
        )
    })?;
    let cache_dir = app.path().app_cache_dir().map_err(|error| {
        format!(
            "Nu am putut localiza folderul de cache Pană Studio: {}",
            error
        )
    })?;
    let log_dir = app.path().app_log_dir().map_err(|error| {
        format!(
            "Nu am putut localiza folderul de log Pană Studio: {}",
            error
        )
    })?;
    let temp_dir = app.path().temp_dir().map_err(|error| {
        format!(
            "Nu am putut localiza folderul temporar Pană Studio: {}",
            error
        )
    })?;

    Ok(AppHomeSnapshot {
        schema_version: APP_HOME_SCHEMA_VERSION,
        identifier,
        projects_config_dir: path_to_string(config_dir.join("projects")),
        mcp_dir: path_to_string(config_dir.join("mcp")),
        sessions_dir: path_to_string(data_dir.join("sessions")),
        kernel_dir: path_to_string(data_dir.join("kernel")),
        write_authority_wal_dir: path_to_string(
            data_dir.join("kernel").join("write-authority-wal"),
        ),
        scratch_dir: path_to_string(cache_dir.join("scratch")),
        preview_cache_dir: path_to_string(cache_dir.join("preview")),
        app_logs_dir: path_to_string(log_dir.join("app")),
        config_dir: path_to_string(config_dir),
        data_dir: path_to_string(data_dir),
        cache_dir: path_to_string(cache_dir),
        log_dir: path_to_string(log_dir),
        temp_dir: path_to_string(temp_dir),
    })
}

fn application_authority_paths(snapshot: &AppHomeSnapshot) -> ApplicationAuthorityPaths {
    ApplicationAuthorityPaths {
        config_dir: PathBuf::from(&snapshot.config_dir),
        data_dir: PathBuf::from(&snapshot.data_dir),
        cache_dir: PathBuf::from(&snapshot.cache_dir),
        log_dir: PathBuf::from(&snapshot.log_dir),
        projects_config_dir: PathBuf::from(&snapshot.projects_config_dir),
        mcp_dir: PathBuf::from(&snapshot.mcp_dir),
        sessions_dir: PathBuf::from(&snapshot.sessions_dir),
        kernel_dir: PathBuf::from(&snapshot.kernel_dir),
        write_authority_wal_dir: PathBuf::from(&snapshot.write_authority_wal_dir),
        scratch_dir: PathBuf::from(&snapshot.scratch_dir),
        preview_cache_dir: PathBuf::from(&snapshot.preview_cache_dir),
        app_logs_dir: PathBuf::from(&snapshot.app_logs_dir),
    }
}

fn snapshot_from_authority_paths(
    identifier: String,
    paths: ApplicationAuthorityPaths,
    temp_dir: PathBuf,
) -> AppHomeSnapshot {
    AppHomeSnapshot {
        schema_version: APP_HOME_SCHEMA_VERSION,
        identifier,
        config_dir: path_to_string(paths.config_dir),
        data_dir: path_to_string(paths.data_dir),
        cache_dir: path_to_string(paths.cache_dir),
        log_dir: path_to_string(paths.log_dir),
        temp_dir: path_to_string(temp_dir),
        projects_config_dir: path_to_string(paths.projects_config_dir),
        mcp_dir: path_to_string(paths.mcp_dir),
        sessions_dir: path_to_string(paths.sessions_dir),
        kernel_dir: path_to_string(paths.kernel_dir),
        write_authority_wal_dir: path_to_string(paths.write_authority_wal_dir),
        scratch_dir: path_to_string(paths.scratch_dir),
        preview_cache_dir: path_to_string(paths.preview_cache_dir),
        app_logs_dir: path_to_string(paths.app_logs_dir),
    }
}

pub fn app_config_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    Ok(app_config_dir(app)?.join("config.json"))
}

pub fn projects_config_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    Ok(app_config_dir(app)?.join("projects"))
}

pub fn project_app_config_path<R: Runtime>(
    app: &AppHandle<R>,
    project_path: &str,
) -> Result<PathBuf, String> {
    Ok(projects_config_dir(app)?.join(format!("{:016x}.json", stable_path_hash(project_path))))
}

pub fn mcp_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    Ok(app_config_dir(app)?.join("mcp"))
}

pub fn mcp_context_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    Ok(mcp_dir(app)?.join("current-context.json"))
}

pub fn mcp_discovery_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    Ok(mcp_dir(app)?.join("mcp.json"))
}

pub fn project_session_id(project_root: &str) -> String {
    format!("{:016x}", stable_path_hash(project_root))
}

pub fn project_session_dir<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &str,
) -> Result<PathBuf, String> {
    Ok(PathBuf::from(app_home_snapshot(app)?.sessions_dir).join(project_session_id(project_root)))
}

pub fn project_session_manifest_path<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &str,
) -> Result<PathBuf, String> {
    Ok(project_session_dir(app, project_root)?.join("manifest.json"))
}

pub fn project_workspace_recovery_path<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &str,
) -> Result<PathBuf, String> {
    Ok(project_session_dir(app, project_root)?.join("project-workspace.json"))
}

pub fn project_open_recovery_decision_path<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &str,
) -> Result<PathBuf, String> {
    Ok(project_session_dir(app, project_root)?.join("project-open-recovery-decision.json"))
}

pub fn project_workspace_save_journal_dir<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &str,
) -> Result<PathBuf, String> {
    Ok(project_session_dir(app, project_root)?.join("project-workspace-save"))
}

fn stable_path_hash(path: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in path.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn app_config_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    Ok(PathBuf::from(app_home_snapshot(app)?.config_dir))
}

fn path_to_string(path: PathBuf) -> String {
    path.to_string_lossy().to_string()
}
