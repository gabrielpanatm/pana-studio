use super::*;

struct RecoveryDirectoryContext {
    parent: OwnedFd,
    target_leaf: OsString,
}

pub(in crate::kernel::write_authority::capability) fn classify_directory_recovery(
    record: &WalRecord,
    phase: WalPhase,
    checkpoint: Option<&WalDirectoryStageCheckpoint>,
) -> Result<DirectoryRecoveryAssessment, String> {
    let WalOperationEvidence::Directory(evidence) = &record.body.operation_evidence else {
        return Err("WriteAuthority Directory recovery a primit altă familie.".into());
    };
    if evidence.protocol_version == 0 {
        if checkpoint.is_some() {
            return Err("Directory legacy refuză checkpoint filename direct.".into());
        }
        return super::super::classify_legacy_directory_recovery(record, phase);
    }
    if evidence.protocol_version != WAL_DIRECTORY_PROTOCOL_VERSION {
        return Err(format!(
            "Directory recovery refuză protocolul necunoscut {}; recordul rămâne hot pentru review manual.",
            evidence.protocol_version
        ));
    }
    validate_phase_checkpoint(phase, checkpoint)?;
    let context = capture_recovery_parent(record, evidence)?;
    fs::flock(&context.parent, FlockOperation::LockExclusive)
        .map_err(|error| format!("Directory v2 recovery parent lock a eșuat: {error}."))?;
    classify_locked(record, evidence, phase, checkpoint, &context)
}

pub(in crate::kernel::write_authority::capability) fn execute_directory_recovery(
    record: &WalRecord,
    phase: WalPhase,
    checkpoint: Option<&WalDirectoryStageCheckpoint>,
    action: DirectoryRecoveryAction,
) -> Result<(), String> {
    let WalOperationEvidence::Directory(evidence) = &record.body.operation_evidence else {
        return Err("WriteAuthority Directory executor a primit altă familie.".into());
    };
    if evidence.protocol_version == 0 {
        if checkpoint.is_some() {
            return Err("Directory legacy executor refuză checkpoint direct.".into());
        }
        return super::super::execute_legacy_directory_recovery(record, phase, action);
    }
    if evidence.protocol_version != WAL_DIRECTORY_PROTOCOL_VERSION {
        return Err("Directory executor refuză protocolul necunoscut; zero mutații.".into());
    }
    validate_phase_checkpoint(phase, checkpoint)?;
    let context = capture_recovery_parent(record, evidence)?;
    fs::flock(&context.parent, FlockOperation::LockExclusive)
        .map_err(|error| format!("Directory v2 recovery parent lock a eșuat: {error}."))?;
    let assessment = classify_locked(record, evidence, phase, checkpoint, &context)?;
    if assessment.automatic_action != Some(action) {
        return Err(format!(
            "Directory v2 recovery CAS a refuzat {action:?}: {}",
            assessment.diagnostic
        ));
    }
    match action {
        DirectoryRecoveryAction::ClearNoEffect => Ok(()),
        DirectoryRecoveryAction::FinalizeCommitted => {
            let checkpoint =
                checkpoint.ok_or_else(|| "Directory v2 finalize cere checkpoint.".to_string())?;
            finalize_exact_target(record, evidence, checkpoint, &context)
        }
    }
}

