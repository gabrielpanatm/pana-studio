use std::io::Write;

use super::*;

pub(in crate::kernel::write_authority::capability) fn classify_append_recovery(
    record: &WalRecord,
    phase: WalPhase,
    checkpoint: Option<&WalAppendStageCheckpoint>,
    read_budget: &mut RecoveryReadBudget,
) -> Result<AppendRecoveryAssessment, String> {
    let WalOperationEvidence::Append(evidence) = &record.body.operation_evidence else {
        return Err("WriteAuthority Append recovery a primit altă familie.".into());
    };
    if evidence.protocol_version == 0 {
        if checkpoint.is_some() {
            return Err("Append legacy refuză checkpoint filename v2.".into());
        }
        return super::super::classify_legacy_append_recovery(record, phase, read_budget);
    }
    if evidence.protocol_version != WAL_APPEND_PROTOCOL_VERSION {
        return Err(format!(
            "Append WAL folosește protocolul necunoscut {}.",
            evidence.protocol_version
        ));
    }
    validate_append_recovery_owner(record, evidence)?;
    match phase {
        WalPhase::Prepared if checkpoint.is_some() => {
            return Err("Append v2 Prepared refuză checkpoint prematur.".into());
        }
        WalPhase::AuxiliaryDurable | WalPhase::EffectVisible | WalPhase::TargetDurable
            if checkpoint.is_none() =>
        {
            return Err(format!(
                "Append v2 {phase:?} cere checkpoint filename cauzal."
            ));
        }
        WalPhase::Preparing => {
            return Err("Append v2 classifier nu execută recorduri Preparing.".into());
        }
        WalPhase::Prepared
        | WalPhase::AuxiliaryDurable
        | WalPhase::EffectVisible
        | WalPhase::TargetDurable => {}
    }

    let context = super::super::capture_recovery_append_context(record, evidence)?;
    let super::super::RecoveryAppendContext::Ready {
        directory,
        target_leaf,
        parent_was_missing,
    } = context
    else {
        return Ok(AppendRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::Conflict,
            automatic_action: None,
            diagnostic: "Append v2 cere parent existent integral; parentul WAL lipsește.".into(),
        });
    };
    if parent_was_missing {
        return Err("Append v2 refuză evidence cu parent creat implicit.".into());
    }
    fs::flock(&directory, FlockOperation::LockExclusive)
        .map_err(|error| format!("Append v2 recovery stable lock a eșuat: {error}."))?;
    classify_locked_target(
        record,
        evidence,
        phase,
        checkpoint,
        &directory,
        &target_leaf,
        read_budget,
    )
}

