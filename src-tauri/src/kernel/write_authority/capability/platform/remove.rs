use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
struct ObservedRemoveLeaf {
    identity: WalFilesystemIdentity,
    kind: WalRemoveLeafKind,
    size: u64,
    mtime_seconds: i64,
    mtime_nanoseconds: u64,
    ctime_seconds: i64,
    ctime_nanoseconds: u64,
    raw_mode: u32,
    link_count: u64,
    owner_uid: u32,
    owner_gid: u32,
    raw_device: u64,
    version_token: String,
    symlink_target_hex: Option<String>,
}

#[derive(Clone, Copy)]
enum RemovePublicState {
    Before,
    Quarantined,
    Removed,
}

pub(in crate::kernel::write_authority::capability) fn plan_remove_leaf(
    target: &WriteTarget,
    operation_id: &str,
) -> Result<Option<RemoveLeafOperationPlan>, String> {
    let lexical = lexical_target(target, false)?;
    if lexical.authority.is_none() {
        return Err(capability_error(
            &lexical.public_label,
            "planul RemoveFile WAL cere authority sigilată",
        ));
    }
    let Some(parent) = capture_existing_target_parent(&lexical)? else {
        return absent_plan_result(target, &lexical.public_label);
    };
    let Some(source_before) =
        leaf_metadata(&parent.directory, &parent.leaf, &lexical.public_label)?
    else {
        return absent_plan_result(target, &lexical.public_label);
    };
    let source_kind = remove_leaf_kind(&source_before, &lexical.public_label)?;
    if target.expected_leaf == ExpectedLeaf::Absent {
        return Err(capability_error(
            &lexical.public_label,
            "RemoveFile a primit expected leaf Absent pentru un target existent",
        ));
    }

    let source_handle = fs::openat(
        &parent.directory,
        &parent.leaf,
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| {
        capability_error(
            &lexical.public_label,
            &format!("RemoveFile WAL nu poate captura leaf-ul: {error}"),
        )
    })?;
    let captured = fs::fstat(&source_handle).map_err(|error| {
        capability_error(
            &lexical.public_label,
            &format!("RemoveFile WAL nu poate citi leaf metadata: {error}"),
        )
    })?;
    validate_remove_named_identity(
        &parent.directory,
        &parent.leaf,
        &captured,
        &lexical.public_label,
        "RemoveFile plan source",
    )?;
    if !same_stable_leaf_version(&source_before, &captured) {
        return Err(capability_error(
            &lexical.public_label,
            "RemoveFile source s-a schimbat în timpul capturii",
        ));
    }

    let expected_content_hash = match &target.expected_leaf {
        ExpectedLeaf::Present(expected) => {
            if expected.tree_fingerprint.is_some() {
                return Err(capability_error(
                    &lexical.public_label,
                    "RemoveFile refuză tree fingerprint; directoarele folosesc RemoveDirectoryTree",
                ));
            }
            if version_token_for_stat(&captured) != expected.version_token {
                return Err(capability_error(
                    &lexical.public_label,
                    "RemoveFile source diferă de disk baseline înainte de WAL prepare",
                ));
            }
            expected.content_hash.clone()
        }
        ExpectedLeaf::Unspecified => None,
        ExpectedLeaf::Absent => unreachable!(),
    };
    let symlink_target_hex = if source_kind == WalRemoveLeafKind::Symlink {
        Some(read_remove_symlink_target(
            &parent.directory,
            &parent.leaf,
            &captured,
            &lexical.public_label,
        )?)
    } else {
        None
    };
    if expected_content_hash.is_some() && source_kind != WalRemoveLeafKind::Regular {
        return Err(capability_error(
            &lexical.public_label,
            "RemoveFile content hash cere un fișier regular",
        ));
    }

    let mut source_content = if let Some(expected_hash) = expected_content_hash.as_deref() {
        let descriptor = fs::openat(
            &parent.directory,
            &parent.leaf,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| {
            capability_error(
                &lexical.public_label,
                &format!("RemoveFile content nu poate fi deschis bounded: {error}"),
            )
        })?;
        let mut file = File::from(descriptor);
        let content_stat = fs::fstat(&file).map_err(|error| {
            capability_error(
                &lexical.public_label,
                &format!("RemoveFile content metadata nu poate fi citită: {error}"),
            )
        })?;
        if !same_file_identity(&captured, &content_stat) {
            return Err(capability_error(
                &lexical.public_label,
                "RemoveFile content descriptor diferă de source handle",
            ));
        }
        validate_remove_expected_content(
            &mut file,
            &content_stat,
            expected_hash,
            &lexical.public_label,
            "RemoveFile WAL plan content",
        )?;
        Some(file)
    } else {
        None
    };

    let source_after = fs::fstat(&source_handle).map_err(|error| {
        capability_error(
            &lexical.public_label,
            &format!("RemoveFile source nu poate fi reverificat: {error}"),
        )
    })?;
    if !same_stable_leaf_version(&captured, &source_after) {
        return Err(capability_error(
            &lexical.public_label,
            "RemoveFile source s-a schimbat în timpul planificării WAL",
        ));
    }
    if let Some(file) = source_content.as_mut() {
        let content_after = fs::fstat(&*file).map_err(|error| {
            capability_error(
                &lexical.public_label,
                &format!("RemoveFile content nu poate fi reverificat: {error}"),
            )
        })?;
        if !same_stable_leaf_version(&captured, &content_after) {
            return Err(capability_error(
                &lexical.public_label,
                "RemoveFile content s-a schimbat în timpul planificării WAL",
            ));
        }
    }
    validate_remove_named_identity(
        &parent.directory,
        &parent.leaf,
        &source_after,
        &lexical.public_label,
        "RemoveFile plan source final",
    )?;

    let quarantine_leaf = remove_quarantine_leaf(operation_id);
    if quarantine_leaf == parent.leaf
        || leaf_metadata(&parent.directory, &quarantine_leaf, &lexical.public_label)?.is_some()
    {
        return Err(capability_error(
            &lexical.public_label,
            "numele determinist RemoveFile quarantine nu este disponibil",
        ));
    }
    let parent_identity = wal_identity_from_fd(&parent.directory, &lexical.public_label)?;
    let (_, parents) = lexical
        .relative_components
        .split_last()
        .ok_or_else(|| capability_error(&lexical.public_label, "RemoveFile WAL cere un leaf"))?;

    Ok(Some(RemoveLeafOperationPlan {
        evidence: WalRemoveLeafEvidence {
            parent: WalParentEvidence {
                relative_components_hex: parents
                    .iter()
                    .map(|component| encode_component_hex(component))
                    .collect(),
                existing_prefix_len: parents.len(),
                existing_ancestor_identity: parent_identity.clone(),
                parent_identity: Some(parent_identity),
            },
            target_leaf_hex: encode_component_hex(&parent.leaf),
            quarantine_leaf_hex: encode_component_hex(&quarantine_leaf),
            source: remove_source_evidence(
                &source_after,
                source_kind,
                expected_content_hash,
                symlink_target_hex,
            )?,
        },
        source_handle,
        source_content,
    }))
}

