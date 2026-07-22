use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, Runtime, State};

use crate::{
    commands::kernel::current_kernel_project_state_snapshot,
    js::PageJsDraftStore,
    kernel::{
        ai_coordination::EditAuthority,
        disk_conflict::scan_disk_conflicts,
        file_buffer_store::{
            bootstrap_file_buffer_store, now_ms as file_buffer_now_ms,
            require_file_buffer_session_binding, FileBufferChangeSetInput,
            FileBufferChangeSetResult, FileBufferCommandReceipt, FileBufferFileSnapshot,
            FileBufferMutationExpectation, FileBufferRequestIdentity, FileBufferStore,
            FileBufferStoreSnapshot, FileBufferTextSnapshot,
        },
        observability::{append_event, now_ms, KernelEventKind, KernelLogEvent, KernelLogLevel},
        project_session::{
            fingerprint_project_root, persist_project_session_open, prepare_project_session,
            record_project_session_opened, ProjectSessionSnapshot,
        },
        project_state::{
            append_kernel_project_transition_decision,
            append_kernel_project_transition_decision_recovery_ack,
            build_kernel_project_transition_decision_evidence, evaluate_project_transition_policy,
            execute_project_transition_decision_retention as apply_project_transition_decision_retention,
            read_kernel_project_transition_decision_journal_snapshot,
            recover_project_transition_decision_retention_hot_journal as apply_project_transition_decision_retention_hot_journal_recovery,
            require_matching_kernel_project_transition_decision, KernelProjectTransitionAction,
            KernelProjectTransitionDecisionInput, KernelProjectTransitionDecisionReceipt,
            KernelProjectTransitionDecisionRecoveryAckInput,
            KernelProjectTransitionDecisionRecoveryAckReceipt,
            KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
            KernelProjectTransitionDecisionRetentionInput,
            KernelProjectTransitionDecisionRetentionReceipt,
            KernelProjectTransitionDecisionRetentionRecoveryReceipt,
        },
        project_workspace::{
            clear_project_open_recovery_decision, clear_project_workspace_recovery,
            commit_project_workspace_session_mutation, inspect_project_workspace_recovery_for_open,
            persist_project_open_recovery_abandonment, persist_project_workspace_recovery,
            recover_project_workspace_save_hot_journal as apply_project_workspace_save_recovery,
            require_project_open_recovery_assessment_unchanged, resolve_project_open_recovery,
            restore_project_workspace_recovery, ProjectOpenRecoveryAssessment,
            ProjectOpenRecoveryDecisionInput, ProjectOpenRecoveryResolution, ProjectWorkspace,
            ProjectWorkspaceHistoryIdentity, ProjectWorkspaceIdentity, ProjectWorkspaceSaveError,
            ProjectWorkspaceSaveReceipt, ProjectWorkspaceSaveRecoveryAction,
            ProjectWorkspaceSaveRecoveryReceipt, ProjectWorkspaceSnapshot,
            WorkspaceDocumentMutation, WorkspaceHistoryDirection, WorkspaceHistorySnapshot,
            WorkspaceMutationMetadata, WorkspaceUndoRedoReceipt,
        },
        recovery_coordinator::{
            scan_recovery_coordinator, RecoveryCoordinatorScan, RecoveryCoordinatorStatus,
        },
        write_authority::WriteAuthorityRuntime,
    },
    preview::{
        schedule_source_browser_refresh, stop_project_preview, stop_source_browser,
        BrowserPreviewRequestIdentity,
    },
    project::{
        init_project_with_starter, read_project_disk_manifest, scan_project_root,
        scan_project_workspace_projection, AcceptedProjectDiskManifest,
    },
    state::AppState,
};

pub fn current_project_root(state: &State<AppState>) -> Option<PathBuf> {
    state.current_root.lock().ok()?.clone()
}

pub fn require_current_project_root(state: &State<AppState>) -> Result<PathBuf, String> {
    current_project_root(state).ok_or_else(|| "Nu există proiect deschis.".to_string())
}

fn current_project_session(
    state: &State<AppState>,
) -> Result<Option<ProjectSessionSnapshot>, String> {
    Ok(state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace.".to_string())?
        .as_ref()
        .map(|workspace| workspace.session.clone()))
}

fn require_current_project_session(
    state: &State<AppState>,
) -> Result<ProjectSessionSnapshot, String> {
    current_project_session(state)?
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())
}

#[tauri::command]
pub fn record_project_transition_operator_decision(
    target_root: String,
    diagnostic: String,
    action: Option<KernelProjectTransitionAction>,
    app: AppHandle,
    state: State<AppState>,
) -> Result<KernelProjectTransitionDecisionReceipt, String> {
    let target_root = PathBuf::from(target_root)
        .canonicalize()
        .map_err(|error| format!("Nu am putut rezolva target-ul tranziției: {error}"))?;
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul proiectului curent.".to_string())?
        .clone()
        .ok_or_else(|| "Nu există proiect curent pentru decizie de tranziție.".to_string())?;
    let inferred_action = if current_root == target_root {
        KernelProjectTransitionAction::ReloadProject
    } else {
        KernelProjectTransitionAction::OpenProject
    };
    let action = action.unwrap_or(inferred_action);
    validate_project_transition_action_target(action, &current_root, &target_root)?;
    let project_state = current_kernel_project_state_snapshot(&state)?;
    let policy = evaluate_project_transition_policy(action, &project_state);
    let evidence = build_project_transition_evidence_for_target(
        &state,
        &target_root,
        action,
        &project_state,
        &policy,
    )?;
    let session = require_current_project_session(&state)?;

    append_kernel_project_transition_decision(
        &app,
        &session,
        &policy,
        evidence,
        KernelProjectTransitionDecisionInput {
            target_project_root: target_root.to_string_lossy().to_string(),
            diagnostic,
        },
    )
}

#[tauri::command]
pub fn acknowledge_project_transition_decision_recovery_plan(
    recovery_plan_evidence_hash: String,
    diagnostic: String,
    app: AppHandle,
    state: State<AppState>,
) -> Result<KernelProjectTransitionDecisionRecoveryAckReceipt, String> {
    let session = require_current_project_session(&state)?;
    let decision_journal =
        read_kernel_project_transition_decision_journal_snapshot(&session, Some(500))?;

    append_kernel_project_transition_decision_recovery_ack(
        &app,
        &session,
        &decision_journal,
        KernelProjectTransitionDecisionRecoveryAckInput {
            recovery_plan_evidence_hash,
            diagnostic,
        },
    )
}

#[tauri::command]
pub fn execute_project_transition_decision_retention(
    recovery_plan_evidence_hash: String,
    acknowledgement_id: String,
    diagnostic: String,
    app: AppHandle,
    state: State<AppState>,
) -> Result<KernelProjectTransitionDecisionRetentionReceipt, String> {
    let session = require_current_project_session(&state)?;

    let retention_result = apply_project_transition_decision_retention(
        &app,
        &session,
        KernelProjectTransitionDecisionRetentionInput {
            recovery_plan_evidence_hash,
            acknowledgement_id,
            diagnostic,
        },
    );
    refresh_recovery_coordinator_scan(&app, &state, &session, retention_result.is_ok())?;
    retention_result
}

