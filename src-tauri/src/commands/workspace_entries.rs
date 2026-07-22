use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::{
    kernel::{
        file_buffer_store::{require_file_buffer_session_binding, FileBufferRequestIdentity},
        observability::now_ms,
        project_path::normalize_project_relative_path,
        project_workspace::{
            commit_project_workspace_session_mutation, ProjectWorkspace, ProjectWorkspaceIdentity,
            ProjectWorkspaceMutationReceipt, ProjectWorkspaceSnapshot, WorkspaceMutationMetadata,
            WorkspaceResourceDelete, WorkspaceResourceMutation,
        },
    },
    project::{build_content_page_draft_with_active_theme, zola_project_root},
    state::AppState,
    zola_theme::active_theme_from_source,
};

pub const WORKSPACE_ENTRY_MUTATION_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEntryMutationReceipt {
    pub schema_version: u32,
    pub project_root: String,
    pub runtime_session_id: String,
    pub relative_path: Option<String>,
    pub mutation: ProjectWorkspaceMutationReceipt,
    pub workspace: ProjectWorkspaceSnapshot,
}

pub(crate) fn current_workspace_identity(workspace: &ProjectWorkspace) -> ProjectWorkspaceIdentity {
    ProjectWorkspaceIdentity {
        expected_project_root: workspace.session.project_root.clone(),
        expected_session_id: workspace.runtime_session_id(),
        expected_revision: workspace.revision,
    }
}

pub(crate) fn mutation_metadata(label: &str, source: &str) -> WorkspaceMutationMetadata {
    WorkspaceMutationMetadata {
        label: label.to_string(),
        source: source.to_string(),
        coalesce_key: None,
        transaction_id: None,
    }
}

pub(crate) fn require_bound_workspace<'a>(
    state: &'a AppState,
    identity: &FileBufferRequestIdentity,
) -> Result<(PathBuf, std::sync::MutexGuard<'a, Option<ProjectWorkspace>>), String> {
    let root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul curent pentru operația de workspace.".to_string())?
        .clone()
        .ok_or_else(|| "Nu există proiect curent pentru operația de workspace.".to_string())?;
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru operația de fișier.".to_string())?;
    let live = workspace
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    let root_string = root.to_string_lossy().into_owned();
    require_file_buffer_session_binding(&root_string, &live.session, &live.documents, identity)?;
    live.accepted_disk.require_live_complete(
        &live.runtime_session_id(),
        &live.session.project_root,
        &root,
    )?;
    Ok((root, workspace))
}

pub(crate) fn finish_mutation(
    app: &AppHandle,
    workspace: &mut ProjectWorkspace,
    relative_path: Option<String>,
    mutate: impl FnOnce(&mut ProjectWorkspace) -> Result<ProjectWorkspaceMutationReceipt, String>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let mutation = commit_project_workspace_session_mutation(app, workspace, mutate)?;
    Ok(WorkspaceEntryMutationReceipt {
        schema_version: WORKSPACE_ENTRY_MUTATION_SCHEMA_VERSION,
        project_root: workspace.session.project_root.clone(),
        runtime_session_id: workspace.runtime_session_id(),
        relative_path,
        mutation,
        workspace: workspace.snapshot(),
    })
}

fn require_text_resource(
    workspace: &ProjectWorkspace,
    relative_path: &str,
) -> Result<String, String> {
    workspace.documents.text_for(relative_path).ok_or_else(|| {
        format!(
            "ProjectWorkspace poate muta, redenumi sau șterge numai resurse text urmărite; {} nu este o astfel de resursă.",
            relative_path
        )
    })
}

fn require_destination_available(
    workspace: &ProjectWorkspace,
    source: &str,
    destination: &str,
) -> Result<(), String> {
    if source == destination {
        return Err("Operația de fișier nu schimbă path-ul resursei.".to_string());
    }
    if workspace.documents.files.contains_key(destination) {
        return Err(format!(
            "ProjectWorkspace a blocat operația: destinația {} există deja în sesiune.",
            destination
        ));
    }
    Ok(())
}

