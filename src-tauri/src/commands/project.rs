use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::Instant,
};
use tauri::{AppHandle, Manager, Runtime, State};

use crate::{
    commands::{
        ai_coordination::publish_ai_coordination_state,
        config::{read_project_app_config_for_bootstrap, ProjectAppConfig},
        kernel::current_kernel_project_state_snapshot,
    },
    js::PageJsDraftStore,
    kernel::{
        ai_coordination::EditAuthority,
        component_legacy_migration::migrate_legacy_component_catalog,
        disk_conflict::scan_disk_conflicts,
        file_buffer_store::{
            bootstrap_file_buffer_store, now_ms as file_buffer_now_ms,
            require_file_buffer_session_binding, FileBufferChangeSetInput,
            FileBufferChangeSetResult, FileBufferCommandReceipt, FileBufferFileSnapshot,
            FileBufferMutationExpectation, FileBufferRequestIdentity, FileBufferStore,
            FileBufferStoreSnapshot, FileBufferTextSnapshot,
        },
        observability::{append_event, now_ms, KernelEventKind, KernelLogEvent, KernelLogLevel},
        preview_projection::{CanvasPatch, CanvasPatchAnchor, CanvasPatchOperation},
        project_session::{
            persist_project_session_open, prepare_project_session_with_fingerprint,
            record_project_session_opened, ProjectSessionSnapshot,
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
            KernelProjectTransitionDecisionRetentionRecoveryReceipt,
        },
        project_workspace::{
            clear_project_open_recovery_decision, clear_project_workspace_recovery,
            commit_project_workspace_session_mutation, emit_project_workspace_mutated,
            inspect_project_workspace_recovery_for_open, persist_project_open_recovery_abandonment,
            persist_project_workspace_recovery,
            recover_project_workspace_save_hot_journal as apply_project_workspace_save_recovery,
            require_project_open_recovery_assessment_unchanged, resolve_project_open_recovery,
            restore_project_workspace_recovery, ProjectOpenRecoveryDecisionInput,
            ProjectOpenRecoveryResolution, ProjectWorkspace, ProjectWorkspaceHistoryIdentity,
            ProjectWorkspaceIdentity, ProjectWorkspaceSaveError, ProjectWorkspaceSaveReceipt,
            ProjectWorkspaceSaveRecoveryAction, ProjectWorkspaceSaveRecoveryReceipt,
            ProjectWorkspaceSnapshot, WorkspaceCanvasHistoryDelta, WorkspaceDocumentMutation,
            WorkspaceHistoryDirection, WorkspaceHistorySnapshot, WorkspaceMutationMetadata,
            WorkspaceUndoRedoReceipt,
        },
        recovery_coordinator::{
            scan_recovery_coordinator, RecoveryCoordinatorScan, RecoveryCoordinatorStatus,
        },
        workbench::{
            persist_workbench, read_persisted_workbench, WorkbenchActivity,
            WorkbenchBottomPanelView, WorkbenchCommandReceipt, WorkbenchGroupId, WorkbenchIdentity,
            WorkbenchIntent, WorkbenchProjectEntryRemap, WorkbenchRuntime, WorkbenchSnapshot,
            WorkbenchSplit, WorkbenchSurface,
        },
        write_authority::WriteAuthorityRuntime,
    },
    preview::{
        schedule_source_browser_refresh, stop_project_preview, stop_source_browser,
        BrowserPreviewRequestIdentity, CanvasProjectionPlan, PersistentPreviewOwner,
        PersistentZolaPreviewEngine,
    },
    project::{
        apply_project_model_preview_routes, read_project_disk_manifest, scan_project_disk_manifest,
        scan_project_root, scan_project_workspace_projection, AcceptedProjectDiskManifest,
        ProjectDiskWatchHandle, ProjectFile, ProjectFileKind, ProjectLifecycleRuntime, ProjectScan,
        PROJECT_OPEN_BOOTSTRAP_SCHEMA_VERSION,
    },
    project_model::{
        build_project_model_from_workspace_projection,
        model::ProjectModel,
        move_engine::html_identity_aliases,
        rebuild_project_model_after_workspace_change,
        template_workbench::{
            resolve_template_workbench_plan, TemplateWorkbenchPlan, TemplateWorkbenchPlanInput,
        },
        ProjectModelIncrementalIntent,
    },
    source_graph::model::SourceNodeKind,
    state::AppState,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBootstrapDocument {
    pub relative_path: String,
    pub source: String,
    pub preview_path: Option<String>,
    pub diagnostic_location: Option<ProjectBootstrapSourceLocation>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBootstrapSourceLocation {
    pub line: u32,
    pub column: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectOpenBootstrapReceipt {
    pub schema_version: u32,
    pub project: ProjectScan,
    pub lifecycle: crate::project::ProjectLifecycleSnapshot,
    pub file_buffers: FileBufferStoreSnapshot,
    pub workspace: ProjectWorkspaceSnapshot,
    pub project_config: ProjectAppConfig,
    pub workbench: WorkbenchSnapshot,
    pub active_document: Option<ProjectBootstrapDocument>,
    pub target_css_file: Option<String>,
    pub initial_surface: Option<ProjectBootstrapInitialSurface>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBootstrapInitialSurface {
    pub document_path: String,
    pub route: String,
    pub preview_url: String,
    pub plan: TemplateWorkbenchPlan,
    pub canvas_projection: CanvasProjectionPlan,
}

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

fn active_workbench_relative_path(snapshot: &WorkbenchSnapshot) -> Option<String> {
    let group = snapshot
        .groups
        .iter()
        .find(|group| group.group_id == snapshot.active_group_id)?;
    let active_id = group.active_document_id.as_deref()?;
    group
        .documents
        .iter()
        .find(|document| document.document_id == active_id)
        .map(|document| document.relative_path.clone())
}

fn initial_project_file<'a>(
    scan: &'a ProjectScan,
    workbench: &WorkbenchSnapshot,
) -> Option<&'a ProjectFile> {
    active_workbench_relative_path(workbench)
        .and_then(|path| scan.files.iter().find(|file| file.relative_path == path))
        .or_else(|| project_index_file(scan))
}

fn project_index_file(scan: &ProjectScan) -> Option<&ProjectFile> {
    scan.files
        .iter()
        .find(|file| file.relative_path == "templates/index.html")
        .or_else(|| {
            let active_theme = scan.active_theme.as_deref()?;
            let themed_index = format!("themes/{active_theme}/templates/index.html");
            scan.files
                .iter()
                .find(|file| file.relative_path == themed_index)
        })
        .or_else(|| {
            scan.files.iter().find(|file| {
                file.role == crate::project::ProjectFileRole::Page
                    && file.preview_path.as_deref() == Some("/")
            })
        })
        .or_else(|| {
            scan.files
                .iter()
                .find(|file| file.role == crate::project::ProjectFileRole::Page)
        })
        .or_else(|| {
            scan.files
                .iter()
                .find(|file| !matches!(file.kind, ProjectFileKind::Dir | ProjectFileKind::Image))
        })
}

fn workbench_surface_for_file(file: &ProjectFile) -> WorkbenchSurface {
    match file.kind {
        ProjectFileKind::Md => WorkbenchSurface::Code,
        ProjectFileKind::Html if file.role == crate::project::ProjectFileRole::Page => {
            WorkbenchSurface::Visual
        }
        _ => WorkbenchSurface::Code,
    }
}

fn prepare_bootstrap_workbench(
    session: &ProjectSessionSnapshot,
    scan: &ProjectScan,
) -> Result<WorkbenchSnapshot, String> {
    let file = project_index_file(scan);
    prepare_bootstrap_workbench_for_file(session, file, None)
}

fn prepare_bootstrap_workbench_for_file(
    session: &ProjectSessionSnapshot,
    file: Option<&ProjectFile>,
    surface_override: Option<WorkbenchSurface>,
) -> Result<WorkbenchSnapshot, String> {
    let runtime = WorkbenchRuntime::default();
    let mut snapshot = runtime.read_or_restore(session, || read_persisted_workbench(session))?;
    let Some(file) = file else {
        return Ok(snapshot);
    };
    for intent in [
        WorkbenchIntent::SetSplit {
            split: WorkbenchSplit::None,
        },
        WorkbenchIntent::SetActivity {
            activity: WorkbenchActivity::Editor,
        },
        WorkbenchIntent::SetBottomPanel {
            open: false,
            active_view: WorkbenchBottomPanelView::Problems,
        },
        WorkbenchIntent::OpenDocument {
            relative_path: file.relative_path.clone(),
            group_id: WorkbenchGroupId::Primary,
            surface: surface_override.unwrap_or_else(|| {
                if file.role == crate::project::ProjectFileRole::Template {
                    WorkbenchSurface::Visual
                } else {
                    workbench_surface_for_file(file)
                }
            }),
            pinned: false,
        },
    ] {
        let identity = WorkbenchIdentity {
            expected_project_root: snapshot.project_root.clone(),
            expected_runtime_session_id: snapshot.runtime_session_id.clone(),
            expected_revision: snapshot.revision,
        };
        snapshot = runtime.apply(session, &identity, intent)?.snapshot;
    }
    Ok(snapshot)
}

fn project_file_from_preview_diagnostic<'a>(
    scan: &'a ProjectScan,
    diagnostic: &str,
) -> Option<&'a ProjectFile> {
    // Zola reports the private projected path (for example
    // `.../source/sass/pagini/index.scss:1170:23`), never the original root.
    // Match against the authoritative workspace namespace so the bootstrap
    // remains independent from the private cache location and its session id.
    scan.files
        .iter()
        .filter(|file| !matches!(file.kind, ProjectFileKind::Dir | ProjectFileKind::Image))
        .filter(|file| diagnostic.contains(&file.relative_path))
        .max_by_key(|file| file.relative_path.len())
}

