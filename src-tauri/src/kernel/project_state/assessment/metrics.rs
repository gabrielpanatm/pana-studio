use crate::kernel::{
    disk_conflict::KernelDiskConflictSnapshot, project_workspace::ProjectWorkspaceSnapshot,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct ProjectStateMetrics {
    pub(super) workspace_dirty: bool,
    pub(super) workspace_revision: Option<u64>,
    pub(super) workspace_disk_generation: Option<u64>,
    pub(super) workspace_dirty_resource_count: usize,
    pub(super) workspace_dirty_document_count: usize,
    pub(super) workspace_created_document_count: usize,
    pub(super) workspace_deleted_document_count: usize,
    pub(super) workspace_dirty_page_js_count: usize,
    pub(super) workspace_undo_count: usize,
    pub(super) workspace_redo_count: usize,
    pub(super) dirty_only_count: usize,
    pub(super) metadata_changed_count: usize,
    pub(super) disk_conflict_count: usize,
    pub(super) disk_blocking_count: usize,
    pub(super) unreadable_file_count: usize,
}

impl ProjectStateMetrics {
    pub(super) fn from(
        disk_conflicts: Option<&KernelDiskConflictSnapshot>,
        workspace: Option<&ProjectWorkspaceSnapshot>,
    ) -> Self {
        let workspace_dirty_resource_count = workspace
            .map(|snapshot| {
                snapshot.dirty_document_count
                    + snapshot.created_document_count
                    + snapshot.deleted_document_count
                    + snapshot.dirty_page_js_count
            })
            .unwrap_or_default();
        Self {
            workspace_dirty: workspace.map(|snapshot| snapshot.dirty).unwrap_or_default(),
            workspace_revision: workspace.map(|snapshot| snapshot.revision),
            workspace_disk_generation: workspace.map(|snapshot| snapshot.disk_generation),
            workspace_dirty_resource_count,
            workspace_dirty_document_count: workspace
                .map(|snapshot| snapshot.dirty_document_count)
                .unwrap_or_default(),
            workspace_created_document_count: workspace
                .map(|snapshot| snapshot.created_document_count)
                .unwrap_or_default(),
            workspace_deleted_document_count: workspace
                .map(|snapshot| snapshot.deleted_document_count)
                .unwrap_or_default(),
            workspace_dirty_page_js_count: workspace
                .map(|snapshot| snapshot.dirty_page_js_count)
                .unwrap_or_default(),
            workspace_undo_count: workspace
                .map(|snapshot| snapshot.history.undo_count)
                .unwrap_or_default(),
            workspace_redo_count: workspace
                .map(|snapshot| snapshot.history.redo_count)
                .unwrap_or_default(),
            dirty_only_count: disk_conflicts
                .map(|snapshot| snapshot.summary.dirty_only_count)
                .unwrap_or_default(),
            metadata_changed_count: disk_conflicts
                .map(|snapshot| snapshot.summary.metadata_changed_count)
                .unwrap_or_default(),
            disk_conflict_count: disk_conflicts
                .map(|snapshot| snapshot.summary.conflict_count)
                .unwrap_or_default(),
            disk_blocking_count: disk_conflicts
                .map(|snapshot| snapshot.summary.blocking_count)
                .unwrap_or_default(),
            unreadable_file_count: disk_conflicts
                .map(|snapshot| {
                    snapshot.summary.unreadable_count
                        + snapshot.summary.not_file_count
                        + snapshot.summary.oversized_count
                        + snapshot.summary.invalid_path_count
                })
                .unwrap_or_default(),
        }
    }
}