pub(in crate::kernel::write_authority::capability) fn remove_leaf_wal(
    target: &WriteTarget,
    mut plan: RemoveLeafOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    let lexical = lexical_target(target, false)?;
    validate_remove_plan_shape(&lexical, target, &plan, guard.operation_id())?;
    let parent = match capture_parent_from_wal_evidence(&lexical, &plan.evidence.parent) {
        Ok(parent) => parent,
        Err(error) => return error.into_operation_result(),
    };
    if parent.created_ancestors {
        return Err(capability_error(
            &lexical.public_label,
            "RemoveFile WAL nu poate crea namespace părinte",
        ));
    }
    let quarantine_leaf = plan.quarantine_leaf()?;
    validate_remove_source(
        &parent.directory,
        &parent.leaf,
        &mut plan,
        true,
        &lexical.public_label,
    )?;
    validate_leaf_absent(
        &parent.directory,
        &quarantine_leaf,
        &lexical.public_label,
        "RemoveFile quarantine",
    )?;
    validate_remove_public_state(&lexical, &plan.evidence, RemovePublicState::Before)?;

    let recovery = |diagnostic: String| {
        wal_recovery_effect(
            0,
            &lexical.public_label,
            format!("{diagnostic} RemoveFile WAL rămâne pentru recovery."),
        )
    };
    if let Err(error) = guard.mark_auxiliary_durable() {
        return Ok(recovery(error));
    }

    run_test_hook(CapabilityTestStage::BeforeRemoveLeafQuarantine);
    if let Err(error) = fs::renameat_with(
        &parent.directory,
        &parent.leaf,
        &parent.directory,
        &quarantine_leaf,
        RenameFlags::NOREPLACE,
    ) {
        return Ok(recovery(capability_error(
            &lexical.public_label,
            &format!("RemoveFile quarantine rename a eșuat: {error}"),
        )));
    }
    if let Err(error) = guard.mark_effect_visible() {
        return Ok(recovery(error));
    }
    if let Err(error) = validate_quarantined_source(
        &parent.directory,
        &parent.leaf,
        &quarantine_leaf,
        &mut plan,
        &lexical.public_label,
    ) {
        return Ok(recovery(error));
    }
    if let Err(error) = sync_directory(&parent.directory, &lexical.public_label) {
        return Ok(recovery(error));
    }
    if let Err(error) =
        validate_remove_public_state(&lexical, &plan.evidence, RemovePublicState::Quarantined)
    {
        return Ok(recovery(error));
    }

    run_test_hook(CapabilityTestStage::BeforeRemoveLeafUnlink);
    if let Err(error) = validate_quarantined_source(
        &parent.directory,
        &parent.leaf,
        &quarantine_leaf,
        &mut plan,
        &lexical.public_label,
    ) {
        return Ok(recovery(error));
    }
    if let Err(error) = fs::unlinkat(&parent.directory, &quarantine_leaf, AtFlags::empty()) {
        return Ok(recovery(capability_error(
            &lexical.public_label,
            &format!("RemoveFile quarantine unlink a eșuat: {error}"),
        )));
    }
    if let Err(error) = sync_directory(&parent.directory, &lexical.public_label) {
        return Ok(recovery(error));
    }
    if let Err(error) =
        validate_remove_public_state(&lexical, &plan.evidence, RemovePublicState::Removed)
    {
        return Ok(recovery(error));
    }
    run_test_hook(CapabilityTestStage::BeforeRemoveLeafTargetDurable);
    if let Err(error) = guard.mark_target_durable() {
        return Ok(recovery(error));
    }
    if let Err(error) =
        validate_remove_public_state(&lexical, &plan.evidence, RemovePublicState::Removed)
    {
        return Ok(recovery(error));
    }
    Ok(CapabilityEffect::changed(0))
}