#[tauri::command]
pub fn recover_project_transition_decision_retention_hot_journal(
    retention_id: String,
    action: KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
    diagnostic: String,
    app: AppHandle,
    state: State<AppState>,
) -> Result<ProjectTransitionDecisionRetentionHotJournalRecoveryCommandResult, String> {
    let session = require_current_project_session(&state)?;

    let recovery_result = apply_project_transition_decision_retention_hot_journal_recovery(
        &app,
        &session,
        &retention_id,
        action,
        diagnostic,
    );

    refresh_recovery_coordinator_scan(&app, &state, &session, recovery_result.is_ok())?;
    let receipt = recovery_result?;
    let recovery_coordinator = state
        .recovery_coordinator_scan
        .lock()
        .map_err(|_| "Nu am putut bloca RecoveryCoordinatorScan.".to_string())?
        .clone()
        .ok_or_else(|| {
            "Transaction Recovery Scan nu este inițializat după ProjectTransition Decision retention recovery."
                .to_string()
        })?;

    Ok(
        ProjectTransitionDecisionRetentionHotJournalRecoveryCommandResult {
            receipt,
            recovery_coordinator,
        },
    )
}

#[tauri::command]
pub fn read_project_session(
    state: State<AppState>,
) -> Result<Option<ProjectSessionSnapshot>, String> {
    current_project_session(&state)
}

fn capture_project_session_attachment(
    state: &AppState,
) -> Result<Option<(PathBuf, String, AcceptedProjectDiskManifest)>, String> {
    // Keep the canonical ProjectTransition lock order. Reattachment is a
    // read-only projection of one already-published runtime session; it must
    // never manufacture a session identity from the stable manifest id.
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul pentru reatașarea ProjectSession.".to_string())?;
    let project_workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru reatașare.".to_string())?;

    match (
        current_root.as_ref(),
        project_workspace.as_ref().map(|workspace| &workspace.session),
        project_workspace.as_ref().map(|workspace| &workspace.accepted_disk),
    ) {
        (None, None, None) => Ok(None),
        (Some(root), Some(session), Some(accepted)) => {
            if Path::new(&session.project_root) != root {
                return Err(format!(
                    "Reatașarea a găsit root-uri divergente: runtime={}, ProjectSession={}.",
                    root.display(),
                    session.project_root
                ));
            }
            let runtime_session_id = session.runtime_instance_id();
            accepted.require_identity(&runtime_session_id, &session.project_root)?;
            Ok(Some((root.clone(), runtime_session_id, accepted.clone())))
        }
        _ => Err(
            "Reatașarea a găsit o stare ProjectSession publicată parțial; proiecția frontend a fost refuzată."
                .to_string(),
        ),
    }
}

fn reattach_project_session_impl(
    state: &AppState,
) -> Result<Option<crate::project::ProjectScan>, String> {
    let Some((root, runtime_session_id, accepted_disk)) =
        capture_project_session_attachment(state)?
    else {
        return Ok(None);
    };

    let projection = {
        let workspace = state
            .project_workspace
            .lock()
            .map_err(|_| "Nu am putut bloca ProjectWorkspace la reatașare.".to_string())?;
        let workspace = workspace
            .as_ref()
            .ok_or_else(|| "ProjectWorkspace a dispărut în timpul reatașării.".to_string())?;
        if workspace.runtime_session_id() != runtime_session_id
            || workspace.session.project_root != root.to_string_lossy()
            || workspace.accepted_disk != accepted_disk
        {
            return Err(
                "ProjectWorkspace s-a schimbat înainte de proiecția reatașării.".to_string(),
            );
        }
        workspace.capture_projection_lease()?
    };
    let scan = scan_project_workspace_projection(&projection)?;

    // Revalidate the exact immutable revision before publishing it to the
    // frontend. A concurrent edit must produce a new ProjectScan, never an
    // overlay settlement over this result.
    let live_attachment = capture_project_session_attachment(state)?;
    if live_attachment.as_ref()
        != Some(&(
            root.clone(),
            runtime_session_id.clone(),
            accepted_disk.clone(),
        ))
    {
        return Err(
            "ProjectSession s-a schimbat în timpul reatașării; ProjectScan a devenit stale."
                .to_string(),
        );
    }
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut revalida ProjectWorkspace la reatașare.".to_string())?;
    workspace
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace a dispărut în timpul reatașării.".to_string())?
        .require_current_projection(&projection)?;
    Ok(Some(scan))
}

/// Rebuilds only the frontend projection after a webview/dev reload. Unlike
/// `open_project`, this command does not run ProjectTransition, replace the
/// FileBufferStore, reset Undo/Redo, or touch the disk.
#[tauri::command]
pub fn reattach_project_session(
    state: State<AppState>,
) -> Result<Option<crate::project::ProjectScan>, String> {
    reattach_project_session_impl(state.inner())
}

#[tauri::command]
pub fn read_file_buffer_store(
    state: State<AppState>,
) -> Result<Option<FileBufferStoreSnapshot>, String> {
    state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace.".to_string())
        .map(|workspace| {
            workspace
                .as_ref()
                .map(|workspace| workspace.documents.snapshot())
        })
}

#[tauri::command]
pub fn read_recovery_coordinator_scan(
    state: State<AppState>,
) -> Result<Option<RecoveryCoordinatorScan>, String> {
    state
        .recovery_coordinator_scan
        .lock()
        .map_err(|_| "Nu am putut bloca RecoveryCoordinatorScan.".to_string())
        .map(|scan| scan.clone())
}

#[tauri::command]
pub fn read_project_workspace_state(
    state: State<AppState>,
) -> Result<Option<ProjectWorkspaceSnapshot>, String> {
    state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace.".to_string())
        .map(|workspace| workspace.as_ref().map(ProjectWorkspace::snapshot))
}

#[tauri::command]
pub fn read_project_workspace_history(
    state: State<AppState>,
) -> Result<Option<WorkspaceHistorySnapshot>, String> {
    read_project_workspace_state(state).map(|workspace| workspace.map(|item| item.history))
}

