use super::*;

#[path = "recovery/snapshot.rs"]
mod snapshot;

use snapshot::*;

pub(in crate::kernel::write_authority::capability) fn classify_external_config_recovery(
    record: &WalRecord,
    phase: WalPhase,
    checkpoint: Option<&WalExternalStageCheckpoint>,
    decision: Option<WalExternalOperatorDecision>,
    read_budget: &mut RecoveryReadBudget,
) -> Result<ExternalConfigRecoveryAssessment, String> {
    let WalOperationEvidence::ExternalConfig(evidence) = &record.body.operation_evidence else {
        return Err("WriteAuthority ExternalConfig classifier a primit alta familie.".into());
    };
    if evidence.protocol_version != WAL_EXTERNAL_CONFIG_PROTOCOL_VERSION {
        return Ok(external_conflict(&format!(
            "ExternalConfig WAL folosește protocolul incompatibil {}. Recordul rămâne hot și nu poate fi rezolvat de această versiune a aplicației.",
            evidence.protocol_version
        )));
    }
    if phase == WalPhase::Prepared {
        if checkpoint.is_some() || decision.is_some() {
            return Ok(external_conflict(
                "ExternalConfig Prepared nu poate purta checkpoint sau decizie operator.",
            ));
        }
    } else {
        let Some(checkpoint) = checkpoint else {
            return Ok(external_conflict(
                "ExternalConfig post-Prepared nu are checkpoint cauzal de identitate; auto-recovery este interzis.",
            ));
        };
        if checkpoint.backup_identity_digest.is_some() {
            return Ok(external_conflict(
                "Protocolul ExternalConfig fără unlink nu acceptă un al doilea inode staged pentru backup.",
            ));
        }
    }
    let context = match capture_external_recovery_context(record, evidence, checkpoint)? {
        Some(context) => context,
        None => {
            return Ok(external_conflict(
                "Parentul ExternalConfig existent la WAL prepare lipseste la recovery.",
            ));
        }
    };
    let oracle = observe_external_oracle(record, evidence, &context, read_budget)?;
    if evidence.backup.is_some() {
        return classify_external_no_unlink_oracle(evidence, phase, checkpoint, decision, &oracle);
    }
    classify_external_oracle(evidence, phase, checkpoint, decision, &oracle)
}

pub(in crate::kernel::write_authority::capability) fn execute_external_config_recovery(
    record: &WalRecord,
    phase: WalPhase,
    checkpoint: Option<&WalExternalStageCheckpoint>,
    decision: Option<WalExternalOperatorDecision>,
    read_budget: &mut RecoveryReadBudget,
) -> Result<(), String> {
    let assessment =
        classify_external_config_recovery(record, phase, checkpoint, decision, read_budget)?;
    let action = assessment.automatic_action.ok_or_else(|| {
        format!(
            "WriteAuthority ExternalConfig recovery CAS nu permite actiune automata: {}",
            assessment.diagnostic
        )
    })?;
    let WalOperationEvidence::ExternalConfig(evidence) = &record.body.operation_evidence else {
        return Err("WriteAuthority ExternalConfig executor a primit alta familie.".into());
    };
    match action {
        ExternalConfigRecoveryAction::ClearNoEffect => Ok(()),
        ExternalConfigRecoveryAction::FinalizeAbsentTarget => {
            finalize_absent_external_target(record, evidence, phase, checkpoint, read_budget)
        }
        ExternalConfigRecoveryAction::RestoreBaselineToTarget => {
            restore_external_baseline_to_target(record, evidence, phase, checkpoint, read_budget)
        }
        ExternalConfigRecoveryAction::FinalizeRestoredBaseline => {
            finalize_restored_external_baseline(record, evidence, phase, checkpoint, read_budget)
        }
        ExternalConfigRecoveryAction::FinalizeCommitted => {
            if evidence.backup.is_some() {
                finalize_external_no_unlink_committed(
                    record,
                    evidence,
                    phase,
                    checkpoint,
                    read_budget,
                )
            } else {
                finalize_external_created(record, evidence, phase, checkpoint, read_budget)
            }
        }
    }
}

