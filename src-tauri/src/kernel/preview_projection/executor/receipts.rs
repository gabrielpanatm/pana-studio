use crate::{
    kernel::project_workspace::ProjectWorkspaceMutationReceipt,
    localization::LocalizedDiagnostic,
    project_model::{
        attribute_engine::ProjectHtmlAttributePatch, delete_engine::ProjectHtmlDeletePatch,
        duplicate_engine::ProjectHtmlDuplicatePatch, insert_engine::ProjectHtmlInsertPatch,
        tag_engine::ProjectHtmlTagPatch, tera_delete_engine::ProjectTeraDeletePatch,
        tera_insert_engine::ProjectTeraInsertPatch, text_engine::ProjectHtmlTextPatch,
    },
};

use super::super::model::{
    CanvasPatch, PreviewHtmlAttributesExecutionReceipt, PreviewHtmlAttributesExecutionStatus,
    PreviewHtmlDeleteExecutionReceipt, PreviewHtmlDeleteExecutionStatus,
    PreviewHtmlDuplicateExecutionReceipt, PreviewHtmlDuplicateExecutionStatus,
    PreviewHtmlInsertDropExecutionReceipt, PreviewHtmlInsertDropExecutionStatus,
    PreviewHtmlTagExecutionReceipt, PreviewHtmlTagExecutionStatus, PreviewHtmlTextExecutionReceipt,
    PreviewHtmlTextExecutionStatus, PreviewProjectionDiagnostic, PreviewProjectionIntentReceipt,
    PreviewTeraDeleteExecutionReceipt, PreviewTeraDeleteExecutionStatus,
    PreviewTeraInsertDropExecutionReceipt, PreviewTeraInsertDropExecutionStatus,
    PREVIEW_HTML_ATTRIBUTES_EXECUTION_SCHEMA_VERSION, PREVIEW_HTML_DELETE_EXECUTION_SCHEMA_VERSION,
    PREVIEW_HTML_DUPLICATE_EXECUTION_SCHEMA_VERSION,
    PREVIEW_HTML_INSERT_DROP_EXECUTION_SCHEMA_VERSION, PREVIEW_HTML_TAG_EXECUTION_SCHEMA_VERSION,
    PREVIEW_HTML_TEXT_EXECUTION_SCHEMA_VERSION, PREVIEW_TERA_DELETE_EXECUTION_SCHEMA_VERSION,
    PREVIEW_TERA_INSERT_DROP_EXECUTION_SCHEMA_VERSION,
};