#[tauri::command]
pub fn save_project_workspace(
    identity: ProjectWorkspaceIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<ProjectWorkspaceSaveReceipt, ProjectWorkspaceSaveError> {
    let current_root = state.current_root.lock().map_err(|_| {
        ProjectWorkspaceSaveError::rejected(
            "Nu am putut bloca root-ul proiectului pentru Save ProjectWorkspace.",
        )
    })?;
    let root = current_root.as_ref().ok_or_else(|| {
        ProjectWorkspaceSaveError::rejected("Save ProjectWorkspace cere un proiect deschis.")
    })?;
    let mut slot = state.project_workspace.lock().map_err(|_| {
        ProjectWorkspaceSaveError::rejected("Nu am putut bloca ProjectWorkspace pentru Save.")
    })?;
    let workspace = slot.as_mut().ok_or_else(|| {
        ProjectWorkspaceSaveError::rejected("ProjectWorkspace nu este inițializat pentru Save.")
    })?;
    let receipt =
        crate::kernel::project_workspace::save_project_workspace(&app, root, workspace, &identity)?;
    if receipt.disk_generation_after != receipt.disk_generation_before {
        schedule_source_browser_refresh(
            &app,
            BrowserPreviewRequestIdentity {
                expected_project_root: workspace.session.project_root.clone(),
                expected_session_id: workspace.runtime_session_id(),
                expected_disk_generation: receipt.disk_generation_after,
            },
        );
    }
    persist_project_workspace_recovery(&app, workspace).map_err(|diagnostic| {
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

pub const PROJECT_WORKSPACE_UNDO_REDO_COMMAND_SCHEMA_VERSION: u32 = 3;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkspaceUndoRedoCommandReceipt {
    pub schema_version: u32,
    pub project_root: String,
    pub runtime_session_id: String,
    pub result: WorkspaceUndoRedoReceipt,
    pub workspace: ProjectWorkspaceSnapshot,
}

#[tauri::command]
pub fn undo_project_workspace(
    identity: ProjectWorkspaceHistoryIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<ProjectWorkspaceUndoRedoCommandReceipt, String> {
    apply_project_workspace_history(app, identity, state, WorkspaceHistoryDirection::Undo)
}

#[tauri::command]
pub fn redo_project_workspace(
    identity: ProjectWorkspaceHistoryIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<ProjectWorkspaceUndoRedoCommandReceipt, String> {
    apply_project_workspace_history(app, identity, state, WorkspaceHistoryDirection::Redo)
}

fn apply_project_workspace_history(
    app: AppHandle,
    identity: ProjectWorkspaceHistoryIdentity,
    state: State<AppState>,
    direction: WorkspaceHistoryDirection,
) -> Result<ProjectWorkspaceUndoRedoCommandReceipt, String> {
    let mut slot = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru Undo/Redo.".to_string())?;
    let workspace = slot
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru Undo/Redo.".to_string())?;
    let workspace_identity = ProjectWorkspaceIdentity {
        expected_project_root: identity.expected_project_root.clone(),
        expected_session_id: identity.expected_session_id.clone(),
        expected_revision: identity.expected_revision,
    };
    let result = commit_project_workspace_session_mutation(&app, workspace, |candidate| {
        candidate.require_history_target(direction, &identity.expected_transaction_id)?;
        match direction {
            WorkspaceHistoryDirection::Undo => {
                candidate.undo(&workspace_identity, file_buffer_now_ms())
            }
            WorkspaceHistoryDirection::Redo => {
                candidate.redo(&workspace_identity, file_buffer_now_ms())
            }
        }
    })?;
    Ok(ProjectWorkspaceUndoRedoCommandReceipt {
        schema_version: PROJECT_WORKSPACE_UNDO_REDO_COMMAND_SCHEMA_VERSION,
        project_root: workspace.session.project_root.clone(),
        runtime_session_id: workspace.runtime_session_id(),
        result,
        workspace: workspace.snapshot(),
    })
}

pub(crate) fn require_recovery_coordinator_clean_for_write(
    state: &State<AppState>,
    session: &ProjectSessionSnapshot,
    caller: &str,
) -> Result<(), String> {
    let scan = state
        .recovery_coordinator_scan
        .lock()
        .map_err(|_| "Nu am putut bloca RecoveryCoordinatorScan.".to_string())?
        .clone()
        .ok_or_else(|| {
            format!(
                "{caller} a blocat scrierea: Transaction Recovery Scan lipsește pentru sesiunea curentă."
            )
        })?;
    if scan.session_id != session.id {
        return Err(format!(
            "{caller} a blocat scrierea: Transaction Recovery Scan aparține sesiunii {}, dar sesiunea curentă este {}.",
            scan.session_id, session.id
        ));
    }
    if scan.project_root != session.project_root {
        return Err(format!(
            "{caller} a blocat scrierea: Transaction Recovery Scan aparține proiectului {}, dar sesiunea curentă este pentru {}.",
            scan.project_root, session.project_root
        ));
    }
    if scan.status != RecoveryCoordinatorStatus::Clean {
        return Err(format!(
            "{caller} a blocat scrierea: Transaction Recovery Scan este {} pentru sesiunea curentă.",
            recovery_coordinator_status_label(scan.status)
        ));
    }
    Ok(())
}

pub(crate) fn require_project_workspace_available_for_write(
    state: &State<AppState>,
) -> Result<(), String> {
    let root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul proiectului pentru mutație.".to_string())?;
    let root = root
        .as_ref()
        .ok_or_else(|| "Nu există proiect curent pentru mutație.".to_string())?;
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru mutație.".to_string())?;
    let workspace = workspace
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru mutație.".to_string())?;
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        root,
    )
}

fn recovery_coordinator_status_label(status: RecoveryCoordinatorStatus) -> &'static str {
    match status {
        RecoveryCoordinatorStatus::Clean => "clean",
        RecoveryCoordinatorStatus::NeedsAttention => "needs_attention",
        RecoveryCoordinatorStatus::Unreadable => "unreadable",
    }
}

fn project_session_root_identity(session: &ProjectSessionSnapshot) -> Result<(u64, u64), String> {
    let device = session
        .root_fingerprint
        .unix_device
        .as_deref()
        .ok_or_else(|| {
            "ProjectSession nu conține identitatea numerică device pentru authority root."
                .to_string()
        })?
        .parse::<u64>()
        .map_err(|error| format!("ProjectSession device identity este invalidă: {error}"))?;
    let inode = session
        .root_fingerprint
        .unix_inode
        .as_deref()
        .ok_or_else(|| {
            "ProjectSession nu conține identitatea numerică inode pentru authority root."
                .to_string()
        })?
        .parse::<u64>()
        .map_err(|error| format!("ProjectSession inode identity este invalidă: {error}"))?;
    Ok((device, inode))
}

fn require_project_transition_for_action(
    app: &AppHandle,
    state: &State<AppState>,
    target_root: &PathBuf,
    action: KernelProjectTransitionAction,
    operator_decision_id: Option<&str>,
) -> Result<(), String> {
    state
        .ai_coordination
        .require_project_transition()
        .map_err(|error| error.to_string())?;
    let coordination = state
        .ai_coordination
        .snapshot(now_ms())
        .map_err(|error| error.to_string())?;
    let ai_reconciliation_reload_authorized = matches!(
        coordination.authority,
        EditAuthority::Reconciling {
            ref project_session_id,
            recovery_reload_authorized: true,
            ..
        } if action == KernelProjectTransitionAction::ReloadProject
            && coordination.project_session_id.as_deref() == Some(project_session_id.as_str())
    );
    if ai_reconciliation_reload_authorized {
        return Ok(());
    }
    let project_state = current_kernel_project_state_snapshot(state)?;
    let policy = evaluate_project_transition_policy(action, &project_state);
    if policy.allows_without_operator() {
        return Ok(());
    }
    if let Some(operator_decision_id) = operator_decision_id {
        if policy.requires_operator_confirmation {
            let evidence = build_project_transition_evidence_for_target(
                state,
                target_root,
                action,
                &project_state,
                &policy,
            )?;
            let session = require_current_project_session(state)?;
            require_matching_kernel_project_transition_decision(
                &session,
                operator_decision_id,
                &evidence,
            )?;
            return Ok(());
        }
    }

    record_project_transition_blocked(app, &policy, target_root);
    Err(policy.guard_error())
}

fn validate_project_transition_action_target(
    action: KernelProjectTransitionAction,
    current_root: &PathBuf,
    target_root: &PathBuf,
) -> Result<(), String> {
    match action {
        KernelProjectTransitionAction::OpenProject if current_root == target_root => Err(
            "Project Transition OpenProject cere un target diferit de proiectul curent."
                .to_string(),
        ),
        KernelProjectTransitionAction::ReloadProject
        | KernelProjectTransitionAction::CloseProject
            if current_root != target_root =>
        {
            Err("Project Transition Reload/Close cere target-ul proiectului curent.".to_string())
        }
        _ => Ok(()),
    }
}

fn build_project_transition_evidence_for_target(
    state: &State<AppState>,
    target_root: &PathBuf,
    action: KernelProjectTransitionAction,
    project_state: &crate::kernel::project_state::KernelProjectStateSnapshot,
    policy: &crate::kernel::project_state::KernelProjectTransitionPolicy,
) -> Result<crate::kernel::project_state::KernelProjectTransitionDecisionEvidence, String> {
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace.".to_string())?;
    let workspace = workspace
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    let session = workspace.session.clone();
    let store = workspace.documents.clone();
    let workspace_snapshot = workspace.snapshot();
    let disk_conflicts = scan_disk_conflicts(&store);
    if policy.action != action {
        return Err("Project Transition Policy nu corespunde acțiunii cerute.".to_string());
    }
    build_kernel_project_transition_decision_evidence(
        &session,
        &store,
        Some(&disk_conflicts),
        &workspace_snapshot,
        project_state,
        policy,
        target_root.to_string_lossy().as_ref(),
    )
}

fn project_transition_action_for_open_target(
    state: &State<AppState>,
    target_root: &PathBuf,
) -> Result<KernelProjectTransitionAction, String> {
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul proiectului curent.".to_string())?
        .clone();
    Ok(if current_root.as_ref() == Some(target_root) {
        KernelProjectTransitionAction::ReloadProject
    } else {
        KernelProjectTransitionAction::OpenProject
    })
}

fn record_project_transition_blocked(
    app: &AppHandle,
    policy: &crate::kernel::project_state::KernelProjectTransitionPolicy,
    target_root: &PathBuf,
) {
    let event = KernelLogEvent::new(
        KernelLogLevel::Warn,
        KernelEventKind::ProjectTransitionBlocked,
        "project_state",
        "project_lifecycle",
        project_transition_operation(policy.action),
        Some(target_root.to_string_lossy().to_string()),
        "Project transition blocked by ProjectState lifecycle policy.",
        Some(policy.guard_error()),
    )
    .with_attribute("action", policy.action)
    .with_attribute("decision", policy.decision)
    .with_attribute("reason", policy.reason)
    .with_attribute("projectStateStatus", policy.project_state_status)
    .with_attribute("projectStateReason", policy.project_state_reason)
    .with_attribute("currentProjectRoot", policy.project_root.clone())
    .with_attribute(
        "targetProjectRoot",
        target_root.to_string_lossy().to_string(),
    )
    .with_attribute("sessionId", policy.session_id.clone())
    .with_attribute(
        "workspaceDirtyResourceCount",
        policy.workspace_dirty_resource_count,
    )
    .with_attribute("workspaceRevision", policy.workspace_revision)
    .with_attribute("workspaceUndoCount", policy.workspace_undo_count)
    .with_attribute("workspaceRedoCount", policy.workspace_redo_count)
    .with_attribute("diskConflictCount", policy.disk_conflict_count)
    .with_attribute("diskBlockingCount", policy.disk_blocking_count);

    if let Err(error) = append_event(app, event) {
        eprintln!(
            "[Pană Studio] project_transition_blocked observability append failed: {}",
            error
        );
    }
}

fn project_transition_operation(action: KernelProjectTransitionAction) -> &'static str {
    match action {
        KernelProjectTransitionAction::OpenProject => "open_project",
        KernelProjectTransitionAction::ReloadProject => "reload_project",
        KernelProjectTransitionAction::CloseProject => "close_project",
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProjectTransitionRuntimeLease {
    current_root: Option<String>,
    project_workspace_fingerprint: Option<String>,
    recovery_fingerprint: Option<String>,
}

fn capture_project_transition_runtime_lease(
    state: &State<AppState>,
) -> Result<ProjectTransitionRuntimeLease, String> {
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut captura root-ul pentru transition lease.".to_string())?;
    let project_workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut captura ProjectWorkspace pentru transition lease.".to_string())?;
    let recovery = state.recovery_coordinator_scan.lock().map_err(|_| {
        "Nu am putut captura RecoveryCoordinatorScan pentru transition lease.".to_string()
    })?;
    project_transition_runtime_lease_from_parts(&current_root, &project_workspace, &recovery)
}

fn project_transition_runtime_lease_from_parts(
    current_root: &Option<PathBuf>,
    project_workspace: &Option<ProjectWorkspace>,
    recovery: &Option<RecoveryCoordinatorScan>,
) -> Result<ProjectTransitionRuntimeLease, String> {
    if let Some(workspace) = project_workspace.as_ref() {
        workspace.accepted_disk.require_identity(
            &workspace.runtime_session_id(),
            &workspace.session.project_root,
        )?;
    }
    Ok(ProjectTransitionRuntimeLease {
        current_root: current_root
            .as_ref()
            .map(|root| root.to_string_lossy().to_string()),
        project_workspace_fingerprint: project_workspace
            .as_ref()
            .map(|workspace| {
                serialize_project_transition_lease("ProjectWorkspace", &workspace.snapshot())
            })
            .transpose()?,
        recovery_fingerprint: recovery
            .as_ref()
            .map(|scan| serialize_project_transition_lease("RecoveryCoordinatorScan", scan))
            .transpose()?,
    })
}

fn serialize_project_transition_lease<T: Serialize>(
    label: &str,
    value: &T,
) -> Result<String, String> {
    serde_json::to_string(value)
        .map_err(|error| format!("{label} nu poate fi serializat pentru lease: {error}"))
}

fn clear_project_runtime_state(
    app: &AppHandle,
    state: &State<AppState>,
    expected_lease: Option<&ProjectTransitionRuntimeLease>,
) -> Result<(), String> {
    state
        .ai_coordination
        .require_project_transition()
        .map_err(|error| error.to_string())?;
    let mut current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul proiectului curent.".to_string())?;
    let mut project_workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace.".to_string())?;
    let mut recovery_coordinator_scan = state
        .recovery_coordinator_scan
        .lock()
        .map_err(|_| "Nu am putut bloca RecoveryCoordinatorScan.".to_string())?;
    if let Some(expected_lease) = expected_lease {
        let live_lease = project_transition_runtime_lease_from_parts(
            &current_root,
            &project_workspace,
            &recovery_coordinator_scan,
        )?;
        if &live_lease != expected_lease {
            return Err(
                "Project Transition close lease a devenit stale; runtime-ul curent nu a fost șters."
                    .to_string(),
            );
        }
    }

    let authority_runtime = app
        .try_state::<WriteAuthorityRuntime>()
        .ok_or_else(|| "WriteAuthorityRuntime lipsește la închiderea proiectului.".to_string())?;
    let mut authority_publication = authority_runtime.project_publication()?;
    authority_publication.revoke();
    *current_root = None;
    *project_workspace = None;
    *recovery_coordinator_scan = None;
    state
        .ai_coordination
        .bind_project(None, crate::kernel::observability::now_ms())
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn record_project_session_closed(app: &AppHandle, session: &ProjectSessionSnapshot) {
    let event = KernelLogEvent::new(
        KernelLogLevel::Info,
        KernelEventKind::SessionClosed,
        "project_session",
        "project_lifecycle",
        "close_project",
        Some(session.project_root.clone()),
        "Project session closed by ProjectTransition lifecycle.",
        None,
    )
    .with_attribute("sessionId", session.id.clone())
    .with_attribute("projectRoot", session.project_root.clone())
    .with_attribute("sessionDir", session.session_dir.clone());

    if let Err(error) = append_event(app, event) {
        eprintln!(
            "[Pană Studio] session_closed observability append failed: {}",
            error
        );
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTransitionDecisionRetentionHotJournalRecoveryCommandResult {
    pub receipt: KernelProjectTransitionDecisionRetentionRecoveryReceipt,
    pub recovery_coordinator: RecoveryCoordinatorScan,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkspaceSaveRecoveryCommandResult {
    pub receipt: ProjectWorkspaceSaveRecoveryReceipt,
    pub recovery_coordinator: RecoveryCoordinatorScan,
    pub workspace: ProjectWorkspaceSnapshot,
}

#[tauri::command]
pub fn recover_project_workspace_save(
    transaction_id: String,
    action: ProjectWorkspaceSaveRecoveryAction,
    diagnostic: String,
    app: AppHandle,
    state: State<AppState>,
) -> Result<ProjectWorkspaceSaveRecoveryCommandResult, String> {
    let session = require_current_project_session(&state)?;
    let root = require_current_project_root(&state)?;
    let receipt = apply_project_workspace_save_recovery(
        &app,
        &session,
        &root,
        &transaction_id,
        action,
        diagnostic,
    )?;

    let scan = scan_project_root(&root)?;
    let documents = bootstrap_file_buffer_store(&app, &session, &root, &scan)?;
    let manifest = read_project_disk_manifest(&root)?;
    let accepted = AcceptedProjectDiskManifest::new(
        session.runtime_instance_id(),
        session.project_root.clone(),
        manifest,
    )?;
    let mut rebuilt = ProjectWorkspace::new(
        session.clone(),
        accepted,
        documents,
        PageJsDraftStore::new(&session),
    )?;
    restore_project_workspace_recovery(&app, &mut rebuilt)?;
    let workspace_snapshot = rebuilt.snapshot();

    {
        let current_root = state.current_root.lock().map_err(|_| {
            "Nu am putut valida root-ul după ProjectWorkspace recovery.".to_string()
        })?;
        if current_root.as_ref() != Some(&root) {
            return Err(
                "ProjectWorkspace recovery a devenit stale: proiectul curent s-a schimbat."
                    .to_string(),
            );
        }
        let mut slot = state
            .project_workspace
            .lock()
            .map_err(|_| "Nu am putut publica ProjectWorkspace recuperat.".to_string())?;
        let live_session = slot
            .as_ref()
            .map(|workspace| workspace.runtime_session_id())
            .ok_or_else(|| "ProjectWorkspace a fost închis în timpul recuperării.".to_string())?;
        if live_session != session.runtime_instance_id() {
            return Err(
                "ProjectWorkspace recovery a devenit stale: instanța sesiunii s-a schimbat."
                    .to_string(),
            );
        }
        *slot = Some(rebuilt);
    }
    refresh_recovery_coordinator_scan(&app, &state, &session, true)?;
    let recovery_coordinator = state
        .recovery_coordinator_scan
        .lock()
        .map_err(|_| "Nu am putut citi scanarea după ProjectWorkspace recovery.".to_string())?
        .clone()
        .ok_or_else(|| {
            "Transaction Recovery Scan lipsește după ProjectWorkspace recovery.".to_string()
        })?;
    Ok(ProjectWorkspaceSaveRecoveryCommandResult {
        receipt,
        recovery_coordinator,
        workspace: workspace_snapshot,
    })
}

#[tauri::command(async)]
pub fn read_file_buffer_text(
    relative_path: String,
    identity: FileBufferRequestIdentity,
    state: State<AppState>,
) -> Result<FileBufferCommandReceipt<FileBufferTextSnapshot>, String> {
    read_file_buffer_text_impl(relative_path, identity, state.inner())
}

fn read_file_buffer_text_impl(
    relative_path: String,
    identity: FileBufferRequestIdentity,
    state: &AppState,
) -> Result<FileBufferCommandReceipt<FileBufferTextSnapshot>, String> {
    with_bound_file_buffer(state, &identity, |_, store| {
        store
            .text_snapshot(&relative_path)
            .ok_or_else(|| format!("FileBufferStore nu are text pentru {relative_path}."))
    })
}

fn with_bound_file_buffer<T>(
    state: &AppState,
    identity: &FileBufferRequestIdentity,
    operation: impl FnOnce(&ProjectSessionSnapshot, &mut FileBufferStore) -> Result<T, String>,
) -> Result<FileBufferCommandReceipt<T>, String> {
    with_bound_project_workspace(state, identity, |workspace| {
        let session = workspace.session.clone();
        let payload = operation(&session, &mut workspace.documents)?;
        Ok(FileBufferCommandReceipt::new(&session, payload))
    })
}

fn with_bound_project_workspace<T>(
    state: &AppState,
    identity: &FileBufferRequestIdentity,
    operation: impl FnOnce(&mut ProjectWorkspace) -> Result<T, String>,
) -> Result<T, String> {
    let current_root_guard = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul curent pentru FileBufferStore.".to_string())?;
    let current_root_path = current_root_guard
        .as_ref()
        .ok_or_else(|| "Nu există proiect curent pentru FileBufferStore.".to_string())?;
    let current_root = current_root_path.to_string_lossy().into_owned();
    let mut project_workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru FileBufferStore.".to_string())?;
    let workspace = project_workspace.as_mut().ok_or_else(|| {
        "ProjectWorkspace nu este inițializat pentru FileBufferStore.".to_string()
    })?;
    require_file_buffer_session_binding(
        &current_root,
        &workspace.session,
        &workspace.documents,
        identity,
    )?;
    operation(workspace)
}

#[tauri::command(async)]
pub fn set_file_buffer_draft(
    relative_path: String,
    contents: String,
    expectation: FileBufferMutationExpectation,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<FileBufferCommandReceipt<FileBufferFileSnapshot>, String> {
    set_file_buffer_draft_impl(
        relative_path,
        contents,
        expectation,
        identity,
        &app,
        state.inner(),
    )
}

fn set_file_buffer_draft_impl(
    relative_path: String,
    contents: String,
    expectation: FileBufferMutationExpectation,
    identity: FileBufferRequestIdentity,
    app: &AppHandle,
    state: &AppState,
) -> Result<FileBufferCommandReceipt<FileBufferFileSnapshot>, String> {
    with_bound_project_workspace(state, &identity, |workspace| {
        let file = commit_project_workspace_session_mutation(app, workspace, |candidate| {
            let mut validation_store = candidate.documents.clone();
            validation_store.set_draft_if_current(
                &relative_path,
                contents.clone(),
                &expectation,
                file_buffer_now_ms(),
            )?;
            let receipt = candidate.stage_document_texts(
                &workspace_identity(candidate),
                WorkspaceMutationMetadata {
                    label: "Editare document".to_string(),
                    source: "code_editor.full_draft".to_string(),
                    coalesce_key: Some(format!("document:{relative_path}")),
                    transaction_id: None,
                },
                vec![WorkspaceDocumentMutation {
                    relative_path: relative_path.clone(),
                    contents,
                }],
                file_buffer_now_ms(),
            )?;
            receipt
                .files
                .into_iter()
                .next()
                .ok_or_else(|| "ProjectWorkspace nu a returnat documentul editat.".to_string())
        })?;
        Ok(FileBufferCommandReceipt::new(&workspace.session, file))
    })
}

#[tauri::command(async)]
pub fn apply_file_buffer_changeset(
    input: FileBufferChangeSetInput,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<FileBufferCommandReceipt<FileBufferChangeSetResult>, String> {
    apply_file_buffer_changeset_impl(input, identity, &app, state.inner())
}

fn apply_file_buffer_changeset_impl(
    input: FileBufferChangeSetInput,
    identity: FileBufferRequestIdentity,
    app: &AppHandle,
    state: &AppState,
) -> Result<FileBufferCommandReceipt<FileBufferChangeSetResult>, String> {
    with_bound_project_workspace(state, &identity, |workspace| {
        let source = input
            .source
            .clone()
            .unwrap_or_else(|| "code_editor.changeset".to_string());
        let relative_path = input.relative_path.clone();
        let result = commit_project_workspace_session_mutation(app, workspace, |candidate| {
            candidate.apply_document_changeset(
                &workspace_identity(candidate),
                WorkspaceMutationMetadata {
                    label: "Editare document".to_string(),
                    source,
                    coalesce_key: Some(format!("document:{relative_path}")),
                    transaction_id: None,
                },
                input,
                file_buffer_now_ms(),
            )
        })?;
        Ok(FileBufferCommandReceipt::new(&workspace.session, result))
    })
}

#[tauri::command(async)]
pub fn clear_file_buffer_draft(
    relative_path: String,
    expectation: FileBufferMutationExpectation,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<FileBufferCommandReceipt<FileBufferFileSnapshot>, String> {
    clear_file_buffer_draft_impl(relative_path, expectation, identity, &app, state.inner())
}

fn clear_file_buffer_draft_impl(
    relative_path: String,
    expectation: FileBufferMutationExpectation,
    identity: FileBufferRequestIdentity,
    app: &AppHandle,
    state: &AppState,
) -> Result<FileBufferCommandReceipt<FileBufferFileSnapshot>, String> {
    with_bound_project_workspace(state, &identity, |workspace| {
        let file = commit_project_workspace_session_mutation(app, workspace, |candidate| {
            let mut validation_store = candidate.documents.clone();
            validation_store.clear_draft_if_current(&relative_path, &expectation)?;
            let baseline = candidate
                .documents
                .baseline_text_for(&relative_path)
                .ok_or_else(|| {
                    format!("ProjectWorkspace nu are baseline pentru {relative_path}.")
                })?;
            let receipt = candidate.stage_document_texts(
                &workspace_identity(candidate),
                WorkspaceMutationMetadata {
                    label: "Renunțare la modificările documentului".to_string(),
                    source: "code_editor.clear_draft".to_string(),
                    coalesce_key: None,
                    transaction_id: None,
                },
                vec![WorkspaceDocumentMutation {
                    relative_path,
                    contents: baseline,
                }],
                file_buffer_now_ms(),
            )?;
            receipt
                .files
                .into_iter()
                .next()
                .ok_or_else(|| "ProjectWorkspace nu a returnat documentul curățat.".to_string())
        })?;
        Ok(FileBufferCommandReceipt::new(&workspace.session, file))
    })
}

fn workspace_identity(workspace: &ProjectWorkspace) -> ProjectWorkspaceIdentity {
    ProjectWorkspaceIdentity {
        expected_project_root: workspace.session.project_root.clone(),
        expected_session_id: workspace.runtime_session_id(),
        expected_revision: workspace.revision,
    }
}

#[tauri::command]
pub fn scan_project(
    path: String,
    state: State<AppState>,
) -> Result<crate::project::ProjectScan, String> {
    let requested_root = PathBuf::from(path);
    let projection = {
        let current_root = state
            .current_root
            .lock()
            .map_err(|_| "Nu am putut valida root-ul pentru ProjectScan.".to_string())?;
        if current_root.as_ref() != Some(&requested_root) {
            return Err(
                "ProjectScan a refuzat un root diferit de ProjectSession activă.".to_string(),
            );
        }
        let workspace = state.project_workspace.lock().map_err(|_| {
            "Nu am putut bloca ProjectWorkspace pentru scanarea proiectului.".to_string()
        })?;
        let workspace = workspace.as_ref().ok_or_else(|| {
            "ProjectWorkspace nu este inițializat pentru ProjectScan.".to_string()
        })?;
        workspace.capture_projection_lease()?
    };
    let scan = scan_project_workspace_projection(&projection)?;
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut revalida root-ul pentru ProjectScan.".to_string())?;
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut revalida ProjectWorkspace pentru ProjectScan.".to_string())?;
    if current_root.as_ref() != Some(&requested_root) {
        return Err("ProjectScan a devenit stale în timpul construcției.".to_string());
    }
    workspace
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace a dispărut în timpul ProjectScan.".to_string())?
        .require_current_projection(&projection)?;
    Ok(scan)
}

#[tauri::command]
pub fn read_current_project_disk_manifest(
    state: State<AppState>,
) -> Result<crate::project::ProjectDiskManifest, String> {
    let root = require_current_project_root(&state)?;
    read_project_disk_manifest(&root)
}

#[tauri::command]
pub fn close_project(
    operator_decision_id: Option<String>,
    app: AppHandle,
    state: State<AppState>,
) -> Result<(), String> {
    let Some(root) = current_project_root(&state) else {
        let transition_lease = capture_project_transition_runtime_lease(&state)?;
        clear_project_runtime_state(&app, &state, Some(&transition_lease))?;
        stop_source_browser(&app, state.inner());
        stop_project_preview(&app, state.inner());
        return Ok(());
    };

    require_project_transition_for_action(
        &app,
        &state,
        &root,
        KernelProjectTransitionAction::CloseProject,
        operator_decision_id.as_deref(),
    )?;
    let transition_lease = capture_project_transition_runtime_lease(&state)?;

    let session = current_project_session(&state)?;

    clear_project_runtime_state(&app, &state, Some(&transition_lease))?;
    stop_source_browser(&app, state.inner());
    stop_project_preview(&app, state.inner());

    if let Some(session) = session {
        record_project_session_closed(&app, &session);
    }

    Ok(())
}

#[tauri::command]
pub fn inspect_project_open_recovery(
    path: String,
    app: AppHandle,
) -> Result<ProjectOpenRecoveryAssessment, String> {
    let root = PathBuf::from(path)
        .canonicalize()
        .map_err(|error| format!("Nu am putut rezolva folderul: {error}"))?;
    let manifest = read_project_disk_manifest(&root)?;
    let root_fingerprint = fingerprint_project_root(&root)?;
    inspect_project_workspace_recovery_for_open(&app, &root, &manifest, &root_fingerprint)
}

#[tauri::command]
pub fn open_project(
    path: String,
    operator_decision_id: Option<String>,
    recovery_decision: Option<ProjectOpenRecoveryDecisionInput>,
    app: AppHandle,
    state: State<AppState>,
) -> Result<crate::project::ProjectScan, String> {
    println!("[Pană Studio] open_project invoked: {}", path);
    app.state::<WriteAuthorityRuntime>()
        .require_recovery_clean()?;
    let root = PathBuf::from(path)
        .canonicalize()
        .map_err(|error| format!("Nu am putut rezolva folderul: {}", error))?;
    let action = project_transition_action_for_open_target(&state, &root)?;
    let reset_session_history = action == KernelProjectTransitionAction::ReloadProject;
    require_project_transition_for_action(
        &app,
        &state,
        &root,
        action,
        operator_decision_id.as_deref(),
    )?;
    let transition_runtime_lease = capture_project_transition_runtime_lease(&state)?;
    let bootstrap_manifest = read_project_disk_manifest(&root)?;

    // Scan the folder regardless — user can init Zola via Deploy pane if needed.
    let mut scan = scan_project_root(&root)?;
    println!(
        "[Pană Studio] open_project scanned: {} files, zola={}, empty={}",
        scan.files.len(),
        scan.is_zola,
        scan.is_empty
    );

    let session = prepare_project_session(&app, &root, &scan)?;
    let (project_device, project_inode) = project_session_root_identity(&session)?;
    let authority_runtime = app
        .try_state::<WriteAuthorityRuntime>()
        .ok_or_else(|| "WriteAuthorityRuntime lipsește la deschiderea proiectului.".to_string())?;
    let pending_project_authority = authority_runtime.capture_pending_project(
        session.runtime_instance_id(),
        PathBuf::from(&session.project_root),
        project_device,
        project_inode,
    )?;
    let page_js_draft_store = PageJsDraftStore::new(&session);
    let recovery_coordinator_scan = scan_recovery_coordinator(&app, &session)?;
    let file_buffer_store = bootstrap_file_buffer_store(&app, &session, &root, &scan)?;
    let verified_manifest = read_project_disk_manifest(&root)?;
    if verified_manifest != bootstrap_manifest {
        return Err(
            "Proiectul s-a modificat pe disk în timpul bootstrap-ului; sesiunea nu a fost publicată. Reîncearcă deschiderea."
                .to_string(),
        );
    }
    scan.kernel_session_id = Some(session.runtime_instance_id());
    scan.accepted_disk_manifest = Some(verified_manifest);
    let commit_manifest = read_project_disk_manifest(&root)?;
    if scan.accepted_disk_manifest.as_ref() != Some(&commit_manifest) {
        return Err(
            "Manifestul proiectului s-a schimbat înainte de publicarea sesiunii; tranziția a fost refuzată."
            .to_string(),
        );
    }
    let next_accepted_disk_manifest = AcceptedProjectDiskManifest::new(
        session.runtime_instance_id(),
        session.project_root.clone(),
        commit_manifest.clone(),
    )?;
    let mut next_project_workspace = ProjectWorkspace::new(
        session.clone(),
        next_accepted_disk_manifest,
        file_buffer_store,
        page_js_draft_store,
    )?;
    let recovery_preflight_enabled = !reset_session_history
        && recovery_coordinator_scan
            .hot_project_workspace_save_journals
            .is_empty();
    if !recovery_preflight_enabled && recovery_decision.is_some() {
        return Err(
            "Decizia project-open recovery nu este validă pentru această tranziție.".to_string(),
        );
    }
    let recovery_assessment = if recovery_preflight_enabled {
        Some(inspect_project_workspace_recovery_for_open(
            &app,
            &root,
            &commit_manifest,
            &session.root_fingerprint,
        )?)
    } else {
        None
    };
    let recovery_resolution = recovery_assessment
        .as_ref()
        .map(|assessment| resolve_project_open_recovery(assessment, recovery_decision.as_ref()))
        .transpose()?
        .unwrap_or(ProjectOpenRecoveryResolution::Skip);
    if recovery_resolution == ProjectOpenRecoveryResolution::Restore {
        restore_project_workspace_recovery(&app, &mut next_project_workspace)?;
    }
    let retire_abandoned_recovery = recovery_assessment.as_ref().is_some_and(|assessment| {
        matches!(
            assessment.status,
            crate::kernel::project_workspace::ProjectOpenRecoveryStatus::Abandoned
                | crate::kernel::project_workspace::ProjectOpenRecoveryStatus::DecisionRequired
        ) && recovery_resolution != ProjectOpenRecoveryResolution::Restore
    });
    let authoritative_scan =
        scan_project_workspace_projection(&next_project_workspace.capture_projection_lease()?)?;
    require_project_transition_for_action(
        &app,
        &state,
        &root,
        action,
        operator_decision_id.as_deref(),
    )?;
    let opened_session_for_event = session.clone();

    {
        let mut current_root = state
            .current_root
            .lock()
            .map_err(|_| "Nu am putut bloca starea proiectului.".to_string())?;
        let mut project_workspace = state
            .project_workspace
            .lock()
            .map_err(|_| "Nu am putut bloca ProjectWorkspace.".to_string())?;
        let mut recovery_scan = state
            .recovery_coordinator_scan
            .lock()
            .map_err(|_| "Nu am putut bloca RecoveryCoordinatorScan.".to_string())?;
        let live_transition_lease = project_transition_runtime_lease_from_parts(
            &current_root,
            &project_workspace,
            &recovery_scan,
        )?;
        if live_transition_lease != transition_runtime_lease {
            return Err(
                "Project Transition lease a devenit stale înainte de commit; nicio sesiune nouă nu a fost publicată."
                    .to_string(),
            );
        }
        let manifest_at_commit = read_project_disk_manifest(&root)?;
        if scan.accepted_disk_manifest.as_ref() != Some(&manifest_at_commit) {
            return Err(
                "Manifestul proiectului s-a schimbat la punctul de commit; runtime-ul vechi a rămas intact."
                    .to_string(),
            );
        }
        if let Some(initial_assessment) = recovery_assessment.as_ref() {
            let commit_assessment = inspect_project_workspace_recovery_for_open(
                &app,
                &root,
                &manifest_at_commit,
                &session.root_fingerprint,
            )?;
            require_project_open_recovery_assessment_unchanged(
                initial_assessment,
                &commit_assessment,
            )?;
            let commit_resolution =
                resolve_project_open_recovery(&commit_assessment, recovery_decision.as_ref())?;
            if commit_resolution != recovery_resolution {
                return Err(
                    "Rezoluția project-open recovery s-a schimbat înainte de commit.".to_string(),
                );
            }
            if commit_resolution == ProjectOpenRecoveryResolution::ExplicitAbandon {
                let decision = recovery_decision.as_ref().ok_or_else(|| {
                    "Lipsește decizia explicită de abandonare la commit.".to_string()
                })?;
                persist_project_open_recovery_abandonment(&app, &commit_assessment, decision)?;
            }
        }
        persist_project_session_open(&app, &session)?;
        if reset_session_history {
            clear_project_workspace_recovery(&app, &session.project_root)?;
            clear_project_open_recovery_decision(&app, &session.project_root)?;
        }
        pending_project_authority.verify_path_binding()?;
        let mut authority_publication = authority_runtime.project_publication()?;
        authority_publication.publish(pending_project_authority)?;
        *current_root = Some(root.clone());
        *project_workspace = Some(next_project_workspace);
        *recovery_scan = Some(recovery_coordinator_scan);
        state
            .ai_coordination
            .bind_project(
                Some(session.runtime_instance_id()),
                crate::kernel::observability::now_ms(),
            )
            .map_err(|error| error.to_string())?;
    }
    if retire_abandoned_recovery {
        match clear_project_workspace_recovery(&app, &session.project_root) {
            Ok(()) => {
                if let Err(error) =
                    clear_project_open_recovery_decision(&app, &session.project_root)
                {
                    eprintln!(
                        "[Pană Studio] marker-ul project-open recovery nu a putut fi curățat după publicare: {error}"
                    );
                }
            }
            Err(error) => {
                // The explicit marker deliberately remains durable. A restart
                // will continue to ignore exactly this recovery/manifest pair,
                // while the old draft bytes are still preserved for diagnosis.
                eprintln!(
                    "[Pană Studio] recovery-ul abandonat nu a putut fi retras după publicare; marker-ul explicit rămâne activ: {error}"
                );
            }
        }
    }
    record_project_session_opened(&app, &opened_session_for_event);
    stop_source_browser(&app, state.inner());
    stop_project_preview(&app, state.inner());
    println!(
        "[Pană Studio] open_project current_root set: {}",
        root.display()
    );

    Ok(authoritative_scan)
}

#[tauri::command]
pub fn zola_init(path: String, app: AppHandle) -> Result<String, String> {
    init_project_with_starter(&app, &PathBuf::from(path))
}

#[tauri::command]
pub fn read_project_file(relative_path: String, state: State<AppState>) -> Result<String, String> {
    require_current_project_root(&state)?;
    let project_workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace.".to_string())?;
    let store = &project_workspace
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?
        .documents;
    if let Some(text) = store.text_for(&relative_path) {
        return Ok(text);
    }
    Err(format!(
        "ProjectWorkspace nu urmărește documentul text {relative_path}; citirea paralelă direct de pe disc este interzisă."
    ))
}

pub(crate) fn refresh_recovery_coordinator_scan<R: Runtime>(
    app: &AppHandle<R>,
    state: &State<AppState>,
    session: &ProjectSessionSnapshot,
    command_succeeded: bool,
) -> Result<(), String> {
    match scan_recovery_coordinator(app, session) {
        Ok(scan) => {
            let live_workspace = state.project_workspace.lock().map_err(|_| {
                "Nu am putut bloca ProjectWorkspace pentru recovery CAS.".to_string()
            })?;
            let Some(live_session) = live_workspace.as_ref().map(|workspace| &workspace.session)
            else {
                return Err(
                    "Transaction Recovery Scan a refuzat publicarea după închiderea sesiunii."
                        .to_string(),
                );
            };
            if live_session.runtime_instance_id() != session.runtime_instance_id() {
                return Err(
                    "Transaction Recovery Scan a refuzat publicarea într-o altă instanță ProjectSession."
                        .to_string(),
                );
            }
            let mut recovery_slot = state
                .recovery_coordinator_scan
                .lock()
                .map_err(|_| "Nu am putut bloca RecoveryCoordinatorScan.".to_string())?;
            *recovery_slot = Some(scan);
            Ok(())
        }
        Err(error) => {
            if let Ok(live_workspace) = state.project_workspace.lock() {
                let matches_live_session = live_workspace.as_ref().is_some_and(|workspace| {
                    let live = &workspace.session;
                    live.runtime_instance_id() == session.runtime_instance_id()
                });
                if matches_live_session {
                    if let Ok(mut recovery_slot) = state.recovery_coordinator_scan.lock() {
                        *recovery_slot = None;
                    }
                }
            }
            if command_succeeded {
                return Err(format!(
                    "Comanda a rulat, dar Transaction Recovery Scan nu a putut fi actualizat: {error}"
                ));
            }
            Ok(())
        }
    }
}
