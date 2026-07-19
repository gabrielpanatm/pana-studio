use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ObservedExternalLeaf {
    Absent,
    Regular {
        evidence: WalLeafEvidence,
        mode_bits: u32,
        identity_digest: String,
        baseline_identity_digest: String,
    },
}

pub(super) struct ExternalRecoveryContext {
    pub(super) directory: OwnedFd,
    pub(super) leaves: OwnedExternalLeaves,
    pub(super) checkpoint: Option<WalExternalStageCheckpoint>,
}

pub(super) struct ExternalOracle {
    pub(super) target: ObservedExternalLeaf,
    pub(super) target_temp: ObservedExternalLeaf,
    pub(super) backup: Option<ObservedExternalLeaf>,
    pub(super) backup_temp: Option<ObservedExternalLeaf>,
}

pub(super) fn capture_external_recovery_context(
    record: &WalRecord,
    evidence: &WalExternalConfigEvidence,
    checkpoint: Option<&WalExternalStageCheckpoint>,
) -> Result<Option<ExternalRecoveryContext>, String> {
    let context = capture_recovery_atomic_context(record, &evidence.target)?;
    let RecoveryAtomicContext::Ready {
        directory,
        target_leaf,
        temp_leaf,
        parent_was_missing,
    } = context
    else {
        return Ok(None);
    };
    if parent_was_missing {
        return Err("ExternalConfig recovery refuza un parent planificat absent.".into());
    }
    let leaves = owned_external_leaves(evidence)?;
    if leaves.target != target_leaf || leaves.target_temp != temp_leaf {
        return Err("ExternalConfig recovery leaf-urile difera de contextul atomic.".into());
    }
    validate_external_leaf_distinctness(leaves.as_borrowed(), &record.body.public_label)?;
    Ok(Some(ExternalRecoveryContext {
        directory,
        leaves,
        checkpoint: checkpoint.cloned(),
    }))
}

pub(super) fn ready_external_context(
    record: &WalRecord,
    evidence: &WalExternalConfigEvidence,
    checkpoint: Option<&WalExternalStageCheckpoint>,
) -> Result<ExternalRecoveryContext, String> {
    capture_external_recovery_context(record, evidence, checkpoint)?
        .ok_or_else(|| "ExternalConfig recovery a pierdut parentul dupa clasificare.".into())
}

pub(super) fn observe_external_oracle(
    record: &WalRecord,
    _evidence: &WalExternalConfigEvidence,
    context: &ExternalRecoveryContext,
    read_budget: &mut RecoveryReadBudget,
) -> Result<ExternalOracle, String> {
    Ok(ExternalOracle {
        target: observe_external_leaf(
            &context.directory,
            &context.leaves.target,
            &record.body.public_label,
            "target",
            "target",
            read_budget,
        )?,
        target_temp: observe_external_leaf(
            &context.directory,
            &context.leaves.target_temp,
            &record.body.public_label,
            "target temp",
            "target",
            read_budget,
        )?,
        backup: context
            .leaves
            .backup
            .as_ref()
            .map(|leaf| {
                observe_external_leaf(
                    &context.directory,
                    leaf,
                    &record.body.public_label,
                    "backup",
                    "backup",
                    read_budget,
                )
            })
            .transpose()?,
        backup_temp: context
            .leaves
            .backup_temp
            .as_ref()
            .map(|leaf| {
                observe_external_leaf(
                    &context.directory,
                    leaf,
                    &record.body.public_label,
                    "backup temp",
                    "backup",
                    read_budget,
                )
            })
            .transpose()?,
    })
}

fn observe_external_leaf(
    parent: &OwnedFd,
    leaf: &OsStr,
    public_label: &str,
    role: &str,
    identity_role: &str,
    read_budget: &mut RecoveryReadBudget,
) -> Result<ObservedExternalLeaf, String> {
    let Some((mut file, stat)) = open_recovery_regular_leaf(parent, leaf, public_label, role)?
    else {
        return Ok(ObservedExternalLeaf::Absent);
    };
    let observed_size = u64::try_from(stat.st_size)
        .map_err(|_| capability_error(public_label, &format!("{role} are dimensiune negativă")))?;
    if observed_size > MAX_WAL_EXTERNAL_CONFIG_BYTES {
        return Err(capability_error(
            public_label,
            &format!(
                "{role} depășește limita ExternalConfig de {MAX_WAL_EXTERNAL_CONFIG_BYTES} bytes"
            ),
        ));
    }
    let evidence = wal_evidence_from_open_file(
        &mut file,
        &stat,
        &ExpectedLeaf::Unspecified,
        public_label,
        role,
        Some(read_budget),
    )?;
    let after = fs::fstat(&file).map_err(|error| {
        capability_error(public_label, &format!("{role} final stat a esuat: {error}"))
    })?;
    if !same_stable_leaf_version(&stat, &after) {
        return Err(capability_error(
            public_label,
            &format!("{role} s-a schimbat in timpul oracle-ului"),
        ));
    }
    validate_named_file_identity(parent, leaf, &after, role)?;
    let identity_digest = external_stage_identity_digest(&file, identity_role)?;
    let baseline_identity_digest = external_baseline_identity_digest(&file)?;
    Ok(ObservedExternalLeaf::Regular {
        evidence,
        mode_bits: external_mode_bits(&after),
        identity_digest,
        baseline_identity_digest,
    })
}

