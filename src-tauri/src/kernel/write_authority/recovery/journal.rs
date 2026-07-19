use super::{
    model::{WalPhase, WalRecord},
    paths::{
        WalAppendStageCheckpoint, WalCopyStageCheckpoint, WalDirectoryStageCheckpoint,
        WalExternalStageCheckpoint, WalRecordName, WalSymlinkStageCheckpoint,
    },
    wal_io::WalDirectory,
};

#[derive(Debug)]
pub(super) struct WalJournalCursor {
    current: WalRecordName,
}

impl WalJournalCursor {
    pub(super) fn prepare(wal: &WalDirectory, record: &WalRecord) -> Result<Self, String> {
        let preparing = WalRecordName::new(&record.body.operation_id, WalPhase::Preparing)?;
        wal.prepare_record(&preparing, &record.to_bytes()?)?;
        let prepared = WalRecordName::new(&record.body.operation_id, WalPhase::Prepared)?;
        wal.rename_phase(&preparing, &prepared)?;
        Ok(Self { current: prepared })
    }

    pub(super) fn operation_id(&self) -> &str {
        &self.current.operation_id
    }

    pub(super) const fn phase(&self) -> WalPhase {
        self.current.phase
    }

    pub(super) fn advance(
        &mut self,
        wal: &WalDirectory,
        expected_next: WalPhase,
    ) -> Result<(), String> {
        if self.current.phase.next() != Some(expected_next) {
            return Err(format!(
                "WriteAuthority WAL refuză saltul {:?} -> {:?}.",
                self.current.phase, expected_next
            ));
        }
        let next = self.current.successor(expected_next)?;
        wal.rename_phase(&self.current, &next)?;
        self.current = next;
        Ok(())
    }

    pub(super) fn advance_external_auxiliary(
        &mut self,
        wal: &WalDirectory,
        checkpoint: WalExternalStageCheckpoint,
    ) -> Result<(), String> {
        if self.current.phase != WalPhase::Prepared
            || self.current.append_stage_checkpoint.is_some()
            || self.current.copy_stage_checkpoint.is_some()
            || self.current.directory_stage_checkpoint.is_some()
            || self.current.symlink_stage_checkpoint.is_some()
            || self.current.external_stage_checkpoint.is_some()
        {
            return Err(
                "WriteAuthority WAL ExternalConfig checkpoint cere faza Prepared legacy.".into(),
            );
        }
        let next = WalRecordName::with_external_stage_checkpoint(
            &self.current.operation_id,
            WalPhase::AuxiliaryDurable,
            checkpoint,
        )?;
        wal.rename_phase(&self.current, &next)?;
        self.current = next;
        Ok(())
    }

    pub(super) fn advance_copy_auxiliary(
        &mut self,
        wal: &WalDirectory,
        checkpoint: WalCopyStageCheckpoint,
    ) -> Result<(), String> {
        if self.current.phase != WalPhase::Prepared
            || self.current.append_stage_checkpoint.is_some()
            || self.current.copy_stage_checkpoint.is_some()
            || self.current.directory_stage_checkpoint.is_some()
            || self.current.symlink_stage_checkpoint.is_some()
            || self.current.external_stage_checkpoint.is_some()
            || self.current.external_operator_decision.is_some()
        {
            return Err(
                "WriteAuthority WAL Copy checkpoint cere faza Prepared fără metadata de filename."
                    .into(),
            );
        }
        let next = WalRecordName::with_copy_stage_checkpoint(
            &self.current.operation_id,
            WalPhase::AuxiliaryDurable,
            checkpoint,
        )?;
        wal.rename_phase(&self.current, &next)?;
        self.current = next;
        Ok(())
    }

    pub(super) fn advance_append_auxiliary(
        &mut self,
        wal: &WalDirectory,
        checkpoint: WalAppendStageCheckpoint,
    ) -> Result<(), String> {
        if self.current.phase != WalPhase::Prepared
            || self.current.append_stage_checkpoint.is_some()
            || self.current.copy_stage_checkpoint.is_some()
            || self.current.directory_stage_checkpoint.is_some()
            || self.current.symlink_stage_checkpoint.is_some()
            || self.current.external_stage_checkpoint.is_some()
            || self.current.external_operator_decision.is_some()
        {
            return Err(
                "WriteAuthority WAL Append checkpoint cere faza Prepared fără metadata de filename."
                    .into(),
            );
        }
        let next = WalRecordName::with_append_stage_checkpoint(
            &self.current.operation_id,
            WalPhase::AuxiliaryDurable,
            checkpoint,
        )?;
        wal.rename_phase(&self.current, &next)?;
        self.current = next;
        Ok(())
    }

    pub(super) fn advance_directory_auxiliary(
        &mut self,
        wal: &WalDirectory,
        checkpoint: WalDirectoryStageCheckpoint,
    ) -> Result<(), String> {
        if self.current.phase != WalPhase::Prepared
            || self.current.append_stage_checkpoint.is_some()
            || self.current.copy_stage_checkpoint.is_some()
            || self.current.directory_stage_checkpoint.is_some()
            || self.current.symlink_stage_checkpoint.is_some()
            || self.current.external_stage_checkpoint.is_some()
            || self.current.external_operator_decision.is_some()
        {
            return Err(
                "WriteAuthority WAL Directory checkpoint cere faza Prepared fără metadata de filename."
                    .into(),
            );
        }
        let next = WalRecordName::with_directory_stage_checkpoint(
            &self.current.operation_id,
            WalPhase::AuxiliaryDurable,
            checkpoint,
        )?;
        wal.rename_phase(&self.current, &next)?;
        self.current = next;
        Ok(())
    }

    pub(super) fn advance_symlink_auxiliary(
        &mut self,
        wal: &WalDirectory,
        checkpoint: WalSymlinkStageCheckpoint,
    ) -> Result<(), String> {
        if self.current.phase != WalPhase::Prepared
            || self.current.append_stage_checkpoint.is_some()
            || self.current.copy_stage_checkpoint.is_some()
            || self.current.directory_stage_checkpoint.is_some()
            || self.current.symlink_stage_checkpoint.is_some()
            || self.current.external_stage_checkpoint.is_some()
            || self.current.external_operator_decision.is_some()
        {
            return Err(
                "WriteAuthority WAL Symlink checkpoint cere faza Prepared fără metadata de filename."
                    .into(),
            );
        }
        let next = WalRecordName::with_symlink_stage_checkpoint(
            &self.current.operation_id,
            WalPhase::AuxiliaryDurable,
            checkpoint,
        )?;
        wal.rename_phase(&self.current, &next)?;
        self.current = next;
        Ok(())
    }

    pub(super) fn remove(&self, wal: &WalDirectory) -> Result<(), String> {
        wal.remove_record(&self.current)
    }
}
