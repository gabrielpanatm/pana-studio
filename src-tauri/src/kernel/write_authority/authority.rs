use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

use tauri::{AppHandle, Manager, Runtime};

use crate::kernel::observability::{
    append_event, now_ms, KernelEventKind, KernelLogEvent, KernelLogLevel,
};
use crate::state::AppState;

use super::{
    boundary::{validate_target_boundary, BoundaryRules},
    capability::{
        append_wal, atomic_write, atomic_write_wal, copy_file_wal, create_directory_all_wal,
        external_config_wal, plan_append, plan_atomic_write, plan_copy, plan_directory,
        plan_external_config, plan_remove_leaf, plan_remove_tree, plan_rename, plan_symlink,
        remove_leaf_wal, remove_tree_wal, rename_entry_wal, symlink_entry_wal, CapabilityEffect,
        CapabilityReplacePolicy,
    },
    model::{
        ConflictPolicy, ExpectedLeaf, RecoveryPolicy, WriteAtomicity, WriteAuthorityError,
        WriteCategory, WriteIntent, WriteOperationKind, WriteOwner, WriteReceipt,
        WriteRecoveryReceipt, WriteTarget,
    },
    operation::{
        build_append_wal_record, build_atomic_wal_record, build_copy_wal_record,
        build_directory_wal_record, build_external_config_wal_record, build_remove_leaf_wal_record,
        build_remove_tree_wal_record, build_rename_wal_record, build_symlink_wal_record,
    },
    registry::{
        matching_write_declaration, validate_authority_path, validate_companion_authority_path,
    },
    root_authority::WriteAuthorityRuntime,
};

pub struct WriteAuthority<'a, R: Runtime> {
    app: &'a AppHandle<R>,
}

static WRITE_OPERATION_COUNTER: AtomicU64 = AtomicU64::new(1);

impl<'a, R: Runtime> WriteAuthority<'a, R> {
    pub fn new(app: &'a AppHandle<R>) -> Self {
        Self { app }
    }

    pub fn write_text(
        &self,
        intent: WriteIntent,
        contents: &str,
    ) -> Result<WriteReceipt, WriteAuthorityError> {
        if intent.operation != WriteOperationKind::WriteText {
            return Err("WriteAuthority.write_text cere operație WriteText.".into());
        }
        if intent.policy.atomicity != WriteAtomicity::AtomicRename {
            return Err("WriteAuthority.write_text cere politica AtomicRename.".into());
        }
        self.execute_write(intent, contents.as_bytes())
    }

    pub fn write_bytes(
        &self,
        intent: WriteIntent,
        contents: &[u8],
    ) -> Result<WriteReceipt, WriteAuthorityError> {
        if intent.operation != WriteOperationKind::WriteBytes {
            return Err("WriteAuthority.write_bytes cere operație WriteBytes.".into());
        }
        if intent.policy.atomicity != WriteAtomicity::AtomicRename {
            return Err("WriteAuthority.write_bytes cere politica AtomicRename.".into());
        }
        self.execute_write(intent, contents)
    }

    pub fn append_text(
        &self,
        intent: WriteIntent,
        contents: &str,
    ) -> Result<WriteReceipt, WriteAuthorityError> {
        if intent.operation != WriteOperationKind::AppendText {
            return Err("WriteAuthority.append_text cere operație AppendText.".into());
        }
        if intent.policy.atomicity != WriteAtomicity::AppendOnly {
            return Err("WriteAuthority.append_text cere politica AppendOnly.".into());
        }
        self.execute_append(intent, contents.as_bytes())
    }

    pub fn external_config_update(
        &self,
        intent: WriteIntent,
        contents: &str,
        backup: Option<(WriteTarget, &str)>,
    ) -> Result<WriteReceipt, WriteAuthorityError> {
        if intent.operation != WriteOperationKind::ExternalConfigUpdate {
            return Err(
                "WriteAuthority.external_config_update cere operație ExternalConfigUpdate.".into(),
            );
        }
        if intent.policy.atomicity != WriteAtomicity::ExternalToolWrite {
            return Err(
                "WriteAuthority.external_config_update cere politica ExternalToolWrite.".into(),
            );
        }
        if let Some((backup_target, previous_contents)) = backup {
            return self.with_authorized_pair(intent, backup_target, |intent, backup_target| {
                self.execute_external_config_update_authorized(
                    intent,
                    contents,
                    Some((backup_target, previous_contents)),
                )
            });
        }
        self.with_authorized_intent(intent, |intent| {
            self.execute_external_config_update_authorized(intent, contents, None)
        })
    }

    fn execute_external_config_update_authorized(
        &self,
        intent: WriteIntent,
        contents: &str,
        backup: Option<(WriteTarget, &str)>,
    ) -> Result<WriteReceipt, WriteAuthorityError> {
        let id = operation_id(intent.owner, intent.operation);
        let started_at_ms = now_ms();
        let backup = if let Some((backup_target, previous_contents)) = backup.as_ref() {
            if backup_target.path == intent.target.path {
                return Err(
                    "ExternalConfigUpdate blocat: backup-ul nu poate fi același fișier cu target-ul."
                        .into(),
                );
            }
            let same_authority = backup_target
                .authority()
                .zip(intent.target.authority())
                .map(|(backup, target)| backup.same_authority(target))
                .unwrap_or(false);
            if !same_authority {
                return Err(
                    "ExternalConfigUpdate blocat: backup-ul și target-ul trebuie să folosească același grant sigilat."
                        .into(),
                );
            }
            Some((backup_target, previous_contents.as_bytes()))
        } else {
            None
        };
        let plan = plan_external_config(&intent.target, contents.as_bytes(), backup, &id)?;
        let record = build_external_config_wal_record(&id, started_at_ms, &intent, &plan)?;
        let runtime = self
            .app
            .try_state::<WriteAuthorityRuntime>()
            .ok_or_else(|| {
                WriteAuthorityError::from(
                    "WriteAuthorityRuntime lipsește înainte de ExternalConfig WAL prepare.",
                )
            })?;
        let coordinator = runtime.recovery_coordinator()?;
        let mut guard = coordinator.begin(record)?;
        if let Err(log_error) = self.log_write_event(
            KernelEventKind::WritePlanned,
            KernelLogLevel::Info,
            &id,
            &intent,
            None,
        ) {
            return match guard.abort_no_effect() {
                Ok(()) => Err(log_error.into()),
                Err(wal_error) => Err(self.wal_recovery_error(
                    &id,
                    &intent,
                    started_at_ms,
                    0,
                    format!(
                        "WritePlanned ExternalConfig a eșuat, iar WAL cleanup nu este durabil: {log_error} {wal_error}"
                    ),
                )),
            };
        }

        let result = external_config_wal(
            &intent.target,
            contents.as_bytes(),
            backup,
            plan,
            &mut guard,
        );
        match result {
            Ok(mut effect) => {
                if effect.recovery_required {
                    drop(guard);
                } else if let Err(error) = guard.commit() {
                    effect = CapabilityEffect::recovery_required(
                        effect.bytes_written,
                        format!(
                            "ExternalConfig este durabil, dar finalizarea WAL a eșuat: {error} Nu repeta operația automat."
                        ),
                    );
                }
                let receipt = WriteReceipt {
                    id: id.clone(),
                    category: intent.category,
                    owner: intent.owner,
                    operation: intent.operation,
                    target: intent.target.public_label.clone(),
                    bytes_written: effect.bytes_written,
                    started_at_ms,
                    completed_at_ms: now_ms(),
                    status: receipt_status(&effect, "committed"),
                };
                self.finish_effect(&id, &intent, receipt, &effect)
            }
            Err(error) => {
                if let Err(wal_error) = guard.abort_no_effect() {
                    return Err(self.wal_recovery_error(
                        &id,
                        &intent,
                        started_at_ms,
                        0,
                        format!(
                            "ExternalConfig a fost refuzat înainte de efect, dar WAL cleanup nu este durabil: {error} {wal_error}"
                        ),
                    ));
                }
                let diagnostic = Some(error.clone());
                let _ = self.log_write_event(
                    KernelEventKind::WriteFailed,
                    KernelLogLevel::Error,
                    &id,
                    &intent,
                    diagnostic,
                );
                Err(error.into())
            }
        }
    }

    pub fn create_directory_all(
        &self,
        intent: WriteIntent,
    ) -> Result<WriteReceipt, WriteAuthorityError> {
        if intent.operation != WriteOperationKind::CreateDirectory {
            return Err(
                "WriteAuthority.create_directory_all cere operație CreateDirectory.".into(),
            );
        }
        if intent.policy.atomicity != WriteAtomicity::FileLifecycle {
            return Err("WriteAuthority.create_directory_all cere politica FileLifecycle.".into());
        }
        self.with_authorized_intent(intent, |intent| {
            let id = operation_id(intent.owner, intent.operation);
            let started_at_ms = now_ms();
            let plan = plan_directory(&intent.target)?;
            let record = build_directory_wal_record(&id, started_at_ms, &intent, &plan)?;
            let runtime = self
                .app
                .try_state::<WriteAuthorityRuntime>()
                .ok_or_else(|| {
                    WriteAuthorityError::from(
                        "WriteAuthorityRuntime lipsește înainte de mkdir WAL prepare.",
                    )
                })?;
            let coordinator = runtime.recovery_coordinator()?;
            let mut guard = coordinator.begin(record)?;
            if let Err(log_error) = self.log_write_event(
                KernelEventKind::WritePlanned,
                KernelLogLevel::Info,
                &id,
                &intent,
                None,
            ) {
                return match guard.abort_no_effect() {
                    Ok(()) => Err(log_error.into()),
                    Err(wal_error) => Err(self.wal_recovery_error(
                        &id,
                        &intent,
                        started_at_ms,
                        0,
                        format!(
                            "WritePlanned mkdir a eșuat, iar WAL cleanup nu este durabil: {log_error} {wal_error}"
                        ),
                    )),
                };
            }

            let result = create_directory_all_wal(&intent.target, &plan, &mut guard);

            match result {
                Ok(mut effect) => {
                    if effect.recovery_required {
                        drop(guard);
                    } else if effect.changed {
                        if let Err(error) = guard.commit() {
                            effect = CapabilityEffect::recovery_required(
                                0,
                                format!(
                                    "Directorul este durabil, dar finalizarea WAL a eșuat: {error} Nu repeta operația automat."
                                ),
                            );
                        }
                    } else if let Err(error) = guard.abort_no_effect() {
                        effect = CapabilityEffect::recovery_required(
                            0,
                            format!(
                                "Operația mkdir era no-op, dar eliminarea WAL nu este durabilă: {error}"
                            ),
                        );
                    }
                    let completed_at_ms = now_ms();
                    let receipt = WriteReceipt {
                        id: id.clone(),
                        category: intent.category,
                        owner: intent.owner,
                        operation: intent.operation,
                        target: intent.target.public_label.clone(),
                        bytes_written: effect.bytes_written,
                        started_at_ms,
                        completed_at_ms,
                        status: receipt_status(
                            &effect,
                            if effect.changed {
                                "committed"
                            } else {
                                "skipped"
                            },
                        ),
                    };
                    self.finish_effect(&id, &intent, receipt, &effect)
                }
                Err(error) => {
                    if let Err(wal_error) = guard.abort_no_effect() {
                        return Err(self.wal_recovery_error(
                            &id,
                            &intent,
                            started_at_ms,
                            0,
                            format!(
                                "Mkdir a fost refuzat înainte de efect, dar WAL cleanup nu este durabil: {error} {wal_error}"
                            ),
                        ));
                    }
                    let diagnostic = Some(error.clone());
                    let _ = self.log_write_event(
                        KernelEventKind::WriteFailed,
                        KernelLogLevel::Error,
                        &id,
                        &intent,
                        diagnostic,
                    );
                    Err(error.into())
                }
            }
        })
    }