fn finalize_absent_external_target(
    record: &WalRecord,
    evidence: &WalExternalConfigEvidence,
    phase: WalPhase,
    checkpoint: Option<&WalExternalStageCheckpoint>,
    read_budget: &mut RecoveryReadBudget,
) -> Result<(), String> {
    if !matches!(phase, WalPhase::AuxiliaryDurable | WalPhase::EffectVisible) {
        return Err("ExternalConfig absent-target finalize refuză faza terminală.".into());
    }
    if evidence.backup.is_some() {
        return Err("ExternalConfig absent-target finalize a primit evidence de replace.".into());
    }
    let context = ready_external_context(record, evidence, checkpoint)?;
    validate_leaf_absent_for_external(
        &context.directory,
        &context.leaves.target,
        &record.body.public_label,
        "absent target",
    )?;
    validate_leaf_absent_for_external(
        &context.directory,
        &context.leaves.target_temp,
        &record.body.public_label,
        "absent target temp",
    )?;
    sync_directory(&context.directory, &record.body.public_label)?;
    let public_context =
        recapture_external_public_context(record, evidence, checkpoint, &context.directory)?;
    let oracle = observe_external_oracle(record, evidence, &public_context, read_budget)?;
    if oracle.target != ObservedExternalLeaf::Absent
        || oracle.target_temp != ObservedExternalLeaf::Absent
    {
        return Err("ExternalConfig absent-target postflight nu mai este absent.".into());
    }
    Ok(())
}

fn restore_external_baseline_to_target(
    record: &WalRecord,
    evidence: &WalExternalConfigEvidence,
    phase: WalPhase,
    checkpoint: Option<&WalExternalStageCheckpoint>,
    read_budget: &mut RecoveryReadBudget,
) -> Result<(), String> {
    if !matches!(phase, WalPhase::AuxiliaryDurable | WalPhase::EffectVisible) {
        return Err("ExternalConfig baseline rollback refuză faza terminală.".into());
    }
    let context = ready_external_context(record, evidence, checkpoint)?;
    let backup_leaf = context
        .leaves
        .backup
        .as_ref()
        .ok_or("ExternalConfig baseline rollback cere backup leaf.")?;
    let baseline_identity = evidence
        .target_before_identity_digest
        .as_deref()
        .ok_or("ExternalConfig baseline rollback cere identity checkpoint.")?;
    validate_leaf_absent_for_external(
        &context.directory,
        &context.leaves.target,
        &record.body.public_label,
        "baseline rollback target",
    )?;
    validate_leaf_absent_for_external(
        &context.directory,
        &context.leaves.target_temp,
        &record.body.public_label,
        "baseline rollback target temp",
    )?;
    if let Some(backup_temp) = context.leaves.backup_temp.as_ref() {
        validate_leaf_absent_for_external(
            &context.directory,
            backup_temp,
            &record.body.public_label,
            "baseline rollback backup temp",
        )?;
    }
    let (mut baseline_file, _) = open_exact_external_baseline(
        &context.directory,
        backup_leaf,
        &evidence.target.before,
        evidence
            .target_before_mode_bits
            .expect("validated baseline mode"),
        baseline_identity,
        &record.body.public_label,
        "baseline rollback source",
        false,
        read_budget,
    )?;
    fs::renameat_with(
        &context.directory,
        backup_leaf,
        &context.directory,
        &context.leaves.target,
        RenameFlags::NOREPLACE,
    )
    .map_err(|error| {
        capability_error(
            &record.body.public_label,
            &format!("baseline rollback NOREPLACE a eșuat: {error}"),
        )
    })?;
    reserve_external_before_read(
        read_budget,
        &evidence.target.before,
        "baseline rollback postflight",
    )?;
    validate_open_external_baseline(
        &mut baseline_file,
        &evidence.target.before,
        evidence
            .target_before_mode_bits
            .expect("validated baseline mode"),
        baseline_identity,
        &context.directory,
        &context.leaves.target,
        &record.body.public_label,
        "baseline rollback postflight",
        false,
    )?;
    baseline_file.sync_all().map_err(|error| {
        capability_error(
            &record.body.public_label,
            &format!("baseline rollback fsync a eșuat: {error}"),
        )
    })?;
    sync_directory(&context.directory, &record.body.public_label)?;
    let public_context =
        recapture_external_public_context(record, evidence, checkpoint, &context.directory)?;
    let oracle = observe_external_oracle(record, evidence, &public_context, read_budget)?;
    if !observed_matches_checkpointed_before(
        &oracle.target,
        &evidence.target.before,
        evidence.target_before_mode_bits,
        Some(baseline_identity),
        false,
    ) || oracle
        .backup
        .as_ref()
        .is_some_and(|leaf| *leaf != ObservedExternalLeaf::Absent)
        || oracle.target_temp != ObservedExternalLeaf::Absent
        || oracle
            .backup_temp
            .as_ref()
            .is_some_and(|leaf| *leaf != ObservedExternalLeaf::Absent)
    {
        return Err("ExternalConfig baseline rollback postflight nu este exact.".into());
    }
    Ok(())
}

