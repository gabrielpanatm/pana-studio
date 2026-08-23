use super::{
    capability_error, capture_directory_authority, capture_wal_leaf_evidence, decode_bytes_hex,
    decode_component_hex, decode_path_hex, fs, leaf_metadata, open_directory_strict,
    same_file_identity, sha256_bytes, sync_directory, validate_named_directory_identity,
    validate_named_file_identity, validate_regular_single_link, version_token_for_stat,
    wal_evidence_from_open_file, wal_identity_from_fd, AppendRecoveryAction,
    AppendRecoveryAssessment, AtFlags, AtomicRecoveryAction, AtomicRecoveryAssessment,
    CapabilityEffect, DirectoryAuthority, DirectoryAuthorityScope, DirectoryRecoveryAction,
    DirectoryRecoveryAssessment, Errno, ExpectedLeaf, File, FileType, Mode, OFlags, OsStr,
    OsString, OwnedFd, RecoveryReadBudget, SeekFrom, WalAppendBefore, WalAppendEvidence,
    WalAtomicFileEvidence, WalDirectoryEvidence, WalLeafEvidence, WalOperationEvidence, WalPhase,
    WalRecord, WriteAuthorityRecoveryClassification, WriteAuthorityRecoveryResolutionAction,
};
use std::io::{Read, Seek};

pub(super) fn wal_recovery_effect(
    bytes_written: u64,
    public_label: &str,
    diagnostic: impl Into<String>,
) -> CapabilityEffect {
    CapabilityEffect::recovery_required(
        bytes_written,
        capability_error(
            public_label,
            &format!(
                "{} Recordul WAL rămâne hot; nu repeta operația automat.",
                diagnostic.into()
            ),
        ),
    )
}

pub(super) enum RecoveryAtomicContext {
    ParentMissing {
        existing_components: usize,
        planned_existing_components: usize,
    },
    Ready {
        directory: OwnedFd,
        target_leaf: OsString,
        temp_leaf: OsString,
        parent_was_missing: bool,
    },
}

pub(super) enum RecoveryAppendContext {
    ParentMissing {
        existing_components: usize,
        planned_existing_components: usize,
    },
    Ready {
        directory: OwnedFd,
        target_leaf: OsString,
        parent_was_missing: bool,
    },
}

pub(super) enum AppendSuffixState {
    Complete,
    PartialExact,
    Conflict(String),
}

