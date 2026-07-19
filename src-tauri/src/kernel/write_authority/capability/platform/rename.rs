use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
struct ObservedRenameLeaf {
    identity: WalFilesystemIdentity,
    kind: WalRenameLeafKind,
    size: u64,
    mtime_seconds: i64,
    mtime_nanoseconds: u64,
    raw_mode: u32,
    link_count: u64,
    version_token: String,
}

enum RecoveryRenameParent {
    Missing {
        observed_prefix_len: usize,
        planned_existing_prefix_len: usize,
    },
    Ready {
        directory: OwnedFd,
        leaf: OsString,
        parent_was_missing: bool,
    },
}

pub(in crate::kernel::write_authority::capability) fn plan_rename(
    source: &WriteTarget,
    destination: &WriteTarget,
) -> Result<RenameOperationPlan, String> {
    if !matches!(source.expected_leaf, ExpectedLeaf::Present(_)) {
        return Err(capability_error(
            &source.public_label,
            "planul rename WAL cere source baseline Present",
        ));
    }
    if destination.expected_leaf != ExpectedLeaf::Absent {
        return Err(capability_error(
            &destination.public_label,
            "planul rename WAL cere destination baseline Absent",
        ));
    }

    let source_lexical = lexical_target(source, false)?;
    let destination_lexical = lexical_target(destination, false)?;
    let source_authority = source_lexical.authority.as_ref().ok_or_else(|| {
        capability_error(
            &source_lexical.public_label,
            "planul rename WAL cere source authority sigilată",
        )
    })?;
    let destination_authority = destination_lexical.authority.as_ref().ok_or_else(|| {
        capability_error(
            &destination_lexical.public_label,
            "planul rename WAL cere destination authority sigilată",
        )
    })?;

    let (source_parent_evidence, source_leaf, source_parent) =
        plan_parent_evidence(&source_lexical, true)?;
    let source_parent = source_parent.expect("source parent required by plan_parent_evidence");
    let (destination_parent_evidence, destination_leaf, destination_existing_parent) =
        plan_parent_evidence(&destination_lexical, false)?;

    if source_authority.same_authority(destination_authority)
        && source_lexical.relative_components == destination_lexical.relative_components
    {
        return Err(capability_error(
            &source_lexical.public_label,
            "rename source și destination sunt aceeași intrare",
        ));
    }

    let source_handle = fs::openat(
        &source_parent,
        &source_leaf,
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| {
        capability_error(
            &source_lexical.public_label,
            &format!("planul rename nu poate captura source leaf: {error}"),
        )
    })?;
    let source_before = fs::fstat(&source_handle).map_err(|error| {
        capability_error(
            &source_lexical.public_label,
            &format!("planul rename nu poate citi source metadata: {error}"),
        )
    })?;
    validate_named_identity(
        &source_parent,
        &source_leaf,
        &source_before,
        &source_lexical.public_label,
        "rename plan source",
    )?;

    let expected = match &source.expected_leaf {
        ExpectedLeaf::Present(expected) => expected,
        _ => unreachable!(),
    };
    if version_token_for_stat(&source_before) != expected.version_token {
        return Err(capability_error(
            &source_lexical.public_label,
            "source rename diferă de disk baseline înainte de WAL prepare",
        ));
    }

    let source_kind = rename_kind(&source_before, &source_lexical.public_label)?;
    if source_kind == WalRenameLeafKind::Directory
        && same_authority_path_prefix(&source_lexical, &destination_lexical)
    {
        return Err(capability_error(
            &source_lexical.public_label,
            "rename refuză mutarea unui director în propriul subarbore",
        ));
    }

    let mut source_content = None;
    let mut source_directory = None;
    match source_kind {
        WalRenameLeafKind::Regular => {
            if expected.tree_fingerprint.is_some() {
                return Err(capability_error(
                    &source_lexical.public_label,
                    "source file rename nu acceptă tree fingerprint",
                ));
            }
            if expected.content_hash.is_some() {
                let descriptor = fs::openat(
                    &source_parent,
                    &source_leaf,
                    OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|error| {
                    capability_error(
                        &source_lexical.public_label,
                        &format!("source content rename nu poate fi deschis: {error}"),
                    )
                })?;
                let mut file = File::from(descriptor);
                let stat = fs::fstat(&file).map_err(|error| {
                    capability_error(
                        &source_lexical.public_label,
                        &format!("source content rename nu poate fi verificat: {error}"),
                    )
                })?;
                if !same_file_identity(&source_before, &stat) {
                    return Err(capability_error(
                        &source_lexical.public_label,
                        "source content rename este alt inode decât handle-ul planificat",
                    ));
                }
                validate_expected_content(
                    &mut file,
                    &stat,
                    expected.content_hash.as_deref(),
                    &source_lexical.public_label,
                    "rename WAL plan content",
                )?;
                source_content = Some(file);
            }
        }
        WalRenameLeafKind::Directory => {
            let expected_tree = expected.tree_fingerprint.as_deref().ok_or_else(|| {
                capability_error(
                    &source_lexical.public_label,
                    "source directory rename cere tree fingerprint",
                )
            })?;
            if expected.content_hash.is_some() {
                return Err(capability_error(
                    &source_lexical.public_label,
                    "source directory rename nu acceptă content hash",
                ));
            }
            let directory =
                open_directory_strict(&source_parent, &source_leaf).map_err(|error| {
                    capability_error(
                        &source_lexical.public_label,
                        &format!("source directory rename nu poate fi capturat: {error}"),
                    )
                })?;
            validate_open_directory_identity(
                &directory,
                &source_before,
                &source_lexical.public_label,
                "rename WAL plan tree",
            )?;
            let observed_tree =
                fingerprint_directory_tree(&directory, &source_lexical.public_label)?;
            if observed_tree != expected_tree {
                return Err(capability_error(
                    &source_lexical.public_label,
                    "source directory tree diferă de disk baseline înainte de WAL prepare",
                ));
            }
            source_directory = Some(directory);
        }
        WalRenameLeafKind::Symlink => {
            if expected.content_hash.is_some() || expected.tree_fingerprint.is_some() {
                return Err(capability_error(
                    &source_lexical.public_label,
                    "source symlink rename nu acceptă payload hash/fingerprint",
                ));
            }
        }
    }

    let source_after = fs::fstat(&source_handle).map_err(|error| {
        capability_error(
            &source_lexical.public_label,
            &format!("source rename nu poate fi reverificat după plan: {error}"),
        )
    })?;
    if version_token_for_stat(&source_before) != version_token_for_stat(&source_after) {
        return Err(capability_error(
            &source_lexical.public_label,
            "source rename s-a schimbat în timpul planificării WAL",
        ));
    }
    validate_named_identity(
        &source_parent,
        &source_leaf,
        &source_after,
        &source_lexical.public_label,
        "rename plan source final",
    )?;

    if let Some(parent) = destination_existing_parent.as_ref() {
        if leaf_metadata(parent, &destination_leaf, &destination_lexical.public_label)?.is_some() {
            return Err(capability_error(
                &destination_lexical.public_label,
                "destination rename nu mai este absentă la planificare",
            ));
        }
    }
    if source_parent_evidence.existing_ancestor_identity.device
        != destination_parent_evidence
            .existing_ancestor_identity
            .device
    {
        return Err(capability_error(
            &destination_lexical.public_label,
            "rename cross-filesystem (EXDEV) este refuzat înainte de orice efect",
        ));
    }

    Ok(RenameOperationPlan {
        evidence: WalRenameEvidence {
            source_parent: source_parent_evidence,
            source_leaf_hex: encode_component_hex(&source_leaf),
            source: rename_source_evidence(
                &source_after,
                source_kind,
                expected.content_hash.clone(),
                expected.tree_fingerprint.clone(),
            )?,
            destination_authority: wal_authority_evidence(destination_authority),
            destination_parent: destination_parent_evidence,
            destination_leaf_hex: encode_component_hex(&destination_leaf),
        },
        source_handle,
        source_content,
        source_directory,
    })
}