pub(in crate::kernel::write_authority::capability) fn execute_append_recovery(
    record: &WalRecord,
    phase: WalPhase,
    checkpoint: Option<&WalAppendStageCheckpoint>,
    read_budget: &mut RecoveryReadBudget,
) -> Result<(), String> {
    let WalOperationEvidence::Append(evidence) = &record.body.operation_evidence else {
        return Err("Append v2 executor a primit altă familie.".into());
    };
    if evidence.protocol_version == 0 {
        if checkpoint.is_some() {
            return Err("Append legacy executor refuză checkpoint v2.".into());
        }
        return super::super::execute_legacy_append_recovery(record, phase, read_budget);
    }
    if evidence.protocol_version != WAL_APPEND_PROTOCOL_VERSION {
        return Err("Append v2 executor refuză protocolul necunoscut.".into());
    }
    match phase {
        WalPhase::Prepared if checkpoint.is_some() => {
            return Err("Append v2 Prepared refuză checkpoint.".into());
        }
        WalPhase::AuxiliaryDurable | WalPhase::EffectVisible | WalPhase::TargetDurable
            if checkpoint.is_none() =>
        {
            return Err("Append v2 fază post-Prepared fără checkpoint.".into());
        }
        WalPhase::Preparing => return Err("Append v2 executor refuză Preparing.".into()),
        _ => {}
    }
    validate_append_recovery_owner(record, evidence)?;
    let context = super::super::capture_recovery_append_context(record, evidence)?;
    let super::super::RecoveryAppendContext::Ready {
        directory,
        target_leaf,
        parent_was_missing,
    } = context
    else {
        return Err("Append v2 executor nu mai găsește parentul.".into());
    };
    if parent_was_missing {
        return Err("Append v2 executor refuză parent creat implicit.".into());
    }
    fs::flock(&directory, FlockOperation::LockExclusive)
        .map_err(|error| format!("Append v2 executor stable lock a eșuat: {error}."))?;
    let assessment = classify_locked_target(
        record,
        evidence,
        phase,
        checkpoint,
        &directory,
        &target_leaf,
        read_budget,
    )?;
    let action = assessment.automatic_action.ok_or_else(|| {
        format!(
            "WriteAuthority Append v2 recovery nu permite acțiune automată: {}",
            assessment.diagnostic
        )
    })?;
    let checkpoint = if action == AppendRecoveryAction::ClearNoEffect {
        checkpoint
    } else {
        Some(checkpoint.ok_or_else(|| "Append v2 executor cere checkpoint.".to_string())?)
    };

    match action {
        AppendRecoveryAction::ClearNoEffect => Ok(()),
        AppendRecoveryAction::ContinueExactPrefix => {
            let checkpoint = checkpoint.expect("effect actions require checkpoint");
            let mut file = open_recovery_append_target(&directory, &target_leaf)?;
            let suffix_size = inspect_present_suffix(&mut file, evidence, checkpoint, read_budget)?;
            if suffix_size == 0 || suffix_size >= evidence.payload_size {
                return Err("Append v2 recovery prefix s-a schimbat înainte de continuare.".into());
            }
            let payload = append_payload(evidence)?;
            let remaining = &payload[suffix_size as usize..];
            let written = file
                .write(remaining)
                .map_err(|error| format!("Append v2 recovery write a eșuat: {error}."))?;
            if written != remaining.len() {
                let _ = file.sync_data();
                return Err(format!(
                    "Append v2 recovery short write {written}/{}; următorul restart va relua prefixul exact.",
                    remaining.len()
                ));
            }
            file.sync_data()
                .map_err(|error| format!("Append v2 recovery fdatasync a eșuat: {error}."))?;
            validate_recovery_complete(
                &mut file,
                &directory,
                &target_leaf,
                evidence,
                checkpoint,
                read_budget,
            )
        }
        AppendRecoveryAction::FinalizeCommitted => {
            let checkpoint = checkpoint.expect("effect actions require checkpoint");
            let mut file = open_recovery_append_target(&directory, &target_leaf)?;
            file.sync_data()
                .map_err(|error| format!("Append v2 recovery fdatasync a eșuat: {error}."))?;
            validate_recovery_complete(
                &mut file,
                &directory,
                &target_leaf,
                evidence,
                checkpoint,
                read_budget,
            )?;
            if matches!(evidence.before, WalAppendBefore::Absent) {
                sync_directory(&directory, &record.body.public_label)?;
            }
            Ok(())
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn classify_locked_target(
    record: &WalRecord,
    evidence: &WalAppendEvidence,
    phase: WalPhase,
    checkpoint: Option<&WalAppendStageCheckpoint>,
    directory: &OwnedFd,
    target_leaf: &OsStr,
    read_budget: &mut RecoveryReadBudget,
) -> Result<AppendRecoveryAssessment, String> {
    if phase != WalPhase::Prepared {
        let Some(checkpoint) = checkpoint else {
            return Ok(conflict("Append v2 fază post-Prepared fără checkpoint."));
        };
        let (role, before_size, expected_identity) = match &evidence.before {
            WalAppendBefore::Absent => (WalAppendStageRole::CreateTarget, 0, None),
            WalAppendBefore::Present { size, .. } => (
                WalAppendStageRole::ExistingTarget,
                *size,
                evidence.before_identity_digest.as_deref(),
            ),
        };
        if !checkpoint.matches_payload_contract(
            &evidence.payload_hash,
            evidence.payload_size,
            before_size,
            role,
        ) || checkpoint.role != role
            || expected_identity
                .is_some_and(|identity| checkpoint.target_identity_digest != identity)
        {
            return Ok(conflict(
                "Append v2 checkpointul filename nu corespunde identity/role/payload evidence.",
            ));
        }
    }
    let observed = super::super::open_recovery_regular_leaf(
        directory,
        target_leaf,
        &record.body.public_label,
        "append-v2-recovery-target",
    )?;
    match (&evidence.before, observed) {
        (WalAppendBefore::Absent, None) => match phase {
            WalPhase::Prepared => Ok(AppendRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::NoEffect,
                automatic_action: Some(AppendRecoveryAction::ClearNoEffect),
                diagnostic:
                    "Append v2 Prepared nu a creat checkpointul pre-efect și targetul este absent."
                        .into(),
            }),
            WalPhase::AuxiliaryDurable
            | WalPhase::EffectVisible
            | WalPhase::TargetDurable => Ok(AppendRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::Conflict,
                automatic_action: None,
                diagnostic: format!(
                    "Append v2 {phase:?} are gate-ul pre-efect trecut, dar targetul lipsește; linkat urmează checkpointului și un efect publicat apoi eliminat nu poate fi exclus."
                ),
            }),
            WalPhase::Preparing => unreachable!(),
        },
        (WalAppendBefore::Absent, Some((mut file, stat))) => {
            if phase == WalPhase::Prepared {
                return Ok(AppendRecoveryAssessment {
                    classification: WriteAuthorityRecoveryClassification::Conflict,
                    automatic_action: None,
                    diagnostic:
                        "Append v2 Prepared a observat un target competitor înainte de checkpoint."
                            .into(),
                });
            }
            let checkpoint = checkpoint.expect("later phases require checkpoint");
            if !checkpoint.matches_payload_contract(
                &evidence.payload_hash,
                evidence.payload_size,
                0,
                WalAppendStageRole::CreateTarget,
            ) || checkpoint.role != WalAppendStageRole::CreateTarget
                || append_identity_digest(&file, WalAppendStageRole::CreateTarget)?
                    != checkpoint.target_identity_digest
                || stat.st_nlink != 1
                || stat.st_mode & 0o7777 != 0o600
                || u64::try_from(stat.st_size).ok() != Some(evidence.payload_size)
            {
                return Ok(conflict("Append v2 create target nu corespunde checkpointului."));
            }
            read_budget.reserve(evidence.payload_size, "append v2 create payload")?;
            let payload = read_exact_range(&mut file, 0, evidence.payload_size)?;
            if sha256_bytes(&payload) != evidence.payload_hash {
                return Ok(conflict("Append v2 create target are payload divergent."));
            }
            Ok(AppendRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::EffectCommitted,
                automatic_action: Some(AppendRecoveryAction::FinalizeCommitted),
                diagnostic:
                    "Append v2 create target este inode-ul checkpointed cu payload complet."
                        .into(),
            })
        }
        (WalAppendBefore::Present { .. }, None) => Ok(conflict(
            "Append v2 targetul Present lipsește la recovery.",
        )),
        (WalAppendBefore::Present { .. }, Some((mut file, stat))) => {
            let before_size = append_before_size(&evidence.before);
            let current_size = u64::try_from(stat.st_size)
                .map_err(|_| "Append v2 recovery target are size negativ.".to_string())?;
            let baseline_identity = evidence.before_identity_digest.as_deref().ok_or_else(|| {
                "Append v2 Present nu are identity digest în record.".to_string()
            })?;
            if append_identity_digest(&file, WalAppendStageRole::ExistingTarget)?
                != baseline_identity
                || stat.st_nlink != 1
            {
                return Ok(conflict(
                    "Append v2 targetul Present nu mai este lifetime-ul baseline.",
                ));
            }
            let (expected_mode, expected_links) = append_baseline_mode_and_links(evidence)?;
            if stat.st_mode != expected_mode || stat.st_nlink != expected_links {
                return Ok(conflict(
                    "Append v2 targetul Present și-a schimbat mode/nlink față de baseline.",
                ));
            }
            if current_size == before_size {
                let WalAppendBefore::Present { version_token, .. } = &evidence.before else {
                    unreachable!()
                };
                if append_version_token(&stat) != *version_token
                    || validate_tail_contract_bounded(
                        &mut file,
                        before_size,
                        evidence,
                        &record.body.public_label,
                        read_budget,
                    )
                    .is_err()
                {
                    return Ok(conflict(
                        "Append v2 baseline fără suffix și-a schimbat versiunea/tail-ul.",
                    ));
                }
                return match phase {
                    WalPhase::Prepared => Ok(AppendRecoveryAssessment {
                            classification: WriteAuthorityRecoveryClassification::NoEffect,
                            automatic_action: Some(AppendRecoveryAction::ClearNoEffect),
                            diagnostic: "Append v2 Prepared nu a publicat niciun byte.".into(),
                        }),
                    WalPhase::AuxiliaryDurable
                    | WalPhase::EffectVisible
                    | WalPhase::TargetDurable => Ok(conflict(format!(
                        "Append v2 {phase:?} a trecut gate-ul pre-efect, dar targetul este din nou baseline exact; un append executat apoi eliminat nu poate fi exclus."
                    ))),
                    WalPhase::Preparing => unreachable!(),
                };
            }
            if phase == WalPhase::Prepared {
                return Ok(conflict(
                    "Append v2 Prepared a observat suffix înainte de checkpoint.",
                ));
            }
            let checkpoint = checkpoint.expect("later phases require checkpoint");
            if checkpoint.target_identity_digest != baseline_identity
                || !checkpoint.matches_payload_contract(
                    &evidence.payload_hash,
                    evidence.payload_size,
                    before_size,
                    WalAppendStageRole::ExistingTarget,
                )
            {
                return Ok(conflict(
                    "Append v2 checkpointul Present nu corespunde recordului.",
                ));
            }
            let suffix_size = inspect_present_suffix(
                &mut file,
                evidence,
                checkpoint,
                read_budget,
            )?;
            if suffix_size == evidence.payload_size {
                return Ok(AppendRecoveryAssessment {
                    classification: WriteAuthorityRecoveryClassification::EffectCommitted,
                    automatic_action: Some(AppendRecoveryAction::FinalizeCommitted),
                    diagnostic:
                        "Append v2 are suffix complet exact, compatibil cu checkpointul și contractul single-writer."
                            .into(),
                });
            }
            if phase != WalPhase::AuxiliaryDurable {
                return Ok(conflict(
                    "Append v2 suffix parțial este compatibil numai cu AuxiliaryDurable.",
                ));
            }
            Ok(AppendRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::PartialAppend,
                automatic_action: Some(AppendRecoveryAction::ContinueExactPrefix),
                diagnostic:
                    "Append v2 are un prefix exact; recovery va scrie numai restul payloadului."
                        .into(),
            })
        }
    }
}

