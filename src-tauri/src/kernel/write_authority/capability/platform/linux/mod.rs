use super::super::CapabilityGenerationCloneStats;
use std::{
    collections::BTreeSet,
    ffi::{OsStr, OsString},
    fs::File,
    io::SeekFrom,
    os::fd::AsFd,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use rustix::{
    fd::OwnedFd,
    fs::{self, AtFlags, Dir, FileType, FlockOperation, Mode, OFlags, RenameFlags, ResolveFlags},
    io::Errno,
};
use sha2::Sha256;
use walkdir::WalkDir;

use crate::kernel::file_buffer_store::hash_bytes;

use super::super::super::{
    operation::{
        atomic_temp_leaf, external_config_backup_temp_leaf, external_config_target_temp_leaf,
        remove_quarantine_leaf, remove_tree_quarantine_leaf, sha256_bytes, wal_authority_evidence,
        AppendOperationPlan, AtomicOperationPlan, CopyOperationPlan, DirectoryOperationPlan,
        ExternalConfigOperationPlan, RemoveLeafOperationPlan, RemoveTreeOperationPlan,
        RenameOperationPlan, SymlinkOperationPlan,
    },
    recovery::{
        decode_bytes_hex, decode_component_hex, decode_path_hex, encode_bytes_hex,
        encode_component_hex, encode_path_hex, AppendRecoveryAction, AppendRecoveryAssessment,
        AtomicRecoveryAction, AtomicRecoveryAssessment, CopyRecoveryAction, CopyRecoveryAssessment,
        DirectoryRecoveryAction, DirectoryRecoveryAssessment, DirectoryResolutionStateBinding,
        DurableWalGuard, ExternalConfigRecoveryAction, ExternalConfigRecoveryAssessment,
        RecoveryReadBudget, RemoveLeafRecoveryAction, RemoveLeafRecoveryAssessment,
        RemoveTreeRecoveryAction, RemoveTreeRecoveryAssessment, RenameRecoveryAction,
        RenameRecoveryAssessment, SymlinkRecoveryAction, SymlinkRecoveryAssessment,
        SymlinkResolutionStateBinding, WalAppendBefore, WalAppendEvidence,
        WalAppendStageCheckpoint, WalAppendStageRole, WalAtomicFileEvidence, WalAuthorityEvidence,
        WalCopyDestinationPolicy, WalCopyEvidence, WalCopySourceEvidence, WalCopyStageCheckpoint,
        WalCopyStageRole, WalDirectoryEvidence, WalDirectoryStageCheckpoint,
        WalExternalConfigEvidence, WalExternalOperatorDecision, WalExternalStageCheckpoint,
        WalFilesystemIdentity, WalLeafEvidence, WalOperationEvidence, WalParentEvidence, WalPhase,
        WalRecord, WalRemoveLeafEvidence, WalRemoveLeafKind, WalRemoveLeafSourceEvidence,
        WalRemoveTreeEvidence, WalRemoveTreeSourceEvidence, WalRenameEvidence, WalRenameLeafKind,
        WalRenameSourceEvidence, WalSymlinkBefore, WalSymlinkEvidence, WalSymlinkStageCheckpoint,
        WriteAuthorityRecoveryClassification, WriteAuthorityRecoveryResolutionAction,
        MAX_WAL_APPEND_PAYLOAD_BYTES, MAX_WAL_APPEND_PREFIX_BYTES, MAX_WAL_APPEND_TAIL_BYTES,
        MAX_WAL_COPY_BYTES, MAX_WAL_EXTERNAL_CONFIG_BYTES, MAX_WAL_RECOVERY_READ_BYTES,
        MAX_WAL_SYMLINK_TARGET_BYTES, WAL_APPEND_PROTOCOL_VERSION, WAL_COPY_PROTOCOL_VERSION,
        WAL_DIRECTORY_PROTOCOL_VERSION, WAL_EXTERNAL_CONFIG_PROTOCOL_VERSION,
        WAL_SYMLINK_PROTOCOL_VERSION,
    },
    root_authority::{DirectoryAuthority, DirectoryAuthorityScope, FilesystemIdentity},
    tree_fingerprint::{tree_fingerprint_from_records, TreeFingerprintRecord},
};

use super::super::{
    CapabilityBoundedFileSnapshot, CapabilityEffect, CapabilityLockMode, CapabilityReplacePolicy,
    ExpectedLeaf, ExpectedLeafVersion, WriteTarget,
};

const DIRECTORY_MODE: Mode = Mode::from_raw_mode(0o755);
const FILE_MODE: Mode = Mode::from_raw_mode(0o666);
const MAX_OPENAT2_RACE_RETRIES: usize = 8;
const MAX_REMOVE_TREE_DEPTH: usize = 128;
const MAX_REMOVE_TREE_ENTRIES: usize = 100_000;

mod anonymous_file;
mod append;
#[cfg(test)]
pub(in crate::kernel::write_authority::capability) use append::plan_legacy_append_for_test;
pub(in crate::kernel::write_authority::capability) use append::{
    append_wal, classify_append_recovery, execute_append_recovery, plan_append,
};
mod copy;
mod directory;
mod external_config;
mod lifecycle;
mod remove;
mod remove_tree;
mod rename;
mod symlink;
pub(in crate::kernel::write_authority::capability) use copy::{
    classify_copy_recovery, copy_file_wal, copy_rebuildable_file, execute_copy_recovery, plan_copy,
    resolve_copy_operator,
};
#[cfg(test)]
pub(in crate::kernel::write_authority::capability) use directory::plan_legacy_directory_for_test;
pub(in crate::kernel::write_authority::capability) use directory::{
    classify_directory_recovery, create_directory_all_wal, execute_directory_recovery,
    plan_directory, resolve_directory_operator,
};
#[cfg(test)]
pub(in crate::kernel::write_authority::capability) use external_config::external_stage_identity_digest_for_test;
pub(in crate::kernel::write_authority::capability) use external_config::{
    classify_external_config_recovery, execute_external_config_recovery,
    external_config_update_wal, plan_external_config,
};
#[cfg(test)]
pub(in crate::kernel::write_authority::capability) use lifecycle::with_symlink_eio_for_test;
pub(in crate::kernel::write_authority::capability) use remove::{
    classify_remove_leaf_recovery, execute_remove_leaf_recovery, plan_remove_leaf, remove_leaf_wal,
    resolve_remove_leaf_operator,
};
pub(in crate::kernel::write_authority::capability) use remove_tree::{
    classify_remove_tree_recovery, execute_remove_tree_recovery, plan_remove_tree,
    remove_rebuildable_tree, remove_tree_wal, resolve_remove_tree_operator,
};
pub(in crate::kernel::write_authority::capability) use rename::{
    classify_rename_recovery, execute_rename_recovery, plan_rename, rename_entry_wal,
};
#[cfg(test)]
pub(in crate::kernel::write_authority::capability) use symlink::plan_legacy_symlink_for_test;
pub(in crate::kernel::write_authority::capability) use symlink::{
    classify_symlink_recovery, execute_symlink_recovery, plan_symlink, resolve_symlink_operator,
    symlink_entry_wal,
};

static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static QUARANTINE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
struct LexicalTarget {
    boundary_components: Vec<OsString>,
    relative_components: Vec<OsString>,
    public_label: String,
    authority: Option<DirectoryAuthority>,
}

struct CapturedBoundary {
    directory: OwnedFd,
    created: bool,
}

struct CapturedParent {
    directory: OwnedFd,
    leaf: OsString,
    created_ancestors: bool,
}

enum CaptureFailure {
    NoEffect(String),
    RecoveryRequired(String),
}

impl CaptureFailure {
    fn no_effect(diagnostic: impl Into<String>) -> Self {
        Self::NoEffect(diagnostic.into())
    }

    fn after_effect(diagnostic: impl Into<String>) -> Self {
        Self::RecoveryRequired(diagnostic.into())
    }

    fn promote(self) -> Self {
        match self {
            Self::NoEffect(diagnostic) | Self::RecoveryRequired(diagnostic) => {
                Self::RecoveryRequired(diagnostic)
            }
        }
    }

    fn into_diagnostic(self) -> String {
        match self {
            Self::NoEffect(diagnostic) | Self::RecoveryRequired(diagnostic) => diagnostic,
        }
    }

    fn into_operation_result(self) -> Result<CapabilityEffect, String> {
        match self {
            Self::NoEffect(diagnostic) => Err(diagnostic),
            Self::RecoveryRequired(diagnostic) => Ok(CapabilityEffect::recovery_required(
                0,
                format!(
                    "{diagnostic} Un namespace de directoare poate fi deja vizibil; nu repeta operația automat."
                ),
            )),
        }
    }
}

mod atomic;
mod atomic_commit;
mod authority;
mod backend_recovery;
mod maintenance;
mod path;
mod read;
mod rebuildable;

mod hooks;
#[cfg(test)]
use atomic::plan_legacy_directory;
pub(in crate::kernel::write_authority::capability) use atomic::{
    atomic_write, atomic_write_wal, plan_atomic_write,
};
use atomic::{
    capture_append_parent_from_plan, capture_parent_from_wal_evidence, capture_wal_leaf_evidence,
    wal_evidence_from_open_file, wal_identity_from_fd,
};
use atomic_commit::{atomic_commit, validate_expected_content, validate_expected_regular_file};
#[cfg(test)]
use authority::capture_directory_lease;
use authority::identity_from_fd;
pub(in crate::kernel::write_authority::capability) use authority::{
    bootstrap_directory_authority, capture_descendant_authority, capture_directory_authority,
    capture_directory_lease_from_authority, create_directory_from_authority, lock_file,
    verify_directory_authority_path, CapabilityDirectoryLease, CapabilityFileLock,
};
use backend_recovery::{
    capture_recovery_append_context, capture_recovery_atomic_context,
    capture_recovery_directory_authority, classify_legacy_append_recovery,
    classify_legacy_directory_recovery, execute_legacy_append_recovery,
    execute_legacy_directory_recovery, leaf_matches_relocated_before, open_recovery_regular_leaf,
    wal_recovery_effect, RecoveryAppendContext, RecoveryAtomicContext,
};
pub(in crate::kernel::write_authority::capability) use backend_recovery::{
    classify_atomic_recovery, discard_rebuildable_atomic_projection, execute_atomic_recovery,
    resolve_atomic_operator,
};
use hooks::{
    append_v2_short_write_limit, fail_external_linkat, force_external_linkat_proc_fallback,
    run_test_hook, CapabilityTestStage,
};
#[cfg(test)]
pub(in crate::kernel::write_authority::capability) use hooks::{
    with_after_append_v2_checkpoint_hook_for_test,
    with_after_append_v2_link_before_phase_hook_for_test,
    with_after_append_v2_recovery_hash_hook_for_test,
    with_after_append_v2_target_durable_hook_for_test,
    with_after_append_v2_target_fsync_hook_for_test,
    with_after_append_v2_write_before_phase_hook_for_test,
    with_after_bounded_read_leaf_opened_hook_for_test,
    with_after_copy_anonymous_stage_checkpoint_hook_for_test,
    with_after_copy_recovery_hash_hook_for_test, with_after_copy_rename_before_phase_hook_for_test,
    with_after_copy_target_durable_hook_for_test, with_after_copy_target_fsync_hook_for_test,
    with_after_copy_target_link_before_phase_hook_for_test,
    with_after_copy_temporary_link_before_phase_hook_for_test,
    with_after_directory_create_before_phase_hook_for_test,
    with_after_directory_v2_checkpoint_hook_for_test,
    with_after_symlink_create_before_phase_hook_for_test,
    with_after_symlink_v2_checkpoint_hook_for_test,
    with_after_symlink_v2_first_open_before_capture_hook_for_test,
    with_append_v2_short_write_for_test, with_before_copy_preview_overwrite_rename_hook_for_test,
    with_before_copy_stream_hook_for_test, with_before_copy_target_durable_hook_for_test,
    with_before_directory_current_state_fresh_capture_hook_for_test,
    with_before_directory_target_durable_hook_for_test,
    with_before_directory_v2_checkpoint_capture_hook_for_test,
    with_before_directory_v2_noop_full_path_hook_for_test,
    with_before_external_target_durable_test_hook,
    with_before_remove_leaf_quarantine_hook_for_test,
    with_before_remove_leaf_target_durable_hook_for_test,
    with_before_remove_leaf_unlink_hook_for_test, with_before_remove_tree_quarantine_hook_for_test,
    with_before_remove_tree_target_durable_hook_for_test,
    with_before_remove_tree_traversal_hook_for_test, with_before_rename_hook_for_test,
    with_before_symlink_current_state_fresh_capture_hook_for_test,
    with_before_symlink_target_durable_hook_for_test,
    with_before_symlink_v2_checkpoint_capture_hook_for_test,
    with_before_symlink_v2_noop_full_path_hook_for_test, with_directory_sync_failure_for_test,
    with_external_backup_committed_test_hook, with_external_baseline_relocated_test_hook,
    with_external_linkat_failure_test_hook, with_external_linkat_proc_fallback_test_hook,
    with_external_post_publication_test_hook,
};
#[cfg(test)]
use hooks::{with_directory_sync_failure, with_test_hook};
use maintenance::create_legacy_directory_all_wal;
pub(in crate::kernel::write_authority::capability) use maintenance::{
    append, create_directory_all, publish_rebuildable_directory, remove_file_if_exists,
    rename_noreplace,
};
use path::{
    absolute_normal_components, capability_error, capture_boundary, capture_boundary_from_path,
    capture_existing_boundary, capture_existing_target_parent,
    capture_existing_target_parent_from_directory, capture_target_parent,
    capture_target_parent_from_directory, cleanup_temp_after_error, create_unique_temp,
    fingerprint_directory_tree, leaf_metadata, lexical_target, open_directory_strict,
    open_filesystem_root, open_or_create_directory_component, relative_normal_components,
    same_file_identity, same_stable_leaf_version, settle_after_implicit_parent_creation,
    sync_directory, validate_atomic_destination, validate_named_directory_identity,
    validate_named_file_identity, validate_open_directory_identity, validate_regular_single_link,
    version_token_for_stat,
};
pub(in crate::kernel::write_authority::capability) use read::{
    open_optional_regular_file_readonly_no_follow, open_regular_file_readonly_no_follow,
    read_bounded_regular_file_from_authority,
};
pub(in crate::kernel::write_authority::capability) use rebuildable::{
    clone_rebuildable_generation_tree, create_private_rebuildable_directory,
    create_rebuildable_generation_directory, is_real_directory_leaf, seal_rebuildable_generation,
    write_rebuildable_generation_file,
};
#[cfg(test)]
mod tests;