pub(in crate::kernel::write_authority::capability) fn rename_entry_wal(
    source: &WriteTarget,
    destination: &WriteTarget,
    mut plan: RenameOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    let source_lexical = lexical_target(source, false)?;
    let destination_lexical = lexical_target(destination, false)?;
    validate_rename_plan_shape(
        &source_lexical,
        &destination_lexical,
        source,
        destination,
        &plan,
    )?;

    let source_parent =
        match capture_parent_from_wal_evidence(&source_lexical, &plan.evidence.source_parent) {
            Ok(parent) => parent,
            Err(error) => return error.into_operation_result(),
        };
    run_test_hook(CapabilityTestStage::AfterRenameSourceParentCaptured);
    validate_runtime_source(
        &source_parent.directory,
        &source_parent.leaf,
        &mut plan,
        true,
        &source_lexical.public_label,
    )?;

    let destination_parent = match capture_parent_from_wal_evidence(
        &destination_lexical,
        &plan.evidence.destination_parent,
    ) {
        Ok(parent) => parent,
        Err(error) => return error.into_operation_result(),
    };
    let auxiliary_created = destination_parent.created_ancestors;
    let recovery = |diagnostic: String| {
        wal_recovery_effect(
            0,
            &source_lexical.public_label,
            format!("{diagnostic} Rename WAL rămâne pentru recovery; nu repeta operația."),
        )
    };

    if let Err(error) = validate_destination_absent(
        &destination_parent.directory,
        &destination_parent.leaf,
        &destination_lexical.public_label,
    ) {
        return if auxiliary_created {
            Ok(recovery(error))
        } else {
            Err(error)
        };
    }
    let source_parent_identity =
        wal_identity_from_fd(&source_parent.directory, &source_lexical.public_label)?;
    let destination_parent_identity = wal_identity_from_fd(
        &destination_parent.directory,
        &destination_lexical.public_label,
    )?;
    if source_parent_identity.device != destination_parent_identity.device {
        let error = capability_error(
            &destination_lexical.public_label,
            "rename cross-filesystem (EXDEV) a fost blocat înainte de renameat2",
        );
        return if auxiliary_created {
            Ok(recovery(error))
        } else {
            Err(error)
        };
    }

    if auxiliary_created {
        if let Err(error) = guard.mark_auxiliary_durable() {
            return Ok(recovery(error));
        }
    }
    if let Err(error) = validate_runtime_source(
        &source_parent.directory,
        &source_parent.leaf,
        &mut plan,
        true,
        &source_lexical.public_label,
    ) {
        return if auxiliary_created {
            Ok(recovery(error))
        } else {
            Err(error)
        };
    }
    if let Err(error) = validate_destination_absent(
        &destination_parent.directory,
        &destination_parent.leaf,
        &destination_lexical.public_label,
    ) {
        return if auxiliary_created {
            Ok(recovery(error))
        } else {
            Err(error)
        };
    }
    if !auxiliary_created {
        if let Err(error) = guard.mark_auxiliary_durable() {
            return Ok(recovery(error));
        }
    }

    if let Err(error) = validate_runtime_public_paths(
        &source_lexical,
        &destination_lexical,
        &source_parent,
        &destination_parent,
        &plan.evidence.source,
        false,
    ) {
        return Ok(recovery(format!(
            "Rename pre-commit full-path CAS a eșuat: {error}"
        )));
    }

    run_test_hook(CapabilityTestStage::BeforeRename);
    if let Err(error) = fs::renameat_with(
        &source_parent.directory,
        &source_parent.leaf,
        &destination_parent.directory,
        &destination_parent.leaf,
        RenameFlags::NOREPLACE,
    ) {
        return Ok(recovery(capability_error(
            &source_lexical.public_label,
            &format!(
                "rename WAL către {} a fost refuzat fără suprascriere: {error}",
                destination_lexical.public_label
            ),
        )));
    }
    if let Err(error) = guard.mark_effect_visible() {
        return Ok(recovery(error));
    }

    if let Err(conflict) = validate_runtime_moved_source(
        &source_parent,
        &destination_parent,
        &mut plan,
        &source_lexical.public_label,
        &destination_lexical.public_label,
    ) {
        let rollback = rollback_runtime_rename(
            &source_lexical,
            &destination_lexical,
            &source_parent,
            &destination_parent,
            &plan.evidence.source,
        );
        return Ok(recovery(match rollback {
            Ok(()) => format!("{conflict} Rename-ul a fost rollback-uit durabil."),
            Err(rollback_error) => format!("{conflict} {rollback_error}"),
        }));
    }

    if let Err(error) = sync_rename_parents(
        &source_parent.directory,
        &destination_parent.directory,
        &source_lexical.public_label,
        &destination_lexical.public_label,
    ) {
        return Ok(recovery(error));
    }
    if let Err(error) = validate_runtime_public_paths(
        &source_lexical,
        &destination_lexical,
        &source_parent,
        &destination_parent,
        &plan.evidence.source,
        true,
    ) {
        return Ok(recovery(error));
    }
    if let Err(error) = guard.mark_target_durable() {
        return Ok(recovery(error));
    }
    if let Err(error) = validate_runtime_public_paths(
        &source_lexical,
        &destination_lexical,
        &source_parent,
        &destination_parent,
        &plan.evidence.source,
        true,
    ) {
        return Ok(recovery(error));
    }

    Ok(CapabilityEffect::changed(0))
}

