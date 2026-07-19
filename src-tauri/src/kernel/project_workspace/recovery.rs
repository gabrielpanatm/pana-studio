use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::{
    app_home::{
        project_open_recovery_decision_path, project_session_dir, project_session_manifest_path,
        project_workspace_recovery_path,
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
    history::WorkspaceHistory,
    model::{
        ProjectWorkspaceIdentity, ProjectWorkspaceSaveError, ProjectWorkspaceSaveReceipt,
        PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_BYTES,
        PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_TOTAL_BYTES,
    },
    ProjectWorkspace, WorkspaceBinaryResource,
};

const PROJECT_WORKSPACE_RECOVERY_SCHEMA_VERSION: u32 = 3;
const PROJECT_WORKSPACE_RECOVERY_MAX_BYTES: u64 = 192 * 1024 * 1024;
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
    if let Some(state) = app.try_state::<AppState>() {
        state
            .ai_coordination
            .require_user_source_mutation()
            .map_err(|error| error.to_string())?;
    }
    let mut candidate = live_workspace.clone();
    let result = mutate(&mut candidate)?;
    persist_project_workspace_recovery_with_projection(app, &candidate, preview_projection)?;
    *live_workspace = candidate;
    Ok(result)
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
    documents: BTreeMap<String, FileBufferEntry>,
    accepted_binary_resource_hashes: BTreeMap<String, String>,
    binary_resources: BTreeMap<String, WorkspaceBinaryResource>,
    deleted_binary_resources: BTreeSet<String>,
    accepted_page_js: BTreeMap<String, PageJsConfig>,
    page_js_drafts: Vec<ProjectWorkspacePageJsRecoveryDraft>,
    history: WorkspaceHistory,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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

pub fn persist_project_workspace_recovery<R: Runtime>(
    app: &AppHandle<R>,
    workspace: &ProjectWorkspace,
) -> Result<(), String> {
    persist_project_workspace_recovery_with_projection(
        app,
        workspace,
        ProjectWorkspacePreviewProjection::Required,
    )
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

fn persist_project_workspace_recovery_with_projection<R: Runtime>(
    app: &AppHandle<R>,
    workspace: &ProjectWorkspace,
    preview_projection: ProjectWorkspacePreviewProjection,
) -> Result<(), String> {
    workspace.accepted_disk.require_identity(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
    )?;
    workspace.accepted_disk.require_complete()?;
    let payload = ProjectWorkspaceRecoveryPayload {
        schema_version: PROJECT_WORKSPACE_RECOVERY_SCHEMA_VERSION,
        project_root: workspace.session.project_root.clone(),
        accepted_manifest: workspace.accepted_disk.manifest.clone(),
        revision: workspace.revision,
        persisted_at_ms: now_ms(),
        documents: workspace.documents.files.clone(),
        accepted_binary_resource_hashes: workspace.accepted_binary_resource_hashes.clone(),
        binary_resources: workspace.binary_resources.clone(),
        deleted_binary_resources: workspace.deleted_binary_resources.clone(),
        accepted_page_js: workspace.accepted_page_js.clone(),
        page_js_drafts: workspace
            .page_js
            .drafts
            .values()
            .map(|draft| ProjectWorkspacePageJsRecoveryDraft {
                template_path: draft.template_path.clone(),
                base: draft.base.clone(),
                current: draft.current.clone(),
                cachebust_assets: draft.cachebust_assets,
                source: draft.source.clone(),
                coalesce_key: draft.coalesce_key.clone(),
                transaction_id: draft.transaction_id.clone(),
            })
            .collect(),
        history: workspace.history.clone(),
    };
    let payload_source = serde_json::to_string(&payload).map_err(|error| {
        format!("ProjectWorkspace recovery nu poate serializa payloadul: {error}")
    })?;
    let envelope = ProjectWorkspaceRecoveryEnvelope {
        schema_version: PROJECT_WORKSPACE_RECOVERY_SCHEMA_VERSION,
        payload_checksum: hash_text(&payload_source),
        payload,
    };
    let source = serde_json::to_string_pretty(&envelope).map_err(|error| {
        format!("ProjectWorkspace recovery nu poate serializa învelișul: {error}")
    })?;
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
        "Persist ProjectWorkspace recovery",
    );
    WriteAuthority::new(app)
        .write_text(intent, &format!("{source}\n"))
        .map_err(|error| error.into_terminal_diagnostic())?;
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
    Ok(())
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
    let Some(source) = read_recovery_source(&recovery_path)? else {
        return Ok(ProjectOpenRecoveryAssessment {
            schema_version: PROJECT_OPEN_RECOVERY_ASSESSMENT_SCHEMA_VERSION,
            status: ProjectOpenRecoveryStatus::Missing,
            project_root,
            assessment_token: None,
            conflict_reason: None,
            root_identity_changed: previous_root_identity_changed(app, &current_root_fingerprint)?,
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
    };

    let assessment_token = project_open_recovery_assessment_token(
        &source,
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

    let envelope = match parse_project_workspace_recovery_envelope(&source) {
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
    let Some(source) = read_recovery_source(&path)? else {
        return Ok(ProjectWorkspaceRecoveryStatus::Missing);
    };
    let envelope = parse_project_workspace_recovery_envelope(&source)?;
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
                documents.files.remove(&relative_path);
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
    for (path, hash) in &payload.accepted_binary_resource_hashes {
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
    workspace.revision = payload.revision;
    workspace.project_model = None;
    workspace.project_model_source_revision = None;
    Ok(ProjectWorkspaceRecoveryStatus::Restored)
}

pub fn clear_project_workspace_recovery<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &str,
) -> Result<(), String> {
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
    resolve_project_write_path(project_root, relative_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{env, path::PathBuf};

    use crate::{
        app_home::{ensure_app_home, TEST_APP_ENV_LOCK},
        js::PageJsDraftStore,
        kernel::{
            file_buffer_store::{hash_bytes, FileBufferStore, FileBufferStoreLimits},
            project_session::{
                fingerprint_project_root, persist_project_session_open, ProjectRootFingerprint,
                ProjectSessionScanSummary, ProjectSessionSnapshot,
            },
            project_workspace::{
                ProjectWorkspaceIdentity, WorkspaceMutationMetadata,
                PROJECT_WORKSPACE_SCHEMA_VERSION,
            },
        },
        project::{read_project_disk_manifest, AcceptedProjectDiskManifest},
    };

    use super::*;

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
        fs::create_dir_all(project_path.join("sursa/templates")).unwrap();
        fs::write(project_path.join("sursa/zola.toml"), "base_url = '/'\n").unwrap();
        fs::write(
            project_path.join("sursa/templates/index.html"),
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
                    relative_path: "sursa/templates/draft.html".to_string(),
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
        fs::create_dir_all(project.join("sursa/static/fonturi/inter")).unwrap();
        fs::write(project.join("sursa/zola.toml"), "base_url = '/'\n").unwrap();
        let project = project.canonicalize().unwrap();
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock app");
        let app_home = ensure_app_home(app.handle()).unwrap();
        let session_dir = PathBuf::from(&app_home.sessions_dir).join("binary-recovery-session");
        fs::create_dir_all(&session_dir).unwrap();
        let session = test_session(&project, &session_dir);

        let mut original = workspace(&project, &session);
        let relative_path = "sursa/static/fonturi/inter/inter-regular.woff2";
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
    fn accepted_binary_hash_survives_recovery_and_redo_normalizes_clean() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = std::env::temp_dir().join(format!(
            "pana-project-workspace-binary-baseline-recovery-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let project = root.join("project");
        fs::create_dir_all(project.join("sursa/static/fonturi/inter")).unwrap();
        fs::write(project.join("sursa/zola.toml"), "base_url = '/'\n").unwrap();
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
        let relative_path = "sursa/static/fonturi/inter/inter-regular.woff2";
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
        fs::create_dir_all(project.join("sursa/templates")).unwrap();
        fs::write(project.join("sursa/zola.toml"), "base_url = '/'\n").unwrap();
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
                        relative_path: "sursa/templates/candidate.html".to_string(),
                        contents: "<main>candidate</main>".to_string(),
                        create_only: true,
                    }],
                    40,
                )
            })
            .unwrap_err();

        assert!(error.contains("project-workspace.json") || error.contains("directory"));
        let after = live.snapshot();
        assert_eq!(after.revision, before.revision);
        assert_eq!(after.dirty, before.dirty);
        assert_eq!(after.history.undo_count, before.history.undo_count);
        assert!(live
            .documents
            .text_for("sursa/templates/candidate.html")
            .is_none());

        drop(app);
        fs::remove_dir_all(root).unwrap();
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
            zola_root: project.join("sursa").to_string_lossy().into_owned(),
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
                is_zola: true,
                is_empty: false,
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
