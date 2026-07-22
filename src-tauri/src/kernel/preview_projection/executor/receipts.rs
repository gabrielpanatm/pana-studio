use crate::{
    kernel::project_workspace::ProjectWorkspaceMutationReceipt,
    project_model::{
        attribute_engine::ProjectHtmlAttributePatch, delete_engine::ProjectHtmlDeletePatch,
        duplicate_engine::ProjectHtmlDuplicatePatch, insert_engine::ProjectHtmlInsertPatch,
        move_engine::ProjectHtmlMovePatch, tag_engine::ProjectHtmlTagPatch,
        template_edit_gate::ProjectTemplateEditPermissionGrant,
        tera_delete_engine::ProjectTeraDeletePatch, tera_insert_engine::ProjectTeraInsertPatch,
        tera_move_engine::ProjectTeraMovePatch, text_engine::ProjectHtmlTextPatch,
    },
};

use super::super::model::{
    CanvasPatch, PreviewHtmlAttributesExecutionReceipt, PreviewHtmlAttributesExecutionStatus,
    PreviewHtmlDeleteExecutionReceipt, PreviewHtmlDeleteExecutionStatus,
    PreviewHtmlDuplicateExecutionReceipt, PreviewHtmlDuplicateExecutionStatus,
    PreviewHtmlInsertDropExecutionReceipt, PreviewHtmlInsertDropExecutionStatus,
    PreviewHtmlTagExecutionReceipt, PreviewHtmlTagExecutionStatus, PreviewHtmlTextExecutionReceipt,
    PreviewHtmlTextExecutionStatus, PreviewLayerDropExecutionReceipt,
    PreviewLayerDropExecutionStatus, PreviewProjectionDiagnostic, PreviewProjectionIntentReceipt,
    PreviewTemplateEditPermissionReceipt, PreviewTemplateEditPermissionStatus,
    PreviewTeraDeleteExecutionReceipt, PreviewTeraDeleteExecutionStatus,
    PreviewTeraInsertDropExecutionReceipt, PreviewTeraInsertDropExecutionStatus,
    PreviewTeraMoveDropExecutionReceipt, PreviewTeraMoveDropExecutionStatus,
    PREVIEW_HTML_ATTRIBUTES_EXECUTION_SCHEMA_VERSION, PREVIEW_HTML_DELETE_EXECUTION_SCHEMA_VERSION,
    PREVIEW_HTML_DUPLICATE_EXECUTION_SCHEMA_VERSION,
    PREVIEW_HTML_INSERT_DROP_EXECUTION_SCHEMA_VERSION, PREVIEW_HTML_TAG_EXECUTION_SCHEMA_VERSION,
    PREVIEW_HTML_TEXT_EXECUTION_SCHEMA_VERSION, PREVIEW_LAYER_DROP_EXECUTION_SCHEMA_VERSION,
    PREVIEW_TEMPLATE_EDIT_PERMISSION_SCHEMA_VERSION, PREVIEW_TERA_DELETE_EXECUTION_SCHEMA_VERSION,
    PREVIEW_TERA_INSERT_DROP_EXECUTION_SCHEMA_VERSION,
    PREVIEW_TERA_MOVE_DROP_EXECUTION_SCHEMA_VERSION,
};

pub(super) fn blocked_layer_drop_receipt(
    intent: PreviewProjectionIntentReceipt,
    model_revision: Option<String>,
    message: impl Into<String>,
    diagnostic: Option<PreviewProjectionDiagnostic>,
) -> PreviewLayerDropExecutionReceipt {
    let diagnostics = diagnostics_with_extra(&intent.diagnostics, diagnostic);

    PreviewLayerDropExecutionReceipt {
        schema_version: PREVIEW_LAYER_DROP_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewLayerDropExecutionStatus::Blocked,
        message: message.into(),
        model_revision,
        projected_source_id: None,
        patch: None,
        canvas_patch: None,
        workspace_mutation: None,
        touched_files: Vec::new(),
        diagnostics,
    }
}