pub(super) fn observed_matches_before(
    observed: &ObservedExternalLeaf,
    before: &WalLeafEvidence,
    expected_mode: Option<u32>,
    require_version_token: bool,
) -> bool {
    match (observed, before, expected_mode) {
        (ObservedExternalLeaf::Absent, WalLeafEvidence::Absent, None) => true,
        (
            ObservedExternalLeaf::Regular {
                evidence,
                mode_bits,
                ..
            },
            WalLeafEvidence::Regular { .. },
            Some(expected_mode),
        ) if *mode_bits == expected_mode => {
            if require_version_token {
                evidence == before
            } else {
                leaf_matches_relocated_before(evidence, before)
            }
        }
        _ => false,
    }
}

pub(super) fn observed_matches_checkpointed_before(
    observed: &ObservedExternalLeaf,
    before: &WalLeafEvidence,
    expected_mode: Option<u32>,
    expected_identity_digest: Option<&str>,
    require_version_token: bool,
) -> bool {
    let Some(expected_identity_digest) = expected_identity_digest else {
        return false;
    };
    observed_matches_before(observed, before, expected_mode, require_version_token)
        && matches!(
            observed,
            ObservedExternalLeaf::Regular {
                baseline_identity_digest,
                ..
            } if baseline_identity_digest == expected_identity_digest
        )
}