fn project_source_location_from_preview_diagnostic(
    diagnostic: &str,
    relative_path: &str,
) -> Option<ProjectBootstrapSourceLocation> {
    let path_end = diagnostic.rfind(relative_path)? + relative_path.len();
    let location = diagnostic.get(path_end..)?.strip_prefix(':')?;
    let (line, remainder) = parse_diagnostic_coordinate(location)?;
    let column = remainder
        .strip_prefix(':')
        .and_then(parse_diagnostic_coordinate)
        .map(|(column, _)| column)
        .unwrap_or(1);
    Some(ProjectBootstrapSourceLocation { line, column })
}

fn parse_diagnostic_coordinate(value: &str) -> Option<(u32, &str)> {
    let digits = value
        .char_indices()
        .take_while(|(_, character)| character.is_ascii_digit())
        .map(|(index, character)| index + character.len_utf8())
        .last()?;
    let coordinate = value.get(..digits)?.parse::<u32>().ok()?;
    (coordinate > 0).then(|| (coordinate, &value[digits..]))
}

#[cfg(test)]
mod bootstrap_preview_diagnostic_tests {
    use super::*;

    #[test]
    fn resolves_the_workspace_file_from_a_private_zola_projection_path() {
        let scan = ProjectScan {
            root: "/project".to_string(),
            preview_base_url: None,
            preview_warning: None,
            active_theme: None,
            files: vec![
                ProjectFile {
                    name: "index.scss".to_string(),
                    relative_path: "sass/pagini/index.scss".to_string(),
                    absolute_path: "/project/sass/pagini/index.scss".to_string(),
                    kind: ProjectFileKind::Scss,
                    role: crate::project::ProjectFileRole::Style,
                    preview_path: None,
                },
                ProjectFile {
                    name: "index.html".to_string(),
                    relative_path: "templates/index.html".to_string(),
                    absolute_path: "/project/templates/index.html".to_string(),
                    kind: ProjectFileKind::Html,
                    role: crate::project::ProjectFileRole::Template,
                    preview_path: None,
                },
            ],
            kernel_session_id: None,
            workspace_revision: None,
            accepted_disk_manifest: None,
            accepted_disk_generation: None,
        };
        let diagnostic = concat!(
            "Zola nu a putut randa: Expected expression. | 1170 | ",
            "//cache/preview/session/source/sass/pagini/index.scss:1170:23",
        );

        let file = project_file_from_preview_diagnostic(&scan, diagnostic)
            .expect("diagnostic source file");
        assert_eq!(file.relative_path, "sass/pagini/index.scss");
        assert_eq!(
            project_source_location_from_preview_diagnostic(diagnostic, &file.relative_path)
                .map(|location| (location.line, location.column)),
            Some((1170, 23)),
        );
    }
}

pub fn current_project_root(state: &State<AppState>) -> Option<PathBuf> {
    state.current_root.lock().ok()?.clone()
}

pub fn require_current_project_root(state: &State<AppState>) -> Result<PathBuf, String> {
    current_project_root(state).ok_or_else(|| "Nu există proiect deschis.".to_string())
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

fn require_current_project_session(
    state: &State<AppState>,
) -> Result<ProjectSessionSnapshot, String> {
    current_project_session(state)?
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())
}

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

#[tauri::command]
pub fn read_project_session(
    state: State<AppState>,
) -> Result<Option<ProjectSessionSnapshot>, String> {
    current_project_session(&state)
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
            Ok(Some((root.clone(), runtime_session_id, accepted.clone())))
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
            || workspace.accepted_disk != accepted_disk
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
    let scan = reattach_project_session_impl(state.inner())?;
    let Some(mut scan) = scan else {
        return Ok(None);
    };
    let session = current_project_session(&state)?
        .ok_or_else(|| "Sesiunea reatașată a dispărut înainte de bootstrap.".to_string())?;
    let lifecycle = state.project_lifecycle.attach_existing_session(&session)?;
    let workbench = state
        .workbench
        .read_or_restore(&session, || read_persisted_workbench(&session))?;
    let project_config =
        read_project_app_config_for_bootstrap(&app, Path::new(&session.project_root))?;
    let (file_buffers, workspace_snapshot, mut active_document, projection, project_model) = {
        let workspace = state
            .project_workspace
            .lock()
            .map_err(|_| "ProjectWorkspace este indisponibil la reatașare.".to_string())?;
        let workspace = workspace
            .as_ref()
            .ok_or_else(|| "ProjectWorkspace lipsește la reatașare.".to_string())?;
        let active_document = initial_project_file(&scan, &workbench).and_then(|file| {
            workspace
                .documents
                .text_for(&file.relative_path)
                .map(|source| ProjectBootstrapDocument {
                    relative_path: file.relative_path.clone(),
                    source,
                    preview_path: file.preview_path.clone(),
                    diagnostic_location: None,
                })
        });
        let projection = workspace.capture_projection_snapshot()?;
        let project_model = (workspace.project_model_source_revision == Some(projection.revision))
            .then(|| workspace.project_model.clone())
            .flatten();
        (
            workspace.documents.snapshot(),
            workspace.snapshot(),
            active_document,
            projection,
            project_model,
        )
    };
    if let Some(model) = project_model.as_ref() {
        apply_project_model_preview_routes(
            &mut scan,
            model
                .source_graph
                .pages
                .iter()
                .map(|page| (page.file.as_str(), page.url.as_str())),
        );
        if let Some(document) = active_document.as_mut() {
            document.preview_path = scan
                .files
                .iter()
                .find(|file| file.relative_path == document.relative_path)
                .and_then(|file| file.preview_path.clone());
        }
    }
    let target_css_file = scan
        .files
        .iter()
        .find(|file| {
            matches!(file.kind, ProjectFileKind::Css | ProjectFileKind::Scss)
                && file.role == crate::project::ProjectFileRole::Style
        })
        .map(|file| file.relative_path.clone());
    let initial_surface = match (
        initial_project_file(&scan, &workbench)
            .filter(|file| file.role == crate::project::ProjectFileRole::Template),
        project_model,
    ) {
        (Some(file), Some(model)) => {
            let plan = resolve_template_workbench_plan(
                &model,
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
                        engine.publish_template_workbench_view(&projection, &model, &plan)?;
                    Some(ProjectBootstrapInitialSurface {
                        document_path: file.relative_path.clone(),
                        route: publication.route,
                        preview_url: publication.preview_url,
                        plan,
                        canvas_projection: publication.canvas_plan,
                    })
                }
                _ => None,
            }
        }
        _ => None,
    };
    Ok(Some(ProjectOpenBootstrapReceipt {
        schema_version: PROJECT_OPEN_BOOTSTRAP_SCHEMA_VERSION,
        project: scan,
        lifecycle,
        file_buffers,
        workspace: workspace_snapshot,
        project_config,
        workbench,
        active_document,
        target_css_file,
        initial_surface,
    }))
}