    pub fn copy_file(
        &self,
        intent: WriteIntent,
        source: &Path,
    ) -> Result<WriteReceipt, WriteAuthorityError> {
        if intent.operation != WriteOperationKind::Copy {
            return Err("WriteAuthority.copy_file cere operație Copy.".into());
        }
        if intent.policy.atomicity != WriteAtomicity::FileLifecycle {
            return Err("WriteAuthority.copy_file cere politica FileLifecycle.".into());
        }
        self.with_authorized_intent(intent, |intent| {
            let id = operation_id(intent.owner, intent.operation);
            let started_at_ms = now_ms();
            let replace_policy = copy_replace_policy(intent.owner)?;
            let runtime = self
                .app
                .try_state::<WriteAuthorityRuntime>()
                .ok_or_else(|| {
                    WriteAuthorityError::from(
                        "WriteAuthorityRuntime lipsește înainte de copy WAL prepare.",
                    )
                })?;
            let coordinator = runtime.recovery_coordinator()?;
            let _copy_io = coordinator.acquire_copy_io()?;
            let plan = plan_copy(&intent.target, source, replace_policy, &id)?;
            let record = build_copy_wal_record(&id, started_at_ms, &intent, &plan)?;
            let mut guard = coordinator.begin(record)?;
            if let Err(log_error) = self.log_write_event(
                KernelEventKind::WritePlanned,
                KernelLogLevel::Info,
                &id,
                &intent,
                None,
            ) {
                return match guard.abort_no_effect() {
                    Ok(()) => Err(log_error.into()),
                    Err(wal_error) => Err(self.wal_recovery_error(
                        &id,
                        &intent,
                        started_at_ms,
                        0,
                        format!(
                            "WritePlanned copy a eșuat, iar WAL cleanup nu este durabil: {log_error} {wal_error}"
                        ),
                    )),
                };
            }

            let result = copy_file_wal(
                &intent.target,
                source,
                replace_policy,
                plan,
                &mut guard,
            );

            match result {
                Ok(mut effect) => {
                    if effect.recovery_required {
                        drop(guard);
                    } else if let Err(error) = guard.commit() {
                        effect = CapabilityEffect::recovery_required(
                            effect.bytes_written,
                            format!(
                                "Copia este durabilă, dar finalizarea WAL a eșuat: {error} Nu repeta operația automat."
                            ),
                        );
                    }
                    let completed_at_ms = now_ms();
                    let receipt = WriteReceipt {
                        id: id.clone(),
                        category: intent.category,
                        owner: intent.owner,
                        operation: intent.operation,
                        target: intent.target.public_label.clone(),
                        bytes_written: effect.bytes_written,
                        started_at_ms,
                        completed_at_ms,
                        status: receipt_status(&effect, "committed"),
                    };
                    self.finish_effect(&id, &intent, receipt, &effect)
                }
                Err(error) => {
                    if let Err(wal_error) = guard.abort_no_effect() {
                        return Err(self.wal_recovery_error(
                            &id,
                            &intent,
                            started_at_ms,
                            0,
                            format!(
                                "Copy a fost refuzat înainte de efect, dar WAL cleanup nu este durabil: {error} {wal_error}"
                            ),
                        ));
                    }
                    let diagnostic = Some(error.clone());
                    let _ = self.log_write_event(
                        KernelEventKind::WriteFailed,
                        KernelLogLevel::Error,
                        &id,
                        &intent,
                        diagnostic,
                    );
                    Err(error.into())
                }
            }
        })
    }

    pub fn symlink_entry(
        &self,
        intent: WriteIntent,
        source: &Path,
    ) -> Result<WriteReceipt, WriteAuthorityError> {
        if intent.operation != WriteOperationKind::Symlink {
            return Err("WriteAuthority.symlink_entry cere operație Symlink.".into());
        }
        if intent.policy.atomicity != WriteAtomicity::FileLifecycle {
            return Err("WriteAuthority.symlink_entry cere politica FileLifecycle.".into());
        }
        self.with_authorized_intent(intent, |intent| {
            let id = operation_id(intent.owner, intent.operation);
            let started_at_ms = now_ms();
            let plan = plan_symlink(&intent.target, source)?;
            let record = build_symlink_wal_record(&id, started_at_ms, &intent, &plan)?;
            let runtime = self
                .app
                .try_state::<WriteAuthorityRuntime>()
                .ok_or_else(|| {
                    WriteAuthorityError::from(
                        "WriteAuthorityRuntime lipsește înainte de symlink WAL prepare.",
                    )
                })?;
            let coordinator = runtime.recovery_coordinator()?;
            let mut guard = coordinator.begin(record)?;
            if let Err(log_error) = self.log_write_event(
                KernelEventKind::WritePlanned,
                KernelLogLevel::Info,
                &id,
                &intent,
                None,
            ) {
                return match guard.abort_no_effect() {
                    Ok(()) => Err(log_error.into()),
                    Err(wal_error) => Err(self.wal_recovery_error(
                        &id,
                        &intent,
                        started_at_ms,
                        0,
                        format!(
                            "WritePlanned symlink a eșuat, iar WAL cleanup nu este durabil: {log_error} {wal_error}"
                        ),
                    )),
                };
            }

            let result = symlink_entry_wal(&intent.target, source, &plan, &mut guard);

            match result {
                Ok(mut effect) => {
                    if effect.recovery_required {
                        drop(guard);
                    } else if effect.changed {
                        if let Err(error) = guard.commit() {
                            effect = CapabilityEffect::recovery_required(
                                0,
                                format!(
                                    "Symlink-ul este durabil, dar finalizarea WAL a eșuat: {error} Nu repeta operația automat."
                                ),
                            );
                        }
                    } else if let Err(error) = guard.abort_no_effect() {
                        effect = CapabilityEffect::recovery_required(
                            0,
                            format!(
                                "Symlink-ul era no-op, dar eliminarea WAL nu este durabilă: {error}"
                            ),
                        );
                    }
                    let completed_at_ms = now_ms();
                    let receipt = WriteReceipt {
                        id: id.clone(),
                        category: intent.category,
                        owner: intent.owner,
                        operation: intent.operation,
                        target: intent.target.public_label.clone(),
                        bytes_written: effect.bytes_written,
                        started_at_ms,
                        completed_at_ms,
                        status: receipt_status(
                            &effect,
                            if effect.changed {
                                "committed"
                            } else {
                                "skipped"
                            },
                        ),
                    };
                    self.finish_effect(&id, &intent, receipt, &effect)
                }
                Err(error) => {
                    if let Err(wal_error) = guard.abort_no_effect() {
                        return Err(self.wal_recovery_error(
                            &id,
                            &intent,
                            started_at_ms,
                            0,
                            format!(
                                "Symlink-ul a fost refuzat înainte de efect, dar WAL cleanup nu este durabil: {error} {wal_error}"
                            ),
                        ));
                    }
                    let diagnostic = Some(error.clone());
                    let _ = self.log_write_event(
                        KernelEventKind::WriteFailed,
                        KernelLogLevel::Error,
                        &id,
                        &intent,
                        diagnostic,
                    );
                    Err(error.into())
                }
            }
        })
    }

    pub fn remove_file_if_exists(
        &self,
        intent: WriteIntent,
    ) -> Result<WriteReceipt, WriteAuthorityError> {
        if intent.operation != WriteOperationKind::RemoveFile {
            return Err("WriteAuthority.remove_file_if_exists cere operație RemoveFile.".into());
        }
        if intent.policy.atomicity != WriteAtomicity::FileLifecycle {
            return Err("WriteAuthority.remove_file_if_exists cere politica FileLifecycle.".into());
        }
        self.with_authorized_intent(intent, |intent| {
            let id = operation_id(intent.owner, intent.operation);
            let started_at_ms = now_ms();
            let Some(plan) = plan_remove_leaf(&intent.target, &id)? else {
                self.log_write_event(
                    KernelEventKind::WritePlanned,
                    KernelLogLevel::Info,
                    &id,
                    &intent,
                    None,
                )?;
                let effect = CapabilityEffect::unchanged();
                let receipt = WriteReceipt {
                    id: id.clone(),
                    category: intent.category,
                    owner: intent.owner,
                    operation: intent.operation,
                    target: intent.target.public_label.clone(),
                    bytes_written: 0,
                    started_at_ms,
                    completed_at_ms: now_ms(),
                    status: "skipped".into(),
                };
                return self.finish_effect(&id, &intent, receipt, &effect);
            };
            let record = build_remove_leaf_wal_record(&id, started_at_ms, &intent, &plan)?;
            let runtime = self
                .app
                .try_state::<WriteAuthorityRuntime>()
                .ok_or_else(|| {
                    WriteAuthorityError::from(
                        "WriteAuthorityRuntime lipsește înainte de RemoveFile WAL prepare.",
                    )
                })?;
            let coordinator = runtime.recovery_coordinator()?;
            let mut guard = coordinator.begin(record)?;
            if let Err(log_error) = self.log_write_event(
                KernelEventKind::WritePlanned,
                KernelLogLevel::Info,
                &id,
                &intent,
                None,
            ) {
                return match guard.abort_no_effect() {
                    Ok(()) => Err(log_error.into()),
                    Err(wal_error) => Err(self.wal_recovery_error(
                        &id,
                        &intent,
                        started_at_ms,
                        0,
                        format!(
                            "WritePlanned RemoveFile a eșuat, iar WAL cleanup nu este durabil: {log_error} {wal_error}"
                        ),
                    )),
                };
            }

            let result = remove_leaf_wal(&intent.target, plan, &mut guard);
            match result {
                Ok(mut effect) => {
                    if effect.recovery_required {
                        drop(guard);
                    } else if effect.changed {
                        if let Err(error) = guard.commit() {
                            effect = CapabilityEffect::recovery_required(
                                0,
                                format!(
                                    "RemoveFile este durabil, dar finalizarea WAL a eșuat: {error} Nu repeta operația automat."
                                ),
                            );
                        }
                    } else if let Err(error) = guard.abort_no_effect() {
                        effect = CapabilityEffect::recovery_required(
                            0,
                            format!(
                                "RemoveFile era no-op, dar eliminarea WAL nu este durabilă: {error}"
                            ),
                        );
                    }
                    let completed_at_ms = now_ms();
                    let receipt = WriteReceipt {
                        id: id.clone(),
                        category: intent.category,
                        owner: intent.owner,
                        operation: intent.operation,
                        target: intent.target.public_label.clone(),
                        bytes_written: effect.bytes_written,
                        started_at_ms,
                        completed_at_ms,
                        status: receipt_status(
                            &effect,
                            if effect.changed {
                                "committed"
                            } else {
                                "skipped"
                            },
                        ),
                    };
                    self.finish_effect(&id, &intent, receipt, &effect)
                }
                Err(error) => {
                    if let Err(wal_error) = guard.abort_no_effect() {
                        return Err(self.wal_recovery_error(
                            &id,
                            &intent,
                            started_at_ms,
                            0,
                            format!(
                                "RemoveFile a fost refuzat înainte de efect, dar WAL cleanup nu este durabil: {error} {wal_error}"
                            ),
                        ));
                    }
                    let diagnostic = Some(error.clone());
                    let _ = self.log_write_event(
                        KernelEventKind::WriteFailed,
                        KernelLogLevel::Error,
                        &id,
                        &intent,
                        diagnostic,
                    );
                    Err(error.into())
                }
            }
        })
    }

    pub fn remove_directory_tree_if_exists(
        &self,
        intent: WriteIntent,
    ) -> Result<WriteReceipt, WriteAuthorityError> {
        if intent.operation != WriteOperationKind::RemoveDirectoryTree {
            return Err(
                "WriteAuthority.remove_directory_tree_if_exists cere operație RemoveDirectoryTree."
                    .into(),
            );
        }
        if intent.policy.atomicity != WriteAtomicity::FileLifecycle {
            return Err(
                "WriteAuthority.remove_directory_tree_if_exists cere politica FileLifecycle."
                    .into(),
            );
        }
        self.with_authorized_intent(intent, |intent| {
            let id = operation_id(intent.owner, intent.operation);
            let started_at_ms = now_ms();
            let Some(plan) = plan_remove_tree(&intent.target, &id)? else {
                self.log_write_event(
                    KernelEventKind::WritePlanned,
                    KernelLogLevel::Info,
                    &id,
                    &intent,
                    None,
                )?;
                let effect = CapabilityEffect::unchanged();
                let receipt = WriteReceipt {
                    id: id.clone(),
                    category: intent.category,
                    owner: intent.owner,
                    operation: intent.operation,
                    target: intent.target.public_label.clone(),
                    bytes_written: 0,
                    started_at_ms,
                    completed_at_ms: now_ms(),
                    status: "skipped".into(),
                };
                return self.finish_effect(&id, &intent, receipt, &effect);
            };
            let record = build_remove_tree_wal_record(&id, started_at_ms, &intent, &plan)?;
            let runtime = self
                .app
                .try_state::<WriteAuthorityRuntime>()
                .ok_or_else(|| {
                    WriteAuthorityError::from(
                        "WriteAuthorityRuntime lipsește înainte de RemoveDirectoryTree WAL prepare.",
                    )
                })?;
            let coordinator = runtime.recovery_coordinator()?;
            let mut guard = coordinator.begin(record)?;
            if let Err(log_error) = self.log_write_event(
                KernelEventKind::WritePlanned,
                KernelLogLevel::Info,
                &id,
                &intent,
                None,
            ) {
                return match guard.abort_no_effect() {
                    Ok(()) => Err(log_error.into()),
                    Err(wal_error) => Err(self.wal_recovery_error(
                        &id,
                        &intent,
                        started_at_ms,
                        0,
                        format!(
                            "WritePlanned RemoveDirectoryTree a eșuat, iar WAL cleanup nu este durabil: {log_error} {wal_error}"
                        ),
                    )),
                };
            }

            let result = remove_tree_wal(&intent.target, plan, &mut guard);
            match result {
                Ok(mut effect) => {
                    if effect.recovery_required {
                        drop(guard);
                    } else if effect.changed {
                        if let Err(error) = guard.commit() {
                            effect = CapabilityEffect::recovery_required(
                                0,
                                format!(
                                    "RemoveDirectoryTree este durabil, dar finalizarea WAL a eșuat: {error} Nu repeta operația automat."
                                ),
                            );
                        }
                    } else if let Err(error) = guard.abort_no_effect() {
                        effect = CapabilityEffect::recovery_required(
                            0,
                            format!(
                                "RemoveDirectoryTree era no-op, dar eliminarea WAL nu este durabilă: {error}"
                            ),
                        );
                    }
                    let completed_at_ms = now_ms();
                    let receipt = WriteReceipt {
                        id: id.clone(),
                        category: intent.category,
                        owner: intent.owner,
                        operation: intent.operation,
                        target: intent.target.public_label.clone(),
                        bytes_written: effect.bytes_written,
                        started_at_ms,
                        completed_at_ms,
                        status: receipt_status(
                            &effect,
                            if effect.changed {
                                "committed"
                            } else {
                                "skipped"
                            },
                        ),
                    };
                    self.finish_effect(&id, &intent, receipt, &effect)
                }
                Err(error) => {
                    if let Err(wal_error) = guard.abort_no_effect() {
                        return Err(self.wal_recovery_error(
                            &id,
                            &intent,
                            started_at_ms,
                            0,
                            format!(
                                "RemoveDirectoryTree a fost refuzat înainte de efect, dar WAL cleanup nu este durabil: {error} {wal_error}"
                            ),
                        ));
                    }
                    let diagnostic = Some(error.clone());
                    let _ = self.log_write_event(
                        KernelEventKind::WriteFailed,
                        KernelLogLevel::Error,
                        &id,
                        &intent,
                        diagnostic,
                    );
                    Err(error.into())
                }
            }
        })
    }