fn finalize_restored_external_baseline(
    record: &WalRecord,
    evidence: &WalExternalConfigEvidence,
    phase: WalPhase,
    checkpoint: Option<&WalExternalStageCheckpoint>,
    read_budget: &mut RecoveryReadBudget,
) -> Result<(), String> {
    if !matches!(phase, WalPhase::AuxiliaryDurable | WalPhase::EffectVisible) {
        return Err("ExternalConfig restored-baseline finalize refuză faza terminală.".into());
    }
    let context = ready_external_context(record, evidence, checkpoint)?;
    let baseline_identity = evidence
        .target_before_identity_digest
        .as_deref()
        .ok_or("ExternalConfig restored-baseline finalize cere identity checkpoint.")?;
    let (baseline_file, _) = open_exact_external_baseline(
        &context.directory,
        &context.leaves.target,
        &evidence.target.before,
        evidence
            .target_before_mode_bits
            .expect("validated baseline mode"),
        baseline_identity,
        &record.body.public_label,
        "restored baseline target",
        false,
        read_budget,
    )?;
    if let Some(backup_leaf) = context.leaves.backup.as_ref() {
        validate_leaf_absent_for_external(
            &context.directory,
            backup_leaf,
            &record.body.public_label,
            "restored baseline backup",
        )?;
    }
    validate_leaf_absent_for_external(
        &context.directory,
        &context.leaves.target_temp,
        &record.body.public_label,
        "restored baseline target temp",
    )?;
    if let Some(backup_temp) = context.leaves.backup_temp.as_ref() {
        validate_leaf_absent_for_external(
            &context.directory,
            backup_temp,
            &record.body.public_label,
            "restored baseline backup temp",
        )?;
    }
    baseline_file.sync_all().map_err(|error| {
        capability_error(
            &record.body.public_label,
            &format!("restored baseline fsync a eșuat: {error}"),
        )
    })?;
    sync_directory(&context.directory, &record.body.public_label)?;
    let public_context =
        recapture_external_public_context(record, evidence, checkpoint, &context.directory)?;
    let oracle = observe_external_oracle(record, evidence, &public_context, read_budget)?;
    if !observed_matches_checkpointed_before(
        &oracle.target,
        &evidence.target.before,
        evidence.target_before_mode_bits,
        Some(baseline_identity),
        false,
    ) || oracle
        .backup
        .as_ref()
        .is_some_and(|leaf| *leaf != ObservedExternalLeaf::Absent)
        || oracle.target_temp != ObservedExternalLeaf::Absent
        || oracle
            .backup_temp
            .as_ref()
            .is_some_and(|leaf| *leaf != ObservedExternalLeaf::Absent)
    {
        return Err("ExternalConfig restored-baseline postflight nu este exact.".into());
    }
    Ok(())
}

