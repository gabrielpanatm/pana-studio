use std::path::Path;

use crate::{
    kernel::{
        editor_navigation::{
            EditorMoveExecution, EditorMoveExecutionReceipt, EditorMoveExecutionStatus,
            EditorMoveInternalTimings, EditorMoveOperation, EDITOR_MOVE_EXECUTION_SCHEMA_VERSION,
        },
        project_session::ProjectSessionSnapshot,
        project_workspace::{ProjectWorkspace, WorkspaceCanvasHistoryDelta},
    },
    project_model::{
        model::ProjectModel,
        move_engine::{plan_html_move, plan_html_move_in_edit_scope},
        tera_move_engine::plan_tera_move,
    },
};

use super::{
    html::issue_canvas_patch,
    runner::{
        confirmed_html_move_position, run_preview_structural_plan_with_model,
        PreviewStructuralPlanCommitted,
    },
    spec::EDITOR_MOVE_PLAN,
};
use crate::kernel::preview_projection::{CanvasPatchAnchor, CanvasPatchOperation};

pub(crate) struct EditorMoveExecutionOutcome {
    pub receipt: EditorMoveExecutionReceipt,
    pub after_model: Option<ProjectModel>,
}

pub(crate) fn execute_editor_move(
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    plan_token: &str,
    operation: EditorMoveOperation,
    execution: EditorMoveExecution,
    before_model: ProjectModel,
) -> Result<EditorMoveExecutionOutcome, String> {
    match execution {
        EditorMoveExecution::Html {
            intent,
            edit_scope_authorized,
            source_render_instance_id,
            target_render_instance_id,
        } => {
            let committed = run_preview_structural_plan_with_model(
                project_root,
                workspace,
                before_model,
                EDITOR_MOVE_PLAN,
                |before_model| {
                    if edit_scope_authorized {
                        plan_html_move_in_edit_scope(before_model, &intent)
                    } else {
                        plan_html_move(before_model, &intent)
                    }
                },
            )?;
            let PreviewStructuralPlanCommitted {
                before_model,
                patch,
                commit,
            } = match committed {
                Ok(committed) => committed,
                Err(blocked) => {
                    return Ok(blocked_outcome(
                        session,
                        plan_token,
                        operation,
                        Some(blocked.model_revision),
                        blocked.diagnostic.code,
                    ))
                }
            };
            let internal_timings = editor_move_internal_timings(&commit);
            let projected_source_id = commit
                .after_model
                .source_graph
                .nodes
                .iter()
                .any(|node| node.id == patch.resolved_source_id)
                .then(|| patch.resolved_source_id.clone());
            let confirmed_position = confirmed_html_move_position(&commit.after_model, &patch)?;
            let forward_operation = CanvasPatchOperation::Move {
                source: CanvasPatchAnchor::source_instance(
                    &patch.resolved_source_id,
                    source_render_instance_id.as_deref(),
                    intent.source_tag.as_deref(),
                ),
                target: CanvasPatchAnchor::source_instance(
                    &patch.resolved_target_id,
                    target_render_instance_id.as_deref(),
                    intent.target_tag.as_deref(),
                ),
                position: confirmed_position,
            };
            let inverse_operation = inverse_html_move_operation(
                &before_model,
                &commit.after_model,
                &patch.resolved_source_id,
                &patch.resolved_target_id,
                source_render_instance_id.as_deref(),
                target_render_instance_id.as_deref(),
            );
            let canvas_patch = issue_canvas_patch(
                session,
                &commit.workspace_mutation,
                &patch.before_revision,
                &patch.after_revision,
                forward_operation.clone(),
            )?;
            if let (Some(inverse), Some(transaction_id)) = (
                inverse_operation,
                commit.workspace_mutation.transaction_id.as_deref(),
            ) {
                workspace.attach_latest_canvas_history_delta(
                    transaction_id,
                    WorkspaceCanvasHistoryDelta {
                        before_model_revision: patch.before_revision.clone(),
                        after_model_revision: patch.after_revision.clone(),
                        forward: forward_operation,
                        inverse,
                    },
                )?;
            }
            let touched_files = vec![patch.file.clone()];
            Ok(EditorMoveExecutionOutcome {
                receipt: EditorMoveExecutionReceipt {
                    schema_version: EDITOR_MOVE_EXECUTION_SCHEMA_VERSION,
                    plan_token: plan_token.to_string(),
                    project_root: session.project_root.clone(),
                    runtime_session_id: session.runtime_instance_id(),
                    status: EditorMoveExecutionStatus::Committed,
                    operation,
                    model_revision: Some(commit.after_model.revision.clone()),
                    projected_source_id,
                    canvas_patch: Some(canvas_patch),
                    workspace_mutation: Some(commit.workspace_mutation),
                    touched_files,
                    diagnostic: None,
                    timings: None,
                    internal_timings,
                },
                after_model: Some(commit.after_model),
            })
        }
        EditorMoveExecution::Tera { intent } => {
            let committed = run_preview_structural_plan_with_model(
                project_root,
                workspace,
                before_model,
                EDITOR_MOVE_PLAN,
                |before_model| plan_tera_move(before_model, &intent),
            )?;
            let PreviewStructuralPlanCommitted { patch, commit, .. } = match committed {
                Ok(committed) => committed,
                Err(blocked) => {
                    return Ok(blocked_outcome(
                        session,
                        plan_token,
                        operation,
                        Some(blocked.model_revision),
                        blocked.diagnostic.code,
                    ))
                }
            };
            let touched_files = vec![patch.file.clone()];
            let internal_timings = editor_move_internal_timings(&commit);
            Ok(EditorMoveExecutionOutcome {
                receipt: EditorMoveExecutionReceipt {
                    schema_version: EDITOR_MOVE_EXECUTION_SCHEMA_VERSION,
                    plan_token: plan_token.to_string(),
                    project_root: session.project_root.clone(),
                    runtime_session_id: session.runtime_instance_id(),
                    status: EditorMoveExecutionStatus::Committed,
                    operation,
                    model_revision: Some(commit.after_model.revision.clone()),
                    projected_source_id: None,
                    canvas_patch: None,
                    workspace_mutation: Some(commit.workspace_mutation),
                    touched_files,
                    diagnostic: None,
                    timings: None,
                    internal_timings,
                },
                after_model: Some(commit.after_model),
            })
        }
    }
}

