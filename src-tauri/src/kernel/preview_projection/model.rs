use std::{
    collections::BTreeMap,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    kernel::project_workspace::ProjectWorkspaceMutationReceipt,
    project_model::{
        attribute_engine::{ProjectHtmlAttributeIntent, ProjectHtmlAttributePatch},
        delete_engine::{ProjectHtmlDeleteIntent, ProjectHtmlDeletePatch},
        duplicate_engine::{ProjectHtmlDuplicateIntent, ProjectHtmlDuplicatePatch},
        insert_engine::{ProjectHtmlInsertIntent, ProjectHtmlInsertPatch},
        move_engine::{ProjectHtmlMoveIntent, ProjectHtmlMovePatch, ProjectMovePosition},
        tag_engine::{ProjectHtmlTagIntent, ProjectHtmlTagPatch},
        template_edit_gate::{
            ProjectTemplateEditPermissionGrant, ProjectTemplateEditPermissionIntent,
        },
        tera_delete_engine::{ProjectTeraDeleteIntent, ProjectTeraDeletePatch},
        tera_insert_engine::{ProjectTeraInsertIntent, ProjectTeraInsertPatch},
        tera_move_engine::{ProjectTeraMoveIntent, ProjectTeraMovePatch},
        text_engine::{ProjectHtmlTextIntent, ProjectHtmlTextPatch},
    },
};

pub const PREVIEW_LAYER_DROP_EXECUTION_SCHEMA_VERSION: u32 = 2;
pub const PREVIEW_HTML_INSERT_DROP_EXECUTION_SCHEMA_VERSION: u32 = 1;
pub const PREVIEW_HTML_ATTRIBUTES_EXECUTION_SCHEMA_VERSION: u32 = 1;
pub const PREVIEW_HTML_TEXT_EXECUTION_SCHEMA_VERSION: u32 = 1;
pub const PREVIEW_HTML_TAG_EXECUTION_SCHEMA_VERSION: u32 = 1;
pub const PREVIEW_HTML_DUPLICATE_EXECUTION_SCHEMA_VERSION: u32 = 1;
pub const PREVIEW_HTML_DELETE_EXECUTION_SCHEMA_VERSION: u32 = 1;
pub const PREVIEW_TERA_INSERT_DROP_EXECUTION_SCHEMA_VERSION: u32 = 1;
pub const PREVIEW_TERA_MOVE_DROP_EXECUTION_SCHEMA_VERSION: u32 = 1;
pub const PREVIEW_TERA_DELETE_EXECUTION_SCHEMA_VERSION: u32 = 1;
pub const PREVIEW_TEMPLATE_EDIT_PERMISSION_SCHEMA_VERSION: u32 = 1;
pub const CANVAS_PATCH_SCHEMA_VERSION: u32 = 1;
const MAX_CANVAS_PATCH_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewStructuralCommandIdentity {
    pub expected_project_root: String,
    pub expected_session_id: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewProjectionIntentInput {
    pub message_type: String,
    #[serde(default)]
    pub preview_revision: Option<u64>,
    #[serde(default)]
    pub source_selector: Option<String>,
    #[serde(default)]
    pub target_selector: Option<String>,
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default)]
    pub target_source_id: Option<String>,
    #[serde(default)]
    pub source_template_source_id: Option<String>,
    #[serde(default)]
    pub target_template_source_id: Option<String>,
    #[serde(default)]
    pub source_session_id: Option<String>,
    #[serde(default)]
    pub target_session_id: Option<String>,
    #[serde(default)]
    pub source_tag: Option<String>,
    #[serde(default)]
    pub target_tag: Option<String>,
    #[serde(default)]
    pub target_kind: Option<String>,
    #[serde(default)]
    pub position: Option<String>,
    #[serde(default)]
    pub item_kind: Option<String>,
    #[serde(default)]
    pub element_tag: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewProjectionIntentKind {
    LayerDrop,
    HtmlInsertDrop,
    HtmlAttributes,
    HtmlText,
    HtmlTag,
    HtmlDuplicate,
    TeraInsertDrop,
    TeraMoveDrop,
    HtmlDelete,
    TemplateDelete,
    TemplateEdit,
    Unsupported,
}

