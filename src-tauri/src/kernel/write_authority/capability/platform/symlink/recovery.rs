use super::*;

struct RecoverySymlinkContext {
    parent: OwnedFd,
    target_leaf: OsString,
}

pub(in crate::kernel::write_authority::capability) fn classify_symlink_recovery(
    record: &WalRecord,
    phase: WalPhase,
    checkpoint: Option<&WalSymlinkStageCheckpoint>,
) -> Result<SymlinkRecoveryAssessment, String> {
    let WalOperationEvidence::Symlink(evidence) = &record.body.operation_evidence else {
        return Err("WriteAuthority Symlink recovery a primit altă familie.".into());
    };
    if evidence.protocol_version == 0 {
        if checkpoint.is_some() {
            return Err("Symlink legacy refuză checkpoint filename direct.".into());
        }
        return super::super::lifecycle::classify_legacy_symlink_recovery(record, phase);
    }
    if evidence.protocol_version != WAL_SYMLINK_PROTOCOL_VERSION {
        return Err(format!(
            "Symlink recovery refuză protocolul necunoscut {}; recordul rămâne hot.",
            evidence.protocol_version
        ));
    }
    validate_phase_checkpoint(phase, checkpoint)?;
    let context = capture_recovery_parent(record, evidence)?;
    fs::flock(&context.parent, FlockOperation::LockExclusive)
        .map_err(|error| format!("Symlink v2 recovery parent lock a eșuat: {error}."))?;
    classify_locked(record, evidence, phase, checkpoint, &context)
}

pub(in crate::kernel::write_authority::capability) fn execute_symlink_recovery(
    record: &WalRecord,
    phase: WalPhase,
    checkpoint: Option<&WalSymlinkStageCheckpoint>,
    action: SymlinkRecoveryAction,
) -> Result<(), String> {
    let WalOperationEvidence::Symlink(evidence) = &record.body.operation_evidence else {
        return Err("WriteAuthority Symlink executor a primit altă familie.".into());
    };
    if evidence.protocol_version == 0 {
        if checkpoint.is_some() {
            return Err("Symlink legacy executor refuză checkpoint direct.".into());
        }
        return super::super::lifecycle::execute_legacy_symlink_recovery(record, phase, action);
    }
    if evidence.protocol_version != WAL_SYMLINK_PROTOCOL_VERSION {
        return Err("Symlink executor refuză protocolul necunoscut; zero mutații.".into());
    }
    validate_phase_checkpoint(phase, checkpoint)?;
    let context = capture_recovery_parent(record, evidence)?;
    fs::flock(&context.parent, FlockOperation::LockExclusive)
        .map_err(|error| format!("Symlink v2 recovery parent lock a eșuat: {error}."))?;
    let assessment = classify_locked(record, evidence, phase, checkpoint, &context)?;
    if assessment.automatic_action != Some(action) {
        return Err(format!(
            "Symlink v2 recovery CAS a refuzat {action:?}: {}",
            assessment.diagnostic
        ));
    }
    match action {
        SymlinkRecoveryAction::ClearNoEffect => Ok(()),
        SymlinkRecoveryAction::FinalizeCommitted => {
            let checkpoint =
                checkpoint.ok_or_else(|| "Symlink v2 finalize cere checkpoint.".to_string())?;
            finalize_exact_target(record, evidence, checkpoint, &context)
        }
    }
}

