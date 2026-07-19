use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use tauri::{AppHandle, Emitter, Manager};

use crate::{
    commands::project::require_recovery_coordinator_clean_for_write,
    kernel::{
        observability::{append_event, KernelEventKind, KernelLogEvent, KernelLogLevel},
        project_session::ProjectSessionSnapshot,
        project_workspace::{
            save_project_workspace_with_recovery, ProjectWorkspace, ProjectWorkspaceIdentity,
            ProjectWorkspaceSaveError, WorkspaceMutationMetadata,
        },
        write_authority::{ActiveProjectReadLease, WriteAuthorityRuntime},
    },
    preview::{
        preprocess::materialize_version_source_tree, start_version_source_browser,
        stop_version_source_browser,
    },
    state::AppState,
    versioning::{
        build_version_restore_plan, network_progress_text, redact_network_text,
        validate_operation_id, PreparedVersionIntegration, PreparedVersionRestore,
        VersionBranchInput, VersionBranchNameInput, VersionDiffInput, VersionDiffReceipt,
        VersionFetchInput, VersionHistoryPage, VersionIntegrationInput, VersionIntegrationKind,
        VersionIntegrationPlan, VersionIntegrationReceipt, VersionIntegrationRecoveryAction,
        VersionIntegrationRecoveryItem, VersionIntegrationRecoveryResolutionInput,
        VersionIntegrationRecoveryResolutionReceipt, VersionIntegrationRecoveryScan,
        VersionIntegrationRecoveryState, VersionIntegrationRelationship, VersionIntegrationStatus,
        VersionIntegrationTargetInput, VersionNetworkCancelInput, VersionNetworkCancelReceipt,
        VersionNetworkOperationControl, VersionNetworkOperationKind, VersionNetworkOperationStatus,
        VersionNetworkProgressEvent, VersionNetworkReceipt, VersionPreviewInput,
        VersionPreviewReceipt, VersionPushInput, VersionRemoteInput, VersionRemoteNameInput,
        VersionRepository, VersionRestoreExpectedFile, VersionRestoreInput, VersionRestoreReceipt,
        VersionRestoreRecoveryAction, VersionRestoreRecoveryItem,
        VersionRestoreRecoveryResolutionInput, VersionRestoreRecoveryResolutionReceipt,
        VersionRestoreRecoveryScan, VersionRestoreRecoveryState, VersionRestoreStatus,
        VersionSwitchBranchInput, VersionSyncComparison, VersionTree, VersionUpstreamInput,
        VersioningCommitInput, VersioningCommitReceipt, VersioningIdentityInput,
        VersioningMutationIdentity, VersioningMutationReceipt, VersioningPathsInput,
        VersioningSessionIdentity, VersioningSnapshot, VERSIONING_SCHEMA_VERSION,
    },
};

const VERSIONING_NETWORK_PROGRESS_EVENT: &str = "pana-versioning-network-progress";
type VersionNetworkProgressCallback = Arc<dyn Fn(&[u8]) + Send + Sync + 'static>;

struct ActiveVersionNetworkLease {
    app: AppHandle,
    operation_id: String,
}

impl Drop for ActiveVersionNetworkLease {
    fn drop(&mut self) {
        let state = self.app.state::<AppState>();
        let Ok(mut active) = state.versioning_network_operation.lock() else {
            eprintln!("[Pană Studio] Mutex-ul operației Git de rețea este compromis la cleanup.");
            return;
        };
        if active
            .as_ref()
            .is_some_and(|operation| operation.operation_id == self.operation_id)
        {
            *active = None;
        }
    }
}

#[derive(Clone)]
struct CapturedVersioningSession {
    root: PathBuf,
    repository_root: PathBuf,
    session: ProjectSessionSnapshot,
    runtime_session_id: String,
}

impl CapturedVersioningSession {
    fn with_repository<R: tauri::Runtime, T>(
        &self,
        app: &AppHandle<R>,
        operation: impl FnOnce(&VersionRepository) -> Result<T, String>,
    ) -> Result<T, String> {
        let runtime = app
            .try_state::<WriteAuthorityRuntime>()
            .ok_or_else(|| "WriteAuthorityRuntime lipsește pentru Git.".to_string())?;
        let lease = runtime
            .acquire_active_project_read_lease_for_session(&self.root, &self.runtime_session_id)?;
        let directory = lease
            .capture_subprocess_directory(Path::new("sursa"), "versioning/git-repository-cwd")?;
        let repository = VersionRepository::new(
            self.session.project_root.clone(),
            self.repository_root.clone(),
            directory.current_dir_path(),
        );
        operation(&repository)
    }
}

fn capture_read_session(
    state: &AppState,
    identity: &VersioningSessionIdentity,
) -> Result<CapturedVersioningSession, String> {
    let root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul proiectului pentru Git.".to_string())?
        .clone()
        .ok_or_else(|| "Nu există proiect deschis pentru Git.".to_string())?;
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru Git.".to_string())?;
    let workspace = workspace
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru Git.".to_string())?;
    capture_from_workspace(
        &root,
        &workspace.session,
        &workspace.runtime_session_id(),
        &identity.expected_project_root,
        &identity.expected_session_id,
    )
}

fn capture_from_workspace(
    root: &Path,
    session: &ProjectSessionSnapshot,
    runtime_session_id: &str,
    expected_project_root: &str,
    expected_session_id: &str,
) -> Result<CapturedVersioningSession, String> {
    if session.project_root != expected_project_root || runtime_session_id != expected_session_id {
        return Err(format!(
            "Versiuni a refuzat un request stale: așteptat root/session {expected_project_root}/{expected_session_id}, activ {}/{}.",
            session.project_root, runtime_session_id
        ));
    }
    if root != Path::new(&session.project_root) {
        return Err("Root-ul activ și ProjectSession nu corespund pentru Git.".to_string());
    }
    let expected_repository_root = root.join("sursa");
    let session_repository_root = PathBuf::from(&session.zola_root);
    if session_repository_root != expected_repository_root {
        return Err(format!(
            "Versiuni acceptă numai zola_root proiect/sursa: sesiunea indică {}, iar rădăcina cerută este {}.",
            session_repository_root.display(),
            expected_repository_root.display()
        ));
    }
    if !session_repository_root.is_dir() {
        return Err(format!(
            "Rădăcina Git autorizată nu este un director: {}.",
            session_repository_root.display()
        ));
    }
    Ok(CapturedVersioningSession {
        root: root.to_path_buf(),
        repository_root: session_repository_root,
        session: session.clone(),
        runtime_session_id: runtime_session_id.to_string(),
    })
}

fn with_mutation_preflight<T>(
    app: &AppHandle,
    identity: &VersioningMutationIdentity,
    operation: impl FnOnce(&VersionRepository) -> Result<T, String>,
) -> Result<T, String> {
    let state = app.state::<AppState>();
    state
        .ai_coordination
        .require_user_source_mutation()
        .map_err(|error| error.to_string())?;
    let root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul proiectului pentru mutația Git.".to_string())?
        .clone()
        .ok_or_else(|| "Nu există proiect deschis pentru mutația Git.".to_string())?;
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru mutația Git.".to_string())?;
    let workspace = workspace
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru mutația Git.".to_string())?;
    let captured = capture_from_workspace(
        &root,
        &workspace.session,
        &workspace.runtime_session_id(),
        &identity.expected_project_root,
        &identity.expected_session_id,
    )?;
    if workspace.is_dirty() {
        return Err(
            "Versiuni a blocat operația: salvează mai întâi modificările din ProjectWorkspace."
                .to_string(),
        );
    }
    require_recovery_coordinator_clean_for_write(&state, &workspace.session, "Versiuni")?;
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        &root,
    )?;
    // Keep the root and ProjectWorkspace guards alive through the complete Git
    // effect. A concurrent Save, draft mutation or project transition cannot
    // invalidate the clean/AcceptedDisk preflight after it succeeded.
    captured.with_repository(app, operation)
}

async fn read_with_repository<T: Send + 'static>(
    app: AppHandle,
    identity: VersioningSessionIdentity,
    operation: impl FnOnce(&VersionRepository) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let captured = capture_read_session(state.inner(), &identity)?;
        captured.with_repository(&app, operation)
    })
    .await
    .map_err(|error| format!("Operația Git a căzut în task-ul de fundal: {error}"))?
}

async fn mutate_with_repository<T: Send + 'static>(
    app: AppHandle,
    identity: VersioningMutationIdentity,
    operation_name: &'static str,
    operation: impl FnOnce(&VersionRepository) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let log_app = app.clone();
    let log_project_root = identity.expected_project_root.clone();
    let result: Result<T, String> = tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let _operation = state
            .versioning_operation
            .lock()
            .map_err(|_| "Mutex-ul operațiilor Git este compromis.".to_string())?;
        with_mutation_preflight(&app, &identity, |repository| {
            repository.require_status_token(
                &identity.expected_status_token,
                identity.expected_head_oid.as_deref(),
            )?;
            if !repository.read_restore_markers()?.is_empty() {
                return Err(
                    "Operația Git este blocată de o restaurare pendentă. Rezolvă mai întâi secțiunea Recovery din panoul Versiuni."
                        .to_string(),
                );
            }
            if !repository.read_integration_markers()?.is_empty() {
                return Err(
                    "Operația Git este blocată de o integrare pendentă. Continuă, finalizează sau anulează integrarea din Recovery."
                        .to_string(),
                );
            }
            operation(repository)
        })
    })
    .await
    .map_err(|error| format!("Mutația Git a căzut în task-ul de fundal: {error}"))?;
    match &result {
        Ok(_) => record_versioning_event(
            &log_app,
            KernelLogLevel::Info,
            KernelEventKind::VersioningMutationCommitted,
            operation_name,
            Some(log_project_root),
            "Operația Git a fost publicată.",
            None,
        ),
        Err(error) => record_versioning_event(
            &log_app,
            KernelLogLevel::Warn,
            KernelEventKind::VersioningMutationFailed,
            operation_name,
            Some(log_project_root),
            "Operația Git a fost blocată sau a eșuat.",
            Some(error.clone()),
        ),
    }
    result
}

