use crate::kernel::project_state::model::{
    KernelProjectStateReason, KernelProjectStateSnapshot, KernelProjectStateStatus,
};

use super::{
    policy::policy, KernelProjectTransitionAction, KernelProjectTransitionDecision,
    KernelProjectTransitionPolicy, KernelProjectTransitionReason,
};

pub fn evaluate_project_transition_policy(
    action: KernelProjectTransitionAction,
    project_state: &KernelProjectStateSnapshot,
) -> KernelProjectTransitionPolicy {
    let (decision, reason) = match (project_state.status, project_state.reason) {
        (KernelProjectStateStatus::Idle, _) => (
            KernelProjectTransitionDecision::Allow,
            KernelProjectTransitionReason::NoOpenProject,
        ),
        (KernelProjectStateStatus::Clean, _) => (
            KernelProjectTransitionDecision::Allow,
            KernelProjectTransitionReason::Clean,
        ),
        (KernelProjectStateStatus::Info, KernelProjectStateReason::MetadataChanged) => (
            KernelProjectTransitionDecision::Allow,
            KernelProjectTransitionReason::MetadataChanged,
        ),
        (KernelProjectStateStatus::Dirty, KernelProjectStateReason::WorkspaceDirty) => (
            KernelProjectTransitionDecision::Confirm,
            KernelProjectTransitionReason::WorkspaceDirty,
        ),
        (KernelProjectStateStatus::Warning, KernelProjectStateReason::DiskConflict)
            if action == KernelProjectTransitionAction::ReloadProject =>
        {
            (
                KernelProjectTransitionDecision::Confirm,
                KernelProjectTransitionReason::DiskConflict,
            )
        }
        (KernelProjectStateStatus::Warning, KernelProjectStateReason::DiskConflict) => (
            KernelProjectTransitionDecision::Block,
            KernelProjectTransitionReason::DiskConflict,
        ),
        (KernelProjectStateStatus::Blocked, _) => (
            KernelProjectTransitionDecision::Block,
            KernelProjectTransitionReason::BlockedProjectState,
        ),
        (KernelProjectStateStatus::Warning, _) => (
            KernelProjectTransitionDecision::Block,
            KernelProjectTransitionReason::UnknownWarning,
        ),
        _ => (
            KernelProjectTransitionDecision::Block,
            KernelProjectTransitionReason::BlockedProjectState,
        ),
    };

    policy(action, project_state, decision, reason)
}