pub(in crate::kernel::write_authority::capability) fn resolve_symlink_operator(
    record: &WalRecord,
    phase: WalPhase,
    checkpoint: Option<&WalSymlinkStageCheckpoint>,
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
            "Symlink direct acceptă numai AcceptRestoredState sau AcceptCurrentState.".into(),
        );
    }
    if phase != WalPhase::Prepared || checkpoint.is_some() {
        return Err(
            "Symlink direct poate accepta o stare operator numai din Prepared fără checkpoint."
                .into(),
        );
    }
    let WalOperationEvidence::Symlink(evidence) = &record.body.operation_evidence else {
        return Err("Symlink direct operator a primit altă familie.".into());
    };
    if evidence.protocol_version != WAL_SYMLINK_PROTOCOL_VERSION {
        return Err(format!(
            "Symlink direct operator refuză protocolul {}.",
            evidence.protocol_version
        ));
    }
    if !matches!(evidence.before, WalSymlinkBefore::Absent) {
        return Err("Symlink direct operator refuză un baseline target existent.".into());
    }

    let context = capture_recovery_parent(record, evidence)?;
    fs::flock(&context.parent, FlockOperation::LockExclusive)
        .map_err(|error| format!("Symlink direct operator parent lock a eșuat: {error}."))?;
    match action {
        WriteAuthorityRecoveryResolutionAction::AcceptRestoredState => {
            if expected_evidence_hash != wal_evidence_binding_hash {
                return Err("Symlink AcceptRestoredState a primit evidence hash stale.".into());
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
    evidence: &WalSymlinkEvidence,
    phase: WalPhase,
    checkpoint: Option<&WalSymlinkStageCheckpoint>,
    context: &RecoverySymlinkContext,
) -> Result<String, String> {
    let assessment = classify_locked(record, evidence, phase, checkpoint, context)?;
    if assessment.classification != WriteAuthorityRecoveryClassification::RollbackCompleted
        || assessment.automatic_action.is_some()
        || !assessment
            .available_resolution_actions
            .contains(&WriteAuthorityRecoveryResolutionAction::AcceptRestoredState)
    {
        return Err(format!(
            "Symlink direct operator CAS nu mai poate accepta starea absentă: {}",
            assessment.diagnostic
        ));
    }
    let recaptured = capture_recovery_parent(record, evidence)?;
    if !matches!(
        observe_symlink_leaf(
            &recaptured.parent,
            &recaptured.target_leaf,
            &record.body.public_label,
            "Symlink direct operator absent target",
        )?,
        ObservedSymlinkLeaf::Absent
    ) {
        return Err("Symlink direct operator a observat targetul reapărut.".into());
    }
    sync_directory(&recaptured.parent, &record.body.public_label)?;
    let postflight = capture_recovery_parent(record, evidence)?;
    if !matches!(
        observe_symlink_leaf(
            &postflight.parent,
            &postflight.target_leaf,
            &record.body.public_label,
            "Symlink direct operator absent postflight",
        )?,
        ObservedSymlinkLeaf::Absent
    ) {
        return Err("Symlink direct operator postflight a observat targetul reapărut.".into());
    }
    Ok(
        "Operatorul a acceptat explicit starea absentă Symlink direct după recapturarea authority/parent, parent flock, fsync parent și postflight absent; niciun target nu a fost creat, adoptat sau șters."
            .into(),
    )
}

fn resolve_current_state(
    record: &WalRecord,
    evidence: &WalSymlinkEvidence,
    context: &RecoverySymlinkContext,
    expected_evidence_hash: &str,
    wal_evidence_binding_hash: &str,
) -> Result<String, String> {
    let observed = observe_symlink_leaf(
        &context.parent,
        &context.target_leaf,
        &record.body.public_label,
        "Symlink direct operator current target",
    )?;
    let scanned_binding = current_state_binding(&observed, evidence).ok_or_else(|| {
        "Symlink AcceptCurrentState cere un symlink stabil cu literalul exact; targetul rămâne neatins și WAL-ul hot."
            .to_string()
    })?;
    if scanned_binding.evidence_hash(wal_evidence_binding_hash) != expected_evidence_hash {
        return Err(
            "Symlink AcceptCurrentState a primit evidence hash stale: lifetime/state diferă de scanare."
                .into(),
        );
    }
    run_test_hook(CapabilityTestStage::BeforeSymlinkCurrentStateFreshCapture);
    let fresh_context = capture_recovery_parent(record, evidence)?;
    let fresh = observe_symlink_leaf(
        &fresh_context.parent,
        &fresh_context.target_leaf,
        &record.body.public_label,
        "Symlink direct operator fresh current target",
    )?;
    let fresh_binding = current_state_binding(&fresh, evidence).ok_or_else(|| {
        "Symlink AcceptCurrentState fresh capture nu mai vede literalul exact.".to_string()
    })?;
    if fresh_binding != scanned_binding {
        return Err("Symlink AcceptCurrentState fresh lifetime/state diferă de token.".into());
    }
    sync_directory(&fresh_context.parent, &record.body.public_label)?;
    let postflight_context = capture_recovery_parent(record, evidence)?;
    let postflight = observe_symlink_leaf(
        &postflight_context.parent,
        &postflight_context.target_leaf,
        &record.body.public_label,
        "Symlink direct operator full-path postflight",
    )?;
    let postflight_binding = current_state_binding(&postflight, evidence).ok_or_else(|| {
        "Symlink AcceptCurrentState full-path postflight nu mai vede literalul exact.".to_string()
    })?;
    if postflight_binding != scanned_binding
        || postflight_binding.evidence_hash(wal_evidence_binding_hash) != expected_evidence_hash
    {
        return Err("Symlink AcceptCurrentState full-path lifetime/state s-a schimbat.".into());
    }
    Ok(
        "Operatorul a acceptat explicit symlink-ul curent, legat de lifetime+state+literalul scanat, după recaptură fresh, fsync parent și full-path exact; rezoluția nu a creat, șters sau redenumit targetul."
            .into(),
    )
}

fn classify_locked(
    record: &WalRecord,
    evidence: &WalSymlinkEvidence,
    phase: WalPhase,
    checkpoint: Option<&WalSymlinkStageCheckpoint>,
    context: &RecoverySymlinkContext,
) -> Result<SymlinkRecoveryAssessment, String> {
    let target = observe_symlink_leaf(
        &context.parent,
        &context.target_leaf,
        &record.body.public_label,
        "Symlink v2 recovery target",
    )?;
    if matches!(evidence.before, WalSymlinkBefore::Exact { .. }) {
        if phase == WalPhase::Prepared
            && checkpoint.is_none()
            && matches!(
                &target,
                ObservedSymlinkLeaf::Symlink(observed)
                    if matches_existing_baseline(observed, evidence)
            )
        {
            return Ok(SymlinkRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::NoEffect,
                automatic_action: Some(SymlinkRecoveryAction::ClearNoEffect),
                available_resolution_actions: Vec::new(),
                resolution_state_binding: None,
                diagnostic:
                    "Symlink v2 preexistent păstrează baseline-ul lifetime/state/literal exact; operația era no-op."
                        .into(),
            });
        }
        return Ok(conflict(
            "Symlink v2 no-op preexistent diferă de baseline sau are o fază imposibilă.",
        ));
    }

    if phase == WalPhase::Prepared {
        return Ok(match target {
            ObservedSymlinkLeaf::Absent => SymlinkRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::RollbackCompleted,
                automatic_action: None,
                available_resolution_actions: vec![
                    WriteAuthorityRecoveryResolutionAction::AcceptRestoredState,
                ],
                resolution_state_binding: None,
                diagnostic:
                    "Symlink direct Prepared vede baseline-ul absent. Un create urmat de dispariție nu poate fi exclus; numai operatorul poate accepta starea restaurată."
                        .into(),
            },
            target if current_state_binding(&target, evidence).is_some() => {
                let binding = current_state_binding(&target, evidence)
                    .expect("guarded by current_state_binding check");
                SymlinkRecoveryAssessment {
                    classification:
                        WriteAuthorityRecoveryClassification::PartialNamespaceCreation,
                    automatic_action: None,
                    available_resolution_actions: vec![
                        WriteAuthorityRecoveryResolutionAction::AcceptCurrentState,
                    ],
                    resolution_state_binding: Some(binding),
                    diagnostic:
                        "Symlink direct Prepared vede literalul exact fără checkpoint cauzal. Nu este adoptat automat; operatorul poate accepta exclusiv lifetime+state-ul legat în token."
                            .into(),
                }
            }
            _ => conflict(
                "Symlink direct Prepared vede target fără checkpoint cu tip/literal diferit; zero adoption și zero cleanup.",
            ),
        });
    }

    let checkpoint = checkpoint.expect("post-Prepared requires checkpoint");
    if is_checkpointed_symlink(&target, evidence, checkpoint) {
        return Ok(SymlinkRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::EffectCommitted,
            automatic_action: Some(SymlinkRecoveryAction::FinalizeCommitted),
            available_resolution_actions: Vec::new(),
            resolution_state_binding: None,
            diagnostic:
                "Symlink v2 este exact lifetime/state/literalul checkpointed; recovery poate finaliza durabilitatea parentului."
                    .into(),
        });
    }
    Ok(conflict(
        "Symlink v2 lipsește, a fost înlocuit ori diferă de checkpoint; namespace-ul rămâne neatins.",
    ))
}

