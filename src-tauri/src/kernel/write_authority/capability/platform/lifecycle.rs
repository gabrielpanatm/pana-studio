use super::*;
use std::os::unix::ffi::OsStrExt;

#[derive(Clone, Debug, Eq, PartialEq)]
struct ObservedSymlink {
    identity: WalFilesystemIdentity,
    version_token: String,
    link_target_hex: String,
}

enum RecoverySymlinkContext {
    ParentMissing {
        observed_prefix_len: usize,
        planned_existing_prefix_len: usize,
    },
    Ready {
        directory: OwnedFd,
        target_leaf: OsString,
        parent_was_missing: bool,
    },
}

#[cfg(test)]
thread_local! {
    static TEST_FAIL_SYMLINK_EIO: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn plan_legacy_symlink(
    target: &WriteTarget,
    source: &Path,
) -> Result<SymlinkOperationPlan, String> {
    validate_symlink_literal(source, &target.public_label)?;
    if matches!(target.expected_leaf, ExpectedLeaf::Present(_)) {
        return Err(capability_error(
            &target.public_label,
            "Symlink WAL nu acceptă un baseline de fișier regular Present",
        ));
    }
    let lexical = lexical_target(target, false)?;
    if lexical.authority.is_none() {
        return Err(capability_error(
            &lexical.public_label,
            "planul symlink WAL cere authority root sigilat",
        ));
    }
    let boundary = capture_existing_boundary(&lexical)?.ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "authority root nu există pentru planul symlink",
        )
    })?;
    let (leaf, parents) = lexical
        .relative_components
        .split_last()
        .ok_or_else(|| capability_error(&lexical.public_label, "planul symlink cere un leaf"))?;
    let mut directory = boundary.directory;
    let mut existing_prefix_len = 0_usize;
    for component in parents {
        match open_directory_strict(&directory, component) {
            Ok(next) => {
                validate_named_directory_identity(
                    &directory,
                    component,
                    &next,
                    &lexical.public_label,
                    "symlink WAL parent",
                )?;
                directory = next;
                existing_prefix_len += 1;
            }
            Err(Errno::NOENT) => break,
            Err(error) => {
                return Err(capability_error(
                    &lexical.public_label,
                    &format!("planul symlink nu poate captura un părinte: {error}"),
                ));
            }
        }
    }
    let existing_ancestor_identity = wal_identity_from_fd(&directory, &lexical.public_label)?;
    let parent_exists = existing_prefix_len == parents.len();
    let parent_identity = parent_exists
        .then(|| wal_identity_from_fd(&directory, &lexical.public_label))
        .transpose()?;
    let desired_link_target_hex = encode_path_hex(source);
    let before = if parent_exists {
        match observe_symlink(&directory, leaf, &lexical.public_label)? {
            None => WalSymlinkBefore::Absent,
            Some(observed) if observed.link_target_hex == desired_link_target_hex => {
                WalSymlinkBefore::Exact {
                    identity: observed.identity,
                    version_token: observed.version_token,
                    link_target_hex: observed.link_target_hex,
                    identity_digest: None,
                    state_digest: None,
                }
            }
            Some(observed) => {
                return Err(capability_error(
                    &lexical.public_label,
                    &format!(
                        "leaf-ul existent este un symlink către alt literal ({})",
                        observed.link_target_hex
                    ),
                ));
            }
        }
    } else {
        WalSymlinkBefore::Absent
    };

    Ok(SymlinkOperationPlan {
        evidence: WalSymlinkEvidence {
            protocol_version: 0,
            parent: WalParentEvidence {
                relative_components_hex: parents
                    .iter()
                    .map(|component| encode_component_hex(component))
                    .collect(),
                existing_prefix_len,
                existing_ancestor_identity,
                parent_identity,
            },
            target_leaf_hex: encode_component_hex(leaf),
            desired_link_target_hex,
            before,
        },
    })
}

