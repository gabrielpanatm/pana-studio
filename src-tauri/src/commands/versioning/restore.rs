use super::*;
use super::{
    observability::{now_ms, record_versioning_event},
    publication::*,
    session::*,
};

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
        let _operation = acquire_git_mutation_gate(&state, "Restaurarea Git")?;
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
            Path::new(""),
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
        let mut candidate = workspace.fork_candidate();
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
        workspace.adopt_candidate(candidate);
        emit_project_workspace_mutated(
            &app,
            workspace,
            ProjectWorkspacePreviewProjection::Required,
        );

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
            Path::new(""),
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
        let _operation = acquire_git_mutation_gate(&state, "Recovery restaurare Git")?;
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
            Path::new(""),
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
                let mut candidate = workspace.fork_candidate();
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
                workspace.adopt_candidate(candidate);
                emit_project_workspace_mutated(
                    &app,
                    workspace,
                    ProjectWorkspacePreviewProjection::Required,
                );
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
