use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::{
    commands::workspace_entries::{
        current_workspace_identity, finish_mutation, mutation_metadata,
        WorkspaceEntryMutationReceipt,
    },
    kernel::{
        file_explorer::{
            initial_text_for_path, FileExplorerCommitPlan, FileExplorerEntryKind,
            FileExplorerOperationPlan, FileExplorerOperationReason, FileExplorerOperationRequest,
            FileExplorerSnapshot, FILE_EXPLORER_SCHEMA_VERSION,
        },
        observability::now_ms,
        project_workspace::{
            ProjectWorkspaceIdentity, WorkspaceBinaryRestoreChange, WorkspaceResourceDelete,
            WorkspaceResourceMutation,
        },
        workbench::{
            persist_workbench, read_persisted_workbench, WorkbenchCommandReceipt,
            WorkbenchIdentity, WorkbenchIntent, WorkbenchProjectEntryRemap, WorkbenchSnapshot,
            WorkbenchSurface,
        },
    },
    project::resolve_project_write_path,
    state::AppState,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileExplorerSelectionRequest {
    pub schema_version: u32,
    pub identity: ProjectWorkspaceIdentity,
    pub expected_workbench_revision: u64,
    pub entry_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileExplorerSelectionReceipt {
    pub schema_version: u32,
    pub project_root: String,
    pub runtime_session_id: String,
    pub workspace_revision: u64,
    pub workbench: WorkbenchCommandReceipt,
    pub snapshot: FileExplorerSnapshot,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileExplorerPlanRequest {
    pub schema_version: u32,
    pub identity: ProjectWorkspaceIdentity,
    pub expected_workbench_revision: u64,
    pub operation: FileExplorerOperationRequest,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileExplorerCommitRequest {
    pub schema_version: u32,
    pub identity: ProjectWorkspaceIdentity,
    pub expected_accepted_disk_generation: u64,
    pub commit_token: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileExplorerCommitReceipt {
    pub schema_version: u32,
    pub project_root: String,
    pub runtime_session_id: String,
    pub mutation: WorkspaceEntryMutationReceipt,
    pub workbench: WorkbenchCommandReceipt,
    pub snapshot: FileExplorerSnapshot,
}

#[tauri::command]
pub fn read_file_explorer_snapshot(
    identity: ProjectWorkspaceIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<FileExplorerSnapshot, String> {
    let (projection, session) = {
        let workspace = state
            .project_workspace
            .lock()
            .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru FileExplorer.".to_string())?;
        let workspace = workspace
            .as_ref()
            .ok_or_else(|| "FileExplorer cere un proiect activ.".to_string())?;
        workspace.require_identity(&identity)?;
        (
            workspace.capture_projection_lease()?,
            workspace.session.clone(),
        )
    };
    let mut workbench = state
        .workbench
        .read_or_restore(&session, || read_persisted_workbench(&session))?;
    let missing_paths = state
        .file_explorer
        .missing_workbench_paths(&projection, &workbench)?;
    if !missing_paths.is_empty() {
        workbench = state
            .workbench
            .apply_latest_persisted(
                &session,
                WorkbenchIntent::ReconcileProjectEntries {
                    remaps: Vec::new(),
                    deleted_prefixes: missing_paths,
                    selection_override: None,
                },
                |snapshot| persist_workbench(&app, &session, snapshot),
            )?
            .snapshot;
    }
    let mut snapshot = state.file_explorer.snapshot(&projection, &workbench)?;
    if state
        .ai_coordination
        .require_user_source_mutation()
        .is_err()
    {
        crate::kernel::file_explorer::FileExplorerRuntime::restrict_mutations_for_edit_authority(
            &mut snapshot,
        );
    }
    Ok(snapshot)
}

#[tauri::command]
pub fn select_file_explorer_entry(
    input: FileExplorerSelectionRequest,
    app: AppHandle,
    state: State<AppState>,
) -> Result<FileExplorerSelectionReceipt, String> {
    if input.schema_version != FILE_EXPLORER_SCHEMA_VERSION {
        return Err("FileExplorer Selection folosește o versiune incompatibilă.".to_string());
    }
    if input.entry_id.trim().is_empty() {
        return Err("FileExplorer Selection cere un entryId nenul.".to_string());
    }
    let (projection, session) = {
        let workspace = state
            .project_workspace
            .lock()
            .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru FileExplorer.".to_string())?;
        let workspace = workspace
            .as_ref()
            .ok_or_else(|| "FileExplorer cere un proiect activ.".to_string())?;
        workspace.require_identity(&input.identity)?;
        (
            workspace.capture_projection_lease()?,
            workspace.session.clone(),
        )
    };
    let before = state
        .workbench
        .read_or_restore(&session, || read_persisted_workbench(&session))?;
    if before.revision != input.expected_workbench_revision {
        return Err(format!(
            "FileExplorer Selection a refuzat Workbench revizia stale {}: revizia Rust este {}.",
            input.expected_workbench_revision, before.revision
        ));
    }
    let explorer_before = state.file_explorer.snapshot(&projection, &before)?;
    let entry = explorer_before
        .entries
        .iter()
        .find(|entry| entry.id == input.entry_id)
        .ok_or_else(|| {
            "FileExplorer Selection nu mai găsește intrarea solicitată în revizia curentă."
                .to_string()
        })?;
    let workbench = state.workbench.apply_persisted(
        &session,
        &WorkbenchIdentity {
            expected_project_root: projection.project_root.clone(),
            expected_runtime_session_id: projection.runtime_session_id.clone(),
            expected_revision: input.expected_workbench_revision,
        },
        WorkbenchIntent::SelectProjectEntry {
            relative_path: entry.relative_path.clone(),
            entry_kind: entry.kind.into(),
            open_surface: entry.open_surface,
        },
        |snapshot: &WorkbenchSnapshot| persist_workbench(&app, &session, snapshot),
    )?;
    let mut snapshot = state
        .file_explorer
        .snapshot(&projection, &workbench.snapshot)?;
    if state
        .ai_coordination
        .require_user_source_mutation()
        .is_err()
    {
        crate::kernel::file_explorer::FileExplorerRuntime::restrict_mutations_for_edit_authority(
            &mut snapshot,
        );
    }
    Ok(FileExplorerSelectionReceipt {
        schema_version: FILE_EXPLORER_SCHEMA_VERSION,
        project_root: projection.project_root,
        runtime_session_id: projection.runtime_session_id,
        workspace_revision: projection.revision,
        workbench,
        snapshot,
    })
}

#[tauri::command]
pub fn plan_file_explorer_operation(
    input: FileExplorerPlanRequest,
    state: State<AppState>,
) -> Result<FileExplorerOperationPlan, String> {
    require_schema(input.schema_version)?;
    let (projection, session) = {
        let workspace = state.project_workspace.lock().map_err(|_| {
            "Nu am putut bloca ProjectWorkspace pentru planul FileExplorer.".to_string()
        })?;
        let workspace = workspace
            .as_ref()
            .ok_or_else(|| "FileExplorer cere un proiect activ.".to_string())?;
        workspace.require_identity(&input.identity)?;
        (
            workspace.capture_projection_lease()?,
            workspace.session.clone(),
        )
    };
    let workbench = state
        .workbench
        .read_or_restore(&session, || read_persisted_workbench(&session))?;
    if workbench.revision != input.expected_workbench_revision {
        return Err(format!(
            "FileExplorer Plan a refuzat Workbench revizia stale {}: revizia Rust este {}.",
            input.expected_workbench_revision, workbench.revision
        ));
    }
    if let Err(error) = state.ai_coordination.require_user_source_mutation() {
        return state.file_explorer.blocked_operation(
            &projection,
            &workbench,
            FileExplorerOperationReason::EditAuthorityUnavailable,
            error.to_string(),
        );
    }
    state
        .file_explorer
        .plan_operation(&projection, &workbench, input.operation)
}

#[tauri::command]
pub fn commit_file_explorer_operation(
    input: FileExplorerCommitRequest,
    app: AppHandle,
    state: State<AppState>,
) -> Result<FileExplorerCommitReceipt, String> {
    require_schema(input.schema_version)?;
    if input.commit_token.trim().is_empty() {
        return Err("FileExplorer Commit cere un token nenul.".to_string());
    }
    state
        .ai_coordination
        .require_user_source_mutation()
        .map_err(|error| error.to_string())?;
    let project_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul pentru FileExplorer Commit.".to_string())?
        .clone()
        .ok_or_else(|| "FileExplorer Commit cere un proiect activ.".to_string())?;
    let mut workspace_slot = state.project_workspace.lock().map_err(|_| {
        "Nu am putut bloca ProjectWorkspace pentru FileExplorer Commit.".to_string()
    })?;
    let workspace = workspace_slot
        .as_mut()
        .ok_or_else(|| "FileExplorer Commit cere un ProjectWorkspace activ.".to_string())?;
    workspace.require_identity(&input.identity)?;
    if workspace.accepted_disk.generation != input.expected_accepted_disk_generation {
        return Err(format!(
            "FileExplorer Commit a refuzat disk generation stale {}: generația Rust este {}.",
            input.expected_accepted_disk_generation, workspace.accepted_disk.generation
        ));
    }
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        &project_root,
    )?;
    let before_projection = workspace.capture_projection_lease()?;
    let plan = state.file_explorer.consume_plan(
        &workspace.runtime_session_id(),
        workspace.revision,
        workspace.accepted_disk.generation,
        &input.commit_token,
    )?;
    let prepared = prepare_workspace_changes(&project_root, &before_projection, &plan)?;
    let session = workspace.session.clone();
    let workbench_before = state
        .workbench
        .read_or_restore(&session, || read_persisted_workbench(&session))?;
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        &project_root,
    )?;
    let receipt_path = plan.destination_path.clone();
    let label = operation_label(&plan.request);
    state
        .ai_coordination
        .require_user_source_mutation()
        .map_err(|error| error.to_string())?;
    let mutation = finish_mutation(&app, workspace, receipt_path, |candidate| {
        candidate.stage_project_bundle_changes(
            &current_workspace_identity(candidate),
            mutation_metadata(label, "file_explorer.commit"),
            prepared.text_changes,
            prepared.text_deletes,
            prepared.binary_changes,
            now_ms(),
        )
    })?;
    if let Some(transaction_id) = mutation.mutation.transaction_id.as_deref() {
        state.file_explorer.record_history_topology(
            &before_projection.runtime_session_id,
            transaction_id,
            &plan,
            workbench_before.selected_project_entry.clone(),
        )?;
    }
    let after_projection = workspace.capture_projection_lease()?;
    drop(workspace_slot);

    if let (Some(source), Some(destination)) = (
        plan.source_path.as_deref(),
        plan.destination_path.as_deref(),
    ) {
        state.file_explorer.remap_entry_prefix(
            &after_projection.runtime_session_id,
            source,
            destination,
        )?;
    }

    let intent = if let Some(source) = plan.source_path.clone() {
        if let Some(destination) = plan.destination_path.clone() {
            WorkbenchIntent::ReconcileProjectEntries {
                remaps: vec![WorkbenchProjectEntryRemap {
                    source_prefix: source,
                    destination_prefix: destination,
                }],
                deleted_prefixes: Vec::new(),
                selection_override: None,
            }
        } else {
            WorkbenchIntent::ReconcileProjectEntries {
                remaps: Vec::new(),
                deleted_prefixes: vec![source],
                selection_override: None,
            }
        }
    } else {
        let provisional = state
            .file_explorer
            .snapshot(&after_projection, &workbench_before)?;
        let projected_surface = plan.destination_path.as_deref().and_then(|destination| {
            provisional
                .entries
                .iter()
                .find(|entry| entry.relative_path == destination)
                .and_then(|entry| entry.open_surface)
        });
        workbench_intent_after_create_commit(&plan, projected_surface)?
    };
    let (workbench, workbench_persistence_warning) = state
        .workbench
        .apply_latest_after_primary_commit(&session, intent, |snapshot| {
            persist_workbench(&app, &session, snapshot)
        })?;
    if let Some(warning) = workbench_persistence_warning {
        eprintln!(
            "[Pană Studio] FileExplorer a comis ProjectWorkspace, dar persistența Workbench necesită reîncercare: {warning}"
        );
    }
    let mut snapshot = state
        .file_explorer
        .snapshot(&after_projection, &workbench.snapshot)?;
    if state
        .ai_coordination
        .require_user_source_mutation()
        .is_err()
    {
        crate::kernel::file_explorer::FileExplorerRuntime::restrict_mutations_for_edit_authority(
            &mut snapshot,
        );
    }
    Ok(FileExplorerCommitReceipt {
        schema_version: FILE_EXPLORER_SCHEMA_VERSION,
        project_root: after_projection.project_root,
        runtime_session_id: after_projection.runtime_session_id,
        mutation,
        workbench,
        snapshot,
    })
}

struct PreparedWorkspaceChanges {
    text_changes: Vec<WorkspaceResourceMutation>,
    text_deletes: Vec<WorkspaceResourceDelete>,
    binary_changes: Vec<WorkspaceBinaryRestoreChange>,
}

fn prepare_workspace_changes(
    project_root: &Path,
    projection: &crate::kernel::project_workspace::WorkspaceProjectionLease,
    plan: &FileExplorerCommitPlan,
) -> Result<PreparedWorkspaceChanges, String> {
    if plan.source_path.is_none() {
        let destination = plan
            .destination_path
            .as_deref()
            .ok_or_else(|| "FileExplorer Create nu are destinație.".to_string())?;
        let entry_kind = match &plan.request {
            FileExplorerOperationRequest::Create { entry_kind, .. } => *entry_kind,
            _ => {
                return Err(
                    "FileExplorer a primit un plan de creare cu operație incompatibilă."
                        .to_string(),
                )
            }
        };
        let (relative_path, contents) = match entry_kind {
            FileExplorerEntryKind::Directory => (
                plan.namespace_paths
                    .first()
                    .cloned()
                    .ok_or_else(|| "Planul directorului nu conține markerul Rust.".to_string())?,
                String::new(),
            ),
            FileExplorerEntryKind::Text => {
                (destination.to_string(), initial_text_for_path(destination))
            }
            FileExplorerEntryKind::Binary => {
                return Err(
                    "FileExplorer nu creează resurse binare fără un import de bytes.".to_string(),
                )
            }
        };
        return Ok(PreparedWorkspaceChanges {
            text_changes: vec![WorkspaceResourceMutation {
                relative_path,
                contents,
                create_only: true,
            }],
            text_deletes: Vec::new(),
            binary_changes: Vec::new(),
        });
    }

    let source = plan.source_path.as_deref().expect("checked above");
    let mut prepared = PreparedWorkspaceChanges {
        text_changes: Vec::new(),
        text_deletes: Vec::new(),
        binary_changes: Vec::new(),
    };
    for path in &plan.namespace_paths {
        let destination = plan
            .destination_path
            .as_deref()
            .map(|destination| remap_prefix(path, source, destination));
        if let Some(contents) = projection.source_texts.get(path) {
            if let Some(destination) = destination {
                prepared.text_changes.push(WorkspaceResourceMutation {
                    relative_path: destination,
                    contents: contents.clone(),
                    create_only: true,
                });
            }
            prepared.text_deletes.push(WorkspaceResourceDelete {
                relative_path: path.clone(),
            });
            continue;
        }

        let accepted_before = accepted_binary_bytes(project_root, projection, path)?;
        let current = projection
            .resource_bytes
            .get(path)
            .cloned()
            .or_else(|| accepted_before.clone())
            .ok_or_else(|| {
                format!("FileExplorer nu poate demonstra bytes pentru resursa binară {path}.")
            })?;
        if accepted_before.is_none() {
            return Err(format!(
                "FileExplorer nu poate muta sau șterge încă resursa binară nesalvată {path}; salvează proiectul pentru a fixa baseline-ul."
            ));
        }
        prepared.binary_changes.push(WorkspaceBinaryRestoreChange {
            relative_path: path.clone(),
            before: accepted_before,
            after: None,
        });
        if let Some(destination) = destination {
            prepared.binary_changes.push(WorkspaceBinaryRestoreChange {
                relative_path: destination,
                before: None,
                after: Some(current),
            });
        }
    }
    Ok(prepared)
}

fn accepted_binary_bytes(
    root: &Path,
    projection: &crate::kernel::project_workspace::WorkspaceProjectionLease,
    relative_path: &str,
) -> Result<Option<Vec<u8>>, String> {
    if !projection
        .accepted_disk
        .manifest
        .files
        .iter()
        .any(|entry| entry.relative_path == relative_path)
    {
        return Ok(None);
    }
    let path = resolve_project_write_path(root, relative_path)?;
    let metadata = fs::symlink_metadata(&path)
        .map_err(|error| format!("Nu am putut verifica {relative_path}: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "FileExplorer a refuzat citirea binară nesigură pentru {relative_path}."
        ));
    }
    fs::read(&path)
        .map(Some)
        .map_err(|error| format!("Nu am putut citi {relative_path}: {error}"))
}

fn remap_prefix(path: &str, source: &str, destination: &str) -> String {
    if path == source {
        destination.to_string()
    } else {
        format!("{destination}{}", &path[source.len()..])
    }
}

fn operation_label(request: &FileExplorerOperationRequest) -> &'static str {
    match request {
        FileExplorerOperationRequest::Create { entry_kind, .. } => match entry_kind {
            FileExplorerEntryKind::Directory => "Creare director",
            FileExplorerEntryKind::Text => "Creare fișier",
            FileExplorerEntryKind::Binary => "Creare resursă",
        },
        FileExplorerOperationRequest::Rename { .. } => "Redenumire intrare",
        FileExplorerOperationRequest::Move { .. } => "Mutare intrare",
        FileExplorerOperationRequest::Delete { .. } => "Ștergere intrare",
    }
}

fn workbench_intent_after_create_commit(
    plan: &FileExplorerCommitPlan,
    projected_surface: Option<WorkbenchSurface>,
) -> Result<WorkbenchIntent, String> {
    let destination = plan
        .destination_path
        .clone()
        .ok_or_else(|| "FileExplorer Create nu a produs destinația planificată.".to_string())?;
    let entry_kind =
        match &plan.request {
            FileExplorerOperationRequest::Create { entry_kind, .. } => *entry_kind,
            _ => return Err(
                "FileExplorer a cerut selecția post-commit pentru o operație care nu este Create."
                    .to_string(),
            ),
        };
    let open_surface = match entry_kind {
        FileExplorerEntryKind::Directory | FileExplorerEntryKind::Binary => None,
        FileExplorerEntryKind::Text => Some(projected_surface.unwrap_or(WorkbenchSurface::Code)),
    };
    Ok(WorkbenchIntent::SelectProjectEntry {
        relative_path: destination,
        entry_kind: entry_kind.into(),
        open_surface,
    })
}

fn require_schema(schema_version: u32) -> Result<(), String> {
    if schema_version != FILE_EXPLORER_SCHEMA_VERSION {
        return Err("FileExplorer folosește o versiune de schemă incompatibilă.".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        kernel::project_workspace::WorkspaceProjectionLease,
        project::{AcceptedProjectDiskManifest, ProjectDiskManifest, ProjectDiskManifestEntry},
    };
    use std::{
        collections::{HashMap, HashSet},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn projection(
        root: &Path,
        source_texts: HashMap<String, String>,
        accepted_files: Vec<ProjectDiskManifestEntry>,
    ) -> WorkspaceProjectionLease {
        let project_root = root.to_string_lossy().into_owned();
        let runtime_session_id = "file-explorer-command-test".to_string();
        WorkspaceProjectionLease {
            project_root: project_root.clone(),
            runtime_session_id: runtime_session_id.clone(),
            revision: 4,
            workspace_transaction_id: None,
            source_texts,
            resource_bytes: HashMap::new(),
            deleted_sources: HashSet::new(),
            changed_paths: HashSet::new(),
            accepted_disk: AcceptedProjectDiskManifest::new(
                runtime_session_id,
                project_root.clone(),
                ProjectDiskManifest {
                    root: project_root,
                    files: accepted_files,
                    truncated: false,
                    max_files: 1_000,
                },
            )
            .unwrap(),
        }
    }

    fn plan(
        request: FileExplorerOperationRequest,
        source: Option<&str>,
        destination: Option<&str>,
        paths: &[&str],
    ) -> FileExplorerCommitPlan {
        FileExplorerCommitPlan {
            request,
            source_path: source.map(str::to_string),
            destination_path: destination.map(str::to_string),
            namespace_paths: paths.iter().map(|path| (*path).to_string()).collect(),
        }
    }

    fn temp_root(label: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pana-file-explorer-{label}-{unique}"))
    }

    #[test]
    fn create_policy_prepares_text_and_hidden_directory_marker() {
        let root = Path::new("/project");
        let projection = projection(root, HashMap::new(), Vec::new());
        let directory = prepare_workspace_changes(
            root,
            &projection,
            &plan(
                FileExplorerOperationRequest::Create {
                    parent_entry_id: None,
                    entry_kind: FileExplorerEntryKind::Directory,
                    name: "partials".to_string(),
                },
                None,
                Some("templates/partials"),
                &["templates/partials/.gitkeep"],
            ),
        )
        .unwrap();
        assert_eq!(directory.text_changes.len(), 1);
        assert_eq!(
            directory.text_changes[0].relative_path,
            "templates/partials/.gitkeep"
        );
        assert!(directory.text_changes[0].create_only);

        let template = prepare_workspace_changes(
            root,
            &projection,
            &plan(
                FileExplorerOperationRequest::Create {
                    parent_entry_id: None,
                    entry_kind: FileExplorerEntryKind::Text,
                    name: "card.html".to_string(),
                },
                None,
                Some("templates/partials/card.html"),
                &["templates/partials/card.html"],
            ),
        )
        .unwrap();
        assert_eq!(template.text_changes.len(), 1);
        assert!(template.text_changes[0].contents.contains("<section"));
        assert!(template.text_changes[0].create_only);
    }

    #[test]
    fn create_selection_uses_the_committed_plan_when_projection_is_not_ready_yet() {
        let directory_plan = plan(
            FileExplorerOperationRequest::Create {
                parent_entry_id: None,
                entry_kind: FileExplorerEntryKind::Directory,
                name: "assets".to_string(),
            },
            None,
            Some("static/assets"),
            &["static/assets/.gitkeep"],
        );
        assert_eq!(
            workbench_intent_after_create_commit(&directory_plan, None).unwrap(),
            WorkbenchIntent::SelectProjectEntry {
                relative_path: "static/assets".to_string(),
                entry_kind: FileExplorerEntryKind::Directory.into(),
                open_surface: None,
            }
        );

        let extensionless_file_plan = plan(
            FileExplorerOperationRequest::Create {
                parent_entry_id: None,
                entry_kind: FileExplorerEntryKind::Text,
                name: "CNAME".to_string(),
            },
            None,
            Some("static/CNAME"),
            &["static/CNAME"],
        );
        assert_eq!(
            workbench_intent_after_create_commit(&extensionless_file_plan, None).unwrap(),
            WorkbenchIntent::SelectProjectEntry {
                relative_path: "static/CNAME".to_string(),
                entry_kind: FileExplorerEntryKind::Text.into(),
                open_surface: Some(WorkbenchSurface::Code),
            }
        );
    }

    #[test]
    fn directory_relocation_is_one_complete_text_bundle() {
        let root = Path::new("/project");
        let projection = projection(
            root,
            HashMap::from([
                ("templates/partials/.gitkeep".to_string(), String::new()),
                (
                    "templates/partials/header.html".to_string(),
                    "<header></header>".to_string(),
                ),
            ]),
            Vec::new(),
        );
        let prepared = prepare_workspace_changes(
            root,
            &projection,
            &plan(
                FileExplorerOperationRequest::Rename {
                    entry_id: "partials".to_string(),
                    new_name: "components".to_string(),
                },
                Some("templates/partials"),
                Some("templates/components"),
                &[
                    "templates/partials/.gitkeep",
                    "templates/partials/header.html",
                ],
            ),
        )
        .unwrap();

        assert_eq!(prepared.text_changes.len(), 2);
        assert_eq!(prepared.text_deletes.len(), 2);
        assert!(prepared
            .text_changes
            .iter()
            .all(|change| change.create_only));
        assert!(prepared
            .text_changes
            .iter()
            .any(|change| change.relative_path == "templates/components/header.html"));
        assert!(prepared.binary_changes.is_empty());
    }

    #[test]
    fn binary_move_captures_before_and_after_bytes_without_writing_disk() {
        let root = temp_root("binary");
        fs::create_dir_all(root.join("static")).unwrap();
        fs::write(root.join("static/logo.png"), [1_u8, 2, 3, 4]).unwrap();
        let projection = projection(
            &root,
            HashMap::new(),
            vec![ProjectDiskManifestEntry {
                relative_path: "static/logo.png".to_string(),
                modified_ms: 1,
                size: 4,
                version_token: "test".to_string(),
            }],
        );
        let prepared = prepare_workspace_changes(
            &root,
            &projection,
            &plan(
                FileExplorerOperationRequest::Move {
                    entry_id: "logo".to_string(),
                    target_directory_entry_id: Some("images".to_string()),
                },
                Some("static/logo.png"),
                Some("static/images/logo.png"),
                &["static/logo.png"],
            ),
        )
        .unwrap();

        assert_eq!(prepared.binary_changes.len(), 2);
        assert_eq!(
            prepared.binary_changes[0].before.as_deref(),
            Some([1_u8, 2, 3, 4].as_slice())
        );
        assert!(prepared.binary_changes[0].after.is_none());
        assert!(prepared.binary_changes[1].before.is_none());
        assert_eq!(
            prepared.binary_changes[1].after.as_deref(),
            Some([1_u8, 2, 3, 4].as_slice())
        );
        assert!(root.join("static/logo.png").is_file());
        assert!(!root.join("static/images/logo.png").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
