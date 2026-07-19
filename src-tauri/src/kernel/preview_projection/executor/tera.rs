use std::path::Path;

use tauri::AppHandle;

use crate::{
    kernel::{project_session::ProjectSessionSnapshot, project_workspace::ProjectWorkspace},
    project_model::{
        model::ProjectModel, tera_delete_engine::plan_tera_delete,
        tera_insert_engine::plan_tera_insert, tera_move_engine::plan_tera_move,
    },
};

use super::super::model::{
    PreviewTeraDeleteExecutionInput, PreviewTeraDeleteExecutionReceipt,
    PreviewTeraInsertDropExecutionInput, PreviewTeraInsertDropExecutionReceipt,
    PreviewTeraMoveDropExecutionInput, PreviewTeraMoveDropExecutionReceipt,
};
use super::{
    events::{
        append_tera_delete_event, append_tera_insert_drop_event, append_tera_move_drop_event,
    },
    gate::require_preview_executor_intent,
    receipts::{
        blocked_tera_delete_receipt, blocked_tera_insert_drop_receipt,
        blocked_tera_move_drop_receipt, committed_tera_delete_receipt,
        committed_tera_insert_drop_receipt, committed_tera_move_drop_receipt,
    },
    runner::{run_preview_structural_plan, PreviewStructuralPlanCommitted},
    spec::{
        TERA_DELETE_INTENT, TERA_DELETE_PLAN, TERA_INSERT_DROP_INTENT, TERA_INSERT_DROP_PLAN,
        TERA_MOVE_DROP_INTENT, TERA_MOVE_DROP_PLAN,
    },
};

pub struct PreviewTeraInsertDropExecutionOutcome {
    pub receipt: PreviewTeraInsertDropExecutionReceipt,
    pub after_model: Option<ProjectModel>,
}

pub struct PreviewTeraMoveDropExecutionOutcome {
    pub receipt: PreviewTeraMoveDropExecutionReceipt,
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
) -> Result<PreviewTeraInsertDropExecutionOutcome, String> {
    let intent_receipt = match require_preview_executor_intent(
        input.intent.clone(),
        session,
        TERA_INSERT_DROP_INTENT,
    ) {
        Ok(intent_receipt) => intent_receipt,
        Err(blocked) => {
            let receipt = blocked_tera_insert_drop_receipt(
                blocked.intent_receipt,
                None,
                TERA_INSERT_DROP_INTENT.preflight_blocked_message,
                blocked.diagnostic,
            );
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
        |before_model| plan_tera_insert(before_model, &input.insert_intent),
    )? {
        Ok(committed) => committed,
        Err(blocked) => {
            let receipt = blocked_tera_insert_drop_receipt(
                intent_receipt,
                Some(blocked.model_revision),
                TERA_INSERT_DROP_PLAN.blocked_message,
                Some(blocked.diagnostic),
            );
            append_tera_insert_drop_event(app, session, &receipt, None);
            return Ok(PreviewTeraInsertDropExecutionOutcome {
                receipt,
                after_model: None,
            });
        }
    };

    let PreviewStructuralPlanCommitted { patch, commit, .. } = committed;
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

pub fn execute_preview_tera_move_drop(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    input: PreviewTeraMoveDropExecutionInput,
) -> Result<PreviewTeraMoveDropExecutionOutcome, String> {
    let intent_receipt =
        match require_preview_executor_intent(input.intent.clone(), session, TERA_MOVE_DROP_INTENT)
        {
            Ok(intent_receipt) => intent_receipt,
            Err(blocked) => {
                let receipt = blocked_tera_move_drop_receipt(
                    blocked.intent_receipt,
                    None,
                    TERA_MOVE_DROP_INTENT.preflight_blocked_message,
                    blocked.diagnostic,
                );
                append_tera_move_drop_event(app, session, &receipt, None);
                return Ok(PreviewTeraMoveDropExecutionOutcome {
                    receipt,
                    after_model: None,
                });
            }
        };

    let committed = match run_preview_structural_plan(
        project_root,
        workspace,
        TERA_MOVE_DROP_PLAN,
        |before_model| plan_tera_move(before_model, &input.move_intent),
    )? {
        Ok(committed) => committed,
        Err(blocked) => {
            let receipt = blocked_tera_move_drop_receipt(
                intent_receipt,
                Some(blocked.model_revision),
                TERA_MOVE_DROP_PLAN.blocked_message,
                Some(blocked.diagnostic),
            );
            append_tera_move_drop_event(app, session, &receipt, None);
            return Ok(PreviewTeraMoveDropExecutionOutcome {
                receipt,
                after_model: None,
            });
        }
    };

    let PreviewStructuralPlanCommitted { patch, commit, .. } = committed;
    let receipt = committed_tera_move_drop_receipt(
        intent_receipt,
        commit.after_model.revision.clone(),
        patch,
        commit.workspace_mutation,
    );
    append_tera_move_drop_event(app, session, &receipt, None);

    Ok(PreviewTeraMoveDropExecutionOutcome {
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
                let receipt = blocked_tera_delete_receipt(
                    blocked.intent_receipt,
                    None,
                    TERA_DELETE_INTENT.preflight_blocked_message,
                    blocked.diagnostic,
                );
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
                TERA_DELETE_PLAN.blocked_message,
                Some(blocked.diagnostic),
            );
            append_tera_delete_event(app, session, &receipt, None);
            return Ok(PreviewTeraDeleteExecutionOutcome {
                receipt,
                after_model: None,
            });
        }
    };

    let PreviewStructuralPlanCommitted { patch, commit, .. } = committed;
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