fn finalize_external_no_unlink_committed(
    record: &WalRecord,
    evidence: &WalExternalConfigEvidence,
    phase: WalPhase,
    checkpoint: Option<&WalExternalStageCheckpoint>,
    read_budget: &mut RecoveryReadBudget,
) -> Result<(), String> {
    let _ = phase;
    let context = ready_external_context(record, evidence, checkpoint)?;
    let causal_checkpoint = context
        .checkpoint
        .as_ref()
        .ok_or("ExternalConfig no-unlink finalize cere target checkpoint.")?;
    let backup_leaf = context
        .leaves
        .backup
        .as_ref()
        .ok_or("ExternalConfig no-unlink finalize cere backup leaf.")?;
    let baseline_identity = evidence
        .target_before_identity_digest
        .as_deref()
        .ok_or("ExternalConfig no-unlink finalize cere baseline checkpoint.")?;
    sync_exact_external_new(
        &context.directory,
        &context.leaves.target,
        &evidence.target,
        evidence.target_new_mode_bits,
        &causal_checkpoint.target_identity_digest,
        "target",
        &record.body.public_label,
        "no-unlink committed target",
        read_budget,
    )?;
    let (backup_file, _) = open_exact_external_baseline(
        &context.directory,
        backup_leaf,
        &evidence.target.before,
        evidence
            .target_before_mode_bits
            .expect("validated baseline mode"),
        baseline_identity,
        &record.body.public_label,
        "no-unlink committed backup",
        false,
        read_budget,
    )?;
    backup_file.sync_all().map_err(|error| {
        capability_error(
            &record.body.public_label,
            &format!("no-unlink committed backup fsync a eșuat: {error}"),
        )
    })?;
    validate_leaf_absent_for_external(
        &context.directory,
        &context.leaves.target_temp,
        &record.body.public_label,
        "no-unlink target temp",
    )?;
    if let Some(backup_temp) = context.leaves.backup_temp.as_ref() {
        validate_leaf_absent_for_external(
            &context.directory,
            backup_temp,
            &record.body.public_label,
            "no-unlink backup temp",
        )?;
    }
    sync_directory(&context.directory, &record.body.public_label)?;
    let public_context =
        recapture_external_public_context(record, evidence, checkpoint, &context.directory)?;
    verify_committed_pair(record, evidence, &public_context, read_budget)
}