pub(in crate::kernel::write_authority::capability) fn classify_atomic_recovery(
    record: &WalRecord,
    phase: WalPhase,
    read_budget: &mut RecoveryReadBudget,
) -> Result<AtomicRecoveryAssessment, String> {
    let WalOperationEvidence::AtomicFile(evidence) = &record.body.operation_evidence else {
        return Err("WriteAuthority WAL atomic classifier a primit altă familie.".into());
    };
    let context = capture_recovery_atomic_context(record, evidence)?;
    let RecoveryAtomicContext::Ready {
        directory,
        target_leaf,
        temp_leaf,
        parent_was_missing,
    } = context
    else {
        let RecoveryAtomicContext::ParentMissing {
            existing_components,
            planned_existing_components,
        } = context
        else {
            unreachable!()
        };
        if phase == WalPhase::Prepared && existing_components == planned_existing_components {
            return Ok(AtomicRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::NoEffect,
                automatic_action: Some(AtomicRecoveryAction::ClearNoEffect),
                diagnostic:
                    "Parentul target este încă absent exact de la frontiera planificată; niciun efect atomic nu este vizibil."
                        .into(),
            });
        }
        return Ok(AtomicRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::Conflict,
            automatic_action: None,
            diagnostic: format!(
                "AtomicFile {phase:?} nu poate atribui un parent absent/parțial (observedPrefix={existing_components}, plannedPrefix={planned_existing_components}); numai Prepared exact no-effect se elimină automat."
            ),
        });
    };

    let target = observe_recovery_leaf(
        &directory,
        &target_leaf,
        &record.body.public_label,
        "target",
        read_budget,
    )?;
    let temp = observe_recovery_leaf(
        &directory,
        &temp_leaf,
        &record.body.public_label,
        "temp",
        read_budget,
    )?;
    let target_is_before = target == evidence.before;
    let target_is_new = leaf_matches_new(&target, evidence);
    let temp_is_absent = matches!(temp, WalLeafEvidence::Absent);
    let temp_is_new = leaf_matches_new(&temp, evidence);
    let temp_is_old = evidence.replace && leaf_matches_relocated_before(&temp, &evidence.before);

    if phase == WalPhase::Prepared && !parent_was_missing && target_is_before && temp_is_absent {
        return Ok(AtomicRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::NoEffect,
            automatic_action: Some(AtomicRecoveryAction::ClearNoEffect),
            diagnostic:
                "AtomicFile Prepared este exact baseline, iar temp-ul lipsește; clear no-effect este singura acțiune automată legacy."
                    .into(),
        });
    }

    if phase == WalPhase::Prepared {
        return Ok(AtomicRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::Conflict,
            automatic_action: None,
            diagnostic: format!(
                "AtomicFile Prepared a observat namespace/payload care nu poate fi atribuit operației (parentCreated={parent_was_missing}, targetBefore={target_is_before}, targetNew={target_is_new}, tempAbsent={temp_is_absent}, tempNew={temp_is_new}, tempOld={temp_is_old}); competitorii rămân neatinși."
            ),
        });
    }

    let plausible = match phase {
        WalPhase::AuxiliaryDurable => {
            target_is_before && temp_is_new || target_is_new && (temp_is_absent || temp_is_old)
        }
        WalPhase::EffectVisible => target_is_new && (temp_is_absent || temp_is_old),
        WalPhase::TargetDurable => target_is_new && temp_is_absent,
        WalPhase::Preparing | WalPhase::Prepared => false,
    };
    if !plausible {
        return Ok(AtomicRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::Conflict,
            automatic_action: None,
            diagnostic: format!(
                "AtomicFile {phase:?} a observat o stare incompatibilă cu ordinea runtime (targetBefore={target_is_before}, targetNew={target_is_new}, tempAbsent={temp_is_absent}, tempNew={temp_is_new}, tempOld={temp_is_old}); nicio mutație recovery nu este permisă."
            ),
        });
    }

    if target_is_before && temp_is_new {
        return Ok(AtomicRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::StagedOnly,
            automatic_action: None,
            diagnostic:
                "AtomicFile AuxiliaryDurable are un temp cu forma payloadului, dar protocolul legacy nu persistă identitatea cauzală; temp-ul rămâne hot și nu este șters automat."
                    .into(),
        });
    }
    if target_is_new && temp_is_absent {
        return Ok(AtomicRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::EffectCommitted,
            automatic_action: None,
            diagnostic: format!(
                "AtomicFile {phase:?} are forma payloadului la target, dar protocolul legacy nu persistă identitatea post-create; finalizarea automată este interzisă."
            ),
        });
    }
    if target_is_new && temp_is_old {
        return Ok(AtomicRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::CleanupRequired,
            automatic_action: None,
            diagnostic:
                "AtomicFile are forma exchange-ului, dar cleanup-ul legacy prin unlink nu are CAS identity→effect; baseline-ul izolat rămâne hot și neatins."
                    .into(),
        });
    }

    Ok(AtomicRecoveryAssessment {
        classification: WriteAuthorityRecoveryClassification::Conflict,
        automatic_action: None,
        diagnostic: format!(
            "Oracle-ul atomic nu recunoaște combinația target/temp (targetBefore={target_is_before}, targetNew={target_is_new}, tempAbsent={temp_is_absent}, tempNew={temp_is_new}, tempOld={temp_is_old})."
        ),
    })
}

pub(in crate::kernel::write_authority::capability) fn execute_atomic_recovery(
    record: &WalRecord,
    phase: WalPhase,
    read_budget: &mut RecoveryReadBudget,
) -> Result<(), String> {
    let assessment = classify_atomic_recovery(record, phase, read_budget)?;
    let action = assessment.automatic_action.ok_or_else(|| {
        format!(
            "WriteAuthority WAL recovery CAS nu mai permite acțiune automată; oracle-ul curent este {:?}: {}",
            assessment.classification, assessment.diagnostic
        )
    })?;
    match (phase, action) {
        (WalPhase::Prepared, AtomicRecoveryAction::ClearNoEffect) => Ok(()),
        _ => Err(format!(
            "WriteAuthority AtomicFile legacy permite automat numai Prepared/ClearNoEffect, nu {phase:?}/{action:?}."
        )),
    }
}

pub(in crate::kernel::write_authority::capability) fn discard_rebuildable_atomic_projection(
    record: &WalRecord,
    phase: WalPhase,
) -> Result<(), String> {
    let WalOperationEvidence::AtomicFile(evidence) = &record.body.operation_evidence else {
        return Err("Cleanup-ul proiecției rebuildable cere evidence AtomicFile.".into());
    };
    if phase == WalPhase::Preparing {
        return Ok(());
    }
    let context = capture_recovery_atomic_context(record, evidence)?;
    let RecoveryAtomicContext::Ready {
        directory,
        temp_leaf,
        ..
    } = context
    else {
        // A missing planned parent means neither the derived target nor its
        // deterministic temp can be present at the authority location.
        return Ok(());
    };
    let descriptor = match fs::openat(
        &directory,
        &temp_leaf,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(descriptor) => descriptor,
        Err(Errno::NOENT) => return Ok(()),
        Err(error) => {
            return Err(capability_error(
                &record.body.public_label,
                &format!("temp-ul rebuildable nu poate fi capturat pentru cleanup: {error}"),
            ));
        }
    };
    let descriptor_stat = fs::fstat(&descriptor).map_err(|error| {
        capability_error(
            &record.body.public_label,
            &format!("temp-ul rebuildable nu poate fi verificat: {error}"),
        )
    })?;
    if FileType::from_raw_mode(descriptor_stat.st_mode) != FileType::RegularFile
        || descriptor_stat.st_nlink != 1
    {
        return Err(capability_error(
            &record.body.public_label,
            "temp-ul rebuildable nu este un fișier regular single-link",
        ));
    }
    validate_named_file_identity(
        &directory,
        &temp_leaf,
        &descriptor_stat,
        "rebuildable-projection-temp",
    )?;
    fs::unlinkat(&directory, &temp_leaf, AtFlags::empty()).map_err(|error| {
        capability_error(
            &record.body.public_label,
            &format!("temp-ul rebuildable nu a putut fi eliminat: {error}"),
        )
    })?;
    sync_directory(&directory, &record.body.public_label)
}

