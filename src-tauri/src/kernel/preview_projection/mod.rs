mod executor;
mod model;
mod preflight;
mod structural_write;

pub(crate) use executor::{execute_editor_move, EditorMoveExecutionOutcome};
pub use executor::{
    execute_preview_html_attributes, execute_preview_html_delete, execute_preview_html_duplicate,
    execute_preview_html_insert_drop, execute_preview_html_tag, execute_preview_html_text,
    execute_preview_selection_batch, execute_preview_tera_delete, execute_preview_tera_insert_drop,
    PreviewHtmlAttributesExecutionOutcome, PreviewHtmlDeleteExecutionOutcome,
    PreviewHtmlDuplicateExecutionOutcome, PreviewHtmlInsertDropExecutionOutcome,
    PreviewHtmlTagExecutionOutcome, PreviewHtmlTextExecutionOutcome,
    PreviewSelectionBatchExecutionOutcome, PreviewTeraDeleteExecutionOutcome,
    PreviewTeraInsertDropExecutionOutcome,
};
pub use model::{
    CanvasPatch, CanvasPatchAnchor, CanvasPatchOperation, PreviewHtmlAttributesExecutionInput,
    PreviewHtmlAttributesExecutionReceipt, PreviewHtmlAttributesExecutionStatus,
    PreviewHtmlDeleteExecutionInput, PreviewHtmlDeleteExecutionReceipt,
    PreviewHtmlDeleteExecutionStatus, PreviewHtmlDuplicateExecutionInput,
    PreviewHtmlDuplicateExecutionReceipt, PreviewHtmlDuplicateExecutionStatus,
    PreviewHtmlInsertDropExecutionInput, PreviewHtmlInsertDropExecutionReceipt,
    PreviewHtmlInsertDropExecutionStatus, PreviewHtmlTagExecutionInput,
    PreviewHtmlTagExecutionReceipt, PreviewHtmlTagExecutionStatus, PreviewHtmlTextExecutionInput,
    PreviewHtmlTextExecutionReceipt, PreviewHtmlTextExecutionStatus, PreviewProjectionDiagnostic,
    PreviewProjectionDiagnosticSeverity, PreviewProjectionEffect, PreviewProjectionIntentInput,
    PreviewProjectionIntentKind, PreviewProjectionIntentReceipt, PreviewProjectionIntentStatus,
    PreviewSelectionBatchAction, PreviewSelectionBatchExecutionInput,
    PreviewSelectionBatchExecutionReceipt, PreviewSelectionBatchExecutionStatus,
    PreviewStructuralCommandIdentity, PreviewStructuralSelectionIdentity,
    PreviewTeraDeleteExecutionInput, PreviewTeraDeleteExecutionReceipt,
    PreviewTeraDeleteExecutionStatus, PreviewTeraInsertDropExecutionInput,
    PreviewTeraInsertDropExecutionReceipt, PreviewTeraInsertDropExecutionStatus,
    CANVAS_PATCH_SCHEMA_VERSION, PREVIEW_HTML_ATTRIBUTES_EXECUTION_SCHEMA_VERSION,
    PREVIEW_HTML_DELETE_EXECUTION_SCHEMA_VERSION, PREVIEW_HTML_DUPLICATE_EXECUTION_SCHEMA_VERSION,
    PREVIEW_HTML_INSERT_DROP_EXECUTION_SCHEMA_VERSION, PREVIEW_HTML_TAG_EXECUTION_SCHEMA_VERSION,
    PREVIEW_HTML_TEXT_EXECUTION_SCHEMA_VERSION, PREVIEW_SELECTION_BATCH_EXECUTION_SCHEMA_VERSION,
    PREVIEW_TERA_DELETE_EXECUTION_SCHEMA_VERSION,
    PREVIEW_TERA_INSERT_DROP_EXECUTION_SCHEMA_VERSION,
};
pub use preflight::preflight_preview_projection_intent;
