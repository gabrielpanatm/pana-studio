use crate::kernel::project_state::model::{
    KernelProjectStateSnapshot, KernelProjectStateStatus, KERNEL_PROJECT_STATE_SCHEMA_VERSION,
};

use super::{
    context::ProjectStateAssessmentContext, evaluator::ProjectStateAssessmentVerdict,
    metrics::ProjectStateMetrics,
};

pub(super) fn snapshot(
    context: ProjectStateAssessmentContext,
    verdict: ProjectStateAssessmentVerdict,
    metrics: ProjectStateMetrics,
) -> KernelProjectStateSnapshot {
    let ProjectStateAssessmentContext {
        project_open,
        session_available: _,
        session_id,
        project_root,
        project_workspace_available,
        disk_conflict_snapshot_available,
    } = context;
    let ProjectStateAssessmentVerdict {
        status,
        reason,
        verdict_reason,
    } = verdict;

    KernelProjectStateSnapshot {
        schema_version: KERNEL_PROJECT_STATE_SCHEMA_VERSION,
        status,
        reason,
        verdict_reason,
        project_open,
        session_id,
        project_root,
        is_clean: matches!(status, KernelProjectStateStatus::Clean),
        write_blocked: matches!(status, KernelProjectStateStatus::Blocked)
            || metrics.disk_blocking_count > 0,
        project_workspace_available,
        disk_conflict_snapshot_available,
        workspace_dirty: metrics.workspace_dirty,
        workspace_revision: metrics.workspace_revision,
        workspace_disk_generation: metrics.workspace_disk_generation,
        workspace_dirty_resource_count: metrics.workspace_dirty_resource_count,
        workspace_dirty_document_count: metrics.workspace_dirty_document_count,
        workspace_created_document_count: metrics.workspace_created_document_count,
        workspace_deleted_document_count: metrics.workspace_deleted_document_count,
        workspace_dirty_page_js_count: metrics.workspace_dirty_page_js_count,
        workspace_undo_count: metrics.workspace_undo_count,
        workspace_redo_count: metrics.workspace_redo_count,
        dirty_only_count: metrics.dirty_only_count,
        metadata_changed_count: metrics.metadata_changed_count,
        disk_conflict_count: metrics.disk_conflict_count,
        disk_blocking_count: metrics.disk_blocking_count,
        unreadable_file_count: metrics.unreadable_file_count,
    }
}