async fn network_mutate_with_repository(
    app: AppHandle,
    identity: VersioningMutationIdentity,
    operation_id: String,
    kind: VersionNetworkOperationKind,
    operation_name: &'static str,
    operation: impl FnOnce(
            &VersionRepository,
            Arc<AtomicBool>,
            VersionNetworkProgressCallback,
        ) -> Result<VersionNetworkReceipt, String>
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
            let state = app.state::<AppState>();
            let _operation = state
                .versioning_operation
                .lock()
                .map_err(|_| "Mutex-ul operațiilor Git este compromis.".to_string())?;
            let cancellation = Arc::new(AtomicBool::new(false));
            {
                let mut active = state.versioning_network_operation.lock().map_err(|_| {
                    "Mutex-ul operației Git de rețea este compromis.".to_string()
                })?;
                if let Some(active) = active.as_ref() {
                    return Err(format!(
                        "Operația Git de rețea {} este deja activă.",
                        active.operation_id
                    ));
                }
                *active = Some(VersionNetworkOperationControl {
                    operation_id: operation_id.clone(),
                    project_root: project_root.clone(),
                    session_id: session_id.clone(),
                    kind,
                    cancellation: cancellation.clone(),
                });
            }
            let _network_lease = ActiveVersionNetworkLease {
                app: app.clone(),
                operation_id: operation_id.clone(),
            };
            emit_network_progress(
                &app,
                &project_root,
                &session_id,
                &operation_id,
                kind,
                VersionNetworkOperationStatus::Started,
                "Operația Git de rețea a pornit.",
            );

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

            let result = with_mutation_preflight(&app, &identity, |repository| {
                repository.require_status_token(
                    &identity.expected_status_token,
                    identity.expected_head_oid.as_deref(),
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
                operation(repository, cancellation, progress)
            });
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

fn emit_network_progress(
    app: &AppHandle,
    project_root: &str,
    session_id: &str,
    operation_id: &str,
    kind: VersionNetworkOperationKind,
    status: VersionNetworkOperationStatus,
    message: &str,
) {
    let event = VersionNetworkProgressEvent {
        schema_version: VERSIONING_SCHEMA_VERSION,
        project_root: project_root.to_string(),
        session_id: session_id.to_string(),
        operation_id: operation_id.to_string(),
        kind,
        status,
        message: redact_network_text(message),
    };
    if let Err(error) = app.emit(VERSIONING_NETWORK_PROGRESS_EVENT, event) {
        eprintln!("[Pană Studio] Evenimentul de progres Git nu a putut fi emis: {error}");
    }
}

async fn execute_source_integration(
    app: AppHandle,
    identity: VersioningMutationIdentity,
    operation_name: &'static str,
    requested_target_ref: String,
    requested_target_oid: String,
    prepare: impl FnOnce(
            &VersionRepository,
            &VersioningSnapshot,
        ) -> Result<Option<PreparedVersionIntegration>, String>
        + Send
        + 'static,
) -> Result<VersionIntegrationReceipt, String> {
    let log_app = app.clone();
    let requested_root = identity.expected_project_root.clone();
    let result: Result<VersionIntegrationReceipt, String> =
        tauri::async_runtime::spawn_blocking(move || {
            let state = app.state::<AppState>();
            let _operation = state
                .versioning_operation
                .lock()
                .map_err(|_| "Mutex-ul operațiilor Git este compromis.".to_string())?;
            state
                .ai_coordination
                .require_user_source_mutation()
                .map_err(|error| error.to_string())?;
            let root_guard = state.current_root.lock().map_err(|_| {
                "Nu am putut bloca root-ul proiectului pentru integrare.".to_string()
            })?;
            let root = root_guard
                .as_ref()
                .ok_or_else(|| "Nu există proiect deschis pentru integrare.".to_string())?;
            let mut workspace_guard = state.project_workspace.lock().map_err(|_| {
                "Nu am putut bloca ProjectWorkspace pentru integrare.".to_string()
            })?;
            let workspace = workspace_guard.as_mut().ok_or_else(|| {
                "ProjectWorkspace nu este inițializat pentru integrare.".to_string()
            })?;
            let captured = capture_from_workspace(
                root,
                &workspace.session,
                &workspace.runtime_session_id(),
                &identity.expected_project_root,
                &identity.expected_session_id,
            )?;
            if workspace.is_dirty() {
                return Err(
                    "Integrarea cere un ProjectWorkspace curat. Salvează sau anulează modificările înainte de operație."
                        .to_string(),
                );
            }
            require_recovery_coordinator_clean_for_write(
                &state,
                &workspace.session,
                "Integrare versiuni Git",
            )?;
            workspace.accepted_disk.require_live_complete(
                &workspace.runtime_session_id(),
                &workspace.session.project_root,
                root,
            )?;

            let authority = app.state::<WriteAuthorityRuntime>();
            let session_lease = authority.acquire_active_project_read_lease_for_session(
                &captured.root,
                &captured.runtime_session_id,
            )?;
            let directory = session_lease.capture_subprocess_directory(
                Path::new("sursa"),
                "versioning/integration-git-repository-cwd",
            )?;
            let repository = VersionRepository::new(
                captured.session.project_root.clone(),
                captured.repository_root.clone(),
                directory.current_dir_path(),
            );
            let before = repository.require_status_token(
                &identity.expected_status_token,
                identity.expected_head_oid.as_deref(),
            )?;
            if !repository.read_restore_markers()?.is_empty() {
                return Err(
                    "Integrarea este blocată de o restaurare pendentă.".to_string(),
                );
            }
            if !repository.read_integration_markers()?.is_empty() {
                return Err(
                    "Există deja o integrare pendentă. Rezolvă Recovery înainte de alta."
                        .to_string(),
                );
            }
            if !before.clean {
                return Err(
                    "Integrarea cere un repository Git complet curat, inclusiv fără fișiere untracked."
                        .to_string(),
                );
            }
            let previous_head_oid = before.head_oid.clone().ok_or_else(|| {
                "Integrarea cere cel puțin un commit pe branch-ul activ.".to_string()
            })?;
            let Some(prepared) = prepare(&repository, &before)? else {
                return Ok(VersionIntegrationReceipt {
                    schema_version: VERSIONING_SCHEMA_VERSION,
                    status: VersionIntegrationStatus::Noop,
                    project_root: captured.session.project_root,
                    session_id: captured.runtime_session_id,
                    transaction_id: None,
                    recovery_ref: None,
                    kind: None,
                    previous_head_oid: previous_head_oid.clone(),
                    target_ref: requested_target_ref,
                    target_oid: requested_target_oid,
                    result_commit_oid: before.head_oid.clone(),
                    changed_paths: Vec::new(),
                    conflict_paths: Vec::new(),
                    diagnostic: Some(
                        "Ținta este deja integrată; sursa și istoricul nu au fost modificate."
                            .to_string(),
                    ),
                    snapshot: Some(before),
                    workspace: Some(workspace.snapshot()),
                });
            };
            let current_tree = repository.previous_integration_tree(&prepared)?;
            let target_tree = repository.integration_tree(&prepared)?;
            let publication = publish_integration_tree(
                &app,
                root,
                workspace,
                &captured,
                &repository,
                session_lease,
                &prepared,
                &current_tree,
                &target_tree,
                &BTreeSet::new(),
                match prepared.kind {
                    VersionIntegrationKind::SwitchBranch => format!(
                        "Switch Git {}",
                        prepared.target_branch.as_deref().unwrap_or("branch")
                    ),
                    VersionIntegrationKind::FastForward => format!(
                        "Fast-forward Git {}",
                        prepared.target_oid.chars().take(8).collect::<String>()
                    ),
                    _ => format!(
                        "Merge Git {}",
                        prepared.target_oid.chars().take(8).collect::<String>()
                    ),
                },
                "versioning_integration",
            )?;
            let changed_paths = match publication {
                IntegrationTreePublication::RecoveryRequired {
                    changed_paths,
                    diagnostic,
                } => {
                    return Ok(integration_receipt(
                        &captured,
                        &prepared,
                        VersionIntegrationStatus::RecoveryRequired,
                        changed_paths,
                        Some(diagnostic),
                        None,
                        Some(workspace.snapshot()),
                    ));
                }
                IntegrationTreePublication::Applied { changed_paths } => changed_paths,
            };
            if prepared.kind == VersionIntegrationKind::MergeConflict {
                let snapshot = repository.snapshot().ok();
                return Ok(integration_receipt(
                    &captured,
                    &prepared,
                    VersionIntegrationStatus::ConflictResolutionRequired,
                    changed_paths,
                    Some(
                        "Merge-ul a fost materializat cu markere de conflict. Rezolvă exclusiv fișierele listate, salvează proiectul, apoi folosește Continuă; Abort revine la arborele anterior."
                            .to_string(),
                    ),
                    snapshot,
                    Some(workspace.snapshot()),
                ));
            }
            match repository.finalize_integration(&prepared) {
                Ok(snapshot) => Ok(integration_receipt(
                    &captured,
                    &prepared,
                    VersionIntegrationStatus::Applied,
                    changed_paths,
                    None,
                    Some(snapshot),
                    Some(workspace.snapshot()),
                )),
                Err(error) => Ok(integration_receipt(
                    &captured,
                    &prepared,
                    VersionIntegrationStatus::RecoveryRequired,
                    changed_paths,
                    Some(format!(
                        "Fișierele au fost publicate, dar referința Git nu a putut fi finalizată: {error} Marker-ul durabil a fost păstrat."
                    )),
                    repository.snapshot().ok(),
                    Some(workspace.snapshot()),
                )),
            }
        })
        .await
        .map_err(|error| format!("Integrarea Git a căzut în task-ul de fundal: {error}"))?;

    match &result {
        Ok(receipt) if receipt.status == VersionIntegrationStatus::ConflictResolutionRequired => {
            record_versioning_event(
                &log_app,
                KernelLogLevel::Warn,
                KernelEventKind::VersioningIntegrationConflict,
                operation_name,
                Some(requested_root),
                "Integrarea Git cere rezolvarea conflictelor.",
                receipt.diagnostic.clone(),
            )
        }
        Ok(receipt) if receipt.status == VersionIntegrationStatus::RecoveryRequired => {
            record_versioning_event(
                &log_app,
                KernelLogLevel::Warn,
                KernelEventKind::VersioningIntegrationRecoveryRequired,
                operation_name,
                Some(requested_root),
                "Integrarea Git cere recovery explicit.",
                receipt.diagnostic.clone(),
            )
        }
        Ok(_) => record_versioning_event(
            &log_app,
            KernelLogLevel::Info,
            KernelEventKind::VersioningIntegrationPublished,
            operation_name,
            Some(requested_root),
            "Integrarea Git a fost publicată.",
            None,
        ),
        Err(error) => record_versioning_event(
            &log_app,
            KernelLogLevel::Warn,
            KernelEventKind::VersioningMutationFailed,
            operation_name,
            Some(requested_root),
            "Integrarea Git a fost blocată sau a eșuat.",
            Some(error.clone()),
        ),
    }
    result
}

fn integration_receipt(
    captured: &CapturedVersioningSession,
    prepared: &PreparedVersionIntegration,
    status: VersionIntegrationStatus,
    changed_paths: Vec<String>,
    diagnostic: Option<String>,
    snapshot: Option<VersioningSnapshot>,
    workspace: Option<crate::kernel::project_workspace::ProjectWorkspaceSnapshot>,
) -> VersionIntegrationReceipt {
    VersionIntegrationReceipt {
        schema_version: VERSIONING_SCHEMA_VERSION,
        status,
        project_root: captured.session.project_root.clone(),
        session_id: captured.runtime_session_id.clone(),
        transaction_id: Some(prepared.transaction_id.clone()),
        recovery_ref: matches!(
            status,
            VersionIntegrationStatus::RecoveryRequired
                | VersionIntegrationStatus::ConflictResolutionRequired
        )
        .then(|| prepared.recovery_ref.clone()),
        kind: Some(prepared.kind),
        previous_head_oid: prepared.previous_head_oid.clone(),
        target_ref: prepared.target_ref.clone(),
        target_oid: prepared.target_oid.clone(),
        result_commit_oid: prepared.result_commit_oid.clone(),
        changed_paths,
        conflict_paths: prepared.conflict_paths.clone(),
        diagnostic,
        snapshot,
        workspace,
    }
}

#[tauri::command]
pub async fn read_versioning_snapshot(
    identity: VersioningSessionIdentity,
    app: AppHandle,
) -> Result<VersioningSnapshot, String> {
    read_with_repository(app, identity, VersionRepository::snapshot).await
}

#[tauri::command]
pub async fn initialize_versioning(
    identity: VersioningMutationIdentity,
    app: AppHandle,
) -> Result<VersioningSnapshot, String> {
    // Initialization is the one mutation whose expected snapshot is
    // `uninitialized`, so it validates the token itself before creating .git.
    let log_app = app.clone();
    let project_root = identity.expected_project_root.clone();
    let result: Result<VersioningSnapshot, String> =
        tauri::async_runtime::spawn_blocking(move || {
            let state = app.state::<AppState>();
            let _operation = state
                .versioning_operation
                .lock()
                .map_err(|_| "Mutex-ul operațiilor Git este compromis.".to_string())?;
            with_mutation_preflight(&app, &identity, |repository| {
                let before = repository.snapshot()?;
                if before.status_token != identity.expected_status_token
                    || before.head_oid != identity.expected_head_oid
                {
                    return Err(
                    "Starea Git s-a schimbat înainte de inițializare; actualizează panoul Versiuni."
                        .to_string(),
                );
                }
                repository.initialize()
            })
        })
        .await
        .map_err(|error| format!("Inițializarea Git a căzut în task-ul de fundal: {error}"))?;
    match &result {
        Ok(_) => record_versioning_event(
            &log_app,
            KernelLogLevel::Info,
            KernelEventKind::VersioningMutationCommitted,
            "initialize",
            Some(project_root),
            "Repository-ul Git local a fost inițializat în sursa/.",
            None,
        ),
        Err(error) => record_versioning_event(
            &log_app,
            KernelLogLevel::Warn,
            KernelEventKind::VersioningMutationFailed,
            "initialize",
            Some(project_root),
            "Inițializarea repository-ului Git a fost blocată.",
            Some(error.clone()),
        ),
    }
    result
}

#[tauri::command]
pub async fn configure_versioning_identity(
    identity: VersioningMutationIdentity,
    input: VersioningIdentityInput,
    app: AppHandle,
) -> Result<VersioningSnapshot, String> {
    mutate_with_repository(app, identity, "configure_identity", move |repository| {
        repository.configure_identity(&input.name, &input.email)
    })
    .await
}

#[tauri::command]
pub async fn configure_version_remote(
    identity: VersioningMutationIdentity,
    input: VersionRemoteInput,
    app: AppHandle,
) -> Result<VersioningSnapshot, String> {
    mutate_with_repository(app, identity, "configure_remote", move |repository| {
        repository.configure_remote(&input)
    })
    .await
}

#[tauri::command]
pub async fn remove_version_remote(
    identity: VersioningMutationIdentity,
    input: VersionRemoteNameInput,
    app: AppHandle,
) -> Result<VersioningSnapshot, String> {
    mutate_with_repository(app, identity, "remove_remote", move |repository| {
        repository.remove_remote(&input.name)
    })
    .await
}

#[tauri::command]
pub async fn configure_version_upstream(
    identity: VersioningMutationIdentity,
    input: VersionUpstreamInput,
    app: AppHandle,
) -> Result<VersioningSnapshot, String> {
    mutate_with_repository(app, identity, "configure_upstream", move |repository| {
        repository.configure_upstream(&input)
    })
    .await
}

#[tauri::command]
pub async fn clear_version_upstream(
    identity: VersioningMutationIdentity,
    input: VersionBranchNameInput,
    app: AppHandle,
) -> Result<VersioningSnapshot, String> {
    mutate_with_repository(app, identity, "clear_upstream", move |repository| {
        repository.clear_upstream(&input.name)
    })
    .await
}

#[tauri::command]
pub async fn create_version_branch(
    identity: VersioningMutationIdentity,
    input: VersionBranchInput,
    app: AppHandle,
) -> Result<VersioningSnapshot, String> {
    mutate_with_repository(app, identity, "create_branch", move |repository| {
        repository.create_branch(&input)
    })
    .await
}

#[tauri::command]
pub async fn delete_version_branch(
    identity: VersioningMutationIdentity,
    input: VersionBranchNameInput,
    app: AppHandle,
) -> Result<VersioningSnapshot, String> {
    mutate_with_repository(app, identity, "delete_branch", move |repository| {
        repository.delete_branch(&input.name)
    })
    .await
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
        move |repository, cancellation, progress| {
            repository.fetch_remote(
                &input.remote,
                input.prune,
                &input.operation_id,
                cancellation,
                progress,
            )
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
        move |repository, cancellation, progress| {
            repository.push_branch(&input, cancellation, progress)
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
    let active = state
        .versioning_network_operation
        .lock()
        .map_err(|_| "Mutex-ul operației Git de rețea este compromis.".to_string())?;
    let cancellation_requested = if let Some(active) = active.as_ref() {
        if active.operation_id != operation_id {
            return Err(format!(
                "Operația Git activă este {}, nu {operation_id}.",
                active.operation_id
            ));
        }
        if active.project_root != identity.expected_project_root
            || active.session_id != identity.expected_session_id
        {
            return Err(
                "Anularea Git a fost refuzată deoarece proiectul sau sesiunea nu corespund."
                    .to_string(),
            );
        }
        active.cancellation.store(true, Ordering::SeqCst);
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

#[tauri::command]
pub async fn read_version_sync_comparison(
    identity: VersioningSessionIdentity,
    app: AppHandle,
) -> Result<VersionSyncComparison, String> {
    read_with_repository(app, identity, VersionRepository::sync_comparison).await
}

#[tauri::command]
pub async fn read_version_integration_plan(
    identity: VersioningSessionIdentity,
    input: VersionIntegrationTargetInput,
    app: AppHandle,
) -> Result<VersionIntegrationPlan, String> {
    read_with_repository(app, identity, move |repository| {
        repository.integration_plan(&input)
    })
    .await
}

#[tauri::command]
pub async fn integrate_version_target(
    identity: VersioningMutationIdentity,
    input: VersionIntegrationInput,
    app: AppHandle,
) -> Result<VersionIntegrationReceipt, String> {
    let requested_ref = input.target_ref.clone();
    let requested_oid = input.expected_target_oid.clone();
    execute_source_integration(
        app,
        identity,
        "integrate_target",
        requested_ref,
        requested_oid,
        move |repository, _snapshot| {
            let plan = repository.integration_plan(&VersionIntegrationTargetInput {
                target_ref: input.target_ref.clone(),
                expected_target_oid: input.expected_target_oid.clone(),
            })?;
            if matches!(
                plan.relationship,
                VersionIntegrationRelationship::Same | VersionIntegrationRelationship::LocalAhead
            ) {
                return Ok(None);
            }
            repository.prepare_integration(&input).map(Some)
        },
    )
    .await
}

#[tauri::command]
pub async fn switch_version_branch(
    identity: VersioningMutationIdentity,
    input: VersionSwitchBranchInput,
    app: AppHandle,
) -> Result<VersionIntegrationReceipt, String> {
    let target_ref = format!("refs/heads/{}", input.branch.trim());
    let target_oid = input.expected_target_oid.clone();
    execute_source_integration(
        app,
        identity,
        "switch_branch",
        target_ref,
        target_oid,
        move |repository, _snapshot| repository.prepare_branch_switch(&input).map(Some),
    )
    .await
}

#[tauri::command]
pub async fn stage_versioning_paths(
    identity: VersioningMutationIdentity,
    input: VersioningPathsInput,
    app: AppHandle,
) -> Result<VersioningMutationReceipt, String> {
    let touched_paths = input.paths.clone();
    mutate_with_repository(app, identity, "stage_paths", move |repository| {
        let before = repository.snapshot()?;
        let snapshot = repository.stage_paths(&input.paths)?;
        Ok(VersioningMutationReceipt {
            schema_version: VERSIONING_SCHEMA_VERSION,
            changed: snapshot.status_token != before.status_token,
            touched_paths,
            snapshot,
        })
    })
    .await
}

#[tauri::command]
pub async fn stage_all_versioning(
    identity: VersioningMutationIdentity,
    app: AppHandle,
) -> Result<VersioningMutationReceipt, String> {
    mutate_with_repository(app, identity, "stage_all", |repository| {
        let before = repository.snapshot()?;
        let touched_paths = before.files.iter().map(|file| file.path.clone()).collect();
        let snapshot = repository.stage_all()?;
        Ok(VersioningMutationReceipt {
            schema_version: VERSIONING_SCHEMA_VERSION,
            changed: snapshot.status_token != before.status_token,
            touched_paths,
            snapshot,
        })
    })
    .await
}

#[tauri::command]
pub async fn unstage_versioning_paths(
    identity: VersioningMutationIdentity,
    input: VersioningPathsInput,
    app: AppHandle,
) -> Result<VersioningMutationReceipt, String> {
    let touched_paths = input.paths.clone();
    mutate_with_repository(app, identity, "unstage_paths", move |repository| {
        let before = repository.snapshot()?;
        let snapshot = repository.unstage_paths(&input.paths)?;
        Ok(VersioningMutationReceipt {
            schema_version: VERSIONING_SCHEMA_VERSION,
            changed: snapshot.status_token != before.status_token,
            touched_paths,
            snapshot,
        })
    })
    .await
}

#[tauri::command]
pub async fn unstage_all_versioning(
    identity: VersioningMutationIdentity,
    app: AppHandle,
) -> Result<VersioningMutationReceipt, String> {
    mutate_with_repository(app, identity, "unstage_all", |repository| {
        let before = repository.snapshot()?;
        let touched_paths = before
            .files
            .iter()
            .filter(|file| file.staged)
            .map(|file| file.path.clone())
            .collect();
        let snapshot = repository.unstage_all()?;
        Ok(VersioningMutationReceipt {
            schema_version: VERSIONING_SCHEMA_VERSION,
            changed: snapshot.status_token != before.status_token,
            touched_paths,
            snapshot,
        })
    })
    .await
}

#[tauri::command]
pub async fn commit_versioning(
    identity: VersioningMutationIdentity,
    input: VersioningCommitInput,
    app: AppHandle,
) -> Result<VersioningCommitReceipt, String> {
    let expected_head_oid = identity.expected_head_oid.clone();
    mutate_with_repository(app, identity, "commit", move |repository| {
        repository.commit(&input.message, expected_head_oid.as_deref())
    })
    .await
}

#[tauri::command]
pub async fn read_version_history(
    identity: VersioningSessionIdentity,
    offset: usize,
    limit: usize,
    app: AppHandle,
) -> Result<VersionHistoryPage, String> {
    read_with_repository(app, identity, move |repository| {
        repository.history(offset, limit)
    })
    .await
}

#[tauri::command]
pub async fn read_version_diff(
    identity: VersioningSessionIdentity,
    input: VersionDiffInput,
    app: AppHandle,
) -> Result<VersionDiffReceipt, String> {
    read_with_repository(app, identity, move |repository| repository.diff(&input)).await
}

#[tauri::command]
pub async fn preview_version(
    identity: VersioningSessionIdentity,
    input: VersionPreviewInput,
    app: AppHandle,
) -> Result<VersionPreviewReceipt, String> {
    let log_app = app.clone();
    let result: Result<VersionPreviewReceipt, String> =
        tauri::async_runtime::spawn_blocking(move || {
            let state = app.state::<AppState>();
            let captured = capture_read_session(state.inner(), &identity)?;
            let tree = captured
                .with_repository(&app, |repository| repository.read_tree(&input.commit_oid))?;
            stop_version_source_browser(&app, state.inner());
            let authority = app.state::<WriteAuthorityRuntime>();
            let _session_lease = authority.acquire_active_project_read_lease_for_session(
                &captured.root,
                &captured.runtime_session_id,
            )?;
            let files = tree
                .files
                .iter()
                .map(|file| (file.path.clone(), file.bytes.clone()))
                .collect::<Vec<_>>();
            let source_root = materialize_version_source_tree(
                &app,
                &captured.repository_root,
                &captured.runtime_session_id,
                &tree.commit_oid,
                &files,
            )?;
            let preview_url = start_version_source_browser(
                &app,
                state.inner(),
                &source_root,
                &captured.session.project_root,
                &captured.runtime_session_id,
                &tree.commit_oid,
            )?;
            Ok(VersionPreviewReceipt {
                schema_version: VERSIONING_SCHEMA_VERSION,
                project_root: captured.session.project_root,
                session_id: captured.runtime_session_id,
                short_oid: tree.commit_oid.chars().take(8).collect(),
                commit_oid: tree.commit_oid,
                preview_url,
                file_count: tree.files.len(),
                total_bytes: tree.total_bytes,
            })
        })
        .await
        .map_err(|error| format!("Preview-ul versiunii a căzut în task-ul de fundal: {error}"))?;
    match &result {
        Ok(receipt) => record_versioning_event(
            &log_app,
            KernelLogLevel::Info,
            KernelEventKind::VersioningPreviewStarted,
            "preview_version",
            Some(receipt.commit_oid.clone()),
            "Preview-ul izolat al versiunii Git a fost publicat.",
            None,
        ),
        Err(error) => record_versioning_event(
            &log_app,
            KernelLogLevel::Warn,
            KernelEventKind::VersioningMutationFailed,
            "preview_version",
            None,
            "Preview-ul izolat al versiunii Git a eșuat.",
            Some(error.clone()),
        ),
    }
    result
}

#[tauri::command]
pub async fn stop_version_preview(
    identity: VersioningSessionIdentity,
    app: AppHandle,
) -> Result<(), String> {
    let log_app = app.clone();
    let project_root = identity.expected_project_root.clone();
    let result: Result<(), String> = tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        capture_read_session(state.inner(), &identity)?;
        stop_version_source_browser(&app, state.inner());
        Ok(())
    })
    .await
    .map_err(|error| format!("Oprirea Preview-ului versiunii a căzut: {error}"))?;
    if result.is_ok() {
        record_versioning_event(
            &log_app,
            KernelLogLevel::Info,
            KernelEventKind::VersioningPreviewStopped,
            "stop_version_preview",
            Some(project_root),
            "Preview-ul izolat al versiunii Git a fost oprit.",
            None,
        );
    }
    result
}

#[tauri::command]
pub async fn restore_version(
    identity: VersioningMutationIdentity,
    input: VersionRestoreInput,
    app: AppHandle,
) -> Result<VersionRestoreReceipt, String> {
    let log_app = app.clone();
    let requested_target = input.target_commit_oid.clone();
    let result: Result<VersionRestoreReceipt, String> =
        tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let _operation = state
            .versioning_operation
            .lock()
            .map_err(|_| "Mutex-ul operațiilor Git este compromis.".to_string())?;
        state
            .ai_coordination
            .require_user_source_mutation()
            .map_err(|error| error.to_string())?;

        // Both guards remain held until Git HEAD and ProjectWorkspace have
        // reached the same durable version. This excludes Save, draft edits
        // and project transitions from the entire restore transaction.
        let root_guard = state.current_root.lock().map_err(|_| {
            "Nu am putut bloca root-ul proiectului pentru restaurare.".to_string()
        })?;
        let root = root_guard
            .as_ref()
            .ok_or_else(|| "Nu există proiect deschis pentru restaurare.".to_string())?;
        let mut workspace_guard = state.project_workspace.lock().map_err(|_| {
            "Nu am putut bloca ProjectWorkspace pentru restaurare.".to_string()
        })?;
        let workspace = workspace_guard
            .as_mut()
            .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru restaurare.".to_string())?;
        let captured = capture_from_workspace(
            root,
            &workspace.session,
            &workspace.runtime_session_id(),
            &identity.expected_project_root,
            &identity.expected_session_id,
        )?;
        if workspace.is_dirty() {
            return Err(
                "Restaurarea cere un ProjectWorkspace curat. Salvează sau anulează modificările înainte de restaurare."
                    .to_string(),
            );
        }
        require_recovery_coordinator_clean_for_write(&state, &workspace.session, "Restaurare versiune")?;
        workspace.accepted_disk.require_live_complete(
            &workspace.runtime_session_id(),
            &workspace.session.project_root,
            root,
        )?;

        let authority = app.state::<WriteAuthorityRuntime>();
        let session_lease = authority.acquire_active_project_read_lease_for_session(
            &captured.root,
            &captured.runtime_session_id,
        )?;
        let directory = session_lease.capture_subprocess_directory(
            Path::new("sursa"),
            "versioning/restore-git-repository-cwd",
        )?;
        let repository = VersionRepository::new(
            captured.session.project_root.clone(),
            captured.repository_root.clone(),
            directory.current_dir_path(),
        );
        let before = repository.require_status_token(
            &identity.expected_status_token,
            identity.expected_head_oid.as_deref(),
        )?;
        if !repository.read_restore_markers()?.is_empty() {
            return Err(
                "Nu poate începe o restaurare nouă cât timp există un marker de recovery. Rezolvă mai întâi restaurarea pendentă."
                    .to_string(),
            );
        }
        if !repository.read_integration_markers()?.is_empty() {
            return Err(
                "Restaurarea este blocată de o integrare Git pendentă. Rezolvă mai întâi Recovery-ul integrării."
                    .to_string(),
            );
        }
        let previous_head_oid = before.head_oid.clone().ok_or_else(|| {
            "Restaurarea cere cel puțin un commit existent pe branch-ul activ.".to_string()
        })?;
        if !before.clean {
            return Err(
                "Restaurarea cere un repository Git complet curat, inclusiv fără fișiere untracked."
                    .to_string(),
            );
        }

        let target_tree = repository.read_tree(&input.target_commit_oid)?;
        let current_tree = repository.read_tree(&previous_head_oid)?;
        let mut plan = build_version_restore_plan(workspace, &current_tree, &target_tree)?;
        if plan.is_empty() {
            return Ok(VersionRestoreReceipt {
                schema_version: VERSIONING_SCHEMA_VERSION,
                status: VersionRestoreStatus::Noop,
                project_root: captured.session.project_root,
                session_id: captured.runtime_session_id,
                transaction_id: None,
                recovery_ref: None,
                target_commit_oid: target_tree.commit_oid,
                previous_head_oid: Some(previous_head_oid),
                restore_commit_oid: None,
                changed_paths: Vec::new(),
                diagnostic: Some(
                    "Versiunea aleasă are același arbore de fișiere ca versiunea curentă."
                        .to_string(),
                ),
                snapshot: Some(before),
                workspace: Some(workspace.snapshot()),
            });
        }

        // Git's current tree tells us which bytes should exist, while this
        // capability read proves the exact live baseline used by the atomic
        // ProjectWorkspace Save (and detects filters or external races).
        for change in &mut plan.binary_changes {
            let live = session_lease.read_bounded_regular_file(
                Path::new(&change.relative_path),
                32 * 1024 * 1024,
                "versioning/restore-binary-baseline",
            )?;
            let live_bytes = live.map(|snapshot| snapshot.bytes);
            if live_bytes != change.before {
                return Err(format!(
                    "Restaurarea a fost blocată: baseline-ul live pentru {} nu corespunde arborelui HEAD Git.",
                    change.relative_path
                ));
            }
            change.before = live_bytes;
        }

        let prepared = repository.prepare_restore(
            &target_tree,
            &input.message,
            &previous_head_oid,
        )?;
        let changed_paths = plan.changed_paths.clone();
        let expected_files = plan.expected_files.clone();
        let mut candidate = workspace.clone();
        let workspace_identity = ProjectWorkspaceIdentity {
            expected_project_root: captured.session.project_root.clone(),
            expected_session_id: captured.runtime_session_id.clone(),
            expected_revision: candidate.revision,
        };
        let metadata = WorkspaceMutationMetadata {
            label: format!(
                "Restore Git {}",
                target_tree.commit_oid.chars().take(8).collect::<String>()
            ),
            source: "versioning_restore".to_string(),
            coalesce_key: None,
            transaction_id: Some(prepared.transaction_id.clone()),
        };
        if let Err(error) = candidate.stage_version_tree_restore(
            &workspace_identity,
            metadata,
            plan.text_changes,
            plan.text_deletes,
            plan.binary_changes,
            now_ms(),
        ) {
            let cleanup = repository.cancel_prepared_restore(&prepared);
            return Err(match cleanup {
                Ok(()) => error,
                Err(cleanup_error) => format!(
                    "{error} Marker-ul durabil {} nu a putut fi eliminat: {cleanup_error}",
                    prepared.recovery_ref
                ),
            });
        }

        // Save needs the exclusive project publication authority; the stable
        // Git cwd capability remains alive in `directory` across this gap.
        drop(session_lease);
        match save_project_workspace_with_recovery(&app, root, &mut candidate, &workspace_identity) {
            Ok(_) => {}
            Err(ProjectWorkspaceSaveError::Rejected { diagnostic }) => {
                let cleanup = repository.cancel_prepared_restore(&prepared);
                return Err(match cleanup {
                    Ok(()) => diagnostic,
                    Err(cleanup_error) => format!(
                        "{diagnostic} Marker-ul durabil {} a fost păstrat deoarece cleanup-ul a eșuat: {cleanup_error}",
                        prepared.recovery_ref
                    ),
                });
            }
            Err(ProjectWorkspaceSaveError::RecoveryRequired { diagnostic, .. }) => {
                return Ok(restore_recovery_receipt(
                    &captured,
                    &prepared,
                    changed_paths,
                    format!(
                        "Save-ul restaurării are nevoie de recovery: {diagnostic} Marker-ul Git durabil a fost păstrat. Nu repeta restaurarea automat."
                    ),
                    None,
                ));
            }
        }

        // The disk is now target_tree. Publish the accepted candidate in RAM
        // before finalizing Git so every later recovery path sees one coherent
        // ProjectWorkspace generation.
        *workspace = candidate;

        let verify_lease = authority.acquire_active_project_read_lease_for_session(
            &captured.root,
            &captured.runtime_session_id,
        )?;
        if let Err(error) = verify_restored_files(&verify_lease, &expected_files) {
            return Ok(restore_recovery_receipt(
                &captured,
                &prepared,
                changed_paths,
                format!(
                    "Fișierele restaurate nu au trecut verificarea byte-cu-byte: {error} Marker-ul Git durabil a fost păstrat; este necesar recovery explicit."
                ),
                Some(workspace.snapshot()),
            ));
        }
        drop(verify_lease);

        let finalization = match repository.finalize_restore(&prepared) {
            Ok(finalization) => finalization,
            Err(error) => {
                return Ok(restore_recovery_receipt(
                    &captured,
                    &prepared,
                    changed_paths,
                    format!(
                        "Fișierele sunt restaurate, dar commit-ul de restaurare nu a putut fi publicat: {error} Marker-ul Git durabil a fost păstrat; este necesar recovery explicit."
                    ),
                    Some(workspace.snapshot()),
                ));
            }
        };
        Ok(VersionRestoreReceipt {
            schema_version: VERSIONING_SCHEMA_VERSION,
            status: VersionRestoreStatus::Restored,
            project_root: captured.session.project_root,
            session_id: captured.runtime_session_id,
            transaction_id: Some(prepared.transaction_id),
            recovery_ref: finalization
                .cleanup_required
                .then_some(prepared.recovery_ref),
            target_commit_oid: prepared.target_commit_oid,
            previous_head_oid: Some(prepared.previous_head_oid),
            restore_commit_oid: Some(prepared.restore_commit_oid),
            changed_paths,
            diagnostic: finalization.diagnostic,
            snapshot: finalization.snapshot,
            workspace: Some(workspace.snapshot()),
        })
    })
    .await
    .map_err(|error| format!("Restaurarea Git a căzut în task-ul de fundal: {error}"))?;
    match &result {
        Ok(receipt) if receipt.status == VersionRestoreStatus::RecoveryRequired => {
            record_versioning_event(
                &log_app,
                KernelLogLevel::Warn,
                KernelEventKind::VersioningRestoreRecoveryRequired,
                "restore_version",
                Some(receipt.target_commit_oid.clone()),
                "Restaurarea Git a păstrat marker-ul durabil și cere recovery explicit.",
                receipt.diagnostic.clone(),
            )
        }
        Ok(receipt) => record_versioning_event(
            &log_app,
            KernelLogLevel::Info,
            KernelEventKind::VersioningRestorePublished,
            "restore_version",
            Some(receipt.target_commit_oid.clone()),
            if receipt.status == VersionRestoreStatus::Noop {
                "Restaurarea Git a fost un no-op demonstrat."
            } else {
                "Restaurarea Git a fost publicată printr-un commit nou."
            },
            receipt.diagnostic.clone(),
        ),
        Err(error) => record_versioning_event(
            &log_app,
            KernelLogLevel::Warn,
            KernelEventKind::VersioningMutationFailed,
            "restore_version",
            Some(requested_target),
            "Restaurarea Git a fost blocată înainte de publicare.",
            Some(error.clone()),
        ),
    }
    result
}

#[tauri::command]
pub async fn read_version_restore_recovery(
    identity: VersioningSessionIdentity,
    app: AppHandle,
) -> Result<VersionRestoreRecoveryScan, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let root_guard = state.current_root.lock().map_err(|_| {
            "Nu am putut bloca root-ul pentru scanarea recovery Git.".to_string()
        })?;
        let root = root_guard
            .as_ref()
            .ok_or_else(|| "Nu există proiect deschis pentru recovery Git.".to_string())?;
        let workspace_guard = state.project_workspace.lock().map_err(|_| {
            "Nu am putut bloca ProjectWorkspace pentru scanarea recovery Git.".to_string()
        })?;
        let workspace = workspace_guard
            .as_ref()
            .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru recovery Git.".to_string())?;
        let captured = capture_from_workspace(
            root,
            &workspace.session,
            &workspace.runtime_session_id(),
            &identity.expected_project_root,
            &identity.expected_session_id,
        )?;
        let authority = app.state::<WriteAuthorityRuntime>();
        let lease = authority.acquire_active_project_read_lease_for_session(
            &captured.root,
            &captured.runtime_session_id,
        )?;
        let directory = lease.capture_subprocess_directory(
            Path::new("sursa"),
            "versioning/recovery-scan-git-cwd",
        )?;
        let repository = VersionRepository::new(
            captured.session.project_root.clone(),
            captured.repository_root.clone(),
            directory.current_dir_path(),
        );
        let snapshot = repository.snapshot()?;
        let markers = repository.read_restore_markers()?;
        if markers.len() > 32 {
            return Err(
                "Recovery Git a găsit peste 32 de restaurări pendinte; este necesară inspecție manuală."
                    .to_string(),
            );
        }
        let mut items = Vec::with_capacity(markers.len());
        for marker in markers {
            let previous_tree = repository.read_tree(&marker.previous_head_oid)?;
            let target_tree = repository.read_tree(&marker.target_commit_oid)?;
            items.push(classify_restore_marker(
                &lease,
                &snapshot,
                &marker,
                &previous_tree,
                &target_tree,
                workspace.is_dirty(),
            )?);
        }
        Ok(VersionRestoreRecoveryScan {
            schema_version: VERSIONING_SCHEMA_VERSION,
            project_root: captured.session.project_root,
            session_id: captured.runtime_session_id,
            items,
        })
    })
    .await
    .map_err(|error| format!("Scanarea recovery Git a căzut în task-ul de fundal: {error}"))?
}

