use serde::{Deserialize, Serialize};
use std::sync::{atomic::AtomicBool, Arc};

use crate::kernel::project_workspace::ProjectWorkspaceSnapshot;

pub const VERSIONING_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersioningSessionIdentity {
    pub expected_project_root: String,
    pub expected_session_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersioningMutationIdentity {
    pub expected_project_root: String,
    pub expected_session_id: String,
    pub expected_status_token: String,
    #[serde(default)]
    pub expected_head_oid: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersioningPathsInput {
    pub paths: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersioningCommitInput {
    pub message: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersioningIdentityInput {
    pub name: String,
    pub email: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionRemoteInput {
    pub name: String,
    pub fetch_url: String,
    #[serde(default)]
    pub push_url: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionRemoteNameInput {
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionUpstreamInput {
    pub local_branch: String,
    pub remote: String,
    pub remote_branch: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionBranchInput {
    pub name: String,
    #[serde(default)]
    pub start_oid: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionBranchNameInput {
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionFetchInput {
    pub operation_id: String,
    pub remote: String,
    #[serde(default)]
    pub prune: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionPushInput {
    pub operation_id: String,
    pub remote: String,
    pub remote_branch: String,
    #[serde(default)]
    pub set_upstream: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionNetworkCancelInput {
    pub operation_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionIntegrationTargetInput {
    pub target_ref: String,
    pub expected_target_oid: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionIntegrationMode {
    FastForward,
    Merge,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionIntegrationInput {
    pub target_ref: String,
    pub expected_target_oid: String,
    pub mode: VersionIntegrationMode,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionSwitchBranchInput {
    pub branch: String,
    pub expected_target_oid: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionRepositoryState {
    Uninitialized,
    Ready,
    Invalid,
    Unsupported,
    GitUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionFileKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Untracked,
    Conflicted,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionPublicationStatus {
    Published,
    PublishedRefreshRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionSyncState {
    NoUpstream,
    UpstreamMissing,
    Unborn,
    UpToDate,
    Ahead,
    Behind,
    Diverged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionIntegrationRelationship {
    Same,
    FastForward,
    LocalAhead,
    Diverged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionIntegrationKind {
    FastForward,
    MergeClean,
    MergeConflict,
    MergeResolved,
    SwitchBranch,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionIntegrationPlan {
    pub schema_version: u32,
    pub head_oid: String,
    pub target_ref: String,
    pub target_oid: String,
    pub relationship: VersionIntegrationRelationship,
    pub ahead: u64,
    pub behind: u64,
    pub local_only: Vec<VersionHistoryEntry>,
    pub target_only: Vec<VersionHistoryEntry>,
    pub fast_forward_allowed: bool,
    pub merge_allowed: bool,
    pub repository_clean: bool,
    pub diagnostic: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionRemote {
    pub name: String,
    pub fetch_url: String,
    pub push_url: String,
    pub usable: bool,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionBranch {
    pub name: String,
    pub oid: Option<String>,
    pub current: bool,
    pub upstream_ref: Option<String>,
    pub upstream_oid: Option<String>,
    pub ahead: u64,
    pub behind: u64,
    pub sync_state: VersionSyncState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionRemoteBranch {
    pub remote: String,
    pub name: String,
    pub ref_name: String,
    pub oid: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionUpstream {
    pub local_branch: String,
    pub remote: String,
    pub remote_branch: String,
    pub ref_name: String,
    pub oid: Option<String>,
    pub ahead: u64,
    pub behind: u64,
    pub sync_state: VersionSyncState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionFileStatus {
    pub path: String,
    pub original_path: Option<String>,
    pub kind: VersionFileKind,
    pub index_status: String,
    pub worktree_status: String,
    pub staged: bool,
    pub unstaged: bool,
    pub conflicted: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersioningSnapshot {
    pub schema_version: u32,
    pub project_root: String,
    pub repository_root: String,
    pub repository_state: VersionRepositoryState,
    pub diagnostic: Option<String>,
    pub git_version: Option<String>,
    pub object_format: Option<String>,
    pub branch: Option<String>,
    pub detached_head: bool,
    pub unborn_head: bool,
    pub head_oid: Option<String>,
    pub status_token: String,
    pub clean: bool,
    pub staged_count: usize,
    pub unstaged_count: usize,
    pub conflicted_count: usize,
    pub files: Vec<VersionFileStatus>,
    pub user_name: Option<String>,
    pub user_email: Option<String>,
    pub remotes: Vec<VersionRemote>,
    pub branches: Vec<VersionBranch>,
    pub remote_branches: Vec<VersionRemoteBranch>,
    pub upstream: Option<VersionUpstream>,
    pub sync_state: VersionSyncState,
}

impl VersioningSnapshot {
    pub fn terminal(
        project_root: impl Into<String>,
        repository_root: impl Into<String>,
        repository_state: VersionRepositoryState,
        diagnostic: Option<String>,
        git_version: Option<String>,
        status_token: String,
    ) -> Self {
        Self {
            schema_version: VERSIONING_SCHEMA_VERSION,
            project_root: project_root.into(),
            repository_root: repository_root.into(),
            repository_state,
            diagnostic,
            git_version,
            object_format: None,
            branch: None,
            detached_head: false,
            unborn_head: false,
            head_oid: None,
            status_token,
            clean: true,
            staged_count: 0,
            unstaged_count: 0,
            conflicted_count: 0,
            files: Vec::new(),
            user_name: None,
            user_email: None,
            remotes: Vec::new(),
            branches: Vec::new(),
            remote_branches: Vec::new(),
            upstream: None,
            sync_state: VersionSyncState::NoUpstream,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionNetworkOperationKind {
    Fetch,
    Push,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionNetworkOperationStatus {
    Started,
    Progress,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionNetworkProgressEvent {
    pub schema_version: u32,
    pub project_root: String,
    pub session_id: String,
    pub operation_id: String,
    pub kind: VersionNetworkOperationKind,
    pub status: VersionNetworkOperationStatus,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionNetworkReceipt {
    pub schema_version: u32,
    pub operation_id: String,
    pub kind: VersionNetworkOperationKind,
    pub remote: String,
    pub branch: Option<String>,
    pub changed: bool,
    pub diagnostic: Option<String>,
    pub snapshot: VersioningSnapshot,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionNetworkCancelReceipt {
    pub schema_version: u32,
    pub operation_id: String,
    pub cancellation_requested: bool,
}

#[derive(Clone)]
pub(crate) struct VersionNetworkOperationControl {
    pub operation_id: String,
    pub project_root: String,
    pub session_id: String,
    pub kind: VersionNetworkOperationKind,
    pub cancellation: Arc<AtomicBool>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionSyncComparison {
    pub schema_version: u32,
    pub local_ref: String,
    pub upstream_ref: String,
    pub ahead: u64,
    pub behind: u64,
    pub local_only: Vec<VersionHistoryEntry>,
    pub remote_only: Vec<VersionHistoryEntry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionIntegrationStatus {
    Applied,
    Noop,
    ConflictResolutionRequired,
    RecoveryRequired,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionIntegrationReceipt {
    pub schema_version: u32,
    pub status: VersionIntegrationStatus,
    pub project_root: String,
    pub session_id: String,
    pub transaction_id: Option<String>,
    pub recovery_ref: Option<String>,
    pub kind: Option<VersionIntegrationKind>,
    pub previous_head_oid: String,
    pub target_ref: String,
    pub target_oid: String,
    pub result_commit_oid: Option<String>,
    pub changed_paths: Vec<String>,
    pub conflict_paths: Vec<String>,
    pub diagnostic: Option<String>,
    pub snapshot: Option<VersioningSnapshot>,
    pub workspace: Option<ProjectWorkspaceSnapshot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionIntegrationRecoveryAction {
    Finalize,
    Continue,
    Rollback,
    Cleanup,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionIntegrationRecoveryState {
    ReadyToFinalize,
    ConflictResolution,
    ReadyToRollback,
    CleanupRequired,
    ManualReview,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionIntegrationRecoveryItem {
    pub transaction_id: String,
    pub recovery_ref: String,
    pub kind: VersionIntegrationKind,
    pub previous_head_oid: String,
    pub target_ref: String,
    pub target_oid: String,
    pub result_commit_oid: Option<String>,
    pub conflict_paths: Vec<String>,
    pub state: VersionIntegrationRecoveryState,
    pub available_actions: Vec<VersionIntegrationRecoveryAction>,
    pub diagnostic: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionIntegrationRecoveryScan {
    pub schema_version: u32,
    pub project_root: String,
    pub session_id: String,
    pub items: Vec<VersionIntegrationRecoveryItem>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionIntegrationRecoveryResolutionInput {
    pub recovery_ref: String,
    pub action: VersionIntegrationRecoveryAction,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionIntegrationRecoveryResolutionReceipt {
    pub schema_version: u32,
    pub project_root: String,
    pub session_id: String,
    pub transaction_id: String,
    pub recovery_ref: String,
    pub action: VersionIntegrationRecoveryAction,
    pub resolved: bool,
    pub diagnostic: Option<String>,
    pub snapshot: Option<VersioningSnapshot>,
    pub workspace: Option<ProjectWorkspaceSnapshot>,
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedVersionIntegration {
    pub transaction_id: String,
    pub recovery_ref: String,
    pub kind: VersionIntegrationKind,
    pub previous_head_oid: String,
    pub previous_tree_oid: String,
    pub full_head_ref: String,
    pub target_ref: String,
    pub target_oid: String,
    pub target_tree_oid: String,
    pub marker_commit_oid: String,
    pub result_commit_oid: Option<String>,
    pub target_branch: Option<String>,
    pub conflict_paths: Vec<String>,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersioningMutationReceipt {
    pub schema_version: u32,
    pub changed: bool,
    pub touched_paths: Vec<String>,
    pub snapshot: VersioningSnapshot,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersioningCommitReceipt {
    pub schema_version: u32,
    pub commit_oid: String,
    pub parent_oid: Option<String>,
    pub message: String,
    pub publication_status: VersionPublicationStatus,
    pub diagnostic: Option<String>,
    pub snapshot: Option<VersioningSnapshot>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionHistoryPage {
    pub schema_version: u32,
    pub offset: usize,
    pub limit: usize,
    pub has_more: bool,
    pub entries: Vec<VersionHistoryEntry>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionHistoryEntry {
    pub oid: String,
    pub short_oid: String,
    pub parent_oids: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    pub authored_at: String,
    pub subject: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionDiffKind {
    Unstaged,
    Staged,
    Commit,
    Integration,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionDiffInput {
    pub kind: VersionDiffKind,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub commit_oid: Option<String>,
    #[serde(default)]
    pub target_ref: Option<String>,
    #[serde(default)]
    pub expected_target_oid: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionDiffReceipt {
    pub schema_version: u32,
    pub kind: VersionDiffKind,
    pub path: Option<String>,
    pub commit_oid: Option<String>,
    pub binary: bool,
    pub truncated: bool,
    pub patch: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionPreviewInput {
    pub commit_oid: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionPreviewReceipt {
    pub schema_version: u32,
    pub project_root: String,
    pub session_id: String,
    pub commit_oid: String,
    pub short_oid: String,
    pub preview_url: String,
    pub file_count: usize,
    pub total_bytes: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct VersionTreeFile {
    pub path: String,
    pub oid: String,
    pub bytes: Vec<u8>,
    pub executable: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct VersionTree {
    pub commit_oid: String,
    pub tree_oid: String,
    pub files: Vec<VersionTreeFile>,
    pub total_bytes: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionRestoreInput {
    pub target_commit_oid: String,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionRestoreStatus {
    Restored,
    Noop,
    RecoveryRequired,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionRestoreReceipt {
    pub schema_version: u32,
    pub status: VersionRestoreStatus,
    pub project_root: String,
    pub session_id: String,
    pub transaction_id: Option<String>,
    pub recovery_ref: Option<String>,
    pub target_commit_oid: String,
    pub previous_head_oid: Option<String>,
    pub restore_commit_oid: Option<String>,
    pub changed_paths: Vec<String>,
    pub diagnostic: Option<String>,
    pub snapshot: Option<VersioningSnapshot>,
    pub workspace: Option<ProjectWorkspaceSnapshot>,
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedVersionRestore {
    pub transaction_id: String,
    pub recovery_ref: String,
    pub target_commit_oid: String,
    pub target_tree_oid: String,
    pub previous_head_oid: String,
    pub restore_commit_oid: String,
    pub full_head_ref: String,
}

#[derive(Clone, Debug)]
pub(crate) struct VersionRestoreFinalization {
    pub snapshot: Option<VersioningSnapshot>,
    pub diagnostic: Option<String>,
    pub cleanup_required: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionRestoreRecoveryAction {
    Finalize,
    Rollback,
    Cleanup,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionRestoreRecoveryState {
    ReadyToFinalize,
    ReadyToRollback,
    CleanupRequired,
    ManualReview,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionRestoreRecoveryItem {
    pub transaction_id: String,
    pub recovery_ref: String,
    pub target_commit_oid: String,
    pub previous_head_oid: String,
    pub restore_commit_oid: String,
    pub state: VersionRestoreRecoveryState,
    pub available_actions: Vec<VersionRestoreRecoveryAction>,
    pub diagnostic: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionRestoreRecoveryScan {
    pub schema_version: u32,
    pub project_root: String,
    pub session_id: String,
    pub items: Vec<VersionRestoreRecoveryItem>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionRestoreRecoveryResolutionInput {
    pub recovery_ref: String,
    pub action: VersionRestoreRecoveryAction,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionRestoreRecoveryResolutionReceipt {
    pub schema_version: u32,
    pub project_root: String,
    pub session_id: String,
    pub transaction_id: String,
    pub recovery_ref: String,
    pub action: VersionRestoreRecoveryAction,
    pub resolved: bool,
    pub diagnostic: Option<String>,
    pub snapshot: Option<VersioningSnapshot>,
    pub workspace: Option<ProjectWorkspaceSnapshot>,
}
