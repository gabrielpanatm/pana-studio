use std::{
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{
    blocks::NativeBlockSlotMutationContext,
    css::rules::{selector_source_target, selector_source_target_at_offset},
    kernel::canvas_interaction::{
        CanvasDragPosition, CanvasInteractionBindingReceipt, CanvasInteractionGesture,
        CanvasInteractionIdentity, CanvasInteractionReceipt, CanvasInteractionRequest,
        CANVAS_INTERACTION_SCHEMA_VERSION,
    },
    kernel::editor_navigation::{
        build_editor_navigation_snapshot, editor_navigation_access_node, editor_navigation_node,
        plan_editor_move as build_editor_move_plan,
        plan_editor_move_with_slot as build_editor_move_plan_with_slot, EditScopeGrant,
        EditScopeOperation, EditorMoveExecutionReceipt, EditorMoveExecutionStatus, EditorMovePlan,
        EditorMoveTimings, EditorNavigationSnapshot,
    },
    kernel::observability::{append_event, KernelEventKind, KernelLogEvent, KernelLogLevel},
    kernel::preview_projection::{execute_editor_move, PreviewStructuralCommandIdentity},
    kernel::project_path::normalize_project_relative_path,
    kernel::selection_coordinator::{
        HoverSnapshot, SelectionCoordinatorSnapshot, SelectionIntent, SelectionObservationInput,
        SelectionObservationReceipt, SELECTION_COORDINATOR_SCHEMA_VERSION,
    },
    preview::{CanvasGraph, CanvasProjectionIdentity},
    project::ActiveProjectReadiness,
    project_model::{
        cache::{
            build_project_model_from_context, capture_project_model_build_context,
            publish_project_model_if_current, ProjectModelBuildContext,
        },
        model::{ProjectModel, ProjectModelFile, ProjectModelFileKind},
        move_engine::ProjectMovePosition,
    },
    state::AppState,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorNavigationSnapshotRequest {
    pub identity: CanvasProjectionIdentity,
    pub route: String,
    #[serde(default)]
    pub active_document_path: Option<String>,
    #[serde(default)]
    pub preview_context_render_instance_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorEditScopeRequest {
    pub identity: CanvasProjectionIdentity,
    pub route: String,
    #[serde(default)]
    pub active_document_path: Option<String>,
    #[serde(default)]
    pub preview_context_render_instance_id: Option<String>,
    pub scope_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorMovePlanRequest {
    pub identity: CanvasProjectionIdentity,
    pub route: String,
    #[serde(default)]
    pub active_document_path: Option<String>,
    #[serde(default)]
    pub preview_context_render_instance_id: Option<String>,
    pub source_node_id: String,
    pub target_node_id: String,
    pub position: ProjectMovePosition,
    #[serde(default)]
    pub edit_scope_grant: Option<EditScopeGrant>,
    #[serde(default)]
    pub native_block_slot: Option<NativeBlockSlotMutationContext>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorMoveCommitRequest {
    pub identity: CanvasProjectionIdentity,
    pub route: String,
    #[serde(default)]
    pub active_document_path: Option<String>,
    #[serde(default)]
    pub preview_context_render_instance_id: Option<String>,
    pub plan_token: String,
    #[serde(default)]
    pub input_emitted_at_ms: u64,
    #[serde(default)]
    pub edit_scope_grant: Option<EditScopeGrant>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasInteractionBindRequest {
    pub schema_version: u32,
    pub identity: CanvasInteractionIdentity,
    #[serde(default)]
    pub active_document_path: Option<String>,
    #[serde(default)]
    pub preview_context_render_instance_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasInteractionResolveRequest {
    pub request: CanvasInteractionRequest,
    #[serde(default)]
    pub edit_scope_grant: Option<EditScopeGrant>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasDragOverResolveRequest {
    pub request: CanvasInteractionRequest,
    pub source_node_id: String,
    #[serde(default)]
    pub edit_scope_grant: Option<EditScopeGrant>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasDragOverTimings {
    pub emitted_at_ms: u64,
    pub rust_received_at_ms: u64,
    pub rust_completed_at_ms: u64,
    pub input_to_plan_duration_ms: u64,
    pub input_to_first_allowed_plan_ms: Option<u64>,
    pub rust_duration_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasDragOverReceipt {
    pub schema_version: u32,
    pub interaction: CanvasInteractionReceipt,
    pub plan: Option<EditorMovePlan>,
    pub timings: CanvasDragOverTimings,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasHoverProjection {
    pub changed: bool,
    pub hover: Option<HoverSnapshot>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasHoverTimings {
    pub emitted_at_ms: u64,
    pub rust_received_at_ms: u64,
    pub rust_completed_at_ms: u64,
    pub input_to_projection_duration_ms: u64,
    pub rust_duration_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasHoverReceipt {
    pub schema_version: u32,
    pub interaction: CanvasInteractionReceipt,
    pub projection: Option<CanvasHoverProjection>,
    pub timings: CanvasHoverTimings,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionCoordinatorRequest {
    pub schema_version: u32,
    pub identity: CanvasProjectionIdentity,
    pub route: String,
    #[serde(default)]
    pub active_document_path: Option<String>,
    #[serde(default)]
    pub preview_context_render_instance_id: Option<String>,
    #[serde(default)]
    pub edit_scope_grant: Option<EditScopeGrant>,
    pub intent: SelectionIntent,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionCoordinatorReadRequest {
    pub schema_version: u32,
    pub identity: CanvasProjectionIdentity,
    pub route: String,
    #[serde(default)]
    pub active_document_path: Option<String>,
    #[serde(default)]
    pub preview_context_render_instance_id: Option<String>,
}

struct EditorNavigationContext {
    build_context: ProjectModelBuildContext,
    model: Arc<ProjectModel>,
    snapshot: Arc<EditorNavigationSnapshot>,
    active_document_path: Option<String>,
}

#[tauri::command]
pub async fn bind_canvas_interaction_agent(
    input: CanvasInteractionBindRequest,
    app: AppHandle,
) -> Result<CanvasInteractionBindingReceipt, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if input.schema_version != CANVAS_INTERACTION_SCHEMA_VERSION {
            return Err(
                "CanvasAgent folosește o versiune incompatibilă a protocolului.".to_string(),
            );
        }
        let state = app.state::<AppState>();
        let snapshot_request = EditorNavigationSnapshotRequest {
            identity: input.identity.canvas.clone(),
            route: input.identity.route.clone(),
            active_document_path: input.active_document_path,
            preview_context_render_instance_id: input.preview_context_render_instance_id,
        };
        let context = resolve_editor_navigation_context(&snapshot_request, state.inner())?;
        let receipt = state.canvas_interaction.bind_agent_with_model(
            Arc::clone(&context.snapshot),
            Arc::clone(&context.model),
            context.active_document_path.as_deref(),
            input.identity,
        )?;
        state
            .selection_coordinator
            .bind_inspector_document(receipt.identity.clone())?;
        publish_project_model_if_current(state.inner(), &context.build_context, context.model)?;
        Ok(receipt)
    })
    .await
    .map_err(|error| {
        format!("Canvas Interaction bind a căzut în task-ul Rust de fundal: {error}")
    })?
}

#[tauri::command]
pub fn resolve_canvas_interaction_intent(
    input: CanvasInteractionResolveRequest,
    state: State<AppState>,
) -> Result<CanvasInteractionReceipt, String> {
    let authorized_scope_id = authorize_canvas_edit_scope(
        &input.request,
        input.edit_scope_grant.as_ref(),
        state.inner(),
    )?;
    state
        .canvas_interaction
        .resolve(authorized_scope_id.as_deref(), &input.request)
}

fn authorize_canvas_edit_scope(
    request: &CanvasInteractionRequest,
    edit_scope_grant: Option<&EditScopeGrant>,
    state: &AppState,
) -> Result<Option<String>, String> {
    if let Some(grant) = edit_scope_grant {
        let scope_context = state.canvas_interaction.scope_context(&request.identity)?;
        let active_document_path =
            scope_context
                .active_document_path
                .as_deref()
                .ok_or_else(|| {
                    "Canvas Interaction cere un template activ pentru EditScopeGrant.".to_string()
                })?;
        state.editor_navigation.require_edit_scope_grant(
            grant,
            &scope_context.identity,
            &scope_context.model_revision,
            &scope_context.route,
            active_document_path,
            &grant.scope_id,
            EditScopeOperation::InspectSharedDefinition,
        )?;
        Ok(Some(grant.scope_id.clone()))
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub fn resolve_canvas_drag_over_intent(
    input: CanvasDragOverResolveRequest,
    state: State<AppState>,
) -> Result<CanvasDragOverReceipt, String> {
    require_editor_node_id(&input.source_node_id)?;
    if input.request.gesture != CanvasInteractionGesture::DragOver {
        return Err("Canvas DragOver a refuzat alt tip de gest.".to_string());
    }
    let rust_started = Instant::now();
    let rust_received_at_ms = wall_clock_ms();
    let authorized_scope_id = authorize_canvas_edit_scope(
        &input.request,
        input.edit_scope_grant.as_ref(),
        state.inner(),
    )?;
    let (interaction, plan) = state.canvas_interaction.resolve_drag_over(
        authorized_scope_id.as_deref(),
        &input.request,
        |snapshot, model, _active_document_path, receipt| {
            let (Some(target), Some(drag_position)) =
                (receipt.target.as_ref(), receipt.drag_position)
            else {
                return Ok(None);
            };
            let position = match drag_position {
                CanvasDragPosition::Before => ProjectMovePosition::Before,
                CanvasDragPosition::After => ProjectMovePosition::After,
                CanvasDragPosition::Inside => ProjectMovePosition::Inside,
            };
            let decision = build_editor_move_plan(
                &state.editor_navigation,
                snapshot,
                model,
                &input.source_node_id,
                &target.editor_node_id,
                position,
                input.edit_scope_grant.as_ref(),
            );
            state
                .editor_navigation
                .issue_editor_move_decision(decision)
                .map(Some)
        },
    )?;
    let rust_completed_at_ms = wall_clock_ms();
    let emitted_at_ms = input.request.emitted_at_ms;
    let input_to_plan_duration_ms = if emitted_at_ms == 0 {
        0
    } else {
        rust_completed_at_ms.saturating_sub(emitted_at_ms)
    };
    let input_to_first_allowed_plan_ms = plan
        .as_ref()
        .and_then(|plan| plan.as_ref())
        .filter(|plan| plan.allowed)
        .map(|_| input_to_plan_duration_ms);
    Ok(CanvasDragOverReceipt {
        schema_version: CANVAS_INTERACTION_SCHEMA_VERSION,
        interaction,
        plan: plan.flatten(),
        timings: CanvasDragOverTimings {
            emitted_at_ms,
            rust_received_at_ms,
            rust_completed_at_ms,
            input_to_plan_duration_ms,
            input_to_first_allowed_plan_ms,
            rust_duration_ms: rust_started.elapsed().as_millis().min(u64::MAX as u128) as u64,
        },
    })
}

#[tauri::command]
pub fn resolve_canvas_hover_intent(
    input: CanvasInteractionResolveRequest,
    state: State<AppState>,
) -> Result<CanvasHoverReceipt, String> {
    let rust_started = Instant::now();
    let rust_received_at_ms = wall_clock_ms();
    let authorized_scope_id = authorize_canvas_edit_scope(
        &input.request,
        input.edit_scope_grant.as_ref(),
        state.inner(),
    )?;
    let document_epoch = input.request.identity.document_epoch;
    let (interaction, projection) = state.canvas_interaction.resolve_pointer_hover(
        authorized_scope_id.as_deref(),
        &input.request,
        |snapshot, active_document_path, receipt| {
            let editor_node_id = receipt
                .target
                .as_ref()
                .map(|target| target.editor_node_id.as_str());
            let (hover, changed) = state.selection_coordinator.apply_hover(
                snapshot,
                active_document_path,
                editor_node_id,
                document_epoch,
            )?;
            Ok(CanvasHoverProjection { changed, hover })
        },
    )?;
    let rust_completed_at_ms = wall_clock_ms();
    let emitted_at_ms = input.request.emitted_at_ms;
    Ok(CanvasHoverReceipt {
        schema_version: CANVAS_INTERACTION_SCHEMA_VERSION,
        interaction,
        projection,
        timings: CanvasHoverTimings {
            emitted_at_ms,
            rust_received_at_ms,
            rust_completed_at_ms,
            input_to_projection_duration_ms: if emitted_at_ms == 0 {
                0
            } else {
                rust_completed_at_ms.saturating_sub(emitted_at_ms)
            },
            rust_duration_ms: rust_started.elapsed().as_millis().min(u64::MAX as u128) as u64,
        },
    })
}

fn wall_clock_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}

#[tauri::command]
pub fn apply_selection_intent(
    input: SelectionCoordinatorRequest,
    state: State<AppState>,
) -> Result<SelectionCoordinatorSnapshot, String> {
    require_selection_schema(input.schema_version)?;
    if !matches!(
        &input.intent,
        SelectionIntent::SelectSourcePosition { .. }
            | SelectionIntent::SelectCssSourceRule { .. }
            | SelectionIntent::SetFocus { .. }
            | SelectionIntent::Rebase
    ) {
        if let Some(context) = state
            .canvas_interaction
            .selection_context(&input.identity, &input.route)?
        {
            let authorized_scope_id = authorize_selection_edit_scope(
                &context.snapshot,
                context.active_document_path.as_deref(),
                &input.route,
                input.edit_scope_grant.as_ref(),
                state.inner(),
            )?;
            let intent = selection_intent_with_access(
                &context.snapshot,
                input.intent,
                authorized_scope_id.as_deref(),
            );
            return state.selection_coordinator.apply(
                &context.snapshot,
                context.active_document_path.as_deref(),
                None,
                intent,
            );
        }
    }
    let snapshot_request = EditorNavigationSnapshotRequest {
        identity: input.identity,
        route: input.route,
        active_document_path: input.active_document_path,
        preview_context_render_instance_id: input.preview_context_render_instance_id,
    };
    let context = resolve_editor_navigation_context(&snapshot_request, state.inner())?;
    let intent = selection_intent_from_project_model(&context.model, input.intent)?;
    let authorized_scope_id = authorize_selection_edit_scope(
        &context.snapshot,
        context.active_document_path.as_deref(),
        &context.snapshot.route,
        input.edit_scope_grant.as_ref(),
        state.inner(),
    )?;
    let intent =
        selection_intent_with_access(&context.snapshot, intent, authorized_scope_id.as_deref());
    let receipt = state.selection_coordinator.apply(
        &context.snapshot,
        context.active_document_path.as_deref(),
        Some(&context.model.source_graph),
        intent,
    )?;
    publish_project_model_if_current(state.inner(), &context.build_context, context.model)?;
    Ok(receipt)
}

fn authorize_selection_edit_scope(
    snapshot: &EditorNavigationSnapshot,
    active_document_path: Option<&str>,
    route: &str,
    edit_scope_grant: Option<&EditScopeGrant>,
    state: &AppState,
) -> Result<Option<String>, String> {
    let Some(grant) = edit_scope_grant else {
        return Ok(None);
    };
    let active_document_path = active_document_path.ok_or_else(|| {
        "SelectionCoordinator cere un document activ pentru EditScopeGrant.".to_string()
    })?;
    state.editor_navigation.require_edit_scope_grant(
        grant,
        &snapshot.identity,
        &snapshot.model_revision,
        route,
        active_document_path,
        &grant.scope_id,
        EditScopeOperation::InspectSharedDefinition,
    )?;
    Ok(Some(grant.scope_id.clone()))
}

fn selection_intent_with_access(
    snapshot: &EditorNavigationSnapshot,
    intent: SelectionIntent,
    authorized_edit_scope_id: Option<&str>,
) -> SelectionIntent {
    let resolve = |editor_node_id: String| {
        editor_navigation_access_node(snapshot, &editor_node_id, authorized_edit_scope_id)
            .map(|node| node.id.clone())
            .unwrap_or(editor_node_id)
    };
    match intent {
        SelectionIntent::SelectEditorNode { editor_node_id } => SelectionIntent::SelectEditorNode {
            editor_node_id: resolve(editor_node_id),
        },
        SelectionIntent::ToggleEditorNode { editor_node_id } => SelectionIntent::ToggleEditorNode {
            editor_node_id: resolve(editor_node_id),
        },
        SelectionIntent::ExtendRangeToEditorNode { editor_node_id } => {
            SelectionIntent::ExtendRangeToEditorNode {
                editor_node_id: resolve(editor_node_id),
            }
        }
        SelectionIntent::SetPrimaryEditorNode { editor_node_id } => {
            SelectionIntent::SetPrimaryEditorNode {
                editor_node_id: resolve(editor_node_id),
            }
        }
        SelectionIntent::SetHover {
            editor_node_id,
            document_epoch,
        } => SelectionIntent::SetHover {
            editor_node_id: resolve(editor_node_id),
            document_epoch,
        },
        other => other,
    }
}

#[tauri::command]
pub fn read_selection_snapshot(
    input: SelectionCoordinatorReadRequest,
    state: State<AppState>,
) -> Result<SelectionCoordinatorSnapshot, String> {
    require_selection_schema(input.schema_version)?;
    let snapshot_request = EditorNavigationSnapshotRequest {
        identity: input.identity,
        route: input.route,
        active_document_path: input.active_document_path,
        preview_context_render_instance_id: input.preview_context_render_instance_id,
    };
    let context = resolve_editor_navigation_context(&snapshot_request, state.inner())?;
    let receipt = state.selection_coordinator.apply(
        &context.snapshot,
        context.active_document_path.as_deref(),
        Some(&context.model.source_graph),
        SelectionIntent::Rebase,
    )?;
    publish_project_model_if_current(state.inner(), &context.build_context, context.model)?;
    Ok(receipt)
}

#[tauri::command]
pub fn accept_selection_observation(
    input: SelectionObservationInput,
    state: State<AppState>,
) -> Result<SelectionObservationReceipt, String> {
    state.selection_coordinator.accept_observation(input)
}

#[tauri::command]
pub fn read_editor_navigation_snapshot(
    input: EditorNavigationSnapshotRequest,
    app: AppHandle,
    state: State<AppState>,
) -> Result<Arc<EditorNavigationSnapshot>, String> {
    let context = resolve_editor_navigation_context(&input, state.inner())?;
    publish_project_model_if_current(state.inner(), &context.build_context, context.model.clone())?;
    finalize_initial_frontend_surface(&app, state.inner(), &input, &context)?;
    Ok(context.snapshot)
}

fn finalize_initial_frontend_surface(
    app: &AppHandle,
    state: &AppState,
    input: &EditorNavigationSnapshotRequest,
    context: &EditorNavigationContext,
) -> Result<(), String> {
    let lifecycle = state.project_lifecycle.snapshot()?;
    let Some(active) = lifecycle.active_session else {
        return Ok(());
    };
    if !matches!(active.readiness, ActiveProjectReadiness::FinalizingFrontend) {
        return Ok(());
    }
    if active.project_root != input.identity.project_root
        || active.runtime_session_id != input.identity.runtime_session_id
    {
        return Err(
            "Suprafața frontend inițială aparține altei sesiuni ProjectLifecycle.".to_string(),
        );
    }
    if let Some(active_document_path) = context.active_document_path.as_deref() {
        if !context.snapshot.route.starts_with("/__pana_workbench/") {
            return Err(
                "Suprafața frontend inițială a unui template cere ruta finală Workbench."
                    .to_string(),
            );
        }
        let focused_document = context
            .snapshot
            .focused_view
            .as_ref()
            .map(|view| view.active_document_path.as_str());
        if !focused_document.is_some_and(|focused| same_project_path(focused, active_document_path))
        {
            return Err(
                "Suprafața frontend inițială nu a confirmat documentul activ din Workbench."
                    .to_string(),
            );
        }
    }
    let lifecycle = state.project_lifecycle.set_readiness(
        &active.project_root,
        &active.runtime_session_id,
        ActiveProjectReadiness::Ready,
        "initial_frontend_surface_ready",
    )?;
    let _ = append_event(
        app,
        KernelLogEvent::new(
            KernelLogLevel::Info,
            KernelEventKind::ProjectLifecycleTransition,
            "project_lifecycle",
            "project_transition",
            "initial_frontend_surface_ready",
            Some(input.identity.transaction_id.clone()),
            "ProjectLifecycle a confirmat documentul, ruta Canvas și navigatorul semantic inițial.",
            None,
        )
        .with_attribute("projectRoot", &active.project_root)
        .with_attribute("sessionId", &active.runtime_session_id)
        .with_attribute("route", &context.snapshot.route)
        .with_attribute(
            "activeDocumentPath",
            context.active_document_path.as_deref().unwrap_or("-"),
        ),
    );
    let _ = app.emit("project-lifecycle-changed", lifecycle);
    Ok(())
}

fn require_selection_schema(schema_version: u32) -> Result<(), String> {
    if schema_version == SELECTION_COORDINATOR_SCHEMA_VERSION {
        Ok(())
    } else {
        Err("SelectionCoordinator folosește o versiune incompatibilă a protocolului.".to_string())
    }
}

fn selection_intent_from_project_model(
    model: &ProjectModel,
    intent: SelectionIntent,
) -> Result<SelectionIntent, String> {
    match intent {
        SelectionIntent::SelectSourcePosition {
            file,
            offset,
            viewport,
        } => {
            let Some(source) = project_model_file(model, &file) else {
                return Ok(SelectionIntent::SelectSourcePosition {
                    file,
                    offset,
                    viewport,
                });
            };
            match source.kind {
                ProjectModelFileKind::Style => {
                    let target = selector_source_target_at_offset(&source.contents, offset)
                        .ok_or_else(|| {
                            "SelectionCoordinator nu găsește un selector CSS/SCSS la poziția din cod."
                                .to_string()
                        })?;
                    Ok(SelectionIntent::SelectCssSourceRule {
                        file: source.relative_path.clone(),
                        selector: target.selector,
                        viewport,
                        range: Some(target.range),
                    })
                }
                ProjectModelFileKind::Script => Ok(SelectionIntent::SetFocus {
                    focus: crate::kernel::selection_coordinator::SelectionFocus::JsBehavior {
                        file: source.relative_path.clone(),
                        behavior_id: None,
                    },
                    expected_selection_revision: None,
                    expected_selection: None,
                    intent_sequence: None,
                }),
                _ => Ok(SelectionIntent::SelectSourcePosition {
                    file,
                    offset,
                    viewport,
                }),
            }
        }
        SelectionIntent::SelectCssSourceRule {
            file,
            selector,
            viewport,
            ..
        } => {
            let source = project_model_file(model, &file).ok_or_else(|| {
                format!("SelectionCoordinator nu găsește fișierul CSS/SCSS {file} în ProjectModel.")
            })?;
            if source.kind != ProjectModelFileKind::Style {
                return Err(format!(
                    "SelectionCoordinator a refuzat ținta CSS deoarece {file} nu este un fișier de stil."
                ));
            }
            let target = selector_source_target(&source.contents, &selector).ok_or_else(|| {
                format!(
                    "SelectionCoordinator nu găsește selectorul {selector} în {}.",
                    source.relative_path
                )
            })?;
            Ok(SelectionIntent::SelectCssSourceRule {
                file: source.relative_path.clone(),
                selector: target.selector,
                viewport,
                range: Some(target.range),
            })
        }
        SelectionIntent::SetFocus {
            focus,
            expected_selection_revision,
            expected_selection,
            intent_sequence,
        } => {
            use crate::kernel::selection_coordinator::SelectionFocus;
            let focus = match focus {
                SelectionFocus::CssRule {
                    file,
                    selector,
                    viewport,
                    ..
                } => {
                    let range = project_model_file(model, &file).and_then(|source| {
                        selector_source_target(&source.contents, &selector)
                            .map(|target| target.range)
                    });
                    SelectionFocus::CssRule {
                        file,
                        selector,
                        viewport,
                        range,
                    }
                }
                SelectionFocus::CssProperty {
                    file,
                    selector,
                    property,
                    viewport,
                    ..
                } => {
                    let range = project_model_file(model, &file).and_then(|source| {
                        selector_source_target(&source.contents, &selector)
                            .map(|target| target.range)
                    });
                    SelectionFocus::CssProperty {
                        file,
                        selector,
                        property,
                        viewport,
                        range,
                    }
                }
                focus => focus,
            };
            Ok(SelectionIntent::SetFocus {
                focus,
                expected_selection_revision,
                expected_selection,
                intent_sequence,
            })
        }
        intent => Ok(intent),
    }
}

fn project_model_file<'a>(
    model: &'a ProjectModel,
    requested_file: &str,
) -> Option<&'a ProjectModelFile> {
    model
        .files
        .iter()
        .find(|source| same_project_path(&source.relative_path, requested_file))
}

#[tauri::command]
pub fn request_editor_edit_scope(
    input: EditorEditScopeRequest,
    state: State<AppState>,
) -> Result<EditScopeGrant, String> {
    if input.scope_id.trim().is_empty() || input.scope_id.len() > 512 {
        return Err("EditScopeGrant a refuzat un scope ID invalid.".to_string());
    }
    let snapshot_request = EditorNavigationSnapshotRequest {
        identity: input.identity.clone(),
        route: input.route,
        active_document_path: input.active_document_path,
        preview_context_render_instance_id: input.preview_context_render_instance_id,
    };
    let context = resolve_editor_navigation_context(&snapshot_request, state.inner())?;
    let active_document_path = context
        .active_document_path
        .as_deref()
        .ok_or_else(|| "EditScopeGrant cere un template activ în Workbench.".to_string())?;
    let node = editor_navigation_node(&context.snapshot, &input.scope_id).ok_or_else(|| {
        "EditScopeGrant nu găsește scope-ul în EditorNavigationSnapshot.".to_string()
    })?;
    let grant = state.editor_navigation.issue_edit_scope_grant(
        &context.snapshot.identity,
        &context.snapshot.model_revision,
        &context.snapshot.route,
        active_document_path,
        node,
    )?;
    publish_project_model_if_current(state.inner(), &context.build_context, context.model)?;
    Ok(grant)
}

#[tauri::command]
pub fn plan_editor_move(
    input: EditorMovePlanRequest,
    state: State<AppState>,
) -> Result<EditorMovePlan, String> {
    require_editor_node_id(&input.source_node_id)?;
    require_editor_node_id(&input.target_node_id)?;
    let snapshot_request = EditorNavigationSnapshotRequest {
        identity: input.identity,
        route: input.route,
        active_document_path: input.active_document_path,
        preview_context_render_instance_id: input.preview_context_render_instance_id,
    };
    let context = resolve_editor_navigation_context(&snapshot_request, state.inner())?;
    let decision = build_editor_move_plan_with_slot(
        &state.editor_navigation,
        &context.snapshot,
        &context.model,
        &input.source_node_id,
        &input.target_node_id,
        input.position,
        input.edit_scope_grant.as_ref(),
        input.native_block_slot,
    );
    let plan = state
        .editor_navigation
        .issue_editor_move_decision(decision)?;
    publish_project_model_if_current(state.inner(), &context.build_context, context.model)?;
    Ok(plan)
}

#[tauri::command(async)]
pub fn commit_editor_move(
    app: AppHandle,
    input: EditorMoveCommitRequest,
    state: State<'_, AppState>,
) -> Result<EditorMoveExecutionReceipt, String> {
    let rust_started = Instant::now();
    let rust_received_at_ms = wall_clock_ms();
    if input.plan_token.trim().is_empty() || input.plan_token.len() > 256 {
        return Err("PlanEditorMove a refuzat un token invalid.".to_string());
    }
    let context = state
        .canvas_interaction
        .planning_context(&input.identity, &input.route)?;
    let active_document_path = context
        .active_document_path
        .as_deref()
        .ok_or_else(|| "PlanEditorMove cere un template activ în Workbench.".to_string())?;
    if input.active_document_path.as_deref() != Some(active_document_path) {
        return Err("PlanEditorMove a refuzat alt document activ la commit.".to_string());
    }
    if input.preview_context_render_instance_id.as_deref()
        != context
            .snapshot
            .focused_view
            .as_ref()
            .and_then(|view| view.preview_context_render_instance_id.as_deref())
    {
        return Err("PlanEditorMove a refuzat alt context randat la commit.".to_string());
    }
    let plan_revalidation_started = Instant::now();
    let stored_decision = state.editor_navigation.consume_editor_move_decision(
        &input.plan_token,
        &context.snapshot.identity,
        &context.snapshot.model_revision,
        &context.snapshot.route,
        active_document_path,
    )?;
    let plan_revalidation_ms = plan_revalidation_started
        .elapsed()
        .as_millis()
        .min(u64::MAX as u128) as u64;
    let stored_plan = stored_decision.plan;
    if !stored_plan.allowed {
        return Err("PlanEditorMove permis a devenit blocat înainte de commit.".to_string());
    }
    let operation = stored_plan
        .operation
        .ok_or_else(|| "PlanEditorMove permis nu conține operația Rust.".to_string())?;
    let execution = stored_decision
        .execution
        .ok_or_else(|| "PlanEditorMove permis nu conține execuția Rust.".to_string())?;
    let command_identity = PreviewStructuralCommandIdentity {
        expected_project_root: input.identity.project_root.clone(),
        expected_session_id: input.identity.runtime_session_id.clone(),
        expected_selection: None,
    };
    let expected_workspace_revision = input.identity.workspace_revision;
    let expected_model_revision = context.snapshot.model_revision.clone();
    let plan_token = input.plan_token;
    let (mut receipt, commit_timings) =
        super::kernel_preview_pipeline::run_preview_structural_write_command_measured(
            &app,
            &state,
            &command_identity,
            "Editor semantic move",
            |write_context, workspace| {
                if write_context.workspace_revision != expected_workspace_revision {
                    return Err(format!(
                        "PlanEditorMove a expirat: workspace revision {} a devenit {}.",
                        expected_workspace_revision, write_context.workspace_revision
                    ));
                }
                if context.model.revision != expected_model_revision {
                    return Err(
                        "PlanEditorMove a expirat deoarece ProjectModel s-a schimbat.".to_string(),
                    );
                }
                if let Some(scope_id) = stored_plan.impact.edit_scope_id.as_deref() {
                    let grant = input.edit_scope_grant.as_ref().ok_or_else(|| {
                        "PlanEditorMove cere EditScopeGrant la commit.".to_string()
                    })?;
                    state.editor_navigation.require_edit_scope_grant(
                        grant,
                        &input.identity,
                        &expected_model_revision,
                        &context.snapshot.route,
                        active_document_path,
                        scope_id,
                        crate::kernel::editor_navigation::EditScopeOperation::MoveHtmlInside,
                    )?;
                }
                execute_editor_move(
                    &write_context.session,
                    &write_context.root,
                    workspace,
                    &plan_token,
                    operation,
                    execution,
                    context.model.clone(),
                )
            },
        )?;
    let rust_completed_at_ms = wall_clock_ms();
    let plan_issued_at_ms = stored_plan.issued_at_ms.min(u64::MAX as u128) as u64;
    let patch_issued_to_receipt_ms = receipt
        .canvas_patch
        .as_ref()
        .map(|patch| rust_completed_at_ms.saturating_sub(patch.issued_at_ms));
    receipt.timings = Some(EditorMoveTimings {
        input_emitted_at_ms: input.input_emitted_at_ms,
        plan_issued_at_ms,
        rust_received_at_ms,
        rust_completed_at_ms,
        input_to_receipt_ms: if input.input_emitted_at_ms == 0 {
            0
        } else {
            rust_completed_at_ms.saturating_sub(input.input_emitted_at_ms)
        },
        pointer_up_to_commit_receipt_ms: if input.input_emitted_at_ms == 0 {
            0
        } else {
            rust_completed_at_ms.saturating_sub(input.input_emitted_at_ms)
        },
        plan_to_receipt_ms: rust_completed_at_ms.saturating_sub(plan_issued_at_ms),
        rust_command_ms: rust_started.elapsed().as_millis().min(u64::MAX as u128) as u64,
        patch_issued_to_receipt_ms,
        candidate_clone_ms: commit_timings.candidate_clone_ms,
        mutation_ms: commit_timings.mutation_ms,
        recovery_persist_ms: commit_timings.recovery_persist_ms,
        authority_publish_ms: commit_timings.authority_publish_ms,
        authority_transaction_ms: commit_timings.total_ms,
        plan_revalidation_ms,
        native_block_contract_ms: receipt.internal_timings.native_block_contract_ms,
        workspace_stage_ms: receipt.internal_timings.workspace_stage_ms,
        after_project_model_build_ms: receipt.internal_timings.after_project_model_build_ms,
        project_model_build_mode: receipt.internal_timings.project_model_build_mode.clone(),
        project_model_fallback_reason: receipt
            .internal_timings
            .project_model_fallback_reason
            .clone(),
        project_model_changed_path_count: receipt.internal_timings.project_model_changed_path_count,
        project_model_invalidated_template_count: receipt
            .internal_timings
            .project_model_invalidated_template_count,
        project_model_invalidated_page_count: receipt
            .internal_timings
            .project_model_invalidated_page_count,
        project_model_replaced_nodes: receipt.internal_timings.project_model_replaced_nodes,
        project_model_reused_nodes: receipt.internal_timings.project_model_reused_nodes,
        project_model_reused_relations: receipt.internal_timings.project_model_reused_relations,
        project_model_clone_ms: receipt.internal_timings.project_model_clone_ms,
        project_model_template_parse_us: receipt.internal_timings.project_model_template_parse_us,
        project_model_component_graph_us: receipt.internal_timings.project_model_component_graph_us,
        project_model_block_graph_us: receipt.internal_timings.project_model_block_graph_us,
        project_model_content_model_us: receipt.internal_timings.project_model_content_model_us,
        project_model_listing_items_us: receipt.internal_timings.project_model_listing_items_us,
        project_model_listing_items_reused: receipt
            .internal_timings
            .project_model_listing_items_reused,
        project_model_dynamic_widget_us: receipt.internal_timings.project_model_dynamic_widget_us,
        project_model_markdown_us: receipt.internal_timings.project_model_markdown_us,
        project_model_node_index_us: receipt.internal_timings.project_model_node_index_us,
    });
    append_editor_move_timing_event(&app, &receipt);
    if receipt.status == EditorMoveExecutionStatus::Committed {
        state.canvas_interaction.revoke_all();
        state.editor_navigation.revoke_all();
    }
    Ok(receipt)
}

fn append_editor_move_timing_event(app: &AppHandle, receipt: &EditorMoveExecutionReceipt) {
    if receipt.status != EditorMoveExecutionStatus::Committed {
        return;
    }
    let Some(timings) = receipt.timings.as_ref() else {
        return;
    };
    let event = KernelLogEvent::new(
        KernelLogLevel::Info,
        KernelEventKind::PreviewEditorMoveCommitted,
        "preview_projection",
        "editor_move",
        "editor_move.commit",
        receipt
            .workspace_mutation
            .as_ref()
            .and_then(|mutation| mutation.transaction_id.clone()),
        "Editor move committed by Rust authority.",
        None,
    )
    .with_attribute("projectRoot", &receipt.project_root)
    .with_attribute("runtimeSessionId", &receipt.runtime_session_id)
    .with_attribute("operation", format!("{:?}", receipt.operation))
    .with_attribute("inputToReceiptMs", timings.input_to_receipt_ms)
    .with_attribute(
        "pointerUpToCommitReceiptMs",
        timings.pointer_up_to_commit_receipt_ms,
    )
    .with_attribute("planToReceiptMs", timings.plan_to_receipt_ms)
    .with_attribute("rustCommandMs", timings.rust_command_ms)
    .with_attribute("candidateCloneMs", timings.candidate_clone_ms)
    .with_attribute("mutationMs", timings.mutation_ms)
    .with_attribute("recoveryPersistMs", timings.recovery_persist_ms)
    .with_attribute("authorityPublishMs", timings.authority_publish_ms)
    .with_attribute("authorityTransactionMs", timings.authority_transaction_ms)
    .with_attribute("planRevalidationMs", timings.plan_revalidation_ms)
    .with_attribute("nativeBlockContractMs", timings.native_block_contract_ms)
    .with_attribute("workspaceStageMs", timings.workspace_stage_ms)
    .with_attribute(
        "afterProjectModelBuildMs",
        timings.after_project_model_build_ms,
    )
    .with_attribute("projectModelBuildMode", &timings.project_model_build_mode)
    .with_attribute(
        "projectModelFallbackReason",
        timings.project_model_fallback_reason.clone(),
    )
    .with_attribute(
        "projectModelChangedPathCount",
        timings.project_model_changed_path_count,
    )
    .with_attribute(
        "projectModelInvalidatedTemplateCount",
        timings.project_model_invalidated_template_count,
    )
    .with_attribute(
        "projectModelInvalidatedPageCount",
        timings.project_model_invalidated_page_count,
    )
    .with_attribute(
        "projectModelReplacedNodes",
        timings.project_model_replaced_nodes,
    )
    .with_attribute(
        "projectModelReusedNodes",
        timings.project_model_reused_nodes,
    )
    .with_attribute(
        "projectModelReusedRelations",
        timings.project_model_reused_relations,
    )
    .with_attribute("projectModelCloneMs", timings.project_model_clone_ms)
    .with_attribute(
        "projectModelTemplateParseUs",
        timings.project_model_template_parse_us,
    )
    .with_attribute(
        "projectModelComponentGraphUs",
        timings.project_model_component_graph_us,
    )
    .with_attribute(
        "projectModelBlockGraphUs",
        timings.project_model_block_graph_us,
    )
    .with_attribute(
        "projectModelContentModelUs",
        timings.project_model_content_model_us,
    )
    .with_attribute(
        "projectModelListingItemsUs",
        timings.project_model_listing_items_us,
    )
    .with_attribute(
        "projectModelListingItemsReused",
        timings.project_model_listing_items_reused,
    )
    .with_attribute(
        "projectModelDynamicWidgetUs",
        timings.project_model_dynamic_widget_us,
    )
    .with_attribute("projectModelMarkdownUs", timings.project_model_markdown_us)
    .with_attribute(
        "projectModelNodeIndexUs",
        timings.project_model_node_index_us,
    )
    .with_attribute("patchIssuedToReceiptMs", timings.patch_issued_to_receipt_ms)
    .with_attribute("canvasPatchIssued", receipt.canvas_patch.is_some());
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _ = append_event(&app, event);
    });
}

fn resolve_editor_navigation_context(
    input: &EditorNavigationSnapshotRequest,
    state: &AppState,
) -> Result<EditorNavigationContext, String> {
    #[cfg(debug_assertions)]
    let started = Instant::now();
    let route = require_navigation_route(&input.route)?;
    let (root, session, build_context) = capture_project_model_build_context(state)?;
    if session.project_root != input.identity.project_root
        || session.runtime_instance_id() != input.identity.runtime_session_id
        || build_context.projection().revision != input.identity.workspace_revision
    {
        return Err(format!(
            "EditorNavigationSnapshot a refuzat identitatea stale pentru workspace revision {}.",
            input.identity.workspace_revision
        ));
    }
    let cached_model = {
        let workspace = state.project_workspace.lock().map_err(|_| {
            "Nu am putut citi cache-ul ProjectModel pentru EditorNavigationSnapshot.".to_string()
        })?;
        let workspace = workspace.as_ref().ok_or_else(|| {
            "ProjectWorkspace lipsește pentru EditorNavigationSnapshot.".to_string()
        })?;
        workspace.require_current_projection(build_context.projection())?;
        let cached_model = if workspace.project_model_source_revision
            == Some(build_context.projection().revision)
        {
            workspace.project_model.clone()
        } else {
            None
        };
        cached_model
    };
    #[cfg(debug_assertions)]
    let model_cache_hit = cached_model.is_some();
    let model = match cached_model {
        Some(model) => model,
        None => build_project_model_from_context(&root, &build_context)?,
    };
    let active_workbench_document =
        authoritative_active_document_path(state, &session, input.active_document_path.as_deref())?;
    let active_document_path = active_workbench_document
        .as_deref()
        .filter(|path| {
            model
                .source_graph
                .templates
                .iter()
                .any(|template| same_project_path(&template.file, path))
        })
        .map(str::to_string);
    let generation = {
        let engine = state
            .preview_engine
            .lock()
            .map_err(|_| "Motorul Preview embedded este indisponibil.".to_string())?;
        let engine = engine.as_ref().ok_or_else(|| {
            "EditorNavigationSnapshot cere o generație Preview activă.".to_string()
        })?;
        engine
            .generation_for_canvas_identity(&input.identity)?
            .ok_or_else(|| {
                "EditorNavigationSnapshot nu mai găsește generația Canvas solicitată.".to_string()
            })?
    };
    if !generation.owner_matches(
        &input.identity.project_root,
        &input.identity.runtime_session_id,
    ) || generation.workspace_revision != input.identity.workspace_revision
        || generation.preview_revision != input.identity.preview_revision
        || generation.canvas_transaction.identity != input.identity
    {
        return Err(
            "EditorNavigationSnapshot a refuzat o generație Preview cu identitate diferită."
                .to_string(),
        );
    }
    let graph = graph_for_surface(&generation, &route)?;
    let cached_snapshot = state.editor_navigation.cached_snapshot(
        &input.identity,
        &route,
        active_document_path.as_deref(),
        input.preview_context_render_instance_id.as_deref(),
    )?;
    #[cfg(debug_assertions)]
    let snapshot_cache_hit = cached_snapshot.is_some();
    let snapshot = match cached_snapshot {
        Some(snapshot) => snapshot,
        None => {
            let snapshot = Arc::new(build_editor_navigation_snapshot(
                input.identity.clone(),
                &route,
                &model,
                &graph,
                active_document_path.as_deref(),
                input.preview_context_render_instance_id.as_deref(),
            )?);
            state.editor_navigation.cache_snapshot(
                active_document_path.as_deref(),
                input.preview_context_render_instance_id.as_deref(),
                Arc::clone(&snapshot),
            )?;
            snapshot
        }
    };
    #[cfg(debug_assertions)]
    eprintln!(
        "[Pană Studio][perf] editor_navigation model_cache_hit={} snapshot_cache_hit={} total_ms={} route={} document={}",
        model_cache_hit,
        snapshot_cache_hit,
        started.elapsed().as_millis(),
        route,
        active_document_path.as_deref().unwrap_or("-")
    );
    Ok(EditorNavigationContext {
        build_context,
        model,
        snapshot,
        active_document_path,
    })
}

fn authoritative_active_document_path(
    state: &AppState,
    session: &crate::kernel::project_session::ProjectSessionSnapshot,
    requested: Option<&str>,
) -> Result<Option<String>, String> {
    let workbench = state.workbench.read(session)?;
    let active_group = workbench
        .groups
        .iter()
        .find(|group| group.group_id == workbench.active_group_id);
    let active_document = active_group
        .and_then(|group| {
            group.active_document_id.as_deref().and_then(|document_id| {
                group
                    .documents
                    .iter()
                    .find(|document| document.document_id == document_id)
            })
        })
        .map(|document| document.relative_path.clone());

    if let Some(requested) = requested {
        let requested = normalize_project_relative_path(requested)?;
        let Some(active_document) = active_document.as_deref() else {
            return Err(
                "EditorNavigationSnapshot a refuzat documentul declarat: Workbench nu are document activ."
                    .to_string(),
            );
        };
        if !same_project_path(active_document, &requested) {
            return Err(format!(
                "EditorNavigationSnapshot a refuzat documentul declarat {requested:?}; Workbench deține {active_document:?}."
            ));
        }
    }
    Ok(active_document)
}

fn same_project_path(left: &str, right: &str) -> bool {
    left.trim_start_matches('/').replace('\\', "/")
        == right.trim_start_matches('/').replace('\\', "/")
}

fn graph_for_surface(
    generation: &crate::preview::ActivePreviewGeneration,
    route: &str,
) -> Result<CanvasGraph, String> {
    if route.starts_with("/__pana_workbench/") {
        let workbench = generation
            .workbench_content
            .read()
            .map_err(|_| "Registrul Context de template este indisponibil.".to_string())?;
        return workbench
            .iter()
            .find(|(candidate, _)| same_surface_route(candidate, route))
            .map(|(_, projection)| projection.graph.clone())
            .ok_or_else(|| {
                format!(
                    "EditorNavigationSnapshot nu găsește graful Workbench pentru ruta {route:?}."
                )
            });
    }
    Ok(generation.canvas_transaction.graph.clone())
}

fn require_navigation_route(route: &str) -> Result<String, String> {
    let route = route.split(['?', '#']).next().unwrap_or(route).trim();
    if route.len() > 2_048
        || route.contains('\\')
        || route.split('/').any(|segment| segment == "..")
    {
        return Err("EditorNavigationSnapshot a refuzat ruta Preview invalidă.".to_string());
    }
    if route.is_empty() {
        return Ok("/".to_string());
    }
    Ok(if route.starts_with('/') {
        route.to_string()
    } else {
        format!("/{route}")
    })
}

fn require_editor_node_id(node_id: &str) -> Result<(), String> {
    let node_id = node_id.trim();
    if node_id.is_empty()
        || node_id.len() > 1_024
        || !(node_id.starts_with("editor_boundary:")
            || node_id.starts_with("editor_render:")
            || node_id.starts_with("editor_source:"))
    {
        return Err("PlanEditorMove a refuzat un editor node ID invalid.".to_string());
    }
    Ok(())
}

fn same_surface_route(left: &str, right: &str) -> bool {
    left.trim_end_matches('/') == right.trim_end_matches('/')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::{
        project_session::{
            ProjectRootFingerprint, ProjectSessionScanSummary, ProjectSessionSnapshot,
        },
        workbench::{WorkbenchGroupId, WorkbenchIdentity, WorkbenchIntent, WorkbenchSurface},
    };

    #[test]
    fn navigation_route_rejects_traversal_and_removes_query() {
        assert_eq!(
            require_navigation_route("blog/?revision=2").unwrap(),
            "/blog/"
        );
        assert!(require_navigation_route("/../secret").is_err());
        assert!(require_navigation_route("/bad\\route").is_err());
    }

    #[test]
    fn editor_node_ids_are_bounded_and_namespaced() {
        assert!(require_editor_node_id("editor_render:render-1").is_ok());
        assert!(require_editor_node_id("editor_boundary:scope-1").is_ok());
        assert!(require_editor_node_id("editor_source:source-1").is_ok());
        assert!(require_editor_node_id("source-node-1").is_err());
        assert!(require_editor_node_id("").is_err());
    }

    #[test]
    fn active_document_is_derived_from_workbench_and_frontend_mismatch_is_rejected() {
        let state = AppState::default();
        let session = ProjectSessionSnapshot {
            schema_version: 1,
            id: "project-session".to_string(),
            project_root: "/project".to_string(),
            zola_root: "/project".to_string(),
            session_dir: "/tmp/project-session".to_string(),
            manifest_path: "/tmp/project-session/manifest.json".to_string(),
            opened_at_ms: 7,
            last_seen_at_ms: 7,
            root_fingerprint: ProjectRootFingerprint {
                canonical_path: "/project".to_string(),
                modified_ms: 0,
                size: 0,
                readonly: false,
                unix_device: None,
                unix_inode: None,
            },
            scan_summary: ProjectSessionScanSummary {
                active_theme: None,
                file_count: 1,
                directory_count: 1,
            },
        };
        let initial = state.workbench.read(&session).unwrap();
        state
            .workbench
            .apply(
                &session,
                &WorkbenchIdentity {
                    expected_project_root: session.project_root.clone(),
                    expected_runtime_session_id: session.runtime_instance_id(),
                    expected_revision: initial.revision,
                },
                WorkbenchIntent::OpenDocument {
                    relative_path: "templates/index.html".to_string(),
                    group_id: WorkbenchGroupId::Primary,
                    surface: WorkbenchSurface::Visual,
                    presentation: crate::kernel::workbench::WorkbenchDocumentPresentation::Html,
                    pinned: false,
                },
            )
            .unwrap();

        assert_eq!(
            authoritative_active_document_path(&state, &session, Some("templates/index.html"),)
                .unwrap()
                .as_deref(),
            Some("templates/index.html"),
        );
        assert!(authoritative_active_document_path(
            &state,
            &session,
            Some("templates/layout.html"),
        )
        .is_err());
    }
}
