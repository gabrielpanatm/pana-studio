use serde::{Deserialize, Serialize};

pub const KERNEL_PROJECT_STATE_SCHEMA_VERSION: u32 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelProjectStateStatus {
    Idle,
    Clean,
    Info,
    Dirty,
    Warning,
    Blocked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelProjectStateReason {
    NoProject,
    ProjectSessionMissing,
    ProjectWorkspaceMissing,
    DiskConflictSnapshotMissing,
    DiskUnverifiable,
    DiskConflict,
    WorkspaceDirty,
    MetadataChanged,
    Clean,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectStateSnapshot {
    pub schema_version: u32,
    pub status: KernelProjectStateStatus,
    pub reason: KernelProjectStateReason,
    pub verdict_reason: String,
    pub project_open: bool,
    pub session_id: Option<String>,
    pub project_root: Option<String>,
    pub is_clean: bool,
    pub write_blocked: bool,
    pub project_workspace_available: bool,
    pub disk_conflict_snapshot_available: bool,
    pub workspace_dirty: bool,
    pub workspace_revision: Option<u64>,
    pub workspace_disk_generation: Option<u64>,
    pub workspace_dirty_resource_count: usize,
    pub workspace_dirty_document_count: usize,
    pub workspace_created_document_count: usize,
    pub workspace_deleted_document_count: usize,
    pub workspace_dirty_page_js_count: usize,
    pub workspace_undo_count: usize,
    pub workspace_redo_count: usize,
    pub dirty_only_count: usize,
    pub metadata_changed_count: usize,
    pub disk_conflict_count: usize,
    pub disk_blocking_count: usize,
    pub unreadable_file_count: usize,
}