pub(in crate::kernel::write_authority::capability) fn classify_rename_recovery(
    record: &WalRecord,
    phase: WalPhase,
) -> Result<RenameRecoveryAssessment, String> {
    let WalOperationEvidence::Rename(evidence) = &record.body.operation_evidence else {
        return Err("WriteAuthority rename recovery a primit altă familie.".into());
    };
    let source_parent = capture_recovery_parent(
        &record.body.authority,
        &evidence.source_parent,
        &evidence.source_leaf_hex,
        &record.body.public_label,
    )?;
    let RecoveryRenameParent::Ready {
        directory: source_directory,
        leaf: source_leaf,
        parent_was_missing: false,
    } = source_parent
    else {
        return Ok(RenameRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::Conflict,
            automatic_action: None,
            diagnostic: "Source parent rename baseline lipsește sau nu mai are identitatea WAL."
                .into(),
        });
    };
    let source = observe_rename_leaf(&source_directory, &source_leaf, &record.body.public_label)?;

    let destination_parent = capture_recovery_parent(
        &evidence.destination_authority,
        &evidence.destination_parent,
        &evidence.destination_leaf_hex,
        &record.body.public_label,
    )?;
    let (destination, destination_namespace_is_baseline, destination_ready) =
        match destination_parent {
            RecoveryRenameParent::Missing {
                observed_prefix_len,
                planned_existing_prefix_len,
            } => {
                if observed_prefix_len < planned_existing_prefix_len {
                    return Ok(RenameRecoveryAssessment {
                        classification: WriteAuthorityRecoveryClassification::Conflict,
                        automatic_action: None,
                        diagnostic:
                            "Un destination ancestor rename care exista în baseline lipsește."
                                .into(),
                    });
                }
                (
                    None,
                    observed_prefix_len == planned_existing_prefix_len,
                    None,
                )
            }
            RecoveryRenameParent::Ready {
                directory,
                leaf,
                parent_was_missing,
            } => {
                let observed = observe_rename_leaf(&directory, &leaf, &record.body.public_label)?;
                (observed, !parent_was_missing, Some((directory, leaf)))
            }
        };

    let source_is_before = source
        .as_ref()
        .is_some_and(|observed| observed_matches_rename_before(observed, &evidence.source));
    let source_is_stable = source
        .as_ref()
        .is_some_and(|observed| observed_matches_rename_moved(observed, &evidence.source));
    let destination_is_moved = destination
        .as_ref()
        .is_some_and(|observed| observed_matches_rename_moved(observed, &evidence.source));

    if source_is_before && destination.is_none() {
        if destination_namespace_is_baseline {
            return Ok(RenameRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::NoEffect,
                automatic_action: (phase == WalPhase::Prepared)
                    .then_some(RenameRecoveryAction::ClearNoEffect),
                diagnostic: if phase == WalPhase::Prepared {
                    "Source rename este baseline, destination este absentă și Prepared permite clear no-effect."
                        .into()
                } else {
                    format!(
                        "Source rename este baseline și destination absentă, dar faza {phase:?} păstrează WAL-ul pentru review."
                    )
                },
            });
        }
        return Ok(RenameRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::PartialNamespaceCreation,
            automatic_action: None,
            diagnostic:
                "Destination parent rename a apărut după baseline; ancestorii nu sunt eliminați automat."
                    .into(),
        });
    }

    if source.is_none() && destination_is_moved && destination_ready.is_some() {
        return Ok(RenameRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::EffectCommitted,
            automatic_action: Some(RenameRecoveryAction::FinalizeCommitted),
            diagnostic:
                "Source rename este absent, iar destination numește exact inode-ul baseline; fsync idempotent poate finaliza efectul."
                    .into(),
        });
    }

    if source_is_stable && !source_is_before && destination.is_none() {
        return Ok(RenameRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::RollbackCompleted,
            automatic_action: None,
            diagnostic:
                "Source rename pare restaurat după rollback, dar ctime-ul diferă; review manual obligatoriu."
                    .into(),
        });
    }

    Ok(RenameRecoveryAssessment {
        classification: WriteAuthorityRecoveryClassification::Conflict,
        automatic_action: None,
        diagnostic: format!(
            "Oracle-ul rename nu poate atribui sigur namespace-ul (sourceBefore={source_is_before}, sourceStable={source_is_stable}, sourceAbsent={}, destinationMoved={destination_is_moved}, destinationAbsent={}).",
            source.is_none(),
            destination.is_none()
        ),
    })
}

