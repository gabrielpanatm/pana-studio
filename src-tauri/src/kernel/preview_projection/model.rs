use std::{
    collections::BTreeMap,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    kernel::{
        project_workspace::ProjectWorkspaceMutationReceipt,
        selection_coordinator::SelectionMutationIdentity,
    },
    localization::LocalizedDiagnostic,
    project_model::{
        attribute_engine::{
            ProjectHtmlAttributeIntent, ProjectHtmlAttributeMutation, ProjectHtmlAttributePatch,
        },
        delete_engine::{ProjectHtmlDeleteIntent, ProjectHtmlDeletePatch},
        duplicate_engine::{ProjectHtmlDuplicateIntent, ProjectHtmlDuplicatePatch},
        insert_engine::{ProjectHtmlInsertIntent, ProjectHtmlInsertPatch},
        move_engine::ProjectMovePosition,
        tag_engine::{ProjectHtmlTagIntent, ProjectHtmlTagPatch},
        tera_delete_engine::{ProjectTeraDeleteIntent, ProjectTeraDeletePatch},
        tera_insert_engine::{ProjectTeraInsertIntent, ProjectTeraInsertPatch},
        text_engine::{ProjectHtmlTextIntent, ProjectHtmlTextPatch},
    },
};

pub const PREVIEW_HTML_INSERT_DROP_EXECUTION_SCHEMA_VERSION: u32 = 2;
pub const PREVIEW_HTML_ATTRIBUTES_EXECUTION_SCHEMA_VERSION: u32 = 2;
pub const PREVIEW_HTML_TEXT_EXECUTION_SCHEMA_VERSION: u32 = 2;
pub const PREVIEW_HTML_TAG_EXECUTION_SCHEMA_VERSION: u32 = 2;
pub const PREVIEW_HTML_DUPLICATE_EXECUTION_SCHEMA_VERSION: u32 = 2;
pub const PREVIEW_HTML_DELETE_EXECUTION_SCHEMA_VERSION: u32 = 2;
pub const PREVIEW_TERA_INSERT_DROP_EXECUTION_SCHEMA_VERSION: u32 = 2;
pub const PREVIEW_TERA_DELETE_EXECUTION_SCHEMA_VERSION: u32 = 2;
pub const PREVIEW_SELECTION_BATCH_EXECUTION_SCHEMA_VERSION: u32 = 1;
pub const CANVAS_PATCH_SCHEMA_VERSION: u32 = 1;
const MAX_CANVAS_PATCH_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewStructuralCommandIdentity {
    pub expected_project_root: String,
    pub expected_session_id: String,
    #[serde(default)]
    pub expected_selection: Option<PreviewStructuralSelectionIdentity>,
}