fn stage_move(
    workspace: &mut ProjectWorkspace,
    source: String,
    destination: String,
    label: &str,
    source_label: &str,
) -> Result<ProjectWorkspaceMutationReceipt, String> {
    let contents = require_text_resource(workspace, &source)?;
    require_destination_available(workspace, &source, &destination)?;
    workspace.stage_composite_changes(
        &current_workspace_identity(workspace),
        mutation_metadata(label, source_label),
        vec![WorkspaceResourceMutation {
            relative_path: destination,
            contents,
            create_only: false,
        }],
        vec![WorkspaceResourceDelete {
            relative_path: source,
        }],
        None,
        now_ms(),
    )
}

#[tauri::command(async)]
pub fn workspace_create_project_text_file(
    relative_path: String,
    contents: String,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let relative_path = normalize_project_relative_path(&relative_path)?;
    let (_root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = slot
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    let receipt_path = relative_path.clone();
    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        candidate.stage_resource_texts(
            &current_workspace_identity(candidate),
            mutation_metadata("Creare fișier", "files.create"),
            vec![WorkspaceResourceMutation {
                relative_path,
                contents,
                create_only: true,
            }],
            now_ms(),
        )
    })
}

#[tauri::command(async)]
pub fn workspace_move_project_entry(
    source_relative_path: String,
    target_directory: String,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let source = normalize_project_relative_path(&source_relative_path)?;
    let target_directory = target_directory.trim().replace('\\', "/");
    let target_directory = if target_directory.is_empty() {
        String::new()
    } else {
        normalize_project_relative_path(&target_directory)?
    };
    let file_name = Path::new(&source)
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .ok_or_else(|| "Path-ul sursă nu are nume de fișier.".to_string())?;
    let destination = if target_directory.is_empty() {
        file_name
    } else {
        format!("{target_directory}/{file_name}")
    };
    let (_root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = slot
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    let receipt_path = destination.clone();
    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        stage_move(
            candidate,
            source,
            destination,
            "Mutare fișier",
            "files.move",
        )
    })
}

#[tauri::command(async)]
pub fn workspace_rename_project_entry(
    source_relative_path: String,
    new_name: String,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let source = normalize_project_relative_path(&source_relative_path)?;
    let normalized_name = normalize_project_relative_path(new_name.trim())?;
    if Path::new(&normalized_name).components().count() != 1 {
        return Err("Redenumirea cere un singur nume, fără directoare.".to_string());
    }
    let destination = Path::new(&source)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(|parent| {
            format!(
                "{}/{}",
                parent.to_string_lossy().replace('\\', "/"),
                normalized_name
            )
        })
        .unwrap_or(normalized_name);
    let (_root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = slot
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    let receipt_path = destination.clone();
    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        stage_move(
            candidate,
            source,
            destination,
            "Redenumire fișier",
            "files.rename",
        )
    })
}

#[tauri::command(async)]
pub fn workspace_delete_project_entry(
    relative_path: String,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let relative_path = normalize_project_relative_path(&relative_path)?;
    let (_root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = slot
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    let receipt_path = relative_path.clone();
    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        require_text_resource(candidate, &relative_path)?;
        candidate.stage_resource_changes(
            &current_workspace_identity(candidate),
            mutation_metadata("Ștergere fișier", "files.delete"),
            Vec::new(),
            vec![WorkspaceResourceDelete { relative_path }],
            now_ms(),
        )
    })
}

#[tauri::command(async)]
pub fn workspace_create_content_page(
    section: String,
    slug: String,
    title: String,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let (root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = slot
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    let active_theme = ["zola.toml", "config.toml"]
        .iter()
        .find_map(|path| workspace.documents.text_for(path))
        .and_then(|source| active_theme_from_source(&source));
    let draft = build_content_page_draft_with_active_theme(
        &zola_project_root(&root),
        &section,
        &slug,
        &title,
        active_theme,
    )?;
    let relative_path = draft.relative_path;
    let receipt_path = relative_path.clone();
    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        candidate.stage_resource_texts(
            &current_workspace_identity(candidate),
            mutation_metadata("Creare pagină", "pages.create"),
            vec![WorkspaceResourceMutation {
                relative_path,
                contents: draft.contents,
                create_only: true,
            }],
            now_ms(),
        )
    })
}
