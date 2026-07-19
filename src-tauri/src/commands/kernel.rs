use tauri::{AppHandle, Manager, State};

use super::kernel_preview_context::require_preview_command_identity;
use super::kernel_preview_pipeline::{
    run_preview_read_command, run_preview_structural_write_command,
};

use crate::{
    kernel::{
        disk_conflict::{scan_disk_conflicts, KernelDiskConflictSnapshot},
        observability::{
            append_event, read_kernel_observability_log_snapshot, KernelEventKind, KernelLogEvent,
            KernelLogLevel, KernelObservabilityLogRequest, KernelObservabilityLogSnapshot,
            KernelObservabilityLogSourceFilter,
        },
        preview_projection::{
            execute_preview_html_attributes, execute_preview_html_delete,
            execute_preview_html_duplicate, execute_preview_html_insert_drop,
            execute_preview_html_tag, execute_preview_html_text, execute_preview_layer_drop,
            execute_preview_template_edit_permission, execute_preview_tera_delete,
            execute_preview_tera_insert_drop, execute_preview_tera_move_drop,
            preflight_preview_projection_intent, PreviewHtmlAttributesExecutionInput,
            PreviewHtmlAttributesExecutionReceipt, PreviewHtmlDeleteExecutionInput,
            PreviewHtmlDeleteExecutionReceipt, PreviewHtmlDuplicateExecutionInput,
            PreviewHtmlDuplicateExecutionReceipt, PreviewHtmlInsertDropExecutionInput,
            PreviewHtmlInsertDropExecutionReceipt, PreviewHtmlTagExecutionInput,
            PreviewHtmlTagExecutionReceipt, PreviewHtmlTextExecutionInput,
            PreviewHtmlTextExecutionReceipt, PreviewLayerDropExecutionInput,
            PreviewLayerDropExecutionReceipt, PreviewProjectionIntentInput,
            PreviewProjectionIntentReceipt, PreviewProjectionIntentStatus,
            PreviewStructuralCommandIdentity, PreviewTemplateEditPermissionInput,
            PreviewTemplateEditPermissionReceipt, PreviewTeraDeleteExecutionInput,
            PreviewTeraDeleteExecutionReceipt, PreviewTeraInsertDropExecutionInput,
            PreviewTeraInsertDropExecutionReceipt, PreviewTeraMoveDropExecutionInput,
            PreviewTeraMoveDropExecutionReceipt,
        },
        project_session::ProjectSessionSnapshot,
        project_state::{
            build_kernel_project_state_snapshot, build_project_transition_policy_matrix,
            evaluate_project_transition_policy,
            read_kernel_project_transition_blocked_audit_snapshot,
            read_kernel_project_transition_decision_journal_snapshot,
            read_kernel_project_transition_decision_recovery_ack_journal_snapshot,
            scan_project_transition_decision_retention_hot_journals, KernelProjectStateSnapshot,
            KernelProjectTransitionAction, KernelProjectTransitionBlockedAuditSnapshot,
            KernelProjectTransitionDecisionJournalSnapshot,
            KernelProjectTransitionDecisionRecoveryAckJournalSnapshot,
            KernelProjectTransitionDecisionRetentionHotJournal, KernelProjectTransitionPolicy,
            KernelProjectTransitionPolicyMatrixSnapshot,
        },
        write_authority::{
            WriteAuthorityRecoveryResolutionInput, WriteAuthorityRecoveryResolutionReceipt,
            WriteAuthorityRecoveryScan, WriteAuthorityRuntime,
        },
    },
    state::AppState,
};

#[tauri::command]
pub async fn read_write_authority_recovery_scan(
    app: AppHandle,
) -> Result<WriteAuthorityRecoveryScan, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<WriteAuthorityRuntime>().recovery_scan()
    })
    .await
    .map_err(|error| format!("WriteAuthority WAL rescan task eșuat: {error}"))?
}

#[tauri::command]
pub async fn resolve_write_authority_recovery(
    app: AppHandle,
    input: WriteAuthorityRecoveryResolutionInput,
) -> Result<WriteAuthorityRecoveryResolutionReceipt, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<WriteAuthorityRuntime>().resolve_recovery(input)
    })
    .await
    .map_err(|error| format!("WriteAuthority WAL operator task eșuat: {error}"))?
}

