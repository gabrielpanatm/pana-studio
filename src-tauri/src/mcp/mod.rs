mod codex;
mod security;
mod server;
mod service;

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::{
    app_home::{
        mcp_context_path as app_home_mcp_context_path, mcp_dir as app_home_mcp_dir,
        mcp_discovery_path as app_home_mcp_discovery_path,
    },
    kernel::{
        context_hub::CanonicalAiContextSnapshot,
        observability::now_ms,
        write_authority::{
            WriteAuthority, WriteCategory, WriteIntent, WriteOperationKind, WriteOwner,
            WritePolicy, WriteTarget,
        },
    },
};

pub use codex::{
    configure_codex, load_or_generate_access_token, read_codex_status, CodexMcpStatus,
};
pub use server::{start_context_server, MCP_ENDPOINT, MCP_PORT};

const CONTEXT_FILE: &str = "current-context.json";
const DISCOVERY_FILE: &str = "mcp.json";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiContextStatus {
    pub context_path: String,
    pub discovery_path: String,
    pub endpoint: String,
    pub context_exists: bool,
    pub discovery_exists: bool,
    pub updated_at: Option<String>,
    pub mode: String,
    pub server_running: bool,
}

pub fn context_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app_home_mcp_dir(app)
}

pub fn current_context_path(app: &AppHandle) -> Result<PathBuf, String> {
    app_home_mcp_context_path(app)
}

pub fn discovery_path(app: &AppHandle) -> Result<PathBuf, String> {
    app_home_mcp_discovery_path(app)
}

pub fn recorded_server_process_id(app: &AppHandle) -> Option<u64> {
    let value: Value =
        serde_json::from_str(&fs::read_to_string(discovery_path(app).ok()?).ok()?).ok()?;
    value
        .get("serverRunning")
        .and_then(Value::as_bool)
        .filter(|running| *running)?;
    value.get("processId").and_then(Value::as_u64)
}

pub fn save_context_snapshot(
    app: &AppHandle,
    snapshot: &CanonicalAiContextSnapshot,
) -> Result<AiContextStatus, String> {
    let dir = context_dir(app)?;
    let context_path = current_context_path(app)?;
    let discovery_path = discovery_path(app)?;
    let server_running = server_running_in_process(app);
    write_context_descriptor(
        app,
        &dir,
        context_path,
        json!({
            "version": 2,
            "available": server_running,
            "lifecycle": if server_running { "live" } else { "server_unavailable" },
            "authoritativeSource": MCP_ENDPOINT,
            "authoritativeOnlyWhenHealthAuthenticated": true,
            "contextRevision": snapshot.context_revision,
            "coordinationRevision": snapshot.coordination.coordination_revision,
            "projectOpen": snapshot.core.project.is_open,
            "projectSessionId": snapshot.core.project.session_id,
            "updatedAtMs": snapshot.updated_at_ms
        }),
    )?;

    write_discovery_file(
        app,
        &dir,
        discovery_path,
        server_running,
        if server_running {
            "live"
        } else {
            "server_unavailable"
        },
        Some(snapshot.context_revision),
        Some(snapshot.coordination.coordination_revision),
        snapshot.core.project.session_id.as_deref(),
        snapshot.updated_at_ms,
    )?;
    read_context_status(app)
}

pub fn mark_context_server_lifecycle(
    app: &AppHandle,
    server_running: bool,
    lifecycle: &str,
) -> Result<(), String> {
    let dir = context_dir(app)?;
    let context_path = current_context_path(app)?;
    let discovery_path = discovery_path(app)?;
    let timestamp = now_ms();
    write_context_descriptor(
        app,
        &dir,
        context_path,
        json!({
            "version": 2,
            "available": false,
            "lifecycle": lifecycle,
            "authoritativeSource": MCP_ENDPOINT,
            "authoritativeOnlyWhenHealthAuthenticated": true,
            "updatedAtMs": timestamp
        }),
    )?;
    write_discovery_file(
        app,
        &dir,
        discovery_path,
        server_running,
        lifecycle,
        None,
        None,
        None,
        timestamp,
    )
}