pub(in crate::kernel::write_authority::capability) fn resolve_directory_operator(
    record: &WalRecord,
    phase: WalPhase,
    checkpoint: Option<&WalDirectoryStageCheckpoint>,
    action: WriteAuthorityRecoveryResolutionAction,
    expected_evidence_hash: &str,
    wal_evidence_binding_hash: &str,
) -> Result<String, String> {
    if !matches!(
        action,
        WriteAuthorityRecoveryResolutionAction::AcceptRestoredState
            | WriteAuthorityRecoveryResolutionAction::AcceptCurrentState
    ) {
        return Err(
            "Directory direct acceptă numai acțiunile operator AcceptRestoredState sau AcceptCurrentState."
                .into(),
        );
    }
    if phase != WalPhase::Prepared || checkpoint.is_some() {
        return Err(
            "Directory direct poate accepta o stare operator numai din Prepared fără checkpoint."
                .into(),
        );
    }
    let WalOperationEvidence::Directory(evidence) = &record.body.operation_evidence else {
        return Err("Directory direct operator a primit altă familie.".into());
    };
    if evidence.protocol_version != WAL_DIRECTORY_PROTOCOL_VERSION {
        return Err(format!(
            "Directory direct operator refuză protocolul {}; sunt acceptate exclusiv recordurile protocol {}.",
            evidence.protocol_version, WAL_DIRECTORY_PROTOCOL_VERSION
        ));
    }
    if evidence.existing_target_identity.is_some()
        || evidence.existing_target_identity_digest.is_some()
        || evidence.existing_target_version_token.is_some()
        || evidence.existing_prefix_len == evidence.relative_components_hex.len()
    {
        return Err(
            "Directory direct operator refuză un record cu baseline target existent.".into(),
        );
    }

    // Recapture from the sealed authority evidence, then keep the exact parent
    // cooperatively locked through the final absence postflight. După return,
    // `RecoveryCoordinator::resolve_operator_exclusive` păstrează lock-ul WAL
    // global până după remove+fsync și blochează astfel orice alt writer
    // WriteAuthority. Parent flock este advisory și nu pretinde izolare față
    // de un proces same-UID care ocolește WriteAuthority.
    let context = capture_recovery_parent(record, evidence)?;
    fs::flock(&context.parent, FlockOperation::LockExclusive)
        .map_err(|error| format!("Directory direct operator parent lock a eșuat: {error}."))?;
    match action {
        WriteAuthorityRecoveryResolutionAction::AcceptRestoredState => {
            if expected_evidence_hash != wal_evidence_binding_hash {
                return Err(
                    "Directory direct AcceptRestoredState a primit evidence hash stale.".into(),
                );
            }
            resolve_absent_state(record, evidence, phase, checkpoint, &context)
        }
        WriteAuthorityRecoveryResolutionAction::AcceptCurrentState => resolve_current_state(
            record,
            evidence,
            &context,
            expected_evidence_hash,
            wal_evidence_binding_hash,
        ),
        _ => unreachable!("action filtered above"),
    }
}

fn resolve_absent_state(
    record: &WalRecord,
    evidence: &WalDirectoryEvidence,
    phase: WalPhase,
    checkpoint: Option<&WalDirectoryStageCheckpoint>,
    context: &RecoveryDirectoryContext,
) -> Result<String, String> {
    let assessment = classify_locked(record, evidence, phase, checkpoint, context)?;
    if assessment.classification != WriteAuthorityRecoveryClassification::RollbackCompleted
        || assessment.automatic_action.is_some()
        || !assessment
            .available_resolution_actions
            .contains(&WriteAuthorityRecoveryResolutionAction::AcceptRestoredState)
    {
        return Err(format!(
            "Directory direct operator CAS nu mai poate accepta starea absentă: {}",
            assessment.diagnostic
        ));
    }

    let recaptured = capture_recovery_parent(record, evidence)?;
    let observed = observe_directory_leaf(
        &recaptured.parent,
        &recaptured.target_leaf,
        &record.body.public_label,
        "Directory direct operator target",
    )?;
    if !matches!(observed, ObservedDirectoryLeaf::Absent) {
        return Err(
            "Directory direct operator a observat targetul reapărut; WAL-ul rămâne hot.".into(),
        );
    }
    sync_directory(&recaptured.parent, &record.body.public_label)?;
    let postflight = observe_directory_leaf(
        &recaptured.parent,
        &recaptured.target_leaf,
        &record.body.public_label,
        "Directory direct operator target postflight",
    )?;
    if !matches!(postflight, ObservedDirectoryLeaf::Absent) {
        return Err(
            "Directory direct operator postflight a observat targetul reapărut; WAL-ul rămâne hot."
                .into(),
        );
    }

    Ok(
        "Operatorul a acceptat explicit starea absentă Directory direct după recapturarea authority/parent, parent flock și postflight absent; niciun target nu a fost creat, adoptat sau șters."
            .into(),
    )
}

