use std::sync::{Mutex, MutexGuard};

use super::{
    executor::execute_automatic_recovery,
    journal::WalJournalCursor,
    model::{
        RecoveryReadBudget, WalPhase, WalRecord, WriteAuthorityRecoveryClassification,
        WriteAuthorityRecoveryItem, WriteAuthorityRecoveryResolutionInput,
        WriteAuthorityRecoveryResolutionReceipt, WriteAuthorityRecoveryScan,
        WRITE_AUTHORITY_RECOVERY_RESOLUTION_SCHEMA_VERSION,
    },
    operator::resolve_recovery_record,
    paths::{
        WalAppendStageCheckpoint, WalCopyStageCheckpoint, WalDirectoryStageCheckpoint,
        WalExternalStageCheckpoint, WalSymlinkStageCheckpoint,
    },
    scan::scan_wal,
    wal_io::{WalDirectory, WalFileLock, WalLockMode},
};
use crate::kernel::observability::now_ms;
use crate::kernel::write_authority::{
    capability,
    root_authority::{DirectoryAuthority, DirectoryAuthorityScope},
};

#[derive(Debug)]
pub(crate) struct RecoveryCoordinator {
    wal: WalDirectory,
    copy_io_gate: Mutex<()>,
    operation_gate: Mutex<()>,
    scan: Mutex<WriteAuthorityRecoveryScan>,
}

impl RecoveryCoordinator {
    pub(crate) fn bootstrap(wal_authority: DirectoryAuthority) -> Result<Self, String> {
        if !matches!(
            wal_authority.scope(),
            DirectoryAuthorityScope::ApplicationWriteAuthorityWal
        ) {
            return Err("WriteAuthority recovery cere authority WAL dedicată.".into());
        }
        capability::verify_directory_authority_path(&wal_authority)?;
        let wal = WalDirectory::new(wal_authority);
        let _exclusive = wal.lock(WalLockMode::Exclusive)?;
        let scan = scan_and_recover(&wal)?;
        Ok(Self {
            wal,
            copy_io_gate: Mutex::new(()),
            operation_gate: Mutex::new(()),
            scan: Mutex::new(scan),
        })
    }

    pub(crate) fn snapshot(&self) -> Result<WriteAuthorityRecoveryScan, String> {
        self.scan
            .lock()
            .map(|scan| scan.clone())
            .map_err(|_| "WriteAuthority WAL scan state este otrăvit.".to_string())
    }

    pub(crate) fn require_clean(&self) -> Result<(), String> {
        let _gate = self
            .operation_gate
            .lock()
            .map_err(|_| "WriteAuthority WAL operation gate este otrăvit.".to_string())?;
        capability::verify_directory_authority_path(self.wal.authority())?;
        let _exclusive = self.wal.lock(WalLockMode::Exclusive)?;
        if self.wal.has_record_entries()? {
            return Err(format!(
                "WRITE_AUTHORITY_RECOVERY_BLOCKED: există cel puțin un record WAL pe disk; recovery este obligatoriu înaintea deschiderii proiectului."
            ));
        }
        self.publish_scan(clean_recovery_scan())
    }

    pub(crate) fn rescan_and_recover_exclusive(
        &self,
    ) -> Result<WriteAuthorityRecoveryScan, String> {
        let _gate = self
            .operation_gate
            .lock()
            .map_err(|_| "WriteAuthority WAL operation gate este otrăvit.".to_string())?;
        capability::verify_directory_authority_path(self.wal.authority())?;
        let _exclusive = self.wal.lock(WalLockMode::Exclusive)?;
        let scan = scan_and_recover(&self.wal)?;
        self.publish_scan(scan.clone())?;
        Ok(scan)
    }