pub(in crate::kernel::write_authority::capability) fn resolve_atomic_operator(
    record: &WalRecord,
    phase: WalPhase,
    action: WriteAuthorityRecoveryResolutionAction,
) -> Result<String, String> {
    if action != WriteAuthorityRecoveryResolutionAction::DiscardStagedWrite {
        return Err(format!(
            "AtomicFile nu acceptă rezoluția operator {action:?}."
        ));
    }
    if phase != WalPhase::AuxiliaryDurable {
        return Err(format!(
            "Abandonarea scrierii pregătite cere faza AuxiliaryDurable, nu {phase:?}."
        ));
    }
    let WalOperationEvidence::AtomicFile(evidence) = &record.body.operation_evidence else {
        return Err("Rezoluția AtomicFile a primit altă familie WAL.".into());
    };

    let mut classification_budget = RecoveryReadBudget::new();
    let assessment = classify_atomic_recovery(record, phase, &mut classification_budget)?;
    if assessment.classification != WriteAuthorityRecoveryClassification::StagedOnly {
        return Err(format!(
            "Scrierea pregătită nu mai este exact staged-only; scanarea este stale ({:?}): {}",
            assessment.classification, assessment.diagnostic
        ));
    }

    let RecoveryAtomicContext::Ready {
        directory,
        target_leaf,
        temp_leaf,
        ..
    } = capture_recovery_atomic_context(record, evidence)?
    else {
        return Err(
            "Scrierea pregătită nu mai are parentul capturabil; nicio ștergere nu a fost executată."
                .into(),
        );
    };
    let mut commit_budget = RecoveryReadBudget::new();
    let target = observe_recovery_leaf(
        &directory,
        &target_leaf,
        &record.body.public_label,
        "target operator discard",
        &mut commit_budget,
    )?;
    if target != evidence.before {
        return Err(
            "Target-ul s-a schimbat după scanare; temp-ul și WAL-ul rămân neatinse.".into(),
        );
    }

    let descriptor = fs::openat(
        &directory,
        &temp_leaf,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| {
        capability_error(
            &record.body.public_label,
            &format!("temp-ul staged nu poate fi capturat pentru abandonare: {error}"),
        )
    })?;
    let mut file = File::from(descriptor);
    let descriptor_stat = fs::fstat(&file).map_err(|error| {
        capability_error(
            &record.body.public_label,
            &format!("temp-ul staged nu poate fi verificat: {error}"),
        )
    })?;
    if FileType::from_raw_mode(descriptor_stat.st_mode) != FileType::RegularFile
        || descriptor_stat.st_nlink != 1
    {
        return Err(capability_error(
            &record.body.public_label,
            "temp-ul staged nu este un fișier regular single-link",
        ));
    }
    let observed_temp = wal_evidence_from_open_file(
        &mut file,
        &descriptor_stat,
        &ExpectedLeaf::Unspecified,
        &record.body.public_label,
        "operator discard staged temp",
        Some(&mut commit_budget),
    )?;
    if !leaf_matches_new(&observed_temp, evidence) {
        return Err(
            "Temp-ul staged nu mai corespunde payloadului WAL; nicio ștergere nu a fost executată."
                .into(),
        );
    }
    validate_named_file_identity(
        &directory,
        &temp_leaf,
        &descriptor_stat,
        "operator-discard-staged-temp",
    )?;
    fs::unlinkat(&directory, &temp_leaf, AtFlags::empty()).map_err(|error| {
        capability_error(
            &record.body.public_label,
            &format!("temp-ul staged nu a putut fi abandonat: {error}"),
        )
    })?;
    sync_directory(&directory, &record.body.public_label)?;
    Ok("Scrierea pregătită a fost abandonată: temp-ul verificat a fost eliminat, iar target-ul original a rămas neschimbat.".into())
}

