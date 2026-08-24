use std::{
    path::{Path, PathBuf},
    time::Instant,
};
use tauri::{AppHandle, Manager, State};

use super::bootstrap::{
    initial_project_file, prepare_bootstrap_workbench, prepare_bootstrap_workbench_for_file,
    project_file_from_preview_diagnostic, project_source_location_from_preview_diagnostic,
    ProjectBootstrapAssembler,
};
use super::contracts::{
    ProjectBootstrapInitialSurface, ProjectBootstrapSourceLocation, ProjectDiskWatchRequest,
    ProjectOpenBootstrapReceipt,
};
use super::disk_watch::ensure_project_disk_watch;
use super::transition_decisions::{
    capture_project_transition_runtime_lease, project_transition_action_for_open_target,
    project_transition_runtime_lease_from_parts, require_project_transition_for_action,
    ProjectTransitionRuntimeLease,
};

use crate::{
    commands::ai_coordination::publish_ai_coordination_state,
    js::PageJsDraftStore,
    kernel::{
        component_legacy_migration::migrate_legacy_component_catalog,
        file_buffer_store::bootstrap_file_buffer_store,
        observability::{append_event, now_ms, KernelEventKind, KernelLogEvent, KernelLogLevel},
        performance::{elapsed_us, with_performance_sample},
        project_runtime_access::{
            current_project_root, current_project_session, require_current_project_root,
        },
        project_session::{
            persist_project_session_open, prepare_project_session_with_fingerprint,
            record_project_session_opened, ProjectSessionSnapshot,
        },
        project_state::KernelProjectTransitionAction,
        project_workspace::{
            clear_project_open_recovery_decision, clear_project_workspace_recovery,
            inspect_project_workspace_recovery_for_open, persist_project_open_recovery_abandonment,
            require_project_open_recovery_assessment_unchanged, resolve_project_open_recovery,
            restore_project_workspace_recovery, ProjectOpenRecoveryDecisionInput,
            ProjectOpenRecoveryResolution, ProjectWorkspace,
        },
        recovery_coordinator::scan_recovery_coordinator,
        workbench::{read_persisted_workbench, WorkbenchSurface},
        write_authority::WriteAuthorityRuntime,
    },
    preview::{
        stop_project_preview, stop_source_browser, PersistentPreviewOwner,
        PersistentZolaPreviewEngine,
    },
    project::{
        read_project_disk_manifest, scan_project_disk_manifest, scan_project_workspace_projection,
        AcceptedProjectDiskManifest, ProjectLifecycleRuntime,
    },
    project_model::{
        build_project_model_from_workspace_projection,
        template_workbench::{resolve_template_workbench_plan, TemplateWorkbenchPlanInput},
    },
    state::AppState,
};

struct ProjectOpenLifecycleGuard {
    app: AppHandle,
    runtime: ProjectLifecycleRuntime,
    operation_id: String,
    committed: bool,
}

struct PreparedProjectPreview {
    app: AppHandle,
    engine: Option<PersistentZolaPreviewEngine>,
}

impl PreparedProjectPreview {
    fn new(app: AppHandle, engine: PersistentZolaPreviewEngine) -> Self {
        Self {
            app,
            engine: Some(engine),
        }
    }

    fn take(&mut self) -> Result<PersistentZolaPreviewEngine, String> {
        self.engine
            .take()
            .ok_or_else(|| "Preview-ul provizoriu a fost deja consumat.".to_string())
    }
}

impl Drop for PreparedProjectPreview {
    fn drop(&mut self) {
        if let Some(engine) = self.engine.take() {
            let _ = engine.stop(&self.app);
        }
    }
}

impl ProjectOpenLifecycleGuard {
    fn new(app: AppHandle, runtime: ProjectLifecycleRuntime, operation_id: String) -> Self {
        Self {
            app,
            runtime,
            operation_id,
            committed: false,
        }
    }

    fn mark_committed(&mut self) {
        self.committed = true;
    }
}

