use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
struct ObservedCopyV2Leaf {
    identity: WalFilesystemIdentity,
    identity_digest: String,
    size: u64,
    version_token: String,
    mode_bits: u32,
}

pub(in crate::kernel::write_authority::capability) fn classify_copy_recovery(
    record: &WalRecord,
    phase: WalPhase,
    checkpoint: Option<&WalCopyStageCheckpoint>,
) -> Result<CopyRecoveryAssessment, String> {
    let WalOperationEvidence::Copy(evidence) = &record.body.operation_evidence else {
        return Err("WriteAuthority Copy recovery a primit altă familie.".into());
    };
    if evidence.protocol_version == 0 {
        if checkpoint.is_some() {
            return Ok(copy_conflict(
                "Copy legacy nu poate purta checkpoint Copy v2 în filename.",
            ));
        }
        return classify_legacy_copy_recovery(record, evidence, phase);
    }
    if evidence.protocol_version != WAL_COPY_PROTOCOL_VERSION {
        return Ok(copy_conflict(&format!(
            "Copy WAL folosește protocolul necunoscut {}.",
            evidence.protocol_version
        )));
    }
    if !copy_owner_contract_matches(record, evidence) {
        return Ok(copy_conflict(
            "Copy v2 owner/category/destination policy nu corespund contractului persistent.",
        ));
    }
    let expected_role = copy_stage_role(evidence);
    match phase {
        WalPhase::Prepared if checkpoint.is_some() => {
            return Ok(copy_conflict(
                "Copy v2 Prepared nu poate purta checkpoint staged.",
            ));
        }
        WalPhase::AuxiliaryDurable | WalPhase::EffectVisible | WalPhase::TargetDurable => {
            let Some(checkpoint) = checkpoint else {
                return Ok(copy_conflict(
                    "Copy v2 post-Prepared nu are checkpoint cauzal staged.",
                ));
            };
            if !checkpoint.matches_payload_contract(
                &evidence.file.new_content_hash,
                evidence.file.new_size,
                evidence.new_mode_bits,
                expected_role,
            ) {
                return Ok(copy_conflict(
                    "Checkpointul Copy v2 nu corespunde hash/size/mode/rol din record.",
                ));
            }
        }
        WalPhase::Preparing => {
            return Ok(copy_conflict(
                "Classifierul Copy nu execută recorduri Preparing.",
            ));
        }
        WalPhase::Prepared => {}
    }

    let context = capture_recovery_atomic_context(record, &evidence.file)?;
    let RecoveryAtomicContext::Ready {
        directory,
        target_leaf,
        temp_leaf,
        parent_was_missing,
    } = context
    else {
        return Ok(copy_conflict(
            "Copy v2 cere parentul existent integral din WAL prepare, dar recovery nu îl mai poate captura.",
        ));
    };
    if parent_was_missing {
        return Ok(copy_conflict(
            "Copy v2 refuză evidence care revendică namespace părinte creat de Copy.",
        ));
    }

    let target = observe_copy_v2_leaf(
        &directory,
        &target_leaf,
        expected_role,
        &record.body.public_label,
    )?;
    let temp = observe_copy_v2_leaf(
        &directory,
        &temp_leaf,
        expected_role,
        &record.body.public_label,
    )?;
    let target_before = observed_v2_matches_before(target.as_ref(), evidence);
    let temp_absent = temp.is_none();

    if phase == WalPhase::Prepared {
        return if target_before && temp_absent {
            Ok(CopyRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::NoEffect,
                automatic_action: Some(CopyRecoveryAction::ClearNoEffect),
                diagnostic:
                    "Copy v2 Prepared este exact baseline și nu are leaf staged nominalizat.".into(),
            })
        } else {
            Ok(copy_conflict(
                "Copy v2 Prepared a observat efect de namespace înainte de checkpoint.",
            ))
        };
    }

    let checkpoint = checkpoint.expect("post-Prepared checkpoint validated");
    let target_new = observed_v2_matches_checkpoint(target.as_ref(), evidence, checkpoint);
    let temp_new = observed_v2_matches_checkpoint(temp.as_ref(), evidence, checkpoint);
    let recovery_can_hash = evidence.file.new_size <= MAX_WAL_RECOVERY_READ_BYTES;

    match evidence.destination_policy {
        WalCopyDestinationPolicy::CreateNew => {
            if !temp_absent {
                return Ok(copy_conflict(
                    "Copy create-only v2 a observat un temp nominalizat, deși protocolul publică direct targetul.",
                ));
            }
            if target_before {
                return if phase == WalPhase::AuxiliaryDurable {
                    Ok(CopyRecoveryAssessment {
                        classification:
                            WriteAuthorityRecoveryClassification::RollbackCompleted,
                        automatic_action: None,
                        diagnostic:
                            "Copy create-only este încă la baseline după checkpointul anonim. Absența numelui nu poate demonstra global că inode-ul nu a fost publicat și mutat; operatorul poate accepta explicit starea baseline."
                                .into(),
                    })
                } else {
                    Ok(copy_conflict(
                        "Copy create-only are marker EffectVisible/TargetDurable, dar targetul a revenit la baseline; efectul publicat a fost mutat sau șters și WAL rămâne hot.",
                    ))
                };
            }
            if target_new {
                return Ok(CopyRecoveryAssessment {
                    classification: WriteAuthorityRecoveryClassification::EffectCommitted,
                    automatic_action: recovery_can_hash
                        .then_some(CopyRecoveryAction::FinalizeCommitted),
                    diagnostic: recovery_hash_diagnostic(
                        recovery_can_hash,
                        "Targetul Copy create-only este inode-ul checkpointat",
                    ),
                });
            }
        }
        WalCopyDestinationPolicy::Replace => {
            if target_before && temp_absent {
                return if phase == WalPhase::AuxiliaryDurable {
                    Ok(CopyRecoveryAssessment {
                        classification:
                            WriteAuthorityRecoveryClassification::RollbackCompleted,
                        automatic_action: None,
                        diagnostic:
                            "Copy Preview este încă la baseline după checkpointul anonim. Absența temp-ului nu poate demonstra global că inode-ul nu a fost publicat și mutat; operatorul poate accepta explicit starea baseline."
                                .into(),
                    })
                } else {
                    Ok(copy_conflict(
                        "Copy Preview are marker EffectVisible/TargetDurable, dar targetul a revenit la baseline; efectul publicat a fost înlocuit și WAL rămâne hot.",
                    ))
                };
            }
            if target_before && temp_new {
                return if phase == WalPhase::AuxiliaryDurable {
                    Ok(CopyRecoveryAssessment {
                        classification: WriteAuthorityRecoveryClassification::StagedOnly,
                        automatic_action: recovery_can_hash
                            .then_some(CopyRecoveryAction::CommitStagedReplace),
                        diagnostic: recovery_hash_diagnostic(
                            recovery_can_hash,
                            "Temp-ul Copy Preview este inode-ul checkpointat, iar targetul este baseline",
                        ),
                    })
                } else {
                    Ok(copy_conflict(
                        "Copy Preview are temp staged și target baseline după markerul EffectVisible/TargetDurable; intervenția externă este obligatoriu revizuită.",
                    ))
                };
            }
            if target_new && temp_absent {
                return Ok(CopyRecoveryAssessment {
                    classification: WriteAuthorityRecoveryClassification::EffectCommitted,
                    automatic_action: recovery_can_hash
                        .then_some(CopyRecoveryAction::FinalizeCommitted),
                    diagnostic: recovery_hash_diagnostic(
                        recovery_can_hash,
                        "Targetul Copy Preview este inode-ul checkpointat și temp-ul lipsește",
                    ),
                });
            }
        }
    }

    Ok(copy_conflict(&format!(
        "Oracle Copy v2 necunoscut (phase={phase:?}, targetBefore={target_before}, targetNew={target_new}, tempAbsent={temp_absent}, tempNew={temp_new})."
    )))
}

