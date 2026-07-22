use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::Path,
};

use crate::{
    js::{PageJsConfig, PageJsDraftStageInput, PageJsDraftStore},
    kernel::{
        file_buffer_store::{
            hash_bytes, hash_text, FileBufferDraft, FileBufferEntry, FileBufferMutationExpectation,
            FileBufferStore,
        },
        project_path::normalize_project_relative_path,
        project_session::ProjectSessionSnapshot,
    },
    project::AcceptedProjectDiskManifest,
    project_model::model::ProjectModel,
};

use super::{
    history::{
        new_history_entry, transition_is_no_op, WorkspaceBinaryResourceTransition,
        WorkspaceDocumentTransition, WorkspaceHistory, WorkspaceHistoryEntry,
        WorkspacePageJsTransition, WorkspaceResourceTransition,
    },
    model::{
        ProjectWorkspaceIdentity, ProjectWorkspaceMutationReceipt, ProjectWorkspaceSnapshot,
        WorkspaceBinaryResource, WorkspaceBinaryRestoreChange, WorkspaceDocumentMutation,
        WorkspaceDocumentProjection, WorkspaceHistoryDirection, WorkspaceMutationMetadata,
        WorkspaceProjectionLease, WorkspaceResourceDelete, WorkspaceResourceMutation,
        WorkspaceUndoRedoReceipt, PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_BYTES,
        PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_TOTAL_BYTES, PROJECT_WORKSPACE_SCHEMA_VERSION,
    },
};

#[derive(Clone)]
pub struct ProjectWorkspace {
    pub schema_version: u32,
    pub session: ProjectSessionSnapshot,
    pub accepted_disk: AcceptedProjectDiskManifest,
    pub documents: FileBufferStore,
    pub(super) accepted_documents: BTreeMap<String, FileBufferEntry>,
    /// Content hashes for binary resources whose bytes were committed by this
    /// ProjectWorkspace. The accepted disk manifest proves identity and
    /// freshness, while these hashes let History normalize Redo back to the
    /// clean accepted state without reading or rewriting the project disk.
    pub(super) accepted_binary_resource_hashes: BTreeMap<String, String>,
    pub(super) binary_resources: BTreeMap<String, WorkspaceBinaryResource>,
    pub(super) deleted_binary_resources: std::collections::BTreeSet<String>,
    pub page_js: PageJsDraftStore,
    pub(super) accepted_page_js: BTreeMap<String, PageJsConfig>,
    pub revision: u64,
    pub project_model: Option<ProjectModel>,
    pub project_model_source_revision: Option<u64>,
    pub source_identity_aliases: HashMap<String, String>,
    pub(super) history: WorkspaceHistory,
}

impl ProjectWorkspace {
    pub fn new(
        session: ProjectSessionSnapshot,
        accepted_disk: AcceptedProjectDiskManifest,
        documents: FileBufferStore,
        page_js: PageJsDraftStore,
    ) -> Result<Self, String> {
        let runtime_session_id = session.runtime_instance_id();
        if documents.project_root != session.project_root
            || documents.runtime_session_id != runtime_session_id
        {
            return Err(format!(
                "ProjectWorkspace a refuzat FileBufferStore din altă sesiune: {}/{}.",
                documents.project_root, documents.runtime_session_id
            ));
        }
        page_js.require_identity(&session.project_root, &runtime_session_id)?;
        accepted_disk.require_identity(&runtime_session_id, &session.project_root)?;
        accepted_disk.require_complete()?;

        Ok(Self {
            schema_version: PROJECT_WORKSPACE_SCHEMA_VERSION,
            session,
            accepted_disk,
            accepted_documents: documents.files.clone(),
            documents,
            accepted_binary_resource_hashes: BTreeMap::new(),
            binary_resources: BTreeMap::new(),
            deleted_binary_resources: std::collections::BTreeSet::new(),
            page_js,
            accepted_page_js: BTreeMap::new(),
            revision: 0,
            project_model: None,
            project_model_source_revision: None,
            source_identity_aliases: HashMap::new(),
            history: WorkspaceHistory::default(),
        })
    }

    pub fn runtime_session_id(&self) -> String {
        self.session.runtime_instance_id()
    }

    pub(crate) fn staged_binary_resources(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.binary_resources
            .iter()
            .map(|(path, resource)| (path.as_str(), resource.bytes.as_slice()))
    }

    pub(crate) fn staged_binary_resource(&self, relative_path: &str) -> Option<&[u8]> {
        self.binary_resources
            .get(relative_path)
            .map(|resource| resource.bytes.as_slice())
    }

    pub(crate) fn deleted_binary_resources(&self) -> impl Iterator<Item = &str> {
        self.deleted_binary_resources.iter().map(String::as_str)
    }

    pub fn is_dirty(&self) -> bool {
        self.documents.snapshot().dirty_file_count > 0
            || self
                .documents
                .files
                .keys()
                .ne(self.accepted_documents.keys())
            || self.page_js.dirty_count() > 0
            || !self.binary_resources.is_empty()
            || !self.deleted_binary_resources.is_empty()
    }

    pub fn snapshot(&self) -> ProjectWorkspaceSnapshot {
        let documents = self.documents.snapshot();
        let page_js = self.page_js.snapshot();
        let created_documents = self.created_document_paths();
        let deleted_documents = self.deleted_document_paths();
        let staged_binary_resources = self.binary_resources.keys().cloned().collect::<Vec<_>>();
        let staged_binary_resource_bytes = self
            .binary_resources
            .values()
            .map(|resource| resource.bytes.len() as u64)
            .sum();
        let deleted_binary_resources = self
            .deleted_binary_resources
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        ProjectWorkspaceSnapshot {
            schema_version: self.schema_version,
            project_root: self.session.project_root.clone(),
            runtime_session_id: self.runtime_session_id(),
            revision: self.revision,
            disk_generation: self.accepted_disk.generation,
            dirty: documents.dirty_file_count > 0
                || !created_documents.is_empty()
                || !deleted_documents.is_empty()
                || !staged_binary_resources.is_empty()
                || !deleted_binary_resources.is_empty()
                || page_js.dirty_count > 0,
            dirty_document_count: documents.dirty_file_count,
            created_document_count: created_documents.len(),
            created_documents,
            deleted_document_count: deleted_documents.len(),
            deleted_documents,
            staged_binary_resource_count: staged_binary_resources.len(),
            staged_binary_resource_bytes,
            staged_binary_resources,
            deleted_binary_resource_count: deleted_binary_resources.len(),
            deleted_binary_resources,
            dirty_page_js_count: page_js.dirty_count,
            project_model_revision: self
                .project_model
                .as_ref()
                .map(|model| model.revision.clone()),
            project_model_source_revision: self.project_model_source_revision,
            documents,
            page_js,
            history: self.history.snapshot(),
        }
    }

    pub fn accept_reconciled_disk_state(
        &mut self,
        accepted_disk: AcceptedProjectDiskManifest,
        documents: FileBufferStore,
        invalidate_history: bool,
    ) -> Result<(), String> {
        let runtime_session_id = self.runtime_session_id();
        accepted_disk.require_identity(&runtime_session_id, &self.session.project_root)?;
        accepted_disk.require_complete()?;
        if documents.runtime_session_id != runtime_session_id
            || documents.project_root != self.session.project_root
        {
            return Err(
                "ProjectWorkspace a refuzat reconcilierea documentelor din altă sesiune."
                    .to_string(),
            );
        }

        let old_manifest_entries = self
            .accepted_disk
            .manifest
            .files
            .iter()
            .map(|entry| (entry.relative_path.as_str(), entry))
            .collect::<HashMap<_, _>>();
        let new_manifest_entries = accepted_disk
            .manifest
            .files
            .iter()
            .map(|entry| (entry.relative_path.as_str(), entry))
            .collect::<HashMap<_, _>>();
        let unchanged_accepted_binary_paths = self
            .accepted_binary_resource_hashes
            .keys()
            .filter(|path| {
                old_manifest_entries.get(path.as_str()) == new_manifest_entries.get(path.as_str())
            })
            .cloned()
            .collect::<HashSet<_>>();
        let binary_history_invalidated =
            unchanged_accepted_binary_paths.len() != self.accepted_binary_resource_hashes.len();

        self.accepted_documents = documents.files.clone();
        self.documents = documents;
        self.accepted_binary_resource_hashes
            .retain(|path, _| unchanged_accepted_binary_paths.contains(path));
        self.binary_resources.clear();
        self.deleted_binary_resources.clear();
        self.accepted_disk = accepted_disk;
        if invalidate_history || binary_history_invalidated {
            self.history = WorkspaceHistory::default();
        } else {
            self.history.break_coalescing_group();
        }
        self.project_model = None;
        self.project_model_source_revision = None;
        self.source_identity_aliases.clear();
        self.revision = self.revision.saturating_add(1);
        Ok(())
    }

    pub fn capture_projection_lease(&self) -> Result<WorkspaceProjectionLease, String> {
        let materialized = super::save::materialize_workspace_for_projection(self)?;
        let workspace_transaction_id = self
            .history
            .snapshot()
            .next_undo
            .map(|entry| entry.transaction_id);
        let changed_paths = materialized
            .documents
            .files
            .iter()
            .filter_map(|(path, entry)| match self.accepted_documents.get(path) {
                Some(accepted) if accepted.current_text() == entry.current_text() => None,
                _ => Some(path.clone()),
            })
            .chain(self.binary_resources.keys().cloned())
            .collect();
        Ok(WorkspaceProjectionLease {
            project_root: self.session.project_root.clone(),
            runtime_session_id: self.runtime_session_id(),
            revision: self.revision,
            workspace_transaction_id,
            source_texts: materialized
                .documents
                .files
                .iter()
                .map(|(path, entry)| (path.clone(), entry.current_text().to_string()))
                .collect(),
            resource_bytes: self
                .binary_resources
                .iter()
                .map(|(path, resource)| (path.clone(), resource.bytes.clone()))
                .collect(),
            deleted_sources: materialized
                .deleted_documents
                .into_iter()
                .chain(self.deleted_binary_resources.iter().cloned())
                .collect(),
            changed_paths,
            accepted_disk: self.accepted_disk.clone(),
        })
    }