fn resolve_current_state(
    record: &WalRecord,
    evidence: &WalDirectoryEvidence,
    context: &RecoveryDirectoryContext,
    expected_evidence_hash: &str,
    wal_evidence_binding_hash: &str,
) -> Result<String, String> {
    let observed = observe_directory_leaf(
        &context.parent,
        &context.target_leaf,
        &record.body.public_label,
        "Directory direct operator current target",
    )?;
    let scanned_binding = current_state_binding(&observed, evidence).ok_or_else(|| {
        "Directory direct AcceptCurrentState cere un director real, stabil, gol și mode 0755; targetul rămâne neatins și WAL-ul hot."
            .to_string()
    })?;
    if scanned_binding.evidence_hash(wal_evidence_binding_hash) != expected_evidence_hash {
        return Err(
            "Directory direct AcceptCurrentState a primit evidence hash stale: lifetime/state diferă de scanarea operatorului."
                .into(),
        );
    }

    run_test_hook(CapabilityTestStage::BeforeDirectoryCurrentStateFreshCapture);

    // O captură nouă după CAS-ul tokenului împiedică acceptarea unui alt
    // director gol/0755 substituit între scan și execuție. Parent flock-ul
    // serializează writerii cooperanți; procesele same-UID necooperante rămân
    // limita advisory explicită a protocolului.
    let fresh_context = capture_recovery_parent(record, evidence)?;
    let fresh = observe_directory_leaf(
        &fresh_context.parent,
        &fresh_context.target_leaf,
        &record.body.public_label,
        "Directory direct operator fresh current target",
    )?;
    let fresh_binding = current_state_binding(&fresh, evidence).ok_or_else(|| {
        "Directory direct AcceptCurrentState fresh capture nu mai vede directorul gol/mode 0755; WAL-ul rămâne hot."
            .to_string()
    })?;
    if fresh_binding != scanned_binding {
        return Err(
            "Directory direct AcceptCurrentState fresh lifetime/state diferă de token; WAL-ul rămâne hot."
                .into(),
        );
    }
    let ObservedDirectoryLeaf::Directory(fresh) = fresh else {
        unreachable!("current_state_binding accepts only directories")
    };
    fs::fsync(&fresh.descriptor).map_err(|error| {
        format!("Directory direct AcceptCurrentState fsync target a eșuat: {error}.")
    })?;
    sync_directory(&fresh_context.parent, &record.body.public_label)?;

    let full_path = capture_recovery_parent(record, evidence)?;
    let postflight = observe_directory_leaf(
        &full_path.parent,
        &full_path.target_leaf,
        &record.body.public_label,
        "Directory direct operator current target full-path postflight",
    )?;
    let postflight_binding = current_state_binding(&postflight, evidence).ok_or_else(|| {
        "Directory direct AcceptCurrentState full-path postflight nu mai vede directorul gol/mode 0755; WAL-ul rămâne hot."
            .to_string()
    })?;
    if postflight_binding != scanned_binding
        || postflight_binding.evidence_hash(wal_evidence_binding_hash) != expected_evidence_hash
    {
        return Err(
            "Directory direct AcceptCurrentState full-path lifetime/state s-a schimbat; WAL-ul rămâne hot."
                .into(),
        );
    }

    Ok(
        "Operatorul a acceptat explicit directorul curent Directory direct, legat de lifetime+state-ul scanat, după recaptură fresh, fsync target+parent și full-path exact; targetul nu a fost creat, șters, redenumit sau chmod-at de rezoluție."
            .into(),
    )
}

