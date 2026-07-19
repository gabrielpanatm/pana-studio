use std::collections::BTreeSet;

use tauri::State;

use crate::{
    kernel::{
        ai_coordination::{
            AiCoordinationSnapshot, EditTransitionReceipt, ProjectCoordinationBlocker,
            ProjectCoordinationBlockerKind, ProjectCoordinationEvidence, ReconciliationInput,
            UiQuiescenceAcknowledgement,
        },
        disk_conflict::{scan_disk_conflicts, KernelDiskConflictKind},
        observability::now_ms,
        recovery_coordinator::RecoveryCoordinatorStatus,
    },
    project::{project_disk_manifest_changed_paths, read_project_disk_manifest},
    state::AppState,
};

#[tauri::command]
pub fn read_ai_coordination_state(
    state: State<AppState>,
) -> Result<AiCoordinationSnapshot, String> {
    state
        .ai_coordination
        .expire(now_ms())
        .map_err(|error| error.to_string())?;
    state
        .ai_coordination
        .snapshot(now_ms())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn acknowledge_ai_edit_quiescence(
    client_session_id: String,
    acknowledgement: UiQuiescenceAcknowledgement,
    state: State<AppState>,
) -> Result<EditTransitionReceipt, String> {
    with_current_project_coordination_evidence(state.inner(), |evidence| {
        state
            .ai_coordination
            .acknowledge_ui_quiescence(&client_session_id, acknowledgement, evidence, now_ms())
            .map_err(|error| error.to_string())
    })
}

#[tauri::command]
pub fn accept_ai_edit_conflict_for_reconciliation(
    state: State<AppState>,
) -> Result<EditTransitionReceipt, String> {
    with_current_project_coordination_evidence(state.inner(), |evidence| {
        let project_session_id = evidence.project_session_id.as_deref().ok_or_else(|| {
            "Nu există ProjectSession pentru acceptarea conflictului AI.".to_string()
        })?;
        let project_revision = evidence.project_revision.ok_or_else(|| {
            "ProjectWorkspace nu are o revizie validă pentru acceptarea conflictului AI."
                .to_string()
        })?;
        if evidence.is_dirty() {
            return Err(
                "Salvează sau aruncă modificările din RAM înainte de acceptarea discului extern."
                    .to_string(),
            );
        }
        if evidence
            .blockers
            .iter()
            .any(|blocker| blocker.kind != ProjectCoordinationBlockerKind::DiskConflict)
        {
            return Err(
                "RecoveryCoordinator trebuie să fie clean înainte de reconcilierea conflictului AI."
                    .to_string(),
            );
        }
        state
            .ai_coordination
            .accept_conflict_for_reconciliation(project_session_id, project_revision, now_ms())
            .map_err(|error| error.to_string())
    })
}

#[tauri::command]
pub fn authorize_ai_reconciliation_recovery_reload(
    state: State<AppState>,
) -> Result<EditTransitionReceipt, String> {
    with_current_project_coordination_evidence(state.inner(), |evidence| {
        state
            .ai_coordination
            .authorize_reconciliation_recovery_reload(evidence, now_ms())
            .map_err(|error| error.to_string())
    })
}

#[tauri::command]
pub fn complete_ai_reconciliation_recovery_reload(
    lease_id: String,
    expected_replacement_session_id: String,
    state: State<AppState>,
) -> Result<EditTransitionReceipt, String> {
    with_current_project_coordination_evidence(state.inner(), |evidence| {
        state
            .ai_coordination
            .complete_reconciliation_recovery_reload(
                &lease_id,
                &expected_replacement_session_id,
                evidence,
                now_ms(),
            )
            .map_err(|error| error.to_string())
    })
}

#[tauri::command]
pub fn complete_ai_edit_reconciliation(
    lease_id: String,
    expected_project_session_id: String,
    expected_project_revision: u64,
    observed_changed_files: Vec<String>,
    state: State<AppState>,
) -> Result<EditTransitionReceipt, String> {
    let workspace_guard = state.project_workspace.lock().map_err(|_| {
        "Nu am putut bloca ProjectWorkspace pentru finalizarea reconcilierii AI.".to_string()
    })?;
    let workspace = workspace_guard
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este deschis pentru reconcilierea AI.".to_string())?;
    let snapshot = workspace.snapshot();
    if snapshot.runtime_session_id != expected_project_session_id
        || snapshot.revision != expected_project_revision
    {
        return Err(format!(
            "Finalizarea reconcilierii AI este stale: aștepta sesiunea {expected_project_session_id}/revizia {expected_project_revision}, iar Rust deține sesiunea {}/revizia {}.",
            snapshot.runtime_session_id, snapshot.revision
        ));
    }
    if snapshot.dirty {
        return Err(
            "ProjectWorkspace a devenit dirty în timpul reconcilierii AI; controlul utilizatorului nu poate fi redat."
                .to_string(),
        );
    }
    let recovery = state.recovery_coordinator_scan.lock().map_err(|_| {
        "Nu am putut bloca RecoveryCoordinatorScan pentru reconcilierea AI.".to_string()
    })?;
    let recovery = recovery.as_ref().ok_or_else(|| {
        "RecoveryCoordinatorScan lipsește la finalizarea reconcilierii AI.".to_string()
    })?;
    if recovery.status != RecoveryCoordinatorStatus::Clean {
        return Err(
            "RecoveryCoordinatorScan nu este clean; reconcilierea AI rămâne blocată.".to_string(),
        );
    }

    let mut conflict_files = scan_disk_conflicts(&workspace.documents)
        .files
        .into_iter()
        .filter(|file| {
            !matches!(
                file.kind,
                KernelDiskConflictKind::Clean
                    | KernelDiskConflictKind::DirtyOnly
                    | KernelDiskConflictKind::MetadataChanged
            )
        })
        .map(|file| file.relative_path)
        .collect::<BTreeSet<_>>();
    let observed_manifest =
        read_project_disk_manifest(std::path::Path::new(&snapshot.project_root))?;
    conflict_files.extend(project_disk_manifest_changed_paths(
        &workspace.accepted_disk.manifest,
        &observed_manifest,
    )?);

    state
        .ai_coordination
        .complete_reconciliation(
            ReconciliationInput {
                lease_id,
                project_session_id: expected_project_session_id,
                project_revision: expected_project_revision,
                observed_changed_files,
                conflict_files: conflict_files.into_iter().collect(),
            },
            now_ms(),
        )
        .map_err(|error| error.to_string())
}

pub(crate) fn with_current_project_coordination_evidence<T>(
    state: &AppState,
    operation: impl FnOnce(&ProjectCoordinationEvidence) -> Result<T, String>,
) -> Result<T, String> {
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru coordonarea AI.".to_string())?;
    let Some(workspace) = workspace.as_ref() else {
        return operation(&ProjectCoordinationEvidence::closed());
    };
    let snapshot = workspace.snapshot();
    let mut dirty_files = BTreeSet::new();
    dirty_files.extend(
        snapshot
            .documents
            .files
            .iter()
            .filter(|file| file.dirty)
            .map(|file| file.relative_path.clone()),
    );
    dirty_files.extend(snapshot.created_documents);
    dirty_files.extend(snapshot.deleted_documents);
    dirty_files.extend(snapshot.staged_binary_resources);
    dirty_files.extend(snapshot.deleted_binary_resources);
    dirty_files.extend(
        snapshot
            .page_js
            .drafts
            .into_iter()
            .map(|draft| draft.template_path),
    );
    let disk_conflicts = scan_disk_conflicts(&workspace.documents);
    let mut disk_conflict_files = disk_conflicts
        .files
        .iter()
        .filter(|file| {
            !matches!(
                file.kind,
                KernelDiskConflictKind::Clean
                    | KernelDiskConflictKind::DirtyOnly
                    | KernelDiskConflictKind::MetadataChanged
            )
        })
        .map(|file| file.relative_path.clone())
        .collect::<BTreeSet<_>>();
    let observed_manifest =
        read_project_disk_manifest(std::path::Path::new(&snapshot.project_root))?;
    disk_conflict_files.extend(project_disk_manifest_changed_paths(
        &workspace.accepted_disk.manifest,
        &observed_manifest,
    )?);
    let mut blockers = Vec::new();
    if !disk_conflict_files.is_empty() {
        blockers.push(ProjectCoordinationBlocker {
            kind: ProjectCoordinationBlockerKind::DiskConflict,
            reason: format!(
                "Discul diferă de AcceptedDisk pentru {} fișier(e). {}",
                disk_conflict_files.len(),
                disk_conflicts.summary.verdict_reason
            ),
            files: disk_conflict_files.into_iter().collect(),
        });
    }

    let recovery = state.recovery_coordinator_scan.lock().map_err(|_| {
        "Nu am putut bloca RecoveryCoordinatorScan pentru coordonarea AI.".to_string()
    })?;
    match recovery.as_ref().map(|scan| scan.status) {
        None => blockers.push(ProjectCoordinationBlocker {
            kind: ProjectCoordinationBlockerKind::RecoveryUnavailable,
            reason: "RecoveryCoordinatorScan nu este disponibil; autoritatea nu poate fi transferată sigur către AI."
                .to_string(),
            files: Vec::new(),
        }),
        Some(RecoveryCoordinatorStatus::NeedsAttention) => {
            blockers.push(ProjectCoordinationBlocker {
                kind: ProjectCoordinationBlockerKind::RecoveryNeedsAttention,
                reason: "RecoveryCoordinatorScan cere intervenție înainte de transferul autorității către AI."
                    .to_string(),
                files: Vec::new(),
            });
        }
        Some(RecoveryCoordinatorStatus::Unreadable) => blockers.push(ProjectCoordinationBlocker {
            kind: ProjectCoordinationBlockerKind::RecoveryUnreadable,
            reason: "RecoveryCoordinatorScan este unreadable; autoritatea nu poate fi transferată sigur către AI."
                .to_string(),
            files: Vec::new(),
        }),
        Some(RecoveryCoordinatorStatus::Clean) => {}
    }

    let evidence = ProjectCoordinationEvidence {
        project_session_id: Some(snapshot.runtime_session_id),
        project_revision: Some(snapshot.revision),
        dirty_files: dirty_files.into_iter().collect(),
        blockers,
    };
    operation(&evidence)
}