    pub fn publish_project_model(
        &mut self,
        lease: &WorkspaceProjectionLease,
        model: ProjectModel,
    ) -> Result<(), String> {
        self.require_current_projection(lease)?;
        self.project_model = Some(model);
        self.project_model_source_revision = Some(self.revision);
        Ok(())
    }

    pub fn stage_document_texts(
        &mut self,
        identity: &ProjectWorkspaceIdentity,
        metadata: WorkspaceMutationMetadata,
        mutations: Vec<WorkspaceDocumentMutation>,
        now_ms: u128,
    ) -> Result<ProjectWorkspaceMutationReceipt, String> {
        self.require_identity(identity)?;
        let revision_before = self.revision;
        if mutations.is_empty() {
            return Ok(self.mutation_receipt(revision_before, Vec::new(), None, None));
        }
        require_distinct_document_paths(&mutations)?;

        let mut next_documents = self.documents.clone();
        let mut transitions = Vec::with_capacity(mutations.len());
        let mut files = Vec::with_capacity(mutations.len());
        for mutation in mutations {
            let before = next_documents
                .text_snapshot(&mutation.relative_path)
                .ok_or_else(|| {
                    format!(
                        "ProjectWorkspace nu urmărește documentul {}.",
                        mutation.relative_path
                    )
                })?;
            let transition = WorkspaceDocumentTransition {
                relative_path: mutation.relative_path.clone(),
                before: before.text,
                after: mutation.contents,
            };
            if transition_is_no_op(&transition) {
                files.push(
                    next_documents
                        .files
                        .get(&transition.relative_path)
                        .expect("document checked above")
                        .snapshot(),
                );
                continue;
            }
            let expectation = FileBufferMutationExpectation {
                expected_revision: before.revision,
                expected_hash: before.hash,
            };
            files.push(next_documents.set_draft_if_current(
                &transition.relative_path,
                transition.after.clone(),
                &expectation,
                now_ms,
            )?);
            transitions.push(transition);
        }

        if transitions.is_empty() {
            return Ok(self.mutation_receipt(revision_before, files, None, None));
        }
        let entry = new_history_entry(
            transaction_id(&metadata, revision_before),
            normalized_label(&metadata.label),
            normalized_source(&metadata.source),
            normalized_optional(metadata.coalesce_key),
            now_ms,
            transitions,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let entry_snapshot = entry.snapshot();
        self.documents = next_documents;
        self.commit_mutation(entry)?;
        Ok(self.mutation_receipt(revision_before, files, None, Some(entry_snapshot)))
    }

    pub fn stage_resource_texts(
        &mut self,
        identity: &ProjectWorkspaceIdentity,
        metadata: WorkspaceMutationMetadata,
        mutations: Vec<WorkspaceResourceMutation>,
        now_ms: u128,
    ) -> Result<ProjectWorkspaceMutationReceipt, String> {
        self.stage_resource_changes(identity, metadata, mutations, Vec::new(), now_ms)
    }

    /// Stages binary project resources as create-only session state. Binary
    /// bytes are persisted only by the ProjectWorkspace Save transaction.
    pub fn stage_binary_resource_creates(
        &mut self,
        identity: &ProjectWorkspaceIdentity,
        metadata: WorkspaceMutationMetadata,
        resources: Vec<WorkspaceBinaryResource>,
        now_ms: u128,
    ) -> Result<ProjectWorkspaceMutationReceipt, String> {
        self.require_identity(identity)?;
        let revision_before = self.revision;
        if resources.is_empty() {
            return Ok(self.mutation_receipt(revision_before, Vec::new(), None, None));
        }
        let mut seen = HashSet::with_capacity(resources.len());
        let mut next_resources = self.binary_resources.clone();
        let accepted_paths = self
            .accepted_disk
            .manifest
            .files
            .iter()
            .map(|entry| entry.relative_path.as_str())
            .collect::<HashSet<_>>();
        let mut transitions = Vec::new();

        for mut resource in resources {
            let normalized = normalize_project_relative_path(&resource.relative_path)?;
            self.require_editable_source_path(&normalized)?;
            if normalized != resource.relative_path || !seen.insert(normalized.clone()) {
                return Err(format!(
                    "ProjectWorkspace a refuzat resursa binară necanonică sau duplicată: {}.",
                    resource.relative_path
                ));
            }
            if resource.bytes.len() as u64 > PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_BYTES {
                return Err(format!(
                    "ProjectWorkspace a refuzat {normalized}: {} bytes depășesc limita de {PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_BYTES}.",
                    resource.bytes.len()
                ));
            }
            if self.documents.files.contains_key(&normalized)
                || accepted_paths.contains(normalized.as_str())
                || self.deleted_binary_resources.contains(&normalized)
            {
                return Err(format!(
                    "ProjectWorkspace a refuzat create-only pentru {normalized}: path-ul există în namespace-ul text sau în baseline-ul disk acceptat."
                ));
            }
            resource.relative_path = normalized.clone();
            match next_resources.get(&normalized) {
                Some(existing) if hash_bytes(&existing.bytes) == hash_bytes(&resource.bytes) => {
                    continue;
                }
                Some(_) => {
                    return Err(format!(
                        "ProjectWorkspace a refuzat înlocuirea resursei binare create-only {normalized}."
                    ));
                }
                None => {}
            }
            transitions.push(WorkspaceBinaryResourceTransition {
                relative_path: normalized.clone(),
                before: None,
                after: Some(resource.clone()),
            });
            next_resources.insert(normalized, resource);
        }

        let total_bytes = next_resources
            .values()
            .try_fold(0_u64, |total, resource| {
                total.checked_add(resource.bytes.len() as u64)
            })
            .ok_or_else(|| {
                "ProjectWorkspace a depășit contorul resurselor binare din sesiune.".to_string()
            })?;
        if total_bytes > PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_TOTAL_BYTES {
            return Err(format!(
                "ProjectWorkspace a refuzat resursele binare: {total_bytes} bytes depășesc limita totală de {PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_TOTAL_BYTES}."
            ));
        }
        if transitions.is_empty() {
            return Ok(self.mutation_receipt(revision_before, Vec::new(), None, None));
        }
        let entry = new_history_entry(
            transaction_id(&metadata, revision_before),
            normalized_label(&metadata.label),
            normalized_source(&metadata.source),
            normalized_optional(metadata.coalesce_key),
            now_ms,
            Vec::new(),
            Vec::new(),
            transitions,
            Vec::new(),
        );
        let entry_snapshot = entry.snapshot();
        self.binary_resources = next_resources;
        self.commit_mutation(entry)?;
        Ok(self.mutation_receipt(revision_before, Vec::new(), None, Some(entry_snapshot)))
    }

    fn stage_binary_restore_changes(
        &mut self,
        identity: &ProjectWorkspaceIdentity,
        metadata: WorkspaceMutationMetadata,
        changes: Vec<WorkspaceBinaryRestoreChange>,
        now_ms: u128,
    ) -> Result<ProjectWorkspaceMutationReceipt, String> {
        self.require_identity(identity)?;
        let revision_before = self.revision;
        if changes.is_empty() {
            return Ok(self.mutation_receipt(revision_before, Vec::new(), None, None));
        }
        let mut seen = HashSet::with_capacity(changes.len());
        let accepted_paths = self
            .accepted_disk
            .manifest
            .files
            .iter()
            .map(|entry| entry.relative_path.as_str())
            .collect::<HashSet<_>>();
        let mut next_resources = self.binary_resources.clone();
        let mut next_deleted = self.deleted_binary_resources.clone();
        let mut transitions = Vec::new();

        for change in changes {
            let normalized = normalize_project_relative_path(&change.relative_path)?;
            if normalized != change.relative_path || !seen.insert(normalized.clone()) {
                return Err(format!(
                    "ProjectWorkspace Restore a refuzat path-ul binar necanonic sau duplicat: {}.",
                    change.relative_path
                ));
            }
            if self.documents.files.contains_key(&normalized) {
                return Err(format!(
                    "ProjectWorkspace Restore a refuzat suprapunerea text/binar pentru {normalized}."
                ));
            }
            let accepted_exists = accepted_paths.contains(normalized.as_str());
            if accepted_exists != change.before.is_some() {
                return Err(format!(
                    "ProjectWorkspace Restore nu poate demonstra baseline-ul binar pentru {normalized}."
                ));
            }
            for bytes in [change.before.as_ref(), change.after.as_ref()]
                .into_iter()
                .flatten()
            {
                if bytes.len() as u64 > PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_BYTES {
                    return Err(format!(
                        "ProjectWorkspace Restore a refuzat {normalized}: {} bytes depășesc limita per fișier de {PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_BYTES}.",
                        bytes.len()
                    ));
                }
            }
            if change.before.as_ref().map(|bytes| hash_bytes(bytes))
                == change.after.as_ref().map(|bytes| hash_bytes(bytes))
            {
                continue;
            }
            let before = change
                .before
                .map(|bytes| WorkspaceBinaryResource::new(normalized.clone(), bytes));
            let after = change
                .after
                .map(|bytes| WorkspaceBinaryResource::new(normalized.clone(), bytes));
            match after.as_ref() {
                Some(resource) => {
                    next_resources.insert(normalized.clone(), resource.clone());
                    next_deleted.remove(&normalized);
                }
                None => {
                    next_resources.remove(&normalized);
                    if accepted_exists {
                        next_deleted.insert(normalized.clone());
                    }
                }
            }
            transitions.push(WorkspaceBinaryResourceTransition {
                relative_path: normalized,
                before,
                after,
            });
        }
        let total_bytes = next_resources
            .values()
            .try_fold(0_u64, |total, resource| {
                total.checked_add(resource.bytes.len() as u64)
            })
            .ok_or_else(|| {
                "ProjectWorkspace Restore a depășit contorul resurselor binare.".to_string()
            })?;
        if total_bytes > PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_TOTAL_BYTES {
            return Err(format!(
                "ProjectWorkspace Restore a refuzat resursele binare: {total_bytes} bytes depășesc limita totală de {PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_TOTAL_BYTES}."
            ));
        }
        if transitions.is_empty() {
            return Ok(self.mutation_receipt(revision_before, Vec::new(), None, None));
        }
        let entry = new_history_entry(
            transaction_id(&metadata, revision_before),
            normalized_label(&metadata.label),
            normalized_source(&metadata.source),
            normalized_optional(metadata.coalesce_key),
            now_ms,
            Vec::new(),
            Vec::new(),
            transitions,
            Vec::new(),
        );
        let entry_snapshot = entry.snapshot();
        self.binary_resources = next_resources;
        self.deleted_binary_resources = next_deleted;
        self.commit_mutation(entry)?;
        Ok(self.mutation_receipt(revision_before, Vec::new(), None, Some(entry_snapshot)))
    }

    pub(crate) fn stage_version_tree_restore(
        &mut self,
        identity: &ProjectWorkspaceIdentity,
        metadata: WorkspaceMutationMetadata,
        text_changes: Vec<WorkspaceResourceMutation>,
        text_deletes: Vec<WorkspaceResourceDelete>,
        binary_changes: Vec<WorkspaceBinaryRestoreChange>,
        now_ms: u128,
    ) -> Result<ProjectWorkspaceMutationReceipt, String> {
        self.require_identity(identity)?;
        let revision_before = self.revision;
        let history_start = self.history.undo_len();
        let transaction_id = transaction_id(&metadata, revision_before);
        let label = normalized_label(&metadata.label);
        let source = normalized_source(&metadata.source);
        let mut candidate = self.clone();

        candidate.stage_resource_changes(
            identity,
            metadata.clone(),
            text_changes,
            text_deletes,
            now_ms,
        )?;
        let binary_identity = ProjectWorkspaceIdentity {
            expected_project_root: candidate.session.project_root.clone(),
            expected_session_id: candidate.runtime_session_id(),
            expected_revision: candidate.revision,
        };
        candidate.stage_binary_restore_changes(
            &binary_identity,
            metadata,
            binary_changes,
            now_ms,
        )?;
        if candidate.revision == revision_before {
            return Ok(self.mutation_receipt(revision_before, Vec::new(), None, None));
        }
        let entry = candidate
            .history
            .merge_recorded_since(history_start, transaction_id, label, source, now_ms)?
            .ok_or_else(|| {
                "ProjectWorkspace Restore a schimbat revizia fără o intrare History.".to_string()
            })?;
        candidate.revision = revision_before
            .checked_add(1)
            .ok_or_else(|| "ProjectWorkspace Restore a atins limita reviziei u64.".to_string())?;
        candidate.invalidate_derived_state();
        let entry_snapshot = entry.snapshot();
        *self = candidate;
        Ok(self.mutation_receipt(revision_before, Vec::new(), None, Some(entry_snapshot)))
    }

    pub fn stage_resource_changes(
        &mut self,
        identity: &ProjectWorkspaceIdentity,
        metadata: WorkspaceMutationMetadata,
        mutations: Vec<WorkspaceResourceMutation>,
        deletes: Vec<WorkspaceResourceDelete>,
        now_ms: u128,
    ) -> Result<ProjectWorkspaceMutationReceipt, String> {
        self.require_identity(identity)?;
        let revision_before = self.revision;
        if mutations.is_empty() && deletes.is_empty() {
            return Ok(self.mutation_receipt(revision_before, Vec::new(), None, None));
        }
        require_distinct_resource_changes(&mutations, &deletes)?;
        for path in mutations
            .iter()
            .map(|mutation| mutation.relative_path.as_str())
            .chain(deletes.iter().map(|delete| delete.relative_path.as_str()))
        {
            self.require_editable_source_path(path)?;
        }

        let mut next_documents = self.documents.clone();
        let mut transitions = Vec::with_capacity(mutations.len() + deletes.len());
        let mut files = Vec::with_capacity(mutations.len());
        for mutation in mutations {
            let before = next_documents.files.get(&mutation.relative_path).cloned();
            if mutation.create_only
                && (before.is_some()
                    || self
                        .accepted_documents
                        .contains_key(&mutation.relative_path))
            {
                return Err(format!(
                    "ProjectWorkspace a refuzat create-only pentru {}: resursa există în starea curentă sau în baseline-ul acceptat.",
                    mutation.relative_path
                ));
            }
            let file = match next_documents.text_snapshot(&mutation.relative_path) {
                Some(current) => {
                    if current.text == mutation.contents {
                        next_documents
                            .files
                            .get(&mutation.relative_path)
                            .expect("resource snapshot checked above")
                            .snapshot()
                    } else {
                        next_documents.set_draft_if_current(
                            &mutation.relative_path,
                            mutation.contents,
                            &FileBufferMutationExpectation {
                                expected_revision: current.revision,
                                expected_hash: current.hash,
                            },
                            now_ms,
                        )?
                    }
                }
                None => {
                    if let Some(accepted) = self.accepted_documents.get(&mutation.relative_path) {
                        next_documents
                            .files
                            .insert(mutation.relative_path.clone(), accepted.clone());
                    }
                    if let Some(current) = next_documents.text_snapshot(&mutation.relative_path) {
                        next_documents.set_draft_if_current(
                            &mutation.relative_path,
                            mutation.contents,
                            &FileBufferMutationExpectation {
                                expected_revision: current.revision,
                                expected_hash: current.hash,
                            },
                            now_ms,
                        )?
                    } else {
                        let file = next_documents.stage_new_text_file(
                            &mutation.relative_path,
                            mutation.contents,
                            now_ms,
                        )?;
                        file
                    }
                }
            };
            let after = next_documents.files.get(&mutation.relative_path).cloned();
            if before.as_ref().map(|entry| entry.current_hash())
                != after.as_ref().map(|entry| entry.current_hash())
            {
                transitions.push(WorkspaceResourceTransition {
                    relative_path: mutation.relative_path,
                    before,
                    after,
                });
            }
            files.push(file);
        }
        for delete in deletes {
            let before = next_documents
                .files
                .remove(&delete.relative_path)
                .ok_or_else(|| {
                    format!(
                        "ProjectWorkspace nu poate șterge {}: resursa nu există în starea curentă.",
                        delete.relative_path
                    )
                })?;
            transitions.push(WorkspaceResourceTransition {
                relative_path: delete.relative_path,
                before: Some(before),
                after: None,
            });
        }

        if transitions.is_empty() {
            return Ok(self.mutation_receipt(revision_before, files, None, None));
        }
        let entry = new_history_entry(
            transaction_id(&metadata, revision_before),
            normalized_label(&metadata.label),
            normalized_source(&metadata.source),
            normalized_optional(metadata.coalesce_key),
            now_ms,
            Vec::new(),
            transitions,
            Vec::new(),
            Vec::new(),
        );
        let entry_snapshot = entry.snapshot();
        self.documents = next_documents;
        self.commit_mutation(entry)?;
        Ok(self.mutation_receipt(revision_before, files, None, Some(entry_snapshot)))
    }

    pub fn stage_composite_changes(
        &mut self,
        identity: &ProjectWorkspaceIdentity,
        metadata: WorkspaceMutationMetadata,
        mutations: Vec<WorkspaceResourceMutation>,
        deletes: Vec<WorkspaceResourceDelete>,
        page_js: Option<PageJsDraftStageInput>,
        now_ms: u128,
    ) -> Result<ProjectWorkspaceMutationReceipt, String> {
        self.require_identity(identity)?;
        let revision_before = self.revision;
        let history_start = self.history.undo_len();
        let transaction_id = transaction_id(&metadata, revision_before);
        let label = normalized_label(&metadata.label);
        let source = normalized_source(&metadata.source);
        let mut candidate = self.clone();

        let resource_receipt = candidate.stage_resource_changes(
            identity,
            metadata.clone(),
            mutations,
            deletes,
            now_ms,
        )?;
        let page_js_receipt = if let Some(input) = page_js {
            let candidate_identity = ProjectWorkspaceIdentity {
                expected_project_root: candidate.session.project_root.clone(),
                expected_session_id: candidate.runtime_session_id(),
                expected_revision: candidate.revision,
            };
            candidate
                .stage_page_js(&candidate_identity, metadata, input, now_ms)?
                .page_js
        } else {
            None
        };

        let changed = candidate.revision != revision_before;
        if !changed {
            return Ok(self.mutation_receipt(
                revision_before,
                resource_receipt.files,
                page_js_receipt,
                None,
            ));
        }

        let entry = candidate
            .history
            .merge_recorded_since(history_start, transaction_id, label, source, now_ms)?
            .ok_or_else(|| {
                "ProjectWorkspace a schimbat revizia compusă fără o intrare History.".to_string()
            })?;
        candidate.revision = revision_before.checked_add(1).ok_or_else(|| {
            "ProjectWorkspace a atins limita reviziei u64 în mutația compusă.".to_string()
        })?;
        candidate.invalidate_derived_state();
        let entry_snapshot = entry.snapshot();
        *self = candidate;
        Ok(self.mutation_receipt(
            revision_before,
            resource_receipt.files,
            page_js_receipt,
            Some(entry_snapshot),
        ))
    }

    pub fn apply_document_changeset(
        &mut self,
        identity: &ProjectWorkspaceIdentity,
        metadata: WorkspaceMutationMetadata,
        input: crate::kernel::file_buffer_store::FileBufferChangeSetInput,
        now_ms: u128,
    ) -> Result<crate::kernel::file_buffer_store::FileBufferChangeSetResult, String> {
        self.require_identity(identity)?;
        let relative_path = input.relative_path.trim().to_string();
        let before = self
            .documents
            .text_snapshot(&relative_path)
            .ok_or_else(|| format!("ProjectWorkspace nu urmărește documentul {relative_path}."))?;
        let mut next_documents = self.documents.clone();
        let result = next_documents.apply_changeset(input, now_ms)?;
        if !result.applied {
            return Ok(result);
        }
        let after = next_documents
            .text_for(&relative_path)
            .ok_or_else(|| format!("ProjectWorkspace a pierdut documentul {relative_path}."))?;
        let entry = new_history_entry(
            transaction_id(&metadata, self.revision),
            normalized_label(&metadata.label),
            normalized_source(&metadata.source),
            normalized_optional(metadata.coalesce_key),
            now_ms,
            vec![WorkspaceDocumentTransition {
                relative_path,
                before: before.text,
                after,
            }],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        self.documents = next_documents;
        self.commit_mutation(entry)?;
        Ok(result)
    }

    pub fn stage_page_js(
        &mut self,
        identity: &ProjectWorkspaceIdentity,
        metadata: WorkspaceMutationMetadata,
        input: PageJsDraftStageInput,
        now_ms: u128,
    ) -> Result<ProjectWorkspaceMutationReceipt, String> {
        self.require_identity(identity)?;
        let revision_before = self.revision;
        let requested_base = input.base_config.clone();
        let requested_cachebust_assets = input.cachebust_assets;
        let mut next_page_js = self.page_js.clone();
        let receipt = next_page_js.stage(input)?;
        let template_path = receipt.template_path.clone();
        let accepted = self
            .accepted_page_js
            .get(&template_path)
            .cloned()
            .unwrap_or_else(|| requested_base.clone());
        if accepted != requested_base {
            return Err(format!(
                "ProjectWorkspace a refuzat draftul Page JS stale pentru {template_path}: baseConfig nu corespunde baseline-ului acceptat."
            ));
        }
        if let Some(existing) = self.page_js.drafts.get(&template_path) {
            if existing.base != accepted {
                return Err(format!(
                    "ProjectWorkspace a detectat un invariant Page JS invalid pentru {template_path}: draftul activ are alt baseline."
                ));
            }
        }
        let before = self
            .page_js
            .drafts
            .get(&template_path)
            .map(|entry| entry.current.clone())
            .unwrap_or_else(|| accepted.clone());
        let before_cachebust_assets = self
            .page_js
            .drafts
            .get(&template_path)
            .map(|entry| entry.cachebust_assets)
            .unwrap_or(requested_cachebust_assets);
        let after = next_page_js
            .drafts
            .get(&template_path)
            .map(|entry| entry.current.clone())
            .unwrap_or_else(|| accepted.clone());
        let after_cachebust_assets = next_page_js
            .drafts
            .get(&template_path)
            .map(|entry| entry.cachebust_assets)
            .unwrap_or(requested_cachebust_assets);
        if !receipt.changed
            || (before == after && before_cachebust_assets == after_cachebust_assets)
        {
            return Ok(self.mutation_receipt(revision_before, Vec::new(), Some(receipt), None));
        }
        let transition = WorkspacePageJsTransition {
            template_path,
            before,
            after,
            before_cachebust_assets,
            after_cachebust_assets,
        };
        let accepted_template_path = transition.template_path.clone();
        let entry = new_history_entry(
            transaction_id(&metadata, revision_before),
            normalized_label(&metadata.label),
            normalized_source(&metadata.source),
            normalized_optional(metadata.coalesce_key),
            now_ms,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![transition],
        );
        let entry_snapshot = entry.snapshot();
        self.accepted_page_js
            .entry(accepted_template_path)
            .or_insert(accepted);
        self.page_js = next_page_js;
        self.commit_mutation(entry)?;
        Ok(self.mutation_receipt(
            revision_before,
            Vec::new(),
            Some(receipt),
            Some(entry_snapshot),
        ))
    }

    pub fn clear_page_js(
        &mut self,
        identity: &ProjectWorkspaceIdentity,
        metadata: WorkspaceMutationMetadata,
        template_path: &str,
        expected_entry_revision: Option<u64>,
        now_ms: u128,
    ) -> Result<ProjectWorkspaceMutationReceipt, String> {
        self.require_identity(identity)?;
        let revision_before = self.revision;
        let mut next_page_js = self.page_js.clone();
        let receipt = next_page_js.clear(template_path, expected_entry_revision)?;
        let template_path = receipt.template_path.clone();
        let Some(accepted) = self
            .accepted_page_js
            .get(&template_path)
            .cloned()
            .or_else(|| {
                self.page_js
                    .drafts
                    .get(&template_path)
                    .map(|entry| entry.base.clone())
            })
        else {
            return Ok(self.mutation_receipt(revision_before, Vec::new(), Some(receipt), None));
        };
        let before_entry = self.page_js.drafts.get(&template_path);
        let before = before_entry
            .map(|entry| entry.current.clone())
            .unwrap_or_else(|| accepted.clone());
        let cachebust_assets = before_entry
            .map(|entry| entry.cachebust_assets)
            .unwrap_or(false);
        if !receipt.changed || before == accepted {
            return Ok(self.mutation_receipt(revision_before, Vec::new(), Some(receipt), None));
        }
        let transition = WorkspacePageJsTransition {
            template_path,
            before,
            after: accepted,
            before_cachebust_assets: cachebust_assets,
            after_cachebust_assets: cachebust_assets,
        };
        let entry = new_history_entry(
            transaction_id(&metadata, revision_before),
            normalized_label(&metadata.label),
            normalized_source(&metadata.source),
            normalized_optional(metadata.coalesce_key),
            now_ms,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![transition],
        );
        let entry_snapshot = entry.snapshot();
        self.page_js = next_page_js;
        self.commit_mutation(entry)?;
        Ok(self.mutation_receipt(
            revision_before,
            Vec::new(),
            Some(receipt),
            Some(entry_snapshot),
        ))
    }

    pub fn undo(
        &mut self,
        identity: &ProjectWorkspaceIdentity,
        now_ms: u128,
    ) -> Result<WorkspaceUndoRedoReceipt, String> {
        self.require_identity(identity)?;
        let revision_before = self.revision;
        let mut next_documents = self.documents.clone();
        let mut next_binary_resources = self.binary_resources.clone();
        let mut next_deleted_binary_resources = self.deleted_binary_resources.clone();
        let mut next_page_js = self.page_js.clone();
        let mut next_history = self.history.clone();
        let entry = next_history.pop_undo()?;
        apply_history_entry(
            &mut next_documents,
            &self.accepted_documents,
            &mut next_binary_resources,
            &mut next_deleted_binary_resources,
            &self.accepted_binary_resource_hashes,
            &self.accepted_disk,
            &mut next_page_js,
            &self.accepted_page_js,
            &entry,
            WorkspaceHistoryDirection::Undo,
            now_ms,
        )?;
        next_history.complete_undo(entry.clone());
        self.documents = next_documents;
        self.binary_resources = next_binary_resources;
        self.deleted_binary_resources = next_deleted_binary_resources;
        self.page_js = next_page_js;
        self.history = next_history;
        self.advance_revision()?;
        self.invalidate_derived_state();
        Ok(self.undo_redo_receipt(WorkspaceHistoryDirection::Undo, revision_before, entry))
    }

    pub fn require_history_target(
        &self,
        direction: WorkspaceHistoryDirection,
        expected_transaction_id: &str,
    ) -> Result<(), String> {
        let expected = expected_transaction_id.trim();
        if expected.is_empty() {
            return Err(
                "ProjectWorkspace History cere un transactionId explicit pentru Undo/Redo."
                    .to_string(),
            );
        }
        let actual = match direction {
            WorkspaceHistoryDirection::Undo => self.history.next_undo_transaction_id(),
            WorkspaceHistoryDirection::Redo => self.history.next_redo_transaction_id(),
        }
        .ok_or_else(|| {
            format!(
                "ProjectWorkspace nu mai are o mutație disponibilă pentru {:?}.",
                direction
            )
        })?;
        if actual != expected {
            return Err(format!(
                "ProjectWorkspace History a refuzat ținta stale: așteptat {expected}, disponibil {actual}."
            ));
        }
        Ok(())
    }

    pub fn redo(
        &mut self,
        identity: &ProjectWorkspaceIdentity,
        now_ms: u128,
    ) -> Result<WorkspaceUndoRedoReceipt, String> {
        self.require_identity(identity)?;
        let revision_before = self.revision;
        let mut next_documents = self.documents.clone();
        let mut next_binary_resources = self.binary_resources.clone();
        let mut next_deleted_binary_resources = self.deleted_binary_resources.clone();
        let mut next_page_js = self.page_js.clone();
        let mut next_history = self.history.clone();
        let entry = next_history.pop_redo()?;
        apply_history_entry(
            &mut next_documents,
            &self.accepted_documents,
            &mut next_binary_resources,
            &mut next_deleted_binary_resources,
            &self.accepted_binary_resource_hashes,
            &self.accepted_disk,
            &mut next_page_js,
            &self.accepted_page_js,
            &entry,
            WorkspaceHistoryDirection::Redo,
            now_ms,
        )?;
        next_history.complete_redo(entry.clone());
        self.documents = next_documents;
        self.binary_resources = next_binary_resources;
        self.deleted_binary_resources = next_deleted_binary_resources;
        self.page_js = next_page_js;
        self.history = next_history;
        self.advance_revision()?;
        self.invalidate_derived_state();
        Ok(self.undo_redo_receipt(WorkspaceHistoryDirection::Redo, revision_before, entry))
    }

    pub fn created_document_paths(&self) -> Vec<String> {
        self.documents
            .files
            .keys()
            .filter(|path| !self.accepted_documents.contains_key(*path))
            .cloned()
            .collect()
    }

    pub fn deleted_document_paths(&self) -> Vec<String> {
        self.accepted_documents
            .keys()
            .filter(|path| !self.documents.files.contains_key(*path))
            .cloned()
            .collect()
    }

    pub fn accepted_page_js_config(&self, template_path: &str) -> Option<&PageJsConfig> {
        self.accepted_page_js.get(template_path)
    }

    pub(super) fn accept_saved_documents(
        &mut self,
        identity: &ProjectWorkspaceIdentity,
        documents: FileBufferStore,
        accepted_page_js: BTreeMap<String, PageJsConfig>,
        accepted_disk: AcceptedProjectDiskManifest,
    ) -> Result<(), String> {
        self.require_identity(identity)?;
        if documents.project_root != self.session.project_root
            || documents.runtime_session_id != self.runtime_session_id()
        {
            return Err(
                "ProjectWorkspace a refuzat proiecția Save din alt FileBufferStore.".to_string(),
            );
        }
        if documents.snapshot().dirty_file_count != 0 {
            return Err(
                "ProjectWorkspace a refuzat proiecția Save: FileBufferStore conține încă drafturi."
                    .to_string(),
            );
        }
        accepted_disk.require_identity(&self.runtime_session_id(), &self.session.project_root)?;
        accepted_disk.require_complete()?;
        for path in &self.deleted_binary_resources {
            self.accepted_binary_resource_hashes.remove(path);
        }
        for (path, resource) in &self.binary_resources {
            self.accepted_binary_resource_hashes
                .insert(path.clone(), hash_bytes(&resource.bytes));
        }
        self.documents = documents;
        self.accepted_documents = self.documents.files.clone();
        self.binary_resources.clear();
        self.deleted_binary_resources.clear();
        self.accepted_page_js = accepted_page_js;
        self.page_js = PageJsDraftStore::new(&self.session);
        self.accepted_disk = accepted_disk;
        self.history.break_coalescing_group();
        self.advance_revision()?;
        self.invalidate_derived_state();
        Ok(())
    }

    fn commit_mutation(&mut self, entry: WorkspaceHistoryEntry) -> Result<(), String> {
        self.advance_revision()?;
        self.history.record(entry);
        self.invalidate_derived_state();
        Ok(())
    }

    fn advance_revision(&mut self) -> Result<(), String> {
        self.revision = self.revision.checked_add(1).ok_or_else(|| {
            "ProjectWorkspace a atins limita reviziei u64 și a blocat mutația fail-closed."
                .to_string()
        })?;
        Ok(())
    }

    fn invalidate_derived_state(&mut self) {
        self.project_model = None;
        self.project_model_source_revision = None;
    }

    fn require_editable_source_path(&self, relative_path: &str) -> Result<(), String> {
        let normalized = normalize_project_relative_path(relative_path)?;
        let project_root = Path::new(&self.session.project_root);
        let zola_root = Path::new(&self.session.zola_root);
        let Some(output_root) = crate::deploy::resolve_artifact_root(project_root, zola_root).ok()
        else {
            return Ok(());
        };
        if project_root.join(&normalized).starts_with(&output_root) {
            return Err(format!(
                "ProjectWorkspace a refuzat `{normalized}`: path-ul aparține output_dir Zola generat și nu este sursă editabilă."
            ));
        }
        Ok(())
    }

    pub(super) fn require_identity(
        &self,
        identity: &ProjectWorkspaceIdentity,
    ) -> Result<(), String> {
        let runtime_session_id = self.runtime_session_id();
        if identity.expected_project_root != self.session.project_root
            || identity.expected_session_id != runtime_session_id
        {
            return Err(format!(
                "ProjectWorkspace a refuzat o mutație stale din root/session {}/{}, activ {}/{}.",
                identity.expected_project_root,
                identity.expected_session_id,
                self.session.project_root,
                runtime_session_id
            ));
        }
        if identity.expected_revision != self.revision {
            return Err(format!(
                "ProjectWorkspace a refuzat o mutație stale: revizia așteptată {}, revizia curentă {}.",
                identity.expected_revision, self.revision
            ));
        }
        Ok(())
    }

    pub(crate) fn require_current_projection(
        &self,
        lease: &WorkspaceProjectionLease,
    ) -> Result<(), String> {
        if lease.project_root != self.session.project_root
            || lease.runtime_session_id != self.runtime_session_id()
            || lease.revision != self.revision
        {
            return Err(
                "ProjectWorkspace a refuzat publicarea unei proiecții construite dintr-o revizie stale."
                    .to_string(),
            );
        }
        Ok(())
    }

    fn mutation_receipt(
        &self,
        revision_before: u64,
        files: Vec<crate::kernel::file_buffer_store::FileBufferFileSnapshot>,
        page_js: Option<crate::js::PageJsDraftStageReceipt>,
        entry: Option<super::model::WorkspaceHistoryEntrySnapshot>,
    ) -> ProjectWorkspaceMutationReceipt {
        let touched_files = entry
            .as_ref()
            .map(|entry| entry.document_paths.clone())
            .unwrap_or_default();
        let transaction_id = entry.as_ref().map(|entry| entry.transaction_id.clone());
        ProjectWorkspaceMutationReceipt {
            schema_version: self.schema_version,
            changed: self.revision != revision_before,
            revision_before,
            revision_after: self.revision,
            dirty: self.is_dirty(),
            transaction_id,
            touched_files,
            entry,
            files,
            page_js,
            history: self.history.snapshot(),
        }
    }

    fn undo_redo_receipt(
        &self,
        direction: WorkspaceHistoryDirection,
        revision_before: u64,
        entry: WorkspaceHistoryEntry,
    ) -> WorkspaceUndoRedoReceipt {
        let mut document_paths = entry
            .documents
            .iter()
            .map(|transition| transition.relative_path.clone())
            .chain(
                entry
                    .resources
                    .iter()
                    .map(|transition| transition.relative_path.clone()),
            )
            .collect::<Vec<_>>();
        document_paths.sort();
        document_paths.dedup();
        let documents = document_paths
            .into_iter()
            .map(|relative_path| WorkspaceDocumentProjection {
                snapshot: self.documents.text_snapshot(&relative_path),
                relative_path,
            })
            .collect();
        WorkspaceUndoRedoReceipt {
            schema_version: self.schema_version,
            direction,
            revision_before,
            revision_after: self.revision,
            dirty: self.is_dirty(),
            entry: entry.snapshot(),
            documents,
            history: self.history.snapshot(),
        }
    }
}

fn apply_history_entry(
    documents: &mut FileBufferStore,
    accepted_documents: &BTreeMap<String, FileBufferEntry>,
    binary_resources: &mut BTreeMap<String, WorkspaceBinaryResource>,
    deleted_binary_resources: &mut std::collections::BTreeSet<String>,
    accepted_binary_resource_hashes: &BTreeMap<String, String>,
    accepted_disk: &AcceptedProjectDiskManifest,
    page_js: &mut PageJsDraftStore,
    accepted_page_js: &BTreeMap<String, PageJsConfig>,
    entry: &WorkspaceHistoryEntry,
    direction: WorkspaceHistoryDirection,
    now_ms: u128,
) -> Result<(), String> {
    for transition in &entry.documents {
        let contents = match direction {
            WorkspaceHistoryDirection::Undo => &transition.before,
            WorkspaceHistoryDirection::Redo => &transition.after,
        };
        let current = documents
            .text_snapshot(&transition.relative_path)
            .ok_or_else(|| {
                format!(
                    "ProjectWorkspace History nu mai poate ancora documentul {}.",
                    transition.relative_path
                )
            })?;
        documents.set_draft_if_current(
            &transition.relative_path,
            contents.clone(),
            &FileBufferMutationExpectation {
                expected_revision: current.revision,
                expected_hash: current.hash,
            },
            now_ms,
        )?;
    }
    for transition in &entry.resources {
        let target = match direction {
            WorkspaceHistoryDirection::Undo => &transition.before,
            WorkspaceHistoryDirection::Redo => &transition.after,
        };
        let next_revision = documents
            .files
            .get(&transition.relative_path)
            .map(|entry| entry.revision)
            .unwrap_or_default()
            .saturating_add(1);
        match target {
            Some(target) => {
                restore_resource_entry(
                    documents,
                    accepted_documents.get(&transition.relative_path),
                    target,
                    next_revision,
                    now_ms,
                )?;
            }
            None => {
                documents.files.remove(&transition.relative_path);
            }
        }
    }
    for transition in &entry.binary_resources {
        let target = match direction {
            WorkspaceHistoryDirection::Undo => &transition.before,
            WorkspaceHistoryDirection::Redo => &transition.after,
        };
        match target {
            Some(resource) => {
                if resource.relative_path != transition.relative_path {
                    return Err(format!(
                        "ProjectWorkspace History are o resursă binară incoerentă pentru {}.",
                        transition.relative_path
                    ));
                }
                if accepted_binary_resource_hashes.get(&transition.relative_path)
                    == Some(&hash_bytes(&resource.bytes))
                {
                    binary_resources.remove(&transition.relative_path);
                } else {
                    binary_resources.insert(transition.relative_path.clone(), resource.clone());
                }
                deleted_binary_resources.remove(&transition.relative_path);
            }
            None => {
                binary_resources.remove(&transition.relative_path);
                if accepted_binary_resource_hashes.contains_key(&transition.relative_path)
                    || accepted_disk
                        .manifest
                        .files
                        .iter()
                        .any(|entry| entry.relative_path == transition.relative_path)
                {
                    deleted_binary_resources.insert(transition.relative_path.clone());
                } else {
                    deleted_binary_resources.remove(&transition.relative_path);
                }
            }
        }
    }
    for transition in &entry.page_js {
        let (target, cachebust_assets) = match direction {
            WorkspaceHistoryDirection::Undo => {
                (&transition.before, transition.before_cachebust_assets)
            }
            WorkspaceHistoryDirection::Redo => {
                (&transition.after, transition.after_cachebust_assets)
            }
        };
        let accepted = accepted_page_js
            .get(&transition.template_path)
            .ok_or_else(|| {
                format!(
                    "ProjectWorkspace History nu mai are baseline Page JS pentru {}.",
                    transition.template_path
                )
            })?;
        restore_page_js_config(
            page_js,
            &transition.template_path,
            accepted,
            target,
            cachebust_assets,
        )?;
    }
    Ok(())
}

fn restore_resource_entry(
    documents: &mut FileBufferStore,
    accepted: Option<&FileBufferEntry>,
    target: &FileBufferEntry,
    next_revision: u64,
    now_ms: u128,
) -> Result<(), String> {
    let relative_path = &target.relative_path;
    let target_text = target.current_text().to_string();
    let mut restored = if let Some(accepted) = accepted {
        let mut restored = accepted.clone();
        restored.revision = restored.revision.max(next_revision);
        if target_text == restored.baseline_text {
            restored.draft = None;
        } else {
            restored.draft = Some(FileBufferDraft {
                hash: hash_text(&target_text),
                bytes: target_text.len() as u64,
                text: target_text,
                updated_at_ms: now_ms,
            });
        }
        restored
    } else {
        documents.files.remove(relative_path);
        documents.stage_new_text_file(relative_path, target_text, now_ms)?;
        documents
            .files
            .remove(relative_path)
            .expect("resource staged immediately above")
    };
    restored.revision = restored.revision.max(next_revision);
    documents.files.insert(relative_path.clone(), restored);
    Ok(())
}

fn restore_page_js_config(
    store: &mut PageJsDraftStore,
    template_path: &str,
    accepted: &PageJsConfig,
    target: &PageJsConfig,
    cachebust_assets: bool,
) -> Result<(), String> {
    if target == accepted {
        store.clear(template_path, None)?;
    } else {
        store.stage(PageJsDraftStageInput {
            template_path: template_path.to_string(),
            expected_project_root: store.project_root.clone(),
            expected_session_id: store.runtime_session_id.clone(),
            base_config: accepted.clone(),
            current_config: target.clone(),
            cachebust_assets,
            source: Some("workspace.history".to_string()),
            coalesce_key: None,
            transaction_id: None,
        })?;
    }
    Ok(())
}

fn require_distinct_document_paths(mutations: &[WorkspaceDocumentMutation]) -> Result<(), String> {
    let mut paths = HashSet::with_capacity(mutations.len());
    for mutation in mutations {
        let path = mutation.relative_path.trim();
        if path.is_empty() || path.contains('\0') {
            return Err("ProjectWorkspace a refuzat un path de document invalid.".to_string());
        }
        if !paths.insert(path) {
            return Err(format!(
                "ProjectWorkspace a refuzat tranzacția: documentul {path} apare de mai multe ori."
            ));
        }
    }
    Ok(())
}

fn require_distinct_resource_changes(
    mutations: &[WorkspaceResourceMutation],
    deletes: &[WorkspaceResourceDelete],
) -> Result<(), String> {
    let mut paths = HashSet::with_capacity(mutations.len() + deletes.len());
    for mutation in mutations {
        let path = mutation.relative_path.trim();
        if path.is_empty() || path.contains('\0') {
            return Err("ProjectWorkspace a refuzat un path de resursă invalid.".to_string());
        }
        if !paths.insert(path) {
            return Err(format!(
                "ProjectWorkspace a refuzat tranzacția: resursa {path} apare de mai multe ori."
            ));
        }
    }
    for delete in deletes {
        let path = delete.relative_path.trim();
        if path.is_empty() || path.contains('\0') {
            return Err("ProjectWorkspace a refuzat un path de ștergere invalid.".to_string());
        }
        if !paths.insert(path) {
            return Err(format!(
                "ProjectWorkspace a refuzat tranzacția: resursa {path} apare de mai multe ori."
            ));
        }
    }
    Ok(())
}

fn normalized_label(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        "Editare workspace".to_string()
    } else {
        value.chars().take(160).collect()
    }
}

fn normalized_source(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        "workspace.unknown".to_string()
    } else {
        value.chars().take(120).collect()
    }
}

