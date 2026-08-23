use super::*;
use super::{observability::record_versioning_event, session::*};

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
            let _operation = acquire_git_mutation_gate(&state, "Inițializarea Git")?;
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
            "Repository-ul Git local a fost inițializat direct în rădăcina Zola.",
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
