use std::path::{Path, PathBuf};

use tauri::{AppHandle, State};

use super::contracts::ProjectTransitionDecisionRetentionHotJournalRecoveryCommandResult;
use crate::{
    commands::kernel::current_kernel_project_state_snapshot,
    kernel::{
        ai_coordination::EditAuthority,
        disk_conflict::scan_disk_conflicts,
        observability::{append_event, now_ms, KernelEventKind, KernelLogEvent, KernelLogLevel},
        project_runtime_access::{
            refresh_recovery_coordinator_scan, require_current_project_session,
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
        },
        project_workspace::ProjectWorkspace,
        recovery_coordinator::{RecoveryCoordinatorScan, RecoveryCoordinatorStatus},
    },
    state::AppState,
};

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

pub(super) fn require_project_transition_for_action<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &State<AppState>,
    target_root: &Path,
    action: KernelProjectTransitionAction,
    operator_decision_id: Option<&str>,
) -> Result<(), String> {
    state
        .versioning_network_operation
        .require_project_transition_allowed()?;
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
    target_root: &Path,
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
    target_root: &Path,
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
    let workspace_snapshot = workspace.snapshot();
    let disk_conflicts = scan_disk_conflicts(&workspace.documents);
    if policy.action != action {
        return Err("Project Transition Policy nu corespunde acțiunii cerute.".to_string());
    }
    build_kernel_project_transition_decision_evidence(
        &workspace.session,
        &workspace.documents,
        Some(&disk_conflicts),
        &workspace_snapshot,
        project_state,
        policy,
        target_root.to_string_lossy().as_ref(),
    )
}

