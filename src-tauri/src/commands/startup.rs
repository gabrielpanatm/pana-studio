use std::path::PathBuf;

use tauri::{AppHandle, Runtime, State};

use crate::{
    project::{
        apply_startup_creation as apply_creation_domain,
        plan_startup_creation as plan_creation_domain,
        read_startup_creation_catalog as read_creation_catalog_domain, StartupCreationApplyRequest,
        StartupCreationCatalog, StartupCreationPlan, StartupCreationPlanRequest,
        StartupCreationReceipt, StartupFlowSnapshot,
    },
    state::AppState,
};

#[tauri::command]
pub fn read_startup_flow(state: State<AppState>) -> Result<StartupFlowSnapshot, String> {
    state.startup_flow.snapshot()
}

#[tauri::command]
pub async fn inspect_startup_folder(
    path: String,
    state: State<'_, AppState>,
) -> Result<StartupFlowSnapshot, String> {
    let requested = PathBuf::from(path);
    let runtime = state.startup_flow.clone();
    tauri::async_runtime::spawn_blocking(move || runtime.inspect(&requested))
        .await
        .map_err(|error| format!("Inspecția Startup Rust s-a oprit neașteptat: {error}"))?
}

#[tauri::command]
pub async fn read_startup_creation_catalog<R: Runtime>(
    expected_snapshot_token: String,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<StartupCreationCatalog, String> {
    let runtime = state.startup_flow.clone();
    tauri::async_runtime::spawn_blocking(move || {
        read_creation_catalog_domain(&app, &runtime, &expected_snapshot_token)
    })
    .await
    .map_err(|error| format!("Catalogul Startup Rust s-a oprit neașteptat: {error}"))?
}

#[tauri::command]
pub async fn plan_startup_creation<R: Runtime>(
    request: StartupCreationPlanRequest,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<StartupCreationPlan, String> {
    let runtime = state.startup_flow.clone();
    tauri::async_runtime::spawn_blocking(move || plan_creation_domain(&app, &runtime, request))
        .await
        .map_err(|error| format!("Planificarea Startup Rust s-a oprit neașteptat: {error}"))?
}

#[tauri::command]
pub async fn apply_startup_creation<R: Runtime>(
    request: StartupCreationApplyRequest,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<StartupCreationReceipt, String> {
    let runtime = state.startup_flow.clone();
    tauri::async_runtime::spawn_blocking(move || apply_creation_domain(&app, &runtime, request))
        .await
        .map_err(|error| format!("Crearea Startup Rust s-a oprit neașteptat: {error}"))?
}