#[tauri::command]
pub async fn read_version_integration_recovery(
    identity: VersioningSessionIdentity,
    app: AppHandle,
) -> Result<VersionIntegrationRecoveryScan, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let root_guard = state
            .current_root
            .lock()
            .map_err(|_| "Nu am putut bloca root-ul pentru scanarea integrării Git.".to_string())?;
        let root = root_guard
            .as_ref()
            .ok_or_else(|| "Nu există proiect deschis pentru integrarea Git.".to_string())?;
        let workspace_guard = state.project_workspace.lock().map_err(|_| {
            "Nu am putut bloca ProjectWorkspace pentru scanarea integrării Git.".to_string()
        })?;
        let workspace = workspace_guard.as_ref().ok_or_else(|| {
            "ProjectWorkspace nu este inițializat pentru integrarea Git.".to_string()
        })?;
        let captured = capture_from_workspace(
            root,
            &workspace.session,
            &workspace.runtime_session_id(),
            &identity.expected_project_root,
            &identity.expected_session_id,
        )?;
        let authority = app.state::<WriteAuthorityRuntime>();
        let lease = authority.acquire_active_project_read_lease_for_session(
            &captured.root,
            &captured.runtime_session_id,
        )?;
        let directory = lease.capture_subprocess_directory(
            Path::new("sursa"),
            "versioning/integration-recovery-scan-git-cwd",
        )?;
        let repository = VersionRepository::new(
            captured.session.project_root.clone(),
            captured.repository_root.clone(),
            directory.current_dir_path(),
        );
        let snapshot = repository.snapshot()?;
        let markers = repository.read_integration_markers()?;
        let mut items = Vec::with_capacity(markers.len());
        for marker in markers {
            let previous_tree = repository.previous_integration_tree(&marker)?;
            let target_tree = repository.integration_tree(&marker)?;
            items.push(classify_integration_marker(
                &lease,
                &snapshot,
                &marker,
                &previous_tree,
                &target_tree,
                workspace.is_dirty(),
            )?);
        }
        Ok(VersionIntegrationRecoveryScan {
            schema_version: VERSIONING_SCHEMA_VERSION,
            project_root: captured.session.project_root,
            session_id: captured.runtime_session_id,
            items,
        })
    })
    .await
    .map_err(|error| format!("Scanarea integrării Git a căzut în task-ul de fundal: {error}"))?
}