pub(in crate::kernel::write_authority::capability) fn execute_copy_recovery(
    record: &WalRecord,
    phase: WalPhase,
    checkpoint: Option<&WalCopyStageCheckpoint>,
    read_budget: &mut RecoveryReadBudget,
) -> Result<(), String> {
    let assessment = classify_copy_recovery(record, phase, checkpoint)?;
    let action = assessment.automatic_action.ok_or_else(|| {
        format!(
            "WriteAuthority Copy recovery CAS nu permite acțiune automată: {}",
            assessment.diagnostic
        )
    })?;
    let WalOperationEvidence::Copy(evidence) = &record.body.operation_evidence else {
        return Err("WriteAuthority Copy executor a primit altă familie.".into());
    };
    match action {
        CopyRecoveryAction::ClearNoEffect => Ok(()),
        CopyRecoveryAction::CommitStagedReplace => {
            commit_staged_copy_replace(record, evidence, checkpoint, read_budget)
        }
        CopyRecoveryAction::FinalizeCommitted => {
            finalize_committed_copy(record, evidence, checkpoint, read_budget)
        }
    }
}

pub(in crate::kernel::write_authority::capability) fn resolve_copy_operator(
    record: &WalRecord,
    phase: WalPhase,
    checkpoint: Option<&WalCopyStageCheckpoint>,
    action: WriteAuthorityRecoveryResolutionAction,
) -> Result<String, String> {
    if action != WriteAuthorityRecoveryResolutionAction::AcceptRestoredState {
        return Err("Copy v2 acceptă numai acțiunea operator AcceptRestoredState.".into());
    }
    if phase != WalPhase::AuxiliaryDurable {
        return Err("Copy v2 poate accepta baseline numai din AuxiliaryDurable.".into());
    }
    let assessment = classify_copy_recovery(record, phase, checkpoint)?;
    if assessment.classification != WriteAuthorityRecoveryClassification::RollbackCompleted
        || assessment.automatic_action.is_some()
    {
        return Err(format!(
            "Copy v2 baseline resolution CAS nu mai este disponibilă: {}",
            assessment.diagnostic
        ));
    }
    let WalOperationEvidence::Copy(evidence) = &record.body.operation_evidence else {
        return Err("Copy v2 operator a primit altă familie.".into());
    };
    finalize_copy_baseline_state(record, evidence, checkpoint)?;
    Ok("Operatorul a acceptat explicit starea baseline Copy v2 după verificare full-path; niciun target sau temp nu a fost modificat.".into())
}