fn inspect_present_suffix(
    file: &mut File,
    evidence: &WalAppendEvidence,
    checkpoint: &WalAppendStageCheckpoint,
    read_budget: &mut RecoveryReadBudget,
) -> Result<u64, String> {
    let before_size = append_before_size(&evidence.before);
    let stat =
        fs::fstat(&*file).map_err(|error| format!("Append v2 recovery fstat a eșuat: {error}."))?;
    let current_size =
        u64::try_from(stat.st_size).map_err(|_| "Append v2 recovery size negativ.".to_string())?;
    let final_size = before_size
        .checked_add(evidence.payload_size)
        .ok_or_else(|| "Append v2 final size overflow.".to_string())?;
    if current_size <= before_size || current_size > final_size || stat.st_nlink != 1 {
        return Err("Append v2 suffix size/nlink incompatibil.".into());
    }
    if append_identity_digest(&*file, WalAppendStageRole::ExistingTarget)?
        != checkpoint.target_identity_digest
    {
        return Err("Append v2 suffix inode diferă de checkpoint.".into());
    }
    validate_tail_contract_bounded(
        file,
        before_size,
        evidence,
        "append-v2-recovery",
        read_budget,
    )?;
    let suffix_size = current_size - before_size;
    read_budget.reserve(suffix_size, "append v2 suffix")?;
    let suffix = read_exact_range(file, before_size, suffix_size)?;
    let payload = append_payload(evidence)?;
    if suffix != payload[..suffix.len()] {
        return Err("Append v2 suffix nu este prefix exact al payloadului.".into());
    }
    Ok(suffix_size)
}