pub(super) fn committed_layer_drop_receipt(
    intent: PreviewProjectionIntentReceipt,
    model_revision: String,
    projected_source_id: Option<String>,
    patch: ProjectHtmlMovePatch,
    canvas_patch: CanvasPatch,
    workspace_mutation: ProjectWorkspaceMutationReceipt,
) -> PreviewLayerDropExecutionReceipt {
    let touched_files = workspace_mutation.touched_files.clone();
    let message = format!(
        "Preview layer drop a fost executat prin kernel în {}.",
        patch.file
    );

    PreviewLayerDropExecutionReceipt {
        schema_version: PREVIEW_LAYER_DROP_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewLayerDropExecutionStatus::Committed,
        message,
        model_revision: Some(model_revision),
        projected_source_id,
        touched_files,
        diagnostics: Vec::new(),
        patch: Some(patch),
        canvas_patch: Some(canvas_patch),
        workspace_mutation: Some(workspace_mutation),
    }
}

pub(super) fn blocked_html_insert_drop_receipt(
    intent: PreviewProjectionIntentReceipt,
    model_revision: Option<String>,
    message: impl Into<String>,
    diagnostic: Option<PreviewProjectionDiagnostic>,
) -> PreviewHtmlInsertDropExecutionReceipt {
    let diagnostics = diagnostics_with_extra(&intent.diagnostics, diagnostic);

    PreviewHtmlInsertDropExecutionReceipt {
        schema_version: PREVIEW_HTML_INSERT_DROP_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewHtmlInsertDropExecutionStatus::Blocked,
        message: message.into(),
        model_revision,
        patch: None,
        canvas_patch: None,
        workspace_mutation: None,
        touched_files: Vec::new(),
        diagnostics,
    }
}

pub(super) fn committed_html_insert_drop_receipt(
    intent: PreviewProjectionIntentReceipt,
    model_revision: String,
    patch: ProjectHtmlInsertPatch,
    canvas_patch: CanvasPatch,
    workspace_mutation: ProjectWorkspaceMutationReceipt,
) -> PreviewHtmlInsertDropExecutionReceipt {
    let touched_files = workspace_mutation.touched_files.clone();
    let message = format!(
        "Preview HTML insert drop a fost executat prin kernel în {}.",
        patch.file
    );

    PreviewHtmlInsertDropExecutionReceipt {
        schema_version: PREVIEW_HTML_INSERT_DROP_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewHtmlInsertDropExecutionStatus::Committed,
        message,
        model_revision: Some(model_revision),
        touched_files,
        diagnostics: Vec::new(),
        patch: Some(patch),
        canvas_patch: Some(canvas_patch),
        workspace_mutation: Some(workspace_mutation),
    }
}

pub(super) fn blocked_html_attributes_receipt(
    intent: PreviewProjectionIntentReceipt,
    model_revision: Option<String>,
    message: impl Into<String>,
    diagnostic: Option<PreviewProjectionDiagnostic>,
) -> PreviewHtmlAttributesExecutionReceipt {
    let diagnostics = diagnostics_with_extra(&intent.diagnostics, diagnostic);

    PreviewHtmlAttributesExecutionReceipt {
        schema_version: PREVIEW_HTML_ATTRIBUTES_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewHtmlAttributesExecutionStatus::Blocked,
        message: message.into(),
        model_revision,
        patch: None,
        canvas_patch: None,
        workspace_mutation: None,
        touched_files: Vec::new(),
        diagnostics,
    }
}

