//! Directory-handle filesystem backend for `WriteAuthority`.
//!
//! The security boundary in this module is an opened directory, not an
//! absolute pathname. Absolute paths are used only while acquiring that
//! capability from `/`, one normal component at a time. Once the boundary is
//! captured, every mutating syscall is relative to a held directory handle.

use std::{collections::BTreeSet, path::Path};

use super::{
    model::{ExpectedLeaf, ExpectedLeafVersion, WriteTarget},
    operation::{
        AppendOperationPlan, AtomicOperationPlan, CopyOperationPlan, DirectoryOperationPlan,
        ExternalConfigOperationPlan, RemoveLeafOperationPlan, RemoveTreeOperationPlan,
        RenameOperationPlan, SymlinkOperationPlan,
    },
    recovery::DurableWalGuard,
    root_authority::{DirectoryAuthority, DirectoryAuthorityScope},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CapabilityReplacePolicy {
    Replace,
    CreateNew,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CapabilityLockMode {
    Shared,
    Exclusive,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct CapabilityEffect {
    pub changed: bool,
    pub bytes_written: u64,
    pub recovery_required: bool,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CapabilityBoundedFileSnapshot {
    pub bytes: Vec<u8>,
    pub version_token: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct CapabilityGenerationCloneStats {
    pub entries: usize,
    pub bytes: u64,
    pub reflinked_files: usize,
    pub copied_files: usize,
}

impl CapabilityEffect {
    pub(super) const fn unchanged() -> Self {
        Self {
            changed: false,
            bytes_written: 0,
            recovery_required: false,
            diagnostic: None,
        }
    }

    const fn changed(bytes_written: u64) -> Self {
        Self {
            changed: true,
            bytes_written,
            recovery_required: false,
            diagnostic: None,
        }
    }

    pub(super) fn recovery_required(bytes_written: u64, diagnostic: impl Into<String>) -> Self {
        Self {
            changed: true,
            bytes_written,
            recovery_required: true,
            diagnostic: Some(diagnostic.into()),
        }
    }
}

mod platform;

pub(super) struct CapabilityFileLock {
    _inner: platform::CapabilityFileLock,
}

pub(super) struct CapabilityDirectoryLease {
    inner: platform::CapabilityDirectoryLease,
}

impl CapabilityDirectoryLease {
    pub(super) fn current_dir_path(&self) -> std::path::PathBuf {
        self.inner.current_dir_path()
    }
}

pub(super) fn capture_directory_lease_from_authority(
    authority: &DirectoryAuthority,
    path: &Path,
    public_label: &str,
) -> Result<CapabilityDirectoryLease, String> {
    platform::capture_directory_lease_from_authority(authority, path, public_label)
        .map(|inner| CapabilityDirectoryLease { inner })
}

pub(super) fn open_regular_file_readonly_no_follow(
    path: &Path,
    public_label: &str,
) -> Result<std::fs::File, String> {
    platform::open_regular_file_readonly_no_follow(path, public_label)
}

pub(super) fn open_optional_regular_file_readonly_no_follow(
    path: &Path,
    public_label: &str,
) -> Result<Option<std::fs::File>, String> {
    platform::open_optional_regular_file_readonly_no_follow(path, public_label)
}

pub(super) fn read_bounded_regular_file_from_authority(
    authority: &DirectoryAuthority,
    path: &Path,
    public_label: &str,
    max_bytes: u64,
) -> Result<Option<CapabilityBoundedFileSnapshot>, String> {
    platform::read_bounded_regular_file_from_authority(authority, path, public_label, max_bytes)
}

pub(super) fn capture_directory_authority(
    path: &Path,
    public_label: &str,
    scope: DirectoryAuthorityScope,
) -> Result<DirectoryAuthority, String> {
    platform::capture_directory_authority(path, public_label, scope)
}

pub(super) fn bootstrap_directory_authority(
    path: &Path,
    public_label: &str,
    scope: DirectoryAuthorityScope,
) -> Result<DirectoryAuthority, String> {
    platform::bootstrap_directory_authority(path, public_label, scope)
}

pub(super) fn create_directory_from_authority(
    authority: &DirectoryAuthority,
    path: &Path,
    public_label: &str,
) -> Result<(), String> {
    platform::create_directory_from_authority(authority, path, public_label)
}

pub(super) fn capture_descendant_authority(
    parent: &DirectoryAuthority,
    path: &Path,
    public_label: &str,
    scope: DirectoryAuthorityScope,
) -> Result<DirectoryAuthority, String> {
    platform::capture_descendant_authority(parent, path, public_label, scope)
}

pub(super) fn verify_directory_authority_path(
    authority: &DirectoryAuthority,
) -> Result<(), String> {
    platform::verify_directory_authority_path(authority)
}

pub(super) fn create_private_rebuildable_directory(
    authority: &DirectoryAuthority,
    path: &Path,
    public_label: &str,
) -> Result<(), String> {
    platform::create_private_rebuildable_directory(authority, path, public_label)
}

pub(super) fn create_rebuildable_generation_directory(
    authority: &DirectoryAuthority,
    relative_path: &Path,
    public_label: &str,
) -> Result<(), String> {
    platform::create_rebuildable_generation_directory(authority, relative_path, public_label)
}

pub(super) fn write_rebuildable_generation_file(
    authority: &DirectoryAuthority,
    relative_path: &Path,
    bytes: &[u8],
    public_label: &str,
) -> Result<(), String> {
    platform::write_rebuildable_generation_file(authority, relative_path, bytes, public_label)
}

pub(super) fn clone_rebuildable_generation_tree(
    source_authority: &DirectoryAuthority,
    target_authority: &DirectoryAuthority,
    excluded: &BTreeSet<std::path::PathBuf>,
    max_entries: usize,
    max_bytes: u64,
    public_label: &str,
) -> Result<CapabilityGenerationCloneStats, String> {
    platform::clone_rebuildable_generation_tree(
        source_authority,
        target_authority,
        excluded,
        max_entries,
        max_bytes,
        public_label,
    )
}

pub(super) fn seal_rebuildable_generation(
    authority: &DirectoryAuthority,
    public_label: &str,
) -> Result<(), String> {
    platform::seal_rebuildable_generation(authority, public_label)
}

pub(super) fn is_real_directory_leaf(
    authority: &DirectoryAuthority,
    path: &Path,
    public_label: &str,
) -> Result<bool, String> {
    platform::is_real_directory_leaf(authority, path, public_label)
}

pub(super) fn lock_file(
    target: &WriteTarget,
    mode: CapabilityLockMode,
) -> Result<CapabilityFileLock, String> {
    platform::lock_file(target, mode).map(|inner| CapabilityFileLock { _inner: inner })
}

pub(super) fn atomic_write(
    target: &WriteTarget,
    bytes: &[u8],
    replace_policy: CapabilityReplacePolicy,
) -> Result<CapabilityEffect, String> {
    platform::atomic_write(target, bytes, replace_policy)
        .map(|effect| settle_authority_postflight(effect, &[target]))
}

pub(super) fn copy_rebuildable_file(
    target: &WriteTarget,
    source: &Path,
) -> Result<CapabilityEffect, String> {
    platform::copy_rebuildable_file(target, source)
        .map(|effect| settle_authority_postflight(effect, &[target]))
}

pub(super) fn create_component_validation_directory(
    target: &WriteTarget,
) -> Result<CapabilityEffect, String> {
    if target.expected_leaf != ExpectedLeaf::Absent
        || !matches!(
            target.authority().map(DirectoryAuthority::scope),
            Some(DirectoryAuthorityScope::ComponentValidation { .. })
        )
    {
        return Err(
            "Capability filesystem a refuzat crearea unui sandbox de validare fără authority și baseline Absent."
                .to_string(),
        );
    }
    let effect = platform::create_directory_all(target)?;
    let effect = settle_authority_postflight(effect, &[target]);
    if !effect.changed && !effect.recovery_required {
        return Err(
            "Capability filesystem a refuzat reutilizarea unui sandbox de validare existent."
                .to_string(),
        );
    }
    Ok(effect)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_external_backup_committed_test_hook<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_external_backup_committed_test_hook(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_external_baseline_relocated_test_hook<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_external_baseline_relocated_test_hook(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_after_bounded_read_leaf_opened_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_after_bounded_read_leaf_opened_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_external_post_publication_test_hook<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_external_post_publication_test_hook(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_before_external_target_durable_test_hook<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_before_external_target_durable_test_hook(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_external_linkat_proc_fallback_test_hook<T>(operation: impl FnOnce() -> T) -> T {
    platform::with_external_linkat_proc_fallback_test_hook(operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_external_linkat_failure_test_hook<T>(operation: impl FnOnce() -> T) -> T {
    platform::with_external_linkat_failure_test_hook(operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn external_stage_identity_digest_for_test(
    path: &Path,
    role: &str,
) -> Result<String, String> {
    platform::external_stage_identity_digest_for_test(path, role)
}

#[cfg(all(target_os = "linux", test))]
macro_rules! append_v2_public_hook {
    ($name:ident) => {
        pub(super) fn $name<T>(hook: impl Fn() + 'static, operation: impl FnOnce() -> T) -> T {
            platform::$name(hook, operation)
        }
    };
}

#[cfg(all(target_os = "linux", test))]
append_v2_public_hook!(with_after_append_v2_checkpoint_hook_for_test);
#[cfg(all(target_os = "linux", test))]
append_v2_public_hook!(with_after_append_v2_write_before_phase_hook_for_test);
#[cfg(all(target_os = "linux", test))]
append_v2_public_hook!(with_after_append_v2_link_before_phase_hook_for_test);
#[cfg(all(target_os = "linux", test))]
append_v2_public_hook!(with_after_append_v2_target_fsync_hook_for_test);
#[cfg(all(target_os = "linux", test))]
append_v2_public_hook!(with_after_append_v2_target_durable_hook_for_test);
#[cfg(all(target_os = "linux", test))]
append_v2_public_hook!(with_after_append_v2_recovery_hash_hook_for_test);

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_append_v2_short_write_for_test<T>(
    bytes: usize,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_append_v2_short_write_for_test(bytes, operation)
}

pub(super) fn plan_atomic_write(
    target: &WriteTarget,
    bytes: &[u8],
    replace_policy: CapabilityReplacePolicy,
    operation_id: &str,
) -> Result<AtomicOperationPlan, String> {
    platform::plan_atomic_write(target, bytes, replace_policy, operation_id)
}

pub(super) fn atomic_write_wal(
    target: &WriteTarget,
    bytes: &[u8],
    replace_policy: CapabilityReplacePolicy,
    plan: &AtomicOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    platform::atomic_write_wal(target, bytes, replace_policy, plan, guard)
        .map(|effect| settle_authority_postflight(effect, &[target]))
}

pub(super) fn classify_atomic_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    read_budget: &mut super::recovery::RecoveryReadBudget,
) -> Result<super::recovery::AtomicRecoveryAssessment, String> {
    platform::classify_atomic_recovery(record, phase, read_budget)
}

pub(super) fn execute_atomic_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    read_budget: &mut super::recovery::RecoveryReadBudget,
) -> Result<(), String> {
    platform::execute_atomic_recovery(record, phase, read_budget)
}

pub(super) fn discard_rebuildable_atomic_projection(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
) -> Result<(), String> {
    platform::discard_rebuildable_atomic_projection(record, phase)
}

pub(super) fn resolve_atomic_operator(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    action: super::recovery::WriteAuthorityRecoveryResolutionAction,
) -> Result<String, String> {
    platform::resolve_atomic_operator(record, phase, action)
}

pub(super) fn classify_append_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    checkpoint: Option<&super::recovery::WalAppendStageCheckpoint>,
    read_budget: &mut super::recovery::RecoveryReadBudget,
) -> Result<super::recovery::AppendRecoveryAssessment, String> {
    platform::classify_append_recovery(record, phase, checkpoint, read_budget)
}

pub(super) fn execute_append_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    checkpoint: Option<&super::recovery::WalAppendStageCheckpoint>,
    read_budget: &mut super::recovery::RecoveryReadBudget,
) -> Result<(), String> {
    platform::execute_append_recovery(record, phase, checkpoint, read_budget)
}

pub(super) fn classify_directory_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    checkpoint: Option<&super::recovery::WalDirectoryStageCheckpoint>,
) -> Result<super::recovery::DirectoryRecoveryAssessment, String> {
    platform::classify_directory_recovery(record, phase, checkpoint)
}

pub(super) fn execute_directory_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    checkpoint: Option<&super::recovery::WalDirectoryStageCheckpoint>,
    action: super::recovery::DirectoryRecoveryAction,
) -> Result<(), String> {
    platform::execute_directory_recovery(record, phase, checkpoint, action)
}

pub(super) fn resolve_directory_operator(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    checkpoint: Option<&super::recovery::WalDirectoryStageCheckpoint>,
    action: super::recovery::WriteAuthorityRecoveryResolutionAction,
    expected_evidence_hash: &str,
    wal_evidence_binding_hash: &str,
) -> Result<String, String> {
    platform::resolve_directory_operator(
        record,
        phase,
        checkpoint,
        action,
        expected_evidence_hash,
        wal_evidence_binding_hash,
    )
}

pub(super) fn classify_symlink_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    checkpoint: Option<&super::recovery::WalSymlinkStageCheckpoint>,
) -> Result<super::recovery::SymlinkRecoveryAssessment, String> {
    platform::classify_symlink_recovery(record, phase, checkpoint)
}

pub(super) fn execute_symlink_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    checkpoint: Option<&super::recovery::WalSymlinkStageCheckpoint>,
    action: super::recovery::SymlinkRecoveryAction,
) -> Result<(), String> {
    platform::execute_symlink_recovery(record, phase, checkpoint, action)
}

pub(super) fn resolve_symlink_operator(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    checkpoint: Option<&super::recovery::WalSymlinkStageCheckpoint>,
    action: super::recovery::WriteAuthorityRecoveryResolutionAction,
    expected_evidence_hash: &str,
    wal_evidence_binding_hash: &str,
) -> Result<String, String> {
    platform::resolve_symlink_operator(
        record,
        phase,
        checkpoint,
        action,
        expected_evidence_hash,
        wal_evidence_binding_hash,
    )
}

pub(super) fn plan_external_config(
    target: &WriteTarget,
    bytes: &[u8],
    backup: Option<(&WriteTarget, &[u8])>,
    operation_id: &str,
) -> Result<ExternalConfigOperationPlan, String> {
    platform::plan_external_config(target, bytes, backup, operation_id)
}

pub(super) fn external_config_wal(
    target: &WriteTarget,
    bytes: &[u8],
    backup: Option<(&WriteTarget, &[u8])>,
    plan: ExternalConfigOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    let backup_target = backup.as_ref().map(|(target, _)| *target);
    platform::external_config_update_wal(target, bytes, backup, plan, guard).map(|effect| {
        let mut targets = vec![target];
        if let Some(backup_target) = backup_target {
            targets.push(backup_target);
        }
        settle_authority_postflight(effect, &targets)
    })
}

pub(super) fn classify_external_config_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    checkpoint: Option<&super::recovery::WalExternalStageCheckpoint>,
    decision: Option<super::recovery::WalExternalOperatorDecision>,
    read_budget: &mut super::recovery::RecoveryReadBudget,
) -> Result<super::recovery::ExternalConfigRecoveryAssessment, String> {
    platform::classify_external_config_recovery(record, phase, checkpoint, decision, read_budget)
}

pub(super) fn execute_external_config_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    checkpoint: Option<&super::recovery::WalExternalStageCheckpoint>,
    decision: Option<super::recovery::WalExternalOperatorDecision>,
    read_budget: &mut super::recovery::RecoveryReadBudget,
) -> Result<(), String> {
    platform::execute_external_config_recovery(record, phase, checkpoint, decision, read_budget)
}

pub(super) fn append(target: &WriteTarget, bytes: &[u8]) -> Result<CapabilityEffect, String> {
    platform::append(target, bytes).map(|effect| settle_authority_postflight(effect, &[target]))
}

pub(super) fn plan_append(
    target: &WriteTarget,
    bytes: &[u8],
) -> Result<AppendOperationPlan, String> {
    platform::plan_append(target, bytes)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn plan_legacy_append_for_test(
    target: &WriteTarget,
    bytes: &[u8],
) -> Result<AppendOperationPlan, String> {
    platform::plan_legacy_append_for_test(target, bytes)
}

pub(super) fn append_wal(
    target: &WriteTarget,
    bytes: &[u8],
    plan: AppendOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    platform::append_wal(target, bytes, plan, guard)
        .map(|effect| settle_authority_postflight(effect, &[target]))
}

pub(super) fn plan_copy(
    target: &WriteTarget,
    source: &Path,
    replace_policy: CapabilityReplacePolicy,
    operation_id: &str,
) -> Result<CopyOperationPlan, String> {
    platform::plan_copy(target, source, replace_policy, operation_id)
}

pub(super) fn copy_file_wal(
    target: &WriteTarget,
    source: &Path,
    replace_policy: CapabilityReplacePolicy,
    plan: CopyOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    platform::copy_file_wal(target, source, replace_policy, plan, guard)
        .map(|effect| settle_authority_postflight(effect, &[target]))
}

pub(super) fn classify_copy_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    checkpoint: Option<&super::recovery::WalCopyStageCheckpoint>,
) -> Result<super::recovery::CopyRecoveryAssessment, String> {
    platform::classify_copy_recovery(record, phase, checkpoint)
}

pub(super) fn execute_copy_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    checkpoint: Option<&super::recovery::WalCopyStageCheckpoint>,
    read_budget: &mut super::recovery::RecoveryReadBudget,
) -> Result<(), String> {
    platform::execute_copy_recovery(record, phase, checkpoint, read_budget)
}

pub(super) fn resolve_copy_operator(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    checkpoint: Option<&super::recovery::WalCopyStageCheckpoint>,
    action: super::recovery::WriteAuthorityRecoveryResolutionAction,
) -> Result<String, String> {
    platform::resolve_copy_operator(record, phase, checkpoint, action)
}

pub(super) fn plan_rename(
    source: &WriteTarget,
    destination: &WriteTarget,
) -> Result<RenameOperationPlan, String> {
    platform::plan_rename(source, destination)
}

pub(super) fn rename_entry_wal(
    source: &WriteTarget,
    destination: &WriteTarget,
    plan: RenameOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    platform::rename_entry_wal(source, destination, plan, guard)
        .map(|effect| settle_authority_postflight(effect, &[source, destination]))
}

pub(super) fn classify_rename_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
) -> Result<super::recovery::RenameRecoveryAssessment, String> {
    platform::classify_rename_recovery(record, phase)
}

pub(super) fn execute_rename_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
) -> Result<(), String> {
    platform::execute_rename_recovery(record, phase)
}

pub(super) fn plan_remove_leaf(
    target: &WriteTarget,
    operation_id: &str,
) -> Result<Option<RemoveLeafOperationPlan>, String> {
    platform::plan_remove_leaf(target, operation_id)
}

pub(super) fn remove_leaf_wal(
    target: &WriteTarget,
    plan: RemoveLeafOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    platform::remove_leaf_wal(target, plan, guard)
        .map(|effect| settle_authority_postflight(effect, &[target]))
}

pub(super) fn classify_remove_leaf_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
) -> Result<super::recovery::RemoveLeafRecoveryAssessment, String> {
    platform::classify_remove_leaf_recovery(record, phase)
}