#[tauri::command]
pub async fn resolve_version_integration_recovery(
    identity: VersioningMutationIdentity,
    input: VersionIntegrationRecoveryResolutionInput,
    app: AppHandle,
) -> Result<VersionIntegrationRecoveryResolutionReceipt, String> {
    let log_app = app.clone();
    let requested_root = identity.expected_project_root.clone();
    let result: Result<VersionIntegrationRecoveryResolutionReceipt, String> =
        tauri::async_runtime::spawn_blocking(move || {
            let state = app.state::<AppState>();
            let _operation = state
                .versioning_operation
                .lock()
                .map_err(|_| "Mutex-ul operațiilor Git este compromis.".to_string())?;
            state
                .ai_coordination
                .require_user_source_mutation()
                .map_err(|error| error.to_string())?;
            let root_guard = state.current_root.lock().map_err(|_| {
                "Nu am putut bloca root-ul pentru recovery integrare Git.".to_string()
            })?;
            let root = root_guard.as_ref().ok_or_else(|| {
                "Nu există proiect deschis pentru recovery integrare Git.".to_string()
            })?;
            let mut workspace_guard = state.project_workspace.lock().map_err(|_| {
                "Nu am putut bloca ProjectWorkspace pentru recovery integrare Git.".to_string()
            })?;
            let workspace = workspace_guard.as_mut().ok_or_else(|| {
                "ProjectWorkspace nu este inițializat pentru recovery integrare Git.".to_string()
            })?;
            let captured = capture_from_workspace(
                root,
                &workspace.session,
                &workspace.runtime_session_id(),
                &identity.expected_project_root,
                &identity.expected_session_id,
            )?;
            if workspace.is_dirty() {
                return Err(
                    "Recovery-ul integrării cere un ProjectWorkspace curat. Salvează sau anulează editările."
                        .to_string(),
                );
            }
            require_recovery_coordinator_clean_for_write(
                &state,
                &workspace.session,
                "Recovery integrare Git",
            )?;
            workspace.accepted_disk.require_live_complete(
                &workspace.runtime_session_id(),
                &workspace.session.project_root,
                root,
            )?;
            let authority = app.state::<WriteAuthorityRuntime>();
            let lease = authority.acquire_active_project_read_lease_for_session(
                &captured.root,
                &captured.runtime_session_id,
            )?;
            let directory = lease.capture_subprocess_directory(
                Path::new("sursa"),
                "versioning/integration-recovery-resolve-git-cwd",
            )?;
            let repository = VersionRepository::new(
                captured.session.project_root.clone(),
                captured.repository_root.clone(),
                directory.current_dir_path(),
            );
            let snapshot = repository.require_status_token(
                &identity.expected_status_token,
                identity.expected_head_oid.as_deref(),
            )?;
            let marker = repository
                .read_integration_markers()?
                .into_iter()
                .find(|marker| marker.recovery_ref == input.recovery_ref)
                .ok_or_else(|| {
                    format!(
                        "Marker-ul integrării {} nu mai există; actualizează panoul Versiuni.",
                        input.recovery_ref
                    )
                })?;
            let previous_tree = repository.previous_integration_tree(&marker)?;
            let target_tree = repository.integration_tree(&marker)?;
            let classification = classify_integration_marker(
                &lease,
                &snapshot,
                &marker,
                &previous_tree,
                &target_tree,
                false,
            )?;
            if !classification.available_actions.contains(&input.action) {
                return Err(format!(
                    "Acțiunea {:?} nu este sigură pentru starea {:?}: {}",
                    input.action, classification.state, classification.diagnostic
                ));
            }

            let receipt = match input.action {
                VersionIntegrationRecoveryAction::Finalize => {
                    let snapshot = repository.finalize_integration(&marker)?;
                    integration_recovery_resolution_receipt(
                        &captured,
                        &marker,
                        input.action,
                        true,
                        None,
                        Some(snapshot),
                        Some(workspace.snapshot()),
                    )
                }
                VersionIntegrationRecoveryAction::Cleanup => {
                    repository.delete_integration_marker(&marker)?;
                    integration_recovery_resolution_receipt(
                        &captured,
                        &marker,
                        input.action,
                        true,
                        None,
                        Some(repository.snapshot()?),
                        Some(workspace.snapshot()),
                    )
                }
                VersionIntegrationRecoveryAction::Continue => {
                    if !integration_conflict_markers_resolved(&lease, &marker.conflict_paths)? {
                        return Err(
                            "Merge-ul conține încă markere standard de conflict.".to_string(),
                        );
                    }
                    drop(lease);
                    let resolved = repository.promote_conflict_resolution(&marker)?;
                    match repository.finalize_integration(&resolved) {
                        Ok(snapshot) => integration_recovery_resolution_receipt(
                            &captured,
                            &resolved,
                            input.action,
                            true,
                            None,
                            Some(snapshot),
                            Some(workspace.snapshot()),
                        ),
                        Err(error) => integration_recovery_resolution_receipt(
                            &captured,
                            &resolved,
                            input.action,
                            false,
                            Some(format!(
                                "Commit-ul merge rezolvat este pregătit durabil, dar publicarea a eșuat: {error}"
                            )),
                            repository.snapshot().ok(),
                            Some(workspace.snapshot()),
                        ),
                    }
                }
                VersionIntegrationRecoveryAction::Rollback
                    if classification.state
                        == VersionIntegrationRecoveryState::ReadyToRollback =>
                {
                    let snapshot = repository.abort_integration_metadata(&marker)?;
                    integration_recovery_resolution_receipt(
                        &captured,
                        &marker,
                        input.action,
                        true,
                        None,
                        Some(snapshot),
                        Some(workspace.snapshot()),
                    )
                }
                VersionIntegrationRecoveryAction::Rollback => {
                    let allowed = marker
                        .conflict_paths
                        .iter()
                        .cloned()
                        .collect::<BTreeSet<_>>();
                    match publish_integration_tree(
                        &app,
                        root,
                        workspace,
                        &captured,
                        &repository,
                        lease,
                        &marker,
                        &target_tree,
                        &previous_tree,
                        &allowed,
                        format!(
                            "Rollback integrare Git {}",
                            marker.target_oid.chars().take(8).collect::<String>()
                        ),
                        "versioning_integration_recovery",
                    )? {
                        IntegrationTreePublication::Applied { .. } => {
                            let snapshot = repository.abort_integration_metadata(&marker)?;
                            integration_recovery_resolution_receipt(
                                &captured,
                                &marker,
                                input.action,
                                true,
                                None,
                                Some(snapshot),
                                Some(workspace.snapshot()),
                            )
                        }
                        IntegrationTreePublication::RecoveryRequired { diagnostic, .. } => {
                            integration_recovery_resolution_receipt(
                                &captured,
                                &marker,
                                input.action,
                                false,
                                Some(diagnostic),
                                repository.snapshot().ok(),
                                Some(workspace.snapshot()),
                            )
                        }
                    }
                }
            };
            Ok(receipt)
        })
        .await
        .map_err(|error| format!("Recovery-ul integrării Git a căzut: {error}"))?;

    match &result {
        Ok(receipt) if receipt.resolved => record_versioning_event(
            &log_app,
            KernelLogLevel::Info,
            KernelEventKind::VersioningIntegrationRecoveryResolved,
            "resolve_integration_recovery",
            Some(requested_root),
            "Recovery-ul integrării Git a fost rezolvat.",
            receipt.diagnostic.clone(),
        ),
        Ok(receipt) => record_versioning_event(
            &log_app,
            KernelLogLevel::Warn,
            KernelEventKind::VersioningIntegrationRecoveryRequired,
            "resolve_integration_recovery",
            Some(requested_root),
            "Recovery-ul integrării Git necesită încă intervenție.",
            receipt.diagnostic.clone(),
        ),
        Err(error) => record_versioning_event(
            &log_app,
            KernelLogLevel::Warn,
            KernelEventKind::VersioningMutationFailed,
            "resolve_integration_recovery",
            Some(requested_root),
            "Recovery-ul integrării Git a fost blocat.",
            Some(error.clone()),
        ),
    }
    result
}