pub(super) fn committed_html_attributes_receipt(
    intent: PreviewProjectionIntentReceipt,
    model_revision: String,
    patch: ProjectHtmlAttributePatch,
    canvas_patch: Option<CanvasPatch>,
    workspace_mutation: ProjectWorkspaceMutationReceipt,
) -> PreviewHtmlAttributesExecutionReceipt {
    let touched_files = workspace_mutation.touched_files.clone();
    let message = format!(
        "Preview HTML attributes a fost executat prin kernel în {}.",
        patch.file
    );

    PreviewHtmlAttributesExecutionReceipt {
        schema_version: PREVIEW_HTML_ATTRIBUTES_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewHtmlAttributesExecutionStatus::Committed,
        message,
        model_revision: Some(model_revision),
        touched_files,
        diagnostics: Vec::new(),
        patch: Some(patch),
        canvas_patch,
        workspace_mutation: Some(workspace_mutation),
    }
}

pub(super) fn blocked_html_text_receipt(
    intent: PreviewProjectionIntentReceipt,
    model_revision: Option<String>,
    message: impl Into<String>,
    diagnostic: Option<PreviewProjectionDiagnostic>,
) -> PreviewHtmlTextExecutionReceipt {
    let diagnostics = diagnostics_with_extra(&intent.diagnostics, diagnostic);

    PreviewHtmlTextExecutionReceipt {
        schema_version: PREVIEW_HTML_TEXT_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewHtmlTextExecutionStatus::Blocked,
        message: message.into(),
        model_revision,
        patch: None,
        canvas_patch: None,
        workspace_mutation: None,
        touched_files: Vec::new(),
        diagnostics,
    }
}

pub(super) fn committed_html_text_receipt(
    intent: PreviewProjectionIntentReceipt,
    model_revision: String,
    patch: ProjectHtmlTextPatch,
    canvas_patch: Option<CanvasPatch>,
    workspace_mutation: ProjectWorkspaceMutationReceipt,
) -> PreviewHtmlTextExecutionReceipt {
    let touched_files = workspace_mutation.touched_files.clone();
    let message = format!(
        "Preview HTML text a fost executat prin kernel în {}.",
        patch.file
    );

    PreviewHtmlTextExecutionReceipt {
        schema_version: PREVIEW_HTML_TEXT_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewHtmlTextExecutionStatus::Committed,
        message,
        model_revision: Some(model_revision),
        touched_files,
        diagnostics: Vec::new(),
        patch: Some(patch),
        canvas_patch,
        workspace_mutation: Some(workspace_mutation),
    }
}

pub(super) fn blocked_html_tag_receipt(
    intent: PreviewProjectionIntentReceipt,
    model_revision: Option<String>,
    message: impl Into<String>,
    diagnostic: Option<PreviewProjectionDiagnostic>,
) -> PreviewHtmlTagExecutionReceipt {
    let diagnostics = diagnostics_with_extra(&intent.diagnostics, diagnostic);

    PreviewHtmlTagExecutionReceipt {
        schema_version: PREVIEW_HTML_TAG_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewHtmlTagExecutionStatus::Blocked,
        message: message.into(),
        model_revision,
        patch: None,
        canvas_patch: None,
        workspace_mutation: None,
        touched_files: Vec::new(),
        diagnostics,
    }
}

pub(super) fn committed_html_tag_receipt(
    intent: PreviewProjectionIntentReceipt,
    model_revision: String,
    patch: ProjectHtmlTagPatch,
    canvas_patch: CanvasPatch,
    workspace_mutation: ProjectWorkspaceMutationReceipt,
) -> PreviewHtmlTagExecutionReceipt {
    let touched_files = workspace_mutation.touched_files.clone();
    let message = format!(
        "Preview HTML tag a fost executat prin kernel în {}.",
        patch.file
    );

    PreviewHtmlTagExecutionReceipt {
        schema_version: PREVIEW_HTML_TAG_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewHtmlTagExecutionStatus::Committed,
        message,
        model_revision: Some(model_revision),
        touched_files,
        diagnostics: Vec::new(),
        patch: Some(patch),
        canvas_patch: Some(canvas_patch),
        workspace_mutation: Some(workspace_mutation),
    }
}

