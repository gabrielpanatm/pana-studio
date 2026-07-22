use std::path::Component;

use serde::{Deserialize, Serialize};

use sha2::{Digest, Sha256};

pub(crate) const WAL_SCHEMA_VERSION: u32 = 1;
// Append v2 persistă recordul JSONL complet, hex-encoded, pentru a putea
// continua un short write fără truncate. Limita per-record rămâne bounded,
// iar bugetul agregat al directorului WAL rămâne neschimbat.
pub(super) const MAX_WAL_RECORD_BYTES: usize = 640 * 1024;
pub(super) const MAX_WAL_RECORDS: usize = 256;
pub(super) const MAX_WAL_TOTAL_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const MAX_WAL_APPEND_PREFIX_BYTES: usize = 16 * 1024;
pub(crate) const MAX_WAL_APPEND_PAYLOAD_BYTES: usize = 256 * 1024;
pub(crate) const MAX_WAL_APPEND_TAIL_BYTES: usize = 64 * 1024;
pub(crate) const MAX_WAL_SYMLINK_TARGET_BYTES: usize = 4096;
pub(crate) const MAX_WAL_COPY_BYTES: u64 = 512 * 1024 * 1024;
pub(crate) const MAX_WAL_EXTERNAL_CONFIG_BYTES: u64 = 4 * 1024 * 1024;
pub(crate) const WAL_COPY_PROTOCOL_VERSION: u32 = 2;
pub(crate) const WAL_EXTERNAL_CONFIG_PROTOCOL_VERSION: u32 = 2;
pub(crate) const WAL_APPEND_PROTOCOL_VERSION: u32 = 2;
pub(crate) const WAL_SYMLINK_PROTOCOL_VERSION: u32 = 2;
// Protocolul on-disk `2` a aparținut prototipului temp+rename. Forma curentă
// publică leaf-ul direct prin mkdirat create-only și nu trebuie să poată
// reinterpreta recordurile vechi; de aceea contractul direct are versiunea 3.
pub(crate) const WAL_DIRECTORY_PROTOCOL_VERSION: u32 = 3;
// Un singur record WAL valid poate fi hot în fluxul normal. Bugetul acoperă
// astfel cel mai mare payload Copy acceptat, fără buffer în memorie: recovery
// hash-uiește streaming și rămâne bounded la 512 MiB per ciclu.
pub(crate) const MAX_WAL_RECOVERY_READ_BYTES: u64 = MAX_WAL_COPY_BYTES;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WalPhase {
    Preparing,
    Prepared,
    AuxiliaryDurable,
    EffectVisible,
    TargetDurable,
}