fn finalize_copy_baseline_state(
    record: &WalRecord,
    evidence: &WalCopyEvidence,
    checkpoint: Option<&WalCopyStageCheckpoint>,
) -> Result<(), String> {
    let (directory, target_leaf, temp_leaf) = ready_copy_context(record, evidence)?;
    let role = copy_stage_role(evidence);
    let target = observe_copy_v2_leaf(&directory, &target_leaf, role, &record.body.public_label)?;
    let temp = observe_copy_v2_leaf(&directory, &temp_leaf, role, &record.body.public_label)?;
    if !observed_v2_matches_before(target.as_ref(), evidence) || temp.is_some() {
        return Err("Copy baseline finalize CAS a observat alt namespace.".into());
    }
    if checkpoint.is_none() {
        return Err("Copy baseline finalize post-Prepared cere checkpoint.".into());
    }
    sync_directory(&directory, &record.body.public_label)?;
    verify_copy_public_baseline(record, evidence)
}

fn commit_staged_copy_replace(
    record: &WalRecord,
    evidence: &WalCopyEvidence,
    checkpoint: Option<&WalCopyStageCheckpoint>,
    read_budget: &mut RecoveryReadBudget,
) -> Result<(), String> {
    if evidence.destination_policy != WalCopyDestinationPolicy::Replace {
        return Err("Copy staged replace executor a primit altă policy.".into());
    }
    let checkpoint = checkpoint.ok_or("Copy staged replace cere checkpoint.")?;
    let (directory, target_leaf, temp_leaf) = ready_copy_context(record, evidence)?;
    let role = WalCopyStageRole::ReplaceTemporary;
    let target = observe_copy_v2_leaf(&directory, &target_leaf, role, &record.body.public_label)?;
    if !observed_v2_matches_before(target.as_ref(), evidence) {
        return Err("Copy staged replace nu mai vede baseline-ul target exact.".into());
    }
    let (mut staged_file, staged_stat) = open_exact_checkpointed_copy_leaf(
        &directory,
        &temp_leaf,
        evidence,
        checkpoint,
        role,
        &record.body.public_label,
        "copy staged replace temp",
        read_budget,
        false,
    )?;
    // Admiterea de resurse precede orice efect de namespace. Un WAL corupt cu
    // mai multe operation IDs poate consuma bugetul printr-un record anterior;
    // Copy nu suprascrie cache-ul înainte să știe că poate valida payloadul.
    read_budget.reserve(
        evidence.file.new_size,
        "copy staged replace committed target",
    )?;
    fs::renameat(&directory, &temp_leaf, &directory, &target_leaf).map_err(|error| {
        capability_error(
            &record.body.public_label,
            &format!("Copy recovery rename temp -> target a eșuat: {error}"),
        )
    })?;
    let committed_before_hash = fs::fstat(&staged_file).map_err(|error| {
        capability_error(
            &record.body.public_label,
            &format!("Copy recovery target fstat a eșuat: {error}"),
        )
    })?;
    if !same_file_identity(&staged_stat, &committed_before_hash)
        || committed_before_hash.st_nlink != 1
        || staged_stat.st_size != committed_before_hash.st_size
        || staged_stat.st_mtime != committed_before_hash.st_mtime
        || staged_stat.st_mtime_nsec != committed_before_hash.st_mtime_nsec
        || mode_bits(&staged_stat) != mode_bits(&committed_before_hash)
    {
        return Err(
            "Copy recovery inode-ul/payloadul staged s-a schimbat în afara efectului ctime permis de rename."
                .into(),
        );
    }
    validate_named_file_identity(
        &directory,
        &target_leaf,
        &committed_before_hash,
        "copy-recovery-committed-target",
    )?;
    let hash = hash_open_file_exact(
        &mut staged_file,
        evidence.file.new_size,
        "copy staged replace committed target",
    )?;
    run_test_hook(CapabilityTestStage::AfterCopyRecoveryHash);
    let committed = fs::fstat(&staged_file).map_err(|error| {
        capability_error(
            &record.body.public_label,
            &format!("Copy recovery target post-hash fstat a eșuat: {error}"),
        )
    })?;
    if !same_file_identity(&committed_before_hash, &committed)
        || version_token_for_stat(&committed_before_hash) != version_token_for_stat(&committed)
        || committed_before_hash.st_nlink != committed.st_nlink
        || hash != evidence.file.new_content_hash
    {
        return Err("Copy recovery targetul s-a schimbat sau are alt hash după rename.".into());
    }
    staged_file.sync_all().map_err(|error| {
        capability_error(
            &record.body.public_label,
            &format!("Copy recovery target fsync a eșuat: {error}"),
        )
    })?;
    let stable_committed = fs::fstat(&staged_file).map_err(|error| {
        capability_error(
            &record.body.public_label,
            &format!("Copy recovery target post-fsync fstat a eșuat: {error}"),
        )
    })?;
    if version_token_for_stat(&committed) != version_token_for_stat(&stable_committed)
        || committed.st_nlink != stable_committed.st_nlink
    {
        return Err("Copy recovery target s-a schimbat după hash înainte de postflight.".into());
    }
    sync_directory(&directory, &record.body.public_label)?;
    verify_copy_public_committed(record, evidence, checkpoint, &stable_committed)
}

