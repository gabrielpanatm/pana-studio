use std::path::{Path, PathBuf};

use tauri::{AppHandle, State};

use crate::{
    kernel::project_runtime_access::require_current_project_root,
    project::{
        apply_project_model_preview_routes, read_project_disk_manifest,
        scan_project_workspace_projection, ProjectDiskManifest, ProjectDiskWatchHandle,
        ProjectScan,
    },
    state::AppState,
};

use super::contracts::{
    ProjectDiskWatchReceipt, ProjectDiskWatchRequest, ProjectDiskWatchStopRequest,
};

#[tauri::command]
pub fn scan_project(path: String, state: State<AppState>) -> Result<ProjectScan, String> {
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
        workspace
            .as_ref()
            .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru ProjectScan.".to_string())?
            .capture_projection_snapshot()?
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
) -> Result<ProjectDiskManifest, String> {
    let root = require_current_project_root(&state)?;
    tauri::async_runtime::spawn_blocking(move || read_project_disk_manifest(&root))
        .await
        .map_err(|error| {
            format!("Monitorizarea discului proiectului s-a oprit neașteptat: {error}")
        })?
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

pub(super) fn ensure_project_disk_watch(
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
        let project_root = workspace.session.project_root.clone();
        let runtime_session_id = workspace.runtime_session_id();
        if project_root != input.expected_project_root
            || runtime_session_id != input.expected_session_id
        {
            return Err("Watcher-ul a refuzat o identitate ProjectSession stale.".to_string());
        }
        (PathBuf::from(project_root), runtime_session_id)
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
        .and_then(|workspace| {
            workspace.as_ref().map(|workspace| {
                (
                    workspace.session.project_root.clone(),
                    workspace.runtime_session_id(),
                )
            })
        })
        .is_some_and(|(root, session_id)| {
            root == receipt.project_root && session_id == receipt.runtime_session_id
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
        if !disk_watch_stop_request_is_current(
            active.matches(
                Path::new(&input.expected_project_root),
                &input.expected_session_id,
            ),
            active.watch_generation(),
            &input,
        ) {
            return Err("Oprirea watcher-ului a refuzat o identitate stale.".to_string());
        }
        slot.take()
    };
    if let Some(watcher) = watcher {
        watcher.stop();
    }
    Ok(())
}

fn disk_watch_stop_request_is_current(
    identity_matches: bool,
    active_generation: u64,
    input: &ProjectDiskWatchStopRequest,
) -> bool {
    identity_matches && active_generation == input.expected_watch_generation
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_identity_requires_exact_generation() {
        let input = ProjectDiskWatchStopRequest {
            expected_project_root: "/project".to_string(),
            expected_session_id: "session".to_string(),
            expected_watch_generation: 7,
        };
        assert!(disk_watch_stop_request_is_current(true, 7, &input));
        assert!(!disk_watch_stop_request_is_current(true, 6, &input));
        assert!(!disk_watch_stop_request_is_current(false, 7, &input));
    }
}
