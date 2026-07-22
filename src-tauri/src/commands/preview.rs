use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::{
    kernel::{
        observability::{append_event, KernelEventKind, KernelLogEvent, KernelLogLevel},
        write_authority::WriteAuthorityRuntime,
    },
    preview::{
        read_http_document, require_browser_preview_session, require_project_preview_session,
        require_project_preview_workspace_revision, start_or_refresh_source_browser,
        stop_project_preview, BrowserPreviewRequestIdentity, BrowserPreviewStartReceipt,
        CanvasProjectionPhase, CanvasProjectionPlan, PersistentPreviewCandidate,
        PersistentPreviewOwner, PersistentZolaPreviewEngine, PreviewPhaseReceipt,
        ProjectPreviewMutationKind, ProjectPreviewMutationReceipt, ProjectPreviewRequestIdentity,
        ProjectPreviewStartReceipt,
    },
    project::{is_zola_project, zola_project_root},
    project_model::template_workbench::{
        resolve_template_workbench_plan, TemplateWorkbenchPlan, TemplateWorkbenchPlanInput,
    },
    state::AppState,
};

const MAX_PROJECTED_PATH_HINTS: usize = 4096;
const PREVIEW_RUNTIME_EVENT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewRuntimeEventKind {
    InteractiveJsRestarted,
    InteractiveJsFailed,
    CanvasPatchRolledBack,
    CanvasFallback,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewRuntimeEventInput {
    pub schema_version: u32,
    pub identity: crate::preview::CanvasProjectionIdentity,
    pub kind: PreviewRuntimeEventKind,
    pub duration_ms: u64,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewRuntimeEventReceipt {
    pub schema_version: u32,
    pub identity: crate::preview::CanvasProjectionIdentity,
    pub kind: PreviewRuntimeEventKind,
    pub accepted: bool,
}

fn append_canvas_phase_event<R: tauri::Runtime>(
    app: &AppHandle<R>,
    plan: &CanvasProjectionPlan,
    kind: KernelEventKind,
    level: KernelLogLevel,
    diagnostic: Option<String>,
    phase_timings_ms: Option<&BTreeMap<String, u64>>,
) {
    let mut event = KernelLogEvent::new(
        level,
        kind,
        "preview_canvas",
        "preview_projection",
        "canvas.phase",
        Some(plan.identity.transaction_id.clone()),
        format!("Canvas transaction reached {:?}.", plan.phase),
        diagnostic,
    )
    .with_attribute("projectRoot", &plan.identity.project_root)
    .with_attribute("runtimeSessionId", &plan.identity.runtime_session_id)
    .with_attribute("workspaceRevision", plan.identity.workspace_revision)
    .with_attribute("workspaceTransactionId", &plan.workspace_transaction_id)
    .with_attribute("canvasTransactionId", &plan.identity.transaction_id)
    .with_attribute("previewRevision", &plan.identity.preview_revision)
    .with_attribute("phase", plan.phase)
    .with_attribute("impactKinds", &plan.impact.kinds)
    .with_attribute("requiresFullDocument", plan.impact.requires_full_document)
    .with_attribute("resourceCount", plan.resources.entries.len())
    .with_attribute("resourceBytes", plan.resources.total_bytes);
    if let Some(phase_timings_ms) = phase_timings_ms {
        event = event.with_attribute("phaseTimingsMs", phase_timings_ms);
    }
    let _ = append_event(app, event);
}

fn append_canvas_observation_event<R: tauri::Runtime>(
    app: &AppHandle<R>,
    plan: &CanvasProjectionPlan,
    kind: KernelEventKind,
    level: KernelLogLevel,
    operation: &str,
    diagnostic: Option<String>,
) {
    let event = KernelLogEvent::new(
        level,
        kind,
        "preview_canvas",
        "preview_projection",
        operation,
        Some(plan.identity.transaction_id.clone()),
        format!("Canvas observation `{operation}` for {:?}.", plan.phase),
        diagnostic,
    )
    .with_attribute("projectRoot", &plan.identity.project_root)
    .with_attribute("runtimeSessionId", &plan.identity.runtime_session_id)
    .with_attribute("workspaceRevision", plan.identity.workspace_revision)
    .with_attribute("workspaceTransactionId", &plan.workspace_transaction_id)
    .with_attribute("canvasTransactionId", &plan.identity.transaction_id)
    .with_attribute("previewRevision", &plan.identity.preview_revision)
    .with_attribute("phase", plan.phase)
    .with_attribute("impactKinds", &plan.impact.kinds)
    .with_attribute("requiresFullDocument", plan.impact.requires_full_document);
    let _ = append_event(app, event);
}

fn append_canvas_stale_identity_event<R: tauri::Runtime>(
    app: &AppHandle<R>,
    identity: &crate::preview::CanvasProjectionIdentity,
    diagnostic: String,
) {
    let event = KernelLogEvent::new(
        KernelLogLevel::Warn,
        KernelEventKind::PreviewCanvasStaleDiscarded,
        "preview_canvas",
        "preview_projection",
        "canvas.stale_rejected",
        Some(identity.transaction_id.clone()),
        "Canvas receipt rejected as stale.",
        Some(diagnostic),
    )
    .with_attribute("projectRoot", &identity.project_root)
    .with_attribute("runtimeSessionId", &identity.runtime_session_id)
    .with_attribute("workspaceRevision", identity.workspace_revision)
    .with_attribute("canvasTransactionId", &identity.transaction_id)
    .with_attribute("previewRevision", &identity.preview_revision);
    let _ = append_event(app, event);
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkspacePreviewRequest {
    pub expected_project_root: String,
    pub expected_session_id: String,
    pub expected_workspace_revision: u64,
    #[serde(default)]
    pub requested_paths: Vec<String>,
}

impl ProjectWorkspacePreviewRequest {
    fn preview_identity(&self) -> ProjectPreviewRequestIdentity {
        ProjectPreviewRequestIdentity {
            expected_project_root: self.expected_project_root.clone(),
            expected_session_id: self.expected_session_id.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TemplateWorkbenchPreviewRequest {
    pub expected_project_root: String,
    pub expected_session_id: String,
    pub expected_workspace_revision: u64,
    pub template_path: String,
    #[serde(default)]
    pub preferred_page_path: Option<String>,
}

impl TemplateWorkbenchPreviewRequest {
    fn preview_identity(&self) -> ProjectPreviewRequestIdentity {
        ProjectPreviewRequestIdentity {
            expected_project_root: self.expected_project_root.clone(),
            expected_session_id: self.expected_session_id.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateWorkbenchPreviewReceipt {
    pub plan: TemplateWorkbenchPlan,
    pub route: String,
    pub preview_url: String,
    pub workspace_revision: u64,
    pub preview_revision: String,
    pub canvas_projection: CanvasProjectionPlan,
}

#[tauri::command(async)]
pub fn read_preview_document(url: String) -> Result<String, String> {
    read_http_document(&url)
}

#[tauri::command]
pub async fn start_project_browser_preview(
    input: BrowserPreviewRequestIdentity,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<BrowserPreviewStartReceipt>, String> {
    let root = require_browser_preview_session(state.inner(), &input)?;
    if !is_zola_project(&root) {
        return Ok(None);
    }
    let task_identity = input.clone();

    let receipt = tauri::async_runtime::spawn_blocking(
        move || -> Result<BrowserPreviewStartReceipt, String> {
            let state = app.state::<AppState>();
            start_or_refresh_source_browser(&app, state.inner(), &task_identity, true)?
                .ok_or_else(|| "Source Browser nu a fost inițializat.".to_string())
        },
    )
    .await
    .map_err(|error| format!("Browser preview task eșuat: {error}"))??;

    require_browser_preview_session(state.inner(), &input)?;
    Ok(Some(receipt))
}

#[tauri::command]
pub async fn start_project_preview(
    input: ProjectPreviewRequestIdentity,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<ProjectPreviewStartReceipt>, String> {
    let root = require_project_preview_session(state.inner(), &input)?;
    if !is_zola_project(&root) {
        return Ok(None);
    }
    let zola_root = zola_project_root(&root);
    let task_identity = input.clone();
    let task_project_root = root.clone();

    let receipt = tauri::async_runtime::spawn_blocking(
        move || -> Result<ProjectPreviewStartReceipt, String> {
            let state = app.state::<AppState>();
            let _operation = state
                .preview_workspace_operation
                .lock()
                .map_err(|_| "Nu am putut serializa pornirea preview-ului embedded.".to_string())?;
            require_project_preview_session(state.inner(), &task_identity)?;
            let authority_runtime = app.state::<WriteAuthorityRuntime>();
            let _project_session_lease = authority_runtime
                .acquire_active_project_read_lease_for_session(
                    &task_project_root,
                    &task_identity.expected_session_id,
                )?;
            let projection = capture_workspace_projection(state.inner(), None)?;
            let owner = PersistentPreviewOwner::new(
                task_identity.expected_project_root.clone(),
                task_identity.expected_session_id.clone(),
            );
            let mut engine_slot = state
                .preview_engine
                .lock()
                .map_err(|_| "Nu am putut bloca motorul Preview persistent.".to_string())?;
            replace_preview_engine_if_needed(&app, &zola_root, &mut engine_slot, &owner)?;
            let engine = engine_slot
                .as_mut()
                .expect("engine replacement must publish an engine");
            if engine.active_matches_revision(projection.revision)? {
                let active = engine
                    .active_generation()?
                    .expect("matching active revision must exist");
                append_canvas_observation_event(
                    &app,
                    &active.canvas_transaction.plan(),
                    KernelEventKind::PreviewCanvasCacheHit,
                    KernelLogLevel::Info,
                    "canvas.cache_hit",
                    None,
                );
                return Ok(ProjectPreviewStartReceipt {
                    url: engine.url()?,
                    project_root: task_identity.expected_project_root.clone(),
                    runtime_session_id: task_identity.expected_session_id.clone(),
                    workspace_revision: active.workspace_revision,
                    preview_revision: active.preview_revision.clone(),
                    canvas_projection: active.canvas_transaction.plan(),
                });
            }
            let candidate = engine.render_candidate(&app, &projection)?;
            let canvas_projection = stage_candidate_if_current(
                &app,
                state.inner(),
                &task_identity,
                &projection,
                engine,
                candidate,
            )?;
            Ok(ProjectPreviewStartReceipt {
                url: engine.url()?,
                project_root: task_identity.expected_project_root.clone(),
                runtime_session_id: task_identity.expected_session_id.clone(),
                workspace_revision: canvas_projection.identity.workspace_revision,
                preview_revision: canvas_projection.identity.preview_revision.clone(),
                canvas_projection,
            })
        },
    )
    .await
    .map_err(|error| format!("Preview task eșuat: {error}"))??;

    require_project_preview_workspace_revision(state.inner(), &input, receipt.workspace_revision)?;
    Ok(Some(receipt))
}

#[tauri::command]
pub async fn project_project_workspace_preview(
    input: ProjectWorkspacePreviewRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ProjectPreviewMutationReceipt, String> {
    let identity = input.preview_identity();
    let root = require_project_preview_workspace_revision(
        state.inner(),
        &identity,
        input.expected_workspace_revision,
    )?;
    let requested_paths = normalize_requested_paths(input.requested_paths)?;
    if !is_zola_project(&root) {
        stop_project_preview(&app, state.inner());
        return Ok(ProjectPreviewMutationReceipt::new(
            &identity,
            ProjectPreviewMutationKind::WorkspaceProjection,
            requested_paths,
            None,
            input.expected_workspace_revision,
        ));
    }

    let zola_root = zola_project_root(&root);
    let task_identity = identity.clone();
    let task_root = root.clone();
    let expected_revision = input.expected_workspace_revision;
    let (canvas_projection, projected_paths) = tauri::async_runtime::spawn_blocking(
        move || -> Result<(Option<CanvasProjectionPlan>, Vec<String>), String> {
            let state = app.state::<AppState>();
            let _operation = state.preview_workspace_operation.lock().map_err(|_| {
                "Nu am putut serializa proiecția ProjectWorkspace Preview.".to_string()
            })?;
            require_project_preview_workspace_revision(
                state.inner(),
                &task_identity,
                expected_revision,
            )?;
            let authority_runtime = app.state::<WriteAuthorityRuntime>();
            let _project_session_lease = authority_runtime
                .acquire_active_project_read_lease_for_session(
                    &task_root,
                    &task_identity.expected_session_id,
                )?;
            let projection = capture_workspace_projection(state.inner(), Some(expected_revision))?;
            let owner = PersistentPreviewOwner::new(
                task_identity.expected_project_root.clone(),
                task_identity.expected_session_id.clone(),
            );
            let mut engine_slot = state
                .preview_engine
                .lock()
                .map_err(|_| "Nu am putut bloca motorul Preview persistent.".to_string())?;
            replace_preview_engine_if_needed(&app, &zola_root, &mut engine_slot, &owner)?;
            let engine = engine_slot
                .as_mut()
                .expect("engine replacement must publish an engine");
            if engine.active_matches_revision(projection.revision)? {
                if let Some(active) = engine.active_generation()? {
                    append_canvas_observation_event(
                        &app,
                        &active.canvas_transaction.plan(),
                        KernelEventKind::PreviewCanvasCacheHit,
                        KernelLogLevel::Info,
                        "canvas.cache_hit",
                        None,
                    );
                }
                return Ok((None, Vec::new()));
            }
            let candidate = engine.render_candidate(&app, &projection)?;
            let projected_paths = candidate.projected_paths.clone();
            let canvas_projection = stage_candidate_if_current(
                &app,
                state.inner(),
                &task_identity,
                &projection,
                engine,
                candidate,
            )?;
            Ok((Some(canvas_projection), projected_paths))
        },
    )
    .await
    .map_err(|error| format!("ProjectWorkspace Preview task eșuat: {error}"))??;

    let receipt_paths = requested_paths
        .into_iter()
        .chain(projected_paths)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok(ProjectPreviewMutationReceipt::new(
        &identity,
        ProjectPreviewMutationKind::WorkspaceProjection,
        receipt_paths,
        canvas_projection,
        expected_revision,
    ))
}

#[tauri::command]
pub async fn project_template_workbench_preview(
    input: TemplateWorkbenchPreviewRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<TemplateWorkbenchPreviewReceipt, String> {
    let identity = input.preview_identity();
    let root = require_project_preview_workspace_revision(
        state.inner(),
        &identity,
        input.expected_workspace_revision,
    )?;
    if !is_zola_project(&root) {
        return Err("Context de template cere un proiect Zola inițializat.".to_string());
    }
    let task_input = input.clone();
    let task_identity = identity.clone();
    let task_root = root.clone();
    let receipt =
        tauri::async_runtime::spawn_blocking(
            move || -> Result<TemplateWorkbenchPreviewReceipt, String> {
                let state = app.state::<AppState>();
                let _operation = state.preview_workspace_operation.lock().map_err(|_| {
                    "Nu am putut serializa proiecția Context de template.".to_string()
                })?;
                require_project_preview_workspace_revision(
                    state.inner(),
                    &task_identity,
                    task_input.expected_workspace_revision,
                )?;
                let authority_runtime = app.state::<WriteAuthorityRuntime>();
                let _project_session_lease = authority_runtime
                    .acquire_active_project_read_lease_for_session(
                        &task_root,
                        &task_identity.expected_session_id,
                    )?;
                let projection = capture_workspace_projection(
                    state.inner(),
                    Some(task_input.expected_workspace_revision),
                )?;
                let model = crate::project_model::build_project_model_from_workspace_projection(
                    &task_root,
                    &projection,
                )?;
                let plan = resolve_template_workbench_plan(
                    &model,
                    &TemplateWorkbenchPlanInput {
                        template_path: task_input.template_path,
                        preferred_page_path: task_input.preferred_page_path,
                    },
                )?;
                let mut engine_slot = state.preview_engine.lock().map_err(|_| {
                    "Nu am putut bloca motorul Preview pentru Workbench.".to_string()
                })?;
                let engine = engine_slot.as_mut().ok_or_else(|| {
                    "Context de template cere mai întâi Preview-ul canonic al aceleiași revizii."
                        .to_string()
                })?;
                let publication = engine.publish_template_workbench_view(&projection, &plan)?;
                Ok(TemplateWorkbenchPreviewReceipt {
                    plan,
                    route: publication.route,
                    preview_url: publication.preview_url,
                    workspace_revision: publication.workspace_revision,
                    preview_revision: publication.preview_revision,
                    canvas_projection: publication.canvas_plan,
                })
            },
        )
        .await
        .map_err(|error| format!("Context de template task eșuat: {error}"))??;

    require_project_preview_workspace_revision(
        state.inner(),
        &identity,
        receipt.workspace_revision,
    )?;
    Ok(receipt)
}

#[tauri::command]
pub fn acknowledge_canvas_projection_phase(
    input: PreviewPhaseReceipt,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CanvasProjectionPlan, String> {
    let identity = ProjectPreviewRequestIdentity {
        expected_project_root: input.identity.project_root.clone(),
        expected_session_id: input.identity.runtime_session_id.clone(),
    };
    if let Err(error) = require_project_preview_workspace_revision(
        state.inner(),
        &identity,
        input.identity.workspace_revision,
    ) {
        append_canvas_stale_identity_event(&app, &input.identity, error.clone());
        return Err(error);
    }
    let _operation = state
        .preview_workspace_operation
        .lock()
        .map_err(|_| "Nu am putut serializa confirmarea Canvas Runtime.".to_string())?;
    let mut engine_slot = state
        .preview_engine
        .lock()
        .map_err(|_| "Nu am putut bloca motorul Preview la confirmarea Canvas.".to_string())?;
    let engine = engine_slot
        .as_mut()
        .ok_or_else(|| "Canvas Runtime nu are motor Preview activ.".to_string())?;
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut valida root-ul la confirmarea Canvas.".to_string())?;
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut valida ProjectWorkspace la confirmarea Canvas.".to_string())?;
    if current_root.as_deref() != Some(std::path::Path::new(&input.identity.project_root)) {
        let error = "ACK-ul Canvas a devenit stale: proiectul activ s-a schimbat.".to_string();
        append_canvas_stale_identity_event(&app, &input.identity, error.clone());
        return Err(error);
    }
    let Some(workspace) = workspace.as_ref() else {
        let error = "ACK-ul Canvas a devenit stale: ProjectWorkspace lipsește.".to_string();
        append_canvas_stale_identity_event(&app, &input.identity, error.clone());
        return Err(error);
    };
    if workspace.session.runtime_instance_id() != input.identity.runtime_session_id
        || workspace.revision != input.identity.workspace_revision
    {
        let error = "ACK-ul Canvas a devenit stale față de ProjectWorkspace.".to_string();
        append_canvas_stale_identity_event(&app, &input.identity, error.clone());
        return Err(error);
    }
    let generation = match engine.acknowledge_candidate_phase(&app, &input) {
        Ok(generation) => generation,
        Err(error) => {
            if error.contains("nu mai are candidatul")
                || error.contains("altă tranzacție")
                || error.contains("altei sesiuni")
            {
                append_canvas_stale_identity_event(&app, &input.identity, error.clone());
            }
            return Err(error);
        }
    };
    let plan = generation.canvas_transaction.plan();
    let (kind, level) = match plan.phase {
        CanvasProjectionPhase::CanonicalVerified => (
            KernelEventKind::PreviewCanvasCanonicalVerified,
            KernelLogLevel::Info,
        ),
        CanvasProjectionPhase::Failed => {
            (KernelEventKind::PreviewCanvasFailed, KernelLogLevel::Error)
        }
        _ => (
            KernelEventKind::PreviewCanvasPhaseAcknowledged,
            KernelLogLevel::Info,
        ),
    };
    append_canvas_phase_event(
        &app,
        &plan,
        kind,
        level,
        input.diagnostic.clone(),
        Some(&input.phase_timings_ms),
    );
    if plan.phase == CanvasProjectionPhase::CanonicalVerified {
        append_canvas_observation_event(
            &app,
            &plan,
            KernelEventKind::PreviewCanvasFoucGuardSatisfied,
            KernelLogLevel::Info,
            "canvas.fouc_guard_satisfied",
            Some("CSS/fonts ready și frame stilizat confirmat înainte de promovare.".to_string()),
        );
    }
    Ok(plan)
}

#[tauri::command]
pub fn record_preview_runtime_event(
    input: PreviewRuntimeEventInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<PreviewRuntimeEventReceipt, String> {
    if input.schema_version != PREVIEW_RUNTIME_EVENT_SCHEMA_VERSION
        || input.duration_ms > 600_000
        || input
            .diagnostic
            .as_deref()
            .is_some_and(|value| value.trim().is_empty() || value.len() > 4_096)
    {
        return Err(
            "Evenimentul Previzualizare interactivă nu respectă protocolul bounded.".to_string(),
        );
    }
    let engine = state.preview_engine.lock().map_err(|_| {
        "Nu am putut valida motorul pentru evenimentul Previzualizare interactivă.".to_string()
    })?;
    let plan = engine
        .as_ref()
        .ok_or_else(|| "Previzualizare interactivă nu are motor Preview activ.".to_string())?
        .canvas_plan_for_identity(&input.identity)?
        .ok_or_else(|| {
            "Evenimentul Preview nu aparține unei generații Canvas reținute.".to_string()
        })?;
    let interactive_event = matches!(
        input.kind,
        PreviewRuntimeEventKind::InteractiveJsRestarted
            | PreviewRuntimeEventKind::InteractiveJsFailed
    );
    if interactive_event && plan.phase != CanvasProjectionPhase::CanonicalVerified {
        return Err(
            "Evenimentul Previzualizare interactivă cere generația canonică activă.".to_string(),
        );
    }
    let (event_kind, level) = match input.kind {
        PreviewRuntimeEventKind::InteractiveJsRestarted => (
            KernelEventKind::PreviewInteractiveJsRestarted,
            KernelLogLevel::Info,
        ),
        PreviewRuntimeEventKind::InteractiveJsFailed => (
            KernelEventKind::PreviewInteractiveJsFailed,
            KernelLogLevel::Error,
        ),
        PreviewRuntimeEventKind::CanvasPatchRolledBack => (
            KernelEventKind::PreviewCanvasPatchRolledBack,
            KernelLogLevel::Warn,
        ),
        PreviewRuntimeEventKind::CanvasFallback => {
            (KernelEventKind::PreviewCanvasFallback, KernelLogLevel::Info)
        }
    };
    let event = KernelLogEvent::new(
        level,
        event_kind,
        if interactive_event {
            "preview_interactive"
        } else {
            "preview_canvas"
        },
        "preview_projection",
        if interactive_event {
            "interactive_js.realm"
        } else {
            "canvas.runtime_observation"
        },
        Some(input.identity.transaction_id.clone()),
        format!("Preview runtime reported {:?}.", input.kind),
        input.diagnostic.clone(),
    )
    .with_attribute("projectRoot", &input.identity.project_root)
    .with_attribute("runtimeSessionId", &input.identity.runtime_session_id)
    .with_attribute("workspaceRevision", input.identity.workspace_revision)
    .with_attribute("canvasTransactionId", &input.identity.transaction_id)
    .with_attribute("previewRevision", &input.identity.preview_revision)
    .with_attribute("durationMs", input.duration_ms);
    append_event(&app, event)?;
    Ok(PreviewRuntimeEventReceipt {
        schema_version: PREVIEW_RUNTIME_EVENT_SCHEMA_VERSION,
        identity: input.identity,
        kind: input.kind,
        accepted: true,
    })
}

fn replace_preview_engine_if_needed<R: tauri::Runtime>(
    app: &AppHandle<R>,
    zola_root: &std::path::Path,
    slot: &mut Option<PersistentZolaPreviewEngine>,
    owner: &PersistentPreviewOwner,
) -> Result<(), String> {
    if slot
        .as_ref()
        .is_some_and(|engine| engine.owner_matches(owner))
    {
        return Ok(());
    }
    if let Some(previous) = slot.take() {
        previous.stop(app)?;
    }
    *slot = Some(PersistentZolaPreviewEngine::start(
        app,
        zola_root,
        owner.clone(),
    )?);
    Ok(())
}

fn stage_candidate_if_current<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    identity: &ProjectPreviewRequestIdentity,
    lease: &crate::kernel::project_workspace::WorkspaceProjectionLease,
    engine: &mut PersistentZolaPreviewEngine,
    candidate: PersistentPreviewCandidate,
) -> Result<CanvasProjectionPlan, String> {
    let candidate_plan = candidate.canvas_plan();
    let mut candidate = Some(candidate);
    let staging = (|| {
        let current_root = state
            .current_root
            .lock()
            .map_err(|_| "Nu am putut valida root-ul la staging Canvas.".to_string())?;
        let workspace = state
            .project_workspace
            .lock()
            .map_err(|_| "Nu am putut valida ProjectWorkspace la staging Canvas.".to_string())?;
        if current_root.as_deref() != Some(std::path::Path::new(&identity.expected_project_root)) {
            return Err(
                "Staging-ul Canvas a fost anulat: proiectul activ s-a schimbat.".to_string(),
            );
        }
        let workspace = workspace.as_ref().ok_or_else(|| {
            "Staging-ul Canvas a fost anulat: ProjectWorkspace lipsește.".to_string()
        })?;
        workspace.require_current_projection(lease)?;
        let generation = engine.stage_candidate(
            app,
            candidate
                .take()
                .expect("candidate is consumed exactly once at staging"),
        )?;
        let plan = generation.canvas_transaction.plan();
        append_canvas_phase_event(
            app,
            &plan,
            KernelEventKind::PreviewCanvasPrepared,
            KernelLogLevel::Info,
            None,
            None,
        );
        append_canvas_observation_event(
            app,
            &plan,
            KernelEventKind::PreviewCanvasCacheMiss,
            KernelLogLevel::Info,
            "canvas.cache_miss",
            None,
        );
        Ok(plan)
    })();

    if staging.is_err() {
        append_canvas_observation_event(
            app,
            &candidate_plan,
            KernelEventKind::PreviewCanvasStaleDiscarded,
            KernelLogLevel::Warn,
            "canvas.stale_discarded",
            staging.as_ref().err().cloned(),
        );
        if let Some(candidate) = candidate.take() {
            let _ = engine.discard_candidate(app, candidate);
        }
    }
    staging
}

fn capture_workspace_projection(
    state: &AppState,
    expected_revision: Option<u64>,
) -> Result<crate::kernel::project_workspace::WorkspaceProjectionLease, String> {
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut captura ProjectWorkspace pentru Preview.".to_string())?;
    let workspace = workspace
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru Preview.".to_string())?;
    if let Some(expected_revision) = expected_revision {
        if workspace.revision != expected_revision {
            return Err(format!(
                "Proiecția Preview a devenit stale înainte de materializare: așteptat {expected_revision}, activ {}.",
                workspace.revision
            ));
        }
    }
    workspace.capture_projection_lease()
}

fn normalize_requested_paths(paths: Vec<String>) -> Result<Vec<String>, String> {
    if paths.len() > MAX_PROJECTED_PATH_HINTS {
        return Err(format!(
            "Proiecția Preview refuză peste {MAX_PROJECTED_PATH_HINTS} path-uri per request."
        ));
    }
    let mut normalized = BTreeSet::new();
    for path in paths {
        let path = path.trim().replace('\\', "/");
        if path.is_empty()
            || path.starts_with('/')
            || path
                .split('/')
                .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
        {
            return Err(format!(
                "Proiecția Preview refuză path-ul nesigur `{path}`."
            ));
        }
        normalized.insert(path);
    }
    Ok(normalized.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requested_paths_are_deduplicated_and_sorted() {
        let paths = normalize_requested_paths(vec![
            "templates/z.html".to_string(),
            "templates/a.html".to_string(),
            "templates/z.html".to_string(),
        ])
        .unwrap();
        assert_eq!(
            paths,
            vec![
                "templates/a.html".to_string(),
                "templates/z.html".to_string(),
            ]
        );
    }

    #[test]
    fn requested_paths_fail_closed_on_traversal() {
        assert!(normalize_requested_paths(vec!["../outside".to_string()]).is_err());
    }
}
