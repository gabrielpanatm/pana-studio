use crate::kernel::project_state::model::KernelProjectStateSnapshot;

use super::{
    evaluate_project_transition_policy, KernelProjectTransitionAction,
    KernelProjectTransitionPolicyMatrixSnapshot,
    KERNEL_PROJECT_TRANSITION_POLICY_MATRIX_SCHEMA_VERSION,
};

pub fn build_project_transition_policy_matrix(
    project_state: KernelProjectStateSnapshot,
) -> KernelProjectTransitionPolicyMatrixSnapshot {
    let policies = [
        KernelProjectTransitionAction::OpenProject,
        KernelProjectTransitionAction::ReloadProject,
        KernelProjectTransitionAction::CloseProject,
    ]
    .into_iter()
    .map(|action| evaluate_project_transition_policy(action, &project_state))
    .collect();

    KernelProjectTransitionPolicyMatrixSnapshot {
        schema_version: KERNEL_PROJECT_TRANSITION_POLICY_MATRIX_SCHEMA_VERSION,
        project_state,
        policies,
    }
}
