use crate::kernel::write_authority::capability;

use super::{
    model::{
        WalOperationEvidence, WalRecord, WriteAuthorityRecoveryResolutionAction,
        WriteAuthorityRecoveryResolutionInput,
    },
    paths::WalRecordName,
    wal_io::WalDirectory,
};

pub(super) fn resolve_recovery_record(
    wal: &WalDirectory,
    input: &WriteAuthorityRecoveryResolutionInput,
) -> Result<String, String> {
    let mut matching = Vec::new();
    for entry in wal.list_entries()? {
        let name = match WalRecordName::parse(&entry.file_name) {
            Ok(name) if name.operation_id == input.operation_id => name,
            Ok(_) => continue,
            Err(error) => {
                return Err(format!(
                    "Rezoluția operator este blocată de un filename WAL invalid: {error}"
                ));
            }
        };
        matching.push((name, entry.bytes?));
    }
    if matching.len() != 1 {
        return Err(format!(
            "Rezoluția operator cere exact un record pentru {}, observate {}.",
            input.operation_id,
            matching.len()
        ));
    }
    let (name, bytes) = matching.pop().expect("length checked");
    if name.phase != input.expected_phase {
        return Err(format!(
            "Rezoluția operator a primit phase stale: expected {:?}, disk {:?}.",
            input.expected_phase, name.phase
        ));
    }
    let record = WalRecord::from_bytes(&bytes)?;
    name.validate_family_metadata(&record.body.operation_evidence)?;
    let wal_evidence_binding_hash = name.evidence_binding_hash(&record.evidence_hash);
    let dynamically_bound_current_state = matches!(
        (&record.body.operation_evidence, input.action),
        (
            WalOperationEvidence::Directory(_) | WalOperationEvidence::Symlink(_),
            WriteAuthorityRecoveryResolutionAction::AcceptCurrentState
        )
    );
    if record.body.operation_id != input.operation_id
        || (!dynamically_bound_current_state && wal_evidence_binding_hash != input.evidence_hash)
    {
        return Err(
            "Rezoluția operator a primit operation ID/evidence hash stale; recitește scanarea."
                .into(),
        );
    }
    let diagnostic = match &record.body.operation_evidence {
        WalOperationEvidence::Copy(_) => capability::resolve_copy_operator(
            &record,
            name.phase,
            name.copy_stage_checkpoint.as_ref(),
            input.action,
        )?,
        WalOperationEvidence::Directory(_) => capability::resolve_directory_operator(
            &record,
            name.phase,
            name.directory_stage_checkpoint.as_ref(),
            input.action,
            &input.evidence_hash,
            &wal_evidence_binding_hash,
        )?,
        WalOperationEvidence::Symlink(_) => capability::resolve_symlink_operator(
            &record,
            name.phase,
            name.symlink_stage_checkpoint.as_ref(),
            input.action,
            &input.evidence_hash,
            &wal_evidence_binding_hash,
        )?,
        WalOperationEvidence::RemoveLeaf(_) => {
            capability::resolve_remove_leaf_operator(&record, name.phase, input.action)?
        }
        WalOperationEvidence::RemoveTree(_) => {
            capability::resolve_remove_tree_operator(&record, name.phase, input.action)?
        }
        WalOperationEvidence::ExternalConfig(_) => {
            return Err(
                "ExternalConfig nu expune rezoluții operator în protocolul curent; un record incompatibil rămâne hot pentru diagnostic."
                    .into(),
            );
        }
        _ => {
            return Err(
                "Rezoluția operator este disponibilă numai pentru familiile Copy/Directory/Symlink/RemoveFile/RemoveDirectoryTree WAL."
                    .into(),
            );
        }
    };
    wal.remove_record(&name)?;
    Ok(diagnostic)
}