pub(super) fn blocked_html_insert_drop_receipt(
    intent: PreviewProjectionIntentReceipt,
    model_revision: Option<String>,
    diagnostic: Option<PreviewProjectionDiagnostic>,
) -> PreviewHtmlInsertDropExecutionReceipt {
    let diagnostics = diagnostics_with_extra(&intent.diagnostics, diagnostic);

    PreviewHtmlInsertDropExecutionReceipt {
        schema_version: PREVIEW_HTML_INSERT_DROP_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewHtmlInsertDropExecutionStatus::Blocked,
        message_diagnostic: blocked_message(),
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
    let message_diagnostic = committed_message(&patch.file);

    PreviewHtmlInsertDropExecutionReceipt {
        schema_version: PREVIEW_HTML_INSERT_DROP_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewHtmlInsertDropExecutionStatus::Committed,
        message_diagnostic,
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
    diagnostic: Option<PreviewProjectionDiagnostic>,
) -> PreviewHtmlAttributesExecutionReceipt {
    let diagnostics = diagnostics_with_extra(&intent.diagnostics, diagnostic);

    PreviewHtmlAttributesExecutionReceipt {
        schema_version: PREVIEW_HTML_ATTRIBUTES_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewHtmlAttributesExecutionStatus::Blocked,
        message_diagnostic: blocked_message(),
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
    let message_diagnostic = committed_message(&patch.file);

    PreviewHtmlAttributesExecutionReceipt {
        schema_version: PREVIEW_HTML_ATTRIBUTES_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewHtmlAttributesExecutionStatus::Committed,
        message_diagnostic,
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
    diagnostic: Option<PreviewProjectionDiagnostic>,
) -> PreviewHtmlTextExecutionReceipt {
    let diagnostics = diagnostics_with_extra(&intent.diagnostics, diagnostic);

    PreviewHtmlTextExecutionReceipt {
        schema_version: PREVIEW_HTML_TEXT_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewHtmlTextExecutionStatus::Blocked,
        message_diagnostic: blocked_message(),
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
    let message_diagnostic = committed_message(&patch.file);

    PreviewHtmlTextExecutionReceipt {
        schema_version: PREVIEW_HTML_TEXT_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewHtmlTextExecutionStatus::Committed,
        message_diagnostic,
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
    diagnostic: Option<PreviewProjectionDiagnostic>,
) -> PreviewHtmlTagExecutionReceipt {
    let diagnostics = diagnostics_with_extra(&intent.diagnostics, diagnostic);

    PreviewHtmlTagExecutionReceipt {
        schema_version: PREVIEW_HTML_TAG_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewHtmlTagExecutionStatus::Blocked,
        message_diagnostic: blocked_message(),
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
    let message_diagnostic = committed_message(&patch.file);

    PreviewHtmlTagExecutionReceipt {
        schema_version: PREVIEW_HTML_TAG_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewHtmlTagExecutionStatus::Committed,
        message_diagnostic,
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
    diagnostic: Option<PreviewProjectionDiagnostic>,
) -> PreviewHtmlDuplicateExecutionReceipt {
    let diagnostics = diagnostics_with_extra(&intent.diagnostics, diagnostic);

    PreviewHtmlDuplicateExecutionReceipt {
        schema_version: PREVIEW_HTML_DUPLICATE_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewHtmlDuplicateExecutionStatus::Blocked,
        message_diagnostic: blocked_message(),
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
    let message_diagnostic = committed_message(&patch.file);

    PreviewHtmlDuplicateExecutionReceipt {
        schema_version: PREVIEW_HTML_DUPLICATE_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewHtmlDuplicateExecutionStatus::Committed,
        message_diagnostic,
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
    diagnostic: Option<PreviewProjectionDiagnostic>,
) -> PreviewHtmlDeleteExecutionReceipt {
    let diagnostics = diagnostics_with_extra(&intent.diagnostics, diagnostic);

    PreviewHtmlDeleteExecutionReceipt {
        schema_version: PREVIEW_HTML_DELETE_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewHtmlDeleteExecutionStatus::Blocked,
        message_diagnostic: blocked_message(),
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
    let message_diagnostic = committed_message(&patch.file);

    PreviewHtmlDeleteExecutionReceipt {
        schema_version: PREVIEW_HTML_DELETE_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewHtmlDeleteExecutionStatus::Committed,
        message_diagnostic,
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
    diagnostic: Option<PreviewProjectionDiagnostic>,
) -> PreviewTeraInsertDropExecutionReceipt {
    let diagnostics = diagnostics_with_extra(&intent.diagnostics, diagnostic);

    PreviewTeraInsertDropExecutionReceipt {
        schema_version: PREVIEW_TERA_INSERT_DROP_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewTeraInsertDropExecutionStatus::Blocked,
        message_diagnostic: blocked_message(),
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
    let message_diagnostic = committed_message(&patch.file);

    PreviewTeraInsertDropExecutionReceipt {
        schema_version: PREVIEW_TERA_INSERT_DROP_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewTeraInsertDropExecutionStatus::Committed,
        message_diagnostic,
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
    diagnostic: Option<PreviewProjectionDiagnostic>,
) -> PreviewTeraDeleteExecutionReceipt {
    let diagnostics = diagnostics_with_extra(&intent.diagnostics, diagnostic);

    PreviewTeraDeleteExecutionReceipt {
        schema_version: PREVIEW_TERA_DELETE_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewTeraDeleteExecutionStatus::Blocked,
        message_diagnostic: blocked_message(),
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
    let message_diagnostic = committed_message(&patch.file);

    PreviewTeraDeleteExecutionReceipt {
        schema_version: PREVIEW_TERA_DELETE_EXECUTION_SCHEMA_VERSION,
        intent,
        status: PreviewTeraDeleteExecutionStatus::Committed,
        message_diagnostic,
        model_revision: Some(model_revision),
        touched_files,
        diagnostics: Vec::new(),
        patch: Some(patch),
        canvas_patch: None,
        workspace_mutation: Some(workspace_mutation),
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

fn blocked_message() -> LocalizedDiagnostic {
    LocalizedDiagnostic::new("preview-projection-execution-blocked")
}

fn committed_message(file: &str) -> LocalizedDiagnostic {
    LocalizedDiagnostic::new("preview-projection-execution-committed").with_argument("file", file)
}
