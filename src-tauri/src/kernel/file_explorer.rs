use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::Path,
    sync::Mutex,
};

use serde::{Deserialize, Serialize};

use crate::{
    kernel::{
        project_workspace::WorkspaceProjectionSnapshot,
        project_workspace::{
            PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_BYTES,
            PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_TOTAL_BYTES,
        },
        workbench::{
            WorkbenchProjectEntryKind, WorkbenchProjectEntrySelection, WorkbenchSnapshot,
            WorkbenchSurface,
        },
    },
    project::{
        scan_project_workspace_projection_full, ProjectFile, ProjectFileKind, ProjectFileRole,
        PROJECT_SCAN_MAX_ENTRIES,
    },
};

pub const FILE_EXPLORER_SCHEMA_VERSION: u32 = 1;
const FILE_EXPLORER_MAX_PENDING_PLANS: usize = 256;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileExplorerOperationReason {
    InvalidName,
    MissingSource,
    MissingTarget,
    TargetNotDirectory,
    SameParent,
    DescendantTarget,
    DestinationConflict,
    ProtectedPath,
    UnsupportedEntryKind,
    TruncatedSnapshot,
    EditAuthorityUnavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum FileExplorerOperationRequest {
    Create {
        parent_entry_id: Option<String>,
        entry_kind: FileExplorerEntryKind,
        name: String,
    },
    Rename {
        entry_id: String,
        new_name: String,
    },
    Move {
        entry_id: String,
        target_directory_entry_id: Option<String>,
    },
    Delete {
        entry_id: String,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileExplorerOperationPlan {
    pub schema_version: u32,
    pub project_root: String,
    pub runtime_session_id: String,
    pub workspace_revision: u64,
    pub accepted_disk_generation: u64,
    pub allowed: bool,
    pub reason: Option<FileExplorerOperationReason>,
    pub diagnostic: Option<String>,
    pub commit_token: Option<String>,
    pub destination_path: Option<String>,
    pub affected_entry_ids: Vec<String>,
    pub affected_paths: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct FileExplorerCommitPlan {
    pub request: FileExplorerOperationRequest,
    pub source_path: Option<String>,
    pub destination_path: Option<String>,
    pub namespace_paths: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileExplorerEntryKind {
    Directory,
    Text,
    Binary,
}

impl From<FileExplorerEntryKind> for WorkbenchProjectEntryKind {
    fn from(value: FileExplorerEntryKind) -> Self {
        match value {
            FileExplorerEntryKind::Directory => Self::Directory,
            FileExplorerEntryKind::Text => Self::Text,
            FileExplorerEntryKind::Binary => Self::Binary,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileExplorerRole {
    Page,
    Template,
    Style,
    Script,
    Asset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileExplorerCapabilityReason {
    NotDocument,
    BinaryEditorUnavailable,
    BinaryMutationUnavailable,
    DirectoryMutationUnavailable,
    RootEntry,
    EditAuthorityUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileExplorerCapability {
    pub allowed: bool,
    pub reason: Option<FileExplorerCapabilityReason>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileExplorerCapabilities {
    pub open: FileExplorerCapability,
    pub create_child: FileExplorerCapability,
    pub rename: FileExplorerCapability,
    pub move_entry: FileExplorerCapability,
    pub delete: FileExplorerCapability,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileExplorerEntry {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub relative_path: String,
    pub depth: usize,
    pub kind: FileExplorerEntryKind,
    pub role: FileExplorerRole,
    pub preview_path: Option<String>,
    pub open_surface: Option<WorkbenchSurface>,
    pub capabilities: FileExplorerCapabilities,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileExplorerSelection {
    pub entry_id: String,
    pub relative_path: String,
    pub kind: FileExplorerEntryKind,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileExplorerSnapshot {
    pub schema_version: u32,
    pub project_root: String,
    pub runtime_session_id: String,
    pub workspace_revision: u64,
    pub accepted_disk_generation: u64,
    pub workbench_revision: u64,
    pub selection_revision: u64,
    pub selected_entry: Option<FileExplorerSelection>,
    pub active_document_path: Option<String>,
    pub entries: Vec<FileExplorerEntry>,
    pub root_capabilities: FileExplorerCapabilities,
    pub truncated: bool,
    pub max_entries: usize,
    pub diagnostics: Vec<String>,
}

#[derive(Default)]
struct FileExplorerState {
    runtime_session_id: String,
    next_entry_id: u64,
    next_plan_id: u64,
    ids_by_path: HashMap<String, String>,
    plans: HashMap<String, StoredFileExplorerPlan>,
    history_topology: HashMap<String, FileExplorerHistoryTopology>,
}

#[derive(Clone, Debug)]
struct StoredFileExplorerPlan {
    workspace_revision: u64,
    accepted_disk_generation: u64,
    commit: FileExplorerCommitPlan,
}

#[derive(Clone, Debug)]
struct FileExplorerHistoryTopology {
    request: FileExplorerOperationRequest,
    source_path: Option<String>,
    destination_path: Option<String>,
    selection_before: Option<WorkbenchProjectEntrySelection>,
}

#[derive(Clone, Debug)]
pub(crate) struct FileExplorerHistoryReconciliation {
    pub remap: Option<(String, String)>,
    pub deleted_prefix: Option<String>,
    pub selection_override: Option<WorkbenchProjectEntrySelection>,
}

#[derive(Default)]
pub struct FileExplorerRuntime {
    state: Mutex<FileExplorerState>,
}

impl FileExplorerRuntime {
    pub fn restrict_mutations_for_edit_authority(snapshot: &mut FileExplorerSnapshot) {
        let unavailable = blocked(FileExplorerCapabilityReason::EditAuthorityUnavailable);
        snapshot.root_capabilities.create_child = unavailable.clone();
        for entry in &mut snapshot.entries {
            entry.capabilities.create_child = unavailable.clone();
            entry.capabilities.rename = unavailable.clone();
            entry.capabilities.move_entry = unavailable.clone();
            entry.capabilities.delete = unavailable.clone();
        }
    }

    pub fn blocked_operation(
        &self,
        projection: &WorkspaceProjectionSnapshot,
        workbench: &WorkbenchSnapshot,
        reason: FileExplorerOperationReason,
        diagnostic: String,
    ) -> Result<FileExplorerOperationPlan, String> {
        Ok(blocked_plan(
            &self.snapshot(projection, workbench)?,
            reason,
            diagnostic,
        ))
    }

    pub fn missing_workbench_paths(
        &self,
        projection: &WorkspaceProjectionSnapshot,
        workbench: &WorkbenchSnapshot,
    ) -> Result<Vec<String>, String> {
        if projection.project_root != workbench.project_root
            || projection.runtime_session_id != workbench.runtime_session_id
        {
            return Err(
                "FileExplorer a refuzat reconcilierea unui Workbench din altă sesiune.".to_string(),
            );
        }
        let namespace = materialized_namespace(projection);
        let mut candidates = workbench
            .groups
            .iter()
            .flat_map(|group| group.documents.iter())
            .map(|document| {
                (
                    document.relative_path.clone(),
                    WorkbenchProjectEntryKind::Text,
                )
            })
            .collect::<Vec<_>>();
        if let Some(selection) = &workbench.selected_project_entry {
            candidates.push((selection.relative_path.clone(), selection.kind));
        }
        Ok(candidates
            .into_iter()
            .filter(|(path, kind)| !namespace_contains_entry(&namespace, path, *kind))
            .map(|(path, _)| path)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect())
    }

    pub fn snapshot(
        &self,
        projection: &WorkspaceProjectionSnapshot,
        workbench: &WorkbenchSnapshot,
    ) -> Result<FileExplorerSnapshot, String> {
        if projection.project_root != workbench.project_root
            || projection.runtime_session_id != workbench.runtime_session_id
        {
            return Err(
                "FileExplorer a refuzat o proiecție Workspace și un Workbench din sesiuni diferite."
                    .to_string(),
            );
        }
        let scan = scan_project_workspace_projection_full(projection)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| "FileExplorerRuntime este indisponibil.".to_string())?;
        bind_session(&mut state, &projection.runtime_session_id);

        let mut ids = BTreeMap::new();
        for file in &scan.files {
            let id = entry_id(&mut state, &file.relative_path)?;
            ids.insert(file.relative_path.clone(), id);
        }

        let entries = scan
            .files
            .into_iter()
            .map(|file| build_entry(file, projection, &ids))
            .collect::<Result<Vec<_>, _>>()?;
        let mut entries = hierarchy_order(entries)?;

        let mut diagnostics = Vec::new();
        let complete_entry_count = entries.len();
        let truncated = complete_entry_count > PROJECT_SCAN_MAX_ENTRIES;
        if truncated {
            entries.truncate(PROJECT_SCAN_MAX_ENTRIES);
            diagnostics.push(format!(
                "FileExplorer a proiectat {complete_entry_count} intrări și a publicat limita de {PROJECT_SCAN_MAX_ENTRIES}; snapshotul este marcat explicit ca trunchiat."
            ));
        }

        let selected_entry = workbench
            .selected_project_entry
            .as_ref()
            .and_then(|selection| {
                entries
                    .iter()
                    .find(|entry| entry.relative_path == selection.relative_path)
            })
            .map(|entry| FileExplorerSelection {
                entry_id: entry.id.clone(),
                relative_path: entry.relative_path.clone(),
                kind: entry.kind,
            });
        if workbench.selected_project_entry.is_some() && selected_entry.is_none() {
            diagnostics.push(
                "Selecția ProjectEntry din Workbench nu mai există în revizia Workspace curentă."
                    .to_string(),
            );
        }

        Ok(FileExplorerSnapshot {
            schema_version: FILE_EXPLORER_SCHEMA_VERSION,
            project_root: projection.project_root.clone(),
            runtime_session_id: projection.runtime_session_id.clone(),
            workspace_revision: projection.revision,
            accepted_disk_generation: projection.accepted_disk.generation,
            workbench_revision: workbench.revision,
            selection_revision: workbench.revision,
            selected_entry,
            active_document_path: active_document_path(workbench),
            entries,
            root_capabilities: root_capabilities(),
            truncated,
            max_entries: PROJECT_SCAN_MAX_ENTRIES,
            diagnostics,
        })
    }

    /// Reprojects only Workbench-owned selection metadata over an immutable
    /// explorer namespace. Selecting a document does not mutate Workspace
    /// membership, capabilities or hierarchy, so rebuilding the full scan a
    /// second time would be both redundant and visible as UI latency.
    pub fn project_workbench_selection(
        snapshot: &mut FileExplorerSnapshot,
        workbench: &WorkbenchSnapshot,
    ) -> Result<(), String> {
        if snapshot.project_root != workbench.project_root
            || snapshot.runtime_session_id != workbench.runtime_session_id
        {
            return Err(
                "FileExplorer a refuzat selecția unui Workbench din altă sesiune.".to_string(),
            );
        }
        snapshot.workbench_revision = workbench.revision;
        snapshot.selection_revision = workbench.revision;
        snapshot.selected_entry = workbench
            .selected_project_entry
            .as_ref()
            .and_then(|selection| {
                snapshot
                    .entries
                    .iter()
                    .find(|entry| entry.relative_path == selection.relative_path)
            })
            .map(|entry| FileExplorerSelection {
                entry_id: entry.id.clone(),
                relative_path: entry.relative_path.clone(),
                kind: entry.kind,
            });
        snapshot.active_document_path = active_document_path(workbench);
        Ok(())
    }

    pub fn remap_entry_prefix(
        &self,
        runtime_session_id: &str,
        source: &str,
        destination: &str,
    ) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "FileExplorerRuntime este indisponibil.".to_string())?;
        bind_session(&mut state, runtime_session_id);
        let remaps = state
            .ids_by_path
            .iter()
            .filter_map(|(path, id)| {
                if path == source {
                    Some((path.clone(), destination.to_string(), id.clone()))
                } else {
                    path.strip_prefix(&format!("{source}/"))
                        .map(|suffix| (path.clone(), format!("{destination}/{suffix}"), id.clone()))
                }
            })
            .collect::<Vec<_>>();
        for (old_path, new_path, id) in remaps {
            state.ids_by_path.remove(&old_path);
            state.ids_by_path.insert(new_path, id);
        }
        Ok(())
    }

    pub fn plan_operation(
        &self,
        projection: &WorkspaceProjectionSnapshot,
        workbench: &WorkbenchSnapshot,
        request: FileExplorerOperationRequest,
    ) -> Result<FileExplorerOperationPlan, String> {
        let snapshot = self.snapshot(projection, workbench)?;
        let namespace = materialized_namespace(projection);
        let evaluation = evaluate_operation(&snapshot, &namespace, request.clone());
        let (source_path, destination_path, namespace_paths, affected_entry_ids, affected_paths) =
            match evaluation {
                Ok(evaluation) => evaluation,
                Err((reason, diagnostic)) => {
                    return Ok(blocked_plan(&snapshot, reason, diagnostic));
                }
            };

        let mut state = self
            .state
            .lock()
            .map_err(|_| "FileExplorerRuntime este indisponibil.".to_string())?;
        bind_session(&mut state, &projection.runtime_session_id);
        if state.plans.len() >= FILE_EXPLORER_MAX_PENDING_PLANS {
            state.plans.clear();
        }
        state.next_plan_id = state
            .next_plan_id
            .checked_add(1)
            .ok_or_else(|| "FileExplorer a epuizat spațiul tokenurilor de plan.".to_string())?;
        let commit_token = format!(
            "file-plan:{}:{}:{}",
            projection.runtime_session_id, projection.revision, state.next_plan_id
        );
        state.plans.insert(
            commit_token.clone(),
            StoredFileExplorerPlan {
                workspace_revision: projection.revision,
                accepted_disk_generation: projection.accepted_disk.generation,
                commit: FileExplorerCommitPlan {
                    request,
                    source_path,
                    destination_path: destination_path.clone(),
                    namespace_paths,
                },
            },
        );
        Ok(FileExplorerOperationPlan {
            schema_version: FILE_EXPLORER_SCHEMA_VERSION,
            project_root: snapshot.project_root,
            runtime_session_id: snapshot.runtime_session_id,
            workspace_revision: snapshot.workspace_revision,
            accepted_disk_generation: snapshot.accepted_disk_generation,
            allowed: true,
            reason: None,
            diagnostic: None,
            commit_token: Some(commit_token),
            destination_path,
            affected_entry_ids,
            affected_paths,
        })
    }

    pub(crate) fn consume_plan(
        &self,
        runtime_session_id: &str,
        workspace_revision: u64,
        accepted_disk_generation: u64,
        token: &str,
    ) -> Result<FileExplorerCommitPlan, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "FileExplorerRuntime este indisponibil.".to_string())?;
        bind_session(&mut state, runtime_session_id);
        let stored = state.plans.remove(token).ok_or_else(|| {
            "FileExplorer Commit a refuzat un token absent, expirat sau deja consumat.".to_string()
        })?;
        if stored.workspace_revision != workspace_revision
            || stored.accepted_disk_generation != accepted_disk_generation
        {
            return Err(
                "FileExplorer Commit a refuzat planul deoarece Workspace sau disk generation s-au schimbat."
                    .to_string(),
            );
        }
        Ok(stored.commit)
    }

    pub(crate) fn record_history_topology(
        &self,
        runtime_session_id: &str,
        transaction_id: &str,
        commit: &FileExplorerCommitPlan,
        selection_before: Option<WorkbenchProjectEntrySelection>,
    ) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "FileExplorerRuntime este indisponibil.".to_string())?;
        bind_session(&mut state, runtime_session_id);
        state.history_topology.insert(
            transaction_id.to_string(),
            FileExplorerHistoryTopology {
                request: commit.request.clone(),
                source_path: commit.source_path.clone(),
                destination_path: commit.destination_path.clone(),
                selection_before,
            },
        );
        Ok(())
    }

    pub(crate) fn history_reconciliation(
        &self,
        runtime_session_id: &str,
        transaction_id: &str,
        undo: bool,
    ) -> Result<Option<FileExplorerHistoryReconciliation>, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "FileExplorerRuntime este indisponibil.".to_string())?;
        bind_session(&mut state, runtime_session_id);
        let Some(topology) = state.history_topology.get(transaction_id) else {
            return Ok(None);
        };
        let (remap, deleted_prefix, selection_override) = match (
            topology.source_path.as_ref(),
            topology.destination_path.as_ref(),
        ) {
            (Some(source), Some(destination)) => {
                let (from, to) = if undo {
                    (destination.clone(), source.clone())
                } else {
                    (source.clone(), destination.clone())
                };
                (Some((from, to)), None, None)
            }
            (Some(source), None) => {
                if undo {
                    (None, None, topology.selection_before.clone())
                } else {
                    (None, Some(source.clone()), None)
                }
            }
            (None, Some(destination)) => {
                if undo {
                    (None, Some(destination.clone()), None)
                } else {
                    let selection_override = match &topology.request {
                        FileExplorerOperationRequest::Create { entry_kind, .. } => {
                            Some(WorkbenchProjectEntrySelection {
                                relative_path: destination.clone(),
                                kind: (*entry_kind).into(),
                            })
                        }
                        _ => None,
                    };
                    (None, None, selection_override)
                }
            }
            (None, None) => (None, None, None),
        };
        Ok(Some(FileExplorerHistoryReconciliation {
            remap,
            deleted_prefix,
            selection_override,
        }))
    }
}

fn bind_session(state: &mut FileExplorerState, runtime_session_id: &str) {
    if state.runtime_session_id != runtime_session_id {
        state.runtime_session_id = runtime_session_id.to_string();
        state.next_entry_id = 0;
        state.next_plan_id = 0;
        state.ids_by_path.clear();
        state.plans.clear();
        state.history_topology.clear();
    }
}

type EvaluatedOperation = (
    Option<String>,
    Option<String>,
    Vec<String>,
    Vec<String>,
    Vec<String>,
);

fn evaluate_operation(
    snapshot: &FileExplorerSnapshot,
    namespace: &BTreeSet<String>,
    request: FileExplorerOperationRequest,
) -> Result<EvaluatedOperation, (FileExplorerOperationReason, String)> {
    match request {
        FileExplorerOperationRequest::Create {
            parent_entry_id,
            entry_kind,
            name,
        } => {
            if entry_kind == FileExplorerEntryKind::Binary {
                return Err((
                    FileExplorerOperationReason::UnsupportedEntryKind,
                    "Crearea resurselor binare cere fluxul Assets, nu un fișier gol.".to_string(),
                ));
            }
            let parent = resolve_target_directory(snapshot, parent_entry_id.as_deref())?;
            if let Some(parent_id) = parent_entry_id.as_deref() {
                let parent_entry = require_entry(snapshot, parent_id)?;
                if !parent_entry.capabilities.create_child.allowed {
                    return Err((
                        FileExplorerOperationReason::UnsupportedEntryKind,
                        "Rust a blocat crearea unui copil în această intrare.".to_string(),
                    ));
                }
            }
            let name = valid_leaf_name(&name)?;
            let destination = join_relative(parent.as_deref().unwrap_or(""), &name);
            require_editable_path(&destination)?;
            require_destination_free(namespace, None, &destination)?;
            let namespace_path = if entry_kind == FileExplorerEntryKind::Directory {
                format!("{destination}/.gitkeep")
            } else {
                destination.clone()
            };
            Ok((
                None,
                Some(destination.clone()),
                vec![namespace_path],
                Vec::new(),
                vec![destination],
            ))
        }
        FileExplorerOperationRequest::Rename { entry_id, new_name } => {
            let source = require_entry(snapshot, &entry_id)?;
            if !source.capabilities.rename.allowed {
                return Err((
                    FileExplorerOperationReason::UnsupportedEntryKind,
                    "Rust nu poate redenumi această intrare în starea curentă.".to_string(),
                ));
            }
            let new_name = valid_leaf_name(&new_name)?;
            let parent = parent_path(&source.relative_path);
            let destination = join_relative(&parent, &new_name);
            evaluate_relocation(snapshot, namespace, source, &destination)
        }
        FileExplorerOperationRequest::Move {
            entry_id,
            target_directory_entry_id,
        } => {
            let source = require_entry(snapshot, &entry_id)?;
            if !source.capabilities.move_entry.allowed {
                return Err((
                    FileExplorerOperationReason::UnsupportedEntryKind,
                    "Rust nu poate muta această intrare în starea curentă.".to_string(),
                ));
            }
            let target = resolve_target_directory(snapshot, target_directory_entry_id.as_deref())?;
            let target = target.unwrap_or_default();
            let destination = join_relative(&target, &source.name);
            if parent_path(&source.relative_path) == target {
                return Err((
                    FileExplorerOperationReason::SameParent,
                    "Intrarea se află deja în directorul țintă.".to_string(),
                ));
            }
            if source.kind == FileExplorerEntryKind::Directory
                && (target == source.relative_path
                    || target.starts_with(&format!("{}/", source.relative_path)))
            {
                return Err((
                    FileExplorerOperationReason::DescendantTarget,
                    "Un director nu poate fi mutat în el însuși sau într-un descendent."
                        .to_string(),
                ));
            }
            evaluate_relocation(snapshot, namespace, source, &destination)
        }
        FileExplorerOperationRequest::Delete { entry_id } => {
            let source = require_entry(snapshot, &entry_id)?;
            if !source.capabilities.delete.allowed {
                return Err((
                    FileExplorerOperationReason::UnsupportedEntryKind,
                    "Rust nu poate șterge această intrare în starea curentă.".to_string(),
                ));
            }
            require_editable_path(&source.relative_path)?;
            let namespace_paths = namespace_subtree(namespace, &source.relative_path);
            if namespace_paths.is_empty() {
                return Err((
                    FileExplorerOperationReason::MissingSource,
                    "Intrarea nu mai are resurse materializate în Workspace.".to_string(),
                ));
            }
            let affected = snapshot
                .entries
                .iter()
                .filter(|entry| path_in_subtree(&entry.relative_path, &source.relative_path))
                .collect::<Vec<_>>();
            Ok((
                Some(source.relative_path.clone()),
                None,
                namespace_paths,
                affected.iter().map(|entry| entry.id.clone()).collect(),
                affected
                    .iter()
                    .map(|entry| entry.relative_path.clone())
                    .collect(),
            ))
        }
    }
}

fn evaluate_relocation(
    snapshot: &FileExplorerSnapshot,
    namespace: &BTreeSet<String>,
    source: &FileExplorerEntry,
    destination: &str,
) -> Result<EvaluatedOperation, (FileExplorerOperationReason, String)> {
    require_editable_path(&source.relative_path)?;
    require_editable_path(destination)?;
    if source.relative_path == destination {
        return Err((
            FileExplorerOperationReason::SameParent,
            "Operația nu schimbă path-ul intrării.".to_string(),
        ));
    }
    require_destination_free(namespace, Some(&source.relative_path), destination)?;
    let namespace_paths = namespace_subtree(namespace, &source.relative_path);
    if namespace_paths.is_empty() {
        return Err((
            FileExplorerOperationReason::MissingSource,
            "Intrarea nu mai are resurse materializate în Workspace.".to_string(),
        ));
    }
    let affected = snapshot
        .entries
        .iter()
        .filter(|entry| path_in_subtree(&entry.relative_path, &source.relative_path))
        .collect::<Vec<_>>();
    Ok((
        Some(source.relative_path.clone()),
        Some(destination.to_string()),
        namespace_paths,
        affected.iter().map(|entry| entry.id.clone()).collect(),
        affected
            .iter()
            .map(|entry| remap_prefix(&entry.relative_path, &source.relative_path, destination))
            .collect(),
    ))
}

fn blocked_plan(
    snapshot: &FileExplorerSnapshot,
    reason: FileExplorerOperationReason,
    diagnostic: String,
) -> FileExplorerOperationPlan {
    FileExplorerOperationPlan {
        schema_version: FILE_EXPLORER_SCHEMA_VERSION,
        project_root: snapshot.project_root.clone(),
        runtime_session_id: snapshot.runtime_session_id.clone(),
        workspace_revision: snapshot.workspace_revision,
        accepted_disk_generation: snapshot.accepted_disk_generation,
        allowed: false,
        reason: Some(reason),
        diagnostic: Some(diagnostic),
        commit_token: None,
        destination_path: None,
        affected_entry_ids: Vec::new(),
        affected_paths: Vec::new(),
    }
}

fn materialized_namespace(projection: &WorkspaceProjectionSnapshot) -> BTreeSet<String> {
    let mut paths = projection
        .accepted_disk
        .manifest
        .files
        .iter()
        .map(|entry| entry.relative_path.clone())
        .collect::<BTreeSet<_>>();
    for deleted in &projection.deleted_sources {
        paths.remove(deleted);
    }
    paths.extend(projection.source_texts.keys().cloned());
    paths.extend(projection.resource_bytes.keys().cloned());
    paths
}

fn namespace_contains_entry(
    namespace: &BTreeSet<String>,
    relative_path: &str,
    kind: WorkbenchProjectEntryKind,
) -> bool {
    match kind {
        WorkbenchProjectEntryKind::Text | WorkbenchProjectEntryKind::Binary => {
            namespace.contains(relative_path)
        }
        WorkbenchProjectEntryKind::Directory => namespace
            .iter()
            .any(|path| path.starts_with(&format!("{relative_path}/"))),
    }
}

fn require_entry<'a>(
    snapshot: &'a FileExplorerSnapshot,
    id: &str,
) -> Result<&'a FileExplorerEntry, (FileExplorerOperationReason, String)> {
    snapshot.entries.iter().find(|entry| entry.id == id).ok_or((
        if snapshot.truncated {
            FileExplorerOperationReason::TruncatedSnapshot
        } else {
            FileExplorerOperationReason::MissingSource
        },
        "FileExplorer nu mai găsește intrarea în snapshotul curent.".to_string(),
    ))
}

fn resolve_target_directory(
    snapshot: &FileExplorerSnapshot,
    id: Option<&str>,
) -> Result<Option<String>, (FileExplorerOperationReason, String)> {
    let Some(id) = id else {
        return Ok(None);
    };
    let entry = snapshot
        .entries
        .iter()
        .find(|entry| entry.id == id)
        .ok_or((
            FileExplorerOperationReason::MissingTarget,
            "Directorul țintă nu mai există în snapshotul curent.".to_string(),
        ))?;
    if entry.kind != FileExplorerEntryKind::Directory {
        return Err((
            FileExplorerOperationReason::TargetNotDirectory,
            "Ținta operației trebuie să fie un director.".to_string(),
        ));
    }
    Ok(Some(entry.relative_path.clone()))
}

fn valid_leaf_name(raw: &str) -> Result<String, (FileExplorerOperationReason, String)> {
    let name = raw.trim();
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
        || Path::new(name).components().count() != 1
    {
        return Err((
            FileExplorerOperationReason::InvalidName,
            "Numele trebuie să fie o singură componentă de path validă.".to_string(),
        ));
    }
    Ok(name.to_string())
}

fn require_editable_path(path: &str) -> Result<(), (FileExplorerOperationReason, String)> {
    const PROTECTED: &[&str] = &[
        ".git",
        ".svelte-kit",
        "build",
        "dist",
        "node_modules",
        "target",
        ".panastudio_preview",
        ".panastudio",
    ];
    if path
        .split('/')
        .any(|component| PROTECTED.contains(&component))
    {
        return Err((
            FileExplorerOperationReason::ProtectedPath,
            "Path-ul aparține unei zone interne sau generate și nu poate fi modificat.".to_string(),
        ));
    }
    Ok(())
}

fn require_destination_free(
    namespace: &BTreeSet<String>,
    source: Option<&str>,
    destination: &str,
) -> Result<(), (FileExplorerOperationReason, String)> {
    let conflict = namespace.iter().any(|path| {
        if source.is_some_and(|source| path_in_subtree(path, source)) {
            return false;
        }
        path == destination
            || path.starts_with(&format!("{destination}/"))
            || destination.starts_with(&format!("{path}/"))
    });
    if conflict {
        return Err((
            FileExplorerOperationReason::DestinationConflict,
            format!("Destinația {destination} există deja sau se suprapune peste o resursă."),
        ));
    }
    Ok(())
}

fn namespace_subtree(namespace: &BTreeSet<String>, source: &str) -> Vec<String> {
    namespace
        .iter()
        .filter(|path| path_in_subtree(path, source))
        .cloned()
        .collect()
}

fn path_in_subtree(path: &str, root: &str) -> bool {
    path == root || path.starts_with(&format!("{root}/"))
}

fn remap_prefix(path: &str, source: &str, destination: &str) -> String {
    if path == source {
        destination.to_string()
    } else {
        format!("{destination}{}", &path[source.len()..])
    }
}

fn parent_path(path: &str) -> String {
    path.rsplit_once('/')
        .map(|(parent, _)| parent.to_string())
        .unwrap_or_default()
}

fn join_relative(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}/{name}")
    }
}

pub(crate) fn initial_text_for_path(relative_path: &str) -> String {
    let name = relative_path.rsplit('/').next().unwrap_or(relative_path);
    let stem = name
        .strip_suffix(".html")
        .unwrap_or(name)
        .trim_start_matches('_');
    if relative_path.starts_with("content/") {
        if name == "_index.md" {
            return "+++\ntitle = \"\"\ntemplate = \"section.html\"\n+++\n".to_string();
        }
        if name.ends_with(".md") {
            return "+++\ntitle = \"\"\n+++\n".to_string();
        }
    }
    if relative_path.starts_with("templates/partials/") && name.ends_with(".html") {
        let stem = if stem.is_empty() { "partial" } else { stem };
        return format!("<section class=\"{stem}\">\n  <h2>{stem}</h2>\n</section>\n");
    }
    if relative_path.starts_with("templates/macros/") && name.ends_with(".html") {
        let stem = if stem.is_empty() { "partial" } else { stem };
        return format!("{{% macro {stem}() %}}\n{{% endmacro %}}\n");
    }
    if relative_path.starts_with("templates/") && name.ends_with(".html") {
        return "{% extends \"base.html\" %}\n\n{% block content %}\n{% endblock %}\n".to_string();
    }
    String::new()
}

fn entry_id(state: &mut FileExplorerState, path: &str) -> Result<String, String> {
    if let Some(id) = state.ids_by_path.get(path) {
        return Ok(id.clone());
    }
    state.next_entry_id = state.next_entry_id.checked_add(1).ok_or_else(|| {
        "FileExplorer a epuizat spațiul identificatorilor de intrare.".to_string()
    })?;
    let id = format!("project-entry:{}", state.next_entry_id);
    state.ids_by_path.insert(path.to_string(), id.clone());
    Ok(id)
}

fn build_entry(
    file: ProjectFile,
    projection: &WorkspaceProjectionSnapshot,
    ids: &BTreeMap<String, String>,
) -> Result<FileExplorerEntry, String> {
    let id = ids
        .get(&file.relative_path)
        .cloned()
        .ok_or_else(|| "FileExplorer nu a alocat identitatea intrării.".to_string())?;
    let parent_path = file
        .relative_path
        .rsplit_once('/')
        .map(|(parent, _)| parent.to_string());
    let parent_id = parent_path
        .as_ref()
        .and_then(|parent| ids.get(parent))
        .cloned();
    let depth = file.relative_path.matches('/').count();
    let kind = if file.kind == ProjectFileKind::Dir {
        FileExplorerEntryKind::Directory
    } else if projection.source_texts.contains_key(&file.relative_path) {
        FileExplorerEntryKind::Text
    } else {
        FileExplorerEntryKind::Binary
    };
    let role = match file.role {
        ProjectFileRole::Page => FileExplorerRole::Page,
        ProjectFileRole::Template => FileExplorerRole::Template,
        ProjectFileRole::Style => FileExplorerRole::Style,
        ProjectFileRole::Script => FileExplorerRole::Script,
        ProjectFileRole::Asset => FileExplorerRole::Asset,
    };
    let open_surface = match kind {
        FileExplorerEntryKind::Text if file.kind == ProjectFileKind::Md => {
            Some(WorkbenchSurface::Code)
        }
        FileExplorerEntryKind::Text
            if role == FileExplorerRole::Template
                || (file.kind == ProjectFileKind::Html && file.preview_path.is_some()) =>
        {
            Some(WorkbenchSurface::Visual)
        }
        FileExplorerEntryKind::Text => Some(WorkbenchSurface::Code),
        FileExplorerEntryKind::Directory | FileExplorerEntryKind::Binary => None,
    };
    let capabilities = entry_capabilities(kind, &file.relative_path, projection);
    Ok(FileExplorerEntry {
        id,
        parent_id,
        name: file.name,
        relative_path: file.relative_path,
        depth,
        kind,
        role,
        preview_path: file.preview_path,
        open_surface,
        capabilities,
    })
}

fn allowed() -> FileExplorerCapability {
    FileExplorerCapability {
        allowed: true,
        reason: None,
    }
}

fn blocked(reason: FileExplorerCapabilityReason) -> FileExplorerCapability {
    FileExplorerCapability {
        allowed: false,
        reason: Some(reason),
    }
}

fn entry_capabilities(
    kind: FileExplorerEntryKind,
    relative_path: &str,
    projection: &WorkspaceProjectionSnapshot,
) -> FileExplorerCapabilities {
    match kind {
        FileExplorerEntryKind::Text => FileExplorerCapabilities {
            open: allowed(),
            create_child: blocked(FileExplorerCapabilityReason::NotDocument),
            rename: allowed(),
            move_entry: allowed(),
            delete: allowed(),
        },
        FileExplorerEntryKind::Binary if binary_subtree_is_mutable(projection, relative_path) => {
            FileExplorerCapabilities {
                open: blocked(FileExplorerCapabilityReason::BinaryEditorUnavailable),
                create_child: blocked(FileExplorerCapabilityReason::NotDocument),
                rename: allowed(),
                move_entry: allowed(),
                delete: allowed(),
            }
        }
        FileExplorerEntryKind::Binary => FileExplorerCapabilities {
            open: blocked(FileExplorerCapabilityReason::BinaryEditorUnavailable),
            create_child: blocked(FileExplorerCapabilityReason::NotDocument),
            rename: blocked(FileExplorerCapabilityReason::BinaryMutationUnavailable),
            move_entry: blocked(FileExplorerCapabilityReason::BinaryMutationUnavailable),
            delete: blocked(FileExplorerCapabilityReason::BinaryMutationUnavailable),
        },
        FileExplorerEntryKind::Directory
            if binary_subtree_is_mutable(projection, relative_path) =>
        {
            FileExplorerCapabilities {
                open: blocked(FileExplorerCapabilityReason::NotDocument),
                create_child: allowed(),
                rename: allowed(),
                move_entry: allowed(),
                delete: allowed(),
            }
        }
        FileExplorerEntryKind::Directory => FileExplorerCapabilities {
            open: blocked(FileExplorerCapabilityReason::NotDocument),
            create_child: allowed(),
            rename: blocked(FileExplorerCapabilityReason::DirectoryMutationUnavailable),
            move_entry: blocked(FileExplorerCapabilityReason::DirectoryMutationUnavailable),
            delete: blocked(FileExplorerCapabilityReason::DirectoryMutationUnavailable),
        },
    }
}

fn binary_subtree_is_mutable(
    projection: &WorkspaceProjectionSnapshot,
    relative_path: &str,
) -> bool {
    let accepted = projection
        .accepted_disk
        .manifest
        .files
        .iter()
        .map(|entry| (entry.relative_path.as_str(), entry.size))
        .collect::<HashMap<_, _>>();
    let paths = materialized_namespace(projection);
    let mut total = projection
        .resource_bytes
        .iter()
        .filter(|(path, _)| !path_in_subtree(path, relative_path))
        .map(|(_, bytes)| bytes.len() as u64)
        .sum::<u64>();
    for path in paths
        .iter()
        .filter(|path| path_in_subtree(path, relative_path))
    {
        if projection.source_texts.contains_key(path) {
            continue;
        }
        let Some(accepted_size) = accepted.get(path.as_str()).copied() else {
            return false;
        };
        let current_size = projection
            .resource_bytes
            .get(path)
            .map(|bytes| bytes.len() as u64)
            .unwrap_or(accepted_size);
        if accepted_size > PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_BYTES
            || current_size > PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_BYTES
        {
            return false;
        }
        total = match total.checked_add(current_size) {
            Some(total) => total,
            None => return false,
        };
    }
    total <= PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_TOTAL_BYTES
}

fn root_capabilities() -> FileExplorerCapabilities {
    FileExplorerCapabilities {
        open: blocked(FileExplorerCapabilityReason::RootEntry),
        create_child: allowed(),
        rename: blocked(FileExplorerCapabilityReason::RootEntry),
        move_entry: blocked(FileExplorerCapabilityReason::RootEntry),
        delete: blocked(FileExplorerCapabilityReason::RootEntry),
    }
}

fn active_document_path(snapshot: &WorkbenchSnapshot) -> Option<String> {
    let group = snapshot
        .groups
        .iter()
        .find(|group| group.group_id == snapshot.active_group_id)?;
    let active = group.active_document_id.as_deref()?;
    group
        .documents
        .iter()
        .find(|document| document.document_id == active)
        .map(|document| document.relative_path.clone())
}

fn hierarchy_order(entries: Vec<FileExplorerEntry>) -> Result<Vec<FileExplorerEntry>, String> {
    let expected_len = entries.len();
    let mut children = HashMap::<Option<String>, Vec<FileExplorerEntry>>::new();
    for entry in entries {
        children
            .entry(entry.parent_id.clone())
            .or_default()
            .push(entry);
    }
    for siblings in children.values_mut() {
        siblings.sort_by(|left, right| {
            let left_rank = usize::from(left.kind != FileExplorerEntryKind::Directory);
            let right_rank = usize::from(right.kind != FileExplorerEntryKind::Directory);
            left_rank
                .cmp(&right_rank)
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
                .then_with(|| left.relative_path.cmp(&right.relative_path))
        });
    }
    fn append_children(
        parent_id: Option<String>,
        children: &mut HashMap<Option<String>, Vec<FileExplorerEntry>>,
        output: &mut Vec<FileExplorerEntry>,
    ) {
        let Some(siblings) = children.remove(&parent_id) else {
            return;
        };
        for entry in siblings {
            let id = entry.id.clone();
            output.push(entry);
            append_children(Some(id), children, output);
        }
    }
    let mut output = Vec::with_capacity(expected_len);
    append_children(None, &mut children, &mut output);
    if output.len() != expected_len {
        return Err("FileExplorer a detectat o ierarhie cu părinți lipsă sau ciclică.".to_string());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        kernel::workbench::{
            WorkbenchActivity, WorkbenchBottomPanelSnapshot, WorkbenchBottomPanelView,
            WorkbenchGroupId, WorkbenchGroupSnapshot, WorkbenchSplit, WORKBENCH_SCHEMA_VERSION,
        },
        project::{AcceptedProjectDiskManifest, ProjectDiskManifest},
    };
    use std::collections::{HashMap, HashSet};

    fn projection(source_texts: HashMap<String, String>) -> WorkspaceProjectionSnapshot {
        let project_root = "/project".to_string();
        let runtime_session_id = "runtime-1".to_string();
        let accepted_disk = AcceptedProjectDiskManifest::new(
            runtime_session_id.clone(),
            project_root.clone(),
            ProjectDiskManifest {
                root: project_root.clone(),
                files: Vec::new(),
                truncated: false,
                max_files: 1_000,
            },
        )
        .unwrap();
        WorkspaceProjectionSnapshot {
            project_root,
            runtime_session_id,
            revision: 7,
            workspace_transaction_id: None,
            source_texts,
            resource_bytes: HashMap::new(),
            deleted_sources: HashSet::new(),
            changed_paths: HashSet::new(),
            accepted_disk,
        }
    }

    fn workbench(selection: Option<WorkbenchProjectEntrySelection>) -> WorkbenchSnapshot {
        WorkbenchSnapshot {
            schema_version: WORKBENCH_SCHEMA_VERSION,
            project_root: "/project".to_string(),
            project_session_id: "project-1".to_string(),
            runtime_session_id: "runtime-1".to_string(),
            revision: 3,
            active_activity: WorkbenchActivity::Editor,
            active_group_id: WorkbenchGroupId::Primary,
            split: WorkbenchSplit::None,
            split_ratio_basis_points: 5_000,
            canvas_viewport: Default::default(),
            groups: vec![WorkbenchGroupSnapshot {
                group_id: WorkbenchGroupId::Primary,
                documents: Vec::new(),
                active_document_id: None,
            }],
            bottom_panel: WorkbenchBottomPanelSnapshot {
                open: false,
                active_view: WorkbenchBottomPanelView::Problems,
            },
            content_workspace: Default::default(),
            selected_project_entry: selection,
        }
    }

    fn entry(
        id: &str,
        parent_id: Option<&str>,
        path: &str,
        kind: FileExplorerEntryKind,
    ) -> FileExplorerEntry {
        FileExplorerEntry {
            id: id.to_string(),
            parent_id: parent_id.map(str::to_string),
            name: path.rsplit('/').next().unwrap_or(path).to_string(),
            relative_path: path.to_string(),
            depth: path.matches('/').count(),
            kind,
            role: FileExplorerRole::Asset,
            preview_path: None,
            open_surface: None,
            capabilities: entry_capabilities(kind, path, &projection(HashMap::new())),
        }
    }

    fn operation_snapshot(entries: Vec<FileExplorerEntry>) -> FileExplorerSnapshot {
        FileExplorerSnapshot {
            schema_version: FILE_EXPLORER_SCHEMA_VERSION,
            project_root: "/project".to_string(),
            runtime_session_id: "runtime-1".to_string(),
            workspace_revision: 7,
            accepted_disk_generation: 1,
            workbench_revision: 3,
            selection_revision: 3,
            selected_entry: None,
            active_document_path: None,
            entries,
            root_capabilities: root_capabilities(),
            truncated: false,
            max_entries: PROJECT_SCAN_MAX_ENTRIES,
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn remap_preserves_entry_identity_for_a_directory_subtree() {
        let runtime = FileExplorerRuntime::default();
        {
            let mut state = runtime.state.lock().unwrap();
            bind_session(&mut state, "runtime-1");
            let parent = entry_id(&mut state, "templates/partials").unwrap();
            let child = entry_id(&mut state, "templates/partials/header.html").unwrap();
            assert_ne!(parent, child);
        }

        runtime
            .remap_entry_prefix("runtime-1", "templates/partials", "templates/components")
            .unwrap();

        let state = runtime.state.lock().unwrap();
        assert!(state.ids_by_path.contains_key("templates/components"));
        assert!(state
            .ids_by_path
            .contains_key("templates/components/header.html"));
        assert!(!state.ids_by_path.contains_key("templates/partials"));
    }

    #[test]
    fn empty_directory_marker_is_hidden_but_directory_is_projected() {
        let runtime = FileExplorerRuntime::default();
        let projection = projection(HashMap::from([(
            "templates/partials/.gitkeep".to_string(),
            String::new(),
        )]));
        let snapshot = runtime.snapshot(&projection, &workbench(None)).unwrap();
        let paths = snapshot
            .entries
            .iter()
            .map(|entry| entry.relative_path.as_str())
            .collect::<BTreeSet<_>>();
        assert!(paths.contains("templates"));
        assert!(paths.contains("templates/partials"));
        assert!(!paths.contains("templates/partials/.gitkeep"));
    }

    #[test]
    fn staged_extensionless_text_file_is_projected_as_a_file() {
        let runtime = FileExplorerRuntime::default();
        let projection = projection(HashMap::from([(
            "static/CNAME".to_string(),
            "example.test".to_string(),
        )]));
        let snapshot = runtime.snapshot(&projection, &workbench(None)).unwrap();
        let entry = snapshot
            .entries
            .iter()
            .find(|entry| entry.relative_path == "static/CNAME")
            .expect("extensionless staged text must remain visible");
        assert_eq!(entry.kind, FileExplorerEntryKind::Text);
        assert_eq!(entry.open_surface, Some(WorkbenchSurface::Code));
    }

    #[test]
    fn snapshot_truncation_is_explicit_and_hierarchy_safe() {
        let runtime = FileExplorerRuntime::default();
        let source_texts = (0..(PROJECT_SCAN_MAX_ENTRIES + 20))
            .map(|index| (format!("content/page-{index:04}.md"), String::new()))
            .collect();
        let snapshot = runtime
            .snapshot(&projection(source_texts), &workbench(None))
            .unwrap();
        assert!(snapshot.truncated);
        assert_eq!(snapshot.entries.len(), PROJECT_SCAN_MAX_ENTRIES);
        assert!(!snapshot.diagnostics.is_empty());
        let ids = snapshot
            .entries
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<BTreeSet<_>>();
        assert!(snapshot.entries.iter().all(|entry| entry
            .parent_id
            .as_deref()
            .is_none_or(|parent| ids.contains(parent))));
    }

    #[test]
    fn operation_planner_rejects_invalid_conflicting_protected_and_descendant_paths() {
        let snapshot = operation_snapshot(vec![
            entry(
                "templates",
                None,
                "templates",
                FileExplorerEntryKind::Directory,
            ),
            entry(
                "partials",
                Some("templates"),
                "templates/partials",
                FileExplorerEntryKind::Directory,
            ),
            entry(
                "header",
                Some("partials"),
                "templates/partials/header.html",
                FileExplorerEntryKind::Text,
            ),
            entry(
                "protected",
                None,
                "node_modules",
                FileExplorerEntryKind::Directory,
            ),
        ]);
        let namespace = BTreeSet::from([
            "templates/partials/header.html".to_string(),
            "node_modules/pkg/index.js".to_string(),
        ]);

        assert!(matches!(
            evaluate_operation(
                &snapshot,
                &namespace,
                FileExplorerOperationRequest::Create {
                    parent_entry_id: None,
                    entry_kind: FileExplorerEntryKind::Text,
                    name: "../escape".to_string(),
                },
            ),
            Err((FileExplorerOperationReason::InvalidName, _))
        ));
        assert!(matches!(
            evaluate_operation(
                &snapshot,
                &namespace,
                FileExplorerOperationRequest::Move {
                    entry_id: "templates".to_string(),
                    target_directory_entry_id: Some("partials".to_string()),
                },
            ),
            Err((FileExplorerOperationReason::DescendantTarget, _))
        ));
        assert!(matches!(
            evaluate_operation(
                &snapshot,
                &namespace,
                FileExplorerOperationRequest::Create {
                    parent_entry_id: Some("partials".to_string()),
                    entry_kind: FileExplorerEntryKind::Text,
                    name: "header.html".to_string(),
                },
            ),
            Err((FileExplorerOperationReason::DestinationConflict, _))
        ));
        assert!(matches!(
            evaluate_operation(
                &snapshot,
                &namespace,
                FileExplorerOperationRequest::Delete {
                    entry_id: "protected".to_string(),
                },
            ),
            Err((FileExplorerOperationReason::ProtectedPath, _))
        ));
    }

    #[test]
    fn missing_workbench_paths_preserves_existing_directories_and_invalidates_missing_files() {
        let runtime = FileExplorerRuntime::default();
        let projection = projection(HashMap::from([(
            "templates/partials/header.html".to_string(),
            String::new(),
        )]));
        let existing_directory = workbench(Some(WorkbenchProjectEntrySelection {
            relative_path: "templates/partials".to_string(),
            kind: WorkbenchProjectEntryKind::Directory,
        }));
        assert!(runtime
            .missing_workbench_paths(&projection, &existing_directory)
            .unwrap()
            .is_empty());

        let missing_file = workbench(Some(WorkbenchProjectEntrySelection {
            relative_path: "templates/missing.html".to_string(),
            kind: WorkbenchProjectEntryKind::Text,
        }));
        assert_eq!(
            runtime
                .missing_workbench_paths(&projection, &missing_file)
                .unwrap(),
            vec!["templates/missing.html".to_string()]
        );
    }
}