fn finalize_exact_target(
    record: &WalRecord,
    evidence: &WalSymlinkEvidence,
    checkpoint: &WalSymlinkStageCheckpoint,
    context: &RecoverySymlinkContext,
) -> Result<(), String> {
    let target = observe_symlink_leaf(
        &context.parent,
        &context.target_leaf,
        &record.body.public_label,
        "Symlink v2 recovery finalize target",
    )?;
    if !is_checkpointed_symlink(&target, evidence, checkpoint) {
        return Err("Symlink v2 recovery targetul nu mai este exact.".into());
    }
    sync_directory(&context.parent, &record.body.public_label)?;
    let recaptured = capture_recovery_parent(record, evidence)?;
    let observed = observe_symlink_leaf(
        &recaptured.parent,
        &recaptured.target_leaf,
        &record.body.public_label,
        "Symlink v2 recovery full-path target",
    )?;
    if !is_checkpointed_symlink(&observed, evidence, checkpoint) {
        return Err("Symlink v2 recovery full-path CAS a eșuat.".into());
    }
    Ok(())
}

fn is_checkpointed_symlink(
    observed: &ObservedSymlinkLeaf,
    evidence: &WalSymlinkEvidence,
    checkpoint: &WalSymlinkStageCheckpoint,
) -> bool {
    matches!(
        observed,
        ObservedSymlinkLeaf::Symlink(observed)
            if observed.identity_digest == checkpoint.target_identity_digest
                && observed.state_digest == checkpoint.target_state_digest
                && observed.link_target_hex == evidence.desired_link_target_hex
    )
}