pub(in crate::kernel::write_authority::capability) fn execute_rename_recovery(
    record: &WalRecord,
    phase: WalPhase,
) -> Result<(), String> {
    let assessment = classify_rename_recovery(record, phase)?;
    let action = assessment.automatic_action.ok_or_else(|| {
        format!(
            "WriteAuthority rename recovery CAS nu mai permite acțiune automată: {}",
            assessment.diagnostic
        )
    })?;
    match action {
        RenameRecoveryAction::ClearNoEffect => Ok(()),
        RenameRecoveryAction::FinalizeCommitted => finalize_committed_rename(record, phase),
    }
}

fn finalize_committed_rename(record: &WalRecord, phase: WalPhase) -> Result<(), String> {
    let WalOperationEvidence::Rename(evidence) = &record.body.operation_evidence else {
        return Err("WriteAuthority rename recovery finalize a primit altă familie.".into());
    };
    let source = capture_recovery_parent(
        &record.body.authority,
        &evidence.source_parent,
        &evidence.source_leaf_hex,
        &record.body.public_label,
    )?;
    let destination = capture_recovery_parent(
        &evidence.destination_authority,
        &evidence.destination_parent,
        &evidence.destination_leaf_hex,
        &record.body.public_label,
    )?;
    let (
        RecoveryRenameParent::Ready {
            directory: source_directory,
            leaf: source_leaf,
            ..
        },
        RecoveryRenameParent::Ready {
            directory: destination_directory,
            leaf: destination_leaf,
            ..
        },
    ) = (source, destination)
    else {
        return Err("Rename recovery finalize nu mai poate captura ambii părinți.".into());
    };
    if observe_rename_leaf(&source_directory, &source_leaf, &record.body.public_label)?.is_some()
        || !observe_rename_leaf(
            &destination_directory,
            &destination_leaf,
            &record.body.public_label,
        )?
        .as_ref()
        .is_some_and(|observed| observed_matches_rename_moved(observed, &evidence.source))
    {
        return Err("Rename recovery finalize CAS s-a schimbat înainte de fsync.".into());
    }
    sync_rename_parents(
        &source_directory,
        &destination_directory,
        &record.body.public_label,
        &record.body.public_label,
    )?;
    let after = classify_rename_recovery(record, phase)?;
    if after.classification != WriteAuthorityRecoveryClassification::EffectCommitted {
        return Err(format!(
            "Rename recovery finalize postflight s-a schimbat: {}",
            after.diagnostic
        ));
    }
    Ok(())
}

