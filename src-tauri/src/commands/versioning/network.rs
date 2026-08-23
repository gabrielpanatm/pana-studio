use super::*;
use super::{observability::record_versioning_event, session::*};

const VERSIONING_NETWORK_PROGRESS_EVENT: &str = "pana-versioning-network-progress";

struct ActiveVersionNetworkLease {
    app: AppHandle,
    operation_id: String,
}

impl Drop for ActiveVersionNetworkLease {
    fn drop(&mut self) {
        let state = self.app.state::<AppState>();
        state
            .versioning_network_operation
            .abandon(&self.operation_id);
    }
}

async fn network_mutate_with_repository(
    app: AppHandle,
    identity: VersioningMutationIdentity,
    operation_id: String,
    kind: VersionNetworkOperationKind,
    operation_name: &'static str,
    prepare: impl FnOnce(&VersionRepository) -> Result<PreparedVersionNetworkOperation, String>
        + Send
        + 'static,
) -> Result<VersionNetworkReceipt, String> {
    let operation_id = validate_operation_id(&operation_id)?;
    let log_app = app.clone();
    let project_root = identity.expected_project_root.clone();
    let log_project_root = project_root.clone();
    let session_id = identity.expected_session_id.clone();
    let result: Result<VersionNetworkReceipt, String> =
        tauri::async_runtime::spawn_blocking(move || {
            let result = (|| -> Result<VersionNetworkReceipt, String> {
                let state = app.state::<AppState>();
                let _operation = acquire_git_mutation_gate(&state, operation_name)?;
                let cancellation = Arc::new(AtomicBool::new(false));
                execute_version_network_phases(
                    || {
                        capture_network_preflight(
                            &app,
                            &identity,
                            &operation_id,
                            kind,
                            cancellation.clone(),
                            prepare,
                        )
                    },
                    |(captured, operation_lease, prepared, network_cleanup)| {
                        let progress_app = app.clone();
                        let progress_root = project_root.clone();
                        let progress_session = session_id.clone();
                        let progress_operation_id = operation_id.clone();
                        let progress: VersionNetworkProgressCallback = Arc::new(move |chunk| {
                            let source = String::from_utf8_lossy(chunk);
                            let message = network_progress_text(&source);
                            emit_network_progress(
                                &progress_app,
                                &progress_root,
                                &progress_session,
                                &progress_operation_id,
                                kind,
                                VersionNetworkOperationStatus::Progress,
                                &message,
                            );
                        });
                        let output = captured
                            .spawn_prepared_network(&app, &prepared, cancellation.clone(), progress)
                            .and_then(crate::versioning::RunningGitCommand::wait)
                            .map_err(|error| classify_network_runtime_error(kind, error))?;
                        Ok((captured, operation_lease, prepared, network_cleanup, output))
                    },
                    |(captured, operation_lease, prepared, _network_cleanup, output)| {
                        publish_network_result(&app, &captured, &operation_lease, prepared, output)
                    },
                )
            })();
            match &result {
                Ok(_) => emit_network_progress(
                    &app,
                    &project_root,
                    &session_id,
                    &operation_id,
                    kind,
                    VersionNetworkOperationStatus::Completed,
                    "Operația Git de rețea s-a încheiat.",
                ),
                Err(error) if error.contains("a fost anulată") => emit_network_progress(
                    &app,
                    &project_root,
                    &session_id,
                    &operation_id,
                    kind,
                    VersionNetworkOperationStatus::Cancelled,
                    "Operația Git de rețea a fost anulată.",
                ),
                Err(error) => emit_network_progress(
                    &app,
                    &project_root,
                    &session_id,
                    &operation_id,
                    kind,
                    VersionNetworkOperationStatus::Failed,
                    &redact_network_text(error),
                ),
            }
            result
        })
        .await
        .map_err(|error| format!("Operația Git de rețea a căzut în task-ul de fundal: {error}"))?;

    match &result {
        Ok(_) => record_versioning_event(
            &log_app,
            KernelLogLevel::Info,
            KernelEventKind::VersioningRemoteCompleted,
            operation_name,
            Some(log_project_root),
            "Operația Git remote a fost finalizată.",
            None,
        ),
        Err(error) if error.contains("a fost anulată") => record_versioning_event(
            &log_app,
            KernelLogLevel::Info,
            KernelEventKind::VersioningRemoteCancelled,
            operation_name,
            Some(log_project_root),
            "Operația Git remote a fost anulată controlat.",
            None,
        ),
        Err(error) => record_versioning_event(
            &log_app,
            KernelLogLevel::Warn,
            KernelEventKind::VersioningRemoteFailed,
            operation_name,
            Some(log_project_root),
            "Operația Git remote a fost blocată sau a eșuat.",
            Some(redact_network_text(error)),
        ),
    }
    result
}

