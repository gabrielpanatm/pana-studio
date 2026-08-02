use std::{path::PathBuf, time::Instant};

use tauri::{AppHandle, Manager, Runtime, State};

use crate::{
    kernel::{
        observability::{append_event, KernelEventKind, KernelLogEvent, KernelLogLevel},
        project_session::fingerprint_project_root,
        project_workspace::inspect_project_workspace_recovery_for_open,
    },
    project::{
        apply_startup_creation as apply_creation_domain,
        plan_startup_creation as plan_creation_domain,
        read_startup_creation_catalog as read_creation_catalog_domain, ActiveProjectReadiness,
        ProjectLifecycleSnapshot, ProjectOpenInspectionReceipt, StartupCreationApplyRequest,
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
pub fn read_project_lifecycle(state: State<AppState>) -> Result<ProjectLifecycleSnapshot, String> {
    state.project_lifecycle.snapshot()
}

#[tauri::command]
pub async fn inspect_startup_folder(
    path: String,
    state: State<'_, AppState>,
) -> Result<StartupFlowSnapshot, String> {
    let requested = PathBuf::from(path);
    let runtime = state.startup_flow.clone();
    let lifecycle = state.project_lifecycle.clone();
    let operation_id = {
        let _transition = state
            .project_lifecycle_transition
            .lock()
            .map_err(|_| "Serializarea ProjectLifecycle este compromisă.".to_string())?;
        lifecycle.begin_inspection(&requested.to_string_lossy())?
    };
    tauri::async_runtime::spawn_blocking(move || {
        let snapshot = runtime.inspect_for_operation(&requested, Some(operation_id.clone()))?;
        if snapshot.candidate.as_ref().is_none_or(|candidate| {
            candidate.kind != crate::project::StartupCandidateKind::ValidProject
        }) {
            let _ = lifecycle.fail_before_commit(&operation_id, "startup_candidate_not_openable");
        }
        Ok(snapshot)
    })
    .await
    .map_err(|error| format!("Inspecția Startup Rust s-a oprit neașteptat: {error}"))?
}

#[tauri::command]
pub async fn inspect_project_open(
    path: String,
    expected_snapshot_token: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ProjectOpenInspectionReceipt, String> {
    let requested = PathBuf::from(path);
    let startup = state.startup_flow.clone();
    let lifecycle = state.project_lifecycle.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (candidate, manifest, startup_operation_id) =
            startup.require_valid_candidate(&requested, &expected_snapshot_token)?;
        let operation_id = if let Some(operation_id) = startup_operation_id {
            operation_id
        } else {
            let app_state = app.state::<AppState>();
            let _transition = app_state
                .project_lifecycle_transition
                .lock()
                .map_err(|_| "Serializarea ProjectLifecycle este compromisă.".to_string())?;
            lifecycle.begin_inspection(&candidate.root)?
        };
        let candidate_token = candidate.snapshot_token.clone();
        if let Some(receipt) = lifecycle.published_inspection(&operation_id, &candidate_token)? {
            return Ok(receipt);
        }
        let inspection_started = Instant::now();
        let inspected = (|| -> Result<ProjectOpenInspectionReceipt, String> {
            let root = PathBuf::from(&candidate.root);
            let root_fingerprint = fingerprint_project_root(&root)?;
            let recovery = inspect_project_workspace_recovery_for_open(
                &app,
                &root,
                &manifest,
                &root_fingerprint,
            )?;
            let receipt = lifecycle.publish_inspection(
                &operation_id,
                candidate,
                manifest,
                root_fingerprint,
                recovery,
            )?;
            let _ = append_event(
                &app,
                KernelLogEvent::new(
                    KernelLogLevel::Info,
                    KernelEventKind::ProjectLifecycleTransition,
                    "project_lifecycle",
                    "project_transition",
                    "inspect",
                    Some(operation_id.clone()),
                    "ProjectLifecycle a publicat inspecția autoritară.",
                    None,
                )
                .with_attribute("operationId", &operation_id)
                .with_attribute("projectRoot", &receipt.recovery.project_root)
                .with_attribute("transition", receipt.lifecycle.transition)
                .with_attribute(
                    "folderSelectedToInspectionMs",
                    crate::kernel::observability::now_ms()
                        .saturating_sub(receipt.operation_started_at_ms)
                        .min(u64::MAX as u128) as u64,
                )
                .with_attribute(
                    "recoveryAssessmentMs",
                    inspection_started
                        .elapsed()
                        .as_millis()
                        .min(u64::MAX as u128) as u64,
                ),
            );
            Ok(receipt)
        })();
        if let Err(error) = &inspected {
            if let Ok(Some(receipt)) =
                lifecycle.published_inspection(&operation_id, &candidate_token)
            {
                return Ok(receipt);
            }
            let _ = lifecycle.fail_before_commit(&operation_id, error);
        }
        inspected
    })
    .await
    .map_err(|error| format!("Inspecția ProjectLifecycle s-a oprit neașteptat: {error}"))?
}

#[tauri::command]
pub fn cancel_project_open(
    operation_id: String,
    diagnostic: String,
    app: AppHandle,
    state: State<AppState>,
) -> Result<ProjectLifecycleSnapshot, String> {
    let _transition = state
        .project_lifecycle_transition
        .lock()
        .map_err(|_| "Serializarea ProjectLifecycle este compromisă.".to_string())?;
    let snapshot = state
        .project_lifecycle
        .fail_before_commit(&operation_id, &format!("cancelled:{diagnostic}"))?;
    append_lifecycle_snapshot_event(
        &app,
        &snapshot,
        "cancel",
        Some(operation_id),
        KernelLogLevel::Info,
    );
    Ok(snapshot)
}

#[tauri::command]
pub fn acknowledge_project_frontend_hydrated(
    project_root: String,
    runtime_session_id: String,
    app: AppHandle,
    state: State<AppState>,
) -> Result<ProjectLifecycleSnapshot, String> {
    let snapshot = state.project_lifecycle.set_readiness(
        &project_root,
        &runtime_session_id,
        ActiveProjectReadiness::PreparingPreview,
        "frontend_hydrated",
    )?;
    append_lifecycle_snapshot_event(
        &app,
        &snapshot,
        "frontend_hydrated",
        None,
        KernelLogLevel::Info,
    );
    Ok(snapshot)
}

#[tauri::command]
pub fn report_project_capability_degraded(
    project_root: String,
    runtime_session_id: String,
    capability: String,
    diagnostic: String,
    app: AppHandle,
    state: State<AppState>,
) -> Result<ProjectLifecycleSnapshot, String> {
    let capability = capability.trim();
    let diagnostic = diagnostic.trim();
    if !matches!(
        capability,
        "frontend" | "preview" | "canvas" | "source_graph"
    ) {
        return Err("Capabilitatea degradată nu este recunoscută.".to_string());
    }
    if diagnostic.is_empty() || diagnostic.len() > 4_096 {
        return Err("Diagnosticul degradării este gol sau depășește limita.".to_string());
    }
    let snapshot = state.project_lifecycle.set_readiness(
        &project_root,
        &runtime_session_id,
        ActiveProjectReadiness::Degraded {
            capability: capability.to_string(),
            diagnostic: diagnostic.to_string(),
        },
        "frontend_capability_degraded",
    )?;
    append_lifecycle_snapshot_event(&app, &snapshot, "degraded", None, KernelLogLevel::Warn);
    Ok(snapshot)
}

fn append_lifecycle_snapshot_event<R: Runtime>(
    app: &AppHandle<R>,
    snapshot: &ProjectLifecycleSnapshot,
    phase: &str,
    operation_id: Option<String>,
    level: KernelLogLevel,
) {
    let mut event = KernelLogEvent::new(
        level,
        KernelEventKind::ProjectLifecycleTransition,
        "project_lifecycle",
        "project_transition",
        phase,
        operation_id,
        "ProjectLifecycle și-a schimbat starea autoritară.",
        None,
    )
    .with_attribute("transition", snapshot.transition)
    .with_attribute("reason", &snapshot.reason)
    .with_attribute("lifecycleRevision", snapshot.revision);
    if let Some(active) = snapshot.active_session.as_ref() {
        event = event
            .with_attribute("projectRoot", &active.project_root)
            .with_attribute("sessionId", &active.runtime_session_id)
            .with_attribute("readiness", &active.readiness);
    }
    let _ = append_event(app, event);
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