pub(super) fn classify_legacy_append_recovery(
    record: &WalRecord,
    phase: WalPhase,
    read_budget: &mut RecoveryReadBudget,
) -> Result<AppendRecoveryAssessment, String> {
    let WalOperationEvidence::Append(evidence) = &record.body.operation_evidence else {
        return Err("WriteAuthority WAL append classifier a primit altă familie.".into());
    };
    let context = capture_recovery_append_context(record, evidence)?;
    let RecoveryAppendContext::Ready {
        directory,
        target_leaf,
        parent_was_missing,
    } = context
    else {
        let RecoveryAppendContext::ParentMissing {
            existing_components,
            planned_existing_components,
        } = context
        else {
            unreachable!()
        };
        return if phase == WalPhase::Prepared && existing_components == planned_existing_components
        {
            Ok(AppendRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::NoEffect,
                automatic_action: Some(AppendRecoveryAction::ClearNoEffect),
                diagnostic:
                    "Append Prepared păstrează exact frontiera parentului absent; clear no-effect este singura acțiune automată legacy."
                        .into(),
            })
        } else {
            Ok(AppendRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::Conflict,
                automatic_action: None,
                diagnostic: format!(
                    "Append {phase:?} nu poate atribui un parent absent/parțial (observedPrefix={existing_components}, plannedPrefix={planned_existing_components}); nicio mutație recovery nu este permisă."
                ),
            })
        };
    };

    let Some((mut file, stat)) = open_recovery_regular_leaf(
        &directory,
        &target_leaf,
        &record.body.public_label,
        "append target",
    )?
    else {
        return match (&evidence.before, phase, parent_was_missing) {
            (WalAppendBefore::Absent, WalPhase::Prepared, false) => {
                Ok(AppendRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::NoEffect,
                automatic_action: Some(AppendRecoveryAction::ClearNoEffect),
                diagnostic:
                    "Append Prepared păstrează exact baseline-ul Absent; clear no-effect este singura acțiune automată legacy."
                        .into(),
                })
            }
            _ => Ok(AppendRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::Conflict,
                automatic_action: None,
                diagnostic: format!(
                    "Append {phase:?} nu poate atribui targetul absent (baseline={:?}, parentCreated={parent_was_missing}); WAL-ul rămâne hot.",
                    evidence.before
                ),
            }),
        };
    };
    let current_size = u64::try_from(stat.st_size)
        .map_err(|_| "WriteAuthority WAL append target are dimensiune negativă.".to_string())?;

    let before_size = match &evidence.before {
        WalAppendBefore::Absent => 0,
        WalAppendBefore::Present {
            identity,
            size,
            version_token,
        } => {
            if stat.st_dev != identity.device || stat.st_ino != identity.inode {
                return Ok(AppendRecoveryAssessment {
                    classification: WriteAuthorityRecoveryClassification::Conflict,
                    automatic_action: None,
                    diagnostic: "Append target-ul nu mai este inode-ul baseline.".into(),
                });
            }
            if current_size == *size {
                return if version_token_for_stat(&stat) == *version_token {
                    match phase {
                        WalPhase::Prepared => Ok(AppendRecoveryAssessment {
                            classification: WriteAuthorityRecoveryClassification::NoEffect,
                            automatic_action: Some(AppendRecoveryAction::ClearNoEffect),
                            diagnostic:
                                "Append Prepared este exact baseline și nu are suffix; clear no-effect este singura acțiune automată legacy."
                                    .into(),
                        }),
                        WalPhase::AuxiliaryDurable => Ok(AppendRecoveryAssessment {
                            classification: WriteAuthorityRecoveryClassification::NoEffect,
                            automatic_action: None,
                            diagnostic:
                                "Append AuxiliaryDurable este încă exact baseline înainte de primul byte, dar protocolul legacy rămâne hot fără acțiune automată."
                                    .into(),
                        }),
                        WalPhase::Preparing
                        | WalPhase::EffectVisible
                        | WalPhase::TargetDurable => Ok(AppendRecoveryAssessment {
                            classification: WriteAuthorityRecoveryClassification::Conflict,
                            automatic_action: None,
                            diagnostic: format!(
                                "Append {phase:?} revendică progres incompatibil cu targetul rămas exact baseline."
                            ),
                        }),
                    }
                } else {
                    Ok(AppendRecoveryAssessment {
                        classification: WriteAuthorityRecoveryClassification::Conflict,
                        automatic_action: None,
                        diagnostic:
                            "Append target-ul are aceeași dimensiune, dar versiunea baseline s-a schimbat."
                                .into(),
                    })
                };
            }
            if current_size < *size {
                return Ok(AppendRecoveryAssessment {
                    classification: WriteAuthorityRecoveryClassification::Conflict,
                    automatic_action: None,
                    diagnostic: "Append target-ul este mai scurt decât baseline-ul.".into(),
                });
            }
            *size
        }
    };

    match assess_append_suffix(&mut file, before_size, evidence, read_budget)? {
        AppendSuffixState::Complete if phase == WalPhase::Prepared => {
            Ok(AppendRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::Conflict,
                automatic_action: None,
                diagnostic:
                    "Append Prepared a observat suffix-ul complet înainte ca runtime-ul să permită primul byte; payloadul poate aparține unui competitor și rămâne neatins."
                        .into(),
            })
        }
        AppendSuffixState::Complete if phase == WalPhase::Preparing => {
            Ok(AppendRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::Conflict,
                automatic_action: None,
                diagnostic:
                    "Append Preparing nu poate conține un suffix publicat; WAL-ul rămâne hot."
                        .into(),
            })
        }
        AppendSuffixState::Complete => Ok(AppendRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::EffectCommitted,
            automatic_action: None,
            diagnostic: format!(
                "Append {phase:?} are forma payloadului complet, dar protocolul legacy nu persistă identitatea/versiunea cauzală post-write; finalizarea automată este interzisă."
            ),
        }),
        AppendSuffixState::PartialExact if phase == WalPhase::AuxiliaryDurable => {
            Ok(AppendRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::PartialAppend,
                automatic_action: None,
                diagnostic:
                    "Append AuxiliaryDurable are un prefix exact al payloadului, dar truncate legacy nu are CAS identity→effect; bytes rămân hot și neatinși."
                        .into(),
            })
        }
        AppendSuffixState::PartialExact => Ok(AppendRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::Conflict,
            automatic_action: None,
            diagnostic: format!(
                "Append {phase:?} a observat un suffix parțial incompatibil cu faza sau neatribuibil cauzal; truncate este interzis și bytes rămân neatinși."
            ),
        }),
        AppendSuffixState::Conflict(diagnostic) => Ok(AppendRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::Conflict,
            automatic_action: None,
            diagnostic,
        }),
    }
}