#[tauri::command]
pub fn read_file_buffer_store(
    state: State<AppState>,
) -> Result<Option<FileBufferStoreSnapshot>, String> {
    state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace.".to_string())
        .map(|workspace| {
            workspace
                .as_ref()
                .map(|workspace| workspace.documents.snapshot())
        })
}

#[tauri::command]
pub fn read_recovery_coordinator_scan(
    state: State<AppState>,
) -> Result<Option<RecoveryCoordinatorScan>, String> {
    state
        .recovery_coordinator_scan
        .lock()
        .map_err(|_| "Nu am putut bloca RecoveryCoordinatorScan.".to_string())
        .map(|scan| scan.clone())
}

#[tauri::command]
pub fn read_project_workspace_state(
    state: State<AppState>,
) -> Result<Option<ProjectWorkspaceSnapshot>, String> {
    state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace.".to_string())
        .map(|workspace| workspace.as_ref().map(ProjectWorkspace::snapshot))
}

#[tauri::command]
pub fn read_project_workspace_history(
    state: State<AppState>,
) -> Result<Option<WorkspaceHistorySnapshot>, String> {
    read_project_workspace_state(state).map(|workspace| workspace.map(|item| item.history))
}

#[tauri::command]
pub fn save_project_workspace(
    identity: ProjectWorkspaceIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<ProjectWorkspaceSaveReceipt, ProjectWorkspaceSaveError> {
    let current_root = state.current_root.lock().map_err(|_| {
        ProjectWorkspaceSaveError::rejected(
            "Nu am putut bloca root-ul proiectului pentru Save ProjectWorkspace.",
        )
    })?;
    let root = current_root.as_ref().ok_or_else(|| {
        ProjectWorkspaceSaveError::rejected("Save ProjectWorkspace cere un proiect deschis.")
    })?;
    let mut slot = state.project_workspace.lock().map_err(|_| {
        ProjectWorkspaceSaveError::rejected("Nu am putut bloca ProjectWorkspace pentru Save.")
    })?;
    let workspace = slot.as_mut().ok_or_else(|| {
        ProjectWorkspaceSaveError::rejected("ProjectWorkspace nu este inițializat pentru Save.")
    })?;
    let receipt =
        crate::kernel::project_workspace::save_project_workspace(&app, root, workspace, &identity)?;
    if receipt.disk_generation_after != receipt.disk_generation_before {
        schedule_source_browser_refresh(
            &app,
            BrowserPreviewRequestIdentity {
                expected_project_root: workspace.session.project_root.clone(),
                expected_session_id: workspace.runtime_session_id(),
                expected_disk_generation: receipt.disk_generation_after,
            },
        );
    }
    persist_project_workspace_recovery(&app, workspace).map_err(|diagnostic| {
        ProjectWorkspaceSaveError::recovery_required(
            receipt
                .transaction_id
                .clone()
                .unwrap_or_else(|| format!("workspace-save-recovery-{}", workspace.revision)),
            receipt
                .written_files
                .iter()
                .chain(&receipt.removed_files)
                .cloned()
                .collect(),
            receipt.write_receipts.clone(),
            format!(
                "Save-ul proiectului a fost acceptat, dar snapshotul de recuperare ProjectWorkspace nu a putut fi persistat: {diagnostic}"
            ),
        )
    })?;
    emit_project_workspace_mutated(
        &app,
        workspace,
        crate::kernel::project_workspace::ProjectWorkspacePreviewProjection::Required,
    );
    Ok(receipt)
}

pub const PROJECT_WORKSPACE_UNDO_REDO_COMMAND_SCHEMA_VERSION: u32 = 4;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkspaceUndoRedoCommandReceipt {
    pub schema_version: u32,
    pub project_root: String,
    pub runtime_session_id: String,
    pub result: WorkspaceUndoRedoReceipt,
    pub workspace: ProjectWorkspaceSnapshot,
    pub workbench: Option<WorkbenchCommandReceipt>,
    pub canvas_patch: Option<CanvasPatch>,
}

#[tauri::command]
pub fn undo_project_workspace(
    identity: ProjectWorkspaceHistoryIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<ProjectWorkspaceUndoRedoCommandReceipt, String> {
    apply_project_workspace_history(app, identity, state, WorkspaceHistoryDirection::Undo)
}

#[tauri::command]
pub fn redo_project_workspace(
    identity: ProjectWorkspaceHistoryIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<ProjectWorkspaceUndoRedoCommandReceipt, String> {
    apply_project_workspace_history(app, identity, state, WorkspaceHistoryDirection::Redo)
}

fn apply_project_workspace_history(
    app: AppHandle,
    identity: ProjectWorkspaceHistoryIdentity,
    state: State<AppState>,
    direction: WorkspaceHistoryDirection,
) -> Result<ProjectWorkspaceUndoRedoCommandReceipt, String> {
    let mut slot = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru Undo/Redo.".to_string())?;
    let workspace = slot
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru Undo/Redo.".to_string())?;
    let workspace_identity = ProjectWorkspaceIdentity {
        expected_project_root: identity.expected_project_root.clone(),
        expected_session_id: identity.expected_session_id.clone(),
        expected_revision: identity.expected_revision,
    };
    let (result, project_model_build) =
        commit_project_workspace_session_mutation(&app, workspace, |candidate| {
            let previous_model = candidate.project_model.clone();
            let previous_model_source_revision = candidate.project_model_source_revision;
            candidate.require_history_target(direction, &identity.expected_transaction_id)?;
            let result = match direction {
                WorkspaceHistoryDirection::Undo => {
                    candidate.undo(&workspace_identity, file_buffer_now_ms())
                }
                WorkspaceHistoryDirection::Redo => {
                    candidate.redo(&workspace_identity, file_buffer_now_ms())
                }
            }?;
            let projection = candidate.capture_projection_snapshot()?;
            let incremental_intent = if result.canvas_delta.is_some() {
                ProjectModelIncrementalIntent::HtmlStructural
            } else {
                ProjectModelIncrementalIntent::Unsupported
            };
            let build = rebuild_project_model_after_workspace_change(
                Path::new(&candidate.session.project_root),
                previous_model.as_ref(),
                previous_model_source_revision,
                &projection,
                &result.entry.document_paths,
                incremental_intent,
            )?;
            let alias_updates = previous_model
                .as_ref()
                .map(|before_model| {
                    history_source_identity_aliases(
                        before_model,
                        &build.model,
                        result.canvas_delta.as_ref(),
                        direction,
                    )
                })
                .unwrap_or_default();
            candidate.publish_project_model(&projection, build.model)?;
            candidate.publish_source_identity_alias_transition(
                result.revision_before,
                result.revision_after,
                alias_updates,
            )?;
            Ok((result, build.report))
        })?;
    append_history_project_model_build_event(&app, direction, &project_model_build);
    let canvas_patch = result.canvas_delta.as_ref().and_then(|delta| {
        let (before_model_revision, after_model_revision, operation) = match direction {
            WorkspaceHistoryDirection::Undo => (
                delta.after_model_revision.as_str(),
                delta.before_model_revision.as_str(),
                delta.inverse.clone(),
            ),
            WorkspaceHistoryDirection::Redo => (
                delta.before_model_revision.as_str(),
                delta.after_model_revision.as_str(),
                delta.forward.clone(),
            ),
        };
        CanvasPatch::issued_for_history(
            &workspace.session.project_root,
            &workspace.runtime_session_id(),
            result.revision_before,
            result.revision_after,
            &result.application_transaction_id,
            before_model_revision,
            after_model_revision,
            operation,
        )
        .ok()
    });
    let workspace_snapshot = workspace.snapshot();
    let session = workspace.session.clone();
    let runtime_session_id = workspace.runtime_session_id();
    let project_root = workspace.session.project_root.clone();
    drop(slot);

    let reconciliation = state.file_explorer.history_reconciliation(
        &runtime_session_id,
        &result.entry.transaction_id,
        matches!(direction, WorkspaceHistoryDirection::Undo),
    )?;
    let workbench = if let Some(reconciliation) = reconciliation {
        if let Some((from, to)) = reconciliation.remap.as_ref() {
            state
                .file_explorer
                .remap_entry_prefix(&runtime_session_id, from, to)?;
        }
        let remaps = reconciliation
            .remap
            .into_iter()
            .map(
                |(source_prefix, destination_prefix)| WorkbenchProjectEntryRemap {
                    source_prefix,
                    destination_prefix,
                },
            )
            .collect();
        let deleted_prefixes = reconciliation.deleted_prefix.into_iter().collect();
        let (receipt, persistence_warning) = state.workbench.apply_latest_after_primary_commit(
            &session,
            WorkbenchIntent::ReconcileProjectEntries {
                remaps,
                deleted_prefixes,
                selection_override: reconciliation.selection_override,
            },
            |snapshot| persist_workbench(&app, &session, snapshot),
        )?;
        if let Some(warning) = persistence_warning {
            eprintln!(
                "[Pană Studio] Undo/Redo a comis ProjectWorkspace, dar persistența Workbench necesită reîncercare: {warning}"
            );
        }
        Some(receipt)
    } else {
        None
    };
    Ok(ProjectWorkspaceUndoRedoCommandReceipt {
        schema_version: PROJECT_WORKSPACE_UNDO_REDO_COMMAND_SCHEMA_VERSION,
        project_root,
        runtime_session_id,
        result,
        workspace: workspace_snapshot,
        workbench,
        canvas_patch,
    })
}

fn history_source_identity_aliases(
    before_model: &ProjectModel,
    after_model: &ProjectModel,
    canvas_delta: Option<&WorkspaceCanvasHistoryDelta>,
    direction: WorkspaceHistoryDirection,
) -> std::collections::HashMap<String, String> {
    let mut aliases = html_identity_aliases(before_model, after_model);
    let Some(canvas_delta) = canvas_delta else {
        return aliases;
    };
    let (before_operation, after_operation) = match direction {
        WorkspaceHistoryDirection::Undo => (&canvas_delta.inverse, &canvas_delta.forward),
        WorkspaceHistoryDirection::Redo => (&canvas_delta.forward, &canvas_delta.inverse),
    };
    for (before_anchor, after_anchor) in
        paired_history_canvas_anchors(before_operation, after_operation)
    {
        let Some(before_source_id) = live_history_anchor_source_id(before_model, before_anchor)
        else {
            continue;
        };
        let Some(after_source_id) = live_history_anchor_source_id(after_model, after_anchor) else {
            continue;
        };
        if before_source_id != after_source_id {
            aliases.insert(before_source_id, after_source_id);
        }
    }
    aliases
}

fn paired_history_canvas_anchors<'a>(
    before: &'a CanvasPatchOperation,
    after: &'a CanvasPatchOperation,
) -> Vec<(&'a CanvasPatchAnchor, &'a CanvasPatchAnchor)> {
    match (before, after) {
        (
            CanvasPatchOperation::SetAttributes { target: before, .. },
            CanvasPatchOperation::SetAttributes { target: after, .. },
        )
        | (
            CanvasPatchOperation::SetBlockOption { target: before, .. },
            CanvasPatchOperation::SetBlockOption { target: after, .. },
        )
        | (
            CanvasPatchOperation::SetText { target: before, .. },
            CanvasPatchOperation::SetText { target: after, .. },
        )
        | (
            CanvasPatchOperation::SetTextHtml { target: before, .. },
            CanvasPatchOperation::SetTextHtml { target: after, .. },
        )
        | (
            CanvasPatchOperation::ReplaceTag { target: before, .. },
            CanvasPatchOperation::ReplaceTag { target: after, .. },
        ) => vec![(before, after)],
        (
            CanvasPatchOperation::Move {
                source: before_source,
                target: before_target,
                ..
            },
            CanvasPatchOperation::Move {
                source: after_source,
                target: after_target,
                ..
            },
        ) => vec![(before_source, after_source), (before_target, after_target)],
        _ => Vec::new(),
    }
}

fn live_history_anchor_source_id(
    model: &ProjectModel,
    anchor: &CanvasPatchAnchor,
) -> Option<String> {
    std::iter::once(anchor.source_id.as_str())
        .chain(anchor.alternate_source_ids.iter().map(String::as_str))
        .find(|source_id| {
            model
                .source_graph
                .nodes
                .iter()
                .any(|node| node.kind == SourceNodeKind::Html && node.id == *source_id)
        })
        .map(str::to_string)
}

#[cfg(test)]
mod source_identity_history_tests {
    use std::{
        collections::{BTreeMap, HashMap},
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::project_model::build_project_model;

    use super::*;

    #[test]
    fn undo_and_redo_publish_the_exact_attribute_target_identity_transition() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(
            root.join("zola.toml"),
            "base_url = \"http://example.test\"\n",
        )
        .unwrap();
        fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = \"Acasă\"\ntemplate = \"index.html\"\n+++\n",
        )
        .unwrap();
        let before_source = concat!(
            "<h1>\n",
            "  <span>Construiește vizual.</span>\n",
            "  <span>Păstrează controlul</span>\n",
            "  <span>asupra codului.</span>\n",
            "</h1>\n",
        );
        let after_source = concat!(
            "<h1>\n",
            "  <span>Construiește vizual.</span>\n",
            "  <span class=\"ps-span-control-abc123\">Păstrează controlul</span>\n",
            "  <span>asupra codului.</span>\n",
            "</h1>\n",
        );
        fs::write(root.join("templates/index.html"), before_source).unwrap();
        let before = build_project_model(&root, &HashMap::new()).unwrap();
        let mut drafts = HashMap::new();
        drafts.insert("templates/index.html".to_string(), after_source.to_string());
        let after = build_project_model(&root, &drafts).unwrap();
        let before_id = span_id_for_text(&before, before_source, "Păstrează controlul");
        let after_id = span_id_for_text(&after, after_source, "Păstrează controlul");
        assert_ne!(before_id, after_id);

        let delta = WorkspaceCanvasHistoryDelta {
            before_model_revision: before.revision.clone(),
            after_model_revision: after.revision.clone(),
            forward: CanvasPatchOperation::SetAttributes {
                target: CanvasPatchAnchor::source(&before_id, None, Some("span")),
                attributes: BTreeMap::from([(
                    "class".to_string(),
                    Some("ps-span-control-abc123".to_string()),
                )]),
            },
            inverse: CanvasPatchOperation::SetAttributes {
                target: CanvasPatchAnchor::source(&after_id, None, Some("span"))
                    .with_alternate_source_ids([before_id.clone()]),
                attributes: BTreeMap::from([("class".to_string(), None)]),
            },
        };

        let undo_aliases = history_source_identity_aliases(
            &after,
            &before,
            Some(&delta),
            WorkspaceHistoryDirection::Undo,
        );
        assert_eq!(undo_aliases.get(&after_id), Some(&before_id));

        let redo_aliases = history_source_identity_aliases(
            &before,
            &after,
            Some(&delta),
            WorkspaceHistoryDirection::Redo,
        );
        assert_eq!(redo_aliases.get(&before_id), Some(&after_id));
        fs::remove_dir_all(root).unwrap();
    }

    fn span_id_for_text(model: &ProjectModel, source: &str, text: &str) -> String {
        model
            .source_graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == SourceNodeKind::Html
                    && node.label.starts_with("<span")
                    && node.range.as_ref().is_some_and(|range| {
                        source
                            .get(range.start..range.end)
                            .is_some_and(|fragment| fragment.contains(text))
                    })
            })
            .expect("span semantic")
            .id
            .clone()
    }

    fn unique_test_dir() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "pana-studio-selection-history-{}-{stamp}",
            std::process::id()
        ))
    }
}