fn finalize_committed_copy(
    record: &WalRecord,
    evidence: &WalCopyEvidence,
    checkpoint: Option<&WalCopyStageCheckpoint>,
    read_budget: &mut RecoveryReadBudget,
) -> Result<(), String> {
    let checkpoint = checkpoint.ok_or("Copy committed finalize cere checkpoint.")?;
    let (directory, target_leaf, temp_leaf) = ready_copy_context(record, evidence)?;
    let role = copy_stage_role(evidence);
    let (target_file, target_stat) = open_exact_checkpointed_copy_leaf(
        &directory,
        &target_leaf,
        evidence,
        checkpoint,
        role,
        &record.body.public_label,
        "copy committed target",
        read_budget,
        true,
    )?;
    if observe_copy_v2_leaf(&directory, &temp_leaf, role, &record.body.public_label)?.is_some() {
        return Err("Copy committed finalize a observat temp neașteptat.".into());
    }
    target_file.sync_all().map_err(|error| {
        capability_error(
            &record.body.public_label,
            &format!("Copy committed target fsync a eșuat: {error}"),
        )
    })?;
    let stable_target = fs::fstat(&target_file).map_err(|error| {
        capability_error(
            &record.body.public_label,
            &format!("Copy committed target post-fsync fstat a eșuat: {error}"),
        )
    })?;
    if version_token_for_stat(&target_stat) != version_token_for_stat(&stable_target)
        || target_stat.st_nlink != stable_target.st_nlink
    {
        return Err("Copy committed target s-a schimbat după hash înainte de postflight.".into());
    }
    sync_directory(&directory, &record.body.public_label)?;
    verify_copy_public_committed(record, evidence, checkpoint, &stable_target)
}