fn plan_parent_evidence(
    lexical: &LexicalTarget,
    require_complete: bool,
) -> Result<(WalParentEvidence, OsString, Option<OwnedFd>), String> {
    let boundary = capture_existing_boundary(lexical)?.ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "authority root nu există pentru planul rename",
        )
    })?;
    let (leaf, parents) = lexical
        .relative_components
        .split_last()
        .ok_or_else(|| capability_error(&lexical.public_label, "planul rename cere leaf"))?;
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
                    "rename WAL parent",
                )?;
                directory = next;
                existing_prefix_len += 1;
            }
            Err(Errno::NOENT) if !require_complete => break,
            Err(Errno::NOENT) => {
                return Err(capability_error(
                    &lexical.public_label,
                    "source parent rename nu există integral",
                ));
            }
            Err(error) => {
                return Err(capability_error(
                    &lexical.public_label,
                    &format!("un parent rename nu poate fi capturat: {error}"),
                ));
            }
        }
    }
    let ancestor_identity = wal_identity_from_fd(&directory, &lexical.public_label)?;
    let complete = existing_prefix_len == parents.len();
    let parent_identity = complete.then_some(ancestor_identity.clone());
    Ok((
        WalParentEvidence {
            relative_components_hex: parents
                .iter()
                .map(|component| encode_component_hex(component))
                .collect(),
            existing_prefix_len,
            existing_ancestor_identity: ancestor_identity,
            parent_identity,
        },
        leaf.clone(),
        complete.then_some(directory),
    ))
}

fn rename_source_evidence(
    stat: &fs::Stat,
    kind: WalRenameLeafKind,
    content_hash: Option<String>,
    tree_fingerprint: Option<String>,
) -> Result<WalRenameSourceEvidence, String> {
    Ok(WalRenameSourceEvidence {
        identity: WalFilesystemIdentity {
            device: stat.st_dev,
            inode: stat.st_ino,
        },
        kind,
        size: u64::try_from(stat.st_size)
            .map_err(|_| "Rename WAL source are dimensiune negativă.".to_string())?,
        mtime_seconds: stat.st_mtime,
        mtime_nanoseconds: stat.st_mtime_nsec,
        raw_mode: stat.st_mode,
        link_count: stat.st_nlink,
        version_token: version_token_for_stat(stat),
        content_hash,
        tree_fingerprint,
    })
}