    pub(crate) fn resolve_operator_exclusive(
        &self,
        input: WriteAuthorityRecoveryResolutionInput,
    ) -> Result<WriteAuthorityRecoveryResolutionReceipt, String> {
        let _gate = self
            .operation_gate
            .lock()
            .map_err(|_| "WriteAuthority WAL operation gate este otrăvit.".to_string())?;
        capability::verify_directory_authority_path(self.wal.authority())?;
        let _exclusive = self.wal.lock(WalLockMode::Exclusive)?;
        let resolution = resolve_recovery_record(&self.wal, &input);
        let scan = scan_and_recover(&self.wal).map_err(|scan_error| match &resolution {
            Ok(_) => format!(
                "Rezoluția operator a fost executată, dar rescanarea WAL a eșuat: {scan_error}"
            ),
            Err(error) => format!("{error} Rescanarea WAL a eșuat: {scan_error}"),
        })?;
        self.publish_scan(scan.clone())?;
        let diagnostic = resolution?;
        Ok(WriteAuthorityRecoveryResolutionReceipt {
            schema_version: WRITE_AUTHORITY_RECOVERY_RESOLUTION_SCHEMA_VERSION,
            operation_id: input.operation_id,
            action: input.action,
            diagnostic,
            recovery_scan: scan,
        })
    }

    pub(crate) fn acquire_copy_io(&self) -> Result<MutexGuard<'_, ()>, String> {
        self.copy_io_gate
            .lock()
            .map_err(|_| "WriteAuthority Copy I/O gate este otrăvit.".to_string())
    }

    pub(crate) fn begin<'a>(&'a self, record: WalRecord) -> Result<DurableWalGuard<'a>, String> {
        let operation_gate = self
            .operation_gate
            .lock()
            .map_err(|_| "WriteAuthority WAL operation gate este otrăvit.".to_string())?;
        capability::verify_directory_authority_path(self.wal.authority())?;
        // The lock covers scan -> WAL publication -> target effect -> WAL
        // terminal removal. Shared locking would allow two application
        // processes to both observe an empty WAL and mutate concurrently.
        let filesystem_lock = self.wal.lock(WalLockMode::Exclusive)?;
        if self.wal.has_record_entries()? {
            self.publish_scan(structural_hot_scan(
                "O mutație a găsit un record WAL existent; clasificarea completă este disponibilă prin controlul explicit Recitește.",
            ))?;
            return Err(format!(
                "WRITE_AUTHORITY_RECOVERY_BLOCKED: există deja cel puțin un record WAL hot."
            ));
        }
        let cursor = match WalJournalCursor::prepare(&self.wal, &record) {
            Ok(cursor) => cursor,
            Err(error) => {
                if self.wal.has_record_entries().unwrap_or(true) {
                    let _ = self.publish_scan(structural_hot_scan(
                        "Publicarea WAL a eșuat și directorul nu mai este structural curat; review obligatoriu.",
                    ));
                }
                return Err(error);
            }
        };
        Ok(DurableWalGuard {
            coordinator: self,
            _operation_gate: operation_gate,
            _filesystem_lock: filesystem_lock,
            cursor: Some(cursor),
            terminal: false,
        })
    }

    fn publish_scan(&self, scan: WriteAuthorityRecoveryScan) -> Result<(), String> {
        self.scan
            .lock()
            .map(|mut slot| *slot = scan)
            .map_err(|_| "WriteAuthority WAL scan state este otrăvit.".to_string())
    }

    fn publish_runtime_barrier(
        &self,
        operation_id: &str,
        phase: WalPhase,
        diagnostic: String,
    ) -> Result<(), String> {
        self.publish_scan(WriteAuthorityRecoveryScan {
            schema_version: super::model::WAL_SCHEMA_VERSION,
            scanned_at_ms: now_ms(),
            blocked: true,
            record_count: 1,
            total_bytes: 0,
            items: vec![WriteAuthorityRecoveryItem {
                file_name: "runtime-hot-guard".into(),
                operation_id: Some(operation_id.to_string()),
                phase: Some(phase),
                classification: WriteAuthorityRecoveryClassification::Conflict,
                automatic_recovery_available: false,
                evidence_hash: None,
                available_resolution_actions: Vec::new(),
                diagnostic,
            }],
        })
    }

    fn refresh_after_terminal(&self) -> Result<(), String> {
        if self.wal.has_record_entries()? {
            self.publish_scan(structural_hot_scan(
                "Operația terminală s-a încheiat, dar alt record WAL este încă prezent.",
            ))
        } else {
            self.publish_scan(clean_recovery_scan())
        }
    }
}

