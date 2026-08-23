#![cfg(target_os = "linux")]

use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    os::unix::ffi::OsStringExt,
    os::unix::fs::{symlink, MetadataExt, PermissionsExt},
    panic::{catch_unwind, AssertUnwindSafe},
    path::{Path, PathBuf},
    sync::{mpsc, Arc},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::kernel::write_authority::{
    capability::{self, CapabilityReplacePolicy},
    model::{WriteCategory, WriteIntent, WriteOperationKind, WriteOwner, WritePolicy, WriteTarget},
    operation::{
        build_append_wal_record, build_atomic_wal_record, build_copy_wal_record,
        build_directory_wal_record, build_external_config_wal_record, build_remove_leaf_wal_record,
        build_remove_tree_wal_record, build_rename_wal_record, build_symlink_wal_record,
    },
    root_authority::DirectoryAuthorityScope,
};

use super::{
    model::{
        WalOperationEvidence, WalPhase, WriteAuthorityRecoveryResolutionAction,
        WriteAuthorityRecoveryResolutionInput, WRITE_AUTHORITY_RECOVERY_RESOLUTION_SCHEMA_VERSION,
    },
    paths::{
        WalAppendStageCheckpoint, WalAppendStageRole, WalCopyStageCheckpoint, WalCopyStageRole,
        WalRecordName,
    },
    RecoveryCoordinator, RecoveryReadBudget,
};
use crate::{kernel::file_buffer_store::hash_bytes, project::project_disk_metadata_version_token};

mod corruption;
mod fixtures;
mod journal;
mod replay;
mod rollback;
mod session_transition;