pub(super) fn execute_legacy_append_recovery(
    record: &WalRecord,
    phase: WalPhase,
    read_budget: &mut RecoveryReadBudget,
) -> Result<(), String> {
    let assessment = classify_legacy_append_recovery(record, phase, read_budget)?;
    let action = assessment.automatic_action.ok_or_else(|| {
        format!(
            "WriteAuthority append recovery CAS nu mai permite acțiune automată: {}",
            assessment.diagnostic
        )
    })?;
    match (phase, action) {
        (WalPhase::Prepared, AppendRecoveryAction::ClearNoEffect) => Ok(()),
        _ => Err(format!(
            "WriteAuthority Append legacy permite automat numai Prepared/ClearNoEffect, nu {phase:?}/{action:?}."
        )),
    }
}

pub(super) fn classify_legacy_directory_recovery(
    record: &WalRecord,
    phase: WalPhase,
) -> Result<DirectoryRecoveryAssessment, String> {
    let WalOperationEvidence::Directory(evidence) = &record.body.operation_evidence else {
        return Err("WriteAuthority mkdir recovery a primit altă familie.".into());
    };
    let (authority, components) = capture_recovery_directory_authority(record, evidence)?;
    let mut directory = rustix::io::dup(authority.directory()).map_err(|error| {
        format!("WriteAuthority mkdir recovery nu poate duplica boundary: {error}.")
    })?;
    if evidence.existing_prefix_len == 0
        && wal_identity_from_fd(&directory, &record.body.public_label)?
            != evidence.existing_ancestor_identity
    {
        return Err("WriteAuthority mkdir recovery authority identity diferă de plan.".into());
    }
    let mut observed_prefix_len = 0_usize;
    for component in &components {
        match open_directory_strict(&directory, component) {
            Ok(next) => {
                validate_named_directory_identity(
                    &directory,
                    component,
                    &next,
                    &record.body.public_label,
                    "mkdir recovery component",
                )?;
                directory = next;
                observed_prefix_len += 1;
                if observed_prefix_len == evidence.existing_prefix_len
                    && wal_identity_from_fd(&directory, &record.body.public_label)?
                        != evidence.existing_ancestor_identity
                {
                    return Err(
                        "WriteAuthority mkdir recovery ancestorul baseline a fost înlocuit.".into(),
                    );
                }
            }
            Err(Errno::NOENT) => break,
            Err(error) => {
                return Err(capability_error(
                    &record.body.public_label,
                    &format!("mkdir recovery a întâlnit un component invalid: {error}"),
                ));
            }
        }
    }
    if observed_prefix_len < evidence.existing_prefix_len {
        return Ok(DirectoryRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::Conflict,
            automatic_action: None,
            available_resolution_actions: Vec::new(),
            resolution_state_binding: None,
            diagnostic:
                "Un director baseline din planul mkdir lipsește la restart; manual review obligatoriu."
                    .into(),
        });
    }
    if evidence.existing_prefix_len == components.len() {
        let observed = wal_identity_from_fd(&directory, &record.body.public_label)?;
        return if evidence.existing_target_identity.as_ref() == Some(&observed)
            && phase == WalPhase::Prepared
        {
            Ok(DirectoryRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::NoEffect,
                automatic_action: Some(DirectoryRecoveryAction::ClearNoEffect),
                available_resolution_actions: Vec::new(),
                resolution_state_binding: None,
                diagnostic:
                    "Directorul exista înainte de WAL, păstrează identitatea baseline, iar faza Prepared este singura fază posibilă pentru acest no-op legacy."
                        .into(),
            })
        } else if evidence.existing_target_identity.as_ref() == Some(&observed) {
            Ok(DirectoryRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::Conflict,
                automatic_action: None,
                available_resolution_actions: Vec::new(),
                resolution_state_binding: None,
                diagnostic: format!(
                    "Directorul baseline este intact, dar faza {phase:?} este imposibilă pentru no-op-ul mkdir legacy; WAL-ul rămâne hot."
                ),
            })
        } else {
            Ok(DirectoryRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::Conflict,
                automatic_action: None,
                available_resolution_actions: Vec::new(),
                resolution_state_binding: None,
                diagnostic: "Directorul existent înainte de WAL are altă identitate la restart."
                    .into(),
            })
        };
    }
    if observed_prefix_len == evidence.existing_prefix_len {
        return Ok(DirectoryRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::Conflict,
            automatic_action: None,
            available_resolution_actions: Vec::new(),
            resolution_state_binding: None,
            diagnostic: format!(
                "Suffix-ul mkdir planificat absent nu este vizibil în faza {phase:?}, dar mkdirat rulează înaintea primei tranziții de fază legacy; un efect creat și apoi eliminat nu poate fi exclus. WAL-ul rămâne hot."
            ),
        });
    }
    if observed_prefix_len == components.len() {
        return Ok(DirectoryRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::EffectCommitted,
            automatic_action: None,
            available_resolution_actions: Vec::new(),
            resolution_state_binding: None,
            diagnostic:
                "Întregul suffix mkdir este vizibil, dar recordul immutable nu conține identitățile post-create; poate aparține unui actor extern și cere manual review."
                    .into(),
        });
    }
    Ok(DirectoryRecoveryAssessment {
        classification: WriteAuthorityRecoveryClassification::PartialNamespaceCreation,
        automatic_action: None,
        available_resolution_actions: Vec::new(),
        resolution_state_binding: None,
        diagnostic:
            "Suffix-ul mkdir este parțial, dar identitățile post-create lipsesc; recovery nu poate distinge efectul propriu de namespace extern."
                .into(),
    })
}