impl Drop for ProjectOpenLifecycleGuard {
    fn drop(&mut self) {
        if !self.committed {
            match self
                .runtime
                .fail_before_commit(&self.operation_id, "open_project_returned_before_commit")
            {
                Ok(snapshot) => {
                    let _ = append_event(
                        &self.app,
                        KernelLogEvent::new(
                            KernelLogLevel::Error,
                            KernelEventKind::ProjectLifecycleTransition,
                            "project_lifecycle",
                            "project_transition",
                            "precommit_failed",
                            Some(self.operation_id.clone()),
                            "ProjectLifecycle a retras candidatul înainte de commit.",
                            Some(snapshot.reason),
                        )
                        .with_attribute("operationId", &self.operation_id)
                        .with_attribute("transition", snapshot.transition),
                    );
                }
                Err(error) => {
                    let _ = append_event(
                        &self.app,
                        KernelLogEvent::new(
                            KernelLogLevel::Warn,
                            KernelEventKind::ProjectLifecycleTransition,
                            "project_lifecycle",
                            "project_transition",
                            "stale_operation_retired",
                            Some(self.operation_id.clone()),
                            "Candidatul provizoriu a fost retras după invalidarea operationId.",
                            Some(error),
                        )
                        .with_attribute("operationId", &self.operation_id)
                        .with_attribute("stale", true),
                    );
                }
            }
        }
    }
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
            Ok(Some((
                root.clone(),
                runtime_session_id,
                accepted.as_ref().clone(),
            )))
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
            || workspace.accepted_disk.as_ref() != &accepted_disk
        {
            return Err(
                "ProjectWorkspace s-a schimbat înainte de proiecția reatașării.".to_string(),
            );
        }
        workspace.capture_projection_snapshot()?
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
    app: AppHandle,
    state: State<AppState>,
) -> Result<Option<ProjectOpenBootstrapReceipt>, String> {
    let Some(scan) = reattach_project_session_impl(state.inner())? else {
        return Ok(None);
    };
    let session = current_project_session(&state)?
        .ok_or_else(|| "Sesiunea reatașată a dispărut înainte de bootstrap.".to_string())?;
    let lifecycle = state.project_lifecycle.attach_existing_session(&session)?;
    let workbench = state
        .workbench
        .read_or_restore(&session, || read_persisted_workbench(&session))?;
    let (projection, project_model, bootstrap) = {
        let workspace = state
            .project_workspace
            .lock()
            .map_err(|_| "ProjectWorkspace este indisponibil la reatașare.".to_string())?;
        let workspace = workspace
            .as_ref()
            .ok_or_else(|| "ProjectWorkspace lipsește la reatașare.".to_string())?;
        let projection = workspace.capture_projection_snapshot()?;
        let project_model = (workspace.project_model_source_revision == Some(projection.revision))
            .then(|| workspace.project_model.clone())
            .flatten();
        let bootstrap = ProjectBootstrapAssembler::prepare(
            scan,
            workspace,
            &workbench,
            project_model.as_deref(),
            None,
        )?;
        (projection, project_model, bootstrap)
    };
    let initial_surface = match (
        initial_project_file(&bootstrap.project, &workbench)
            .filter(|file| file.role == crate::project::ProjectFileRole::Template),
        project_model.as_ref(),
    ) {
        (Some(file), Some(model)) => {
            let plan = resolve_template_workbench_plan(
                model,
                &TemplateWorkbenchPlanInput {
                    template_path: file.relative_path.clone(),
                    preferred_page_path: None,
                    preferred_route: None,
                },
            )?;
            let _preview_operation = state.preview_workspace_operation.lock().map_err(|_| {
                "Nu am putut serializa suprafața inițială la reatașare.".to_string()
            })?;
            let mut preview_slot = state.preview_engine.lock().map_err(|_| {
                "Nu am putut bloca Preview-ul pentru suprafața inițială.".to_string()
            })?;
            match preview_slot.as_mut() {
                Some(engine)
                    if engine
                        .generation_for_workspace_revision(projection.revision)?
                        .is_some() =>
                {
                    let publication =
                        engine.publish_template_workbench_view(&projection, model, &plan)?;
                    Some(ProjectBootstrapInitialSurface {
                        document_path: file.relative_path.clone(),
                        route: publication.route,
                        preview_url: publication.preview_url,
                        reuse_token: publication.reuse_token,
                        plan,
                        canvas_projection: publication.canvas_plan,
                    })
                }
                _ => None,
            }
        }
        _ => None,
    };
    let receipt = bootstrap.finish(lifecycle, workbench, initial_surface);
    crate::synchronize_main_window_title(&app, Some(Path::new(&session.project_root)));
    Ok(Some(receipt))
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

fn clear_project_runtime_state(
    app: &AppHandle,
    state: &State<AppState>,
    expected_lease: Option<&ProjectTransitionRuntimeLease>,
) -> Result<(), String> {
    state
        .versioning_network_operation
        .require_project_transition_allowed()?;
    state
        .ai_coordination
        .require_project_transition()
        .map_err(|error| error.to_string())?;
    let _disk_watch_transition = state
        .project_disk_watch_transition
        .lock()
        .map_err(|_| "Serializarea watcher-ului este compromisă la închidere.".to_string())?;

    // Capture the exact session under the canonical lock order, then release
    // every runtime-state mutex before the best-effort Workbench disk flush.
    let workbench_session = {
        let current_root = state
            .current_root
            .lock()
            .map_err(|_| "Nu am putut bloca root-ul proiectului curent.".to_string())?;
        let project_workspace = state
            .project_workspace
            .lock()
            .map_err(|_| "Nu am putut bloca ProjectWorkspace.".to_string())?;
        let recovery_coordinator_scan = state
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
        project_workspace
            .as_ref()
            .map(|workspace| workspace.session.clone())
    };

    if let Some(session) = workbench_session.as_ref() {
        let flush_result = state.workbench.read(session).and_then(|snapshot| {
            state
                .workbench_projection_persistence
                .flush_latest(app, session, &snapshot)
        });
        if let Err(error) = flush_result {
            // Workbench is an internal UI projection. A failed best-effort
            // flush must not falsely block a validated project transition.
            eprintln!("[Pană Studio] Workbench close flush failed: {error}");
        }
    }

    // Revalidate after I/O. Only the short atomic publication window holds the
    // root/workspace/recovery locks; watcher join happens after their release.
    let disk_watcher = {
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
                    "Project Transition close lease a devenit stale după flush; runtime-ul curent nu a fost șters."
                        .to_string(),
                );
            }
        }

        let disk_watcher = state
            .project_disk_watch
            .lock()
            .map_err(|_| "Slot-ul watcher-ului este compromis la închidere.".to_string())?
            .take();
        let authority_runtime = app.try_state::<WriteAuthorityRuntime>().ok_or_else(|| {
            "WriteAuthorityRuntime lipsește la închiderea proiectului.".to_string()
        })?;
        let mut authority_publication = authority_runtime.project_publication()?;
        authority_publication.revoke();
        *current_root = None;
        *project_workspace = None;
        *recovery_coordinator_scan = None;
        state.clear_publish_authorization()?;
        state
            .ai_coordination
            .bind_project(None, crate::kernel::observability::now_ms())
            .map_err(|error| error.to_string())?;
        disk_watcher
    };
    if let Some(disk_watcher) = disk_watcher {
        disk_watcher.stop();
    }
    let _ = publish_ai_coordination_state(app);
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

