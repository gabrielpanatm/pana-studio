use crate::kernel::project_state::model::KernelProjectStateSnapshot;

use super::KernelProjectTransitionAction;

pub(super) fn action_context(action: KernelProjectTransitionAction) -> &'static str {
    match action {
        KernelProjectTransitionAction::OpenProject => {
            "Open Project cere înlocuirea sesiunii curente."
        }
        KernelProjectTransitionAction::ReloadProject => {
            "Reload Project cere reconstruirea sesiunii curente."
        }
        KernelProjectTransitionAction::CloseProject => {
            "Close Project cere închiderea sesiunii curente."
        }
    }
}

pub(super) fn transition_evidence(project_state: &KernelProjectStateSnapshot) -> String {
    format!(
        "status={:?}, reason={:?}, workspaceDirty={}, workspaceRevision={:?}, workspaceUndo={}, workspaceRedo={}, conflicts={}, blocking={}, metadata={}. {}",
        project_state.status,
        project_state.reason,
        project_state.workspace_dirty_resource_count,
        project_state.workspace_revision,
        project_state.workspace_undo_count,
        project_state.workspace_redo_count,
        project_state.disk_conflict_count,
        project_state.disk_blocking_count,
        project_state.metadata_changed_count,
        project_state.verdict_reason
    )
}
