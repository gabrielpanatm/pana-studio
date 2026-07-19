use std::{collections::BTreeSet, path::Path};

use tauri::{AppHandle, Manager, Runtime, State};

use crate::{
    commands::kernel::current_kernel_project_state_snapshot,
    kernel::{
        file_buffer_store::{
            commit_clean_external_reconcile, now_ms as file_buffer_now_ms,
            plan_clean_external_reconcile, read_clean_external_reconcile_plan,
            CleanExternalReconcilePlan, CleanExternalReconcilePlanResult,
            CleanExternalReconcileReadResult, KernelExternalDiskReconcileInput,
            KernelExternalDiskReconcileReceipt, KernelExternalDiskReconcileStatus,
        },
        observability::{append_event, KernelEventKind, KernelLogEvent, KernelLogLevel},
        project_path::normalize_project_relative_path,
        recovery_coordinator::{RecoveryCoordinatorScan, RecoveryCoordinatorStatus},
    },
    preview::schedule_source_browser_refresh,
    project::{
        project_disk_manifest_changed_paths, read_project_disk_manifest,
        AcceptedProjectDiskManifest,
    },
    state::AppState,
};

#[derive(Clone, Debug)]
struct ExternalReconcileRuntimeVersion {
    accepted_disk_manifest_fingerprint: String,
    workspace_revision: u64,
    recovery_scanned_at_ms: u128,
    recovery_status: RecoveryCoordinatorStatus,
    recovery_fingerprint: String,
}

#[tauri::command]
pub async fn reconcile_clean_external_project_files(
    input: KernelExternalDiskReconcileInput,
    app: AppHandle,
) -> Result<KernelExternalDiskReconcileReceipt, String> {
    tauri::async_runtime::spawn_blocking(move || {
        reconcile_clean_external_project_files_impl(&app, input)
    })
    .await
    .map_err(|error| format!("External disk reconcile task eșuat: {error}"))?
}