impl WalPhase {
    pub(crate) const fn next(self) -> Option<Self> {
        match self {
            Self::Preparing => Some(Self::Prepared),
            Self::Prepared => Some(Self::AuxiliaryDurable),
            Self::AuxiliaryDurable => Some(Self::EffectVisible),
            Self::EffectVisible => Some(Self::TargetDurable),
            Self::TargetDurable => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WalFilesystemIdentity {
    pub device: u64,
    pub inode: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WalAuthorityEvidence {
    pub scope: String,
    pub boundary_path_hex: String,
    pub boundary_display: String,
    pub identity: WalFilesystemIdentity,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WalParentEvidence {
    pub relative_components_hex: Vec<String>,
    pub existing_prefix_len: usize,
    pub existing_ancestor_identity: WalFilesystemIdentity,
    pub parent_identity: Option<WalFilesystemIdentity>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum WalLeafEvidence {
    Absent,
    Regular {
        identity: WalFilesystemIdentity,
        size: u64,
        version_token: String,
        content_hash: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WalAtomicFileEvidence {
    pub parent: WalParentEvidence,
    pub target_leaf_hex: String,
    pub temp_leaf_hex: String,
    pub before: WalLeafEvidence,
    pub new_size: u64,
    pub new_content_hash: String,
    pub replace: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum WalAppendBefore {
    Absent,
    Present {
        identity: WalFilesystemIdentity,
        size: u64,
        version_token: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WalAppendEvidence {
    /// `0` este protocolul legacy, păstrat exclusiv pentru diagnostic și
    /// `Prepared + no_effect`. Append v2 nu atribuie efecte fără checkpoint.
    #[serde(default)]
    pub protocol_version: u32,
    pub parent: WalParentEvidence,
    pub target_leaf_hex: String,
    pub before: WalAppendBefore,
    pub payload_size: u64,
    pub payload_hash: String,
    pub payload_prefix_hex: String,
    pub payload_complete_in_record: bool,
    /// Payloadul complet v2. Hex-ul evită orice ambiguitate de framing JSONL.
    #[serde(default)]
    pub payload_hex: Option<String>,
    /// Digest statx lifetime al targetului Present. Pentru Absent identitatea
    /// inode-ului anonim este publicată ulterior în checkpointul filename.
    #[serde(default)]
    pub before_identity_digest: Option<String>,
    /// Fereastră bounded de la sfârșitul baseline-ului, pentru a întări CAS-ul
    /// de frontieră fără a hash-ui întregul jurnal la fiecare append.
    #[serde(default)]
    pub before_tail_size: u64,
    #[serde(default)]
    pub before_tail_hash: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WalDirectoryEvidence {
    /// `0` identifică recordurile mkdir legacy multi-component. Producția
    /// curentă emite exclusiv forma Directory v2 single-leaf, protocol on-disk
    /// `3` pentru publicarea directă create-only.
    #[serde(default)]
    pub protocol_version: u32,
    pub relative_components_hex: Vec<String>,
    pub existing_prefix_len: usize,
    pub existing_ancestor_identity: WalFilesystemIdentity,
    pub existing_target_identity: Option<WalFilesystemIdentity>,
    /// Câmpurile de mai jos lipsesc din recordurile legacy și sunt obligatorii
    /// pentru Directory v2 direct.
    #[serde(default)]
    pub parent_identity: Option<WalFilesystemIdentity>,
    #[serde(default)]
    pub target_leaf_hex: Option<String>,
    #[serde(default)]
    pub existing_target_identity_digest: Option<String>,
    #[serde(default)]
    pub existing_target_version_token: Option<String>,
    #[serde(default)]
    pub desired_mode_bits: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum WalSymlinkBefore {
    Absent,
    Exact {
        identity: WalFilesystemIdentity,
        version_token: String,
        link_target_hex: String,
        #[serde(default)]
        identity_digest: Option<String>,
        #[serde(default)]
        state_digest: Option<String>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WalSymlinkEvidence {
    /// `0` este forma lifecycle legacy, păstrată numai pentru recovery
    /// conservator și teste explicite. Producția Preview emite protocolul 2
    /// direct create-only, fără temp/rename/unlink.
    #[serde(default)]
    pub protocol_version: u32,
    pub parent: WalParentEvidence,
    pub target_leaf_hex: String,
    pub desired_link_target_hex: String,
    pub before: WalSymlinkBefore,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WalCopySourceEvidence {
    pub path_hex: String,
    pub identity: WalFilesystemIdentity,
    pub size: u64,
    pub version_token: String,
    pub content_hash: String,
    pub mode_bits: u32,
    pub link_count: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WalCopyDestinationPolicy {
    CreateNew,
    Replace,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WalCopyEvidence {
    /// `0` este forma legacy implicită a recordurilor WAL v1. Protocolul
    /// activ este versionat separat, astfel încât recordurile Copy vechi să
    /// rămână lizibile pentru diagnostic fără a primi automat semantics v2.
    #[serde(default)]
    pub protocol_version: u32,
    pub file: WalAtomicFileEvidence,
    pub source: WalCopySourceEvidence,
    pub destination_policy: WalCopyDestinationPolicy,
    pub before_mode_bits: Option<u32>,
    pub new_mode_bits: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WalExternalConfigEvidence {
    /// `0` este forma legacy implicită a recordurilor v1; ea poate fi citită
    /// pentru diagnostic, dar nu este executată automat. Protocolul activ
    /// este versionat separat ca să nu invalideze celelalte familii WAL v1.
    #[serde(default)]
    pub protocol_version: u32,
    pub target: WalAtomicFileEvidence,
    pub backup: Option<WalAtomicFileEvidence>,
    pub target_before_mode_bits: Option<u32>,
    /// Digest statx (mount/inode/btime) al baseline-ului capturat înainte de
    /// WAL. Prezența lui selectează protocolul ExternalConfig fără unlink:
    /// inode-ul original este relocat direct în backup.
    #[serde(default)]
    pub target_before_identity_digest: Option<String>,
    pub target_new_mode_bits: u32,
    pub backup_mode_bits: Option<u32>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WalRenameLeafKind {
    Regular,
    Directory,
    Symlink,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WalRenameSourceEvidence {
    pub identity: WalFilesystemIdentity,
    pub kind: WalRenameLeafKind,
    pub size: u64,
    pub mtime_seconds: i64,
    pub mtime_nanoseconds: u64,
    pub raw_mode: u32,
    pub link_count: u64,
    pub version_token: String,
    pub content_hash: Option<String>,
    pub tree_fingerprint: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WalRenameEvidence {
    pub source_parent: WalParentEvidence,
    pub source_leaf_hex: String,
    pub source: WalRenameSourceEvidence,
    pub destination_authority: WalAuthorityEvidence,
    pub destination_parent: WalParentEvidence,
    pub destination_leaf_hex: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WalRemoveLeafKind {
    Regular,
    Symlink,
    Other,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WalRemoveLeafSourceEvidence {
    pub identity: WalFilesystemIdentity,
    pub kind: WalRemoveLeafKind,
    pub size: u64,
    pub mtime_seconds: i64,
    pub mtime_nanoseconds: u64,
    pub ctime_seconds: i64,
    pub ctime_nanoseconds: u64,
    pub raw_mode: u32,
    pub link_count: u64,
    pub owner_uid: u32,
    pub owner_gid: u32,
    pub raw_device: u64,
    pub version_token: String,
    pub content_hash: Option<String>,
    pub symlink_target_hex: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WalRemoveLeafEvidence {
    pub parent: WalParentEvidence,
    pub target_leaf_hex: String,
    pub quarantine_leaf_hex: String,
    pub source: WalRemoveLeafSourceEvidence,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WalRemoveTreeSourceEvidence {
    pub identity: WalFilesystemIdentity,
    pub size: u64,
    pub mtime_seconds: i64,
    pub mtime_nanoseconds: u64,
    pub ctime_seconds: i64,
    pub ctime_nanoseconds: u64,
    pub raw_mode: u32,
    pub link_count: u64,
    pub owner_uid: u32,
    pub owner_gid: u32,
    pub raw_device: u64,
    pub version_token: String,
    pub tree_fingerprint: String,
    pub entry_count: u64,
    pub mount_id: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WalRemoveTreeEvidence {
    pub parent: WalParentEvidence,
    pub target_leaf_hex: String,
    pub quarantine_leaf_hex: String,
    pub source: WalRemoveTreeSourceEvidence,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "family", content = "evidence", rename_all = "snake_case")]
pub(crate) enum WalOperationEvidence {
    AtomicFile(WalAtomicFileEvidence),
    Append(WalAppendEvidence),
    Copy(WalCopyEvidence),
    Directory(WalDirectoryEvidence),
    ExternalConfig(WalExternalConfigEvidence),
    RemoveLeaf(WalRemoveLeafEvidence),
    RemoveTree(WalRemoveTreeEvidence),
    Rename(WalRenameEvidence),
    Symlink(WalSymlinkEvidence),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WalRecordBody {
    pub schema_version: u32,
    pub operation_id: String,
    pub created_at_ms: u128,
    pub process_id: u32,
    pub category: String,
    pub owner: String,
    pub operation: String,
    pub recovery_policy: String,
    pub public_label: String,
    pub authority: WalAuthorityEvidence,
    pub runtime_session_id: Option<String>,
    pub command_id: Option<String>,
    pub transaction_id: Option<String>,
    pub operation_evidence: WalOperationEvidence,
}

impl WalRecordBody {
    pub(super) fn validate(&self) -> Result<(), String> {
        if self.schema_version != WAL_SCHEMA_VERSION {
            return Err(format!(
                "WriteAuthority WAL refuză schema {} (suportată: {}).",
                self.schema_version, WAL_SCHEMA_VERSION
            ));
        }
        super::paths::validate_operation_id(&self.operation_id)?;
        if self.public_label.len() > 1024
            || self.category.len() > 64
            || self.owner.len() > 64
            || self.operation.len() > 64
            || self.recovery_policy.len() > 64
            || self.authority.scope.len() > 128
            || self.authority.boundary_display.len() > 4096
        {
            return Err(
                "WriteAuthority WAL refuză un câmp textual peste limita contractului.".into(),
            );
        }
        super::paths::decode_path_hex(&self.authority.boundary_path_hex)?;
        let family_matches_operation = match &self.operation_evidence {
            WalOperationEvidence::AtomicFile(_) => {
                matches!(self.operation.as_str(), "write_text" | "write_bytes")
            }
            WalOperationEvidence::Append(_) => self.operation == "append_text",
            WalOperationEvidence::Copy(_) => self.operation == "copy",
            WalOperationEvidence::Directory(_) => self.operation == "create_directory",
            WalOperationEvidence::ExternalConfig(_) => self.operation == "external_config_update",
            WalOperationEvidence::RemoveLeaf(_) => self.operation == "remove_file",
            WalOperationEvidence::RemoveTree(_) => self.operation == "remove_directory_tree",
            WalOperationEvidence::Rename(_) => self.operation == "rename",
            WalOperationEvidence::Symlink(_) => self.operation == "symlink",
        };
        if !family_matches_operation {
            return Err(format!(
                "WriteAuthority WAL refuză familia incompatibilă cu operația {}.",
                self.operation
            ));
        }
        match &self.operation_evidence {
            WalOperationEvidence::AtomicFile(evidence) => {
                if evidence.parent.relative_components_hex.len() > 256
                    || evidence.parent.existing_prefix_len
                        > evidence.parent.relative_components_hex.len()
                {
                    return Err(
                        "WriteAuthority WAL refuză planul atomic cu adâncime invalidă.".into(),
                    );
                }
                for component in &evidence.parent.relative_components_hex {
                    super::paths::decode_component_hex(component)?;
                }
                super::paths::decode_component_hex(&evidence.target_leaf_hex)?;
                super::paths::decode_component_hex(&evidence.temp_leaf_hex)?;
                if evidence.new_content_hash.len() != 64 {
                    return Err(
                        "WriteAuthority WAL refuză hash-ul nou cu format necunoscut.".into(),
                    );
                }
                if let WalLeafEvidence::Regular { content_hash, .. } = &evidence.before {
                    if content_hash.len() != 64 {
                        return Err(
                            "WriteAuthority WAL refuză hash-ul baseline cu format necunoscut."
                                .into(),
                        );
                    }
                }
            }
            WalOperationEvidence::Append(evidence) => {
                if evidence.parent.relative_components_hex.len() > 256
                    || evidence.parent.existing_prefix_len
                        > evidence.parent.relative_components_hex.len()
                {
                    return Err(
                        "WriteAuthority WAL refuză planul append cu adâncime invalidă.".into(),
                    );
                }
                for component in &evidence.parent.relative_components_hex {
                    super::paths::decode_component_hex(component)?;
                }
                super::paths::decode_component_hex(&evidence.target_leaf_hex)?;
                let prefix = super::paths::decode_bytes_hex(&evidence.payload_prefix_hex)?;
                if prefix.len() > MAX_WAL_APPEND_PREFIX_BYTES
                    || prefix.len() as u64 > evidence.payload_size
                    || evidence.payload_hash.len() != 64
                    || (evidence.payload_complete_in_record
                        && prefix.len() as u64 != evidence.payload_size)
                {
                    return Err("WriteAuthority WAL refuză evidence append invalidă.".into());
                }
                if !matches!(evidence.protocol_version, 0 | WAL_APPEND_PROTOCOL_VERSION) {
                    return Err(format!(
                        "WriteAuthority WAL refuză protocolul Append {}.",
                        evidence.protocol_version
                    ));
                }
                if evidence.protocol_version == WAL_APPEND_PROTOCOL_VERSION {
                    if self.category != "internal_app_write"
                        || self.operation != "append_text"
                        || self.recovery_policy != "append_only_journal"
                        || self.authority.scope != "application_data"
                    {
                        return Err("WriteAuthority WAL refuză authority/policy Append v2.".into());
                    }
                    let parents = evidence
                        .parent
                        .relative_components_hex
                        .iter()
                        .map(|component| super::paths::decode_component_hex(component))
                        .collect::<Result<Vec<_>, _>>()?;
                    let leaf = super::paths::decode_component_hex(&evidence.target_leaf_hex)?;
                    let leaf = leaf
                        .to_str()
                        .ok_or_else(|| "Append v2 cere target leaf UTF-8 declarat.".to_string())?;
                    let session_parent = parents.len() == 2
                        && parents[0].to_str() == Some("sessions")
                        && !parents[1].is_empty()
                        && evidence.parent.existing_prefix_len == parents.len()
                        && evidence.parent.parent_identity.is_some()
                        && evidence.parent.parent_identity.as_ref()
                            == Some(&evidence.parent.existing_ancestor_identity);
                    let owner_path = match self.owner.as_str() {
                        "kernel" => matches!(
                            leaf,
                            "project-transition-decisions.jsonl"
                                | "project-transition-decision-recovery-acknowledgements.jsonl"
                        ),
                        _ => false,
                    };
                    if !session_parent || !owner_path {
                        return Err("WriteAuthority WAL refuză owner/path Append v2.".into());
                    }
                    let payload_hex = evidence.payload_hex.as_deref().ok_or_else(|| {
                        "WriteAuthority WAL Append v2 cere payloadul complet.".to_string()
                    })?;
                    let payload = super::paths::decode_bytes_hex(payload_hex)?;
                    if payload.is_empty()
                        || payload.len() > MAX_WAL_APPEND_PAYLOAD_BYTES
                        || payload.len() as u64 != evidence.payload_size
                        || prefix != payload[..payload.len().min(MAX_WAL_APPEND_PREFIX_BYTES)]
                        || evidence.payload_complete_in_record
                            != (payload.len() <= MAX_WAL_APPEND_PREFIX_BYTES)
                        || format!("{:x}", Sha256::digest(&payload)) != evidence.payload_hash
                    {
                        return Err(
                            "WriteAuthority WAL refuză contractul payload Append v2.".into()
                        );
                    }
                    let line = payload.strip_suffix(b"\n").ok_or_else(|| {
                        "Append v2 cere exact o linie JSONL terminată cu newline.".to_string()
                    })?;
                    if line.is_empty()
                        || line.contains(&b'\n')
                        || line.contains(&b'\r')
                        || std::str::from_utf8(line).is_err()
                        || serde_json::from_slice::<serde_json::Value>(line).is_err()
                    {
                        return Err("WriteAuthority WAL refuză framingul/JSON-ul Append v2.".into());
                    }
                    match &evidence.before {
                        WalAppendBefore::Absent => {
                            if evidence.before_identity_digest.is_some()
                                || evidence.before_tail_size != 0
                                || evidence.before_tail_hash.is_some()
                            {
                                return Err(
                                    "Append v2 Absent conține baseline evidence imposibilă.".into(),
                                );
                            }
                        }
                        WalAppendBefore::Present { size, .. } => {
                            let identity =
                                evidence.before_identity_digest.as_deref().ok_or_else(|| {
                                    "Append v2 Present cere identitate statx.".to_string()
                                })?;
                            let tail_hash =
                                evidence.before_tail_hash.as_deref().ok_or_else(|| {
                                    "Append v2 Present cere hash-ul ferestrei tail.".to_string()
                                })?;
                            if !is_lower_hex_digest(identity, 32)
                                || !is_lower_hex_digest(tail_hash, 64)
                                || evidence.before_tail_size
                                    != (*size).min(MAX_WAL_APPEND_TAIL_BYTES as u64)
                            {
                                return Err(
                                    "WriteAuthority WAL refuză baseline-ul Append v2.".into()
                                );
                            }
                            size.checked_add(evidence.payload_size).ok_or_else(|| {
                                "Append v2 refuză overflow-ul dimensiunii finale.".to_string()
                            })?;
                        }
                    }
                }
            }
            WalOperationEvidence::Copy(evidence) => {
                let file = &evidence.file;
                if file.parent.relative_components_hex.len() > 256
                    || file.parent.existing_prefix_len > file.parent.relative_components_hex.len()
                {
                    return Err(
                        "WriteAuthority WAL refuză planul copy cu adâncime invalidă.".into(),
                    );
                }
                for component in &file.parent.relative_components_hex {
                    super::paths::decode_component_hex(component)?;
                }
                let target_leaf = super::paths::decode_component_hex(&file.target_leaf_hex)?;
                let temp_leaf = super::paths::decode_component_hex(&file.temp_leaf_hex)?;
                let source_path = super::paths::decode_path_hex(&evidence.source.path_hex)?;
                if !source_path.is_absolute()
                    || source_path.components().count() < 2
                    || !source_path
                        .components()
                        .enumerate()
                        .all(|(index, component)| {
                            matches!(component, Component::RootDir) && index == 0
                                || matches!(component, Component::Normal(_)) && index > 0
                        })
                    || target_leaf == temp_leaf
                    || evidence.source.size > MAX_WAL_COPY_BYTES
                    || evidence.source.content_hash.len() != 64
                    || !evidence
                        .source
                        .content_hash
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit())
                    || evidence.source.version_token.is_empty()
                    || evidence.source.version_token.len() > 512
                    || evidence.source.mode_bits > 0o7777
                    || evidence.source.link_count == 0
                    || !matches!(evidence.protocol_version, 0 | WAL_COPY_PROTOCOL_VERSION)
                    || (evidence.protocol_version == WAL_COPY_PROTOCOL_VERSION
                        && (file.parent.existing_prefix_len
                            != file.parent.relative_components_hex.len()
                            || file.parent.parent_identity.is_none()))
                    || (evidence.protocol_version == WAL_COPY_PROTOCOL_VERSION
                        && match evidence.destination_policy {
                            WalCopyDestinationPolicy::CreateNew => {
                                self.owner != "project_initializer"
                                    || self.category != "project_source_write"
                                    || !self.authority.scope.starts_with("project_bootstrap:")
                            }
                            WalCopyDestinationPolicy::Replace => {
                                self.owner != "preview"
                                    || self.category != "preview_workspace_write"
                                    || self.authority.scope != "application_preview_cache"
                            }
                        })
                    || evidence.before_mode_bits.is_some()
                        != matches!(file.before, WalLeafEvidence::Regular { .. })
                    || evidence.before_mode_bits.is_some_and(|mode| mode > 0o7777)
                    || file.replace != matches!(file.before, WalLeafEvidence::Regular { .. })
                    || (evidence.destination_policy == WalCopyDestinationPolicy::CreateNew
                        && !matches!(file.before, WalLeafEvidence::Absent))
                    || evidence.new_mode_bits != evidence.source.mode_bits
                    || file.new_size != evidence.source.size
                    || file.new_content_hash != evidence.source.content_hash
                {
                    return Err("WriteAuthority WAL refuză evidence copy invalidă.".into());
                }
                if let WalLeafEvidence::Regular { content_hash, .. } = &file.before {
                    if content_hash.len() != 64 {
                        return Err(
                            "WriteAuthority WAL refuză hash-ul baseline copy invalid.".into()
                        );
                    }
                }
            }
            WalOperationEvidence::Directory(evidence) => {
                if evidence.relative_components_hex.len() > 256
                    || evidence.existing_prefix_len > evidence.relative_components_hex.len()
                    || (evidence.existing_target_identity.is_some()
                        != (evidence.existing_prefix_len == evidence.relative_components_hex.len()))
                {
                    return Err("WriteAuthority WAL refuză planul de directoare invalid.".into());
                }
                for component in &evidence.relative_components_hex {
                    super::paths::decode_component_hex(component)?;
                }
                match evidence.protocol_version {
                    0 => {
                        if evidence.parent_identity.is_some()
                            || evidence.target_leaf_hex.is_some()
                            || evidence.existing_target_identity_digest.is_some()
                            || evidence.existing_target_version_token.is_some()
                            || evidence.desired_mode_bits.is_some()
                        {
                            return Err(
                                "WriteAuthority WAL refuză metadata v2 pe Directory legacy.".into(),
                            );
                        }
                    }
                    WAL_DIRECTORY_PROTOCOL_VERSION => {
                        let component_count = evidence.relative_components_hex.len();
                        if component_count == 0
                            || !matches!(
                                evidence.existing_prefix_len,
                                value if value == component_count || value + 1 == component_count
                            )
                            || evidence.parent_identity.is_none()
                            || evidence.target_leaf_hex.is_none()
                            || evidence.desired_mode_bits != Some(0o755)
                        {
                            return Err(
                                "WriteAuthority WAL refuză evidence Directory v2 incompletă."
                                    .into(),
                            );
                        }
                        let target = super::paths::decode_component_hex(
                            evidence.target_leaf_hex.as_deref().unwrap_or_default(),
                        )?;
                        if evidence
                            .relative_components_hex
                            .last()
                            .is_none_or(|component| {
                                super::paths::decode_component_hex(component).ok().as_ref()
                                    != Some(&target)
                            })
                        {
                            return Err(
                                "WriteAuthority WAL refuză leaf-ul Directory direct inconsistent."
                                    .into(),
                            );
                        }
                        let target_existed = evidence.existing_prefix_len == component_count;
                        if evidence.existing_target_identity_digest.is_some() != target_existed
                            || evidence.existing_target_version_token.is_some() != target_existed
                        {
                            return Err(
                                "WriteAuthority WAL refuză baseline-ul Directory v2 inconsistent."
                                    .into(),
                            );
                        }
                        let expected_ancestor = if target_existed {
                            evidence.existing_target_identity.as_ref()
                        } else {
                            evidence.parent_identity.as_ref()
                        };
                        if expected_ancestor != Some(&evidence.existing_ancestor_identity) {
                            return Err(
                                "WriteAuthority WAL refuză ancestor identity Directory v2 inconsistent."
                                    .into(),
                            );
                        }
                        if evidence
                            .existing_target_identity_digest
                            .as_deref()
                            .is_some_and(|digest| {
                                digest.len() != 32
                                    || !digest.bytes().all(|byte| {
                                        byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')
                                    })
                            })
                        {
                            return Err(
                                "WriteAuthority WAL refuză identity digest Directory v2 invalid."
                                    .into(),
                            );
                        }
                        let project_initializer = self.owner == "project_initializer"
                            && self.category == "project_source_write"
                            && self.recovery_policy == "logged_atomic_file"
                            && self.authority.scope.starts_with("project_bootstrap:");
                        let preview = self.owner == "preview"
                            && self.category == "preview_workspace_write"
                            && self.recovery_policy == "ephemeral_rebuildable"
                            && self.authority.scope == "application_preview_cache";
                        if !project_initializer && !preview {
                            return Err(
                                "WriteAuthority WAL refuză owner/category/scope/policy Directory v2."
                                    .into(),
                            );
                        }
                    }
                    version => {
                        return Err(format!(
                            "WriteAuthority WAL refuză protocolul Directory necunoscut {version}."
                        ));
                    }
                }
            }
            WalOperationEvidence::ExternalConfig(evidence) => {
                validate_atomic_file_evidence(&evidence.target, "external config target")?;
                if evidence.target.parent.existing_prefix_len
                    != evidence.target.parent.relative_components_hex.len()
                    || evidence.target.parent.parent_identity.is_none()
                    || evidence.target.new_size > MAX_WAL_EXTERNAL_CONFIG_BYTES
                    || evidence.target_new_mode_bits > 0o7777
                    || evidence
                        .target_before_mode_bits
                        .is_some_and(|mode| mode > 0o7777)
                    || evidence.backup_mode_bits.is_some_and(|mode| mode > 0o7777)
                {
                    return Err(
                        "WriteAuthority WAL refuză target evidence ExternalConfig invalidă.".into(),
                    );
                }
                match (&evidence.target.before, &evidence.backup) {
                    (WalLeafEvidence::Absent, None) if !evidence.target.replace => {}
                    (
                        WalLeafEvidence::Regular {
                            size,
                            version_token,
                            content_hash,
                            ..
                        },
                        Some(backup),
                    ) if evidence.target.replace => {
                        validate_atomic_file_evidence(backup, "external config backup")?;
                        if backup.parent != evidence.target.parent
                            || backup.parent.existing_prefix_len
                                != backup.parent.relative_components_hex.len()
                            || backup.parent.parent_identity.is_none()
                            || !matches!(backup.before, WalLeafEvidence::Absent)
                            || backup.replace
                            || backup.new_size != *size
                            || backup.new_content_hash != *content_hash
                            || backup.new_size > MAX_WAL_EXTERNAL_CONFIG_BYTES
                            || version_token.is_empty()
                            || version_token.len() > 512
                            || evidence.target_before_mode_bits.is_none()
                            || !matches!(
                                evidence.protocol_version,
                                0 | WAL_EXTERNAL_CONFIG_PROTOCOL_VERSION
                            )
                            || (evidence.protocol_version == 0
                                && evidence.target_before_identity_digest.is_some())
                            || (evidence.protocol_version == WAL_EXTERNAL_CONFIG_PROTOCOL_VERSION
                                && !evidence
                                    .target_before_identity_digest
                                    .as_deref()
                                    .is_some_and(is_external_identity_digest))
                            || evidence.backup_mode_bits != evidence.target_before_mode_bits
                            || evidence.target_new_mode_bits
                                != evidence.target_before_mode_bits.unwrap_or_default()
                        {
                            return Err(
                                "WriteAuthority WAL refuză perechea backup/target ExternalConfig invalidă."
                                    .into(),
                            );
                        }
                        let target_leaf =
                            super::paths::decode_component_hex(&evidence.target.target_leaf_hex)?;
                        let target_temp =
                            super::paths::decode_component_hex(&evidence.target.temp_leaf_hex)?;
                        let backup_leaf =
                            super::paths::decode_component_hex(&backup.target_leaf_hex)?;
                        let backup_temp =
                            super::paths::decode_component_hex(&backup.temp_leaf_hex)?;
                        if target_leaf == backup_leaf
                            || target_leaf == target_temp
                            || target_leaf == backup_temp
                            || backup_leaf == target_temp
                            || backup_leaf == backup_temp
                            || target_temp == backup_temp
                        {
                            return Err(
                                "WriteAuthority WAL refuză leaf-uri ExternalConfig suprapuse."
                                    .into(),
                            );
                        }
                    }
                    _ => {
                        return Err(
                            "WriteAuthority WAL cere backup exact pentru targetul ExternalConfig existent și interzice backupul la create-new."
                                .into(),
                        );
                    }
                }
                if evidence.backup.is_none()
                    && (!matches!(
                        evidence.protocol_version,
                        0 | WAL_EXTERNAL_CONFIG_PROTOCOL_VERSION
                    ) || evidence.target_before_mode_bits.is_some()
                        || evidence.target_before_identity_digest.is_some()
                        || evidence.backup_mode_bits.is_some()
                        || evidence.target_new_mode_bits != 0o600)
                {
                    return Err(
                        "WriteAuthority WAL cere mode 0600 pentru ExternalConfig create-new."
                            .into(),
                    );
                }
            }
            WalOperationEvidence::RemoveLeaf(evidence) => {
                validate_parent_evidence(&evidence.parent, "remove leaf")?;
                if evidence.parent.existing_prefix_len
                    != evidence.parent.relative_components_hex.len()
                    || evidence.parent.parent_identity.is_none()
                {
                    return Err(
                        "WriteAuthority WAL refuză un parent RemoveFile care nu exista integral."
                            .into(),
                    );
                }
                let target_leaf = super::paths::decode_component_hex(&evidence.target_leaf_hex)?;
                let quarantine_leaf =
                    super::paths::decode_component_hex(&evidence.quarantine_leaf_hex)?;
                if target_leaf == quarantine_leaf
                    || evidence.source.version_token.is_empty()
                    || evidence.source.version_token.len() > 512
                    || evidence.source.raw_mode > 0o177777
                    || evidence.source.link_count == 0
                    || !(0..1_000_000_000).contains(&evidence.source.mtime_nanoseconds)
                    || !(0..1_000_000_000).contains(&evidence.source.ctime_nanoseconds)
                    || evidence
                        .source
                        .content_hash
                        .as_deref()
                        .is_some_and(|hash| !valid_digest(hash))
                {
                    return Err("WriteAuthority WAL refuză evidence RemoveFile invalidă.".into());
                }
                let file_type = evidence.source.raw_mode & 0o170000;
                match evidence.source.kind {
                    WalRemoveLeafKind::Regular if file_type != 0o100000 => {
                        return Err(
                            "WriteAuthority WAL refuză kind regular cu mode divergent la RemoveFile."
                                .into(),
                        );
                    }
                    WalRemoveLeafKind::Symlink if file_type != 0o120000 => {
                        return Err(
                            "WriteAuthority WAL refuză kind symlink cu mode divergent la RemoveFile."
                                .into(),
                        );
                    }
                    WalRemoveLeafKind::Other
                        if matches!(file_type, 0o040000 | 0o100000 | 0o120000) =>
                    {
                        return Err(
                            "WriteAuthority WAL refuză kind other pentru directory/file/symlink la RemoveFile."
                                .into(),
                        );
                    }
                    _ => {}
                }
                if evidence.source.kind != WalRemoveLeafKind::Regular
                    && evidence.source.content_hash.is_some()
                {
                    return Err(
                        "WriteAuthority WAL refuză content hash pe RemoveFile non-regular.".into(),
                    );
                }
                match (
                    evidence.source.kind,
                    evidence.source.symlink_target_hex.as_deref(),
                ) {
                    (WalRemoveLeafKind::Symlink, Some(target_hex)) => {
                        let target = super::paths::decode_bytes_hex(target_hex)?;
                        if target.is_empty()
                            || target.len() > MAX_WAL_SYMLINK_TARGET_BYTES
                            || target.contains(&0)
                        {
                            return Err(
                                "WriteAuthority WAL refuză literalul symlink RemoveFile invalid."
                                    .into(),
                            );
                        }
                    }
                    (WalRemoveLeafKind::Symlink, None) => {
                        return Err(
                            "WriteAuthority WAL cere literalul symlink pentru RemoveFile.".into(),
                        );
                    }
                    (_, Some(_)) => {
                        return Err(
                            "WriteAuthority WAL refuză literal symlink pe alt kind RemoveFile."
                                .into(),
                        );
                    }
                    (_, None) => {}
                }
            }
            WalOperationEvidence::RemoveTree(evidence) => {
                validate_parent_evidence(&evidence.parent, "remove tree")?;
                if evidence.parent.existing_prefix_len
                    != evidence.parent.relative_components_hex.len()
                    || evidence.parent.parent_identity.is_none()
                {
                    return Err(
                        "WriteAuthority WAL refuză un parent RemoveDirectoryTree care nu exista integral."
                            .into(),
                    );
                }
                let target_leaf = super::paths::decode_component_hex(&evidence.target_leaf_hex)?;
                let quarantine_leaf =
                    super::paths::decode_component_hex(&evidence.quarantine_leaf_hex)?;
                if target_leaf == quarantine_leaf
                    || evidence.source.version_token.is_empty()
                    || evidence.source.version_token.len() > 512
                    || !valid_digest(&evidence.source.tree_fingerprint)
                    || evidence.source.raw_mode & 0o170000 != 0o040000
                    || evidence.source.raw_mode > 0o177777
                    || evidence.source.link_count == 0
                    || evidence.source.entry_count > 100_000
                    || evidence.source.mount_id == 0
                    || !(0..1_000_000_000).contains(&evidence.source.mtime_nanoseconds)
                    || !(0..1_000_000_000).contains(&evidence.source.ctime_nanoseconds)
                {
                    return Err(
                        "WriteAuthority WAL refuză evidence RemoveDirectoryTree invalidă.".into(),
                    );
                }
            }
            WalOperationEvidence::Rename(evidence) => {
                validate_parent_evidence(&evidence.source_parent, "rename source")?;
                validate_parent_evidence(&evidence.destination_parent, "rename destination")?;
                if evidence.source_parent.existing_prefix_len
                    != evidence.source_parent.relative_components_hex.len()
                    || evidence.source_parent.parent_identity.is_none()
                {
                    return Err(
                        "WriteAuthority WAL refuză un source parent rename care nu exista integral."
                            .into(),
                    );
                }
                let source_leaf = super::paths::decode_component_hex(&evidence.source_leaf_hex)?;
                let destination_leaf =
                    super::paths::decode_component_hex(&evidence.destination_leaf_hex)?;
                validate_authority_evidence(&evidence.destination_authority, "rename destination")?;
                if evidence.source.version_token.is_empty()
                    || evidence.source.version_token.len() > 512
                    || evidence.source.raw_mode > 0o177777
                    || evidence.source.link_count == 0
                    || !(0..1_000_000_000).contains(&evidence.source.mtime_nanoseconds)
                {
                    return Err("WriteAuthority WAL refuză source evidence rename invalidă.".into());
                }
                if evidence
                    .source
                    .content_hash
                    .as_deref()
                    .is_some_and(|hash| !valid_digest(hash))
                    || evidence
                        .source
                        .tree_fingerprint
                        .as_deref()
                        .is_some_and(|hash| !valid_digest(hash))
                {
                    return Err("WriteAuthority WAL refuză hash/fingerprint rename invalid.".into());
                }
                match evidence.source.kind {
                    WalRenameLeafKind::Regular if evidence.source.tree_fingerprint.is_some() => {
                        return Err(
                            "WriteAuthority WAL refuză tree fingerprint pe source file rename."
                                .into(),
                        );
                    }
                    WalRenameLeafKind::Directory
                        if evidence.source.tree_fingerprint.is_none()
                            || evidence.source.content_hash.is_some() =>
                    {
                        return Err(
                            "WriteAuthority WAL cere tree fingerprint exclusiv pentru source directory rename."
                                .into(),
                        );
                    }
                    WalRenameLeafKind::Symlink
                        if evidence.source.content_hash.is_some()
                            || evidence.source.tree_fingerprint.is_some() =>
                    {
                        return Err(
                            "WriteAuthority WAL refuză payload hash pe source symlink rename."
                                .into(),
                        );
                    }
                    _ => {}
                }
                if self.authority.identity == evidence.destination_authority.identity
                    && self.authority.boundary_path_hex
                        == evidence.destination_authority.boundary_path_hex
                    && evidence.source_parent.relative_components_hex
                        == evidence.destination_parent.relative_components_hex
                    && source_leaf == destination_leaf
                {
                    return Err("WriteAuthority WAL refuză rename source=destination.".into());
                }
            }
            WalOperationEvidence::Symlink(evidence) => {
                if evidence.parent.relative_components_hex.len() > 256
                    || evidence.parent.existing_prefix_len
                        > evidence.parent.relative_components_hex.len()
                {
                    return Err("WriteAuthority WAL refuză planul symlink invalid.".into());
                }
                for component in &evidence.parent.relative_components_hex {
                    super::paths::decode_component_hex(component)?;
                }
                super::paths::decode_component_hex(&evidence.target_leaf_hex)?;
                validate_symlink_target_hex(&evidence.desired_link_target_hex)?;
                if let WalSymlinkBefore::Exact {
                    link_target_hex,
                    identity_digest,
                    state_digest,
                    ..
                } = &evidence.before
                {
                    validate_symlink_target_hex(link_target_hex)?;
                    if evidence.protocol_version == WAL_SYMLINK_PROTOCOL_VERSION
                        && (!identity_digest
                            .as_deref()
                            .is_some_and(|value| is_lower_hex_digest(value, 32))
                            || !state_digest
                                .as_deref()
                                .is_some_and(|value| is_lower_hex_digest(value, 32)))
                    {
                        return Err(
                            "WriteAuthority WAL refuză baseline-ul Symlink v2 fără lifetime/state digest."
                                .into(),
                        );
                    }
                }
                match evidence.protocol_version {
                    0 => {
                        if matches!(
                            &evidence.before,
                            WalSymlinkBefore::Exact {
                                identity_digest: Some(_),
                                ..
                            } | WalSymlinkBefore::Exact {
                                state_digest: Some(_),
                                ..
                            }
                        ) {
                            return Err(
                                "WriteAuthority WAL refuză metadata v2 pe Symlink legacy.".into()
                            );
                        }
                    }
                    WAL_SYMLINK_PROTOCOL_VERSION => {
                        if matches!(
                            &evidence.before,
                            WalSymlinkBefore::Exact { link_target_hex, .. }
                                if link_target_hex != &evidence.desired_link_target_hex
                        ) {
                            return Err(
                                "WriteAuthority WAL refuză Symlink v2 Exact cu literal diferit de destinația dorită."
                                    .into(),
                            );
                        }
                        if evidence.parent.existing_prefix_len
                            != evidence.parent.relative_components_hex.len()
                            || evidence.parent.parent_identity.is_none()
                            || evidence.parent.parent_identity.as_ref()
                                != Some(&evidence.parent.existing_ancestor_identity)
                            || self.owner != "preview"
                            || self.category != "preview_workspace_write"
                            || self.recovery_policy != "ephemeral_rebuildable"
                            || self.authority.scope != "application_preview_cache"
                        {
                            return Err(
                                "WriteAuthority WAL refuză owner/category/scope/policy/parent Symlink v2."
                                    .into(),
                            );
                        }
                    }
                    version => {
                        return Err(format!(
                            "WriteAuthority WAL refuză protocolul Symlink necunoscut {version}."
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

fn is_lower_hex_digest(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn is_external_identity_digest(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WalRecord {
    pub body: WalRecordBody,
    pub evidence_hash: String,
}

pub(super) fn is_legacy_mcp_projection_record(record: &WalRecord) -> bool {
    let WalOperationEvidence::AtomicFile(evidence) = &record.body.operation_evidence else {
        return false;
    };
    let Ok(parents) = evidence
        .parent
        .relative_components_hex
        .iter()
        .map(|component| super::paths::decode_component_hex(component))
        .collect::<Result<Vec<_>, _>>()
    else {
        return false;
    };
    let Ok(target_leaf) = super::paths::decode_component_hex(&evidence.target_leaf_hex) else {
        return false;
    };
    let label_matches_path = match record.body.public_label.as_str() {
        "mcp/current-context.json" => target_leaf == "current-context.json",
        "mcp/mcp.json" => target_leaf == "mcp.json",
        _ => false,
    };

    record.body.category == "internal_app_write"
        && record.body.owner == "mcp_context"
        && record.body.operation == "write_text"
        && record.body.recovery_policy == "logged_atomic_file"
        && record.body.authority.scope == "application_config"
        && parents.len() == 1
        && parents[0] == "mcp"
        && evidence.parent.existing_prefix_len == parents.len()
        && label_matches_path
}

impl WalRecord {
    pub(crate) fn seal(body: WalRecordBody) -> Result<Self, String> {
        body.validate()?;
        let evidence_hash = evidence_hash(&body)?;
        Ok(Self {
            body,
            evidence_hash,
        })
    }

    pub(super) fn validate(&self) -> Result<(), String> {
        self.body.validate()?;
        let expected = evidence_hash(&self.body)?;
        if self.evidence_hash != expected {
            return Err(format!(
                "WriteAuthority WAL evidence hash diferă (expected {expected}, observed {}).",
                self.evidence_hash
            ));
        }
        Ok(())
    }

    pub(super) fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let mut bytes = serde_json::to_vec(self)
            .map_err(|error| format!("WriteAuthority WAL nu poate serializa recordul: {error}"))?;
        bytes.push(b'\n');
        if bytes.len() > MAX_WAL_RECORD_BYTES {
            return Err(format!(
                "WriteAuthority WAL record depășește limita de {} bytes.",
                MAX_WAL_RECORD_BYTES
            ));
        }
        Ok(bytes)
    }

    pub(super) fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > MAX_WAL_RECORD_BYTES {
            return Err(format!(
                "WriteAuthority WAL record are dimensiune invalidă: {} bytes.",
                bytes.len()
            ));
        }
        let record: Self = serde_json::from_slice(bytes)
            .map_err(|error| format!("WriteAuthority WAL record JSON invalid: {error}"))?;
        record.validate()?;
        Ok(record)
    }
}

fn evidence_hash(body: &WalRecordBody) -> Result<String, String> {
    serde_json::to_vec(body)
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
        .map_err(|error| format!("WriteAuthority WAL nu poate calcula evidence hash: {error}"))
}

fn validate_parent_evidence(evidence: &WalParentEvidence, role: &str) -> Result<(), String> {
    if evidence.relative_components_hex.len() > 256
        || evidence.existing_prefix_len > evidence.relative_components_hex.len()
        || evidence.parent_identity.is_some()
            != (evidence.existing_prefix_len == evidence.relative_components_hex.len())
    {
        return Err(format!(
            "WriteAuthority WAL refuză parent evidence invalidă pentru {role}."
        ));
    }
    for component in &evidence.relative_components_hex {
        super::paths::decode_component_hex(component)?;
    }
    Ok(())
}

fn validate_atomic_file_evidence(
    evidence: &WalAtomicFileEvidence,
    role: &str,
) -> Result<(), String> {
    validate_parent_evidence(&evidence.parent, role)?;
    let target_leaf = super::paths::decode_component_hex(&evidence.target_leaf_hex)?;
    let temp_leaf = super::paths::decode_component_hex(&evidence.temp_leaf_hex)?;
    if target_leaf == temp_leaf
        || evidence.new_content_hash.len() != 64
        || !evidence
            .new_content_hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(format!(
            "WriteAuthority WAL refuză evidence atomică invalidă pentru {role}."
        ));
    }
    if let WalLeafEvidence::Regular {
        version_token,
        content_hash,
        ..
    } = &evidence.before
    {
        if version_token.is_empty()
            || version_token.len() > 512
            || content_hash.len() != 64
            || !content_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(format!(
                "WriteAuthority WAL refuză baseline atomic invalid pentru {role}."
            ));
        }
    }
    Ok(())
}

fn validate_authority_evidence(evidence: &WalAuthorityEvidence, role: &str) -> Result<(), String> {
    if evidence.scope.is_empty()
        || evidence.scope.len() > 128
        || evidence.boundary_display.len() > 4096
    {
        return Err(format!(
            "WriteAuthority WAL refuză authority evidence invalidă pentru {role}."
        ));
    }
    let path = super::paths::decode_path_hex(&evidence.boundary_path_hex)?;
    if !path.is_absolute()
        || !path.components().enumerate().all(|(index, component)| {
            matches!(component, Component::RootDir) && index == 0
                || matches!(component, Component::Normal(_)) && index > 0
        })
    {
        return Err(format!(
            "WriteAuthority WAL refuză authority path non-canonic pentru {role}."
        ));
    }
    Ok(())
}

fn valid_digest(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.len() % 2 == 0
        && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn validate_symlink_target_hex(value: &str) -> Result<(), String> {
    let bytes = super::paths::decode_bytes_hex(value)?;
    if bytes.is_empty() || bytes.len() > MAX_WAL_SYMLINK_TARGET_BYTES || bytes.contains(&0) {
        return Err("WriteAuthority WAL refuză target-ul literal al symlink-ului.".into());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteAuthorityRecoveryClassification {
    NoEffect,
    StagedOnly,
    EffectCommitted,
    RollbackCompleted,
    CleanupRequired,
    PartialAppend,
    PartialNamespaceCreation,
    PartialTreeRemoval,
    Conflict,
    UnreadableOrCorrupt,
}

pub const WRITE_AUTHORITY_RECOVERY_RESOLUTION_SCHEMA_VERSION: u32 = 6;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteAuthorityRecoveryResolutionAction {
    RestoreOriginal,
    AcceptRestoredState,
    AcceptCurrentState,
    ContinueTreeRemoval,
    RestoreRemainingTree,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteAuthorityRecoveryResolutionInput {
    pub operation_id: String,
    pub expected_phase: WalPhase,
    pub evidence_hash: String,
    pub action: WriteAuthorityRecoveryResolutionAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AtomicRecoveryAction {
    ClearNoEffect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AtomicRecoveryAssessment {
    pub classification: WriteAuthorityRecoveryClassification,
    pub automatic_action: Option<AtomicRecoveryAction>,
    pub diagnostic: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AppendRecoveryAction {
    ClearNoEffect,
    ContinueExactPrefix,
    FinalizeCommitted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AppendRecoveryAssessment {
    pub classification: WriteAuthorityRecoveryClassification,
    pub automatic_action: Option<AppendRecoveryAction>,
    pub diagnostic: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CopyRecoveryAction {
    ClearNoEffect,
    CommitStagedReplace,
    FinalizeCommitted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CopyRecoveryAssessment {
    pub classification: WriteAuthorityRecoveryClassification,
    pub automatic_action: Option<CopyRecoveryAction>,
    pub diagnostic: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExternalConfigRecoveryAction {
    ClearNoEffect,
    FinalizeAbsentTarget,
    RestoreBaselineToTarget,
    FinalizeRestoredBaseline,
    FinalizeCommitted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExternalConfigRecoveryAssessment {
    pub classification: WriteAuthorityRecoveryClassification,
    pub automatic_action: Option<ExternalConfigRecoveryAction>,
    pub available_resolution_actions: Vec<WriteAuthorityRecoveryResolutionAction>,
    pub diagnostic: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RenameRecoveryAction {
    ClearNoEffect,
    FinalizeCommitted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RenameRecoveryAssessment {
    pub classification: WriteAuthorityRecoveryClassification,
    pub automatic_action: Option<RenameRecoveryAction>,
    pub diagnostic: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RemoveLeafRecoveryAction {
    ClearNoEffect,
    FinalizeCommitted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RemoveLeafRecoveryAssessment {
    pub classification: WriteAuthorityRecoveryClassification,
    pub automatic_action: Option<RemoveLeafRecoveryAction>,
    pub diagnostic: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RemoveTreeRecoveryAction {
    ClearNoEffect,
    FinalizeCommitted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RemoveTreeRecoveryAssessment {
    pub classification: WriteAuthorityRecoveryClassification,
    pub automatic_action: Option<RemoveTreeRecoveryAction>,
    pub available_resolution_actions: Vec<WriteAuthorityRecoveryResolutionAction>,
    pub diagnostic: String,
}

#[derive(Debug)]
pub(crate) struct RecoveryReadBudget {
    remaining_bytes: u64,
}

impl RecoveryReadBudget {
    pub(crate) const fn new() -> Self {
        Self {
            remaining_bytes: MAX_WAL_RECOVERY_READ_BYTES,
        }
    }

    pub(crate) fn reserve(&mut self, bytes: u64, role: &str) -> Result<(), String> {
        if bytes > self.remaining_bytes {
            return Err(format!(
                "WriteAuthority recovery depășește bugetul agregat de citire la {role}: necesar {bytes}, rămas {}.",
                self.remaining_bytes
            ));
        }
        self.remaining_bytes -= bytes;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) const fn with_limit(limit: u64) -> Self {
        Self {
            remaining_bytes: limit,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DirectoryRecoveryAction {
    ClearNoEffect,
    FinalizeCommitted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DirectoryResolutionStateBinding {
    pub identity_digest: String,
    pub state_digest: String,
}

impl DirectoryResolutionStateBinding {
    pub(crate) fn evidence_hash(&self, wal_evidence_binding_hash: &str) -> String {
        let mut digest = Sha256::new();
        digest.update(b"pana-directory-current-state-resolution-v1\0");
        digest.update(wal_evidence_binding_hash.as_bytes());
        digest.update(b"\0");
        digest.update(self.identity_digest.as_bytes());
        digest.update(b"\0");
        digest.update(self.state_digest.as_bytes());
        format!("{:x}", digest.finalize())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DirectoryRecoveryAssessment {
    pub classification: WriteAuthorityRecoveryClassification,
    pub automatic_action: Option<DirectoryRecoveryAction>,
    pub available_resolution_actions: Vec<WriteAuthorityRecoveryResolutionAction>,
    pub resolution_state_binding: Option<DirectoryResolutionStateBinding>,
    pub diagnostic: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SymlinkRecoveryAction {
    ClearNoEffect,
    FinalizeCommitted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SymlinkResolutionStateBinding {
    pub identity_digest: String,
    pub state_digest: String,
}

impl SymlinkResolutionStateBinding {
    pub(crate) fn evidence_hash(&self, wal_evidence_binding_hash: &str) -> String {
        let mut digest = Sha256::new();
        digest.update(b"pana-symlink-current-state-resolution-v2\0");
        digest.update(wal_evidence_binding_hash.as_bytes());
        digest.update(b"\0");
        digest.update(self.identity_digest.as_bytes());
        digest.update(b"\0");
        digest.update(self.state_digest.as_bytes());
        format!("{:x}", digest.finalize())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SymlinkRecoveryAssessment {
    pub classification: WriteAuthorityRecoveryClassification,
    pub automatic_action: Option<SymlinkRecoveryAction>,
    pub available_resolution_actions: Vec<WriteAuthorityRecoveryResolutionAction>,
    pub resolution_state_binding: Option<SymlinkResolutionStateBinding>,
    pub diagnostic: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteAuthorityRecoveryItem {
    pub file_name: String,
    pub operation_id: Option<String>,
    pub phase: Option<WalPhase>,
    pub classification: WriteAuthorityRecoveryClassification,
    pub automatic_recovery_available: bool,
    pub evidence_hash: Option<String>,
    pub available_resolution_actions: Vec<WriteAuthorityRecoveryResolutionAction>,
    pub diagnostic: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteAuthorityRecoveryScan {
    pub schema_version: u32,
    pub scanned_at_ms: u128,
    pub blocked: bool,
    pub record_count: usize,
    pub total_bytes: usize,
    pub items: Vec<WriteAuthorityRecoveryItem>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteAuthorityRecoveryResolutionReceipt {
    pub schema_version: u32,
    pub operation_id: String,
    pub action: WriteAuthorityRecoveryResolutionAction,
    pub diagnostic: String,
    pub recovery_scan: WriteAuthorityRecoveryScan,
}

#[cfg(test)]
mod protocol_compatibility_tests {
    use serde_json::json;

    use super::{WalCopyEvidence, WAL_COPY_PROTOCOL_VERSION};

    fn copy_evidence_json() -> serde_json::Value {
        let hash = "0".repeat(64);
        json!({
            "file": {
                "parent": {
                    "relativeComponentsHex": [],
                    "existingPrefixLen": 0,
                    "existingAncestorIdentity": { "device": 1, "inode": 2 },
                    "parentIdentity": { "device": 1, "inode": 2 }
                },
                "targetLeafHex": "74",
                "tempLeafHex": "78",
                "before": { "kind": "absent" },
                "newSize": 0,
                "newContentHash": hash,
                "replace": false
            },
            "source": {
                "pathHex": "2f73",
                "identity": { "device": 3, "inode": 4 },
                "size": 0,
                "versionToken": "source-v1",
                "contentHash": "0".repeat(64),
                "modeBits": 384,
                "linkCount": 1
            },
            "destinationPolicy": "create_new",
            "beforeModeBits": null,
            "newModeBits": 384
        })
    }

    #[test]
    fn copy_evidence_without_protocol_version_deserializes_as_legacy_zero() {
        let evidence: WalCopyEvidence = serde_json::from_value(copy_evidence_json()).unwrap();
        assert_eq!(evidence.protocol_version, 0);
    }

    #[test]
    fn copy_evidence_v2_serializes_explicit_protocol_version() {
        let mut evidence: WalCopyEvidence = serde_json::from_value(copy_evidence_json()).unwrap();
        evidence.protocol_version = WAL_COPY_PROTOCOL_VERSION;
        let serialized = serde_json::to_value(evidence).unwrap();
        assert_eq!(
            serialized
                .get("protocolVersion")
                .and_then(|value| value.as_u64()),
            Some(u64::from(WAL_COPY_PROTOCOL_VERSION))
        );
    }
}