pub(in crate::kernel::write_authority::capability) fn symlink_entry_legacy_wal(
    target: &WriteTarget,
    source: &Path,
    plan: &SymlinkOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    validate_symlink_literal(source, &target.public_label)?;
    let lexical = lexical_target(target, false)?;
    validate_symlink_plan_shape(&lexical, source, plan)?;
    let parent = match capture_parent_from_wal_evidence(&lexical, &plan.evidence.parent) {
        Ok(parent) => parent,
        Err(error) => return error.into_operation_result(),
    };
    run_test_hook(CapabilityTestStage::AfterTargetParentCaptured);
    let parent_changed = parent.created_ancestors;
    let observed = match observe_symlink(&parent.directory, &parent.leaf, &lexical.public_label) {
        Ok(observed) => observed,
        Err(error) if parent_changed => {
            return Ok(wal_recovery_effect(0, &lexical.public_label, error));
        }
        Err(error) => return Err(error),
    };
    if !observed_matches_before(observed.as_ref(), &plan.evidence.before) {
        let diagnostic = capability_error(
            &lexical.public_label,
            "symlink baseline diferă de planul WAL",
        );
        return if parent_changed {
            Ok(wal_recovery_effect(0, &lexical.public_label, diagnostic))
        } else {
            Err(diagnostic)
        };
    }
    if matches!(plan.evidence.before, WalSymlinkBefore::Exact { .. }) {
        let exact = observed.as_ref().expect("baseline Exact was matched above");
        validate_symlink_runtime_postflight(&lexical, &parent.directory, exact, plan)?;
        return Ok(CapabilityEffect::unchanged());
    }

    if parent_changed {
        if let Err(error) = guard.mark_auxiliary_durable() {
            return Ok(wal_recovery_effect(0, &lexical.public_label, error));
        }
    }
    let desired_target = plan.desired_target()?;
    if let Err(error) = symlinkat_exact(&desired_target, &parent.directory, &parent.leaf) {
        let diagnostic = capability_error(
            &lexical.public_label,
            &format!("symlinkat protejat de WAL a eșuat: {error}"),
        );
        return if parent_changed || error == Errno::IO {
            Ok(wal_recovery_effect(0, &lexical.public_label, diagnostic))
        } else {
            Err(diagnostic)
        };
    }
    run_test_hook(CapabilityTestStage::AfterSymlinkCreateBeforePhase);
    if !parent_changed {
        if let Err(error) = guard.mark_auxiliary_durable() {
            return Ok(wal_recovery_effect(0, &lexical.public_label, error));
        }
    }
    if let Err(error) = guard.mark_effect_visible() {
        return Ok(wal_recovery_effect(0, &lexical.public_label, error));
    }
    let committed_observed =
        match observe_symlink(&parent.directory, &parent.leaf, &lexical.public_label) {
            Ok(Some(observed))
                if observed.link_target_hex == plan.evidence.desired_link_target_hex =>
            {
                observed
            }
            Ok(_) => {
                return Ok(wal_recovery_effect(
                    0,
                    &lexical.public_label,
                    "symlink-ul vizibil nu conține literalul planificat",
                ));
            }
            Err(error) => {
                return Ok(wal_recovery_effect(0, &lexical.public_label, error));
            }
        };
    if let Err(error) = sync_directory(&parent.directory, &lexical.public_label) {
        return Ok(wal_recovery_effect(0, &lexical.public_label, error));
    }
    run_test_hook(CapabilityTestStage::BeforeSymlinkTargetDurable);
    if let Err(error) =
        validate_symlink_runtime_postflight(&lexical, &parent.directory, &committed_observed, plan)
    {
        return Ok(wal_recovery_effect(0, &lexical.public_label, error));
    }
    if let Err(error) = guard.mark_target_durable() {
        return Ok(wal_recovery_effect(0, &lexical.public_label, error));
    }
    Ok(CapabilityEffect::changed(0))
}