pub(crate) struct DurableWalGuard<'a> {
    coordinator: &'a RecoveryCoordinator,
    _operation_gate: MutexGuard<'a, ()>,
    _filesystem_lock: WalFileLock,
    cursor: Option<WalJournalCursor>,
    terminal: bool,
}

impl DurableWalGuard<'_> {
    pub(crate) fn operation_id(&self) -> &str {
        self.cursor
            .as_ref()
            .expect("WAL cursor exists until terminal completion")
            .operation_id()
    }

    pub(crate) fn phase(&self) -> WalPhase {
        self.cursor
            .as_ref()
            .expect("WAL cursor exists until terminal completion")
            .phase()
    }

    pub(crate) fn mark_auxiliary_durable(&mut self) -> Result<(), String> {
        self.advance(WalPhase::AuxiliaryDurable)
    }

    pub(crate) fn mark_external_auxiliary_durable(
        &mut self,
        checkpoint: WalExternalStageCheckpoint,
    ) -> Result<(), String> {
        self.cursor
            .as_mut()
            .expect("WAL cursor exists until terminal completion")
            .advance_external_auxiliary(&self.coordinator.wal, checkpoint)
    }

    pub(crate) fn mark_copy_auxiliary_durable(
        &mut self,
        checkpoint: WalCopyStageCheckpoint,
    ) -> Result<(), String> {
        self.cursor
            .as_mut()
            .expect("WAL cursor exists until terminal completion")
            .advance_copy_auxiliary(&self.coordinator.wal, checkpoint)
    }

    pub(crate) fn mark_append_auxiliary_durable(
        &mut self,
        checkpoint: WalAppendStageCheckpoint,
    ) -> Result<(), String> {
        self.cursor
            .as_mut()
            .expect("WAL cursor exists until terminal completion")
            .advance_append_auxiliary(&self.coordinator.wal, checkpoint)
    }

    pub(crate) fn mark_directory_auxiliary_durable(
        &mut self,
        checkpoint: WalDirectoryStageCheckpoint,
    ) -> Result<(), String> {
        self.cursor
            .as_mut()
            .expect("WAL cursor exists until terminal completion")
            .advance_directory_auxiliary(&self.coordinator.wal, checkpoint)
    }

    pub(crate) fn mark_symlink_auxiliary_durable(
        &mut self,
        checkpoint: WalSymlinkStageCheckpoint,
    ) -> Result<(), String> {
        self.cursor
            .as_mut()
            .expect("WAL cursor exists until terminal completion")
            .advance_symlink_auxiliary(&self.coordinator.wal, checkpoint)
    }

    pub(crate) fn mark_effect_visible(&mut self) -> Result<(), String> {
        self.advance(WalPhase::EffectVisible)
    }

    pub(crate) fn mark_target_durable(&mut self) -> Result<(), String> {
        self.advance(WalPhase::TargetDurable)
    }

    pub(crate) fn abort_no_effect(mut self) -> Result<(), String> {
        if self.phase() != WalPhase::Prepared {
            return Err(format!(
                "WriteAuthority WAL nu poate declara no-effect din faza {:?}.",
                self.phase()
            ));
        }
        self.remove_terminal()
    }

    pub(crate) fn commit(mut self) -> Result<(), String> {
        if self.phase() != WalPhase::TargetDurable {
            return Err(format!(
                "WriteAuthority WAL nu poate comite din faza {:?}.",
                self.phase()
            ));
        }
        self.remove_terminal()
    }

    fn advance(&mut self, phase: WalPhase) -> Result<(), String> {
        self.cursor
            .as_mut()
            .expect("WAL cursor exists until terminal completion")
            .advance(&self.coordinator.wal, phase)
    }

    fn remove_terminal(&mut self) -> Result<(), String> {
        let cursor = self
            .cursor
            .take()
            .expect("WAL cursor exists until terminal completion");
        self.terminal = true;
        if let Err(error) = cursor.remove(&self.coordinator.wal) {
            let _ = self.coordinator.publish_runtime_barrier(
                cursor.operation_id(),
                cursor.phase(),
                format!(
                    "WAL unlink/final directory fsync a eșuat și runtime-ul rămâne blocat: {error} Oracle-ul disk complet este amânat pentru rescan/restart."
                ),
            );
            return Err(error);
        }
        if let Err(error) = self.coordinator.refresh_after_terminal() {
            let _ = self.coordinator.publish_runtime_barrier(
                cursor.operation_id(),
                cursor.phase(),
                format!(
                    "WAL-ul terminal a fost eliminat, dar scanarea de confirmare a eșuat: {error} Runtime-ul rămâne blocat până la o recitire structurală."
                ),
            );
            return Err(error);
        }
        Ok(())
    }
}