pub(in crate::kernel::write_authority::capability) fn classify_remove_leaf_recovery(
    record: &WalRecord,
    phase: WalPhase,
) -> Result<RemoveLeafRecoveryAssessment, String> {
    let WalOperationEvidence::RemoveLeaf(evidence) = &record.body.operation_evidence else {
        return Err("WriteAuthority RemoveFile recovery a primit altă familie.".into());
    };
    let (parent, target_leaf, quarantine_leaf) = capture_remove_recovery_parent(record, evidence)?;
    let target = observe_remove_leaf(&parent, &target_leaf, &record.body.public_label)?;
    let quarantine = observe_remove_leaf(&parent, &quarantine_leaf, &record.body.public_label)?;
    let target_is_before = target
        .as_ref()
        .is_some_and(|observed| observed_matches_remove_before(observed, &evidence.source));
    let target_is_stable = target
        .as_ref()
        .is_some_and(|observed| observed_matches_remove_moved(observed, &evidence.source));
    let quarantine_is_source = quarantine
        .as_ref()
        .is_some_and(|observed| observed_matches_remove_moved(observed, &evidence.source));

    if target_is_before && quarantine.is_none() {
        return Ok(RemoveLeafRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::NoEffect,
            automatic_action: (phase == WalPhase::Prepared)
                .then_some(RemoveLeafRecoveryAction::ClearNoEffect),
            diagnostic: if phase == WalPhase::Prepared {
                "RemoveFile source este baseline, iar quarantine lipsește; Prepared permite clear no-effect."
                    .into()
            } else {
                format!(
                    "RemoveFile source este baseline, dar faza {phase:?} păstrează WAL-ul pentru review."
                )
            },
        });
    }
    if target.is_none() && quarantine_is_source {
        return Ok(RemoveLeafRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::CleanupRequired,
            automatic_action: None,
            diagnostic:
                "RemoveFile source este izolat în quarantine exact. Namespace-ul nu este mutat automat; operatorul trebuie să aleagă restore sau finalizare."
                    .into(),
        });
    }
    if target.is_none() && quarantine.is_none() {
        if matches!(phase, WalPhase::EffectVisible | WalPhase::TargetDurable) {
            return Ok(RemoveLeafRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::EffectCommitted,
                automatic_action: Some(RemoveLeafRecoveryAction::FinalizeCommitted),
                diagnostic:
                    "RemoveFile a trecut de effect-visible, iar source și quarantine sunt absente; fsync/recheck fără mutație poate finaliza."
                        .into(),
            });
        }
        return Ok(RemoveLeafRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::Conflict,
            automatic_action: None,
            diagnostic:
                "RemoveFile source și quarantine lipsesc înainte de effect-visible; efectul nu poate fi atribuit sigur."
                    .into(),
        });
    }
    if target_is_stable && !target_is_before && quarantine.is_none() {
        return Ok(RemoveLeafRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::RollbackCompleted,
            automatic_action: None,
            diagnostic:
                "RemoveFile source pare restaurat, dar ctime-ul diferă; review manual obligatoriu."
                    .into(),
        });
    }
    Ok(RemoveLeafRecoveryAssessment {
        classification: WriteAuthorityRecoveryClassification::Conflict,
        automatic_action: None,
        diagnostic: format!(
            "Oracle-ul RemoveFile nu poate atribui namespace-ul (targetBefore={target_is_before}, targetStable={target_is_stable}, targetAbsent={}, quarantineSource={quarantine_is_source}, quarantineAbsent={}).",
            target.is_none(),
            quarantine.is_none()
        ),
    })
}

