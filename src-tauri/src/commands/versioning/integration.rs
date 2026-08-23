use super::*;
use super::{observability::record_versioning_event, publication::*, session::*};

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
            let _operation = acquire_git_mutation_gate(&state, operation_name)?;
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
                Path::new(""),
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
            Path::new(""),
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
            let _operation = acquire_git_mutation_gate(&state, "Recovery integrare Git")?;
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
                Path::new(""),
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
        .filter(|file| !conflicts.contains(file.project_relative_path.as_str()))
        .collect::<Vec<_>>();
    verify_restored_files(lease, &expected).is_ok()
}

fn integration_conflict_markers_resolved(
    lease: &ActiveProjectReadLease<'_>,
    conflicts: &[String],
) -> Result<bool, String> {
    for path in conflicts {
        let Some(file) = lease.read_bounded_regular_file(
            Path::new(path),
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