pub(super) fn project_transition_action_for_open_target(
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

fn record_project_transition_blocked<R: tauri::Runtime>(
    app: &AppHandle<R>,
    policy: &crate::kernel::project_state::KernelProjectTransitionPolicy,
    target_root: &Path,
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
pub(super) struct ProjectTransitionRuntimeLease {
    current_root: Option<String>,
    project_workspace: Option<ProjectTransitionWorkspaceLease>,
    recovery: Option<ProjectTransitionRecoveryLease>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProjectTransitionWorkspaceLease {
    schema_version: u32,
    project_root: String,
    runtime_session_id: String,
    revision: u64,
    disk_generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProjectTransitionRecoveryLease {
    schema_version: u32,
    session_id: String,
    project_root: String,
    scanned_at_ms: u128,
    status: RecoveryCoordinatorStatus,
    workspace_journals: Vec<(String, u64, u128)>,
    transition_journals: Vec<(String, u128, String, String, String)>,
    diagnostics: Vec<(String, Option<String>)>,
}

pub(super) fn capture_project_transition_runtime_lease(
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

pub(super) fn project_transition_runtime_lease_from_parts(
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
        project_workspace: project_workspace.as_ref().map(|workspace| {
            ProjectTransitionWorkspaceLease {
                schema_version: workspace.schema_version,
                project_root: workspace.session.project_root.clone(),
                runtime_session_id: workspace.runtime_session_id(),
                revision: workspace.revision,
                disk_generation: workspace.accepted_disk.generation,
            }
        }),
        recovery: recovery
            .as_ref()
            .map(|scan| ProjectTransitionRecoveryLease {
                schema_version: scan.schema_version,
                session_id: scan.session_id.clone(),
                project_root: scan.project_root.clone(),
                scanned_at_ms: scan.scanned_at_ms,
                status: scan.status,
                workspace_journals: scan
                    .hot_project_workspace_save_journals
                    .iter()
                    .map(|journal| {
                        (
                            journal.transaction_id.clone(),
                            journal.revision,
                            journal.prepared_at_ms,
                        )
                    })
                    .collect(),
                transition_journals: scan
                    .hot_project_transition_decision_retention_journals
                    .iter()
                    .map(|journal| {
                        (
                            journal.retention_id.clone(),
                            journal.created_at_ms,
                            journal.before_journal_hash.clone(),
                            journal.after_journal_hash.clone(),
                            journal.archive_hash.clone(),
                        )
                    })
                    .collect(),
                diagnostics: scan
                    .diagnostics
                    .iter()
                    .map(|diagnostic| (diagnostic.code.clone(), diagnostic.transaction_id.clone()))
                    .collect(),
            }),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{atomic::AtomicBool, Arc};

    use tauri::Manager;

    use crate::{
        kernel::recovery_coordinator::RECOVERY_COORDINATOR_SCHEMA_VERSION,
        project::{AcceptedProjectDiskManifest, ProjectDiskManifest},
        versioning::{VersionNetworkOperationKind, VersionNetworkOperationLease},
    };

    use super::*;

    #[test]
    fn transition_action_must_match_the_exact_current_and_target_roots() {
        let current = PathBuf::from("/project/current");
        assert!(validate_project_transition_action_target(
            KernelProjectTransitionAction::OpenProject,
            &current,
            Path::new("/project/next"),
        )
        .is_ok());
        assert!(validate_project_transition_action_target(
            KernelProjectTransitionAction::OpenProject,
            &current,
            &current,
        )
        .unwrap_err()
        .contains("target diferit"));
        assert!(validate_project_transition_action_target(
            KernelProjectTransitionAction::ReloadProject,
            &current,
            Path::new("/project/next"),
        )
        .unwrap_err()
        .contains("proiectului curent"));
    }

    #[test]
    fn active_remote_operation_rejects_project_transition_immediately() {
        let root = "/project/current".to_string();
        let session_id = "transition-session".to_string();
        let operation_id = "fetch-transition-12345678";
        let accepted_disk = AcceptedProjectDiskManifest::new(
            &session_id,
            &root,
            ProjectDiskManifest {
                root: root.clone(),
                files: Vec::new(),
                truncated: false,
                max_files: 1_000,
            },
        )
        .unwrap();
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock app");
        app.handle().manage(AppState::default());
        let state = app.state::<AppState>();
        state
            .versioning_network_operation
            .begin(
                VersionNetworkOperationLease {
                    operation_id: operation_id.to_string(),
                    project_root: root.clone(),
                    session_id,
                    kind: VersionNetworkOperationKind::Fetch,
                    workspace_revision: 1,
                    disk_generation: accepted_disk.generation,
                    accepted_disk: Arc::new(accepted_disk),
                    expected_status_token: "status-before".to_string(),
                    expected_head_oid: None,
                },
                Arc::new(AtomicBool::new(false)),
            )
            .unwrap();

        let started = std::time::Instant::now();
        let error = require_project_transition_for_action(
            app.handle(),
            &state,
            Path::new(&root),
            KernelProjectTransitionAction::CloseProject,
            None,
        )
        .unwrap_err();
        assert!(error.contains(operation_id), "{error}");
        assert!(started.elapsed() < std::time::Duration::from_millis(50));

        state.versioning_network_operation.abandon(operation_id);
    }

    #[test]
    fn compact_transition_lease_rejects_root_and_recovery_generation_changes() {
        let root = Some(PathBuf::from("/project/current"));
        let recovery = RecoveryCoordinatorScan {
            schema_version: RECOVERY_COORDINATOR_SCHEMA_VERSION,
            session_id: "runtime-session".to_string(),
            project_root: "/project/current".to_string(),
            scanned_at_ms: 10,
            status: RecoveryCoordinatorStatus::Clean,
            hot_project_workspace_save_journals: Vec::new(),
            hot_project_transition_decision_retention_journals: Vec::new(),
            hot_journal_families: Vec::new(),
            diagnostics: Vec::new(),
        };
        let lease =
            project_transition_runtime_lease_from_parts(&root, &None, &Some(recovery.clone()))
                .unwrap();
        let same =
            project_transition_runtime_lease_from_parts(&root, &None, &Some(recovery.clone()))
                .unwrap();
        assert_eq!(lease, same);

        let different_root = project_transition_runtime_lease_from_parts(
            &Some(PathBuf::from("/project/next")),
            &None,
            &Some(recovery.clone()),
        )
        .unwrap();
        assert_ne!(lease, different_root);

        let mut refreshed_recovery = recovery;
        refreshed_recovery.scanned_at_ms += 1;
        let refreshed =
            project_transition_runtime_lease_from_parts(&root, &None, &Some(refreshed_recovery))
                .unwrap();
        assert_ne!(lease, refreshed);
    }
}