fn classify_external_no_unlink_oracle(
    evidence: &WalExternalConfigEvidence,
    phase: WalPhase,
    checkpoint: Option<&WalExternalStageCheckpoint>,
    decision: Option<WalExternalOperatorDecision>,
    oracle: &ExternalOracle,
) -> Result<ExternalConfigRecoveryAssessment, String> {
    if decision.is_some() {
        return Ok(external_conflict(
            "Protocolul ExternalConfig fără unlink nu acceptă decizii operator legacy.",
        ));
    }
    let checkpoint = checkpoint;
    let baseline_identity = evidence.target_before_identity_digest.as_deref();
    let target_before = observed_matches_checkpointed_before(
        &oracle.target,
        &evidence.target.before,
        evidence.target_before_mode_bits,
        baseline_identity,
        true,
    );
    let target_restored = observed_matches_checkpointed_before(
        &oracle.target,
        &evidence.target.before,
        evidence.target_before_mode_bits,
        baseline_identity,
        false,
    );
    let target_absent = oracle.target == ObservedExternalLeaf::Absent;
    let target_new = observed_matches_new(
        &oracle.target,
        &evidence.target,
        evidence.target_new_mode_bits,
        checkpoint.map(|value| value.target_identity_digest.as_str()),
    );
    let backup = oracle
        .backup
        .as_ref()
        .ok_or("ExternalConfig no-unlink cere backup oracle.")?;
    let backup_absent = *backup == ObservedExternalLeaf::Absent;
    let backup_baseline = observed_matches_checkpointed_before(
        backup,
        &evidence.target.before,
        evidence.target_before_mode_bits,
        baseline_identity,
        false,
    );
    let auxiliaries_absent = oracle.target_temp == ObservedExternalLeaf::Absent
        && oracle
            .backup_temp
            .as_ref()
            .is_some_and(|leaf| *leaf == ObservedExternalLeaf::Absent);

    if !auxiliaries_absent {
        return Ok(external_conflict(
            "Protocolul ExternalConfig fără unlink a observat un leaf auxiliar nominalizat; nu îl adoptă și nu îl șterge automat.",
        ));
    }
    if phase == WalPhase::Prepared && target_before && backup_absent {
        return Ok(ExternalConfigRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::NoEffect,
            automatic_action: Some(ExternalConfigRecoveryAction::ClearNoEffect),
            available_resolution_actions: Vec::new(),
            diagnostic: "ExternalConfig Prepared este exact baseline; nu există efect nominalizat."
                .into(),
        });
    }
    if matches!(phase, WalPhase::AuxiliaryDurable | WalPhase::EffectVisible)
        && target_restored
        && backup_absent
    {
        return Ok(ExternalConfigRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::RollbackCompleted,
            automatic_action: Some(ExternalConfigRecoveryAction::FinalizeRestoredBaseline),
            available_resolution_actions: Vec::new(),
            diagnostic:
                "Payloadul O_TMPFILE anonim a dispărut la crash, iar baseline-ul este intact."
                    .into(),
        });
    }
    if matches!(phase, WalPhase::AuxiliaryDurable | WalPhase::EffectVisible)
        && target_absent
        && backup_baseline
    {
        return Ok(ExternalConfigRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::CleanupRequired,
            automatic_action: Some(ExternalConfigRecoveryAction::RestoreBaselineToTarget),
            available_resolution_actions: Vec::new(),
            diagnostic: "Baseline-ul cauzal a fost relocat în backup înainte ca payloadul anonim să primească nume; rollback-ul create-only îl poate restaura la target."
                .into(),
        });
    }
    if phase >= WalPhase::AuxiliaryDurable && target_new && backup_baseline {
        return Ok(ExternalConfigRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::EffectCommitted,
            automatic_action: Some(ExternalConfigRecoveryAction::FinalizeCommitted),
            available_resolution_actions: Vec::new(),
            diagnostic: "Targetul checkpointat și inode-ul baseline relocat în backup formează perechea finală exactă."
                .into(),
        });
    }

    Ok(external_conflict(&format!(
        "Oracle ExternalConfig no-unlink necunoscut (phase={phase:?}, targetBefore={target_before}, targetRestored={target_restored}, targetAbsent={target_absent}, targetNew={target_new}, backupAbsent={backup_absent}, backupBaseline={backup_baseline}, auxiliariesAbsent={auxiliaries_absent})."
    )))
}