pub(super) fn execute_legacy_directory_recovery(
    record: &WalRecord,
    phase: WalPhase,
    action: DirectoryRecoveryAction,
) -> Result<(), String> {
    let assessment = classify_legacy_directory_recovery(record, phase)?;
    if assessment.automatic_action != Some(action) {
        return Err(format!(
            "WriteAuthority mkdir recovery CAS a refuzat {action:?}: {}",
            assessment.diagnostic
        ));
    }
    match (phase, action) {
        (WalPhase::Prepared, DirectoryRecoveryAction::ClearNoEffect) => Ok(()),
        _ => Err(format!(
            "WriteAuthority mkdir legacy permite automat numai Prepared/ClearNoEffect pentru un target baseline existent, nu {phase:?}/{action:?}."
        )),
    }
}

pub(super) fn capture_recovery_directory_authority(
    record: &WalRecord,
    evidence: &WalDirectoryEvidence,
) -> Result<(DirectoryAuthority, Vec<OsString>), String> {
    let boundary_path = decode_path_hex(&record.body.authority.boundary_path_hex)?;
    if !boundary_path.is_absolute() {
        return Err("WriteAuthority mkdir recovery refuză boundary non-absolut.".into());
    }
    let authority = capture_directory_authority(
        &boundary_path,
        "write-authority-wal/mkdir-recovery-target",
        DirectoryAuthorityScope::RecoveryTarget,
    )?;
    let identity = authority.identity();
    if identity.device != record.body.authority.identity.device
        || identity.inode != record.body.authority.identity.inode
    {
        return Err("WriteAuthority mkdir recovery boundary identity diferă.".into());
    }
    let components = evidence
        .relative_components_hex
        .iter()
        .map(|component| decode_component_hex(component))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((authority, components))
}

pub(super) fn capture_recovery_append_context(
    record: &WalRecord,
    evidence: &WalAppendEvidence,
) -> Result<RecoveryAppendContext, String> {
    let boundary_path = decode_path_hex(&record.body.authority.boundary_path_hex)?;
    if !boundary_path.is_absolute() {
        return Err("WriteAuthority append recovery refuză boundary non-absolut.".into());
    }
    let authority = capture_directory_authority(
        &boundary_path,
        "write-authority-wal/append-recovery-target",
        DirectoryAuthorityScope::RecoveryTarget,
    )?;
    let identity = authority.identity();
    if identity.device != record.body.authority.identity.device
        || identity.inode != record.body.authority.identity.inode
    {
        return Err("WriteAuthority append recovery boundary identity diferă.".into());
    }
    let parents = evidence
        .parent
        .relative_components_hex
        .iter()
        .map(|component| decode_component_hex(component))
        .collect::<Result<Vec<_>, _>>()?;
    let target_leaf = decode_component_hex(&evidence.target_leaf_hex)?;
    let mut directory = rustix::io::dup(authority.directory()).map_err(|error| {
        format!("WriteAuthority append recovery nu poate duplica boundary: {error}.")
    })?;
    let mut existing_components = 0_usize;
    for component in &parents {
        match open_directory_strict(&directory, component) {
            Ok(next) => {
                directory = next;
                existing_components += 1;
            }
            Err(Errno::NOENT) => {
                if existing_components == evidence.parent.existing_prefix_len {
                    let observed = wal_identity_from_fd(&directory, &record.body.public_label)?;
                    if observed != evidence.parent.existing_ancestor_identity {
                        return Err(capability_error(
                            &record.body.public_label,
                            "Append recovery frontiera parentului absent nu mai este ancestorul baseline",
                        ));
                    }
                }
                return Ok(RecoveryAppendContext::ParentMissing {
                    existing_components,
                    planned_existing_components: evidence.parent.existing_prefix_len,
                });
            }
            Err(error) => {
                return Err(format!(
                    "WriteAuthority append recovery nu poate captura parentul: {error}."
                ));
            }
        }
    }
    let observed = wal_identity_from_fd(&directory, &record.body.public_label)?;
    if let Some(expected) = &evidence.parent.parent_identity {
        if &observed != expected {
            return Err("WriteAuthority append recovery parent identity diferă.".into());
        }
    }
    Ok(RecoveryAppendContext::Ready {
        directory,
        target_leaf,
        parent_was_missing: evidence.parent.parent_identity.is_none(),
    })
}