fn current_state_binding(
    observed: &ObservedSymlinkLeaf,
    evidence: &WalSymlinkEvidence,
) -> Option<SymlinkResolutionStateBinding> {
    match observed {
        ObservedSymlinkLeaf::Symlink(observed)
            if observed.link_target_hex == evidence.desired_link_target_hex =>
        {
            Some(SymlinkResolutionStateBinding {
                identity_digest: observed.identity_digest.clone(),
                state_digest: observed.state_digest.clone(),
            })
        }
        _ => None,
    }
}

fn capture_recovery_parent(
    record: &WalRecord,
    evidence: &WalSymlinkEvidence,
) -> Result<RecoverySymlinkContext, String> {
    let boundary_path = decode_path_hex(&record.body.authority.boundary_path_hex)?;
    if !boundary_path.is_absolute() {
        return Err("Symlink v2 recovery refuză boundary non-absolut.".into());
    }
    let authority = capture_directory_authority(
        &boundary_path,
        "write-authority-wal/symlink-v2-recovery-target",
        DirectoryAuthorityScope::RecoveryTarget,
    )?;
    let identity = authority.identity();
    if identity.device != record.body.authority.identity.device
        || identity.inode != record.body.authority.identity.inode
    {
        return Err("Symlink v2 recovery boundary identity diferă.".into());
    }
    let parents = evidence
        .parent
        .relative_components_hex
        .iter()
        .map(|component| decode_component_hex(component))
        .collect::<Result<Vec<_>, _>>()?;
    if evidence.parent.existing_prefix_len != parents.len() {
        return Err("Symlink v2 recovery refuză parent incomplet în evidence.".into());
    }
    let mut parent = rustix::io::dup(authority.directory())
        .map_err(|error| format!("Symlink v2 recovery nu poate duplica authority: {error}."))?;
    for component in &parents {
        let next = open_directory_strict(&parent, component)
            .map_err(|error| format!("Symlink v2 recovery parent invalid: {error}."))?;
        validate_named_directory_identity(
            &parent,
            component,
            &next,
            &record.body.public_label,
            "Symlink v2 recovery parent",
        )?;
        parent = next;
    }
    if wal_identity_from_fd(&parent, &record.body.public_label)?
        != *evidence
            .parent
            .parent_identity
            .as_ref()
            .ok_or_else(|| "Symlink v2 recovery parent identity lipsește.".to_string())?
    {
        return Err("Symlink v2 recovery parent identity diferă de record.".into());
    }
    Ok(RecoverySymlinkContext {
        parent,
        target_leaf: decode_component_hex(&evidence.target_leaf_hex)?,
    })
}

fn validate_phase_checkpoint(
    phase: WalPhase,
    checkpoint: Option<&WalSymlinkStageCheckpoint>,
) -> Result<(), String> {
    match phase {
        WalPhase::Preparing => Err("Symlink v2 classifier nu execută Preparing.".into()),
        WalPhase::Prepared if checkpoint.is_some() => {
            Err("Symlink v2 Prepared refuză checkpoint prematur.".into())
        }
        WalPhase::AuxiliaryDurable | WalPhase::EffectVisible | WalPhase::TargetDurable
            if checkpoint.is_none() =>
        {
            Err(format!(
                "Symlink v2 {phase:?} cere checkpoint filename cauzal."
            ))
        }
        _ => Ok(()),
    }
}

fn conflict(diagnostic: impl Into<String>) -> SymlinkRecoveryAssessment {
    SymlinkRecoveryAssessment {
        classification: WriteAuthorityRecoveryClassification::Conflict,
        automatic_action: None,
        available_resolution_actions: Vec::new(),
        resolution_state_binding: None,
        diagnostic: diagnostic.into(),
    }
}