impl PreviewProjectionIntentKind {
    pub fn operation_label(self) -> &'static str {
        match self {
            Self::LayerDrop => "preview.layer.drop",
            Self::HtmlInsertDrop => "preview.html.insert_drop",
            Self::HtmlAttributes => "preview.html.attributes",
            Self::HtmlText => "preview.html.text",
            Self::HtmlTag => "preview.html.tag",
            Self::HtmlDuplicate => "preview.html.duplicate_selected",
            Self::TeraInsertDrop => "preview.tera.insert_drop",
            Self::TeraMoveDrop => "preview.tera.move_drop",
            Self::HtmlDelete => "preview.html.delete_selected",
            Self::TemplateDelete => "preview.template.delete_selected",
            Self::TemplateEdit => "preview.template.edit_selected",
            Self::Unsupported => "preview.unsupported",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewProjectionIntentStatus {
    Accepted,
    Blocked,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewProjectionEffect {
    KernelMutationPreflight,
    TemplatePermissionPreflight,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewProjectionDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewProjectionDiagnostic {
    pub code: String,
    pub severity: PreviewProjectionDiagnosticSeverity,
    pub message: String,
    pub blocking: bool,
}

impl PreviewProjectionDiagnostic {
    pub fn blocking(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: PreviewProjectionDiagnosticSeverity::Error,
            message: message.into(),
            blocking: true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewProjectionIntentReceipt {
    pub schema_version: u32,
    pub intent_id: String,
    pub kind: PreviewProjectionIntentKind,
    pub status: PreviewProjectionIntentStatus,
    pub effect: PreviewProjectionEffect,
    pub accepted: bool,
    pub requires_project_session: bool,
    pub project_session_id: Option<String>,
    pub project_root: Option<String>,
    pub runtime_session_id: Option<String>,
    pub preview_revision: Option<u64>,
    pub message: String,
    pub diagnostics: Vec<PreviewProjectionDiagnostic>,
}

/// A source-backed anchor for a one-shot optimistic Canvas patch. Source IDs
/// are authoritative; selector data is only a guarded fallback for the exact
/// currently mounted document. Render instance IDs are populated when the
/// active CanvasGraph can disambiguate a repeated source occurrence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasPatchAnchor {
    pub source_id: String,
    pub render_instance_id: Option<String>,
    pub selector_fallback: Option<String>,
    pub expected_tag: Option<String>,
}

impl CanvasPatchAnchor {
    pub(crate) fn source(
        source_id: impl Into<String>,
        selector_fallback: Option<&str>,
        expected_tag: Option<&str>,
    ) -> Self {
        Self {
            source_id: source_id.into(),
            render_instance_id: None,
            selector_fallback: bounded_optional(selector_fallback, 4_096),
            expected_tag: bounded_optional(expected_tag, 128),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CanvasPatchOperation {
    SetAttributes {
        target: CanvasPatchAnchor,
        attributes: BTreeMap<String, Option<String>>,
    },
    SetText {
        target: CanvasPatchAnchor,
        text: String,
    },
    ReplaceTag {
        target: CanvasPatchAnchor,
        new_tag: String,
    },
    Insert {
        target: CanvasPatchAnchor,
        position: ProjectMovePosition,
        html: String,
    },
    Move {
        source: CanvasPatchAnchor,
        target: CanvasPatchAnchor,
        position: ProjectMovePosition,
    },
    Duplicate {
        source: CanvasPatchAnchor,
        html: String,
    },
    Delete {
        target: CanvasPatchAnchor,
    },
}

/// Rust-issued, one-shot DOM acceleration for an already committed
/// ProjectWorkspace mutation. It is never canonical by itself: the Zola
/// candidate carrying the same `workspace_transaction_id` must still reach
/// `canonicalVerified`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasPatch {
    pub schema_version: u32,
    pub patch_id: String,
    pub issued_at_ms: u64,
    pub project_root: String,
    pub runtime_session_id: String,
    pub base_workspace_revision: u64,
    pub workspace_revision: u64,
    pub workspace_transaction_id: String,
    pub before_model_revision: String,
    pub after_model_revision: String,
    pub operation: CanvasPatchOperation,
}

impl CanvasPatch {
    fn current_time_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    pub(crate) fn issued(
        project_root: &str,
        runtime_session_id: &str,
        workspace_mutation: &ProjectWorkspaceMutationReceipt,
        before_model_revision: &str,
        after_model_revision: &str,
        operation: CanvasPatchOperation,
    ) -> Result<Self, String> {
        let workspace_transaction_id = workspace_mutation
            .transaction_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty() && value.len() <= 256)
            .ok_or_else(|| {
                "CanvasPatch cere transactionId-ul mutației ProjectWorkspace.".to_string()
            })?;
        if !workspace_mutation.changed
            || workspace_mutation.revision_after <= workspace_mutation.revision_before
            || project_root.trim().is_empty()
            || runtime_session_id.trim().is_empty()
            || before_model_revision.trim().is_empty()
            || after_model_revision.trim().is_empty()
        {
            return Err(
                "CanvasPatch a refuzat o mutație fără identitate sau revizie validă.".to_string(),
            );
        }
        let canonical = serde_json::to_vec(&(
            CANVAS_PATCH_SCHEMA_VERSION,
            project_root,
            runtime_session_id,
            workspace_mutation.revision_before,
            workspace_mutation.revision_after,
            workspace_transaction_id,
            before_model_revision,
            after_model_revision,
            &operation,
        ))
        .map_err(|error| format!("CanvasPatch nu a putut fi serializat: {error}"))?;
        if canonical.len() > MAX_CANVAS_PATCH_BYTES {
            return Err("CanvasPatch depășește bugetul de 2 MiB.".to_string());
        }
        let patch_id = format!("canvas_patch_{}", full_hex(&Sha256::digest(&canonical)));
        Ok(Self {
            schema_version: CANVAS_PATCH_SCHEMA_VERSION,
            patch_id,
            issued_at_ms: Self::current_time_ms(),
            project_root: project_root.to_string(),
            runtime_session_id: runtime_session_id.to_string(),
            base_workspace_revision: workspace_mutation.revision_before,
            workspace_revision: workspace_mutation.revision_after,
            workspace_transaction_id: workspace_transaction_id.to_string(),
            before_model_revision: before_model_revision.to_string(),
            after_model_revision: after_model_revision.to_string(),
            operation,
        })
    }

    /// Starts the browser-delivery clock only after the authoritative
    /// ProjectWorkspace mutation and its recovery record are durable. The
    /// timestamp is observability metadata and is intentionally excluded from
    /// `patch_id`.
    pub(crate) fn mark_issued_now(&mut self) {
        self.issued_at_ms = Self::current_time_ms();
    }
}

fn bounded_optional(value: Option<&str>, max_len: usize) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= max_len)
        .map(str::to_string)
}

fn full_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewLayerDropExecutionInput {
    pub intent: PreviewProjectionIntentInput,
    pub move_intent: ProjectHtmlMoveIntent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewLayerDropExecutionStatus {
    Committed,
    Blocked,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewLayerDropExecutionReceipt {
    pub schema_version: u32,
    pub intent: PreviewProjectionIntentReceipt,
    pub status: PreviewLayerDropExecutionStatus,
    pub message: String,
    pub model_revision: Option<String>,
    /// Canonical Source Graph identity of the moved node in `model_revision`.
    /// This is intentionally distinct from the pre-move ID carried by the
    /// patch: positional DOM selectors and preview session IDs are not stable
    /// after the Zola document is reloaded.
    pub projected_source_id: Option<String>,
    pub patch: Option<ProjectHtmlMovePatch>,
    pub canvas_patch: Option<CanvasPatch>,
    pub workspace_mutation: Option<ProjectWorkspaceMutationReceipt>,
    pub touched_files: Vec<String>,
    pub diagnostics: Vec<PreviewProjectionDiagnostic>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewHtmlInsertDropExecutionInput {
    pub intent: PreviewProjectionIntentInput,
    pub insert_intent: ProjectHtmlInsertIntent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewHtmlInsertDropExecutionStatus {
    Committed,
    Blocked,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewHtmlInsertDropExecutionReceipt {
    pub schema_version: u32,
    pub intent: PreviewProjectionIntentReceipt,
    pub status: PreviewHtmlInsertDropExecutionStatus,
    pub message: String,
    pub model_revision: Option<String>,
    pub patch: Option<ProjectHtmlInsertPatch>,
    pub canvas_patch: Option<CanvasPatch>,
    pub workspace_mutation: Option<ProjectWorkspaceMutationReceipt>,
    pub touched_files: Vec<String>,
    pub diagnostics: Vec<PreviewProjectionDiagnostic>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewHtmlAttributesExecutionInput {
    pub intent: PreviewProjectionIntentInput,
    pub attribute_intent: ProjectHtmlAttributeIntent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewHtmlAttributesExecutionStatus {
    Committed,
    Blocked,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewHtmlAttributesExecutionReceipt {
    pub schema_version: u32,
    pub intent: PreviewProjectionIntentReceipt,
    pub status: PreviewHtmlAttributesExecutionStatus,
    pub message: String,
    pub model_revision: Option<String>,
    pub patch: Option<ProjectHtmlAttributePatch>,
    pub canvas_patch: Option<CanvasPatch>,
    pub workspace_mutation: Option<ProjectWorkspaceMutationReceipt>,
    pub touched_files: Vec<String>,
    pub diagnostics: Vec<PreviewProjectionDiagnostic>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewHtmlTextExecutionInput {
    pub intent: PreviewProjectionIntentInput,
    pub text_intent: ProjectHtmlTextIntent,
    #[serde(default)]
    pub defer_canonical_projection: bool,
    #[serde(default)]
    pub edit_session_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewHtmlTextExecutionStatus {
    Committed,
    Blocked,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewHtmlTextExecutionReceipt {
    pub schema_version: u32,
    pub intent: PreviewProjectionIntentReceipt,
    pub status: PreviewHtmlTextExecutionStatus,
    pub message: String,
    pub model_revision: Option<String>,
    pub patch: Option<ProjectHtmlTextPatch>,
    pub canvas_patch: Option<CanvasPatch>,
    pub workspace_mutation: Option<ProjectWorkspaceMutationReceipt>,
    pub touched_files: Vec<String>,
    pub diagnostics: Vec<PreviewProjectionDiagnostic>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewHtmlTagExecutionInput {
    pub intent: PreviewProjectionIntentInput,
    pub tag_intent: ProjectHtmlTagIntent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewHtmlTagExecutionStatus {
    Committed,
    Blocked,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewHtmlTagExecutionReceipt {
    pub schema_version: u32,
    pub intent: PreviewProjectionIntentReceipt,
    pub status: PreviewHtmlTagExecutionStatus,
    pub message: String,
    pub model_revision: Option<String>,
    pub patch: Option<ProjectHtmlTagPatch>,
    pub canvas_patch: Option<CanvasPatch>,
    pub workspace_mutation: Option<ProjectWorkspaceMutationReceipt>,
    pub touched_files: Vec<String>,
    pub diagnostics: Vec<PreviewProjectionDiagnostic>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewHtmlDuplicateExecutionInput {
    pub intent: PreviewProjectionIntentInput,
    pub duplicate_intent: ProjectHtmlDuplicateIntent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewHtmlDuplicateExecutionStatus {
    Committed,
    Blocked,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewHtmlDuplicateExecutionReceipt {
    pub schema_version: u32,
    pub intent: PreviewProjectionIntentReceipt,
    pub status: PreviewHtmlDuplicateExecutionStatus,
    pub message: String,
    pub model_revision: Option<String>,
    pub patch: Option<ProjectHtmlDuplicatePatch>,
    pub canvas_patch: Option<CanvasPatch>,
    pub workspace_mutation: Option<ProjectWorkspaceMutationReceipt>,
    pub touched_files: Vec<String>,
    pub diagnostics: Vec<PreviewProjectionDiagnostic>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewHtmlDeleteExecutionInput {
    pub intent: PreviewProjectionIntentInput,
    pub delete_intent: ProjectHtmlDeleteIntent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewHtmlDeleteExecutionStatus {
    Committed,
    Blocked,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewHtmlDeleteExecutionReceipt {
    pub schema_version: u32,
    pub intent: PreviewProjectionIntentReceipt,
    pub status: PreviewHtmlDeleteExecutionStatus,
    pub message: String,
    pub model_revision: Option<String>,
    pub patch: Option<ProjectHtmlDeletePatch>,
    pub canvas_patch: Option<CanvasPatch>,
    pub workspace_mutation: Option<ProjectWorkspaceMutationReceipt>,
    pub touched_files: Vec<String>,
    pub diagnostics: Vec<PreviewProjectionDiagnostic>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewTeraInsertDropExecutionInput {
    pub intent: PreviewProjectionIntentInput,
    pub insert_intent: ProjectTeraInsertIntent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewTeraInsertDropExecutionStatus {
    Committed,
    Blocked,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewTeraInsertDropExecutionReceipt {
    pub schema_version: u32,
    pub intent: PreviewProjectionIntentReceipt,
    pub status: PreviewTeraInsertDropExecutionStatus,
    pub message: String,
    pub model_revision: Option<String>,
    pub patch: Option<ProjectTeraInsertPatch>,
    pub canvas_patch: Option<CanvasPatch>,
    pub workspace_mutation: Option<ProjectWorkspaceMutationReceipt>,
    pub touched_files: Vec<String>,
    pub diagnostics: Vec<PreviewProjectionDiagnostic>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewTeraMoveDropExecutionInput {
    pub intent: PreviewProjectionIntentInput,
    pub move_intent: ProjectTeraMoveIntent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewTeraMoveDropExecutionStatus {
    Committed,
    Blocked,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewTeraMoveDropExecutionReceipt {
    pub schema_version: u32,
    pub intent: PreviewProjectionIntentReceipt,
    pub status: PreviewTeraMoveDropExecutionStatus,
    pub message: String,
    pub model_revision: Option<String>,
    pub patch: Option<ProjectTeraMovePatch>,
    pub canvas_patch: Option<CanvasPatch>,
    pub workspace_mutation: Option<ProjectWorkspaceMutationReceipt>,
    pub touched_files: Vec<String>,
    pub diagnostics: Vec<PreviewProjectionDiagnostic>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewTeraDeleteExecutionInput {
    pub intent: PreviewProjectionIntentInput,
    pub delete_intent: ProjectTeraDeleteIntent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewTeraDeleteExecutionStatus {
    Committed,
    Blocked,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewTeraDeleteExecutionReceipt {
    pub schema_version: u32,
    pub intent: PreviewProjectionIntentReceipt,
    pub status: PreviewTeraDeleteExecutionStatus,
    pub message: String,
    pub model_revision: Option<String>,
    pub patch: Option<ProjectTeraDeletePatch>,
    pub canvas_patch: Option<CanvasPatch>,
    pub workspace_mutation: Option<ProjectWorkspaceMutationReceipt>,
    pub touched_files: Vec<String>,
    pub diagnostics: Vec<PreviewProjectionDiagnostic>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewTemplateEditPermissionInput {
    pub intent: PreviewProjectionIntentInput,
    pub edit_intent: ProjectTemplateEditPermissionIntent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewTemplateEditPermissionStatus {
    Granted,
    Blocked,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewTemplateEditPermissionReceipt {
    pub schema_version: u32,
    pub intent: PreviewProjectionIntentReceipt,
    pub status: PreviewTemplateEditPermissionStatus,
    pub message: String,
    pub model_revision: Option<String>,
    pub grant: Option<ProjectTemplateEditPermissionGrant>,
    pub diagnostics: Vec<PreviewProjectionDiagnostic>,
}