fn classify_locked(
    record: &WalRecord,
    evidence: &WalDirectoryEvidence,
    phase: WalPhase,
    checkpoint: Option<&WalDirectoryStageCheckpoint>,
    context: &RecoveryDirectoryContext,
) -> Result<DirectoryRecoveryAssessment, String> {
    let target = observe_directory_leaf(
        &context.parent,
        &context.target_leaf,
        &record.body.public_label,
        "Directory v2 recovery target",
    )?;

    if evidence.existing_target_identity.is_some() {
        if phase == WalPhase::Prepared
            && checkpoint.is_none()
            && matches!(
                &target,
                ObservedDirectoryLeaf::Directory(observed)
                    if matches_existing_baseline(observed, evidence)
            )
        {
            return Ok(DirectoryRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::NoEffect,
                automatic_action: Some(DirectoryRecoveryAction::ClearNoEffect),
                available_resolution_actions: Vec::new(),
                resolution_state_binding: None,
                diagnostic:
                    "Directory v2 targetul preexistent păstrează baseline-ul exact; operația era no-op."
                        .into(),
            });
        }
        return Ok(conflict(
            "Directory v2 no-op preexistent nu mai păstrează baseline-ul exact sau are o fază imposibilă.",
        ));
    }

    if phase == WalPhase::Prepared {
        // mkdirat publică direct leaf-ul înainte de primul checkpoint. Chiar
        // dacă targetul este absent la restart, un create urmat de dispariție
        // nu poate fi exclus; recordul nu este șters automat.
        return Ok(match target {
            ObservedDirectoryLeaf::Absent => DirectoryRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::RollbackCompleted,
                automatic_action: None,
                available_resolution_actions: vec![
                    WriteAuthorityRecoveryResolutionAction::AcceptRestoredState,
                ],
                resolution_state_binding: None,
                diagnostic:
                    "Directory direct Prepared vede din nou baseline-ul absent. Un create urmat de dispariție nu poate fi exclus, deci numai operatorul poate accepta explicit starea restaurată."
                        .into(),
            },
            target if current_state_binding(&target, evidence).is_some() => {
                let binding = current_state_binding(&target, evidence)
                    .expect("guarded by current_state_binding check");
                DirectoryRecoveryAssessment {
                    classification:
                        WriteAuthorityRecoveryClassification::PartialNamespaceCreation,
                    automatic_action: None,
                    available_resolution_actions: vec![
                        WriteAuthorityRecoveryResolutionAction::AcceptCurrentState,
                    ],
                    resolution_state_binding: Some(binding),
                    diagnostic:
                        "Directory direct Prepared vede un director real, stabil, gol și mode 0755 fără checkpoint cauzal. Nu este adoptat automat; operatorul poate accepta exclusiv lifetime+state-ul legat în tokenul scanării."
                            .into(),
                }
            }
            _ => conflict(
                "Directory direct Prepared vede target fără checkpoint, dar acesta nu este un director real, stabil, gol și mode 0755; zero adoption și zero cleanup.",
            ),
        });
    }

    let checkpoint = checkpoint.expect("post-Prepared requires checkpoint");
    if is_checkpointed_empty_directory(&target, evidence, checkpoint) {
        return Ok(DirectoryRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::EffectCommitted,
            automatic_action: Some(DirectoryRecoveryAction::FinalizeCommitted),
            available_resolution_actions: Vec::new(),
            resolution_state_binding: None,
            diagnostic:
                "Directory v2 targetul direct gol este exact lifetime/state-ul checkpointed; recovery poate finaliza durabilitatea."
                    .into(),
        });
    }
    Ok(conflict(
        "Directory v2 targetul lipsește, a fost înlocuit, nu mai este gol sau diferă de checkpoint; namespace-ul rămâne neatins.",
    ))
}

fn finalize_exact_target(
    record: &WalRecord,
    evidence: &WalDirectoryEvidence,
    checkpoint: &WalDirectoryStageCheckpoint,
    context: &RecoveryDirectoryContext,
) -> Result<(), String> {
    let target = observe_directory_leaf(
        &context.parent,
        &context.target_leaf,
        &record.body.public_label,
        "Directory v2 recovery finalize target",
    )?;
    if !is_checkpointed_empty_directory(&target, evidence, checkpoint) {
        return Err("Directory v2 recovery targetul nu mai este exact și gol.".into());
    }
    let ObservedDirectoryLeaf::Directory(target) = target else {
        unreachable!("exact helper accepted only directory")
    };
    fs::fsync(&target.descriptor)
        .map_err(|error| format!("Directory v2 recovery fsync target a eșuat: {error}."))?;
    sync_directory(&context.parent, &record.body.public_label)?;

    // Full-path postflight: recapture authority and parent, then require the
    // same empty checkpointed lifetime/state at the public target name.
    let recaptured = capture_recovery_parent(record, evidence)?;
    let observed = observe_directory_leaf(
        &recaptured.parent,
        &recaptured.target_leaf,
        &record.body.public_label,
        "Directory v2 recovery full-path target",
    )?;
    if !is_checkpointed_empty_directory(&observed, evidence, checkpoint) {
        return Err("Directory v2 recovery full-path CAS a eșuat.".into());
    }
    Ok(())
}