fn integration_recovery_resolution_receipt(
    captured: &CapturedVersioningSession,
    marker: &PreparedVersionIntegration,
    action: VersionIntegrationRecoveryAction,
    resolved: bool,
    diagnostic: Option<String>,
    snapshot: Option<VersioningSnapshot>,
    workspace: Option<crate::kernel::project_workspace::ProjectWorkspaceSnapshot>,
) -> VersionIntegrationRecoveryResolutionReceipt {
    VersionIntegrationRecoveryResolutionReceipt {
        schema_version: VERSIONING_SCHEMA_VERSION,
        project_root: captured.session.project_root.clone(),
        session_id: captured.runtime_session_id.clone(),
        transaction_id: marker.transaction_id.clone(),
        recovery_ref: marker.recovery_ref.clone(),
        action,
        resolved,
        diagnostic,
        snapshot,
        workspace,
    }
}

#[tauri::command]
pub async fn resolve_version_restore_recovery(
    identity: VersioningMutationIdentity,
    input: VersionRestoreRecoveryResolutionInput,
    app: AppHandle,
) -> Result<VersionRestoreRecoveryResolutionReceipt, String> {
    let log_app = app.clone();
    let requested_ref = input.recovery_ref.clone();
    let result: Result<VersionRestoreRecoveryResolutionReceipt, String> =
        tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let _operation = state
            .versioning_operation
            .lock()
            .map_err(|_| "Mutex-ul operațiilor Git este compromis.".to_string())?;
        state
            .ai_coordination
            .require_user_source_mutation()
            .map_err(|error| error.to_string())?;
        let root_guard = state.current_root.lock().map_err(|_| {
            "Nu am putut bloca root-ul proiectului pentru recovery Git.".to_string()
        })?;
        let root = root_guard
            .as_ref()
            .ok_or_else(|| "Nu există proiect deschis pentru recovery Git.".to_string())?;
        let mut workspace_guard = state.project_workspace.lock().map_err(|_| {
            "Nu am putut bloca ProjectWorkspace pentru recovery Git.".to_string()
        })?;
        let workspace = workspace_guard
            .as_mut()
            .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru recovery Git.".to_string())?;
        let captured = capture_from_workspace(
            root,
            &workspace.session,
            &workspace.runtime_session_id(),
            &identity.expected_project_root,
            &identity.expected_session_id,
        )?;
        if workspace.is_dirty() {
            return Err(
                "Recovery Git cere un ProjectWorkspace curat; păstrează sau anulează mai întâi editările curente."
                    .to_string(),
            );
        }
        require_recovery_coordinator_clean_for_write(
            &state,
            &workspace.session,
            "Recovery restaurare Git",
        )?;
        workspace.accepted_disk.require_live_complete(
            &workspace.runtime_session_id(),
            &workspace.session.project_root,
            root,
        )?;

        let authority = app.state::<WriteAuthorityRuntime>();
        let lease = authority.acquire_active_project_read_lease_for_session(
            &captured.root,
            &captured.runtime_session_id,
        )?;
        let directory = lease.capture_subprocess_directory(
            Path::new("sursa"),
            "versioning/recovery-resolve-git-cwd",
        )?;
        let repository = VersionRepository::new(
            captured.session.project_root.clone(),
            captured.repository_root.clone(),
            directory.current_dir_path(),
        );
        let snapshot = repository.require_status_token(
            &identity.expected_status_token,
            identity.expected_head_oid.as_deref(),
        )?;
        let marker = repository
            .read_restore_markers()?
            .into_iter()
            .find(|marker| marker.recovery_ref == input.recovery_ref)
            .ok_or_else(|| {
                format!(
                    "Marker-ul recovery {} nu mai există; actualizează panoul Versiuni.",
                    input.recovery_ref
                )
            })?;
        let previous_tree = repository.read_tree(&marker.previous_head_oid)?;
        let target_tree = repository.read_tree(&marker.target_commit_oid)?;
        let classification = classify_restore_marker(
            &lease,
            &snapshot,
            &marker,
            &previous_tree,
            &target_tree,
            false,
        )?;
        if !classification.available_actions.contains(&input.action) {
            return Err(format!(
                "Acțiunea {:?} nu este sigură pentru starea recovery {:?}: {}",
                input.action, classification.state, classification.diagnostic
            ));
        }

        match input.action {
            VersionRestoreRecoveryAction::Finalize => {
                let finalization = repository.finalize_restore(&marker)?;
                Ok(VersionRestoreRecoveryResolutionReceipt {
                    schema_version: VERSIONING_SCHEMA_VERSION,
                    project_root: captured.session.project_root,
                    session_id: captured.runtime_session_id,
                    transaction_id: marker.transaction_id,
                    recovery_ref: marker.recovery_ref,
                    action: input.action,
                    resolved: !finalization.cleanup_required,
                    diagnostic: finalization.diagnostic,
                    snapshot: finalization.snapshot,
                    workspace: Some(workspace.snapshot()),
                })
            }
            VersionRestoreRecoveryAction::Cleanup => {
                repository.cancel_prepared_restore(&marker)?;
                Ok(VersionRestoreRecoveryResolutionReceipt {
                    schema_version: VERSIONING_SCHEMA_VERSION,
                    project_root: captured.session.project_root,
                    session_id: captured.runtime_session_id,
                    transaction_id: marker.transaction_id,
                    recovery_ref: marker.recovery_ref,
                    action: input.action,
                    resolved: true,
                    diagnostic: None,
                    snapshot: Some(repository.snapshot()?),
                    workspace: Some(workspace.snapshot()),
                })
            }
            VersionRestoreRecoveryAction::Rollback
                if classification.state == VersionRestoreRecoveryState::ReadyToRollback =>
            {
                let snapshot = repository.abort_prepared_restore(&marker)?;
                Ok(VersionRestoreRecoveryResolutionReceipt {
                    schema_version: VERSIONING_SCHEMA_VERSION,
                    project_root: captured.session.project_root,
                    session_id: captured.runtime_session_id,
                    transaction_id: marker.transaction_id,
                    recovery_ref: marker.recovery_ref,
                    action: input.action,
                    resolved: true,
                    diagnostic: None,
                    snapshot: Some(snapshot),
                    workspace: Some(workspace.snapshot()),
                })
            }
            VersionRestoreRecoveryAction::Rollback => {
                let mut plan = build_version_restore_plan(workspace, &target_tree, &previous_tree)?;
                for change in &mut plan.binary_changes {
                    let live = lease.read_bounded_regular_file(
                        Path::new(&change.relative_path),
                        32 * 1024 * 1024,
                        "versioning/recovery-rollback-binary-baseline",
                    )?;
                    let live_bytes = live.map(|item| item.bytes);
                    if live_bytes != change.before {
                        return Err(format!(
                            "Rollback-ul a fost blocat: baseline-ul live pentru {} s-a schimbat.",
                            change.relative_path
                        ));
                    }
                    change.before = live_bytes;
                }
                let expected_files = expected_tree_files(&target_tree, &previous_tree);
                let mut candidate = workspace.clone();
                let workspace_identity = ProjectWorkspaceIdentity {
                    expected_project_root: captured.session.project_root.clone(),
                    expected_session_id: captured.runtime_session_id.clone(),
                    expected_revision: candidate.revision,
                };
                candidate.stage_version_tree_restore(
                    &workspace_identity,
                    WorkspaceMutationMetadata {
                        label: format!(
                            "Rollback restore Git {}",
                            marker
                                .target_commit_oid
                                .chars()
                                .take(8)
                                .collect::<String>()
                        ),
                        source: "versioning_restore_recovery".to_string(),
                        coalesce_key: None,
                        transaction_id: Some(format!("{}-rollback", marker.transaction_id)),
                    },
                    plan.text_changes,
                    plan.text_deletes,
                    plan.binary_changes,
                    now_ms(),
                )?;
                drop(lease);
                match save_project_workspace_with_recovery(
                    &app,
                    root,
                    &mut candidate,
                    &workspace_identity,
                ) {
                    Ok(_) => {}
                    Err(error) => {
                        return Ok(unresolved_recovery_resolution(
                            &captured,
                            &marker,
                            input.action,
                            format!(
                                "Rollback-ul recovery nu s-a încheiat: {error} Marker-ul Git a fost păstrat; nu repeta automat."
                            ),
                            Some(workspace.snapshot()),
                        ));
                    }
                }
                *workspace = candidate;
                let verify_lease = authority.acquire_active_project_read_lease_for_session(
                    &captured.root,
                    &captured.runtime_session_id,
                )?;
                if let Err(error) = verify_restored_files(&verify_lease, &expected_files) {
                    return Ok(unresolved_recovery_resolution(
                        &captured,
                        &marker,
                        input.action,
                        format!(
                            "Rollback-ul nu a trecut verificarea byte-cu-byte: {error} Marker-ul Git a fost păstrat."
                        ),
                        Some(workspace.snapshot()),
                    ));
                }
                drop(verify_lease);
                let snapshot = repository.abort_prepared_restore(&marker)?;
                Ok(VersionRestoreRecoveryResolutionReceipt {
                    schema_version: VERSIONING_SCHEMA_VERSION,
                    project_root: captured.session.project_root,
                    session_id: captured.runtime_session_id,
                    transaction_id: marker.transaction_id,
                    recovery_ref: marker.recovery_ref,
                    action: input.action,
                    resolved: true,
                    diagnostic: None,
                    snapshot: Some(snapshot),
                    workspace: Some(workspace.snapshot()),
                })
            }
        }
    })
    .await
    .map_err(|error| format!("Rezolvarea recovery Git a căzut în task-ul de fundal: {error}"))?;
    match &result {
        Ok(receipt) if receipt.resolved => record_versioning_event(
            &log_app,
            KernelLogLevel::Info,
            KernelEventKind::VersioningRestoreRecoveryResolved,
            "resolve_restore_recovery",
            Some(receipt.recovery_ref.clone()),
            "Recovery-ul restaurării Git a fost rezolvat explicit.",
            receipt.diagnostic.clone(),
        ),
        Ok(receipt) => record_versioning_event(
            &log_app,
            KernelLogLevel::Warn,
            KernelEventKind::VersioningRestoreRecoveryRequired,
            "resolve_restore_recovery",
            Some(receipt.recovery_ref.clone()),
            "Recovery-ul restaurării Git rămâne pendent; retry-ul automat este interzis.",
            receipt.diagnostic.clone(),
        ),
        Err(error) => record_versioning_event(
            &log_app,
            KernelLogLevel::Warn,
            KernelEventKind::VersioningMutationFailed,
            "resolve_restore_recovery",
            Some(requested_ref),
            "Rezoluția recovery Git a fost blocată.",
            Some(error.clone()),
        ),
    }
    result
}

