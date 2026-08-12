use std::path::Path;

use tauri::AppHandle;

use crate::{
    kernel::{
        project_session::ProjectSessionSnapshot,
        project_workspace::{ProjectWorkspace, WorkspaceSourceTreeHistoryAction},
    },
    project_model::{
        model::ProjectModel, tera_delete_engine::plan_tera_delete,
        tera_insert_engine::plan_tera_insert_for_active_document,
    },
    source_graph::identity::{capture_source_forest_identity, capture_source_tree_identity},
};

use super::super::model::{
    PreviewTeraDeleteExecutionInput, PreviewTeraDeleteExecutionReceipt,
    PreviewTeraInsertDropExecutionInput, PreviewTeraInsertDropExecutionReceipt,
};
use super::{
    events::{append_tera_delete_event, append_tera_insert_drop_event},
    gate::require_preview_executor_intent,
    html::attach_source_tree_history,
    receipts::{
        blocked_tera_delete_receipt, blocked_tera_insert_drop_receipt,
        committed_tera_delete_receipt, committed_tera_insert_drop_receipt,
    },
    runner::{
        inserted_tera_source_nodes, run_preview_structural_plan, PreviewStructuralPlanCommitted,
    },
    spec::{TERA_DELETE_INTENT, TERA_DELETE_PLAN, TERA_INSERT_DROP_INTENT, TERA_INSERT_DROP_PLAN},
};

pub struct PreviewTeraInsertDropExecutionOutcome {
    pub receipt: PreviewTeraInsertDropExecutionReceipt,
    pub after_model: Option<ProjectModel>,
}

pub struct PreviewTeraDeleteExecutionOutcome {
    pub receipt: PreviewTeraDeleteExecutionReceipt,
    pub after_model: Option<ProjectModel>,
}

pub fn execute_preview_tera_insert_drop(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    input: PreviewTeraInsertDropExecutionInput,
    active_document_path: Option<&str>,
) -> Result<PreviewTeraInsertDropExecutionOutcome, String> {
    let intent_receipt = match require_preview_executor_intent(
        input.intent.clone(),
        session,
        TERA_INSERT_DROP_INTENT,
    ) {
        Ok(intent_receipt) => intent_receipt,
        Err(blocked) => {
            let receipt =
                blocked_tera_insert_drop_receipt(blocked.intent_receipt, None, blocked.diagnostic);
            append_tera_insert_drop_event(app, session, &receipt, None);
            return Ok(PreviewTeraInsertDropExecutionOutcome {
                receipt,
                after_model: None,
            });
        }
    };

    let committed = match run_preview_structural_plan(
        project_root,
        workspace,
        TERA_INSERT_DROP_PLAN,
        |before_model| {
            plan_tera_insert_for_active_document(
                before_model,
                &input.insert_intent,
                active_document_path,
            )
        },
    )? {
        Ok(committed) => committed,
        Err(blocked) => {
            let receipt = blocked_tera_insert_drop_receipt(
                intent_receipt,
                Some(blocked.model_revision),
                Some(blocked.diagnostic),
            );
            append_tera_insert_drop_event(app, session, &receipt, None);
            return Ok(PreviewTeraInsertDropExecutionOutcome {
                receipt,
                after_model: None,
            });
        }
    };

    let PreviewStructuralPlanCommitted {
        before_model,
        patch,
        commit,
    } = committed;
    let inserted = inserted_tera_source_nodes(&before_model, &commit.after_model, &patch)?;
    let inserted_source_ids = inserted
        .iter()
        .map(|node| node.id.clone())
        .collect::<Vec<_>>();
    let inserted_source_tree =
        capture_source_forest_identity(&commit.after_model.source_graph, &inserted_source_ids)?;
    attach_source_tree_history(
        workspace,
        &commit.workspace_mutation,
        WorkspaceSourceTreeHistoryAction::Inserted,
        Some(inserted_source_tree),
    )?;
    let receipt = committed_tera_insert_drop_receipt(
        intent_receipt,
        commit.after_model.revision.clone(),
        patch,
        commit.workspace_mutation,
    );
    append_tera_insert_drop_event(app, session, &receipt, None);

    Ok(PreviewTeraInsertDropExecutionOutcome {
        receipt,
        after_model: Some(commit.after_model),
    })
}

pub fn execute_preview_tera_delete(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    input: PreviewTeraDeleteExecutionInput,
) -> Result<PreviewTeraDeleteExecutionOutcome, String> {
    let intent_receipt =
        match require_preview_executor_intent(input.intent.clone(), session, TERA_DELETE_INTENT) {
            Ok(intent_receipt) => intent_receipt,
            Err(blocked) => {
                let receipt =
                    blocked_tera_delete_receipt(blocked.intent_receipt, None, blocked.diagnostic);
                append_tera_delete_event(app, session, &receipt, None);
                return Ok(PreviewTeraDeleteExecutionOutcome {
                    receipt,
                    after_model: None,
                });
            }
        };

    let committed = match run_preview_structural_plan(
        project_root,
        workspace,
        TERA_DELETE_PLAN,
        |before_model| plan_tera_delete(before_model, &input.delete_intent),
    )? {
        Ok(committed) => committed,
        Err(blocked) => {
            let receipt = blocked_tera_delete_receipt(
                intent_receipt,
                Some(blocked.model_revision),
                Some(blocked.diagnostic),
            );
            append_tera_delete_event(app, session, &receipt, None);
            return Ok(PreviewTeraDeleteExecutionOutcome {
                receipt,
                after_model: None,
            });
        }
    };

    let PreviewStructuralPlanCommitted {
        before_model,
        patch,
        commit,
    } = committed;
    let deleted_source_tree =
        capture_source_tree_identity(&before_model.source_graph, &patch.resolved_target_id)?;
    attach_source_tree_history(
        workspace,
        &commit.workspace_mutation,
        WorkspaceSourceTreeHistoryAction::Deleted,
        Some(deleted_source_tree),
    )?;
    let receipt = committed_tera_delete_receipt(
        intent_receipt,
        commit.after_model.revision.clone(),
        patch,
        commit.workspace_mutation,
    );
    append_tera_delete_event(app, session, &receipt, None);

    Ok(PreviewTeraDeleteExecutionOutcome {
        receipt,
        after_model: Some(commit.after_model),
    })
}