fn append_history_project_model_build_event(
    app: &AppHandle,
    direction: WorkspaceHistoryDirection,
    report: &crate::project_model::ProjectModelIncrementalBuildReport,
) {
    let event = KernelLogEvent::new(
        KernelLogLevel::Info,
        match direction {
            WorkspaceHistoryDirection::Undo => KernelEventKind::UndoApplied,
            WorkspaceHistoryDirection::Redo => KernelEventKind::RedoApplied,
        },
        "project_workspace",
        "history_project_model",
        "project_workspace.history.project_model",
        report.workspace_transaction_id.clone(),
        "Undo/Redo rebuilt ProjectModel under Rust authority.",
        None,
    )
    .with_attribute("projectModelBuildMode", report.mode.label())
    .with_attribute("projectModelFallbackReason", report.fallback_reason.clone())
    .with_attribute("projectModelBuildMs", report.duration_ms)
    .with_attribute("changedPathCount", report.changed_paths.len())
    .with_attribute(
        "invalidatedTemplateCount",
        report.invalidated_template_files.len(),
    )
    .with_attribute("invalidatedPageCount", report.invalidated_page_files.len())
    .with_attribute("replacedNodes", report.replaced_nodes)
    .with_attribute("reusedNodes", report.reused_nodes)
    .with_attribute("reusedRelations", report.reused_relations)
    .with_attribute("projectModelCloneMs", report.model_clone_ms)
    .with_attribute("projectModelTemplateParseMs", report.template_parse_ms)
    .with_attribute("projectModelComponentGraphMs", report.component_graph_ms)
    .with_attribute("projectModelBlockGraphMs", report.block_graph_ms)
    .with_attribute("projectModelTeraGraphMs", report.tera_graph_ms);
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _ = append_event(&app, event);
    });
}