fn capture_network_preflight(
    app: &AppHandle,
    identity: &VersioningMutationIdentity,
    operation_id: &str,
    kind: VersionNetworkOperationKind,
    cancellation: Arc<AtomicBool>,
    prepare: impl FnOnce(&VersionRepository) -> Result<PreparedVersionNetworkOperation, String>,
) -> Result<
    (
        CapturedVersioningSession,
        VersionNetworkOperationLease,
        PreparedVersionNetworkOperation,
        ActiveVersionNetworkLease,
    ),
    String,
> {
    let state = app.state::<AppState>();
    state
        .ai_coordination
        .require_user_source_mutation()
        .map_err(|error| error.to_string())?;
    let root_guard = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul proiectului pentru Git remote.".to_string())?;
    let root = root_guard
        .as_ref()
        .ok_or_else(|| "Nu există proiect deschis pentru Git remote.".to_string())?;
    let workspace_guard = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru Git remote.".to_string())?;
    let workspace = workspace_guard
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru Git remote.".to_string())?;
    let captured = capture_from_workspace(
        root,
        &workspace.session,
        &workspace.runtime_session_id(),
        &identity.expected_project_root,
        &identity.expected_session_id,
    )?;
    if workspace.is_dirty() {
        return Err(
            "Versiuni a blocat operația remote: salvează mai întâi modificările din ProjectWorkspace."
                .to_string(),
        );
    }
    require_recovery_coordinator_clean_for_write(&state, &workspace.session, "Versiuni remote")?;
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        root,
    )?;
    let operation_lease = VersionNetworkOperationLease {
        operation_id: operation_id.to_string(),
        project_root: workspace.session.project_root.clone(),
        session_id: workspace.runtime_session_id(),
        kind,
        workspace_revision: workspace.revision,
        disk_generation: workspace.accepted_disk.generation,
        accepted_disk: Arc::clone(&workspace.accepted_disk),
        expected_status_token: identity.expected_status_token.clone(),
        expected_head_oid: identity.expected_head_oid.clone(),
    };
    state
        .versioning_network_operation
        .begin(operation_lease.clone(), cancellation)?;
    let cleanup = ActiveVersionNetworkLease {
        app: app.clone(),
        operation_id: operation_id.to_string(),
    };
    drop(workspace_guard);
    drop(root_guard);
    emit_network_progress(
        app,
        &operation_lease.project_root,
        &operation_lease.session_id,
        &operation_lease.operation_id,
        kind,
        VersionNetworkOperationStatus::Started,
        "Operația Git de rețea a pornit.",
    );
    let prepared = captured.with_repository(app, |repository| {
        repository.require_status_token(
            &operation_lease.expected_status_token,
            operation_lease.expected_head_oid.as_deref(),
        )?;
        if !repository.read_restore_markers()?.is_empty() {
            return Err(
                "Operația Git de rețea este blocată de o restaurare pendentă. Rezolvă mai întâi Recovery."
                    .to_string(),
            );
        }
        if !repository.read_integration_markers()?.is_empty() {
            return Err(
                "Operația Git de rețea este blocată de o integrare pendentă. Rezolvă mai întâi Recovery."
                    .to_string(),
            );
        }
        prepare(repository)
    })?;
    Ok((captured, operation_lease, prepared, cleanup))
}

fn publish_network_result(
    app: &AppHandle,
    captured: &CapturedVersioningSession,
    operation_lease: &VersionNetworkOperationLease,
    prepared: PreparedVersionNetworkOperation,
    output: crate::versioning::GitCommandOutput,
) -> Result<VersionNetworkReceipt, String> {
    let state = app.state::<AppState>();
    let remote_succeeded = output.success();
    validate_network_publication_state(&state, captured, operation_lease, false).map_err(
        |error| classify_network_publication_error(operation_lease.kind, error, remote_succeeded),
    )?;
    let receipt = captured.with_repository(app, |repository| {
        repository.finalize_prepared_network(prepared, output)
    })?;
    validate_network_publication_state(&state, captured, operation_lease, true).map_err(
        |error| classify_network_publication_error(operation_lease.kind, error, remote_succeeded),
    )?;
    Ok(receipt)
}

fn validate_network_publication_state(
    state: &AppState,
    captured: &CapturedVersioningSession,
    operation_lease: &VersionNetworkOperationLease,
    finish: bool,
) -> Result<(), String> {
    let root_guard = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut revalida root-ul după operația Git remote.".to_string())?;
    let root = root_guard
        .as_ref()
        .ok_or_else(|| "Proiectul a fost închis înainte de publicarea Git remote.".to_string())?;
    let workspace_guard = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut revalida ProjectWorkspace după Git remote.".to_string())?;
    let workspace = workspace_guard.as_ref().ok_or_else(|| {
        "ProjectWorkspace a fost închis înainte de publicarea Git remote.".to_string()
    })?;
    state
        .versioning_network_operation
        .require_current(operation_lease)?;
    if root != &captured.root
        || workspace.session.project_root != operation_lease.project_root
        || workspace.runtime_session_id() != operation_lease.session_id
        || workspace.revision != operation_lease.workspace_revision
        || workspace.accepted_disk.generation != operation_lease.disk_generation
        || workspace.accepted_disk.as_ref() != operation_lease.accepted_disk.as_ref()
    {
        return Err(format!(
            "Operația Git remote {} a devenit stale: proiectul, sesiunea sau ProjectWorkspace s-au schimbat înainte de publicare.",
            operation_lease.operation_id
        ));
    }
    if workspace.is_dirty() {
        return Err(format!(
            "Operația Git remote {} nu poate publica peste un ProjectWorkspace devenit dirty.",
            operation_lease.operation_id
        ));
    }
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        root,
    )?;
    if finish {
        state
            .versioning_network_operation
            .finish_success(operation_lease)
    } else {
        state
            .versioning_network_operation
            .require_current(operation_lease)
    }
}

