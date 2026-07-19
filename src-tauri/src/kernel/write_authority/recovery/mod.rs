mod coordinator;
mod executor;
mod journal;
mod model;
mod operator;
mod paths;
mod scan;
mod wal_io;

pub(crate) use coordinator::{DurableWalGuard, RecoveryCoordinator};
pub(crate) use model::{
    AppendRecoveryAction, AppendRecoveryAssessment, AtomicRecoveryAction, AtomicRecoveryAssessment,
    CopyRecoveryAction, CopyRecoveryAssessment, DirectoryRecoveryAction,
    DirectoryRecoveryAssessment, DirectoryResolutionStateBinding, ExternalConfigRecoveryAction,
    ExternalConfigRecoveryAssessment, RecoveryReadBudget, RemoveLeafRecoveryAction,
    RemoveLeafRecoveryAssessment, RemoveTreeRecoveryAction, RemoveTreeRecoveryAssessment,
    RenameRecoveryAction, RenameRecoveryAssessment, SymlinkRecoveryAction,
    SymlinkRecoveryAssessment, SymlinkResolutionStateBinding, WalAppendBefore, WalAppendEvidence,
    WalAtomicFileEvidence, WalAuthorityEvidence, WalCopyDestinationPolicy, WalCopyEvidence,
    WalCopySourceEvidence, WalDirectoryEvidence, WalExternalConfigEvidence, WalFilesystemIdentity,
    WalLeafEvidence, WalOperationEvidence, WalParentEvidence, WalRecord, WalRecordBody,
    WalRemoveLeafEvidence, WalRemoveLeafKind, WalRemoveLeafSourceEvidence, WalRemoveTreeEvidence,
    WalRemoveTreeSourceEvidence, WalRenameEvidence, WalRenameLeafKind, WalRenameSourceEvidence,
    WalSymlinkBefore, WalSymlinkEvidence, MAX_WAL_APPEND_PAYLOAD_BYTES,
    MAX_WAL_APPEND_PREFIX_BYTES, MAX_WAL_APPEND_TAIL_BYTES, MAX_WAL_COPY_BYTES,
    MAX_WAL_EXTERNAL_CONFIG_BYTES, MAX_WAL_RECOVERY_READ_BYTES, MAX_WAL_SYMLINK_TARGET_BYTES,
    WAL_APPEND_PROTOCOL_VERSION, WAL_COPY_PROTOCOL_VERSION, WAL_DIRECTORY_PROTOCOL_VERSION,
    WAL_EXTERNAL_CONFIG_PROTOCOL_VERSION, WAL_SCHEMA_VERSION, WAL_SYMLINK_PROTOCOL_VERSION,
};
pub use model::{
    WalPhase, WriteAuthorityRecoveryClassification, WriteAuthorityRecoveryItem,
    WriteAuthorityRecoveryResolutionAction, WriteAuthorityRecoveryResolutionInput,
    WriteAuthorityRecoveryResolutionReceipt, WriteAuthorityRecoveryScan,
};
pub(crate) use paths::{
    decode_bytes_hex, decode_component_hex, decode_path_hex, encode_bytes_hex,
    encode_component_hex, encode_path_hex, WalAppendStageCheckpoint, WalAppendStageRole,
    WalCopyStageCheckpoint, WalCopyStageRole, WalDirectoryStageCheckpoint,
    WalExternalOperatorDecision, WalExternalStageCheckpoint, WalSymlinkStageCheckpoint,
};

#[cfg(test)]
mod tests;