pub(in crate::kernel::write_authority::capability) fn execute_remove_leaf_recovery(
    record: &WalRecord,
    phase: WalPhase,
) -> Result<(), String> {
    let assessment = classify_remove_leaf_recovery(record, phase)?;
    let action = assessment.automatic_action.ok_or_else(|| {
        format!(
            "WriteAuthority RemoveFile recovery nu permite acțiune automată: {}",
            assessment.diagnostic
        )
    })?;
    match action {
        RemoveLeafRecoveryAction::ClearNoEffect => Ok(()),
        RemoveLeafRecoveryAction::FinalizeCommitted => {
            let WalOperationEvidence::RemoveLeaf(evidence) = &record.body.operation_evidence else {
                return Err("WriteAuthority RemoveFile finalize a primit altă familie.".into());
            };
            let (parent, target_leaf, quarantine_leaf) =
                capture_remove_recovery_parent(record, evidence)?;
            if observe_remove_leaf(&parent, &target_leaf, &record.body.public_label)?.is_some()
                || observe_remove_leaf(&parent, &quarantine_leaf, &record.body.public_label)?
                    .is_some()
            {
                return Err(
                    "WriteAuthority RemoveFile finalize CAS a observat un nume reapărut.".into(),
                );
            }
            sync_directory(&parent, &record.body.public_label)?;
            let after = classify_remove_leaf_recovery(record, phase)?;
            if after.classification != WriteAuthorityRecoveryClassification::EffectCommitted {
                return Err(format!(
                    "WriteAuthority RemoveFile finalize postflight s-a schimbat: {}",
                    after.diagnostic
                ));
            }
            Ok(())
        }
    }
}

pub(in crate::kernel::write_authority::capability) fn resolve_remove_leaf_operator(
    record: &WalRecord,
    phase: WalPhase,
    action: WriteAuthorityRecoveryResolutionAction,
) -> Result<String, String> {
    let WalOperationEvidence::RemoveLeaf(evidence) = &record.body.operation_evidence else {
        return Err("WriteAuthority RemoveFile operator a primit altă familie.".into());
    };
    let assessment = classify_remove_leaf_recovery(record, phase)?;
    match action {
        WriteAuthorityRecoveryResolutionAction::RestoreOriginal => {
            if assessment.classification != WriteAuthorityRecoveryClassification::CleanupRequired {
                return Err(format!(
                    "RestoreOriginal nu este permis pentru {:?}: {}",
                    assessment.classification, assessment.diagnostic
                ));
            }
            let (parent, target_leaf, quarantine_leaf) =
                capture_remove_recovery_parent(record, evidence)?;
            if observe_remove_leaf(&parent, &target_leaf, &record.body.public_label)?.is_some() {
                return Err(
                    "RestoreOriginal a fost blocat: targetul original nu mai este absent.".into(),
                );
            }
            let quarantine =
                observe_remove_leaf(&parent, &quarantine_leaf, &record.body.public_label)?
                    .ok_or_else(|| {
                        "RestoreOriginal a fost blocat: quarantine lipsește.".to_string()
                    })?;
            if !observed_matches_remove_moved(&quarantine, &evidence.source) {
                return Err(
                    "RestoreOriginal a fost blocat: quarantine nu mai este inode-ul WAL exact."
                        .into(),
                );
            }
            fs::renameat_with(
                &parent,
                &quarantine_leaf,
                &parent,
                &target_leaf,
                RenameFlags::NOREPLACE,
            )
            .map_err(|error| {
                capability_error(
                    &record.body.public_label,
                    &format!("RestoreOriginal RENAME_NOREPLACE a eșuat: {error}"),
                )
            })?;
            let restored = observe_remove_leaf(&parent, &target_leaf, &record.body.public_label)?
                .ok_or_else(|| {
                "RestoreOriginal postflight nu mai găsește targetul restaurat.".to_string()
            })?;
            if !observed_matches_remove_moved(&restored, &evidence.source)
                || observe_remove_leaf(&parent, &quarantine_leaf, &record.body.public_label)?
                    .is_some()
            {
                return Err(
                    "RestoreOriginal postflight nu poate demonstra inode-ul WAL restaurat; recordul rămâne hot."
                        .into(),
                );
            }
            sync_directory(&parent, &record.body.public_label)?;
            let after = classify_remove_leaf_recovery(record, phase)?;
            if !matches!(
                after.classification,
                WriteAuthorityRecoveryClassification::NoEffect
                    | WriteAuthorityRecoveryClassification::RollbackCompleted
            ) {
                return Err(format!(
                    "RestoreOriginal postflight public s-a schimbat: {}",
                    after.diagnostic
                ));
            }
            Ok("RemoveFile quarantine exact a fost restaurat durabil la numele original.".into())
        }
        WriteAuthorityRecoveryResolutionAction::AcceptRestoredState => {
            if !matches!(
                assessment.classification,
                WriteAuthorityRecoveryClassification::NoEffect
                    | WriteAuthorityRecoveryClassification::RollbackCompleted
            ) || assessment.automatic_action.is_some()
            {
                return Err(format!(
                    "AcceptRestoredState nu este permis pentru {:?}: {}",
                    assessment.classification, assessment.diagnostic
                ));
            }
            let (parent, target_leaf, quarantine_leaf) =
                capture_remove_recovery_parent(record, evidence)?;
            let restored = observe_remove_leaf(&parent, &target_leaf, &record.body.public_label)?
                .ok_or_else(|| {
                "AcceptRestoredState a fost blocat: targetul original lipsește.".to_string()
            })?;
            if !observed_matches_remove_moved(&restored, &evidence.source)
                || observe_remove_leaf(&parent, &quarantine_leaf, &record.body.public_label)?
                    .is_some()
            {
                return Err(
                    "AcceptRestoredState a fost blocat: starea restaurată nu mai corespunde WAL."
                        .into(),
                );
            }
            sync_directory(&parent, &record.body.public_label)?;
            let after = classify_remove_leaf_recovery(record, phase)?;
            if !matches!(
                after.classification,
                WriteAuthorityRecoveryClassification::NoEffect
                    | WriteAuthorityRecoveryClassification::RollbackCompleted
            ) {
                return Err(format!(
                    "AcceptRestoredState postflight s-a schimbat: {}",
                    after.diagnostic
                ));
            }
            Ok(
                "Starea RemoveFile restaurată a fost acceptată explicit fără mutație de namespace."
                    .into(),
            )
        }
        WriteAuthorityRecoveryResolutionAction::ContinueTreeRemoval
        | WriteAuthorityRecoveryResolutionAction::RestoreRemainingTree => {
            Err("Acțiunea operator este rezervată familiei RemoveDirectoryTree.".into())
        }
        WriteAuthorityRecoveryResolutionAction::AcceptCurrentState => Err(
            "Acțiunea operator AcceptCurrentState este rezervată familiilor Directory/Symlink."
                .into(),
        ),
    }
}

