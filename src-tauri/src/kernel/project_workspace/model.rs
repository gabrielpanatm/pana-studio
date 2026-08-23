use std::{
    collections::{BTreeMap, HashMap, HashSet},
    ops::{Deref, DerefMut},
    sync::{Arc, OnceLock},
};

#[cfg(test)]
use std::cell::Cell;

use serde::{Deserialize, Serialize};

use crate::project::AcceptedProjectDiskManifest;

use crate::{
    js::{PageJsDraftStageReceipt, PageJsDraftStoreSnapshot},
    kernel::file_buffer_store::{
        FileBufferFileSnapshot, FileBufferStore, FileBufferStoreSnapshot, FileBufferTextSnapshot,
    },
    kernel::write_authority::WriteReceipt,
    project::ProjectDiskManifest,
};

pub const PROJECT_WORKSPACE_SCHEMA_VERSION: u32 = 3;
pub(crate) const PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_BYTES: u64 = 32 * 1024 * 1024;
pub(crate) const PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_TOTAL_BYTES: u64 = 64 * 1024 * 1024;

type WorkspaceProjectionOwnedView<T> = Arc<OnceLock<Arc<HashMap<String, T>>>>;

#[cfg(test)]
thread_local! {
    static SOURCE_TEXT_DEEP_MATERIALIZATIONS: Cell<u64> = const { Cell::new(0) };
    static RESOURCE_BYTE_DEEP_MATERIALIZATIONS: Cell<u64> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_projection_deep_materializations() {
    SOURCE_TEXT_DEEP_MATERIALIZATIONS.with(|counter| counter.set(0));
    RESOURCE_BYTE_DEEP_MATERIALIZATIONS.with(|counter| counter.set(0));
}

#[cfg(test)]
pub(crate) fn projection_deep_materializations() -> (u64, u64) {
    (
        SOURCE_TEXT_DEEP_MATERIALIZATIONS.with(Cell::get),
        RESOURCE_BYTE_DEEP_MATERIALIZATIONS.with(Cell::get),
    )
}

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
    pub last_projection_transaction_id: Option<String>,
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
    pub documents: Vec<WorkspaceDocumentProjection>,
    pub entry: Option<WorkspaceHistoryEntrySnapshot>,
    pub files: Vec<FileBufferFileSnapshot>,
    pub page_js: Option<PageJsDraftStageReceipt>,
    pub history: WorkspaceHistorySnapshot,
    #[serde(skip)]
    pub(crate) project_model_performance:
        Option<crate::kernel::performance::ProjectModelPerformanceSample>,
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
    pub application_transaction_id: String,
    #[serde(skip)]
    pub(crate) canvas_delta: Option<super::history::WorkspaceCanvasHistoryDelta>,
    #[serde(skip)]
    pub(crate) source_tree: Option<super::history::WorkspaceSourceTreeHistory>,
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

#[derive(Clone, Debug)]
pub struct WorkspaceProjectionSnapshot {
    pub project_root: String,
    pub runtime_session_id: String,
    pub revision: u64,
    /// Transaction that produced the current editable state, when the state
    /// is the result of a recorded workspace mutation. Bootstrap/recovery
    /// projections may legitimately have no originating transaction.
    pub workspace_transaction_id: Option<String>,
    /// Complete materialized text namespace for this exact workspace revision.
    /// Consumers must not fill missing text from the live project disk.
    pub source_texts: WorkspaceProjectionSourceTexts,
    /// Complete staged binary overlay for this exact workspace revision.
    pub resource_bytes: WorkspaceProjectionResourceBytes,
    pub deleted_sources: HashSet<String>,
    /// Paths whose materialized value differs from the accepted disk baseline.
    pub changed_paths: HashSet<String>,
    /// Runtime-scoped disk baseline for non-text assets copied into derived
    /// projections. It is checked both before and after materialization.
    pub accepted_disk: Arc<AcceptedProjectDiskManifest>,
}

/// Copy-on-write text view over the exact materialized FileBufferStore.
///
/// A ProjectModel cache hit only needs projection identity and never pays to
/// duplicate every source. Consumers that actually rebuild a model obtain the
/// semantic-builder map lazily, once per shared projection.
#[derive(Clone, Debug)]
pub struct WorkspaceProjectionSourceTexts {
    state: Arc<WorkspaceProjectionSourceState>,
}

#[derive(Clone, Debug)]
enum WorkspaceProjectionSourceState {
    Materialized {
        documents: Arc<FileBufferStore>,
        owned_view: WorkspaceProjectionOwnedView<String>,
    },
    Owned(HashMap<String, String>),
}

impl WorkspaceProjectionSourceTexts {
    pub(crate) fn from_materialized(documents: FileBufferStore) -> Self {
        Self {
            state: Arc::new(WorkspaceProjectionSourceState::Materialized {
                documents: Arc::new(documents),
                owned_view: Arc::new(OnceLock::new()),
            }),
        }
    }

    pub(crate) fn contains_key(&self, path: &str) -> bool {
        match self.state.as_ref() {
            WorkspaceProjectionSourceState::Materialized { documents, .. } => {
                documents.files.contains_key(path)
            }
            WorkspaceProjectionSourceState::Owned(source_texts) => source_texts.contains_key(path),
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        match self.state.as_ref() {
            WorkspaceProjectionSourceState::Materialized { documents, .. } => documents.files.len(),
            WorkspaceProjectionSourceState::Owned(source_texts) => source_texts.len(),
        }
    }

    pub(crate) fn keys(&self) -> WorkspaceProjectionSourceKeys<'_> {
        match self.state.as_ref() {
            WorkspaceProjectionSourceState::Materialized { documents, .. } => {
                WorkspaceProjectionSourceKeys::Materialized(documents.files.keys())
            }
            WorkspaceProjectionSourceState::Owned(source_texts) => {
                WorkspaceProjectionSourceKeys::Owned(source_texts.keys())
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn shares_file_entries_with(
        &self,
        entries: &Arc<BTreeMap<String, Arc<crate::kernel::file_buffer_store::FileBufferEntry>>>,
    ) -> bool {
        match self.state.as_ref() {
            WorkspaceProjectionSourceState::Materialized { documents, .. } => {
                Arc::ptr_eq(&documents.files, entries)
            }
            WorkspaceProjectionSourceState::Owned(_) => false,
        }
    }

    #[cfg(test)]
    pub(crate) fn owned_view_is_materialized(&self) -> bool {
        match self.state.as_ref() {
            WorkspaceProjectionSourceState::Materialized { owned_view, .. } => {
                owned_view.get().is_some()
            }
            WorkspaceProjectionSourceState::Owned(_) => true,
        }
    }
}

pub(crate) enum WorkspaceProjectionSourceKeys<'a> {
    Materialized(
        std::collections::btree_map::Keys<
            'a,
            String,
            Arc<crate::kernel::file_buffer_store::FileBufferEntry>,
        >,
    ),
    Owned(std::collections::hash_map::Keys<'a, String, String>),
}

impl<'a> Iterator for WorkspaceProjectionSourceKeys<'a> {
    type Item = &'a String;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Materialized(keys) => keys.next(),
            Self::Owned(keys) => keys.next(),
        }
    }
}

impl From<HashMap<String, String>> for WorkspaceProjectionSourceTexts {
    fn from(value: HashMap<String, String>) -> Self {
        Self {
            state: Arc::new(WorkspaceProjectionSourceState::Owned(value)),
        }
    }
}

impl Deref for WorkspaceProjectionSourceTexts {
    type Target = HashMap<String, String>;

    fn deref(&self) -> &Self::Target {
        match self.state.as_ref() {
            WorkspaceProjectionSourceState::Materialized {
                documents,
                owned_view,
            } => owned_view
                .get_or_init(|| {
                    #[cfg(test)]
                    SOURCE_TEXT_DEEP_MATERIALIZATIONS
                        .with(|counter| counter.set(counter.get().saturating_add(1)));
                    Arc::new(
                        documents
                            .files
                            .iter()
                            .map(|(path, entry)| (path.clone(), entry.current_text().to_string()))
                            .collect(),
                    )
                })
                .as_ref(),
            WorkspaceProjectionSourceState::Owned(source_texts) => source_texts,
        }
    }
}

impl DerefMut for WorkspaceProjectionSourceTexts {
    fn deref_mut(&mut self) -> &mut Self::Target {
        if matches!(
            self.state.as_ref(),
            WorkspaceProjectionSourceState::Materialized { .. }
        ) {
            let owned: HashMap<String, String> = Deref::deref(self).clone();
            self.state = Arc::new(WorkspaceProjectionSourceState::Owned(owned));
        }
        match Arc::make_mut(&mut self.state) {
            WorkspaceProjectionSourceState::Owned(source_texts) => source_texts,
            WorkspaceProjectionSourceState::Materialized { .. } => {
                unreachable!("starea materializată a fost detașată înainte de mutație")
            }
        }
    }
}

impl PartialEq for WorkspaceProjectionSourceTexts {
    fn eq(&self, other: &Self) -> bool {
        self.deref() == other.deref()
    }
}

impl Eq for WorkspaceProjectionSourceTexts {}

impl<'a> IntoIterator for &'a WorkspaceProjectionSourceTexts {
    type Item = (&'a String, &'a String);
    type IntoIter = std::collections::hash_map::Iter<'a, String, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.deref().iter()
    }
}

/// Copy-on-write byte view over the exact staged binary-resource map.
#[derive(Clone, Debug)]
pub struct WorkspaceProjectionResourceBytes {
    state: Arc<WorkspaceProjectionResourceState>,
}

#[derive(Clone, Debug)]
enum WorkspaceProjectionResourceState {
    Materialized {
        resources: Arc<BTreeMap<String, Arc<WorkspaceBinaryResource>>>,
        owned_view: WorkspaceProjectionOwnedView<Vec<u8>>,
    },
    Owned(HashMap<String, Vec<u8>>),
}

impl WorkspaceProjectionResourceBytes {
    pub(crate) fn from_materialized(
        resources: Arc<BTreeMap<String, Arc<WorkspaceBinaryResource>>>,
    ) -> Self {
        Self {
            state: Arc::new(WorkspaceProjectionResourceState::Materialized {
                resources,
                owned_view: Arc::new(OnceLock::new()),
            }),
        }
    }

    pub(crate) fn get(&self, path: &str) -> Option<&Vec<u8>> {
        match self.state.as_ref() {
            WorkspaceProjectionResourceState::Materialized { resources, .. } => {
                resources.get(path).map(|resource| &resource.bytes)
            }
            WorkspaceProjectionResourceState::Owned(resource_bytes) => resource_bytes.get(path),
        }
    }

    pub(crate) fn contains_key(&self, path: &str) -> bool {
        match self.state.as_ref() {
            WorkspaceProjectionResourceState::Materialized { resources, .. } => {
                resources.contains_key(path)
            }
            WorkspaceProjectionResourceState::Owned(resource_bytes) => {
                resource_bytes.contains_key(path)
            }
        }
    }

    pub(crate) fn keys(&self) -> WorkspaceProjectionResourceKeys<'_> {
        match self.state.as_ref() {
            WorkspaceProjectionResourceState::Materialized { resources, .. } => {
                WorkspaceProjectionResourceKeys::Materialized(resources.keys())
            }
            WorkspaceProjectionResourceState::Owned(resource_bytes) => {
                WorkspaceProjectionResourceKeys::Owned(resource_bytes.keys())
            }
        }
    }

    pub(crate) fn iter(&self) -> WorkspaceProjectionResourceIter<'_> {
        match self.state.as_ref() {
            WorkspaceProjectionResourceState::Materialized { resources, .. } => {
                WorkspaceProjectionResourceIter::Materialized(resources.iter())
            }
            WorkspaceProjectionResourceState::Owned(resource_bytes) => {
                WorkspaceProjectionResourceIter::Owned(resource_bytes.iter())
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn shares_resources_with(
        &self,
        resources: &Arc<BTreeMap<String, Arc<WorkspaceBinaryResource>>>,
    ) -> bool {
        match self.state.as_ref() {
            WorkspaceProjectionResourceState::Materialized {
                resources: projected,
                ..
            } => Arc::ptr_eq(projected, resources),
            WorkspaceProjectionResourceState::Owned(_) => false,
        }
    }

    #[cfg(test)]
    pub(crate) fn owned_view_is_materialized(&self) -> bool {
        match self.state.as_ref() {
            WorkspaceProjectionResourceState::Materialized { owned_view, .. } => {
                owned_view.get().is_some()
            }
            WorkspaceProjectionResourceState::Owned(_) => true,
        }
    }
}

pub(crate) enum WorkspaceProjectionResourceKeys<'a> {
    Materialized(std::collections::btree_map::Keys<'a, String, Arc<WorkspaceBinaryResource>>),
    Owned(std::collections::hash_map::Keys<'a, String, Vec<u8>>),
}

impl<'a> Iterator for WorkspaceProjectionResourceKeys<'a> {
    type Item = &'a String;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Materialized(keys) => keys.next(),
            Self::Owned(keys) => keys.next(),
        }
    }
}

pub(crate) enum WorkspaceProjectionResourceIter<'a> {
    Materialized(std::collections::btree_map::Iter<'a, String, Arc<WorkspaceBinaryResource>>),
    Owned(std::collections::hash_map::Iter<'a, String, Vec<u8>>),
}

impl<'a> Iterator for WorkspaceProjectionResourceIter<'a> {
    type Item = (&'a String, &'a Vec<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Materialized(resources) => resources
                .next()
                .map(|(path, resource)| (path, &resource.bytes)),
            Self::Owned(resources) => resources.next(),
        }
    }
}

impl From<HashMap<String, Vec<u8>>> for WorkspaceProjectionResourceBytes {
    fn from(value: HashMap<String, Vec<u8>>) -> Self {
        Self {
            state: Arc::new(WorkspaceProjectionResourceState::Owned(value)),
        }
    }
}

impl Deref for WorkspaceProjectionResourceBytes {
    type Target = HashMap<String, Vec<u8>>;

    fn deref(&self) -> &Self::Target {
        match self.state.as_ref() {
            WorkspaceProjectionResourceState::Materialized {
                resources,
                owned_view,
            } => owned_view
                .get_or_init(|| {
                    #[cfg(test)]
                    RESOURCE_BYTE_DEEP_MATERIALIZATIONS
                        .with(|counter| counter.set(counter.get().saturating_add(1)));
                    Arc::new(
                        resources
                            .iter()
                            .map(|(path, resource)| (path.clone(), resource.bytes.clone()))
                            .collect(),
                    )
                })
                .as_ref(),
            WorkspaceProjectionResourceState::Owned(resource_bytes) => resource_bytes,
        }
    }
}

impl DerefMut for WorkspaceProjectionResourceBytes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        if matches!(
            self.state.as_ref(),
            WorkspaceProjectionResourceState::Materialized { .. }
        ) {
            let owned: HashMap<String, Vec<u8>> = Deref::deref(self).clone();
            self.state = Arc::new(WorkspaceProjectionResourceState::Owned(owned));
        }
        match Arc::make_mut(&mut self.state) {
            WorkspaceProjectionResourceState::Owned(resource_bytes) => resource_bytes,
            WorkspaceProjectionResourceState::Materialized { .. } => {
                unreachable!("starea binară materializată a fost detașată înainte de mutație")
            }
        }
    }
}

impl PartialEq for WorkspaceProjectionResourceBytes {
    fn eq(&self, other: &Self) -> bool {
        self.deref() == other.deref()
    }
}

impl Eq for WorkspaceProjectionResourceBytes {}

impl<'a> IntoIterator for &'a WorkspaceProjectionResourceBytes {
    type Item = (&'a String, &'a Vec<u8>);
    type IntoIter = std::collections::hash_map::Iter<'a, String, Vec<u8>>;

    fn into_iter(self) -> Self::IntoIter {
        self.deref().iter()
    }
}

impl PartialEq for WorkspaceProjectionSnapshot {
    fn eq(&self, other: &Self) -> bool {
        self.project_root == other.project_root
            && self.runtime_session_id == other.runtime_session_id
            && self.revision == other.revision
            && self.workspace_transaction_id == other.workspace_transaction_id
            && self.source_texts == other.source_texts
            && self.resource_bytes == other.resource_bytes
            && self.deleted_sources == other.deleted_sources
            && self.changed_paths == other.changed_paths
            && self.accepted_disk == other.accepted_disk
    }
}

impl Eq for WorkspaceProjectionSnapshot {}

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