fn classify_external_oracle(
    evidence: &WalExternalConfigEvidence,
    phase: WalPhase,
    checkpoint: Option<&WalExternalStageCheckpoint>,
    decision: Option<WalExternalOperatorDecision>,
    oracle: &ExternalOracle,
) -> Result<ExternalConfigRecoveryAssessment, String> {
    if evidence.backup.is_some() {
        return Ok(external_conflict(
            "Classifierul create-new ExternalConfig a primit evidence de replace.",
        ));
    }
    if decision.is_some() {
        return Ok(external_conflict(
            "ExternalConfig v2 nu acceptă decizii operator legacy.",
        ));
    }
    let target_before = observed_matches_before(
        &oracle.target,
        &evidence.target.before,
        evidence.target_before_mode_bits,
        true,
    );
    let target_new = observed_matches_new(
        &oracle.target,
        &evidence.target,
        evidence.target_new_mode_bits,
        checkpoint.map(|value| value.target_identity_digest.as_str()),
    );
    let target_temp_absent = oracle.target_temp == ObservedExternalLeaf::Absent;

    if phase == WalPhase::Prepared && target_before && target_temp_absent {
        return Ok(ExternalConfigRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::NoEffect,
            automatic_action: Some(ExternalConfigRecoveryAction::ClearNoEffect),
            available_resolution_actions: Vec::new(),
            diagnostic:
                "ExternalConfig create-new este exact baseline Absent, fără temp sau efect.".into(),
        });
    }
    if matches!(phase, WalPhase::AuxiliaryDurable | WalPhase::EffectVisible)
        && target_before
        && target_temp_absent
    {
        return Ok(ExternalConfigRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::RollbackCompleted,
            automatic_action: Some(ExternalConfigRecoveryAction::FinalizeAbsentTarget),
            available_resolution_actions: Vec::new(),
            diagnostic:
                "ExternalConfig create-new nu are target nominalizat; O_TMPFILE anonim a fost recuperat de kernel."
                    .into(),
        });
    }
    if phase >= WalPhase::AuxiliaryDurable && target_new && target_temp_absent {
        return Ok(ExternalConfigRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::EffectCommitted,
            automatic_action: Some(ExternalConfigRecoveryAction::FinalizeCommitted),
            available_resolution_actions: Vec::new(),
            diagnostic:
                "ExternalConfig create-new este exact la target; fsync/finalizarea sunt sigure."
                    .into(),
        });
    }

    Ok(external_conflict(&format!(
        "Oracle ExternalConfig create-new necunoscut (phase={phase:?}, targetBefore={target_before}, targetNew={target_new}, tempAbsent={target_temp_absent})."
    )))
}
fn finalize_external_created(
    record: &WalRecord,
    evidence: &WalExternalConfigEvidence,
    phase: WalPhase,
    checkpoint: Option<&WalExternalStageCheckpoint>,
    read_budget: &mut RecoveryReadBudget,
) -> Result<(), String> {
    let _ = phase;
    if evidence.backup.is_some() {
        return Err("ExternalConfig create-new finalize a primit evidence de backup.".into());
    }
    let context = ready_external_context(record, evidence, checkpoint)?;
    let causal_checkpoint = context
        .checkpoint
        .as_ref()
        .ok_or("ExternalConfig create-new finalize cere checkpoint cauzal.")?;
    sync_exact_external_new(
        &context.directory,
        &context.leaves.target,
        &evidence.target,
        evidence.target_new_mode_bits,
        &causal_checkpoint.target_identity_digest,
        "target",
        &record.body.public_label,
        "create-new committed target",
        read_budget,
    )?;
    validate_leaf_absent_for_external(
        &context.directory,
        &context.leaves.target_temp,
        &record.body.public_label,
        "create-new target temp",
    )?;
    sync_directory(&context.directory, &record.body.public_label)?;
    let public_context =
        recapture_external_public_context(record, evidence, checkpoint, &context.directory)?;
    verify_committed_pair(record, evidence, &public_context, read_budget)
}

fn recapture_external_public_context(
    record: &WalRecord,
    evidence: &WalExternalConfigEvidence,
    checkpoint: Option<&WalExternalStageCheckpoint>,
    held_parent: &OwnedFd,
) -> Result<ExternalRecoveryContext, String> {
    let recaptured = ready_external_context(record, evidence, checkpoint)?;
    let held_identity = wal_identity_from_fd(held_parent, &record.body.public_label)?;
    let recaptured_identity =
        wal_identity_from_fd(&recaptured.directory, &record.body.public_label)?;
    if recaptured_identity != held_identity {
        return Err(capability_error(
            &record.body.public_label,
            "ExternalConfig recovery full-path CAS a recapturat alt parent",
        ));
    }
    Ok(recaptured)
}
#[cfg(test)]
mod tests {
    use super::*;

    fn parent() -> WalParentEvidence {
        WalParentEvidence {
            relative_components_hex: Vec::new(),
            existing_prefix_len: 0,
            existing_ancestor_identity: WalFilesystemIdentity {
                device: 1,
                inode: 1,
            },
            parent_identity: Some(WalFilesystemIdentity {
                device: 1,
                inode: 1,
            }),
        }
    }