fn validate_rename_plan_shape(
    source_lexical: &LexicalTarget,
    destination_lexical: &LexicalTarget,
    source: &WriteTarget,
    destination: &WriteTarget,
    plan: &RenameOperationPlan,
) -> Result<(), String> {
    let (source_leaf, source_parents) = source_lexical
        .relative_components
        .split_last()
        .ok_or_else(|| capability_error(&source_lexical.public_label, "rename source fără leaf"))?;
    let (destination_leaf, destination_parents) = destination_lexical
        .relative_components
        .split_last()
        .ok_or_else(|| {
            capability_error(
                &destination_lexical.public_label,
                "rename destination fără leaf",
            )
        })?;
    let planned_source_parents = decode_parent_components(&plan.evidence.source_parent)?;
    let planned_destination_parents = decode_parent_components(&plan.evidence.destination_parent)?;
    let destination_authority = destination_lexical.authority.as_ref().ok_or_else(|| {
        capability_error(
            &destination_lexical.public_label,
            "rename destination authority lipsește",
        )
    })?;
    let expected = match &source.expected_leaf {
        ExpectedLeaf::Present(expected) => expected,
        _ => {
            return Err(capability_error(
                &source_lexical.public_label,
                "rename source nu mai declară baseline Present",
            ));
        }
    };
    if destination.expected_leaf != ExpectedLeaf::Absent
        || planned_source_parents != source_parents
        || planned_destination_parents != destination_parents
        || decode_component_hex(&plan.evidence.source_leaf_hex)? != *source_leaf
        || decode_component_hex(&plan.evidence.destination_leaf_hex)? != *destination_leaf
        || plan.evidence.destination_authority != wal_authority_evidence(destination_authority)
        || plan.evidence.source.version_token != expected.version_token
        || plan.evidence.source.content_hash != expected.content_hash
        || plan.evidence.source.tree_fingerprint != expected.tree_fingerprint
    {
        return Err(capability_error(
            &source_lexical.public_label,
            "planul rename WAL nu corespunde perechii source/destination executate",
        ));
    }
    Ok(())
}

fn validate_runtime_source(
    parent: &OwnedFd,
    leaf: &OsStr,
    plan: &mut RenameOperationPlan,
    require_exact_pre_version: bool,
    public_label: &str,
) -> Result<(), String> {
    let handle_stat = fs::fstat(&plan.source_handle).map_err(|error| {
        capability_error(
            public_label,
            &format!("rename source handle fstat a eșuat: {error}"),
        )
    })?;
    let observed = observed_from_stat(&handle_stat, public_label)?;
    let matches = if require_exact_pre_version {
        observed_matches_rename_before(&observed, &plan.evidence.source)
    } else {
        observed_matches_rename_moved(&observed, &plan.evidence.source)
    };
    if !matches {
        return Err(capability_error(
            public_label,
            "rename source handle diferă de snapshotul WAL",
        ));
    }
    validate_named_identity(
        parent,
        leaf,
        &handle_stat,
        public_label,
        "rename source named",
    )
}

fn validate_runtime_moved_source(
    source_parent: &CapturedParent,
    destination_parent: &CapturedParent,
    plan: &mut RenameOperationPlan,
    source_label: &str,
    destination_label: &str,
) -> Result<(), String> {
    if leaf_metadata(&source_parent.directory, &source_parent.leaf, source_label)?.is_some() {
        return Err(capability_error(
            source_label,
            "source name este încă prezent după rename",
        ));
    }
    let moved = leaf_metadata(
        &destination_parent.directory,
        &destination_parent.leaf,
        destination_label,
    )?
    .ok_or_else(|| capability_error(destination_label, "destination lipsește după rename"))?;
    let observed = observed_from_stat(&moved, destination_label)?;
    if !observed_matches_rename_moved(&observed, &plan.evidence.source) {
        return Err(capability_error(
            destination_label,
            "destination rename nu este inode-ul source planificat",
        ));
    }
    let handle_after = fs::fstat(&plan.source_handle).map_err(|error| {
        capability_error(
            source_label,
            &format!("source handle nu poate fi verificat după rename: {error}"),
        )
    })?;
    if !same_file_identity(&moved, &handle_after)
        || !observed_matches_rename_moved(
            &observed_from_stat(&handle_after, source_label)?,
            &plan.evidence.source,
        )
    {
        return Err(capability_error(
            source_label,
            "source handle s-a schimbat în timpul rename-ului",
        ));
    }
    if let Some(file) = plan.source_content.as_mut() {
        validate_expected_content(
            file,
            &handle_after,
            plan.evidence.source.content_hash.as_deref(),
            source_label,
            "rename WAL post-commit content",
        )?;
    }
    if let (Some(directory), Some(expected_tree)) = (
        plan.source_directory.as_ref(),
        plan.evidence.source.tree_fingerprint.as_deref(),
    ) {
        let observed_tree = fingerprint_directory_tree(directory, source_label)?;
        if observed_tree != expected_tree {
            return Err(capability_error(
                source_label,
                "source tree s-a schimbat în timpul rename-ului",
            ));
        }
    }
    let handle_final = fs::fstat(&plan.source_handle).map_err(|error| {
        capability_error(
            source_label,
            &format!("source handle final fstat a eșuat: {error}"),
        )
    })?;
    if version_token_for_stat(&handle_after) != version_token_for_stat(&handle_final) {
        return Err(capability_error(
            source_label,
            "source a suferit o schimbare concurentă în postflight-ul rename",
        ));
    }
    Ok(())
}

