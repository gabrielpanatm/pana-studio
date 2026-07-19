use std::path::Path;

use tauri::AppHandle;

use crate::{
    kernel::{file_buffer_store::FileBufferStore, project_session::ProjectSessionSnapshot},
    project_model::{build_project_model, template_edit_gate::plan_template_edit_permission},
};

use super::super::{
    model::{
        PreviewProjectionDiagnostic, PreviewTemplateEditPermissionInput,
        PreviewTemplateEditPermissionReceipt,
    },
    structural_write::source_texts_from_store,
};
use super::{
    events::append_template_edit_permission_event,
    gate::require_preview_executor_intent,
    receipts::{
        blocked_template_edit_permission_receipt, granted_template_edit_permission_receipt,
    },
    spec::{TEMPLATE_EDIT_PERMISSION_INTENT, TEMPLATE_EDIT_PERMISSION_PLAN},
};

pub struct PreviewTemplateEditPermissionOutcome {
    pub receipt: PreviewTemplateEditPermissionReceipt,
}

pub fn execute_preview_template_edit_permission(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    store: &FileBufferStore,
    input: PreviewTemplateEditPermissionInput,
) -> Result<PreviewTemplateEditPermissionOutcome, String> {
    let intent_receipt = match require_preview_executor_intent(
        input.intent.clone(),
        session,
        TEMPLATE_EDIT_PERMISSION_INTENT,
    ) {
        Ok(intent_receipt) => intent_receipt,
        Err(blocked) => {
            let receipt = blocked_template_edit_permission_receipt(
                blocked.intent_receipt,
                None,
                TEMPLATE_EDIT_PERMISSION_INTENT.preflight_blocked_message,
                blocked.diagnostic,
            );
            append_template_edit_permission_event(app, session, &receipt, None);
            return Ok(PreviewTemplateEditPermissionOutcome { receipt });
        }
    };

    let source_texts = source_texts_from_store(store);
    let model = build_project_model(project_root, &source_texts)?;
    let plan = plan_template_edit_permission(&model, &input.edit_intent);
    let Some(grant) = plan.grant else {
        let diagnostic = PreviewProjectionDiagnostic::blocking(
            TEMPLATE_EDIT_PERMISSION_PLAN.blocked_code,
            plan.diagnostic
                .unwrap_or_else(|| TEMPLATE_EDIT_PERMISSION_PLAN.blocked_fallback.to_string()),
        );
        let receipt = blocked_template_edit_permission_receipt(
            intent_receipt,
            Some(plan.model_revision),
            TEMPLATE_EDIT_PERMISSION_PLAN.blocked_message,
            Some(diagnostic),
        );
        append_template_edit_permission_event(app, session, &receipt, None);
        return Ok(PreviewTemplateEditPermissionOutcome { receipt });
    };

    let receipt =
        granted_template_edit_permission_receipt(intent_receipt, model.revision.clone(), grant);
    append_template_edit_permission_event(app, session, &receipt, None);

    Ok(PreviewTemplateEditPermissionOutcome { receipt })
}
