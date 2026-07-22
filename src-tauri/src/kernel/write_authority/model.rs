use std::{fmt, path::PathBuf};

use serde::Serialize;

use super::root_authority::DirectoryAuthority;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteCategory {
    InternalAppWrite,
    ProjectSourceWrite,
    PreviewWorkspaceWrite,
    ExternalIntegrationWrite,
    BuildOutputWrite,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteOwner {
    Kernel,
    ProjectSession,
    ProjectWorkspace,
    Workbench,
    ScratchState,
    AppConfig,
    McpContext,
    CodexMcp,
    ProjectInitializer,
    Preview,
    ImageOptimizer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteOperationKind {
    WriteText,
    AppendText,
    WriteBytes,
    RemoveFile,
    RemoveDirectoryTree,
    CreateDirectory,
    Rename,
    Copy,
    Symlink,
    ExternalConfigUpdate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteAtomicity {
    AtomicRename,
    AppendOnly,
    MultiFileJournaled,
    FileLifecycle,
    ExternalToolWrite,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictPolicy {
    SingleOwnerInternal,
    RequireDiskBaseline,
    RequireExplicitOverride,
    ExternalBackupRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryPolicy {
    LoggedAtomicFile,
    AppendOnlyJournal,
    TransactionJournalRequired,
    HotRollbackJournal,
    BackupBeforeWrite,
    EphemeralRebuildable,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WritePolicy {
    pub atomicity: WriteAtomicity,
    pub conflict: ConflictPolicy,
    pub recovery: RecoveryPolicy,
    pub log_required: bool,
}

#[derive(Clone, Debug)]
pub struct WriteTarget {
    pub path: PathBuf,
    pub boundary_root: PathBuf,
    pub public_label: String,
    pub expected_leaf: ExpectedLeaf,
    /// Optional live ProjectSession identity that must still own the active
    /// project authority when the filesystem effect is committed.
    ///
    /// Most callers do not need a session-specific CAS and therefore keep
    /// the default `None`. Long-lived UI requests can bind the runtime session
    /// they originated from so reopening the same root cannot authorize a
    /// stale effect.
    pub expected_runtime_session_id: Option<String>,
    authority: Option<DirectoryAuthority>,
}

/// Commit-time precondition for the final filesystem name.
///
/// `RequireDiskBaseline` operations must never infer this from the path inside
/// `WriteAuthority`: the caller has already performed the domain conflict
/// check and must carry that exact observation to the capability backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpectedLeaf {
    /// Reserved for single-owner/internal writes whose registry policy does
    /// not require a disk baseline.
    Unspecified,
    /// The final name must still be absent at the atomic commit point.
    Absent,
    /// The final name must still designate the observed disk version.
    Present(ExpectedLeafVersion),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedLeafVersion {
    /// Platform token captured from the same descriptor/read used by the
    /// domain preflight. On Linux this includes device, inode, size, times and
    /// mode.
    pub version_token: String,
    /// Content digest when the domain owns file contents. It closes in-place
    /// modification races that metadata alone cannot represent reliably.
    pub content_hash: Option<String>,
    /// Deterministic metadata fingerprint for every descendant when the leaf
    /// is a directory lifecycle source. The root itself is represented by
    /// `version_token`; this fingerprint prevents child edits from crossing
    /// the preflight/rename window unnoticed.
    pub tree_fingerprint: Option<String>,
}

#[derive(Clone, Debug)]
pub struct WriteIntent {
    pub category: WriteCategory,
    pub owner: WriteOwner,
    pub operation: WriteOperationKind,
    pub target: WriteTarget,
    pub policy: WritePolicy,
    pub description: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteReceipt {
    pub id: String,
    pub category: WriteCategory,
    pub owner: WriteOwner,
    pub operation: WriteOperationKind,
    pub target: String,
    pub bytes_written: u64,
    pub started_at_ms: u128,
    pub completed_at_ms: u128,
    pub status: String,
}

/// Typed terminal failure returned by every public `WriteAuthority`
/// operation.
///
/// A rejection means no filesystem effect was accepted. A recovery receipt
/// means an effect may already be visible and callers must not turn the error
/// into an automatic retry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum WriteAuthorityError {
    Rejected(WriteRejection),
    RecoveryRequired(Box<WriteRecoveryReceipt>),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteRejection {
    pub diagnostic: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteRecoveryReceipt {
    pub receipt: WriteReceipt,
    pub diagnostic: String,
    retry_forbidden: bool,
}

impl WriteRejection {
    pub fn new(diagnostic: impl Into<String>) -> Self {
        Self {
            diagnostic: diagnostic.into(),
        }
    }
}

impl WriteRecoveryReceipt {
    pub fn new(mut receipt: WriteReceipt, diagnostic: impl Into<String>) -> Self {
        // Recovery is an error terminal, never an apparently successful
        // receipt. Keep the complete operation receipt for reconciliation.
        receipt.status = "recovery_required".to_string();
        Self {
            receipt,
            diagnostic: diagnostic.into(),
            retry_forbidden: true,
        }
    }

    pub const fn retry_forbidden(&self) -> bool {
        self.retry_forbidden
    }
}

impl WriteAuthorityError {
    pub fn diagnostic(&self) -> &str {
        match self {
            Self::Rejected(rejection) => &rejection.diagnostic,
            Self::RecoveryRequired(recovery) => &recovery.diagnostic,
        }
    }

    pub const fn retry_forbidden(&self) -> bool {
        match self {
            Self::Rejected(_) => false,
            Self::RecoveryRequired(recovery) => recovery.retry_forbidden(),
        }
    }

    pub fn receipt(&self) -> Option<&WriteReceipt> {
        match self {
            Self::Rejected(_) => None,
            Self::RecoveryRequired(recovery) => Some(&recovery.receipt),
        }
    }

    /// Explicit diagnostic boundary for domains whose public contract has not
    /// yet been migrated from `String`. Unlike a blanket `From` conversion,
    /// every use remains visible to audit and preserves the non-retryable
    /// recovery marker in the rendered diagnostic.
    pub fn into_terminal_diagnostic(self) -> String {
        self.to_string()
    }
}

impl From<String> for WriteAuthorityError {
    fn from(diagnostic: String) -> Self {
        Self::Rejected(WriteRejection::new(diagnostic))
    }
}

impl From<&str> for WriteAuthorityError {
    fn from(diagnostic: &str) -> Self {
        Self::Rejected(WriteRejection::new(diagnostic))
    }
}

impl fmt::Display for WriteAuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(rejection) => formatter.write_str(&rejection.diagnostic),
            Self::RecoveryRequired(recovery) => write!(
                formatter,
                "WRITE_RECOVERY_REQUIRED [{}]: {} Nu repeta operația automat.",
                recovery.receipt.id, recovery.diagnostic
            ),
        }
    }
}

impl std::error::Error for WriteAuthorityError {}

impl WritePolicy {
    pub fn internal_atomic() -> Self {
        Self {
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::LoggedAtomicFile,
            log_required: true,
        }
    }

    pub fn internal_lifecycle() -> Self {
        Self {
            atomicity: WriteAtomicity::FileLifecycle,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::LoggedAtomicFile,
            log_required: true,
        }
    }

    pub fn internal_append() -> Self {
        Self {
            atomicity: WriteAtomicity::AppendOnly,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::AppendOnlyJournal,
            log_required: true,
        }
    }

    pub fn mcp_projection_atomic() -> Self {
        Self {
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::SingleOwnerInternal,
            // MCP discovery/context are derived diagnostics. A crash may leave
            // either the old or new complete projection, but must never create
            // a global WAL barrier that blocks project source recovery.
            recovery: RecoveryPolicy::EphemeralRebuildable,
            log_required: true,
        }
    }

    pub fn scratch_state_atomic() -> Self {
        Self {
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::EphemeralRebuildable,
            log_required: true,
        }
    }

    pub fn workbench_projection_atomic() -> Self {
        Self {
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::EphemeralRebuildable,
            log_required: true,
        }
    }

    pub fn scratch_state_lifecycle() -> Self {
        Self {
            atomicity: WriteAtomicity::FileLifecycle,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::EphemeralRebuildable,
            log_required: true,
        }
    }

    pub fn project_workspace_write() -> Self {
        Self {
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::RequireDiskBaseline,
            recovery: RecoveryPolicy::HotRollbackJournal,
            log_required: true,
        }
    }

    pub fn project_workspace_remove() -> Self {
        Self {
            atomicity: WriteAtomicity::FileLifecycle,
            conflict: ConflictPolicy::RequireDiskBaseline,
            recovery: RecoveryPolicy::HotRollbackJournal,
            log_required: true,
        }
    }

    pub fn project_workspace_save_journal_write() -> Self {
        Self {
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::HotRollbackJournal,
            log_required: true,
        }
    }

    pub fn project_workspace_save_journal_remove() -> Self {
        Self {
            atomicity: WriteAtomicity::FileLifecycle,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::HotRollbackJournal,
            log_required: true,
        }
    }

    pub fn project_entry_rename() -> Self {
        Self {
            atomicity: WriteAtomicity::FileLifecycle,
            conflict: ConflictPolicy::RequireDiskBaseline,
            recovery: RecoveryPolicy::TransactionJournalRequired,
            log_required: true,
        }
    }

    pub fn project_creation_lifecycle() -> Self {
        Self {
            atomicity: WriteAtomicity::FileLifecycle,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::LoggedAtomicFile,
            log_required: true,
        }
    }

    pub fn external_config_update() -> Self {
        Self {
            atomicity: WriteAtomicity::ExternalToolWrite,
            conflict: ConflictPolicy::ExternalBackupRequired,
            recovery: RecoveryPolicy::BackupBeforeWrite,
            log_required: true,
        }
    }

    pub fn session_trash_retention_journal() -> Self {
        Self {
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::HotRollbackJournal,
            log_required: true,
        }
    }

    pub fn session_trash_retention_lifecycle() -> Self {
        Self {
            atomicity: WriteAtomicity::FileLifecycle,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::HotRollbackJournal,
            log_required: true,
        }
    }

    pub fn project_transition_decision_retention_journal() -> Self {
        Self {
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::HotRollbackJournal,
            log_required: true,
        }
    }

    pub fn project_transition_decision_retention_lifecycle() -> Self {
        Self {
            atomicity: WriteAtomicity::FileLifecycle,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::HotRollbackJournal,
            log_required: true,
        }
    }

    pub fn preview_workspace_atomic() -> Self {
        Self {
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::EphemeralRebuildable,
            log_required: true,
        }
    }

    pub fn preview_workspace_lifecycle() -> Self {
        Self {
            atomicity: WriteAtomicity::FileLifecycle,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::EphemeralRebuildable,
            log_required: true,
        }
    }

    pub fn build_output_atomic() -> Self {
        Self {
            atomicity: WriteAtomicity::AtomicRename,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::EphemeralRebuildable,
            log_required: true,
        }
    }

    pub fn build_output_lifecycle() -> Self {
        Self {
            atomicity: WriteAtomicity::FileLifecycle,
            conflict: ConflictPolicy::SingleOwnerInternal,
            recovery: RecoveryPolicy::EphemeralRebuildable,
            log_required: true,
        }
    }
}

impl WriteTarget {
    pub fn new(
        path: impl Into<PathBuf>,
        boundary_root: impl Into<PathBuf>,
        public_label: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            boundary_root: boundary_root.into(),
            public_label: public_label.into(),
            expected_leaf: ExpectedLeaf::Unspecified,
            expected_runtime_session_id: None,
            authority: None,
        }
    }

    pub fn with_expected_runtime_session_id(
        mut self,
        runtime_session_id: impl Into<String>,
    ) -> Self {
        self.expected_runtime_session_id = Some(runtime_session_id.into());
        self
    }

    pub(super) fn bind_authority(mut self, authority: DirectoryAuthority) -> Result<Self, String> {
        if !self.path.starts_with(authority.root_path()) {
            return Err(format!(
                "WriteTarget {} nu aparține authority root {}.",
                self.public_label,
                authority.root_path().display()
            ));
        }
        self.boundary_root = authority.root_path().to_path_buf();
        self.authority = Some(authority);
        Ok(self)
    }

    pub(super) fn authority(&self) -> Option<&DirectoryAuthority> {
        self.authority.as_ref()
    }

    pub fn with_expected_absent(mut self) -> Self {
        self.expected_leaf = ExpectedLeaf::Absent;
        self
    }

    pub fn with_expected_present(
        mut self,
        version_token: impl Into<String>,
        content_hash: Option<String>,
    ) -> Self {
        self.expected_leaf = ExpectedLeaf::Present(ExpectedLeafVersion {
            version_token: version_token.into(),
            content_hash,
            tree_fingerprint: None,
        });
        self
    }

    pub fn with_expected_present_tree(
        mut self,
        version_token: impl Into<String>,
        tree_fingerprint: impl Into<String>,
    ) -> Self {
        self.expected_leaf = ExpectedLeaf::Present(ExpectedLeafVersion {
            version_token: version_token.into(),
            content_hash: None,
            tree_fingerprint: Some(tree_fingerprint.into()),
        });
        self
    }
}

impl WriteIntent {
    pub fn new(
        category: WriteCategory,
        owner: WriteOwner,
        operation: WriteOperationKind,
        target: WriteTarget,
        policy: WritePolicy,
        description: impl Into<String>,
    ) -> Self {
        Self {
            category,
            owner,
            operation,
            target,
            policy,
            description: description.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        WriteAuthorityError, WriteCategory, WriteOperationKind, WriteOwner, WriteReceipt,
        WriteRecoveryReceipt,
    };

    #[test]
    fn string_conversion_is_a_zero_effect_rejection() {
        let error = WriteAuthorityError::from("input invalid");

        assert_eq!(error.diagnostic(), "input invalid");
        assert!(!error.retry_forbidden());
        assert!(error.receipt().is_none());
        assert!(matches!(error, WriteAuthorityError::Rejected(_)));
    }

    #[test]
    fn rejected_error_serializes_without_a_false_recovery_receipt() {
        let error = WriteAuthorityError::from("boundary rejected".to_string());
        let serialized = serde_json::to_value(&error).unwrap();

        assert_eq!(serialized["kind"], "rejected");
        assert_eq!(serialized["detail"]["diagnostic"], "boundary rejected");
        assert!(serialized["detail"].get("receipt").is_none());
        assert!(serialized["detail"].get("retryForbidden").is_none());
    }

    #[test]
    fn recovery_error_serializes_complete_receipt_and_forbids_retry() {
        let recovery = WriteRecoveryReceipt::new(receipt("committed"), "fsync uncertain");
        let error = WriteAuthorityError::RecoveryRequired(Box::new(recovery));
        let serialized = serde_json::to_value(&error).unwrap();

        assert!(error.retry_forbidden());
        assert_eq!(error.diagnostic(), "fsync uncertain");
        assert_eq!(error.receipt().unwrap().status, "recovery_required");
        assert_eq!(serialized["kind"], "recovery_required");
        assert_eq!(serialized["detail"]["diagnostic"], "fsync uncertain");
        assert_eq!(serialized["detail"]["retryForbidden"], true);
        assert_eq!(serialized["detail"]["receipt"]["id"], "write-test-1");
        assert_eq!(
            serialized["detail"]["receipt"]["category"],
            "project_source_write"
        );
        assert_eq!(
            serialized["detail"]["receipt"]["owner"],
            "project_workspace"
        );
        assert_eq!(serialized["detail"]["receipt"]["operation"], "write_text");
        assert_eq!(serialized["detail"]["receipt"]["target"], "project/a.txt");
        assert_eq!(serialized["detail"]["receipt"]["bytesWritten"], 7);
        assert_eq!(serialized["detail"]["receipt"]["startedAtMs"], 10);
        assert_eq!(serialized["detail"]["receipt"]["completedAtMs"], 20);
        assert_eq!(
            serialized["detail"]["receipt"]["status"],
            "recovery_required"
        );
    }

    fn receipt(status: &str) -> WriteReceipt {
        WriteReceipt {
            id: "write-test-1".to_string(),
            category: WriteCategory::ProjectSourceWrite,
            owner: WriteOwner::ProjectWorkspace,
            operation: WriteOperationKind::WriteText,
            target: "project/a.txt".to_string(),
            bytes_written: 7,
            started_at_ms: 10,
            completed_at_ms: 20,
            status: status.to_string(),
        }
    }
}