pub type PreviewStructuralSelectionIdentity = SelectionMutationIdentity;

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PreviewSelectionBatchAction {
    SetAttributes {
        #[serde(default)]
        attributes: Vec<ProjectHtmlAttributeMutation>,
    },
    MutateClasses {
        #[serde(default)]
        add: Vec<String>,
        #[serde(default)]
        remove: Vec<String>,
    },
    GenerateSharedClass,
    Duplicate,
    Delete,
    Move {
        target_source_id: String,
        #[serde(default)]
        target_tag: Option<String>,
        position: ProjectMovePosition,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewSelectionBatchExecutionInput {
    pub schema_version: u32,
    pub action: PreviewSelectionBatchAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewSelectionBatchExecutionStatus {
    Committed,
    Blocked,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewSelectionBatchExecutionReceipt {
    pub schema_version: u32,
    pub status: PreviewSelectionBatchExecutionStatus,
    pub model_revision: Option<String>,
    pub affected_source_ids: Vec<String>,
    pub primary_affected_source_id: Option<String>,
    pub generated_class: Option<String>,
    pub canvas_patch: Option<CanvasPatch>,
    pub workspace_mutation: Option<ProjectWorkspaceMutationReceipt>,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewProjectionIntentInput {
    pub message_type: String,
    #[serde(default)]
    pub preview_revision: Option<u64>,
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
    HtmlInsertDrop,
    HtmlAttributes,
    HtmlText,
    HtmlTag,
    HtmlDuplicate,
    TeraInsertDrop,
    HtmlDelete,
    TemplateDelete,
    Unsupported,
}

impl PreviewProjectionIntentKind {
    pub fn operation_label(self) -> &'static str {
        match self {
            Self::HtmlInsertDrop => "preview.html.insert_drop",
            Self::HtmlAttributes => "preview.html.attributes",
            Self::HtmlText => "preview.html.text",
            Self::HtmlTag => "preview.html.tag",
            Self::HtmlDuplicate => "preview.html.duplicate_selected",
            Self::TeraInsertDrop => "preview.tera.insert_drop",
            Self::HtmlDelete => "preview.html.delete_selected",
            Self::TemplateDelete => "preview.template.delete_selected",
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
    pub diagnostic: LocalizedDiagnostic,
    pub blocking: bool,
}

impl PreviewProjectionDiagnostic {
    pub fn blocking(code: impl Into<String>, diagnostic: LocalizedDiagnostic) -> Self {
        Self {
            code: code.into(),
            severity: PreviewProjectionDiagnosticSeverity::Error,
            diagnostic,
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
    pub message_diagnostic: LocalizedDiagnostic,
    pub diagnostics: Vec<PreviewProjectionDiagnostic>,
}

/// A source-backed anchor for a one-shot optimistic Canvas patch. Source IDs
/// are authoritative. Render instance IDs are populated when the active
/// CanvasGraph can disambiguate a repeated source occurrence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasPatchAnchor {
    pub source_id: String,
    pub render_instance_id: Option<String>,
    pub expected_tag: Option<String>,
}

impl CanvasPatchAnchor {
    pub(crate) fn source(source_id: impl Into<String>, expected_tag: Option<&str>) -> Self {
        Self {
            source_id: source_id.into(),
            render_instance_id: None,
            expected_tag: bounded_optional(expected_tag, 128),
        }
    }

    pub(crate) fn source_instance(
        source_id: impl Into<String>,
        render_instance_id: Option<&str>,
        expected_tag: Option<&str>,
    ) -> Self {
        Self {
            source_id: source_id.into(),
            render_instance_id: bounded_optional(render_instance_id, 512),
            expected_tag: bounded_optional(expected_tag, 128),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CanvasPatchOperation {
    Batch {
        operations: Vec<CanvasPatchOperation>,
    },
    SetAttributes {
        target: CanvasPatchAnchor,
        attributes: BTreeMap<String, Option<String>>,
    },
    SetBlockOption {
        target: CanvasPatchAnchor,
        provider_id: String,
        option_id: String,
        attribute: String,
        value: Option<String>,
    },
    SetIcon {
        target: CanvasPatchAnchor,
        provider_id: String,
        icon_identity: String,
        attributes: BTreeMap<String, Option<String>>,
        children_html: String,
    },
    SetText {
        target: CanvasPatchAnchor,
        text: String,
    },
    SetTextHtml {
        target: CanvasPatchAnchor,
        escaped_text: String,
    },
    ReplaceTag {
        target: CanvasPatchAnchor,
        new_tag: String,
    },
    Insert {
        target: CanvasPatchAnchor,
        position: ProjectMovePosition,
        html: String,
        #[serde(default)]
        inserted: Option<CanvasPatchAnchor>,
    },
    Move {
        source: CanvasPatchAnchor,
        target: CanvasPatchAnchor,
        position: ProjectMovePosition,
    },
    Duplicate {
        source: CanvasPatchAnchor,
        html: String,
        #[serde(default)]
        inserted: Option<CanvasPatchAnchor>,
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
        require_canvas_patch_operation(&operation)?;
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

    // The signed history identity keeps every canonicalized field explicit and ordered.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn issued_for_history(
        project_root: &str,
        runtime_session_id: &str,
        base_workspace_revision: u64,
        workspace_revision: u64,
        workspace_transaction_id: &str,
        before_model_revision: &str,
        after_model_revision: &str,
        operation: CanvasPatchOperation,
    ) -> Result<Self, String> {
        let transaction_id = workspace_transaction_id.trim();
        if transaction_id.is_empty()
            || transaction_id.len() > 256
            || workspace_revision <= base_workspace_revision
            || project_root.trim().is_empty()
            || runtime_session_id.trim().is_empty()
            || before_model_revision.trim().is_empty()
            || after_model_revision.trim().is_empty()
        {
            return Err(
                "CanvasPatch History a refuzat o identitate sau revizie invalidă.".to_string(),
            );
        }
        require_canvas_patch_operation(&operation)?;
        let canonical = serde_json::to_vec(&(
            CANVAS_PATCH_SCHEMA_VERSION,
            project_root,
            runtime_session_id,
            base_workspace_revision,
            workspace_revision,
            transaction_id,
            before_model_revision,
            after_model_revision,
            &operation,
        ))
        .map_err(|error| format!("CanvasPatch History nu a putut fi serializat: {error}"))?;
        if canonical.len() > MAX_CANVAS_PATCH_BYTES {
            return Err("CanvasPatch History depășește bugetul de 2 MiB.".to_string());
        }
        Ok(Self {
            schema_version: CANVAS_PATCH_SCHEMA_VERSION,
            patch_id: format!("canvas_patch_{}", full_hex(&Sha256::digest(&canonical))),
            issued_at_ms: Self::current_time_ms(),
            project_root: project_root.to_string(),
            runtime_session_id: runtime_session_id.to_string(),
            base_workspace_revision,
            workspace_revision,
            workspace_transaction_id: transaction_id.to_string(),
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

fn require_canvas_patch_operation(operation: &CanvasPatchOperation) -> Result<(), String> {
    if let CanvasPatchOperation::Batch { operations } = operation {
        if operations.is_empty() || operations.len() > 256 {
            return Err("CanvasPatch batch cere între 1 și 256 de operații.".to_string());
        }
        if operations
            .iter()
            .any(|operation| matches!(operation, CanvasPatchOperation::Batch { .. }))
        {
            return Err("CanvasPatch batch nu permite batch-uri imbricate.".to_string());
        }
    }
    Ok(())
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
    pub message_diagnostic: LocalizedDiagnostic,
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
    pub message_diagnostic: LocalizedDiagnostic,
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
    pub message_diagnostic: LocalizedDiagnostic,
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
    pub message_diagnostic: LocalizedDiagnostic,
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
    pub message_diagnostic: LocalizedDiagnostic,
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
    pub message_diagnostic: LocalizedDiagnostic,
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
    pub message_diagnostic: LocalizedDiagnostic,
    pub model_revision: Option<String>,
    pub patch: Option<ProjectTeraInsertPatch>,
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
    pub message_diagnostic: LocalizedDiagnostic,
    pub model_revision: Option<String>,
    pub patch: Option<ProjectTeraDeletePatch>,
    pub canvas_patch: Option<CanvasPatch>,
    pub workspace_mutation: Option<ProjectWorkspaceMutationReceipt>,
    pub touched_files: Vec<String>,
    pub diagnostics: Vec<PreviewProjectionDiagnostic>,
}

#[cfg(test)]
mod performance_tests {
    use std::{collections::BTreeMap, hint::black_box, time::Instant};

    use super::{CanvasPatch, CanvasPatchAnchor, CanvasPatchOperation};

    fn batch_attribute_operation(index: usize) -> CanvasPatchOperation {
        CanvasPatchOperation::SetAttributes {
            target: CanvasPatchAnchor::source(format!("sgn_batch_{index}"), Some("div")),
            attributes: BTreeMap::from([("class".to_string(), Some("selected".to_string()))]),
        }
    }

    fn issue_batch(operations: Vec<CanvasPatchOperation>) -> Result<CanvasPatch, String> {
        CanvasPatch::issued_for_history(
            "/project",
            "session",
            1,
            2,
            "transaction",
            "before",
            "after",
            CanvasPatchOperation::Batch { operations },
        )
    }

    #[test]
    fn canvas_batch_contract_is_bounded_and_non_recursive() {
        assert!(issue_batch(Vec::new()).is_err());
        assert!(issue_batch(vec![CanvasPatchOperation::Batch {
            operations: vec![batch_attribute_operation(0)],
        }])
        .is_err());
        assert!(issue_batch((0..=256).map(batch_attribute_operation).collect()).is_err());
        assert!(issue_batch((0..256).map(batch_attribute_operation).collect()).is_ok());
    }

    #[test]
    #[ignore = "release performance budget"]
    fn canvas_patch_warm_p95_is_below_sixteen_milliseconds() {
        let payload = "x".repeat(256 * 1024);
        let mut samples = Vec::with_capacity(256);
        for sample in 0..264u64 {
            let operation = CanvasPatchOperation::SetAttributes {
                target: CanvasPatchAnchor::source_instance(
                    "sgn_benchmark_target",
                    Some("render_benchmark_target"),
                    Some("div"),
                ),
                attributes: BTreeMap::from([
                    ("data-payload".to_string(), Some(payload.clone())),
                    ("aria-label".to_string(), Some("benchmark".to_string())),
                ]),
            };
            let started = Instant::now();
            let patch = CanvasPatch::issued_for_history(
                "/benchmark/project",
                "benchmark-session",
                sample + 1,
                sample + 2,
                &format!("benchmark-transaction-{sample}"),
                "model-before",
                "model-after",
                operation,
            )
            .unwrap();
            let elapsed = started.elapsed().as_nanos();
            black_box(patch.patch_id);
            if sample >= 8 {
                samples.push(elapsed);
            }
        }
        samples.sort_unstable();
        let percentile =
            |percent: usize| samples[(samples.len() * percent).div_ceil(100).saturating_sub(1)];
        let p50 = percentile(50);
        let p95 = percentile(95);
        let p99 = percentile(99);
        eprintln!(
            "canvas_patch payload_bytes={} p50_ns={p50} p95_ns={p95} p99_ns={p99}",
            payload.len()
        );
        assert!(
            p95 < 16_000_000,
            "CanvasPatch warm p95 {} ms depășește bugetul de 16 ms",
            p95 as f64 / 1_000_000.0
        );
    }

    #[test]
    #[ignore = "release performance budget"]
    fn canvas_batch_patch_warm_p95_is_below_fifty_milliseconds() {
        let operations = (0..256).map(batch_attribute_operation).collect::<Vec<_>>();
        let mut samples = Vec::with_capacity(128);
        for sample in 0..136u64 {
            let started = Instant::now();
            let patch = CanvasPatch::issued_for_history(
                "/benchmark/project",
                "benchmark-session",
                sample + 1,
                sample + 2,
                &format!("benchmark-batch-{sample}"),
                "model-before",
                "model-after",
                CanvasPatchOperation::Batch {
                    operations: operations.clone(),
                },
            )
            .unwrap();
            let elapsed = started.elapsed().as_nanos();
            black_box(patch.patch_id);
            if sample >= 8 {
                samples.push(elapsed);
            }
        }
        samples.sort_unstable();
        let p95 = samples[(samples.len() * 95).div_ceil(100).saturating_sub(1)];
        eprintln!("canvas_batch_patch members=256 p95_ns={p95}");
        assert!(
            p95 < 50_000_000,
            "CanvasPatch batch warm p95 {} ms depășește bugetul de 50 ms",
            p95 as f64 / 1_000_000.0
        );
    }
}
