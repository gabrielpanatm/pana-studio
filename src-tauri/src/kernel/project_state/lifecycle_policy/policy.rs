use crate::kernel::project_state::model::KernelProjectStateSnapshot;

use super::{
    evidence::{action_context, transition_evidence},
    KernelProjectTransitionAction, KernelProjectTransitionDecision, KernelProjectTransitionPolicy,
    KernelProjectTransitionReason, KERNEL_PROJECT_TRANSITION_POLICY_SCHEMA_VERSION,
};

pub(super) fn policy(
    action: KernelProjectTransitionAction,
    project_state: &KernelProjectStateSnapshot,
    decision: KernelProjectTransitionDecision,
    reason: KernelProjectTransitionReason,
    title: &str,
    message: &str,
    recommended_action: &str,
) -> KernelProjectTransitionPolicy {
    KernelProjectTransitionPolicy {
        schema_version: KERNEL_PROJECT_TRANSITION_POLICY_SCHEMA_VERSION,
        action,
        decision,
        reason,
        project_state_status: project_state.status,
        project_state_reason: project_state.reason,
        project_root: project_state.project_root.clone(),
        session_id: project_state.session_id.clone(),
        requires_operator_confirmation: decision == KernelProjectTransitionDecision::Confirm,
        blocks_transition: decision != KernelProjectTransitionDecision::Allow,
        title: title.to_string(),
        message: format!("{} {}", action_context(action), message),
        evidence: transition_evidence(project_state),
        recommended_action: recommended_action.to_string(),
        workspace_dirty_resource_count: project_state.workspace_dirty_resource_count,
        workspace_revision: project_state.workspace_revision,
        workspace_undo_count: project_state.workspace_undo_count,
        workspace_redo_count: project_state.workspace_redo_count,
        disk_conflict_count: project_state.disk_conflict_count,
        disk_blocking_count: project_state.disk_blocking_count,
        metadata_changed_count: project_state.metadata_changed_count,
    }
}