    pub fn rename_entry(
        &self,
        intent: WriteIntent,
        destination: WriteTarget,
    ) -> Result<WriteReceipt, WriteAuthorityError> {
        if intent.operation != WriteOperationKind::Rename {
            return Err("WriteAuthority.rename_entry cere operație Rename.".into());
        }
        if intent.policy.atomicity != WriteAtomicity::FileLifecycle {
            return Err("WriteAuthority.rename_entry cere politica FileLifecycle.".into());
        }
        self.with_authorized_pair(intent, destination, |intent, destination| {
            if intent.policy.conflict == ConflictPolicy::RequireDiskBaseline
                && destination.expected_leaf != ExpectedLeaf::Absent
            {
                return Err(format!(
                    "WriteAuthority a blocat rename {}: destinația RequireDiskBaseline trebuie să declare expected leaf Absent.",
                    destination.public_label
                )
                .into());
            }

            let id = operation_id(intent.owner, intent.operation);
            let started_at_ms = now_ms();
            let plan = plan_rename(&intent.target, &destination)?;
            let record = build_rename_wal_record(&id, started_at_ms, &intent, &plan)?;
            let runtime = self
                .app
                .try_state::<WriteAuthorityRuntime>()
                .ok_or_else(|| {
                    WriteAuthorityError::from(
                        "WriteAuthorityRuntime lipsește înainte de rename WAL prepare.",
                    )
                })?;
            let coordinator = runtime.recovery_coordinator()?;
            let mut guard = coordinator.begin(record)?;
            if let Err(log_error) = self.log_write_event(
                KernelEventKind::WritePlanned,
                KernelLogLevel::Info,
                &id,
                &intent,
                None,
            ) {
                return match guard.abort_no_effect() {
                    Ok(()) => Err(log_error.into()),
                    Err(wal_error) => Err(self.wal_recovery_error(
                        &id,
                        &intent,
                        started_at_ms,
                        0,
                        format!(
                            "WritePlanned rename a eșuat, iar WAL cleanup nu este durabil: {log_error} {wal_error}"
                        ),
                    )),
                };
            }

            let result = rename_entry_wal(&intent.target, &destination, plan, &mut guard);
            match result {
                Ok(mut effect) => {
                    if effect.recovery_required {
                        drop(guard);
                    } else if effect.changed {
                        if let Err(error) = guard.commit() {
                            effect = CapabilityEffect::recovery_required(
                                0,
                                format!(
                                    "Rename-ul este durabil, dar finalizarea WAL a eșuat: {error} Nu repeta operația automat."
                                ),
                            );
                        }
                    } else if let Err(error) = guard.abort_no_effect() {
                        effect = CapabilityEffect::recovery_required(
                            0,
                            format!(
                                "Rename-ul era no-op, dar eliminarea WAL nu este durabilă: {error}"
                            ),
                        );
                    }
                    let completed_at_ms = now_ms();
                    let receipt = WriteReceipt {
                        id: id.clone(),
                        category: intent.category,
                        owner: intent.owner,
                        operation: intent.operation,
                        target: format!(
                            "{} -> {}",
                            intent.target.public_label, destination.public_label
                        ),
                        bytes_written: effect.bytes_written,
                        started_at_ms,
                        completed_at_ms,
                        status: receipt_status(&effect, "committed"),
                    };
                    self.finish_effect(&id, &intent, receipt, &effect)
                }
                Err(error) => {
                    if let Err(wal_error) = guard.abort_no_effect() {
                        return Err(self.wal_recovery_error(
                            &id,
                            &intent,
                            started_at_ms,
                            0,
                            format!(
                                "Rename a fost refuzat înainte de efect, dar WAL cleanup nu este durabil: {error} {wal_error}"
                            ),
                        ));
                    }
                    let diagnostic = Some(error.clone());
                    let _ = self.log_write_event(
                        KernelEventKind::WriteFailed,
                        KernelLogLevel::Error,
                        &id,
                        &intent,
                        diagnostic,
                    );
                    Err(error.into())
                }
            }
        })
    }

    fn execute_write(
        &self,
        intent: WriteIntent,
        bytes: &[u8],
    ) -> Result<WriteReceipt, WriteAuthorityError> {
        self.with_authorized_intent(intent, |intent| {
            let id = operation_id(intent.owner, intent.operation);
            let started_at_ms = now_ms();
            let replace_policy = match &intent.target.expected_leaf {
                ExpectedLeaf::Absent => CapabilityReplacePolicy::CreateNew,
                ExpectedLeaf::Present(_) | ExpectedLeaf::Unspecified => {
                    if intent.policy.conflict == ConflictPolicy::RequireExplicitOverride {
                        CapabilityReplacePolicy::CreateNew
                    } else {
                        CapabilityReplacePolicy::Replace
                    }
                }
            };
            if intent.policy.recovery == RecoveryPolicy::EphemeralRebuildable {
                return self.execute_rebuildable_atomic_write(
                    intent,
                    bytes,
                    replace_policy,
                    id,
                    started_at_ms,
                );
            }
            let plan = plan_atomic_write(&intent.target, bytes, replace_policy, &id)?;
            let record = build_atomic_wal_record(&id, started_at_ms, &intent, &plan)?;
            let runtime = self
                .app
                .try_state::<WriteAuthorityRuntime>()
                .ok_or_else(|| {
                    WriteAuthorityError::from(
                        "WriteAuthorityRuntime lipsește înainte de WAL prepare.",
                    )
                })?;
            let coordinator = runtime.recovery_coordinator()?;
            let mut guard = coordinator.begin(record)?;
            if let Err(log_error) = self.log_write_event(
                KernelEventKind::WritePlanned,
                KernelLogLevel::Info,
                &id,
                &intent,
                None,
            ) {
                return match guard.abort_no_effect() {
                    Ok(()) => Err(log_error.into()),
                    Err(wal_error) => Err(self.wal_recovery_error(
                        &id,
                        &intent,
                        started_at_ms,
                        0,
                        format!(
                            "WritePlanned a eșuat înainte de target, iar WAL cleanup nu este durabil: {log_error} {wal_error}"
                        ),
                    )),
                };
            }

            let result = atomic_write_wal(
                &intent.target,
                bytes,
                replace_policy,
                &plan,
                &mut guard,
            );
            match result {
                Ok(mut effect) => {
                    if !effect.recovery_required {
                        if let Err(error) = guard.commit() {
                            effect = CapabilityEffect::recovery_required(
                                effect.bytes_written,
                                format!(
                                    "Target-ul este durabil, dar eliminarea/finalizarea WAL a eșuat: {error} Nu repeta operația automat."
                                ),
                            );
                        }
                    } else {
                        drop(guard);
                    }
                    let completed_at_ms = now_ms();
                    let receipt = WriteReceipt {
                        id: id.clone(),
                        category: intent.category,
                        owner: intent.owner,
                        operation: intent.operation,
                        target: intent.target.public_label.clone(),
                        bytes_written: effect.bytes_written,
                        started_at_ms,
                        completed_at_ms,
                        status: receipt_status(&effect, "committed"),
                    };
                    self.finish_effect(&id, &intent, receipt, &effect)
                }
                Err(error) => {
                    if let Err(wal_error) = guard.abort_no_effect() {
                        return Err(self.wal_recovery_error(
                            &id,
                            &intent,
                            started_at_ms,
                            0,
                            format!(
                                "Operația a fost refuzată înainte de primul efect, dar WAL cleanup nu este durabil: {error} {wal_error}"
                            ),
                        ));
                    }
                    let diagnostic = Some(error.clone());
                    let _ = self.log_write_event(
                        KernelEventKind::WriteFailed,
                        KernelLogLevel::Error,
                        &id,
                        &intent,
                        diagnostic,
                    );
                    Err(error.into())
                }
            }
        })
    }

    fn execute_rebuildable_atomic_write(
        &self,
        intent: WriteIntent,
        bytes: &[u8],
        replace_policy: CapabilityReplacePolicy,
        id: String,
        started_at_ms: u128,
    ) -> Result<WriteReceipt, WriteAuthorityError> {
        self.log_write_event(
            KernelEventKind::WritePlanned,
            KernelLogLevel::Info,
            &id,
            &intent,
            None,
        )?;
        match atomic_write(&intent.target, bytes, replace_policy) {
            Ok(effect) => {
                let receipt = WriteReceipt {
                    id: id.clone(),
                    category: intent.category,
                    owner: intent.owner,
                    operation: intent.operation,
                    target: intent.target.public_label.clone(),
                    bytes_written: effect.bytes_written,
                    started_at_ms,
                    completed_at_ms: now_ms(),
                    status: receipt_status(&effect, "committed"),
                };
                self.finish_effect(&id, &intent, receipt, &effect)
            }
            Err(error) => {
                let _ = self.log_write_event(
                    KernelEventKind::WriteFailed,
                    KernelLogLevel::Error,
                    &id,
                    &intent,
                    Some(error.clone()),
                );
                Err(error.into())
            }
        }
    }

    fn wal_recovery_error(
        &self,
        id: &str,
        intent: &WriteIntent,
        started_at_ms: u128,
        bytes_written: u64,
        diagnostic: String,
    ) -> WriteAuthorityError {
        let receipt = WriteReceipt {
            id: id.to_string(),
            category: intent.category,
            owner: intent.owner,
            operation: intent.operation,
            target: intent.target.public_label.clone(),
            bytes_written,
            started_at_ms,
            completed_at_ms: now_ms(),
            status: "recovery_required".into(),
        };
        WriteAuthorityError::RecoveryRequired(Box::new(WriteRecoveryReceipt::new(
            receipt, diagnostic,
        )))
    }