pub(super) fn execute_remove_leaf_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
) -> Result<(), String> {
    platform::execute_remove_leaf_recovery(record, phase)
}

pub(super) fn resolve_remove_leaf_operator(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    action: super::recovery::WriteAuthorityRecoveryResolutionAction,
) -> Result<String, String> {
    platform::resolve_remove_leaf_operator(record, phase, action)
}

pub(super) fn plan_remove_tree(
    target: &WriteTarget,
    operation_id: &str,
) -> Result<Option<RemoveTreeOperationPlan>, String> {
    platform::plan_remove_tree(target, operation_id)
}

pub(super) fn remove_tree_wal(
    target: &WriteTarget,
    plan: RemoveTreeOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    platform::remove_tree_wal(target, plan, guard)
        .map(|effect| settle_authority_postflight(effect, &[target]))
}

pub(super) fn classify_remove_tree_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
) -> Result<super::recovery::RemoveTreeRecoveryAssessment, String> {
    platform::classify_remove_tree_recovery(record, phase)
}

pub(super) fn execute_remove_tree_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
) -> Result<(), String> {
    platform::execute_remove_tree_recovery(record, phase)
}

pub(super) fn resolve_remove_tree_operator(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    action: super::recovery::WriteAuthorityRecoveryResolutionAction,
) -> Result<String, String> {
    platform::resolve_remove_tree_operator(record, phase, action)
}

