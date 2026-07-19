use crate::kernel::{
    disk_conflict::KernelDiskConflictSnapshot, project_session::ProjectSessionSnapshot,
    project_workspace::ProjectWorkspaceSnapshot,
};

use super::model::KernelProjectStateSnapshot;

mod context;
mod evaluator;
mod metrics;
mod snapshot;

use context::ProjectStateAssessmentContext;
use evaluator::evaluate_project_state_verdict;
use metrics::ProjectStateMetrics;
use snapshot::snapshot;

pub fn build_kernel_project_state_snapshot(
    project_root: Option<&str>,
    session: Option<&ProjectSessionSnapshot>,
    workspace: Option<&ProjectWorkspaceSnapshot>,
    disk_conflicts: Option<&KernelDiskConflictSnapshot>,
) -> KernelProjectStateSnapshot {
    let context = ProjectStateAssessmentContext::from_inputs(
        project_root,
        session,
        workspace.is_some(),
        disk_conflicts.is_some(),
    );
    let metrics = ProjectStateMetrics::from(disk_conflicts, workspace);
    let verdict = evaluate_project_state_verdict(&context, disk_conflicts, &metrics);

    snapshot(context, verdict, metrics)
}