    fn execute_append(
        &self,
        intent: WriteIntent,
        bytes: &[u8],
    ) -> Result<WriteReceipt, WriteAuthorityError> {
        self.with_authorized_intent(intent, |intent| {
            let id = operation_id(intent.owner, intent.operation);
            let started_at_ms = now_ms();
            let plan = plan_append(&intent.target, bytes)?;
            let record = build_append_wal_record(&id, started_at_ms, &intent, &plan)?;
            let runtime = self
                .app
                .try_state::<WriteAuthorityRuntime>()
                .ok_or_else(|| {
                    WriteAuthorityError::from(
                        "WriteAuthorityRuntime lipsește înainte de append WAL prepare.",
                    )
                })?;
            let coordinator = runtime.recovery_coordinator()?;
            let mut guard = coordinator.begin(record)?;
            if let Err(log_error) = self.log_write_event(
                KernelEventKind::WritePlanned,
                KernelLogLevel::Info,
                &id,
                &intent,
                None,
            ) {
                return match guard.abort_no_effect() {
                    Ok(()) => Err(log_error.into()),
                    Err(wal_error) => Err(self.wal_recovery_error(
                        &id,
                        &intent,
                        started_at_ms,
                        0,
                        format!(
                            "WritePlanned append a eșuat, iar WAL cleanup nu este durabil: {log_error} {wal_error}"
                        ),
                    )),
                };
            }

            let result = append_wal(&intent.target, bytes, plan, &mut guard);
            match result {
                Ok(mut effect) => {
                    if !effect.recovery_required {
                        if let Err(error) = guard.commit() {
                            effect = CapabilityEffect::recovery_required(
                                effect.bytes_written,
                                format!(
                                    "Append-ul este durabil, dar finalizarea WAL a eșuat: {error} Nu repeta operația automat."
                                ),
                            );
                        }
                    } else {
                        drop(guard);
                    }
                    let completed_at_ms = now_ms();
                    let receipt = WriteReceipt {
                        id: id.clone(),
                        category: intent.category,
                        owner: intent.owner,
                        operation: intent.operation,
                        target: intent.target.public_label.clone(),
                        bytes_written: effect.bytes_written,
                        started_at_ms,
                        completed_at_ms,
                        status: receipt_status(&effect, "committed"),
                    };
                    self.finish_effect(&id, &intent, receipt, &effect)
                }
                Err(error) => {
                    if let Err(wal_error) = guard.abort_no_effect() {
                        return Err(self.wal_recovery_error(
                            &id,
                            &intent,
                            started_at_ms,
                            0,
                            format!(
                                "Append-ul a fost refuzat înainte de efect, dar WAL cleanup nu este durabil: {error} {wal_error}"
                            ),
                        ));
                    }
                    let diagnostic = Some(error.clone());
                    let _ = self.log_write_event(
                        KernelEventKind::WriteFailed,
                        KernelLogLevel::Error,
                        &id,
                        &intent,
                        diagnostic,
                    );
                    Err(error.into())
                }
            }
        })
    }

    fn with_authorized_intent<T>(
        &self,
        intent: WriteIntent,
        action: impl FnOnce(WriteIntent) -> Result<T, WriteAuthorityError>,
    ) -> Result<T, WriteAuthorityError> {
        // Validate the caller claim first for diagnostics, but never use it as
        // the execution authority. The runtime binder replaces boundary_root
        // with a sealed, process/session-owned directory handle.
        self.validate_boundary(&intent)?;
        self.require_user_source_authority(&intent)?;
        let runtime = self
            .app
            .try_state::<WriteAuthorityRuntime>()
            .ok_or_else(|| {
                WriteAuthorityError::from(
                    "WriteAuthorityRuntime lipsește; scrierea este blocată fail-closed.",
                )
            })?;
        let lease = runtime.acquire_write_lease(&intent)?;
        let mut intent = intent;
        intent.target = lease.bind_target(&intent.target)?;
        self.validate_boundary(&intent)?;
        validate_authority_path(&intent)?;
        let result = action(intent);
        drop(lease);
        result
    }

    fn with_authorized_pair<T>(
        &self,
        intent: WriteIntent,
        destination: WriteTarget,
        action: impl FnOnce(WriteIntent, WriteTarget) -> Result<T, WriteAuthorityError>,
    ) -> Result<T, WriteAuthorityError> {
        self.validate_boundary(&intent)?;
        self.require_user_source_authority(&intent)?;
        validate_target_boundary(&destination, BoundaryRules::WRITE)?;
        let runtime = self
            .app
            .try_state::<WriteAuthorityRuntime>()
            .ok_or_else(|| {
                WriteAuthorityError::from(
                    "WriteAuthorityRuntime lipsește; operația compusă este blocată fail-closed.",
                )
            })?;
        let lease = runtime.acquire_write_lease(&intent)?;
        let mut intent = intent;
        intent.target = lease.bind_target(&intent.target)?;
        let destination = lease.bind_target(&destination)?;
        self.validate_boundary(&intent)?;
        validate_target_boundary(&destination, BoundaryRules::WRITE)?;
        validate_authority_path(&intent)?;
        validate_companion_authority_path(&intent, &destination)?;
        let result = action(intent, destination);
        drop(lease);
        result
    }

    fn require_user_source_authority(
        &self,
        intent: &WriteIntent,
    ) -> Result<(), WriteAuthorityError> {
        if intent.category != WriteCategory::ProjectSourceWrite {
            return Ok(());
        }
        let Some(state) = self.app.try_state::<AppState>() else {
            // Unit-test and recovery harnesses may exercise WriteAuthority in
            // isolation. The production builder always manages AppState.
            return Ok(());
        };
        state
            .ai_coordination
            .require_user_source_mutation()
            .map_err(|error| WriteAuthorityError::from(error.to_string()))
    }

    fn validate_boundary(&self, intent: &WriteIntent) -> Result<(), String> {
        if matching_write_declaration(intent).is_none() {
            return Err(format!(
                "Scriere blocată: combinația {:?}/{:?}/{:?} cu politica {:?}/{:?}/{:?} nu este declarată în Write Registry.",
                intent.category,
                intent.owner,
                intent.operation,
                intent.policy.atomicity,
                intent.policy.conflict,
                intent.policy.recovery
            ));
        }
        self.validate_expected_leaf_contract(intent)?;
        let rules = match intent.operation {
            WriteOperationKind::CreateDirectory => BoundaryRules::CREATE_DIRECTORY,
            WriteOperationKind::RemoveFile | WriteOperationKind::RemoveDirectoryTree => {
                if intent.operation == WriteOperationKind::RemoveFile {
                    BoundaryRules::UNLINK_LEAF
                } else {
                    BoundaryRules::WRITE
                }
            }
            WriteOperationKind::Rename => BoundaryRules::RENAME_SOURCE,
            _ => BoundaryRules::WRITE,
        };
        validate_target_boundary(&intent.target, rules)
    }

    fn validate_expected_leaf_contract(&self, intent: &WriteIntent) -> Result<(), String> {
        match intent.policy.conflict {
            ConflictPolicy::RequireDiskBaseline => match intent.operation {
                WriteOperationKind::WriteText | WriteOperationKind::WriteBytes => {
                    if intent.target.expected_leaf == ExpectedLeaf::Unspecified {
                        return Err(format!(
                            "WriteAuthority a blocat {}: politica RequireDiskBaseline cere expected leaf Absent sau Present.",
                            intent.target.public_label
                        ));
                    }
                }
                WriteOperationKind::RemoveFile
                | WriteOperationKind::RemoveDirectoryTree
                | WriteOperationKind::Rename => {
                    if !matches!(&intent.target.expected_leaf, ExpectedLeaf::Present(_)) {
                        return Err(format!(
                            "WriteAuthority a blocat {}: {:?} cu RequireDiskBaseline cere expected leaf Present.",
                            intent.target.public_label, intent.operation
                        ));
                    }
                }
                _ => {
                    return Err(format!(
                        "WriteAuthority a blocat {}: {:?} nu are contract leaf-CAS definit pentru RequireDiskBaseline.",
                        intent.target.public_label, intent.operation
                    ));
                }
            },
            ConflictPolicy::RequireExplicitOverride => {
                if intent.target.expected_leaf != ExpectedLeaf::Absent {
                    return Err(format!(
                        "WriteAuthority a blocat {}: politica create-only RequireExplicitOverride cere expected leaf Absent.",
                        intent.target.public_label
                    ));
                }
            }
            ConflictPolicy::SingleOwnerInternal | ConflictPolicy::ExternalBackupRequired => {}
        }
        Ok(())
    }

    fn log_write_event(
        &self,
        kind: KernelEventKind,
        level: KernelLogLevel,
        id: &str,
        intent: &WriteIntent,
        diagnostic: Option<String>,
    ) -> Result<(), String> {
        if !intent.policy.log_required {
            return Ok(());
        }
        let message = format!("{} [{}]", intent.description, id);
        let event = KernelLogEvent::new(
            level,
            kind,
            owner_label(intent.owner),
            category_label(intent.category),
            operation_label(intent.operation),
            Some(intent.target.public_label.clone()),
            message,
            diagnostic,
        )
        .with_attribute("pathExecution", "directory_handle_relative")
        .with_attribute("capabilityBackend", capability_backend_label());
        append_event(self.app, event)
    }

    fn log_effect_terminal_best_effort(
        &self,
        id: &str,
        intent: &WriteIntent,
        effect: &CapabilityEffect,
    ) {
        let (kind, level, diagnostic) = if effect.recovery_required {
            (
                KernelEventKind::WriteRecoveryRequired,
                KernelLogLevel::Error,
                effect.diagnostic.clone(),
            )
        } else {
            (KernelEventKind::WriteCommitted, KernelLogLevel::Info, None)
        };
        if let Err(error) = self.log_write_event(kind, level, id, intent, diagnostic) {
            // The filesystem effect is already visible (and may explicitly
            // require recovery) at this point. Never turn a secondary logging
            // failure into a retryable operation error.
            eprintln!("[Pană Studio] Write terminal observability append failed for {id}: {error}");
        }
    }

    fn finish_effect(
        &self,
        id: &str,
        intent: &WriteIntent,
        receipt: WriteReceipt,
        effect: &CapabilityEffect,
    ) -> Result<WriteReceipt, WriteAuthorityError> {
        self.log_effect_terminal_best_effort(id, intent, effect);
        if effect.recovery_required {
            let diagnostic = effect
                .diagnostic
                .clone()
                .unwrap_or_else(|| {
                    "Efectul filesystem este vizibil, dar starea lui durabilă este incertă; consultă kernel.write.recovery_required."
                        .to_string()
                });
            return Err(WriteAuthorityError::RecoveryRequired(Box::new(
                WriteRecoveryReceipt::new(receipt, diagnostic),
            )));
        }
        Ok(receipt)
    }
}

fn receipt_status(effect: &CapabilityEffect, durable_status: &str) -> String {
    if effect.recovery_required {
        "recovery_required".to_string()
    } else {
        durable_status.to_string()
    }
}

fn operation_id(owner: WriteOwner, operation: WriteOperationKind) -> String {
    let sequence = WRITE_OPERATION_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(
        "write-{}-{}-{}-{}-{}",
        owner_label(owner),
        operation_label(operation),
        now_ms(),
        std::process::id(),
        sequence
    )
}

#[cfg(target_os = "linux")]
fn capability_backend_label() -> &'static str {
    "rustix_at_linux"
}

#[cfg(not(target_os = "linux"))]
fn capability_backend_label() -> &'static str {
    "unsupported_fail_closed"
}

fn category_label(category: WriteCategory) -> &'static str {
    match category {
        WriteCategory::InternalAppWrite => "internal_app_write",
        WriteCategory::ProjectSourceWrite => "project_source_write",
        WriteCategory::PreviewWorkspaceWrite => "preview_workspace_write",
        WriteCategory::ExternalIntegrationWrite => "external_integration_write",
    }
}

fn owner_label(owner: WriteOwner) -> &'static str {
    match owner {
        WriteOwner::Kernel => "kernel",
        WriteOwner::ProjectSession => "project_session",
        WriteOwner::ProjectWorkspace => "project_workspace",
        WriteOwner::Workbench => "workbench",
        WriteOwner::ScratchState => "scratch_state",
        WriteOwner::AppConfig => "app_config",
        WriteOwner::McpContext => "mcp_context",
        WriteOwner::CodexMcp => "codex_mcp",
        WriteOwner::ProjectInitializer => "project_initializer",
        WriteOwner::Preview => "preview",
    }
}

fn operation_label(operation: WriteOperationKind) -> &'static str {
    match operation {
        WriteOperationKind::WriteText => "write_text",
        WriteOperationKind::AppendText => "append_text",
        WriteOperationKind::WriteBytes => "write_bytes",
        WriteOperationKind::RemoveFile => "remove_file",
        WriteOperationKind::RemoveDirectoryTree => "remove_directory_tree",
        WriteOperationKind::CreateDirectory => "create_directory",
        WriteOperationKind::Rename => "rename",
        WriteOperationKind::Copy => "copy",
        WriteOperationKind::Symlink => "symlink",
        WriteOperationKind::ExternalConfigUpdate => "external_config_update",
    }
}

