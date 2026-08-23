use super::{observability::record_versioning_event, *};

pub(super) type VersionNetworkProgressCallback = Arc<dyn Fn(&[u8]) + Send + Sync + 'static>;

#[derive(Clone)]
pub(super) struct CapturedVersioningSession {
    pub(super) root: PathBuf,
    pub(super) repository_root: PathBuf,
    pub(super) session: ProjectSessionSnapshot,
    pub(super) runtime_session_id: String,
}

impl CapturedVersioningSession {
    pub(super) fn with_repository<R: tauri::Runtime, T>(
        &self,
        app: &AppHandle<R>,
        operation: impl FnOnce(&VersionRepository) -> Result<T, String>,
    ) -> Result<T, String> {
        let runtime = app
            .try_state::<WriteAuthorityRuntime>()
            .ok_or_else(|| "WriteAuthorityRuntime lipsește pentru Git.".to_string())?;
        let lease = runtime
            .acquire_active_project_read_lease_for_session(&self.root, &self.runtime_session_id)?;
        let directory =
            lease.capture_subprocess_directory(Path::new(""), "versioning/git-repository-cwd")?;
        let repository = VersionRepository::new(
            self.session.project_root.clone(),
            self.repository_root.clone(),
            directory.current_dir_path(),
        );
        operation(&repository)
    }

    pub(super) fn spawn_prepared_network<R: tauri::Runtime>(
        &self,
        app: &AppHandle<R>,
        prepared: &PreparedVersionNetworkOperation,
        cancellation: Arc<AtomicBool>,
        progress: VersionNetworkProgressCallback,
    ) -> Result<crate::versioning::RunningGitCommand, String> {
        let runtime = app
            .try_state::<WriteAuthorityRuntime>()
            .ok_or_else(|| "WriteAuthorityRuntime lipsește pentru Git remote.".to_string())?;
        let authority_lease = runtime
            .acquire_active_project_read_lease_for_session(&self.root, &self.runtime_session_id)?;
        let directory = authority_lease
            .capture_subprocess_directory(Path::new(""), "versioning/git-network-cwd")?;
        let repository = VersionRepository::new(
            self.session.project_root.clone(),
            self.repository_root.clone(),
            directory.current_dir_path(),
        );
        let running = repository.spawn_prepared_network(prepared, cancellation, progress)?;
        drop(repository);
        drop(directory);
        drop(authority_lease);
        Ok(running)
    }
}

pub(super) fn capture_read_session(
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

pub(super) fn capture_from_workspace(
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
    let expected_repository_root = root.to_path_buf();
    let session_repository_root = PathBuf::from(&session.zola_root);
    if session_repository_root != expected_repository_root {
        return Err(format!(
            "Versiuni cere ca rădăcina Git și rădăcina Zola să fie dosarul selectat: sesiunea indică {}, iar rădăcina cerută este {}.",
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

pub(super) fn with_mutation_preflight<T>(
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
    // Local Git mutations remain one bounded transaction under the
    // ProjectWorkspace guard. Remote operations use the separate
    // capture/execute/revalidate path and never enter this helper.
    captured.with_repository(app, operation)
}

pub(super) async fn read_with_repository<T: Send + 'static>(
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

pub(super) fn acquire_git_mutation_gate<'a>(
    state: &'a AppState,
    operation: &str,
) -> Result<std::sync::MutexGuard<'a, ()>, String> {
    state
        .versioning_network_operation
        .require_git_mutation_allowed(operation)?;
    let guard = match state.versioning_operation.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            state
                .versioning_network_operation
                .require_git_mutation_allowed(operation)?;
            return Err(format!(
                "{operation} este blocată rapid deoarece o altă operație Git este activă."
            ));
        }
        Err(std::sync::TryLockError::Poisoned(_)) => {
            return Err("Mutex-ul operațiilor Git este compromis.".to_string());
        }
    };
    state
        .versioning_network_operation
        .require_git_mutation_allowed(operation)?;
    Ok(guard)
}

pub(super) async fn mutate_with_repository<T: Send + 'static>(
    app: AppHandle,
    identity: VersioningMutationIdentity,
    operation_name: &'static str,
    operation: impl FnOnce(&VersionRepository) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let log_app = app.clone();
    let log_project_root = identity.expected_project_root.clone();
    let result: Result<T, String> = tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let _operation = acquire_git_mutation_gate(&state, operation_name)?;
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
