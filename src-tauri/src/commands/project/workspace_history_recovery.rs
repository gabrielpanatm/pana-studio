use std::path::Path;

use tauri::{AppHandle, State};

use super::contracts::{
    ProjectWorkspaceSaveRecoveryCommandResult, ProjectWorkspaceUndoRedoCommandReceipt,
    PROJECT_WORKSPACE_UNDO_REDO_COMMAND_SCHEMA_VERSION,
};
use crate::{
    js::PageJsDraftStore,
    kernel::{
        file_buffer_store::{bootstrap_file_buffer_store, now_ms as file_buffer_now_ms},
        observability::{append_event, KernelEventKind, KernelLogEvent, KernelLogLevel},
        preview_projection::{CanvasPatch, CanvasPatchOperation},
        project_runtime_access::{
            refresh_recovery_coordinator_scan, require_current_project_root,
            require_current_project_session,
        },
        project_workspace::{
            commit_project_workspace_session_mutation, emit_project_workspace_mutated,
            persist_project_workspace_recovery,
            recover_project_workspace_save_hot_journal as apply_project_workspace_save_recovery,
            restore_project_workspace_recovery, ProjectWorkspace, ProjectWorkspaceHistoryIdentity,
            ProjectWorkspaceIdentity, ProjectWorkspaceSaveError, ProjectWorkspaceSaveReceipt,
            ProjectWorkspaceSaveRecoveryAction, ProjectWorkspaceSnapshot,
            WorkspaceCanvasHistoryDelta, WorkspaceHistoryDirection, WorkspaceSourceTreeHistory,
            WorkspaceSourceTreeHistoryAction,
        },
        recovery_coordinator::RecoveryCoordinatorScan,
        workbench::{
            persist_workbench, WorkbenchDocumentPresentation, WorkbenchDocumentPresentationEntry,
            WorkbenchIntent, WorkbenchProjectEntryRemap,
        },
    },
    preview::{schedule_source_browser_refresh, BrowserPreviewRequestIdentity},
    project::{read_project_disk_manifest, scan_project_root, AcceptedProjectDiskManifest},
    project_model::{
        model::ProjectModel, rebuild_project_model_after_workspace_change_with_source_changes,
        ProjectModelIncrementalIntent,
    },
    source_graph::identity::{SourceChangeSet, SourceTreeMovePosition},
    state::AppState,
};

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
            let source_changes = previous_model
                .as_ref()
                .map(|before_model| {
                    history_source_changes(
                        before_model,
                        &projection,
                        &result.entry.document_paths,
                        result.canvas_delta.as_ref(),
                        result.source_tree.as_ref(),
                        direction,
                    )
                })
                .transpose()?;
            let build = rebuild_project_model_after_workspace_change_with_source_changes(
                Path::new(&candidate.session.project_root),
                previous_model.as_deref(),
                previous_model_source_revision,
                &projection,
                &result.entry.document_paths,
                incremental_intent,
                source_changes,
            )?;
            candidate.publish_project_model(&projection, build.model)?;
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
    let document_presentations = workspace_snapshot
        .documents
        .files
        .iter()
        .map(|entry| WorkbenchDocumentPresentationEntry {
            relative_path: entry.relative_path.clone(),
            presentation: WorkbenchDocumentPresentation::from_text_language(entry.language),
        })
        .collect();
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
                document_presentations,
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