fn classify_restore_marker(
    lease: &ActiveProjectReadLease<'_>,
    snapshot: &VersioningSnapshot,
    marker: &PreparedVersionRestore,
    previous_tree: &VersionTree,
    target_tree: &VersionTree,
    workspace_dirty: bool,
) -> Result<VersionRestoreRecoveryItem, String> {
    let changed_paths = changed_tree_paths(previous_tree, target_tree);
    let status_paths = snapshot
        .files
        .iter()
        .flat_map(|file| [Some(file.path.as_str()), file.original_path.as_deref()])
        .flatten()
        .collect::<BTreeSet<_>>();
    let status_scope_safe = snapshot.conflicted_count == 0
        && status_paths
            .iter()
            .all(|path| changed_paths.contains(*path));
    let target_matches = status_scope_safe
        && verify_restored_files(lease, &expected_tree_files(previous_tree, target_tree)).is_ok();
    let previous_matches = status_scope_safe
        && verify_restored_files(lease, &expected_tree_files(target_tree, previous_tree)).is_ok();
    let live_head = snapshot.head_oid.as_deref();

    let (state, available_actions, diagnostic) = if workspace_dirty {
        (
            VersionRestoreRecoveryState::ManualReview,
            Vec::new(),
            "ProjectWorkspace are editări nesalvate; recovery nu poate modifica sursele până la rezolvarea lor."
                .to_string(),
        )
    } else if live_head == Some(marker.restore_commit_oid.as_str())
        && target_matches
        && snapshot.clean
    {
        (
            VersionRestoreRecoveryState::CleanupRequired,
            vec![VersionRestoreRecoveryAction::Cleanup],
            "Commit-ul de restaurare este deja publicat; a rămas numai marker-ul intern de curățat."
                .to_string(),
        )
    } else if live_head == Some(marker.previous_head_oid.as_str()) && target_matches {
        (
            VersionRestoreRecoveryState::ReadyToFinalize,
            vec![
                VersionRestoreRecoveryAction::Finalize,
                VersionRestoreRecoveryAction::Rollback,
            ],
            "Fișierele corespund exact versiunii țintă, iar HEAD este încă versiunea anterioară. Restaurarea poate fi finalizată sau anulată."
                .to_string(),
        )
    } else if live_head == Some(marker.previous_head_oid.as_str()) && previous_matches {
        (
            VersionRestoreRecoveryState::ReadyToRollback,
            vec![VersionRestoreRecoveryAction::Rollback],
            "Fișierele corespund exact versiunii anterioare; indexul și marker-ul intern pot fi readuse la starea inițială."
                .to_string(),
        )
    } else {
        (
            VersionRestoreRecoveryState::ManualReview,
            Vec::new(),
            "HEAD sau fișierele live au divergat de ambele stări demonstrate. Pană Studio nu va presupune automat o rezoluție."
                .to_string(),
        )
    };
    Ok(VersionRestoreRecoveryItem {
        transaction_id: marker.transaction_id.clone(),
        recovery_ref: marker.recovery_ref.clone(),
        target_commit_oid: marker.target_commit_oid.clone(),
        previous_head_oid: marker.previous_head_oid.clone(),
        restore_commit_oid: marker.restore_commit_oid.clone(),
        state,
        available_actions,
        diagnostic,
    })
}