fn normalized_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.chars().take(160).collect())
    })
}

fn transaction_id(metadata: &WorkspaceMutationMetadata, revision_before: u64) -> String {
    normalized_optional(metadata.transaction_id.clone()).unwrap_or_else(|| {
        format!(
            "workspace-{}-{}",
            revision_before.saturating_add(1),
            crate::kernel::observability::now_ms()
        )
    })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use crate::{
        js::{PageJsConfig, PanaComponent},
        kernel::{
            file_buffer_store::{
                hash_text, FileBufferBaseline, FileBufferEntry, FileBufferStoreLimits,
                TextBufferLanguage, TextBufferRole,
            },
            project_session::{ProjectRootFingerprint, ProjectSessionScanSummary},
        },
        project::ProjectDiskManifest,
    };

    use super::*;

    #[test]
    fn document_mutation_and_undo_redo_never_touch_disk() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("templates")).unwrap();
        let path = root.join("templates/index.html");
        fs::write(&path, "<h1>Disk</h1>").unwrap();
        let mut workspace = workspace(&root, &[("templates/index.html", "<h1>Disk</h1>")]);

        let receipt = workspace
            .stage_document_texts(
                &identity(&workspace),
                metadata("Text titlu", Some("inspector.content")),
                vec![WorkspaceDocumentMutation {
                    relative_path: "templates/index.html".to_string(),
                    contents: "<h1>Draft</h1>".to_string(),
                }],
                10,
            )
            .unwrap();

        assert!(receipt.changed);
        assert!(receipt.dirty);
        assert!(receipt.entry.as_ref().unwrap().topology_paths.is_empty());
        assert_eq!(fs::read_to_string(&path).unwrap(), "<h1>Disk</h1>");
        let target_transaction_id = receipt.entry.unwrap().transaction_id;
        assert!(workspace
            .require_history_target(WorkspaceHistoryDirection::Undo, "wrong-transaction")
            .is_err());
        workspace
            .require_history_target(WorkspaceHistoryDirection::Undo, &target_transaction_id)
            .unwrap();
        let undo = workspace.undo(&identity(&workspace), 11).unwrap();
        assert_eq!(undo.entry.transaction_id, target_transaction_id);
        assert_eq!(undo.documents.len(), 1);
        assert_eq!(
            undo.documents[0]
                .snapshot
                .as_ref()
                .map(|snapshot| snapshot.text.as_str()),
            Some("<h1>Disk</h1>")
        );
        assert_eq!(
            workspace.documents.text_for("templates/index.html"),
            Some("<h1>Disk</h1>".to_string())
        );
        assert!(!workspace.is_dirty());
        assert_eq!(fs::read_to_string(&path).unwrap(), "<h1>Disk</h1>");
        let redo = workspace.redo(&identity(&workspace), 12).unwrap();
        assert_eq!(redo.entry.transaction_id, target_transaction_id);
        assert_eq!(redo.documents.len(), 1);
        assert_eq!(
            redo.documents[0]
                .snapshot
                .as_ref()
                .map(|snapshot| snapshot.text.as_str()),
            Some("<h1>Draft</h1>")
        );
        assert_eq!(
            workspace.documents.text_for("templates/index.html"),
            Some("<h1>Draft</h1>".to_string())
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "<h1>Disk</h1>");
        let resource_edit = workspace
            .stage_resource_texts(
                &identity(&workspace),
                metadata("Existing template resource edit", None),
                vec![WorkspaceResourceMutation {
                    relative_path: "templates/index.html".into(),
                    contents: "<h1>Draft 2</h1>".into(),
                    create_only: false,
                }],
                13,
            )
            .unwrap();
        assert!(resource_edit.entry.unwrap().topology_paths.is_empty());
        assert_eq!(fs::read_to_string(&path).unwrap(), "<h1>Disk</h1>");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stale_revision_and_partial_multi_document_mutations_are_fail_closed() {
        let root = unique_test_dir();
        let mut workspace = workspace(
            &root,
            &[("templates/a.html", "a"), ("templates/b.html", "b")],
        );
        let stale = ProjectWorkspaceIdentity {
            expected_revision: 8,
            ..identity(&workspace)
        };
        assert!(workspace
            .stage_document_texts(
                &stale,
                metadata("stale", None),
                vec![WorkspaceDocumentMutation {
                    relative_path: "templates/a.html".to_string(),
                    contents: "changed".to_string(),
                }],
                10,
            )
            .is_err());
        assert_eq!(
            workspace.documents.text_for("templates/a.html"),
            Some("a".into())
        );

        assert!(workspace
            .stage_document_texts(
                &identity(&workspace),
                metadata("atomic", None),
                vec![
                    WorkspaceDocumentMutation {
                        relative_path: "templates/a.html".to_string(),
                        contents: "changed".to_string(),
                    },
                    WorkspaceDocumentMutation {
                        relative_path: "templates/missing.html".to_string(),
                        contents: "missing".to_string(),
                    },
                ],
                11,
            )
            .is_err());
        assert_eq!(workspace.revision, 0);
        assert_eq!(
            workspace.documents.text_for("templates/a.html"),
            Some("a".into())
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn page_js_is_part_of_the_same_revision_and_history() {
        let root = unique_test_dir();
        let mut workspace = workspace(&root, &[("templates/index.html", "<main></main>")]);
        let session_id = workspace.runtime_session_id();
        let project_root = workspace.session.project_root.clone();
        let staged = workspace
            .stage_page_js(
                &identity(&workspace),
                metadata("Tabs", Some("js.components")),
                PageJsDraftStageInput {
                    template_path: "templates/index.html".to_string(),
                    expected_project_root: project_root,
                    expected_session_id: session_id,
                    base_config: PageJsConfig::default(),
                    current_config: PageJsConfig {
                        version: Some(1),
                        components: vec![PanaComponent { id: "tabs".into() }],
                        motion: None,
                    },
                    cachebust_assets: false,
                    source: Some("inspector.js".into()),
                    coalesce_key: Some("js.components".into()),
                    transaction_id: Some("js-1".into()),
                },
                20,
            )
            .unwrap();
        assert_eq!(staged.revision_after, 1);
        assert_eq!(workspace.page_js.dirty_count(), 1);

        workspace.undo(&identity(&workspace), 21).unwrap();
        assert_eq!(workspace.page_js.dirty_count(), 0);
        workspace.redo(&identity(&workspace), 22).unwrap();
        assert_eq!(workspace.page_js.dirty_count(), 1);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn projection_publish_requires_the_exact_workspace_revision() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(root.join("zola.toml"), "base_url = 'http://example.test'\n").unwrap();
        fs::write(root.join("templates/index.html"), "<main></main>").unwrap();
        let mut workspace = workspace(&root, &[("templates/index.html", "<main></main>")]);
        let lease = workspace.capture_projection_lease().unwrap();
        workspace
            .stage_document_texts(
                &identity(&workspace),
                metadata("edit", None),
                vec![WorkspaceDocumentMutation {
                    relative_path: "templates/index.html".into(),
                    contents: "<main>draft</main>".into(),
                }],
                1,
            )
            .unwrap();
        let model =
            crate::project_model::build_project_model_from_workspace_projection(&root, &lease)
                .unwrap();
        assert!(workspace.publish_project_model(&lease, model).is_err());
        assert!(workspace.project_model.is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn projection_lease_contains_the_complete_text_revision_and_exact_change_set() {
        let root = unique_test_dir();
        let mut workspace = workspace(
            &root,
            &[
                ("zola.toml", "base_url = '/'\n"),
                ("templates/index.html", "<main>Baseline</main>"),
            ],
        );
        let receipt = workspace
            .stage_document_texts(
                &identity(&workspace),
                metadata("edit", None),
                vec![WorkspaceDocumentMutation {
                    relative_path: "templates/index.html".into(),
                    contents: "<main>Draft</main>".into(),
                }],
                1,
            )
            .unwrap();

        let lease = workspace.capture_projection_lease().unwrap();
        assert_eq!(
            lease.workspace_transaction_id.as_deref(),
            receipt.transaction_id.as_deref()
        );
        assert_eq!(lease.source_texts.len(), 2);
        assert_eq!(
            lease.source_texts.get("zola.toml").map(String::as_str),
            Some("base_url = '/'\n")
        );
        assert_eq!(
            lease
                .source_texts
                .get("templates/index.html")
                .map(String::as_str),
            Some("<main>Draft</main>")
        );
        assert_eq!(
            lease.changed_paths,
            std::collections::HashSet::from(["templates/index.html".to_string()])
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn resource_delete_is_a_tombstone_projection_and_is_fully_reversible() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(root.join("zola.toml"), "base_url = 'http://example.test'\n").unwrap();
        let path = root.join("templates/index.html");
        fs::write(&path, "<main>Disk</main>").unwrap();
        let mut workspace = workspace(&root, &[("templates/index.html", "<main>Disk</main>")]);

        workspace
            .stage_resource_changes(
                &identity(&workspace),
                metadata("Delete template", None),
                Vec::new(),
                vec![WorkspaceResourceDelete {
                    relative_path: "templates/index.html".into(),
                }],
                10,
            )
            .unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "<main>Disk</main>");
        assert_eq!(
            workspace.deleted_document_paths(),
            vec!["templates/index.html"]
        );
        assert!(workspace
            .capture_projection_lease()
            .unwrap()
            .deleted_sources
            .contains("templates/index.html"));
        workspace.undo(&identity(&workspace), 11).unwrap();
        assert_eq!(
            workspace.documents.text_for("templates/index.html"),
            Some("<main>Disk</main>".into())
        );
        assert!(!workspace.is_dirty());
        workspace.redo(&identity(&workspace), 12).unwrap();
        assert!(workspace
            .documents
            .text_for("templates/index.html")
            .is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn binary_resource_is_session_only_projected_and_reversible() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("static/fonturi/inter")).unwrap();
        let relative_path = "static/fonturi/inter/inter-regular.woff2";
        let disk_path = root.join(relative_path);
        let bytes = vec![0x77, 0x4f, 0x46, 0x32, 1, 2, 3, 4];
        let mut workspace = workspace(&root, &[]);

        let receipt = workspace
            .stage_binary_resource_creates(
                &identity(&workspace),
                metadata("Download font", None),
                vec![WorkspaceBinaryResource::new(relative_path, bytes.clone())],
                30,
            )
            .unwrap();

        assert!(receipt.changed);
        assert!(receipt.dirty);
        assert_eq!(receipt.touched_files, vec![relative_path]);
        assert!(!disk_path.exists());
        let snapshot = workspace.snapshot();
        assert_eq!(snapshot.staged_binary_resource_count, 1);
        assert_eq!(snapshot.staged_binary_resource_bytes, bytes.len() as u64);
        assert_eq!(snapshot.staged_binary_resources, vec![relative_path]);
        let projection = workspace.capture_projection_lease().unwrap();
        assert_eq!(projection.resource_bytes.get(relative_path), Some(&bytes));
        assert!(projection.changed_paths.contains(relative_path));

        workspace.undo(&identity(&workspace), 31).unwrap();
        assert!(!workspace.is_dirty());
        assert!(workspace.staged_binary_resource(relative_path).is_none());
        assert!(!disk_path.exists());

        workspace.redo(&identity(&workspace), 32).unwrap();
        assert_eq!(
            workspace.staged_binary_resource(relative_path),
            Some(bytes.as_slice())
        );
        assert!(workspace.is_dirty());
        assert!(!disk_path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn configured_output_is_not_an_editable_workspace_namespace() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::write(
            root.join("zola.toml"),
            "base_url = '/'\noutput_dir = 'generated/site'\n",
        )
        .unwrap();
        let mut workspace = workspace(&root, &[]);

        let text_error = workspace
            .stage_resource_texts(
                &identity(&workspace),
                metadata("generated text", None),
                vec![WorkspaceResourceMutation {
                    relative_path: "generated/site/index.html".to_string(),
                    contents: "generated".to_string(),
                    create_only: true,
                }],
                35,
            )
            .unwrap_err();
        assert!(text_error.contains("nu este sursă editabilă"));

        let binary_error = workspace
            .stage_binary_resource_creates(
                &identity(&workspace),
                metadata("generated binary", None),
                vec![WorkspaceBinaryResource::new(
                    "generated/site/image.webp",
                    vec![1, 2, 3],
                )],
                36,
            )
            .unwrap_err();
        assert!(binary_error.contains("nu este sursă editabilă"));
        assert!(!workspace.is_dirty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn binary_create_is_fail_closed_against_text_and_accepted_disk_namespaces() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("static")).unwrap();
        fs::write(root.join("static/existing.bin"), b"disk").unwrap();
        let mut workspace = workspace(&root, &[("static/text.json", "{\"value\":true}")]);
        workspace.accepted_disk = AcceptedProjectDiskManifest::new(
            workspace.runtime_session_id(),
            workspace.session.project_root.clone(),
            crate::project::read_project_disk_manifest(&root).unwrap(),
        )
        .unwrap();

        let text_error = workspace
            .stage_binary_resource_creates(
                &identity(&workspace),
                metadata("collision", None),
                vec![WorkspaceBinaryResource::new("static/text.json", vec![1])],
                40,
            )
            .unwrap_err();
        assert!(text_error.contains("namespace-ul text"));

        let disk_error = workspace
            .stage_binary_resource_creates(
                &identity(&workspace),
                metadata("collision", None),
                vec![WorkspaceBinaryResource::new("static/existing.bin", vec![1])],
                41,
            )
            .unwrap_err();
        assert!(disk_error.contains("baseline-ul disk acceptat"));
        assert_eq!(workspace.revision, 0);
        assert!(!workspace.is_dirty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn external_reconcile_preserves_or_invalidates_binary_history_by_exact_path() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("static/fonturi/inter")).unwrap();
        let relative_path = "static/fonturi/inter/inter-regular.woff2";
        let bytes = vec![0x77, 0x4f, 0x46, 0x32, 51, 52];
        let mut workspace = workspace(&root, &[]);
        workspace
            .stage_binary_resource_creates(
                &identity(&workspace),
                metadata("Download font", None),
                vec![WorkspaceBinaryResource::new(relative_path, bytes.clone())],
                50,
            )
            .unwrap();
        fs::write(root.join(relative_path), &bytes).unwrap();
        let accepted = workspace
            .accepted_disk
            .next(
                &workspace.runtime_session_id(),
                &workspace.session.project_root,
                crate::project::read_project_disk_manifest(&root).unwrap(),
            )
            .unwrap();
        let save_identity = identity(&workspace);
        let saved_documents = workspace.documents.clone();
        let accepted_page_js = workspace.accepted_page_js.clone();
        workspace
            .accept_saved_documents(&save_identity, saved_documents, accepted_page_js, accepted)
            .unwrap();
        assert_eq!(workspace.snapshot().history.undo_count, 1);

        fs::write(root.join("static/unrelated.txt"), "external").unwrap();
        let reconciled = workspace
            .accepted_disk
            .next(
                &workspace.runtime_session_id(),
                &workspace.session.project_root,
                crate::project::read_project_disk_manifest(&root).unwrap(),
            )
            .unwrap();
        let reconciled_documents = workspace.documents.clone();
        workspace
            .accept_reconciled_disk_state(reconciled, reconciled_documents, false)
            .unwrap();
        assert_eq!(workspace.snapshot().history.undo_count, 1);
        assert!(workspace
            .accepted_binary_resource_hashes
            .contains_key(relative_path));
        workspace.undo(&identity(&workspace), 51).unwrap();
        workspace.redo(&identity(&workspace), 52).unwrap();
        assert!(!workspace.is_dirty());

        fs::write(root.join(relative_path), b"externally replaced").unwrap();
        let reconciled = workspace
            .accepted_disk
            .next(
                &workspace.runtime_session_id(),
                &workspace.session.project_root,
                crate::project::read_project_disk_manifest(&root).unwrap(),
            )
            .unwrap();
        let reconciled_documents = workspace.documents.clone();
        workspace
            .accept_reconciled_disk_state(reconciled, reconciled_documents, false)
            .unwrap();
        assert_eq!(workspace.snapshot().history.undo_count, 0);
        assert!(workspace.accepted_binary_resource_hashes.is_empty());
        assert!(!workspace.is_dirty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn history_rebases_resource_existence_across_a_saved_baseline() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("templates")).unwrap();
        let mut workspace = workspace(&root, &[]);
        let create_receipt = workspace
            .stage_resource_texts(
                &identity(&workspace),
                metadata("Create template", None),
                vec![WorkspaceResourceMutation {
                    relative_path: "templates/new.html".into(),
                    contents: "<main>New</main>".into(),
                    create_only: true,
                }],
                20,
            )
            .unwrap();
        assert_eq!(
            create_receipt.entry.unwrap().topology_paths,
            vec!["templates/new.html"]
        );
        let path = root.join("templates/new.html");
        fs::write(&path, "<main>New</main>").unwrap();
        let mut saved_documents = workspace.documents.clone();
        saved_documents
            .record_saved_text("templates/new.html", "<main>New</main>".into())
            .unwrap();
        let accepted = workspace
            .accepted_disk
            .next(
                &workspace.runtime_session_id(),
                &workspace.session.project_root,
                crate::project::read_project_disk_manifest(&root).unwrap(),
            )
            .unwrap();
        workspace
            .accept_saved_documents(
                &identity(&workspace),
                saved_documents,
                workspace.accepted_page_js.clone(),
                accepted,
            )
            .unwrap();
        assert!(!workspace.is_dirty());

        let undo = workspace.undo(&identity(&workspace), 21).unwrap();
        assert_eq!(undo.entry.topology_paths, vec!["templates/new.html"]);
        assert_eq!(
            workspace.deleted_document_paths(),
            vec!["templates/new.html"]
        );
        let redo = workspace.redo(&identity(&workspace), 22).unwrap();
        assert_eq!(redo.entry.topology_paths, vec!["templates/new.html"]);
        assert_eq!(
            workspace.documents.text_for("templates/new.html"),
            Some("<main>New</main>".into())
        );
        assert!(!workspace.is_dirty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn version_restore_stages_create_update_and_delete_as_one_history_entry() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("templates")).unwrap();
        let mut workspace = workspace(
            &root,
            &[
                ("templates/index.html", "old"),
                ("templates/remove.html", "remove"),
            ],
        );
        let revision_before = workspace.revision;
        let undo_before = workspace.history.snapshot().undo_count;
        let receipt = workspace
            .stage_version_tree_restore(
                &identity(&workspace),
                WorkspaceMutationMetadata {
                    label: "Restore Git abcdef01".to_string(),
                    source: "versioning_restore".to_string(),
                    coalesce_key: None,
                    transaction_id: Some("restore-test".to_string()),
                },
                vec![
                    WorkspaceResourceMutation {
                        relative_path: "templates/index.html".to_string(),
                        contents: "restored".to_string(),
                        create_only: false,
                    },
                    WorkspaceResourceMutation {
                        relative_path: "templates/new.html".to_string(),
                        contents: "new".to_string(),
                        create_only: true,
                    },
                ],
                vec![WorkspaceResourceDelete {
                    relative_path: "templates/remove.html".to_string(),
                }],
                Vec::new(),
                40,
            )
            .unwrap();
        assert!(receipt.changed);
        assert_eq!(workspace.revision, revision_before + 1);
        assert_eq!(workspace.history.snapshot().undo_count, undo_before + 1);
        assert_eq!(
            workspace.documents.text_for("templates/index.html"),
            Some("restored".to_string())
        );
        assert_eq!(
            workspace.documents.text_for("templates/new.html"),
            Some("new".to_string())
        );
        assert_eq!(
            workspace.deleted_document_paths(),
            vec!["templates/remove.html"]
        );

        workspace.undo(&identity(&workspace), 41).unwrap();
        assert_eq!(
            workspace.documents.text_for("templates/index.html"),
            Some("old".to_string())
        );
        assert_eq!(
            workspace.documents.text_for("templates/remove.html"),
            Some("remove".to_string())
        );
        assert_eq!(workspace.documents.text_for("templates/new.html"), None);
    }

    fn workspace(root: &PathBuf, files: &[(&str, &str)]) -> ProjectWorkspace {
        let session = session(root);
        let mut documents = FileBufferStore::for_project_session(
            &session,
            1,
            FileBufferStoreLimits {
                max_files: 100,
                max_file_bytes: 1024 * 1024,
                max_total_bytes: 4 * 1024 * 1024,
            },
        );
        for (relative_path, text) in files {
            documents.insert_loaded_file(FileBufferEntry {
                relative_path: (*relative_path).to_string(),
                absolute_path: root.join(relative_path).to_string_lossy().to_string(),
                language: TextBufferLanguage::Html,
                role: TextBufferRole::Template,
                baseline: FileBufferBaseline {
                    hash: hash_text(text),
                    modified_ms: 1,
                    size: text.len() as u64,
                    readonly: false,
                },
                baseline_text: (*text).to_string(),
                draft: None,
                revision: 1,
            });
        }
        let accepted = AcceptedProjectDiskManifest::new(
            session.runtime_instance_id(),
            session.project_root.clone(),
            ProjectDiskManifest {
                root: session.project_root.clone(),
                files: Vec::new(),
                truncated: false,
                max_files: 1000,
            },
        )
        .unwrap();
        let page_js = PageJsDraftStore::new(&session);
        ProjectWorkspace::new(session, accepted, documents, page_js).unwrap()
    }

    fn identity(workspace: &ProjectWorkspace) -> ProjectWorkspaceIdentity {
        ProjectWorkspaceIdentity {
            expected_project_root: workspace.session.project_root.clone(),
            expected_session_id: workspace.runtime_session_id(),
            expected_revision: workspace.revision,
        }
    }

    fn metadata(label: &str, coalesce_key: Option<&str>) -> WorkspaceMutationMetadata {
        WorkspaceMutationMetadata {
            label: label.to_string(),
            source: "test".to_string(),
            coalesce_key: coalesce_key.map(str::to_string),
            transaction_id: None,
        }
    }

    fn session(root: &PathBuf) -> ProjectSessionSnapshot {
        ProjectSessionSnapshot {
            schema_version: 1,
            id: "workspace-test".to_string(),
            project_root: root.to_string_lossy().to_string(),
            zola_root: root.to_path_buf().to_string_lossy().to_string(),
            session_dir: root.join("session").to_string_lossy().to_string(),
            manifest_path: root.join("session.json").to_string_lossy().to_string(),
            opened_at_ms: 7,
            last_seen_at_ms: 7,
            root_fingerprint: ProjectRootFingerprint {
                canonical_path: root.to_string_lossy().to_string(),
                modified_ms: 1,
                size: 0,
                readonly: false,
                unix_device: None,
                unix_inode: None,
            },
            scan_summary: ProjectSessionScanSummary {
                is_zola: true,
                is_empty: false,
                active_theme: None,
                file_count: 0,
                directory_count: 0,
            },
        }
    }

    fn unique_test_dir() -> PathBuf {
        static NEXT_TEST_DIR_ID: AtomicU64 = AtomicU64::new(1);
        std::env::temp_dir().join(format!(
            "pana-project-workspace-{}-{}-{}",
            std::process::id(),
            crate::kernel::observability::now_ms(),
            NEXT_TEST_DIR_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }
}