fn validate_recovery_complete(
    file: &mut File,
    directory: &OwnedFd,
    target_leaf: &OsStr,
    evidence: &WalAppendEvidence,
    checkpoint: &WalAppendStageCheckpoint,
    read_budget: &mut RecoveryReadBudget,
) -> Result<(), String> {
    let before_size = append_before_size(&evidence.before);
    let final_size = before_size
        .checked_add(evidence.payload_size)
        .ok_or_else(|| "Append v2 recovery final size overflow.".to_string())?;
    let stat = fs::fstat(&*file)
        .map_err(|error| format!("Append v2 recovery post fstat a eșuat: {error}."))?;
    if u64::try_from(stat.st_size).ok() != Some(final_size)
        || stat.st_nlink != 1
        || (matches!(evidence.before, WalAppendBefore::Absent) && stat.st_mode & 0o7777 != 0o600)
    {
        return Err("Append v2 recovery postflight size/nlink diferă.".into());
    }
    validate_named_file_identity(directory, target_leaf, &stat, "append-v2-recovery-post")?;
    let role = if matches!(evidence.before, WalAppendBefore::Absent) {
        WalAppendStageRole::CreateTarget
    } else {
        WalAppendStageRole::ExistingTarget
    };
    let identity_before_hash = append_identity_digest(&*file, role)?;
    if identity_before_hash != checkpoint.target_identity_digest {
        return Err("Append v2 recovery postflight identity diferă.".into());
    }
    if matches!(evidence.before, WalAppendBefore::Present { .. }) {
        let (expected_mode, expected_links) = append_baseline_mode_and_links(evidence)?;
        if stat.st_mode != expected_mode || stat.st_nlink != expected_links {
            return Err("Append v2 recovery postflight mode/nlink diferă de baseline.".into());
        }
        validate_tail_contract_bounded(
            file,
            before_size,
            evidence,
            "append-v2-recovery-post",
            read_budget,
        )?;
    }
    read_budget.reserve(
        evidence.payload_size,
        "append v2 recovery payload postflight",
    )?;
    let payload = read_exact_range(file, before_size, evidence.payload_size)?;
    if sha256_bytes(&payload) != evidence.payload_hash {
        return Err("Append v2 recovery postflight hash diferă.".into());
    }
    run_test_hook(CapabilityTestStage::AfterAppendV2RecoveryHash);
    let after = fs::fstat(&*file)
        .map_err(|error| format!("Append v2 recovery post-hash fstat a eșuat: {error}."))?;
    if !same_file_identity(&stat, &after)
        || append_version_token(&stat) != append_version_token(&after)
        || u64::try_from(after.st_size).ok() != Some(final_size)
        || after.st_nlink != 1
        || append_identity_digest(&*file, role)? != identity_before_hash
    {
        return Err("Append v2 recovery target s-a schimbat în timpul hash-ului.".into());
    }
    validate_named_file_identity(
        directory,
        target_leaf,
        &after,
        "append-v2-recovery-post-hash",
    )?;
    Ok(())
}