pub(super) fn plan_directory(target: &WriteTarget) -> Result<DirectoryOperationPlan, String> {
    platform::plan_directory(target)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn plan_legacy_directory_for_test(
    target: &WriteTarget,
) -> Result<DirectoryOperationPlan, String> {
    platform::plan_legacy_directory_for_test(target)
}

pub(super) fn create_directory_all_wal(
    target: &WriteTarget,
    plan: &DirectoryOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    platform::create_directory_all_wal(target, plan, guard)
        .map(|effect| settle_authority_postflight(effect, &[target]))
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_directory_sync_failure_for_test<T>(operation: impl FnOnce() -> T) -> T {
    platform::with_directory_sync_failure_for_test(operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_before_directory_target_durable_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_before_directory_target_durable_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_after_directory_create_before_phase_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_after_directory_create_before_phase_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_before_directory_v2_checkpoint_capture_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_before_directory_v2_checkpoint_capture_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_after_directory_v2_checkpoint_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_after_directory_v2_checkpoint_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_before_directory_v2_noop_full_path_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_before_directory_v2_noop_full_path_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_before_directory_current_state_fresh_capture_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_before_directory_current_state_fresh_capture_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_before_copy_stream_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_before_copy_stream_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_after_copy_anonymous_stage_checkpoint_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_after_copy_anonymous_stage_checkpoint_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_after_copy_temporary_link_before_phase_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_after_copy_temporary_link_before_phase_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_after_copy_target_link_before_phase_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_after_copy_target_link_before_phase_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_after_copy_rename_before_phase_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_after_copy_rename_before_phase_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_after_copy_target_fsync_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_after_copy_target_fsync_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_before_copy_preview_overwrite_rename_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_before_copy_preview_overwrite_rename_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_after_copy_recovery_hash_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_after_copy_recovery_hash_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_before_rename_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_before_rename_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_before_remove_leaf_quarantine_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_before_remove_leaf_quarantine_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_before_remove_leaf_unlink_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_before_remove_leaf_unlink_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_before_remove_leaf_target_durable_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_before_remove_leaf_target_durable_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_before_remove_tree_quarantine_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_before_remove_tree_quarantine_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_before_remove_tree_traversal_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_before_remove_tree_traversal_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_before_remove_tree_target_durable_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_before_remove_tree_target_durable_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_before_copy_target_durable_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_before_copy_target_durable_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_after_copy_target_durable_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_after_copy_target_durable_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_before_symlink_target_durable_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_before_symlink_target_durable_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_after_symlink_create_before_phase_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_after_symlink_create_before_phase_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_after_symlink_v2_first_open_before_capture_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_after_symlink_v2_first_open_before_capture_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_before_symlink_v2_checkpoint_capture_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_before_symlink_v2_checkpoint_capture_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_after_symlink_v2_checkpoint_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_after_symlink_v2_checkpoint_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_before_symlink_v2_noop_full_path_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_before_symlink_v2_noop_full_path_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_before_symlink_current_state_fresh_capture_hook_for_test<T>(
    hook: impl Fn() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    platform::with_before_symlink_current_state_fresh_capture_hook_for_test(hook, operation)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn with_symlink_eio_for_test<T>(operation: impl FnOnce() -> T) -> T {
    platform::with_symlink_eio_for_test(operation)
}

pub(super) fn plan_symlink(
    target: &WriteTarget,
    source: &Path,
) -> Result<SymlinkOperationPlan, String> {
    platform::plan_symlink(target, source)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn plan_legacy_symlink_for_test(
    target: &WriteTarget,
    source: &Path,
) -> Result<SymlinkOperationPlan, String> {
    platform::plan_legacy_symlink_for_test(target, source)
}

pub(super) fn symlink_entry_wal(
    target: &WriteTarget,
    source: &Path,
    plan: &SymlinkOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    platform::symlink_entry_wal(target, source, plan, guard)
        .map(|effect| settle_authority_postflight(effect, &[target]))
}

pub(super) fn remove_file_if_exists_maintenance(
    target: &WriteTarget,
) -> Result<CapabilityEffect, String> {
    platform::remove_file_if_exists(target)
        .map(|effect| settle_authority_postflight(effect, &[target]))
}

pub(super) fn rename_noreplace(
    source: &WriteTarget,
    destination: &WriteTarget,
) -> Result<CapabilityEffect, String> {
    platform::rename_noreplace(source, destination)
        .map(|effect| settle_authority_postflight(effect, &[source, destination]))
}

pub(super) fn publish_rebuildable_directory(
    source: &WriteTarget,
    destination: &WriteTarget,
) -> Result<CapabilityEffect, String> {
    platform::publish_rebuildable_directory(source, destination)
        .map(|effect| settle_authority_postflight(effect, &[source, destination]))
}

pub(super) fn remove_rebuildable_directory_if_exists(
    target: &WriteTarget,
    operation_id: &str,
) -> Result<CapabilityEffect, String> {
    platform::remove_rebuildable_tree(target, operation_id)
        .map(|effect| settle_authority_postflight(effect, &[target]))
}

fn settle_authority_postflight(
    mut effect: CapabilityEffect,
    targets: &[&WriteTarget],
) -> CapabilityEffect {
    let mut failures = Vec::new();
    for target in targets {
        let Some(authority) = target.authority() else {
            continue;
        };
        if let Err(error) = platform::verify_directory_authority_path(authority) {
            failures.push(error);
        }
    }
    if failures.is_empty() {
        return effect;
    }
    effect.recovery_required = true;
    let postflight = format!(
        "Authority pathname s-a schimbat după efect: {} Replacement-ul nu a fost folosit; reconcilierea este obligatorie și retry-ul automat este interzis.",
        failures.join(" ")
    );
    effect.diagnostic = Some(match effect.diagnostic.take() {
        Some(existing) => format!("{existing} {postflight}"),
        None => postflight,
    });
    effect
}
