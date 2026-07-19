use std::{ffi::OsString, path::PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{
    model::WriteIntent,
    recovery::{
        encode_path_hex, WalAppendEvidence, WalAtomicFileEvidence, WalAuthorityEvidence,
        WalCopyEvidence, WalDirectoryEvidence, WalExternalConfigEvidence, WalFilesystemIdentity,
        WalOperationEvidence, WalRecord, WalRecordBody, WalRemoveLeafEvidence,
        WalRemoveTreeEvidence, WalRenameEvidence, WalSymlinkEvidence, WAL_SCHEMA_VERSION,
    },
    root_authority::{DirectoryAuthority, DirectoryAuthorityScope},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AtomicOperationPlan {
    pub evidence: WalAtomicFileEvidence,
}

#[derive(Debug)]
pub(super) struct AppendOperationPlan {
    pub evidence: WalAppendEvidence,
}

#[derive(Debug)]
pub(super) struct CopyOperationPlan {
    pub evidence: WalCopyEvidence,
    #[cfg(target_os = "linux")]
    pub source_file: std::fs::File,
}

#[derive(Debug)]
pub(super) struct ExternalConfigOperationPlan {
    pub evidence: WalExternalConfigEvidence,
    #[cfg(target_os = "linux")]
    pub existing_target: Option<std::fs::File>,
}

#[derive(Debug)]
pub(super) struct RenameOperationPlan {
    pub evidence: WalRenameEvidence,
    #[cfg(target_os = "linux")]
    pub source_handle: rustix::fd::OwnedFd,
    #[cfg(target_os = "linux")]
    pub source_content: Option<std::fs::File>,
    #[cfg(target_os = "linux")]
    pub source_directory: Option<rustix::fd::OwnedFd>,
}

#[derive(Debug)]
pub(super) struct RemoveLeafOperationPlan {
    pub evidence: WalRemoveLeafEvidence,
    #[cfg(target_os = "linux")]
    pub source_handle: rustix::fd::OwnedFd,
    #[cfg(target_os = "linux")]
    pub source_content: Option<std::fs::File>,
}

#[derive(Debug)]
pub(super) struct RemoveTreeOperationPlan {
    pub evidence: WalRemoveTreeEvidence,
    #[cfg(target_os = "linux")]
    pub source_directory: rustix::fd::OwnedFd,
    #[cfg(target_os = "linux")]
    pub source_records: Vec<super::tree_fingerprint::TreeFingerprintRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DirectoryOperationPlan {
    pub evidence: WalDirectoryEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SymlinkOperationPlan {
    pub evidence: WalSymlinkEvidence,
}

impl AtomicOperationPlan {
    pub(super) fn temp_leaf(&self) -> Result<OsString, String> {
        super::recovery::decode_component_hex(&self.evidence.temp_leaf_hex)
    }
}

impl CopyOperationPlan {
    pub(super) fn temp_leaf(&self) -> Result<OsString, String> {
        super::recovery::decode_component_hex(&self.evidence.file.temp_leaf_hex)
    }
}

impl SymlinkOperationPlan {
    pub(super) fn desired_target(&self) -> Result<PathBuf, String> {
        super::recovery::decode_path_hex(&self.evidence.desired_link_target_hex)
    }
}

pub(super) fn atomic_temp_leaf(operation_id: &str) -> OsString {
    let digest = format!("{:x}", Sha256::digest(operation_id.as_bytes()));
    OsString::from(format!(".pana-wa-{}.tmp", &digest[..32]))
}

pub(super) fn remove_quarantine_leaf(operation_id: &str) -> OsString {
    let digest = format!("{:x}", Sha256::digest(operation_id.as_bytes()));
    OsString::from(format!(".pana-wa-{}.remove-quarantine", &digest[..32]))
}

pub(super) fn remove_tree_quarantine_leaf(operation_id: &str) -> OsString {
    let digest = format!("{:x}", Sha256::digest(operation_id.as_bytes()));
    OsString::from(format!(".pana-wa-{}.remove-tree-quarantine", &digest[..32]))
}

pub(super) fn external_config_target_temp_leaf(operation_id: &str) -> OsString {
    let digest = format!("{:x}", Sha256::digest(operation_id.as_bytes()));
    OsString::from(format!(".pana-wa-{}.external-target.tmp", &digest[..32]))
}

pub(super) fn external_config_backup_temp_leaf(operation_id: &str) -> OsString {
    let digest = format!("{:x}", Sha256::digest(operation_id.as_bytes()));
    OsString::from(format!(".pana-wa-{}.external-backup.tmp", &digest[..32]))
}

pub(super) fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn build_atomic_wal_record(
    operation_id: &str,
    created_at_ms: u128,
    intent: &WriteIntent,
    plan: &AtomicOperationPlan,
) -> Result<WalRecord, String> {
    build_wal_record(
        operation_id,
        created_at_ms,
        intent,
        WalOperationEvidence::AtomicFile(plan.evidence.clone()),
    )
}

pub(super) fn build_append_wal_record(
    operation_id: &str,
    created_at_ms: u128,
    intent: &WriteIntent,
    plan: &AppendOperationPlan,
) -> Result<WalRecord, String> {
    build_wal_record(
        operation_id,
        created_at_ms,
        intent,
        WalOperationEvidence::Append(plan.evidence.clone()),
    )
}

pub(super) fn build_directory_wal_record(
    operation_id: &str,
    created_at_ms: u128,
    intent: &WriteIntent,
    plan: &DirectoryOperationPlan,
) -> Result<WalRecord, String> {
    build_wal_record(
        operation_id,
        created_at_ms,
        intent,
        WalOperationEvidence::Directory(plan.evidence.clone()),
    )
}

pub(super) fn build_copy_wal_record(
    operation_id: &str,
    created_at_ms: u128,
    intent: &WriteIntent,
    plan: &CopyOperationPlan,
) -> Result<WalRecord, String> {
    build_wal_record(
        operation_id,
        created_at_ms,
        intent,
        WalOperationEvidence::Copy(plan.evidence.clone()),
    )
}

pub(super) fn build_external_config_wal_record(
    operation_id: &str,
    created_at_ms: u128,
    intent: &WriteIntent,
    plan: &ExternalConfigOperationPlan,
) -> Result<WalRecord, String> {
    build_wal_record(
        operation_id,
        created_at_ms,
        intent,
        WalOperationEvidence::ExternalConfig(plan.evidence.clone()),
    )
}

pub(super) fn build_rename_wal_record(
    operation_id: &str,
    created_at_ms: u128,
    intent: &WriteIntent,
    plan: &RenameOperationPlan,
) -> Result<WalRecord, String> {
    build_wal_record(
        operation_id,
        created_at_ms,
        intent,
        WalOperationEvidence::Rename(plan.evidence.clone()),
    )
}

pub(super) fn build_remove_leaf_wal_record(
    operation_id: &str,
    created_at_ms: u128,
    intent: &WriteIntent,
    plan: &RemoveLeafOperationPlan,
) -> Result<WalRecord, String> {
    build_wal_record(
        operation_id,
        created_at_ms,
        intent,
        WalOperationEvidence::RemoveLeaf(plan.evidence.clone()),
    )
}

pub(super) fn build_remove_tree_wal_record(
    operation_id: &str,
    created_at_ms: u128,
    intent: &WriteIntent,
    plan: &RemoveTreeOperationPlan,
) -> Result<WalRecord, String> {
    build_wal_record(
        operation_id,
        created_at_ms,
        intent,
        WalOperationEvidence::RemoveTree(plan.evidence.clone()),
    )
}

pub(super) fn build_symlink_wal_record(
    operation_id: &str,
    created_at_ms: u128,
    intent: &WriteIntent,
    plan: &SymlinkOperationPlan,
) -> Result<WalRecord, String> {
    build_wal_record(
        operation_id,
        created_at_ms,
        intent,
        WalOperationEvidence::Symlink(plan.evidence.clone()),
    )
}

fn build_wal_record(
    operation_id: &str,
    created_at_ms: u128,
    intent: &WriteIntent,
    operation_evidence: WalOperationEvidence,
) -> Result<WalRecord, String> {
    let authority = intent.target.authority().ok_or_else(|| {
        "WriteAuthority WAL refuză un target care nu a fost legat de authority canonică."
            .to_string()
    })?;
    WalRecord::seal(WalRecordBody {
        schema_version: WAL_SCHEMA_VERSION,
        operation_id: operation_id.to_string(),
        created_at_ms,
        process_id: std::process::id(),
        category: serde_label(&intent.category)?,
        owner: serde_label(&intent.owner)?,
        operation: serde_label(&intent.operation)?,
        recovery_policy: serde_label(&intent.policy.recovery)?,
        public_label: intent.target.public_label.clone(),
        authority: wal_authority_evidence(authority),
        runtime_session_id: intent.target.expected_runtime_session_id.clone(),
        command_id: None,
        transaction_id: None,
        operation_evidence,
    })
}

pub(super) fn wal_authority_evidence(authority: &DirectoryAuthority) -> WalAuthorityEvidence {
    let identity = authority.identity();
    WalAuthorityEvidence {
        scope: authority_scope_label(authority.scope()),
        boundary_path_hex: encode_path_hex(authority.root_path()),
        boundary_display: authority.root_path().to_string_lossy().to_string(),
        identity: WalFilesystemIdentity {
            device: identity.device,
            inode: identity.inode,
        },
    }
}

fn serde_label(value: &impl Serialize) -> Result<String, String> {
    match serde_json::to_value(value)
        .map_err(|error| format!("WriteAuthority WAL nu poate serializa un label: {error}"))?
    {
        serde_json::Value::String(value) => Ok(value),
        _ => Err("WriteAuthority WAL a primit un label non-string.".into()),
    }
}

fn authority_scope_label(scope: &DirectoryAuthorityScope) -> String {
    match scope {
        DirectoryAuthorityScope::ApplicationConfig => "application_config".into(),
        DirectoryAuthorityScope::ApplicationData => "application_data".into(),
        DirectoryAuthorityScope::ApplicationCache => "application_cache".into(),
        DirectoryAuthorityScope::ApplicationLogs => "application_logs".into(),
        DirectoryAuthorityScope::ApplicationPreviewCache => "application_preview_cache".into(),
        DirectoryAuthorityScope::ApplicationWriteAuthorityWal => {
            "application_write_authority_wal".into()
        }
        DirectoryAuthorityScope::RecoveryTarget => "recovery_target".into(),
        DirectoryAuthorityScope::ProjectRoot => "project_root".into(),
        DirectoryAuthorityScope::ProjectBootstrap { lease_id } => {
            format!("project_bootstrap:{lease_id}")
        }
        DirectoryAuthorityScope::ExternalCodex { lease_id } => {
            format!("external_codex:{lease_id}")
        }
    }
}