fn editor_move_internal_timings(
    commit: &super::super::structural_write::PreviewStructuralWriteCommit,
) -> EditorMoveInternalTimings {
    let build = &commit.project_model_build;
    EditorMoveInternalTimings {
        native_block_contract_ms: commit.timings.native_block_contract_ms,
        workspace_stage_ms: commit.timings.workspace_stage_ms,
        after_project_model_build_ms: commit.timings.after_project_model_build_ms,
        project_model_build_mode: build.mode.label().to_string(),
        project_model_fallback_reason: build.fallback_reason.clone(),
        project_model_changed_path_count: build.changed_paths.len(),
        project_model_invalidated_template_count: build.invalidated_template_files.len(),
        project_model_invalidated_page_count: build.invalidated_page_files.len(),
        project_model_replaced_nodes: build.replaced_nodes,
        project_model_reused_nodes: build.reused_nodes,
        project_model_reused_relations: build.reused_relations,
        project_model_clone_ms: build.model_clone_ms,
        project_model_template_parse_ms: build.template_parse_ms,
        project_model_component_graph_ms: build.component_graph_ms,
        project_model_block_graph_ms: build.block_graph_ms,
        project_model_tera_graph_ms: build.tera_graph_ms,
    }
}

fn inverse_html_move_operation(
    before_model: &ProjectModel,
    after_model: &ProjectModel,
    source_id: &str,
    current_target_id: &str,
    source_render_instance_id: Option<&str>,
    current_target_render_instance_id: Option<&str>,
) -> Option<CanvasPatchOperation> {
    let source = before_model.source_graph.node_by_id(source_id)?;
    let parent_id = source.parent.as_deref()?;
    let parent = before_model.source_graph.node_by_id(parent_id)?;
    let source_index = parent
        .children
        .iter()
        .position(|child| child == source_id)?;
    let (target_before_id, position) =
        if let Some(next_sibling) = parent.children.get(source_index.saturating_add(1)) {
            (
                next_sibling.as_str(),
                crate::project_model::move_engine::ProjectMovePosition::Before,
            )
        } else {
            (
                parent_id,
                crate::project_model::move_engine::ProjectMovePosition::Inside,
            )
        };
    let source_after_id = projected_node_id(after_model, source_id)?;
    let target_after_id = projected_node_id(after_model, target_before_id)?;
    let target_render_instance_id = (target_before_id == current_target_id)
        .then_some(current_target_render_instance_id)
        .flatten();
    Some(CanvasPatchOperation::Move {
        source: CanvasPatchAnchor::source_instance(
            &source_after_id,
            source_render_instance_id,
            None,
        ),
        target: CanvasPatchAnchor::source_instance(
            &target_after_id,
            target_render_instance_id,
            None,
        ),
        position,
    })
}

fn projected_node_id(after_model: &ProjectModel, before_id: &str) -> Option<String> {
    after_model
        .source_graph
        .nodes
        .iter()
        .any(|node| node.id == before_id)
        .then(|| before_id.to_string())
}

fn blocked_outcome(
    session: &ProjectSessionSnapshot,
    plan_token: &str,
    operation: EditorMoveOperation,
    model_revision: Option<String>,
    diagnostic: String,
) -> EditorMoveExecutionOutcome {
    EditorMoveExecutionOutcome {
        receipt: EditorMoveExecutionReceipt {
            schema_version: EDITOR_MOVE_EXECUTION_SCHEMA_VERSION,
            plan_token: plan_token.to_string(),
            project_root: session.project_root.clone(),
            runtime_session_id: session.runtime_instance_id(),
            status: EditorMoveExecutionStatus::Blocked,
            operation,
            model_revision,
            projected_source_id: None,
            canvas_patch: None,
            workspace_mutation: None,
            touched_files: Vec::new(),
            diagnostic: Some(diagnostic),
            timings: None,
            internal_timings: EditorMoveInternalTimings::default(),
        },
        after_model: None,
    }
}
