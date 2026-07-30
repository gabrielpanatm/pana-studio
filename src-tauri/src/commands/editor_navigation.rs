use std::time::Instant;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::{
    css::rules::{selector_source_target, selector_source_target_at_offset},
    kernel::canvas_interaction::{
        CanvasInteractionBindingReceipt, CanvasInteractionIdentity, CanvasInteractionReceipt,
        CanvasInteractionRequest, CANVAS_INTERACTION_SCHEMA_VERSION,
    },
    kernel::editor_navigation::{
        build_editor_navigation_snapshot, editor_navigation_node,
        plan_editor_move as build_editor_move_plan, EditScopeGrant, EditScopeOperation,
        EditorMoveExecutionReceipt, EditorMoveExecutionStatus, EditorMovePlan,
        EditorNavigationSnapshot,
    },
    kernel::preview_projection::{execute_editor_move, PreviewStructuralCommandIdentity},
    kernel::project_path::normalize_project_relative_path,
    kernel::selection_coordinator::{
        HoverSnapshot, SelectionCoordinatorSnapshot, SelectionIntent, SelectionObservationInput,
        SelectionObservationReceipt, SELECTION_COORDINATOR_SCHEMA_VERSION,
    },
    preview::{CanvasGraph, CanvasProjectionIdentity},
    project_model::{
        cache::{
            capture_project_model_build_lease, publish_project_model_if_current,
            ProjectModelBuildLease,
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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasHoverProjection {
    pub changed: bool,
    pub hover: Option<HoverSnapshot>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasHoverReceipt {
    pub schema_version: u32,
    pub interaction: CanvasInteractionReceipt,
    pub projection: Option<CanvasHoverProjection>,
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
    lease: ProjectModelBuildLease,
    model: ProjectModel,
    snapshot: EditorNavigationSnapshot,
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
        let receipt = state.canvas_interaction.bind_agent(
            &context.snapshot,
            context.active_document_path.as_deref(),
            input.identity,
        )?;
        state
            .selection_coordinator
            .bind_inspector_document(receipt.identity.clone())?;
        publish_project_model_if_current(state.inner(), &context.lease, context.model)?;
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
    let authorized_scope_id = authorize_canvas_edit_scope(&input, state.inner())?;
    state
        .canvas_interaction
        .resolve(authorized_scope_id.as_deref(), &input.request)
}

fn authorize_canvas_edit_scope(
    input: &CanvasInteractionResolveRequest,
    state: &AppState,
) -> Result<Option<String>, String> {
    if let Some(grant) = input.edit_scope_grant.as_ref() {
        let scope_context = state
            .canvas_interaction
            .scope_context(&input.request.identity)?;
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
pub fn resolve_canvas_hover_intent(
    input: CanvasInteractionResolveRequest,
    state: State<AppState>,
) -> Result<CanvasHoverReceipt, String> {
    let authorized_scope_id = authorize_canvas_edit_scope(&input, state.inner())?;
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
    Ok(CanvasHoverReceipt {
        schema_version: CANVAS_INTERACTION_SCHEMA_VERSION,
        interaction,
        projection,
    })
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
            | SelectionIntent::SetFocus { .. }
            | SelectionIntent::Rebase
    ) {
        if let Some(context) = state
            .canvas_interaction
            .selection_context(&input.identity, &input.route)?
        {
            return state.selection_coordinator.apply(
                &context.snapshot,
                context.active_document_path.as_deref(),
                None,
                input.intent,
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
    let receipt = state.selection_coordinator.apply(
        &context.snapshot,
        context.active_document_path.as_deref(),
        Some(&context.model.source_graph),
        intent,
    )?;
    publish_project_model_if_current(state.inner(), &context.lease, context.model)?;
    Ok(receipt)
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
    publish_project_model_if_current(state.inner(), &context.lease, context.model)?;
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
    state: State<AppState>,
) -> Result<EditorNavigationSnapshot, String> {
    let context = resolve_editor_navigation_context(&input, state.inner())?;
    publish_project_model_if_current(state.inner(), &context.lease, context.model)?;
    Ok(context.snapshot)
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
                    Ok(SelectionIntent::SetFocus {
                        focus: crate::kernel::selection_coordinator::SelectionFocus::CssRule {
                            file: source.relative_path.clone(),
                            selector: target.selector,
                            viewport,
                            range: Some(target.range),
                        },
                        expected_selection_revision: None,
                    })
                }
                ProjectModelFileKind::Script => Ok(SelectionIntent::SetFocus {
                    focus: crate::kernel::selection_coordinator::SelectionFocus::JsBehavior {
                        file: source.relative_path.clone(),
                        behavior_id: None,
                    },
                    expected_selection_revision: None,
                }),
                _ => Ok(SelectionIntent::SelectSourcePosition {
                    file,
                    offset,
                    viewport,
                }),
            }
        }
        SelectionIntent::SetFocus {
            focus,
            expected_selection_revision,
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
    publish_project_model_if_current(state.inner(), &context.lease, context.model)?;
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
    let decision = build_editor_move_plan(
        &state.editor_navigation,
        &context.snapshot,
        &context.model,
        &input.source_node_id,
        &input.target_node_id,
        input.position,
        input.edit_scope_grant.as_ref(),
    );
    let plan = state
        .editor_navigation
        .issue_editor_move_plan(decision.plan)?;
    publish_project_model_if_current(state.inner(), &context.lease, context.model)?;
    Ok(plan)
}

#[tauri::command(async)]
pub fn commit_editor_move(
    app: AppHandle,
    input: EditorMoveCommitRequest,
    state: State<'_, AppState>,
) -> Result<EditorMoveExecutionReceipt, String> {
    if input.plan_token.trim().is_empty() || input.plan_token.len() > 256 {
        return Err("PlanEditorMove a refuzat un token invalid.".to_string());
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
        .ok_or_else(|| "PlanEditorMove cere un template activ în Workbench.".to_string())?;
    let stored_plan = state.editor_navigation.consume_editor_move_plan(
        &input.plan_token,
        &context.snapshot.identity,
        &context.snapshot.model_revision,
        &context.snapshot.route,
        active_document_path,
    )?;
    let mut decision = build_editor_move_plan(
        &state.editor_navigation,
        &context.snapshot,
        &context.model,
        &stored_plan.source_node_id,
        &stored_plan.target_node_id,
        stored_plan.position,
        input.edit_scope_grant.as_ref(),
    );
    if !decision.plan.allowed || decision.plan.operation != stored_plan.operation {
        return Err(decision
            .plan
            .reason
            .unwrap_or_else(|| "PlanEditorMove nu mai este valid la commit.".to_string()));
    }
    let operation = stored_plan
        .operation
        .ok_or_else(|| "PlanEditorMove permis nu conține operația Rust.".to_string())?;
    let execution = decision
        .execution
        .take()
        .ok_or_else(|| "PlanEditorMove permis nu conține execuția Rust.".to_string())?;
    let command_identity = PreviewStructuralCommandIdentity {
        expected_project_root: input.identity.project_root.clone(),
        expected_session_id: input.identity.runtime_session_id.clone(),
        expected_selection: None,
    };
    let expected_workspace_revision = input.identity.workspace_revision;
    let expected_model_revision = context.snapshot.model_revision.clone();
    let plan_token = input.plan_token;
    let receipt = super::kernel_preview_pipeline::run_preview_structural_write_command(
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
            let lease = workspace.capture_projection_lease()?;
            let current_model =
                crate::project_model::build_project_model_from_workspace_projection(
                    &write_context.root,
                    &lease,
                )?;
            if current_model.revision != expected_model_revision {
                return Err(
                    "PlanEditorMove a expirat deoarece ProjectModel s-a schimbat.".to_string(),
                );
            }
            if let Some(scope_id) = stored_plan.impact.edit_scope_id.as_deref() {
                let grant = input
                    .edit_scope_grant
                    .as_ref()
                    .ok_or_else(|| "PlanEditorMove cere EditScopeGrant la commit.".to_string())?;
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
            )
        },
    )?;
    if receipt.status == EditorMoveExecutionStatus::Committed {
        state.canvas_interaction.revoke_all();
        state.editor_navigation.revoke_all();
    }
    Ok(receipt)
}

fn resolve_editor_navigation_context(
    input: &EditorNavigationSnapshotRequest,
    state: &AppState,
) -> Result<EditorNavigationContext, String> {
    let started = Instant::now();
    let route = require_navigation_route(&input.route)?;
    let (root, session, lease) = capture_project_model_build_lease(state)?;
    if session.project_root != input.identity.project_root
        || session.runtime_instance_id() != input.identity.runtime_session_id
        || lease.projection().revision != input.identity.workspace_revision
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
        workspace.require_current_projection(lease.projection())?;
        if workspace.project_model_source_revision == Some(lease.projection().revision) {
            workspace.project_model.clone()
        } else {
            None
        }
    };
    let model_cache_hit = cached_model.is_some();
    let model = match cached_model {
        Some(model) => model,
        None => crate::project_model::build_project_model_from_workspace_projection(
            &root,
            lease.projection(),
        )?,
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
    let snapshot_cache_hit = cached_snapshot.is_some();
    let snapshot = match cached_snapshot {
        Some(snapshot) => snapshot,
        None => {
            let snapshot = build_editor_navigation_snapshot(
                input.identity.clone(),
                &route,
                &model,
                &graph,
                active_document_path.as_deref(),
                input.preview_context_render_instance_id.as_deref(),
            )?;
            state.editor_navigation.cache_snapshot(
                active_document_path.as_deref(),
                input.preview_context_render_instance_id.as_deref(),
                &snapshot,
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
        lease,
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
