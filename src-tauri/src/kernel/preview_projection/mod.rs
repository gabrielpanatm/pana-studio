mod executor;
mod model;
mod preflight;
mod structural_write;

pub use executor::{
    execute_preview_html_attributes, execute_preview_html_delete, execute_preview_html_duplicate,
    execute_preview_html_insert_drop, execute_preview_html_tag, execute_preview_html_text,
    execute_preview_layer_drop, execute_preview_template_edit_permission,
    execute_preview_tera_delete, execute_preview_tera_insert_drop, execute_preview_tera_move_drop,
    PreviewHtmlAttributesExecutionOutcome, PreviewHtmlDeleteExecutionOutcome,
    PreviewHtmlDuplicateExecutionOutcome, PreviewHtmlInsertDropExecutionOutcome,
    PreviewHtmlTagExecutionOutcome, PreviewHtmlTextExecutionOutcome,
    PreviewLayerDropExecutionOutcome, PreviewTemplateEditPermissionOutcome,
    PreviewTeraDeleteExecutionOutcome, PreviewTeraInsertDropExecutionOutcome,
    PreviewTeraMoveDropExecutionOutcome,
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
    PreviewHtmlTextExecutionReceipt, PreviewHtmlTextExecutionStatus,
    PreviewLayerDropExecutionInput, PreviewLayerDropExecutionReceipt,
    PreviewLayerDropExecutionStatus, PreviewProjectionDiagnostic,
    PreviewProjectionDiagnosticSeverity, PreviewProjectionEffect, PreviewProjectionIntentInput,
    PreviewProjectionIntentKind, PreviewProjectionIntentReceipt, PreviewProjectionIntentStatus,
    PreviewStructuralCommandIdentity, PreviewTemplateEditPermissionInput,
    PreviewTemplateEditPermissionReceipt, PreviewTemplateEditPermissionStatus,
    PreviewTeraDeleteExecutionInput, PreviewTeraDeleteExecutionReceipt,
    PreviewTeraDeleteExecutionStatus, PreviewTeraInsertDropExecutionInput,
    PreviewTeraInsertDropExecutionReceipt, PreviewTeraInsertDropExecutionStatus,
    PreviewTeraMoveDropExecutionInput, PreviewTeraMoveDropExecutionReceipt,
    PreviewTeraMoveDropExecutionStatus, CANVAS_PATCH_SCHEMA_VERSION,
    PREVIEW_HTML_ATTRIBUTES_EXECUTION_SCHEMA_VERSION, PREVIEW_HTML_DELETE_EXECUTION_SCHEMA_VERSION,
    PREVIEW_HTML_DUPLICATE_EXECUTION_SCHEMA_VERSION,
    PREVIEW_HTML_INSERT_DROP_EXECUTION_SCHEMA_VERSION, PREVIEW_HTML_TAG_EXECUTION_SCHEMA_VERSION,
    PREVIEW_HTML_TEXT_EXECUTION_SCHEMA_VERSION, PREVIEW_LAYER_DROP_EXECUTION_SCHEMA_VERSION,
    PREVIEW_TEMPLATE_EDIT_PERMISSION_SCHEMA_VERSION, PREVIEW_TERA_DELETE_EXECUTION_SCHEMA_VERSION,
    PREVIEW_TERA_INSERT_DROP_EXECUTION_SCHEMA_VERSION,
    PREVIEW_TERA_MOVE_DROP_EXECUTION_SCHEMA_VERSION,
};
pub use preflight::preflight_preview_projection_intent;