fn emit_network_progress(
    app: &AppHandle,
    project_root: &str,
    session_id: &str,
    operation_id: &str,
    kind: VersionNetworkOperationKind,
    status: VersionNetworkOperationStatus,
    _message: &str,
) {
    let code = match (kind, status) {
        (VersionNetworkOperationKind::Fetch, VersionNetworkOperationStatus::Started) => {
            "version-network-fetch-started"
        }
        (VersionNetworkOperationKind::Fetch, VersionNetworkOperationStatus::Progress) => {
            "version-network-fetch-progress"
        }
        (VersionNetworkOperationKind::Fetch, VersionNetworkOperationStatus::Completed) => {
            "version-network-fetch-completed"
        }
        (VersionNetworkOperationKind::Fetch, VersionNetworkOperationStatus::Failed) => {
            "version-network-fetch-failed"
        }
        (VersionNetworkOperationKind::Fetch, VersionNetworkOperationStatus::Cancelled) => {
            "version-network-fetch-cancelled"
        }
        (VersionNetworkOperationKind::Push, VersionNetworkOperationStatus::Started) => {
            "version-network-push-started"
        }
        (VersionNetworkOperationKind::Push, VersionNetworkOperationStatus::Progress) => {
            "version-network-push-progress"
        }
        (VersionNetworkOperationKind::Push, VersionNetworkOperationStatus::Completed) => {
            "version-network-push-completed"
        }
        (VersionNetworkOperationKind::Push, VersionNetworkOperationStatus::Failed) => {
            "version-network-push-failed"
        }
        (VersionNetworkOperationKind::Push, VersionNetworkOperationStatus::Cancelled) => {
            "version-network-push-cancelled"
        }
    };
    let event = VersionNetworkProgressEvent {
        schema_version: 3,
        project_root: project_root.to_string(),
        session_id: session_id.to_string(),
        operation_id: operation_id.to_string(),
        kind,
        status,
        message_diagnostic: crate::localization::LocalizedDiagnostic::new(code),
    };
    if let Err(error) = app.emit(VERSIONING_NETWORK_PROGRESS_EVENT, event) {
        eprintln!("[Pană Studio] Evenimentul de progres Git nu a putut fi emis: {error}");
    }
}

#[tauri::command]
pub async fn fetch_version_remote(
    identity: VersioningMutationIdentity,
    input: VersionFetchInput,
    app: AppHandle,
) -> Result<VersionNetworkReceipt, String> {
    let operation_id = input.operation_id.clone();
    network_mutate_with_repository(
        app,
        identity,
        operation_id,
        VersionNetworkOperationKind::Fetch,
        "fetch_remote",
        move |repository| {
            repository
                .prepare_fetch_remote(&input.remote, input.prune, &input.operation_id)
                .map(PreparedVersionNetworkOperation::Fetch)
        },
    )
    .await
}

#[tauri::command]
pub async fn push_version_branch(
    identity: VersioningMutationIdentity,
    input: VersionPushInput,
    app: AppHandle,
) -> Result<VersionNetworkReceipt, String> {
    let operation_id = input.operation_id.clone();
    network_mutate_with_repository(
        app,
        identity,
        operation_id,
        VersionNetworkOperationKind::Push,
        "push_branch",
        move |repository| {
            repository
                .prepare_push_branch(&input)
                .map(PreparedVersionNetworkOperation::Push)
        },
    )
    .await
}

#[tauri::command]
pub async fn cancel_version_network_operation(
    identity: VersioningSessionIdentity,
    input: VersionNetworkCancelInput,
    app: AppHandle,
) -> Result<VersionNetworkCancelReceipt, String> {
    let operation_id = validate_operation_id(&input.operation_id)?;
    let state = app.state::<AppState>();
    let active = state.versioning_network_operation.request_cancellation(
        &operation_id,
        &identity.expected_project_root,
        &identity.expected_session_id,
    )?;
    let cancellation_requested = if let Some(active) = active {
        emit_network_progress(
            &app,
            &active.project_root,
            &active.session_id,
            &active.operation_id,
            active.kind,
            VersionNetworkOperationStatus::Progress,
            "Anularea a fost solicitată; procesul Git este oprit controlat.",
        );
        true
    } else {
        false
    };
    Ok(VersionNetworkCancelReceipt {
        schema_version: VERSIONING_SCHEMA_VERSION,
        operation_id,
        cancellation_requested,
    })
}