pub(super) fn blocked_html_duplicate_receipt(
    intent: PreviewProjectionIntentReceipt,
    model_revision: Option<String>,
    message: impl Into<String>,
    diagnostic: Option<PreviewProjectionDiagnostic>,
) -> PreviewHtmlDuplicateExecutionReceipt {
    let diagnostics = diagnostics_with_extra(&intent.diagnostics, diagnostic);

    PreviewHtmlDuplicateExecutionReceipt {
        schema_version: PREVIEW_HTML_DUPLICATE_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewHtmlDuplicateExecutionStatus::Blocked,
        message: message.into(),
        model_revision,
        patch: None,
        canvas_patch: None,
        workspace_mutation: None,
        touched_files: Vec::new(),
        diagnostics,
    }
}

pub(super) fn committed_html_duplicate_receipt(
    intent: PreviewProjectionIntentReceipt,
    model_revision: String,
    patch: ProjectHtmlDuplicatePatch,
    canvas_patch: Option<CanvasPatch>,
    workspace_mutation: ProjectWorkspaceMutationReceipt,
) -> PreviewHtmlDuplicateExecutionReceipt {
    let touched_files = workspace_mutation.touched_files.clone();
    let message = format!(
        "Preview HTML duplicate a fost executat prin kernel în {}.",
        patch.file
    );

    PreviewHtmlDuplicateExecutionReceipt {
        schema_version: PREVIEW_HTML_DUPLICATE_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewHtmlDuplicateExecutionStatus::Committed,
        message,
        model_revision: Some(model_revision),
        touched_files,
        diagnostics: Vec::new(),
        patch: Some(patch),
        canvas_patch,
        workspace_mutation: Some(workspace_mutation),
    }
}

pub(super) fn blocked_html_delete_receipt(
    intent: PreviewProjectionIntentReceipt,
    model_revision: Option<String>,
    message: impl Into<String>,
    diagnostic: Option<PreviewProjectionDiagnostic>,
) -> PreviewHtmlDeleteExecutionReceipt {
    let diagnostics = diagnostics_with_extra(&intent.diagnostics, diagnostic);

    PreviewHtmlDeleteExecutionReceipt {
        schema_version: PREVIEW_HTML_DELETE_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewHtmlDeleteExecutionStatus::Blocked,
        message: message.into(),
        model_revision,
        patch: None,
        canvas_patch: None,
        workspace_mutation: None,
        touched_files: Vec::new(),
        diagnostics,
    }
}

pub(super) fn committed_html_delete_receipt(
    intent: PreviewProjectionIntentReceipt,
    model_revision: String,
    patch: ProjectHtmlDeletePatch,
    canvas_patch: CanvasPatch,
    workspace_mutation: ProjectWorkspaceMutationReceipt,
) -> PreviewHtmlDeleteExecutionReceipt {
    let touched_files = workspace_mutation.touched_files.clone();
    let message = format!(
        "Preview HTML delete a fost executat prin kernel în {}.",
        patch.file
    );

    PreviewHtmlDeleteExecutionReceipt {
        schema_version: PREVIEW_HTML_DELETE_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewHtmlDeleteExecutionStatus::Committed,
        message,
        model_revision: Some(model_revision),
        touched_files,
        diagnostics: Vec::new(),
        patch: Some(patch),
        canvas_patch: Some(canvas_patch),
        workspace_mutation: Some(workspace_mutation),
    }
}

pub(super) fn blocked_tera_insert_drop_receipt(
    intent: PreviewProjectionIntentReceipt,
    model_revision: Option<String>,
    message: impl Into<String>,
    diagnostic: Option<PreviewProjectionDiagnostic>,
) -> PreviewTeraInsertDropExecutionReceipt {
    let diagnostics = diagnostics_with_extra(&intent.diagnostics, diagnostic);

    PreviewTeraInsertDropExecutionReceipt {
        schema_version: PREVIEW_TERA_INSERT_DROP_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewTeraInsertDropExecutionStatus::Blocked,
        message: message.into(),
        model_revision,
        patch: None,
        canvas_patch: None,
        workspace_mutation: None,
        touched_files: Vec::new(),
        diagnostics,
    }
}

