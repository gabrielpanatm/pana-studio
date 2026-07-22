//! Directory-handle filesystem backend for `WriteAuthority`.
//!
//! The security boundary in this module is an opened directory, not an
//! absolute pathname. Absolute paths are used only while acquiring that
//! capability from `/`, one normal component at a time. Once the boundary is
//! captured, every mutating syscall is relative to a held directory handle.

use std::path::Path;

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

#[cfg(target_os = "linux")]
mod platform {
    use std::{
        ffi::{OsStr, OsString},
        fs::File,
        io::{Read, Seek, SeekFrom, Write},
        os::fd::{AsFd, AsRawFd},
        os::unix::ffi::OsStringExt,
        path::{Component, Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    use rustix::{
        fd::OwnedFd,
        fs::{
            self, AtFlags, Dir, FileType, FlockOperation, Mode, OFlags, RenameFlags, ResolveFlags,
        },
        io::Errno,
    };
    use sha2::{Digest, Sha256};

    use crate::kernel::file_buffer_store::hash_bytes;

    use super::super::{
        operation::{
            atomic_temp_leaf, external_config_backup_temp_leaf, external_config_target_temp_leaf,
            remove_quarantine_leaf, remove_tree_quarantine_leaf, sha256_bytes,
            wal_authority_evidence, AppendOperationPlan, AtomicOperationPlan, CopyOperationPlan,
            DirectoryOperationPlan, ExternalConfigOperationPlan, RemoveLeafOperationPlan,
            RemoveTreeOperationPlan, RenameOperationPlan, SymlinkOperationPlan,
        },
        recovery::{
            decode_bytes_hex, decode_component_hex, decode_path_hex, encode_bytes_hex,
            encode_component_hex, encode_path_hex, AppendRecoveryAction, AppendRecoveryAssessment,
            AtomicRecoveryAction, AtomicRecoveryAssessment, CopyRecoveryAction,
            CopyRecoveryAssessment, DirectoryRecoveryAction, DirectoryRecoveryAssessment,
            DirectoryResolutionStateBinding, DurableWalGuard, ExternalConfigRecoveryAction,
            ExternalConfigRecoveryAssessment, RecoveryReadBudget, RemoveLeafRecoveryAction,
            RemoveLeafRecoveryAssessment, RemoveTreeRecoveryAction, RemoveTreeRecoveryAssessment,
            RenameRecoveryAction, RenameRecoveryAssessment, SymlinkRecoveryAction,
            SymlinkRecoveryAssessment, SymlinkResolutionStateBinding, WalAppendBefore,
            WalAppendEvidence, WalAppendStageCheckpoint, WalAppendStageRole, WalAtomicFileEvidence,
            WalAuthorityEvidence, WalCopyDestinationPolicy, WalCopyEvidence, WalCopySourceEvidence,
            WalCopyStageCheckpoint, WalCopyStageRole, WalDirectoryEvidence,
            WalDirectoryStageCheckpoint, WalExternalConfigEvidence, WalExternalOperatorDecision,
            WalExternalStageCheckpoint, WalFilesystemIdentity, WalLeafEvidence,
            WalOperationEvidence, WalParentEvidence, WalPhase, WalRecord, WalRemoveLeafEvidence,
            WalRemoveLeafKind, WalRemoveLeafSourceEvidence, WalRemoveTreeEvidence,
            WalRemoveTreeSourceEvidence, WalRenameEvidence, WalRenameLeafKind,
            WalRenameSourceEvidence, WalSymlinkBefore, WalSymlinkEvidence,
            WalSymlinkStageCheckpoint, WriteAuthorityRecoveryClassification,
            WriteAuthorityRecoveryResolutionAction, MAX_WAL_APPEND_PAYLOAD_BYTES,
            MAX_WAL_APPEND_PREFIX_BYTES, MAX_WAL_APPEND_TAIL_BYTES, MAX_WAL_COPY_BYTES,
            MAX_WAL_EXTERNAL_CONFIG_BYTES, MAX_WAL_RECOVERY_READ_BYTES,
            MAX_WAL_SYMLINK_TARGET_BYTES, WAL_APPEND_PROTOCOL_VERSION, WAL_COPY_PROTOCOL_VERSION,
            WAL_DIRECTORY_PROTOCOL_VERSION, WAL_EXTERNAL_CONFIG_PROTOCOL_VERSION,
            WAL_SYMLINK_PROTOCOL_VERSION,
        },
        root_authority::{DirectoryAuthority, DirectoryAuthorityScope, FilesystemIdentity},
        tree_fingerprint::{tree_fingerprint_from_records, TreeFingerprintRecord},
    };

    use super::{
        CapabilityBoundedFileSnapshot, CapabilityEffect, CapabilityLockMode,
        CapabilityReplacePolicy, ExpectedLeaf, ExpectedLeafVersion, WriteTarget,
    };

    const DIRECTORY_MODE: Mode = Mode::from_raw_mode(0o755);
    const FILE_MODE: Mode = Mode::from_raw_mode(0o666);
    const MAX_OPENAT2_RACE_RETRIES: usize = 8;
    const MAX_REMOVE_TREE_DEPTH: usize = 128;
    const MAX_REMOVE_TREE_ENTRIES: usize = 100_000;

    #[path = "anonymous_file.rs"]
    mod anonymous_file;
    #[path = "append.rs"]
    mod append;
    #[cfg(test)]
    pub(super) use append::plan_legacy_append_for_test;
    pub(super) use append::{
        append_wal, classify_append_recovery, execute_append_recovery, plan_append,
    };
    #[path = "copy.rs"]
    mod copy;
    #[path = "directory.rs"]
    mod directory;
    #[path = "external_config.rs"]
    mod external_config;
    #[path = "lifecycle.rs"]
    mod lifecycle;
    #[path = "remove.rs"]
    mod remove;
    #[path = "remove_tree.rs"]
    mod remove_tree;
    #[path = "rename.rs"]
    mod rename;
    #[path = "symlink.rs"]
    mod symlink;
    pub(super) use copy::{
        classify_copy_recovery, copy_file_wal, execute_copy_recovery, plan_copy,
        resolve_copy_operator,
    };
    #[cfg(test)]
    pub(super) use directory::plan_legacy_directory_for_test;
    pub(super) use directory::{
        classify_directory_recovery, create_directory_all_wal, execute_directory_recovery,
        plan_directory, resolve_directory_operator,
    };
    #[cfg(test)]
    pub(super) use external_config::external_stage_identity_digest_for_test;
    pub(super) use external_config::{
        classify_external_config_recovery, execute_external_config_recovery,
        external_config_update_wal, plan_external_config,
    };
    #[cfg(test)]
    pub(super) use lifecycle::with_symlink_eio_for_test;
    pub(super) use remove::{
        classify_remove_leaf_recovery, execute_remove_leaf_recovery, plan_remove_leaf,
        remove_leaf_wal, resolve_remove_leaf_operator,
    };
    pub(super) use remove_tree::{
        classify_remove_tree_recovery, execute_remove_tree_recovery, plan_remove_tree,
        remove_rebuildable_tree, remove_tree_wal, resolve_remove_tree_operator,
    };
    pub(super) use rename::{
        classify_rename_recovery, execute_rename_recovery, plan_rename, rename_entry_wal,
    };
    #[cfg(test)]
    pub(super) use symlink::plan_legacy_symlink_for_test;
    pub(super) use symlink::{
        classify_symlink_recovery, execute_symlink_recovery, plan_symlink,
        resolve_symlink_operator, symlink_entry_wal,
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

    pub(super) struct CapabilityFileLock {
        _descriptor: OwnedFd,
    }

    pub(super) struct CapabilityDirectoryLease {
        directory: OwnedFd,
        #[cfg_attr(not(test), allow(dead_code))]
        public_label: String,
    }

    impl CapabilityDirectoryLease {
        pub(super) fn current_dir_path(&self) -> PathBuf {
            PathBuf::from(format!("/proc/self/fd/{}", self.directory.as_raw_fd()))
        }

        #[cfg(test)]
        pub(super) fn require_empty(&self) -> Result<(), String> {
            let mut stream = Dir::read_from(&self.directory).map_err(|error| {
                capability_error(
                    &self.public_label,
                    &format!("directorul capturat nu a putut fi enumerat: {error}"),
                )
            })?;
            while let Some(entry) = stream.read() {
                let entry = entry.map_err(|error| {
                    capability_error(
                        &self.public_label,
                        &format!("enumerarea directorului capturat a eșuat: {error}"),
                    )
                })?;
                let name = entry.file_name().to_bytes();
                if name != b"." && name != b".." {
                    return Err(capability_error(
                        &self.public_label,
                        "directorul capturat nu mai este gol",
                    ));
                }
            }
            Ok(())
        }
    }

    #[cfg(test)]
    pub(super) fn capture_directory_lease(
        path: &Path,
        public_label: &str,
    ) -> Result<CapabilityDirectoryLease, String> {
        let target = WriteTarget::new(path, path, public_label);
        let lexical = lexical_target(&target, true)?;
        let captured = capture_existing_boundary(&lexical)?
            .ok_or_else(|| capability_error(public_label, "directorul subprocess nu există"))?;
        let metadata = fs::fstat(&captured.directory).map_err(|error| {
            capability_error(
                public_label,
                &format!("identitatea directorului subprocess nu poate fi citită: {error}"),
            )
        })?;
        if FileType::from_raw_mode(metadata.st_mode) != FileType::Directory {
            return Err(capability_error(
                public_label,
                "capability-ul subprocess nu este director",
            ));
        }
        Ok(CapabilityDirectoryLease {
            directory: captured.directory,
            public_label: public_label.to_string(),
        })
    }

    pub(super) fn capture_directory_lease_from_authority(
        authority: &DirectoryAuthority,
        path: &Path,
        public_label: &str,
    ) -> Result<CapabilityDirectoryLease, String> {
        verify_directory_authority_path(authority)?;
        let target = WriteTarget::new(path, authority.root_path(), public_label)
            .bind_authority(authority.clone())?;
        let lexical = lexical_target(&target, true)?;
        let mut directory = rustix::io::dup(authority.directory()).map_err(|error| {
            capability_error(
                public_label,
                &format!("authority subprocess nu a putut fi duplicată: {error}"),
            )
        })?;
        for component in &lexical.relative_components {
            directory = open_directory_strict(&directory, component).map_err(|error| {
                capability_error(
                    public_label,
                    &format!("directorul subprocess nu a putut fi derivat: {error}"),
                )
            })?;
        }
        Ok(CapabilityDirectoryLease {
            directory,
            public_label: public_label.to_string(),
        })
    }

    pub(super) fn capture_directory_authority(
        path: &Path,
        public_label: &str,
        scope: DirectoryAuthorityScope,
    ) -> Result<DirectoryAuthority, String> {
        let target = WriteTarget::new(path, path, public_label);
        let lexical = lexical_target(&target, true)?;
        let captured = capture_boundary_from_path(&lexical, false)
            .map_err(CaptureFailure::into_diagnostic)?
            .ok_or_else(|| capability_error(public_label, "authority root nu există"))?;
        authority_from_captured(path, public_label, scope, captured.directory)
    }

    pub(super) fn bootstrap_directory_authority(
        path: &Path,
        public_label: &str,
        scope: DirectoryAuthorityScope,
    ) -> Result<DirectoryAuthority, String> {
        let target = WriteTarget::new(path, path, public_label);
        let lexical = lexical_target(&target, true)?;
        let captured = capture_boundary_from_path(&lexical, true)
            .map_err(CaptureFailure::into_diagnostic)?
            .ok_or_else(|| {
                capability_error(public_label, "authority root nu a putut fi creat/capturat")
            })?;
        sync_directory(&captured.directory, public_label)?;
        authority_from_captured(path, public_label, scope, captured.directory)
    }

    pub(super) fn create_directory_from_authority(
        authority: &DirectoryAuthority,
        path: &Path,
        public_label: &str,
    ) -> Result<(), String> {
        let target = WriteTarget::new(path, authority.root_path(), public_label)
            .bind_authority(authority.clone())?;
        let effect = create_directory_all(&target)?;
        if effect.recovery_required {
            return Err(effect.diagnostic.unwrap_or_else(|| {
                format!(
                    "Capability filesystem a creat {public_label}, dar durabilitatea cere recovery."
                )
            }));
        }
        Ok(())
    }

    pub(super) fn capture_descendant_authority(
        parent: &DirectoryAuthority,
        path: &Path,
        public_label: &str,
        scope: DirectoryAuthorityScope,
    ) -> Result<DirectoryAuthority, String> {
        verify_directory_authority_path(parent)?;
        let target = WriteTarget::new(path, parent.root_path(), public_label)
            .bind_authority(parent.clone())?;
        let lexical = lexical_target(&target, true)?;
        let mut directory = rustix::io::dup(parent.directory()).map_err(|error| {
            capability_error(
                public_label,
                &format!("authority parent nu a putut fi duplicată: {error}"),
            )
        })?;
        for component in &lexical.relative_components {
            directory = open_directory_strict(&directory, component).map_err(|error| {
                capability_error(
                    public_label,
                    &format!("authority descendant nu a putut fi capturată: {error}"),
                )
            })?;
        }
        authority_from_captured(path, public_label, scope, directory)
    }

    pub(super) fn verify_directory_authority_path(
        authority: &DirectoryAuthority,
    ) -> Result<(), String> {
        let target = WriteTarget::new(
            authority.root_path(),
            authority.root_path(),
            "authority/path-binding",
        );
        let lexical = lexical_target(&target, true)?;
        let captured = capture_boundary_from_path(&lexical, false)
            .map_err(CaptureFailure::into_diagnostic)?
            .ok_or_else(|| {
                capability_error(
                    "authority/path-binding",
                    &format!(
                        "pathname-ul authority {} nu mai există",
                        authority.root_path().display()
                    ),
                )
            })?;
        let observed = identity_from_fd(&captured.directory, "authority/path-binding")?;
        if observed != authority.identity() {
            return Err(capability_error(
                "authority/path-binding",
                &format!(
                    "pathname-ul {} a fost înlocuit (expected dev={} ino={}, observed dev={} ino={})",
                    authority.root_path().display(),
                    authority.identity().device,
                    authority.identity().inode,
                    observed.device,
                    observed.inode
                ),
            ));
        }
        Ok(())
    }

    fn authority_from_captured(
        path: &Path,
        public_label: &str,
        scope: DirectoryAuthorityScope,
        directory: OwnedFd,
    ) -> Result<DirectoryAuthority, String> {
        let identity = identity_from_fd(&directory, public_label)?;
        Ok(DirectoryAuthority::from_opened_directory(
            path.to_path_buf(),
            identity,
            scope,
            directory,
        ))
    }

    fn identity_from_fd(
        directory: &OwnedFd,
        public_label: &str,
    ) -> Result<FilesystemIdentity, String> {
        let metadata = fs::fstat(directory).map_err(|error| {
            capability_error(
                public_label,
                &format!("identitatea authority nu poate fi citită: {error}"),
            )
        })?;
        if FileType::from_raw_mode(metadata.st_mode) != FileType::Directory {
            return Err(capability_error(
                public_label,
                "authority handle nu desemnează un director",
            ));
        }
        Ok(FilesystemIdentity {
            device: metadata.st_dev,
            inode: metadata.st_ino,
        })
    }

    pub(super) fn lock_file(
        target: &WriteTarget,
        mode: CapabilityLockMode,
    ) -> Result<CapabilityFileLock, String> {
        let lexical = lexical_target(target, false)?;
        let parent = capture_existing_target_parent(&lexical)?.ok_or_else(|| {
            capability_error(
                &lexical.public_label,
                "folderul părinte al lock-ului nu a putut fi capturat",
            )
        })?;
        let descriptor = fs::openat(
            &parent.directory,
            &parent.leaf,
            OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            FILE_MODE,
        )
        .map_err(|error| {
            capability_error(
                &lexical.public_label,
                &format!("fișierul lock stabil nu a putut fi deschis: {error}"),
            )
        })?;
        validate_regular_single_link(&descriptor, &lexical.public_label, "StableLock")?;
        let operation = match mode {
            CapabilityLockMode::Shared => FlockOperation::LockShared,
            CapabilityLockMode::Exclusive => FlockOperation::LockExclusive,
        };
        fs::flock(&descriptor, operation).map_err(|error| {
            capability_error(
                &lexical.public_label,
                &format!("lock-ul stabil nu a putut fi obținut: {error}"),
            )
        })?;
        Ok(CapabilityFileLock {
            _descriptor: descriptor,
        })
    }

    pub(super) fn atomic_write(
        target: &WriteTarget,
        bytes: &[u8],
        replace_policy: CapabilityReplacePolicy,
    ) -> Result<CapabilityEffect, String> {
        let lexical = lexical_target(target, false)?;
        let parent = match capture_target_parent(&lexical, true) {
            Ok(Some(parent)) => parent,
            Ok(None) => {
                return Err(capability_error(
                    &lexical.public_label,
                    "folderul părinte nu a putut fi capturat",
                ));
            }
            Err(error) => return error.into_operation_result(),
        };
        run_test_hook(CapabilityTestStage::AfterTargetParentCaptured);
        let result = (|| {
            validate_atomic_destination(&parent.directory, &parent.leaf, replace_policy, &lexical)?;

            atomic_commit(
                &parent.directory,
                &parent.leaf,
                replace_policy,
                &target.expected_leaf,
                &lexical.public_label,
                |file| {
                    file.write_all(bytes).map_err(|error| {
                        capability_error(
                            &lexical.public_label,
                            &format!("fișierul temporar nu a putut fi scris: {error}"),
                        )
                    })?;
                    Ok(bytes.len() as u64)
                },
            )
        })();
        settle_after_implicit_parent_creation(
            parent.created_ancestors,
            result,
            &lexical.public_label,
        )
    }

    pub(super) fn plan_atomic_write(
        target: &WriteTarget,
        bytes: &[u8],
        replace_policy: CapabilityReplacePolicy,
        operation_id: &str,
    ) -> Result<AtomicOperationPlan, String> {
        let lexical = lexical_target(target, false)?;
        if lexical.authority.is_none() {
            return Err(capability_error(
                &lexical.public_label,
                "planul WAL cere authority root sigilat",
            ));
        }
        let boundary = capture_existing_boundary(&lexical)?.ok_or_else(|| {
            capability_error(
                &lexical.public_label,
                "authority root nu există pentru planul atomic",
            )
        })?;
        let (leaf, parents) = lexical.relative_components.split_last().ok_or_else(|| {
            capability_error(
                &lexical.public_label,
                "planul atomic cere un leaf sub authority root",
            )
        })?;

        let mut directory = boundary.directory;
        let mut existing_prefix_len = 0_usize;
        for component in parents {
            match open_directory_strict(&directory, component) {
                Ok(next) => {
                    directory = next;
                    existing_prefix_len += 1;
                }
                Err(Errno::NOENT) => break,
                Err(error) => {
                    return Err(capability_error(
                        &lexical.public_label,
                        &format!("planul atomic nu poate captura un părinte: {error}"),
                    ));
                }
            }
        }
        let existing_ancestor_identity = wal_identity_from_fd(&directory, &lexical.public_label)?;
        let parent_exists = existing_prefix_len == parents.len();
        let parent_identity = parent_exists
            .then(|| wal_identity_from_fd(&directory, &lexical.public_label))
            .transpose()?;

        let before = if parent_exists {
            validate_atomic_destination(&directory, leaf, replace_policy, &lexical)?;
            capture_wal_leaf_evidence(
                &directory,
                leaf,
                &target.expected_leaf,
                &lexical.public_label,
                None,
            )?
        } else {
            if matches!(target.expected_leaf, ExpectedLeaf::Present(_)) {
                return Err(capability_error(
                    &lexical.public_label,
                    "disk baseline-ul Present nu poate exista sub un părinte absent",
                ));
            }
            WalLeafEvidence::Absent
        };

        if matches!(target.expected_leaf, ExpectedLeaf::Present(_))
            && matches!(before, WalLeafEvidence::Absent)
        {
            return Err(capability_error(
                &lexical.public_label,
                "target-ul disk baseline Present lipsește la planificare",
            ));
        }

        let temp_leaf = atomic_temp_leaf(operation_id);
        if parent_exists && leaf_metadata(&directory, &temp_leaf, &lexical.public_label)?.is_some()
        {
            return Err(capability_error(
                &lexical.public_label,
                "numele temp determinist al operației există deja",
            ));
        }

        Ok(AtomicOperationPlan {
            evidence: WalAtomicFileEvidence {
                parent: WalParentEvidence {
                    relative_components_hex: parents
                        .iter()
                        .map(|component| encode_component_hex(component))
                        .collect(),
                    existing_prefix_len,
                    existing_ancestor_identity,
                    parent_identity,
                },
                target_leaf_hex: encode_component_hex(leaf),
                temp_leaf_hex: encode_component_hex(&temp_leaf),
                replace: !matches!(before, WalLeafEvidence::Absent),
                before,
                new_size: bytes.len() as u64,
                new_content_hash: sha256_bytes(bytes),
            },
        })
    }

    #[cfg(test)]
    fn plan_legacy_directory(target: &WriteTarget) -> Result<DirectoryOperationPlan, String> {
        let lexical = lexical_target(target, true)?;
        if lexical.authority.is_none() {
            return Err(capability_error(
                &lexical.public_label,
                "planul mkdir WAL cere authority root sigilat",
            ));
        }
        let boundary = capture_existing_boundary(&lexical)?.ok_or_else(|| {
            capability_error(
                &lexical.public_label,
                "authority root nu există pentru planul mkdir",
            )
        })?;
        let mut directory = boundary.directory;
        let mut existing_prefix_len = 0_usize;
        for component in &lexical.relative_components {
            match open_directory_strict(&directory, component) {
                Ok(next) => {
                    directory = next;
                    existing_prefix_len += 1;
                }
                Err(Errno::NOENT) => break,
                Err(error) => {
                    return Err(capability_error(
                        &lexical.public_label,
                        &format!("planul mkdir nu poate captura un component: {error}"),
                    ));
                }
            }
        }
        let existing_ancestor_identity = wal_identity_from_fd(&directory, &lexical.public_label)?;
        let target_exists = existing_prefix_len == lexical.relative_components.len();
        Ok(DirectoryOperationPlan {
            evidence: WalDirectoryEvidence {
                protocol_version: 0,
                relative_components_hex: lexical
                    .relative_components
                    .iter()
                    .map(|component| encode_component_hex(component))
                    .collect(),
                existing_prefix_len,
                existing_ancestor_identity: existing_ancestor_identity.clone(),
                existing_target_identity: target_exists.then_some(existing_ancestor_identity),
                parent_identity: None,
                target_leaf_hex: None,
                existing_target_identity_digest: None,
                existing_target_version_token: None,
                desired_mode_bits: None,
            },
        })
    }

    fn capture_wal_leaf_evidence(
        parent: &OwnedFd,
        leaf: &OsStr,
        expected_leaf: &ExpectedLeaf,
        public_label: &str,
        read_budget: Option<&mut RecoveryReadBudget>,
    ) -> Result<WalLeafEvidence, String> {
        let Some(stat) = leaf_metadata(parent, leaf, public_label)? else {
            return Ok(WalLeafEvidence::Absent);
        };
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
            return Err(capability_error(
                public_label,
                "WAL atomic baseline nu este fișier regular",
            ));
        }
        let descriptor = fs::openat(
            parent,
            leaf,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| {
            capability_error(
                public_label,
                &format!("WAL atomic baseline nu poate fi deschis: {error}"),
            )
        })?;
        let mut file = File::from(descriptor);
        let captured = fs::fstat(&file).map_err(|error| {
            capability_error(
                public_label,
                &format!("WAL atomic baseline metadata nu poate fi citită: {error}"),
            )
        })?;
        if !same_file_identity(&stat, &captured) {
            return Err(capability_error(
                public_label,
                "WAL atomic baseline s-a schimbat în timpul capturii",
            ));
        }
        let evidence = wal_evidence_from_open_file(
            &mut file,
            &captured,
            expected_leaf,
            public_label,
            "WAL plan preflight",
            read_budget,
        )?;
        validate_named_file_identity(parent, leaf, &captured, "wal-baseline")?;
        Ok(evidence)
    }

    fn wal_evidence_from_open_file(
        file: &mut File,
        captured: &fs::Stat,
        expected_leaf: &ExpectedLeaf,
        public_label: &str,
        stage: &str,
        read_budget: Option<&mut RecoveryReadBudget>,
    ) -> Result<WalLeafEvidence, String> {
        if let ExpectedLeaf::Present(expected) = expected_leaf {
            validate_expected_regular_file(file, captured, expected, public_label, stage)?;
        }
        let size = u64::try_from(captured.st_size)
            .map_err(|_| capability_error(public_label, "WAL baseline are dimensiune negativă"))?;
        const MAX_WAL_HASH_BYTES: u64 = 512 * 1024 * 1024;
        if size > MAX_WAL_HASH_BYTES {
            return Err(capability_error(
                public_label,
                &format!("WAL baseline depășește limita de {MAX_WAL_HASH_BYTES} bytes"),
            ));
        }
        if let Some(read_budget) = read_budget {
            read_budget.reserve(size, stage)?;
        }
        file.seek(SeekFrom::Start(0)).map_err(|error| {
            capability_error(
                public_label,
                &format!("WAL baseline nu poate reveni la început: {error}"),
            )
        })?;
        let mut hasher = Sha256::new();
        let mut observed = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let count = file.read(&mut buffer).map_err(|error| {
                capability_error(
                    public_label,
                    &format!("WAL baseline nu poate fi hash-uit: {error}"),
                )
            })?;
            if count == 0 {
                break;
            }
            observed = observed.saturating_add(count as u64);
            if observed > size {
                return Err(capability_error(
                    public_label,
                    "WAL baseline a crescut în timpul hash-ului",
                ));
            }
            hasher.update(&buffer[..count]);
        }
        let captured_after = fs::fstat(&*file).map_err(|error| {
            capability_error(
                public_label,
                &format!("WAL baseline nu poate fi reverificat: {error}"),
            )
        })?;
        if observed != size || !same_stable_leaf_version(&captured, &captured_after) {
            return Err(capability_error(
                public_label,
                "WAL baseline s-a modificat în timpul hash-ului",
            ));
        }
        Ok(WalLeafEvidence::Regular {
            identity: WalFilesystemIdentity {
                device: captured.st_dev,
                inode: captured.st_ino,
            },
            size,
            version_token: version_token_for_stat(&captured),
            content_hash: format!("{:x}", hasher.finalize()),
        })
    }

    fn wal_identity_from_fd(
        directory: &OwnedFd,
        public_label: &str,
    ) -> Result<WalFilesystemIdentity, String> {
        let identity = identity_from_fd(directory, public_label)?;
        Ok(WalFilesystemIdentity {
            device: identity.device,
            inode: identity.inode,
        })
    }

    pub(super) fn atomic_write_wal(
        target: &WriteTarget,
        bytes: &[u8],
        replace_policy: CapabilityReplacePolicy,
        plan: &AtomicOperationPlan,
        guard: &mut DurableWalGuard<'_>,
    ) -> Result<CapabilityEffect, String> {
        let lexical = lexical_target(target, false)?;
        validate_atomic_plan_shape(&lexical, bytes, replace_policy, plan, guard.operation_id())?;
        let parent = match capture_atomic_parent_from_plan(&lexical, plan) {
            Ok(parent) => parent,
            Err(error) => return error.into_operation_result(),
        };
        let parent_changed = parent.created_ancestors;
        run_test_hook(CapabilityTestStage::AfterTargetParentCaptured);

        let observed_before = match capture_wal_leaf_evidence(
            &parent.directory,
            &parent.leaf,
            &target.expected_leaf,
            &lexical.public_label,
            None,
        ) {
            Ok(evidence) => evidence,
            Err(error) if parent_changed => {
                return Ok(wal_recovery_effect(
                    0,
                    &lexical.public_label,
                    format!("{error} Părinții planificați au fost deja creați."),
                ));
            }
            Err(error) => return Err(error),
        };
        if observed_before != plan.evidence.before {
            let error = capability_error(
                &lexical.public_label,
                "baseline-ul target diferă de planul WAL înainte de temp create",
            );
            return if parent_changed {
                Ok(wal_recovery_effect(0, &lexical.public_label, error))
            } else {
                Err(error)
            };
        }
        let temp_name = plan.temp_leaf()?;
        match leaf_metadata(&parent.directory, &temp_name, &lexical.public_label) {
            Ok(None) => {}
            Ok(Some(_)) => {
                let error = capability_error(
                    &lexical.public_label,
                    "temp leaf-ul WAL determinist există înainte de O_EXCL",
                );
                return if parent_changed {
                    Ok(wal_recovery_effect(0, &lexical.public_label, error))
                } else {
                    Err(error)
                };
            }
            Err(error) => {
                return if parent_changed {
                    Ok(wal_recovery_effect(0, &lexical.public_label, error))
                } else {
                    Err(error)
                };
            }
        }

        let descriptor = match fs::openat(
            &parent.directory,
            &temp_name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            FILE_MODE,
        ) {
            Ok(descriptor) => descriptor,
            Err(error) => {
                let diagnostic = capability_error(
                    &lexical.public_label,
                    &format!("temp leaf-ul WAL nu a putut fi creat exact: {error}"),
                );
                return if parent_changed {
                    Ok(wal_recovery_effect(0, &lexical.public_label, diagnostic))
                } else {
                    Err(diagnostic)
                };
            }
        };
        let mut temp_file = File::from(descriptor);
        if let Err(error) = temp_file
            .write_all(bytes)
            .and_then(|()| temp_file.sync_all())
        {
            return Ok(wal_recovery_effect(
                0,
                &lexical.public_label,
                format!("temp leaf-ul WAL poate fi parțial după write/fsync: {error}"),
            ));
        }
        let temp_identity = match fs::fstat(&temp_file) {
            Ok(stat)
                if FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile
                    && stat.st_nlink == 1
                    && u64::try_from(stat.st_size).ok() == Some(plan.evidence.new_size) =>
            {
                stat
            }
            Ok(_) => {
                return Ok(wal_recovery_effect(
                    bytes.len() as u64,
                    &lexical.public_label,
                    "temp leaf-ul WAL nu are tip/link/size așteptat",
                ));
            }
            Err(error) => {
                return Ok(wal_recovery_effect(
                    bytes.len() as u64,
                    &lexical.public_label,
                    format!("temp leaf-ul WAL nu poate fi verificat: {error}"),
                ));
            }
        };
        if let Err(error) =
            validate_named_file_identity(&parent.directory, &temp_name, &temp_identity, "wal-temp")
        {
            return Ok(wal_recovery_effect(
                bytes.len() as u64,
                &lexical.public_label,
                error,
            ));
        }
        if let Err(error) = guard.mark_auxiliary_durable() {
            return Ok(wal_recovery_effect(
                bytes.len() as u64,
                &lexical.public_label,
                error,
            ));
        }

        run_test_hook(CapabilityTestStage::BeforeAtomicCommit);
        if plan.evidence.replace {
            let previous_descriptor = match fs::openat(
                &parent.directory,
                &parent.leaf,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            ) {
                Ok(descriptor) => descriptor,
                Err(error) => {
                    return Ok(wal_recovery_effect(
                        bytes.len() as u64,
                        &lexical.public_label,
                        format!("target-ul replace nu mai poate fi capturat: {error}"),
                    ));
                }
            };
            let mut previous_file = File::from(previous_descriptor);
            let previous_before = match fs::fstat(&previous_file) {
                Ok(stat) => stat,
                Err(error) => {
                    return Ok(wal_recovery_effect(
                        bytes.len() as u64,
                        &lexical.public_label,
                        format!("target-ul replace nu mai are metadata: {error}"),
                    ));
                }
            };
            let previous_evidence = match wal_evidence_from_open_file(
                &mut previous_file,
                &previous_before,
                &target.expected_leaf,
                &lexical.public_label,
                "WAL replace commit preflight",
                None,
            ) {
                Ok(evidence) => evidence,
                Err(error) => {
                    return Ok(wal_recovery_effect(
                        bytes.len() as u64,
                        &lexical.public_label,
                        error,
                    ));
                }
            };
            if previous_evidence != plan.evidence.before {
                return Ok(wal_recovery_effect(
                    bytes.len() as u64,
                    &lexical.public_label,
                    "target-ul replace diferă de baseline-ul WAL la commit",
                ));
            }
            if let Err(error) = fs::renameat_with(
                &parent.directory,
                &temp_name,
                &parent.directory,
                &parent.leaf,
                RenameFlags::EXCHANGE,
            ) {
                return Ok(wal_recovery_effect(
                    bytes.len() as u64,
                    &lexical.public_label,
                    format!("WAL atomic exchange a eșuat: {error}"),
                ));
            }
            run_test_hook(CapabilityTestStage::AfterAtomicExchange);
            if let Err(error) = guard.mark_effect_visible() {
                return Ok(wal_recovery_effect(
                    bytes.len() as u64,
                    &lexical.public_label,
                    error,
                ));
            }
            let moved_previous =
                match fs::statat(&parent.directory, &temp_name, AtFlags::SYMLINK_NOFOLLOW) {
                    Ok(stat) => stat,
                    Err(error) => {
                        return Ok(wal_recovery_effect(
                            bytes.len() as u64,
                            &lexical.public_label,
                            format!("versiunea veche WAL nu poate fi găsită: {error}"),
                        ));
                    }
                };
            let previous_after = match fs::fstat(&previous_file) {
                Ok(stat) => stat,
                Err(error) => {
                    return Ok(wal_recovery_effect(
                        bytes.len() as u64,
                        &lexical.public_label,
                        format!("versiunea veche WAL nu poate fi reverificată: {error}"),
                    ));
                }
            };
            if !same_file_identity(&previous_before, &moved_previous)
                || !same_stable_leaf_version(&previous_before, &previous_after)
            {
                return Ok(wal_recovery_effect(
                    bytes.len() as u64,
                    &lexical.public_label,
                    "WAL exchange a izolat alt inode/versiune decât baseline-ul",
                ));
            }
            if let Err(error) = validate_named_file_identity(
                &parent.directory,
                &parent.leaf,
                &temp_identity,
                "wal-replace-target",
            ) {
                return Ok(wal_recovery_effect(
                    bytes.len() as u64,
                    &lexical.public_label,
                    error,
                ));
            }
            if let Err(error) = sync_directory(&parent.directory, &lexical.public_label) {
                return Ok(wal_recovery_effect(
                    bytes.len() as u64,
                    &lexical.public_label,
                    format!("{error} Mappingul exchange nu este confirmat durabil."),
                ));
            }
            if let Err(error) = fs::unlinkat(&parent.directory, &temp_name, AtFlags::empty()) {
                return Ok(wal_recovery_effect(
                    bytes.len() as u64,
                    &lexical.public_label,
                    format!("versiunea veche WAL nu poate fi curățată: {error}"),
                ));
            }
            if let Err(error) = sync_directory(&parent.directory, &lexical.public_label) {
                return Ok(wal_recovery_effect(
                    bytes.len() as u64,
                    &lexical.public_label,
                    format!("{error} Cleanup-ul versiunii vechi nu este durabil."),
                ));
            }
        } else {
            if let Err(error) = fs::renameat_with(
                &parent.directory,
                &temp_name,
                &parent.directory,
                &parent.leaf,
                RenameFlags::NOREPLACE,
            ) {
                return Ok(wal_recovery_effect(
                    bytes.len() as u64,
                    &lexical.public_label,
                    format!("WAL atomic create rename a eșuat: {error}"),
                ));
            }
            if let Err(error) = guard.mark_effect_visible() {
                return Ok(wal_recovery_effect(
                    bytes.len() as u64,
                    &lexical.public_label,
                    error,
                ));
            }
            if let Err(error) = validate_named_file_identity(
                &parent.directory,
                &parent.leaf,
                &temp_identity,
                "wal-create-target",
            ) {
                return Ok(wal_recovery_effect(
                    bytes.len() as u64,
                    &lexical.public_label,
                    error,
                ));
            }
            if let Err(error) = sync_directory(&parent.directory, &lexical.public_label) {
                return Ok(wal_recovery_effect(
                    bytes.len() as u64,
                    &lexical.public_label,
                    format!("{error} Mappingul create nu este confirmat durabil."),
                ));
            }
        }

        drop(temp_file);
        if let Err(error) = guard.mark_target_durable() {
            return Ok(wal_recovery_effect(
                bytes.len() as u64,
                &lexical.public_label,
                error,
            ));
        }
        Ok(CapabilityEffect::changed(bytes.len() as u64))
    }

    fn validate_atomic_plan_shape(
        lexical: &LexicalTarget,
        bytes: &[u8],
        replace_policy: CapabilityReplacePolicy,
        plan: &AtomicOperationPlan,
        operation_id: &str,
    ) -> Result<(), String> {
        let (leaf, parents) = lexical.relative_components.split_last().ok_or_else(|| {
            capability_error(
                &lexical.public_label,
                "planul WAL atomic nu are target leaf",
            )
        })?;
        let planned_parents = plan
            .evidence
            .parent
            .relative_components_hex
            .iter()
            .map(|component| decode_component_hex(component))
            .collect::<Result<Vec<_>, _>>()?;
        if planned_parents != parents
            || decode_component_hex(&plan.evidence.target_leaf_hex)? != *leaf
            || plan.temp_leaf()? != atomic_temp_leaf(operation_id)
            || plan.evidence.new_size != bytes.len() as u64
            || plan.evidence.new_content_hash != sha256_bytes(bytes)
            || (replace_policy == CapabilityReplacePolicy::CreateNew && plan.evidence.replace)
        {
            return Err(capability_error(
                &lexical.public_label,
                "planul WAL atomic nu corespunde targetului/payloadului executat",
            ));
        }
        Ok(())
    }

    fn capture_atomic_parent_from_plan(
        lexical: &LexicalTarget,
        plan: &AtomicOperationPlan,
    ) -> Result<CapturedParent, CaptureFailure> {
        capture_parent_from_wal_evidence(lexical, &plan.evidence.parent)
    }

    fn capture_append_parent_from_plan(
        lexical: &LexicalTarget,
        plan: &AppendOperationPlan,
    ) -> Result<CapturedParent, CaptureFailure> {
        capture_parent_from_wal_evidence(lexical, &plan.evidence.parent)
    }

    fn capture_parent_from_wal_evidence(
        lexical: &LexicalTarget,
        evidence: &WalParentEvidence,
    ) -> Result<CapturedParent, CaptureFailure> {
        let (leaf, parents) = lexical.relative_components.split_last().ok_or_else(|| {
            CaptureFailure::no_effect(capability_error(
                &lexical.public_label,
                "planul WAL atomic cere un leaf",
            ))
        })?;
        if evidence.existing_prefix_len > parents.len() {
            return Err(CaptureFailure::no_effect(capability_error(
                &lexical.public_label,
                "planul WAL atomic are existing prefix invalid",
            )));
        }
        let boundary = capture_boundary(lexical, false)?.ok_or_else(|| {
            CaptureFailure::no_effect(capability_error(
                &lexical.public_label,
                "authority root a dispărut înainte de execuția WAL",
            ))
        })?;
        let mut directory = boundary.directory;
        for component in parents.iter().take(evidence.existing_prefix_len) {
            directory = open_directory_strict(&directory, component).map_err(|error| {
                CaptureFailure::no_effect(capability_error(
                    &lexical.public_label,
                    &format!("existing prefix WAL nu poate fi recapturat: {error}"),
                ))
            })?;
        }
        let observed = wal_identity_from_fd(&directory, &lexical.public_label)
            .map_err(CaptureFailure::no_effect)?;
        if observed != evidence.existing_ancestor_identity {
            return Err(CaptureFailure::no_effect(capability_error(
                &lexical.public_label,
                "existing ancestor identity diferă de planul WAL",
            )));
        }
        if evidence.existing_prefix_len == parents.len() {
            if evidence.parent_identity.as_ref() != Some(&observed) {
                return Err(CaptureFailure::no_effect(capability_error(
                    &lexical.public_label,
                    "parent identity diferă de planul WAL",
                )));
            }
            return Ok(CapturedParent {
                directory,
                leaf: leaf.clone(),
                created_ancestors: false,
            });
        }
        if evidence.parent_identity.is_some() {
            return Err(CaptureFailure::no_effect(capability_error(
                &lexical.public_label,
                "planul WAL declară parent identity pentru un suffix absent",
            )));
        }

        let mut created = false;
        for component in parents.iter().skip(evidence.existing_prefix_len) {
            match open_directory_strict(&directory, component) {
                Err(Errno::NOENT) => {}
                Ok(_) => {
                    let error = capability_error(
                        &lexical.public_label,
                        "un părinte planificat absent a apărut înainte de mkdirat",
                    );
                    return Err(if created {
                        CaptureFailure::after_effect(error)
                    } else {
                        CaptureFailure::no_effect(error)
                    });
                }
                Err(error) => {
                    let diagnostic = capability_error(
                        &lexical.public_label,
                        &format!("un părinte WAL nu poate fi verificat: {error}"),
                    );
                    return Err(if created {
                        CaptureFailure::after_effect(diagnostic)
                    } else {
                        CaptureFailure::no_effect(diagnostic)
                    });
                }
            }
            if let Err(error) = fs::mkdirat(&directory, component, DIRECTORY_MODE) {
                let diagnostic = capability_error(
                    &lexical.public_label,
                    &format!("mkdirat planificat de WAL a eșuat: {error}"),
                );
                return Err(if created {
                    CaptureFailure::after_effect(diagnostic)
                } else {
                    CaptureFailure::no_effect(diagnostic)
                });
            }
            created = true;
            let next = open_directory_strict(&directory, component).map_err(|error| {
                CaptureFailure::after_effect(capability_error(
                    &lexical.public_label,
                    &format!("părintele WAL creat nu poate fi recapturat: {error}"),
                ))
            })?;
            validate_named_directory_identity(
                &directory,
                component,
                &next,
                &lexical.public_label,
                "WAL parent component",
            )
            .map_err(CaptureFailure::after_effect)?;
            sync_directory(&directory, &lexical.public_label)
                .map_err(CaptureFailure::after_effect)?;
            directory = next;
        }
        Ok(CapturedParent {
            directory,
            leaf: leaf.clone(),
            created_ancestors: created,
        })
    }

    fn wal_recovery_effect(
        bytes_written: u64,
        public_label: &str,
        diagnostic: impl Into<String>,
    ) -> CapabilityEffect {
        CapabilityEffect::recovery_required(
            bytes_written,
            capability_error(
                public_label,
                &format!(
                    "{} Recordul WAL rămâne hot; nu repeta operația automat.",
                    diagnostic.into()
                ),
            ),
        )
    }

    enum RecoveryAtomicContext {
        ParentMissing {
            existing_components: usize,
            planned_existing_components: usize,
        },
        Ready {
            directory: OwnedFd,
            target_leaf: OsString,
            temp_leaf: OsString,
            parent_was_missing: bool,
        },
    }

    enum RecoveryAppendContext {
        ParentMissing {
            existing_components: usize,
            planned_existing_components: usize,
        },
        Ready {
            directory: OwnedFd,
            target_leaf: OsString,
            parent_was_missing: bool,
        },
    }

    enum AppendSuffixState {
        Complete,
        PartialExact,
        Conflict(String),
    }

    pub(super) fn classify_atomic_recovery(
        record: &WalRecord,
        phase: WalPhase,
        read_budget: &mut RecoveryReadBudget,
    ) -> Result<AtomicRecoveryAssessment, String> {
        let WalOperationEvidence::AtomicFile(evidence) = &record.body.operation_evidence else {
            return Err("WriteAuthority WAL atomic classifier a primit altă familie.".into());
        };
        let context = capture_recovery_atomic_context(record, evidence)?;
        let RecoveryAtomicContext::Ready {
            directory,
            target_leaf,
            temp_leaf,
            parent_was_missing,
        } = context
        else {
            let RecoveryAtomicContext::ParentMissing {
                existing_components,
                planned_existing_components,
            } = context
            else {
                unreachable!()
            };
            if phase == WalPhase::Prepared && existing_components == planned_existing_components {
                return Ok(AtomicRecoveryAssessment {
                    classification: WriteAuthorityRecoveryClassification::NoEffect,
                    automatic_action: Some(AtomicRecoveryAction::ClearNoEffect),
                    diagnostic:
                        "Parentul target este încă absent exact de la frontiera planificată; niciun efect atomic nu este vizibil."
                            .into(),
                });
            }
            return Ok(AtomicRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::Conflict,
                automatic_action: None,
                diagnostic: format!(
                    "AtomicFile {phase:?} nu poate atribui un parent absent/parțial (observedPrefix={existing_components}, plannedPrefix={planned_existing_components}); numai Prepared exact no-effect se elimină automat."
                ),
            });
        };

        let target = observe_recovery_leaf(
            &directory,
            &target_leaf,
            &record.body.public_label,
            "target",
            read_budget,
        )?;
        let temp = observe_recovery_leaf(
            &directory,
            &temp_leaf,
            &record.body.public_label,
            "temp",
            read_budget,
        )?;
        let target_is_before = target == evidence.before;
        let target_is_new = leaf_matches_new(&target, evidence);
        let temp_is_absent = matches!(temp, WalLeafEvidence::Absent);
        let temp_is_new = leaf_matches_new(&temp, evidence);
        let temp_is_old =
            evidence.replace && leaf_matches_relocated_before(&temp, &evidence.before);

        if phase == WalPhase::Prepared && !parent_was_missing && target_is_before && temp_is_absent
        {
            return Ok(AtomicRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::NoEffect,
                automatic_action: Some(AtomicRecoveryAction::ClearNoEffect),
                diagnostic:
                    "AtomicFile Prepared este exact baseline, iar temp-ul lipsește; clear no-effect este singura acțiune automată legacy."
                        .into(),
            });
        }