fn copy_replace_policy(owner: WriteOwner) -> Result<CapabilityReplacePolicy, String> {
    match owner {
        WriteOwner::ProjectInitializer => Ok(CapabilityReplacePolicy::CreateNew),
        WriteOwner::Preview => Ok(CapabilityReplacePolicy::Replace),
        WriteOwner::Kernel
        | WriteOwner::ProjectSession
        | WriteOwner::ProjectWorkspace
        | WriteOwner::Workbench
        | WriteOwner::ScratchState
        | WriteOwner::AppConfig
        | WriteOwner::McpContext
        | WriteOwner::CodexMcp => Err(format!(
            "WriteAuthority Copy refuză ownerul {owner:?}; numai ProjectInitializer și Preview au contract copy."
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use tauri::Manager;

    use super::CapabilityEffect;
    use crate::kernel::write_authority::capability::{
        append, with_before_external_target_durable_test_hook,
        with_external_backup_committed_test_hook, with_external_baseline_relocated_test_hook,
        with_external_linkat_failure_test_hook, with_external_linkat_proc_fallback_test_hook,
        with_external_post_publication_test_hook,
    };
    use crate::{
        app_home::{ensure_app_home, TEST_APP_ENV_LOCK},
        kernel::write_authority::{
            test_support::install_test_project_authority, CodexConfigLease, ProjectBootstrapLease,
            WriteAuthority, WriteAuthorityError, WriteAuthorityRuntime, WriteCategory, WriteIntent,
            WriteOperationKind, WriteOwner, WritePolicy, WriteReceipt, WriteTarget,
        },
    };

    #[test]
    fn append_bytes_preserves_existing_entries() {
        let root = unique_test_dir("append-write");
        fs::create_dir_all(&root).unwrap();
        let target_path = root.join("transactions.jsonl");
        let target = WriteTarget::new(&target_path, &root, "test/transactions.jsonl");

        append(&target, b"{\"one\":true}\n").unwrap();
        append(&target, b"{\"two\":true}\n").unwrap();

        assert_eq!(
            fs::read_to_string(&target_path).unwrap(),
            "{\"one\":true}\n{\"two\":true}\n"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rebuildable_mcp_projection_commits_without_a_global_wal_record() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("mcp-rebuildable-projection");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");
        let app_home = ensure_app_home(app.handle()).expect("test app home should be available");
        app.state::<WriteAuthorityRuntime>()
            .boot_recovery()
            .expect("recovery bootstrap should be clean");
        let boundary = PathBuf::from(&app_home.config_dir);
        let target = PathBuf::from(&app_home.mcp_dir).join("mcp.json");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        let intent = WriteIntent::new(
            WriteCategory::InternalAppWrite,
            WriteOwner::McpContext,
            WriteOperationKind::WriteText,
            WriteTarget::new(target.clone(), boundary, "mcp/mcp.json"),
            WritePolicy::mcp_projection_atomic(),
            "Rebuildable MCP projection test.",
        );

        WriteAuthority::new(app.handle())
            .write_text(intent, "{\"live\":true}\n")
            .unwrap();

        assert_eq!(fs::read_to_string(target).unwrap(), "{\"live\":true}\n");
        let wal_records = fs::read_dir(&app_home.write_authority_wal_dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.path().extension().and_then(|value| value.to_str()) == Some("json")
            })
            .count();
        assert_eq!(wal_records, 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn finish_effect_returns_typed_recovery_with_complete_receipt() {
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");
        let mut policy = WritePolicy::internal_atomic();
        policy.log_required = false;
        let intent = WriteIntent::new(
            WriteCategory::InternalAppWrite,
            WriteOwner::Kernel,
            WriteOperationKind::WriteText,
            WriteTarget::new("/tmp/recovery.json", "/tmp", "test/recovery.json"),
            policy,
            "Typed recovery test.",
        );
        let receipt = WriteReceipt {
            id: "write-recovery-test".to_string(),
            category: intent.category,
            owner: intent.owner,
            operation: intent.operation,
            target: intent.target.public_label.clone(),
            bytes_written: 4,
            started_at_ms: 10,
            completed_at_ms: 20,
            status: "committed".to_string(),
        };
        let effect = CapabilityEffect {
            changed: true,
            bytes_written: 4,
            recovery_required: true,
            diagnostic: Some("durability uncertain".to_string()),
        };

        let error = WriteAuthority::new(app.handle())
            .finish_effect("write-recovery-test", &intent, receipt, &effect)
            .unwrap_err();

        let WriteAuthorityError::RecoveryRequired(recovery) = error else {
            panic!("recovery effect must not be downgraded to a rejection");
        };
        assert!(recovery.retry_forbidden());
        assert_eq!(recovery.diagnostic, "durability uncertain");
        assert_eq!(recovery.receipt.id, "write-recovery-test");
        assert_eq!(recovery.receipt.bytes_written, 4);
        assert_eq!(recovery.receipt.status, "recovery_required");
    }

    #[test]
    fn external_config_update_writes_backup_before_config() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("external-config-update");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let codex_dir = root.join("home/.codex");
        fs::create_dir_all(&codex_dir).unwrap();
        let config_path = codex_dir.join("config.toml");
        let backup_path = codex_dir.join("config.toml.pana-studio-test.bak");
        fs::write(&config_path, "old = true\n").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&config_path, fs::Permissions::from_mode(0o600)).unwrap();

        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");
        let app_handle = app.handle().clone();
        ensure_app_home(&app_handle).expect("test app home should be available");
        let lease = CodexConfigLease::capture(&codex_dir).unwrap();

        let intent = WriteIntent::new(
            WriteCategory::ExternalIntegrationWrite,
            WriteOwner::CodexMcp,
            WriteOperationKind::ExternalConfigUpdate,
            lease
                .target(config_path.clone(), "external:~/.codex/config.toml")
                .unwrap(),
            WritePolicy::external_config_update(),
            "Test external config update.",
        );

        WriteAuthority::new(&app_handle)
            .external_config_update(
                intent,
                "new = true\n",
                Some((
                    lease
                        .target(backup_path.clone(), "external:~/.codex/config.toml backup")
                        .unwrap(),
                    "old = true\n",
                )),
            )
            .unwrap();

        assert_eq!(fs::read_to_string(&config_path).unwrap(), "new = true\n");
        assert_eq!(fs::read_to_string(&backup_path).unwrap(), "old = true\n");
        #[cfg(unix)]
        {
            assert_eq!(
                fs::metadata(&config_path).unwrap().permissions().mode() & 0o7777,
                0o600
            );
            assert_eq!(
                fs::metadata(&backup_path).unwrap().permissions().mode() & 0o7777,
                0o600
            );
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn external_config_create_new_is_private_and_does_not_invent_backup() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("external-config-create-new");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let codex_dir = root.join("home/.codex");
        let config_path = codex_dir.join("config.toml");
        fs::create_dir_all(&codex_dir).unwrap();

        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");
        ensure_app_home(app.handle()).unwrap();
        let lease = CodexConfigLease::capture(&codex_dir).unwrap();
        let intent = WriteIntent::new(
            WriteCategory::ExternalIntegrationWrite,
            WriteOwner::CodexMcp,
            WriteOperationKind::ExternalConfigUpdate,
            lease
                .target(config_path.clone(), "external:~/.codex/config.toml")
                .unwrap(),
            WritePolicy::external_config_update(),
            "External config create-new must be private.",
        );

        let receipt = WriteAuthority::new(app.handle())
            .external_config_update(intent, "new = true\n", None)
            .unwrap();

        assert_eq!(receipt.status, "committed");
        assert_eq!(fs::read_to_string(&config_path).unwrap(), "new = true\n");
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(&config_path).unwrap().permissions().mode() & 0o7777,
            0o600
        );
        assert_eq!(fs::read_dir(&codex_dir).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn external_config_replace_commits_through_proc_fd_linkat_fallback() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("external-config-proc-fallback-replace");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let codex_dir = root.join("home/.codex");
        let config_path = codex_dir.join("config.toml");
        let backup_path = codex_dir.join("config.toml.pana-studio-proc-fallback.bak");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(&config_path, "old = true\n").unwrap();

        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");
        ensure_app_home(app.handle()).unwrap();
        let lease = CodexConfigLease::capture(&codex_dir).unwrap();
        let intent = WriteIntent::new(
            WriteCategory::ExternalIntegrationWrite,
            WriteOwner::CodexMcp,
            WriteOperationKind::ExternalConfigUpdate,
            lease
                .target(config_path.clone(), "external:~/.codex/config.toml")
                .unwrap(),
            WritePolicy::external_config_update(),
            "External config replace through proc fd linkat fallback.",
        );

        let receipt = with_external_linkat_proc_fallback_test_hook(|| {
            WriteAuthority::new(app.handle()).external_config_update(
                intent,
                "new = true\n",
                Some((
                    lease
                        .target(backup_path.clone(), "external:~/.codex/config.toml backup")
                        .unwrap(),
                    "old = true\n",
                )),
            )
        })
        .unwrap();

        assert_eq!(receipt.status, "committed");
        assert_eq!(fs::read_to_string(&config_path).unwrap(), "new = true\n");
        assert_eq!(fs::read_to_string(&backup_path).unwrap(), "old = true\n");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn external_config_create_new_commits_through_proc_fd_linkat_fallback() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("external-config-proc-fallback-create");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let codex_dir = root.join("home/.codex");
        let config_path = codex_dir.join("config.toml");
        fs::create_dir_all(&codex_dir).unwrap();

        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");
        ensure_app_home(app.handle()).unwrap();
        let lease = CodexConfigLease::capture(&codex_dir).unwrap();
        let intent = WriteIntent::new(
            WriteCategory::ExternalIntegrationWrite,
            WriteOwner::CodexMcp,
            WriteOperationKind::ExternalConfigUpdate,
            lease
                .target(config_path.clone(), "external:~/.codex/config.toml")
                .unwrap(),
            WritePolicy::external_config_update(),
            "External config create-new through proc fd linkat fallback.",
        );

        let receipt = with_external_linkat_proc_fallback_test_hook(|| {
            WriteAuthority::new(app.handle()).external_config_update(intent, "new = true\n", None)
        })
        .unwrap();

        assert_eq!(receipt.status, "committed");
        assert_eq!(fs::read_to_string(&config_path).unwrap(), "new = true\n");
        assert_eq!(fs::read_dir(&codex_dir).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn external_config_replace_authority_root_swap_never_false_commits() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("external-config-authority-swap-replace");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let codex_dir = root.join("home/.codex");
        let held_dir = root.join("home/.codex-held");
        let replacement_dir = root.join("home/.codex-replacement");
        let config_path = codex_dir.join("config.toml");
        let backup_path = codex_dir.join("config.toml.pana-studio-authority-swap.bak");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::create_dir_all(&replacement_dir).unwrap();
        fs::write(&config_path, "old = true\n").unwrap();
        fs::write(replacement_dir.join("config.toml"), "competitor = true\n").unwrap();

        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        ensure_app_home(app.handle()).unwrap();
        let lease = CodexConfigLease::capture(&codex_dir).unwrap();
        let intent = WriteIntent::new(
            WriteCategory::ExternalIntegrationWrite,
            WriteOwner::CodexMcp,
            WriteOperationKind::ExternalConfigUpdate,
            lease
                .target(config_path.clone(), "external:~/.codex/config.toml")
                .unwrap(),
            WritePolicy::external_config_update(),
            "External config replace must retain its public authority binding.",
        );
        let backup_target = lease
            .target(backup_path, "external:authority-swap-backup")
            .unwrap();
        let swap_source = codex_dir.clone();
        let swap_held = held_dir.clone();
        let swap_replacement = replacement_dir.clone();

        let error = with_external_baseline_relocated_test_hook(
            move || {
                fs::rename(&swap_source, &swap_held).unwrap();
                fs::rename(&swap_replacement, &swap_source).unwrap();
            },
            || {
                WriteAuthority::new(app.handle()).external_config_update(
                    intent,
                    "new = true\n",
                    Some((backup_target, "old = true\n")),
                )
            },
        )
        .unwrap_err();

        assert!(matches!(error, WriteAuthorityError::RecoveryRequired(_)));
        assert_eq!(
            fs::read_to_string(&config_path).unwrap(),
            "competitor = true\n"
        );
        assert_eq!(
            fs::read_to_string(held_dir.join("config.toml")).unwrap(),
            "new = true\n"
        );
        assert_eq!(
            fs::read_to_string(held_dir.join("config.toml.pana-studio-authority-swap.bak"))
                .unwrap(),
            "old = true\n"
        );
        let scan = app
            .state::<WriteAuthorityRuntime>()
            .recovery_scan()
            .unwrap();
        assert!(scan.blocked, "{scan:?}");
        assert_eq!(
            scan.items[0].classification,
            crate::kernel::write_authority::WriteAuthorityRecoveryClassification::Conflict
        );
        assert_eq!(
            fs::read_to_string(&config_path).unwrap(),
            "competitor = true\n"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn external_config_create_new_authority_root_swap_never_false_commits() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("external-config-authority-swap-create");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let codex_dir = root.join("home/.codex");
        let held_dir = root.join("home/.codex-held");
        let replacement_dir = root.join("home/.codex-replacement");
        let config_path = codex_dir.join("config.toml");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::create_dir_all(&replacement_dir).unwrap();
        fs::write(replacement_dir.join("config.toml"), "competitor = true\n").unwrap();

        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        ensure_app_home(app.handle()).unwrap();
        let lease = CodexConfigLease::capture(&codex_dir).unwrap();
        let intent = WriteIntent::new(
            WriteCategory::ExternalIntegrationWrite,
            WriteOwner::CodexMcp,
            WriteOperationKind::ExternalConfigUpdate,
            lease
                .target(config_path.clone(), "external:~/.codex/config.toml")
                .unwrap(),
            WritePolicy::external_config_update(),
            "External config create-new must retain its public authority binding.",
        );
        let swap_source = codex_dir.clone();
        let swap_held = held_dir.clone();
        let swap_replacement = replacement_dir.clone();

        let error = with_external_post_publication_test_hook(
            move || {
                fs::rename(&swap_source, &swap_held).unwrap();
                fs::rename(&swap_replacement, &swap_source).unwrap();
            },
            || {
                WriteAuthority::new(app.handle()).external_config_update(
                    intent,
                    "new = true\n",
                    None,
                )
            },
        )
        .unwrap_err();

        assert!(matches!(error, WriteAuthorityError::RecoveryRequired(_)));
        assert_eq!(
            fs::read_to_string(&config_path).unwrap(),
            "competitor = true\n"
        );
        assert_eq!(
            fs::read_to_string(held_dir.join("config.toml")).unwrap(),
            "new = true\n"
        );
        let scan = app
            .state::<WriteAuthorityRuntime>()
            .recovery_scan()
            .unwrap();
        assert!(scan.blocked, "{scan:?}");
        assert_eq!(
            scan.items[0].classification,
            crate::kernel::write_authority::WriteAuthorityRecoveryClassification::Conflict
        );
        assert_eq!(
            fs::read_to_string(&config_path).unwrap(),
            "competitor = true\n"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn external_config_stale_previous_bytes_reject_before_wal_or_backup() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("external-config-stale-previous");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let codex_dir = root.join("home/.codex");
        let config_path = codex_dir.join("config.toml");
        let backup_path = codex_dir.join("config.toml.pana-studio-stale.bak");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(&config_path, "competitor = true\n").unwrap();

        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        ensure_app_home(app.handle()).unwrap();
        let lease = CodexConfigLease::capture(&codex_dir).unwrap();
        let intent = WriteIntent::new(
            WriteCategory::ExternalIntegrationWrite,
            WriteOwner::CodexMcp,
            WriteOperationKind::ExternalConfigUpdate,
            lease
                .target(config_path.clone(), "external:~/.codex/config.toml")
                .unwrap(),
            WritePolicy::external_config_update(),
            "Stale external config preflight.",
        );

        let error = WriteAuthority::new(app.handle())
            .external_config_update(
                intent,
                "ours = true\n",
                Some((
                    lease
                        .target(backup_path.clone(), "external:backup")
                        .unwrap(),
                    "old = true\n",
                )),
            )
            .unwrap_err();

        assert!(matches!(error, WriteAuthorityError::Rejected(_)));
        assert_eq!(
            fs::read_to_string(&config_path).unwrap(),
            "competitor = true\n"
        );
        assert!(!backup_path.exists());
        assert!(
            !app.state::<WriteAuthorityRuntime>()
                .recovery_scan()
                .unwrap()
                .blocked
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn external_config_rejects_oversized_payload_before_wal() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("external-config-size-limit");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let codex_dir = root.join("home/.codex");
        let config_path = codex_dir.join("config.toml");
        fs::create_dir_all(&codex_dir).unwrap();
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        ensure_app_home(app.handle()).unwrap();
        let lease = CodexConfigLease::capture(&codex_dir).unwrap();
        let intent = WriteIntent::new(
            WriteCategory::ExternalIntegrationWrite,
            WriteOwner::CodexMcp,
            WriteOperationKind::ExternalConfigUpdate,
            lease
                .target(config_path.clone(), "external:~/.codex/config.toml")
                .unwrap(),
            WritePolicy::external_config_update(),
            "Oversized external config must fail before WAL.",
        );
        let oversized = "x".repeat(
            crate::kernel::write_authority::recovery::MAX_WAL_EXTERNAL_CONFIG_BYTES as usize + 1,
        );

        let error = WriteAuthority::new(app.handle())
            .external_config_update(intent, &oversized, None)
            .unwrap_err();

        assert!(matches!(error, WriteAuthorityError::Rejected(_)));
        assert!(!config_path.exists());
        assert!(
            !app.state::<WriteAuthorityRuntime>()
                .recovery_scan()
                .unwrap()
                .blocked
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn external_config_backup_collision_preserves_both_existing_files() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("external-config-backup-collision");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let codex_dir = root.join("home/.codex");
        let config_path = codex_dir.join("config.toml");
        let backup_path = codex_dir.join("config.toml.pana-studio-collision.bak");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(&config_path, "old = true\n").unwrap();
        fs::write(&backup_path, "competitor backup\n").unwrap();
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        ensure_app_home(app.handle()).unwrap();
        let lease = CodexConfigLease::capture(&codex_dir).unwrap();
        let intent = WriteIntent::new(
            WriteCategory::ExternalIntegrationWrite,
            WriteOwner::CodexMcp,
            WriteOperationKind::ExternalConfigUpdate,
            lease
                .target(config_path.clone(), "external:~/.codex/config.toml")
                .unwrap(),
            WritePolicy::external_config_update(),
            "External backup collision must be create-only.",
        );

        let error = WriteAuthority::new(app.handle())
            .external_config_update(
                intent,
                "new = true\n",
                Some((
                    lease
                        .target(backup_path.clone(), "external:backup")
                        .unwrap(),
                    "old = true\n",
                )),
            )
            .unwrap_err();

        assert!(matches!(error, WriteAuthorityError::Rejected(_)));
        assert_eq!(fs::read_to_string(&config_path).unwrap(), "old = true\n");
        assert_eq!(
            fs::read_to_string(&backup_path).unwrap(),
            "competitor backup\n"
        );
        assert!(
            !app.state::<WriteAuthorityRuntime>()
                .recovery_scan()
                .unwrap()
                .blocked
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn external_config_missing_backup_after_publication_stays_hot_without_inventing_inode() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("external-config-backup-race");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let codex_dir = root.join("home/.codex");
        let config_path = codex_dir.join("config.toml");
        let backup_path = codex_dir.join("config.toml.pana-studio-race.bak");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(&config_path, "old = true\n").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&config_path, fs::Permissions::from_mode(0o600)).unwrap();

        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        ensure_app_home(app.handle()).unwrap();
        let lease = CodexConfigLease::capture(&codex_dir).unwrap();
        let intent = WriteIntent::new(
            WriteCategory::ExternalIntegrationWrite,
            WriteOwner::CodexMcp,
            WriteOperationKind::ExternalConfigUpdate,
            lease
                .target(config_path.clone(), "external:~/.codex/config.toml")
                .unwrap(),
            WritePolicy::external_config_update(),
            "External config backup race.",
        );
        let backup_target = lease
            .target(backup_path.clone(), "external:backup")
            .unwrap();
        let removed_backup = backup_path.clone();

        let error = with_external_post_publication_test_hook(
            move || fs::remove_file(&removed_backup).unwrap(),
            || {
                WriteAuthority::new(app.handle()).external_config_update(
                    intent,
                    "new = true\n",
                    Some((backup_target, "old = true\n")),
                )
            },
        )
        .unwrap_err();

        assert!(matches!(error, WriteAuthorityError::RecoveryRequired(_)));
        assert_eq!(fs::read_to_string(&config_path).unwrap(), "new = true\n");
        assert!(!backup_path.exists());
        let scan = app
            .state::<WriteAuthorityRuntime>()
            .recovery_scan()
            .unwrap();
        assert!(scan.blocked, "{scan:?}");
        assert_eq!(
            scan.items[0].classification,
            crate::kernel::write_authority::WriteAuthorityRecoveryClassification::Conflict
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn external_config_target_race_after_backup_never_reports_committed() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("external-config-target-race");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let codex_dir = root.join("home/.codex");
        let config_path = codex_dir.join("config.toml");
        let backup_path = codex_dir.join("config.toml.pana-studio-target-race.bak");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(&config_path, "old = true\n").unwrap();

        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        ensure_app_home(app.handle()).unwrap();
        let lease = CodexConfigLease::capture(&codex_dir).unwrap();
        let intent = WriteIntent::new(
            WriteCategory::ExternalIntegrationWrite,
            WriteOwner::CodexMcp,
            WriteOperationKind::ExternalConfigUpdate,
            lease
                .target(config_path.clone(), "external:~/.codex/config.toml")
                .unwrap(),
            WritePolicy::external_config_update(),
            "External config target race.",
        );
        let backup_target = lease
            .target(backup_path.clone(), "external:backup")
            .unwrap();
        let racing_target = config_path.clone();

        let error = with_external_backup_committed_test_hook(
            move || fs::write(&racing_target, "competitor = true\n").unwrap(),
            || {
                WriteAuthority::new(app.handle()).external_config_update(
                    intent,
                    "new = true\n",
                    Some((backup_target, "old = true\n")),
                )
            },
        )
        .unwrap_err();

        assert!(matches!(error, WriteAuthorityError::RecoveryRequired(_)));
        assert_eq!(fs::read_to_string(&backup_path).unwrap(), "old = true\n");
        assert_eq!(
            fs::read_to_string(&config_path).unwrap(),
            "competitor = true\n"
        );
        let scan = app
            .state::<WriteAuthorityRuntime>()
            .recovery_scan()
            .unwrap();
        assert!(scan.blocked);
        assert_eq!(
            scan.items[0].classification,
            crate::kernel::write_authority::WriteAuthorityRecoveryClassification::Conflict
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn external_config_linkat_failure_restores_baseline_and_finalizes_on_rescan() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("external-config-linkat-rollback");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let codex_dir = root.join("home/.codex");
        let config_path = codex_dir.join("config.toml");
        let backup_path = codex_dir.join("config.toml.pana-studio-linkat.bak");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(&config_path, "old = true\n").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&config_path, fs::Permissions::from_mode(0o600)).unwrap();

        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        ensure_app_home(app.handle()).unwrap();
        let lease = CodexConfigLease::capture(&codex_dir).unwrap();
        let intent = WriteIntent::new(
            WriteCategory::ExternalIntegrationWrite,
            WriteOwner::CodexMcp,
            WriteOperationKind::ExternalConfigUpdate,
            lease
                .target(config_path.clone(), "external:~/.codex/config.toml")
                .unwrap(),
            WritePolicy::external_config_update(),
            "External config linkat rollback.",
        );
        let backup_target = lease
            .target(backup_path.clone(), "external:backup")
            .unwrap();

        let error = with_external_linkat_failure_test_hook(|| {
            WriteAuthority::new(app.handle()).external_config_update(
                intent,
                "new = true\n",
                Some((backup_target, "old = true\n")),
            )
        })
        .unwrap_err();

        assert!(matches!(error, WriteAuthorityError::RecoveryRequired(_)));
        assert_eq!(fs::read_to_string(&config_path).unwrap(), "old = true\n");
        assert!(!backup_path.exists());

        let runtime = app.state::<WriteAuthorityRuntime>();
        let first = runtime.recovery_scan().unwrap();
        assert!(!first.blocked, "{first:?}");
        assert_eq!(fs::read_to_string(&config_path).unwrap(), "old = true\n");
        assert!(!backup_path.exists());

        let second = runtime.recovery_scan().unwrap();
        assert!(!second.blocked, "{second:?}");
        assert_eq!(fs::read_to_string(&config_path).unwrap(), "old = true\n");
        assert!(!backup_path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn external_config_competitor_after_baseline_relocation_preserves_both_inodes() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("external-config-baseline-relocation-race");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let codex_dir = root.join("home/.codex");
        let config_path = codex_dir.join("config.toml");
        let backup_path = codex_dir.join("config.toml.pana-studio-relocation-race.bak");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(&config_path, "old = true\n").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&config_path, fs::Permissions::from_mode(0o600)).unwrap();

        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        ensure_app_home(app.handle()).unwrap();
        let lease = CodexConfigLease::capture(&codex_dir).unwrap();
        let intent = WriteIntent::new(
            WriteCategory::ExternalIntegrationWrite,
            WriteOwner::CodexMcp,
            WriteOperationKind::ExternalConfigUpdate,
            lease
                .target(config_path.clone(), "external:~/.codex/config.toml")
                .unwrap(),
            WritePolicy::external_config_update(),
            "External config baseline relocation race.",
        );
        let backup_target = lease
            .target(backup_path.clone(), "external:backup")
            .unwrap();
        let competing_target = config_path.clone();

        let error = with_external_baseline_relocated_test_hook(
            move || fs::write(&competing_target, "competitor = true\n").unwrap(),
            || {
                WriteAuthority::new(app.handle()).external_config_update(
                    intent,
                    "new = true\n",
                    Some((backup_target, "old = true\n")),
                )
            },
        )
        .unwrap_err();

        assert!(matches!(error, WriteAuthorityError::RecoveryRequired(_)));
        assert_eq!(
            fs::read_to_string(&config_path).unwrap(),
            "competitor = true\n"
        );
        assert_eq!(fs::read_to_string(&backup_path).unwrap(), "old = true\n");

        let scan = app
            .state::<WriteAuthorityRuntime>()
            .recovery_scan()
            .unwrap();
        assert!(scan.blocked, "{scan:?}");
        assert_eq!(
            scan.items[0].classification,
            crate::kernel::write_authority::WriteAuthorityRecoveryClassification::Conflict
        );
        assert_eq!(
            fs::read_to_string(&config_path).unwrap(),
            "competitor = true\n"
        );
        assert_eq!(fs::read_to_string(&backup_path).unwrap(), "old = true\n");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn external_config_lost_backup_after_publication_never_false_commits() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("external-config-lost-backup-before-target");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let codex_dir = root.join("home/.codex");
        let config_path = codex_dir.join("config.toml");
        let backup_path = codex_dir.join("config.toml.pana-studio-lost.bak");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(&config_path, "old = true\n").unwrap();

        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        ensure_app_home(app.handle()).unwrap();
        let lease = CodexConfigLease::capture(&codex_dir).unwrap();
        let intent = WriteIntent::new(
            WriteCategory::ExternalIntegrationWrite,
            WriteOwner::CodexMcp,
            WriteOperationKind::ExternalConfigUpdate,
            lease
                .target(config_path.clone(), "external:~/.codex/config.toml")
                .unwrap(),
            WritePolicy::external_config_update(),
            "Lost external backup must roll back before target commit.",
        );
        let backup_target = lease
            .target(backup_path.clone(), "external:backup")
            .unwrap();
        let removed_backup = backup_path.clone();

        let error = with_external_backup_committed_test_hook(
            move || fs::remove_file(&removed_backup).unwrap(),
            || {
                WriteAuthority::new(app.handle()).external_config_update(
                    intent,
                    "new = true\n",
                    Some((backup_target, "old = true\n")),
                )
            },
        )
        .unwrap_err();

        assert!(matches!(error, WriteAuthorityError::RecoveryRequired(_)));
        assert_eq!(fs::read_to_string(&config_path).unwrap(), "new = true\n");
        assert!(!backup_path.exists());
        let scan = app
            .state::<WriteAuthorityRuntime>()
            .recovery_scan()
            .unwrap();
        assert!(scan.blocked, "{scan:?}");
        assert_eq!(
            scan.items[0].classification,
            crate::kernel::write_authority::WriteAuthorityRecoveryClassification::Conflict
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn external_config_final_byte_identical_backup_inode_never_false_commits() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("external-config-final-backup-restore");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let codex_dir = root.join("home/.codex");
        let config_path = codex_dir.join("config.toml");
        let backup_path = codex_dir.join("config.toml.pana-studio-final.bak");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(&config_path, "old = true\n").unwrap();

        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        ensure_app_home(app.handle()).unwrap();
        let lease = CodexConfigLease::capture(&codex_dir).unwrap();
        let intent = WriteIntent::new(
            WriteCategory::ExternalIntegrationWrite,
            WriteOwner::CodexMcp,
            WriteOperationKind::ExternalConfigUpdate,
            lease
                .target(config_path.clone(), "external:~/.codex/config.toml")
                .unwrap(),
            WritePolicy::external_config_update(),
            "Final backup postflight restore.",
        );
        let backup_target = lease
            .target(backup_path.clone(), "external:backup")
            .unwrap();
        let replaced_backup = backup_path.clone();

        let error = with_before_external_target_durable_test_hook(
            move || {
                fs::remove_file(&replaced_backup).unwrap();
                fs::write(&replaced_backup, "old = true\n").unwrap();
                #[cfg(unix)]
                fs::set_permissions(&replaced_backup, fs::Permissions::from_mode(0o600)).unwrap();
            },
            || {
                WriteAuthority::new(app.handle()).external_config_update(
                    intent,
                    "new = true\n",
                    Some((backup_target, "old = true\n")),
                )
            },
        )
        .unwrap_err();

        assert!(matches!(error, WriteAuthorityError::RecoveryRequired(_)));
        assert_eq!(fs::read_to_string(&config_path).unwrap(), "new = true\n");
        assert_eq!(fs::read_to_string(&backup_path).unwrap(), "old = true\n");
        let scan = app
            .state::<WriteAuthorityRuntime>()
            .recovery_scan()
            .unwrap();
        assert!(scan.blocked, "{scan:?}");
        assert_eq!(
            scan.items[0].classification,
            crate::kernel::write_authority::WriteAuthorityRecoveryClassification::Conflict
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn external_config_final_byte_identical_target_inode_never_reports_committed() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("external-config-final-target-race");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let codex_dir = root.join("home/.codex");
        let config_path = codex_dir.join("config.toml");
        let backup_path = codex_dir.join("config.toml.pana-studio-final-race.bak");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(&config_path, "old = true\n").unwrap();

        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        ensure_app_home(app.handle()).unwrap();
        let lease = CodexConfigLease::capture(&codex_dir).unwrap();
        let intent = WriteIntent::new(
            WriteCategory::ExternalIntegrationWrite,
            WriteOwner::CodexMcp,
            WriteOperationKind::ExternalConfigUpdate,
            lease
                .target(config_path.clone(), "external:~/.codex/config.toml")
                .unwrap(),
            WritePolicy::external_config_update(),
            "Final target competitor race.",
        );
        let backup_target = lease
            .target(backup_path.clone(), "external:backup")
            .unwrap();
        let racing_target = config_path.clone();

        let error = with_before_external_target_durable_test_hook(
            move || {
                fs::remove_file(&racing_target).unwrap();
                fs::write(&racing_target, "new = true\n").unwrap();
                #[cfg(unix)]
                fs::set_permissions(&racing_target, fs::Permissions::from_mode(0o600)).unwrap();
            },
            || {
                WriteAuthority::new(app.handle()).external_config_update(
                    intent,
                    "new = true\n",
                    Some((backup_target, "old = true\n")),
                )
            },
        )
        .unwrap_err();

        assert!(matches!(error, WriteAuthorityError::RecoveryRequired(_)));
        assert_eq!(fs::read_to_string(&config_path).unwrap(), "new = true\n");
        assert_eq!(fs::read_to_string(&backup_path).unwrap(), "old = true\n");
        let scan = app
            .state::<WriteAuthorityRuntime>()
            .recovery_scan()
            .unwrap();
        assert!(scan.blocked);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn external_config_create_new_byte_identical_target_inode_never_reports_committed() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("external-config-create-final-race");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let codex_dir = root.join("home/.codex");
        let config_path = codex_dir.join("config.toml");
        fs::create_dir_all(&codex_dir).unwrap();

        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        ensure_app_home(app.handle()).unwrap();
        let lease = CodexConfigLease::capture(&codex_dir).unwrap();
        let intent = WriteIntent::new(
            WriteCategory::ExternalIntegrationWrite,
            WriteOwner::CodexMcp,
            WriteOperationKind::ExternalConfigUpdate,
            lease
                .target(config_path.clone(), "external:~/.codex/config.toml")
                .unwrap(),
            WritePolicy::external_config_update(),
            "Create-new final target competitor race.",
        );
        let racing_target = config_path.clone();

        let error = with_before_external_target_durable_test_hook(
            move || {
                fs::remove_file(&racing_target).unwrap();
                fs::write(&racing_target, "new = true\n").unwrap();
                #[cfg(unix)]
                fs::set_permissions(&racing_target, fs::Permissions::from_mode(0o600)).unwrap();
            },
            || {
                WriteAuthority::new(app.handle()).external_config_update(
                    intent,
                    "new = true\n",
                    None,
                )
            },
        )
        .unwrap_err();

        assert!(matches!(error, WriteAuthorityError::RecoveryRequired(_)));
        assert_eq!(fs::read_to_string(&config_path).unwrap(), "new = true\n");
        let scan = app
            .state::<WriteAuthorityRuntime>()
            .recovery_scan()
            .unwrap();
        assert!(scan.blocked);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn external_config_preflight_failure_does_not_create_backup() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("external-config-partial-recovery");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let codex_dir = root.join("home/.codex");
        let config_path = codex_dir.join("config.toml");
        let backup_path = codex_dir.join("config.toml.pana-studio-test.bak");
        fs::create_dir_all(&config_path).unwrap();

        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");
        ensure_app_home(app.handle()).expect("test app home should be available");
        let lease = CodexConfigLease::capture(&codex_dir).unwrap();
        let intent = WriteIntent::new(
            WriteCategory::ExternalIntegrationWrite,
            WriteOwner::CodexMcp,
            WriteOperationKind::ExternalConfigUpdate,
            lease
                .target(config_path.clone(), "external:~/.codex/config.toml")
                .unwrap(),
            WritePolicy::external_config_update(),
            "External config invalid target must fail before backup.",
        );

        let error = WriteAuthority::new(app.handle())
            .external_config_update(
                intent,
                "new = true\n",
                Some((
                    lease
                        .target(backup_path.clone(), "external:~/.codex/config.toml backup")
                        .unwrap(),
                    "old = true\n",
                )),
            )
            .unwrap_err();

        assert!(error.diagnostic().contains("regular"));
        assert!(!backup_path.exists());
        assert!(config_path.is_dir());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn external_config_update_rejects_unsafe_target_and_backup_without_mutation() {
        use std::os::unix::fs::symlink;

        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("external-config-boundary");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let config_dir = root.join("home/.codex");
        let safe = root.join("safe");
        let outside = root.join("outside");
        fs::create_dir_all(&config_dir).unwrap();
        fs::create_dir_all(&safe).unwrap();
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, safe.join("link")).unwrap();
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");
        ensure_app_home(app.handle()).expect("test app home should be available");

        let unsafe_boundary = safe.join("link/config");
        let unsafe_target_error = CodexConfigLease::capture(&unsafe_boundary).unwrap_err();
        assert!(unsafe_target_error.contains("symlink"));
        assert!(!outside.join("config/config.toml").exists());

        let config_path = config_dir.join("config.toml");
        fs::write(&config_path, "old = true\n").unwrap();
        let lease = CodexConfigLease::capture(&config_dir).unwrap();
        let valid_intent = WriteIntent::new(
            WriteCategory::ExternalIntegrationWrite,
            WriteOwner::CodexMcp,
            WriteOperationKind::ExternalConfigUpdate,
            lease
                .target(config_path.clone(), "external:~/.codex/config.toml")
                .unwrap(),
            WritePolicy::external_config_update(),
            "Unsafe external config backup must be rejected.",
        );
        let unsafe_backup_boundary = safe.join("link/backup");
        let unsafe_backup = lease
            .target(
                unsafe_backup_boundary.join("config.toml.pana-studio-test.bak"),
                "external:unsafe/config.toml backup",
            )
            .unwrap_err();
        assert!(unsafe_backup.contains("authority root"));
        drop(valid_intent);
        assert_eq!(fs::read_to_string(&config_path).unwrap(), "old = true\n");
        assert!(!outside.join("backup/config.toml.bak").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn write_authority_rejects_undeclared_intent_before_disk_write() {
        let root = unique_test_dir("undeclared-intent");
        fs::create_dir_all(&root).unwrap();
        let target = root.join("deploy.txt");

        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");
        let app_handle = app.handle().clone();

        let intent = WriteIntent::new(
            WriteCategory::InternalAppWrite,
            WriteOwner::Preview,
            WriteOperationKind::WriteText,
            WriteTarget::new(target.clone(), root.clone(), "undeclared/deploy.txt"),
            WritePolicy::internal_atomic(),
            "Intent nedeclarat pentru test.",
        );

        let error = WriteAuthority::new(&app_handle)
            .write_text(intent, "should not be written\n")
            .unwrap_err();

        assert!(error.diagnostic().contains("nu este declarată"));
        assert!(!target.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn write_authority_blocks_parent_traversal_before_disk_write() {
        let root = unique_test_dir("write-parent-traversal");
        let boundary = root.join("preview");
        fs::create_dir_all(&boundary).unwrap();
        let outside = root.join("escaped.html");
        let target = boundary.join("templates/new/../../../escaped.html");
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");

        let intent = WriteIntent::new(
            WriteCategory::PreviewWorkspaceWrite,
            WriteOwner::Preview,
            WriteOperationKind::WriteText,
            WriteTarget::new(target, boundary, "preview/traversal"),
            WritePolicy::preview_workspace_atomic(),
            "Traversal write must be rejected.",
        );

        let error = WriteAuthority::new(app.handle())
            .write_text(intent, "blocked")
            .unwrap_err();

        assert!(error.diagnostic().contains("traversare"));
        assert!(!outside.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn write_authority_blocks_symlink_ancestor_with_missing_descendants() {
        use std::os::unix::fs::symlink;

        let root = unique_test_dir("write-symlink-ancestor");
        let boundary = root.join("project");
        let outside = root.join("outside");
        fs::create_dir_all(&boundary).unwrap();
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, boundary.join("link")).unwrap();
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");

        let intent = WriteIntent::new(
            WriteCategory::PreviewWorkspaceWrite,
            WriteOwner::Preview,
            WriteOperationKind::WriteText,
            WriteTarget::new(
                boundary.join("link/new/file.txt"),
                boundary,
                "preview/link/new/file.txt",
            ),
            WritePolicy::preview_workspace_atomic(),
            "Symlink ancestor write must be rejected.",
        );

        let error = WriteAuthority::new(app.handle())
            .write_text(intent, "blocked")
            .unwrap_err();

        assert!(
            error.diagnostic().contains("symlink"),
            "{}",
            error.diagnostic()
        );
        assert!(!outside.join("new/file.txt").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn write_authority_blocks_boundary_below_symlink_to_outside() {
        use std::os::unix::fs::symlink;

        let root = unique_test_dir("write-symlink-boundary");
        let safe = root.join("safe");
        let outside = root.join("outside");
        fs::create_dir_all(&safe).unwrap();
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, safe.join("link")).unwrap();
        let boundary = safe.join("link/missing-namespace");
        let external_target = outside.join("missing-namespace/file.txt");
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");

        let intent = WriteIntent::new(
            WriteCategory::PreviewWorkspaceWrite,
            WriteOwner::Preview,
            WriteOperationKind::WriteText,
            WriteTarget::new(
                boundary.join("file.txt"),
                boundary,
                "preview/symlink-boundary/file.txt",
            ),
            WritePolicy::preview_workspace_atomic(),
            "Symlink in the declared boundary must be rejected.",
        );

        let error = WriteAuthority::new(app.handle())
            .write_text(intent, "blocked")
            .unwrap_err();

        assert!(
            error.diagnostic().contains("symlink"),
            "{}",
            error.diagnostic()
        );
        assert!(!external_target.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn append_rejects_dangling_symlink_leaf_without_creating_external_target() {
        use std::os::unix::fs::symlink;

        let root = unique_test_dir("append-dangling-leaf");
        let boundary = root.join("session");
        let outside = root.join("outside");
        fs::create_dir_all(&boundary).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let external_target = outside.join("transactions.jsonl");
        let link = boundary.join("transactions.jsonl");
        symlink(&external_target, &link).unwrap();
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");

        let intent = WriteIntent::new(
            WriteCategory::InternalAppWrite,
            WriteOwner::Kernel,
            WriteOperationKind::AppendText,
            WriteTarget::new(link, boundary, "session/transactions.jsonl"),
            WritePolicy::internal_append(),
            "Dangling append symlink must be rejected.",
        );

        let error = WriteAuthority::new(app.handle())
            .append_text(intent, "{}\n")
            .unwrap_err();

        assert!(error.diagnostic().contains("symlink"));
        assert!(!external_target.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn append_and_copy_reject_hardlink_targets_without_external_mutation() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("in-place-hardlink");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let outside = root.join("outside.txt");
        let source = root.join("source.txt");
        fs::create_dir_all(&root).unwrap();
        fs::write(&outside, "outside").unwrap();
        fs::write(&source, "replacement").unwrap();
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");
        let app_home = ensure_app_home(app.handle()).expect("test app home should be available");
        let session_boundary = PathBuf::from(app_home.sessions_dir).join("hardlink-test");
        let preview_boundary =
            PathBuf::from(app_home.preview_cache_dir).join("project-hardlink-test");
        fs::create_dir_all(&session_boundary).unwrap();
        fs::create_dir_all(&preview_boundary).unwrap();
        let append_target = session_boundary.join("transactions.jsonl");
        let copy_target = preview_boundary.join("copy.txt");
        fs::hard_link(&outside, &append_target).unwrap();
        fs::hard_link(&outside, &copy_target).unwrap();

        let append_intent = WriteIntent::new(
            WriteCategory::InternalAppWrite,
            WriteOwner::Kernel,
            WriteOperationKind::AppendText,
            WriteTarget::new(
                append_target,
                session_boundary,
                "session/transactions.jsonl",
            ),
            WritePolicy::internal_append(),
            "Append must reject hardlink targets.",
        );
        let copy_intent = WriteIntent::new(
            WriteCategory::PreviewWorkspaceWrite,
            WriteOwner::Preview,
            WriteOperationKind::Copy,
            WriteTarget::new(copy_target, preview_boundary, "preview/copy.txt"),
            WritePolicy::preview_workspace_lifecycle(),
            "Copy must reject hardlink targets.",
        );

        assert!(WriteAuthority::new(app.handle())
            .append_text(append_intent, "{}\n")
            .unwrap_err()
            .diagnostic()
            .contains("hardlink"));
        assert!(WriteAuthority::new(app.handle())
            .copy_file(copy_intent, &source)
            .unwrap_err()
            .diagnostic()
            .contains("hardlink"));
        assert_eq!(fs::read_to_string(&outside).unwrap(), "outside");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn create_only_policies_refuse_existing_leaf_without_overwrite() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("create-only-policy");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let project = root.join("project");
        let source = root.join("source.txt");
        let initializer_target = project.join("config.toml");
        fs::create_dir_all(initializer_target.parent().unwrap()).unwrap();
        fs::write(&source, "new initializer").unwrap();
        fs::write(&initializer_target, "existing initializer").unwrap();
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");
        let app_home = ensure_app_home(app.handle()).expect("test app home should be available");
        let session_dir = PathBuf::from(app_home.sessions_dir).join("create-only-test");
        fs::create_dir_all(&session_dir).unwrap();
        install_test_project_authority(
            app.handle(),
            "create-only-test/runtime",
            &project,
            &session_dir,
        )
        .unwrap();
        let bootstrap = ProjectBootstrapLease::capture(&project).unwrap();

        let initializer_intent = WriteIntent::new(
            WriteCategory::ProjectSourceWrite,
            WriteOwner::ProjectInitializer,
            WriteOperationKind::Copy,
            bootstrap
                .target(initializer_target.clone(), "project/config.toml")
                .unwrap(),
            WritePolicy::project_creation_lifecycle(),
            "Project initializer copy must be create-only.",
        );
        assert!(WriteAuthority::new(app.handle())
            .copy_file(initializer_intent, &source)
            .unwrap_err()
            .diagnostic()
            .contains("există"));
        assert_eq!(
            fs::read_to_string(initializer_target).unwrap(),
            "existing initializer"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn append_and_copy_reject_non_regular_leaf_without_blocking() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("in-place-non-regular");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let source = root.join("source.txt");
        fs::create_dir_all(&root).unwrap();
        fs::write(&source, "source").unwrap();
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");
        let app_home = ensure_app_home(app.handle()).expect("test app home should be available");
        let session_boundary = PathBuf::from(app_home.sessions_dir).join("non-regular-test");
        let preview_boundary =
            PathBuf::from(app_home.preview_cache_dir).join("project-non-regular-test");
        let append_special_path = session_boundary.join("transactions.jsonl");
        let copy_special_path = preview_boundary.join("special-directory");
        fs::create_dir_all(&append_special_path).unwrap();
        fs::create_dir_all(&copy_special_path).unwrap();

        let append_intent = WriteIntent::new(
            WriteCategory::InternalAppWrite,
            WriteOwner::Kernel,
            WriteOperationKind::AppendText,
            WriteTarget::new(
                append_special_path.clone(),
                session_boundary,
                "session/transactions.jsonl",
            ),
            WritePolicy::internal_append(),
            "Append must reject special filesystem leaves.",
        );
        let copy_intent = WriteIntent::new(
            WriteCategory::PreviewWorkspaceWrite,
            WriteOwner::Preview,
            WriteOperationKind::Copy,
            WriteTarget::new(
                copy_special_path.clone(),
                preview_boundary,
                "preview/special-directory",
            ),
            WritePolicy::preview_workspace_lifecycle(),
            "Copy must reject special filesystem leaves.",
        );

        assert!(WriteAuthority::new(app.handle())
            .append_text(append_intent, "{}\n")
            .unwrap_err()
            .diagnostic()
            .contains("regular"));
        assert!(WriteAuthority::new(app.handle())
            .copy_file(copy_intent, &source)
            .unwrap_err()
            .diagnostic()
            .contains("regular"));
        assert!(append_special_path.is_dir());
        assert!(copy_special_path.is_dir());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn create_copy_and_symlink_reject_symlink_ancestor() {
        use std::os::unix::fs::symlink;

        let root = unique_test_dir("lifecycle-symlink-ancestor");
        let boundary = root.join("preview");
        let outside = root.join("outside");
        let source = root.join("source.txt");
        fs::create_dir_all(&boundary).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(&source, "source").unwrap();
        symlink(&outside, boundary.join("link")).unwrap();
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");

        let create_intent = WriteIntent::new(
            WriteCategory::PreviewWorkspaceWrite,
            WriteOwner::Preview,
            WriteOperationKind::CreateDirectory,
            WriteTarget::new(
                boundary.join("link/new-directory"),
                boundary.clone(),
                "preview/link/new-directory",
            ),
            WritePolicy::preview_workspace_lifecycle(),
            "Symlink ancestor create must be rejected.",
        );
        let copy_intent = WriteIntent::new(
            WriteCategory::PreviewWorkspaceWrite,
            WriteOwner::Preview,
            WriteOperationKind::Copy,
            WriteTarget::new(
                boundary.join("link/new/file.txt"),
                boundary.clone(),
                "preview/link/new/file.txt",
            ),
            WritePolicy::preview_workspace_lifecycle(),
            "Symlink ancestor copy must be rejected.",
        );
        let symlink_intent = WriteIntent::new(
            WriteCategory::PreviewWorkspaceWrite,
            WriteOwner::Preview,
            WriteOperationKind::Symlink,
            WriteTarget::new(
                boundary.join("link/new/source-link"),
                boundary,
                "preview/link/new/source-link",
            ),
            WritePolicy::preview_workspace_lifecycle(),
            "Symlink ancestor link creation must be rejected.",
        );

        assert!(WriteAuthority::new(app.handle())
            .create_directory_all(create_intent)
            .is_err());
        assert!(WriteAuthority::new(app.handle())
            .copy_file(copy_intent, &source)
            .is_err());
        assert!(WriteAuthority::new(app.handle())
            .symlink_entry(symlink_intent, &source)
            .is_err());
        assert!(!outside.join("new-directory").exists());
        assert!(!outside.join("new/file.txt").exists());
        assert!(fs::symlink_metadata(outside.join("new/source-link")).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn remove_file_unlinks_symlink_leaf_without_touching_external_target() {
        use std::os::unix::fs::symlink;

        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = unique_test_dir("unlink-symlink-leaf");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let outside = root.join("outside.txt");
        fs::create_dir_all(&root).unwrap();
        fs::write(&outside, "outside").unwrap();
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");
        let app_home = ensure_app_home(app.handle()).expect("test app home should be available");
        let boundary = PathBuf::from(app_home.preview_cache_dir).join("project-unlink-test");
        let link = boundary.join("link");
        fs::create_dir_all(&boundary).unwrap();
        symlink(&outside, &link).unwrap();

        let intent = WriteIntent::new(
            WriteCategory::PreviewWorkspaceWrite,
            WriteOwner::Preview,
            WriteOperationKind::RemoveFile,
            WriteTarget::new(link.clone(), boundary, "preview/link"),
            WritePolicy::preview_workspace_lifecycle(),
            "Unlink preview symlink leaf.",
        );

        WriteAuthority::new(app.handle())
            .remove_file_if_exists(intent)
            .unwrap();

        assert!(fs::symlink_metadata(&link).is_err());
        assert_eq!(fs::read_to_string(&outside).unwrap(), "outside");
        fs::remove_dir_all(root).unwrap();
    }

    fn unique_test_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pana-studio-{label}-{nanos}"))
    }

    struct TestEnvGuard {
        previous_values: Vec<(&'static str, Option<String>)>,
    }

    impl TestEnvGuard {
        fn from_root(root: &Path) -> Self {
            let bindings = [
                ("XDG_CONFIG_HOME", root.join("config")),
                ("XDG_DATA_HOME", root.join("data")),
                ("XDG_CACHE_HOME", root.join("cache")),
                ("XDG_STATE_HOME", root.join("state")),
            ];
            let previous_values = bindings
                .iter()
                .map(|(key, _)| (*key, env::var(key).ok()))
                .collect::<Vec<_>>();
            for (key, path) in bindings {
                env::set_var(key, path);
            }
            Self { previous_values }
        }
    }

    impl Drop for TestEnvGuard {
        fn drop(&mut self) {
            for (key, value) in &self.previous_values {
                if let Some(value) = value {
                    env::set_var(key, value);
                } else {
                    env::remove_var(key);
                }
            }
        }
    }
}
