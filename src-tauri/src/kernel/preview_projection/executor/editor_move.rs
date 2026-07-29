use std::{collections::HashMap, path::Path};

use crate::{
    kernel::{
        editor_navigation::{
            EditorMoveExecution, EditorMoveExecutionReceipt, EditorMoveExecutionStatus,
            EditorMoveOperation, EDITOR_MOVE_EXECUTION_SCHEMA_VERSION,
        },
        project_session::ProjectSessionSnapshot,
        project_workspace::ProjectWorkspace,
    },
    project_model::{
        model::ProjectModel,
        move_engine::{plan_html_move, plan_html_move_in_edit_scope},
        tera_move_engine::plan_tera_move,
    },
};

use super::{
    html::{html_move_alias_updates, html_move_projected_source_id, issue_canvas_patch},
    runner::{run_preview_structural_plan, PreviewStructuralPlanCommitted},
    spec::EDITOR_MOVE_PLAN,
};
use crate::kernel::preview_projection::{CanvasPatchAnchor, CanvasPatchOperation};

pub(crate) struct EditorMoveExecutionOutcome {
    pub receipt: EditorMoveExecutionReceipt,
    pub after_model: Option<ProjectModel>,
    pub alias_updates: HashMap<String, String>,
}

pub(crate) fn execute_editor_move(
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    plan_token: &str,
    operation: EditorMoveOperation,
    execution: EditorMoveExecution,
) -> Result<EditorMoveExecutionOutcome, String> {
    match execution {
        EditorMoveExecution::Html {
            intent,
            edit_scope_authorized,
        } => {
            let exact_snapshot_aliases = HashMap::new();
            let committed = run_preview_structural_plan(
                project_root,
                workspace,
                EDITOR_MOVE_PLAN,
                |before_model| {
                    if edit_scope_authorized {
                        plan_html_move_in_edit_scope(before_model, &intent, &exact_snapshot_aliases)
                    } else {
                        plan_html_move(before_model, &intent, &exact_snapshot_aliases)
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
            let alias_updates = html_move_alias_updates(&before_model, &commit.after_model, &patch);
            let projected_source_id = html_move_projected_source_id(
                &commit.after_model,
                &patch.resolved_source_id,
                &alias_updates,
            );
            let canvas_patch = issue_canvas_patch(
                session,
                &commit.workspace_mutation,
                &patch.before_revision,
                &patch.after_revision,
                CanvasPatchOperation::Move {
                    source: CanvasPatchAnchor::source(
                        &patch.resolved_source_id,
                        None,
                        intent.source_tag.as_deref(),
                    ),
                    target: CanvasPatchAnchor::source(
                        &patch.resolved_target_id,
                        None,
                        intent.target_tag.as_deref(),
                    ),
                    position: intent.position,
                },
            )?;
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
                },
                after_model: Some(commit.after_model),
                alias_updates,
            })
        }
        EditorMoveExecution::Tera { intent } => {
            let committed = run_preview_structural_plan(
                project_root,
                workspace,
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
                },
                after_model: Some(commit.after_model),
                alias_updates: HashMap::new(),
            })
        }
    }
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
        },
        after_model: None,
        alias_updates: HashMap::new(),
    }
}
