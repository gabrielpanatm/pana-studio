use crate::kernel::project_state::{
    lifecycle_policy::KernelProjectTransitionReason,
    model::{KernelProjectStateReason, KernelProjectStateStatus},
};

use super::super::{KernelProjectTransitionBlockedCause, KernelProjectTransitionBlockedRecord};

pub(super) fn classify_blocked_cause(
    record: &KernelProjectTransitionBlockedRecord,
) -> KernelProjectTransitionBlockedCause {
    if record.reason == Some(KernelProjectTransitionReason::DiskConflict)
        || matches!(
            record.project_state_reason,
            Some(
                KernelProjectStateReason::DiskConflict
                    | KernelProjectStateReason::DiskUnverifiable
                    | KernelProjectStateReason::DiskConflictSnapshotMissing
            )
        )
        || record.disk_blocking_count > 0
        || record.disk_conflict_count > 0
    {
        return KernelProjectTransitionBlockedCause::DiskConflict;
    }

    if record.reason == Some(KernelProjectTransitionReason::WorkspaceDirty)
        || record.project_state_reason == Some(KernelProjectStateReason::WorkspaceDirty)
        || record.workspace_dirty_resource_count > 0
    {
        return KernelProjectTransitionBlockedCause::WorkspaceDirty;
    }

    if record.reason == Some(KernelProjectTransitionReason::BlockedProjectState)
        || record.project_state_status == Some(KernelProjectStateStatus::Blocked)
    {
        return KernelProjectTransitionBlockedCause::BlockedProjectState;
    }

    KernelProjectTransitionBlockedCause::Unknown
}