fn history_source_changes(
    before_model: &ProjectModel,
    projection: &crate::kernel::project_workspace::WorkspaceProjectionSnapshot,
    changed_paths: &[String],
    canvas_delta: Option<&WorkspaceCanvasHistoryDelta>,
    source_tree: Option<&WorkspaceSourceTreeHistory>,
    direction: WorkspaceHistoryDirection,
) -> Result<Vec<SourceChangeSet>, String> {
    let mut changes = changed_paths
        .iter()
        .filter_map(|path| {
            let before = before_model
                .files
                .iter()
                .find(|file| file.relative_path == *path)?;
            let after = projection.source_texts.get(path)?;
            Some(SourceChangeSet::between(path, &before.contents, after))
        })
        .collect::<Vec<_>>();
    let restore_tree = source_tree.is_some_and(|history| {
        matches!(
            (history.action, direction),
            (
                WorkspaceSourceTreeHistoryAction::Inserted,
                WorkspaceHistoryDirection::Redo
            ) | (
                WorkspaceSourceTreeHistoryAction::Deleted,
                WorkspaceHistoryDirection::Undo
            )
        )
    });
    if restore_tree {
        if let Some(history) = source_tree {
            for tree in &history.trees {
                if let Some(change) = changes.iter_mut().find(|change| change.file == tree.file) {
                    *change = change.clone().with_tree_restore(tree.clone());
                }
            }
        }
    }
    let remove_tree = source_tree.is_some_and(|history| {
        matches!(
            (history.action, direction),
            (
                WorkspaceSourceTreeHistoryAction::Inserted,
                WorkspaceHistoryDirection::Undo
            ) | (
                WorkspaceSourceTreeHistoryAction::Deleted,
                WorkspaceHistoryDirection::Redo
            )
        )
    });
    if remove_tree {
        if let Some(history) = source_tree {
            for tree in &history.trees {
                if let Some(change) = changes.iter_mut().find(|change| change.file == tree.file) {
                    *change = change
                        .clone()
                        .with_tree_delete_many(tree.root_source_node_ids()?);
                }
            }
        }
    }

    let operation = canvas_delta.map(|delta| match direction {
        WorkspaceHistoryDirection::Undo => &delta.inverse,
        WorkspaceHistoryDirection::Redo => &delta.forward,
    });
    let mut move_operations = Vec::new();
    if let Some(operation) = operation {
        collect_canvas_move_operations(operation, &mut move_operations);
    }
    for (source, target, position) in move_operations {
        let Some(source_file) = before_model
            .source_graph
            .node_by_id(&source.source_id)
            .map(|node| node.file.as_str())
        else {
            continue;
        };
        let tree_position = match position {
            crate::project_model::move_engine::ProjectMovePosition::Before => {
                SourceTreeMovePosition::Before
            }
            crate::project_model::move_engine::ProjectMovePosition::After => {
                SourceTreeMovePosition::After
            }
            crate::project_model::move_engine::ProjectMovePosition::Inside => {
                SourceTreeMovePosition::Inside
            }
        };
        if let Some(change) = changes.iter_mut().find(|change| change.file == source_file) {
            *change =
                change
                    .clone()
                    .with_tree_move(&source.source_id, &target.source_id, tree_position);
        }
    }
    Ok(changes)
}

fn collect_canvas_move_operations<'a>(
    operation: &'a CanvasPatchOperation,
    moves: &mut Vec<(
        &'a crate::kernel::preview_projection::CanvasPatchAnchor,
        &'a crate::kernel::preview_projection::CanvasPatchAnchor,
        &'a crate::project_model::move_engine::ProjectMovePosition,
    )>,
) {
    match operation {
        CanvasPatchOperation::Move {
            source,
            target,
            position,
        } => moves.push((source, target, position)),
        CanvasPatchOperation::Batch { operations } => {
            for operation in operations {
                collect_canvas_move_operations(operation, moves);
            }
        }
        _ => {}
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
    .with_attribute("projectModelTemplateParseUs", report.template_parse_us)
    .with_attribute("projectModelComponentGraphUs", report.component_graph_us)
    .with_attribute("projectModelBlockGraphUs", report.block_graph_us)
    .with_attribute("projectModelContentModelUs", report.content_model_us)
    .with_attribute("projectModelListingItemsUs", report.listing_items_us)
    .with_attribute(
        "projectModelListingItemsReused",
        report.listing_items_reused,
    )
    .with_attribute("projectModelDynamicWidgetUs", report.dynamic_widget_us)
    .with_attribute("projectModelMarkdownUs", report.markdown_us)
    .with_attribute("projectModelNodeIndexUs", report.node_index_us);
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _ = append_event(&app, event);
    });
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