fn reconcile_clean_external_project_files_impl<R: Runtime>(
    app: &AppHandle<R>,
    input: KernelExternalDiskReconcileInput,
) -> Result<KernelExternalDiskReconcileReceipt, String> {
    let state = app.state::<AppState>();
    let started_at_ms = file_buffer_now_ms();
    let project_state = current_kernel_project_state_snapshot(&state)?;
    let session = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru external reconcile.".to_string())?
        .as_ref()
        .map(|workspace| workspace.session.clone())
        .ok_or_else(|| {
            "ProjectSession nu este inițializat pentru external reconcile.".to_string()
        })?;
    let session_instance_id = session.runtime_instance_id();
    if input.expected_session_id != session_instance_id
        || input.expected_project_root != session.project_root
    {
        let receipt = KernelExternalDiskReconcileReceipt::stale_by_session_guard(
            session_instance_id,
            session.project_root,
            started_at_ms,
            &input,
            "External reconcile a refuzat o cerere dintr-o altă instanță ProjectSession.",
        );
        log_external_reconcile_receipt(app, &receipt);
        return Ok(receipt);
    }

    if let Err(error) = state.ai_coordination.require_external_reconciliation() {
        let receipt = KernelExternalDiskReconcileReceipt::blocked_by_runtime_guard(
            session.runtime_instance_id(),
            session.project_root,
            started_at_ms,
            &input,
            "ai_edit_authority_busy",
            error.to_string(),
        );
        log_external_reconcile_receipt(app, &receipt);
        return Ok(receipt);
    }

    if let Some((code, message)) = external_reconcile_project_state_blocker(&project_state) {
        let receipt = KernelExternalDiskReconcileReceipt::blocked_by_runtime_guard(
            session.runtime_instance_id(),
            session.project_root,
            started_at_ms,
            &input,
            code,
            message,
        );
        log_external_reconcile_receipt(app, &receipt);
        return Ok(receipt);
    }

    if let Some((code, message)) = external_reconcile_runtime_blocker(&state)? {
        let receipt = KernelExternalDiskReconcileReceipt::blocked_by_runtime_guard(
            session.runtime_instance_id(),
            session.project_root,
            started_at_ms,
            &input,
            code,
            message,
        );
        log_external_reconcile_receipt(app, &receipt);
        return Ok(receipt);
    }
    let runtime_version = match capture_external_reconcile_runtime_version(&state, &session, &input)
    {
        Ok(version) => version,
        Err(message) => {
            let receipt = KernelExternalDiskReconcileReceipt::blocked_by_runtime_guard(
                session.runtime_instance_id(),
                session.project_root,
                started_at_ms,
                &input,
                "runtime_version_not_clean",
                message,
            );
            log_external_reconcile_receipt(app, &receipt);
            return Ok(receipt);
        }
    };

    let plan = {
        let workspace = state
            .project_workspace
            .lock()
            .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru plan reconcile.".to_string())?;
        let store = workspace
            .as_ref()
            .map(|workspace| &workspace.documents)
            .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru reconcile.".to_string())?;
        plan_clean_external_reconcile(store, input, started_at_ms)
    };
    let plan = match plan {
        CleanExternalReconcilePlanResult::Ready(plan) => plan,
        CleanExternalReconcilePlanResult::Terminal(receipt) => {
            log_external_reconcile_receipt(app, &receipt);
            return Ok(receipt);
        }
    };
    log_external_reconcile_planned(app, &plan);

    let staged = match read_clean_external_reconcile_plan(plan, file_buffer_now_ms()) {
        CleanExternalReconcileReadResult::Staged(staged) => staged,
        CleanExternalReconcileReadResult::Terminal(receipt) => {
            log_external_reconcile_receipt(app, &receipt);
            return Ok(receipt);
        }
    };
    let (mut receipt, accepted_disk_changed_paths) = {
        let mut live_workspace_guard = state
            .project_workspace
            .lock()
            .map_err(|_| "Nu am putut bloca ProjectWorkspace la commit reconcile.".to_string())?;
        let Some(live_workspace) = live_workspace_guard.as_mut() else {
            let receipt = KernelExternalDiskReconcileReceipt::stale_manifest(
                staged.plan(),
                "ProjectWorkspace a fost închis înainte de commit.",
                file_buffer_now_ms(),
            );
            log_external_reconcile_receipt(app, &receipt);
            return Ok(receipt);
        };
        if let Err(error) = state.ai_coordination.require_external_reconciliation() {
            let receipt = KernelExternalDiskReconcileReceipt::stale_manifest(
                staged.plan(),
                format!(
                    "Autoritatea de editare s-a schimbat înainte de commit-ul external reconcile: {error}"
                ),
                file_buffer_now_ms(),
            );
            log_external_reconcile_receipt(app, &receipt);
            return Ok(receipt);
        }
        let live_session = live_workspace.session.clone();
        if live_session.runtime_instance_id() != staged.plan().session_id()
            || live_session.project_root != staged.plan().project_root()
        {
            let receipt = KernelExternalDiskReconcileReceipt::stale_manifest(
                staged.plan(),
                "ProjectSession s-a schimbat înainte de commit.",
                file_buffer_now_ms(),
            );
            log_external_reconcile_receipt(app, &receipt);
            return Ok(receipt);
        }

        let live_accepted_disk_manifest = &live_workspace.accepted_disk;
        live_accepted_disk_manifest.require_identity(
            &live_session.runtime_instance_id(),
            &live_session.project_root,
        )?;
        live_accepted_disk_manifest.require_complete()?;

        let live_store = &live_workspace.documents;
        let recovery_guard = state.recovery_coordinator_scan.lock().map_err(|_| {
            "Nu am putut bloca RecoveryCoordinatorScan la commit reconcile.".to_string()
        })?;
        let live_recovery = recovery_guard
            .as_ref()
            .ok_or_else(|| "RecoveryCoordinatorScan a dispărut înainte de commit.".to_string())?;
        let runtime_is_stale = live_workspace.revision != runtime_version.workspace_revision
            || accepted_disk_manifest_fingerprint(live_accepted_disk_manifest)?
                != runtime_version.accepted_disk_manifest_fingerprint
            || live_workspace.is_dirty()
            || live_recovery.scanned_at_ms != runtime_version.recovery_scanned_at_ms
            || live_recovery.status != runtime_version.recovery_status
            || recovery_coordinator_fingerprint(live_recovery)?
                != runtime_version.recovery_fingerprint
            || live_recovery.status != RecoveryCoordinatorStatus::Clean;
        if runtime_is_stale {
            let receipt = KernelExternalDiskReconcileReceipt::stale_manifest(
                staged.plan(),
                "O autoritate runtime s-a schimbat între plan și commit; batch-ul a fost refuzat.",
                file_buffer_now_ms(),
            );
            log_external_reconcile_receipt(app, &receipt);
            return Ok(receipt);
        }

        // This is the commit-point disk evidence. It is deliberately read
        // only after every Rust authority participating in the CAS is held,
        // so an in-process mutation cannot slip between manifest validation
        // and the FileBufferStore commit.
        let manifest_at_commit =
            read_project_disk_manifest(Path::new(staged.plan().project_root()))
                .map_err(|error| format!("Manifestul de commit nu poate fi verificat: {error}"))?;
        if &manifest_at_commit != staged.plan().observed_manifest() {
            let receipt = KernelExternalDiskReconcileReceipt::stale_manifest(
                staged.plan(),
                "Manifestul proiectului s-a schimbat înaintea punctului de commit; batch-ul nu a fost comis.",
                file_buffer_now_ms(),
            );
            log_external_reconcile_receipt(app, &receipt);
            return Ok(receipt);
        }
        let accepted_disk_changed_paths = project_disk_manifest_changed_paths(
            &live_accepted_disk_manifest.manifest,
            &manifest_at_commit,
        )?;
        let accepted_disk_changed = !accepted_disk_changed_paths.is_empty();
        let next_accepted_disk_manifest = if accepted_disk_changed {
            live_accepted_disk_manifest.next(
                &live_session.runtime_instance_id(),
                &live_session.project_root,
                manifest_at_commit,
            )?
        } else {
            live_accepted_disk_manifest.clone()
        };

        let invalidates_history = staged.invalidates_history();
        let mut next_store = live_store.clone();
        let mut receipt =
            commit_clean_external_reconcile(&mut next_store, staged, file_buffer_now_ms());
        if !matches!(
            receipt.status,
            KernelExternalDiskReconcileStatus::Applied | KernelExternalDiskReconcileStatus::Noop
        ) {
            log_external_reconcile_receipt(app, &receipt);
            return Ok(receipt);
        }

        receipt.accepted_disk_generation = Some(next_accepted_disk_manifest.generation);
        live_workspace.accept_reconciled_disk_state(
            next_accepted_disk_manifest,
            next_store,
            invalidates_history,
        )?;
        receipt.workspace_revision = Some(live_workspace.revision);
        receipt.mark_committed_runtime_effects(invalidates_history, false);
        (receipt, accepted_disk_changed_paths)
    };

    let accepted_disk_changed = !accepted_disk_changed_paths.is_empty();
    if accepted_disk_changed || receipt.status == KernelExternalDiskReconcileStatus::Applied {
        receipt.source_graph_invalidated = true;
    }
    if accepted_disk_changed {
        let accepted_disk_generation = receipt.accepted_disk_generation.ok_or_else(|| {
            "External reconcile a avansat discul fără generație AcceptedDisk.".to_string()
        })?;
        schedule_source_browser_refresh(
            app,
            crate::preview::BrowserPreviewRequestIdentity {
                expected_project_root: session.project_root.clone(),
                expected_session_id: session.runtime_instance_id(),
                expected_disk_generation: accepted_disk_generation,
            },
        );
        receipt.projection_hints.project_rescan = true;
        receipt.projection_hints.source_graph = true;
        receipt.projection_hints.preview |= accepted_disk_changed_paths
            .iter()
            .any(|path| accepted_disk_path_affects_preview(path));
        receipt.projection_hints.page_js |= accepted_disk_changed_paths
            .iter()
            .any(|path| path.ends_with(".js"));
        receipt.projection_hints.scss |= accepted_disk_changed_paths
            .iter()
            .any(|path| path.ends_with(".scss") || path.ends_with(".css"));
    }

    log_external_reconcile_receipt(app, &receipt);
    Ok(receipt)
}