fn symlinkat_exact(desired_target: &Path, parent: &OwnedFd, leaf: &OsStr) -> Result<(), Errno> {
    #[cfg(test)]
    if TEST_FAIL_SYMLINK_EIO.with(std::cell::Cell::get) {
        return Err(Errno::IO);
    }
    fs::symlinkat(desired_target, parent, leaf)
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn with_symlink_eio_for_test<T>(
    operation: impl FnOnce() -> T,
) -> T {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            TEST_FAIL_SYMLINK_EIO.with(|flag| flag.set(false));
        }
    }
    TEST_FAIL_SYMLINK_EIO.with(|flag| {
        assert!(
            !flag.replace(true),
            "symlink EIO injection already installed"
        );
    });
    let _reset = Reset;
    operation()
}

fn validate_symlink_runtime_postflight(
    lexical: &LexicalTarget,
    expected_parent: &OwnedFd,
    expected_leaf: &ObservedSymlink,
    plan: &SymlinkOperationPlan,
) -> Result<(), String> {
    let authority = lexical.authority.as_ref().ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "symlink postflight cere authority root sigilat",
        )
    })?;
    let (leaf, parents) = lexical
        .relative_components
        .split_last()
        .ok_or_else(|| capability_error(&lexical.public_label, "symlink postflight cere leaf"))?;
    let mut directory = rustix::io::dup(authority.directory()).map_err(|error| {
        capability_error(
            &lexical.public_label,
            &format!("symlink postflight nu poate duplica authority: {error}"),
        )
    })?;
    for component in parents {
        let next = open_directory_strict(&directory, component).map_err(|error| {
            capability_error(
                &lexical.public_label,
                &format!("symlink postflight nu poate recaptura parentul: {error}"),
            )
        })?;
        validate_named_directory_identity(
            &directory,
            component,
            &next,
            &lexical.public_label,
            "symlink postflight parent",
        )?;
        directory = next;
    }
    if wal_identity_from_fd(&directory, &lexical.public_label)?
        != wal_identity_from_fd(expected_parent, &lexical.public_label)?
    {
        return Err(capability_error(
            &lexical.public_label,
            "symlink postflight path-ul nu mai numește parentul sincronizat",
        ));
    }
    let observed = observe_symlink(&directory, leaf, &lexical.public_label)?.ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "symlink postflight nu mai vede leaf-ul",
        )
    })?;
    if observed.identity != expected_leaf.identity
        || observed.link_target_hex != plan.evidence.desired_link_target_hex
    {
        return Err(capability_error(
            &lexical.public_label,
            "symlink postflight leaf-ul nu mai este efectul sincronizat",
        ));
    }
    Ok(())
}