fn validate_tail_contract_bounded(
    file: &mut File,
    before_size: u64,
    evidence: &WalAppendEvidence,
    public_label: &str,
    read_budget: &mut RecoveryReadBudget,
) -> Result<(), String> {
    read_budget.reserve(evidence.before_tail_size, "append v2 baseline tail")?;
    validate_tail_contract(file, before_size, evidence, public_label)
}

fn append_baseline_mode_and_links(evidence: &WalAppendEvidence) -> Result<(u32, u64), String> {
    let WalAppendBefore::Present { version_token, .. } = &evidence.before else {
        return Err("Append v2 mode baseline este disponibil numai pentru Present.".into());
    };
    let fields = version_token.split(':').collect::<Vec<_>>();
    if fields.len() != 10 || fields[0] != "append-v2" {
        return Err("Append v2 version token Present nu este canonic.".into());
    }
    let mode = fields[8]
        .parse::<u32>()
        .map_err(|_| "Append v2 version token are mode invalid.".to_string())?;
    let links = fields[9]
        .parse::<u64>()
        .map_err(|_| "Append v2 version token are nlink invalid.".to_string())?;
    Ok((mode, links))
}

fn open_recovery_append_target(parent: &OwnedFd, leaf: &OsStr) -> Result<File, String> {
    let descriptor = fs::openat(
        parent,
        leaf,
        OFlags::RDWR | OFlags::APPEND | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| format!("Append v2 recovery open a eșuat: {error}."))?;
    validate_regular_single_link(&descriptor, "append-v2-recovery", "Append v2 recovery")?;
    Ok(File::from(descriptor))
}

