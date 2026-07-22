use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::project::AcceptedProjectDiskManifest;

use crate::{
    js::{PageJsDraftStageReceipt, PageJsDraftStoreSnapshot},
    kernel::file_buffer_store::{
        FileBufferFileSnapshot, FileBufferStoreSnapshot, FileBufferTextSnapshot,
    },
    kernel::write_authority::WriteReceipt,
    project::ProjectDiskManifest,
};

pub const PROJECT_WORKSPACE_SCHEMA_VERSION: u32 = 3;
pub(crate) const PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_BYTES: u64 = 32 * 1024 * 1024;
pub(crate) const PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_TOTAL_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkspaceIdentity {
    pub expected_project_root: String,
    pub expected_session_id: String,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkspaceHistoryIdentity {
    pub expected_project_root: String,
    pub expected_session_id: String,
    pub expected_revision: u64,
    pub expected_transaction_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceMutationMetadata {
    pub label: String,
    pub source: String,
    #[serde(default)]
    pub coalesce_key: Option<String>,
    #[serde(default)]
    pub transaction_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDocumentMutation {
    pub relative_path: String,
    pub contents: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceResourceMutation {
    pub relative_path: String,
    pub contents: String,
    #[serde(default)]
    pub create_only: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceResourceDelete {
    pub relative_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceBinaryResource {
    pub relative_path: String,
    #[serde(with = "binary_bytes_base64")]
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub(crate) struct WorkspaceBinaryRestoreChange {
    pub relative_path: String,
    pub before: Option<Vec<u8>>,
    pub after: Option<Vec<u8>>,
}

impl WorkspaceBinaryResource {
    pub fn new(relative_path: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self {
            relative_path: relative_path.into(),
            bytes,
        }
    }
}

mod binary_bytes_base64 {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        STANDARD.decode(encoded).map_err(serde::de::Error::custom)
    }
}

/// A pure text change description used by planners before the mutation is
/// committed to ProjectWorkspace. It never writes to disk by itself.
#[derive(Clone, Debug)]
pub struct WorkspaceTextChange {
    pub relative_path: String,
    pub new_text: String,
}

#[derive(Clone, Debug)]
pub struct WorkspaceTextDelete {
    pub relative_path: String,
}

#[derive(Clone, Debug)]
pub struct WorkspaceTextMutationInput {
    pub label: String,
    pub target: String,
    pub changes: Vec<WorkspaceTextChange>,
}

#[derive(Clone, Debug)]
pub struct WorkspaceTextResourceMutationInput {
    pub label: String,
    pub target: String,
    pub changes: Vec<WorkspaceTextChange>,
    pub deletes: Vec<WorkspaceTextDelete>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkspaceSnapshot {
    pub schema_version: u32,
    pub project_root: String,
    pub runtime_session_id: String,
    pub revision: u64,
    pub disk_generation: u64,
    pub dirty: bool,
    pub dirty_document_count: usize,
    pub created_document_count: usize,
    pub created_documents: Vec<String>,
    pub deleted_document_count: usize,
    pub deleted_documents: Vec<String>,
    pub staged_binary_resource_count: usize,
    pub staged_binary_resource_bytes: u64,
    pub staged_binary_resources: Vec<String>,
    pub deleted_binary_resource_count: usize,
    pub deleted_binary_resources: Vec<String>,
    pub dirty_page_js_count: usize,
    pub project_model_revision: Option<String>,
    pub project_model_source_revision: Option<u64>,
    pub documents: FileBufferStoreSnapshot,
    pub page_js: PageJsDraftStoreSnapshot,
    pub history: WorkspaceHistorySnapshot,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkspaceMutationReceipt {
    pub schema_version: u32,
    pub changed: bool,
    pub revision_before: u64,
    pub revision_after: u64,
    pub dirty: bool,
    pub transaction_id: Option<String>,
    pub touched_files: Vec<String>,
    pub entry: Option<WorkspaceHistoryEntrySnapshot>,
    pub files: Vec<FileBufferFileSnapshot>,
    pub page_js: Option<PageJsDraftStageReceipt>,
    pub history: WorkspaceHistorySnapshot,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceUndoRedoReceipt {
    pub schema_version: u32,
    pub direction: WorkspaceHistoryDirection,
    pub revision_before: u64,
    pub revision_after: u64,
    pub dirty: bool,
    pub entry: WorkspaceHistoryEntrySnapshot,
    pub documents: Vec<WorkspaceDocumentProjection>,
    pub history: WorkspaceHistorySnapshot,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDocumentProjection {
    pub relative_path: String,
    pub snapshot: Option<FileBufferTextSnapshot>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceHistoryDirection {
    Undo,
    Redo,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceHistorySnapshot {
    pub undo_count: usize,
    pub redo_count: usize,
    pub can_undo: bool,
    pub can_redo: bool,
    pub retained_bytes: u64,
    pub retained_bytes_limit: u64,
    pub entry_limit: usize,
    pub next_undo: Option<WorkspaceHistoryEntrySnapshot>,
    pub next_redo: Option<WorkspaceHistoryEntrySnapshot>,
    pub undo_entries: Vec<WorkspaceHistoryEntrySnapshot>,
    pub redo_entries: Vec<WorkspaceHistoryEntrySnapshot>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceHistoryEntrySnapshot {
    pub transaction_id: String,
    pub label: String,
    pub source: String,
    pub coalesce_key: Option<String>,
    pub created_at_ms: u128,
    pub updated_at_ms: u128,
    pub mutation_count: u32,
    pub document_paths: Vec<String>,
    /// Paths whose existence changes when this history entry is applied.
    /// Content-only resource mutations deliberately stay out of this list so
    /// the frontend can re-scan project topology only when it is necessary.
    pub topology_paths: Vec<String>,
    pub page_js_paths: Vec<String>,
    pub retained_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceProjectionLease {
    pub project_root: String,
    pub runtime_session_id: String,
    pub revision: u64,
    /// Transaction that produced the current editable state, when the state
    /// is the result of a recorded workspace mutation. Bootstrap/recovery
    /// projections may legitimately have no originating transaction.
    pub workspace_transaction_id: Option<String>,
    /// Complete materialized text namespace for this exact workspace revision.
    /// Consumers must not fill missing text from the live project disk.
    pub source_texts: HashMap<String, String>,
    /// Complete staged binary overlay for this exact workspace revision.
    pub resource_bytes: HashMap<String, Vec<u8>>,
    pub deleted_sources: HashSet<String>,
    /// Paths whose materialized value differs from the accepted disk baseline.
    pub changed_paths: HashSet<String>,
    /// Runtime-scoped disk baseline for non-text assets copied into derived
    /// projections. It is checked both before and after materialization.
    pub accepted_disk: AcceptedProjectDiskManifest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectWorkspaceSaveStatus {
    Noop,
    Saved,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkspaceSaveReceipt {
    pub schema_version: u32,
    pub transaction_id: Option<String>,
    pub status: ProjectWorkspaceSaveStatus,
    pub project_root: String,
    pub runtime_session_id: String,
    pub revision_before: u64,
    pub revision_after: u64,
    pub disk_generation_before: u64,
    pub disk_generation_after: u64,
    pub written_files: Vec<String>,
    pub removed_files: Vec<String>,
    pub write_receipts: Vec<WriteReceipt>,
    pub accepted_manifest: ProjectDiskManifest,
    pub workspace: ProjectWorkspaceSnapshot,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum ProjectWorkspaceSaveError {
    Rejected {
        diagnostic: String,
    },
    RecoveryRequired {
        transaction_id: String,
        touched_files: Vec<String>,
        committed_writes: Vec<WriteReceipt>,
        diagnostic: String,
        retry_forbidden: bool,
    },
}

impl ProjectWorkspaceSaveError {
    pub fn rejected(diagnostic: impl Into<String>) -> Self {
        Self::Rejected {
            diagnostic: diagnostic.into(),
        }
    }

    pub fn recovery_required(
        transaction_id: impl Into<String>,
        touched_files: Vec<String>,
        committed_writes: Vec<WriteReceipt>,
        diagnostic: impl Into<String>,
    ) -> Self {
        Self::RecoveryRequired {
            transaction_id: transaction_id.into(),
            touched_files,
            committed_writes,
            diagnostic: diagnostic.into(),
            retry_forbidden: true,
        }
    }
}

impl std::fmt::Display for ProjectWorkspaceSaveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected { diagnostic } => formatter.write_str(diagnostic),
            Self::RecoveryRequired { diagnostic, .. } => write!(
                formatter,
                "PROJECT_WORKSPACE_SAVE_RECOVERY_REQUIRED: {diagnostic} Nu repeta Save automat."
            ),
        }
    }
}

impl std::error::Error for ProjectWorkspaceSaveError {}
