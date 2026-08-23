use std::{collections::BTreeSet, path::Path};

use super::super::super::{
    model::WriteTarget,
    operation::{
        AppendOperationPlan, AtomicOperationPlan, CopyOperationPlan, DirectoryOperationPlan,
        ExternalConfigOperationPlan, RemoveLeafOperationPlan, RemoveTreeOperationPlan,
        RenameOperationPlan, SymlinkOperationPlan,
    },
    recovery::DurableWalGuard,
    root_authority::{DirectoryAuthority, DirectoryAuthorityScope},
};
use super::super::{
    CapabilityBoundedFileSnapshot, CapabilityEffect, CapabilityGenerationCloneStats,
    CapabilityLockMode, CapabilityReplacePolicy,
};

fn unsupported() -> Result<CapabilityEffect, String> {
    Err(
        "Capability filesystem nu este implementat pe această platformă; scrierea este blocată fail-closed."
            .to_string(),
    )
}

pub(in crate::kernel::write_authority::capability) struct CapabilityFileLock;

pub(in crate::kernel::write_authority::capability) struct CapabilityDirectoryLease;

impl CapabilityDirectoryLease {
    pub(in crate::kernel::write_authority::capability) fn current_dir_path(
        &self,
    ) -> std::path::PathBuf {
        std::path::PathBuf::new()
    }

    pub(in crate::kernel::write_authority::capability) fn require_empty(
        &self,
    ) -> Result<(), String> {
        unsupported().map(|_| ())
    }
}

pub(in crate::kernel::write_authority::capability) fn capture_directory_lease_from_authority(
    _authority: &DirectoryAuthority,
    _path: &Path,
    _public_label: &str,
) -> Result<CapabilityDirectoryLease, String> {
    Err(
        "Capability filesystem nu poate deriva cwd-ul din authority pe această platformă; operația este blocată fail-closed."
            .to_string(),
    )
}

pub(in crate::kernel::write_authority::capability) fn open_regular_file_readonly_no_follow(
    _path: &Path,
    _public_label: &str,
) -> Result<std::fs::File, String> {
    Err(
        "Capability filesystem read-only no-follow nu este implementat pe această platformă."
            .to_string(),
    )
}

pub(in crate::kernel::write_authority::capability) fn open_optional_regular_file_readonly_no_follow(
    _path: &Path,
    _public_label: &str,
) -> Result<Option<std::fs::File>, String> {
    Err(
        "Capability filesystem read-only no-follow nu este implementat pe această platformă."
            .to_string(),
    )
}

pub(in crate::kernel::write_authority::capability) fn read_bounded_regular_file_from_authority(
    _authority: &DirectoryAuthority,
    _path: &Path,
    _public_label: &str,
    _max_bytes: u64,
) -> Result<Option<CapabilityBoundedFileSnapshot>, String> {
    Err(
        "Capability filesystem bounded ProjectRoot read nu este implementat pe această platformă; operația este blocată fail-closed."
            .to_string(),
    )
}

pub(in crate::kernel::write_authority::capability) fn capture_directory_authority(
    _path: &Path,
    _public_label: &str,
    _scope: DirectoryAuthorityScope,
) -> Result<DirectoryAuthority, String> {
    Err(
        "Capability filesystem nu poate captura authority roots pe această platformă; operația este blocată fail-closed."
            .to_string(),
    )
}

pub(in crate::kernel::write_authority::capability) fn bootstrap_directory_authority(
    _path: &Path,
    _public_label: &str,
    _scope: DirectoryAuthorityScope,
) -> Result<DirectoryAuthority, String> {
    Err(
        "Capability filesystem nu poate bootstrap-a authority roots pe această platformă; operația este blocată fail-closed."
            .to_string(),
    )
}

pub(in crate::kernel::write_authority::capability) fn create_directory_from_authority(
    _authority: &DirectoryAuthority,
    _path: &Path,
    _public_label: &str,
) -> Result<(), String> {
    unsupported().map(|_| ())
}

pub(in crate::kernel::write_authority::capability) fn capture_descendant_authority(
    _parent: &DirectoryAuthority,
    _path: &Path,
    _public_label: &str,
    _scope: DirectoryAuthorityScope,
) -> Result<DirectoryAuthority, String> {
    Err(
        "Capability filesystem nu poate deriva authority roots pe această platformă; operația este blocată fail-closed."
            .to_string(),
    )
}

