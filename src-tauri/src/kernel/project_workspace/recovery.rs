use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::Path,
    sync::Arc,
    time::Instant,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::{
    app_home::{
        project_open_recovery_decision_path, project_session_dir, project_session_manifest_path,
        project_workspace_recovery_journal_path, project_workspace_recovery_path,
    },
    js::{PageJsConfig, PageJsDraftStageInput, PageJsDraftStore},
    kernel::{
        file_buffer_store::{hash_text, now_ms, FileBufferEntry, FileBufferMutationExpectation},
        project_session::{ProjectRootFingerprint, ProjectSessionSnapshot},
        write_authority::{
            WriteAuthority, WriteCategory, WriteIntent, WriteOperationKind, WriteOwner,
            WritePolicy, WriteTarget,
        },
    },
    project::{resolve_project_write_path, ProjectDiskManifest},
    state::AppState,
};

use super::{
    history::{WorkspaceHistory, WorkspaceHistoryRecoveryPatch},
    model::{
        ProjectWorkspaceIdentity, ProjectWorkspaceSaveError, ProjectWorkspaceSaveReceipt,
        PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_BYTES,
        PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_TOTAL_BYTES,
    },
    ProjectWorkspace, WorkspaceBinaryResource,
};

// Version 5 persists opaque SourceNodeId subtree placement (parent + sibling
// index) for exact structural undo/redo. Older location-derived history must
// fail closed instead of being interpreted under the new identity contract.
// v6 stores one or more contiguous SourceGraph roots for exact structural
// Undo/Redo; older recovery payloads fail closed instead of inventing IDs.
const PROJECT_WORKSPACE_RECOVERY_SCHEMA_VERSION: u32 = 6;
const PROJECT_WORKSPACE_RECOVERY_MAX_BYTES: u64 = 192 * 1024 * 1024;
const PROJECT_WORKSPACE_RECOVERY_JOURNAL_SCHEMA_VERSION: u32 = 1;
const PROJECT_WORKSPACE_RECOVERY_JOURNAL_MAX_BYTES: u64 = 192 * 1024 * 1024;
const PROJECT_WORKSPACE_RECOVERY_JOURNAL_RECORD_MAX_BYTES: usize = 256 * 1024;
const PROJECT_WORKSPACE_RECOVERY_CHECKPOINT_INTERVAL: u64 = 32;
const PROJECT_OPEN_RECOVERY_ASSESSMENT_SCHEMA_VERSION: u32 = 1;
const PROJECT_OPEN_RECOVERY_DECISION_SCHEMA_VERSION: u32 = 1;
const PROJECT_OPEN_RECOVERY_DECISION_MAX_BYTES: u64 = 64 * 1024;
const PROJECT_SESSION_MANIFEST_MAX_BYTES: u64 = 1024 * 1024;
pub const PROJECT_WORKSPACE_MUTATED_EVENT: &str = "pana-project-workspace-mutated";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectWorkspacePreviewProjection {
    Required,
    Deferred,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ProjectWorkspaceCommitTimings {
    pub candidate_clone_ms: u64,
    pub candidate_clone_us: u64,
    pub mutation_ms: u64,
    pub mutation_us: u64,
    pub recovery_persist_ms: u64,
    pub recovery_persist_us: u64,
    pub authority_publish_ms: u64,
    pub authority_publish_us: u64,
    pub total_ms: u64,
    pub total_us: u64,
}

/// Commits a session-only ProjectWorkspace mutation as one recovery-backed
/// transaction. The live workspace is published only after the candidate
/// snapshot is durably persisted; a recovery failure therefore cannot leave
/// callers with an error while the in-memory authority has already advanced.
pub fn commit_project_workspace_session_mutation<R: Runtime, T>(
    app: &AppHandle<R>,
    live_workspace: &mut ProjectWorkspace,
    mutate: impl FnOnce(&mut ProjectWorkspace) -> Result<T, String>,
) -> Result<T, String> {
    commit_project_workspace_session_mutation_with_projection(
        app,
        live_workspace,
        ProjectWorkspacePreviewProjection::Required,
        mutate,
    )
}

pub fn commit_project_workspace_session_mutation_with_projection<R: Runtime, T>(
    app: &AppHandle<R>,
    live_workspace: &mut ProjectWorkspace,
    preview_projection: ProjectWorkspacePreviewProjection,
    mutate: impl FnOnce(&mut ProjectWorkspace) -> Result<T, String>,
) -> Result<T, String> {
    commit_project_workspace_session_mutation_with_projection_measured(
        app,
        live_workspace,
        preview_projection,
        mutate,
    )
    .map(|(result, _)| result)
}

pub fn commit_project_workspace_session_mutation_with_projection_measured<R: Runtime, T>(
    app: &AppHandle<R>,
    live_workspace: &mut ProjectWorkspace,
    preview_projection: ProjectWorkspacePreviewProjection,
    mutate: impl FnOnce(&mut ProjectWorkspace) -> Result<T, String>,
) -> Result<(T, ProjectWorkspaceCommitTimings), String> {
    let total_started = Instant::now();
    if let Some(state) = app.try_state::<AppState>() {
        state
            .ai_coordination
            .require_user_source_mutation()
            .map_err(|error| error.to_string())?;
        state
            .versioning_network_operation
            .require_source_mutation_allowed(
                "Mutația ProjectWorkspace",
                &live_workspace.session.project_root,
                &live_workspace.runtime_session_id(),
            )?;
    }
    let clone_started = Instant::now();
    let mut candidate = live_workspace.fork_candidate();
    let candidate_clone_us = elapsed_us(clone_started);
    let mutation_started = Instant::now();
    let result = mutate(&mut candidate)?;
    let mutation_us = elapsed_us(mutation_started);
    let recovery_started = Instant::now();
    persist_project_workspace_recovery_transaction(app, live_workspace, &candidate)?;
    let recovery_persist_us = elapsed_us(recovery_started);
    let publish_started = Instant::now();
    candidate.prepare_candidate_for_publish();
    *live_workspace = candidate;
    emit_project_workspace_mutated(app, live_workspace, preview_projection);
    let authority_publish_us = elapsed_us(publish_started);
    let total_us = elapsed_us(total_started);
    Ok((
        result,
        ProjectWorkspaceCommitTimings {
            candidate_clone_ms: candidate_clone_us / 1_000,
            candidate_clone_us,
            mutation_ms: mutation_us / 1_000,
            mutation_us,
            recovery_persist_ms: recovery_persist_us / 1_000,
            recovery_persist_us,
            authority_publish_ms: authority_publish_us / 1_000,
            authority_publish_us,
            total_ms: total_us / 1_000,
            total_us,
        },
    ))
}

/// Publishes a candidate that was fully planned and validated without holding
/// the live ProjectWorkspace mutex. The caller must hold that mutex only for
/// this CAS + durable recovery barrier.
#[allow(clippy::too_many_arguments)]
pub fn publish_prepared_project_workspace_candidate<R: Runtime>(
    app: &AppHandle<R>,
    live_workspace: &mut ProjectWorkspace,
    expected_base_revision: u64,
    mut candidate: ProjectWorkspace,
    preview_projection: ProjectWorkspacePreviewProjection,
    candidate_clone_ms: u64,
    mutation_ms: u64,
    total_started: Instant,
) -> Result<ProjectWorkspaceCommitTimings, String> {
    if let Some(state) = app.try_state::<AppState>() {
        state
            .versioning_network_operation
            .require_source_mutation_allowed(
                "Publicarea ProjectWorkspace",
                &live_workspace.session.project_root,
                &live_workspace.runtime_session_id(),
            )?;
    }
    if live_workspace.revision != expected_base_revision {
        return Err(format!(
            "ProjectWorkspace CAS a refuzat candidatul stale: baza {}, revizia activă {}.",
            expected_base_revision, live_workspace.revision
        ));
    }
    if live_workspace.session.project_root != candidate.session.project_root
        || live_workspace.runtime_session_id() != candidate.runtime_session_id()
        || live_workspace.accepted_disk != candidate.accepted_disk
    {
        return Err(
            "ProjectWorkspace CAS a refuzat un candidat din altă autoritate de sesiune."
                .to_string(),
        );
    }
    let recovery_started = Instant::now();
    persist_project_workspace_recovery_transaction(app, live_workspace, &candidate)?;
    let recovery_persist_us = elapsed_us(recovery_started);
    let publish_started = Instant::now();
    candidate.prepare_candidate_for_publish();
    *live_workspace = candidate;
    emit_project_workspace_mutated(app, live_workspace, preview_projection);
    let authority_publish_us = elapsed_us(publish_started);
    let total_us = elapsed_us(total_started);
    Ok(ProjectWorkspaceCommitTimings {
        candidate_clone_ms,
        candidate_clone_us: candidate_clone_ms.saturating_mul(1_000),
        mutation_ms,
        mutation_us: mutation_ms.saturating_mul(1_000),
        recovery_persist_ms: recovery_persist_us / 1_000,
        recovery_persist_us,
        authority_publish_ms: authority_publish_us / 1_000,
        authority_publish_us,
        total_ms: total_us / 1_000,
        total_us,
    })
}

fn elapsed_us(started: Instant) -> u64 {
    started.elapsed().as_micros().min(u64::MAX as u128) as u64
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectWorkspaceMutationEvent {
    project_root: String,
    runtime_session_id: String,
    workspace_revision: u64,
    dirty: bool,
    preview_projection_required: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectWorkspaceRecoveryStatus {
    Missing,
    Restored,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectOpenRecoveryStatus {
    Missing,
    Restorable,
    DecisionRequired,
    Abandoned,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectOpenRecoveryConflictReason {
    DiskBaselineChanged,
    ProjectRootReplaced,
    RecoveryInvalid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectOpenRecoveryAssessment {
    pub schema_version: u32,
    pub status: ProjectOpenRecoveryStatus,
    pub project_root: String,
    pub assessment_token: Option<String>,
    pub conflict_reason: Option<ProjectOpenRecoveryConflictReason>,
    pub root_identity_changed: Option<bool>,
    pub recovery_revision: Option<u64>,
    pub dirty_document_count: usize,
    pub staged_binary_resource_count: usize,
    pub deleted_binary_resource_count: usize,
    pub page_js_draft_count: usize,
    pub undo_count: usize,
    pub redo_count: usize,
    pub accepted_file_count: usize,
    pub current_file_count: usize,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectOpenRecoveryDecisionAction {
    Abandon,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectOpenRecoveryDecisionInput {
    pub action: ProjectOpenRecoveryDecisionAction,
    pub assessment_token: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectOpenRecoveryResolution {
    Restore,
    Skip,
    ExplicitAbandon,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectOpenRecoveryDecisionMarker {
    schema_version: u32,
    project_root: String,
    assessment_token: String,
    decided_at_ms: u128,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectWorkspaceRecoveryEnvelope {
    schema_version: u32,
    payload_checksum: String,
    payload: ProjectWorkspaceRecoveryPayload,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectWorkspaceRecoveryPayload {
    schema_version: u32,
    project_root: String,
    accepted_manifest: ProjectDiskManifest,
    revision: u64,
    persisted_at_ms: u128,
    documents: Arc<BTreeMap<String, Arc<FileBufferEntry>>>,
    accepted_binary_resource_hashes: Arc<BTreeMap<String, String>>,
    binary_resources: Arc<BTreeMap<String, Arc<WorkspaceBinaryResource>>>,
    deleted_binary_resources: Arc<BTreeSet<String>>,
    accepted_page_js: Arc<BTreeMap<String, PageJsConfig>>,
    page_js_drafts: Vec<ProjectWorkspacePageJsRecoveryDraft>,
    history: WorkspaceHistory,
    #[serde(default)]
    last_projection_transaction_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectWorkspaceRecoveryPayloadRef<'a> {
    schema_version: u32,
    project_root: &'a str,
    accepted_manifest: &'a ProjectDiskManifest,
    revision: u64,
    persisted_at_ms: u128,
    documents: &'a BTreeMap<String, Arc<FileBufferEntry>>,
    accepted_binary_resource_hashes: &'a BTreeMap<String, String>,
    binary_resources: &'a BTreeMap<String, Arc<WorkspaceBinaryResource>>,
    deleted_binary_resources: &'a BTreeSet<String>,
    accepted_page_js: &'a BTreeMap<String, PageJsConfig>,
    page_js_drafts: Vec<ProjectWorkspacePageJsRecoveryDraftRef<'a>>,
    history: &'a WorkspaceHistory,
    last_projection_transaction_id: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectWorkspacePageJsRecoveryDraftRef<'a> {
    template_path: &'a str,
    base: &'a PageJsConfig,
    current: &'a PageJsConfig,
    cachebust_assets: bool,
    source: &'a str,
    coalesce_key: Option<&'a str>,
    transaction_id: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectWorkspacePageJsRecoveryDraft {
    template_path: String,
    base: PageJsConfig,
    current: PageJsConfig,
    cachebust_assets: bool,
    source: String,
    coalesce_key: Option<String>,
    transaction_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectWorkspaceRecoveryJournalEnvelope {
    schema_version: u32,
    payload_checksum: String,
    payload: ProjectWorkspaceRecoveryJournalPayload,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectWorkspaceRecoveryJournalPayload {
    schema_version: u32,
    project_root: String,
    base_revision: u64,
    revision: u64,
    persisted_at_ms: u128,
    accepted_manifest: Option<ProjectDiskManifest>,
    document_changes: BTreeMap<String, Option<Arc<FileBufferEntry>>>,
    accepted_binary_resource_hash_changes: BTreeMap<String, Option<String>>,
    binary_resource_changes: BTreeMap<String, Option<Arc<WorkspaceBinaryResource>>>,
    deleted_binary_resources: Option<Arc<BTreeSet<String>>>,
    accepted_page_js_changes: BTreeMap<String, Option<PageJsConfig>>,
    page_js_draft_changes: BTreeMap<String, Option<ProjectWorkspacePageJsRecoveryDraft>>,
    history: WorkspaceHistoryRecoveryPatch,
    last_projection_transaction_id: Option<String>,
}

pub fn persist_project_workspace_recovery<R: Runtime>(
    app: &AppHandle<R>,
    workspace: &ProjectWorkspace,
) -> Result<(), String> {
    persist_project_workspace_recovery_snapshot(app, workspace)
}

/// Canonical durable Save boundary for callers that already own a detached
/// ProjectWorkspace candidate. The candidate is returned as successful only
/// after both source publication and the recovery snapshot are durable.
pub fn save_project_workspace_with_recovery<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    identity: &ProjectWorkspaceIdentity,
) -> Result<ProjectWorkspaceSaveReceipt, ProjectWorkspaceSaveError> {
    let receipt = super::save::save_project_workspace(app, project_root, workspace, identity)?;
    persist_project_workspace_recovery(app, workspace).map_err(|diagnostic| {
        ProjectWorkspaceSaveError::recovery_required(
            receipt
                .transaction_id
                .clone()
                .unwrap_or_else(|| format!("workspace-save-recovery-{}", workspace.revision)),
            receipt
                .written_files
                .iter()
                .chain(&receipt.removed_files)
                .cloned()
                .collect(),
            receipt.write_receipts.clone(),
            format!(
                "Save-ul proiectului a fost acceptat, dar snapshotul de recuperare ProjectWorkspace nu a putut fi persistat: {diagnostic}"
            ),
        )
    })?;
    Ok(receipt)
}

fn persist_project_workspace_recovery_snapshot<R: Runtime>(
    app: &AppHandle<R>,
    workspace: &ProjectWorkspace,
) -> Result<(), String> {
    let source = serialize_recovery_envelope_from_workspace(workspace)?;
    if source.len() as u64 > PROJECT_WORKSPACE_RECOVERY_MAX_BYTES {
        return Err(format!(
            "ProjectWorkspace recovery depășește limita de {} bytes.",
            PROJECT_WORKSPACE_RECOVERY_MAX_BYTES
        ));
    }

    let path = project_workspace_recovery_path(app, &workspace.session.project_root)?;
    let boundary = project_session_dir(app, &workspace.session.project_root)?;
    let intent = WriteIntent::new(
        WriteCategory::InternalAppWrite,
        WriteOwner::ProjectWorkspace,
        WriteOperationKind::WriteText,
        WriteTarget::new(path, boundary, "sessions/project-workspace.json"),
        WritePolicy::internal_atomic(),
        "Persist ProjectWorkspace recovery checkpoint",
    );
    WriteAuthority::new(app)
        .write_text(intent, &format!("{source}\n"))
        .map_err(|error| error.into_terminal_diagnostic())?;

    // A checkpoint already contains every committed journal entry. Stale
    // records are ignored by revision during recovery, so cleanup failure does
    // not invalidate this durable transaction.
    if let Err(error) =
        remove_project_workspace_recovery_journal(app, &workspace.session.project_root)
    {
        eprintln!("[Pană Studio] Jurnalul recovery compactat nu a putut fi curățat: {error}");
    }
    Ok(())
}

fn persist_project_workspace_recovery_transaction<R: Runtime>(
    app: &AppHandle<R>,
    base: &ProjectWorkspace,
    current: &ProjectWorkspace,
) -> Result<(), String> {
    if base.session.project_root != current.session.project_root
        || base.runtime_session_id() != current.runtime_session_id()
    {
        return Err(
            "ProjectWorkspace recovery incremental a refuzat o tranziție între sesiuni."
                .to_string(),
        );
    }
    if current.revision <= base.revision {
        return persist_project_workspace_recovery_snapshot(app, current);
    }

    let checkpoint_path = project_workspace_recovery_path(app, &current.session.project_root)?;
    let checkpoint_exists =
        regular_file_len_if_exists(&checkpoint_path, "Checkpoint-ul ProjectWorkspace recovery")?
            .is_some();
    if !checkpoint_exists
        || current
            .revision
            .is_multiple_of(PROJECT_WORKSPACE_RECOVERY_CHECKPOINT_INTERVAL)
    {
        return persist_project_workspace_recovery_snapshot(app, current);
    }

    let journal_source = serialize_recovery_journal_transaction(base, current)?;
    if journal_source.len() + 1 > PROJECT_WORKSPACE_RECOVERY_JOURNAL_RECORD_MAX_BYTES {
        return persist_project_workspace_recovery_snapshot(app, current);
    }
    let journal_path = project_workspace_recovery_journal_path(app, &current.session.project_root)?;
    let existing_journal_bytes =
        regular_file_len_if_exists(&journal_path, "Jurnalul ProjectWorkspace recovery")?
            .unwrap_or_default();
    let journal_bytes_after = existing_journal_bytes
        .checked_add(journal_source.len() as u64 + 1)
        .ok_or_else(|| "Dimensiunea jurnalului recovery a depășit contorul.".to_string())?;
    if journal_bytes_after > PROJECT_WORKSPACE_RECOVERY_JOURNAL_MAX_BYTES {
        return persist_project_workspace_recovery_snapshot(app, current);
    }

    let boundary = project_session_dir(app, &current.session.project_root)?;
    let intent = WriteIntent::new(
        WriteCategory::InternalAppWrite,
        WriteOwner::ProjectWorkspace,
        WriteOperationKind::AppendText,
        WriteTarget::new(
            journal_path,
            boundary,
            "sessions/project-workspace.journal.jsonl",
        ),
        WritePolicy::internal_append(),
        "Append ProjectWorkspace recovery delta",
    );
    WriteAuthority::new(app)
        .append_text(intent, &format!("{journal_source}\n"))
        .map_err(|error| error.into_terminal_diagnostic())?;
    Ok(())
}

fn serialize_recovery_envelope_from_workspace(
    workspace: &ProjectWorkspace,
) -> Result<String, String> {
    workspace.accepted_disk.require_identity(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
    )?;
    workspace.accepted_disk.require_complete()?;
    validate_workspace_recovery_paths(workspace)?;
    let page_js_drafts = workspace
        .page_js
        .drafts
        .values()
        .map(|draft| ProjectWorkspacePageJsRecoveryDraftRef {
            template_path: &draft.template_path,
            base: &draft.base,
            current: &draft.current,
            cachebust_assets: draft.cachebust_assets,
            source: &draft.source,
            coalesce_key: draft.coalesce_key.as_deref(),
            transaction_id: draft.transaction_id.as_deref(),
        })
        .collect();
    let payload = ProjectWorkspaceRecoveryPayloadRef {
        schema_version: PROJECT_WORKSPACE_RECOVERY_SCHEMA_VERSION,
        project_root: &workspace.session.project_root,
        accepted_manifest: &workspace.accepted_disk.manifest,
        revision: workspace.revision,
        persisted_at_ms: now_ms(),
        documents: workspace.documents.files.as_ref(),
        accepted_binary_resource_hashes: workspace.accepted_binary_resource_hashes.as_ref(),
        binary_resources: workspace.binary_resources.as_ref(),
        deleted_binary_resources: workspace.deleted_binary_resources.as_ref(),
        accepted_page_js: workspace.accepted_page_js.as_ref(),
        page_js_drafts,
        history: &workspace.history,
        last_projection_transaction_id: workspace.last_projection_transaction_id.as_deref(),
    };
    let payload_source = serde_json::to_string(&payload).map_err(|error| {
        format!("ProjectWorkspace recovery nu poate serializa payloadul: {error}")
    })?;
    let checksum = hash_text(&payload_source);
    Ok(format!(
        "{{\"schemaVersion\":{PROJECT_WORKSPACE_RECOVERY_SCHEMA_VERSION},\"payloadChecksum\":\"{checksum}\",\"payload\":{payload_source}}}"
    ))
}

fn serialize_recovery_journal_transaction(
    base: &ProjectWorkspace,
    current: &ProjectWorkspace,
) -> Result<String, String> {
    validate_workspace_recovery_paths(base)?;
    validate_workspace_recovery_paths(current)?;
    current
        .accepted_disk
        .require_identity(&current.runtime_session_id(), &current.session.project_root)?;
    current.accepted_disk.require_complete()?;
    let payload = ProjectWorkspaceRecoveryJournalPayload {
        schema_version: PROJECT_WORKSPACE_RECOVERY_JOURNAL_SCHEMA_VERSION,
        project_root: current.session.project_root.clone(),
        base_revision: base.revision,
        revision: current.revision,
        persisted_at_ms: now_ms(),
        accepted_manifest: (base.accepted_disk.manifest != current.accepted_disk.manifest)
            .then(|| current.accepted_disk.manifest.clone()),
        document_changes: recovery_map_changes(
            base.documents.files.as_ref(),
            current.documents.files.as_ref(),
        ),
        accepted_binary_resource_hash_changes: recovery_map_changes(
            base.accepted_binary_resource_hashes.as_ref(),
            current.accepted_binary_resource_hashes.as_ref(),
        ),
        binary_resource_changes: recovery_map_changes(
            base.binary_resources.as_ref(),
            current.binary_resources.as_ref(),
        ),
        deleted_binary_resources: (base.deleted_binary_resources
            != current.deleted_binary_resources)
            .then(|| current.deleted_binary_resources.clone()),
        accepted_page_js_changes: recovery_map_changes(
            base.accepted_page_js.as_ref(),
            current.accepted_page_js.as_ref(),
        ),
        page_js_draft_changes: recovery_page_js_draft_changes(base, current),
        history: current.history.recovery_patch_from(&base.history),
        last_projection_transaction_id: current.last_projection_transaction_id.clone(),
    };
    let payload_source = serde_json::to_string(&payload)
        .map_err(|error| format!("Jurnalul ProjectWorkspace nu poate serializa delta: {error}"))?;
    let checksum = hash_text(&payload_source);
    Ok(format!(
        "{{\"schemaVersion\":{PROJECT_WORKSPACE_RECOVERY_JOURNAL_SCHEMA_VERSION},\"payloadChecksum\":\"{checksum}\",\"payload\":{payload_source}}}"
    ))
}

fn recovery_map_changes<V: Clone + PartialEq>(
    base: &BTreeMap<String, V>,
    current: &BTreeMap<String, V>,
) -> BTreeMap<String, Option<V>> {
    base.keys()
        .chain(current.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter_map(|key| {
            let before = base.get(&key);
            let after = current.get(&key);
            (before != after).then(|| (key, after.cloned()))
        })
        .collect()
}

fn recovery_page_js_draft_changes(
    base: &ProjectWorkspace,
    current: &ProjectWorkspace,
) -> BTreeMap<String, Option<ProjectWorkspacePageJsRecoveryDraft>> {
    base.page_js
        .drafts
        .keys()
        .chain(current.page_js.drafts.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter_map(|key| {
            let before = base.page_js.drafts.get(&key);
            let after = current.page_js.drafts.get(&key);
            let equivalent = match (before, after) {
                (Some(before), Some(after)) => {
                    before.template_path == after.template_path
                        && before.base == after.base
                        && before.current == after.current
                        && before.cachebust_assets == after.cachebust_assets
                        && before.source == after.source
                        && before.coalesce_key == after.coalesce_key
                        && before.transaction_id == after.transaction_id
                }
                (None, None) => true,
                _ => false,
            };
            (!equivalent).then(|| {
                (
                    key,
                    after.map(|draft| ProjectWorkspacePageJsRecoveryDraft {
                        template_path: draft.template_path.clone(),
                        base: draft.base.clone(),
                        current: draft.current.clone(),
                        cachebust_assets: draft.cachebust_assets,
                        source: draft.source.clone(),
                        coalesce_key: draft.coalesce_key.clone(),
                        transaction_id: draft.transaction_id.clone(),
                    }),
                )
            })
        })
        .collect()
}

fn regular_file_len_if_exists(path: &Path, label: &str) -> Result<Option<u64>, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
            Ok(Some(metadata.len()))
        }
        Ok(_) => Err(format!(
            "{label} a refuzat un fișier symlink sau non-regular."
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("{label} nu poate citi metadata: {error}")),
    }
}

pub fn emit_project_workspace_mutated<R: Runtime>(
    app: &AppHandle<R>,
    workspace: &ProjectWorkspace,
    preview_projection: ProjectWorkspacePreviewProjection,
) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Err(error) = state.clear_publish_authorization() {
            eprintln!("[Pană Studio] Autorizația Publish nu a putut fi invalidată: {error}");
        }
    }
    if let Err(error) = app.emit(
        PROJECT_WORKSPACE_MUTATED_EVENT,
        ProjectWorkspaceMutationEvent {
            project_root: workspace.session.project_root.clone(),
            runtime_session_id: workspace.runtime_session_id(),
            workspace_revision: workspace.revision,
            dirty: workspace.is_dirty(),
            preview_projection_required: preview_projection
                == ProjectWorkspacePreviewProjection::Required,
        },
    ) {
        eprintln!("[Pană Studio] ProjectWorkspace mutation event nu a putut fi emis: {error}");
    }
}

pub fn inspect_project_workspace_recovery_for_open<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &Path,
    current_manifest: &ProjectDiskManifest,
    current_root_fingerprint: &ProjectRootFingerprint,
) -> Result<ProjectOpenRecoveryAssessment, String> {
    let canonical_root = project_root.canonicalize().map_err(|error| {
        format!("Nu am putut rezolva proiectul pentru recovery preflight: {error}")
    })?;
    let project_root = canonical_root.to_string_lossy().to_string();
    if current_manifest.root != project_root
        || current_root_fingerprint.canonical_path != project_root
    {
        return Err(
            "Recovery preflight a primit manifest sau fingerprint pentru alt proiect.".to_string(),
        );
    }

    let recovery_path = project_workspace_recovery_path(app, &project_root)?;
    let recovery_journal_path = project_workspace_recovery_journal_path(app, &project_root)?;
    let checkpoint_source = read_recovery_source(&recovery_path)?;
    let journal_source = read_recovery_journal_source(&recovery_journal_path)?;
    if checkpoint_source.is_none() && journal_source.is_none() {
        return Ok(ProjectOpenRecoveryAssessment {
            schema_version: PROJECT_OPEN_RECOVERY_ASSESSMENT_SCHEMA_VERSION,
            status: ProjectOpenRecoveryStatus::Missing,
            project_root,
            assessment_token: None,
            conflict_reason: None,
            root_identity_changed: previous_root_identity_changed(app, current_root_fingerprint)?,
            recovery_revision: None,
            dirty_document_count: 0,
            staged_binary_resource_count: 0,
            deleted_binary_resource_count: 0,
            page_js_draft_count: 0,
            undo_count: 0,
            redo_count: 0,
            accepted_file_count: 0,
            current_file_count: current_manifest.files.len(),
            diagnostic: None,
        });
    }

    let assessment_token = project_open_recovery_assessment_token(
        &recovery_bundle_evidence(checkpoint_source.as_deref(), journal_source.as_deref()),
        current_manifest,
        current_root_fingerprint,
    )?;
    let root_identity_changed = previous_root_identity_changed(app, current_root_fingerprint)?;
    let marker_matches = read_project_open_recovery_decision_marker(app, &project_root)?
        .is_some_and(|marker| {
            marker.schema_version == PROJECT_OPEN_RECOVERY_DECISION_SCHEMA_VERSION
                && marker.project_root == project_root
                && marker.assessment_token == assessment_token
        });

    let envelope = match parse_project_workspace_recovery_bundle(
        checkpoint_source.as_deref(),
        journal_source.as_deref(),
    ) {
        Ok(envelope) => envelope,
        Err(diagnostic) => {
            return Ok(ProjectOpenRecoveryAssessment {
                schema_version: PROJECT_OPEN_RECOVERY_ASSESSMENT_SCHEMA_VERSION,
                status: if marker_matches {
                    ProjectOpenRecoveryStatus::Abandoned
                } else {
                    ProjectOpenRecoveryStatus::DecisionRequired
                },
                project_root,
                assessment_token: Some(assessment_token),
                conflict_reason: Some(ProjectOpenRecoveryConflictReason::RecoveryInvalid),
                root_identity_changed,
                recovery_revision: None,
                dirty_document_count: 0,
                staged_binary_resource_count: 0,
                deleted_binary_resource_count: 0,
                page_js_draft_count: 0,
                undo_count: 0,
                redo_count: 0,
                accepted_file_count: 0,
                current_file_count: current_manifest.files.len(),
                diagnostic: Some(diagnostic),
            });
        }
    };
    let payload = &envelope.payload;
    let root_matches =
        payload.project_root == project_root && payload.accepted_manifest.root == project_root;
    let manifest_matches = root_matches && payload.accepted_manifest == *current_manifest;
    let root_replaced = root_identity_changed == Some(true);
    let conflict_reason = if !root_matches {
        Some(ProjectOpenRecoveryConflictReason::RecoveryInvalid)
    } else if root_replaced {
        Some(ProjectOpenRecoveryConflictReason::ProjectRootReplaced)
    } else if !manifest_matches {
        Some(ProjectOpenRecoveryConflictReason::DiskBaselineChanged)
    } else {
        None
    };
    let status = if marker_matches {
        ProjectOpenRecoveryStatus::Abandoned
    } else if conflict_reason.is_none() {
        ProjectOpenRecoveryStatus::Restorable
    } else {
        ProjectOpenRecoveryStatus::DecisionRequired
    };
    let history = payload.history.snapshot();

    Ok(ProjectOpenRecoveryAssessment {
        schema_version: PROJECT_OPEN_RECOVERY_ASSESSMENT_SCHEMA_VERSION,
        status,
        project_root,
        assessment_token: Some(assessment_token),
        conflict_reason,
        root_identity_changed,
        recovery_revision: Some(payload.revision),
        dirty_document_count: payload
            .documents
            .values()
            .filter(|entry| entry.is_dirty())
            .count(),
        staged_binary_resource_count: payload.binary_resources.len(),
        deleted_binary_resource_count: payload.deleted_binary_resources.len(),
        page_js_draft_count: payload.page_js_drafts.len(),
        undo_count: history.undo_count,
        redo_count: history.redo_count,
        accepted_file_count: payload.accepted_manifest.files.len(),
        current_file_count: current_manifest.files.len(),
        diagnostic: conflict_reason.map(|reason| match reason {
            ProjectOpenRecoveryConflictReason::DiskBaselineChanged => {
                "Conținutul de pe disk nu mai corespunde baseline-ului sesiunii recuperabile."
                    .to_string()
            }
            ProjectOpenRecoveryConflictReason::ProjectRootReplaced => {
                "Calea proiectului desemnează acum un alt dosar fizic decât sesiunea recuperabilă."
                    .to_string()
            }
            ProjectOpenRecoveryConflictReason::RecoveryInvalid => {
                "Recovery-ul aparține altei rădăcini de proiect.".to_string()
            }
        }),
    })
}

pub fn resolve_project_open_recovery(
    assessment: &ProjectOpenRecoveryAssessment,
    decision: Option<&ProjectOpenRecoveryDecisionInput>,
) -> Result<ProjectOpenRecoveryResolution, String> {
    match assessment.status {
        ProjectOpenRecoveryStatus::Missing | ProjectOpenRecoveryStatus::Abandoned => {
            if decision.is_some() {
                return Err(
                    "Decizia de abandonare nu mai corespunde unui recovery conflictual activ."
                        .to_string(),
                );
            }
            Ok(ProjectOpenRecoveryResolution::Skip)
        }
        ProjectOpenRecoveryStatus::Restorable => {
            if decision.is_some() {
                return Err(
                    "Recovery-ul este compatibil și nu poate fi abandonat printr-o decizie stale."
                        .to_string(),
                );
            }
            Ok(ProjectOpenRecoveryResolution::Restore)
        }
        ProjectOpenRecoveryStatus::DecisionRequired => {
            let decision = decision.ok_or_else(|| {
                "Deschiderea proiectului necesită o decizie explicită pentru recovery-ul incompatibil."
                    .to_string()
            })?;
            if decision.action != ProjectOpenRecoveryDecisionAction::Abandon
                || assessment.assessment_token.as_deref()
                    != Some(decision.assessment_token.as_str())
            {
                return Err(
                    "Decizia de recovery este stale sau nu corespunde exact stării inspectate."
                        .to_string(),
                );
            }
            Ok(ProjectOpenRecoveryResolution::ExplicitAbandon)
        }
    }
}

pub fn require_project_open_recovery_assessment_unchanged(
    before: &ProjectOpenRecoveryAssessment,
    after: &ProjectOpenRecoveryAssessment,
) -> Result<(), String> {
    if before.project_root != after.project_root
        || before.status != after.status
        || before.assessment_token != after.assessment_token
        || before.root_identity_changed != after.root_identity_changed
    {
        return Err(
            "Recovery-ul sau dosarul țintă s-a schimbat în timpul deschiderii; decizia trebuie reevaluată."
                .to_string(),
        );
    }
    Ok(())
}

pub fn persist_project_open_recovery_abandonment<R: Runtime>(
    app: &AppHandle<R>,
    assessment: &ProjectOpenRecoveryAssessment,
    decision: &ProjectOpenRecoveryDecisionInput,
) -> Result<(), String> {
    if resolve_project_open_recovery(assessment, Some(decision))?
        != ProjectOpenRecoveryResolution::ExplicitAbandon
    {
        return Err("Marker-ul de abandonare cere o decizie explicită validă.".to_string());
    }
    let marker = ProjectOpenRecoveryDecisionMarker {
        schema_version: PROJECT_OPEN_RECOVERY_DECISION_SCHEMA_VERSION,
        project_root: assessment.project_root.clone(),
        assessment_token: decision.assessment_token.clone(),
        decided_at_ms: now_ms(),
    };
    let source = serde_json::to_string_pretty(&marker)
        .map_err(|error| format!("Decizia de recovery nu poate fi serializată: {error}"))?;
    let path = project_open_recovery_decision_path(app, &assessment.project_root)?;
    let boundary = project_session_dir(app, &assessment.project_root)?;
    let intent = WriteIntent::new(
        WriteCategory::InternalAppWrite,
        WriteOwner::ProjectWorkspace,
        WriteOperationKind::WriteText,
        WriteTarget::new(
            path,
            boundary,
            "sessions/project-open-recovery-decision.json",
        ),
        WritePolicy::internal_atomic(),
        "Persist explicit project-open recovery decision",
    );
    WriteAuthority::new(app)
        .write_text(intent, &format!("{source}\n"))
        .map_err(|error| error.into_terminal_diagnostic())?;
    Ok(())
}

pub fn clear_project_open_recovery_decision<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &str,
) -> Result<(), String> {
    let path = project_open_recovery_decision_path(app, project_root)?;
    let boundary = project_session_dir(app, project_root)?;
    let intent = WriteIntent::new(
        WriteCategory::InternalAppWrite,
        WriteOwner::ProjectWorkspace,
        WriteOperationKind::RemoveFile,
        WriteTarget::new(
            path,
            boundary,
            "sessions/project-open-recovery-decision.json",
        ),
        WritePolicy::internal_lifecycle(),
        "Clear project-open recovery decision",
    );
    WriteAuthority::new(app)
        .remove_file_if_exists(intent)
        .map_err(|error| error.into_terminal_diagnostic())?;
    Ok(())
}

pub fn restore_project_workspace_recovery<R: Runtime>(
    app: &AppHandle<R>,
    workspace: &mut ProjectWorkspace,
) -> Result<ProjectWorkspaceRecoveryStatus, String> {
    let path = project_workspace_recovery_path(app, &workspace.session.project_root)?;
    let journal_path =
        project_workspace_recovery_journal_path(app, &workspace.session.project_root)?;
    let checkpoint_source = read_recovery_source(&path)?;
    let journal_source = read_recovery_journal_source(&journal_path)?;
    if checkpoint_source.is_none() && journal_source.is_none() {
        return Ok(ProjectWorkspaceRecoveryStatus::Missing);
    }
    let envelope = parse_project_workspace_recovery_bundle(
        checkpoint_source.as_deref(),
        journal_source.as_deref(),
    )?;
    let payload = envelope.payload;
    if payload.project_root != workspace.session.project_root
        || payload.accepted_manifest.root != workspace.session.project_root
    {
        return Err("ProjectWorkspace recovery aparține altei rădăcini de proiect.".to_string());
    }
    if payload.accepted_manifest != workspace.accepted_disk.manifest {
        return Err(
            "ProjectWorkspace recovery nu a fost aplicat: proiectul s-a schimbat extern față de baseline-ul sesiunii recuperabile. Este necesară o decizie explicită de păstrare sau abandonare a drafturilor."
                .to_string(),
        );
    }
    payload.history.validate_recovery_limits()?;
    for path in payload
        .documents
        .keys()
        .map(String::as_str)
        .chain(
            payload
                .accepted_binary_resource_hashes
                .keys()
                .map(String::as_str),
        )
        .chain(payload.binary_resources.keys().map(String::as_str))
        .chain(payload.deleted_binary_resources.iter().map(String::as_str))
        .chain(payload.history.recovery_paths())
    {
        validate_recovery_path(Path::new(&workspace.session.project_root), path)?;
    }

    let accepted_documents = workspace.documents.files.clone();
    let mut documents = workspace.documents.clone();
    let all_paths = accepted_documents
        .keys()
        .chain(payload.documents.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    for relative_path in all_paths {
        match (
            accepted_documents.get(&relative_path),
            payload.documents.get(&relative_path),
        ) {
            (Some(accepted), Some(recovered)) => {
                if accepted.baseline_text != recovered.baseline_text
                    || accepted.baseline.hash != recovered.baseline.hash
                {
                    return Err(format!(
                        "ProjectWorkspace recovery are baseline divergent pentru {relative_path}."
                    ));
                }
                let current = documents
                    .text_snapshot(&relative_path)
                    .ok_or_else(|| format!("Lipsește baseline-ul {relative_path}."))?;
                let recovered_text = recovered.current_text().to_string();
                if recovered_text != current.text {
                    documents.set_draft_if_current(
                        &relative_path,
                        recovered_text,
                        &FileBufferMutationExpectation {
                            expected_revision: current.revision,
                            expected_hash: current.hash,
                        },
                        now_ms(),
                    )?;
                }
            }
            (Some(_), None) => {
                documents.files_mut().remove(&relative_path);
            }
            (None, Some(recovered)) => {
                documents.stage_new_text_file(
                    &relative_path,
                    recovered.current_text().to_string(),
                    now_ms(),
                )?;
            }
            (None, None) => unreachable!("path collected from at least one map"),
        }
    }

    let mut page_js = PageJsDraftStore::new(&workspace.session);
    for draft in payload.page_js_drafts {
        let accepted = payload
            .accepted_page_js
            .get(&draft.template_path)
            .cloned()
            .unwrap_or_else(|| draft.base.clone());
        if accepted != draft.base {
            return Err(format!(
                "ProjectWorkspace recovery are baseline Page JS divergent pentru {}.",
                draft.template_path
            ));
        }
        page_js.stage(PageJsDraftStageInput {
            template_path: draft.template_path,
            expected_project_root: workspace.session.project_root.clone(),
            expected_session_id: workspace.runtime_session_id(),
            base_config: draft.base,
            current_config: draft.current,
            cachebust_assets: draft.cachebust_assets,
            source: Some(draft.source),
            coalesce_key: draft.coalesce_key,
            transaction_id: draft.transaction_id,
        })?;
    }

    workspace.documents = documents;
    workspace.accepted_documents = accepted_documents;
    let accepted_disk_paths = payload
        .accepted_manifest
        .files
        .iter()
        .map(|entry| entry.relative_path.as_str())
        .collect::<BTreeSet<_>>();
    for (path, hash) in payload.accepted_binary_resource_hashes.iter() {
        if !accepted_disk_paths.contains(path.as_str())
            || hash.len() != 16
            || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(format!(
                "ProjectWorkspace recovery are un baseline binar invalid pentru {path}."
            ));
        }
    }
    if payload
        .binary_resources
        .keys()
        .any(|path| payload.deleted_binary_resources.contains(path))
    {
        return Err(
            "ProjectWorkspace recovery conține aceeași resursă binară ca draft și delete."
                .to_string(),
        );
    }
    let binary_bytes = payload
        .binary_resources
        .values()
        .try_fold(0_u64, |total, resource| {
            if resource.bytes.len() as u64 > PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_BYTES {
                return None;
            }
            total.checked_add(resource.bytes.len() as u64)
        })
        .ok_or_else(|| {
            "ProjectWorkspace recovery a depășit contorul resurselor binare.".to_string()
        })?;
    if binary_bytes > PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_TOTAL_BYTES {
        return Err(
            "ProjectWorkspace recovery depășește limita resurselor binare din sesiune.".to_string(),
        );
    }
    workspace.accepted_binary_resource_hashes = payload.accepted_binary_resource_hashes;
    workspace.binary_resources = payload.binary_resources;
    workspace.deleted_binary_resources = payload.deleted_binary_resources;
    workspace.accepted_page_js = payload.accepted_page_js;
    workspace.page_js = page_js;
    workspace.history = payload.history;
    workspace.last_projection_transaction_id =
        payload.last_projection_transaction_id.or_else(|| {
            workspace
                .history
                .snapshot()
                .next_undo
                .map(|entry| entry.transaction_id)
        });
    workspace.revision = payload.revision;
    workspace.project_model = None;
    workspace.project_model_source_revision = None;
    Ok(ProjectWorkspaceRecoveryStatus::Restored)
}

pub fn clear_project_workspace_recovery<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &str,
) -> Result<(), String> {
    remove_project_workspace_recovery_journal(app, project_root)?;
    let path = project_workspace_recovery_path(app, project_root)?;
    let boundary = project_session_dir(app, project_root)?;
    let intent = WriteIntent::new(
        WriteCategory::InternalAppWrite,
        WriteOwner::ProjectWorkspace,
        WriteOperationKind::RemoveFile,
        WriteTarget::new(path, boundary, "sessions/project-workspace.json"),
        WritePolicy::internal_lifecycle(),
        "Clear ProjectWorkspace recovery",
    );
    WriteAuthority::new(app)
        .remove_file_if_exists(intent)
        .map_err(|error| error.into_terminal_diagnostic())?;
    Ok(())
}

fn remove_project_workspace_recovery_journal<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &str,
) -> Result<(), String> {
    let path = project_workspace_recovery_journal_path(app, project_root)?;
    let boundary = project_session_dir(app, project_root)?;
    let intent = WriteIntent::new(
        WriteCategory::InternalAppWrite,
        WriteOwner::ProjectWorkspace,
        WriteOperationKind::RemoveFile,
        WriteTarget::new(path, boundary, "sessions/project-workspace.journal.jsonl"),
        WritePolicy::internal_lifecycle(),
        "Clear ProjectWorkspace recovery journal",
    );
    WriteAuthority::new(app)
        .remove_file_if_exists(intent)
        .map_err(|error| error.into_terminal_diagnostic())?;
    Ok(())
}

fn parse_project_workspace_recovery_envelope(
    source: &str,
) -> Result<ProjectWorkspaceRecoveryEnvelope, String> {
    let envelope = serde_json::from_str::<ProjectWorkspaceRecoveryEnvelope>(source)
        .map_err(|error| format!("ProjectWorkspace recovery este JSON invalid: {error}"))?;
    if envelope.schema_version != PROJECT_WORKSPACE_RECOVERY_SCHEMA_VERSION
        || envelope.payload.schema_version != PROJECT_WORKSPACE_RECOVERY_SCHEMA_VERSION
    {
        return Err(format!(
            "ProjectWorkspace recovery are schema incompatibilă {}/{}.",
            envelope.schema_version, envelope.payload.schema_version
        ));
    }
    let payload_source = serde_json::to_string(&envelope.payload).map_err(|error| {
        format!("ProjectWorkspace recovery nu poate reserializa payloadul: {error}")
    })?;
    if hash_text(&payload_source) != envelope.payload_checksum {
        return Err(
            "ProjectWorkspace recovery a eșuat verificarea checksum; starea nu a fost restaurată."
                .to_string(),
        );
    }
    Ok(envelope)
}

fn parse_project_workspace_recovery_bundle(
    checkpoint_source: Option<&str>,
    journal_source: Option<&str>,
) -> Result<ProjectWorkspaceRecoveryEnvelope, String> {
    let checkpoint_source = checkpoint_source.ok_or_else(|| {
        "Jurnalul ProjectWorkspace recovery există fără un checkpoint de bază.".to_string()
    })?;
    let mut envelope = parse_project_workspace_recovery_envelope(checkpoint_source)?;
    if let Some(journal_source) = journal_source {
        for (index, line) in journal_source.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let journal = serde_json::from_str::<ProjectWorkspaceRecoveryJournalEnvelope>(line)
                .map_err(|error| {
                    format!(
                        "Jurnalul ProjectWorkspace recovery are JSON invalid la linia {}: {error}",
                        index + 1
                    )
                })?;
            if journal.schema_version != PROJECT_WORKSPACE_RECOVERY_JOURNAL_SCHEMA_VERSION
                || journal.payload.schema_version
                    != PROJECT_WORKSPACE_RECOVERY_JOURNAL_SCHEMA_VERSION
            {
                return Err(format!(
                    "Jurnalul ProjectWorkspace recovery are schema incompatibilă {}/{} la linia {}.",
                    journal.schema_version,
                    journal.payload.schema_version,
                    index + 1
                ));
            }
            let payload_source = serde_json::to_string(&journal.payload).map_err(|error| {
                format!(
                    "Jurnalul ProjectWorkspace recovery nu poate reserializa linia {}: {error}",
                    index + 1
                )
            })?;
            if hash_text(&payload_source) != journal.payload_checksum {
                return Err(format!(
                    "Jurnalul ProjectWorkspace recovery a eșuat checksum la linia {}.",
                    index + 1
                ));
            }
            if journal.payload.revision <= envelope.payload.revision {
                continue;
            }
            apply_project_workspace_recovery_journal_payload(
                &mut envelope.payload,
                journal.payload,
            )
            .map_err(|error| format!("{error} Linia jurnalului: {}.", index + 1))?;
        }
    }
    let effective_payload_source = serde_json::to_string(&envelope.payload).map_err(|error| {
        format!("ProjectWorkspace recovery efectiv nu poate fi serializat: {error}")
    })?;
    envelope.payload_checksum = hash_text(&effective_payload_source);
    Ok(envelope)
}

fn apply_project_workspace_recovery_journal_payload(
    checkpoint: &mut ProjectWorkspaceRecoveryPayload,
    journal: ProjectWorkspaceRecoveryJournalPayload,
) -> Result<(), String> {
    if journal.project_root != checkpoint.project_root
        || journal.base_revision != checkpoint.revision
        || journal.revision <= journal.base_revision
    {
        return Err(
            "Jurnalul ProjectWorkspace recovery nu continuă exact checkpoint-ul anterior."
                .to_string(),
        );
    }
    if let Some(accepted_manifest) = journal.accepted_manifest {
        checkpoint.accepted_manifest = accepted_manifest;
    }
    apply_recovery_map_changes(
        Arc::make_mut(&mut checkpoint.documents),
        journal.document_changes,
    );
    apply_recovery_map_changes(
        Arc::make_mut(&mut checkpoint.accepted_binary_resource_hashes),
        journal.accepted_binary_resource_hash_changes,
    );
    apply_recovery_map_changes(
        Arc::make_mut(&mut checkpoint.binary_resources),
        journal.binary_resource_changes,
    );
    if let Some(deleted_binary_resources) = journal.deleted_binary_resources {
        checkpoint.deleted_binary_resources = deleted_binary_resources;
    }
    apply_recovery_map_changes(
        Arc::make_mut(&mut checkpoint.accepted_page_js),
        journal.accepted_page_js_changes,
    );
    let mut page_js_drafts = checkpoint
        .page_js_drafts
        .drain(..)
        .map(|draft| (draft.template_path.clone(), draft))
        .collect::<BTreeMap<_, _>>();
    for (template_path, draft) in journal.page_js_draft_changes {
        match draft {
            Some(draft) if draft.template_path == template_path => {
                page_js_drafts.insert(template_path, draft);
            }
            Some(_) => {
                return Err(
                    "Jurnalul ProjectWorkspace recovery are o identitate Page JS divergentă."
                        .to_string(),
                );
            }
            None => {
                page_js_drafts.remove(&template_path);
            }
        }
    }
    checkpoint.page_js_drafts = page_js_drafts.into_values().collect();
    checkpoint.history.apply_recovery_patch(journal.history)?;
    checkpoint.last_projection_transaction_id = journal.last_projection_transaction_id;
    checkpoint.revision = journal.revision;
    checkpoint.persisted_at_ms = journal.persisted_at_ms;
    Ok(())
}

fn apply_recovery_map_changes<V>(
    target: &mut BTreeMap<String, V>,
    changes: BTreeMap<String, Option<V>>,
) {
    for (key, value) in changes {
        if let Some(value) = value {
            target.insert(key, value);
        } else {
            target.remove(&key);
        }
    }
}

fn recovery_bundle_evidence(
    checkpoint_source: Option<&str>,
    journal_source: Option<&str>,
) -> String {
    format!(
        "checkpoint\0{}\0journal\0{}",
        checkpoint_source.unwrap_or_default(),
        journal_source.unwrap_or_default()
    )
}

fn project_open_recovery_assessment_token(
    recovery_source: &str,
    current_manifest: &ProjectDiskManifest,
    current_root_fingerprint: &ProjectRootFingerprint,
) -> Result<String, String> {
    let current_manifest = serde_json::to_vec(current_manifest).map_err(|error| {
        format!("Manifestul recovery preflight nu poate fi serializat: {error}")
    })?;
    let current_root_fingerprint =
        serde_json::to_vec(current_root_fingerprint).map_err(|error| {
            format!("Fingerprintul recovery preflight nu poate fi serializat: {error}")
        })?;
    let mut hasher = Sha256::new();
    hasher.update(b"pana-project-open-recovery-v1\0");
    hasher.update(recovery_source.as_bytes());
    hasher.update(b"\0manifest\0");
    hasher.update(current_manifest);
    hasher.update(b"\0root-fingerprint\0");
    hasher.update(current_root_fingerprint);
    Ok(format!("{:x}", hasher.finalize()))
}

fn previous_root_identity_changed<R: Runtime>(
    app: &AppHandle<R>,
    current: &ProjectRootFingerprint,
) -> Result<Option<bool>, String> {
    let path = project_session_manifest_path(app, &current.canonical_path)?;
    let Some(source) = read_bounded_regular_utf8(
        &path,
        PROJECT_SESSION_MANIFEST_MAX_BYTES,
        "Manifestul ProjectSession anterior",
    )?
    else {
        return Ok(None);
    };
    let previous = match serde_json::from_str::<ProjectSessionSnapshot>(&source) {
        Ok(previous) if previous.project_root == current.canonical_path => previous,
        _ => return Ok(None),
    };
    match (
        previous.root_fingerprint.unix_device.as_deref(),
        previous.root_fingerprint.unix_inode.as_deref(),
        current.unix_device.as_deref(),
        current.unix_inode.as_deref(),
    ) {
        (Some(previous_device), Some(previous_inode), Some(device), Some(inode)) => {
            Ok(Some(previous_device != device || previous_inode != inode))
        }
        _ => Ok(None),
    }
}

fn read_project_open_recovery_decision_marker<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &str,
) -> Result<Option<ProjectOpenRecoveryDecisionMarker>, String> {
    let path = project_open_recovery_decision_path(app, project_root)?;
    let Some(source) = read_bounded_regular_utf8(
        &path,
        PROJECT_OPEN_RECOVERY_DECISION_MAX_BYTES,
        "Decizia project-open recovery",
    )?
    else {
        return Ok(None);
    };
    Ok(serde_json::from_str::<ProjectOpenRecoveryDecisionMarker>(&source).ok())
}

fn read_recovery_source(path: &Path) -> Result<Option<String>, String> {
    read_bounded_regular_utf8(
        path,
        PROJECT_WORKSPACE_RECOVERY_MAX_BYTES,
        "ProjectWorkspace recovery",
    )
}

fn read_recovery_journal_source(path: &Path) -> Result<Option<String>, String> {
    read_bounded_regular_utf8(
        path,
        PROJECT_WORKSPACE_RECOVERY_JOURNAL_MAX_BYTES,
        "Jurnalul ProjectWorkspace recovery",
    )
}

fn read_bounded_regular_utf8(
    path: &Path,
    max_bytes: u64,
    label: &str,
) -> Result<Option<String>, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "{label} nu poate citi metadata {}: {error}",
                path.display()
            ));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "{label} a refuzat un fișier symlink sau non-regular."
        ));
    }
    if metadata.len() > max_bytes {
        return Err(format!(
            "{label} are {} bytes, peste limita de {}.",
            metadata.len(),
            max_bytes
        ));
    }
    let mut source = String::new();
    fs::File::open(path)
        .map_err(|error| format!("{label} nu poate fi deschis: {error}"))?
        .take(max_bytes + 1)
        .read_to_string(&mut source)
        .map_err(|error| format!("{label} nu este UTF-8 valid: {error}"))?;
    if source.len() as u64 > max_bytes {
        return Err(format!("{label} a depășit limita în timpul citirii."));
    }
    Ok(Some(source))
}