impl RemoveLeafOperationPlan {
    fn quarantine_leaf(&self) -> Result<OsString, String> {
        decode_component_hex(&self.evidence.quarantine_leaf_hex)
    }
}

fn absent_plan_result(
    target: &WriteTarget,
    public_label: &str,
) -> Result<Option<RemoveLeafOperationPlan>, String> {
    if matches!(target.expected_leaf, ExpectedLeaf::Present(_)) {
        Err(capability_error(
            public_label,
            "RemoveFile expected Present, dar leaf-ul lipsește înainte de WAL",
        ))
    } else {
        Ok(None)
    }
}

fn remove_source_evidence(
    stat: &fs::Stat,
    kind: WalRemoveLeafKind,
    content_hash: Option<String>,
    symlink_target_hex: Option<String>,
) -> Result<WalRemoveLeafSourceEvidence, String> {
    Ok(WalRemoveLeafSourceEvidence {
        identity: WalFilesystemIdentity {
            device: stat.st_dev,
            inode: stat.st_ino,
        },
        kind,
        size: u64::try_from(stat.st_size)
            .map_err(|_| "RemoveFile leaf are dimensiune negativă.".to_string())?,
        mtime_seconds: stat.st_mtime,
        mtime_nanoseconds: stat.st_mtime_nsec,
        ctime_seconds: stat.st_ctime,
        ctime_nanoseconds: stat.st_ctime_nsec,
        raw_mode: stat.st_mode,
        link_count: stat.st_nlink,
        owner_uid: stat.st_uid,
        owner_gid: stat.st_gid,
        raw_device: stat.st_rdev,
        version_token: version_token_for_stat(stat),
        content_hash,
        symlink_target_hex,
    })
}

fn validate_remove_plan_shape(
    lexical: &LexicalTarget,
    target: &WriteTarget,
    plan: &RemoveLeafOperationPlan,
    operation_id: &str,
) -> Result<(), String> {
    let (leaf, parents) = lexical
        .relative_components
        .split_last()
        .ok_or_else(|| capability_error(&lexical.public_label, "RemoveFile WAL cere un leaf"))?;
    let planned_parents = plan
        .evidence
        .parent
        .relative_components_hex
        .iter()
        .map(|component| decode_component_hex(component))
        .collect::<Result<Vec<_>, _>>()?;
    if planned_parents != parents
        || decode_component_hex(&plan.evidence.target_leaf_hex)? != *leaf
        || decode_component_hex(&plan.evidence.quarantine_leaf_hex)?
            != remove_quarantine_leaf(operation_id)
    {
        return Err(capability_error(
            &lexical.public_label,
            "planul RemoveFile WAL nu corespunde targetului/operației",
        ));
    }
    if let ExpectedLeaf::Present(expected) = &target.expected_leaf {
        if expected.version_token != plan.evidence.source.version_token
            || expected.content_hash != plan.evidence.source.content_hash
            || expected.tree_fingerprint.is_some()
        {
            return Err(capability_error(
                &lexical.public_label,
                "planul RemoveFile WAL nu corespunde disk baseline-ului declarat",
            ));
        }
    }
    Ok(())
}