#[tauri::command]
pub fn close_project(
    operator_decision_id: Option<String>,
    app: AppHandle,
    state: State<AppState>,
) -> Result<(), String> {
    let Some(root) = current_project_root(&state) else {
        let transition_lease = capture_project_transition_runtime_lease(&state)?;
        clear_project_runtime_state(&app, &state, Some(&transition_lease))?;
        state.selection_coordinator.revoke_all();
        stop_source_browser(&app, state.inner());
        stop_project_preview(&app, state.inner());
        state.startup_flow.reset()?;
        crate::synchronize_main_window_title(&app, None);
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
    state.selection_coordinator.revoke_all();
    stop_source_browser(&app, state.inner());
    stop_project_preview(&app, state.inner());

    if let Some(session) = session {
        record_project_session_closed(&app, &session);
    }
    state.startup_flow.reset()?;
    state.project_lifecycle.clear_active("project_closed")?;
    crate::synchronize_main_window_title(&app, None);

    Ok(())
}

#[tauri::command]
pub fn open_project(
    path: String,
    operation_id: String,
    candidate_token: String,
    operator_decision_id: Option<String>,
    recovery_decision: Option<ProjectOpenRecoveryDecisionInput>,
    app: AppHandle,
    state: State<AppState>,
) -> Result<ProjectOpenBootstrapReceipt, String> {
    let lifecycle_started = Instant::now();
    app.state::<WriteAuthorityRuntime>()
        .require_recovery_clean()?;
    let root = PathBuf::from(path)
        .canonicalize()
        .map_err(|error| format!("Nu am putut rezolva folderul: {}", error))?;
    let action = project_transition_action_for_open_target(&state, &root)?;
    let reset_session_history = action == KernelProjectTransitionAction::ReloadProject;
    let recovery_decision_token = recovery_decision
        .as_ref()
        .map(|decision| decision.assessment_token.as_str());
    let inspection = state.project_lifecycle.begin_preparing(
        &operation_id,
        &candidate_token,
        !reset_session_history,
        recovery_decision_token,
    )?;
    if inspection.operation_id != operation_id {
        return Err("Contextul inspecției nu corespunde operației solicitate.".to_string());
    }
    let _ = append_event(
        &app,
        KernelLogEvent::new(
            KernelLogLevel::Info,
            KernelEventKind::ProjectLifecycleTransition,
            "project_lifecycle",
            "project_transition",
            "prepare",
            Some(operation_id.clone()),
            "ProjectLifecycle a început pregătirea provizorie.",
            None,
        )
        .with_attribute("operationId", &operation_id)
        .with_attribute("projectRoot", &inspection.candidate.root)
        .with_attribute(
            "folderSelectedToPrepareMs",
            now_ms()
                .saturating_sub(inspection.operation_started_at_ms)
                .min(u64::MAX as u128) as u64,
        ),
    );
    let mut lifecycle_guard = ProjectOpenLifecycleGuard::new(
        app.clone(),
        state.project_lifecycle.clone(),
        operation_id.clone(),
    );
    if root != Path::new(&inspection.candidate.root) {
        return Err(
            "ProjectLifecycle a refuzat deschiderea altui root decât cel inspectat.".to_string(),
        );
    }
    require_project_transition_for_action(
        &app,
        &state,
        &root,
        action,
        operator_decision_id.as_deref(),
    )?;
    let transition_runtime_lease = capture_project_transition_runtime_lease(&state)?;
    let bootstrap_manifest = inspection.manifest.clone();

    let mut scan = scan_project_disk_manifest(&root, &inspection.manifest)?;

    let session = prepare_project_session_with_fingerprint(
        &app,
        &root,
        &scan,
        inspection.root_fingerprint.clone(),
    )?;
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
    scan.kernel_session_id = Some(session.runtime_instance_id());
    scan.accepted_disk_manifest = Some(bootstrap_manifest.clone());
    let next_accepted_disk_manifest = AcceptedProjectDiskManifest::new(
        session.runtime_instance_id(),
        session.project_root.clone(),
        bootstrap_manifest.clone(),
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
        Some(inspection.recovery.clone())
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
    migrate_legacy_component_catalog(&root, &mut next_project_workspace, now_ms())?;
    let retire_abandoned_recovery = recovery_assessment.as_ref().is_some_and(|assessment| {
        matches!(
            assessment.status,
            crate::kernel::project_workspace::ProjectOpenRecoveryStatus::Abandoned
                | crate::kernel::project_workspace::ProjectOpenRecoveryStatus::DecisionRequired
        ) && recovery_resolution != ProjectOpenRecoveryResolution::Restore
    });
    let bootstrap_projection = next_project_workspace.capture_projection_snapshot()?;
    let mut authoritative_scan = scan_project_workspace_projection(&bootstrap_projection)?;
    let mut workbench = prepare_bootstrap_workbench(&session, &authoritative_scan)?;
    let preview_owner =
        PersistentPreviewOwner::new(session.project_root.clone(), session.runtime_instance_id());
    let preview_prepare_started = Instant::now();
    let mut preview_engine =
        PersistentZolaPreviewEngine::start(&app, Path::new(&session.zola_root), preview_owner)?;
    let preview_base_url = preview_engine.url()?;
    authoritative_scan.preview_base_url = Some(preview_base_url.clone());
    scan.preview_base_url = Some(preview_base_url);
    let mut bootstrap_diagnostic_target: Option<(String, ProjectBootstrapSourceLocation)> = None;
    let preview_candidate = match preview_engine.render_candidate_with_pending_project_authority(
        &app,
        &bootstrap_projection,
        &pending_project_authority,
    ) {
        Ok(candidate) => Some(candidate),
        Err(diagnostic) => {
            // A broken Zola/Tera/SCSS render disables only Preview. The source
            // workspace remains valid and must be committed so the user can
            // repair it inside Pană Studio instead of being trapped on Startup.
            authoritative_scan.preview_warning = Some(diagnostic.clone());
            scan.preview_warning = Some(diagnostic.clone());
            if let Some(file) =
                project_file_from_preview_diagnostic(&authoritative_scan, &diagnostic)
            {
                if let Some(location) = project_source_location_from_preview_diagnostic(
                    &diagnostic,
                    &file.relative_path,
                ) {
                    bootstrap_diagnostic_target = Some((file.relative_path.clone(), location));
                }
                workbench = prepare_bootstrap_workbench_for_file(
                    &session,
                    &authoritative_scan,
                    Some(file),
                    Some(WorkbenchSurface::Code),
                )?;
            }
            let _ = append_event(
                &app,
                KernelLogEvent::new(
                    KernelLogLevel::Warn,
                    KernelEventKind::ProjectLifecycleTransition,
                    "project_lifecycle",
                    "project_open_preview",
                    "degraded",
                    Some(operation_id.clone()),
                    "Proiectul va fi deschis pentru reparare, fără generația Preview inițială.",
                    Some(diagnostic),
                )
                .with_attribute("operationId", &operation_id)
                .with_attribute("projectRoot", &session.project_root),
            );
            None
        }
    };
    let preview_timings = preview_candidate
        .as_ref()
        .map(|candidate| candidate.timings)
        .unwrap_or_default();
    let preview_plan = preview_candidate
        .as_ref()
        .map(|candidate| candidate.canvas_plan());
    let preview_prepare_ms = preview_prepare_started
        .elapsed()
        .as_millis()
        .min(u64::MAX as u128) as u64;
    let preview_model = match preview_candidate.as_ref() {
        Some(candidate) => candidate.project_model_arc(),
        None => std::sync::Arc::new(build_project_model_from_workspace_projection(
            &root,
            &bootstrap_projection,
        )?),
    };
    next_project_workspace.publish_project_model(&bootstrap_projection, preview_model.clone())?;
    let bootstrap = ProjectBootstrapAssembler::prepare(
        authoritative_scan,
        &next_project_workspace,
        &workbench,
        Some(&preview_model),
        bootstrap_diagnostic_target.as_ref(),
    )?;
    let preview_generation_available = preview_candidate.is_some();
    if let Some(candidate) = preview_candidate {
        preview_engine.stage_candidate(&app, candidate)?;
    }
    let initial_surface = if preview_generation_available {
        initial_project_file(&bootstrap.project, &workbench)
            .filter(|file| file.role == crate::project::ProjectFileRole::Template)
            .map(|file| {
                let plan = resolve_template_workbench_plan(
                    &preview_model,
                    &TemplateWorkbenchPlanInput {
                        template_path: file.relative_path.clone(),
                        preferred_page_path: None,
                        preferred_route: None,
                    },
                )?;
                let publication = preview_engine.publish_template_workbench_view(
                    &bootstrap_projection,
                    &preview_model,
                    &plan,
                )?;
                Ok::<_, String>(ProjectBootstrapInitialSurface {
                    document_path: file.relative_path.clone(),
                    route: publication.route,
                    preview_url: publication.preview_url,
                    reuse_token: publication.reuse_token,
                    plan,
                    canvas_projection: publication.canvas_plan,
                })
            })
            .transpose()?
    } else {
        None
    };
    let mut prepared_preview = PreparedProjectPreview::new(app.clone(), preview_engine);
    require_project_transition_for_action(
        &app,
        &state,
        &root,
        action,
        operator_decision_id.as_deref(),
    )?;
    let lifecycle_commit_transition = state
        .project_lifecycle_transition
        .lock()
        .map_err(|_| "Serializarea commit-ului ProjectLifecycle este compromisă.".to_string())?;
    state.project_lifecycle.begin_commit(&operation_id)?;
    let opened_session_for_event = session.clone();

    // Disk/recovery inspection and the provisional session write can be slow.
    // They run before the runtime-state mutexes; the compact transition lease
    // is revalidated under the final commit locks immediately afterwards.
    let manifest_at_commit = read_project_disk_manifest(&root)?;
    if scan.accepted_disk_manifest.as_ref() != Some(&manifest_at_commit) {
        return Err(
            "Manifestul proiectului s-a schimbat la punctul de commit; runtime-ul vechi a rămas intact."
                .to_string(),
        );
    }
    let commit_recovery_assessment = if let Some(initial_assessment) = recovery_assessment.as_ref()
    {
        let commit_assessment = inspect_project_workspace_recovery_for_open(
            &app,
            &root,
            &manifest_at_commit,
            &session.root_fingerprint,
        )?;
        require_project_open_recovery_assessment_unchanged(initial_assessment, &commit_assessment)?;
        let commit_resolution =
            resolve_project_open_recovery(&commit_assessment, recovery_decision.as_ref())?;
        if commit_resolution != recovery_resolution {
            return Err(
                "Rezoluția project-open recovery s-a schimbat înainte de commit.".to_string(),
            );
        }
        Some(commit_assessment)
    } else {
        None
    };
    persist_project_session_open(&app, &session)?;

    let current_root_lock_wait_us;
    let current_root_lock_held_us;
    let project_workspace_lock_wait_us;
    let project_workspace_lock_held_us;
    let previous_preview = {
        let _preview_operation = state
            .preview_workspace_operation
            .lock()
            .map_err(|_| "Nu am putut serializa commit-ul Preview inițial.".to_string())?;
        let mut preview_slot = state
            .preview_engine
            .lock()
            .map_err(|_| "Nu am putut bloca motorul Preview la commit.".to_string())?;
        let current_root_lock_wait_started = Instant::now();
        let mut current_root = state
            .current_root
            .lock()
            .map_err(|_| "Nu am putut bloca starea proiectului.".to_string())?;
        current_root_lock_wait_us = elapsed_us(current_root_lock_wait_started);
        let current_root_lock_held_started = Instant::now();
        let project_workspace_lock_wait_started = Instant::now();
        let mut project_workspace = state
            .project_workspace
            .lock()
            .map_err(|_| "Nu am putut bloca ProjectWorkspace.".to_string())?;
        project_workspace_lock_wait_us = elapsed_us(project_workspace_lock_wait_started);
        let project_workspace_lock_held_started = Instant::now();
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
        if recovery_resolution == ProjectOpenRecoveryResolution::ExplicitAbandon {
            if let Some(commit_assessment) = commit_recovery_assessment.as_ref() {
                let decision = recovery_decision.as_ref().ok_or_else(|| {
                    "Lipsește decizia explicită de abandonare la commit.".to_string()
                })?;
                persist_project_open_recovery_abandonment(&app, commit_assessment, decision)?;
            }
        }
        if reset_session_history {
            clear_project_workspace_recovery(&app, &session.project_root)?;
            clear_project_open_recovery_decision(&app, &session.project_root)?;
        }
        pending_project_authority.verify_path_binding()?;
        let mut authority_publication = authority_runtime.project_publication()?;
        authority_publication.publish(pending_project_authority)?;
        state
            .workbench
            .publish_prepared(&session, workbench.clone())?;
        *current_root = Some(root.clone());
        *project_workspace = Some(next_project_workspace);
        *recovery_scan = Some(recovery_coordinator_scan);
        state.clear_publish_authorization()?;
        state
            .ai_coordination
            .bind_project(
                Some(session.runtime_instance_id()),
                crate::kernel::observability::now_ms(),
            )
            .map_err(|error| error.to_string())?;
        let previous_preview = preview_slot.replace(prepared_preview.take()?);
        project_workspace_lock_held_us = elapsed_us(project_workspace_lock_held_started);
        current_root_lock_held_us = elapsed_us(current_root_lock_held_started);
        previous_preview
    };
    let _ = publish_ai_coordination_state(&app);
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
    state.selection_coordinator.revoke_all();
    record_project_session_opened(&app, &opened_session_for_event);
    stop_source_browser(&app, state.inner());
    if let Some(previous_preview) = previous_preview {
        if let Err(error) = previous_preview.stop(&app) {
            eprintln!("[Pană Studio] Preview-ul sesiunii vechi nu s-a retras curat: {error}");
        }
    }
    let lifecycle = state
        .project_lifecycle
        .commit_session(&operation_id, &opened_session_for_event)?;
    lifecycle_guard.mark_committed();
    crate::synchronize_main_window_title(
        &app,
        Some(Path::new(&opened_session_for_event.project_root)),
    );
    drop(lifecycle_commit_transition);
    let mut commit_event = with_performance_sample(
        KernelLogEvent::new(
            KernelLogLevel::Info,
            KernelEventKind::ProjectLifecycleTransition,
            "project_lifecycle",
            "project_transition",
            "commit",
            Some(operation_id.clone()),
            "ProjectLifecycle a publicat atomic noua sesiune.",
            None,
        ),
        "project_open",
        "commit",
        elapsed_us(lifecycle_started),
    )
    .with_attribute("operationId", &operation_id)
    .with_attribute("sessionId", opened_session_for_event.runtime_instance_id())
    .with_attribute("projectRoot", &opened_session_for_event.project_root)
    .with_attribute(
        "folderSelectedToCommitMs",
        now_ms()
            .saturating_sub(inspection.operation_started_at_ms)
            .min(u64::MAX as u128) as u64,
    )
    .with_attribute(
        "prepareToCommitMs",
        lifecycle_started
            .elapsed()
            .as_millis()
            .min(u64::MAX as u128) as u64,
    )
    .with_attribute("previewPrepareMs", preview_prepare_ms)
    .with_attribute("zolaValidationBuildCount", 1_u64)
    .with_attribute("zolaRenderMs", preview_timings.zola_render_ms)
    .with_attribute(
        "projectModelBuildMs",
        preview_timings.project_model_build_ms,
    )
    .with_attribute("currentRootLockWaitUs", current_root_lock_wait_us)
    .with_attribute("currentRootLockHeldUs", current_root_lock_held_us)
    .with_attribute("projectWorkspaceLockWaitUs", project_workspace_lock_wait_us)
    .with_attribute("projectWorkspaceLockHeldUs", project_workspace_lock_held_us)
    .with_attribute("previewPrepareUs", preview_prepare_ms.saturating_mul(1_000))
    .with_attribute(
        "zolaRenderUs",
        preview_timings.zola_render_ms.saturating_mul(1_000),
    )
    .with_attribute(
        "projectModelBuildUs",
        preview_timings.project_model_build_ms.saturating_mul(1_000),
    )
    .with_attribute("previewGenerationAvailable", preview_plan.is_some());
    if let Some(preview_plan) = preview_plan.as_ref() {
        commit_event = commit_event
            .with_attribute(
                "workspaceRevision",
                preview_plan.identity.workspace_revision,
            )
            .with_attribute("previewRevision", &preview_plan.identity.preview_revision)
            .with_attribute("canvasTransactionId", &preview_plan.identity.transaction_id);
    } else {
        commit_event =
            commit_event.with_attribute("workspaceRevision", bootstrap_projection.revision);
    }
    let _ = append_event(&app, commit_event);
    let watch_input = ProjectDiskWatchRequest {
        expected_project_root: opened_session_for_event.project_root.clone(),
        expected_session_id: opened_session_for_event.runtime_instance_id(),
    };
    if let Ok(_transition) = state.project_disk_watch_transition.lock() {
        if let Err(error) = ensure_project_disk_watch(&app, state.inner(), &watch_input) {
            eprintln!(
                "[Pană Studio] watcher-ul disk nu a pornit la commit-ul ProjectLifecycle: {error}"
            );
        }
    }

    Ok(bootstrap.finish(lifecycle, workbench, initial_surface))
}

#[tauri::command]
pub fn read_project_file(relative_path: String, state: State<AppState>) -> Result<String, String> {
    require_current_project_root(&state)?;
    let project_workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace.".to_string())?;
    let workspace = project_workspace
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    if let Some(snapshot) = workspace.projected_text_snapshot(&relative_path)? {
        return Ok(snapshot.text);
    }
    Err(format!(
        "ProjectWorkspace nu urmărește documentul text proiectat {relative_path}; citirea paralelă direct de pe disc este interzisă."
    ))
}

#[cfg(test)]
mod tests {
    use crate::{
        kernel::{
            file_buffer_store::{FileBufferStore, FileBufferStoreLimits},
            project_session::{
                ProjectRootFingerprint, ProjectSessionScanSummary, ProjectSessionSnapshot,
            },
        },
        project::ProjectDiskManifest,
    };

    use super::*;

    #[test]
    fn reattach_is_absent_only_when_runtime_state_is_completely_empty() {
        let state = AppState::default();
        assert!(capture_project_session_attachment(&state)
            .unwrap()
            .is_none());

        *state.current_root.lock().unwrap() = Some(PathBuf::from("/tmp/partial-project"));
        let error = capture_project_session_attachment(&state).unwrap_err();
        assert!(error.contains("publicată parțial"));
    }

    #[test]
    fn reattach_projects_the_active_workspace_without_reopening_or_mutating_it() {
        let root = PathBuf::from("/tmp/pana-project-reattach-behavior");
        let session = session(&root);
        let runtime_session_id = session.runtime_instance_id();
        let accepted = AcceptedProjectDiskManifest::new(
            runtime_session_id.clone(),
            session.project_root.clone(),
            ProjectDiskManifest {
                root: session.project_root.clone(),
                files: Vec::new(),
                truncated: false,
                max_files: 1_000,
            },
        )
        .unwrap();
        let documents = FileBufferStore::for_project_session(
            &session,
            1,
            FileBufferStoreLimits {
                max_files: 32,
                max_file_bytes: 1024 * 1024,
                max_total_bytes: 4 * 1024 * 1024,
            },
        );
        let workspace = ProjectWorkspace::new(
            session.clone(),
            accepted,
            documents,
            PageJsDraftStore::new(&session),
        )
        .unwrap();
        let revision = workspace.revision;
        let state = AppState::default();
        *state.current_root.lock().unwrap() = Some(root.clone());
        *state.project_workspace.lock().unwrap() = Some(workspace);

        let scan = reattach_project_session_impl(&state)
            .unwrap()
            .expect("active session projection");
        assert_eq!(scan.root, root.to_string_lossy());
        assert_eq!(
            scan.kernel_session_id.as_deref(),
            Some(runtime_session_id.as_str())
        );
        let workspace = state.project_workspace.lock().unwrap();
        let workspace = workspace.as_ref().unwrap();
        assert_eq!(workspace.revision, revision);
        assert_eq!(workspace.runtime_session_id(), runtime_session_id);
    }

    fn session(root: &Path) -> ProjectSessionSnapshot {
        let root = root.to_string_lossy().to_string();
        ProjectSessionSnapshot {
            schema_version: 2,
            id: "reattach-stable".to_string(),
            project_root: root.clone(),
            zola_root: root.clone(),
            session_dir: "/tmp/pana-project-reattach-session".to_string(),
            manifest_path: "/tmp/pana-project-reattach-session/manifest.json".to_string(),
            opened_at_ms: 17,
            last_seen_at_ms: 17,
            root_fingerprint: ProjectRootFingerprint {
                canonical_path: root,
                modified_ms: 1,
                size: 0,
                readonly: false,
                unix_device: None,
                unix_inode: None,
            },
            scan_summary: ProjectSessionScanSummary {
                active_theme: None,
                file_count: 0,
                directory_count: 0,
            },
        }
    }
}