pub(super) fn open_recovery_regular_leaf(
    parent: &OwnedFd,
    leaf: &OsStr,
    public_label: &str,
    role: &str,
) -> Result<Option<(File, fs::Stat)>, String> {
    let Some(metadata) = leaf_metadata(parent, leaf, public_label)? else {
        return Ok(None);
    };
    if FileType::from_raw_mode(metadata.st_mode) != FileType::RegularFile {
        return Err(capability_error(
            public_label,
            &format!("{role} nu este fișier regular"),
        ));
    }
    let descriptor = fs::openat(
        parent,
        leaf,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| capability_error(public_label, &format!("{role} open a eșuat: {error}")))?;
    validate_regular_single_link(&descriptor, public_label, role)?;
    let file = File::from(descriptor);
    let stat = fs::fstat(&file).map_err(|error| {
        capability_error(public_label, &format!("{role} stat a eșuat: {error}"))
    })?;
    if !same_file_identity(&metadata, &stat) {
        return Err(capability_error(
            public_label,
            &format!("{role} s-a schimbat în timpul capturii"),
        ));
    }
    validate_named_file_identity(parent, leaf, &stat, role)?;
    Ok(Some((file, stat)))
}

pub(super) fn assess_append_suffix(
    file: &mut File,
    before_size: u64,
    evidence: &WalAppendEvidence,
    read_budget: &mut RecoveryReadBudget,
) -> Result<AppendSuffixState, String> {
    const MAX_APPEND_AUTO_RECOVERY_BYTES: u64 = 128 * 1024 * 1024;
    let stat = fs::fstat(&*file)
        .map_err(|error| format!("Append recovery suffix stat a eșuat: {error}."))?;
    let current_size = u64::try_from(stat.st_size)
        .map_err(|_| "Append recovery suffix are dimensiune negativă.".to_string())?;
    if current_size < before_size {
        return Ok(AppendSuffixState::Conflict(
            "Append target-ul este mai scurt decât beforeSize.".into(),
        ));
    }
    let suffix_size = current_size - before_size;
    if suffix_size > evidence.payload_size {
        return Ok(AppendSuffixState::Conflict(
            "Append target-ul conține bytes concurenți după payload.".into(),
        ));
    }
    if suffix_size > MAX_APPEND_AUTO_RECOVERY_BYTES {
        return Ok(AppendSuffixState::Conflict(format!(
            "Append suffix depășește limita auto-recovery de {MAX_APPEND_AUTO_RECOVERY_BYTES} bytes."
        )));
    }
    read_budget.reserve(suffix_size, "append recovery suffix")?;
    file.seek(SeekFrom::Start(before_size))
        .map_err(|error| format!("Append recovery seek a eșuat: {error}."))?;
    let mut suffix = Vec::with_capacity(suffix_size as usize);
    file.take(suffix_size.saturating_add(1))
        .read_to_end(&mut suffix)
        .map_err(|error| format!("Append recovery suffix read a eșuat: {error}."))?;
    if suffix.len() as u64 != suffix_size {
        return Ok(AppendSuffixState::Conflict(
            "Append suffix s-a schimbat în timpul citirii.".into(),
        ));
    }
    if suffix_size == evidence.payload_size {
        return Ok(if sha256_bytes(&suffix) == evidence.payload_hash {
            AppendSuffixState::Complete
        } else {
            AppendSuffixState::Conflict(
                "Append suffix complet are alt hash decât payloadul.".into(),
            )
        });
    }
    let prefix = decode_bytes_hex(&evidence.payload_prefix_hex)?;
    if suffix.len() <= prefix.len() && suffix == prefix[..suffix.len()] {
        Ok(AppendSuffixState::PartialExact)
    } else {
        Ok(AppendSuffixState::Conflict(
            "Append suffix parțial nu este prefix exact al payloadului persistat.".into(),
        ))
    }
}