pub(in crate::kernel::write_authority::capability) fn classify_legacy_symlink_recovery(
    record: &WalRecord,
    phase: WalPhase,
) -> Result<SymlinkRecoveryAssessment, String> {
    let WalOperationEvidence::Symlink(evidence) = &record.body.operation_evidence else {
        return Err("WriteAuthority symlink recovery a primit altă familie.".into());
    };
    let context = capture_recovery_symlink_context(record, evidence)?;
    let RecoverySymlinkContext::Ready {
        directory,
        target_leaf,
        parent_was_missing,
    } = context
    else {
        let RecoverySymlinkContext::ParentMissing {
            observed_prefix_len,
            planned_existing_prefix_len,
        } = context
        else {
            unreachable!()
        };
        if matches!(evidence.before, WalSymlinkBefore::Exact { .. })
            || observed_prefix_len < planned_existing_prefix_len
        {
            return Ok(SymlinkRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::Conflict,
                automatic_action: None,
                available_resolution_actions: Vec::new(),
                resolution_state_binding: None,
                diagnostic:
                    "Un părinte baseline al symlink-ului lipsește; manual review obligatoriu."
                        .into(),
            });
        }
        return if observed_prefix_len == planned_existing_prefix_len {
            Ok(SymlinkRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::Conflict,
                automatic_action: None,
                available_resolution_actions: Vec::new(),
                resolution_state_binding: None,
                diagnostic: format!(
                    "Primul părinte planificat absent nu este vizibil în faza {phase:?}, dar crearea părinților rulează înaintea checkpointului legacy; un efect creat și apoi eliminat nu poate fi exclus. WAL-ul rămâne hot."
                ),
            })
        } else {
            Ok(SymlinkRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::PartialNamespaceCreation,
                automatic_action: None,
                available_resolution_actions: Vec::new(),
                resolution_state_binding: None,
                diagnostic:
                    "Părinții symlink sunt parțial creați, dar recordul nu conține identități post-create; manual review obligatoriu."
                        .into(),
            })
        };
    };

    let observed = observe_symlink(&directory, &target_leaf, &record.body.public_label)?;
    match &evidence.before {
        WalSymlinkBefore::Exact { .. } => {
            if observed_matches_before(observed.as_ref(), &evidence.before)
                && phase == WalPhase::Prepared
            {
                Ok(SymlinkRecoveryAssessment {
                    classification: WriteAuthorityRecoveryClassification::NoEffect,
                    automatic_action: Some(SymlinkRecoveryAction::ClearNoEffect),
                    available_resolution_actions: Vec::new(),
                    resolution_state_binding: None,
                    diagnostic:
                        "Symlink-ul exista înainte de WAL, păstrează identitatea/literalul baseline, iar faza Prepared este singura fază posibilă pentru acest no-op legacy."
                            .into(),
                })
            } else if observed_matches_before(observed.as_ref(), &evidence.before) {
                Ok(SymlinkRecoveryAssessment {
                    classification: WriteAuthorityRecoveryClassification::Conflict,
                    automatic_action: None,
                    available_resolution_actions: Vec::new(),
                    resolution_state_binding: None,
                    diagnostic: format!(
                        "Symlink-ul baseline este intact, dar faza {phase:?} este imposibilă pentru no-op-ul symlink legacy; WAL-ul rămâne hot."
                    ),
                })
            } else {
                Ok(SymlinkRecoveryAssessment {
                    classification: WriteAuthorityRecoveryClassification::Conflict,
                    automatic_action: None,
                    available_resolution_actions: Vec::new(),
                    resolution_state_binding: None,
                    diagnostic:
                        "Symlink-ul baseline a fost schimbat sau eliminat; manual review obligatoriu."
                            .into(),
                })
            }
        }
        WalSymlinkBefore::Absent => match observed {
            None if parent_was_missing => Ok(SymlinkRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::PartialNamespaceCreation,
                automatic_action: None,
                available_resolution_actions: Vec::new(),
                resolution_state_binding: None,
                diagnostic:
                    "Părinții au apărut după plan, dar identitatea creatorului nu poate fi demonstrată; manual review obligatoriu."
                        .into(),
            }),
            None => Ok(SymlinkRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::Conflict,
                automatic_action: None,
                available_resolution_actions: Vec::new(),
                resolution_state_binding: None,
                diagnostic: format!(
                    "Parentul este baseline, iar leaf-ul symlink lipsește în faza {phase:?}; symlinkat rulează înaintea primei tranziții de fază legacy, deci un efect creat și apoi eliminat nu poate fi exclus. WAL-ul rămâne hot."
                ),
            }),
            Some(observed)
                if observed.link_target_hex == evidence.desired_link_target_hex =>
            {
                Ok(SymlinkRecoveryAssessment {
                    classification: WriteAuthorityRecoveryClassification::EffectCommitted,
                    automatic_action: None,
                    available_resolution_actions: Vec::new(),
                    resolution_state_binding: None,
                    diagnostic:
                        "Symlink-ul conține literalul dorit, dar inode-ul post-create nu există în recordul immutable; manual review obligatoriu."
                            .into(),
                })
            }
            Some(_) => Ok(SymlinkRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::Conflict,
                automatic_action: None,
                available_resolution_actions: Vec::new(),
                resolution_state_binding: None,
                diagnostic:
                    "Leaf-ul symlink vizibil nu conține literalul dorit; nu va fi șters automat."
                        .into(),
            }),
        },
    }
}