pub(super) fn committed_tera_insert_drop_receipt(
    intent: PreviewProjectionIntentReceipt,
    model_revision: String,
    patch: ProjectTeraInsertPatch,
    workspace_mutation: ProjectWorkspaceMutationReceipt,
) -> PreviewTeraInsertDropExecutionReceipt {
    let touched_files = workspace_mutation.touched_files.clone();
    let message = format!(
        "Preview Tera insert drop a fost executat prin kernel în {}.",
        patch.file
    );

    PreviewTeraInsertDropExecutionReceipt {
        schema_version: PREVIEW_TERA_INSERT_DROP_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewTeraInsertDropExecutionStatus::Committed,
        message,
        model_revision: Some(model_revision),
        touched_files,
        diagnostics: Vec::new(),
        patch: Some(patch),
        canvas_patch: None,
        workspace_mutation: Some(workspace_mutation),
    }
}

pub(super) fn blocked_tera_move_drop_receipt(
    intent: PreviewProjectionIntentReceipt,
    model_revision: Option<String>,
    message: impl Into<String>,
    diagnostic: Option<PreviewProjectionDiagnostic>,
) -> PreviewTeraMoveDropExecutionReceipt {
    let diagnostics = diagnostics_with_extra(&intent.diagnostics, diagnostic);

    PreviewTeraMoveDropExecutionReceipt {
        schema_version: PREVIEW_TERA_MOVE_DROP_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewTeraMoveDropExecutionStatus::Blocked,
        message: message.into(),
        model_revision,
        patch: None,
        canvas_patch: None,
        workspace_mutation: None,
        touched_files: Vec::new(),
        diagnostics,
    }
}

pub(super) fn committed_tera_move_drop_receipt(
    intent: PreviewProjectionIntentReceipt,
    model_revision: String,
    patch: ProjectTeraMovePatch,
    workspace_mutation: ProjectWorkspaceMutationReceipt,
) -> PreviewTeraMoveDropExecutionReceipt {
    let touched_files = workspace_mutation.touched_files.clone();
    let message = format!(
        "Preview Tera move drop a fost executat prin kernel în {}.",
        patch.file
    );

    PreviewTeraMoveDropExecutionReceipt {
        schema_version: PREVIEW_TERA_MOVE_DROP_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewTeraMoveDropExecutionStatus::Committed,
        message,
        model_revision: Some(model_revision),
        touched_files,
        diagnostics: Vec::new(),
        patch: Some(patch),
        canvas_patch: None,
        workspace_mutation: Some(workspace_mutation),
    }
}

pub(super) fn blocked_tera_delete_receipt(
    intent: PreviewProjectionIntentReceipt,
    model_revision: Option<String>,
    message: impl Into<String>,
    diagnostic: Option<PreviewProjectionDiagnostic>,
) -> PreviewTeraDeleteExecutionReceipt {
    let diagnostics = diagnostics_with_extra(&intent.diagnostics, diagnostic);

    PreviewTeraDeleteExecutionReceipt {
        schema_version: PREVIEW_TERA_DELETE_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewTeraDeleteExecutionStatus::Blocked,
        message: message.into(),
        model_revision,
        patch: None,
        canvas_patch: None,
        workspace_mutation: None,
        touched_files: Vec::new(),
        diagnostics,
    }
}

