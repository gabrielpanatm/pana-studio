use tauri::State;

use crate::{
    kernel::command_center::{
        search_command_center_index, CommandCenterSearchRequest, CommandCenterSearchResponse,
    },
    localization::LocalizedDiagnostic,
    project_model::{
        cache::{
            build_project_model_from_context, capture_project_model_build_context,
            publish_project_model_if_current,
        },
        model::ProjectModel,
    },
    state::AppState,
};

#[tauri::command(async)]
pub fn search_command_center(
    request: CommandCenterSearchRequest,
    state: State<AppState>,
) -> Result<CommandCenterSearchResponse, LocalizedDiagnostic> {
    let identity = current_project_identity(&state)?;
    let Some((project_root, runtime_session_id)) = identity else {
        if request.expected_project_root.is_some() || request.expected_session_id.is_some() {
            return Err(LocalizedDiagnostic::new(
                "command-center-closed-session-identity",
            ));
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

fn current_project_identity(
    state: &AppState,
) -> Result<Option<(String, String)>, LocalizedDiagnostic> {
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| LocalizedDiagnostic::new("command-center-workspace-lock-failed"))?;
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
) -> Result<(), LocalizedDiagnostic> {
    if request.expected_project_root.as_deref() != Some(project_root)
        || request.expected_session_id.as_deref() != Some(runtime_session_id)
    {
        return Err(LocalizedDiagnostic::new(
            "command-center-session-identity-mismatch",
        ));
    }
    Ok(())
}

fn current_project_model(
    state: &AppState,
    project_root: &str,
    runtime_session_id: &str,
) -> Result<ProjectModel, LocalizedDiagnostic> {
    {
        let workspace = state
            .project_workspace
            .lock()
            .map_err(|_| LocalizedDiagnostic::new("command-center-model-cache-lock-failed"))?;
        let workspace = workspace
            .as_ref()
            .ok_or_else(|| LocalizedDiagnostic::new("command-center-workspace-lost"))?;
        if workspace.session.project_root != project_root
            || workspace.runtime_session_id() != runtime_session_id
        {
            return Err(LocalizedDiagnostic::new(
                "command-center-index-stale-before-read",
            ));
        }
        if workspace.project_model_source_revision == Some(workspace.revision) {
            if let Some(model) = workspace.project_model.as_ref() {
                return Ok(model.clone());
            }
        }
    }

    let (root, session, context) =
        capture_project_model_build_context(state).map_err(|details| {
            LocalizedDiagnostic::new("command-center-model-build-context-failed")
                .with_argument("details", details)
        })?;
    if session.project_root != project_root || session.runtime_instance_id() != runtime_session_id {
        return Err(LocalizedDiagnostic::new(
            "command-center-index-stale-during-build",
        ));
    }
    let model = build_project_model_from_context(&root, &context).map_err(|details| {
        LocalizedDiagnostic::new("command-center-model-build-failed")
            .with_argument("details", details)
    })?;
    publish_project_model_if_current(state, &context, model.clone()).map_err(|details| {
        LocalizedDiagnostic::new("command-center-model-publish-failed")
            .with_argument("details", details)
    })?;
    Ok(model)
}
