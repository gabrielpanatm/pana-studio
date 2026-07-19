use std::collections::HashMap;

use crate::kernel::observability::now_ms;
use crate::kernel::write_authority::capability;

use super::{
    model::{
        RecoveryReadBudget, WalOperationEvidence, WalPhase, WalRecord,
        WriteAuthorityRecoveryClassification, WriteAuthorityRecoveryItem,
        WriteAuthorityRecoveryResolutionAction, WriteAuthorityRecoveryScan, WAL_SCHEMA_VERSION,
    },
    paths::WalRecordName,
    wal_io::WalDirectory,
};

pub(super) fn scan_wal(
    wal: &WalDirectory,
    read_budget: &mut RecoveryReadBudget,
) -> Result<WriteAuthorityRecoveryScan, String> {
    let entries = wal.list_entries()?;
    let mut total_bytes = 0_usize;
    let mut parsed = Vec::with_capacity(entries.len());
    let mut operation_counts = HashMap::<String, usize>::new();

    for entry in entries {
        let record_name = WalRecordName::parse(&entry.file_name);
        let record = entry.bytes.and_then(|bytes| {
            total_bytes = total_bytes.saturating_add(bytes.len());
            WalRecord::from_bytes(&bytes)
        });
        if let Ok(name) = &record_name {
            *operation_counts
                .entry(name.operation_id.clone())
                .or_default() += 1;
        }
        parsed.push((entry.file_name, record_name, record));
    }

    let mut items = Vec::new();
    for (file_name, record_name, record) in parsed {
        let (name, record) = match (record_name, record) {
            (Ok(name), Ok(record)) => (name, record),
            (name, record) => {
                let operation_id = name.as_ref().ok().map(|value| value.operation_id.clone());
                let phase = name.as_ref().ok().map(|value| value.phase);
                let diagnostic = match (name.err(), record.err()) {
                    (Some(name_error), Some(record_error)) => {
                        format!("{name_error} {record_error}")
                    }
                    (Some(error), None) | (None, Some(error)) => error,
                    (None, None) => "WriteAuthority WAL entry invalidă.".to_string(),
                };
                items.push(WriteAuthorityRecoveryItem {
                    file_name,
                    operation_id,
                    phase,
                    classification: WriteAuthorityRecoveryClassification::UnreadableOrCorrupt,
                    automatic_recovery_available: false,
                    evidence_hash: None,
                    available_resolution_actions: Vec::new(),
                    diagnostic,
                });
                continue;
            }
        };

        if record.body.operation_id != name.operation_id {
            items.push(WriteAuthorityRecoveryItem {
                file_name,
                operation_id: Some(name.operation_id),
                phase: Some(name.phase),
                classification: WriteAuthorityRecoveryClassification::UnreadableOrCorrupt,
                automatic_recovery_available: false,
                evidence_hash: Some(record.evidence_hash.clone()),
                available_resolution_actions: Vec::new(),
                diagnostic:
                    "Operation ID din filename nu corespunde recordului WAL; manual review obligatoriu."
                        .into(),
            });
            continue;
        }
        if operation_counts
            .get(&name.operation_id)
            .copied()
            .unwrap_or_default()
            != 1
        {
            items.push(WriteAuthorityRecoveryItem {
                file_name,
                operation_id: Some(name.operation_id),
                phase: Some(name.phase),
                classification: WriteAuthorityRecoveryClassification::Conflict,
                automatic_recovery_available: false,
                evidence_hash: Some(record.evidence_hash.clone()),
                available_resolution_actions: Vec::new(),
                diagnostic:
                    "Mai multe fișiere WAL revendică același operation ID; auto-recovery este blocat."
                        .into(),
            });
            continue;
        }

        if let Err(error) = name.validate_family_metadata(&record.body.operation_evidence) {
            items.push(WriteAuthorityRecoveryItem {
                file_name,
                operation_id: Some(name.operation_id.clone()),
                phase: Some(name.phase),
                classification: WriteAuthorityRecoveryClassification::Conflict,
                automatic_recovery_available: false,
                evidence_hash: Some(name.evidence_binding_hash(&record.evidence_hash)),
                available_resolution_actions: Vec::new(),
                diagnostic: error,
            });
            continue;
        }

        let mut family_resolution_actions = Vec::new();
        let mut directory_resolution_state_binding = None;
        let mut symlink_resolution_state_binding = None;
        let (mut classification, mut automatic_recovery_available, mut diagnostic) = match name.phase {
            WalPhase::Preparing => (
                WriteAuthorityRecoveryClassification::NoEffect,
                true,
                "Recordul .preparing precede publication point-ul WAL; niciun efect target nu era permis."
                    .to_string(),
            ),
            WalPhase::Prepared
            | WalPhase::AuxiliaryDurable
            | WalPhase::EffectVisible
            | WalPhase::TargetDurable => {
                let assessment = match &record.body.operation_evidence {
                    WalOperationEvidence::AtomicFile(_) => capability::classify_atomic_recovery(&record, name.phase, read_budget)
                        .map(|assessment| {
                            (
                                assessment.classification,
                                assessment.automatic_action.is_some(),
                                assessment.diagnostic,
                            )
                        }),
                    WalOperationEvidence::Append(_) => capability::classify_append_recovery(
                        &record,
                        name.phase,
                        name.append_stage_checkpoint.as_ref(),
                        read_budget,
                    )
                        .map(|assessment| {
                            (
                                assessment.classification,
                                assessment.automatic_action.is_some(),
                                assessment.diagnostic,
                            )
                        }),
                    WalOperationEvidence::Copy(_) => {
                        capability::classify_copy_recovery(
                            &record,
                            name.phase,
                            name.copy_stage_checkpoint.as_ref(),
                        )
                        .map(|assessment| {
                            (
                                assessment.classification,
                                assessment.automatic_action.is_some(),
                                assessment.diagnostic,
                            )
                        })
                    }
                    WalOperationEvidence::Directory(_) => {
                        capability::classify_directory_recovery(
                            &record,
                            name.phase,
                            name.directory_stage_checkpoint.as_ref(),
                        ).map(|assessment| {
                            family_resolution_actions =
                                assessment.available_resolution_actions;
                            directory_resolution_state_binding =
                                assessment.resolution_state_binding;
                            (
                                assessment.classification,
                                assessment.automatic_action.is_some(),
                                assessment.diagnostic,
                            )
                        })
                    }
                    WalOperationEvidence::ExternalConfig(_) => {
                        capability::classify_external_config_recovery(
                            &record,
                            name.phase,
                            name.external_stage_checkpoint.as_ref(),
                            name.external_operator_decision,
                            read_budget,
                        )
                        .map(|assessment| {
                            family_resolution_actions =
                                assessment.available_resolution_actions;
                            (
                                assessment.classification,
                                assessment.automatic_action.is_some(),
                                assessment.diagnostic,
                            )
                        })
                    }
                    WalOperationEvidence::RemoveLeaf(_) => {
                        capability::classify_remove_leaf_recovery(&record, name.phase).map(
                            |assessment| {
                                (
                                    assessment.classification,
                                    assessment.automatic_action.is_some(),
                                    assessment.diagnostic,
                                )
                            },
                        )
                    }
                    WalOperationEvidence::RemoveTree(_) => {
                        capability::classify_remove_tree_recovery(&record, name.phase).map(
                            |assessment| {
                                family_resolution_actions =
                                    assessment.available_resolution_actions;
                                (
                                    assessment.classification,
                                    assessment.automatic_action.is_some(),
                                    assessment.diagnostic,
                                )
                            },
                        )
                    }
                    WalOperationEvidence::Rename(_) => {
                        capability::classify_rename_recovery(&record, name.phase).map(|assessment| {
                            (
                                assessment.classification,
                                assessment.automatic_action.is_some(),
                                assessment.diagnostic,
                            )
                        })
                    }
                    WalOperationEvidence::Symlink(_) => {
                        capability::classify_symlink_recovery(
                            &record,
                            name.phase,
                            name.symlink_stage_checkpoint.as_ref(),
                        )
                        .map(|assessment| {
                            family_resolution_actions =
                                assessment.available_resolution_actions;
                            symlink_resolution_state_binding =
                                assessment.resolution_state_binding;
                            (
                                assessment.classification,
                                assessment.automatic_action.is_some(),
                                assessment.diagnostic,
                            )
                        })
                    }
                };
                assessment.unwrap_or_else(|error| {
                    (
                        WriteAuthorityRecoveryClassification::Conflict,
                        false,
                        format!(
                            "Oracle-ul filesystem a refuzat auto-recovery: {error} Manual review obligatoriu."
                        ),
                    )
                })
            }
        };
        if name.phase != WalPhase::Preparing
            && super::model::is_legacy_mcp_projection_record(&record)
        {
            classification = WriteAuthorityRecoveryClassification::CleanupRequired;
            automatic_recovery_available = true;
            diagnostic = "Proiecția MCP este derivată și non-authoritative; temp-ul legacy și WAL-ul pot fi abandonate fără a modifica target-ul vizibil."
                .to_string();
        }
        if classification == WriteAuthorityRecoveryClassification::NoEffect
            && name.phase != WalPhase::Preparing
            && name.phase != WalPhase::Prepared
        {
            automatic_recovery_available = false;
            diagnostic.push_str(&format!(
                " Faza {:?} dovedește progres după publication point; no-effect nu poate elimina automat WAL-ul.",
                name.phase
            ));
        }
        let available_resolution_actions = if !family_resolution_actions.is_empty() {
            family_resolution_actions
        } else {
            match (&record.body.operation_evidence, classification) {
                (
                    WalOperationEvidence::Copy(_),
                    WriteAuthorityRecoveryClassification::RollbackCompleted,
                ) if !automatic_recovery_available => {
                    vec![WriteAuthorityRecoveryResolutionAction::AcceptRestoredState]
                }
                (
                    WalOperationEvidence::Directory(_),
                    WriteAuthorityRecoveryClassification::RollbackCompleted,
                ) if !automatic_recovery_available => {
                    vec![WriteAuthorityRecoveryResolutionAction::AcceptRestoredState]
                }
                (
                    WalOperationEvidence::RemoveLeaf(_),
                    WriteAuthorityRecoveryClassification::CleanupRequired,
                ) if !automatic_recovery_available => {
                    vec![WriteAuthorityRecoveryResolutionAction::RestoreOriginal]
                }
                (
                    WalOperationEvidence::RemoveLeaf(_),
                    WriteAuthorityRecoveryClassification::NoEffect
                    | WriteAuthorityRecoveryClassification::RollbackCompleted,
                ) if !automatic_recovery_available => {
                    vec![WriteAuthorityRecoveryResolutionAction::AcceptRestoredState]
                }
                _ => Vec::new(),
            }
        };
        let wal_evidence_binding_hash = name.evidence_binding_hash(&record.evidence_hash);
        let evidence_binding_hash =
            if let Some(binding) = directory_resolution_state_binding.as_ref() {
                binding.evidence_hash(&wal_evidence_binding_hash)
            } else if let Some(binding) = symlink_resolution_state_binding.as_ref() {
                binding.evidence_hash(&wal_evidence_binding_hash)
            } else {
                wal_evidence_binding_hash
            };
        items.push(WriteAuthorityRecoveryItem {
            file_name,
            operation_id: Some(name.operation_id),
            phase: Some(name.phase),
            classification,
            automatic_recovery_available,
            evidence_hash: Some(evidence_binding_hash),
            available_resolution_actions,
            diagnostic,
        });
    }

    Ok(WriteAuthorityRecoveryScan {
        schema_version: WAL_SCHEMA_VERSION,
        scanned_at_ms: now_ms(),
        blocked: !items.is_empty(),
        record_count: items.len(),
        total_bytes,
        items,
    })
}