#[tauri::command(async)]
pub fn normalize_preview_projection_intent(
    app: AppHandle,
    state: State<AppState>,
    input: PreviewProjectionIntentInput,
    identity: PreviewStructuralCommandIdentity,
) -> Result<PreviewProjectionIntentReceipt, String> {
    let session = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace.".to_string())?
        .as_ref()
        .map(|workspace| workspace.session.clone());
    let live_session = session
        .as_ref()
        .ok_or_else(|| "Preview Projection cere un ProjectSession activ.".to_string())?;
    require_preview_command_identity(live_session, &identity)?;
    let receipt = preflight_preview_projection_intent(input, session.as_ref());
    log_preview_projection_intent(&app, &receipt);
    Ok(receipt)
}

#[tauri::command(async)]
pub fn execute_preview_template_edit_intent(
    app: AppHandle,
    state: State<AppState>,
    input: PreviewTemplateEditPermissionInput,
    identity: PreviewStructuralCommandIdentity,
) -> Result<PreviewTemplateEditPermissionReceipt, String> {
    run_preview_read_command(&state, &identity, |context, store| {
        execute_preview_template_edit_permission(
            &app,
            &context.session,
            &context.root,
            store,
            input,
        )
        .map(|outcome| outcome.receipt)
    })
}

#[tauri::command(async)]
pub fn execute_preview_layer_drop_intent(
    app: AppHandle,
    state: State<AppState>,
    input: PreviewLayerDropExecutionInput,
    identity: PreviewStructuralCommandIdentity,
) -> Result<PreviewLayerDropExecutionReceipt, String> {
    run_preview_structural_write_command(
        &app,
        &state,
        &identity,
        "Preview layer drop",
        |context, workspace| {
            execute_preview_layer_drop(
                &app,
                &context.session,
                &context.root,
                workspace,
                input,
                &context.aliases,
            )
        },
    )
}

#[tauri::command(async)]
pub fn execute_preview_html_insert_drop_intent(
    app: AppHandle,
    state: State<AppState>,
    input: PreviewHtmlInsertDropExecutionInput,
    identity: PreviewStructuralCommandIdentity,
) -> Result<PreviewHtmlInsertDropExecutionReceipt, String> {
    run_preview_structural_write_command(
        &app,
        &state,
        &identity,
        "Preview HTML insert drop",
        |context, workspace| {
            execute_preview_html_insert_drop(
                &app,
                &context.session,
                &context.root,
                workspace,
                input,
                &context.aliases,
            )
        },
    )
}

#[tauri::command(async)]
pub fn execute_preview_html_attributes_intent(
    app: AppHandle,
    state: State<AppState>,
    input: PreviewHtmlAttributesExecutionInput,
    identity: PreviewStructuralCommandIdentity,
) -> Result<PreviewHtmlAttributesExecutionReceipt, String> {
    run_preview_structural_write_command(
        &app,
        &state,
        &identity,
        "Preview HTML attributes",
        |context, workspace| {
            execute_preview_html_attributes(
                &app,
                &context.session,
                &context.root,
                workspace,
                input,
                &context.aliases,
            )
        },
    )
}

#[tauri::command(async)]
pub fn execute_preview_html_text_intent(
    app: AppHandle,
    state: State<AppState>,
    input: PreviewHtmlTextExecutionInput,
    identity: PreviewStructuralCommandIdentity,
) -> Result<PreviewHtmlTextExecutionReceipt, String> {
    let preview_projection = if input.defer_canonical_projection {
        crate::kernel::project_workspace::ProjectWorkspacePreviewProjection::Deferred
    } else {
        crate::kernel::project_workspace::ProjectWorkspacePreviewProjection::Required
    };
    super::kernel_preview_pipeline::run_preview_structural_write_command_with_projection(
        &app,
        &state,
        &identity,
        "Preview HTML text",
        preview_projection,
        |context, workspace| {
            execute_preview_html_text(
                &app,
                &context.session,
                &context.root,
                workspace,
                input,
                &context.aliases,
            )
        },
    )
}

#[tauri::command(async)]
pub fn execute_preview_html_tag_intent(
    app: AppHandle,
    state: State<AppState>,
    input: PreviewHtmlTagExecutionInput,
    identity: PreviewStructuralCommandIdentity,
) -> Result<PreviewHtmlTagExecutionReceipt, String> {
    run_preview_structural_write_command(
        &app,
        &state,
        &identity,
        "Preview HTML tag",
        |context, workspace| {
            execute_preview_html_tag(
                &app,
                &context.session,
                &context.root,
                workspace,
                input,
                &context.aliases,
            )
        },
    )
}