fn classify_integration_marker(
    lease: &ActiveProjectReadLease<'_>,
    snapshot: &VersioningSnapshot,
    marker: &PreparedVersionIntegration,
    previous_tree: &VersionTree,
    target_tree: &VersionTree,
    workspace_dirty: bool,
) -> Result<VersionIntegrationRecoveryItem, String> {
    let changed_paths = changed_tree_paths(previous_tree, target_tree);
    let status_paths = snapshot
        .files
        .iter()
        .flat_map(|file| [Some(file.path.as_str()), file.original_path.as_deref()])
        .flatten()
        .collect::<BTreeSet<_>>();
    let status_scope_safe = snapshot.conflicted_count == 0
        && status_paths
            .iter()
            .all(|path| changed_paths.contains(*path));
    let target_matches = status_scope_safe
        && verify_restored_files(lease, &expected_tree_files(previous_tree, target_tree)).is_ok();
    let previous_matches = status_scope_safe
        && verify_restored_files(lease, &expected_tree_files(target_tree, previous_tree)).is_ok();
    let initial_branch = marker.full_head_ref.strip_prefix("refs/heads/");
    let on_initial_head = snapshot.head_oid.as_deref() == Some(marker.previous_head_oid.as_str())
        && snapshot.branch.as_deref() == initial_branch;
    let published = match marker.kind {
        VersionIntegrationKind::SwitchBranch => {
            snapshot.head_oid.as_deref() == Some(marker.target_oid.as_str())
                && snapshot.branch.as_deref() == marker.target_branch.as_deref()
        }
        _ => marker.result_commit_oid.as_deref().is_some_and(|result| {
            snapshot.head_oid.as_deref() == Some(result)
                && snapshot.branch.as_deref() == initial_branch
        }),
    };

    let (state, available_actions, diagnostic) = if workspace_dirty {
        (
            VersionIntegrationRecoveryState::ManualReview,
            Vec::new(),
            "ProjectWorkspace are editări nesalvate; salvează sau anulează editările înainte de recovery."
                .to_string(),
        )
    } else if published && target_matches && snapshot.clean {
        (
            VersionIntegrationRecoveryState::CleanupRequired,
            vec![VersionIntegrationRecoveryAction::Cleanup],
            "Integrarea este deja publicată; a rămas numai marker-ul intern de curățat."
                .to_string(),
        )
    } else if marker.kind == VersionIntegrationKind::MergeConflict && on_initial_head {
        if previous_matches && status_paths.is_empty() {
            (
                VersionIntegrationRecoveryState::ReadyToRollback,
                vec![VersionIntegrationRecoveryAction::Rollback],
                "Arborele live este încă versiunea anterioară; merge-ul întrerupt poate fi anulat în siguranță."
                    .to_string(),
            )
        } else if integration_non_conflict_files_match(
            lease,
            previous_tree,
            target_tree,
            &marker.conflict_paths,
        ) && status_scope_safe
        {
            let resolved = integration_conflict_markers_resolved(lease, &marker.conflict_paths)?;
            let mut actions = vec![VersionIntegrationRecoveryAction::Rollback];
            if resolved {
                actions.insert(0, VersionIntegrationRecoveryAction::Continue);
            }
            (
                VersionIntegrationRecoveryState::ConflictResolution,
                actions,
                if resolved {
                    "Fișierele fără conflict corespund merge-ului, iar markerele standard nu mai sunt prezente. Merge-ul poate fi continuat sau anulat."
                        .to_string()
                } else {
                    "Rezolvă toate markerele <<<<<<<, ======= și >>>>>>> din fișierele conflictuale, apoi salvează proiectul."
                        .to_string()
                },
            )
        } else {
            (
                VersionIntegrationRecoveryState::ManualReview,
                Vec::new(),
                "Fișiere din afara conflictelor declarate au divergat; Pană Studio nu va continua automat merge-ul."
                    .to_string(),
            )
        }
    } else if on_initial_head && target_matches {
        (
            VersionIntegrationRecoveryState::ReadyToFinalize,
            vec![
                VersionIntegrationRecoveryAction::Finalize,
                VersionIntegrationRecoveryAction::Rollback,
            ],
            "Fișierele corespund exact integrării pregătite; referința Git poate fi finalizată sau operația poate fi anulată."
                .to_string(),
        )
    } else if on_initial_head && previous_matches {
        (
            VersionIntegrationRecoveryState::ReadyToRollback,
            vec![VersionIntegrationRecoveryAction::Rollback],
            "Fișierele corespund exact arborelui anterior; integrarea poate fi anulată în siguranță."
                .to_string(),
        )
    } else {
        (
            VersionIntegrationRecoveryState::ManualReview,
            Vec::new(),
            "HEAD, branch-ul activ sau fișierele live au divergat de stările demonstrate; este necesară inspecție manuală."
                .to_string(),
        )
    };

    Ok(VersionIntegrationRecoveryItem {
        transaction_id: marker.transaction_id.clone(),
        recovery_ref: marker.recovery_ref.clone(),
        kind: marker.kind,
        previous_head_oid: marker.previous_head_oid.clone(),
        target_ref: marker.target_ref.clone(),
        target_oid: marker.target_oid.clone(),
        result_commit_oid: marker.result_commit_oid.clone(),
        conflict_paths: marker.conflict_paths.clone(),
        state,
        available_actions,
        diagnostic,
    })
}