fn accepted_disk_path_affects_preview(path: &str) -> bool {
    path.starts_with("sursa/content/")
        || path.starts_with("sursa/templates/")
        || path.starts_with("sursa/themes/")
        || path.starts_with("sursa/sass/")
        || path.starts_with("sursa/static/")
        || matches!(path, "sursa/zola.toml" | "sursa/config.toml")
}

fn external_reconcile_project_state_blocker(
    project_state: &crate::kernel::project_state::KernelProjectStateSnapshot,
) -> Option<(&'static str, String)> {
    if !project_state.project_workspace_available {
        return Some((
            "project_workspace_unavailable",
            "ProjectWorkspace nu este disponibil; reconcilierea externă nu poate fi aplicată sigur."
                .to_string(),
        ));
    }
    if project_state.workspace_dirty {
        return Some((
            "project_workspace_dirty",
            format!(
                "ProjectWorkspace are {} resurse nesalvate; schimbarea externă cere rezolvare explicită.",
                project_state.workspace_dirty_resource_count
            ),
        ));
    }
    None
}

fn external_reconcile_runtime_blocker(
    state: &State<AppState>,
) -> Result<Option<(&'static str, String)>, String> {
    let workspace_guard = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut verifica ProjectWorkspace pentru reconcile.".to_string())?;
    let Some(workspace) = workspace_guard.as_ref() else {
        return Ok(Some((
            "project_workspace_unavailable",
            "ProjectWorkspace nu este inițializat.".to_string(),
        )));
    };
    if workspace.is_dirty() {
        let snapshot = workspace.snapshot();
        let dirty_resource_count = snapshot.dirty_document_count
            + snapshot.created_document_count
            + snapshot.deleted_document_count
            + snapshot.dirty_page_js_count;
        return Ok(Some((
            "project_workspace_dirty",
            format!(
                "ProjectWorkspace are {} resurse nesalvate; auto-reconcile este blocat.",
                dirty_resource_count
            ),
        )));
    }
    drop(workspace_guard);

    let recovery_guard = state
        .recovery_coordinator_scan
        .lock()
        .map_err(|_| "Nu am putut verifica RecoveryCoordinatorScan.".to_string())?;
    let Some(recovery) = recovery_guard.as_ref() else {
        return Ok(Some((
            "transaction_recovery_unavailable",
            "RecoveryCoordinatorScan nu este inițializat.".to_string(),
        )));
    };
    if recovery.status != RecoveryCoordinatorStatus::Clean {
        return Ok(Some((
            "transaction_recovery_not_clean",
            format!(
                "RecoveryCoordinatorScan este {}.",
                recovery_coordinator_status_label(recovery.status)
            ),
        )));
    }
    Ok(None)
}