#[tauri::command(async)]
pub fn execute_preview_html_duplicate_intent(
    app: AppHandle,
    state: State<AppState>,
    input: PreviewHtmlDuplicateExecutionInput,
    identity: PreviewStructuralCommandIdentity,
) -> Result<PreviewHtmlDuplicateExecutionReceipt, String> {
    run_preview_structural_write_command(
        &app,
        &state,
        &identity,
        "Preview HTML duplicate",
        |context, workspace| {
            execute_preview_html_duplicate(
                &app,
                &context.session,
                &context.root,
                workspace,
                input,
                &context.aliases,
            )
        },
    )
}

#[tauri::command(async)]
pub fn execute_preview_html_delete_intent(
    app: AppHandle,
    state: State<AppState>,
    input: PreviewHtmlDeleteExecutionInput,
    identity: PreviewStructuralCommandIdentity,
) -> Result<PreviewHtmlDeleteExecutionReceipt, String> {
    run_preview_structural_write_command(
        &app,
        &state,
        &identity,
        "Preview HTML delete",
        |context, workspace| {
            execute_preview_html_delete(
                &app,
                &context.session,
                &context.root,
                workspace,
                input,
                &context.aliases,
            )
        },
    )
}

#[tauri::command(async)]
pub fn execute_preview_tera_insert_drop_intent(
    app: AppHandle,
    state: State<AppState>,
    input: PreviewTeraInsertDropExecutionInput,
    identity: PreviewStructuralCommandIdentity,
) -> Result<PreviewTeraInsertDropExecutionReceipt, String> {
    run_preview_structural_write_command(
        &app,
        &state,
        &identity,
        "Preview Tera insert drop",
        |context, workspace| {
            execute_preview_tera_insert_drop(
                &app,
                &context.session,
                &context.root,
                workspace,
                input,
            )
        },
    )
}

#[tauri::command(async)]
pub fn execute_preview_tera_move_drop_intent(
    app: AppHandle,
    state: State<AppState>,
    input: PreviewTeraMoveDropExecutionInput,
    identity: PreviewStructuralCommandIdentity,
) -> Result<PreviewTeraMoveDropExecutionReceipt, String> {
    run_preview_structural_write_command(
        &app,
        &state,
        &identity,
        "Preview Tera move drop",
        |context, workspace| {
            execute_preview_tera_move_drop(&app, &context.session, &context.root, workspace, input)
        },
    )
}

#[tauri::command(async)]
pub fn execute_preview_tera_delete_intent(
    app: AppHandle,
    state: State<AppState>,
    input: PreviewTeraDeleteExecutionInput,
    identity: PreviewStructuralCommandIdentity,
) -> Result<PreviewTeraDeleteExecutionReceipt, String> {
    run_preview_structural_write_command(
        &app,
        &state,
        &identity,
        "Preview Tera delete",
        |context, workspace| {
            execute_preview_tera_delete(&app, &context.session, &context.root, workspace, input)
        },
    )
}

fn log_preview_projection_intent(app: &AppHandle, receipt: &PreviewProjectionIntentReceipt) {
    let level = match receipt.status {
        PreviewProjectionIntentStatus::Accepted => KernelLogLevel::Info,
        PreviewProjectionIntentStatus::Blocked | PreviewProjectionIntentStatus::Unsupported => {
            KernelLogLevel::Warn
        }
    };
    let kind = match receipt.status {
        PreviewProjectionIntentStatus::Accepted => KernelEventKind::PreviewProjectionIntentAccepted,
        PreviewProjectionIntentStatus::Blocked | PreviewProjectionIntentStatus::Unsupported => {
            KernelEventKind::PreviewProjectionIntentBlocked
        }
    };
    let diagnostic = receipt
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.blocking)
        .map(|diagnostic| diagnostic.message.clone());
    let mut event = KernelLogEvent::new(
        level,
        kind,
        "preview_projection",
        "preview_projection",
        receipt.kind.operation_label(),
        receipt.project_session_id.clone(),
        receipt.message.clone(),
        diagnostic,
    )
    .with_attribute("intentId", &receipt.intent_id)
    .with_attribute("kind", receipt.kind)
    .with_attribute("status", receipt.status)
    .with_attribute("effect", receipt.effect)
    .with_attribute("accepted", receipt.accepted)
    .with_attribute("requiresProjectSession", receipt.requires_project_session);
    if let Some(preview_revision) = receipt.preview_revision {
        event = event.with_attribute("previewRevision", preview_revision);
    }
    if let Some(project_session_id) = receipt.project_session_id.as_ref() {
        event = event.with_attribute("projectSessionId", project_session_id);
    }
    if !receipt.diagnostics.is_empty() {
        event = event.with_attribute("diagnosticCount", receipt.diagnostics.len());
    }
    let _ = append_event(app, event);
}