fn verify_copy_public_baseline(
    record: &WalRecord,
    evidence: &WalCopyEvidence,
) -> Result<(), String> {
    let (directory, target_leaf, temp_leaf) = ready_copy_context(record, evidence)?;
    let role = copy_stage_role(evidence);
    let target = observe_copy_v2_leaf(&directory, &target_leaf, role, &record.body.public_label)?;
    let temp = observe_copy_v2_leaf(&directory, &temp_leaf, role, &record.body.public_label)?;
    if observed_v2_matches_before(target.as_ref(), evidence) && temp.is_none() {
        Ok(())
    } else {
        Err("Copy public baseline postflight nu mai este exact.".into())
    }
}

fn verify_copy_public_committed(
    record: &WalRecord,
    evidence: &WalCopyEvidence,
    checkpoint: &WalCopyStageCheckpoint,
    expected_stat: &fs::Stat,
) -> Result<(), String> {
    let (directory, target_leaf, temp_leaf) = ready_copy_context(record, evidence)?;
    let role = copy_stage_role(evidence);
    validate_named_file_identity(
        &directory,
        &target_leaf,
        expected_stat,
        "copy-recovery-public-target",
    )?;
    let target = observe_copy_v2_leaf(&directory, &target_leaf, role, &record.body.public_label)?;
    let temp = observe_copy_v2_leaf(&directory, &temp_leaf, role, &record.body.public_label)?;
    if observed_v2_matches_checkpoint(target.as_ref(), evidence, checkpoint) && temp.is_none() {
        let observed = target.expect("checkpoint match requires target");
        if observed.version_token == version_token_for_stat(expected_stat) {
            Ok(())
        } else {
            Err(
                "Copy public committed postflight a detectat o versiune schimbată după hash."
                    .into(),
            )
        }
    } else {
        Err("Copy public committed postflight nu mai este exact.".into())
    }
}