pub(in crate::kernel::write_authority::capability) fn execute_legacy_symlink_recovery(
    record: &WalRecord,
    phase: WalPhase,
    action: SymlinkRecoveryAction,
) -> Result<(), String> {
    let assessment = classify_legacy_symlink_recovery(record, phase)?;
    if assessment.automatic_action != Some(action) {
        return Err(format!(
            "WriteAuthority symlink recovery CAS a refuzat {action:?}: {}",
            assessment.diagnostic
        ));
    }
    match (phase, action) {
        (WalPhase::Prepared, SymlinkRecoveryAction::ClearNoEffect) => Ok(()),
        _ => Err(format!(
            "WriteAuthority symlink legacy permite automat numai Prepared/ClearNoEffect pentru un symlink baseline existent, nu {phase:?}/{action:?}."
        )),
    }
}

fn capture_recovery_symlink_context(
    record: &WalRecord,
    evidence: &WalSymlinkEvidence,
) -> Result<RecoverySymlinkContext, String> {
    let (authority, parents, target_leaf) = recovery_symlink_inputs(record, evidence)?;
    let mut directory = rustix::io::dup(authority.directory()).map_err(|error| {
        format!("WriteAuthority symlink recovery nu poate duplica boundary: {error}.")
    })?;
    if evidence.parent.existing_prefix_len == 0
        && wal_identity_from_fd(&directory, &record.body.public_label)?
            != evidence.parent.existing_ancestor_identity
    {
        return Err("WriteAuthority symlink recovery authority identity diferă de plan.".into());
    }
    let mut observed_prefix_len = 0_usize;
    for component in &parents {
        match open_directory_strict(&directory, component) {
            Ok(next) => {
                validate_named_directory_identity(
                    &directory,
                    component,
                    &next,
                    &record.body.public_label,
                    "symlink recovery parent",
                )?;
                directory = next;
                observed_prefix_len += 1;
                if observed_prefix_len == evidence.parent.existing_prefix_len
                    && wal_identity_from_fd(&directory, &record.body.public_label)?
                        != evidence.parent.existing_ancestor_identity
                {
                    return Err("WriteAuthority symlink recovery ancestor baseline diferă.".into());
                }
            }
            Err(Errno::NOENT) => {
                return Ok(RecoverySymlinkContext::ParentMissing {
                    observed_prefix_len,
                    planned_existing_prefix_len: evidence.parent.existing_prefix_len,
                });
            }
            Err(error) => {
                return Err(capability_error(
                    &record.body.public_label,
                    &format!("symlink recovery nu poate captura parentul: {error}"),
                ));
            }
        }
    }
    let observed_parent = wal_identity_from_fd(&directory, &record.body.public_label)?;
    if let Some(expected_parent) = &evidence.parent.parent_identity {
        if &observed_parent != expected_parent {
            return Err("WriteAuthority symlink recovery parent identity diferă.".into());
        }
    }
    Ok(RecoverySymlinkContext::Ready {
        directory,
        target_leaf,
        parent_was_missing: evidence.parent.parent_identity.is_none(),
    })
}

fn recovery_symlink_inputs(
    record: &WalRecord,
    evidence: &WalSymlinkEvidence,
) -> Result<(DirectoryAuthority, Vec<OsString>, OsString), String> {
    let boundary_path = decode_path_hex(&record.body.authority.boundary_path_hex)?;
    if !boundary_path.is_absolute() {
        return Err("WriteAuthority symlink recovery refuză boundary non-absolut.".into());
    }
    let authority = capture_directory_authority(
        &boundary_path,
        "write-authority-wal/symlink-recovery-target",
        DirectoryAuthorityScope::RecoveryTarget,
    )?;
    let identity = authority.identity();
    if identity.device != record.body.authority.identity.device
        || identity.inode != record.body.authority.identity.inode
    {
        return Err("WriteAuthority symlink recovery boundary identity diferă.".into());
    }
    let parents = evidence
        .parent
        .relative_components_hex
        .iter()
        .map(|component| decode_component_hex(component))
        .collect::<Result<Vec<_>, _>>()?;
    let target_leaf = decode_component_hex(&evidence.target_leaf_hex)?;
    Ok((authority, parents, target_leaf))
}