fn validate_remove_source(
    parent: &OwnedFd,
    leaf: &OsStr,
    plan: &mut RemoveLeafOperationPlan,
    require_before_token: bool,
    public_label: &str,
) -> Result<(), String> {
    let handle_stat = fs::fstat(&plan.source_handle).map_err(|error| {
        capability_error(
            public_label,
            &format!("RemoveFile source handle nu poate fi citit: {error}"),
        )
    })?;
    let symlink_target_hex = if plan.evidence.source.kind == WalRemoveLeafKind::Symlink {
        Some(read_remove_symlink_target(
            parent,
            leaf,
            &handle_stat,
            public_label,
        )?)
    } else {
        None
    };
    let observed = observed_remove_from_stat(&handle_stat, symlink_target_hex, public_label)?;
    if !observed_matches_remove_moved(&observed, &plan.evidence.source)
        || require_before_token && observed.version_token != plan.evidence.source.version_token
    {
        return Err(capability_error(
            public_label,
            "RemoveFile source handle diferă de planul WAL",
        ));
    }
    validate_remove_named_identity(
        parent,
        leaf,
        &handle_stat,
        public_label,
        "RemoveFile source",
    )?;
    validate_remove_content(plan, &handle_stat, public_label)
}

fn validate_quarantined_source(
    parent: &OwnedFd,
    original_leaf: &OsStr,
    quarantine_leaf: &OsStr,
    plan: &mut RemoveLeafOperationPlan,
    public_label: &str,
) -> Result<(), String> {
    validate_leaf_absent(parent, original_leaf, public_label, "RemoveFile source")?;
    validate_remove_source(parent, quarantine_leaf, plan, false, public_label)
}

fn validate_remove_content(
    plan: &mut RemoveLeafOperationPlan,
    handle_stat: &fs::Stat,
    public_label: &str,
) -> Result<(), String> {
    let Some(expected_hash) = plan.evidence.source.content_hash.as_deref() else {
        return Ok(());
    };
    let file = plan.source_content.as_mut().ok_or_else(|| {
        capability_error(
            public_label,
            "RemoveFile plan a pierdut descriptorul content hash",
        )
    })?;
    let content_stat = fs::fstat(&*file).map_err(|error| {
        capability_error(
            public_label,
            &format!("RemoveFile content handle nu poate fi citit: {error}"),
        )
    })?;
    if !same_stable_leaf_version(handle_stat, &content_stat) {
        return Err(capability_error(
            public_label,
            "RemoveFile content handle diferă de source handle",
        ));
    }
    validate_remove_expected_content(
        file,
        &content_stat,
        expected_hash,
        public_label,
        "RemoveFile runtime content",
    )
}

fn validate_remove_expected_content(
    file: &mut File,
    stat: &fs::Stat,
    expected_hash: &str,
    public_label: &str,
    stage: &str,
) -> Result<(), String> {
    const MAX_REMOVE_HASH_BYTES: u64 = 512 * 1024 * 1024;
    let expected_size = u64::try_from(stat.st_size)
        .map_err(|_| capability_error(public_label, &format!("{stage}: dimensiune negativă")))?;
    if expected_size > MAX_REMOVE_HASH_BYTES {
        return Err(capability_error(
            public_label,
            &format!("{stage}: verificarea hash depășește limita de {MAX_REMOVE_HASH_BYTES} bytes"),
        ));
    }
    file.seek(SeekFrom::Start(0)).map_err(|error| {
        capability_error(
            public_label,
            &format!("{stage}: descriptorul nu poate reveni la început: {error}"),
        )
    })?;
    let mut hash = 0xcbf29ce484222325u64;
    let mut observed = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| {
            capability_error(
                public_label,
                &format!("{stage}: conținutul nu poate fi citit streaming: {error}"),
            )
        })?;
        if count == 0 {
            break;
        }
        observed = observed.saturating_add(count as u64);
        if observed > expected_size {
            return Err(capability_error(
                public_label,
                &format!("{stage}: conținutul a crescut în timpul hash-ului"),
            ));
        }
        for byte in &buffer[..count] {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    file.seek(SeekFrom::Start(0)).map_err(|error| {
        capability_error(
            public_label,
            &format!("{stage}: descriptorul nu poate fi resetat: {error}"),
        )
    })?;
    let observed_hash = format!("{hash:016x}");
    if observed != expected_size || observed_hash != expected_hash {
        return Err(capability_error(
            public_label,
            &format!(
                "{stage}: conținutul disk s-a schimbat (expected hash {expected_hash}, observed {observed_hash})"
            ),
        ));
    }
    let after = fs::fstat(&*file).map_err(|error| {
        capability_error(
            public_label,
            &format!("{stage}: metadata post-hash nu poate fi citită: {error}"),
        )
    })?;
    if !same_stable_leaf_version(stat, &after) {
        return Err(capability_error(
            public_label,
            &format!("{stage}: fișierul s-a schimbat în timpul hash-ului"),
        ));
    }
    Ok(())
}