fn is_checkpointed_empty_directory(
    observed: &ObservedDirectoryLeaf,
    evidence: &WalDirectoryEvidence,
    checkpoint: &WalDirectoryStageCheckpoint,
) -> bool {
    matches!(
        observed,
        ObservedDirectoryLeaf::Directory(observed)
            if observed.identity_digest == checkpoint.target_identity_digest
                && observed.state_digest == checkpoint.target_state_digest
                && observed.empty
                && mode_bits(&observed.stat) == evidence.desired_mode_bits.unwrap_or(u32::MAX)
    )
}

fn current_state_binding(
    observed: &ObservedDirectoryLeaf,
    evidence: &WalDirectoryEvidence,
) -> Option<DirectoryResolutionStateBinding> {
    match observed {
        ObservedDirectoryLeaf::Directory(observed)
            if evidence.desired_mode_bits == Some(DIRECTORY_V2_MODE_BITS)
                && observed.empty
                && mode_bits(&observed.stat) == DIRECTORY_V2_MODE_BITS =>
        {
            Some(DirectoryResolutionStateBinding {
                identity_digest: observed.identity_digest.clone(),
                state_digest: observed.state_digest.clone(),
            })
        }
        _ => None,
    }
}

fn capture_recovery_parent(
    record: &WalRecord,
    evidence: &WalDirectoryEvidence,
) -> Result<RecoveryDirectoryContext, String> {
    let (authority, components) =
        super::super::capture_recovery_directory_authority(record, evidence)?;
    let (target_leaf, parents) = components
        .split_last()
        .ok_or_else(|| "Directory v2 recovery cere un leaf.".to_string())?;
    let mut parent = rustix::io::dup(authority.directory())
        .map_err(|error| format!("Directory v2 recovery nu poate duplica authority: {error}."))?;
    for component in parents {
        let next = open_directory_strict(&parent, component)
            .map_err(|error| format!("Directory v2 recovery parent component invalid: {error}."))?;
        validate_named_directory_identity(
            &parent,
            component,
            &next,
            &record.body.public_label,
            "Directory v2 recovery parent",
        )?;
        parent = next;
    }
    if wal_identity_from_fd(&parent, &record.body.public_label)?
        != *evidence
            .parent_identity
            .as_ref()
            .ok_or_else(|| "Directory v2 recovery parent identity lipsește.".to_string())?
    {
        return Err("Directory v2 recovery parent identity diferă de record.".into());
    }
    let encoded_target = evidence
        .target_leaf_hex
        .as_deref()
        .ok_or_else(|| "Directory v2 recovery target leaf lipsește.".to_string())?;
    if decode_component_hex(encoded_target)? != *target_leaf {
        return Err("Directory v2 recovery target leaf diferă de path.".into());
    }
    Ok(RecoveryDirectoryContext {
        parent,
        target_leaf: target_leaf.clone(),
    })
}

fn validate_phase_checkpoint(
    phase: WalPhase,
    checkpoint: Option<&WalDirectoryStageCheckpoint>,
) -> Result<(), String> {
    match phase {
        WalPhase::Preparing => Err("Directory v2 classifier nu execută Preparing.".into()),
        WalPhase::Prepared if checkpoint.is_some() => {
            Err("Directory v2 Prepared refuză checkpoint prematur.".into())
        }
        WalPhase::AuxiliaryDurable | WalPhase::EffectVisible | WalPhase::TargetDurable
            if checkpoint.is_none() =>
        {
            Err(format!(
                "Directory v2 {phase:?} cere checkpoint filename cauzal."
            ))
        }
        _ => Ok(()),
    }
}

fn conflict(diagnostic: impl Into<String>) -> DirectoryRecoveryAssessment {
    DirectoryRecoveryAssessment {
        classification: WriteAuthorityRecoveryClassification::Conflict,
        automatic_action: None,
        available_resolution_actions: Vec::new(),
        resolution_state_binding: None,
        diagnostic: diagnostic.into(),
    }
}