pub(in crate::kernel::write_authority::capability) fn verify_directory_authority_path(
    _authority: &DirectoryAuthority,
) -> Result<(), String> {
    unsupported().map(|_| ())
}

pub(in crate::kernel::write_authority::capability) fn create_private_rebuildable_directory(
    _authority: &DirectoryAuthority,
    _path: &Path,
    _public_label: &str,
) -> Result<(), String> {
    unsupported().map(|_| ())
}

pub(in crate::kernel::write_authority::capability) fn create_rebuildable_generation_directory(
    _authority: &DirectoryAuthority,
    _relative_path: &Path,
    _public_label: &str,
) -> Result<(), String> {
    unsupported().map(|_| ())
}

pub(in crate::kernel::write_authority::capability) fn write_rebuildable_generation_file(
    _authority: &DirectoryAuthority,
    _relative_path: &Path,
    _bytes: &[u8],
    _public_label: &str,
) -> Result<(), String> {
    unsupported().map(|_| ())
}

pub(in crate::kernel::write_authority::capability) fn clone_rebuildable_generation_tree(
    _source_authority: &DirectoryAuthority,
    _target_authority: &DirectoryAuthority,
    _excluded: &BTreeSet<std::path::PathBuf>,
    _max_entries: usize,
    _max_bytes: u64,
    _public_label: &str,
) -> Result<CapabilityGenerationCloneStats, String> {
    unsupported().map(|_| CapabilityGenerationCloneStats::default())
}

pub(in crate::kernel::write_authority::capability) fn seal_rebuildable_generation(
    _authority: &DirectoryAuthority,
    _public_label: &str,
) -> Result<(), String> {
    unsupported().map(|_| ())
}

pub(in crate::kernel::write_authority::capability) fn is_real_directory_leaf(
    _authority: &DirectoryAuthority,
    _path: &Path,
    _public_label: &str,
) -> Result<bool, String> {
    unsupported().map(|_| false)
}

pub(in crate::kernel::write_authority::capability) fn lock_file(
    _target: &WriteTarget,
    _mode: CapabilityLockMode,
) -> Result<CapabilityFileLock, String> {
    Err(
        "Capability filesystem nu este implementat pe această platformă; lock-ul este blocat fail-closed."
            .to_string(),
    )
}

