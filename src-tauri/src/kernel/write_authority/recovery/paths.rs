use std::{ffi::OsString, path::PathBuf};

#[cfg(target_os = "linux")]
use std::os::unix::ffi::{OsStrExt, OsStringExt};

use sha2::{Digest, Sha256};

use super::model::{WalOperationEvidence, WalPhase};

pub(super) const WAL_LOCK_FILE: &str = "coordinator.lock";
const MAX_WAL_FILE_NAME_BYTES: usize = 255;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WalAppendStageRole {
    CreateTarget,
    ExistingTarget,
}

impl WalAppendStageRole {
    const fn token(self) -> &'static str {
        match self {
            Self::CreateTarget => "c",
            Self::ExistingTarget => "e",
        }
    }

    fn parse(token: &str) -> Option<Self> {
        match token {
            "c" => Some(Self::CreateTarget),
            "e" => Some(Self::ExistingTarget),
            _ => None,
        }
    }
}

/// Checkpoint pre-efect Append v2. Identitatea este lifetime-bound prin statx,
/// iar contractul leagă rolul, before-size și payloadul complet din record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WalAppendStageCheckpoint {
    pub target_identity_digest: String,
    pub payload_contract_digest: String,
    pub role: WalAppendStageRole,
}

/// Checkpoint post-publicare pentru Directory v2 direct. Digesturile leagă
/// lifetime-ul statx și starea exactă a targetului gol deja durabil; recordul
/// immutable leagă parentul, leaf-ul final și mode-ul.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WalDirectoryStageCheckpoint {
    pub target_identity_digest: String,
    pub target_state_digest: String,
}

/// Checkpoint post-publicare pentru Symlink v2 direct. Digesturile leagă
/// lifetime-ul statx al inode-ului symlink și starea completă, inclusiv
/// literalul raw, de filename-ul WAL înainte de EffectVisible.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WalSymlinkStageCheckpoint {
    pub target_identity_digest: String,
    pub target_state_digest: String,
}

impl WalSymlinkStageCheckpoint {
    pub(crate) fn new(
        target_identity_digest: String,
        target_state_digest: String,
    ) -> Result<Self, String> {
        validate_symlink_identity_digest(&target_identity_digest)?;
        validate_symlink_state_digest(&target_state_digest)?;
        Ok(Self {
            target_identity_digest,
            target_state_digest,
        })
    }
}

impl WalDirectoryStageCheckpoint {
    pub(crate) fn new(
        target_identity_digest: String,
        target_state_digest: String,
    ) -> Result<Self, String> {
        validate_directory_identity_digest(&target_identity_digest)?;
        validate_directory_state_digest(&target_state_digest)?;
        Ok(Self {
            target_identity_digest,
            target_state_digest,
        })
    }
}

impl WalAppendStageCheckpoint {
    pub(crate) fn new(
        target_identity_digest: String,
        expected_content_hash: &str,
        expected_size: u64,
        before_size: u64,
        role: WalAppendStageRole,
    ) -> Result<Self, String> {
        validate_append_identity_digest(&target_identity_digest)?;
        let payload_contract_digest = append_payload_contract_digest(
            expected_content_hash,
            expected_size,
            before_size,
            role,
        )?;
        Ok(Self {
            target_identity_digest,
            payload_contract_digest,
            role,
        })
    }

    pub(crate) fn matches_payload_contract(
        &self,
        expected_content_hash: &str,
        expected_size: u64,
        before_size: u64,
        role: WalAppendStageRole,
    ) -> bool {
        self.role == role
            && append_payload_contract_digest(
                expected_content_hash,
                expected_size,
                before_size,
                role,
            )
            .is_ok_and(|digest| digest == self.payload_contract_digest)
    }