fn validate_remove_public_state(
    lexical: &LexicalTarget,
    evidence: &WalRemoveLeafEvidence,
    expected_state: RemovePublicState,
) -> Result<(), String> {
    let parent = capture_existing_target_parent(lexical)?.ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "RemoveFile full-path CAS nu mai poate captura parentul",
        )
    })?;
    let observed_parent = wal_identity_from_fd(&parent.directory, &lexical.public_label)?;
    if evidence.parent.parent_identity.as_ref() != Some(&observed_parent) {
        return Err(capability_error(
            &lexical.public_label,
            "RemoveFile full-path CAS a observat alt parent",
        ));
    }
    let target = observe_remove_leaf(&parent.directory, &parent.leaf, &lexical.public_label)?;
    let quarantine_leaf = decode_component_hex(&evidence.quarantine_leaf_hex)?;
    let quarantine =
        observe_remove_leaf(&parent.directory, &quarantine_leaf, &lexical.public_label)?;
    let valid = match expected_state {
        RemovePublicState::Before => {
            target
                .as_ref()
                .is_some_and(|observed| observed_matches_remove_before(observed, &evidence.source))
                && quarantine.is_none()
        }
        RemovePublicState::Quarantined => {
            target.is_none()
                && quarantine.as_ref().is_some_and(|observed| {
                    observed_matches_remove_moved(observed, &evidence.source)
                })
        }
        RemovePublicState::Removed => target.is_none() && quarantine.is_none(),
    };
    if !valid {
        return Err(capability_error(
            &lexical.public_label,
            "RemoveFile full-path CAS diferă de starea WAL așteptată",
        ));
    }
    Ok(())
}

fn capture_remove_recovery_parent(
    record: &WalRecord,
    evidence: &WalRemoveLeafEvidence,
) -> Result<(OwnedFd, OsString, OsString), String> {
    let boundary_path = decode_path_hex(&record.body.authority.boundary_path_hex)?;
    let authority = capture_directory_authority(
        &boundary_path,
        "write-authority-wal/remove-leaf-recovery-target",
        DirectoryAuthorityScope::RecoveryTarget,
    )?;
    if authority.identity().device != record.body.authority.identity.device
        || authority.identity().inode != record.body.authority.identity.inode
    {
        return Err(capability_error(
            &record.body.public_label,
            "RemoveFile recovery authority identity diferă de WAL",
        ));
    }
    let parents = evidence
        .parent
        .relative_components_hex
        .iter()
        .map(|component| decode_component_hex(component))
        .collect::<Result<Vec<_>, _>>()?;
    let mut directory = rustix::io::dup(authority.directory()).map_err(|error| {
        capability_error(
            &record.body.public_label,
            &format!("RemoveFile recovery nu poate duplica authority: {error}"),
        )
    })?;
    for component in parents {
        let next = open_directory_strict(&directory, &component).map_err(|error| {
            capability_error(
                &record.body.public_label,
                &format!("RemoveFile recovery parent capture a eșuat: {error}"),
            )
        })?;
        validate_named_directory_identity(
            &directory,
            &component,
            &next,
            &record.body.public_label,
            "RemoveFile recovery parent",
        )?;
        directory = next;
    }
    let observed_parent = wal_identity_from_fd(&directory, &record.body.public_label)?;
    if evidence.parent.parent_identity.as_ref() != Some(&observed_parent) {
        return Err(capability_error(
            &record.body.public_label,
            "RemoveFile recovery parent identity diferă de WAL",
        ));
    }
    Ok((
        directory,
        decode_component_hex(&evidence.target_leaf_hex)?,
        decode_component_hex(&evidence.quarantine_leaf_hex)?,
    ))
}

fn observe_remove_leaf(
    parent: &OwnedFd,
    leaf: &OsStr,
    public_label: &str,
) -> Result<Option<ObservedRemoveLeaf>, String> {
    leaf_metadata(parent, leaf, public_label)?
        .map(|stat| {
            let target = if FileType::from_raw_mode(stat.st_mode) == FileType::Symlink {
                Some(read_remove_symlink_target(
                    parent,
                    leaf,
                    &stat,
                    public_label,
                )?)
            } else {
                None
            };
            observed_remove_from_stat(&stat, target, public_label)
        })
        .transpose()
}

