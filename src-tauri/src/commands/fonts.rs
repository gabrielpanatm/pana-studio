use std::{collections::HashSet, fs, path::Path};
use tauri::{AppHandle, State};

use crate::{
    commands::project::{
        require_current_project_root, require_project_workspace_available_for_write,
    },
    fonts::{
        overlay_staged_font_resources, plan_google_font_family_download, scan_font_inventory,
        search_google_fonts as search_google_fonts_impl, FontInventory, GoogleFontCatalogFamily,
        GoogleFontDownloadFileWrite, GoogleFontDownloadResult,
    },
    kernel::{
        file_buffer_store::hash_bytes,
        project_workspace::{
            commit_project_workspace_session_mutation, ProjectWorkspaceIdentity,
            WorkspaceBinaryResource, WorkspaceMutationMetadata,
        },
    },
    project::{resolve_project_write_path, zola_project_root},
    state::AppState,
};

#[tauri::command]
pub fn get_font_inventory(state: State<AppState>) -> Result<FontInventory, String> {
    let root = zola_project_root(&require_current_project_root(&state)?);
    let workspace = state.project_workspace.lock().map_err(|_| {
        "Nu am putut bloca ProjectWorkspace pentru inventarul fonturilor.".to_string()
    })?;
    let workspace = workspace
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    let deleted = workspace
        .deleted_binary_resources()
        .map(str::to_string)
        .collect::<HashSet<_>>();
    let mut inventory = overlay_staged_font_resources(
        scan_font_inventory(&root),
        workspace
            .staged_binary_resources()
            .map(|(path, bytes)| (path, bytes.len())),
    );
    for family in &mut inventory.families {
        family.files.retain(|file| !deleted.contains(&file.file));
    }
    inventory.families.retain(|family| !family.files.is_empty());
    Ok(inventory)
}

#[tauri::command]
pub fn download_google_font_family(
    family: String,
    weights: Vec<u16>,
    variable: bool,
    app: AppHandle,
    state: State<AppState>,
) -> Result<GoogleFontDownloadResult, String> {
    let project_root = require_current_project_root(&state)?;
    require_project_workspace_available_for_write(&state)?;
    let plan = plan_google_font_family_download(&family, &weights, variable)?;
    let mut workspace_slot = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru fonturi.".to_string())?;
    let workspace = workspace_slot
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    if workspace.session.project_root != project_root.to_string_lossy() {
        return Err("Font Manager a refuzat un ProjectWorkspace din alt proiect.".to_string());
    }
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        &project_root,
    )?;
    let resources = plan
        .writes
        .iter()
        .filter_map(|write| {
            match font_resource_already_available(workspace, &project_root, write) {
                Ok(true) => None,
                Ok(false) => Some(Ok(WorkspaceBinaryResource::new(
                    write.project_relative_path.clone(),
                    write.bytes.clone(),
                ))),
                Err(error) => Some(Err(error)),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !resources.is_empty() {
        commit_project_workspace_session_mutation(&app, workspace, |candidate| {
            let identity = ProjectWorkspaceIdentity {
                expected_project_root: candidate.session.project_root.clone(),
                expected_session_id: candidate.runtime_session_id(),
                expected_revision: candidate.revision,
            };
            candidate.stage_binary_resource_creates(
                &identity,
                WorkspaceMutationMetadata {
                    label: format!("Descarcă fontul {}", plan.result.family.family),
                    source: "project_workspace.font_download".to_string(),
                    coalesce_key: None,
                    transaction_id: None,
                },
                resources,
                crate::kernel::observability::now_ms(),
            )
        })?;
    }

    Ok(plan.result)
}

#[tauri::command]
pub fn search_google_fonts(
    query: String,
    limit: Option<usize>,
    offset: Option<usize>,
    _state: State<AppState>,
) -> Result<Vec<GoogleFontCatalogFamily>, String> {
    search_google_fonts_impl(&query, limit.unwrap_or(40), offset.unwrap_or(0))
}

fn font_resource_already_available(
    workspace: &crate::kernel::project_workspace::ProjectWorkspace,
    project_root: &Path,
    write: &GoogleFontDownloadFileWrite,
) -> Result<bool, String> {
    if let Some(staged) = workspace.staged_binary_resource(&write.project_relative_path) {
        if hash_bytes(staged) == hash_bytes(&write.bytes) {
            return Ok(true);
        }
        return Err(format!(
            "Font Manager a refuzat resursa staged divergentă {}.",
            write.project_relative_path
        ));
    }
    let target = resolve_project_write_path(project_root, &write.project_relative_path)?;
    match fs::symlink_metadata(&target) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(format!(
                "Font Manager a blocat scrierea: {} este symlink.",
                write.project_relative_path
            ));
        }
        Ok(metadata) if metadata.is_dir() => {
            return Err(format!(
                "Font Manager a blocat scrierea: {} este director.",
                write.project_relative_path
            ));
        }
        Ok(_) => {
            let existing = fs::read(&target).map_err(|error| {
                format!(
                    "Nu am putut citi fontul existent {} înainte de conflict check: {}",
                    write.project_relative_path, error
                )
            })?;
            if hash_bytes(&existing) == hash_bytes(&write.bytes) {
                return Ok(true);
            }
            return Err(format!(
                "Font Manager a blocat suprascrierea fontului existent {}. Șterge sau redenumește fișierul înainte de re-descărcare.",
                write.project_relative_path
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(format!(
                "Nu am putut verifica fontul {} înainte de scriere: {}",
                write.project_relative_path, error
            ));
        }
    }
}