pub(super) fn observed_matches_new(
    observed: &ObservedExternalLeaf,
    evidence: &WalAtomicFileEvidence,
    expected_mode: u32,
    expected_identity_digest: Option<&str>,
) -> bool {
    let Some(expected_identity_digest) = expected_identity_digest else {
        return false;
    };
    matches!(
        observed,
        ObservedExternalLeaf::Regular {
            evidence: observed_evidence,
            mode_bits,
            identity_digest,
            ..
        } if *mode_bits == expected_mode
            && identity_digest == expected_identity_digest
            && leaf_matches_payload(
                observed_evidence,
                evidence.new_size,
                &evidence.new_content_hash,
            )
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn open_exact_external_baseline(
    parent: &OwnedFd,
    leaf: &OsStr,
    expected: &WalLeafEvidence,
    expected_mode: u32,
    expected_identity_digest: &str,
    public_label: &str,
    role: &str,
    require_version: bool,
    read_budget: &mut RecoveryReadBudget,
) -> Result<(File, fs::Stat), String> {
    let (mut file, stat) = open_recovery_regular_leaf(parent, leaf, public_label, role)?
        .ok_or_else(|| capability_error(public_label, &format!("{role} lipseste")))?;
    let expected_size = match expected {
        WalLeafEvidence::Regular { size, .. } => *size,
        WalLeafEvidence::Absent => {
            return Err(capability_error(
                public_label,
                &format!("{role} nu poate deschide un baseline absent"),
            ));
        }
    };
    validate_external_recovery_size(&stat, expected_size, public_label, role)?;
    reserve_external_before_read(read_budget, expected, role)?;
    validate_open_external_baseline(
        &mut file,
        expected,
        expected_mode,
        expected_identity_digest,
        parent,
        leaf,
        public_label,
        role,
        require_version,
    )?;
    Ok((file, stat))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn open_exact_external_new(
    parent: &OwnedFd,
    leaf: &OsStr,
    evidence: &WalAtomicFileEvidence,
    expected_mode: u32,
    expected_identity_digest: &str,
    identity_role: &str,
    public_label: &str,
    role: &str,
    read_budget: &mut RecoveryReadBudget,
) -> Result<(File, fs::Stat), String> {
    let (mut file, stat) = open_recovery_regular_leaf(parent, leaf, public_label, role)?
        .ok_or_else(|| capability_error(public_label, &format!("{role} lipseste")))?;
    validate_external_recovery_size(&stat, evidence.new_size, public_label, role)?;
    reserve_external_read(read_budget, evidence, role)?;
    validate_open_new_payload(
        &mut file,
        &stat,
        evidence,
        expected_mode,
        parent,
        leaf,
        public_label,
        role,
    )?;
    let observed_identity = external_stage_identity_digest(&file, identity_role)?;
    if observed_identity != expected_identity_digest {
        return Err(capability_error(
            public_label,
            &format!("{role} nu corespunde checkpointului de identitate"),
        ));
    }
    Ok((file, stat))
}

fn validate_external_recovery_size(
    stat: &fs::Stat,
    expected_size: u64,
    public_label: &str,
    role: &str,
) -> Result<(), String> {
    let observed_size = u64::try_from(stat.st_size)
        .map_err(|_| capability_error(public_label, &format!("{role} are dimensiune negativă")))?;
    if observed_size > MAX_WAL_EXTERNAL_CONFIG_BYTES || observed_size != expected_size {
        return Err(capability_error(
            public_label,
            &format!(
                "{role} are {observed_size} bytes, diferit de {expected_size} ori peste limita ExternalConfig {MAX_WAL_EXTERNAL_CONFIG_BYTES}"
            ),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn sync_exact_external_new(
    parent: &OwnedFd,
    leaf: &OsStr,
    evidence: &WalAtomicFileEvidence,
    expected_mode: u32,
    expected_identity_digest: &str,
    identity_role: &str,
    public_label: &str,
    role: &str,
    read_budget: &mut RecoveryReadBudget,
) -> Result<(), String> {
    let (file, _) = open_exact_external_new(
        parent,
        leaf,
        evidence,
        expected_mode,
        expected_identity_digest,
        identity_role,
        public_label,
        role,
        read_budget,
    )?;
    file.sync_all().map_err(|error| {
        capability_error(public_label, &format!("{role} fsync a esuat: {error}"))
    })?;
    let observed =
        observe_external_leaf(parent, leaf, public_label, role, identity_role, read_budget)?;
    if !observed_matches_new(
        &observed,
        evidence,
        expected_mode,
        Some(expected_identity_digest),
    ) {
        return Err(capability_error(
            public_label,
            &format!("{role} s-a schimbat dupa fsync"),
        ));
    }
    Ok(())
}

pub(super) fn verify_committed_pair(
    record: &WalRecord,
    evidence: &WalExternalConfigEvidence,
    context: &ExternalRecoveryContext,
    read_budget: &mut RecoveryReadBudget,
) -> Result<(), String> {
    let checkpoint = context
        .checkpoint
        .as_ref()
        .ok_or("ExternalConfig committed postflight nu are checkpoint cauzal.")?;
    let oracle = observe_external_oracle(record, evidence, context, read_budget)?;
    if !observed_matches_new(
        &oracle.target,
        &evidence.target,
        evidence.target_new_mode_bits,
        Some(&checkpoint.target_identity_digest),
    ) || oracle.target_temp != ObservedExternalLeaf::Absent
    {
        return Err("ExternalConfig committed target postflight nu este exact.".into());
    }
    if evidence.backup.is_some() {
        let observed_backup = oracle
            .backup
            .as_ref()
            .ok_or("ExternalConfig committed postflight nu are backup oracle.")?;
        let backup_is_restored_baseline = observed_matches_checkpointed_before(
            observed_backup,
            &evidence.target.before,
            evidence.target_before_mode_bits,
            evidence.target_before_identity_digest.as_deref(),
            false,
        );
        if !backup_is_restored_baseline
            || oracle
                .backup_temp
                .as_ref()
                .is_some_and(|leaf| *leaf != ObservedExternalLeaf::Absent)
        {
            return Err("ExternalConfig committed backup postflight nu este exact.".into());
        }
    }
    Ok(())
}

pub(super) fn reserve_external_read(
    read_budget: &mut RecoveryReadBudget,
    evidence: &WalAtomicFileEvidence,
    role: &str,
) -> Result<(), String> {
    read_budget.reserve(evidence.new_size, role)
}

pub(super) fn reserve_external_before_read(
    read_budget: &mut RecoveryReadBudget,
    evidence: &WalLeafEvidence,
    role: &str,
) -> Result<(), String> {
    match evidence {
        WalLeafEvidence::Absent => Ok(()),
        WalLeafEvidence::Regular { size, .. } => read_budget.reserve(*size, role),
    }
}

pub(super) fn external_conflict(diagnostic: &str) -> ExternalConfigRecoveryAssessment {
    ExternalConfigRecoveryAssessment {
        classification: WriteAuthorityRecoveryClassification::Conflict,
        automatic_action: None,
        available_resolution_actions: Vec::new(),
        diagnostic: diagnostic.into(),
    }
}