pub(in crate::kernel::write_authority::capability) fn atomic_write(
    _target: &WriteTarget,
    _bytes: &[u8],
    _replace_policy: CapabilityReplacePolicy,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

pub(in crate::kernel::write_authority::capability) fn copy_rebuildable_file(
    _target: &WriteTarget,
    _source: &Path,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

pub(in crate::kernel::write_authority::capability) fn create_directory_all(
    _target: &WriteTarget,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

pub(in crate::kernel::write_authority::capability) fn plan_atomic_write(
    _target: &WriteTarget,
    _bytes: &[u8],
    _replace_policy: CapabilityReplacePolicy,
    _operation_id: &str,
) -> Result<AtomicOperationPlan, String> {
    Err("WriteAuthority WAL este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn atomic_write_wal(
    _target: &WriteTarget,
    _bytes: &[u8],
    _replace_policy: CapabilityReplacePolicy,
    _plan: &AtomicOperationPlan,
    _guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

pub(in crate::kernel::write_authority::capability) fn classify_atomic_recovery(
    _record: &super::super::super::recovery::WalRecord,
    _phase: super::super::super::recovery::WalPhase,
    _read_budget: &mut super::super::super::recovery::RecoveryReadBudget,
) -> Result<super::super::super::recovery::AtomicRecoveryAssessment, String> {
    Err("WriteAuthority WAL recovery este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn execute_atomic_recovery(
    _record: &super::super::super::recovery::WalRecord,
    _phase: super::super::super::recovery::WalPhase,
    _read_budget: &mut super::super::super::recovery::RecoveryReadBudget,
) -> Result<(), String> {
    Err("WriteAuthority WAL recovery este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn discard_rebuildable_atomic_projection(
    _record: &super::super::super::recovery::WalRecord,
    _phase: super::super::super::recovery::WalPhase,
) -> Result<(), String> {
    Err("Cleanup-ul proiecției rebuildable este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn resolve_atomic_operator(
    _record: &super::super::super::recovery::WalRecord,
    _phase: super::super::super::recovery::WalPhase,
    _action: super::super::super::recovery::WriteAuthorityRecoveryResolutionAction,
) -> Result<String, String> {
    Err("WriteAuthority AtomicFile operator recovery este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn classify_append_recovery(
    _record: &super::super::super::recovery::WalRecord,
    _phase: super::super::super::recovery::WalPhase,
    _checkpoint: Option<&super::super::super::recovery::WalAppendStageCheckpoint>,
    _read_budget: &mut super::super::super::recovery::RecoveryReadBudget,
) -> Result<super::super::super::recovery::AppendRecoveryAssessment, String> {
    Err("WriteAuthority append WAL recovery este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn classify_directory_recovery(
    _record: &super::super::super::recovery::WalRecord,
    _phase: super::super::super::recovery::WalPhase,
    _checkpoint: Option<&super::super::super::recovery::WalDirectoryStageCheckpoint>,
) -> Result<super::super::super::recovery::DirectoryRecoveryAssessment, String> {
    Err("WriteAuthority mkdir WAL recovery este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn execute_directory_recovery(
    _record: &super::super::super::recovery::WalRecord,
    _phase: super::super::super::recovery::WalPhase,
    _checkpoint: Option<&super::super::super::recovery::WalDirectoryStageCheckpoint>,
    _action: super::super::super::recovery::DirectoryRecoveryAction,
) -> Result<(), String> {
    Err("WriteAuthority mkdir WAL recovery este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn resolve_directory_operator(
    _record: &super::super::super::recovery::WalRecord,
    _phase: super::super::super::recovery::WalPhase,
    _checkpoint: Option<&super::super::super::recovery::WalDirectoryStageCheckpoint>,
    _action: super::super::super::recovery::WriteAuthorityRecoveryResolutionAction,
    _expected_evidence_hash: &str,
    _wal_evidence_binding_hash: &str,
) -> Result<String, String> {
    Err("WriteAuthority Directory operator recovery este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn classify_symlink_recovery(
    _record: &super::super::super::recovery::WalRecord,
    _phase: super::super::super::recovery::WalPhase,
    _checkpoint: Option<&super::super::super::recovery::WalSymlinkStageCheckpoint>,
) -> Result<super::super::super::recovery::SymlinkRecoveryAssessment, String> {
    Err("WriteAuthority symlink WAL recovery este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn execute_symlink_recovery(
    _record: &super::super::super::recovery::WalRecord,
    _phase: super::super::super::recovery::WalPhase,
    _checkpoint: Option<&super::super::super::recovery::WalSymlinkStageCheckpoint>,
    _action: super::super::super::recovery::SymlinkRecoveryAction,
) -> Result<(), String> {
    Err("WriteAuthority symlink WAL recovery este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn resolve_symlink_operator(
    _record: &super::super::super::recovery::WalRecord,
    _phase: super::super::super::recovery::WalPhase,
    _checkpoint: Option<&super::super::super::recovery::WalSymlinkStageCheckpoint>,
    _action: super::super::super::recovery::WriteAuthorityRecoveryResolutionAction,
    _expected_evidence_hash: &str,
    _wal_evidence_binding_hash: &str,
) -> Result<String, String> {
    Err("WriteAuthority Symlink operator recovery este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn execute_append_recovery(
    _record: &super::super::super::recovery::WalRecord,
    _phase: super::super::super::recovery::WalPhase,
    _checkpoint: Option<&super::super::super::recovery::WalAppendStageCheckpoint>,
    _read_budget: &mut super::super::super::recovery::RecoveryReadBudget,
) -> Result<(), String> {
    Err("WriteAuthority append WAL recovery este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn plan_external_config(
    _target: &WriteTarget,
    _bytes: &[u8],
    _backup: Option<(&WriteTarget, &[u8])>,
    _operation_id: &str,
) -> Result<ExternalConfigOperationPlan, String> {
    Err("WriteAuthority ExternalConfig WAL este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn external_config_update_wal(
    _target: &WriteTarget,
    _bytes: &[u8],
    _backup: Option<(&WriteTarget, &[u8])>,
    _plan: ExternalConfigOperationPlan,
    _guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

pub(in crate::kernel::write_authority::capability) fn classify_external_config_recovery(
    _record: &super::super::super::recovery::WalRecord,
    _phase: super::super::super::recovery::WalPhase,
    _checkpoint: Option<&super::super::super::recovery::WalExternalStageCheckpoint>,
    _decision: Option<super::super::super::recovery::WalExternalOperatorDecision>,
    _read_budget: &mut super::super::super::recovery::RecoveryReadBudget,
) -> Result<super::super::super::recovery::ExternalConfigRecoveryAssessment, String> {
    Err("WriteAuthority ExternalConfig recovery este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn execute_external_config_recovery(
    _record: &super::super::super::recovery::WalRecord,
    _phase: super::super::super::recovery::WalPhase,
    _checkpoint: Option<&super::super::super::recovery::WalExternalStageCheckpoint>,
    _decision: Option<super::super::super::recovery::WalExternalOperatorDecision>,
    _read_budget: &mut super::super::super::recovery::RecoveryReadBudget,
) -> Result<(), String> {
    Err("WriteAuthority ExternalConfig recovery este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn append(
    _target: &WriteTarget,
    _bytes: &[u8],
) -> Result<CapabilityEffect, String> {
    unsupported()
}

pub(in crate::kernel::write_authority::capability) fn plan_append(
    _target: &WriteTarget,
    _bytes: &[u8],
) -> Result<AppendOperationPlan, String> {
    Err("WriteAuthority append WAL este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn append_wal(
    _target: &WriteTarget,
    _bytes: &[u8],
    _plan: AppendOperationPlan,
    _guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

pub(in crate::kernel::write_authority::capability) fn plan_copy(
    _target: &WriteTarget,
    _source: &Path,
    _replace_policy: CapabilityReplacePolicy,
    _operation_id: &str,
) -> Result<CopyOperationPlan, String> {
    Err("WriteAuthority copy WAL este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn copy_file_wal(
    _target: &WriteTarget,
    _source: &Path,
    _replace_policy: CapabilityReplacePolicy,
    _plan: CopyOperationPlan,
    _guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

pub(in crate::kernel::write_authority::capability) fn classify_copy_recovery(
    _record: &super::super::super::recovery::WalRecord,
    _phase: super::super::super::recovery::WalPhase,
    _checkpoint: Option<&super::super::super::recovery::WalCopyStageCheckpoint>,
) -> Result<super::super::super::recovery::CopyRecoveryAssessment, String> {
    Err("WriteAuthority copy recovery este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn execute_copy_recovery(
    _record: &super::super::super::recovery::WalRecord,
    _phase: super::super::super::recovery::WalPhase,
    _checkpoint: Option<&super::super::super::recovery::WalCopyStageCheckpoint>,
    _read_budget: &mut super::super::super::recovery::RecoveryReadBudget,
) -> Result<(), String> {
    Err("WriteAuthority copy recovery este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn resolve_copy_operator(
    _record: &super::super::super::recovery::WalRecord,
    _phase: super::super::super::recovery::WalPhase,
    _checkpoint: Option<&super::super::super::recovery::WalCopyStageCheckpoint>,
    _action: super::super::super::recovery::WriteAuthorityRecoveryResolutionAction,
) -> Result<String, String> {
    Err("WriteAuthority Copy operator recovery este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn plan_rename(
    _source: &WriteTarget,
    _destination: &WriteTarget,
) -> Result<RenameOperationPlan, String> {
    Err("WriteAuthority rename WAL este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn rename_entry_wal(
    _source: &WriteTarget,
    _destination: &WriteTarget,
    _plan: RenameOperationPlan,
    _guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

pub(in crate::kernel::write_authority::capability) fn classify_rename_recovery(
    _record: &super::super::super::recovery::WalRecord,
    _phase: super::super::super::recovery::WalPhase,
) -> Result<super::super::super::recovery::RenameRecoveryAssessment, String> {
    Err("WriteAuthority rename recovery este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn plan_remove_leaf(
    _target: &WriteTarget,
    _operation_id: &str,
) -> Result<Option<RemoveLeafOperationPlan>, String> {
    Err("WriteAuthority RemoveFile WAL este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn remove_leaf_wal(
    _target: &WriteTarget,
    _plan: RemoveLeafOperationPlan,
    _guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

pub(in crate::kernel::write_authority::capability) fn classify_remove_leaf_recovery(
    _record: &super::super::super::recovery::WalRecord,
    _phase: super::super::super::recovery::WalPhase,
) -> Result<super::super::super::recovery::RemoveLeafRecoveryAssessment, String> {
    Err("WriteAuthority RemoveFile recovery este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn execute_remove_leaf_recovery(
    _record: &super::super::super::recovery::WalRecord,
    _phase: super::super::super::recovery::WalPhase,
) -> Result<(), String> {
    Err("WriteAuthority RemoveFile recovery este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn resolve_remove_leaf_operator(
    _record: &super::super::super::recovery::WalRecord,
    _phase: super::super::super::recovery::WalPhase,
    _action: super::super::super::recovery::WriteAuthorityRecoveryResolutionAction,
) -> Result<String, String> {
    Err("WriteAuthority RemoveFile operator recovery este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn plan_remove_tree(
    _target: &WriteTarget,
    _operation_id: &str,
) -> Result<Option<RemoveTreeOperationPlan>, String> {
    Err("WriteAuthority RemoveDirectoryTree WAL este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn remove_tree_wal(
    _target: &WriteTarget,
    _plan: RemoveTreeOperationPlan,
    _guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

pub(in crate::kernel::write_authority::capability) fn classify_remove_tree_recovery(
    _record: &super::super::super::recovery::WalRecord,
    _phase: super::super::super::recovery::WalPhase,
) -> Result<super::super::super::recovery::RemoveTreeRecoveryAssessment, String> {
    Err("WriteAuthority RemoveDirectoryTree recovery este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn execute_remove_tree_recovery(
    _record: &super::super::super::recovery::WalRecord,
    _phase: super::super::super::recovery::WalPhase,
) -> Result<(), String> {
    Err("WriteAuthority RemoveDirectoryTree recovery este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn resolve_remove_tree_operator(
    _record: &super::super::super::recovery::WalRecord,
    _phase: super::super::super::recovery::WalPhase,
    _action: super::super::super::recovery::WriteAuthorityRecoveryResolutionAction,
) -> Result<String, String> {
    Err(
        "WriteAuthority RemoveDirectoryTree operator recovery este fail-closed în afara Linux."
            .into(),
    )
}

pub(in crate::kernel::write_authority::capability) fn execute_rename_recovery(
    _record: &super::super::super::recovery::WalRecord,
    _phase: super::super::super::recovery::WalPhase,
) -> Result<(), String> {
    Err("WriteAuthority rename recovery este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn plan_directory(
    _target: &WriteTarget,
) -> Result<DirectoryOperationPlan, String> {
    Err("WriteAuthority mkdir WAL este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn create_directory_all_wal(
    _target: &WriteTarget,
    _plan: &DirectoryOperationPlan,
    _guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

pub(in crate::kernel::write_authority::capability) fn plan_symlink(
    _target: &WriteTarget,
    _source: &Path,
) -> Result<SymlinkOperationPlan, String> {
    Err("WriteAuthority symlink WAL este fail-closed în afara Linux.".into())
}

pub(in crate::kernel::write_authority::capability) fn symlink_entry_wal(
    _target: &WriteTarget,
    _source: &Path,
    _plan: &SymlinkOperationPlan,
    _guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

pub(in crate::kernel::write_authority::capability) fn remove_file_if_exists(
    _target: &WriteTarget,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

pub(in crate::kernel::write_authority::capability) fn rename_noreplace(
    _source: &WriteTarget,
    _destination: &WriteTarget,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

pub(in crate::kernel::write_authority::capability) fn publish_rebuildable_directory(
    _source: &WriteTarget,
    _destination: &WriteTarget,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

pub(in crate::kernel::write_authority::capability) fn remove_rebuildable_tree(
    _target: &WriteTarget,
    _operation_id: &str,
) -> Result<CapabilityEffect, String> {
    unsupported()
}