fn observe_symlink(
    parent: &OwnedFd,
    leaf: &OsStr,
    public_label: &str,
) -> Result<Option<ObservedSymlink>, String> {
    let Some(before) = leaf_metadata(parent, leaf, public_label)? else {
        return Ok(None);
    };
    if FileType::from_raw_mode(before.st_mode) != FileType::Symlink {
        return Err(capability_error(
            public_label,
            "leaf-ul symlink este ocupat de alt tip de intrare",
        ));
    }
    if before.st_nlink != 1 {
        return Err(capability_error(
            public_label,
            "leaf-ul symlink nu este single-link",
        ));
    }
    let literal = fs::readlinkat(parent, leaf, Vec::new()).map_err(|error| {
        capability_error(
            public_label,
            &format!("literalul symlink nu poate fi citit: {error}"),
        )
    })?;
    let bytes = literal.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_WAL_SYMLINK_TARGET_BYTES || bytes.contains(&0) {
        return Err(capability_error(
            public_label,
            "literalul symlink depășește contractul WAL",
        ));
    }
    let after = fs::statat(parent, leaf, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
        capability_error(
            public_label,
            &format!("symlink-ul nu poate fi reverificat: {error}"),
        )
    })?;
    if !same_stable_leaf_version(&before, &after)
        || version_token_for_stat(&before) != version_token_for_stat(&after)
    {
        return Err(capability_error(
            public_label,
            "symlink-ul s-a schimbat în timpul readlinkat",
        ));
    }
    Ok(Some(ObservedSymlink {
        identity: WalFilesystemIdentity {
            device: before.st_dev,
            inode: before.st_ino,
        },
        version_token: version_token_for_stat(&before),
        link_target_hex: encode_bytes_hex(bytes),
    }))
}

fn observed_matches_before(observed: Option<&ObservedSymlink>, before: &WalSymlinkBefore) -> bool {
    match (observed, before) {
        (None, WalSymlinkBefore::Absent) => true,
        (
            Some(observed),
            WalSymlinkBefore::Exact {
                identity,
                version_token,
                link_target_hex,
                ..
            },
        ) => {
            &observed.identity == identity
                && &observed.version_token == version_token
                && &observed.link_target_hex == link_target_hex
        }
        _ => false,
    }
}

fn validate_symlink_plan_shape(
    lexical: &LexicalTarget,
    source: &Path,
    plan: &SymlinkOperationPlan,
) -> Result<(), String> {
    let (leaf, parents) = lexical
        .relative_components
        .split_last()
        .ok_or_else(|| capability_error(&lexical.public_label, "planul symlink cere un leaf"))?;
    let planned_parents = plan
        .evidence
        .parent
        .relative_components_hex
        .iter()
        .map(|component| decode_component_hex(component))
        .collect::<Result<Vec<_>, _>>()?;
    if planned_parents != parents
        || decode_component_hex(&plan.evidence.target_leaf_hex)? != *leaf
        || plan.evidence.desired_link_target_hex != encode_path_hex(source)
    {
        return Err(capability_error(
            &lexical.public_label,
            "targetul sau literalul symlink diferă de planul WAL",
        ));
    }
    Ok(())
}

fn validate_symlink_literal(source: &Path, public_label: &str) -> Result<(), String> {
    let bytes = source.as_os_str().as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_WAL_SYMLINK_TARGET_BYTES || bytes.contains(&0) {
        return Err(capability_error(
            public_label,
            "literalul symlink este gol, conține NUL sau depășește limita WAL",
        ));
    }
    Ok(())
}
