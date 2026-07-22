use tauri::State;

use crate::{
    kernel::command_center::{
        search_command_center_index, CommandCenterSearchRequest, CommandCenterSearchResponse,
    },
    project_model::{
        build_project_model_from_workspace_projection,
        cache::{capture_project_model_build_lease, publish_project_model_if_current},
        model::ProjectModel,
    },
    state::AppState,
};

#[tauri::command(async)]
pub fn search_command_center(
    request: CommandCenterSearchRequest,
    state: State<AppState>,
) -> Result<CommandCenterSearchResponse, String> {
    let identity = current_project_identity(&state)?;
    let Some((project_root, runtime_session_id)) = identity else {
        if request.expected_project_root.is_some() || request.expected_session_id.is_some() {
            return Err("Command Center a refuzat identitatea unei sesiuni închise.".to_string());
        }
        return search_command_center_index(request, None, None, None);
    };
    require_request_identity(&request, &project_root, &runtime_session_id)?;
    let model = current_project_model(&state, &project_root, &runtime_session_id)?;
    search_command_center_index(
        request,
        Some(&project_root),
        Some(&runtime_session_id),
        Some(&model),
    )
}

fn current_project_identity(state: &AppState) -> Result<Option<(String, String)>, String> {
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Command Center nu a putut citi ProjectWorkspace.".to_string())?;
    Ok(workspace.as_ref().map(|workspace| {
        (
            workspace.session.project_root.clone(),
            workspace.runtime_session_id(),
        )
    }))
}

fn require_request_identity(
    request: &CommandCenterSearchRequest,
    project_root: &str,
    runtime_session_id: &str,
) -> Result<(), String> {
    if request.expected_project_root.as_deref() != Some(project_root)
        || request.expected_session_id.as_deref() != Some(runtime_session_id)
    {
        return Err("Command Center a refuzat o căutare pentru altă ProjectSession.".to_string());
    }
    Ok(())
}

fn current_project_model(
    state: &AppState,
    project_root: &str,
    runtime_session_id: &str,
) -> Result<ProjectModel, String> {
    {
        let workspace = state
            .project_workspace
            .lock()
            .map_err(|_| "Command Center nu a putut citi ProjectModel cache.".to_string())?;
        let workspace = workspace
            .as_ref()
            .ok_or_else(|| "Command Center a pierdut ProjectWorkspace.".to_string())?;
        if workspace.session.project_root != project_root
            || workspace.runtime_session_id() != runtime_session_id
        {
            return Err("Command Center a devenit stale înainte de citirea indexului.".to_string());
        }
        if workspace.project_model_source_revision == Some(workspace.revision) {
            if let Some(model) = workspace.project_model.as_ref() {
                return Ok(model.clone());
            }
        }
    }

    let (root, session, lease) = capture_project_model_build_lease(state)?;
    if session.project_root != project_root || session.runtime_instance_id() != runtime_session_id {
        return Err("Command Center a devenit stale în timpul construirii indexului.".to_string());
    }
    let model = build_project_model_from_workspace_projection(&root, lease.projection())?;
    publish_project_model_if_current(state, &lease, model.clone())?;
    Ok(model)
}