    fn regular(identity: u64, size: u64, version: &str, hash: &str) -> WalLeafEvidence {
        WalLeafEvidence::Regular {
            identity: WalFilesystemIdentity {
                device: 1,
                inode: identity,
            },
            size,
            version_token: version.into(),
            content_hash: hash.into(),
        }
    }

    fn observed(
        evidence: WalLeafEvidence,
        mode_bits: u32,
        stage_identity: &str,
        baseline_identity: &str,
    ) -> ObservedExternalLeaf {
        ObservedExternalLeaf::Regular {
            evidence,
            mode_bits,
            identity_digest: stage_identity.into(),
            baseline_identity_digest: baseline_identity.into(),
        }
    }

    fn checkpoint() -> WalExternalStageCheckpoint {
        WalExternalStageCheckpoint::new("a".repeat(32), None).unwrap()
    }

    fn create_new_evidence() -> WalExternalConfigEvidence {
        WalExternalConfigEvidence {
            protocol_version: WAL_EXTERNAL_CONFIG_PROTOCOL_VERSION,
            target: WalAtomicFileEvidence {
                parent: parent(),
                target_leaf_hex: "746172676574".into(),
                temp_leaf_hex: "74656d70".into(),
                before: WalLeafEvidence::Absent,
                new_size: 3,
                new_content_hash: "new".into(),
                replace: false,
            },
            backup: None,
            target_before_mode_bits: None,
            target_before_identity_digest: None,
            target_new_mode_bits: 0o600,
            backup_mode_bits: None,
        }
    }

    fn replace_evidence() -> WalExternalConfigEvidence {
        let before = regular(7, 3, "before-version", "old");
        WalExternalConfigEvidence {
            protocol_version: WAL_EXTERNAL_CONFIG_PROTOCOL_VERSION,
            target: WalAtomicFileEvidence {
                parent: parent(),
                target_leaf_hex: "746172676574".into(),
                temp_leaf_hex: "7461726765742d74656d70".into(),
                before,
                new_size: 3,
                new_content_hash: "new".into(),
                replace: true,
            },
            backup: Some(WalAtomicFileEvidence {
                parent: parent(),
                target_leaf_hex: "6261636b7570".into(),
                temp_leaf_hex: "6261636b75702d74656d70".into(),
                before: WalLeafEvidence::Absent,
                new_size: 3,
                new_content_hash: "old".into(),
                replace: false,
            }),
            target_before_mode_bits: Some(0o600),
            target_before_identity_digest: Some("c".repeat(32)),
            target_new_mode_bits: 0o600,
            backup_mode_bits: Some(0o600),
        }
    }

    fn replace_oracle(
        target: ObservedExternalLeaf,
        backup: ObservedExternalLeaf,
    ) -> ExternalOracle {
        ExternalOracle {
            target,
            target_temp: ObservedExternalLeaf::Absent,
            backup: Some(backup),
            backup_temp: Some(ObservedExternalLeaf::Absent),
        }
    }

    #[test]
    fn prepared_create_new_named_temp_is_never_adopted_or_removed() {
        let evidence = create_new_evidence();
        let oracle = ExternalOracle {
            target: ObservedExternalLeaf::Absent,
            target_temp: observed(
                regular(9, 3, "temp-version", "new"),
                0o600,
                &"a".repeat(32),
                &"z".repeat(32),
            ),
            backup: None,
            backup_temp: None,
        };

        let assessment =
            classify_external_oracle(&evidence, WalPhase::Prepared, None, None, &oracle).unwrap();

        assert_eq!(
            assessment.classification,
            WriteAuthorityRecoveryClassification::Conflict
        );
        assert_eq!(assessment.automatic_action, None);
    }

    #[test]
    fn create_new_target_durable_absent_is_conflict_not_rollback() {
        let evidence = create_new_evidence();
        let oracle = ExternalOracle {
            target: ObservedExternalLeaf::Absent,
            target_temp: ObservedExternalLeaf::Absent,
            backup: None,
            backup_temp: None,
        };

        let assessment = classify_external_oracle(
            &evidence,
            WalPhase::TargetDurable,
            Some(&checkpoint()),
            None,
            &oracle,
        )
        .unwrap();

        assert_eq!(
            assessment.classification,
            WriteAuthorityRecoveryClassification::Conflict
        );
    }