pub(crate) fn require_recovery_coordinator_clean_for_write(
    state: &State<AppState>,
    session: &ProjectSessionSnapshot,
    caller: &str,
) -> Result<(), String> {
    let scan = state
        .recovery_coordinator_scan
        .lock()
        .map_err(|_| "Nu am putut bloca RecoveryCoordinatorScan.".to_string())?
        .clone()
        .ok_or_else(|| {
            format!(
                "{caller} a blocat scrierea: Transaction Recovery Scan lipsește pentru sesiunea curentă."
            )
        })?;
    if scan.session_id != session.id {
        return Err(format!(
            "{caller} a blocat scrierea: Transaction Recovery Scan aparține sesiunii {}, dar sesiunea curentă este {}.",
            scan.session_id, session.id
        ));
    }
    if scan.project_root != session.project_root {
        return Err(format!(
            "{caller} a blocat scrierea: Transaction Recovery Scan aparține proiectului {}, dar sesiunea curentă este pentru {}.",
            scan.project_root, session.project_root
        ));
    }
    if scan.status != RecoveryCoordinatorStatus::Clean {
        return Err(format!(
            "{caller} a blocat scrierea: Transaction Recovery Scan este {} pentru sesiunea curentă.",
            recovery_coordinator_status_label(scan.status)
        ));
    }
    Ok(())
}

pub(crate) fn require_project_workspace_available_for_write(
    state: &State<AppState>,
) -> Result<(), String> {
    let root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul proiectului pentru mutație.".to_string())?;
    let root = root
        .as_ref()
        .ok_or_else(|| "Nu există proiect curent pentru mutație.".to_string())?;
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru mutație.".to_string())?;
    let workspace = workspace
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru mutație.".to_string())?;
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        root,
    )
}

fn recovery_coordinator_status_label(status: RecoveryCoordinatorStatus) -> &'static str {
    match status {
        RecoveryCoordinatorStatus::Clean => "clean",
        RecoveryCoordinatorStatus::NeedsAttention => "needs_attention",
        RecoveryCoordinatorStatus::Unreadable => "unreadable",
    }
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

fn require_project_transition_for_action(
    app: &AppHandle,
    state: &State<AppState>,
    target_root: &PathBuf,
    action: KernelProjectTransitionAction,
    operator_decision_id: Option<&str>,
) -> Result<(), String> {
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
    target_root: &PathBuf,
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
    target_root: &PathBuf,
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
    let session = workspace.session.clone();
    let store = workspace.documents.clone();
    let workspace_snapshot = workspace.snapshot();
    let disk_conflicts = scan_disk_conflicts(&store);
    if policy.action != action {
        return Err("Project Transition Policy nu corespunde acțiunii cerute.".to_string());
    }
    build_kernel_project_transition_decision_evidence(
        &session,
        &store,
        Some(&disk_conflicts),
        &workspace_snapshot,
        project_state,
        policy,
        target_root.to_string_lossy().as_ref(),
    )
}