    fn from_persisted(
        target_identity_digest: String,
        payload_contract_digest: String,
        role: WalAppendStageRole,
    ) -> Result<Self, String> {
        validate_append_identity_digest(&target_identity_digest)?;
        validate_append_contract_digest(&payload_contract_digest)?;
        Ok(Self {
            target_identity_digest,
            payload_contract_digest,
            role,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WalCopyStageRole {
    CreateTarget,
    ReplaceTemporary,
}

impl WalCopyStageRole {
    const fn token(self) -> &'static str {
        match self {
            Self::CreateTarget => "c",
            Self::ReplaceTemporary => "r",
        }
    }

    fn parse(token: &str) -> Option<Self> {
        match token {
            "c" => Some(Self::CreateTarget),
            "r" => Some(Self::ReplaceTemporary),
            _ => None,
        }
    }
}

/// Checkpointul Copy nu duplică payloadul în filename. El persistă identitatea
/// lifetime a inode-ului staged și un digest domain-separated peste contractul
/// immutable deja prezent în record: rol, SHA-256, size și mode. Astfel numele
/// rămâne sub NAME_MAX chiar la operation ID-ul maxim. Recovery folosește
/// digestul pentru binding structural, apoi confirmă SHA-256 prin citire
/// streaming bounded înainte de orice finalizare automată a payloadului.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WalCopyStageCheckpoint {
    pub staged_identity_digest: String,
    pub payload_contract_digest: String,
    pub role: WalCopyStageRole,
}

impl WalCopyStageCheckpoint {
    pub(crate) fn new(
        staged_identity_digest: String,
        expected_content_hash: &str,
        expected_size: u64,
        expected_mode_bits: u32,
        role: WalCopyStageRole,
    ) -> Result<Self, String> {
        validate_copy_identity_digest(&staged_identity_digest)?;
        let payload_contract_digest = copy_payload_contract_digest(
            expected_content_hash,
            expected_size,
            expected_mode_bits,
            role,
        )?;
        Ok(Self {
            staged_identity_digest,
            payload_contract_digest,
            role,
        })
    }

    pub(crate) fn matches_payload_contract(
        &self,
        expected_content_hash: &str,
        expected_size: u64,
        expected_mode_bits: u32,
        role: WalCopyStageRole,
    ) -> bool {
        self.role == role
            && copy_payload_contract_digest(
                expected_content_hash,
                expected_size,
                expected_mode_bits,
                role,
            )
            .is_ok_and(|digest| digest == self.payload_contract_digest)
    }

    fn from_persisted(
        staged_identity_digest: String,
        payload_contract_digest: String,
        role: WalCopyStageRole,
    ) -> Result<Self, String> {
        validate_copy_identity_digest(&staged_identity_digest)?;
        validate_copy_contract_digest(&payload_contract_digest)?;
        Ok(Self {
            staged_identity_digest,
            payload_contract_digest,
            role,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WalExternalStageCheckpoint {
    pub target_identity_digest: String,
    pub backup_identity_digest: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WalExternalOperatorDecision {
    ContinueUpdate,
    AcceptBackup,
}

impl WalExternalOperatorDecision {
    const fn token(self) -> &'static str {
        match self {
            Self::ContinueUpdate => "c",
            Self::AcceptBackup => "a",
        }
    }

    fn parse(token: &str) -> Option<Self> {
        match token {
            "c" => Some(Self::ContinueUpdate),
            "a" => Some(Self::AcceptBackup),
            _ => None,
        }
    }
}

impl WalExternalStageCheckpoint {
    pub(crate) fn new(
        target_identity_digest: String,
        backup_identity_digest: Option<String>,
    ) -> Result<Self, String> {
        validate_identity_digest(&target_identity_digest)?;
        if let Some(backup) = backup_identity_digest.as_deref() {
            validate_identity_digest(backup)?;
        }
        Ok(Self {
            target_identity_digest,
            backup_identity_digest,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WalRecordName {
    pub operation_id: String,
    pub phase: WalPhase,
    pub file_name: String,
    pub append_stage_checkpoint: Option<WalAppendStageCheckpoint>,
    pub copy_stage_checkpoint: Option<WalCopyStageCheckpoint>,
    pub directory_stage_checkpoint: Option<WalDirectoryStageCheckpoint>,
    pub symlink_stage_checkpoint: Option<WalSymlinkStageCheckpoint>,
    pub external_stage_checkpoint: Option<WalExternalStageCheckpoint>,
    pub external_operator_decision: Option<WalExternalOperatorDecision>,
}

impl WalRecordName {
    pub(super) fn validate_family_metadata(
        &self,
        evidence: &WalOperationEvidence,
    ) -> Result<(), String> {
        let append_metadata = self.append_stage_checkpoint.is_some();
        let copy_metadata = self.copy_stage_checkpoint.is_some();
        let directory_metadata = self.directory_stage_checkpoint.is_some();
        let symlink_metadata = self.symlink_stage_checkpoint.is_some();
        let external_metadata =
            self.external_stage_checkpoint.is_some() || self.external_operator_decision.is_some();
        let compatible = match evidence {
            WalOperationEvidence::Append(evidence) => {
                !copy_metadata
                    && !directory_metadata
                    && !symlink_metadata
                    && !external_metadata
                    && (!append_metadata
                        || evidence.protocol_version == super::model::WAL_APPEND_PROTOCOL_VERSION)
            }
            WalOperationEvidence::Copy(_) => {
                !append_metadata && !directory_metadata && !symlink_metadata && !external_metadata
            }
            WalOperationEvidence::Directory(evidence) => {
                !append_metadata
                    && !copy_metadata
                    && !symlink_metadata
                    && !external_metadata
                    && (!directory_metadata
                        || evidence.protocol_version
                            == super::model::WAL_DIRECTORY_PROTOCOL_VERSION)
            }
            WalOperationEvidence::ExternalConfig(_) => {
                !append_metadata && !copy_metadata && !directory_metadata && !symlink_metadata
            }
            WalOperationEvidence::Symlink(evidence) => {
                !append_metadata
                    && !copy_metadata
                    && !directory_metadata
                    && !external_metadata
                    && (!symlink_metadata
                        || evidence.protocol_version == super::model::WAL_SYMLINK_PROTOCOL_VERSION)
            }
            WalOperationEvidence::AtomicFile(_)
            | WalOperationEvidence::RemoveLeaf(_)
            | WalOperationEvidence::RemoveTree(_)
            | WalOperationEvidence::Rename(_) => {
                !append_metadata
                    && !copy_metadata
                    && !directory_metadata
                    && !symlink_metadata
                    && !external_metadata
            }
        };
        if compatible {
            Ok(())
        } else {
            Err(format!(
                "WriteAuthority WAL refuză metadata filename incompatibilă cu familia recordului {}.",
                self.file_name
            ))
        }
    }

    pub(super) fn new(operation_id: &str, phase: WalPhase) -> Result<Self, String> {
        validate_operation_id(operation_id)?;
        let suffix = match phase {
            WalPhase::Preparing => "preparing",
            WalPhase::Prepared => "prepared.json",
            WalPhase::AuxiliaryDurable => "auxiliary-durable.json",
            WalPhase::EffectVisible => "effect-visible.json",
            WalPhase::TargetDurable => "target-durable.json",
        };
        let file_name = format!("{operation_id}.{suffix}");
        validate_wal_file_name_length(&file_name)?;
        Ok(Self {
            operation_id: operation_id.to_string(),
            phase,
            file_name,
            append_stage_checkpoint: None,
            copy_stage_checkpoint: None,
            directory_stage_checkpoint: None,
            symlink_stage_checkpoint: None,
            external_stage_checkpoint: None,
            external_operator_decision: None,
        })
    }

    pub(super) fn with_external_stage_checkpoint(
        operation_id: &str,
        phase: WalPhase,
        checkpoint: WalExternalStageCheckpoint,
    ) -> Result<Self, String> {
        Self::with_external_metadata(operation_id, phase, checkpoint, None)
    }

    pub(super) fn with_copy_stage_checkpoint(
        operation_id: &str,
        phase: WalPhase,
        checkpoint: WalCopyStageCheckpoint,
    ) -> Result<Self, String> {
        validate_operation_id(operation_id)?;
        let phase_name = checkpoint_phase_name(phase, "Copy")?;
        validate_copy_identity_digest(&checkpoint.staged_identity_digest)?;
        validate_copy_contract_digest(&checkpoint.payload_contract_digest)?;
        let file_name = format!(
            "{operation_id}.{phase_name}.cp{}.{}.{}.json",
            checkpoint.role.token(),
            checkpoint.staged_identity_digest,
            checkpoint.payload_contract_digest,
        );
        validate_wal_file_name_length(&file_name)?;
        Ok(Self {
            operation_id: operation_id.to_string(),
            phase,
            file_name,
            append_stage_checkpoint: None,
            copy_stage_checkpoint: Some(checkpoint),
            directory_stage_checkpoint: None,
            symlink_stage_checkpoint: None,
            external_stage_checkpoint: None,
            external_operator_decision: None,
        })
    }

    pub(super) fn with_append_stage_checkpoint(
        operation_id: &str,
        phase: WalPhase,
        checkpoint: WalAppendStageCheckpoint,
    ) -> Result<Self, String> {
        validate_operation_id(operation_id)?;
        let phase_name = checkpoint_phase_name(phase, "Append")?;
        validate_append_identity_digest(&checkpoint.target_identity_digest)?;
        validate_append_contract_digest(&checkpoint.payload_contract_digest)?;
        let file_name = format!(
            "{operation_id}.{phase_name}.ap{}.{}.{}.json",
            checkpoint.role.token(),
            checkpoint.target_identity_digest,
            checkpoint.payload_contract_digest,
        );
        validate_wal_file_name_length(&file_name)?;
        Ok(Self {
            operation_id: operation_id.to_string(),
            phase,
            file_name,
            append_stage_checkpoint: Some(checkpoint),
            copy_stage_checkpoint: None,
            directory_stage_checkpoint: None,
            symlink_stage_checkpoint: None,
            external_stage_checkpoint: None,
            external_operator_decision: None,
        })
    }

    pub(super) fn with_directory_stage_checkpoint(
        operation_id: &str,
        phase: WalPhase,
        checkpoint: WalDirectoryStageCheckpoint,
    ) -> Result<Self, String> {
        validate_operation_id(operation_id)?;
        let phase_name = checkpoint_phase_name(phase, "Directory")?;
        validate_directory_identity_digest(&checkpoint.target_identity_digest)?;
        validate_directory_state_digest(&checkpoint.target_state_digest)?;
        let file_name = format!(
            "{operation_id}.{phase_name}.dr.{}.{}.json",
            checkpoint.target_identity_digest, checkpoint.target_state_digest,
        );
        validate_wal_file_name_length(&file_name)?;
        Ok(Self {
            operation_id: operation_id.to_string(),
            phase,
            file_name,
            append_stage_checkpoint: None,
            copy_stage_checkpoint: None,
            directory_stage_checkpoint: Some(checkpoint),
            symlink_stage_checkpoint: None,
            external_stage_checkpoint: None,
            external_operator_decision: None,
        })
    }

    pub(super) fn with_symlink_stage_checkpoint(
        operation_id: &str,
        phase: WalPhase,
        checkpoint: WalSymlinkStageCheckpoint,
    ) -> Result<Self, String> {
        validate_operation_id(operation_id)?;
        let phase_name = checkpoint_phase_name(phase, "Symlink")?;
        validate_symlink_identity_digest(&checkpoint.target_identity_digest)?;
        validate_symlink_state_digest(&checkpoint.target_state_digest)?;
        let file_name = format!(
            "{operation_id}.{phase_name}.sl.{}.{}.json",
            checkpoint.target_identity_digest, checkpoint.target_state_digest,
        );
        validate_wal_file_name_length(&file_name)?;
        Ok(Self {
            operation_id: operation_id.to_string(),
            phase,
            file_name,
            append_stage_checkpoint: None,
            copy_stage_checkpoint: None,
            directory_stage_checkpoint: None,
            symlink_stage_checkpoint: Some(checkpoint),
            external_stage_checkpoint: None,
            external_operator_decision: None,
        })
    }

    fn with_external_metadata(
        operation_id: &str,
        phase: WalPhase,
        checkpoint: WalExternalStageCheckpoint,
        decision: Option<WalExternalOperatorDecision>,
    ) -> Result<Self, String> {
        validate_operation_id(operation_id)?;
        let phase_name = match phase {
            WalPhase::AuxiliaryDurable => "auxiliary-durable",
            WalPhase::EffectVisible => "effect-visible",
            WalPhase::TargetDurable => "target-durable",
            WalPhase::Preparing | WalPhase::Prepared => {
                return Err(
                    "Checkpointul ExternalConfig este permis numai din AuxiliaryDurable.".into(),
                );
            }
        };
        validate_identity_digest(&checkpoint.target_identity_digest)?;
        let backup = checkpoint
            .backup_identity_digest
            .as_deref()
            .unwrap_or("none");
        if backup != "none" {
            validate_identity_digest(backup)?;
        }
        let decision_suffix = decision
            .map(|value| format!(".{}", value.token()))
            .unwrap_or_default();
        let file_name = format!(
            "{operation_id}.{phase_name}.{}.{}{decision_suffix}.json",
            checkpoint.target_identity_digest, backup
        );
        validate_wal_file_name_length(&file_name)?;
        Ok(Self {
            operation_id: operation_id.to_string(),
            phase,
            file_name,
            append_stage_checkpoint: None,
            copy_stage_checkpoint: None,
            directory_stage_checkpoint: None,
            symlink_stage_checkpoint: None,
            external_stage_checkpoint: Some(checkpoint),
            external_operator_decision: decision,
        })
    }

    pub(super) fn successor(&self, phase: WalPhase) -> Result<Self, String> {
        if self.phase.next() != Some(phase) {
            return Err("WriteAuthority WAL a refuzat un successor non-monoton.".into());
        }
        match (
            self.append_stage_checkpoint.clone(),
            self.copy_stage_checkpoint.clone(),
            self.directory_stage_checkpoint.clone(),
            self.symlink_stage_checkpoint.clone(),
            self.external_stage_checkpoint.clone(),
        ) {
            (Some(checkpoint), None, None, None, None) => {
                Self::with_append_stage_checkpoint(&self.operation_id, phase, checkpoint)
            }
            (None, Some(checkpoint), None, None, None) => {
                Self::with_copy_stage_checkpoint(&self.operation_id, phase, checkpoint)
            }
            (None, None, Some(checkpoint), None, None) => {
                Self::with_directory_stage_checkpoint(&self.operation_id, phase, checkpoint)
            }
            (None, None, None, Some(checkpoint), None) => {
                Self::with_symlink_stage_checkpoint(&self.operation_id, phase, checkpoint)
            }
            (None, None, None, None, Some(checkpoint)) => Self::with_external_metadata(
                &self.operation_id,
                phase,
                checkpoint,
                self.external_operator_decision,
            ),
            (None, None, None, None, None) => Self::new(&self.operation_id, phase),
            _ => Err("WriteAuthority WAL refuză checkpointuri de familie simultane.".into()),
        }
    }

    pub(super) fn evidence_binding_hash(&self, record_evidence_hash: &str) -> String {
        if let Some(checkpoint) = &self.append_stage_checkpoint {
            let mut hash = Sha256::new();
            hash.update(b"pana-wal-append-filename-binding-v2\0");
            hash.update(record_evidence_hash.as_bytes());
            hash.update(b"\0");
            hash.update(checkpoint.role.token().as_bytes());
            hash.update(b"\0");
            hash.update(checkpoint.target_identity_digest.as_bytes());
            hash.update(b"\0");
            hash.update(checkpoint.payload_contract_digest.as_bytes());
            return format!("{:x}", hash.finalize());
        }
        if let Some(checkpoint) = &self.copy_stage_checkpoint {
            let mut hash = Sha256::new();
            hash.update(b"pana-wal-copy-filename-binding-v1\0");
            hash.update(record_evidence_hash.as_bytes());
            hash.update(b"\0");
            hash.update(checkpoint.role.token().as_bytes());
            hash.update(b"\0");
            hash.update(checkpoint.staged_identity_digest.as_bytes());
            hash.update(b"\0");
            hash.update(checkpoint.payload_contract_digest.as_bytes());
            return format!("{:x}", hash.finalize());
        }
        if let Some(checkpoint) = &self.directory_stage_checkpoint {
            let mut hash = Sha256::new();
            hash.update(b"pana-wal-directory-direct-filename-binding-v3\0");
            hash.update(record_evidence_hash.as_bytes());
            hash.update(b"\0");
            hash.update(checkpoint.target_identity_digest.as_bytes());
            hash.update(b"\0");
            hash.update(checkpoint.target_state_digest.as_bytes());
            return format!("{:x}", hash.finalize());
        }
        if let Some(checkpoint) = &self.symlink_stage_checkpoint {
            let mut hash = Sha256::new();
            hash.update(b"pana-wal-symlink-direct-filename-binding-v2\0");
            hash.update(record_evidence_hash.as_bytes());
            hash.update(b"\0");
            hash.update(checkpoint.target_identity_digest.as_bytes());
            hash.update(b"\0");
            hash.update(checkpoint.target_state_digest.as_bytes());
            return format!("{:x}", hash.finalize());
        }
        let Some(checkpoint) = &self.external_stage_checkpoint else {
            return record_evidence_hash.to_string();
        };
        let mut hash = Sha256::new();
        hash.update(b"pana-wal-filename-binding-v1\0");
        hash.update(record_evidence_hash.as_bytes());
        hash.update(b"\0");
        hash.update(checkpoint.target_identity_digest.as_bytes());
        hash.update(b"\0");
        hash.update(
            checkpoint
                .backup_identity_digest
                .as_deref()
                .unwrap_or("none")
                .as_bytes(),
        );
        hash.update(b"\0");
        hash.update(
            self.external_operator_decision
                .map(WalExternalOperatorDecision::token)
                .unwrap_or("none")
                .as_bytes(),
        );
        format!("{:x}", hash.finalize())
    }

    pub(super) fn parse(file_name: &str) -> Result<Self, String> {
        if let Some(stem) = file_name.strip_suffix(".json") {
            let parts = stem.split('.').collect::<Vec<_>>();
            if parts.len() == 5 && parts[2].starts_with("ap") {
                let operation_id = parts[0];
                let phase = parse_checkpoint_phase(parts[1]);
                if let Some(phase) = phase {
                    let role_token = parts[2].strip_prefix("ap").unwrap_or_default();
                    let role = WalAppendStageRole::parse(role_token).ok_or_else(|| {
                        format!(
                            "WriteAuthority WAL refuză rolul checkpoint Append necunoscut din {file_name}."
                        )
                    })?;
                    let checkpoint = WalAppendStageCheckpoint::from_persisted(
                        parts[3].to_string(),
                        parts[4].to_string(),
                        role,
                    )?;
                    let parsed =
                        Self::with_append_stage_checkpoint(operation_id, phase, checkpoint)?;
                    if parsed.file_name != file_name {
                        return Err(format!(
                            "WriteAuthority WAL refuză numele checkpoint Append non-canonic {file_name}."
                        ));
                    }
                    return Ok(parsed);
                }
            }
            if parts.len() == 5 && parts[2].starts_with("cp") {
                let operation_id = parts[0];
                let phase = parse_checkpoint_phase(parts[1]);
                if let Some(phase) = phase {
                    let role_token = parts[2].strip_prefix("cp").unwrap_or_default();
                    let role = WalCopyStageRole::parse(role_token).ok_or_else(|| {
                        format!(
                            "WriteAuthority WAL refuză rolul checkpoint Copy necunoscut din {file_name}."
                        )
                    })?;
                    let checkpoint = WalCopyStageCheckpoint::from_persisted(
                        parts[3].to_string(),
                        parts[4].to_string(),
                        role,
                    )?;
                    let parsed = Self::with_copy_stage_checkpoint(operation_id, phase, checkpoint)?;
                    if parsed.file_name != file_name {
                        return Err(format!(
                            "WriteAuthority WAL refuză numele checkpoint Copy non-canonic {file_name}."
                        ));
                    }
                    return Ok(parsed);
                }
            }
            if parts.len() == 5 && parts[2] == "dr" {
                let operation_id = parts[0];
                if let Some(phase) = parse_checkpoint_phase(parts[1]) {
                    let checkpoint = WalDirectoryStageCheckpoint::new(
                        parts[3].to_string(),
                        parts[4].to_string(),
                    )?;
                    let parsed =
                        Self::with_directory_stage_checkpoint(operation_id, phase, checkpoint)?;
                    if parsed.file_name != file_name {
                        return Err(format!(
                            "WriteAuthority WAL refuză numele checkpoint Directory non-canonic {file_name}."
                        ));
                    }
                    return Ok(parsed);
                }
            }
            if parts.len() == 5 && parts[2] == "sl" {
                let operation_id = parts[0];
                if let Some(phase) = parse_checkpoint_phase(parts[1]) {
                    let checkpoint =
                        WalSymlinkStageCheckpoint::new(parts[3].to_string(), parts[4].to_string())?;
                    let parsed =
                        Self::with_symlink_stage_checkpoint(operation_id, phase, checkpoint)?;
                    if parsed.file_name != file_name {
                        return Err(format!(
                            "WriteAuthority WAL refuză numele checkpoint Symlink non-canonic {file_name}."
                        ));
                    }
                    return Ok(parsed);
                }
            }
            if matches!(parts.len(), 4 | 5) {
                let operation_id = parts[0];
                let phase = parts[1];
                let target = parts[2];
                let backup = parts[3];
                let phase = match phase {
                    "auxiliary-durable" => Some(WalPhase::AuxiliaryDurable),
                    "effect-visible" => Some(WalPhase::EffectVisible),
                    "target-durable" => Some(WalPhase::TargetDurable),
                    _ => None,
                };
                if let Some(phase) = phase {
                    let decision = if parts.len() == 5 {
                        Some(WalExternalOperatorDecision::parse(parts[4]).ok_or_else(|| {
                            format!(
                                "WriteAuthority WAL refuză decizia ExternalConfig necunoscută din {file_name}."
                            )
                        })?)
                    } else {
                        None
                    };
                    let checkpoint = WalExternalStageCheckpoint::new(
                        target.to_string(),
                        (backup != "none").then(|| backup.to_string()),
                    )?;
                    let parsed =
                        Self::with_external_metadata(operation_id, phase, checkpoint, decision)?;
                    if parsed.file_name != file_name {
                        return Err(format!(
                            "WriteAuthority WAL refuză numele checkpoint non-canonic {file_name}."
                        ));
                    }
                    return Ok(parsed);
                }
            }
        }
        for (suffix, phase) in [
            (".target-durable.json", WalPhase::TargetDurable),
            (".auxiliary-durable.json", WalPhase::AuxiliaryDurable),
            (".effect-visible.json", WalPhase::EffectVisible),
            (".prepared.json", WalPhase::Prepared),
            (".preparing", WalPhase::Preparing),
        ] {
            if let Some(operation_id) = file_name.strip_suffix(suffix) {
                return Self::new(operation_id, phase);
            }
        }
        Err(format!(
            "WriteAuthority WAL nu recunoaște numele de record {file_name}."
        ))
    }
}

fn checkpoint_phase_name(phase: WalPhase, family: &str) -> Result<&'static str, String> {
    match phase {
        WalPhase::AuxiliaryDurable => Ok("auxiliary-durable"),
        WalPhase::EffectVisible => Ok("effect-visible"),
        WalPhase::TargetDurable => Ok("target-durable"),
        WalPhase::Preparing | WalPhase::Prepared => Err(format!(
            "Checkpointul {family} este permis numai din AuxiliaryDurable."
        )),
    }
}

fn parse_checkpoint_phase(value: &str) -> Option<WalPhase> {
    match value {
        "auxiliary-durable" => Some(WalPhase::AuxiliaryDurable),
        "effect-visible" => Some(WalPhase::EffectVisible),
        "target-durable" => Some(WalPhase::TargetDurable),
        _ => None,
    }
}

fn copy_payload_contract_digest(
    expected_content_hash: &str,
    expected_size: u64,
    expected_mode_bits: u32,
    role: WalCopyStageRole,
) -> Result<String, String> {
    validate_copy_content_hash(expected_content_hash)?;
    if expected_mode_bits > 0o7777 {
        return Err("Checkpointul Copy refuză mode bits în afara 0o7777.".into());
    }
    let mut hash = Sha256::new();
    hash.update(b"pana-wal-copy-stage-contract-v1\0");
    hash.update(role.token().as_bytes());
    hash.update(b"\0");
    hash.update(expected_content_hash.as_bytes());
    hash.update(b"\0");
    hash.update(expected_size.to_le_bytes());
    hash.update(expected_mode_bits.to_le_bytes());
    let digest = format!("{:x}", hash.finalize());
    Ok(digest[..32].to_string())
}

fn append_payload_contract_digest(
    expected_content_hash: &str,
    expected_size: u64,
    before_size: u64,
    role: WalAppendStageRole,
) -> Result<String, String> {
    validate_copy_content_hash(expected_content_hash)
        .map_err(|_| "Checkpointul Append cere SHA-256 lowercase de exact 256 biți.".to_string())?;
    before_size
        .checked_add(expected_size)
        .ok_or_else(|| "Checkpointul Append refuză overflow-ul dimensiunii finale.".to_string())?;
    let mut hash = Sha256::new();
    hash.update(b"pana-wal-append-stage-contract-v2\0");
    hash.update(role.token().as_bytes());
    hash.update(b"\0");
    hash.update(expected_content_hash.as_bytes());
    hash.update(b"\0");
    hash.update(expected_size.to_le_bytes());
    hash.update(before_size.to_le_bytes());
    let digest = format!("{:x}", hash.finalize());
    Ok(digest[..32].to_string())
}

fn validate_append_identity_digest(value: &str) -> Result<(), String> {
    validate_lower_hex_digest(value, 32).map_err(|()| {
        "Checkpointul Append cere identitate statx lowercase de exact 128 biți.".into()
    })
}

fn validate_append_contract_digest(value: &str) -> Result<(), String> {
    validate_lower_hex_digest(value, 32).map_err(|()| {
        "Checkpointul Append cere contract digest lowercase de exact 128 biți.".into()
    })
}

fn validate_copy_content_hash(value: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err("Checkpointul Copy cere SHA-256 lowercase de exact 256 biți.".into());
    }
    Ok(())
}

fn validate_copy_identity_digest(value: &str) -> Result<(), String> {
    validate_lower_hex_digest(value, 32)
        .map_err(|()| "Checkpointul Copy cere identitate statx lowercase de exact 128 biți.".into())
}

fn validate_directory_identity_digest(value: &str) -> Result<(), String> {
    validate_lower_hex_digest(value, 32).map_err(|()| {
        "Checkpointul Directory cere identitate statx lowercase de exact 128 biți.".into()
    })
}

fn validate_directory_state_digest(value: &str) -> Result<(), String> {
    validate_lower_hex_digest(value, 32).map_err(|()| {
        "Checkpointul Directory cere state digest lowercase de exact 128 biți.".into()
    })
}

fn validate_symlink_identity_digest(value: &str) -> Result<(), String> {
    validate_lower_hex_digest(value, 32).map_err(|()| {
        "Checkpointul Symlink cere identitate statx lowercase de exact 128 biți.".into()
    })
}

fn validate_symlink_state_digest(value: &str) -> Result<(), String> {
    validate_lower_hex_digest(value, 32)
        .map_err(|()| "Checkpointul Symlink cere state digest lowercase de exact 128 biți.".into())
}

fn validate_copy_contract_digest(value: &str) -> Result<(), String> {
    validate_lower_hex_digest(value, 32)
        .map_err(|()| "Checkpointul Copy cere contract digest lowercase de exact 128 biți.".into())
}

fn validate_identity_digest(value: &str) -> Result<(), String> {
    if validate_lower_hex_digest(value, 32).is_err() {
        return Err("Checkpointul ExternalConfig cere digest lowercase de exact 128 biți.".into());
    }
    Ok(())
}

fn validate_lower_hex_digest(value: &str, expected_len: usize) -> Result<(), ()> {
    (value.len() == expected_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')))
    .then_some(())
    .ok_or(())
}

fn validate_wal_file_name_length(file_name: &str) -> Result<(), String> {
    if file_name.len() > MAX_WAL_FILE_NAME_BYTES {
        return Err(format!(
            "WriteAuthority WAL filename depășește NAME_MAX: {} bytes.",
            file_name.len()
        ));
    }
    Ok(())
}

pub(super) fn validate_operation_id(operation_id: &str) -> Result<(), String> {
    if operation_id.is_empty()
        || operation_id.len() > 160
        || !operation_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(
            "WriteAuthority WAL operation ID trebuie să fie ASCII sigur și sub 160 bytes.".into(),
        );
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub(crate) fn encode_path_hex(path: &std::path::Path) -> String {
    encode_hex(path.as_os_str().as_bytes())
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn encode_path_hex(path: &std::path::Path) -> String {
    encode_hex(path.to_string_lossy().as_bytes())
}

#[cfg(target_os = "linux")]
pub(crate) fn decode_path_hex(value: &str) -> Result<PathBuf, String> {
    decode_hex(value).map(|bytes| PathBuf::from(OsString::from_vec(bytes)))
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn decode_path_hex(value: &str) -> Result<PathBuf, String> {
    let bytes = decode_hex(value)?;
    String::from_utf8(bytes)
        .map(PathBuf::from)
        .map_err(|_| "WriteAuthority WAL path-ul nu este UTF-8 pe această platformă.".into())
}

#[cfg(target_os = "linux")]
pub(crate) fn encode_component_hex(component: &std::ffi::OsStr) -> String {
    encode_hex(component.as_bytes())
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn encode_component_hex(component: &std::ffi::OsStr) -> String {
    encode_hex(component.to_string_lossy().as_bytes())
}

#[cfg(target_os = "linux")]
pub(crate) fn decode_component_hex(value: &str) -> Result<OsString, String> {
    let bytes = decode_hex(value)?;
    validate_component_bytes(&bytes)?;
    Ok(OsString::from_vec(bytes))
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn decode_component_hex(value: &str) -> Result<OsString, String> {
    let bytes = decode_hex(value)?;
    validate_component_bytes(&bytes)?;
    String::from_utf8(bytes)
        .map(OsString::from)
        .map_err(|_| "WriteAuthority WAL componenta nu este UTF-8 pe această platformă.".into())
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(char::from(HEX[usize::from(byte >> 4)]));
        result.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    result
}

pub(crate) fn encode_bytes_hex(bytes: &[u8]) -> String {
    encode_hex(bytes)
}

pub(crate) fn decode_bytes_hex(value: &str) -> Result<Vec<u8>, String> {
    decode_hex_bounded(value, super::model::MAX_WAL_APPEND_PAYLOAD_BYTES * 2)
}

fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
    decode_hex_bounded(value, 32 * 1024)
}

fn decode_hex_bounded(value: &str, max_encoded_bytes: usize) -> Result<Vec<u8>, String> {
    if value.len() % 2 != 0 || value.len() > max_encoded_bytes {
        return Err("WriteAuthority WAL a întâlnit un câmp hex invalid.".into());
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    let raw = value.as_bytes();
    for pair in raw.chunks_exact(2) {
        let high = decode_nibble(pair[0])?;
        let low = decode_nibble(pair[1])?;
        bytes.push((high << 4) | low);
    }
    Ok(bytes)
}

fn decode_nibble(value: u8) -> Result<u8, String> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err("WriteAuthority WAL a întâlnit un caracter hex invalid.".into()),
    }
}

fn validate_component_bytes(bytes: &[u8]) -> Result<(), String> {
    if bytes.is_empty()
        || bytes.len() > 255
        || bytes == b"."
        || bytes == b".."
        || bytes.contains(&0)
        || bytes.contains(&b'/')
    {
        return Err("WriteAuthority WAL a refuzat o componentă de path invalidă.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const COPY_HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn checkpoint() -> WalExternalStageCheckpoint {
        WalExternalStageCheckpoint::new("a".repeat(32), None).unwrap()
    }

    fn copy_checkpoint(role: WalCopyStageRole) -> WalCopyStageCheckpoint {
        WalCopyStageCheckpoint::new("b".repeat(32), COPY_HASH, 4096, 0o640, role).unwrap()
    }

    fn append_checkpoint(role: WalAppendStageRole) -> WalAppendStageCheckpoint {
        WalAppendStageCheckpoint::new("c".repeat(32), COPY_HASH, 8192, 4096, role).unwrap()
    }

    fn directory_checkpoint() -> WalDirectoryStageCheckpoint {
        WalDirectoryStageCheckpoint::new("d".repeat(32), "e".repeat(32)).unwrap()
    }

    fn symlink_checkpoint() -> WalSymlinkStageCheckpoint {
        WalSymlinkStageCheckpoint::new("a".repeat(32), "b".repeat(32)).unwrap()
    }

    #[test]
    fn directory_checkpoint_filename_binds_identity_state_and_survives_successors() {
        let operation_id = "o".repeat(160);
        let auxiliary = WalRecordName::with_directory_stage_checkpoint(
            &operation_id,
            WalPhase::AuxiliaryDurable,
            directory_checkpoint(),
        )
        .unwrap();
        assert!(auxiliary.file_name.len() <= MAX_WAL_FILE_NAME_BYTES);
        assert_eq!(
            WalRecordName::parse(&auxiliary.file_name).unwrap(),
            auxiliary
        );
        let effect = auxiliary.successor(WalPhase::EffectVisible).unwrap();
        let target = effect.successor(WalPhase::TargetDurable).unwrap();
        assert_eq!(
            target.directory_stage_checkpoint,
            auxiliary.directory_stage_checkpoint
        );
        assert_eq!(WalRecordName::parse(&target.file_name).unwrap(), target);
        assert_ne!(
            auxiliary.evidence_binding_hash("body-hash"),
            WalRecordName::with_directory_stage_checkpoint(
                &operation_id,
                WalPhase::AuxiliaryDurable,
                WalDirectoryStageCheckpoint::new("d".repeat(32), "f".repeat(32)).unwrap(),
            )
            .unwrap()
            .evidence_binding_hash("body-hash")
        );
    }

    #[test]
    fn symlink_checkpoint_filename_binds_identity_state_and_survives_successors() {
        let operation_id = "o".repeat(160);
        let auxiliary = WalRecordName::with_symlink_stage_checkpoint(
            &operation_id,
            WalPhase::AuxiliaryDurable,
            symlink_checkpoint(),
        )
        .unwrap();
        assert!(auxiliary.file_name.len() <= MAX_WAL_FILE_NAME_BYTES);
        assert_eq!(
            WalRecordName::parse(&auxiliary.file_name).unwrap(),
            auxiliary
        );
        let effect = auxiliary.successor(WalPhase::EffectVisible).unwrap();
        let target = effect.successor(WalPhase::TargetDurable).unwrap();
        assert_eq!(
            target.symlink_stage_checkpoint,
            auxiliary.symlink_stage_checkpoint
        );
        assert_eq!(WalRecordName::parse(&target.file_name).unwrap(), target);
        assert_ne!(
            auxiliary.evidence_binding_hash("body-hash"),
            WalRecordName::with_symlink_stage_checkpoint(
                &operation_id,
                WalPhase::AuxiliaryDurable,
                WalSymlinkStageCheckpoint::new("a".repeat(32), "c".repeat(32)).unwrap(),
            )
            .unwrap()
            .evidence_binding_hash("body-hash")
        );
    }

    #[test]
    fn append_checkpoint_filename_round_trips_and_survives_phase_successors() {
        let operation_id = "o".repeat(160);
        let auxiliary = WalRecordName::with_append_stage_checkpoint(
            &operation_id,
            WalPhase::AuxiliaryDurable,
            append_checkpoint(WalAppendStageRole::ExistingTarget),
        )
        .unwrap();
        assert!(auxiliary.file_name.len() <= MAX_WAL_FILE_NAME_BYTES);
        assert_eq!(auxiliary.file_name.len(), 253);
        assert_eq!(
            WalRecordName::parse(&auxiliary.file_name).unwrap(),
            auxiliary
        );
        let effect = auxiliary.successor(WalPhase::EffectVisible).unwrap();
        let target = effect.successor(WalPhase::TargetDurable).unwrap();
        assert_eq!(
            effect.append_stage_checkpoint,
            auxiliary.append_stage_checkpoint
        );
        assert_eq!(
            target.append_stage_checkpoint,
            auxiliary.append_stage_checkpoint
        );
        assert_eq!(WalRecordName::parse(&target.file_name).unwrap(), target);
        assert!(auxiliary
            .append_stage_checkpoint
            .as_ref()
            .unwrap()
            .matches_payload_contract(COPY_HASH, 8192, 4096, WalAppendStageRole::ExistingTarget));
    }

    #[test]
    fn copy_checkpoint_filename_round_trips_and_survives_phase_successors() {
        let operation_id = "o".repeat(160);
        let auxiliary = WalRecordName::with_copy_stage_checkpoint(
            &operation_id,
            WalPhase::AuxiliaryDurable,
            copy_checkpoint(WalCopyStageRole::ReplaceTemporary),
        )
        .unwrap();
        assert!(auxiliary.file_name.len() <= MAX_WAL_FILE_NAME_BYTES);
        assert_eq!(auxiliary.file_name.len(), 253);
        assert_eq!(
            WalRecordName::parse(&auxiliary.file_name).unwrap(),
            auxiliary
        );

        let effect = auxiliary.successor(WalPhase::EffectVisible).unwrap();
        let target = effect.successor(WalPhase::TargetDurable).unwrap();
        assert_eq!(
            effect.copy_stage_checkpoint,
            auxiliary.copy_stage_checkpoint
        );
        assert_eq!(
            target.copy_stage_checkpoint,
            auxiliary.copy_stage_checkpoint
        );
        assert_eq!(WalRecordName::parse(&target.file_name).unwrap(), target);
    }

    #[test]
    fn copy_checkpoint_binds_identity_and_payload_contract() {
        let checkpoint = copy_checkpoint(WalCopyStageRole::CreateTarget);
        assert!(checkpoint.matches_payload_contract(
            COPY_HASH,
            4096,
            0o640,
            WalCopyStageRole::CreateTarget
        ));
        assert!(!checkpoint.matches_payload_contract(
            COPY_HASH,
            4097,
            0o640,
            WalCopyStageRole::CreateTarget
        ));
        assert!(!checkpoint.matches_payload_contract(
            COPY_HASH,
            4096,
            0o640,
            WalCopyStageRole::ReplaceTemporary
        ));

        let prepared = WalRecordName::new("operation", WalPhase::Prepared).unwrap();
        let auxiliary = WalRecordName::with_copy_stage_checkpoint(
            "operation",
            WalPhase::AuxiliaryDurable,
            checkpoint,
        )
        .unwrap();
        assert_eq!(prepared.evidence_binding_hash("body-hash"), "body-hash");
        assert_ne!(
            auxiliary.evidence_binding_hash("body-hash"),
            auxiliary.evidence_binding_hash("other-body-hash")
        );
    }

    #[test]
    fn copy_checkpoint_rejects_noncanonical_or_invalid_contract_fields() {
        assert!(WalCopyStageCheckpoint::new(
            "B".repeat(32),
            COPY_HASH,
            1,
            0o600,
            WalCopyStageRole::CreateTarget
        )
        .is_err());
        assert!(WalCopyStageCheckpoint::new(
            "b".repeat(32),
            "f".repeat(63).as_str(),
            1,
            0o600,
            WalCopyStageRole::CreateTarget
        )
        .is_err());
        assert!(WalCopyStageCheckpoint::new(
            "b".repeat(32),
            COPY_HASH,
            1,
            0o10_000,
            WalCopyStageRole::CreateTarget
        )
        .is_err());
    }

    #[test]
    fn legacy_wal_names_remain_checkpoint_free() {
        for name in [
            "legacy.prepared.json",
            "legacy.auxiliary-durable.json",
            "legacy.effect-visible.json",
            "legacy.target-durable.json",
        ] {
            let parsed = WalRecordName::parse(name).unwrap();
            assert!(parsed.copy_stage_checkpoint.is_none());
            assert!(parsed.directory_stage_checkpoint.is_none());
            assert!(parsed.symlink_stage_checkpoint.is_none());
            assert!(parsed.external_stage_checkpoint.is_none());
        }
    }

    #[test]
    fn external_checkpoint_filename_round_trips_at_operation_id_limit() {
        let operation_id = "o".repeat(160);
        let auxiliary = WalRecordName::with_external_stage_checkpoint(
            &operation_id,
            WalPhase::AuxiliaryDurable,
            checkpoint(),
        )
        .unwrap();
        assert!(auxiliary.file_name.len() <= MAX_WAL_FILE_NAME_BYTES);
        assert_eq!(
            WalRecordName::parse(&auxiliary.file_name).unwrap(),
            auxiliary
        );
        assert_eq!(auxiliary.phase, WalPhase::AuxiliaryDurable);
    }

    #[test]
    fn external_checkpoint_binds_record_evidence_hash() {
        let prepared = WalRecordName::new("operation", WalPhase::Prepared).unwrap();
        let auxiliary = WalRecordName::with_external_stage_checkpoint(
            "operation",
            WalPhase::AuxiliaryDurable,
            checkpoint(),
        )
        .unwrap();
        assert_eq!(prepared.evidence_binding_hash("body-hash"), "body-hash");
        assert_ne!(
            prepared.evidence_binding_hash("body-hash"),
            auxiliary.evidence_binding_hash("body-hash")
        );
    }

    #[test]
    fn external_checkpoint_rejects_noncanonical_digest() {
        assert!(WalExternalStageCheckpoint::new("A".repeat(32), None).is_err());
        assert!(WalExternalStageCheckpoint::new("a".repeat(31), None).is_err());
    }
}