pub fn read_context_status(app: &AppHandle) -> Result<AiContextStatus, String> {
    let context_path = current_context_path(app)?;
    let discovery_path = discovery_path(app)?;
    let updated_at = read_updated_at(&context_path);

    Ok(AiContextStatus {
        context_exists: context_path.is_file(),
        discovery_exists: discovery_path.is_file(),
        context_path: path_to_string(&context_path),
        discovery_path: path_to_string(&discovery_path),
        endpoint: MCP_ENDPOINT.to_string(),
        updated_at,
        mode: "authenticated_mcp_http_with_diagnostic_files".to_string(),
        server_running: server_running_in_process(app),
    })
}

fn write_discovery_file(
    app: &AppHandle,
    boundary: &Path,
    path: PathBuf,
    server_running: bool,
    lifecycle: &str,
    context_revision: Option<u64>,
    coordination_revision: Option<u64>,
    project_session_id: Option<&str>,
    updated_at_ms: u128,
) -> Result<(), String> {
    let discovery = json!({
        "version": 2,
        "enabled": true,
        "mode": "read_only_data_with_ram_coordination",
        "serverRunning": server_running,
        "lifecycle": lifecycle,
        "transport": {
            "current": "authenticated_mcp_http",
            "mcpEndpoint": MCP_ENDPOINT,
            "health": format!("http://127.0.0.1:{}/health", MCP_PORT),
            "context": format!("http://127.0.0.1:{}/context", MCP_PORT),
            "authentication": {
                "required": true,
                "header": security::ACCESS_TOKEN_HEADER,
                "credentialSource": "Codex config.toml http_headers"
            }
        },
        "diagnosticFilesAreAuthoritative": false,
        "contextPath": path.with_file_name(CONTEXT_FILE).to_string_lossy().to_string(),
        "projectSessionId": project_session_id,
        "contextRevision": context_revision,
        "coordinationRevision": coordination_revision,
        "updatedAtMs": updated_at_ms,
        "processId": std::process::id()
    });
    let body = serde_json::to_string_pretty(&discovery)
        .map_err(|error| format!("Nu am putut serializa discovery MCP: {}", error))?;
    write_mcp_file(
        app,
        boundary,
        path,
        format!("mcp/{DISCOVERY_FILE}"),
        "Scriere discovery MCP Pană Studio",
        format!("{}\n", body),
    )
}

fn write_context_descriptor(
    app: &AppHandle,
    boundary: &Path,
    path: PathBuf,
    value: Value,
) -> Result<(), String> {
    let body = serde_json::to_string_pretty(&value)
        .map_err(|error| format!("Nu am putut serializa descriptorul MCP: {error}"))?;
    write_mcp_file(
        app,
        boundary,
        path,
        format!("mcp/{CONTEXT_FILE}"),
        "Scriere descriptor lifecycle MCP Pană Studio",
        format!("{body}\n"),
    )
}

fn server_running_in_process(app: &AppHandle) -> bool {
    app.try_state::<crate::state::AppState>()
        .and_then(|state| {
            state
                .mcp_server
                .lock()
                .ok()
                .map(|slot| slot.as_ref().is_some_and(|handle| handle.is_running()))
        })
        .unwrap_or(false)
}

fn write_mcp_file(
    app: &AppHandle,
    boundary: &Path,
    path: PathBuf,
    public_label: impl Into<String>,
    description: impl Into<String>,
    contents: String,
) -> Result<(), String> {
    let target = WriteTarget::new(path, boundary.to_path_buf(), public_label);
    let intent = WriteIntent::new(
        WriteCategory::InternalAppWrite,
        WriteOwner::McpContext,
        WriteOperationKind::WriteText,
        target,
        WritePolicy::mcp_projection_atomic(),
        description,
    );
    WriteAuthority::new(app)
        .write_text(intent, &contents)
        .map_err(|error| error.into_terminal_diagnostic())
        .map(|_| ())
}

fn read_updated_at(path: &Path) -> Option<String> {
    let source = fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&source).ok()?;
    value
        .get("updatedAtMs")
        .and_then(Value::as_u64)
        .map(|value| value.to_string())
        .or_else(|| {
            value
                .get("updatedAt")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