fn project_transition_action_for_open_target(
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

fn record_project_transition_blocked(
    app: &AppHandle,
    policy: &crate::kernel::project_state::KernelProjectTransitionPolicy,
    target_root: &PathBuf,
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
struct ProjectTransitionRuntimeLease {
    current_root: Option<String>,
    project_workspace_fingerprint: Option<String>,
    recovery_fingerprint: Option<String>,
}

fn capture_project_transition_runtime_lease(
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

fn project_transition_runtime_lease_from_parts(
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
        project_workspace_fingerprint: project_workspace
            .as_ref()
            .map(|workspace| {
                serialize_project_transition_lease("ProjectWorkspace", &workspace.snapshot())
            })
            .transpose()?,
        recovery_fingerprint: recovery
            .as_ref()
            .map(|scan| serialize_project_transition_lease("RecoveryCoordinatorScan", scan))
            .transpose()?,
    })
}

fn serialize_project_transition_lease<T: Serialize>(
    label: &str,
    value: &T,
) -> Result<String, String> {
    serde_json::to_string(value)
        .map_err(|error| format!("{label} nu poate fi serializat pentru lease: {error}"))
}

fn clear_project_runtime_state(
    app: &AppHandle,
    state: &State<AppState>,
    expected_lease: Option<&ProjectTransitionRuntimeLease>,
) -> Result<(), String> {
    state
        .ai_coordination
        .require_project_transition()
        .map_err(|error| error.to_string())?;
    let _disk_watch_transition = state
        .project_disk_watch_transition
        .lock()
        .map_err(|_| "Serializarea watcher-ului este compromisă la închidere.".to_string())?;
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
                "Project Transition close lease a devenit stale; runtime-ul curent nu a fost șters."
                    .to_string(),
            );
        }
    }

    let disk_watcher = state
        .project_disk_watch
        .lock()
        .map_err(|_| "Slot-ul watcher-ului este compromis la închidere.".to_string())?
        .take();
    if let Some(disk_watcher) = disk_watcher {
        disk_watcher.stop();
    }
    let authority_runtime = app
        .try_state::<WriteAuthorityRuntime>()
        .ok_or_else(|| "WriteAuthorityRuntime lipsește la închiderea proiectului.".to_string())?;
    let mut authority_publication = authority_runtime.project_publication()?;
    authority_publication.revoke();
    *current_root = None;
    *project_workspace = None;
    *recovery_coordinator_scan = None;
    state
        .ai_coordination
        .bind_project(None, crate::kernel::observability::now_ms())
        .map_err(|error| error.to_string())?;
    drop(recovery_coordinator_scan);
    drop(project_workspace);
    drop(current_root);
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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTransitionDecisionRetentionHotJournalRecoveryCommandResult {
    pub receipt: KernelProjectTransitionDecisionRetentionRecoveryReceipt,
    pub recovery_coordinator: RecoveryCoordinatorScan,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkspaceSaveRecoveryCommandResult {
    pub receipt: ProjectWorkspaceSaveRecoveryReceipt,
    pub recovery_coordinator: RecoveryCoordinatorScan,
    pub workspace: ProjectWorkspaceSnapshot,
}

#[tauri::command]
pub fn recover_project_workspace_save(
    transaction_id: String,
    action: ProjectWorkspaceSaveRecoveryAction,
    diagnostic: String,
    app: AppHandle,
    state: State<AppState>,
) -> Result<ProjectWorkspaceSaveRecoveryCommandResult, String> {
    let session = require_current_project_session(&state)?;
    let root = require_current_project_root(&state)?;
    let receipt = apply_project_workspace_save_recovery(
        &app,
        &session,
        &root,
        &transaction_id,
        action,
        diagnostic,
    )?;

    let scan = scan_project_root(&root)?;
    let documents = bootstrap_file_buffer_store(&app, &session, &root, &scan)?;
    let manifest = read_project_disk_manifest(&root)?;
    let accepted = AcceptedProjectDiskManifest::new(
        session.runtime_instance_id(),
        session.project_root.clone(),
        manifest,
    )?;
    let mut rebuilt = ProjectWorkspace::new(
        session.clone(),
        accepted,
        documents,
        PageJsDraftStore::new(&session),
    )?;
    restore_project_workspace_recovery(&app, &mut rebuilt)?;
    migrate_legacy_component_catalog(&root, &mut rebuilt, now_ms())?;
    let workspace_snapshot = rebuilt.snapshot();

    {
        let current_root = state.current_root.lock().map_err(|_| {
            "Nu am putut valida root-ul după ProjectWorkspace recovery.".to_string()
        })?;
        if current_root.as_ref() != Some(&root) {
            return Err(
                "ProjectWorkspace recovery a devenit stale: proiectul curent s-a schimbat."
                    .to_string(),
            );
        }
        let mut slot = state
            .project_workspace
            .lock()
            .map_err(|_| "Nu am putut publica ProjectWorkspace recuperat.".to_string())?;
        let live_session = slot
            .as_ref()
            .map(|workspace| workspace.runtime_session_id())
            .ok_or_else(|| "ProjectWorkspace a fost închis în timpul recuperării.".to_string())?;
        if live_session != session.runtime_instance_id() {
            return Err(
                "ProjectWorkspace recovery a devenit stale: instanța sesiunii s-a schimbat."
                    .to_string(),
            );
        }
        *slot = Some(rebuilt);
    }
    refresh_recovery_coordinator_scan(&app, &state, &session, true)?;
    let recovery_coordinator = state
        .recovery_coordinator_scan
        .lock()
        .map_err(|_| "Nu am putut citi scanarea după ProjectWorkspace recovery.".to_string())?
        .clone()
        .ok_or_else(|| {
            "Transaction Recovery Scan lipsește după ProjectWorkspace recovery.".to_string()
        })?;
    Ok(ProjectWorkspaceSaveRecoveryCommandResult {
        receipt,
        recovery_coordinator,
        workspace: workspace_snapshot,
    })
}

#[tauri::command(async)]
pub fn read_file_buffer_text(
    relative_path: String,
    identity: FileBufferRequestIdentity,
    state: State<AppState>,
) -> Result<FileBufferCommandReceipt<FileBufferTextSnapshot>, String> {
    read_file_buffer_text_impl(relative_path, identity, state.inner())
}

fn read_file_buffer_text_impl(
    relative_path: String,
    identity: FileBufferRequestIdentity,
    state: &AppState,
) -> Result<FileBufferCommandReceipt<FileBufferTextSnapshot>, String> {
    with_bound_file_buffer(state, &identity, |_, store| {
        store
            .text_snapshot(&relative_path)
            .ok_or_else(|| format!("FileBufferStore nu are text pentru {relative_path}."))
    })
}

fn with_bound_file_buffer<T>(
    state: &AppState,
    identity: &FileBufferRequestIdentity,
    operation: impl FnOnce(&ProjectSessionSnapshot, &mut FileBufferStore) -> Result<T, String>,
) -> Result<FileBufferCommandReceipt<T>, String> {
    with_bound_project_workspace(state, identity, |workspace| {
        let session = workspace.session.clone();
        let payload = operation(&session, &mut workspace.documents)?;
        Ok(FileBufferCommandReceipt::new(
            &session,
            workspace.revision,
            payload,
        ))
    })
}

fn with_bound_project_workspace<T>(
    state: &AppState,
    identity: &FileBufferRequestIdentity,
    operation: impl FnOnce(&mut ProjectWorkspace) -> Result<T, String>,
) -> Result<T, String> {
    let current_root_guard = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul curent pentru FileBufferStore.".to_string())?;
    let current_root_path = current_root_guard
        .as_ref()
        .ok_or_else(|| "Nu există proiect curent pentru FileBufferStore.".to_string())?;
    let current_root = current_root_path.to_string_lossy().into_owned();
    let mut project_workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru FileBufferStore.".to_string())?;
    let workspace = project_workspace.as_mut().ok_or_else(|| {
        "ProjectWorkspace nu este inițializat pentru FileBufferStore.".to_string()
    })?;
    require_file_buffer_session_binding(
        &current_root,
        &workspace.session,
        &workspace.documents,
        identity,
    )?;
    operation(workspace)
}

#[tauri::command(async)]
pub fn set_file_buffer_draft(
    relative_path: String,
    contents: String,
    expectation: FileBufferMutationExpectation,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<FileBufferCommandReceipt<FileBufferFileSnapshot>, String> {
    set_file_buffer_draft_impl(
        relative_path,
        contents,
        expectation,
        identity,
        &app,
        state.inner(),
    )
}

fn set_file_buffer_draft_impl(
    relative_path: String,
    contents: String,
    expectation: FileBufferMutationExpectation,
    identity: FileBufferRequestIdentity,
    app: &AppHandle,
    state: &AppState,
) -> Result<FileBufferCommandReceipt<FileBufferFileSnapshot>, String> {
    with_bound_project_workspace(state, &identity, |workspace| {
        let file = commit_project_workspace_session_mutation(app, workspace, |candidate| {
            let mut validation_store = candidate.documents.clone();
            validation_store.set_draft_if_current(
                &relative_path,
                contents.clone(),
                &expectation,
                file_buffer_now_ms(),
            )?;
            let receipt = candidate.stage_document_texts(
                &workspace_identity(candidate),
                WorkspaceMutationMetadata {
                    label: "Editare document".to_string(),
                    source: "code_editor.full_draft".to_string(),
                    coalesce_key: Some(format!("document:{relative_path}")),
                    transaction_id: None,
                },
                vec![WorkspaceDocumentMutation {
                    relative_path: relative_path.clone(),
                    contents,
                }],
                file_buffer_now_ms(),
            )?;
            receipt
                .files
                .into_iter()
                .next()
                .ok_or_else(|| "ProjectWorkspace nu a returnat documentul editat.".to_string())
        })?;
        Ok(FileBufferCommandReceipt::new(
            &workspace.session,
            workspace.revision,
            file,
        ))
    })
}

#[tauri::command(async)]
pub fn apply_file_buffer_changeset(
    input: FileBufferChangeSetInput,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<FileBufferCommandReceipt<FileBufferChangeSetResult>, String> {
    apply_file_buffer_changeset_impl(input, identity, &app, state.inner())
}

fn apply_file_buffer_changeset_impl(
    input: FileBufferChangeSetInput,
    identity: FileBufferRequestIdentity,
    app: &AppHandle,
    state: &AppState,
) -> Result<FileBufferCommandReceipt<FileBufferChangeSetResult>, String> {
    with_bound_project_workspace(state, &identity, |workspace| {
        let source = input
            .source
            .clone()
            .unwrap_or_else(|| "code_editor.changeset".to_string());
        let relative_path = input.relative_path.clone();
        let result = commit_project_workspace_session_mutation(app, workspace, |candidate| {
            candidate.apply_document_changeset(
                &workspace_identity(candidate),
                WorkspaceMutationMetadata {
                    label: "Editare document".to_string(),
                    source,
                    coalesce_key: Some(format!("document:{relative_path}")),
                    transaction_id: None,
                },
                input,
                file_buffer_now_ms(),
            )
        })?;
        Ok(FileBufferCommandReceipt::new(
            &workspace.session,
            workspace.revision,
            result,
        ))
    })
}

#[tauri::command(async)]
pub fn clear_file_buffer_draft(
    relative_path: String,
    expectation: FileBufferMutationExpectation,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<FileBufferCommandReceipt<FileBufferFileSnapshot>, String> {
    clear_file_buffer_draft_impl(relative_path, expectation, identity, &app, state.inner())
}

fn clear_file_buffer_draft_impl(
    relative_path: String,
    expectation: FileBufferMutationExpectation,
    identity: FileBufferRequestIdentity,
    app: &AppHandle,
    state: &AppState,
) -> Result<FileBufferCommandReceipt<FileBufferFileSnapshot>, String> {
    with_bound_project_workspace(state, &identity, |workspace| {
        let file = commit_project_workspace_session_mutation(app, workspace, |candidate| {
            let mut validation_store = candidate.documents.clone();
            validation_store.clear_draft_if_current(&relative_path, &expectation)?;
            let baseline = candidate
                .documents
                .baseline_text_for(&relative_path)
                .ok_or_else(|| {
                    format!("ProjectWorkspace nu are baseline pentru {relative_path}.")
                })?;
            let receipt = candidate.stage_document_texts(
                &workspace_identity(candidate),
                WorkspaceMutationMetadata {
                    label: "Renunțare la modificările documentului".to_string(),
                    source: "code_editor.clear_draft".to_string(),
                    coalesce_key: None,
                    transaction_id: None,
                },
                vec![WorkspaceDocumentMutation {
                    relative_path,
                    contents: baseline,
                }],
                file_buffer_now_ms(),
            )?;
            receipt
                .files
                .into_iter()
                .next()
                .ok_or_else(|| "ProjectWorkspace nu a returnat documentul curățat.".to_string())
        })?;
        Ok(FileBufferCommandReceipt::new(
            &workspace.session,
            workspace.revision,
            file,
        ))
    })
}

fn workspace_identity(workspace: &ProjectWorkspace) -> ProjectWorkspaceIdentity {
    ProjectWorkspaceIdentity {
        expected_project_root: workspace.session.project_root.clone(),
        expected_session_id: workspace.runtime_session_id(),
        expected_revision: workspace.revision,
    }
}

#[tauri::command]
pub fn scan_project(
    path: String,
    state: State<AppState>,
) -> Result<crate::project::ProjectScan, String> {
    let requested_root = PathBuf::from(path);
    let projection = {
        let current_root = state
            .current_root
            .lock()
            .map_err(|_| "Nu am putut valida root-ul pentru ProjectScan.".to_string())?;
        if current_root.as_ref() != Some(&requested_root) {
            return Err(
                "ProjectScan a refuzat un root diferit de ProjectSession activă.".to_string(),
            );
        }
        let workspace = state.project_workspace.lock().map_err(|_| {
            "Nu am putut bloca ProjectWorkspace pentru scanarea proiectului.".to_string()
        })?;
        let workspace = workspace.as_ref().ok_or_else(|| {
            "ProjectWorkspace nu este inițializat pentru ProjectScan.".to_string()
        })?;
        workspace.capture_projection_snapshot()?
    };
    let mut scan = scan_project_workspace_projection(&projection)?;
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut revalida root-ul pentru ProjectScan.".to_string())?;
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut revalida ProjectWorkspace pentru ProjectScan.".to_string())?;
    if current_root.as_ref() != Some(&requested_root) {
        return Err("ProjectScan a devenit stale în timpul construcției.".to_string());
    }
    let workspace = workspace
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace a dispărut în timpul ProjectScan.".to_string())?;
    workspace.require_current_projection(&projection)?;
    if workspace.project_model_source_revision == Some(projection.revision) {
        if let Some(model) = workspace.project_model.as_ref() {
            apply_project_model_preview_routes(
                &mut scan,
                model
                    .source_graph
                    .pages
                    .iter()
                    .map(|page| (page.file.as_str(), page.url.as_str())),
            );
        }
    }
    Ok(scan)
}

#[tauri::command]
pub async fn read_current_project_disk_manifest(
    state: State<'_, AppState>,
) -> Result<crate::project::ProjectDiskManifest, String> {
    let root = require_current_project_root(&state)?;
    tauri::async_runtime::spawn_blocking(move || read_project_disk_manifest(&root))
        .await
        .map_err(|error| {
            format!("Monitorizarea discului proiectului s-a oprit neașteptat: {error}")
        })?
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectDiskWatchRequest {
    pub expected_project_root: String,
    pub expected_session_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectDiskWatchStopRequest {
    pub expected_project_root: String,
    pub expected_session_id: String,
    pub expected_watch_generation: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDiskWatchReceipt {
    pub project_root: String,
    pub runtime_session_id: String,
    pub watch_generation: u64,
}

#[tauri::command]
pub fn start_project_disk_watch(
    input: ProjectDiskWatchRequest,
    app: AppHandle,
    state: State<AppState>,
) -> Result<ProjectDiskWatchReceipt, String> {
    let _transition = state
        .project_disk_watch_transition
        .lock()
        .map_err(|_| "Serializarea watcher-ului este compromisă.".to_string())?;
    ensure_project_disk_watch(&app, state.inner(), &input)
}

fn ensure_project_disk_watch(
    app: &AppHandle,
    state: &AppState,
    input: &ProjectDiskWatchRequest,
) -> Result<ProjectDiskWatchReceipt, String> {
    let (project_root, runtime_session_id) = {
        let workspace = state
            .project_workspace
            .lock()
            .map_err(|_| "ProjectWorkspace este indisponibil pentru watcher.".to_string())?;
        let workspace = workspace
            .as_ref()
            .ok_or_else(|| "Watcher-ul cere un ProjectSession activ.".to_string())?;
        let snapshot = workspace.snapshot();
        if snapshot.project_root != input.expected_project_root
            || snapshot.runtime_session_id != input.expected_session_id
        {
            return Err("Watcher-ul a refuzat o identitate ProjectSession stale.".to_string());
        }
        (
            PathBuf::from(&snapshot.project_root),
            snapshot.runtime_session_id,
        )
    };

    if let Some(receipt) = state
        .project_disk_watch
        .lock()
        .map_err(|_| "Slot-ul watcher-ului este compromis.".to_string())?
        .as_ref()
        .filter(|watcher| watcher.matches(&project_root, &runtime_session_id))
        .map(|watcher| ProjectDiskWatchReceipt {
            project_root: project_root.to_string_lossy().to_string(),
            runtime_session_id: runtime_session_id.clone(),
            watch_generation: watcher.watch_generation(),
        })
    {
        return Ok(receipt);
    }

    let watcher = ProjectDiskWatchHandle::start(
        app.clone(),
        project_root.clone(),
        runtime_session_id.clone(),
    )?;
    let receipt = ProjectDiskWatchReceipt {
        project_root: project_root.to_string_lossy().to_string(),
        runtime_session_id: runtime_session_id.clone(),
        watch_generation: watcher.watch_generation(),
    };
    let still_current = state
        .project_workspace
        .lock()
        .ok()
        .and_then(|workspace| workspace.as_ref().map(ProjectWorkspace::snapshot))
        .is_some_and(|snapshot| {
            snapshot.project_root == receipt.project_root
                && snapshot.runtime_session_id == receipt.runtime_session_id
        });
    if !still_current {
        watcher.stop();
        return Err("ProjectSession s-a schimbat înainte de publicarea watcher-ului.".to_string());
    }
    let previous = {
        let mut slot = state
            .project_disk_watch
            .lock()
            .map_err(|_| "Slot-ul watcher-ului este compromis.".to_string())?;
        slot.replace(watcher)
    };
    if let Some(previous) = previous {
        previous.stop();
    }
    Ok(receipt)
}

#[tauri::command]
pub fn stop_project_disk_watch(
    input: ProjectDiskWatchStopRequest,
    state: State<AppState>,
) -> Result<(), String> {
    let _transition = state
        .project_disk_watch_transition
        .lock()
        .map_err(|_| "Serializarea watcher-ului este compromisă.".to_string())?;
    let watcher = {
        let mut slot = state
            .project_disk_watch
            .lock()
            .map_err(|_| "Slot-ul watcher-ului este compromis.".to_string())?;
        let Some(active) = slot.as_ref() else {
            return Ok(());
        };
        if !active.matches(
            Path::new(&input.expected_project_root),
            &input.expected_session_id,
        ) || active.watch_generation() != input.expected_watch_generation
        {
            return Err("Oprirea watcher-ului a refuzat o identitate stale.".to_string());
        }
        slot.take()
    };
    if let Some(watcher) = watcher {
        watcher.stop();
    }
    Ok(())
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
    println!("[Pană Studio] open_project invoked: {}", path);
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
    if root != PathBuf::from(&inspection.candidate.root) {
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
    println!(
        "[Pană Studio] open_project scanned: {} files from validated Zola root",
        scan.files.len()
    );

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
    let file_buffers = next_project_workspace.documents.snapshot();
    let workspace_snapshot = next_project_workspace.snapshot();
    let project_config = read_project_app_config_for_bootstrap(&app, &root)?;
    let mut workbench = prepare_bootstrap_workbench(&session, &authoritative_scan)?;
    let target_css_file = authoritative_scan
        .files
        .iter()
        .find(|file| {
            matches!(file.kind, ProjectFileKind::Css | ProjectFileKind::Scss)
                && file.role == crate::project::ProjectFileRole::Style
        })
        .map(|file| file.relative_path.clone());
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
        Some(candidate) => candidate.project_model().clone(),
        None => build_project_model_from_workspace_projection(&root, &bootstrap_projection)?,
    };
    apply_project_model_preview_routes(
        &mut authoritative_scan,
        preview_model
            .source_graph
            .pages
            .iter()
            .map(|page| (page.file.as_str(), page.url.as_str())),
    );
    let mut active_document =
        initial_project_file(&authoritative_scan, &workbench).and_then(|file| {
            next_project_workspace
                .documents
                .text_for(&file.relative_path)
                .map(|source| ProjectBootstrapDocument {
                    relative_path: file.relative_path.clone(),
                    source,
                    preview_path: file.preview_path.clone(),
                    diagnostic_location: bootstrap_diagnostic_target
                        .as_ref()
                        .filter(|(relative_path, _)| relative_path == &file.relative_path)
                        .map(|(_, location)| *location),
                })
        });
    if let Some(document) = active_document.as_mut() {
        document.preview_path = authoritative_scan
            .files
            .iter()
            .find(|file| file.relative_path == document.relative_path)
            .and_then(|file| file.preview_path.clone());
    }
    next_project_workspace.publish_project_model(&bootstrap_projection, preview_model.clone())?;
    let preview_generation_available = preview_candidate.is_some();
    if let Some(candidate) = preview_candidate {
        preview_engine.stage_candidate(&app, candidate)?;
    }
    let initial_surface = if preview_generation_available {
        initial_project_file(&authoritative_scan, &workbench)
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

    let previous_preview = {
        let _preview_operation = state
            .preview_workspace_operation
            .lock()
            .map_err(|_| "Nu am putut serializa commit-ul Preview inițial.".to_string())?;
        let mut preview_slot = state
            .preview_engine
            .lock()
            .map_err(|_| "Nu am putut bloca motorul Preview la commit.".to_string())?;
        let mut current_root = state
            .current_root
            .lock()
            .map_err(|_| "Nu am putut bloca starea proiectului.".to_string())?;
        let mut project_workspace = state
            .project_workspace
            .lock()
            .map_err(|_| "Nu am putut bloca ProjectWorkspace.".to_string())?;
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
        let manifest_at_commit = read_project_disk_manifest(&root)?;
        if scan.accepted_disk_manifest.as_ref() != Some(&manifest_at_commit) {
            return Err(
                "Manifestul proiectului s-a schimbat la punctul de commit; runtime-ul vechi a rămas intact."
                    .to_string(),
            );
        }
        if let Some(initial_assessment) = recovery_assessment.as_ref() {
            let commit_assessment = inspect_project_workspace_recovery_for_open(
                &app,
                &root,
                &manifest_at_commit,
                &session.root_fingerprint,
            )?;
            require_project_open_recovery_assessment_unchanged(
                initial_assessment,
                &commit_assessment,
            )?;
            let commit_resolution =
                resolve_project_open_recovery(&commit_assessment, recovery_decision.as_ref())?;
            if commit_resolution != recovery_resolution {
                return Err(
                    "Rezoluția project-open recovery s-a schimbat înainte de commit.".to_string(),
                );
            }
            if commit_resolution == ProjectOpenRecoveryResolution::ExplicitAbandon {
                let decision = recovery_decision.as_ref().ok_or_else(|| {
                    "Lipsește decizia explicită de abandonare la commit.".to_string()
                })?;
                persist_project_open_recovery_abandonment(&app, &commit_assessment, decision)?;
            }
        }
        persist_project_session_open(&app, &session)?;
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
        state
            .ai_coordination
            .bind_project(
                Some(session.runtime_instance_id()),
                crate::kernel::observability::now_ms(),
            )
            .map_err(|error| error.to_string())?;
        preview_slot.replace(prepared_preview.take()?)
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
    println!(
        "[Pană Studio] open_project current_root set: {}",
        root.display()
    );
    let lifecycle = state
        .project_lifecycle
        .commit_session(&operation_id, &opened_session_for_event)?;
    lifecycle_guard.mark_committed();
    drop(lifecycle_commit_transition);
    let mut commit_event = KernelLogEvent::new(
        KernelLogLevel::Info,
        KernelEventKind::ProjectLifecycleTransition,
        "project_lifecycle",
        "project_transition",
        "commit",
        Some(operation_id.clone()),
        "ProjectLifecycle a publicat atomic noua sesiune.",
        None,
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

    Ok(ProjectOpenBootstrapReceipt {
        schema_version: PROJECT_OPEN_BOOTSTRAP_SCHEMA_VERSION,
        project: authoritative_scan,
        lifecycle,
        file_buffers,
        workspace: workspace_snapshot,
        project_config,
        workbench,
        active_document,
        target_css_file,
        initial_surface,
    })
}

#[tauri::command]
pub fn read_project_file(relative_path: String, state: State<AppState>) -> Result<String, String> {
    require_current_project_root(&state)?;
    let project_workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace.".to_string())?;
    let store = &project_workspace
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?
        .documents;
    if let Some(text) = store.text_for(&relative_path) {
        return Ok(text);
    }
    Err(format!(
        "ProjectWorkspace nu urmărește documentul text {relative_path}; citirea paralelă direct de pe disc este interzisă."
    ))
}

pub(crate) fn refresh_recovery_coordinator_scan<R: Runtime>(
    app: &AppHandle<R>,
    state: &State<AppState>,
    session: &ProjectSessionSnapshot,
    command_succeeded: bool,
) -> Result<(), String> {
    match scan_recovery_coordinator(app, session) {
        Ok(scan) => {
            let live_workspace = state.project_workspace.lock().map_err(|_| {
                "Nu am putut bloca ProjectWorkspace pentru recovery CAS.".to_string()
            })?;
            let Some(live_session) = live_workspace.as_ref().map(|workspace| &workspace.session)
            else {
                return Err(
                    "Transaction Recovery Scan a refuzat publicarea după închiderea sesiunii."
                        .to_string(),
                );
            };
            if live_session.runtime_instance_id() != session.runtime_instance_id() {
                return Err(
                    "Transaction Recovery Scan a refuzat publicarea într-o altă instanță ProjectSession."
                        .to_string(),
                );
            }
            let mut recovery_slot = state
                .recovery_coordinator_scan
                .lock()
                .map_err(|_| "Nu am putut bloca RecoveryCoordinatorScan.".to_string())?;
            *recovery_slot = Some(scan);
            Ok(())
        }
        Err(error) => {
            if let Ok(live_workspace) = state.project_workspace.lock() {
                let matches_live_session = live_workspace.as_ref().is_some_and(|workspace| {
                    let live = &workspace.session;
                    live.runtime_instance_id() == session.runtime_instance_id()
                });
                if matches_live_session {
                    if let Ok(mut recovery_slot) = state.recovery_coordinator_scan.lock() {
                        *recovery_slot = None;
                    }
                }
            }
            if command_succeeded {
                return Err(format!(
                    "Comanda a rulat, dar Transaction Recovery Scan nu a putut fi actualizat: {error}"
                ));
            }
            Ok(())
        }
    }
}