fn rollback_runtime_rename(
    source_lexical: &LexicalTarget,
    destination_lexical: &LexicalTarget,
    source_parent: &CapturedParent,
    destination_parent: &CapturedParent,
    source_evidence: &WalRenameSourceEvidence,
) -> Result<(), String> {
    fs::renameat_with(
        &destination_parent.directory,
        &destination_parent.leaf,
        &source_parent.directory,
        &source_parent.leaf,
        RenameFlags::NOREPLACE,
    )
    .map_err(|error| {
        capability_error(
            &source_lexical.public_label,
            &format!("rollback rename WAL a eșuat: {error}"),
        )
    })?;
    sync_rename_parents(
        &source_parent.directory,
        &destination_parent.directory,
        &source_lexical.public_label,
        &destination_lexical.public_label,
    )?;
    validate_runtime_public_paths(
        source_lexical,
        destination_lexical,
        source_parent,
        destination_parent,
        source_evidence,
        false,
    )
}

fn validate_runtime_public_paths(
    source_lexical: &LexicalTarget,
    destination_lexical: &LexicalTarget,
    source_parent: &CapturedParent,
    destination_parent: &CapturedParent,
    source_evidence: &WalRenameSourceEvidence,
    expect_moved: bool,
) -> Result<(), String> {
    let recaptured_source = capture_existing_target_parent(source_lexical)?.ok_or_else(|| {
        capability_error(
            &source_lexical.public_label,
            "rename postflight nu poate recaptura source parent public",
        )
    })?;
    let recaptured_destination =
        capture_existing_target_parent(destination_lexical)?.ok_or_else(|| {
            capability_error(
                &destination_lexical.public_label,
                "rename postflight nu poate recaptura destination parent public",
            )
        })?;
    if wal_identity_from_fd(&recaptured_source.directory, &source_lexical.public_label)?
        != wal_identity_from_fd(&source_parent.directory, &source_lexical.public_label)?
        || wal_identity_from_fd(
            &recaptured_destination.directory,
            &destination_lexical.public_label,
        )? != wal_identity_from_fd(
            &destination_parent.directory,
            &destination_lexical.public_label,
        )?
    {
        return Err(capability_error(
            &source_lexical.public_label,
            "rename postflight path-ul public nu mai numește părinții sincronizați",
        ));
    }
    let source = observe_rename_leaf(
        &recaptured_source.directory,
        &recaptured_source.leaf,
        &source_lexical.public_label,
    )?;
    let destination = observe_rename_leaf(
        &recaptured_destination.directory,
        &recaptured_destination.leaf,
        &destination_lexical.public_label,
    )?;
    let valid = if expect_moved {
        source.is_none()
            && destination
                .as_ref()
                .is_some_and(|observed| observed_matches_rename_moved(observed, source_evidence))
    } else {
        destination.is_none()
            && source
                .as_ref()
                .is_some_and(|observed| observed_matches_rename_moved(observed, source_evidence))
    };
    if !valid {
        return Err(capability_error(
            &source_lexical.public_label,
            "rename postflight public source/destination diferă de efectul planificat",
        ));
    }
    Ok(())
}

fn capture_recovery_parent(
    authority_evidence: &WalAuthorityEvidence,
    parent_evidence: &WalParentEvidence,
    leaf_hex: &str,
    public_label: &str,
) -> Result<RecoveryRenameParent, String> {
    let boundary_path = decode_path_hex(&authority_evidence.boundary_path_hex)?;
    let authority = capture_directory_authority(
        &boundary_path,
        "write-authority-wal/rename-recovery-target",
        DirectoryAuthorityScope::RecoveryTarget,
    )?;
    if authority.identity().device != authority_evidence.identity.device
        || authority.identity().inode != authority_evidence.identity.inode
    {
        return Err(capability_error(
            public_label,
            "rename recovery authority identity diferă de WAL",
        ));
    }
    let parents = decode_parent_components(parent_evidence)?;
    let leaf = decode_component_hex(leaf_hex)?;
    let mut directory = rustix::io::dup(authority.directory()).map_err(|error| {
        capability_error(
            public_label,
            &format!("rename recovery nu poate duplica authority: {error}"),
        )
    })?;
    let mut observed_prefix_len = 0_usize;
    for component in &parents {
        match open_directory_strict(&directory, component) {
            Ok(next) => {
                directory = next;
                observed_prefix_len += 1;
            }
            Err(Errno::NOENT) => {
                if observed_prefix_len == parent_evidence.existing_prefix_len {
                    let observed = wal_identity_from_fd(&directory, public_label)?;
                    if observed != parent_evidence.existing_ancestor_identity {
                        return Err(capability_error(
                            public_label,
                            "rename recovery absent frontier ancestor diferă de WAL",
                        ));
                    }
                }
                return Ok(RecoveryRenameParent::Missing {
                    observed_prefix_len,
                    planned_existing_prefix_len: parent_evidence.existing_prefix_len,
                });
            }
            Err(error) => {
                return Err(capability_error(
                    public_label,
                    &format!("rename recovery parent capture a eșuat: {error}"),
                ));
            }
        }
    }
    let observed_parent = wal_identity_from_fd(&directory, public_label)?;
    if let Some(expected_parent) = &parent_evidence.parent_identity {
        if &observed_parent != expected_parent {
            return Err(capability_error(
                public_label,
                "rename recovery parent identity diferă de WAL",
            ));
        }
    }
    Ok(RecoveryRenameParent::Ready {
        directory,
        leaf,
        parent_was_missing: parent_evidence.parent_identity.is_none(),
    })
}