fn validate_recovery_path(project_root: &Path, relative_path: &str) -> Result<(), String> {
    if relative_path.is_empty()
        || relative_path.starts_with('/')
        || relative_path.contains('\\')
        || relative_path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(format!(
            "ProjectWorkspace recovery conține path necanonic: {relative_path}."
        ));
    }
    if Path::new(relative_path)
        .file_name()
        .and_then(|name| name.to_str())
        == Some(".env")
    {
        return Err(
            "ProjectWorkspace recovery a refuzat .env; credentialele nu sunt recuperabile."
                .to_string(),
        );
    }
    resolve_project_write_path(project_root, relative_path)?;
    Ok(())
}

fn validate_workspace_recovery_paths(workspace: &ProjectWorkspace) -> Result<(), String> {
    let project_root = Path::new(&workspace.session.project_root);
    for path in workspace
        .documents
        .files
        .keys()
        .map(String::as_str)
        .chain(
            workspace
                .accepted_binary_resource_hashes
                .keys()
                .map(String::as_str),
        )
        .chain(workspace.binary_resources.keys().map(String::as_str))
        .chain(
            workspace
                .deleted_binary_resources
                .iter()
                .map(String::as_str),
        )
        .chain(workspace.history.recovery_paths())
    {
        validate_recovery_path(project_root, path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, env, path::PathBuf, sync::atomic::AtomicBool};

    use crate::{
        app_home::{ensure_app_home, TEST_APP_ENV_LOCK},
        js::{MotionCustomCode, MotionDocument, PageJsDraftStore},
        kernel::{
            file_buffer_store::{
                hash_bytes, hash_text, FileBufferBaseline, FileBufferChangeCoordinateSpace,
                FileBufferChangeSetInput, FileBufferEntry, FileBufferMutationExpectation,
                FileBufferStore, FileBufferStoreLimits, FileBufferTextChange, TextBufferLanguage,
                TextBufferRole,
            },
            project_session::{
                fingerprint_project_root, persist_project_session_open, ProjectRootFingerprint,
                ProjectSessionScanSummary, ProjectSessionSnapshot,
            },
            project_workspace::{
                ProjectWorkspaceIdentity, WorkspaceDocumentMutation, WorkspaceMutationMetadata,
                WorkspaceResourceMutation, PROJECT_WORKSPACE_SCHEMA_VERSION,
            },
            taxonomy_mutation::{
                plan_taxonomy_mutation, stage_taxonomy_mutation, TaxonomyDefinitionInput,
                TaxonomyMutationInput, TaxonomyMutationOperation,
            },
        },
        project::{read_project_disk_manifest, AcceptedProjectDiskManifest},
        project_model::test_support::ProjectModelTestFixture,
        source_graph::{build_source_graph_from_workspace_projection, build_taxonomy_catalog},
        state::AppState,
        versioning::{VersionNetworkOperationKind, VersionNetworkOperationLease},
    };

    use super::*;

    #[test]
    fn recovery_paths_reject_root_and_nested_env_files() {
        let root = Path::new("/tmp/panastudio-recovery-env-boundary");
        for path in [".env", "config/.env"] {
            let error = validate_recovery_path(root, path).unwrap_err();
            assert!(error.contains("credentialele nu sunt recuperabile"));
        }
    }

    #[test]
    fn active_remote_operation_rejects_workspace_mutation_and_save_without_state_change() {
        let root = std::env::temp_dir().join(format!(
            "pana-version-network-workspace-gate-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(root.join("zola.toml"), "base_url = '/'\n").unwrap();
        fs::write(root.join("templates/index.html"), "<main>baseline</main>\n").unwrap();
        let root = root.canonicalize().unwrap();
        let session = test_session(&root, &root.join(".test-session"));
        let mut workspace = workspace(&root, &session);
        let before_revision = workspace.revision;
        let before_generation = workspace.accepted_disk.generation;
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock app");
        app.handle().manage(AppState::default());
        let state = app.state::<AppState>();
        let operation_id = "fetch-workspace-gate-12345678";
        state
            .versioning_network_operation
            .begin(
                VersionNetworkOperationLease {
                    operation_id: operation_id.to_string(),
                    project_root: workspace.session.project_root.clone(),
                    session_id: workspace.runtime_session_id(),
                    kind: VersionNetworkOperationKind::Fetch,
                    workspace_revision: workspace.revision,
                    disk_generation: workspace.accepted_disk.generation,
                    accepted_disk: workspace.accepted_disk.clone(),
                    expected_status_token: "status-before".to_string(),
                    expected_head_oid: None,
                },
                Arc::new(AtomicBool::new(false)),
            )
            .unwrap();

        let mutation_ran = Cell::new(false);
        let mutation_error =
            commit_project_workspace_session_mutation(app.handle(), &mut workspace, |_candidate| {
                mutation_ran.set(true);
                Ok(())
            })
            .unwrap_err();
        assert!(mutation_error.contains(operation_id), "{mutation_error}");
        assert!(!mutation_ran.get());

        let save_identity = identity(&workspace);
        let save_error = crate::kernel::project_workspace::save_project_workspace(
            app.handle(),
            &root,
            &mut workspace,
            &save_identity,
        )
        .unwrap_err()
        .to_string();
        assert!(save_error.contains(operation_id), "{save_error}");
        assert_eq!(workspace.revision, before_revision);
        assert_eq!(workspace.accepted_disk.generation, before_generation);
        assert!(!workspace.is_dirty());

        state.versioning_network_operation.abandon(operation_id);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn recreated_project_path_requires_explicit_recovery_decision_and_preserves_drafts() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = std::env::temp_dir().join(format!(
            "pana-project-open-recovery-recreated-root-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let project_path = root.join("project");
        fs::create_dir_all(project_path.join("templates")).unwrap();
        fs::write(project_path.join("zola.toml"), "base_url = '/'\n").unwrap();
        fs::write(
            project_path.join("templates/index.html"),
            "<main>baseline</main>\n",
        )
        .unwrap();
        let project = project_path.canonicalize().unwrap();
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock app");
        let app_home = ensure_app_home(app.handle()).unwrap();
        let session_dir = PathBuf::from(&app_home.sessions_dir).join("open-recovery-session");
        fs::create_dir_all(&session_dir).unwrap();
        let mut session = test_session(&project, &session_dir);
        session.root_fingerprint = fingerprint_project_root(&project).unwrap();
        persist_project_session_open(app.handle(), &session).unwrap();

        let mut original = workspace(&project, &session);
        original
            .stage_resource_texts(
                &identity(&original),
                WorkspaceMutationMetadata {
                    label: "Create unsaved template".to_string(),
                    source: "test.project_open_recovery".to_string(),
                    coalesce_key: None,
                    transaction_id: None,
                },
                vec![super::super::WorkspaceResourceMutation {
                    relative_path: "templates/draft.html".to_string(),
                    contents: "<main>unsaved</main>\n".to_string(),
                    create_only: true,
                }],
                50,
            )
            .unwrap();
        persist_project_workspace_recovery(app.handle(), &original).unwrap();
        let recovery_path =
            project_workspace_recovery_path(app.handle(), &session.project_root).unwrap();
        assert!(recovery_path.is_file());

        // Keep the old directory inode alive while the same canonical path is
        // recreated, so Unix cannot immediately recycle the exact identity.
        let _held_old_root = fs::File::open(&project).unwrap();
        fs::remove_dir_all(&project).unwrap();
        fs::create_dir_all(&project).unwrap();
        let current_manifest = read_project_disk_manifest(&project).unwrap();
        let current_fingerprint = fingerprint_project_root(&project).unwrap();
        let first = inspect_project_workspace_recovery_for_open(
            app.handle(),
            &project,
            &current_manifest,
            &current_fingerprint,
        )
        .unwrap();
        assert_eq!(first.status, ProjectOpenRecoveryStatus::DecisionRequired);
        assert_eq!(
            first.conflict_reason,
            Some(ProjectOpenRecoveryConflictReason::ProjectRootReplaced)
        );
        assert_eq!(first.root_identity_changed, Some(true));
        assert_eq!(first.dirty_document_count, 1);
        assert_eq!(first.undo_count, 1);
        assert!(resolve_project_open_recovery(&first, None).is_err());
        assert!(recovery_path.is_file(), "preflight must be read-only");

        fs::write(project.join("new-project.txt"), "current project\n").unwrap();
        let changed_manifest = read_project_disk_manifest(&project).unwrap();
        let changed_fingerprint = fingerprint_project_root(&project).unwrap();
        let changed = inspect_project_workspace_recovery_for_open(
            app.handle(),
            &project,
            &changed_manifest,
            &changed_fingerprint,
        )
        .unwrap();
        assert!(require_project_open_recovery_assessment_unchanged(&first, &changed).is_err());
        let stale_decision = ProjectOpenRecoveryDecisionInput {
            action: ProjectOpenRecoveryDecisionAction::Abandon,
            assessment_token: first.assessment_token.clone().unwrap(),
        };
        assert!(resolve_project_open_recovery(&changed, Some(&stale_decision)).is_err());

        let decision = ProjectOpenRecoveryDecisionInput {
            action: ProjectOpenRecoveryDecisionAction::Abandon,
            assessment_token: changed.assessment_token.clone().unwrap(),
        };
        assert_eq!(
            resolve_project_open_recovery(&changed, Some(&decision)).unwrap(),
            ProjectOpenRecoveryResolution::ExplicitAbandon
        );
        persist_project_open_recovery_abandonment(app.handle(), &changed, &decision).unwrap();
        let marked = inspect_project_workspace_recovery_for_open(
            app.handle(),
            &project,
            &changed_manifest,
            &changed_fingerprint,
        )
        .unwrap();
        assert_eq!(marked.status, ProjectOpenRecoveryStatus::Abandoned);
        assert_eq!(
            resolve_project_open_recovery(&marked, None).unwrap(),
            ProjectOpenRecoveryResolution::Skip
        );
        assert!(
            recovery_path.is_file(),
            "the marker must not delete recovery bytes"
        );

        clear_project_workspace_recovery(app.handle(), &session.project_root).unwrap();
        clear_project_open_recovery_decision(app.handle(), &session.project_root).unwrap();
        drop(app);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn binary_resource_and_history_survive_recovery_roundtrip() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = std::env::temp_dir().join(format!(
            "pana-project-workspace-binary-recovery-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let project = root.join("project");
        fs::create_dir_all(project.join("static/fonturi/inter")).unwrap();
        fs::write(project.join("zola.toml"), "base_url = '/'\n").unwrap();
        let project = project.canonicalize().unwrap();
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock app");
        let app_home = ensure_app_home(app.handle()).unwrap();
        let session_dir = PathBuf::from(&app_home.sessions_dir).join("binary-recovery-session");
        fs::create_dir_all(&session_dir).unwrap();
        let session = test_session(&project, &session_dir);

        let mut original = workspace(&project, &session);
        let relative_path = "static/fonturi/inter/inter-regular.woff2";
        let bytes = vec![0x77, 0x4f, 0x46, 0x32, 21, 22, 23];
        original
            .stage_binary_resource_creates(
                &identity(&original),
                WorkspaceMutationMetadata {
                    label: "Download Inter".to_string(),
                    source: "test.binary_recovery".to_string(),
                    coalesce_key: None,
                    transaction_id: None,
                },
                vec![WorkspaceBinaryResource::new(relative_path, bytes.clone())],
                20,
            )
            .unwrap();
        persist_project_workspace_recovery(app.handle(), &original).unwrap();

        let mut restored = workspace(&project, &session);
        let status = restore_project_workspace_recovery(app.handle(), &mut restored).unwrap();
        assert_eq!(status, ProjectWorkspaceRecoveryStatus::Restored);
        assert_eq!(restored.schema_version, PROJECT_WORKSPACE_SCHEMA_VERSION);
        assert_eq!(restored.revision, original.revision);
        assert_eq!(
            restored.staged_binary_resource(relative_path),
            Some(bytes.as_slice())
        );
        assert_eq!(restored.snapshot().history.undo_count, 1);
        assert!(!project.join(relative_path).exists());

        restored.undo(&identity(&restored), 21).unwrap();
        assert!(restored.staged_binary_resource(relative_path).is_none());
        assert!(!restored.is_dirty());
        assert!(!project.join(relative_path).exists());

        drop(app);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn taxonomy_composite_mutation_survives_recovery_as_one_history_entry() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = std::env::temp_dir().join(format!(
            "pana-project-workspace-taxonomy-recovery-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let project_path = root.join("project");
        fs::create_dir_all(project_path.join("content")).unwrap();
        let config = "base_url = \"https://example.test\"\ntaxonomies = [{ name = \"tags\" }]\n";
        let page = "+++\ntitle = \"Unu\"\n[taxonomies]\ntags = [\"Rust\"]\n+++\n\nCorp\n";
        fs::write(project_path.join("zola.toml"), config).unwrap();
        fs::write(project_path.join("content/unu.md"), page).unwrap();
        let project = project_path.canonicalize().unwrap();
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock app");
        let app_home = ensure_app_home(app.handle()).unwrap();
        let session_dir = PathBuf::from(&app_home.sessions_dir).join("taxonomy-recovery-session");
        fs::create_dir_all(&session_dir).unwrap();
        let session = test_session(&project, &session_dir);

        let mut original = workspace(&project, &session);
        load_taxonomy_recovery_baselines(&mut original, &project, config, page);
        let projection = original.capture_projection_snapshot().unwrap();
        let graph = build_source_graph_from_workspace_projection(&project, &projection).unwrap();
        let catalog = build_taxonomy_catalog(&graph, "zola.toml", config);
        let planned = plan_taxonomy_mutation(
            &graph,
            &catalog,
            &projection.source_texts,
            &TaxonomyMutationInput {
                operation: TaxonomyMutationOperation::UpsertDefinition {
                    original_name: Some("tags".to_string()),
                    original_language: Some("en".to_string()),
                    definition: TaxonomyDefinitionInput {
                        name: "topics".to_string(),
                        language: "en".to_string(),
                        render: true,
                        feed: false,
                        paginate_by: None,
                        paginate_path: None,
                    },
                },
            },
        )
        .unwrap();
        let (_, receipt) = stage_taxonomy_mutation(&mut original, planned, 20).unwrap();
        assert_eq!(receipt.history.undo_count, 1);
        assert_eq!(receipt.touched_files.len(), 2);
        persist_project_workspace_recovery(app.handle(), &original).unwrap();

        let mut restored = workspace(&project, &session);
        load_taxonomy_recovery_baselines(&mut restored, &project, config, page);
        assert_eq!(
            restore_project_workspace_recovery(app.handle(), &mut restored).unwrap(),
            ProjectWorkspaceRecoveryStatus::Restored
        );
        assert_eq!(restored.snapshot().history.undo_count, 1);
        assert!(restored
            .documents
            .text_for("zola.toml")
            .unwrap()
            .contains("name = \"topics\""));
        assert!(restored
            .documents
            .text_for("content/unu.md")
            .unwrap()
            .contains("topics = [\"Rust\"]"));

        restored.undo(&identity(&restored), 21).unwrap();
        assert_eq!(
            restored.documents.text_for("zola.toml").as_deref(),
            Some(config)
        );
        assert_eq!(
            restored.documents.text_for("content/unu.md").as_deref(),
            Some(page)
        );

        clear_project_workspace_recovery(app.handle(), &session.project_root).unwrap();
        drop(app);
        fs::remove_dir_all(root).unwrap();
    }

    fn load_taxonomy_recovery_baselines(
        workspace: &mut ProjectWorkspace,
        project: &Path,
        config: &str,
        page: &str,
    ) {
        for (relative_path, source, language, role) in [
            (
                "zola.toml",
                config,
                TextBufferLanguage::Toml,
                TextBufferRole::Config,
            ),
            (
                "content/unu.md",
                page,
                TextBufferLanguage::Markdown,
                TextBufferRole::Page,
            ),
        ] {
            workspace.documents.insert_loaded_file(FileBufferEntry {
                relative_path: relative_path.to_string(),
                absolute_path: project.join(relative_path).to_string_lossy().into_owned(),
                language,
                role,
                baseline: FileBufferBaseline {
                    hash: hash_text(source),
                    modified_ms: 1,
                    size: source.len() as u64,
                    readonly: false,
                },
                baseline_text: source.to_string().into(),
                draft: None,
                revision: 1,
            });
        }
    }

    #[test]
    fn accepted_binary_hash_survives_recovery_and_redo_normalizes_clean() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = std::env::temp_dir().join(format!(
            "pana-project-workspace-binary-baseline-recovery-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let project = root.join("project");
        fs::create_dir_all(project.join("static/fonturi/inter")).unwrap();
        fs::write(project.join("zola.toml"), "base_url = '/'\n").unwrap();
        let project = project.canonicalize().unwrap();
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock app");
        let app_home = ensure_app_home(app.handle()).unwrap();
        let session_dir =
            PathBuf::from(&app_home.sessions_dir).join("binary-baseline-recovery-session");
        fs::create_dir_all(&session_dir).unwrap();
        let session = test_session(&project, &session_dir);

        let mut original = workspace(&project, &session);
        let relative_path = "static/fonturi/inter/inter-regular.woff2";
        let bytes = vec![0x77, 0x4f, 0x46, 0x32, 31, 32, 33];
        original
            .stage_binary_resource_creates(
                &identity(&original),
                WorkspaceMutationMetadata {
                    label: "Download Inter".to_string(),
                    source: "test.binary_baseline_recovery".to_string(),
                    coalesce_key: None,
                    transaction_id: None,
                },
                vec![WorkspaceBinaryResource::new(relative_path, bytes.clone())],
                30,
            )
            .unwrap();

        // Simulate the already-tested Save acceptance boundary, then retain
        // its History while Undo creates a session-only delete.
        fs::write(project.join(relative_path), &bytes).unwrap();
        let accepted = original
            .accepted_disk
            .next(
                &original.runtime_session_id(),
                &original.session.project_root,
                read_project_disk_manifest(&project).unwrap(),
            )
            .unwrap();
        let save_identity = identity(&original);
        let saved_documents = original.documents.clone();
        let accepted_page_js = original.accepted_page_js.clone();
        original
            .accept_saved_documents(&save_identity, saved_documents, accepted_page_js, accepted)
            .unwrap();
        assert_eq!(
            original.accepted_binary_resource_hashes.get(relative_path),
            Some(&hash_bytes(&bytes))
        );
        original.undo(&identity(&original), 31).unwrap();
        assert!(original.is_dirty());
        persist_project_workspace_recovery(app.handle(), &original).unwrap();

        let mut restored = workspace(&project, &session);
        assert_eq!(
            restore_project_workspace_recovery(app.handle(), &mut restored).unwrap(),
            ProjectWorkspaceRecoveryStatus::Restored
        );
        assert_eq!(
            restored.accepted_binary_resource_hashes.get(relative_path),
            Some(&hash_bytes(&bytes))
        );
        restored.redo(&identity(&restored), 32).unwrap();
        assert!(!restored.is_dirty());
        assert!(restored.staged_binary_resource(relative_path).is_none());
        assert!(restored.deleted_binary_resources().next().is_none());
        assert_eq!(fs::read(project.join(relative_path)).unwrap(), bytes);

        drop(app);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recovery_failure_never_publishes_the_candidate_workspace() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = std::env::temp_dir().join(format!(
            "pana-project-workspace-recovery-transaction-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let project = root.join("project");
        fs::create_dir_all(project.join("templates")).unwrap();
        fs::write(project.join("zola.toml"), "base_url = '/'\n").unwrap();
        let project = project.canonicalize().unwrap();
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock app");
        ensure_app_home(app.handle()).unwrap();
        let session_dir = root.join("session");
        fs::create_dir_all(&session_dir).unwrap();
        let session = test_session(&project, &session_dir);
        let mut live = workspace(&project, &session);
        let before = live.snapshot();

        let recovery_path =
            project_workspace_recovery_path(app.handle(), &session.project_root).unwrap();
        fs::create_dir_all(&recovery_path).unwrap();
        let error =
            commit_project_workspace_session_mutation(app.handle(), &mut live, |candidate| {
                candidate.stage_resource_texts(
                    &identity(candidate),
                    WorkspaceMutationMetadata {
                        label: "Create candidate".to_string(),
                        source: "test.recovery_transaction".to_string(),
                        coalesce_key: None,
                        transaction_id: None,
                    },
                    vec![super::super::WorkspaceResourceMutation {
                        relative_path: "templates/candidate.html".to_string(),
                        contents: "<main>candidate</main>".to_string(),
                        create_only: true,
                    }],
                    40,
                )
            })
            .unwrap_err();

        assert!(
            !error.trim().is_empty(),
            "eșecul persistenței recovery trebuie să păstreze un diagnostic"
        );
        let after = live.snapshot();
        assert_eq!(after.revision, before.revision);
        assert_eq!(after.dirty, before.dirty);
        assert_eq!(after.history.undo_count, before.history.undo_count);
        assert!(live
            .documents
            .text_for("templates/candidate.html")
            .is_none());

        drop(app);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn prepared_candidate_cas_never_overwrites_a_newer_workspace_revision() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = std::env::temp_dir().join(format!(
            "pana-project-workspace-prepared-cas-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let project = root.join("project");
        fs::create_dir_all(project.join("templates")).unwrap();
        fs::write(project.join("zola.toml"), "base_url = '/'\n").unwrap();
        let project = project.canonicalize().unwrap();
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock app");
        ensure_app_home(app.handle()).unwrap();
        let session_dir = root.join("session");
        fs::create_dir_all(&session_dir).unwrap();
        let session = test_session(&project, &session_dir);
        let mut live = workspace(&project, &session);
        let base_revision = live.revision;
        let mut candidate = live.fork_candidate();
        candidate
            .stage_resource_texts(
                &identity(&candidate),
                WorkspaceMutationMetadata {
                    label: "Detached candidate".to_string(),
                    source: "test.prepared_cas".to_string(),
                    coalesce_key: None,
                    transaction_id: None,
                },
                vec![super::super::WorkspaceResourceMutation {
                    relative_path: "templates/candidate.html".to_string(),
                    contents: "<main>candidate</main>".to_string(),
                    create_only: true,
                }],
                41,
            )
            .unwrap();
        live.stage_resource_texts(
            &identity(&live),
            WorkspaceMutationMetadata {
                label: "Concurrent winner".to_string(),
                source: "test.prepared_cas".to_string(),
                coalesce_key: None,
                transaction_id: None,
            },
            vec![super::super::WorkspaceResourceMutation {
                relative_path: "templates/winner.html".to_string(),
                contents: "<main>winner</main>".to_string(),
                create_only: true,
            }],
            42,
        )
        .unwrap();
        let winning_revision = live.revision;

        let error = publish_prepared_project_workspace_candidate(
            app.handle(),
            &mut live,
            base_revision,
            candidate,
            ProjectWorkspacePreviewProjection::Required,
            0,
            0,
            Instant::now(),
        )
        .unwrap_err();
        assert!(error.contains("CAS"));
        assert_eq!(live.revision, winning_revision);
        assert!(live.documents.text_for("templates/winner.html").is_some());
        assert!(live
            .documents
            .text_for("templates/candidate.html")
            .is_none());

        drop(app);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn session_mutations_append_incremental_recovery_and_restore_the_latest_revision() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = std::env::temp_dir().join(format!(
            "pana-project-workspace-incremental-recovery-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let project = root.join("project");
        fs::create_dir_all(project.join("templates")).unwrap();
        fs::write(project.join("zola.toml"), "base_url = '/'\n").unwrap();
        let project = project.canonicalize().unwrap();
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock app");
        ensure_app_home(app.handle()).unwrap();
        let session_dir = root.join("session");
        fs::create_dir_all(&session_dir).unwrap();
        let session = test_session(&project, &session_dir);
        let mut live = workspace(&project, &session);
        persist_project_workspace_recovery(app.handle(), &live).unwrap();

        for (index, name) in ["one", "two"].into_iter().enumerate() {
            commit_project_workspace_session_mutation(app.handle(), &mut live, |candidate| {
                candidate.stage_resource_texts(
                    &identity(candidate),
                    WorkspaceMutationMetadata {
                        label: format!("Create {name}"),
                        source: "test.incremental_recovery".to_string(),
                        coalesce_key: None,
                        transaction_id: Some(format!("incremental-{name}")),
                    },
                    vec![super::super::WorkspaceResourceMutation {
                        relative_path: format!("templates/{name}.html"),
                        contents: format!("<main>{name}</main>\n"),
                        create_only: true,
                    }],
                    50 + index as u128,
                )
            })
            .unwrap();
        }
        commit_project_workspace_session_mutation(app.handle(), &mut live, |candidate| {
            candidate.undo(&identity(candidate), 60)
        })
        .unwrap();

        let checkpoint_path =
            project_workspace_recovery_path(app.handle(), &session.project_root).unwrap();
        let checkpoint_source = read_recovery_source(&checkpoint_path).unwrap().unwrap();
        assert_eq!(
            parse_project_workspace_recovery_envelope(&checkpoint_source)
                .unwrap()
                .payload
                .revision,
            0
        );
        let journal_path =
            project_workspace_recovery_journal_path(app.handle(), &session.project_root).unwrap();
        let journal_source = read_recovery_journal_source(&journal_path)
            .unwrap()
            .unwrap();
        assert_eq!(journal_source.lines().count(), 3);

        let mut restored = workspace(&project, &session);
        assert_eq!(
            restore_project_workspace_recovery(app.handle(), &mut restored).unwrap(),
            ProjectWorkspaceRecoveryStatus::Restored
        );
        assert_eq!(restored.revision, 3);
        assert_eq!(restored.snapshot().history.undo_count, 1);
        assert_eq!(restored.snapshot().history.redo_count, 1);
        assert_eq!(
            restored.documents.text_for("templates/one.html").as_deref(),
            Some("<main>one</main>\n")
        );
        assert!(restored.documents.text_for("templates/two.html").is_none());

        clear_project_workspace_recovery(app.handle(), &session.project_root).unwrap();
        drop(app);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn motion_draft_recovery_regenerates_portable_source_and_runtime_without_disk_writes() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = std::env::temp_dir().join(format!(
            "pana-project-workspace-motion-recovery-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("zola.toml"), "base_url = '/'\n").unwrap();
        let project = project.canonicalize().unwrap();
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock app");
        ensure_app_home(app.handle()).unwrap();
        let session_dir = root.join("session");
        fs::create_dir_all(&session_dir).unwrap();
        let session = test_session(&project, &session_dir);
        let mut live = workspace(&project, &session);
        persist_project_workspace_recovery(app.handle(), &live).unwrap();

        commit_project_workspace_session_mutation(app.handle(), &mut live, |candidate| {
            candidate.stage_resource_texts(
                &identity(candidate),
                WorkspaceMutationMetadata {
                    label: "Create Motion template".to_string(),
                    source: "test.motion_recovery".to_string(),
                    coalesce_key: None,
                    transaction_id: Some("motion-template".to_string()),
                },
                vec![super::super::WorkspaceResourceMutation {
                    relative_path: "templates/index.html".to_string(),
                    contents: "<main data-anim=\"hero\">Hero</main>\n".to_string(),
                    create_only: true,
                }],
                70,
            )
        })
        .unwrap();
        commit_project_workspace_session_mutation(app.handle(), &mut live, |candidate| {
            candidate.stage_page_js(
                &identity(candidate),
                WorkspaceMutationMetadata {
                    label: "Create Motion".to_string(),
                    source: "test.motion_recovery".to_string(),
                    coalesce_key: None,
                    transaction_id: Some("motion-draft".to_string()),
                },
                PageJsDraftStageInput {
                    template_path: "templates/index.html".to_string(),
                    expected_project_root: candidate.session.project_root.clone(),
                    expected_session_id: candidate.runtime_session_id(),
                    base_config: PageJsConfig::default(),
                    current_config: PageJsConfig {
                        motion: Some(MotionDocument {
                            custom_code: vec![MotionCustomCode {
                                id: "recovered-motion".to_string(),
                                name: "Recovered Motion".to_string(),
                                enabled: true,
                                code: "window.__recoveredMotion=true".to_string(),
                            }],
                            ..MotionDocument::default()
                        }),
                    },
                    cachebust_assets: false,
                    source: Some("test.motion_recovery".to_string()),
                    coalesce_key: None,
                    transaction_id: Some("motion-draft".to_string()),
                },
                71,
            )
        })
        .unwrap();

        let mut restored = workspace(&project, &session);
        assert_eq!(
            restore_project_workspace_recovery(app.handle(), &mut restored).unwrap(),
            ProjectWorkspaceRecoveryStatus::Restored
        );
        assert_eq!(restored.revision, 2);
        assert_eq!(restored.page_js.dirty_count(), 1);
        let projection = restored.capture_projection_snapshot().unwrap();
        assert!(projection
            .source_texts
            .get(".panastudio/motion/templates/index.json")
            .is_some_and(|source| source.contains("recovered-motion")));
        assert!(projection
            .source_texts
            .get("static/js/pana-index.js")
            .is_some_and(|source| source.contains("/js/vendor/animejs-4.4.1/index.js")));
        assert!(projection
            .source_texts
            .contains_key("static/js/vendor/animejs-4.4.1/index.js"));
        assert!(!projection
            .source_texts
            .contains_key("static/js/pana-motion-runtime.js"));
        assert!(!project
            .join(".panastudio/motion/templates/index.json")
            .exists());
        assert!(!project.join("static/js/pana-index.js").exists());

        clear_project_workspace_recovery(app.handle(), &session.project_root).unwrap();
        drop(app);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn incremental_recovery_cost_scales_with_the_delta_not_the_workspace() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = std::env::temp_dir().join(format!(
            "pana-project-workspace-recovery-cost-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let small = recovery_serialization_measurement(&root, "small", 8, 4 * 1024);
        let large = recovery_serialization_measurement(&root, "large", 96, 32 * 1024);
        eprintln!(
            "[Pană Studio][perf] recovery_delta small_checkpoint_bytes={} small_journal_bytes={} small_checkpoint_us={} small_journal_us={} large_checkpoint_bytes={} large_journal_bytes={} large_checkpoint_us={} large_journal_us={}",
            small.0,
            small.1,
            small.2,
            small.3,
            large.0,
            large.1,
            large.2,
            large.3,
        );
        assert!(small.1 < small.0);
        assert!(large.1.saturating_mul(20) < large.0);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "performance baseline; run explicitly with --ignored --nocapture"]
    fn project_workspace_runtime_cost_baseline() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = std::env::temp_dir().join(format!(
            "pana-project-workspace-runtime-cost-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock app");
        ensure_app_home(app.handle()).unwrap();

        let small = workspace_runtime_measurement(app.handle(), &root, "small", 8, 4 * 1024);
        let large = workspace_runtime_measurement(app.handle(), &root, "large", 96, 32 * 1024);
        eprintln!(
            "[Pană Studio][perf] project_workspace_runtime \
small_documents_baseline_bytes={} small_accepted_baseline_bytes={} small_workspace_snapshot_bytes={} small_file_buffer_snapshot_bytes={} small_duplicate_bootstrap_bytes={} small_candidate_clone_us={} small_snapshot_us={} small_project_model_clone_us={} small_file_buffer_mutation_us={} small_set_transaction_us={} small_apply_transaction_us={} small_clear_transaction_us={} small_composite_transaction_us={} small_undo_transaction_us={} small_redo_transaction_us={} \
large_documents_baseline_bytes={} large_accepted_baseline_bytes={} large_workspace_snapshot_bytes={} large_file_buffer_snapshot_bytes={} large_duplicate_bootstrap_bytes={} large_candidate_clone_us={} large_snapshot_us={} large_project_model_clone_us={} large_file_buffer_mutation_us={} large_set_transaction_us={} large_apply_transaction_us={} large_clear_transaction_us={} large_composite_transaction_us={} large_undo_transaction_us={} large_redo_transaction_us={}",
            small.documents_baseline_bytes,
            small.accepted_baseline_bytes,
            small.workspace_snapshot_bytes,
            small.file_buffer_snapshot_bytes,
            small.duplicate_bootstrap_bytes,
            small.workspace_clone_us,
            small.snapshot_us,
            small.project_model_clone_us,
            small.file_buffer_mutation_us,
            small.set_transaction_us,
            small.apply_transaction_us,
            small.clear_transaction_us,
            small.composite_transaction_us,
            small.undo_transaction_us,
            small.redo_transaction_us,
            large.documents_baseline_bytes,
            large.accepted_baseline_bytes,
            large.workspace_snapshot_bytes,
            large.file_buffer_snapshot_bytes,
            large.duplicate_bootstrap_bytes,
            large.workspace_clone_us,
            large.snapshot_us,
            large.project_model_clone_us,
            large.file_buffer_mutation_us,
            large.set_transaction_us,
            large.apply_transaction_us,
            large.clear_transaction_us,
            large.composite_transaction_us,
            large.undo_transaction_us,
            large.redo_transaction_us,
        );
        assert_eq!(
            small.documents_baseline_bytes,
            small.accepted_baseline_bytes
        );
        assert_eq!(
            large.documents_baseline_bytes,
            large.accepted_baseline_bytes
        );
        assert!(small.duplicate_bootstrap_bytes > small.workspace_snapshot_bytes);
        assert!(large.duplicate_bootstrap_bytes > large.workspace_snapshot_bytes);
        assert!(large.workspace_snapshot_bytes > small.workspace_snapshot_bytes);

        fs::remove_dir_all(root).unwrap();
    }

    struct WorkspaceRuntimeMeasurement {
        documents_baseline_bytes: usize,
        accepted_baseline_bytes: usize,
        workspace_snapshot_bytes: usize,
        file_buffer_snapshot_bytes: usize,
        duplicate_bootstrap_bytes: usize,
        workspace_clone_us: u128,
        snapshot_us: u128,
        project_model_clone_us: u128,
        file_buffer_mutation_us: u128,
        set_transaction_us: u128,
        apply_transaction_us: u128,
        clear_transaction_us: u128,
        composite_transaction_us: u128,
        undo_transaction_us: u128,
        redo_transaction_us: u128,
    }

    fn workspace_runtime_measurement<R: Runtime>(
        app: &AppHandle<R>,
        root: &Path,
        label: &str,
        file_count: usize,
        bytes_per_file: usize,
    ) -> WorkspaceRuntimeMeasurement {
        let project = root.join(label);
        fs::create_dir_all(project.join("templates")).unwrap();
        fs::write(
            project.join("zola.toml"),
            "base_url = 'http://example.test'\n",
        )
        .unwrap();
        let project = project.canonicalize().unwrap();
        let session = test_session(&project, &root.join(format!("{label}-runtime-session")));
        let mut workspace = workspace(&project, &session);
        let mut model_fixture = ProjectModelTestFixture::new(&project).unwrap();
        model_fixture.source("zola.toml", "base_url = 'http://example.test'\n");

        for index in 0..file_count {
            let relative_path = format!("templates/page-{index:03}.html");
            let prefix = format!("<main data-index=\"{index}\">");
            let source = format!(
                "{prefix}{}</main>\n",
                "x".repeat(bytes_per_file.saturating_sub(prefix.len() + 9))
            );
            workspace.documents.insert_loaded_file(FileBufferEntry {
                relative_path: relative_path.clone(),
                absolute_path: project.join(&relative_path).to_string_lossy().into_owned(),
                language: TextBufferLanguage::Html,
                role: TextBufferRole::Template,
                baseline: FileBufferBaseline {
                    hash: hash_text(&source),
                    modified_ms: 1,
                    size: source.len() as u64,
                    readonly: false,
                },
                baseline_text: source.clone().into(),
                draft: None,
                revision: 1,
            });
            model_fixture.source(relative_path, source);
        }
        workspace.accepted_documents = workspace.documents.files.clone();
        workspace.project_model = Some(model_fixture.build_model().unwrap().into());
        workspace.project_model_source_revision = Some(workspace.revision);
        let documents_baseline_bytes = workspace
            .documents
            .files
            .values()
            .map(|entry| entry.baseline_text.len())
            .sum();
        let accepted_baseline_bytes = workspace
            .accepted_documents
            .values()
            .map(|entry| entry.baseline_text.len())
            .sum();

        let mut workspace_clone_us = u128::MAX;
        let mut snapshot_us = u128::MAX;
        let mut project_model_clone_us = u128::MAX;
        let mut file_buffer_mutation_us = u128::MAX;
        let mut workspace_snapshot_bytes = 0;
        let mut file_buffer_snapshot_bytes = 0;
        let mutation_path = "templates/page-000.html";
        let mutation_snapshot = workspace.documents.text_snapshot(mutation_path).unwrap();
        let mutation_expectation = FileBufferMutationExpectation {
            expected_revision: mutation_snapshot.revision,
            expected_hash: mutation_snapshot.hash,
        };
        let mutation_contents = format!("{}!", mutation_snapshot.text);
        for _ in 0..5 {
            let clone_started = Instant::now();
            std::hint::black_box(workspace.fork_candidate());
            workspace_clone_us = workspace_clone_us.min(clone_started.elapsed().as_micros());

            let model_clone_started = Instant::now();
            std::hint::black_box(workspace.project_model.as_ref().unwrap().clone());
            project_model_clone_us =
                project_model_clone_us.min(model_clone_started.elapsed().as_micros());

            let snapshot_started = Instant::now();
            let workspace_snapshot = workspace.snapshot();
            workspace_snapshot_bytes = serde_json::to_vec(&workspace_snapshot).unwrap().len();
            snapshot_us = snapshot_us.min(snapshot_started.elapsed().as_micros());
            file_buffer_snapshot_bytes = serde_json::to_vec(&workspace.documents.snapshot())
                .unwrap()
                .len();

            let mut mutation_candidate = workspace.fork_candidate();
            let contents = mutation_contents.clone();
            let mutation_started = Instant::now();
            mutation_candidate
                .documents
                .set_draft_if_current(mutation_path, contents, &mutation_expectation, 2)
                .unwrap();
            file_buffer_mutation_us =
                file_buffer_mutation_us.min(mutation_started.elapsed().as_micros());
        }

        persist_project_workspace_recovery(app, &workspace).unwrap();
        let transaction_metadata = |source: &str| WorkspaceMutationMetadata {
            label: "Runtime baseline".to_string(),
            source: source.to_string(),
            coalesce_key: None,
            transaction_id: None,
        };

        let set_started = Instant::now();
        commit_project_workspace_session_mutation(app, &mut workspace, |candidate| {
            candidate.stage_document_texts(
                &identity(candidate),
                transaction_metadata("perf.set"),
                vec![WorkspaceDocumentMutation {
                    relative_path: mutation_path.to_string(),
                    contents: mutation_contents.clone(),
                }],
                3,
            )
        })
        .unwrap();
        let set_transaction_us = set_started.elapsed().as_micros();

        let apply_snapshot = workspace.documents.text_snapshot(mutation_path).unwrap();
        let apply_started = Instant::now();
        commit_project_workspace_session_mutation(app, &mut workspace, |candidate| {
            candidate.apply_document_changeset(
                &identity(candidate),
                transaction_metadata("perf.apply"),
                FileBufferChangeSetInput {
                    relative_path: mutation_path.to_string(),
                    base_revision: Some(apply_snapshot.revision),
                    base_hash: Some(apply_snapshot.hash.clone()),
                    coordinate_space: FileBufferChangeCoordinateSpace::Utf16,
                    source: Some("perf.apply".to_string()),
                    changes: vec![FileBufferTextChange {
                        from: 0,
                        to: 0,
                        insert: "!".to_string(),
                    }],
                },
                4,
            )
        })
        .unwrap();
        let apply_transaction_us = apply_started.elapsed().as_micros();

        let clear_started = Instant::now();
        commit_project_workspace_session_mutation(app, &mut workspace, |candidate| {
            let baseline = candidate
                .documents
                .baseline_text_for(mutation_path)
                .unwrap();
            candidate.stage_document_texts(
                &identity(candidate),
                transaction_metadata("perf.clear"),
                vec![WorkspaceDocumentMutation {
                    relative_path: mutation_path.to_string(),
                    contents: baseline,
                }],
                5,
            )
        })
        .unwrap();
        let clear_transaction_us = clear_started.elapsed().as_micros();

        let composite_started = Instant::now();
        commit_project_workspace_session_mutation(app, &mut workspace, |candidate| {
            candidate.stage_composite_changes(
                &identity(candidate),
                transaction_metadata("perf.composite"),
                vec![WorkspaceResourceMutation {
                    relative_path: "templates/perf-composite.html".to_string(),
                    contents: "<main>composite</main>\n".to_string(),
                    create_only: true,
                }],
                Vec::new(),
                None,
                6,
            )
        })
        .unwrap();
        let composite_transaction_us = composite_started.elapsed().as_micros();

        let undo_started = Instant::now();
        commit_project_workspace_session_mutation(app, &mut workspace, |candidate| {
            candidate.undo(&identity(candidate), 7)
        })
        .unwrap();
        let undo_transaction_us = undo_started.elapsed().as_micros();

        let redo_started = Instant::now();
        commit_project_workspace_session_mutation(app, &mut workspace, |candidate| {
            candidate.redo(&identity(candidate), 8)
        })
        .unwrap();
        let redo_transaction_us = redo_started.elapsed().as_micros();

        WorkspaceRuntimeMeasurement {
            documents_baseline_bytes,
            accepted_baseline_bytes,
            workspace_snapshot_bytes,
            file_buffer_snapshot_bytes,
            duplicate_bootstrap_bytes: workspace_snapshot_bytes + file_buffer_snapshot_bytes,
            workspace_clone_us,
            snapshot_us,
            project_model_clone_us,
            file_buffer_mutation_us,
            set_transaction_us,
            apply_transaction_us,
            clear_transaction_us,
            composite_transaction_us,
            undo_transaction_us,
            redo_transaction_us,
        }
    }

    fn recovery_serialization_measurement(
        root: &Path,
        label: &str,
        file_count: usize,
        bytes_per_file: usize,
    ) -> (usize, usize, u128, u128) {
        let project = root.join(label);
        fs::create_dir_all(project.join("templates")).unwrap();
        fs::write(project.join("zola.toml"), "base_url = '/'\n").unwrap();
        let mut sources = Vec::new();
        for index in 0..file_count {
            let relative_path = format!("templates/page-{index:03}.html");
            let prefix = format!("<main data-index=\"{index}\">");
            let source = format!(
                "{prefix}{}</main>\n",
                "x".repeat(bytes_per_file.saturating_sub(prefix.len() + 9))
            );
            fs::write(project.join(&relative_path), &source).unwrap();
            sources.push((relative_path, source));
        }
        let project = project.canonicalize().unwrap();
        let session = test_session(&project, &root.join(format!("{label}-session")));
        let mut base = workspace(&project, &session);
        for (relative_path, source) in &sources {
            base.documents.insert_loaded_file(FileBufferEntry {
                relative_path: relative_path.clone(),
                absolute_path: project.join(relative_path).to_string_lossy().into_owned(),
                language: TextBufferLanguage::Html,
                role: TextBufferRole::Template,
                baseline: FileBufferBaseline {
                    hash: hash_text(source),
                    modified_ms: 1,
                    size: source.len() as u64,
                    readonly: false,
                },
                baseline_text: source.clone().into(),
                draft: None,
                revision: 1,
            });
        }
        base.accepted_documents = base.documents.files.clone();
        let mut current = base.fork_candidate();
        let changed_path = &sources[file_count / 2].0;
        let snapshot = current.documents.text_snapshot(changed_path).unwrap();
        let changed = snapshot
            .text
            .replacen("<main", "<main data-mutated=\"true\"", 1);
        current
            .documents
            .set_draft_if_current(
                changed_path,
                changed,
                &FileBufferMutationExpectation {
                    expected_revision: snapshot.revision,
                    expected_hash: snapshot.hash,
                },
                2,
            )
            .unwrap();
        current.revision = 1;

        let mut checkpoint_bytes = 0;
        let mut checkpoint_us = u128::MAX;
        let mut journal_bytes = 0;
        let mut journal_us = u128::MAX;
        for _ in 0..5 {
            let checkpoint_started = Instant::now();
            let checkpoint = serialize_recovery_envelope_from_workspace(&current).unwrap();
            checkpoint_us = checkpoint_us.min(checkpoint_started.elapsed().as_micros());
            checkpoint_bytes = checkpoint.len();

            let journal_started = Instant::now();
            let journal = serialize_recovery_journal_transaction(&base, &current).unwrap();
            journal_us = journal_us.min(journal_started.elapsed().as_micros());
            journal_bytes = journal.len();
        }
        (checkpoint_bytes, journal_bytes, checkpoint_us, journal_us)
    }

    fn workspace(project: &Path, session: &ProjectSessionSnapshot) -> ProjectWorkspace {
        let accepted = AcceptedProjectDiskManifest::new(
            session.runtime_instance_id(),
            session.project_root.clone(),
            read_project_disk_manifest(project).unwrap(),
        )
        .unwrap();
        let documents = FileBufferStore::for_project_session(
            session,
            1,
            FileBufferStoreLimits {
                max_files: 100,
                max_file_bytes: 1024 * 1024,
                max_total_bytes: 4 * 1024 * 1024,
            },
        );
        ProjectWorkspace::new(
            session.clone(),
            accepted,
            documents,
            PageJsDraftStore::new(session),
        )
        .unwrap()
    }

    fn identity(workspace: &ProjectWorkspace) -> ProjectWorkspaceIdentity {
        ProjectWorkspaceIdentity {
            expected_project_root: workspace.session.project_root.clone(),
            expected_session_id: workspace.runtime_session_id(),
            expected_revision: workspace.revision,
        }
    }

    fn test_session(project: &Path, session_dir: &Path) -> ProjectSessionSnapshot {
        ProjectSessionSnapshot {
            schema_version: 1,
            id: "binary-recovery-session".to_string(),
            project_root: project.to_string_lossy().into_owned(),
            zola_root: project.to_path_buf().to_string_lossy().into_owned(),
            session_dir: session_dir.to_string_lossy().into_owned(),
            manifest_path: session_dir
                .join("manifest.json")
                .to_string_lossy()
                .into_owned(),
            opened_at_ms: 88,
            last_seen_at_ms: 88,
            root_fingerprint: ProjectRootFingerprint {
                canonical_path: project.to_string_lossy().into_owned(),
                modified_ms: 1,
                size: 0,
                readonly: false,
                unix_device: None,
                unix_inode: None,
            },
            scan_summary: ProjectSessionScanSummary {
                active_theme: None,
                file_count: 1,
                directory_count: 4,
            },
        }
    }

    struct TestEnvGuard {
        previous_values: Vec<(&'static str, Option<String>)>,
    }

    impl TestEnvGuard {
        fn from_root(root: &Path) -> Self {
            let bindings = [
                ("XDG_CONFIG_HOME", root.join("config")),
                ("XDG_DATA_HOME", root.join("data")),
                ("XDG_CACHE_HOME", root.join("cache")),
                ("XDG_STATE_HOME", root.join("state")),
            ];
            let previous_values = bindings
                .iter()
                .map(|(key, _)| (*key, env::var(key).ok()))
                .collect::<Vec<_>>();
            for (key, path) in bindings {
                env::set_var(key, path);
            }
            Self { previous_values }
        }
    }

    impl Drop for TestEnvGuard {
        fn drop(&mut self) {
            for (key, value) in &self.previous_values {
                if let Some(value) = value {
                    env::set_var(key, value);
                } else {
                    env::remove_var(key);
                }
            }
        }
    }
}