fn ready_copy_context(
    record: &WalRecord,
    evidence: &WalCopyEvidence,
) -> Result<(OwnedFd, OsString, OsString), String> {
    match capture_recovery_atomic_context(record, &evidence.file)? {
        RecoveryAtomicContext::Ready {
            directory,
            target_leaf,
            temp_leaf,
            parent_was_missing: false,
        } => Ok((directory, target_leaf, temp_leaf)),
        RecoveryAtomicContext::Ready {
            parent_was_missing: true,
            ..
        } => Err("Copy v2 recovery refuză parent creat de Copy.".into()),
        RecoveryAtomicContext::ParentMissing { .. } => {
            Err("Copy v2 recovery nu mai găsește parentul persistent.".into())
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn open_exact_checkpointed_copy_leaf(
    directory: &OwnedFd,
    leaf: &OsStr,
    evidence: &WalCopyEvidence,
    checkpoint: &WalCopyStageCheckpoint,
    role: WalCopyStageRole,
    public_label: &str,
    stage: &str,
    read_budget: &mut RecoveryReadBudget,
    verify_hash: bool,
) -> Result<(File, fs::Stat), String> {
    let descriptor = fs::openat(
        directory,
        leaf,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| capability_error(public_label, &format!("{stage}: open a eșuat: {error}")))?;
    let mut file = File::from(descriptor);
    let before = fs::fstat(&file).map_err(|error| {
        capability_error(public_label, &format!("{stage}: fstat a eșuat: {error}"))
    })?;
    let observed = observed_copy_v2_from_file(directory, leaf, &file, &before, role, public_label)?;
    if !observed_v2_matches_checkpoint(Some(&observed), evidence, checkpoint) {
        return Err(capability_error(
            public_label,
            &format!("{stage}: metadata/identity nu corespund checkpointului"),
        ));
    }
    if !verify_hash {
        validate_named_file_identity(directory, leaf, &before, stage)?;
        return Ok((file, before));
    }
    read_budget.reserve(evidence.file.new_size, stage)?;
    let hash = hash_open_file_exact(&mut file, evidence.file.new_size, stage)?;
    run_test_hook(CapabilityTestStage::AfterCopyRecoveryHash);
    let after = fs::fstat(&file).map_err(|error| {
        capability_error(
            public_label,
            &format!("{stage}: post-hash fstat a eșuat: {error}"),
        )
    })?;
    if !same_file_identity(&before, &after)
        || version_token_for_stat(&before) != version_token_for_stat(&after)
        || before.st_nlink != after.st_nlink
        || hash != evidence.file.new_content_hash
    {
        return Err(capability_error(
            public_label,
            &format!("{stage}: payloadul sau inode-ul s-a schimbat în timpul hash-ului"),
        ));
    }
    validate_named_file_identity(directory, leaf, &after, stage)?;
    Ok((file, after))
}

fn observe_copy_v2_leaf(
    directory: &OwnedFd,
    leaf: &OsStr,
    role: WalCopyStageRole,
    public_label: &str,
) -> Result<Option<ObservedCopyV2Leaf>, String> {
    let Some(named) = leaf_metadata(directory, leaf, public_label)? else {
        return Ok(None);
    };
    if FileType::from_raw_mode(named.st_mode) != FileType::RegularFile || named.st_nlink != 1 {
        return Err(capability_error(
            public_label,
            "Copy v2 recovery leaf nu este regular single-link",
        ));
    }
    let descriptor = fs::openat(
        directory,
        leaf,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| {
        capability_error(
            public_label,
            &format!("Copy v2 recovery leaf open a eșuat: {error}"),
        )
    })?;
    let file = File::from(descriptor);
    let opened = fs::fstat(&file).map_err(|error| {
        capability_error(
            public_label,
            &format!("Copy v2 recovery leaf fstat a eșuat: {error}"),
        )
    })?;
    if !same_file_identity(&named, &opened) {
        return Err(capability_error(
            public_label,
            "Copy v2 recovery leaf s-a schimbat în timpul open",
        ));
    }
    observed_copy_v2_from_file(directory, leaf, &file, &opened, role, public_label).map(Some)
}

fn observed_copy_v2_from_file(
    directory: &OwnedFd,
    leaf: &OsStr,
    file: &File,
    stat: &fs::Stat,
    role: WalCopyStageRole,
    public_label: &str,
) -> Result<ObservedCopyV2Leaf, String> {
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile || stat.st_nlink != 1 {
        return Err(capability_error(
            public_label,
            "Copy v2 recovery descriptor nu este regular single-link",
        ));
    }
    validate_named_file_identity(directory, leaf, stat, "copy-v2-recovery-leaf")?;
    let identity_digest = copy_stage_identity_digest(file, role)
        .map_err(|error| capability_error(public_label, &error))?;
    let after = fs::fstat(file).map_err(|error| {
        capability_error(
            public_label,
            &format!("Copy v2 recovery leaf re-fstat a eșuat: {error}"),
        )
    })?;
    if version_token_for_stat(stat) != version_token_for_stat(&after)
        || stat.st_nlink != after.st_nlink
    {
        return Err(capability_error(
            public_label,
            "Copy v2 recovery leaf s-a schimbat în timpul capturii",
        ));
    }
    validate_named_file_identity(directory, leaf, &after, "copy-v2-recovery-leaf")?;
    Ok(ObservedCopyV2Leaf {
        identity: WalFilesystemIdentity {
            device: after.st_dev,
            inode: after.st_ino,
        },
        identity_digest,
        size: u64::try_from(after.st_size)
            .map_err(|_| capability_error(public_label, "Copy v2 leaf are size negativ"))?,
        version_token: version_token_for_stat(&after),
        mode_bits: mode_bits(&after),
    })
}

fn observed_v2_matches_before(
    observed: Option<&ObservedCopyV2Leaf>,
    evidence: &WalCopyEvidence,
) -> bool {
    match (&evidence.file.before, observed) {
        (WalLeafEvidence::Absent, None) => true,
        (
            WalLeafEvidence::Regular {
                identity,
                size,
                version_token,
                ..
            },
            Some(observed),
        ) => {
            observed.identity == *identity
                && observed.size == *size
                && observed.version_token == *version_token
                && Some(observed.mode_bits) == evidence.before_mode_bits
        }
        _ => false,
    }
}

fn observed_v2_matches_checkpoint(
    observed: Option<&ObservedCopyV2Leaf>,
    evidence: &WalCopyEvidence,
    checkpoint: &WalCopyStageCheckpoint,
) -> bool {
    matches!(
        observed,
        Some(observed)
            if observed.identity_digest == checkpoint.staged_identity_digest
                && observed.size == evidence.file.new_size
                && observed.mode_bits == evidence.new_mode_bits
    )
}

fn copy_owner_contract_matches(record: &WalRecord, evidence: &WalCopyEvidence) -> bool {
    match evidence.destination_policy {
        WalCopyDestinationPolicy::CreateNew => {
            record.body.owner == "project_initializer"
                && matches!(
                    record.body.category.as_str(),
                    "project_source_write" | "project_design_write"
                )
                && record
                    .body
                    .authority
                    .scope
                    .starts_with("project_bootstrap:")
                && !evidence.file.replace
        }
        WalCopyDestinationPolicy::Replace => {
            record.body.owner == "preview"
                && record.body.category == "preview_workspace_write"
                && record.body.authority.scope == "application_preview_cache"
        }
    }
}

fn recovery_hash_diagnostic(available: bool, prefix: &str) -> String {
    if available {
        format!("{prefix}; executorul va confirma SHA-256 streaming înainte de finalizare.")
    } else {
        format!(
            "{prefix}, dar payloadul depășește bugetul recovery de {MAX_WAL_RECOVERY_READ_BYTES} bytes; review manual obligatoriu."
        )
    }
}

fn copy_conflict(diagnostic: &str) -> CopyRecoveryAssessment {
    CopyRecoveryAssessment {
        classification: WriteAuthorityRecoveryClassification::Conflict,
        automatic_action: None,
        diagnostic: diagnostic.into(),
    }
}

fn classify_legacy_copy_recovery(
    record: &WalRecord,
    evidence: &WalCopyEvidence,
    phase: WalPhase,
) -> Result<CopyRecoveryAssessment, String> {
    let context = capture_recovery_atomic_context(record, &evidence.file)?;
    let RecoveryAtomicContext::Ready {
        directory,
        target_leaf,
        temp_leaf,
        parent_was_missing,
    } = context
    else {
        return Ok(copy_conflict(
            "Copy legacy nu mai poate captura namespace-ul planificat; review manual obligatoriu.",
        ));
    };
    let target = observe_copy_leaf(&directory, &target_leaf, &record.body.public_label)?;
    let temp = observe_copy_leaf(&directory, &temp_leaf, &record.body.public_label)?;
    let target_before = observed_matches_copy_before(target.as_ref(), evidence);
    let temp_absent = temp.is_none();
    let target_new_shape = observed_matches_copy_new_shape(target.as_ref(), evidence);
    let temp_new_shape = observed_matches_copy_new_shape(temp.as_ref(), evidence);
    let temp_old =
        evidence.file.replace && observed_matches_relocated_copy_before(temp.as_ref(), evidence);

    if !parent_was_missing && phase == WalPhase::Prepared && target_before && temp_absent {
        return Ok(CopyRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::NoEffect,
            automatic_action: Some(CopyRecoveryAction::ClearNoEffect),
            diagnostic: "Copy legacy Prepared este exact baseline; clear no-effect este sigur."
                .into(),
        });
    }
    let (classification, diagnostic) = if target_before && temp_new_shape {
        (
            WriteAuthorityRecoveryClassification::StagedOnly,
            "Copy legacy are un temp cu forma nouă, dar fără identitate cauzală.",
        )
    } else if target_new_shape && temp_absent {
        (
            WriteAuthorityRecoveryClassification::EffectCommitted,
            "Copy legacy are un target cu forma nouă, dar fără identitate cauzală.",
        )
    } else if target_new_shape && temp_old {
        (
            WriteAuthorityRecoveryClassification::CleanupRequired,
            "Copy legacy pare exchange incomplet și nu permite cleanup automat.",
        )
    } else {
        (
            WriteAuthorityRecoveryClassification::Conflict,
            "Copy legacy are un oracle necunoscut.",
        )
    };
    Ok(CopyRecoveryAssessment {
        classification,
        automatic_action: None,
        diagnostic: format!("{diagnostic} Recordul protocol 0 rămâne pentru review manual."),
    })
}