fn scan_and_recover(wal: &WalDirectory) -> Result<WriteAuthorityRecoveryScan, String> {
    let mut read_budget = RecoveryReadBudget::new();
    let initial_scan = scan_wal(wal, &mut read_budget)?;
    let had_automatic_recovery = initial_scan
        .items
        .iter()
        .any(|item| item.automatic_recovery_available);
    let recovery_diagnostics = execute_automatic_recovery(wal, &initial_scan, &mut read_budget);
    let mut scan = if had_automatic_recovery {
        scan_wal(wal, &mut read_budget)?
    } else {
        initial_scan
    };
    if !recovery_diagnostics.is_empty() {
        scan.blocked = true;
        for diagnostic in recovery_diagnostics {
            scan.items.push(WriteAuthorityRecoveryItem {
                file_name: "automatic-recovery".into(),
                operation_id: None,
                phase: None,
                classification: WriteAuthorityRecoveryClassification::Conflict,
                automatic_recovery_available: false,
                evidence_hash: None,
                available_resolution_actions: Vec::new(),
                diagnostic,
            });
        }
        scan.record_count = scan.items.len();
    }
    Ok(scan)
}

fn clean_recovery_scan() -> WriteAuthorityRecoveryScan {
    WriteAuthorityRecoveryScan {
        schema_version: super::model::WAL_SCHEMA_VERSION,
        scanned_at_ms: now_ms(),
        blocked: false,
        record_count: 0,
        total_bytes: 0,
        items: Vec::new(),
    }
}

fn structural_hot_scan(diagnostic: &str) -> WriteAuthorityRecoveryScan {
    WriteAuthorityRecoveryScan {
        schema_version: super::model::WAL_SCHEMA_VERSION,
        scanned_at_ms: now_ms(),
        blocked: true,
        record_count: 1,
        total_bytes: 0,
        items: vec![WriteAuthorityRecoveryItem {
            file_name: "structural-wal-guard".into(),
            operation_id: None,
            phase: None,
            classification: WriteAuthorityRecoveryClassification::Conflict,
            automatic_recovery_available: false,
            evidence_hash: None,
            available_resolution_actions: Vec::new(),
            diagnostic: diagnostic.to_string(),
        }],
    }
}

impl Drop for DurableWalGuard<'_> {
    fn drop(&mut self) {
        if !self.terminal {
            if let Some(cursor) = self.cursor.as_ref() {
                let _ = self.coordinator.publish_runtime_barrier(
                    cursor.operation_id(),
                    cursor.phase(),
                    "Operația a lăsat un WAL hot. Runtime-ul este blocat imediat; clasificarea disk bounded este amânată pentru rescan/restart ca să nu blocheze threadul UI."
                        .into(),
                );
            }
        }
    }
}