pub(super) fn committed_tera_delete_receipt(
    intent: PreviewProjectionIntentReceipt,
    model_revision: String,
    patch: ProjectTeraDeletePatch,
    workspace_mutation: ProjectWorkspaceMutationReceipt,
) -> PreviewTeraDeleteExecutionReceipt {
    let touched_files = workspace_mutation.touched_files.clone();
    let message = format!(
        "Preview Tera delete a fost executat prin kernel în {}.",
        patch.file
    );

    PreviewTeraDeleteExecutionReceipt {
        schema_version: PREVIEW_TERA_DELETE_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewTeraDeleteExecutionStatus::Committed,
        message,
        model_revision: Some(model_revision),
        touched_files,
        diagnostics: Vec::new(),
        patch: Some(patch),
        canvas_patch: None,
        workspace_mutation: Some(workspace_mutation),
    }
}

pub(super) fn blocked_template_edit_permission_receipt(
    intent: PreviewProjectionIntentReceipt,
    model_revision: Option<String>,
    message: impl Into<String>,
    diagnostic: Option<PreviewProjectionDiagnostic>,
) -> PreviewTemplateEditPermissionReceipt {
    let diagnostics = diagnostics_with_extra(&intent.diagnostics, diagnostic);

    PreviewTemplateEditPermissionReceipt {
        schema_version: PREVIEW_TEMPLATE_EDIT_PERMISSION_SCHEMA_VERSION,
        intent,
        status: PreviewTemplateEditPermissionStatus::Blocked,
        message: message.into(),
        model_revision,
        grant: None,
        diagnostics,
    }
}

pub(super) fn granted_template_edit_permission_receipt(
    intent: PreviewProjectionIntentReceipt,
    model_revision: String,
    grant: ProjectTemplateEditPermissionGrant,
) -> PreviewTemplateEditPermissionReceipt {
    let message = format!(
        "Preview template edit permission a fost acordat pentru {}.",
        grant.target_label
    );

    PreviewTemplateEditPermissionReceipt {
        schema_version: PREVIEW_TEMPLATE_EDIT_PERMISSION_SCHEMA_VERSION,
        intent,
        status: PreviewTemplateEditPermissionStatus::Granted,
        message,
        model_revision: Some(model_revision),
        grant: Some(grant),
        diagnostics: Vec::new(),
    }
}

fn diagnostics_with_extra(
    diagnostics: &[PreviewProjectionDiagnostic],
    extra: Option<PreviewProjectionDiagnostic>,
) -> Vec<PreviewProjectionDiagnostic> {
    let mut diagnostics = diagnostics.to_vec();
    if let Some(extra) = extra {
        diagnostics.push(extra);
    }
    diagnostics
}

#[cfg(test)]
mod tests {
    use crate::kernel::preview_projection::{
        PreviewProjectionEffect, PreviewProjectionIntentKind, PreviewProjectionIntentReceipt,
        PreviewProjectionIntentStatus, PREVIEW_LAYER_DROP_EXECUTION_SCHEMA_VERSION,
    };

    use super::blocked_layer_drop_receipt;

    #[test]
    fn blocked_layer_drop_receipt_has_no_post_commit_identity() {
        let receipt = blocked_layer_drop_receipt(
            PreviewProjectionIntentReceipt {
                schema_version: 1,
                intent_id: "intent:blocked-move".to_string(),
                kind: PreviewProjectionIntentKind::LayerDrop,
                status: PreviewProjectionIntentStatus::Accepted,
                effect: PreviewProjectionEffect::KernelMutationPreflight,
                accepted: true,
                requires_project_session: true,
                project_session_id: Some("session".to_string()),
                project_root: Some("/project".to_string()),
                runtime_session_id: Some("session:runtime".to_string()),
                preview_revision: Some(1),
                message: "accepted".to_string(),
                diagnostics: Vec::new(),
            },
            Some("model:before".to_string()),
            "blocked",
            None,
        );

        assert_eq!(
            receipt.schema_version,
            PREVIEW_LAYER_DROP_EXECUTION_SCHEMA_VERSION
        );
        assert!(receipt.projected_source_id.is_none());
        assert!(receipt.patch.is_none());
        assert!(receipt.touched_files.is_empty());
    }
}