#[tauri::command]
pub fn read_kernel_observability_log(
    app: AppHandle,
    limit: Option<usize>,
    recovery_only: Option<bool>,
    include_archives: Option<bool>,
    levels: Option<Vec<KernelLogLevel>>,
    source_filter: Option<KernelObservabilityLogSourceFilter>,
) -> Result<KernelObservabilityLogSnapshot, String> {
    read_kernel_observability_log_snapshot(
        &app,
        KernelObservabilityLogRequest {
            limit,
            recovery_only,
            include_archives,
            levels,
            source_filter,
            event_names: None,
        },
    )
}

#[tauri::command]
pub fn read_kernel_disk_conflicts(
    state: State<AppState>,
) -> Result<Option<KernelDiskConflictSnapshot>, String> {
    let store = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace.".to_string())?
        .as_ref()
        .map(|workspace| workspace.documents.clone());
    Ok(store.as_ref().map(scan_disk_conflicts))
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

pub(crate) fn current_kernel_project_state_snapshot(
    state: &State<AppState>,
) -> Result<KernelProjectStateSnapshot, String> {
    let (project_root, session, workspace_snapshot, file_buffer_store) = {
        // Match the ProjectTransition lock order and clone one coherent
        // runtime generation. Releasing and reacquiring these locks one by
        // one can otherwise combine root A with session/store B.
        let current_root = state
            .current_root
            .lock()
            .map_err(|_| "Nu am putut bloca root-ul proiectului curent.".to_string())?;
        let project_workspace = state
            .project_workspace
            .lock()
            .map_err(|_| "Nu am putut bloca ProjectWorkspace.".to_string())?;
        (
            current_root
                .as_ref()
                .map(|path| path.to_string_lossy().to_string()),
            project_workspace
                .as_ref()
                .map(|workspace| workspace.session.clone()),
            project_workspace
                .as_ref()
                .map(|workspace| workspace.snapshot()),
            project_workspace
                .as_ref()
                .map(|workspace| workspace.documents.clone()),
        )
    };
    let disk_conflicts = file_buffer_store.as_ref().map(scan_disk_conflicts);
    Ok(build_kernel_project_state_snapshot(
        project_root.as_deref(),
        session.as_ref(),
        workspace_snapshot.as_ref(),
        disk_conflicts.as_ref(),
    ))
}

#[tauri::command]
pub fn read_kernel_project_transition_policy(
    action: KernelProjectTransitionAction,
    state: State<AppState>,
) -> Result<KernelProjectTransitionPolicy, String> {
    let project_state = current_kernel_project_state_snapshot(&state)?;
    Ok(evaluate_project_transition_policy(action, &project_state))
}

#[tauri::command]
pub fn read_kernel_project_transition_policy_matrix(
    state: State<AppState>,
) -> Result<KernelProjectTransitionPolicyMatrixSnapshot, String> {
    let project_state = current_kernel_project_state_snapshot(&state)?;
    Ok(build_project_transition_policy_matrix(project_state))
}

#[tauri::command]
pub fn read_kernel_project_transition_blocked_audit(
    app: AppHandle,
    limit: Option<usize>,
    include_archives: Option<bool>,
) -> Result<KernelProjectTransitionBlockedAuditSnapshot, String> {
    read_kernel_project_transition_blocked_audit_snapshot(&app, limit, include_archives)
}

#[tauri::command]
pub fn read_kernel_project_transition_decision_journal(
    state: State<AppState>,
    limit: Option<usize>,
) -> Result<Option<KernelProjectTransitionDecisionJournalSnapshot>, String> {
    let session = current_project_session(&state)?;
    let Some(session) = session else {
        return Ok(None);
    };

    read_kernel_project_transition_decision_journal_snapshot(&session, limit).map(Some)
}

#[tauri::command]
pub fn read_kernel_project_transition_decision_recovery_ack_journal(
    state: State<AppState>,
    limit: Option<usize>,
) -> Result<Option<KernelProjectTransitionDecisionRecoveryAckJournalSnapshot>, String> {
    let session = current_project_session(&state)?;
    let Some(session) = session else {
        return Ok(None);
    };

    read_kernel_project_transition_decision_recovery_ack_journal_snapshot(&session, limit).map(Some)
}

#[tauri::command]
pub fn read_kernel_project_transition_decision_retention_hot_journals(
    state: State<AppState>,
) -> Result<Option<Vec<KernelProjectTransitionDecisionRetentionHotJournal>>, String> {
    let session = current_project_session(&state)?;
    let Some(session) = session else {
        return Ok(None);
    };

    scan_project_transition_decision_retention_hot_journals(&session).map(Some)
}