    #[test]
    fn replace_auxiliary_missing_target_restores_checkpointed_baseline() {
        let evidence = replace_evidence();
        let oracle = replace_oracle(
            ObservedExternalLeaf::Absent,
            observed(
                regular(7, 3, "relocated-version", "old"),
                0o600,
                &"x".repeat(32),
                &"c".repeat(32),
            ),
        );

        let assessment = classify_external_no_unlink_oracle(
            &evidence,
            WalPhase::AuxiliaryDurable,
            Some(&checkpoint()),
            None,
            &oracle,
        )
        .unwrap();

        assert_eq!(
            assessment.automatic_action,
            Some(ExternalConfigRecoveryAction::RestoreBaselineToTarget)
        );
    }

    #[test]
    fn replace_effect_visible_completed_rollback_clears_wal() {
        let evidence = replace_evidence();
        let oracle = replace_oracle(
            observed(
                evidence.target.before.clone(),
                0o600,
                &"x".repeat(32),
                &"c".repeat(32),
            ),
            ObservedExternalLeaf::Absent,
        );

        let assessment = classify_external_no_unlink_oracle(
            &evidence,
            WalPhase::EffectVisible,
            Some(&checkpoint()),
            None,
            &oracle,
        )
        .unwrap();

        assert_eq!(
            assessment.automatic_action,
            Some(ExternalConfigRecoveryAction::FinalizeRestoredBaseline)
        );
    }

    #[test]
    fn replace_target_durable_baseline_is_conflict() {
        let evidence = replace_evidence();
        let oracle = replace_oracle(
            observed(
                evidence.target.before.clone(),
                0o600,
                &"x".repeat(32),
                &"c".repeat(32),
            ),
            ObservedExternalLeaf::Absent,
        );

        let assessment = classify_external_no_unlink_oracle(
            &evidence,
            WalPhase::TargetDurable,
            Some(&checkpoint()),
            None,
            &oracle,
        )
        .unwrap();

        assert_eq!(
            assessment.classification,
            WriteAuthorityRecoveryClassification::Conflict
        );
    }

    #[test]
    fn replace_wrong_lifetime_digest_never_adopts_byte_identical_backup() {
        let evidence = replace_evidence();
        let oracle = replace_oracle(
            observed(
                regular(9, 3, "new-version", "new"),
                0o600,
                &"a".repeat(32),
                &"z".repeat(32),
            ),
            observed(
                regular(7, 3, "relocated-version", "old"),
                0o600,
                &"x".repeat(32),
                &"d".repeat(32),
            ),
        );

        let assessment = classify_external_no_unlink_oracle(
            &evidence,
            WalPhase::TargetDurable,
            Some(&checkpoint()),
            None,
            &oracle,
        )
        .unwrap();

        assert_eq!(
            assessment.classification,
            WriteAuthorityRecoveryClassification::Conflict
        );
    }

    #[test]
    fn replace_committed_pair_finalizes_from_auxiliary_or_target_durable() {
        let evidence = replace_evidence();
        for phase in [WalPhase::AuxiliaryDurable, WalPhase::TargetDurable] {
            let oracle = replace_oracle(
                observed(
                    regular(9, 3, "new-version", "new"),
                    0o600,
                    &"a".repeat(32),
                    &"z".repeat(32),
                ),
                observed(
                    regular(7, 3, "relocated-version", "old"),
                    0o600,
                    &"x".repeat(32),
                    &"c".repeat(32),
                ),
            );

            let assessment = classify_external_no_unlink_oracle(
                &evidence,
                phase,
                Some(&checkpoint()),
                None,
                &oracle,
            )
            .unwrap();

            assert_eq!(
                assessment.automatic_action,
                Some(ExternalConfigRecoveryAction::FinalizeCommitted)
            );
        }
    }
}
