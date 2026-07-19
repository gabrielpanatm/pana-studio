use std::collections::HashMap;

use crate::kernel::write_authority::capability;

use super::{
    model::{
        RecoveryReadBudget, WalOperationEvidence, WalPhase, WalRecord, WriteAuthorityRecoveryScan,
    },
    paths::WalRecordName,
    wal_io::WalDirectory,
};

pub(super) fn execute_automatic_recovery(
    wal: &WalDirectory,
    scan: &WriteAuthorityRecoveryScan,
    read_budget: &mut RecoveryReadBudget,
) -> Vec<String> {
    let automatic = scan
        .items
        .iter()
        .filter(|item| item.automatic_recovery_available)
        .map(|item| (item.file_name.clone(), item.phase))
        .collect::<HashMap<_, _>>();
    if automatic.is_empty() {
        return Vec::new();
    }

    let entries = match wal.list_entries() {
        Ok(entries) => entries,
        Err(error) => return vec![error],
    };
    let mut diagnostics = Vec::new();
    for entry in entries {
        let Some(expected_phase) = automatic.get(&entry.file_name).copied().flatten() else {
            continue;
        };
        let result = (|| {
            let name = WalRecordName::parse(&entry.file_name)?;
            if name.phase != expected_phase {
                return Err(
                    "WAL phase s-a schimbat după scan; recovery CAS a oprit operația.".into(),
                );
            }
            let bytes = entry.bytes?;
            let record = WalRecord::from_bytes(&bytes)?;
            if record.body.operation_id != name.operation_id {
                return Err("WAL operation ID s-a schimbat după scan.".into());
            }
            name.validate_family_metadata(&record.body.operation_evidence)?;
            if name.phase == WalPhase::Preparing {
                return wal.remove_record(&name);
            }
            if super::model::is_legacy_mcp_projection_record(&record) {
                capability::discard_rebuildable_atomic_projection(&record, name.phase)?;
            } else {
                match &record.body.operation_evidence {
                    WalOperationEvidence::AtomicFile(_) => {
                        capability::execute_atomic_recovery(&record, name.phase, read_budget)?;
                    }
                    WalOperationEvidence::Append(_) => {
                        capability::execute_append_recovery(
                            &record,
                            name.phase,
                            name.append_stage_checkpoint.as_ref(),
                            read_budget,
                        )?;
                    }
                    WalOperationEvidence::Copy(_) => {
                        capability::execute_copy_recovery(
                            &record,
                            name.phase,
                            name.copy_stage_checkpoint.as_ref(),
                            read_budget,
                        )?;
                    }
                    WalOperationEvidence::Directory(_) => {
                        let assessment = capability::classify_directory_recovery(
                            &record,
                            name.phase,
                            name.directory_stage_checkpoint.as_ref(),
                        )?;
                        let action = assessment.automatic_action.ok_or_else(|| {
                            format!(
                                "Oracle-ul mkdir s-a schimbat după scan: {}",
                                assessment.diagnostic
                            )
                        })?;
                        capability::execute_directory_recovery(
                            &record,
                            name.phase,
                            name.directory_stage_checkpoint.as_ref(),
                            action,
                        )?;
                    }
                    WalOperationEvidence::ExternalConfig(_) => {
                        capability::execute_external_config_recovery(
                            &record,
                            name.phase,
                            name.external_stage_checkpoint.as_ref(),
                            name.external_operator_decision,
                            read_budget,
                        )?;
                    }
                    WalOperationEvidence::RemoveLeaf(_) => {
                        capability::execute_remove_leaf_recovery(&record, name.phase)?;
                    }
                    WalOperationEvidence::RemoveTree(_) => {
                        capability::execute_remove_tree_recovery(&record, name.phase)?;
                    }
                    WalOperationEvidence::Rename(_) => {
                        capability::execute_rename_recovery(&record, name.phase)?;
                    }
                    WalOperationEvidence::Symlink(_) => {
                        let assessment = capability::classify_symlink_recovery(
                            &record,
                            name.phase,
                            name.symlink_stage_checkpoint.as_ref(),
                        )?;
                        let action = assessment.automatic_action.ok_or_else(|| {
                            format!(
                                "Oracle-ul symlink s-a schimbat după scan: {}",
                                assessment.diagnostic
                            )
                        })?;
                        capability::execute_symlink_recovery(
                            &record,
                            name.phase,
                            name.symlink_stage_checkpoint.as_ref(),
                            action,
                        )?;
                    }
                }
            }
            wal.remove_record(&name)
        })();
        if let Err(error) = result {
            diagnostics.push(format!(
                "Auto-recovery {} nu s-a finalizat: {error}",
                entry.file_name
            ));
        }
    }
    diagnostics
}