        if phase == WalPhase::Prepared {
            return Ok(AtomicRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::Conflict,
                automatic_action: None,
                diagnostic: format!(
                    "AtomicFile Prepared a observat namespace/payload care nu poate fi atribuit operației (parentCreated={parent_was_missing}, targetBefore={target_is_before}, targetNew={target_is_new}, tempAbsent={temp_is_absent}, tempNew={temp_is_new}, tempOld={temp_is_old}); competitorii rămân neatinși."
                ),
            });
        }

        let plausible = match phase {
            WalPhase::AuxiliaryDurable => {
                target_is_before && temp_is_new || target_is_new && (temp_is_absent || temp_is_old)
            }
            WalPhase::EffectVisible => target_is_new && (temp_is_absent || temp_is_old),
            WalPhase::TargetDurable => target_is_new && temp_is_absent,
            WalPhase::Preparing | WalPhase::Prepared => false,
        };
        if !plausible {
            return Ok(AtomicRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::Conflict,
                automatic_action: None,
                diagnostic: format!(
                    "AtomicFile {phase:?} a observat o stare incompatibilă cu ordinea runtime (targetBefore={target_is_before}, targetNew={target_is_new}, tempAbsent={temp_is_absent}, tempNew={temp_is_new}, tempOld={temp_is_old}); nicio mutație recovery nu este permisă."
                ),
            });
        }

        if target_is_before && temp_is_new {
            return Ok(AtomicRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::StagedOnly,
                automatic_action: None,
                diagnostic:
                    "AtomicFile AuxiliaryDurable are un temp cu forma payloadului, dar protocolul legacy nu persistă identitatea cauzală; temp-ul rămâne hot și nu este șters automat."
                        .into(),
            });
        }
        if target_is_new && temp_is_absent {
            return Ok(AtomicRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::EffectCommitted,
                automatic_action: None,
                diagnostic: format!(
                    "AtomicFile {phase:?} are forma payloadului la target, dar protocolul legacy nu persistă identitatea post-create; finalizarea automată este interzisă."
                ),
            });
        }
        if target_is_new && temp_is_old {
            return Ok(AtomicRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::CleanupRequired,
                automatic_action: None,
                diagnostic:
                    "AtomicFile are forma exchange-ului, dar cleanup-ul legacy prin unlink nu are CAS identity→effect; baseline-ul izolat rămâne hot și neatins."
                        .into(),
            });
        }

        Ok(AtomicRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::Conflict,
            automatic_action: None,
            diagnostic: format!(
                "Oracle-ul atomic nu recunoaște combinația target/temp (targetBefore={target_is_before}, targetNew={target_is_new}, tempAbsent={temp_is_absent}, tempNew={temp_is_new}, tempOld={temp_is_old})."
            ),
        })
    }

    pub(super) fn execute_atomic_recovery(
        record: &WalRecord,
        phase: WalPhase,
        read_budget: &mut RecoveryReadBudget,
    ) -> Result<(), String> {
        let assessment = classify_atomic_recovery(record, phase, read_budget)?;
        let action = assessment.automatic_action.ok_or_else(|| {
            format!(
                "WriteAuthority WAL recovery CAS nu mai permite acțiune automată; oracle-ul curent este {:?}: {}",
                assessment.classification, assessment.diagnostic
            )
        })?;
        match (phase, action) {
            (WalPhase::Prepared, AtomicRecoveryAction::ClearNoEffect) => Ok(()),
            _ => Err(format!(
                "WriteAuthority AtomicFile legacy permite automat numai Prepared/ClearNoEffect, nu {phase:?}/{action:?}."
            )),
        }
    }

    pub(super) fn discard_rebuildable_atomic_projection(
        record: &WalRecord,
        phase: WalPhase,
    ) -> Result<(), String> {
        let WalOperationEvidence::AtomicFile(evidence) = &record.body.operation_evidence else {
            return Err("Cleanup-ul proiecției rebuildable cere evidence AtomicFile.".into());
        };
        if phase == WalPhase::Preparing {
            return Ok(());
        }
        let context = capture_recovery_atomic_context(record, evidence)?;
        let RecoveryAtomicContext::Ready {
            directory,
            temp_leaf,
            ..
        } = context
        else {
            // A missing planned parent means neither the derived target nor its
            // deterministic temp can be present at the authority location.
            return Ok(());
        };
        let descriptor = match fs::openat(
            &directory,
            &temp_leaf,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(descriptor) => descriptor,
            Err(Errno::NOENT) => return Ok(()),
            Err(error) => {
                return Err(capability_error(
                    &record.body.public_label,
                    &format!("temp-ul rebuildable nu poate fi capturat pentru cleanup: {error}"),
                ));
            }
        };
        let descriptor_stat = fs::fstat(&descriptor).map_err(|error| {
            capability_error(
                &record.body.public_label,
                &format!("temp-ul rebuildable nu poate fi verificat: {error}"),
            )
        })?;
        if FileType::from_raw_mode(descriptor_stat.st_mode) != FileType::RegularFile
            || descriptor_stat.st_nlink != 1
        {
            return Err(capability_error(
                &record.body.public_label,
                "temp-ul rebuildable nu este un fișier regular single-link",
            ));
        }
        validate_named_file_identity(
            &directory,
            &temp_leaf,
            &descriptor_stat,
            "rebuildable-projection-temp",
        )?;
        fs::unlinkat(&directory, &temp_leaf, AtFlags::empty()).map_err(|error| {
            capability_error(
                &record.body.public_label,
                &format!("temp-ul rebuildable nu a putut fi eliminat: {error}"),
            )
        })?;
        sync_directory(&directory, &record.body.public_label)
    }

    pub(super) fn classify_legacy_append_recovery(
        record: &WalRecord,
        phase: WalPhase,
        read_budget: &mut RecoveryReadBudget,
    ) -> Result<AppendRecoveryAssessment, String> {
        let WalOperationEvidence::Append(evidence) = &record.body.operation_evidence else {
            return Err("WriteAuthority WAL append classifier a primit altă familie.".into());
        };
        let context = capture_recovery_append_context(record, evidence)?;
        let RecoveryAppendContext::Ready {
            directory,
            target_leaf,
            parent_was_missing,
        } = context
        else {
            let RecoveryAppendContext::ParentMissing {
                existing_components,
                planned_existing_components,
            } = context
            else {
                unreachable!()
            };
            return if phase == WalPhase::Prepared
                && existing_components == planned_existing_components
            {
                Ok(AppendRecoveryAssessment {
                    classification: WriteAuthorityRecoveryClassification::NoEffect,
                    automatic_action: Some(AppendRecoveryAction::ClearNoEffect),
                    diagnostic:
                        "Append Prepared păstrează exact frontiera parentului absent; clear no-effect este singura acțiune automată legacy."
                            .into(),
                })
            } else {
                Ok(AppendRecoveryAssessment {
                    classification: WriteAuthorityRecoveryClassification::Conflict,
                    automatic_action: None,
                    diagnostic: format!(
                        "Append {phase:?} nu poate atribui un parent absent/parțial (observedPrefix={existing_components}, plannedPrefix={planned_existing_components}); nicio mutație recovery nu este permisă."
                    ),
                })
            };
        };

        let Some((mut file, stat)) = open_recovery_regular_leaf(
            &directory,
            &target_leaf,
            &record.body.public_label,
            "append target",
        )?
        else {
            return match (&evidence.before, phase, parent_was_missing) {
                (WalAppendBefore::Absent, WalPhase::Prepared, false) => {
                    Ok(AppendRecoveryAssessment {
                    classification: WriteAuthorityRecoveryClassification::NoEffect,
                    automatic_action: Some(AppendRecoveryAction::ClearNoEffect),
                    diagnostic:
                        "Append Prepared păstrează exact baseline-ul Absent; clear no-effect este singura acțiune automată legacy."
                            .into(),
                    })
                }
                _ => Ok(AppendRecoveryAssessment {
                    classification: WriteAuthorityRecoveryClassification::Conflict,
                    automatic_action: None,
                    diagnostic: format!(
                        "Append {phase:?} nu poate atribui targetul absent (baseline={:?}, parentCreated={parent_was_missing}); WAL-ul rămâne hot.",
                        evidence.before
                    ),
                }),
            };
        };
        let current_size = u64::try_from(stat.st_size)
            .map_err(|_| "WriteAuthority WAL append target are dimensiune negativă.".to_string())?;

        let before_size = match &evidence.before {
            WalAppendBefore::Absent => 0,
            WalAppendBefore::Present {
                identity,
                size,
                version_token,
            } => {
                if stat.st_dev != identity.device || stat.st_ino != identity.inode {
                    return Ok(AppendRecoveryAssessment {
                        classification: WriteAuthorityRecoveryClassification::Conflict,
                        automatic_action: None,
                        diagnostic: "Append target-ul nu mai este inode-ul baseline.".into(),
                    });
                }
                if current_size == *size {
                    return if version_token_for_stat(&stat) == *version_token {
                        match phase {
                            WalPhase::Prepared => Ok(AppendRecoveryAssessment {
                                classification: WriteAuthorityRecoveryClassification::NoEffect,
                                automatic_action: Some(AppendRecoveryAction::ClearNoEffect),
                                diagnostic:
                                    "Append Prepared este exact baseline și nu are suffix; clear no-effect este singura acțiune automată legacy."
                                        .into(),
                            }),
                            WalPhase::AuxiliaryDurable => Ok(AppendRecoveryAssessment {
                                classification: WriteAuthorityRecoveryClassification::NoEffect,
                                automatic_action: None,
                                diagnostic:
                                    "Append AuxiliaryDurable este încă exact baseline înainte de primul byte, dar protocolul legacy rămâne hot fără acțiune automată."
                                        .into(),
                            }),
                            WalPhase::Preparing
                            | WalPhase::EffectVisible
                            | WalPhase::TargetDurable => Ok(AppendRecoveryAssessment {
                                classification: WriteAuthorityRecoveryClassification::Conflict,
                                automatic_action: None,
                                diagnostic: format!(
                                    "Append {phase:?} revendică progres incompatibil cu targetul rămas exact baseline."
                                ),
                            }),
                        }
                    } else {
                        Ok(AppendRecoveryAssessment {
                            classification: WriteAuthorityRecoveryClassification::Conflict,
                            automatic_action: None,
                            diagnostic:
                                "Append target-ul are aceeași dimensiune, dar versiunea baseline s-a schimbat."
                                    .into(),
                        })
                    };
                }
                if current_size < *size {
                    return Ok(AppendRecoveryAssessment {
                        classification: WriteAuthorityRecoveryClassification::Conflict,
                        automatic_action: None,
                        diagnostic: "Append target-ul este mai scurt decât baseline-ul.".into(),
                    });
                }
                *size
            }
        };

        match assess_append_suffix(&mut file, before_size, evidence, read_budget)? {
            AppendSuffixState::Complete if phase == WalPhase::Prepared => {
                Ok(AppendRecoveryAssessment {
                    classification: WriteAuthorityRecoveryClassification::Conflict,
                    automatic_action: None,
                    diagnostic:
                        "Append Prepared a observat suffix-ul complet înainte ca runtime-ul să permită primul byte; payloadul poate aparține unui competitor și rămâne neatins."
                            .into(),
                })
            }
            AppendSuffixState::Complete if phase == WalPhase::Preparing => {
                Ok(AppendRecoveryAssessment {
                    classification: WriteAuthorityRecoveryClassification::Conflict,
                    automatic_action: None,
                    diagnostic:
                        "Append Preparing nu poate conține un suffix publicat; WAL-ul rămâne hot."
                            .into(),
                })
            }
            AppendSuffixState::Complete => Ok(AppendRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::EffectCommitted,
                automatic_action: None,
                diagnostic: format!(
                    "Append {phase:?} are forma payloadului complet, dar protocolul legacy nu persistă identitatea/versiunea cauzală post-write; finalizarea automată este interzisă."
                ),
            }),
            AppendSuffixState::PartialExact if phase == WalPhase::AuxiliaryDurable => {
                Ok(AppendRecoveryAssessment {
                    classification: WriteAuthorityRecoveryClassification::PartialAppend,
                    automatic_action: None,
                    diagnostic:
                        "Append AuxiliaryDurable are un prefix exact al payloadului, dar truncate legacy nu are CAS identity→effect; bytes rămân hot și neatinși."
                            .into(),
                })
            }
            AppendSuffixState::PartialExact => Ok(AppendRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::Conflict,
                automatic_action: None,
                diagnostic: format!(
                    "Append {phase:?} a observat un suffix parțial incompatibil cu faza sau neatribuibil cauzal; truncate este interzis și bytes rămân neatinși."
                ),
            }),
            AppendSuffixState::Conflict(diagnostic) => Ok(AppendRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::Conflict,
                automatic_action: None,
                diagnostic,
            }),
        }
    }

    pub(super) fn execute_legacy_append_recovery(
        record: &WalRecord,
        phase: WalPhase,
        read_budget: &mut RecoveryReadBudget,
    ) -> Result<(), String> {
        let assessment = classify_legacy_append_recovery(record, phase, read_budget)?;
        let action = assessment.automatic_action.ok_or_else(|| {
            format!(
                "WriteAuthority append recovery CAS nu mai permite acțiune automată: {}",
                assessment.diagnostic
            )
        })?;
        match (phase, action) {
            (WalPhase::Prepared, AppendRecoveryAction::ClearNoEffect) => Ok(()),
            _ => Err(format!(
                "WriteAuthority Append legacy permite automat numai Prepared/ClearNoEffect, nu {phase:?}/{action:?}."
            )),
        }
    }

    fn classify_legacy_directory_recovery(
        record: &WalRecord,
        phase: WalPhase,
    ) -> Result<DirectoryRecoveryAssessment, String> {
        let WalOperationEvidence::Directory(evidence) = &record.body.operation_evidence else {
            return Err("WriteAuthority mkdir recovery a primit altă familie.".into());
        };
        let (authority, components) = capture_recovery_directory_authority(record, evidence)?;
        let mut directory = rustix::io::dup(authority.directory()).map_err(|error| {
            format!("WriteAuthority mkdir recovery nu poate duplica boundary: {error}.")
        })?;
        if evidence.existing_prefix_len == 0
            && wal_identity_from_fd(&directory, &record.body.public_label)?
                != evidence.existing_ancestor_identity
        {
            return Err("WriteAuthority mkdir recovery authority identity diferă de plan.".into());
        }
        let mut observed_prefix_len = 0_usize;
        for component in &components {
            match open_directory_strict(&directory, component) {
                Ok(next) => {
                    validate_named_directory_identity(
                        &directory,
                        component,
                        &next,
                        &record.body.public_label,
                        "mkdir recovery component",
                    )?;
                    directory = next;
                    observed_prefix_len += 1;
                    if observed_prefix_len == evidence.existing_prefix_len
                        && wal_identity_from_fd(&directory, &record.body.public_label)?
                            != evidence.existing_ancestor_identity
                    {
                        return Err(
                            "WriteAuthority mkdir recovery ancestorul baseline a fost înlocuit."
                                .into(),
                        );
                    }
                }
                Err(Errno::NOENT) => break,
                Err(error) => {
                    return Err(capability_error(
                        &record.body.public_label,
                        &format!("mkdir recovery a întâlnit un component invalid: {error}"),
                    ));
                }
            }
        }
        if observed_prefix_len < evidence.existing_prefix_len {
            return Ok(DirectoryRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::Conflict,
                automatic_action: None,
                available_resolution_actions: Vec::new(),
                resolution_state_binding: None,
                diagnostic:
                    "Un director baseline din planul mkdir lipsește la restart; manual review obligatoriu."
                        .into(),
            });
        }
        if evidence.existing_prefix_len == components.len() {
            let observed = wal_identity_from_fd(&directory, &record.body.public_label)?;
            return if evidence.existing_target_identity.as_ref() == Some(&observed)
                && phase == WalPhase::Prepared
            {
                Ok(DirectoryRecoveryAssessment {
                    classification: WriteAuthorityRecoveryClassification::NoEffect,
                    automatic_action: Some(DirectoryRecoveryAction::ClearNoEffect),
                    available_resolution_actions: Vec::new(),
                    resolution_state_binding: None,
                    diagnostic:
                        "Directorul exista înainte de WAL, păstrează identitatea baseline, iar faza Prepared este singura fază posibilă pentru acest no-op legacy."
                            .into(),
                })
            } else if evidence.existing_target_identity.as_ref() == Some(&observed) {
                Ok(DirectoryRecoveryAssessment {
                    classification: WriteAuthorityRecoveryClassification::Conflict,
                    automatic_action: None,
                    available_resolution_actions: Vec::new(),
                    resolution_state_binding: None,
                    diagnostic: format!(
                        "Directorul baseline este intact, dar faza {phase:?} este imposibilă pentru no-op-ul mkdir legacy; WAL-ul rămâne hot."
                    ),
                })
            } else {
                Ok(DirectoryRecoveryAssessment {
                    classification: WriteAuthorityRecoveryClassification::Conflict,
                    automatic_action: None,
                    available_resolution_actions: Vec::new(),
                    resolution_state_binding: None,
                    diagnostic:
                        "Directorul existent înainte de WAL are altă identitate la restart.".into(),
                })
            };
        }
        if observed_prefix_len == evidence.existing_prefix_len {
            return Ok(DirectoryRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::Conflict,
                automatic_action: None,
                available_resolution_actions: Vec::new(),
                resolution_state_binding: None,
                diagnostic: format!(
                    "Suffix-ul mkdir planificat absent nu este vizibil în faza {phase:?}, dar mkdirat rulează înaintea primei tranziții de fază legacy; un efect creat și apoi eliminat nu poate fi exclus. WAL-ul rămâne hot."
                ),
            });
        }
        if observed_prefix_len == components.len() {
            return Ok(DirectoryRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::EffectCommitted,
                automatic_action: None,
                available_resolution_actions: Vec::new(),
                resolution_state_binding: None,
                diagnostic:
                    "Întregul suffix mkdir este vizibil, dar recordul immutable nu conține identitățile post-create; poate aparține unui actor extern și cere manual review."
                        .into(),
            });
        }
        Ok(DirectoryRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::PartialNamespaceCreation,
            automatic_action: None,
            available_resolution_actions: Vec::new(),
            resolution_state_binding: None,
            diagnostic:
                "Suffix-ul mkdir este parțial, dar identitățile post-create lipsesc; recovery nu poate distinge efectul propriu de namespace extern."
                    .into(),
        })
    }

    fn execute_legacy_directory_recovery(
        record: &WalRecord,
        phase: WalPhase,
        action: DirectoryRecoveryAction,
    ) -> Result<(), String> {
        let assessment = classify_legacy_directory_recovery(record, phase)?;
        if assessment.automatic_action != Some(action) {
            return Err(format!(
                "WriteAuthority mkdir recovery CAS a refuzat {action:?}: {}",
                assessment.diagnostic
            ));
        }
        match (phase, action) {
            (WalPhase::Prepared, DirectoryRecoveryAction::ClearNoEffect) => Ok(()),
            _ => Err(format!(
                "WriteAuthority mkdir legacy permite automat numai Prepared/ClearNoEffect pentru un target baseline existent, nu {phase:?}/{action:?}."
            )),
        }
    }

    fn capture_recovery_directory_authority(
        record: &WalRecord,
        evidence: &WalDirectoryEvidence,
    ) -> Result<(DirectoryAuthority, Vec<OsString>), String> {
        let boundary_path = decode_path_hex(&record.body.authority.boundary_path_hex)?;
        if !boundary_path.is_absolute() {
            return Err("WriteAuthority mkdir recovery refuză boundary non-absolut.".into());
        }
        let authority = capture_directory_authority(
            &boundary_path,
            "write-authority-wal/mkdir-recovery-target",
            DirectoryAuthorityScope::RecoveryTarget,
        )?;
        let identity = authority.identity();
        if identity.device != record.body.authority.identity.device
            || identity.inode != record.body.authority.identity.inode
        {
            return Err("WriteAuthority mkdir recovery boundary identity diferă.".into());
        }
        let components = evidence
            .relative_components_hex
            .iter()
            .map(|component| decode_component_hex(component))
            .collect::<Result<Vec<_>, _>>()?;
        Ok((authority, components))
    }

    fn capture_recovery_append_context(
        record: &WalRecord,
        evidence: &WalAppendEvidence,
    ) -> Result<RecoveryAppendContext, String> {
        let boundary_path = decode_path_hex(&record.body.authority.boundary_path_hex)?;
        if !boundary_path.is_absolute() {
            return Err("WriteAuthority append recovery refuză boundary non-absolut.".into());
        }
        let authority = capture_directory_authority(
            &boundary_path,
            "write-authority-wal/append-recovery-target",
            DirectoryAuthorityScope::RecoveryTarget,
        )?;
        let identity = authority.identity();
        if identity.device != record.body.authority.identity.device
            || identity.inode != record.body.authority.identity.inode
        {
            return Err("WriteAuthority append recovery boundary identity diferă.".into());
        }
        let parents = evidence
            .parent
            .relative_components_hex
            .iter()
            .map(|component| decode_component_hex(component))
            .collect::<Result<Vec<_>, _>>()?;
        let target_leaf = decode_component_hex(&evidence.target_leaf_hex)?;
        let mut directory = rustix::io::dup(authority.directory()).map_err(|error| {
            format!("WriteAuthority append recovery nu poate duplica boundary: {error}.")
        })?;
        let mut existing_components = 0_usize;
        for component in &parents {
            match open_directory_strict(&directory, component) {
                Ok(next) => {
                    directory = next;
                    existing_components += 1;
                }
                Err(Errno::NOENT) => {
                    if existing_components == evidence.parent.existing_prefix_len {
                        let observed = wal_identity_from_fd(&directory, &record.body.public_label)?;
                        if observed != evidence.parent.existing_ancestor_identity {
                            return Err(capability_error(
                                &record.body.public_label,
                                "Append recovery frontiera parentului absent nu mai este ancestorul baseline",
                            ));
                        }
                    }
                    return Ok(RecoveryAppendContext::ParentMissing {
                        existing_components,
                        planned_existing_components: evidence.parent.existing_prefix_len,
                    });
                }
                Err(error) => {
                    return Err(format!(
                        "WriteAuthority append recovery nu poate captura parentul: {error}."
                    ));
                }
            }
        }
        let observed = wal_identity_from_fd(&directory, &record.body.public_label)?;
        if let Some(expected) = &evidence.parent.parent_identity {
            if &observed != expected {
                return Err("WriteAuthority append recovery parent identity diferă.".into());
            }
        }
        Ok(RecoveryAppendContext::Ready {
            directory,
            target_leaf,
            parent_was_missing: evidence.parent.parent_identity.is_none(),
        })
    }

    fn open_recovery_regular_leaf(
        parent: &OwnedFd,
        leaf: &OsStr,
        public_label: &str,
        role: &str,
    ) -> Result<Option<(File, fs::Stat)>, String> {
        let Some(metadata) = leaf_metadata(parent, leaf, public_label)? else {
            return Ok(None);
        };
        if FileType::from_raw_mode(metadata.st_mode) != FileType::RegularFile {
            return Err(capability_error(
                public_label,
                &format!("{role} nu este fișier regular"),
            ));
        }
        let descriptor = fs::openat(
            parent,
            leaf,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| {
            capability_error(public_label, &format!("{role} open a eșuat: {error}"))
        })?;
        validate_regular_single_link(&descriptor, public_label, role)?;
        let file = File::from(descriptor);
        let stat = fs::fstat(&file).map_err(|error| {
            capability_error(public_label, &format!("{role} stat a eșuat: {error}"))
        })?;
        if !same_file_identity(&metadata, &stat) {
            return Err(capability_error(
                public_label,
                &format!("{role} s-a schimbat în timpul capturii"),
            ));
        }
        validate_named_file_identity(parent, leaf, &stat, role)?;
        Ok(Some((file, stat)))
    }

    fn assess_append_suffix(
        file: &mut File,
        before_size: u64,
        evidence: &WalAppendEvidence,
        read_budget: &mut RecoveryReadBudget,
    ) -> Result<AppendSuffixState, String> {
        const MAX_APPEND_AUTO_RECOVERY_BYTES: u64 = 128 * 1024 * 1024;
        let stat = fs::fstat(&*file)
            .map_err(|error| format!("Append recovery suffix stat a eșuat: {error}."))?;
        let current_size = u64::try_from(stat.st_size)
            .map_err(|_| "Append recovery suffix are dimensiune negativă.".to_string())?;
        if current_size < before_size {
            return Ok(AppendSuffixState::Conflict(
                "Append target-ul este mai scurt decât beforeSize.".into(),
            ));
        }
        let suffix_size = current_size - before_size;
        if suffix_size > evidence.payload_size {
            return Ok(AppendSuffixState::Conflict(
                "Append target-ul conține bytes concurenți după payload.".into(),
            ));
        }
        if suffix_size > MAX_APPEND_AUTO_RECOVERY_BYTES {
            return Ok(AppendSuffixState::Conflict(format!(
                "Append suffix depășește limita auto-recovery de {MAX_APPEND_AUTO_RECOVERY_BYTES} bytes."
            )));
        }
        read_budget.reserve(suffix_size, "append recovery suffix")?;
        file.seek(SeekFrom::Start(before_size))
            .map_err(|error| format!("Append recovery seek a eșuat: {error}."))?;
        let mut suffix = Vec::with_capacity(suffix_size as usize);
        file.take(suffix_size.saturating_add(1))
            .read_to_end(&mut suffix)
            .map_err(|error| format!("Append recovery suffix read a eșuat: {error}."))?;
        if suffix.len() as u64 != suffix_size {
            return Ok(AppendSuffixState::Conflict(
                "Append suffix s-a schimbat în timpul citirii.".into(),
            ));
        }
        if suffix_size == evidence.payload_size {
            return Ok(if sha256_bytes(&suffix) == evidence.payload_hash {
                AppendSuffixState::Complete
            } else {
                AppendSuffixState::Conflict(
                    "Append suffix complet are alt hash decât payloadul.".into(),
                )
            });
        }
        let prefix = decode_bytes_hex(&evidence.payload_prefix_hex)?;
        if suffix.len() <= prefix.len() && suffix == prefix[..suffix.len()] {
            Ok(AppendSuffixState::PartialExact)
        } else {
            Ok(AppendSuffixState::Conflict(
                "Append suffix parțial nu este prefix exact al payloadului persistat.".into(),
            ))
        }
    }

    fn capture_recovery_atomic_context(
        record: &WalRecord,
        evidence: &WalAtomicFileEvidence,
    ) -> Result<RecoveryAtomicContext, String> {
        let boundary_path = decode_path_hex(&record.body.authority.boundary_path_hex)?;
        if !boundary_path.is_absolute() {
            return Err("WriteAuthority WAL recovery refuză boundary non-absolut.".into());
        }
        let authority = capture_directory_authority(
            &boundary_path,
            "write-authority-wal/recovery-target",
            DirectoryAuthorityScope::RecoveryTarget,
        )?;
        let identity = authority.identity();
        if identity.device != record.body.authority.identity.device
            || identity.inode != record.body.authority.identity.inode
        {
            return Err(format!(
                "WriteAuthority WAL boundary identity diferă: expected dev={} ino={}, observed dev={} ino={}.",
                record.body.authority.identity.device,
                record.body.authority.identity.inode,
                identity.device,
                identity.inode
            ));
        }
        let parents = evidence
            .parent
            .relative_components_hex
            .iter()
            .map(|component| decode_component_hex(component))
            .collect::<Result<Vec<_>, _>>()?;
        let target_leaf = decode_component_hex(&evidence.target_leaf_hex)?;
        let temp_leaf = decode_component_hex(&evidence.temp_leaf_hex)?;
        let mut directory = rustix::io::dup(authority.directory()).map_err(|error| {
            format!("WriteAuthority WAL recovery nu poate duplica boundary handle: {error}.")
        })?;
        let mut existing_components = 0_usize;
        for component in &parents {
            match open_directory_strict(&directory, component) {
                Ok(next) => {
                    directory = next;
                    existing_components += 1;
                }
                Err(Errno::NOENT) => {
                    if existing_components == evidence.parent.existing_prefix_len {
                        let observed = wal_identity_from_fd(&directory, &record.body.public_label)?;
                        if observed != evidence.parent.existing_ancestor_identity {
                            return Err(capability_error(
                                &record.body.public_label,
                                "WAL recovery frontiera parentului absent nu mai este ancestorul baseline",
                            ));
                        }
                    }
                    return Ok(RecoveryAtomicContext::ParentMissing {
                        existing_components,
                        planned_existing_components: evidence.parent.existing_prefix_len,
                    });
                }
                Err(error) => {
                    return Err(capability_error(
                        &record.body.public_label,
                        &format!("WAL recovery nu poate captura parentul: {error}"),
                    ));
                }
            }
        }
        let observed_parent = wal_identity_from_fd(&directory, &record.body.public_label)?;
        let parent_was_missing = evidence.parent.parent_identity.is_none();
        if let Some(expected_parent) = &evidence.parent.parent_identity {
            if &observed_parent != expected_parent {
                return Err(capability_error(
                    &record.body.public_label,
                    "WAL recovery parent identity diferă de record",
                ));
            }
        }
        Ok(RecoveryAtomicContext::Ready {
            directory,
            target_leaf,
            temp_leaf,
            parent_was_missing,
        })
    }

    fn observe_recovery_leaf(
        parent: &OwnedFd,
        leaf: &OsStr,
        public_label: &str,
        role: &str,
        read_budget: &mut RecoveryReadBudget,
    ) -> Result<WalLeafEvidence, String> {
        let evidence = capture_wal_leaf_evidence(
            parent,
            leaf,
            &ExpectedLeaf::Unspecified,
            public_label,
            Some(read_budget),
        )?;
        if let WalLeafEvidence::Regular { identity, .. } = &evidence {
            let stat = fs::statat(parent, leaf, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
                capability_error(
                    public_label,
                    &format!("WAL recovery {role} stat a eșuat: {error}"),
                )
            })?;
            if stat.st_nlink != 1 || stat.st_dev != identity.device || stat.st_ino != identity.inode
            {
                return Err(capability_error(
                    public_label,
                    &format!("WAL recovery {role} nu este single-link stabil"),
                ));
            }
        }
        Ok(evidence)
    }

    fn leaf_matches_new(evidence: &WalLeafEvidence, plan: &WalAtomicFileEvidence) -> bool {
        matches!(
            evidence,
            WalLeafEvidence::Regular {
                size,
                content_hash,
                ..
            } if *size == plan.new_size && *content_hash == plan.new_content_hash
        )
    }

    fn leaf_matches_relocated_before(observed: &WalLeafEvidence, before: &WalLeafEvidence) -> bool {
        matches!(
            (observed, before),
            (
                WalLeafEvidence::Regular {
                    identity: observed_identity,
                    size: observed_size,
                    content_hash: observed_hash,
                    ..
                },
                WalLeafEvidence::Regular {
                    identity: before_identity,
                    size: before_size,
                    content_hash: before_hash,
                    ..
                }
            ) if observed_identity == before_identity
                && observed_size == before_size
                && observed_hash == before_hash
        )
    }

    pub(super) fn append(target: &WriteTarget, bytes: &[u8]) -> Result<CapabilityEffect, String> {
        let lexical = lexical_target(target, false)?;
        let parent = match capture_target_parent(&lexical, true) {
            Ok(Some(parent)) => parent,
            Ok(None) => {
                return Err(capability_error(
                    &lexical.public_label,
                    "folderul părinte nu a putut fi capturat",
                ));
            }
            Err(error) => return error.into_operation_result(),
        };
        run_test_hook(CapabilityTestStage::AfterTargetParentCaptured);

        let leaf_was_absent =
            match leaf_metadata(&parent.directory, &parent.leaf, &lexical.public_label) {
                Ok(metadata) => metadata.is_none(),
                Err(error) => {
                    return settle_after_implicit_parent_creation(
                        parent.created_ancestors,
                        Err(error),
                        &lexical.public_label,
                    );
                }
            };

        let descriptor = match fs::openat(
            &parent.directory,
            &parent.leaf,
            OFlags::WRONLY
                | OFlags::APPEND
                | OFlags::CREATE
                | OFlags::NOFOLLOW
                | OFlags::NONBLOCK
                | OFlags::CLOEXEC,
            FILE_MODE,
        ) {
            Ok(descriptor) => descriptor,
            Err(error) => {
                return settle_after_implicit_parent_creation(
                    parent.created_ancestors,
                    Err(capability_error(
                &lexical.public_label,
                &format!(
                    "AppendText cere un fișier regular deschis fără symlink; leaf-ul a fost refuzat: {error}"
                ),
                    )),
                    &lexical.public_label,
                );
            }
        };
        run_test_hook(CapabilityTestStage::AfterAppendLeafOpened);
        let result = (|| {
            validate_regular_single_link(&descriptor, &lexical.public_label, "AppendText")?;
            fs::flock(&descriptor, FlockOperation::LockExclusive).map_err(|error| {
                capability_error(
                    &lexical.public_label,
                    &format!("append-ul nu a putut obține lock exclusiv: {error}"),
                )
            })?;
            // Revalidate after acquiring the cooperative writer lock. The lock
            // serializes Pană Studio writers; the second fstat also closes the
            // interval between the initial type check and lock acquisition.
            validate_regular_single_link(&descriptor, &lexical.public_label, "AppendText")?;

            let mut file = File::from(descriptor);
            if let Err(error) = file.write_all(bytes) {
                let sync_diagnostic = file
                    .sync_data()
                    .err()
                    .map(|sync_error| format!("; sync_data a eșuat de asemenea: {sync_error}"))
                    .unwrap_or_default();
                return Ok(CapabilityEffect::recovery_required(
                    0,
                    capability_error(
                        &lexical.public_label,
                        &format!(
                            "append-ul poate fi parțial după eroarea de scriere: {error}{sync_diagnostic}. Nu repeta recordul automat"
                        ),
                    ),
                ));
            }
            if let Err(error) = file.sync_data() {
                return Ok(CapabilityEffect::recovery_required(
                    bytes.len() as u64,
                    capability_error(
                        &lexical.public_label,
                        &format!(
                            "append-ul este vizibil, dar sync_data a eșuat: {error}. Nu repeta recordul automat"
                        ),
                    ),
                ));
            }
            if let Err(error) = sync_directory(&parent.directory, &lexical.public_label) {
                return Ok(CapabilityEffect::recovery_required(
                    bytes.len() as u64,
                    format!("{error} Append-ul este deja vizibil; nu repeta recordul automat."),
                ));
            }

            Ok(CapabilityEffect::changed(bytes.len() as u64))
        })();
        settle_after_implicit_parent_creation(
            parent.created_ancestors || leaf_was_absent,
            result,
            &lexical.public_label,
        )
    }

    fn create_legacy_directory_all_wal(
        target: &WriteTarget,
        plan: &DirectoryOperationPlan,
        guard: &mut DurableWalGuard<'_>,
    ) -> Result<CapabilityEffect, String> {
        let lexical = lexical_target(target, true)?;
        let planned_components = plan
            .evidence
            .relative_components_hex
            .iter()
            .map(|component| decode_component_hex(component))
            .collect::<Result<Vec<_>, _>>()?;
        if planned_components != lexical.relative_components {
            return Err(capability_error(
                &lexical.public_label,
                "path-ul mkdir nu corespunde planului WAL",
            ));
        }
        let authority = lexical.authority.as_ref().ok_or_else(|| {
            capability_error(
                &lexical.public_label,
                "execuția mkdir WAL cere authority root sigilat",
            )
        })?;
        let mut directory = rustix::io::dup(authority.directory()).map_err(|error| {
            capability_error(
                &lexical.public_label,
                &format!("authority mkdir nu poate fi duplicată: {error}"),
            )
        })?;
        for component in lexical
            .relative_components
            .iter()
            .take(plan.evidence.existing_prefix_len)
        {
            directory = open_directory_strict(&directory, component).map_err(|error| {
                capability_error(
                    &lexical.public_label,
                    &format!("baseline-ul mkdir nu mai poate fi capturat: {error}"),
                )
            })?;
        }
        if wal_identity_from_fd(&directory, &lexical.public_label)?
            != plan.evidence.existing_ancestor_identity
        {
            return Err(capability_error(
                &lexical.public_label,
                "ancestorul mkdir diferă de identitatea din planul WAL",
            ));
        }

        if plan.evidence.existing_prefix_len == lexical.relative_components.len() {
            let observed = wal_identity_from_fd(&directory, &lexical.public_label)?;
            if plan.evidence.existing_target_identity.as_ref() != Some(&observed) {
                return Err(capability_error(
                    &lexical.public_label,
                    "directorul existent s-a schimbat după planificare",
                ));
            }
            validate_directory_runtime_postflight(&lexical, &observed)?;
            return Ok(CapabilityEffect::unchanged());
        }

        let mut changed = false;
        for component in lexical
            .relative_components
            .iter()
            .skip(plan.evidence.existing_prefix_len)
        {
            match open_directory_strict(&directory, component) {
                Err(Errno::NOENT) => {}
                Ok(_) => {
                    let diagnostic = capability_error(
                        &lexical.public_label,
                        "un component mkdir planificat absent a apărut înaintea efectului",
                    );
                    return if changed {
                        Ok(wal_recovery_effect(0, &lexical.public_label, diagnostic))
                    } else {
                        Err(diagnostic)
                    };
                }
                Err(error) => {
                    let diagnostic = capability_error(
                        &lexical.public_label,
                        &format!("componentul mkdir nu poate fi reverificat: {error}"),
                    );
                    return if changed {
                        Ok(wal_recovery_effect(0, &lexical.public_label, diagnostic))
                    } else {
                        Err(diagnostic)
                    };
                }
            }
            if let Err(error) = fs::mkdirat(&directory, component, DIRECTORY_MODE) {
                let diagnostic = capability_error(
                    &lexical.public_label,
                    &format!("mkdirat protejat de WAL a eșuat: {error}"),
                );
                return if changed {
                    Ok(wal_recovery_effect(0, &lexical.public_label, diagnostic))
                } else {
                    Err(diagnostic)
                };
            }
            run_test_hook(CapabilityTestStage::AfterDirectoryCreateBeforePhase);
            let first_effect = !changed;
            changed = true;
            let next = match open_directory_strict(&directory, component) {
                Ok(next) => next,
                Err(error) => {
                    return Ok(wal_recovery_effect(
                        0,
                        &lexical.public_label,
                        format!("directorul creat nu poate fi recapturat: {error}"),
                    ));
                }
            };
            if let Err(error) = validate_named_directory_identity(
                &directory,
                component,
                &next,
                &lexical.public_label,
                "mkdir WAL component",
            ) {
                return Ok(wal_recovery_effect(0, &lexical.public_label, error));
            }
            if let Err(error) = sync_directory(&directory, &lexical.public_label) {
                return Ok(wal_recovery_effect(0, &lexical.public_label, error));
            }
            if first_effect {
                if let Err(error) = guard.mark_auxiliary_durable() {
                    return Ok(wal_recovery_effect(0, &lexical.public_label, error));
                }
                if let Err(error) = guard.mark_effect_visible() {
                    return Ok(wal_recovery_effect(0, &lexical.public_label, error));
                }
            }
            directory = next;
        }
        if let Err(error) = sync_directory(&directory, &lexical.public_label) {
            return Ok(wal_recovery_effect(0, &lexical.public_label, error));
        }
        run_test_hook(CapabilityTestStage::BeforeDirectoryTargetDurable);
        let final_identity = match wal_identity_from_fd(&directory, &lexical.public_label) {
            Ok(identity) => identity,
            Err(error) => {
                return Ok(wal_recovery_effect(0, &lexical.public_label, error));
            }
        };
        if let Err(error) = validate_directory_runtime_postflight(&lexical, &final_identity) {
            return Ok(wal_recovery_effect(0, &lexical.public_label, error));
        }
        if let Err(error) = guard.mark_target_durable() {
            return Ok(wal_recovery_effect(0, &lexical.public_label, error));
        }
        Ok(CapabilityEffect::changed(0))
    }

    fn validate_directory_runtime_postflight(
        lexical: &LexicalTarget,
        expected_target: &WalFilesystemIdentity,
    ) -> Result<(), String> {
        let authority = lexical.authority.as_ref().ok_or_else(|| {
            capability_error(
                &lexical.public_label,
                "mkdir postflight cere authority root sigilat",
            )
        })?;
        let mut directory = rustix::io::dup(authority.directory()).map_err(|error| {
            capability_error(
                &lexical.public_label,
                &format!("mkdir postflight nu poate duplica authority: {error}"),
            )
        })?;
        for component in &lexical.relative_components {
            let next = open_directory_strict(&directory, component).map_err(|error| {
                capability_error(
                    &lexical.public_label,
                    &format!("mkdir postflight nu poate recaptura path-ul: {error}"),
                )
            })?;
            validate_named_directory_identity(
                &directory,
                component,
                &next,
                &lexical.public_label,
                "mkdir postflight component",
            )?;
            directory = next;
        }
        let observed = wal_identity_from_fd(&directory, &lexical.public_label)?;
        if &observed != expected_target {
            return Err(capability_error(
                &lexical.public_label,
                "mkdir postflight path-ul nu mai numește inode-ul sincronizat",
            ));
        }
        Ok(())
    }

    pub(super) fn create_directory_all(target: &WriteTarget) -> Result<CapabilityEffect, String> {
        let lexical = lexical_target(target, true)?;
        let mut boundary = match capture_boundary(&lexical, true) {
            Ok(Some(boundary)) => boundary,
            Ok(None) => {
                return Err(capability_error(
                    &lexical.public_label,
                    "boundary-ul nu a putut fi capturat sau creat",
                ));
            }
            Err(error) => return error.into_operation_result(),
        };
        let mut changed = boundary.created;

        for component in &lexical.relative_components {
            let (next, created) = match open_or_create_directory_component(
                &boundary.directory,
                component,
                &lexical.public_label,
            ) {
                Ok(result) => result,
                Err(error) => {
                    return if changed {
                        error.promote().into_operation_result()
                    } else {
                        error.into_operation_result()
                    };
                }
            };
            changed |= created;
            boundary.directory = next;
        }
        if let Err(error) = sync_directory(&boundary.directory, &lexical.public_label) {
            if changed {
                return Ok(CapabilityEffect::recovery_required(
                    0,
                    format!(
                        "{error} Directorul a fost creat, dar durabilitatea lui cere recovery; nu repeta operația automat."
                    ),
                ));
            }
            return Err(error);
        }

        Ok(CapabilityEffect {
            changed,
            bytes_written: 0,
            recovery_required: false,
            diagnostic: None,
        })
    }

    pub(super) fn remove_file_if_exists(target: &WriteTarget) -> Result<CapabilityEffect, String> {
        let lexical = lexical_target(target, false)?;
        let Some(parent) = capture_existing_target_parent(&lexical)? else {
            return Ok(CapabilityEffect::unchanged());
        };
        run_test_hook(CapabilityTestStage::AfterTargetParentCaptured);

        if let ExpectedLeaf::Present(expected) = &target.expected_leaf {
            return remove_expected_file(&parent, expected, &lexical.public_label);
        }

        let Some(metadata) = leaf_metadata(&parent.directory, &parent.leaf, &lexical.public_label)?
        else {
            return Ok(CapabilityEffect::unchanged());
        };
        if FileType::from_raw_mode(metadata.st_mode) == FileType::Directory {
            return Err(capability_error(
                &lexical.public_label,
                "RemoveFile a primit un director; folosește RemoveDirectoryTree",
            ));
        }

        match fs::unlinkat(&parent.directory, &parent.leaf, AtFlags::empty()) {
            Ok(()) => match sync_directory(&parent.directory, &lexical.public_label) {
                Ok(()) => Ok(CapabilityEffect::changed(0)),
                Err(error) => Ok(CapabilityEffect::recovery_required(
                    0,
                    format!("{error} Leaf-ul a fost eliminat; nu repeta operația automat."),
                )),
            },
            Err(Errno::NOENT) => Ok(CapabilityEffect::unchanged()),
            Err(error) => Err(capability_error(
                &lexical.public_label,
                &format!("leaf-ul nu a putut fi eliminat: {error}"),
            )),
        }
    }

    fn remove_expected_file(
        parent: &CapturedParent,
        expected: &ExpectedLeafVersion,
        public_label: &str,
    ) -> Result<CapabilityEffect, String> {
        let descriptor = fs::openat(
            &parent.directory,
            &parent.leaf,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| {
            capability_error(
                public_label,
                &format!("leaf-CAS remove nu a putut captura target-ul: {error}"),
            )
        })?;
        let mut captured = File::from(descriptor);
        let captured_before = fs::fstat(&captured).map_err(|error| {
            capability_error(
                public_label,
                &format!("leaf-CAS remove nu a putut citi metadata: {error}"),
            )
        })?;
        validate_expected_regular_file(
            &mut captured,
            &captured_before,
            expected,
            public_label,
            "remove pre-commit",
        )?;
        run_test_hook(CapabilityTestStage::AfterExpectedLeafCaptured);

        let quarantine_name =
            quarantine_leaf_noreplace(&parent.directory, &parent.leaf, public_label)?;
        let quarantined = match fs::statat(
            &parent.directory,
            &quarantine_name,
            AtFlags::SYMLINK_NOFOLLOW,
        ) {
            Ok(stat) => stat,
            Err(error) => {
                return Ok(CapabilityEffect::recovery_required(
                    0,
                    capability_error(
                        public_label,
                        &format!(
                            "leaf-ul mutat în {} nu poate fi verificat: {error}; recovery necesar",
                            quarantine_name.to_string_lossy()
                        ),
                    ),
                ));
            }
        };
        let validation = (|| {
            if FileType::from_raw_mode(quarantined.st_mode) != FileType::RegularFile
                || !same_file_identity(&captured_before, &quarantined)
            {
                return Err(capability_error(
                    public_label,
                    "remove ar elimina alt inode decât disk baseline-ul capturat",
                ));
            }
            let captured_after = fs::fstat(&captured).map_err(|error| {
                capability_error(
                    public_label,
                    &format!("leaf-ul quarantine nu mai poate fi verificat: {error}"),
                )
            })?;
            if !same_stable_leaf_version(&captured_before, &captured_after) {
                return Err(capability_error(
                    public_label,
                    "leaf-ul s-a modificat în timpul remove-ului condițional",
                ));
            }
            validate_expected_content(
                &mut captured,
                &captured_after,
                expected.content_hash.as_deref(),
                public_label,
                "remove post-quarantine",
            )?;
            let captured_final = fs::fstat(&captured).map_err(|error| {
                capability_error(
                    public_label,
                    &format!("leaf-ul quarantine nu poate fi reverificat: {error}"),
                )
            })?;
            if version_token_for_stat(&captured_after) != version_token_for_stat(&captured_final) {
                return Err(capability_error(
                    public_label,
                    "leaf-ul a suferit o schimbare ABA în timpul postflight-ului remove",
                ));
            }
            Ok(())
        })();

        if let Err(conflict) = validation {
            return restore_leaf_after_conflict(
                &parent.directory,
                &parent.leaf,
                &quarantine_name,
                &quarantined,
                public_label,
                conflict,
            );
        }
        if let Err(error) = validate_named_file_identity(
            &parent.directory,
            &quarantine_name,
            &captured_before,
            "remove-quarantine",
        ) {
            return Ok(CapabilityEffect::recovery_required(
                0,
                format!(
                    "{error} Quarantine {} cere recovery; nu repeta remove-ul automat.",
                    quarantine_name.to_string_lossy()
                ),
            ));
        }
        if let Err(error) = fs::unlinkat(&parent.directory, &quarantine_name, AtFlags::empty()) {
            return Ok(CapabilityEffect::recovery_required(
                0,
                capability_error(
                    public_label,
                    &format!(
                        "leaf-ul este izolat în {}, dar unlink a eșuat: {error}; nu repeta remove-ul automat",
                        quarantine_name.to_string_lossy()
                    ),
                ),
            ));
        }
        match sync_directory(&parent.directory, public_label) {
            Ok(()) => Ok(CapabilityEffect::changed(0)),
            Err(error) => Ok(CapabilityEffect::recovery_required(
                0,
                format!(
                    "{error} Remove-ul leaf-CAS este vizibil, dar durabilitatea este incertă; nu repeta operația automat."
                ),
            )),
        }
    }

    fn quarantine_leaf_noreplace(
        parent: &OwnedFd,
        source_name: &OsStr,
        public_label: &str,
    ) -> Result<OsString, String> {
        for _ in 0..32 {
            let sequence = QUARANTINE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let quarantine_name = OsString::from(format!(
                ".pana-capability-leaf-{}-{sequence}.quarantine",
                std::process::id()
            ));
            match fs::renameat_with(
                parent,
                source_name,
                parent,
                &quarantine_name,
                RenameFlags::NOREPLACE,
            ) {
                Ok(()) => return Ok(quarantine_name),
                Err(Errno::EXIST) => continue,
                Err(error) => {
                    return Err(capability_error(
                        public_label,
                        &format!("leaf-ul nu a putut intra în quarantine: {error}"),
                    ));
                }
            }
        }
        Err(capability_error(
            public_label,
            "nu a putut fi rezervat un nume leaf quarantine unic",
        ))
    }

    fn restore_leaf_after_conflict(
        parent: &OwnedFd,
        original_name: &OsStr,
        quarantine_name: &OsStr,
        expected_identity: &fs::Stat,
        public_label: &str,
        conflict: String,
    ) -> Result<CapabilityEffect, String> {
        if let Err(error) = fs::renameat_with(
            parent,
            quarantine_name,
            parent,
            original_name,
            RenameFlags::NOREPLACE,
        ) {
            return Ok(CapabilityEffect::recovery_required(
                0,
                capability_error(
                    public_label,
                    &format!(
                        "{conflict} Restaurarea din {} a eșuat: {error}; recovery necesar și fără retry automat",
                        quarantine_name.to_string_lossy()
                    ),
                ),
            ));
        }
        let restored = fs::statat(parent, original_name, AtFlags::SYMLINK_NOFOLLOW);
        if !matches!(restored, Ok(ref stat) if same_file_identity(stat, expected_identity)) {
            return Ok(CapabilityEffect::recovery_required(
                0,
                capability_error(
                    public_label,
                    &format!(
                        "{conflict} Numele original nu mai poate demonstra inode-ul restaurat; recovery necesar"
                    ),
                ),
            ));
        }
        if let Err(error) = sync_directory(parent, public_label) {
            return Ok(CapabilityEffect::recovery_required(
                0,
                format!(
                    "{conflict} Leaf-ul a fost restaurat, dar fsync rollback a eșuat: {error} Nu repeta operația automat."
                ),
            ));
        }
        Err(format!(
            "{conflict} Remove-ul leaf-CAS a fost anulat, iar versiunea concurentă a fost restaurată."
        ))
    }

    pub(super) fn rename_noreplace(
        source: &WriteTarget,
        destination: &WriteTarget,
    ) -> Result<CapabilityEffect, String> {
        let source_lexical = lexical_target(source, false)?;
        let destination_lexical = lexical_target(destination, false)?;
        let (source_parent, destination_parent) = if source.boundary_root
            == destination.boundary_root
        {
            // A rename inside one authority must resolve both names from the
            // same captured boundary object. Capturing the absolute boundary
            // twice would reopen a race in which the path is replaced between
            // source and destination acquisition.
            let boundary = capture_existing_boundary(&source_lexical)?.ok_or_else(|| {
                capability_error(&source_lexical.public_label, "boundary-ul sursei nu există")
            })?;
            let source_base = rustix::io::dup(&boundary.directory).map_err(|error| {
                capability_error(
                    &source_lexical.public_label,
                    &format!("boundary-ul comun nu a putut fi duplicat: {error}"),
                )
            })?;
            let source_parent =
                capture_existing_target_parent_from_directory(&source_lexical, source_base)?
                    .ok_or_else(|| {
                        capability_error(
                            &source_lexical.public_label,
                            "folderul părinte al sursei nu există",
                        )
                    })?;
            run_test_hook(CapabilityTestStage::AfterRenameSourceParentCaptured);
            let destination_parent = match capture_target_parent_from_directory(
                &destination_lexical,
                boundary.directory,
                true,
                false,
            ) {
                Ok(Some(parent)) => parent,
                Ok(None) => {
                    return Err(capability_error(
                        &destination_lexical.public_label,
                        "folderul părinte al destinației nu a putut fi capturat",
                    ));
                }
                Err(error) => return error.into_operation_result(),
            };
            (source_parent, destination_parent)
        } else {
            let source_parent =
                capture_existing_target_parent(&source_lexical)?.ok_or_else(|| {
                    capability_error(&source_lexical.public_label, "boundary-ul sursei nu există")
                })?;
            run_test_hook(CapabilityTestStage::AfterRenameSourceParentCaptured);
            let destination_parent = match capture_target_parent(&destination_lexical, true) {
                Ok(Some(parent)) => parent,
                Ok(None) => {
                    return Err(capability_error(
                        &destination_lexical.public_label,
                        "folderul părinte al destinației nu a putut fi capturat",
                    ));
                }
                Err(error) => return error.into_operation_result(),
            };
            (source_parent, destination_parent)
        };
        let result = if let ExpectedLeaf::Present(expected) = &source.expected_leaf {
            rename_expected_noreplace(
                &source_parent,
                &destination_parent,
                expected,
                &source_lexical.public_label,
                &destination_lexical.public_label,
            )
        } else {
            (|| {
                run_test_hook(CapabilityTestStage::BeforeRename);

                fs::renameat_with(
                    &source_parent.directory,
                    &source_parent.leaf,
                    &destination_parent.directory,
                    &destination_parent.leaf,
                    RenameFlags::NOREPLACE,
                )
                .map_err(|error| {
                    capability_error(
                        &source_lexical.public_label,
                        &format!(
                            "rename către {} a fost refuzat fără suprascriere: {error}",
                            destination_lexical.public_label
                        ),
                    )
                })?;
                let mut diagnostics = Vec::new();
                if let Err(error) =
                    sync_directory(&source_parent.directory, &source_lexical.public_label)
                {
                    diagnostics.push(error);
                }
                if let Err(error) = sync_directory(
                    &destination_parent.directory,
                    &destination_lexical.public_label,
                ) {
                    diagnostics.push(error);
                }

                if diagnostics.is_empty() {
                    Ok(CapabilityEffect::changed(0))
                } else {
                    Ok(CapabilityEffect::recovery_required(
                        0,
                        format!(
                            "Rename-ul este deja vizibil, dar sincronizarea directoarelor a eșuat: {} Nu repeta operația automat.",
                            diagnostics.join(" ")
                        ),
                    ))
                }
            })()
        };
        settle_after_implicit_parent_creation(
            destination_parent.created_ancestors,
            result,
            &destination_lexical.public_label,
        )
    }

    /// Atomically publishes a complete rebuildable directory. Both names
    /// must be sibling leaves under one sealed authority. If a previous
    /// artifact exists Linux exchanges the two directory names in one syscall;
    /// otherwise NOREPLACE closes the absent-destination race.
    pub(super) fn publish_rebuildable_directory(
        source: &WriteTarget,
        destination: &WriteTarget,
    ) -> Result<CapabilityEffect, String> {
        let source_lexical = lexical_target(source, false)?;
        let destination_lexical = lexical_target(destination, false)?;
        if source.boundary_root != destination.boundary_root
            || source_lexical.relative_components.len() != 1
            || destination_lexical.relative_components.len() != 1
            || !matches!(
                (source.authority(), destination.authority()),
                (Some(left), Some(right)) if left.same_authority(right)
            )
        {
            return Err(capability_error(
                &source_lexical.public_label,
                "publicarea rebuildable cere două leaf-uri sibling sub aceeași authority sigilată",
            ));
        }

        let boundary = capture_existing_boundary(&source_lexical)?.ok_or_else(|| {
            capability_error(
                &source_lexical.public_label,
                "authority root pentru publicare nu mai există",
            )
        })?;
        let source_leaf = &source_lexical.relative_components[0];
        let destination_leaf = &destination_lexical.relative_components[0];
        if source_leaf == destination_leaf {
            return Err(capability_error(
                &source_lexical.public_label,
                "generația staged și artifactul public nu pot avea același nume",
            ));
        }

        let source_before = fs::statat(&boundary.directory, source_leaf, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|error| {
                capability_error(
                    &source_lexical.public_label,
                    &format!("generația staged nu poate fi capturată: {error}"),
                )
            })?;
        if FileType::from_raw_mode(source_before.st_mode) != FileType::Directory {
            return Err(capability_error(
                &source_lexical.public_label,
                "generația staged nu este un director real",
            ));
        }
        let source_directory =
            open_directory_strict(&boundary.directory, source_leaf).map_err(|error| {
                capability_error(
                    &source_lexical.public_label,
                    &format!("generația staged nu poate fi deschisă sigur: {error}"),
                )
            })?;
        validate_open_directory_identity(
            &source_directory,
            &source_before,
            &source_lexical.public_label,
            "rebuildable publication source",
        )?;

        let previous = match fs::statat(
            &boundary.directory,
            destination_leaf,
            AtFlags::SYMLINK_NOFOLLOW,
        ) {
            Ok(stat) => {
                if FileType::from_raw_mode(stat.st_mode) != FileType::Directory {
                    return Err(capability_error(
                        &destination_lexical.public_label,
                        "artifactul existent nu este un director real",
                    ));
                }
                let directory = open_directory_strict(&boundary.directory, destination_leaf)
                    .map_err(|error| {
                        capability_error(
                            &destination_lexical.public_label,
                            &format!("artifactul existent nu poate fi deschis sigur: {error}"),
                        )
                    })?;
                validate_open_directory_identity(
                    &directory,
                    &stat,
                    &destination_lexical.public_label,
                    "rebuildable publication destination",
                )?;
                Some((stat, directory))
            }
            Err(Errno::NOENT) => None,
            Err(error) => {
                return Err(capability_error(
                    &destination_lexical.public_label,
                    &format!("artifactul existent nu poate fi inspectat: {error}"),
                ));
            }
        };

        let flags = if previous.is_some() {
            RenameFlags::EXCHANGE
        } else {
            RenameFlags::NOREPLACE
        };
        fs::renameat_with(
            &boundary.directory,
            source_leaf,
            &boundary.directory,
            destination_leaf,
            flags,
        )
        .map_err(|error| {
            capability_error(
                &destination_lexical.public_label,
                &format!(
                    "commit-ul atomic al generației Zola a fost refuzat; artifactul precedent rămâne publicat: {error}"
                ),
            )
        })?;

        let postflight = (|| {
            validate_named_directory_identity(
                &boundary.directory,
                destination_leaf,
                &source_directory,
                &destination_lexical.public_label,
                "rebuildable publication committed destination",
            )?;
            if let Some((_stat, previous_directory)) = &previous {
                validate_named_directory_identity(
                    &boundary.directory,
                    source_leaf,
                    previous_directory,
                    &source_lexical.public_label,
                    "rebuildable publication exchanged previous artifact",
                )?;
            } else if leaf_metadata(
                &boundary.directory,
                source_leaf,
                &source_lexical.public_label,
            )?
            .is_some()
            {
                return Err(capability_error(
                    &source_lexical.public_label,
                    "numele staged trebuia să fie absent după publicare",
                ));
            }
            sync_directory(&boundary.directory, &destination_lexical.public_label)
        })();
        match postflight {
            Ok(()) => Ok(CapabilityEffect::changed(0)),
            Err(error) => Ok(CapabilityEffect::recovery_required(
                0,
                format!("{error} Commit-ul poate fi deja vizibil; nu repeta publicarea automat."),
            )),
        }
    }

    fn rename_expected_noreplace(
        source: &CapturedParent,
        destination: &CapturedParent,
        expected: &ExpectedLeafVersion,
        source_label: &str,
        destination_label: &str,
    ) -> Result<CapabilityEffect, String> {
        let handle = fs::openat(
            &source.directory,
            &source.leaf,
            OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| {
            capability_error(
                source_label,
                &format!("rename leaf-CAS nu a putut captura sursa: {error}"),
            )
        })?;
        let before = fs::fstat(&handle).map_err(|error| {
            capability_error(
                source_label,
                &format!("rename leaf-CAS nu a putut citi metadata sursei: {error}"),
            )
        })?;
        let observed_token = version_token_for_stat(&before);
        if observed_token != expected.version_token {
            return Err(capability_error(
                source_label,
                &format!(
                    "rename disk baseline s-a schimbat înainte de commit (expected {}, observed {})",
                    expected.version_token, observed_token
                ),
            ));
        }
        let source_type = FileType::from_raw_mode(before.st_mode);
        let mut content_file = if expected.content_hash.is_some() {
            if source_type != FileType::RegularFile {
                return Err(capability_error(
                    source_label,
                    "rename cu content hash cere o sursă regular file",
                ));
            }
            let descriptor = fs::openat(
                &source.directory,
                &source.leaf,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|error| {
                capability_error(
                    source_label,
                    &format!("rename nu a putut deschide conținutul sursei: {error}"),
                )
            })?;
            let mut file = File::from(descriptor);
            let readable_stat = fs::fstat(&file).map_err(|error| {
                capability_error(
                    source_label,
                    &format!("rename nu a putut verifica descriptorul sursei: {error}"),
                )
            })?;
            if !same_file_identity(&before, &readable_stat) {
                return Err(capability_error(
                    source_label,
                    "rename a observat alt inode între handle și descriptorul de conținut",
                ));
            }
            validate_expected_content(
                &mut file,
                &readable_stat,
                expected.content_hash.as_deref(),
                source_label,
                "rename pre-commit",
            )?;
            Some(file)
        } else {
            None
        };
        let captured_directory = if let Some(expected_tree) = expected.tree_fingerprint.as_deref() {
            if source_type != FileType::Directory {
                return Err(capability_error(
                    source_label,
                    "rename cu tree fingerprint cere o sursă director",
                ));
            }
            let directory =
                open_directory_strict(&source.directory, &source.leaf).map_err(|error| {
                    capability_error(
                        source_label,
                        &format!("directorul lifecycle nu poate fi capturat: {error}"),
                    )
                })?;
            validate_open_directory_identity(
                &directory,
                &before,
                source_label,
                "rename tree source",
            )?;
            let observed_tree = fingerprint_directory_tree(&directory, source_label)?;
            if observed_tree != expected_tree {
                return Err(capability_error(
                    source_label,
                    &format!(
                        "descendenții sursei s-au schimbat înainte de rename (expected {expected_tree}, observed {observed_tree})"
                    ),
                ));
            }
            Some(directory)
        } else {
            if source_type == FileType::Directory {
                return Err(capability_error(
                    source_label,
                    "rename RequireDiskBaseline pentru director cere tree fingerprint",
                ));
            }
            None
        };

        run_test_hook(CapabilityTestStage::BeforeRename);
        fs::renameat_with(
            &source.directory,
            &source.leaf,
            &destination.directory,
            &destination.leaf,
            RenameFlags::NOREPLACE,
        )
        .map_err(|error| {
            capability_error(
                source_label,
                &format!(
                    "rename leaf-CAS către {destination_label} a fost refuzat fără suprascriere: {error}"
                ),
            )
        })?;

        let moved = match fs::statat(
            &destination.directory,
            &destination.leaf,
            AtFlags::SYMLINK_NOFOLLOW,
        ) {
            Ok(stat) => stat,
            Err(error) => {
                return Ok(CapabilityEffect::recovery_required(
                    0,
                    capability_error(
                        source_label,
                        &format!(
                            "rename-ul este vizibil, dar destinația nu poate fi verificată: {error}; recovery necesar și fără retry automat"
                        ),
                    ),
                ));
            }
        };
        let validation = (|| {
            if !same_file_identity(&before, &moved) {
                return Err(capability_error(
                    source_label,
                    "rename a mutat alt inode decât sursa capturată",
                ));
            }
            let after = fs::fstat(&handle).map_err(|error| {
                capability_error(
                    source_label,
                    &format!("sursa mutată nu mai poate fi verificată: {error}"),
                )
            })?;
            if !same_stable_leaf_version(&before, &after) {
                return Err(capability_error(
                    source_label,
                    "sursa s-a modificat în timpul rename-ului condițional",
                ));
            }
            if let Some(file) = content_file.as_mut() {
                validate_expected_content(
                    file,
                    &after,
                    expected.content_hash.as_deref(),
                    source_label,
                    "rename post-commit",
                )?;
            }
            if let (Some(directory), Some(expected_tree)) = (
                captured_directory.as_ref(),
                expected.tree_fingerprint.as_deref(),
            ) {
                let observed_tree = fingerprint_directory_tree(directory, source_label)?;
                if observed_tree != expected_tree {
                    return Err(capability_error(
                        source_label,
                        &format!(
                            "descendenții sursei s-au schimbat în timpul rename-ului (expected {expected_tree}, observed {observed_tree})"
                        ),
                    ));
                }
            }
            let after_validation = fs::fstat(&handle).map_err(|error| {
                capability_error(
                    source_label,
                    &format!("sursa nu mai poate fi reverificată după postflight: {error}"),
                )
            })?;
            if version_token_for_stat(&after) != version_token_for_stat(&after_validation) {
                return Err(capability_error(
                    source_label,
                    "sursa a suferit o schimbare ABA în timpul postflight-ului rename",
                ));
            }
            Ok(())
        })();
        if let Err(conflict) = validation {
            return rollback_conditional_rename(
                source,
                destination,
                &moved,
                source_label,
                conflict,
            );
        }

        let mut diagnostics = Vec::new();
        if let Err(error) = sync_directory(&source.directory, source_label) {
            diagnostics.push(error);
        }
        if let Err(error) = sync_directory(&destination.directory, destination_label) {
            diagnostics.push(error);
        }
        if diagnostics.is_empty() {
            Ok(CapabilityEffect::changed(0))
        } else {
            Ok(CapabilityEffect::recovery_required(
                0,
                format!(
                    "Rename-ul leaf-CAS este vizibil, dar sincronizarea directoarelor a eșuat: {} Nu repeta operația automat.",
                    diagnostics.join(" ")
                ),
            ))
        }
    }

    fn rollback_conditional_rename(
        source: &CapturedParent,
        destination: &CapturedParent,
        moved_identity: &fs::Stat,
        source_label: &str,
        conflict: String,
    ) -> Result<CapabilityEffect, String> {
        if let Err(error) = fs::renameat_with(
            &destination.directory,
            &destination.leaf,
            &source.directory,
            &source.leaf,
            RenameFlags::NOREPLACE,
        ) {
            return Ok(CapabilityEffect::recovery_required(
                0,
                capability_error(
                    source_label,
                    &format!(
                        "{conflict} Rollback-ul rename a eșuat: {error}; destinația cere recovery și operația nu trebuie repetată automat"
                    ),
                ),
            ));
        }
        let restored = fs::statat(&source.directory, &source.leaf, AtFlags::SYMLINK_NOFOLLOW);
        if !matches!(restored, Ok(ref stat) if same_file_identity(stat, moved_identity)) {
            return Ok(CapabilityEffect::recovery_required(
                0,
                capability_error(
                    source_label,
                    &format!(
                        "{conflict} Rollback-ul rename nu poate demonstra identitatea restaurată; recovery necesar"
                    ),
                ),
            ));
        }
        let mut diagnostics = Vec::new();
        if let Err(error) = sync_directory(&source.directory, source_label) {
            diagnostics.push(error);
        }
        if let Err(error) = sync_directory(&destination.directory, source_label) {
            diagnostics.push(error);
        }
        if diagnostics.is_empty() {
            Err(format!(
                "{conflict} Rename-ul leaf-CAS a fost anulat, iar sursa concurentă a fost restaurată."
            ))
        } else {
            Ok(CapabilityEffect::recovery_required(
                0,
                format!(
                    "{conflict} Sursa a fost restaurată după conflict, dar rollback-ul nu este confirmat durabil: {} Nu repeta operația automat.",
                    diagnostics.join(" ")
                ),
            ))
        }
    }

    fn atomic_commit<F>(
        parent: &OwnedFd,
        leaf: &OsStr,
        replace_policy: CapabilityReplacePolicy,
        expected_leaf: &ExpectedLeaf,
        public_label: &str,
        writer: F,
    ) -> Result<CapabilityEffect, String>
    where
        F: FnOnce(&mut File) -> Result<u64, String>,
    {
        let (temp_name, descriptor) = create_unique_temp(parent)?;
        let mut file = File::from(descriptor);
        let write_result = writer(&mut file).and_then(|bytes_written| {
            file.sync_all().map_err(|error| {
                format!("Fișierul temporar nu a putut fi sincronizat: {error}.")
            })?;
            Ok(bytes_written)
        });

        let bytes_written = match write_result {
            Ok(bytes_written) => bytes_written,
            Err(error) => {
                let diagnostic = cleanup_temp_after_error(parent, &temp_name, error);
                drop(file);
                return Err(diagnostic);
            }
        };
        let temp_identity = fs::fstat(&file).map_err(|error| {
            cleanup_temp_after_error(
                parent,
                &temp_name,
                format!("Identitatea descriptorului temporar nu a putut fi citită: {error}."),
            )
        })?;

        run_test_hook(CapabilityTestStage::BeforeAtomicCommit);
        if let Err(error) =
            validate_named_file_identity(parent, &temp_name, &temp_identity, "atomic-temp")
        {
            let diagnostic = cleanup_temp_after_error(parent, &temp_name, error);
            drop(file);
            return Err(diagnostic);
        }
        if let ExpectedLeaf::Present(expected) = expected_leaf {
            return conditional_atomic_replace(
                parent,
                leaf,
                &temp_name,
                &mut file,
                &temp_identity,
                expected,
                bytes_written,
                public_label,
            );
        }
        if replace_policy == CapabilityReplacePolicy::Replace {
            match leaf_metadata(parent, leaf, public_label)? {
                Some(stat) if FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile => {
                    return unconditional_atomic_replace(
                        parent,
                        leaf,
                        &temp_name,
                        &mut file,
                        &temp_identity,
                        bytes_written,
                        public_label,
                    );
                }
                Some(_) => {
                    return Err(cleanup_temp_after_error(
                        parent,
                        &temp_name,
                        capability_error(
                            public_label,
                            "target-ul Replace s-a schimbat într-un leaf non-regular înainte de commit",
                        ),
                    ));
                }
                None => {}
            }
        }
        let commit_result =
            fs::renameat_with(parent, &temp_name, parent, leaf, RenameFlags::NOREPLACE);
        if let Err(error) = commit_result {
            let diagnostic = cleanup_temp_after_error(
                parent,
                &temp_name,
                format!("Commit-ul atomic fd-relative a eșuat: {error}."),
            );
            drop(file);
            return Err(diagnostic);
        }
        if let Err(error) =
            validate_named_file_identity(parent, leaf, &temp_identity, "atomic-leaf")
        {
            drop(file);
            return Ok(CapabilityEffect::recovery_required(
                bytes_written,
                format!(
                    "Commit-ul atomic a avut loc, dar leaf-ul a fost înlocuit imediat după rename: {error} Nu repeta operația automat."
                ),
            ));
        }
        drop(file);
        match sync_directory(parent, "atomic-commit") {
            Ok(()) => Ok(CapabilityEffect::changed(bytes_written)),
            Err(error) => Ok(CapabilityEffect::recovery_required(
                bytes_written,
                format!(
                    "Commit-ul atomic este vizibil, dar folderul nu a putut fi sincronizat: {error}. Nu repeta operația automat."
                ),
            )),
        }
    }

    fn unconditional_atomic_replace(
        parent: &OwnedFd,
        leaf: &OsStr,
        temp_name: &OsStr,
        _temp_file: &mut File,
        temp_identity: &fs::Stat,
        bytes_written: u64,
        public_label: &str,
    ) -> Result<CapabilityEffect, String> {
        if let Err(error) =
            fs::renameat_with(parent, temp_name, parent, leaf, RenameFlags::EXCHANGE)
        {
            return Err(cleanup_temp_after_error(
                parent,
                temp_name,
                capability_error(
                    public_label,
                    &format!("atomic Replace exchange a eșuat fără commit: {error}"),
                ),
            ));
        }
        run_test_hook(CapabilityTestStage::AfterAtomicExchange);
        if let Err(error) =
            validate_named_file_identity(parent, leaf, temp_identity, "atomic-replace-leaf")
        {
            return Ok(CapabilityEffect::recovery_required(
                bytes_written,
                format!(
                    "Atomic Replace a făcut exchange, dar target-ul nu mai este temp-ul sincronizat: {error} Leaf-ul anterior este păstrat în {}; nu repeta operația automat.",
                    temp_name.to_string_lossy()
                ),
            ));
        }
        let previous = match fs::statat(parent, temp_name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat) => stat,
            Err(error) => {
                return Ok(CapabilityEffect::recovery_required(
                    bytes_written,
                    capability_error(
                        public_label,
                        &format!(
                            "atomic Replace este vizibil, dar leaf-ul anterior {} nu poate fi verificat: {error}; recovery necesar",
                            temp_name.to_string_lossy()
                        ),
                    ),
                ));
            }
        };
        if FileType::from_raw_mode(previous.st_mode) != FileType::RegularFile {
            return Ok(CapabilityEffect::recovery_required(
                bytes_written,
                capability_error(
                    public_label,
                    &format!(
                        "atomic Replace este vizibil, dar leaf-ul anterior {} nu este regular și a fost păstrat pentru recovery",
                        temp_name.to_string_lossy()
                    ),
                ),
            ));
        }
        if let Err(error) =
            validate_named_file_identity(parent, temp_name, &previous, "atomic-replace-old")
        {
            return Ok(CapabilityEffect::recovery_required(
                bytes_written,
                format!(
                    "{error} Atomic Replace păstrează leaf-ul anterior pentru recovery; nu repeta operația automat."
                ),
            ));
        }
        if let Err(error) = fs::unlinkat(parent, temp_name, AtFlags::empty()) {
            return Ok(CapabilityEffect::recovery_required(
                bytes_written,
                capability_error(
                    public_label,
                    &format!(
                        "atomic Replace este vizibil, dar cleanup-ul {} a eșuat: {error}; recovery necesar",
                        temp_name.to_string_lossy()
                    ),
                ),
            ));
        }
        match sync_directory(parent, public_label) {
            Ok(()) => Ok(CapabilityEffect::changed(bytes_written)),
            Err(error) => Ok(CapabilityEffect::recovery_required(
                bytes_written,
                format!(
                    "{error} Atomic Replace este vizibil, dar durabilitatea este incertă; nu repeta operația automat."
                ),
            )),
        }
    }

    fn conditional_atomic_replace(
        parent: &OwnedFd,
        leaf: &OsStr,
        temp_name: &OsStr,
        _temp_file: &mut File,
        temp_identity: &fs::Stat,
        expected: &ExpectedLeafVersion,
        bytes_written: u64,
        public_label: &str,
    ) -> Result<CapabilityEffect, String> {
        let descriptor = fs::openat(
            parent,
            leaf,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| {
            cleanup_temp_after_error(
                parent,
                temp_name,
                capability_error(
                    public_label,
                    &format!("leaf-CAS replace nu a putut captura versiunea așteptată: {error}"),
                ),
            )
        })?;
        let mut previous_file = File::from(descriptor);
        let previous_before = fs::fstat(&previous_file).map_err(|error| {
            cleanup_temp_after_error(
                parent,
                temp_name,
                capability_error(
                    public_label,
                    &format!("leaf-CAS replace nu a putut citi metadata: {error}"),
                ),
            )
        })?;
        validate_expected_regular_file(
            &mut previous_file,
            &previous_before,
            expected,
            public_label,
            "replace pre-commit",
        )
        .map_err(|error| cleanup_temp_after_error(parent, temp_name, error))?;

        run_test_hook(CapabilityTestStage::AfterExpectedLeafCaptured);
        if let Err(error) =
            validate_named_file_identity(parent, temp_name, temp_identity, "atomic-temp-cas")
        {
            return Err(cleanup_temp_after_error(parent, temp_name, error));
        }

        if let Err(error) =
            fs::renameat_with(parent, temp_name, parent, leaf, RenameFlags::EXCHANGE)
        {
            return Err(cleanup_temp_after_error(
                parent,
                temp_name,
                capability_error(
                    public_label,
                    &format!("leaf-CAS exchange a eșuat fără commit: {error}"),
                ),
            ));
        }
        run_test_hook(CapabilityTestStage::AfterAtomicExchange);

        let moved_previous = match fs::statat(parent, temp_name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat) => stat,
            Err(error) => {
                return Ok(CapabilityEffect::recovery_required(
                    bytes_written,
                    capability_error(
                        public_label,
                        &format!(
                            "leaf-ul vechi mutat sub {} nu poate fi verificat: {error}; recovery necesar și fără retry automat",
                            temp_name.to_string_lossy()
                        ),
                    ),
                ));
            }
        };
        let validation = (|| {
            if FileType::from_raw_mode(moved_previous.st_mode) != FileType::RegularFile
                || !same_file_identity(&previous_before, &moved_previous)
            {
                return Err(capability_error(
                    public_label,
                    "leaf-ul de la commit nu este inode-ul capturat de disk baseline",
                ));
            }
            let previous_after = fs::fstat(&previous_file).map_err(|error| {
                capability_error(
                    public_label,
                    &format!("versiunea veche izolată nu mai poate fi verificată: {error}"),
                )
            })?;
            if !same_stable_leaf_version(&previous_before, &previous_after) {
                return Err(capability_error(
                    public_label,
                    "leaf-ul vechi s-a modificat în timpul commit-ului condițional",
                ));
            }
            validate_expected_content(
                &mut previous_file,
                &previous_after,
                expected.content_hash.as_deref(),
                public_label,
                "replace post-exchange",
            )?;
            let previous_final = fs::fstat(&previous_file).map_err(|error| {
                capability_error(
                    public_label,
                    &format!("versiunea veche nu poate fi reverificată după hash: {error}"),
                )
            })?;
            if version_token_for_stat(&previous_after) != version_token_for_stat(&previous_final) {
                return Err(capability_error(
                    public_label,
                    "leaf-ul vechi a suferit o schimbare ABA în timpul postflight-ului replace",
                ));
            }
            Ok(())
        })();

        if let Err(conflict) = validation {
            return rollback_atomic_exchange(
                parent,
                leaf,
                temp_name,
                temp_identity,
                &moved_previous,
                bytes_written,
                public_label,
                conflict,
            );
        }

        if let Err(error) =
            validate_named_file_identity(parent, leaf, temp_identity, "cas-committed-leaf")
        {
            return Ok(CapabilityEffect::recovery_required(
                bytes_written,
                format!(
                    "Commit-ul leaf-CAS a făcut exchange, dar target-ul nu mai este inode-ul temporar sincronizat: {error} Versiunea veche este păstrată în {}; nu repeta operația automat.",
                    temp_name.to_string_lossy()
                ),
            ));
        }

        if let Err(error) =
            validate_named_file_identity(parent, temp_name, &previous_before, "cas-old-leaf")
        {
            return Ok(CapabilityEffect::recovery_required(
                bytes_written,
                format!(
                    "{error} Noul conținut este deja la target, dar versiunea veche izolată cere recovery; nu repeta operația automat."
                ),
            ));
        }
        if let Err(error) = fs::unlinkat(parent, temp_name, AtFlags::empty()) {
            return Ok(CapabilityEffect::recovery_required(
                bytes_written,
                capability_error(
                    public_label,
                    &format!(
                        "commit-ul leaf-CAS este vizibil, dar versiunea veche izolată în {} nu a putut fi eliminată: {error}; nu repeta operația automat",
                        temp_name.to_string_lossy()
                    ),
                ),
            ));
        }
        match sync_directory(parent, public_label) {
            Ok(()) => Ok(CapabilityEffect::changed(bytes_written)),
            Err(error) => Ok(CapabilityEffect::recovery_required(
                bytes_written,
                format!(
                    "{error} Commit-ul leaf-CAS este vizibil, dar durabilitatea directorului este incertă; nu repeta operația automat."
                ),
            )),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn rollback_atomic_exchange(
        parent: &OwnedFd,
        leaf: &OsStr,
        temp_name: &OsStr,
        temp_identity: &fs::Stat,
        restored_identity: &fs::Stat,
        bytes_written: u64,
        public_label: &str,
        conflict: String,
    ) -> Result<CapabilityEffect, String> {
        if let Err(error) =
            fs::renameat_with(parent, temp_name, parent, leaf, RenameFlags::EXCHANGE)
        {
            return Ok(CapabilityEffect::recovery_required(
                bytes_written,
                capability_error(
                    public_label,
                    &format!(
                        "{conflict} Rollback-ul exchange a eșuat: {error}. Target-ul și {} cer recovery; nu repeta operația automat",
                        temp_name.to_string_lossy()
                    ),
                ),
            ));
        }

        let restored = fs::statat(parent, leaf, AtFlags::SYMLINK_NOFOLLOW);
        let temp = fs::statat(parent, temp_name, AtFlags::SYMLINK_NOFOLLOW);
        if !matches!(restored, Ok(ref stat) if same_file_identity(stat, restored_identity))
            || !matches!(temp, Ok(ref stat) if same_file_identity(stat, temp_identity))
        {
            return Ok(CapabilityEffect::recovery_required(
                bytes_written,
                capability_error(
                    public_label,
                    &format!(
                        "{conflict} Rollback-ul exchange nu a putut demonstra restaurarea identităților; {} cere recovery și operația nu trebuie repetată automat",
                        temp_name.to_string_lossy()
                    ),
                ),
            ));
        }
        if let Err(error) = fs::unlinkat(parent, temp_name, AtFlags::empty()) {
            return Ok(CapabilityEffect::recovery_required(
                bytes_written,
                capability_error(
                    public_label,
                    &format!(
                        "{conflict} Leaf-ul anterior a fost restaurat, dar temp-ul rollback {} nu a putut fi eliminat: {error}; recovery necesar",
                        temp_name.to_string_lossy()
                    ),
                ),
            ));
        }
        if let Err(error) = sync_directory(parent, public_label) {
            return Ok(CapabilityEffect::recovery_required(
                bytes_written,
                format!(
                    "{conflict} Leaf-ul anterior a fost restaurat, dar rollback-ul nu este confirmat durabil: {error} Nu repeta operația automat."
                ),
            ));
        }
        Err(format!(
            "{conflict} Operația leaf-CAS a fost anulată, iar versiunea concurentă a fost restaurată."
        ))
    }

    fn validate_expected_regular_file(
        file: &mut File,
        stat: &fs::Stat,
        expected: &ExpectedLeafVersion,
        public_label: &str,
        stage: &str,
    ) -> Result<(), String> {
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
            return Err(capability_error(
                public_label,
                &format!("{stage}: expected leaf nu este fișier regular"),
            ));
        }
        let observed_token = version_token_for_stat(stat);
        if observed_token != expected.version_token {
            return Err(capability_error(
                public_label,
                &format!(
                    "{stage}: disk baseline s-a schimbat înainte de commit (expected {}, observed {})",
                    expected.version_token, observed_token
                ),
            ));
        }
        validate_expected_content(
            file,
            stat,
            expected.content_hash.as_deref(),
            public_label,
            stage,
        )
    }

    fn validate_expected_content(
        file: &mut File,
        stat: &fs::Stat,
        expected_hash: Option<&str>,
        public_label: &str,
        stage: &str,
    ) -> Result<(), String> {
        let Some(expected_hash) = expected_hash else {
            return Ok(());
        };
        let expected_size = u64::try_from(stat.st_size).map_err(|_| {
            capability_error(public_label, &format!("{stage}: dimensiune negativă"))
        })?;
        const MAX_CONDITIONAL_HASH_BYTES: u64 = 512 * 1024 * 1024;
        if expected_size > MAX_CONDITIONAL_HASH_BYTES {
            return Err(capability_error(
                public_label,
                &format!(
                    "{stage}: verificarea hash depășește limita de {MAX_CONDITIONAL_HASH_BYTES} bytes"
                ),
            ));
        }
        file.seek(SeekFrom::Start(0)).map_err(|error| {
            capability_error(
                public_label,
                &format!("{stage}: descriptorul nu poate reveni la început: {error}"),
            )
        })?;
        let mut bytes = Vec::with_capacity(expected_size as usize);
        (&mut *file)
            .take(expected_size.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| {
                capability_error(
                    public_label,
                    &format!("{stage}: conținutul nu poate fi citit bounded: {error}"),
                )
            })?;
        file.seek(SeekFrom::Start(0)).map_err(|error| {
            capability_error(
                public_label,
                &format!("{stage}: descriptorul nu poate fi resetat: {error}"),
            )
        })?;
        let observed_hash = hash_bytes(&bytes);
        if bytes.len() as u64 != expected_size || observed_hash != expected_hash {
            return Err(capability_error(
                public_label,
                &format!(
                    "{stage}: conținutul disk s-a schimbat (expected hash {expected_hash}, observed {observed_hash})"
                ),
            ));
        }
        Ok(())
    }

    fn version_token_for_stat(stat: &fs::Stat) -> String {
        format!(
            "unix:{}:{}:{}:{}:{}:{}:{}:{}",
            stat.st_dev,
            stat.st_ino,
            stat.st_size,
            stat.st_mtime,
            stat.st_mtime_nsec,
            stat.st_ctime,
            stat.st_ctime_nsec,
            stat.st_mode,
        )
    }

    fn same_stable_leaf_version(before: &fs::Stat, after: &fs::Stat) -> bool {
        same_file_identity(before, after)
            && before.st_size == after.st_size
            && before.st_mtime == after.st_mtime
            && before.st_mtime_nsec == after.st_mtime_nsec
            && before.st_mode == after.st_mode
            && before.st_nlink == after.st_nlink
    }

    fn create_unique_temp(parent: &OwnedFd) -> Result<(OsString, OwnedFd), String> {
        for _ in 0..32 {
            let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let name = OsString::from(format!(
                ".pana-capability-{}-{sequence}.tmp",
                std::process::id()
            ));
            match fs::openat(
                parent,
                &name,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                FILE_MODE,
            ) {
                Ok(descriptor) => return Ok((name, descriptor)),
                Err(Errno::EXIST) => continue,
                Err(error) => {
                    return Err(format!(
                        "Fișierul temporar fd-relative nu a putut fi creat: {error}."
                    ));
                }
            }
        }
        Err("Nu a putut fi rezervat un nume temporar fd-relative unic.".to_string())
    }

    fn cleanup_temp_after_error(parent: &OwnedFd, temp_name: &OsStr, original: String) -> String {
        match fs::unlinkat(parent, temp_name, AtFlags::empty()) {
            Ok(()) | Err(Errno::NOENT) => original,
            Err(cleanup_error) => format!(
                "{original} Curățarea fișierului temporar a eșuat fail-closed: {cleanup_error}."
            ),
        }
    }

    fn validate_named_file_identity(
        parent: &OwnedFd,
        name: &OsStr,
        expected: &fs::Stat,
        role: &str,
    ) -> Result<(), String> {
        let observed = fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|error| format!("{role} nu mai poate fi verificat: {error}."))?;
        if FileType::from_raw_mode(observed.st_mode) != FileType::RegularFile
            || !same_file_identity(expected, &observed)
        {
            return Err(format!(
                "{role} nu mai numește inode-ul temporar sincronizat."
            ));
        }
        Ok(())
    }

    fn validate_open_directory_identity(
        directory: &OwnedFd,
        expected: &fs::Stat,
        public_label: &str,
        role: &str,
    ) -> Result<(), String> {
        let observed = fs::fstat(directory).map_err(|error| {
            capability_error(
                public_label,
                &format!("{role} nu și-a putut citi identitatea: {error}"),
            )
        })?;
        if !same_file_identity(expected, &observed) {
            return Err(capability_error(
                public_label,
                &format!("{role} a fost înlocuit concurent înainte de capturare"),
            ));
        }
        Ok(())
    }

    fn validate_named_directory_identity(
        parent: &OwnedFd,
        name: &OsStr,
        captured: &OwnedFd,
        public_label: &str,
        role: &str,
    ) -> Result<(), String> {
        let named = fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
            capability_error(
                public_label,
                &format!("{role} nu mai poate fi verificat înainte de rmdir: {error}"),
            )
        })?;
        let opened = fs::fstat(captured).map_err(|error| {
            capability_error(
                public_label,
                &format!("{role} capturat nu mai poate fi verificat: {error}"),
            )
        })?;
        if FileType::from_raw_mode(named.st_mode) != FileType::Directory
            || !same_file_identity(&named, &opened)
        {
            return Err(capability_error(
                public_label,
                &format!("{role} a fost înlocuit concurent înainte de rmdir"),
            ));
        }
        Ok(())
    }

    fn same_file_identity(left: &fs::Stat, right: &fs::Stat) -> bool {
        left.st_dev == right.st_dev && left.st_ino == right.st_ino
    }

    fn validate_atomic_destination(
        parent: &OwnedFd,
        leaf: &OsStr,
        replace_policy: CapabilityReplacePolicy,
        lexical: &LexicalTarget,
    ) -> Result<(), String> {
        let Some(metadata) = leaf_metadata(parent, leaf, &lexical.public_label)? else {
            return Ok(());
        };
        if replace_policy == CapabilityReplacePolicy::CreateNew {
            return Err(capability_error(
                &lexical.public_label,
                "target-ul create-only există deja",
            ));
        }
        match FileType::from_raw_mode(metadata.st_mode) {
            FileType::Symlink => Err(capability_error(
                &lexical.public_label,
                "target-ul atomic este symlink",
            )),
            FileType::Directory => Err(capability_error(
                &lexical.public_label,
                "target-ul atomic este director",
            )),
            FileType::RegularFile => Ok(()),
            _ => Err(capability_error(
                &lexical.public_label,
                "target-ul atomic nu este fișier regular",
            )),
        }
    }

    fn validate_regular_single_link(
        descriptor: &OwnedFd,
        public_label: &str,
        operation: &str,
    ) -> Result<(), String> {
        let metadata = fs::fstat(descriptor).map_err(|error| {
            capability_error(
                public_label,
                &format!("{operation} nu a putut verifica descriptorul: {error}"),
            )
        })?;
        if FileType::from_raw_mode(metadata.st_mode) != FileType::RegularFile {
            return Err(capability_error(
                public_label,
                &format!("{operation} cere un fișier regular"),
            ));
        }
        if metadata.st_nlink > 1 {
            return Err(capability_error(
                public_label,
                &format!("{operation} refuză un inode cu mai multe hardlink-uri"),
            ));
        }
        Ok(())
    }

    fn leaf_metadata(
        parent: &OwnedFd,
        leaf: &OsStr,
        public_label: &str,
    ) -> Result<Option<fs::Stat>, String> {
        match fs::statat(parent, leaf, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(metadata) => Ok(Some(metadata)),
            Err(Errno::NOENT) => Ok(None),
            Err(error) => Err(capability_error(
                public_label,
                &format!("leaf-ul nu a putut fi verificat fd-relative: {error}"),
            )),
        }
    }

    fn capture_target_parent(
        lexical: &LexicalTarget,
        create_missing: bool,
    ) -> Result<Option<CapturedParent>, CaptureFailure> {
        let Some(boundary) = capture_boundary(lexical, create_missing)? else {
            return Ok(None);
        };
        capture_target_parent_from_directory(
            lexical,
            boundary.directory,
            create_missing,
            boundary.created,
        )
    }

    fn capture_existing_target_parent(
        lexical: &LexicalTarget,
    ) -> Result<Option<CapturedParent>, String> {
        capture_target_parent(lexical, false).map_err(CaptureFailure::into_diagnostic)
    }

    fn capture_existing_target_parent_from_directory(
        lexical: &LexicalTarget,
        directory: OwnedFd,
    ) -> Result<Option<CapturedParent>, String> {
        capture_target_parent_from_directory(lexical, directory, false, false)
            .map_err(CaptureFailure::into_diagnostic)
    }

    fn capture_existing_boundary(
        lexical: &LexicalTarget,
    ) -> Result<Option<CapturedBoundary>, String> {
        capture_boundary(lexical, false).map_err(CaptureFailure::into_diagnostic)
    }

    fn settle_after_implicit_parent_creation(
        created_ancestors: bool,
        result: Result<CapabilityEffect, String>,
        public_label: &str,
    ) -> Result<CapabilityEffect, String> {
        if !created_ancestors {
            return result;
        }
        match result {
            Ok(effect) if effect.recovery_required => Ok(effect),
            Ok(effect) if effect.changed => Ok(effect),
            Ok(effect) => Ok(CapabilityEffect::recovery_required(
                effect.bytes_written,
                capability_error(
                    public_label,
                    "operația leaf a fost Noop, dar namespace-ul părinte a fost creat",
                ),
            )),
            Err(error) => Ok(CapabilityEffect::recovery_required(
                0,
                format!(
                    "{error} Namespace-ul părinte a fost creat durabil înaintea refuzului; nu repeta operația automat."
                ),
            )),
        }
    }

    fn capture_target_parent_from_directory(
        lexical: &LexicalTarget,
        mut directory: OwnedFd,
        create_missing: bool,
        mut created_ancestors: bool,
    ) -> Result<Option<CapturedParent>, CaptureFailure> {
        let (leaf, parents) = lexical.relative_components.split_last().ok_or_else(|| {
            CaptureFailure::no_effect(capability_error(
                &lexical.public_label,
                "operația cere un leaf sub boundary",
            ))
        })?;

        for component in parents {
            match open_directory_strict(&directory, component) {
                Ok(next) => directory = next,
                Err(Errno::NOENT) if create_missing => {
                    match open_or_create_directory_component(
                        &directory,
                        component,
                        &lexical.public_label,
                    ) {
                        Ok((next, created)) => {
                            created_ancestors |= created;
                            directory = next;
                        }
                        Err(error) => {
                            return Err(if created_ancestors {
                                error.promote()
                            } else {
                                error
                            });
                        }
                    }
                }
                Err(Errno::NOENT) => return Ok(None),
                Err(error) => {
                    let diagnostic = capability_error(
                        &lexical.public_label,
                        &format!("un părinte nu a putut fi capturat fără symlink: {error}"),
                    );
                    return Err(if created_ancestors {
                        CaptureFailure::after_effect(diagnostic)
                    } else {
                        CaptureFailure::no_effect(diagnostic)
                    });
                }
            }
        }

        Ok(Some(CapturedParent {
            directory,
            leaf: leaf.clone(),
            created_ancestors,
        }))
    }

    fn capture_boundary(
        lexical: &LexicalTarget,
        create_missing: bool,
    ) -> Result<Option<CapturedBoundary>, CaptureFailure> {
        if let Some(authority) = lexical.authority.as_ref() {
            if create_missing && !authority.root_path().exists() {
                return Err(CaptureFailure::no_effect(capability_error(
                    &lexical.public_label,
                    "authority root ținut nu mai are pathname; root-ul nu poate fi recreat implicit",
                )));
            }
            verify_directory_authority_path(authority).map_err(CaptureFailure::no_effect)?;
            run_test_hook(CapabilityTestStage::AfterAuthorityPathVerified);
            let directory = rustix::io::dup(authority.directory()).map_err(|error| {
                CaptureFailure::no_effect(capability_error(
                    &lexical.public_label,
                    &format!("authority handle nu a putut fi duplicat: {error}"),
                ))
            })?;
            let observed = identity_from_fd(&directory, &lexical.public_label)
                .map_err(CaptureFailure::no_effect)?;
            if observed != authority.identity() {
                return Err(CaptureFailure::no_effect(capability_error(
                    &lexical.public_label,
                    "authority handle nu mai corespunde identității instalate",
                )));
            }
            return Ok(Some(CapturedBoundary {
                directory,
                created: false,
            }));
        }
        capture_boundary_from_path(lexical, create_missing)
    }

    fn capture_boundary_from_path(
        lexical: &LexicalTarget,
        create_missing: bool,
    ) -> Result<Option<CapturedBoundary>, CaptureFailure> {
        let mut current =
            open_filesystem_root(&lexical.public_label).map_err(CaptureFailure::no_effect)?;
        let mut created = false;

        for component in &lexical.boundary_components {
            match open_directory_strict(&current, component) {
                Ok(next) => current = next,
                Err(Errno::NOENT) if create_missing => {
                    match open_or_create_directory_component(
                        &current,
                        component,
                        &lexical.public_label,
                    ) {
                        Ok((next, component_created)) => {
                            created |= component_created;
                            current = next;
                        }
                        Err(error) => {
                            return Err(if created { error.promote() } else { error });
                        }
                    }
                }
                Err(Errno::NOENT) => return Ok(None),
                Err(error) => {
                    let diagnostic = capability_error(
                        &lexical.public_label,
                        &format!("boundary-ul nu a putut fi capturat fără symlink: {error}"),
                    );
                    return Err(if created {
                        CaptureFailure::after_effect(diagnostic)
                    } else {
                        CaptureFailure::no_effect(diagnostic)
                    });
                }
            }
        }

        Ok(Some(CapturedBoundary {
            directory: current,
            created,
        }))
    }

    fn open_filesystem_root(public_label: &str) -> Result<OwnedFd, String> {
        fs::openat(
            fs::CWD,
            Path::new("/"),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| {
            capability_error(
                public_label,
                &format!("rădăcina filesystem nu a putut fi capturată: {error}"),
            )
        })
    }

    pub(super) fn read_bounded_regular_file_from_authority(
        authority: &DirectoryAuthority,
        path: &Path,
        public_label: &str,
        max_bytes: u64,
    ) -> Result<Option<CapabilityBoundedFileSnapshot>, String> {
        verify_directory_authority_path(authority)?;
        let target = WriteTarget::new(path, authority.root_path(), public_label)
            .bind_authority(authority.clone())?;
        let lexical = lexical_target(&target, false)?;
        let Some(parent) = capture_existing_target_parent(&lexical)? else {
            return Ok(None);
        };

        let descriptor = match open_regular_file_strict(&parent.directory, &parent.leaf) {
            Ok(descriptor) => descriptor,
            Err(Errno::NOENT) => return Ok(None),
            Err(error) => {
                return Err(capability_error(
                    public_label,
                    &format!(
                        "bounded read nu poate deschide leaf-ul fd-relative fără symlink: {error}"
                    ),
                ));
            }
        };
        let before = fs::fstat(&descriptor).map_err(|error| {
            capability_error(
                public_label,
                &format!("bounded read nu poate verifica descriptorul: {error}"),
            )
        })?;
        if FileType::from_raw_mode(before.st_mode) != FileType::RegularFile {
            return Err(capability_error(
                public_label,
                "bounded read cere un fișier regular",
            ));
        }
        if before.st_nlink != 1 {
            return Err(capability_error(
                public_label,
                "bounded read refuză un inode cu mai multe hardlink-uri",
            ));
        }
        let expected_size = u64::try_from(before.st_size).map_err(|_| {
            capability_error(
                public_label,
                "bounded read a observat o dimensiune negativă",
            )
        })?;
        if expected_size > max_bytes {
            return Err(capability_error(
                public_label,
                &format!(
                    "fișierul are {expected_size} bytes și depășește limita de {max_bytes} bytes"
                ),
            ));
        }
        let capacity = usize::try_from(expected_size).map_err(|_| {
            capability_error(
                public_label,
                "dimensiunea fișierului nu încape în memoria adresabilă",
            )
        })?;

        run_test_hook(CapabilityTestStage::AfterBoundedReadLeafOpened);
        let mut file = File::from(descriptor);
        let mut bytes = Vec::with_capacity(capacity);
        std::io::Read::by_ref(&mut file)
            .take(max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| {
                capability_error(
                    public_label,
                    &format!("fișierul nu poate fi citit bounded: {error}"),
                )
            })?;
        if bytes.len() as u64 != expected_size {
            return Err(capability_error(
                public_label,
                "fișierul și-a schimbat dimensiunea în timpul citirii bounded",
            ));
        }
        let after = fs::fstat(&file).map_err(|error| {
            capability_error(
                public_label,
                &format!("bounded read nu poate face fstat postflight: {error}"),
            )
        })?;
        if version_token_for_stat(&after) != version_token_for_stat(&before)
            || after.st_nlink != before.st_nlink
        {
            return Err(capability_error(
                public_label,
                "fișierul s-a schimbat în timpul citirii bounded",
            ));
        }

        let boundary = rustix::io::dup(authority.directory()).map_err(|error| {
            capability_error(
                public_label,
                &format!("authority root nu poate fi duplicat la postflight: {error}"),
            )
        })?;
        let Some(recaptured_parent) =
            capture_existing_target_parent_from_directory(&lexical, boundary)?
        else {
            return Err(capability_error(
                public_label,
                "parentul numit a dispărut în timpul citirii bounded",
            ));
        };
        let captured_parent_stat = fs::fstat(&parent.directory).map_err(|error| {
            capability_error(
                public_label,
                &format!("parentul capturat nu poate fi verificat la postflight: {error}"),
            )
        })?;
        let recaptured_parent_stat = fs::fstat(&recaptured_parent.directory).map_err(|error| {
            capability_error(
                public_label,
                &format!("parentul recapturat nu poate fi verificat la postflight: {error}"),
            )
        })?;
        if !same_file_identity(&captured_parent_stat, &recaptured_parent_stat) {
            return Err(capability_error(
                public_label,
                "path-ul nu mai numește parentul capturat în timpul citirii bounded",
            ));
        }
        let named = fs::statat(
            &recaptured_parent.directory,
            &recaptured_parent.leaf,
            AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(|error| {
            capability_error(
                public_label,
                &format!("leaf-ul nu poate fi verificat la postflight: {error}"),
            )
        })?;
        if FileType::from_raw_mode(named.st_mode) != FileType::RegularFile
            || named.st_nlink != 1
            || version_token_for_stat(&named) != version_token_for_stat(&after)
        {
            return Err(capability_error(
                public_label,
                "numele leaf nu mai indică versiunea fișierului citit",
            ));
        }
        verify_directory_authority_path(authority)?;

        Ok(Some(CapabilityBoundedFileSnapshot {
            bytes,
            version_token: version_token_for_stat(&after),
        }))
    }

    fn open_regular_file_strict(parent: &OwnedFd, leaf: &OsStr) -> Result<OwnedFd, Errno> {
        let open_flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
        let resolve_flags =
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS;
        for _ in 0..MAX_OPENAT2_RACE_RETRIES {
            match fs::openat2(parent, leaf, open_flags, Mode::empty(), resolve_flags) {
                Ok(descriptor) => return Ok(descriptor),
                Err(Errno::AGAIN) => continue,
                Err(Errno::NOSYS) => {
                    return fs::openat(parent, leaf, open_flags, Mode::empty());
                }
                Err(error) => return Err(error),
            }
        }
        Err(Errno::AGAIN)
    }

    pub(super) fn open_optional_regular_file_readonly_no_follow(
        path: &Path,
        public_label: &str,
    ) -> Result<Option<File>, String> {
        let components = absolute_normal_components(path, public_label, "read-only source")?;
        let (leaf, parents) = components.split_last().ok_or_else(|| {
            capability_error(public_label, "read-only source trebuie să aibă un leaf")
        })?;
        let mut directory = open_filesystem_root(public_label)?;
        for parent in parents {
            directory = match open_directory_strict(&directory, parent) {
                Ok(directory) => directory,
                Err(Errno::NOENT) => return Ok(None),
                Err(error) => {
                    return Err(capability_error(
                        public_label,
                        &format!(
                            "un părinte al read-only source nu poate fi deschis fără symlink: {error}"
                        ),
                    ));
                }
            };
        }
        let descriptor = match fs::openat(
            &directory,
            leaf,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(descriptor) => descriptor,
            Err(Errno::NOENT) => return Ok(None),
            Err(error) => {
                return Err(capability_error(
                    public_label,
                    &format!("read-only source nu poate fi deschis fără symlink: {error}"),
                ));
            }
        };
        let stat = fs::fstat(&descriptor).map_err(|error| {
            capability_error(
                public_label,
                &format!("read-only source nu poate fi verificat: {error}"),
            )
        })?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
            return Err(capability_error(
                public_label,
                "read-only source nu este fișier regular",
            ));
        }
        Ok(Some(File::from(descriptor)))
    }

    pub(super) fn open_regular_file_readonly_no_follow(
        path: &Path,
        public_label: &str,
    ) -> Result<File, String> {
        open_optional_regular_file_readonly_no_follow(path, public_label)?
            .ok_or_else(|| capability_error(public_label, "read-only source nu există"))
    }

    fn open_or_create_directory_component(
        parent: &OwnedFd,
        component: &OsStr,
        public_label: &str,
    ) -> Result<(OwnedFd, bool), CaptureFailure> {
        match open_directory_strict(parent, component) {
            Ok(directory) => return Ok((directory, false)),
            Err(Errno::NOENT) => {}
            Err(error) => {
                return Err(CaptureFailure::no_effect(capability_error(
                    public_label,
                    &format!("directorul existent nu poate fi deschis sigur: {error}"),
                )));
            }
        }

        let created = match fs::mkdirat(parent, component, DIRECTORY_MODE) {
            Ok(()) => true,
            Err(Errno::EXIST) => false,
            Err(error) => {
                return Err(CaptureFailure::no_effect(capability_error(
                    public_label,
                    &format!("directorul nu a putut fi creat fd-relative: {error}"),
                )));
            }
        };
        let directory = open_directory_strict(parent, component).map_err(|error| {
            let diagnostic = capability_error(
                public_label,
                &format!("directorul creat nu a putut fi recapturat sigur: {error}"),
            );
            if created {
                CaptureFailure::after_effect(diagnostic)
            } else {
                CaptureFailure::no_effect(diagnostic)
            }
        })?;
        if created {
            sync_directory(parent, public_label).map_err(CaptureFailure::after_effect)?;
        }
        Ok((directory, created))
    }

    fn open_directory_strict(parent: &OwnedFd, component: &OsStr) -> Result<OwnedFd, Errno> {
        let open_flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let resolve_flags =
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS;

        for _ in 0..MAX_OPENAT2_RACE_RETRIES {
            match fs::openat2(parent, component, open_flags, Mode::empty(), resolve_flags) {
                Ok(directory) => return Ok(directory),
                Err(Errno::AGAIN) => continue,
                // The fallback remains fd-relative and receives exactly one
                // validated normal component. It never falls back to a raw
                // absolute or boundary-relative pathname. NO_XDEV is omitted
                // intentionally because Pană Studio permits mounted project
                // and output directories.
                Err(Errno::NOSYS) => {
                    return fs::openat(parent, component, open_flags, Mode::empty());
                }
                Err(error) => return Err(error),
            }
        }
        Err(Errno::AGAIN)
    }

    fn fingerprint_directory_tree(
        directory: &OwnedFd,
        public_label: &str,
    ) -> Result<String, String> {
        let mut budget = 0_usize;
        let mut records = Vec::new();
        collect_directory_fingerprint_records(
            directory,
            "",
            0,
            &mut budget,
            &mut records,
            public_label,
        )?;
        Ok(tree_fingerprint_from_records(records))
    }

    fn collect_directory_fingerprint_records(
        directory: &OwnedFd,
        relative_prefix: &str,
        depth: usize,
        budget: &mut usize,
        records: &mut Vec<TreeFingerprintRecord>,
        public_label: &str,
    ) -> Result<(), String> {
        if depth > MAX_REMOVE_TREE_DEPTH {
            return Err(capability_error(
                public_label,
                &format!(
                    "fingerprint-ul directorului depășește adâncimea {}",
                    MAX_REMOVE_TREE_DEPTH
                ),
            ));
        }
        let mut stream = Dir::read_from(directory).map_err(|error| {
            capability_error(
                public_label,
                &format!("directorul nu poate fi enumerat pentru fingerprint: {error}"),
            )
        })?;
        let mut names = Vec::new();
        while let Some(entry) = stream.read() {
            let entry = entry.map_err(|error| {
                capability_error(
                    public_label,
                    &format!("enumerarea fingerprint a eșuat: {error}"),
                )
            })?;
            let bytes = entry.file_name().to_bytes();
            if bytes == b"." || bytes == b".." {
                continue;
            }
            let name = std::str::from_utf8(bytes).map_err(|_| {
                capability_error(
                    public_label,
                    "fingerprint-ul lifecycle refuză nume descendant non-UTF-8",
                )
            })?;
            *budget = budget.saturating_add(1);
            if *budget > MAX_REMOVE_TREE_ENTRIES {
                return Err(capability_error(
                    public_label,
                    &format!(
                        "fingerprint-ul directorului depășește {} intrări",
                        MAX_REMOVE_TREE_ENTRIES
                    ),
                ));
            }
            names.push((name.to_string(), OsString::from_vec(bytes.to_vec())));
        }
        drop(stream);
        names.sort_by(|left, right| left.0.cmp(&right.0));

        for (name, os_name) in names {
            let stat =
                fs::statat(directory, &os_name, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
                    capability_error(
                        public_label,
                        &format!("un descendent nu poate fi verificat pentru fingerprint: {error}"),
                    )
                })?;
            let relative_path = if relative_prefix.is_empty() {
                name
            } else {
                format!("{relative_prefix}/{name}")
            };
            let file_type = FileType::from_raw_mode(stat.st_mode);
            let kind = match file_type {
                FileType::Directory => b'd',
                FileType::RegularFile => b'f',
                FileType::Symlink => b'l',
                _ => b'o',
            };
            records.push(TreeFingerprintRecord {
                relative_path: relative_path.clone(),
                kind,
                version_token: version_token_for_stat(&stat),
            });
            if file_type == FileType::Directory {
                let child = open_directory_strict(directory, &os_name).map_err(|error| {
                    capability_error(
                        public_label,
                        &format!("un descendent director nu poate fi capturat: {error}"),
                    )
                })?;
                validate_open_directory_identity(
                    &child,
                    &stat,
                    public_label,
                    "tree fingerprint child",
                )?;
                collect_directory_fingerprint_records(
                    &child,
                    &relative_path,
                    depth + 1,
                    budget,
                    records,
                    public_label,
                )?;
            }
        }
        Ok(())
    }

    fn sync_directory(directory: &OwnedFd, public_label: &str) -> Result<(), String> {
        #[cfg(test)]
        if TEST_FAIL_DIRECTORY_SYNC.with(std::cell::Cell::get) {
            return Err(capability_error(
                public_label,
                "failure injection: fsync director refuzat după efect",
            ));
        }
        fs::fsync(directory).map_err(|error| {
            capability_error(
                public_label,
                &format!("directorul capturat nu a putut fi sincronizat: {error}"),
            )
        })
    }

    fn lexical_target(
        target: &WriteTarget,
        allow_boundary_root: bool,
    ) -> Result<LexicalTarget, String> {
        if target.path.as_os_str().is_empty() || target.boundary_root.as_os_str().is_empty() {
            return Err(capability_error(
                &target.public_label,
                "target-ul și boundary-ul trebuie să fie ne-goale",
            ));
        }
        if !target.path.is_absolute() || !target.boundary_root.is_absolute() {
            return Err(capability_error(
                &target.public_label,
                "target-ul și boundary-ul trebuie să fie absolute",
            ));
        }

        let boundary_components =
            absolute_normal_components(&target.boundary_root, &target.public_label, "boundary")?;
        let relative = target
            .path
            .strip_prefix(&target.boundary_root)
            .map_err(|_| {
                capability_error(
                    &target.public_label,
                    "target-ul nu este descendent lexical al boundary-ului",
                )
            })?;
        let relative_components = relative_normal_components(relative, &target.public_label)?;
        if relative_components.is_empty() && !allow_boundary_root {
            return Err(capability_error(
                &target.public_label,
                "operația nu poate folosi boundary root drept leaf",
            ));
        }

        Ok(LexicalTarget {
            boundary_components,
            relative_components,
            public_label: target.public_label.clone(),
            authority: target.authority().cloned(),
        })
    }

    fn absolute_normal_components(
        path: &Path,
        public_label: &str,
        role: &str,
    ) -> Result<Vec<OsString>, String> {
        let mut saw_root = false;
        let mut components = Vec::new();
        for component in path.components() {
            match component {
                Component::RootDir if !saw_root => saw_root = true,
                Component::Normal(value) if saw_root => components.push(value.to_os_string()),
                _ => {
                    return Err(capability_error(
                        public_label,
                        &format!("{role}-ul conține componente relative sau non-canonice"),
                    ));
                }
            }
        }
        if !saw_root {
            return Err(capability_error(
                public_label,
                &format!("{role}-ul nu are rădăcină absolută"),
            ));
        }
        Ok(components)
    }

    fn relative_normal_components(
        path: &Path,
        public_label: &str,
    ) -> Result<Vec<OsString>, String> {
        path.components()
            .map(|component| match component {
                Component::Normal(value) => Ok(value.to_os_string()),
                _ => Err(capability_error(
                    public_label,
                    "target-ul relativ conține traversal sau componente non-canonice",
                )),
            })
            .collect()
    }

    fn capability_error(public_label: &str, reason: &str) -> String {
        format!("Capability filesystem a blocat {public_label}: {reason}.")
    }

    #[cfg(test)]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum CapabilityTestStage {
        AfterAuthorityPathVerified,
        AfterTargetParentCaptured,
        AfterBoundedReadLeafOpened,
        AfterAppendLeafOpened,
        AfterAppendV2Checkpoint,
        AfterAppendV2WriteBeforePhase,
        AfterAppendV2LinkBeforePhase,
        AfterAppendV2TargetFsync,
        AfterAppendV2TargetDurable,
        AfterAppendV2RecoveryHash,
        AfterDirectoryCreateBeforePhase,
        BeforeDirectoryV2CheckpointCapture,
        AfterDirectoryV2Checkpoint,
        BeforeDirectoryV2NoopFullPath,
        BeforeDirectoryCurrentStateFreshCapture,
        AfterRenameSourceParentCaptured,
        AfterExpectedLeafCaptured,
        AfterExternalBaselineRelocated,
        AfterExternalBackupCommitted,
        AfterExternalPublication,
        AfterAtomicExchange,
        AfterCopyAnonymousStageCheckpoint,
        AfterCopyTemporaryLinkBeforePhase,
        AfterCopyTargetLinkBeforePhase,
        AfterCopyRenameBeforePhase,
        AfterCopyRecoveryHash,
        AfterCopyTargetFsync,
        AfterCopyTargetDurable,
        BeforeExternalTargetDurable,
        BeforeDirectoryTargetDurable,
        BeforeCopyPreviewOverwriteRename,
        BeforeCopyStream,
        BeforeCopyTargetDurable,
        BeforeRemoveLeafQuarantine,
        BeforeRemoveLeafTargetDurable,
        BeforeRemoveLeafUnlink,
        BeforeRemoveTreeQuarantine,
        BeforeRemoveTreeTargetDurable,
        BeforeRemoveTreeTraversal,
        BeforeSymlinkTargetDurable,
        AfterSymlinkCreateBeforePhase,
        AfterSymlinkV2FirstOpenBeforeCapture,
        BeforeSymlinkV2CheckpointCapture,
        AfterSymlinkV2Checkpoint,
        BeforeSymlinkV2NoopFullPath,
        BeforeSymlinkCurrentStateFreshCapture,
        BeforeAtomicCommit,
        BeforeRename,
    }

    #[cfg(test)]
    thread_local! {
        static TEST_HOOK: std::cell::RefCell<Option<Box<dyn Fn(CapabilityTestStage)>>> =
            std::cell::RefCell::new(None);
        static TEST_FAIL_DIRECTORY_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        static TEST_FORCE_EXTERNAL_LINKAT_PROC_FALLBACK: std::cell::Cell<bool> =
            const { std::cell::Cell::new(false) };
        static TEST_FAIL_EXTERNAL_LINKAT: std::cell::Cell<bool> =
            const { std::cell::Cell::new(false) };
        static TEST_APPEND_V2_SHORT_WRITE: std::cell::Cell<Option<usize>> =
            const { std::cell::Cell::new(None) };
    }

    #[cfg(test)]
    fn run_test_hook(stage: CapabilityTestStage) {
        TEST_HOOK.with(|hook| {
            if let Some(hook) = hook.borrow().as_ref() {
                hook(stage);
            }
        });
    }

    #[cfg(not(test))]
    #[allow(dead_code)]
    #[derive(Clone, Copy)]
    enum CapabilityTestStage {
        AfterAuthorityPathVerified,
        AfterTargetParentCaptured,
        AfterBoundedReadLeafOpened,
        AfterAppendLeafOpened,
        AfterAppendV2Checkpoint,
        AfterAppendV2WriteBeforePhase,
        AfterAppendV2LinkBeforePhase,
        AfterAppendV2TargetFsync,
        AfterAppendV2TargetDurable,
        AfterAppendV2RecoveryHash,
        AfterDirectoryCreateBeforePhase,
        BeforeDirectoryV2CheckpointCapture,
        AfterDirectoryV2Checkpoint,
        BeforeDirectoryV2NoopFullPath,
        BeforeDirectoryCurrentStateFreshCapture,
        AfterRenameSourceParentCaptured,
        AfterExpectedLeafCaptured,
        AfterExternalBaselineRelocated,
        AfterExternalBackupCommitted,
        AfterExternalPublication,
        AfterAtomicExchange,
        AfterCopyAnonymousStageCheckpoint,
        AfterCopyTemporaryLinkBeforePhase,
        AfterCopyTargetLinkBeforePhase,
        AfterCopyRenameBeforePhase,
        AfterCopyRecoveryHash,
        AfterCopyTargetFsync,
        AfterCopyTargetDurable,
        BeforeExternalTargetDurable,
        BeforeDirectoryTargetDurable,
        BeforeCopyPreviewOverwriteRename,
        BeforeCopyStream,
        BeforeCopyTargetDurable,
        BeforeRemoveLeafQuarantine,
        BeforeRemoveLeafTargetDurable,
        BeforeRemoveLeafUnlink,
        BeforeRemoveTreeQuarantine,
        BeforeRemoveTreeTargetDurable,
        BeforeRemoveTreeTraversal,
        BeforeSymlinkTargetDurable,
        AfterSymlinkCreateBeforePhase,
        AfterSymlinkV2FirstOpenBeforeCapture,
        BeforeSymlinkV2CheckpointCapture,
        AfterSymlinkV2Checkpoint,
        BeforeSymlinkV2NoopFullPath,
        BeforeSymlinkCurrentStateFreshCapture,
        BeforeAtomicCommit,
        BeforeRename,
    }

    #[cfg(not(test))]
    fn run_test_hook(_stage: CapabilityTestStage) {}

    #[cfg(test)]
    fn force_external_linkat_proc_fallback() -> bool {
        TEST_FORCE_EXTERNAL_LINKAT_PROC_FALLBACK.with(std::cell::Cell::get)
    }

    #[cfg(not(test))]
    fn force_external_linkat_proc_fallback() -> bool {
        false
    }

    #[cfg(test)]
    fn fail_external_linkat() -> bool {
        TEST_FAIL_EXTERNAL_LINKAT.with(std::cell::Cell::get)
    }

    #[cfg(not(test))]
    fn fail_external_linkat() -> bool {
        false
    }

    #[cfg(test)]
    fn with_test_hook<T>(
        hook: impl Fn(CapabilityTestStage) + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        struct ResetHook;
        impl Drop for ResetHook {
            fn drop(&mut self) {
                TEST_HOOK.with(|slot| {
                    *slot.borrow_mut() = None;
                });
            }
        }

        TEST_HOOK.with(|slot| {
            let previous = slot.borrow_mut().replace(Box::new(hook));
            assert!(previous.is_none(), "capability test hook already installed");
        });
        let _reset = ResetHook;
        operation()
    }

    #[cfg(test)]
    fn with_append_v2_stage_hook_for_test<T>(
        expected: CapabilityTestStage,
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == expected {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_after_bounded_read_leaf_opened_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::AfterBoundedReadLeafOpened {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_append_v2_short_write_for_test<T>(
        bytes: usize,
        operation: impl FnOnce() -> T,
    ) -> T {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                TEST_APPEND_V2_SHORT_WRITE.with(|slot| slot.set(None));
            }
        }
        TEST_APPEND_V2_SHORT_WRITE.with(|slot| {
            assert!(slot.replace(Some(bytes)).is_none());
        });
        let _reset = Reset;
        operation()
    }

    #[cfg(test)]
    fn append_v2_short_write_limit() -> Option<usize> {
        TEST_APPEND_V2_SHORT_WRITE.with(std::cell::Cell::get)
    }

    #[cfg(not(test))]
    fn append_v2_short_write_limit() -> Option<usize> {
        None
    }

    #[cfg(test)]
    macro_rules! append_v2_stage_hook {
        ($name:ident, $stage:ident) => {
            pub(super) fn $name<T>(hook: impl Fn() + 'static, operation: impl FnOnce() -> T) -> T {
                with_append_v2_stage_hook_for_test(CapabilityTestStage::$stage, hook, operation)
            }
        };
    }

    #[cfg(test)]
    append_v2_stage_hook!(
        with_after_append_v2_checkpoint_hook_for_test,
        AfterAppendV2Checkpoint
    );
    #[cfg(test)]
    append_v2_stage_hook!(
        with_after_append_v2_write_before_phase_hook_for_test,
        AfterAppendV2WriteBeforePhase
    );
    #[cfg(test)]
    append_v2_stage_hook!(
        with_after_append_v2_link_before_phase_hook_for_test,
        AfterAppendV2LinkBeforePhase
    );
    #[cfg(test)]
    append_v2_stage_hook!(
        with_after_append_v2_target_fsync_hook_for_test,
        AfterAppendV2TargetFsync
    );
    #[cfg(test)]
    append_v2_stage_hook!(
        with_after_append_v2_target_durable_hook_for_test,
        AfterAppendV2TargetDurable
    );
    #[cfg(test)]
    append_v2_stage_hook!(
        with_after_append_v2_recovery_hash_hook_for_test,
        AfterAppendV2RecoveryHash
    );

    #[cfg(test)]
    pub(super) fn with_external_backup_committed_test_hook<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::AfterExternalBackupCommitted {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_external_baseline_relocated_test_hook<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::AfterExternalBaselineRelocated {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_external_post_publication_test_hook<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::AfterExternalPublication {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_before_external_target_durable_test_hook<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::BeforeExternalTargetDurable {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_external_linkat_proc_fallback_test_hook<T>(
        operation: impl FnOnce() -> T,
    ) -> T {
        struct ResetFallback;
        impl Drop for ResetFallback {
            fn drop(&mut self) {
                TEST_FORCE_EXTERNAL_LINKAT_PROC_FALLBACK.with(|flag| flag.set(false));
            }
        }

        TEST_FORCE_EXTERNAL_LINKAT_PROC_FALLBACK.with(|flag| {
            assert!(
                !flag.replace(true),
                "external linkat proc fallback test hook already installed"
            );
        });
        let _reset = ResetFallback;
        operation()
    }

    #[cfg(test)]
    pub(super) fn with_external_linkat_failure_test_hook<T>(operation: impl FnOnce() -> T) -> T {
        struct ResetFailure;
        impl Drop for ResetFailure {
            fn drop(&mut self) {
                TEST_FAIL_EXTERNAL_LINKAT.with(|flag| flag.set(false));
            }
        }

        TEST_FAIL_EXTERNAL_LINKAT.with(|flag| {
            assert!(
                !flag.replace(true),
                "external linkat failure test hook already installed"
            );
        });
        let _reset = ResetFailure;
        operation()
    }

    #[cfg(test)]
    fn with_directory_sync_failure<T>(operation: impl FnOnce() -> T) -> T {
        struct ResetFailure;
        impl Drop for ResetFailure {
            fn drop(&mut self) {
                TEST_FAIL_DIRECTORY_SYNC.with(|flag| flag.set(false));
            }
        }
        TEST_FAIL_DIRECTORY_SYNC.with(|flag| {
            assert!(
                !flag.replace(true),
                "directory sync failure already installed"
            );
        });
        let _reset = ResetFailure;
        operation()
    }

    #[cfg(test)]
    pub(super) fn with_directory_sync_failure_for_test<T>(operation: impl FnOnce() -> T) -> T {
        with_directory_sync_failure(operation)
    }

    #[cfg(test)]
    pub(super) fn with_before_directory_target_durable_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::BeforeDirectoryTargetDurable {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_after_directory_create_before_phase_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::AfterDirectoryCreateBeforePhase {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_before_directory_v2_checkpoint_capture_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::BeforeDirectoryV2CheckpointCapture {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_after_directory_v2_checkpoint_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::AfterDirectoryV2Checkpoint {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_before_directory_v2_noop_full_path_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::BeforeDirectoryV2NoopFullPath {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_before_directory_current_state_fresh_capture_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::BeforeDirectoryCurrentStateFreshCapture {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_before_symlink_target_durable_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::BeforeSymlinkTargetDurable {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_after_symlink_create_before_phase_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::AfterSymlinkCreateBeforePhase {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_after_symlink_v2_first_open_before_capture_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::AfterSymlinkV2FirstOpenBeforeCapture {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_before_symlink_v2_checkpoint_capture_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::BeforeSymlinkV2CheckpointCapture {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_after_symlink_v2_checkpoint_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::AfterSymlinkV2Checkpoint {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_before_symlink_v2_noop_full_path_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::BeforeSymlinkV2NoopFullPath {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_before_symlink_current_state_fresh_capture_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::BeforeSymlinkCurrentStateFreshCapture {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_before_copy_stream_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::BeforeCopyStream {
                    hook();
                }
            },
            operation,
        )
    }

    // Checkpointurile de mai jos sunt infrastructură pentru matricea Copy v2.
    // Ele nu sunt apelate de protocolul v1 și nu modifică producția; Copy v2
    // le va publica exact lângă syscall-ul/faza pe care o denumește fiecare.
    #[cfg(test)]
    pub(super) fn with_after_copy_anonymous_stage_checkpoint_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::AfterCopyAnonymousStageCheckpoint {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_after_copy_temporary_link_before_phase_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::AfterCopyTemporaryLinkBeforePhase {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_after_copy_target_link_before_phase_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::AfterCopyTargetLinkBeforePhase {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_after_copy_rename_before_phase_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::AfterCopyRenameBeforePhase {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_after_copy_target_fsync_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::AfterCopyTargetFsync {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_before_copy_preview_overwrite_rename_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::BeforeCopyPreviewOverwriteRename {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_after_copy_recovery_hash_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::AfterCopyRecoveryHash {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_before_rename_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::BeforeRename {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_before_remove_leaf_quarantine_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::BeforeRemoveLeafQuarantine {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_before_remove_leaf_unlink_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::BeforeRemoveLeafUnlink {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_before_remove_leaf_target_durable_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::BeforeRemoveLeafTargetDurable {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_before_remove_tree_quarantine_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::BeforeRemoveTreeQuarantine {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_before_remove_tree_traversal_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::BeforeRemoveTreeTraversal {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_before_remove_tree_target_durable_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::BeforeRemoveTreeTargetDurable {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_before_copy_target_durable_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::BeforeCopyTargetDurable {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    pub(super) fn with_after_copy_target_durable_hook_for_test<T>(
        hook: impl Fn() + 'static,
        operation: impl FnOnce() -> T,
    ) -> T {
        with_test_hook(
            move |stage| {
                if stage == CapabilityTestStage::AfterCopyTargetDurable {
                    hook();
                }
            },
            operation,
        )
    }

    #[cfg(test)]
    mod tests {
        use std::{
            fs,
            os::unix::fs::symlink,
            path::{Path, PathBuf},
            process::Command,
            sync::{Arc, Barrier},
            thread,
            time::{SystemTime, UNIX_EPOCH},
        };

        use super::*;

        #[test]
        fn sealed_authority_rejects_root_replacement_before_effect() {
            let root = unique_test_dir("sealed-authority-preflight-swap");
            let authority_path = root.join("project");
            let held_path = root.join("project-held");
            let target_path = authority_path.join("document.txt");
            fs::create_dir_all(target_path.parent().unwrap()).unwrap();
            fs::write(&target_path, "original-before").unwrap();

            let authority = capture_directory_authority(
                &authority_path,
                "test/sealed-authority-preflight",
                DirectoryAuthorityScope::ProjectRoot,
            )
            .unwrap();
            let target = WriteTarget::new(
                target_path.clone(),
                authority_path.clone(),
                "test/sealed-authority-preflight/document.txt",
            )
            .bind_authority(authority)
            .unwrap();

            fs::rename(&authority_path, &held_path).unwrap();
            fs::create_dir_all(target_path.parent().unwrap()).unwrap();
            fs::write(&target_path, "replacement-sentinel").unwrap();

            let error = crate::kernel::write_authority::capability::atomic_write(
                &target,
                b"must-not-write",
                CapabilityReplacePolicy::Replace,
            )
            .unwrap_err();

            assert!(error.contains("înlocuit"));
            assert_eq!(
                fs::read_to_string(held_path.join("document.txt")).unwrap(),
                "original-before"
            );
            assert_eq!(
                fs::read_to_string(&target_path).unwrap(),
                "replacement-sentinel"
            );
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn sealed_authority_stays_on_held_inode_during_root_replacement() {
            let root = unique_test_dir("sealed-authority-live-swap");
            let authority_path = root.join("project");
            let held_path = root.join("project-held");
            let replacement_path = root.join("project-replacement");
            let target_path = authority_path.join("document.txt");
            fs::create_dir_all(target_path.parent().unwrap()).unwrap();
            fs::write(&target_path, "original-before").unwrap();
            fs::create_dir_all(replacement_path.to_path_buf()).unwrap();
            fs::write(
                replacement_path.join("document.txt"),
                "replacement-sentinel",
            )
            .unwrap();

            let authority = capture_directory_authority(
                &authority_path,
                "test/sealed-authority-live",
                DirectoryAuthorityScope::ProjectRoot,
            )
            .unwrap();
            let target = WriteTarget::new(
                target_path.clone(),
                authority_path.clone(),
                "test/sealed-authority-live/document.txt",
            )
            .bind_authority(authority)
            .unwrap();

            let hook_authority_path = authority_path.clone();
            let hook_held_path = held_path.clone();
            let hook_replacement_path = replacement_path.clone();
            let effect = with_test_hook(
                move |stage| {
                    if stage == CapabilityTestStage::AfterAuthorityPathVerified {
                        fs::rename(&hook_authority_path, &hook_held_path).unwrap();
                        fs::rename(&hook_replacement_path, &hook_authority_path).unwrap();
                    }
                },
                || {
                    crate::kernel::write_authority::capability::atomic_write(
                        &target,
                        b"original-after",
                        CapabilityReplacePolicy::Replace,
                    )
                },
            )
            .unwrap();

            assert!(effect.changed);
            assert!(effect.recovery_required);
            assert!(effect
                .diagnostic
                .as_deref()
                .is_some_and(|diagnostic| diagnostic.contains("Replacement-ul nu a fost folosit")));
            assert_eq!(
                fs::read_to_string(held_path.join("document.txt")).unwrap(),
                "original-after"
            );
            assert_eq!(
                fs::read_to_string(&target_path).unwrap(),
                "replacement-sentinel"
            );
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn sealed_authority_noop_still_detects_live_root_replacement() {
            let root = unique_test_dir("sealed-authority-noop-swap");
            let authority_path = root.join("project");
            let held_path = root.join("project-held");
            let replacement_path = root.join("project-replacement");
            let target_path = authority_path.join("missing.txt");
            fs::create_dir_all(authority_path.to_path_buf()).unwrap();
            fs::create_dir_all(replacement_path.to_path_buf()).unwrap();
            fs::write(replacement_path.join("missing.txt"), "replacement-sentinel").unwrap();

            let authority = capture_directory_authority(
                &authority_path,
                "test/sealed-authority-noop",
                DirectoryAuthorityScope::ProjectRoot,
            )
            .unwrap();
            let target = WriteTarget::new(
                target_path.clone(),
                authority_path.clone(),
                "test/sealed-authority-noop/missing.txt",
            )
            .bind_authority(authority)
            .unwrap();

            let hook_authority_path = authority_path.clone();
            let hook_held_path = held_path.clone();
            let hook_replacement_path = replacement_path.clone();
            let effect = with_test_hook(
                move |stage| {
                    if stage == CapabilityTestStage::AfterAuthorityPathVerified {
                        fs::rename(&hook_authority_path, &hook_held_path).unwrap();
                        fs::rename(&hook_replacement_path, &hook_authority_path).unwrap();
                    }
                },
                || {
                    crate::kernel::write_authority::capability::remove_file_if_exists_maintenance(
                        &target,
                    )
                },
            )
            .unwrap();

            assert!(!effect.changed);
            assert!(effect.recovery_required);
            assert!(held_path.to_path_buf().is_dir());
            assert_eq!(
                fs::read_to_string(&target_path).unwrap(),
                "replacement-sentinel"
            );
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn subprocess_directory_lease_survives_path_replacement() {
            let root = unique_test_dir("subprocess-directory-lease");
            let original = root.join("project");
            let moved = root.join("project-held");
            fs::create_dir_all(&original).unwrap();

            let lease = capture_directory_lease(&original, "test/subprocess-directory").unwrap();
            lease.require_empty().unwrap();
            fs::rename(&original, &moved).unwrap();
            fs::create_dir_all(&original).unwrap();

            let status = Command::new("/bin/sh")
                .arg("-c")
                .arg("printf original > child-marker.txt")
                .current_dir(lease.current_dir_path())
                .status()
                .unwrap();

            assert!(status.success());
            assert_eq!(
                fs::read_to_string(moved.join("child-marker.txt")).unwrap(),
                "original"
            );
            assert!(!original.join("child-marker.txt").exists());
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn parent_creation_sync_failure_is_recovery_not_rejection() {
            let root = unique_test_dir("parent-creation-recovery");
            fs::create_dir_all(&root).unwrap();
            let boundary = root.join("session/new-project");
            let target_path = boundary.join("recovery.json");
            let target = WriteTarget::new(&target_path, &boundary, "test/parent-creation-recovery");

            let effect = with_directory_sync_failure(|| {
                atomic_write(&target, b"{}", CapabilityReplacePolicy::CreateNew)
            })
            .expect("a visible parent must return a recovery effect");

            assert!(effect.changed);
            assert!(effect.recovery_required);
            assert!(effect
                .diagnostic
                .as_deref()
                .is_some_and(|diagnostic| diagnostic.contains("namespace")));
            assert!(root.join("session").is_dir());
            assert!(!target_path.exists());
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn append_leaf_created_before_validation_failure_is_recovery() {
            let root = unique_test_dir("append-create-recovery");
            fs::create_dir_all(&root).unwrap();
            let target_path = root.join("transactions.jsonl");
            let alias_path = root.join("transactions-alias.jsonl");
            let target = WriteTarget::new(&target_path, &root, "test/append-create-recovery");
            let hook_target = target_path.clone();
            let hook_alias = alias_path.clone();

            let effect = with_test_hook(
                move |stage| {
                    if stage == CapabilityTestStage::AfterAppendLeafOpened {
                        fs::hard_link(&hook_target, &hook_alias).unwrap();
                    }
                },
                || append(&target, b"record\n"),
            )
            .expect("a newly visible append leaf must return a recovery effect");

            assert!(effect.changed);
            assert!(effect.recovery_required);
            assert_eq!(fs::metadata(&target_path).unwrap().len(), 0);
            assert_eq!(fs::metadata(&alias_path).unwrap().len(), 0);
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn atomic_write_remains_anchored_after_ancestor_swap() {
            let root = unique_test_dir("capability-ancestor-swap");
            let safe = root.join("safe");
            let held = root.join("safe-held");
            let outside = root.join("outside");
            let boundary = safe.join("boundary");
            let target_path = boundary.join("nested/document.txt");
            let outside_target = outside.join("boundary/nested/document.txt");
            fs::create_dir_all(target_path.parent().unwrap()).unwrap();
            fs::create_dir_all(outside_target.parent().unwrap()).unwrap();
            fs::write(&target_path, "inside-before").unwrap();
            fs::write(&outside_target, "outside-sentinel").unwrap();

            let entered = Arc::new(Barrier::new(2));
            let swapped = Arc::new(Barrier::new(2));
            let attacker_entered = Arc::clone(&entered);
            let attacker_swapped = Arc::clone(&swapped);
            let attacker_root = root.clone();
            let attacker = thread::spawn(move || {
                attacker_entered.wait();
                fs::rename(attacker_root.join("safe"), attacker_root.join("safe-held")).unwrap();
                symlink(attacker_root.join("outside"), attacker_root.join("safe")).unwrap();
                attacker_swapped.wait();
            });

            let operation_entered = Arc::clone(&entered);
            let operation_swapped = Arc::clone(&swapped);
            let target = WriteTarget::new(&target_path, &boundary, "test/ancestor-swap");
            let effect = with_test_hook(
                move |stage| {
                    if stage == CapabilityTestStage::AfterTargetParentCaptured {
                        operation_entered.wait();
                        operation_swapped.wait();
                    }
                },
                || atomic_write(&target, b"inside-after", CapabilityReplacePolicy::Replace),
            )
            .unwrap();
            attacker.join().unwrap();

            assert!(effect.changed);
            assert_eq!(
                fs::read_to_string(&outside_target).unwrap(),
                "outside-sentinel"
            );
            assert_eq!(
                fs::read_to_string(held.join("boundary/nested/document.txt")).unwrap(),
                "inside-after"
            );
            assert_no_temp_files(&held.join("boundary/nested"));

            fs::remove_file(root.join("safe")).unwrap();
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn atomic_create_new_cleans_temp_when_collision_arrives_before_commit() {
            let root = unique_test_dir("capability-atomic-collision");
            let boundary = root.join("boundary");
            let target_path = boundary.join("document.txt");
            fs::create_dir_all(&boundary).unwrap();

            let entered = Arc::new(Barrier::new(2));
            let collided = Arc::new(Barrier::new(2));
            let attacker_entered = Arc::clone(&entered);
            let attacker_collided = Arc::clone(&collided);
            let attacker_target = target_path.clone();
            let attacker = thread::spawn(move || {
                attacker_entered.wait();
                fs::write(attacker_target, "competitor").unwrap();
                attacker_collided.wait();
            });

            let operation_entered = Arc::clone(&entered);
            let operation_collided = Arc::clone(&collided);
            let target = WriteTarget::new(&target_path, &boundary, "test/atomic-collision");
            let error = with_test_hook(
                move |stage| {
                    if stage == CapabilityTestStage::BeforeAtomicCommit {
                        operation_entered.wait();
                        operation_collided.wait();
                    }
                },
                || atomic_write(&target, b"ours", CapabilityReplacePolicy::CreateNew),
            )
            .unwrap_err();
            attacker.join().unwrap();

            assert!(error.contains("Commit-ul atomic"));
            assert_eq!(fs::read_to_string(&target_path).unwrap(), "competitor");
            assert_no_temp_files(&boundary);
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn atomic_post_commit_sync_failure_returns_terminal_recovery_effect() {
            let root = unique_test_dir("capability-atomic-recovery");
            let boundary = root.join("boundary");
            let target_path = boundary.join("document.txt");
            fs::create_dir_all(&boundary).unwrap();
            fs::write(&target_path, "before").unwrap();
            let target = WriteTarget::new(&target_path, &boundary, "test/atomic-recovery");

            let effect = with_directory_sync_failure(|| {
                atomic_write(&target, b"after", CapabilityReplacePolicy::Replace)
            })
            .unwrap();

            assert!(effect.changed);
            assert!(effect.recovery_required);
            assert!(effect
                .diagnostic
                .as_deref()
                .is_some_and(|value| value.to_lowercase().contains("nu repeta")));
            assert_eq!(fs::read_to_string(&target_path).unwrap(), "after");
            assert_no_temp_files(&boundary);
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn atomic_replace_preserves_old_leaf_when_target_is_substituted_after_exchange() {
            let root = unique_test_dir("capability-atomic-replace-post-exchange");
            let boundary = root.join("boundary");
            let target_path = boundary.join("document.txt");
            let displaced_new = boundary.join("our-displaced-new.txt");
            fs::create_dir_all(&boundary).unwrap();
            fs::write(&target_path, "previous").unwrap();
            let target = WriteTarget::new(&target_path, &boundary, "test/atomic-replace-race");
            let racing_target = target_path.clone();
            let racing_displaced = displaced_new.clone();

            let effect = with_test_hook(
                move |stage| {
                    if stage == CapabilityTestStage::AfterAtomicExchange {
                        fs::rename(&racing_target, &racing_displaced).unwrap();
                        fs::write(&racing_target, "competitor").unwrap();
                    }
                },
                || atomic_write(&target, b"ours", CapabilityReplacePolicy::Replace),
            )
            .unwrap();

            assert!(effect.recovery_required);
            assert_eq!(fs::read_to_string(&target_path).unwrap(), "competitor");
            assert_eq!(fs::read_to_string(&displaced_new).unwrap(), "ours");
            let preserved_old = fs::read_dir(&boundary)
                .unwrap()
                .filter_map(Result::ok)
                .find(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
                .expect("previous leaf should remain for recovery")
                .path();
            assert_eq!(fs::read_to_string(preserved_old).unwrap(), "previous");
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn atomic_commit_rejects_substituted_temp_inode_without_touching_target() {
            let root = unique_test_dir("capability-temp-substitution");
            let boundary = root.join("boundary");
            let target_path = boundary.join("document.txt");
            let outside = root.join("outside.txt");
            fs::create_dir_all(&boundary).unwrap();
            fs::write(&target_path, "original").unwrap();
            fs::write(&outside, "outside-sentinel").unwrap();

            let entered = Arc::new(Barrier::new(2));
            let substituted = Arc::new(Barrier::new(2));
            let attacker_entered = Arc::clone(&entered);
            let attacker_substituted = Arc::clone(&substituted);
            let attacker_boundary = boundary.clone();
            let attacker_outside = outside.clone();
            let attacker = thread::spawn(move || {
                attacker_entered.wait();
                let temp = fs::read_dir(&attacker_boundary)
                    .unwrap()
                    .filter_map(Result::ok)
                    .find(|entry| {
                        entry
                            .file_name()
                            .to_string_lossy()
                            .starts_with(".pana-capability-")
                    })
                    .expect("atomic temp should be visible")
                    .path();
                fs::remove_file(&temp).unwrap();
                fs::hard_link(attacker_outside, temp).unwrap();
                attacker_substituted.wait();
            });

            let operation_entered = Arc::clone(&entered);
            let operation_substituted = Arc::clone(&substituted);
            let target = WriteTarget::new(&target_path, &boundary, "test/temp-substitution");
            let error = with_test_hook(
                move |stage| {
                    if stage == CapabilityTestStage::BeforeAtomicCommit {
                        operation_entered.wait();
                        operation_substituted.wait();
                    }
                },
                || atomic_write(&target, b"ours", CapabilityReplacePolicy::Replace),
            )
            .unwrap_err();
            attacker.join().unwrap();

            assert!(error.contains("inode-ul temporar"));
            assert_eq!(fs::read_to_string(&target_path).unwrap(), "original");
            assert_eq!(fs::read_to_string(&outside).unwrap(), "outside-sentinel");
            assert_no_temp_files(&boundary);
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn conditional_atomic_replace_restores_racing_leaf_without_overwrite() {
            let root = unique_test_dir("capability-leaf-cas-replace");
            let boundary = root.join("boundary");
            let target_path = boundary.join("document.txt");
            let displaced = boundary.join("captured-original.txt");
            fs::create_dir_all(&boundary).unwrap();
            fs::write(&target_path, "expected-original").unwrap();
            let version = crate::project::project_disk_metadata_version_token(
                &fs::metadata(&target_path).unwrap(),
            );
            let target = WriteTarget::new(&target_path, &boundary, "test/leaf-cas-replace")
                .with_expected_present(version, Some(hash_bytes(b"expected-original")));
            let racing_target = target_path.clone();
            let racing_displaced = displaced.clone();

            let error = with_test_hook(
                move |stage| {
                    if stage == CapabilityTestStage::AfterExpectedLeafCaptured {
                        fs::rename(&racing_target, &racing_displaced).unwrap();
                        fs::write(&racing_target, "competitor").unwrap();
                    }
                },
                || atomic_write(&target, b"ours", CapabilityReplacePolicy::Replace),
            )
            .unwrap_err();

            assert!(error.contains("restaurată"));
            assert_eq!(fs::read_to_string(&target_path).unwrap(), "competitor");
            assert_eq!(fs::read_to_string(&displaced).unwrap(), "expected-original");
            assert_no_temp_files(&boundary);
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn conditional_atomic_replace_preserves_old_leaf_when_target_is_substituted_after_exchange()
        {
            let root = unique_test_dir("capability-leaf-cas-post-exchange");
            let boundary = root.join("boundary");
            let target_path = boundary.join("document.txt");
            let displaced_new = boundary.join("our-displaced-new.txt");
            fs::create_dir_all(&boundary).unwrap();
            fs::write(&target_path, "expected-original").unwrap();
            let version = crate::project::project_disk_metadata_version_token(
                &fs::metadata(&target_path).unwrap(),
            );
            let target = WriteTarget::new(&target_path, &boundary, "test/leaf-cas-post-exchange")
                .with_expected_present(version, Some(hash_bytes(b"expected-original")));
            let racing_target = target_path.clone();
            let racing_displaced = displaced_new.clone();

            let effect = with_test_hook(
                move |stage| {
                    if stage == CapabilityTestStage::AfterAtomicExchange {
                        fs::rename(&racing_target, &racing_displaced).unwrap();
                        fs::write(&racing_target, "competitor").unwrap();
                    }
                },
                || atomic_write(&target, b"ours", CapabilityReplacePolicy::Replace),
            )
            .unwrap();

            assert!(effect.recovery_required);
            assert_eq!(fs::read_to_string(&target_path).unwrap(), "competitor");
            assert_eq!(fs::read_to_string(&displaced_new).unwrap(), "ours");
            let preserved_old = fs::read_dir(&boundary)
                .unwrap()
                .filter_map(Result::ok)
                .find(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
                .expect("old leaf should remain in temp for recovery")
                .path();
            assert_eq!(
                fs::read_to_string(preserved_old).unwrap(),
                "expected-original"
            );
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn conditional_remove_restores_racing_leaf_without_deletion() {
            let root = unique_test_dir("capability-leaf-cas-remove");
            let boundary = root.join("boundary");
            let target_path = boundary.join("document.txt");
            let displaced = boundary.join("captured-original.txt");
            fs::create_dir_all(&boundary).unwrap();
            fs::write(&target_path, "expected-original").unwrap();
            let version = crate::project::project_disk_metadata_version_token(
                &fs::metadata(&target_path).unwrap(),
            );
            let target = WriteTarget::new(&target_path, &boundary, "test/leaf-cas-remove")
                .with_expected_present(version, Some(hash_bytes(b"expected-original")));
            let racing_target = target_path.clone();
            let racing_displaced = displaced.clone();

            let error = with_test_hook(
                move |stage| {
                    if stage == CapabilityTestStage::AfterExpectedLeafCaptured {
                        fs::rename(&racing_target, &racing_displaced).unwrap();
                        fs::write(&racing_target, "competitor").unwrap();
                    }
                },
                || remove_file_if_exists(&target),
            )
            .unwrap_err();

            assert!(error.contains("restaurată"));
            assert_eq!(fs::read_to_string(&target_path).unwrap(), "competitor");
            assert_eq!(fs::read_to_string(&displaced).unwrap(), "expected-original");
            assert_no_temp_files(&boundary);
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn rename_noreplace_preserves_racing_destination_and_source() {
            let root = unique_test_dir("capability-rename-collision");
            let source_boundary = root.join("source");
            let destination_boundary = root.join("destination");
            let source_path = source_boundary.join("entry.txt");
            let destination_path = destination_boundary.join("entry.txt");
            fs::create_dir_all(&source_boundary).unwrap();
            fs::create_dir_all(&destination_boundary).unwrap();
            fs::write(&source_path, "source").unwrap();

            let entered = Arc::new(Barrier::new(2));
            let collided = Arc::new(Barrier::new(2));
            let attacker_entered = Arc::clone(&entered);
            let attacker_collided = Arc::clone(&collided);
            let attacker_destination = destination_path.clone();
            let attacker = thread::spawn(move || {
                attacker_entered.wait();
                fs::write(attacker_destination, "competitor").unwrap();
                attacker_collided.wait();
            });

            let operation_entered = Arc::clone(&entered);
            let operation_collided = Arc::clone(&collided);
            let source = WriteTarget::new(&source_path, &source_boundary, "test/rename-source");
            let destination = WriteTarget::new(
                &destination_path,
                &destination_boundary,
                "test/rename-destination",
            );
            let error = with_test_hook(
                move |stage| {
                    if stage == CapabilityTestStage::BeforeRename {
                        operation_entered.wait();
                        operation_collided.wait();
                    }
                },
                || rename_noreplace(&source, &destination),
            )
            .unwrap_err();
            attacker.join().unwrap();

            assert!(error.contains("fără suprascriere"));
            assert_eq!(fs::read_to_string(&source_path).unwrap(), "source");
            assert_eq!(fs::read_to_string(&destination_path).unwrap(), "competitor");
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn conditional_rename_restores_racing_source_and_keeps_destination_absent() {
            let root = unique_test_dir("capability-leaf-cas-rename");
            let boundary = root.join("boundary");
            let source_path = boundary.join("source.txt");
            let destination_path = boundary.join("destination.txt");
            let displaced = boundary.join("captured-original.txt");
            fs::create_dir_all(&boundary).unwrap();
            fs::write(&source_path, "expected-original").unwrap();
            let version = crate::project::project_disk_metadata_version_token(
                &fs::metadata(&source_path).unwrap(),
            );
            let source = WriteTarget::new(&source_path, &boundary, "test/leaf-cas-source")
                .with_expected_present(version, Some(hash_bytes(b"expected-original")));
            let destination =
                WriteTarget::new(&destination_path, &boundary, "test/leaf-cas-destination")
                    .with_expected_absent();
            let racing_source = source_path.clone();
            let racing_displaced = displaced.clone();

            let error = with_test_hook(
                move |stage| {
                    if stage == CapabilityTestStage::BeforeRename {
                        fs::rename(&racing_source, &racing_displaced).unwrap();
                        fs::write(&racing_source, "competitor").unwrap();
                    }
                },
                || rename_noreplace(&source, &destination),
            )
            .unwrap_err();

            assert!(error.contains("restaurată"));
            assert_eq!(fs::read_to_string(&source_path).unwrap(), "competitor");
            assert_eq!(fs::read_to_string(&displaced).unwrap(), "expected-original");
            assert!(!destination_path.exists());
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn conditional_directory_rename_rolls_back_child_edit_after_tree_preflight() {
            let root = unique_test_dir("capability-tree-cas-rename");
            let boundary = root.join("boundary");
            let source_path = boundary.join("source");
            let destination_path = boundary.join("destination");
            let child_path = source_path.join("child.txt");
            fs::create_dir_all(&source_path).unwrap();
            fs::write(&child_path, "accepted").unwrap();
            let source_version = crate::project::project_disk_metadata_version_token(
                &fs::metadata(&source_path).unwrap(),
            );
            let tree_fingerprint = tree_fingerprint_from_records(vec![TreeFingerprintRecord {
                relative_path: "child.txt".to_string(),
                kind: b'f',
                version_token: crate::project::project_disk_metadata_version_token(
                    &fs::metadata(&child_path).unwrap(),
                ),
            }]);
            let source = WriteTarget::new(&source_path, &boundary, "test/tree-cas-source")
                .with_expected_present_tree(source_version, tree_fingerprint);
            let destination =
                WriteTarget::new(&destination_path, &boundary, "test/tree-cas-destination")
                    .with_expected_absent();
            let racing_child = child_path.clone();

            let error = with_test_hook(
                move |stage| {
                    if stage == CapabilityTestStage::BeforeRename {
                        fs::write(&racing_child, "external-after-preflight").unwrap();
                    }
                },
                || rename_noreplace(&source, &destination),
            )
            .unwrap_err();

            assert!(error.contains("descendenții sursei s-au schimbat"));
            assert!(source_path.is_dir());
            assert!(!destination_path.exists());
            assert_eq!(
                fs::read_to_string(child_path).unwrap(),
                "external-after-preflight"
            );
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn same_boundary_rename_uses_one_capture_across_ancestor_swap() {
            let root = unique_test_dir("capability-rename-one-boundary");
            let boundary = root.join("project");
            let held = root.join("project-held");
            let outside = root.join("outside");
            let source_path = boundary.join("source.txt");
            let destination_path = boundary.join("nested/destination.txt");
            fs::create_dir_all(destination_path.parent().unwrap()).unwrap();
            fs::create_dir_all(outside.join("nested")).unwrap();
            fs::write(&source_path, "inside").unwrap();
            fs::write(outside.join("source.txt"), "outside-source").unwrap();
            fs::write(
                outside.join("nested/destination.txt"),
                "outside-destination",
            )
            .unwrap();

            let entered = Arc::new(Barrier::new(2));
            let swapped = Arc::new(Barrier::new(2));
            let attacker_entered = Arc::clone(&entered);
            let attacker_swapped = Arc::clone(&swapped);
            let attacker_boundary = boundary.clone();
            let attacker_held = held.clone();
            let attacker_outside = outside.clone();
            let attacker_root = root.clone();
            let attacker = thread::spawn(move || {
                attacker_entered.wait();
                fs::rename(attacker_boundary, attacker_held).unwrap();
                symlink(attacker_outside, attacker_root.join("project")).unwrap();
                attacker_swapped.wait();
            });

            let operation_entered = Arc::clone(&entered);
            let operation_swapped = Arc::clone(&swapped);
            let source = WriteTarget::new(&source_path, &boundary, "test/rename-source");
            let destination =
                WriteTarget::new(&destination_path, &boundary, "test/rename-destination");
            let effect = with_test_hook(
                move |stage| {
                    if stage == CapabilityTestStage::AfterRenameSourceParentCaptured {
                        operation_entered.wait();
                        operation_swapped.wait();
                    }
                },
                || rename_noreplace(&source, &destination),
            )
            .unwrap();
            attacker.join().unwrap();

            assert!(effect.changed);
            assert!(!held.join("source.txt").exists());
            assert_eq!(
                fs::read_to_string(held.join("nested/destination.txt")).unwrap(),
                "inside"
            );
            assert_eq!(
                fs::read_to_string(outside.join("source.txt")).unwrap(),
                "outside-source"
            );
            assert_eq!(
                fs::read_to_string(outside.join("nested/destination.txt")).unwrap(),
                "outside-destination"
            );

            fs::remove_file(root.join("project")).unwrap();
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn append_rejects_hardlinked_leaf_before_mutation() {
            let root = unique_test_dir("capability-append-hardlink");
            let boundary = root.join("boundary");
            let outside = root.join("outside.txt");
            let target_path = boundary.join("journal.jsonl");
            fs::create_dir_all(&boundary).unwrap();
            fs::write(&outside, "sentinel").unwrap();
            fs::hard_link(&outside, &target_path).unwrap();

            let target = WriteTarget::new(&target_path, &boundary, "test/append-hardlink");
            let error = append(&target, b"mutated").unwrap_err();

            assert!(error.contains("hardlink"));
            assert_eq!(fs::read_to_string(&outside).unwrap(), "sentinel");
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn stable_lock_file_excludes_a_second_writer_across_open_descriptors() {
            let root = unique_test_dir("capability-stable-lock");
            let boundary = root.join("logs");
            let lock_path = boundary.join(".kernel-log.lock");
            fs::create_dir_all(&boundary).unwrap();
            let target = WriteTarget::new(&lock_path, &boundary, "test/stable-lock");

            let first = lock_file(&target, CapabilityLockMode::Exclusive).unwrap();
            let lexical = lexical_target(&target, false).unwrap();
            let parent = capture_existing_target_parent(&lexical)
                .unwrap()
                .expect("lock parent should exist");
            let second_descriptor = rustix::fs::openat(
                &parent.directory,
                &parent.leaf,
                OFlags::RDWR | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .unwrap();
            let error =
                rustix::fs::flock(&second_descriptor, FlockOperation::NonBlockingLockExclusive)
                    .unwrap_err();
            assert_eq!(error, Errno::WOULDBLOCK);

            drop(first);
            rustix::fs::flock(&second_descriptor, FlockOperation::NonBlockingLockExclusive)
                .unwrap();
            drop(second_descriptor);
            fs::remove_dir_all(root).unwrap();
        }

        fn assert_no_temp_files(directory: &Path) {
            let leftovers = fs::read_dir(directory)
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".pana-capability-")
                })
                .count();
            assert_eq!(leftovers, 0, "capability temp files must be cleaned");
        }

        fn unique_test_dir(label: &str) -> PathBuf {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            std::env::temp_dir().join(format!(
                "pana-studio-{label}-{}-{nanos}",
                std::process::id()
            ))
        }
    }
}

#[cfg(target_os = "linux")]
pub(super) struct CapabilityFileLock {
    _inner: platform::CapabilityFileLock,
}

#[cfg(target_os = "linux")]
pub(super) struct CapabilityDirectoryLease {
    inner: platform::CapabilityDirectoryLease,
}

#[cfg(target_os = "linux")]
impl CapabilityDirectoryLease {
    pub(super) fn current_dir_path(&self) -> std::path::PathBuf {
        self.inner.current_dir_path()
    }
}

#[cfg(target_os = "linux")]
pub(super) fn capture_directory_lease_from_authority(
    authority: &DirectoryAuthority,
    path: &Path,
    public_label: &str,
) -> Result<CapabilityDirectoryLease, String> {
    platform::capture_directory_lease_from_authority(authority, path, public_label)
        .map(|inner| CapabilityDirectoryLease { inner })
}

#[cfg(target_os = "linux")]
pub(super) fn open_regular_file_readonly_no_follow(
    path: &Path,
    public_label: &str,
) -> Result<std::fs::File, String> {
    platform::open_regular_file_readonly_no_follow(path, public_label)
}

#[cfg(target_os = "linux")]
pub(super) fn open_optional_regular_file_readonly_no_follow(
    path: &Path,
    public_label: &str,
) -> Result<Option<std::fs::File>, String> {
    platform::open_optional_regular_file_readonly_no_follow(path, public_label)
}

#[cfg(target_os = "linux")]
pub(super) fn read_bounded_regular_file_from_authority(
    authority: &DirectoryAuthority,
    path: &Path,
    public_label: &str,
    max_bytes: u64,
) -> Result<Option<CapabilityBoundedFileSnapshot>, String> {
    platform::read_bounded_regular_file_from_authority(authority, path, public_label, max_bytes)
}

#[cfg(target_os = "linux")]
pub(super) fn capture_directory_authority(
    path: &Path,
    public_label: &str,
    scope: DirectoryAuthorityScope,
) -> Result<DirectoryAuthority, String> {
    platform::capture_directory_authority(path, public_label, scope)
}

#[cfg(target_os = "linux")]
pub(super) fn bootstrap_directory_authority(
    path: &Path,
    public_label: &str,
    scope: DirectoryAuthorityScope,
) -> Result<DirectoryAuthority, String> {
    platform::bootstrap_directory_authority(path, public_label, scope)
}

#[cfg(target_os = "linux")]
pub(super) fn create_directory_from_authority(
    authority: &DirectoryAuthority,
    path: &Path,
    public_label: &str,
) -> Result<(), String> {
    platform::create_directory_from_authority(authority, path, public_label)
}

#[cfg(target_os = "linux")]
pub(super) fn capture_descendant_authority(
    parent: &DirectoryAuthority,
    path: &Path,
    public_label: &str,
    scope: DirectoryAuthorityScope,
) -> Result<DirectoryAuthority, String> {
    platform::capture_descendant_authority(parent, path, public_label, scope)
}

#[cfg(target_os = "linux")]
pub(super) fn verify_directory_authority_path(
    authority: &DirectoryAuthority,
) -> Result<(), String> {
    platform::verify_directory_authority_path(authority)
}

#[cfg(target_os = "linux")]
pub(super) fn lock_file(
    target: &WriteTarget,
    mode: CapabilityLockMode,
) -> Result<CapabilityFileLock, String> {
    platform::lock_file(target, mode).map(|inner| CapabilityFileLock { _inner: inner })
}

#[cfg(target_os = "linux")]
pub(super) fn atomic_write(
    target: &WriteTarget,
    bytes: &[u8],
    replace_policy: CapabilityReplacePolicy,
) -> Result<CapabilityEffect, String> {
    platform::atomic_write(target, bytes, replace_policy)
        .map(|effect| settle_authority_postflight(effect, &[target]))
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

#[cfg(target_os = "linux")]
pub(super) fn plan_atomic_write(
    target: &WriteTarget,
    bytes: &[u8],
    replace_policy: CapabilityReplacePolicy,
    operation_id: &str,
) -> Result<AtomicOperationPlan, String> {
    platform::plan_atomic_write(target, bytes, replace_policy, operation_id)
}

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
pub(super) fn classify_atomic_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    read_budget: &mut super::recovery::RecoveryReadBudget,
) -> Result<super::recovery::AtomicRecoveryAssessment, String> {
    platform::classify_atomic_recovery(record, phase, read_budget)
}

#[cfg(target_os = "linux")]
pub(super) fn execute_atomic_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    read_budget: &mut super::recovery::RecoveryReadBudget,
) -> Result<(), String> {
    platform::execute_atomic_recovery(record, phase, read_budget)
}

#[cfg(target_os = "linux")]
pub(super) fn discard_rebuildable_atomic_projection(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
) -> Result<(), String> {
    platform::discard_rebuildable_atomic_projection(record, phase)
}

#[cfg(target_os = "linux")]
pub(super) fn classify_append_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    checkpoint: Option<&super::recovery::WalAppendStageCheckpoint>,
    read_budget: &mut super::recovery::RecoveryReadBudget,
) -> Result<super::recovery::AppendRecoveryAssessment, String> {
    platform::classify_append_recovery(record, phase, checkpoint, read_budget)
}

#[cfg(target_os = "linux")]
pub(super) fn execute_append_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    checkpoint: Option<&super::recovery::WalAppendStageCheckpoint>,
    read_budget: &mut super::recovery::RecoveryReadBudget,
) -> Result<(), String> {
    platform::execute_append_recovery(record, phase, checkpoint, read_budget)
}

#[cfg(target_os = "linux")]
pub(super) fn classify_directory_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    checkpoint: Option<&super::recovery::WalDirectoryStageCheckpoint>,
) -> Result<super::recovery::DirectoryRecoveryAssessment, String> {
    platform::classify_directory_recovery(record, phase, checkpoint)
}

#[cfg(target_os = "linux")]
pub(super) fn execute_directory_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    checkpoint: Option<&super::recovery::WalDirectoryStageCheckpoint>,
    action: super::recovery::DirectoryRecoveryAction,
) -> Result<(), String> {
    platform::execute_directory_recovery(record, phase, checkpoint, action)
}

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
pub(super) fn classify_symlink_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    checkpoint: Option<&super::recovery::WalSymlinkStageCheckpoint>,
) -> Result<super::recovery::SymlinkRecoveryAssessment, String> {
    platform::classify_symlink_recovery(record, phase, checkpoint)
}

#[cfg(target_os = "linux")]
pub(super) fn execute_symlink_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    checkpoint: Option<&super::recovery::WalSymlinkStageCheckpoint>,
    action: super::recovery::SymlinkRecoveryAction,
) -> Result<(), String> {
    platform::execute_symlink_recovery(record, phase, checkpoint, action)
}

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
pub(super) fn plan_external_config(
    target: &WriteTarget,
    bytes: &[u8],
    backup: Option<(&WriteTarget, &[u8])>,
    operation_id: &str,
) -> Result<ExternalConfigOperationPlan, String> {
    platform::plan_external_config(target, bytes, backup, operation_id)
}

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
pub(super) fn classify_external_config_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    checkpoint: Option<&super::recovery::WalExternalStageCheckpoint>,
    decision: Option<super::recovery::WalExternalOperatorDecision>,
    read_budget: &mut super::recovery::RecoveryReadBudget,
) -> Result<super::recovery::ExternalConfigRecoveryAssessment, String> {
    platform::classify_external_config_recovery(record, phase, checkpoint, decision, read_budget)
}

#[cfg(target_os = "linux")]
pub(super) fn execute_external_config_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    checkpoint: Option<&super::recovery::WalExternalStageCheckpoint>,
    decision: Option<super::recovery::WalExternalOperatorDecision>,
    read_budget: &mut super::recovery::RecoveryReadBudget,
) -> Result<(), String> {
    platform::execute_external_config_recovery(record, phase, checkpoint, decision, read_budget)
}

#[cfg(target_os = "linux")]
pub(super) fn append(target: &WriteTarget, bytes: &[u8]) -> Result<CapabilityEffect, String> {
    platform::append(target, bytes).map(|effect| settle_authority_postflight(effect, &[target]))
}

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
pub(super) fn append_wal(
    target: &WriteTarget,
    bytes: &[u8],
    plan: AppendOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    platform::append_wal(target, bytes, plan, guard)
        .map(|effect| settle_authority_postflight(effect, &[target]))
}

#[cfg(target_os = "linux")]
pub(super) fn plan_copy(
    target: &WriteTarget,
    source: &Path,
    replace_policy: CapabilityReplacePolicy,
    operation_id: &str,
) -> Result<CopyOperationPlan, String> {
    platform::plan_copy(target, source, replace_policy, operation_id)
}

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
pub(super) fn classify_copy_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    checkpoint: Option<&super::recovery::WalCopyStageCheckpoint>,
) -> Result<super::recovery::CopyRecoveryAssessment, String> {
    platform::classify_copy_recovery(record, phase, checkpoint)
}

#[cfg(target_os = "linux")]
pub(super) fn execute_copy_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    checkpoint: Option<&super::recovery::WalCopyStageCheckpoint>,
    read_budget: &mut super::recovery::RecoveryReadBudget,
) -> Result<(), String> {
    platform::execute_copy_recovery(record, phase, checkpoint, read_budget)
}

#[cfg(target_os = "linux")]
pub(super) fn resolve_copy_operator(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    checkpoint: Option<&super::recovery::WalCopyStageCheckpoint>,
    action: super::recovery::WriteAuthorityRecoveryResolutionAction,
) -> Result<String, String> {
    platform::resolve_copy_operator(record, phase, checkpoint, action)
}

#[cfg(target_os = "linux")]
pub(super) fn plan_rename(
    source: &WriteTarget,
    destination: &WriteTarget,
) -> Result<RenameOperationPlan, String> {
    platform::plan_rename(source, destination)
}

#[cfg(target_os = "linux")]
pub(super) fn rename_entry_wal(
    source: &WriteTarget,
    destination: &WriteTarget,
    plan: RenameOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    platform::rename_entry_wal(source, destination, plan, guard)
        .map(|effect| settle_authority_postflight(effect, &[source, destination]))
}

#[cfg(target_os = "linux")]
pub(super) fn classify_rename_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
) -> Result<super::recovery::RenameRecoveryAssessment, String> {
    platform::classify_rename_recovery(record, phase)
}

#[cfg(target_os = "linux")]
pub(super) fn execute_rename_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
) -> Result<(), String> {
    platform::execute_rename_recovery(record, phase)
}

#[cfg(target_os = "linux")]
pub(super) fn plan_remove_leaf(
    target: &WriteTarget,
    operation_id: &str,
) -> Result<Option<RemoveLeafOperationPlan>, String> {
    platform::plan_remove_leaf(target, operation_id)
}

#[cfg(target_os = "linux")]
pub(super) fn remove_leaf_wal(
    target: &WriteTarget,
    plan: RemoveLeafOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    platform::remove_leaf_wal(target, plan, guard)
        .map(|effect| settle_authority_postflight(effect, &[target]))
}

#[cfg(target_os = "linux")]
pub(super) fn classify_remove_leaf_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
) -> Result<super::recovery::RemoveLeafRecoveryAssessment, String> {
    platform::classify_remove_leaf_recovery(record, phase)
}

#[cfg(target_os = "linux")]
pub(super) fn execute_remove_leaf_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
) -> Result<(), String> {
    platform::execute_remove_leaf_recovery(record, phase)
}

#[cfg(target_os = "linux")]
pub(super) fn resolve_remove_leaf_operator(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    action: super::recovery::WriteAuthorityRecoveryResolutionAction,
) -> Result<String, String> {
    platform::resolve_remove_leaf_operator(record, phase, action)
}

#[cfg(target_os = "linux")]
pub(super) fn plan_remove_tree(
    target: &WriteTarget,
    operation_id: &str,
) -> Result<Option<RemoveTreeOperationPlan>, String> {
    platform::plan_remove_tree(target, operation_id)
}

#[cfg(target_os = "linux")]
pub(super) fn remove_tree_wal(
    target: &WriteTarget,
    plan: RemoveTreeOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    platform::remove_tree_wal(target, plan, guard)
        .map(|effect| settle_authority_postflight(effect, &[target]))
}

#[cfg(target_os = "linux")]
pub(super) fn classify_remove_tree_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
) -> Result<super::recovery::RemoveTreeRecoveryAssessment, String> {
    platform::classify_remove_tree_recovery(record, phase)
}

#[cfg(target_os = "linux")]
pub(super) fn execute_remove_tree_recovery(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
) -> Result<(), String> {
    platform::execute_remove_tree_recovery(record, phase)
}

#[cfg(target_os = "linux")]
pub(super) fn resolve_remove_tree_operator(
    record: &super::recovery::WalRecord,
    phase: super::recovery::WalPhase,
    action: super::recovery::WriteAuthorityRecoveryResolutionAction,
) -> Result<String, String> {
    platform::resolve_remove_tree_operator(record, phase, action)
}

#[cfg(target_os = "linux")]
pub(super) fn plan_directory(target: &WriteTarget) -> Result<DirectoryOperationPlan, String> {
    platform::plan_directory(target)
}

#[cfg(all(target_os = "linux", test))]
pub(super) fn plan_legacy_directory_for_test(
    target: &WriteTarget,
) -> Result<DirectoryOperationPlan, String> {
    platform::plan_legacy_directory_for_test(target)
}

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
pub(super) fn symlink_entry_wal(
    target: &WriteTarget,
    source: &Path,
    plan: &SymlinkOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    platform::symlink_entry_wal(target, source, plan, guard)
        .map(|effect| settle_authority_postflight(effect, &[target]))
}

#[cfg(target_os = "linux")]
pub(super) fn remove_file_if_exists_maintenance(
    target: &WriteTarget,
) -> Result<CapabilityEffect, String> {
    platform::remove_file_if_exists(target)
        .map(|effect| settle_authority_postflight(effect, &[target]))
}

#[cfg(target_os = "linux")]
pub(super) fn rename_noreplace(
    source: &WriteTarget,
    destination: &WriteTarget,
) -> Result<CapabilityEffect, String> {
    platform::rename_noreplace(source, destination)
        .map(|effect| settle_authority_postflight(effect, &[source, destination]))
}

#[cfg(target_os = "linux")]
pub(super) fn publish_rebuildable_directory(
    source: &WriteTarget,
    destination: &WriteTarget,
) -> Result<CapabilityEffect, String> {
    platform::publish_rebuildable_directory(source, destination)
        .map(|effect| settle_authority_postflight(effect, &[source, destination]))
}

#[cfg(target_os = "linux")]
pub(super) fn remove_rebuildable_directory_if_exists(
    target: &WriteTarget,
    operation_id: &str,
) -> Result<CapabilityEffect, String> {
    platform::remove_rebuildable_tree(target, operation_id)
        .map(|effect| settle_authority_postflight(effect, &[target]))
}

#[cfg(target_os = "linux")]
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

#[cfg(not(target_os = "linux"))]
fn unsupported() -> Result<CapabilityEffect, String> {
    Err(
        "Capability filesystem nu este implementat pe această platformă; scrierea este blocată fail-closed."
            .to_string(),
    )
}

#[cfg(not(target_os = "linux"))]
pub(super) struct CapabilityFileLock;

#[cfg(not(target_os = "linux"))]
pub(super) struct CapabilityDirectoryLease;

#[cfg(not(target_os = "linux"))]
impl CapabilityDirectoryLease {
    pub(super) fn current_dir_path(&self) -> std::path::PathBuf {
        std::path::PathBuf::new()
    }

    pub(super) fn require_empty(&self) -> Result<(), String> {
        unsupported().map(|_| ())
    }
}

#[cfg(not(target_os = "linux"))]
pub(super) fn capture_directory_lease_from_authority(
    _authority: &DirectoryAuthority,
    _path: &Path,
    _public_label: &str,
) -> Result<CapabilityDirectoryLease, String> {
    Err(
        "Capability filesystem nu poate deriva cwd-ul din authority pe această platformă; operația este blocată fail-closed."
            .to_string(),
    )
}

#[cfg(not(target_os = "linux"))]
pub(super) fn open_regular_file_readonly_no_follow(
    _path: &Path,
    _public_label: &str,
) -> Result<std::fs::File, String> {
    Err(
        "Capability filesystem read-only no-follow nu este implementat pe această platformă."
            .to_string(),
    )
}

#[cfg(not(target_os = "linux"))]
pub(super) fn open_optional_regular_file_readonly_no_follow(
    _path: &Path,
    _public_label: &str,
) -> Result<Option<std::fs::File>, String> {
    Err(
        "Capability filesystem read-only no-follow nu este implementat pe această platformă."
            .to_string(),
    )
}

#[cfg(not(target_os = "linux"))]
pub(super) fn read_bounded_regular_file_from_authority(
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

#[cfg(not(target_os = "linux"))]
pub(super) fn capture_directory_authority(
    _path: &Path,
    _public_label: &str,
    _scope: DirectoryAuthorityScope,
) -> Result<DirectoryAuthority, String> {
    Err(
        "Capability filesystem nu poate captura authority roots pe această platformă; operația este blocată fail-closed."
            .to_string(),
    )
}

#[cfg(not(target_os = "linux"))]
pub(super) fn bootstrap_directory_authority(
    _path: &Path,
    _public_label: &str,
    _scope: DirectoryAuthorityScope,
) -> Result<DirectoryAuthority, String> {
    Err(
        "Capability filesystem nu poate bootstrap-a authority roots pe această platformă; operația este blocată fail-closed."
            .to_string(),
    )
}

#[cfg(not(target_os = "linux"))]
pub(super) fn create_directory_from_authority(
    _authority: &DirectoryAuthority,
    _path: &Path,
    _public_label: &str,
) -> Result<(), String> {
    unsupported().map(|_| ())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn capture_descendant_authority(
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

#[cfg(not(target_os = "linux"))]
pub(super) fn verify_directory_authority_path(
    _authority: &DirectoryAuthority,
) -> Result<(), String> {
    unsupported().map(|_| ())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn lock_file(
    _target: &WriteTarget,
    _mode: CapabilityLockMode,
) -> Result<CapabilityFileLock, String> {
    Err(
        "Capability filesystem nu este implementat pe această platformă; lock-ul este blocat fail-closed."
            .to_string(),
    )
}

#[cfg(not(target_os = "linux"))]
pub(super) fn atomic_write(
    _target: &WriteTarget,
    _bytes: &[u8],
    _replace_policy: CapabilityReplacePolicy,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

#[cfg(not(target_os = "linux"))]
pub(super) fn plan_atomic_write(
    _target: &WriteTarget,
    _bytes: &[u8],
    _replace_policy: CapabilityReplacePolicy,
    _operation_id: &str,
) -> Result<AtomicOperationPlan, String> {
    Err("WriteAuthority WAL este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn atomic_write_wal(
    _target: &WriteTarget,
    _bytes: &[u8],
    _replace_policy: CapabilityReplacePolicy,
    _plan: &AtomicOperationPlan,
    _guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

#[cfg(not(target_os = "linux"))]
pub(super) fn classify_atomic_recovery(
    _record: &super::recovery::WalRecord,
    _phase: super::recovery::WalPhase,
    _read_budget: &mut super::recovery::RecoveryReadBudget,
) -> Result<super::recovery::AtomicRecoveryAssessment, String> {
    Err("WriteAuthority WAL recovery este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn execute_atomic_recovery(
    _record: &super::recovery::WalRecord,
    _phase: super::recovery::WalPhase,
    _read_budget: &mut super::recovery::RecoveryReadBudget,
) -> Result<(), String> {
    Err("WriteAuthority WAL recovery este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn discard_rebuildable_atomic_projection(
    _record: &super::recovery::WalRecord,
    _phase: super::recovery::WalPhase,
) -> Result<(), String> {
    Err("Cleanup-ul proiecției rebuildable este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn classify_append_recovery(
    _record: &super::recovery::WalRecord,
    _phase: super::recovery::WalPhase,
    _checkpoint: Option<&super::recovery::WalAppendStageCheckpoint>,
    _read_budget: &mut super::recovery::RecoveryReadBudget,
) -> Result<super::recovery::AppendRecoveryAssessment, String> {
    Err("WriteAuthority append WAL recovery este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn classify_directory_recovery(
    _record: &super::recovery::WalRecord,
    _phase: super::recovery::WalPhase,
    _checkpoint: Option<&super::recovery::WalDirectoryStageCheckpoint>,
) -> Result<super::recovery::DirectoryRecoveryAssessment, String> {
    Err("WriteAuthority mkdir WAL recovery este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn execute_directory_recovery(
    _record: &super::recovery::WalRecord,
    _phase: super::recovery::WalPhase,
    _checkpoint: Option<&super::recovery::WalDirectoryStageCheckpoint>,
    _action: super::recovery::DirectoryRecoveryAction,
) -> Result<(), String> {
    Err("WriteAuthority mkdir WAL recovery este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn resolve_directory_operator(
    _record: &super::recovery::WalRecord,
    _phase: super::recovery::WalPhase,
    _checkpoint: Option<&super::recovery::WalDirectoryStageCheckpoint>,
    _action: super::recovery::WriteAuthorityRecoveryResolutionAction,
    _expected_evidence_hash: &str,
    _wal_evidence_binding_hash: &str,
) -> Result<String, String> {
    Err("WriteAuthority Directory operator recovery este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn classify_symlink_recovery(
    _record: &super::recovery::WalRecord,
    _phase: super::recovery::WalPhase,
    _checkpoint: Option<&super::recovery::WalSymlinkStageCheckpoint>,
) -> Result<super::recovery::SymlinkRecoveryAssessment, String> {
    Err("WriteAuthority symlink WAL recovery este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn execute_symlink_recovery(
    _record: &super::recovery::WalRecord,
    _phase: super::recovery::WalPhase,
    _checkpoint: Option<&super::recovery::WalSymlinkStageCheckpoint>,
    _action: super::recovery::SymlinkRecoveryAction,
) -> Result<(), String> {
    Err("WriteAuthority symlink WAL recovery este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn resolve_symlink_operator(
    _record: &super::recovery::WalRecord,
    _phase: super::recovery::WalPhase,
    _checkpoint: Option<&super::recovery::WalSymlinkStageCheckpoint>,
    _action: super::recovery::WriteAuthorityRecoveryResolutionAction,
    _expected_evidence_hash: &str,
    _wal_evidence_binding_hash: &str,
) -> Result<String, String> {
    Err("WriteAuthority Symlink operator recovery este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn execute_append_recovery(
    _record: &super::recovery::WalRecord,
    _phase: super::recovery::WalPhase,
    _checkpoint: Option<&super::recovery::WalAppendStageCheckpoint>,
    _read_budget: &mut super::recovery::RecoveryReadBudget,
) -> Result<(), String> {
    Err("WriteAuthority append WAL recovery este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn plan_external_config(
    _target: &WriteTarget,
    _bytes: &[u8],
    _backup: Option<(&WriteTarget, &[u8])>,
    _operation_id: &str,
) -> Result<ExternalConfigOperationPlan, String> {
    Err("WriteAuthority ExternalConfig WAL este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn external_config_wal(
    _target: &WriteTarget,
    _bytes: &[u8],
    _backup: Option<(&WriteTarget, &[u8])>,
    _plan: ExternalConfigOperationPlan,
    _guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

#[cfg(not(target_os = "linux"))]
pub(super) fn classify_external_config_recovery(
    _record: &super::recovery::WalRecord,
    _phase: super::recovery::WalPhase,
    _checkpoint: Option<&super::recovery::WalExternalStageCheckpoint>,
    _decision: Option<super::recovery::WalExternalOperatorDecision>,
    _read_budget: &mut super::recovery::RecoveryReadBudget,
) -> Result<super::recovery::ExternalConfigRecoveryAssessment, String> {
    Err("WriteAuthority ExternalConfig recovery este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn execute_external_config_recovery(
    _record: &super::recovery::WalRecord,
    _phase: super::recovery::WalPhase,
    _checkpoint: Option<&super::recovery::WalExternalStageCheckpoint>,
    _decision: Option<super::recovery::WalExternalOperatorDecision>,
    _read_budget: &mut super::recovery::RecoveryReadBudget,
) -> Result<(), String> {
    Err("WriteAuthority ExternalConfig recovery este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn append(_target: &WriteTarget, _bytes: &[u8]) -> Result<CapabilityEffect, String> {
    unsupported()
}

#[cfg(not(target_os = "linux"))]
pub(super) fn plan_append(
    _target: &WriteTarget,
    _bytes: &[u8],
) -> Result<AppendOperationPlan, String> {
    Err("WriteAuthority append WAL este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn append_wal(
    _target: &WriteTarget,
    _bytes: &[u8],
    _plan: AppendOperationPlan,
    _guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

#[cfg(not(target_os = "linux"))]
pub(super) fn plan_copy(
    _target: &WriteTarget,
    _source: &Path,
    _replace_policy: CapabilityReplacePolicy,
    _operation_id: &str,
) -> Result<CopyOperationPlan, String> {
    Err("WriteAuthority copy WAL este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn copy_file_wal(
    _target: &WriteTarget,
    _source: &Path,
    _replace_policy: CapabilityReplacePolicy,
    _plan: CopyOperationPlan,
    _guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

#[cfg(not(target_os = "linux"))]
pub(super) fn classify_copy_recovery(
    _record: &super::recovery::WalRecord,
    _phase: super::recovery::WalPhase,
    _checkpoint: Option<&super::recovery::WalCopyStageCheckpoint>,
) -> Result<super::recovery::CopyRecoveryAssessment, String> {
    Err("WriteAuthority copy recovery este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn execute_copy_recovery(
    _record: &super::recovery::WalRecord,
    _phase: super::recovery::WalPhase,
    _checkpoint: Option<&super::recovery::WalCopyStageCheckpoint>,
    _read_budget: &mut super::recovery::RecoveryReadBudget,
) -> Result<(), String> {
    Err("WriteAuthority copy recovery este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn resolve_copy_operator(
    _record: &super::recovery::WalRecord,
    _phase: super::recovery::WalPhase,
    _checkpoint: Option<&super::recovery::WalCopyStageCheckpoint>,
    _action: super::recovery::WriteAuthorityRecoveryResolutionAction,
) -> Result<String, String> {
    Err("WriteAuthority Copy operator recovery este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn plan_rename(
    _source: &WriteTarget,
    _destination: &WriteTarget,
) -> Result<RenameOperationPlan, String> {
    Err("WriteAuthority rename WAL este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn rename_entry_wal(
    _source: &WriteTarget,
    _destination: &WriteTarget,
    _plan: RenameOperationPlan,
    _guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

#[cfg(not(target_os = "linux"))]
pub(super) fn classify_rename_recovery(
    _record: &super::recovery::WalRecord,
    _phase: super::recovery::WalPhase,
) -> Result<super::recovery::RenameRecoveryAssessment, String> {
    Err("WriteAuthority rename recovery este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn plan_remove_leaf(
    _target: &WriteTarget,
    _operation_id: &str,
) -> Result<Option<RemoveLeafOperationPlan>, String> {
    Err("WriteAuthority RemoveFile WAL este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn remove_leaf_wal(
    _target: &WriteTarget,
    _plan: RemoveLeafOperationPlan,
    _guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

#[cfg(not(target_os = "linux"))]
pub(super) fn classify_remove_leaf_recovery(
    _record: &super::recovery::WalRecord,
    _phase: super::recovery::WalPhase,
) -> Result<super::recovery::RemoveLeafRecoveryAssessment, String> {
    Err("WriteAuthority RemoveFile recovery este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn execute_remove_leaf_recovery(
    _record: &super::recovery::WalRecord,
    _phase: super::recovery::WalPhase,
) -> Result<(), String> {
    Err("WriteAuthority RemoveFile recovery este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn resolve_remove_leaf_operator(
    _record: &super::recovery::WalRecord,
    _phase: super::recovery::WalPhase,
    _action: super::recovery::WriteAuthorityRecoveryResolutionAction,
) -> Result<String, String> {
    Err("WriteAuthority RemoveFile operator recovery este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn plan_remove_tree(
    _target: &WriteTarget,
    _operation_id: &str,
) -> Result<Option<RemoveTreeOperationPlan>, String> {
    Err("WriteAuthority RemoveDirectoryTree WAL este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn remove_tree_wal(
    _target: &WriteTarget,
    _plan: RemoveTreeOperationPlan,
    _guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

#[cfg(not(target_os = "linux"))]
pub(super) fn classify_remove_tree_recovery(
    _record: &super::recovery::WalRecord,
    _phase: super::recovery::WalPhase,
) -> Result<super::recovery::RemoveTreeRecoveryAssessment, String> {
    Err("WriteAuthority RemoveDirectoryTree recovery este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn execute_remove_tree_recovery(
    _record: &super::recovery::WalRecord,
    _phase: super::recovery::WalPhase,
) -> Result<(), String> {
    Err("WriteAuthority RemoveDirectoryTree recovery este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn resolve_remove_tree_operator(
    _record: &super::recovery::WalRecord,
    _phase: super::recovery::WalPhase,
    _action: super::recovery::WriteAuthorityRecoveryResolutionAction,
) -> Result<String, String> {
    Err(
        "WriteAuthority RemoveDirectoryTree operator recovery este fail-closed în afara Linux."
            .into(),
    )
}

#[cfg(not(target_os = "linux"))]
pub(super) fn execute_rename_recovery(
    _record: &super::recovery::WalRecord,
    _phase: super::recovery::WalPhase,
) -> Result<(), String> {
    Err("WriteAuthority rename recovery este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn plan_directory(_target: &WriteTarget) -> Result<DirectoryOperationPlan, String> {
    Err("WriteAuthority mkdir WAL este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn create_directory_all_wal(
    _target: &WriteTarget,
    _plan: &DirectoryOperationPlan,
    _guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

#[cfg(not(target_os = "linux"))]
pub(super) fn plan_symlink(
    _target: &WriteTarget,
    _source: &Path,
) -> Result<SymlinkOperationPlan, String> {
    Err("WriteAuthority symlink WAL este fail-closed în afara Linux.".into())
}

#[cfg(not(target_os = "linux"))]
pub(super) fn symlink_entry_wal(
    _target: &WriteTarget,
    _source: &Path,
    _plan: &SymlinkOperationPlan,
    _guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

#[cfg(not(target_os = "linux"))]
pub(super) fn remove_file_if_exists_maintenance(
    _target: &WriteTarget,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

#[cfg(not(target_os = "linux"))]
pub(super) fn rename_noreplace(
    _source: &WriteTarget,
    _destination: &WriteTarget,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

#[cfg(not(target_os = "linux"))]
pub(super) fn publish_rebuildable_directory(
    _source: &WriteTarget,
    _destination: &WriteTarget,
) -> Result<CapabilityEffect, String> {
    unsupported()
}

#[cfg(not(target_os = "linux"))]
pub(super) fn remove_rebuildable_directory_if_exists(
    _target: &WriteTarget,
    _operation_id: &str,
) -> Result<CapabilityEffect, String> {
    unsupported()
}
