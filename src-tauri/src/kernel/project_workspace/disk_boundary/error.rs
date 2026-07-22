use std::fmt;

use serde::Serialize;

use crate::kernel::{
    file_buffer_store::FileBufferSaveStamp,
    write_authority::{WriteAuthorityError, WriteReceipt, WriteRecoveryReceipt},
};

pub const PROJECT_WORKSPACE_DISK_RECOVERY_RECEIPT_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectWorkspaceDiskRecoveryPhase {
    WriteAuthority,
    FileBufferProjection,
}

/// Terminal Save failure after a filesystem effect may already be visible.
///
/// `write_recovery` is populated when WriteAuthority itself could not prove
/// durability. `committed_write` is populated when the disk commit succeeded
/// but FileBufferStore could not project the committed state. Both variants
/// are non-retryable until recovery/reload reconciles disk and memory.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkspaceDiskRecoveryReceipt {
    pub schema_version: u32,
    pub relative_path: String,
    pub phase: ProjectWorkspaceDiskRecoveryPhase,
    pub diagnostic: String,
    pub write_recovery: Option<WriteRecoveryReceipt>,
    pub committed_write: Option<WriteReceipt>,
    pub file_buffer_before: Option<FileBufferSaveStamp>,
    pub file_buffer_after: Option<FileBufferSaveStamp>,
    retry_forbidden: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum ProjectWorkspaceDiskError {
    Rejected { diagnostic: String },
    RecoveryRequired(Box<ProjectWorkspaceDiskRecoveryReceipt>),
}

impl ProjectWorkspaceDiskRecoveryReceipt {
    pub fn from_write_authority(
        relative_path: impl Into<String>,
        recovery: WriteRecoveryReceipt,
    ) -> Self {
        Self {
            schema_version: PROJECT_WORKSPACE_DISK_RECOVERY_RECEIPT_SCHEMA_VERSION,
            relative_path: relative_path.into(),
            phase: ProjectWorkspaceDiskRecoveryPhase::WriteAuthority,
            diagnostic: recovery.diagnostic.clone(),
            write_recovery: Some(recovery),
            committed_write: None,
            file_buffer_before: None,
            file_buffer_after: None,
            retry_forbidden: true,
        }
    }

    pub fn file_buffer_projection(
        relative_path: impl Into<String>,
        committed_write: WriteReceipt,
        diagnostic: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: PROJECT_WORKSPACE_DISK_RECOVERY_RECEIPT_SCHEMA_VERSION,
            relative_path: relative_path.into(),
            phase: ProjectWorkspaceDiskRecoveryPhase::FileBufferProjection,
            diagnostic: diagnostic.into(),
            write_recovery: None,
            committed_write: Some(committed_write),
            file_buffer_before: None,
            file_buffer_after: None,
            retry_forbidden: true,
        }
    }

    pub fn file_buffer_projection_with_stamps(
        relative_path: impl Into<String>,
        committed_write: WriteReceipt,
        file_buffer_before: Option<FileBufferSaveStamp>,
        file_buffer_after: Option<FileBufferSaveStamp>,
        diagnostic: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: PROJECT_WORKSPACE_DISK_RECOVERY_RECEIPT_SCHEMA_VERSION,
            relative_path: relative_path.into(),
            phase: ProjectWorkspaceDiskRecoveryPhase::FileBufferProjection,
            diagnostic: diagnostic.into(),
            write_recovery: None,
            committed_write: Some(committed_write),
            file_buffer_before,
            file_buffer_after,
            retry_forbidden: true,
        }
    }
}

impl ProjectWorkspaceDiskError {
    pub fn rejected(diagnostic: impl Into<String>) -> Self {
        Self::Rejected {
            diagnostic: diagnostic.into(),
        }
    }

    pub fn from_write_authority(
        relative_path: impl Into<String>,
        error: WriteAuthorityError,
    ) -> Self {
        match error {
            WriteAuthorityError::Rejected(rejection) => Self::rejected(rejection.diagnostic),
            WriteAuthorityError::RecoveryRequired(recovery) => Self::RecoveryRequired(Box::new(
                ProjectWorkspaceDiskRecoveryReceipt::from_write_authority(relative_path, *recovery),
            )),
        }
    }

    pub fn file_buffer_projection(
        relative_path: impl Into<String>,
        committed_write: WriteReceipt,
        diagnostic: impl Into<String>,
    ) -> Self {
        Self::RecoveryRequired(Box::new(
            ProjectWorkspaceDiskRecoveryReceipt::file_buffer_projection(
                relative_path,
                committed_write,
                diagnostic,
            ),
        ))
    }

    pub fn file_buffer_projection_with_stamps(
        relative_path: impl Into<String>,
        committed_write: WriteReceipt,
        file_buffer_before: Option<FileBufferSaveStamp>,
        file_buffer_after: Option<FileBufferSaveStamp>,
        diagnostic: impl Into<String>,
    ) -> Self {
        Self::RecoveryRequired(Box::new(
            ProjectWorkspaceDiskRecoveryReceipt::file_buffer_projection_with_stamps(
                relative_path,
                committed_write,
                file_buffer_before,
                file_buffer_after,
                diagnostic,
            ),
        ))
    }

    pub fn diagnostic(&self) -> &str {
        match self {
            Self::Rejected { diagnostic } => diagnostic,
            Self::RecoveryRequired(recovery) => &recovery.diagnostic,
        }
    }

    pub fn recovery(&self) -> Option<&ProjectWorkspaceDiskRecoveryReceipt> {
        match self {
            Self::Rejected { .. } => None,
            Self::RecoveryRequired(recovery) => Some(recovery.as_ref()),
        }
    }
}

impl From<String> for ProjectWorkspaceDiskError {
    fn from(diagnostic: String) -> Self {
        Self::rejected(diagnostic)
    }
}

impl From<&str> for ProjectWorkspaceDiskError {
    fn from(diagnostic: &str) -> Self {
        Self::rejected(diagnostic)
    }
}

impl fmt::Display for ProjectWorkspaceDiskError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected { diagnostic } => formatter.write_str(diagnostic),
            Self::RecoveryRequired(recovery) => write!(
                formatter,
                "SAVE_RECOVERY_REQUIRED [{}]: {} Nu repeta operația automat.",
                recovery.relative_path, recovery.diagnostic
            ),
        }
    }
}

impl std::error::Error for ProjectWorkspaceDiskError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::write_authority::{
        WriteCategory, WriteOperationKind, WriteOwner, WriteReceipt,
    };

    #[test]
    fn projection_failure_keeps_committed_write_evidence() {
        let receipt = write_receipt("committed");
        let error = ProjectWorkspaceDiskError::file_buffer_projection(
            "templates/index.html",
            receipt.clone(),
            "FileBufferStore projection failed",
        );

        let recovery = error.recovery().expect("recovery receipt");
        assert_eq!(
            recovery.phase,
            ProjectWorkspaceDiskRecoveryPhase::FileBufferProjection
        );
        assert_eq!(recovery.committed_write.as_ref(), Some(&receipt));
        assert!(recovery.retry_forbidden);
    }

    fn write_receipt(status: &str) -> WriteReceipt {
        WriteReceipt {
            id: "write-save-1".to_string(),
            category: WriteCategory::ProjectSourceWrite,
            owner: WriteOwner::ProjectWorkspace,
            operation: WriteOperationKind::WriteText,
            target: "project/templates/index.html".to_string(),
            bytes_written: 12,
            started_at_ms: 1,
            completed_at_ms: 2,
            status: status.to_string(),
        }
    }
}