fn observed_remove_from_stat(
    stat: &fs::Stat,
    symlink_target_hex: Option<String>,
    public_label: &str,
) -> Result<ObservedRemoveLeaf, String> {
    Ok(ObservedRemoveLeaf {
        identity: WalFilesystemIdentity {
            device: stat.st_dev,
            inode: stat.st_ino,
        },
        kind: remove_leaf_kind(stat, public_label)?,
        size: u64::try_from(stat.st_size).map_err(|_| {
            capability_error(public_label, "RemoveFile leaf are dimensiune negativă")
        })?,
        mtime_seconds: stat.st_mtime,
        mtime_nanoseconds: stat.st_mtime_nsec,
        ctime_seconds: stat.st_ctime,
        ctime_nanoseconds: stat.st_ctime_nsec,
        raw_mode: stat.st_mode,
        link_count: stat.st_nlink,
        owner_uid: stat.st_uid,
        owner_gid: stat.st_gid,
        raw_device: stat.st_rdev,
        version_token: version_token_for_stat(stat),
        symlink_target_hex,
    })
}

fn observed_matches_remove_before(
    observed: &ObservedRemoveLeaf,
    expected: &WalRemoveLeafSourceEvidence,
) -> bool {
    observed_matches_remove_moved(observed, expected)
        && observed.version_token == expected.version_token
}

fn observed_matches_remove_moved(
    observed: &ObservedRemoveLeaf,
    expected: &WalRemoveLeafSourceEvidence,
) -> bool {
    observed.identity == expected.identity
        && observed.kind == expected.kind
        && observed.size == expected.size
        && observed.mtime_seconds == expected.mtime_seconds
        && observed.mtime_nanoseconds == expected.mtime_nanoseconds
        && observed.raw_mode == expected.raw_mode
        && observed.link_count == expected.link_count
        && observed.owner_uid == expected.owner_uid
        && observed.owner_gid == expected.owner_gid
        && observed.raw_device == expected.raw_device
        && observed.symlink_target_hex == expected.symlink_target_hex
}

fn read_remove_symlink_target(
    parent: &OwnedFd,
    leaf: &OsStr,
    expected: &fs::Stat,
    public_label: &str,
) -> Result<String, String> {
    let literal = fs::readlinkat(parent, leaf, Vec::new()).map_err(|error| {
        capability_error(
            public_label,
            &format!("RemoveFile literalul symlink nu poate fi citit: {error}"),
        )
    })?;
    let bytes = literal.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_WAL_SYMLINK_TARGET_BYTES || bytes.contains(&0) {
        return Err(capability_error(
            public_label,
            "RemoveFile literalul symlink depășește contractul WAL",
        ));
    }
    let after = fs::statat(parent, leaf, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
        capability_error(
            public_label,
            &format!("RemoveFile symlink nu poate fi reverificat: {error}"),
        )
    })?;
    if version_token_for_stat(expected) != version_token_for_stat(&after)
        || !same_stable_leaf_version(expected, &after)
    {
        return Err(capability_error(
            public_label,
            "RemoveFile symlink s-a schimbat în timpul readlinkat",
        ));
    }
    Ok(encode_bytes_hex(bytes))
}

fn remove_leaf_kind(stat: &fs::Stat, public_label: &str) -> Result<WalRemoveLeafKind, String> {
    match FileType::from_raw_mode(stat.st_mode) {
        FileType::Directory => Err(capability_error(
            public_label,
            "RemoveFile a primit un director; folosește RemoveDirectoryTree",
        )),
        FileType::RegularFile => Ok(WalRemoveLeafKind::Regular),
        FileType::Symlink => Ok(WalRemoveLeafKind::Symlink),
        _ => Ok(WalRemoveLeafKind::Other),
    }
}

fn validate_remove_named_identity(
    parent: &OwnedFd,
    leaf: &OsStr,
    expected: &fs::Stat,
    public_label: &str,
    role: &str,
) -> Result<(), String> {
    let observed = fs::statat(parent, leaf, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
        capability_error(
            public_label,
            &format!("{role}: named stat a eșuat: {error}"),
        )
    })?;
    if !same_file_identity(expected, &observed) {
        return Err(capability_error(
            public_label,
            &format!("{role}: numele nu mai indică inode-ul capturat"),
        ));
    }
    Ok(())
}

fn validate_leaf_absent(
    parent: &OwnedFd,
    leaf: &OsStr,
    public_label: &str,
    role: &str,
) -> Result<(), String> {
    if leaf_metadata(parent, leaf, public_label)?.is_some() {
        return Err(capability_error(
            public_label,
            &format!("{role} nu mai este absent"),
        ));
    }
    Ok(())
}