fn capture_external_reconcile_runtime_version(
    state: &State<AppState>,
    session: &crate::kernel::project_session::ProjectSessionSnapshot,
    input: &KernelExternalDiskReconcileInput,
) -> Result<ExternalReconcileRuntimeVersion, String> {
    let workspace_guard = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut captura ProjectWorkspace version.".to_string())?;
    let workspace = workspace_guard
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    let accepted_disk_manifest = &workspace.accepted_disk;
    accepted_disk_manifest
        .require_identity(&session.runtime_instance_id(), &session.project_root)?;
    accepted_disk_manifest.require_complete()?;
    require_exact_reconcile_manifest_delta(accepted_disk_manifest, input)?;
    let accepted_disk_manifest_fingerprint =
        accepted_disk_manifest_fingerprint(accepted_disk_manifest)?;

    let workspace_revision = workspace.revision;
    drop(workspace_guard);
    let recovery = state
        .recovery_coordinator_scan
        .lock()
        .map_err(|_| "Nu am putut captura RecoveryCoordinatorScan revision.".to_string())?
        .clone()
        .ok_or_else(|| "RecoveryCoordinatorScan nu este inițializat.".to_string())?;
    let recovery_fingerprint = recovery_coordinator_fingerprint(&recovery)?;
    Ok(ExternalReconcileRuntimeVersion {
        accepted_disk_manifest_fingerprint,
        workspace_revision,
        recovery_scanned_at_ms: recovery.scanned_at_ms,
        recovery_status: recovery.status,
        recovery_fingerprint,
    })
}

fn accepted_disk_manifest_fingerprint(
    accepted: &AcceptedProjectDiskManifest,
) -> Result<String, String> {
    serde_json::to_string(accepted).map_err(|error| {
        format!("AcceptedProjectDiskManifest fingerprint nu poate fi serializat: {error}")
    })
}