fn observe_rename_leaf(
    parent: &OwnedFd,
    leaf: &OsStr,
    public_label: &str,
) -> Result<Option<ObservedRenameLeaf>, String> {
    leaf_metadata(parent, leaf, public_label)?
        .map(|stat| observed_from_stat(&stat, public_label))
        .transpose()
}

fn observed_from_stat(stat: &fs::Stat, public_label: &str) -> Result<ObservedRenameLeaf, String> {
    Ok(ObservedRenameLeaf {
        identity: WalFilesystemIdentity {
            device: stat.st_dev,
            inode: stat.st_ino,
        },
        kind: rename_kind(stat, public_label)?,
        size: u64::try_from(stat.st_size)
            .map_err(|_| capability_error(public_label, "rename leaf are dimensiune negativă"))?,
        mtime_seconds: stat.st_mtime,
        mtime_nanoseconds: stat.st_mtime_nsec,
        raw_mode: stat.st_mode,
        link_count: stat.st_nlink,
        version_token: version_token_for_stat(stat),
    })
}

fn observed_matches_rename_before(
    observed: &ObservedRenameLeaf,
    expected: &WalRenameSourceEvidence,
) -> bool {
    observed_matches_rename_moved(observed, expected)
        && observed.version_token == expected.version_token
}

fn observed_matches_rename_moved(
    observed: &ObservedRenameLeaf,
    expected: &WalRenameSourceEvidence,
) -> bool {
    observed.identity == expected.identity
        && observed.kind == expected.kind
        && observed.size == expected.size
        && observed.mtime_seconds == expected.mtime_seconds
        && observed.mtime_nanoseconds == expected.mtime_nanoseconds
        && observed.raw_mode == expected.raw_mode
        && observed.link_count == expected.link_count
}

fn validate_named_identity(
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

fn validate_destination_absent(
    parent: &OwnedFd,
    leaf: &OsStr,
    public_label: &str,
) -> Result<(), String> {
    if leaf_metadata(parent, leaf, public_label)?.is_some() {
        return Err(capability_error(
            public_label,
            "destination rename nu mai este absentă",
        ));
    }
    Ok(())
}

fn sync_rename_parents(
    source_parent: &OwnedFd,
    destination_parent: &OwnedFd,
    source_label: &str,
    destination_label: &str,
) -> Result<(), String> {
    sync_directory(source_parent, source_label)?;
    let source_identity = wal_identity_from_fd(source_parent, source_label)?;
    let destination_identity = wal_identity_from_fd(destination_parent, destination_label)?;
    if source_identity != destination_identity {
        sync_directory(destination_parent, destination_label)?;
    }
    Ok(())
}

fn rename_kind(stat: &fs::Stat, public_label: &str) -> Result<WalRenameLeafKind, String> {
    match FileType::from_raw_mode(stat.st_mode) {
        FileType::RegularFile => Ok(WalRenameLeafKind::Regular),
        FileType::Directory => Ok(WalRenameLeafKind::Directory),
        FileType::Symlink => Ok(WalRenameLeafKind::Symlink),
        _ => Err(capability_error(
            public_label,
            "rename WAL acceptă numai regular file, directory sau symlink",
        )),
    }
}

fn same_authority_path_prefix(source: &LexicalTarget, destination: &LexicalTarget) -> bool {
    source
        .authority
        .as_ref()
        .zip(destination.authority.as_ref())
        .is_some_and(|(source_authority, destination_authority)| {
            source_authority.same_authority(destination_authority)
                && destination
                    .relative_components
                    .starts_with(&source.relative_components)
        })
}

fn decode_parent_components(evidence: &WalParentEvidence) -> Result<Vec<OsString>, String> {
    evidence
        .relative_components_hex
        .iter()
        .map(|component| decode_component_hex(component))
        .collect()
}