fn integration_non_conflict_files_match(
    lease: &ActiveProjectReadLease<'_>,
    previous_tree: &VersionTree,
    target_tree: &VersionTree,
    conflicts: &[String],
) -> bool {
    let conflicts = conflicts
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let expected = expected_tree_files(previous_tree, target_tree)
        .into_iter()
        .filter(|file| {
            file.project_relative_path
                .strip_prefix("sursa/")
                .is_some_and(|path| !conflicts.contains(path))
        })
        .collect::<Vec<_>>();
    verify_restored_files(lease, &expected).is_ok()
}

fn integration_conflict_markers_resolved(
    lease: &ActiveProjectReadLease<'_>,
    conflicts: &[String],
) -> Result<bool, String> {
    for path in conflicts {
        let project_path = format!("sursa/{path}");
        let Some(file) = lease.read_bounded_regular_file(
            Path::new(&project_path),
            32 * 1024 * 1024,
            "versioning/integration-conflict-resolution",
        )?
        else {
            continue;
        };
        if contains_standard_conflict_marker(&file.bytes) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn contains_standard_conflict_marker(bytes: &[u8]) -> bool {
    bytes.split(|byte| *byte == b'\n').any(|line| {
        line.starts_with(b"<<<<<<< ") || line == b"=======" || line.starts_with(b">>>>>>> ")
    })
}

enum IntegrationTreePublication {
    Applied {
        changed_paths: Vec<String>,
    },
    RecoveryRequired {
        changed_paths: Vec<String>,
        diagnostic: String,
    },
}

#[allow(clippy::too_many_arguments)]
fn publish_integration_tree(
    app: &AppHandle,
    root: &Path,
    workspace: &mut ProjectWorkspace,
    captured: &CapturedVersioningSession,
    repository: &VersionRepository,
    session_lease: ActiveProjectReadLease<'_>,
    prepared: &PreparedVersionIntegration,
    current_tree: &VersionTree,
    target_tree: &VersionTree,
    allowed_baseline_divergence: &BTreeSet<String>,
    label: String,
    source: &str,
) -> Result<IntegrationTreePublication, String> {
    let mut plan = build_version_restore_plan(workspace, current_tree, target_tree)?;
    for change in &mut plan.binary_changes {
        let live = session_lease.read_bounded_regular_file(
            Path::new(&change.relative_path),
            32 * 1024 * 1024,
            "versioning/integration-binary-baseline",
        )?;
        let live_bytes = live.map(|snapshot| snapshot.bytes);
        let source_relative = change
            .relative_path
            .strip_prefix("sursa/")
            .unwrap_or(&change.relative_path);
        if live_bytes != change.before && !allowed_baseline_divergence.contains(source_relative) {
            return Err(format!(
                "Integrarea a fost blocată: baseline-ul live pentru {} nu corespunde arborelui HEAD Git.",
                change.relative_path
            ));
        }
        change.before = live_bytes;
    }
    let changed_paths = plan.changed_paths.clone();
    let expected_files = plan.expected_files.clone();
    let mut candidate = workspace.clone();
    let workspace_identity = ProjectWorkspaceIdentity {
        expected_project_root: captured.session.project_root.clone(),
        expected_session_id: captured.runtime_session_id.clone(),
        expected_revision: candidate.revision,
    };
    let metadata = WorkspaceMutationMetadata {
        label,
        source: source.to_string(),
        coalesce_key: None,
        transaction_id: Some(prepared.transaction_id.clone()),
    };
    if let Err(error) = candidate.stage_version_tree_restore(
        &workspace_identity,
        metadata,
        plan.text_changes,
        plan.text_deletes,
        plan.binary_changes,
        now_ms(),
    ) {
        let cleanup = repository.delete_integration_marker(prepared);
        return Err(match cleanup {
            Ok(()) => error,
            Err(cleanup_error) => format!(
                "{error} Marker-ul durabil {} nu a putut fi eliminat: {cleanup_error}",
                prepared.recovery_ref
            ),
        });
    }

    drop(session_lease);
    match save_project_workspace_with_recovery(app, root, &mut candidate, &workspace_identity) {
        Ok(_) => {}
        Err(ProjectWorkspaceSaveError::Rejected { diagnostic }) => {
            let cleanup = repository.delete_integration_marker(prepared);
            return Err(match cleanup {
                Ok(()) => diagnostic,
                Err(cleanup_error) => format!(
                    "{diagnostic} Marker-ul durabil {} a fost păstrat deoarece cleanup-ul a eșuat: {cleanup_error}",
                    prepared.recovery_ref
                ),
            });
        }
        Err(ProjectWorkspaceSaveError::RecoveryRequired { diagnostic, .. }) => {
            return Ok(IntegrationTreePublication::RecoveryRequired {
                changed_paths,
                diagnostic: format!(
                    "Save-ul integrării are nevoie de recovery: {diagnostic} Marker-ul Git durabil a fost păstrat. Nu repeta operația automat."
                ),
            });
        }
    }
    *workspace = candidate;

    let authority = app.state::<WriteAuthorityRuntime>();
    let verify_lease = authority.acquire_active_project_read_lease_for_session(
        &captured.root,
        &captured.runtime_session_id,
    )?;
    if let Err(error) = verify_restored_files(&verify_lease, &expected_files) {
        return Ok(IntegrationTreePublication::RecoveryRequired {
            changed_paths,
            diagnostic: format!(
                "Fișierele integrării nu au trecut verificarea byte-cu-byte: {error} Marker-ul Git durabil a fost păstrat."
            ),
        });
    }
    Ok(IntegrationTreePublication::Applied { changed_paths })
}

fn changed_tree_paths<'a>(current: &'a VersionTree, target: &'a VersionTree) -> BTreeSet<&'a str> {
    let current_files = current
        .files
        .iter()
        .map(|file| (file.path.as_str(), (&file.oid, file.executable)))
        .collect::<std::collections::BTreeMap<_, _>>();
    let target_files = target
        .files
        .iter()
        .map(|file| (file.path.as_str(), (&file.oid, file.executable)))
        .collect::<std::collections::BTreeMap<_, _>>();
    current_files
        .keys()
        .chain(target_files.keys())
        .copied()
        .filter(|path| current_files.get(path) != target_files.get(path))
        .collect()
}

fn expected_tree_files(
    current: &VersionTree,
    target: &VersionTree,
) -> Vec<VersionRestoreExpectedFile> {
    let target_paths = target
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<BTreeSet<_>>();
    let mut expected = target
        .files
        .iter()
        .map(|file| VersionRestoreExpectedFile {
            project_relative_path: format!("sursa/{}", file.path),
            expected_bytes: Some(file.bytes.clone()),
        })
        .collect::<Vec<_>>();
    expected.extend(
        current
            .files
            .iter()
            .filter(|file| !target_paths.contains(file.path.as_str()))
            .map(|file| VersionRestoreExpectedFile {
                project_relative_path: format!("sursa/{}", file.path),
                expected_bytes: None,
            }),
    );
    expected.sort_by(|left, right| left.project_relative_path.cmp(&right.project_relative_path));
    expected
}

fn unresolved_recovery_resolution(
    captured: &CapturedVersioningSession,
    marker: &PreparedVersionRestore,
    action: VersionRestoreRecoveryAction,
    diagnostic: String,
    workspace: Option<crate::kernel::project_workspace::ProjectWorkspaceSnapshot>,
) -> VersionRestoreRecoveryResolutionReceipt {
    VersionRestoreRecoveryResolutionReceipt {
        schema_version: VERSIONING_SCHEMA_VERSION,
        project_root: captured.session.project_root.clone(),
        session_id: captured.runtime_session_id.clone(),
        transaction_id: marker.transaction_id.clone(),
        recovery_ref: marker.recovery_ref.clone(),
        action,
        resolved: false,
        diagnostic: Some(diagnostic),
        snapshot: None,
        workspace,
    }
}

fn verify_restored_files(
    lease: &ActiveProjectReadLease<'_>,
    expected_files: &[VersionRestoreExpectedFile],
) -> Result<(), String> {
    for expected in expected_files {
        let expected_size = expected
            .expected_bytes
            .as_ref()
            .map(|bytes| bytes.len() as u64)
            .unwrap_or(0);
        let live = lease.read_bounded_regular_file(
            Path::new(&expected.project_relative_path),
            expected_size.saturating_add(1),
            "versioning/restore-byte-verification",
        )?;
        let live_bytes = live.map(|snapshot| snapshot.bytes);
        if live_bytes != expected.expected_bytes {
            return Err(format!(
                "{} diferă de versiunea țintă.",
                expected.project_relative_path
            ));
        }
    }
    Ok(())
}

fn restore_recovery_receipt(
    captured: &CapturedVersioningSession,
    prepared: &crate::versioning::PreparedVersionRestore,
    changed_paths: Vec<String>,
    diagnostic: String,
    workspace: Option<crate::kernel::project_workspace::ProjectWorkspaceSnapshot>,
) -> VersionRestoreReceipt {
    VersionRestoreReceipt {
        schema_version: VERSIONING_SCHEMA_VERSION,
        status: VersionRestoreStatus::RecoveryRequired,
        project_root: captured.session.project_root.clone(),
        session_id: captured.runtime_session_id.clone(),
        transaction_id: Some(prepared.transaction_id.clone()),
        recovery_ref: Some(prepared.recovery_ref.clone()),
        target_commit_oid: prepared.target_commit_oid.clone(),
        previous_head_oid: Some(prepared.previous_head_oid.clone()),
        restore_commit_oid: Some(prepared.restore_commit_oid.clone()),
        changed_paths,
        diagnostic: Some(diagnostic),
        snapshot: None,
        workspace,
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn record_versioning_event(
    app: &AppHandle,
    level: KernelLogLevel,
    kind: KernelEventKind,
    operation: &str,
    target: Option<String>,
    message: &str,
    diagnostic: Option<String>,
) {
    let event = KernelLogEvent::new(
        level,
        kind,
        "versioning",
        "project_source_write",
        operation,
        target,
        message,
        diagnostic,
    );
    if let Err(error) = append_event(app, event) {
        eprintln!("[Pană Studio] Evenimentul de observabilitate Git nu a fost scris: {error}");
    }
}
