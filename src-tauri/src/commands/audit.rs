use tauri::State;

use crate::{
    kernel::audit::{build_project_audit, ProjectAuditSnapshot},
    project_model::cache::{capture_project_model_build_lease, publish_project_model_if_current},
    state::AppState,
};

/// Builds the user-facing audit exclusively from the current Rust-owned
/// ProjectWorkspace projection. No parallel browser scan or direct disk read
/// can become a competing source of truth.
#[tauri::command]
pub fn read_project_audit(state: State<AppState>) -> Result<ProjectAuditSnapshot, String> {
    let (root, session, lease) = capture_project_model_build_lease(&state)?;
    let file_buffer_diagnostics = {
        let workspace = state
            .project_workspace
            .lock()
            .map_err(|_| "Nu am putut captura ProjectWorkspace pentru Audit.".to_string())?;
        let workspace = workspace
            .as_ref()
            .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru Audit.".to_string())?;
        if workspace.session.project_root != lease.projection().project_root
            || workspace.runtime_session_id() != lease.projection().runtime_session_id
            || workspace.revision != lease.projection().revision
        {
            return Err(
                "Audit a refuzat o proiecție stale: sesiunea sau revizia workspace s-a schimbat."
                    .to_string(),
            );
        }
        workspace.documents.diagnostics.clone()
    };
    let model = crate::project_model::build_project_model_from_workspace_projection(
        &root,
        lease.projection(),
    )?;
    let snapshot = build_project_audit(
        &model,
        &file_buffer_diagnostics,
        session.runtime_instance_id(),
        lease.projection().revision,
    );
    publish_project_model_if_current(&state, &lease, model)?;
    Ok(snapshot)
}