fn append_payload(evidence: &WalAppendEvidence) -> Result<Vec<u8>, String> {
    let payload = decode_bytes_hex(
        evidence
            .payload_hex
            .as_deref()
            .ok_or_else(|| "Append v2 recovery cere payload complet.".to_string())?,
    )?;
    if payload.len() as u64 != evidence.payload_size
        || payload.len() > MAX_WAL_APPEND_PAYLOAD_BYTES
        || sha256_bytes(&payload) != evidence.payload_hash
    {
        return Err("Append v2 recovery payload contract invalid.".into());
    }
    Ok(payload)
}

fn validate_append_recovery_owner(
    record: &WalRecord,
    evidence: &WalAppendEvidence,
) -> Result<(), String> {
    if record.body.category != "internal_app_write"
        || record.body.operation != "append_text"
        || record.body.recovery_policy != "append_only_journal"
        || record.body.authority.scope != "application_data"
    {
        return Err("Append v2 recovery refuză category/operation/policy/scope.".into());
    }
    let parents = evidence
        .parent
        .relative_components_hex
        .iter()
        .map(|component| decode_component_hex(component))
        .collect::<Result<Vec<_>, _>>()?;
    if parents.len() != 2
        || parents[0] != OsString::from("sessions")
        || parents[1].is_empty()
        || evidence.parent.existing_prefix_len != parents.len()
        || evidence.parent.parent_identity.is_none()
    {
        return Err("Append v2 recovery refuză parent evidence non-session.".into());
    }
    let leaf = decode_component_hex(&evidence.target_leaf_hex)?;
    let leaf = leaf
        .to_str()
        .ok_or_else(|| "Append v2 recovery cere leaf UTF-8 declarat.".to_string())?;
    let allowed = match record.body.owner.as_str() {
        "kernel" => matches!(
            leaf,
            "project-transition-decisions.jsonl"
                | "project-transition-decision-recovery-acknowledgements.jsonl"
        ),
        _ => false,
    };
    if !allowed {
        return Err("Append v2 recovery refuză owner/path neautorizat.".into());
    }
    Ok(())
}

fn conflict(diagnostic: impl Into<String>) -> AppendRecoveryAssessment {
    AppendRecoveryAssessment {
        classification: WriteAuthorityRecoveryClassification::Conflict,
        automatic_action: None,
        diagnostic: diagnostic.into(),
    }
}