pub(super) fn capture_recovery_atomic_context(
    record: &WalRecord,
    evidence: &WalAtomicFileEvidence,
) -> Result<RecoveryAtomicContext, String> {
    let boundary_path = decode_path_hex(&record.body.authority.boundary_path_hex)?;
    if !boundary_path.is_absolute() {
        return Err("WriteAuthority WAL recovery refuză boundary non-absolut.".into());
    }
    let authority = capture_directory_authority(
        &boundary_path,
        "write-authority-wal/recovery-target",
        DirectoryAuthorityScope::RecoveryTarget,
    )?;
    let identity = authority.identity();
    if identity.device != record.body.authority.identity.device
        || identity.inode != record.body.authority.identity.inode
    {
        return Err(format!(
            "WriteAuthority WAL boundary identity diferă: expected dev={} ino={}, observed dev={} ino={}.",
            record.body.authority.identity.device,
            record.body.authority.identity.inode,
            identity.device,
            identity.inode
        ));
    }
    let parents = evidence
        .parent
        .relative_components_hex
        .iter()
        .map(|component| decode_component_hex(component))
        .collect::<Result<Vec<_>, _>>()?;
    let target_leaf = decode_component_hex(&evidence.target_leaf_hex)?;
    let temp_leaf = decode_component_hex(&evidence.temp_leaf_hex)?;
    let mut directory = rustix::io::dup(authority.directory()).map_err(|error| {
        format!("WriteAuthority WAL recovery nu poate duplica boundary handle: {error}.")
    })?;
    let mut existing_components = 0_usize;
    for component in &parents {
        match open_directory_strict(&directory, component) {
            Ok(next) => {
                directory = next;
                existing_components += 1;
            }
            Err(Errno::NOENT) => {
                if existing_components == evidence.parent.existing_prefix_len {
                    let observed = wal_identity_from_fd(&directory, &record.body.public_label)?;
                    if observed != evidence.parent.existing_ancestor_identity {
                        return Err(capability_error(
                            &record.body.public_label,
                            "WAL recovery frontiera parentului absent nu mai este ancestorul baseline",
                        ));
                    }
                }
                return Ok(RecoveryAtomicContext::ParentMissing {
                    existing_components,
                    planned_existing_components: evidence.parent.existing_prefix_len,
                });
            }
            Err(error) => {
                return Err(capability_error(
                    &record.body.public_label,
                    &format!("WAL recovery nu poate captura parentul: {error}"),
                ));
            }
        }
    }
    let observed_parent = wal_identity_from_fd(&directory, &record.body.public_label)?;
    let parent_was_missing = evidence.parent.parent_identity.is_none();
    if let Some(expected_parent) = &evidence.parent.parent_identity {
        if &observed_parent != expected_parent {
            return Err(capability_error(
                &record.body.public_label,
                "WAL recovery parent identity diferă de record",
            ));
        }
    }
    Ok(RecoveryAtomicContext::Ready {
        directory,
        target_leaf,
        temp_leaf,
        parent_was_missing,
    })
}

pub(super) fn observe_recovery_leaf(
    parent: &OwnedFd,
    leaf: &OsStr,
    public_label: &str,
    role: &str,
    read_budget: &mut RecoveryReadBudget,
) -> Result<WalLeafEvidence, String> {
    let evidence = capture_wal_leaf_evidence(
        parent,
        leaf,
        &ExpectedLeaf::Unspecified,
        public_label,
        Some(read_budget),
    )?;
    if let WalLeafEvidence::Regular { identity, .. } = &evidence {
        let stat = fs::statat(parent, leaf, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
            capability_error(
                public_label,
                &format!("WAL recovery {role} stat a eșuat: {error}"),
            )
        })?;
        if stat.st_nlink != 1 || stat.st_dev != identity.device || stat.st_ino != identity.inode {
            return Err(capability_error(
                public_label,
                &format!("WAL recovery {role} nu este single-link stabil"),
            ));
        }
    }
    Ok(evidence)
}

pub(super) fn leaf_matches_new(evidence: &WalLeafEvidence, plan: &WalAtomicFileEvidence) -> bool {
    matches!(
        evidence,
        WalLeafEvidence::Regular {
            size,
            content_hash,
            ..
        } if *size == plan.new_size && *content_hash == plan.new_content_hash
    )
}

pub(super) fn leaf_matches_relocated_before(
    observed: &WalLeafEvidence,
    before: &WalLeafEvidence,
) -> bool {
    matches!(
        (observed, before),
        (
            WalLeafEvidence::Regular {
                identity: observed_identity,
                size: observed_size,
                content_hash: observed_hash,
                ..
            },
            WalLeafEvidence::Regular {
                identity: before_identity,
                size: before_size,
                content_hash: before_hash,
                ..
            }
        ) if observed_identity == before_identity
            && observed_size == before_size
            && observed_hash == before_hash
    )
}