fn require_exact_reconcile_manifest_delta(
    accepted: &AcceptedProjectDiskManifest,
    input: &KernelExternalDiskReconcileInput,
) -> Result<(), String> {
    let changed_paths =
        project_disk_manifest_changed_paths(&accepted.manifest, &input.observed_manifest)?
            .into_iter()
            .collect::<BTreeSet<_>>();
    let requested_paths = input
        .relative_paths
        .iter()
        .map(|path| normalize_project_relative_path(path))
        .collect::<Result<BTreeSet<_>, _>>()?;
    // A caller may acknowledge an idempotent write whose final metadata is
    // byte-for-byte identical to the accepted baseline. Once a real delta
    // exists, however, its scope must be exact: no omitted or extra paths.
    if !changed_paths.is_empty() && changed_paths != requested_paths {
        let missing = changed_paths
            .difference(&requested_paths)
            .cloned()
            .collect::<Vec<_>>();
        let unexpected = requested_paths
            .difference(&changed_paths)
            .cloned()
            .collect::<Vec<_>>();
        return Err(format!(
            "Reconcile manifest delta nu corespunde batch-ului exact: lipsesc {:?}, sunt revendicate în plus {:?}.",
            missing, unexpected
        ));
    }
    Ok(())
}

fn recovery_coordinator_fingerprint(scan: &RecoveryCoordinatorScan) -> Result<String, String> {
    serde_json::to_string(scan).map_err(|error| {
        format!("RecoveryCoordinatorScan fingerprint nu poate fi serializat: {error}")
    })
}

fn recovery_coordinator_status_label(status: RecoveryCoordinatorStatus) -> &'static str {
    match status {
        RecoveryCoordinatorStatus::Clean => "clean",
        RecoveryCoordinatorStatus::NeedsAttention => "needs_attention",
        RecoveryCoordinatorStatus::Unreadable => "unreadable",
    }
}

fn log_external_reconcile_planned<R: Runtime>(
    app: &AppHandle<R>,
    plan: &CleanExternalReconcilePlan,
) {
    let event = KernelLogEvent::new(
        KernelLogLevel::Info,
        KernelEventKind::ExternalDiskReconcilePlanned,
        "file_buffer_store",
        "external_disk",
        "reconcile_clean_external_project_files",
        Some(format!("session/{}", plan.session_id())),
        "External disk reconcile a trecut preflight-ul și citește bounded din disk.",
        None,
    )
    .with_attribute("projectRoot", plan.project_root())
    .with_attribute("manifestFileCount", plan.observed_manifest().files.len());
    if let Err(error) = append_event(app, event) {
        eprintln!("[Pană Studio] external reconcile planned log failed: {error}");
    }
}

fn log_external_reconcile_receipt<R: Runtime>(
    app: &AppHandle<R>,
    receipt: &KernelExternalDiskReconcileReceipt,
) {
    let (level, kind) = match receipt.status {
        KernelExternalDiskReconcileStatus::Applied | KernelExternalDiskReconcileStatus::Noop => (
            KernelLogLevel::Info,
            KernelEventKind::ExternalDiskReconcileApplied,
        ),
        KernelExternalDiskReconcileStatus::Blocked
        | KernelExternalDiskReconcileStatus::ReloadRequired
        | KernelExternalDiskReconcileStatus::StaleEvidence => (
            KernelLogLevel::Warn,
            KernelEventKind::ExternalDiskReconcileBlocked,
        ),
    };
    let event = KernelLogEvent::new(
        level,
        kind,
        "file_buffer_store",
        "external_disk",
        "reconcile_clean_external_project_files",
        Some(receipt.operation_id.clone()),
        receipt.verdict_reason.clone(),
        None,
    )
    .with_attribute("status", receipt.status)
    .with_attribute("sessionId", receipt.session_id.clone())
    .with_attribute("projectRoot", receipt.project_root.clone())
    .with_attribute("requestedCount", receipt.requested_count)
    .with_attribute("reconciledCount", receipt.reconciled_count)
    .with_attribute("historyInvalidated", receipt.history_invalidated);
    if let Err(error) = append_event(app, event) {
        eprintln!("[Pană Studio] external reconcile receipt log failed: {error}");
    }
}
