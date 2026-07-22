use std::path::Path;

use tauri::{AppHandle, State};

use crate::{
    commands::project::require_current_project_root,
    kernel::{
        file_buffer_store::FileBufferStore,
        project_workspace::{
            commit_project_workspace_session_mutation, ProjectWorkspaceIdentity,
            WorkspaceMutationMetadata, WorkspaceResourceMutation,
        },
    },
    project::zola_project_root,
    state::AppState,
};

pub(super) struct ConfigWorkspaceMutationInput {
    label: String,
    target: String,
    changes: Vec<WorkspaceResourceMutation>,
}

pub(super) fn zola_to_project_relative_path(path: &str) -> String {
    path.to_string()
}

pub(super) fn read_current_project_text(
    store: &FileBufferStore,
    project_relative_path: &str,
) -> Option<String> {
    store.text_for(project_relative_path)
}

pub(super) fn read_current_project_text_from_state(
    state: &State<AppState>,
    project_relative_path: &str,
) -> Result<Option<String>, String> {
    require_current_project_root(state)?;
    let slot = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace.".to_string())?;
    let workspace = slot
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    Ok(workspace.documents.text_for(project_relative_path))
}

pub(super) fn push_text_change_if_changed(
    changes: &mut Vec<WorkspaceResourceMutation>,
    relative_path: String,
    before: &str,
    after: String,
) {
    if after == before {
        return;
    }
    changes.push(WorkspaceResourceMutation {
        relative_path,
        contents: after,
        create_only: false,
    });
}

pub(super) fn workspace_mutation_input(
    label: impl Into<String>,
    target: impl Into<String>,
    changes: Vec<WorkspaceResourceMutation>,
) -> Option<ConfigWorkspaceMutationInput> {
    if changes.is_empty() {
        return None;
    }
    Some(ConfigWorkspaceMutationInput {
        label: label.into(),
        target: target.into(),
        changes,
    })
}

pub(super) fn execute_config_workspace_mutation<R>(
    app: &AppHandle,
    state: &State<AppState>,
    build: impl FnOnce(
        &Path,
        &Path,
        &FileBufferStore,
    ) -> Result<(Option<ConfigWorkspaceMutationInput>, R), String>,
) -> Result<R, String> {
    let project_root = require_current_project_root(state)?;
    let zola_root = zola_project_root(&project_root);
    let mut slot = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace.".to_string())?;
    let workspace = slot
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        &project_root,
    )?;
    let (input, result_value) = build(&project_root, &zola_root, &workspace.documents)?;
    let Some(input) = input else {
        return Ok(result_value);
    };
    commit_project_workspace_session_mutation(app, workspace, |candidate| {
        let identity = ProjectWorkspaceIdentity {
            expected_project_root: candidate.session.project_root.clone(),
            expected_session_id: candidate.runtime_session_id(),
            expected_revision: candidate.revision,
        };
        candidate.stage_resource_texts(
            &identity,
            WorkspaceMutationMetadata {
                label: input.label,
                source: "config.panel".to_string(),
                coalesce_key: None,
                transaction_id: Some(input.target),
            },
            input.changes,
            crate::kernel::file_buffer_store::now_ms(),
        )
    })?;
    Ok(result_value)
}
