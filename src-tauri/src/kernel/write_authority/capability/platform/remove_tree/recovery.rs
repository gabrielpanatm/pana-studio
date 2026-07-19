use super::{
    snapshot::{capture_tree_snapshot, mount_id_for_fd, mount_id_for_name, TreeSnapshot},
    traversal::{records_by_key, remove_planned_tree_contents},
    *,
};

#[derive(Clone, Debug)]
pub(super) struct ObservedRemoveTree {
    pub(super) identity: WalFilesystemIdentity,
    pub(super) is_directory: bool,
    pub(super) size: u64,
    pub(super) mtime_seconds: i64,
    pub(super) mtime_nanoseconds: u64,
    pub(super) ctime_seconds: i64,
    pub(super) ctime_nanoseconds: u64,
    pub(super) raw_mode: u32,
    pub(super) link_count: u64,
    pub(super) owner_uid: u32,
    pub(super) owner_gid: u32,
    pub(super) raw_device: u64,
    pub(super) version_token: String,
    pub(super) mount_id: Option<u64>,
    pub(super) snapshot: Option<TreeSnapshot>,
}

pub(in crate::kernel::write_authority::capability) fn classify_remove_tree_recovery(
    record: &WalRecord,
    phase: WalPhase,
) -> Result<RemoveTreeRecoveryAssessment, String> {
    let WalOperationEvidence::RemoveTree(evidence) = &record.body.operation_evidence else {
        return Err("WriteAuthority RemoveDirectoryTree recovery a primit altă familie.".into());
    };
    let (parent, target_leaf, quarantine_leaf) = capture_recovery_parent(record, evidence)?;
    let target = observe_tree(&parent, &target_leaf, &record.body.public_label)?;
    let quarantine = observe_tree(&parent, &quarantine_leaf, &record.body.public_label)?;

    let target_before = target
        .as_ref()
        .is_some_and(|observed| observed_matches_before(observed, &evidence.source));
    let target_restored_intact = target
        .as_ref()
        .is_some_and(|observed| observed_matches_intact_moved(observed, &evidence.source));
    let target_is_causal = target
        .as_ref()
        .is_some_and(|observed| observed_matches_causal_root(observed, &evidence.source));
    let quarantine_intact = quarantine
        .as_ref()
        .is_some_and(|observed| observed_matches_intact_moved(observed, &evidence.source));
    let quarantine_is_causal = quarantine
        .as_ref()
        .is_some_and(|observed| observed_matches_causal_root(observed, &evidence.source));

    if target_before && quarantine.is_none() {
        return Ok(RemoveTreeRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::NoEffect,
            automatic_action: (phase == WalPhase::Prepared)
                .then_some(RemoveTreeRecoveryAction::ClearNoEffect),
            available_resolution_actions: (phase != WalPhase::Prepared)
                .then_some(vec![
                    WriteAuthorityRecoveryResolutionAction::AcceptRestoredState,
                ])
                .unwrap_or_default(),
            diagnostic: if phase == WalPhase::Prepared {
                "RemoveDirectoryTree source este baseline exact, iar quarantine lipsește; Prepared permite clear no-effect."
                    .into()
            } else {
                format!(
                    "RemoveDirectoryTree source este baseline, dar faza {phase:?} păstrează WAL-ul pentru confirmare operator."
                )
            },
        });
    }
    if target_restored_intact && quarantine.is_none() {
        return Ok(RemoveTreeRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::RollbackCompleted,
            automatic_action: None,
            available_resolution_actions: vec![
                WriteAuthorityRecoveryResolutionAction::AcceptRestoredState,
            ],
            diagnostic:
                "RemoveDirectoryTree root și arborele exact par restaurate, dar ctime-ul dovedește o mutație de namespace; confirmarea operatorului este obligatorie."
                    .into(),
        });
    }
    if target.is_none() && quarantine_intact {
        return Ok(RemoveTreeRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::CleanupRequired,
            automatic_action: None,
            available_resolution_actions: vec![
                WriteAuthorityRecoveryResolutionAction::RestoreOriginal,
                WriteAuthorityRecoveryResolutionAction::ContinueTreeRemoval,
            ],
            diagnostic:
                "RemoveDirectoryTree source este intact și izolat în quarantine. Numai operatorul poate alege restore integral sau continuarea ștergerii."
                    .into(),
        });
    }
    if target.is_none() && quarantine_is_causal {
        return Ok(RemoveTreeRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::PartialTreeRemoval,
            automatic_action: None,
            available_resolution_actions: vec![
                WriteAuthorityRecoveryResolutionAction::RestoreRemainingTree,
                WriteAuthorityRecoveryResolutionAction::ContinueTreeRemoval,
            ],
            diagnostic:
                "RemoveDirectoryTree quarantine păstrează root-ul WAL, dar fingerprint-ul descendenților s-a schimbat: ștergere parțială. RestoreRemainingTree nu poate recrea descendenții deja eliminați."
                    .into(),
        });
    }
    if target.is_none() && quarantine.is_none() {
        if matches!(phase, WalPhase::EffectVisible | WalPhase::TargetDurable) {
            return Ok(RemoveTreeRecoveryAssessment {
                classification: WriteAuthorityRecoveryClassification::EffectCommitted,
                automatic_action: Some(RemoveTreeRecoveryAction::FinalizeCommitted),
                available_resolution_actions: Vec::new(),
                diagnostic:
                    "RemoveDirectoryTree a trecut de effect-visible, iar source și quarantine sunt absente; fsync/recheck poate finaliza fără mutație."
                        .into(),
            });
        }
        return Ok(RemoveTreeRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::Conflict,
            automatic_action: None,
            available_resolution_actions: Vec::new(),
            diagnostic:
                "RemoveDirectoryTree source și quarantine lipsesc înainte de effect-visible; efectul nu poate fi atribuit sigur."
                    .into(),
        });
    }
    if target_is_causal && quarantine.is_none() {
        return Ok(RemoveTreeRecoveryAssessment {
            classification: WriteAuthorityRecoveryClassification::PartialTreeRemoval,
            automatic_action: None,
            available_resolution_actions: vec![
                WriteAuthorityRecoveryResolutionAction::AcceptRestoredState,
            ],
            diagnostic:
                "RemoveDirectoryTree root-ul WAL este la numele public, dar arborele nu mai este baseline-ul inițial; starea poate fi acceptată numai explicit."
                    .into(),
        });
    }
    Ok(RemoveTreeRecoveryAssessment {
        classification: WriteAuthorityRecoveryClassification::Conflict,
        automatic_action: None,
        available_resolution_actions: Vec::new(),
        diagnostic: format!(
            "Oracle-ul RemoveDirectoryTree nu poate atribui namespace-ul (targetBefore={target_before}, targetCausal={target_is_causal}, targetAbsent={}, quarantineIntact={quarantine_intact}, quarantineCausal={quarantine_is_causal}, quarantineAbsent={}).",
            target.is_none(),
            quarantine.is_none()
        ),
    })
}

pub(in crate::kernel::write_authority::capability) fn execute_remove_tree_recovery(
    record: &WalRecord,
    phase: WalPhase,
) -> Result<(), String> {
    let assessment = classify_remove_tree_recovery(record, phase)?;
    let action = assessment.automatic_action.ok_or_else(|| {
        format!(
            "WriteAuthority RemoveDirectoryTree recovery nu permite acțiune automată: {}",
            assessment.diagnostic
        )
    })?;
    match action {
        RemoveTreeRecoveryAction::ClearNoEffect => Ok(()),
        RemoveTreeRecoveryAction::FinalizeCommitted => {
            let WalOperationEvidence::RemoveTree(evidence) = &record.body.operation_evidence else {
                return Err(
                    "WriteAuthority RemoveDirectoryTree finalize a primit altă familie.".into(),
                );
            };
            let (parent, target_leaf, quarantine_leaf) = capture_recovery_parent(record, evidence)?;
            if observe_tree(&parent, &target_leaf, &record.body.public_label)?.is_some()
                || observe_tree(&parent, &quarantine_leaf, &record.body.public_label)?.is_some()
            {
                return Err(
                    "WriteAuthority RemoveDirectoryTree finalize CAS a observat un nume reapărut."
                        .into(),
                );
            }
            sync_directory(&parent, &record.body.public_label)?;
            let after = classify_remove_tree_recovery(record, phase)?;
            if after.classification != WriteAuthorityRecoveryClassification::EffectCommitted {
                return Err(format!(
                    "WriteAuthority RemoveDirectoryTree finalize postflight s-a schimbat: {}",
                    after.diagnostic
                ));
            }
            Ok(())
        }
    }
}

pub(in crate::kernel::write_authority::capability) fn resolve_remove_tree_operator(
    record: &WalRecord,
    phase: WalPhase,
    action: WriteAuthorityRecoveryResolutionAction,
) -> Result<String, String> {
    let WalOperationEvidence::RemoveTree(evidence) = &record.body.operation_evidence else {
        return Err("WriteAuthority RemoveDirectoryTree operator a primit altă familie.".into());
    };
    let assessment = classify_remove_tree_recovery(record, phase)?;
    if !assessment.available_resolution_actions.contains(&action) {
        return Err(format!(
            "Acțiunea {action:?} nu este permisă pentru {:?}: {}",
            assessment.classification, assessment.diagnostic
        ));
    }
    let (parent, target_leaf, quarantine_leaf) = capture_recovery_parent(record, evidence)?;

    match action {
        WriteAuthorityRecoveryResolutionAction::RestoreOriginal => {
            let quarantine = require_tree(
                observe_tree(&parent, &quarantine_leaf, &record.body.public_label)?,
                "RestoreOriginal quarantine lipsește sau nu este director",
            )?;
            if !observed_matches_intact_moved(&quarantine, &evidence.source)
                || observe_tree(&parent, &target_leaf, &record.body.public_label)?.is_some()
            {
                return Err(
                    "RestoreOriginal cere quarantine intact exact și target public absent.".into(),
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
            sync_directory(&parent, &record.body.public_label)?;
            let restored = require_tree(
                observe_tree(&parent, &target_leaf, &record.body.public_label)?,
                "RestoreOriginal postflight nu găsește targetul",
            )?;
            if !observed_matches_intact_moved(&restored, &evidence.source)
                || observe_tree(&parent, &quarantine_leaf, &record.body.public_label)?.is_some()
            {
                return Err(
                    "RestoreOriginal postflight nu demonstrează arborele WAL restaurat; recordul rămâne hot."
                        .into(),
                );
            }
            Ok("RemoveDirectoryTree quarantine intact a fost restaurat durabil la numele original."
                .into())
        }
        WriteAuthorityRecoveryResolutionAction::RestoreRemainingTree => {
            if observe_tree(&parent, &target_leaf, &record.body.public_label)?.is_some() {
                return Err("RestoreRemainingTree cere targetul public absent.".into());
            }
            let before = require_tree(
                observe_tree(&parent, &quarantine_leaf, &record.body.public_label)?,
                "RestoreRemainingTree quarantine lipsește sau nu este director",
            )?;
            if !observed_matches_causal_root(&before, &evidence.source) {
                return Err(
                    "RestoreRemainingTree quarantine nu mai este root-ul WAL exact.".into(),
                );
            }
            let before_snapshot = before.snapshot.clone().ok_or_else(|| {
                "RestoreRemainingTree nu poate captura arborele rămas.".to_string()
            })?;
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
                    &format!("RestoreRemainingTree RENAME_NOREPLACE a eșuat: {error}"),
                )
            })?;
            sync_directory(&parent, &record.body.public_label)?;
            let restored = require_tree(
                observe_tree(&parent, &target_leaf, &record.body.public_label)?,
                "RestoreRemainingTree postflight nu găsește targetul",
            )?;
            if !observed_matches_causal_root(&restored, &evidence.source)
                || restored.snapshot.as_ref() != Some(&before_snapshot)
                || observe_tree(&parent, &quarantine_leaf, &record.body.public_label)?.is_some()
            {
                return Err(
                    "RestoreRemainingTree postflight nu demonstrează exact arborele parțial restaurat; recordul rămâne hot."
                        .into(),
                );
            }
            Ok(
                "Arborele rămas a fost restaurat durabil. Descendenții eliminați înainte de recovery nu au putut fi recreați."
                    .into(),
            )
        }
        WriteAuthorityRecoveryResolutionAction::ContinueTreeRemoval => {
            if observe_tree(&parent, &target_leaf, &record.body.public_label)?.is_some() {
                return Err("ContinueTreeRemoval cere targetul public absent.".into());
            }
            let quarantine = require_tree(
                observe_tree(&parent, &quarantine_leaf, &record.body.public_label)?,
                "ContinueTreeRemoval quarantine lipsește sau nu este director",
            )?;
            if !observed_matches_causal_root(&quarantine, &evidence.source) {
                return Err("ContinueTreeRemoval quarantine nu mai este root-ul WAL exact.".into());
            }
            let snapshot = quarantine.snapshot.ok_or_else(|| {
                "ContinueTreeRemoval nu poate captura arborele rămas.".to_string()
            })?;
            let directory = open_directory_strict(&parent, &quarantine_leaf).map_err(|error| {
                capability_error(
                    &record.body.public_label,
                    &format!("ContinueTreeRemoval nu poate captura quarantine: {error}"),
                )
            })?;
            let root_stat = fs::fstat(&directory).map_err(|error| {
                capability_error(
                    &record.body.public_label,
                    &format!("ContinueTreeRemoval nu poate citi root metadata: {error}"),
                )
            })?;
            if root_stat.st_dev != evidence.source.identity.device
                || root_stat.st_ino != evidence.source.identity.inode
                || mount_id_for_fd(
                    &directory,
                    &record.body.public_label,
                    "ContinueTreeRemoval root",
                )? != evidence.source.mount_id
            {
                return Err("ContinueTreeRemoval root descriptor diferă de WAL.".into());
            }
            let planned = records_by_key(&snapshot.records);
            let mut removed = 0_u64;
            remove_planned_tree_contents(
                &directory,
                "",
                0,
                &mut removed,
                evidence.source.mount_id,
                &planned,
                &record.body.public_label,
            )?;
            if removed != snapshot.entry_count {
                return Err(format!(
                    "ContinueTreeRemoval a eliminat {removed} intrări din {} capturate.",
                    snapshot.entry_count
                ));
            }
            validate_named_directory_identity(
                &parent,
                &quarantine_leaf,
                &directory,
                &record.body.public_label,
                "ContinueTreeRemoval quarantine root",
            )?;
            fs::unlinkat(&parent, &quarantine_leaf, AtFlags::REMOVEDIR).map_err(|error| {
                capability_error(
                    &record.body.public_label,
                    &format!("ContinueTreeRemoval nu poate elimina root-ul gol: {error}"),
                )
            })?;
            sync_directory(&parent, &record.body.public_label)?;
            if observe_tree(&parent, &target_leaf, &record.body.public_label)?.is_some()
                || observe_tree(&parent, &quarantine_leaf, &record.body.public_label)?.is_some()
            {
                return Err(
                    "ContinueTreeRemoval postflight a observat un nume reapărut; recordul rămâne hot."
                        .into(),
                );
            }
            Ok("Ștergerea explicită a arborelui rămas a fost finalizată durabil.".into())
        }
        WriteAuthorityRecoveryResolutionAction::AcceptRestoredState => {
            if observe_tree(&parent, &quarantine_leaf, &record.body.public_label)?.is_some() {
                return Err("AcceptRestoredState cere quarantine absent.".into());
            }
            let before = require_tree(
                observe_tree(&parent, &target_leaf, &record.body.public_label)?,
                "AcceptRestoredState targetul public lipsește sau nu este director",
            )?;
            if !observed_matches_causal_root(&before, &evidence.source) {
                return Err("AcceptRestoredState targetul nu mai este root-ul WAL exact.".into());
            }
            let before_snapshot = before
                .snapshot
                .ok_or_else(|| "AcceptRestoredState nu poate captura arborele.".to_string())?;
            sync_directory(&parent, &record.body.public_label)?;
            let after = require_tree(
                observe_tree(&parent, &target_leaf, &record.body.public_label)?,
                "AcceptRestoredState postflight nu găsește targetul",
            )?;
            if !observed_matches_causal_root(&after, &evidence.source)
                || after.snapshot.as_ref() != Some(&before_snapshot)
                || observe_tree(&parent, &quarantine_leaf, &record.body.public_label)?.is_some()
            {
                return Err(
                    "AcceptRestoredState postflight s-a schimbat; recordul rămâne hot.".into(),
                );
            }
            Ok(
                "Starea RemoveDirectoryTree restaurată a fost acceptată explicit și sincronizată."
                    .into(),
            )
        }
        WriteAuthorityRecoveryResolutionAction::AcceptCurrentState => Err(
            "Acțiunea operator AcceptCurrentState este rezervată familiilor Directory/Symlink."
                .into(),
        ),
    }
}

fn capture_recovery_parent(
    record: &WalRecord,
    evidence: &WalRemoveTreeEvidence,
) -> Result<(OwnedFd, OsString, OsString), String> {
    let boundary_path = decode_path_hex(&record.body.authority.boundary_path_hex)?;
    let authority = capture_directory_authority(
        &boundary_path,
        "write-authority-wal/remove-tree-recovery-target",
        DirectoryAuthorityScope::RecoveryTarget,
    )?;
    if authority.identity().device != record.body.authority.identity.device
        || authority.identity().inode != record.body.authority.identity.inode
    {
        return Err(capability_error(
            &record.body.public_label,
            "RemoveDirectoryTree recovery authority identity diferă de WAL",
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
            &format!("RemoveDirectoryTree recovery nu poate duplica authority: {error}"),
        )
    })?;
    for component in parents {
        let next = open_directory_strict(&directory, &component).map_err(|error| {
            capability_error(
                &record.body.public_label,
                &format!("RemoveDirectoryTree recovery parent capture a eșuat: {error}"),
            )
        })?;
        validate_named_directory_identity(
            &directory,
            &component,
            &next,
            &record.body.public_label,
            "RemoveDirectoryTree recovery parent",
        )?;
        directory = next;
    }
    let observed_parent = wal_identity_from_fd(&directory, &record.body.public_label)?;
    if evidence.parent.parent_identity.as_ref() != Some(&observed_parent) {
        return Err(capability_error(
            &record.body.public_label,
            "RemoveDirectoryTree recovery parent identity diferă de WAL",
        ));
    }
    Ok((
        directory,
        decode_component_hex(&evidence.target_leaf_hex)?,
        decode_component_hex(&evidence.quarantine_leaf_hex)?,
    ))
}

pub(super) fn observe_tree(
    parent: &OwnedFd,
    leaf: &OsStr,
    public_label: &str,
) -> Result<Option<ObservedRemoveTree>, String> {
    let Some(stat) = leaf_metadata(parent, leaf, public_label)? else {
        return Ok(None);
    };
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory {
        return observed_root_from_stat(&stat, None, None, public_label).map(Some);
    }
    let mount_id = mount_id_for_name(parent, leaf, public_label, "RemoveDirectoryTree observed")?;
    let directory = open_directory_strict(parent, leaf).map_err(|error| {
        capability_error(
            public_label,
            &format!("RemoveDirectoryTree observed root nu poate fi capturat: {error}"),
        )
    })?;
    validate_named_directory_identity(
        parent,
        leaf,
        &directory,
        public_label,
        "RemoveDirectoryTree observed root",
    )?;
    if mount_id_for_fd(
        &directory,
        public_label,
        "RemoveDirectoryTree observed root",
    )? != mount_id
    {
        return Err(capability_error(
            public_label,
            "RemoveDirectoryTree observed root are mount ID instabil",
        ));
    }
    let snapshot = capture_tree_snapshot(
        &directory,
        mount_id,
        public_label,
        "RemoveDirectoryTree recovery oracle",
    )?;
    let after = fs::fstat(&directory).map_err(|error| {
        capability_error(
            public_label,
            &format!("RemoveDirectoryTree observed metadata nu poate fi citită: {error}"),
        )
    })?;
    observed_root_from_stat(&after, Some(mount_id), Some(snapshot), public_label).map(Some)
}

pub(super) fn observed_root_from_stat(
    stat: &fs::Stat,
    mount_id: Option<u64>,
    snapshot: Option<TreeSnapshot>,
    public_label: &str,
) -> Result<ObservedRemoveTree, String> {
    Ok(ObservedRemoveTree {
        identity: WalFilesystemIdentity {
            device: stat.st_dev,
            inode: stat.st_ino,
        },
        is_directory: FileType::from_raw_mode(stat.st_mode) == FileType::Directory,
        size: u64::try_from(stat.st_size).map_err(|_| {
            capability_error(
                public_label,
                "RemoveDirectoryTree observed size este negativ",
            )
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
        mount_id,
        snapshot,
    })
}

pub(super) fn observed_matches_before(
    observed: &ObservedRemoveTree,
    expected: &WalRemoveTreeSourceEvidence,
) -> bool {
    observed_matches_intact_moved(observed, expected)
        && observed.version_token == expected.version_token
        && observed.ctime_seconds == expected.ctime_seconds
        && observed.ctime_nanoseconds == expected.ctime_nanoseconds
}

pub(super) fn observed_matches_intact_moved(
    observed: &ObservedRemoveTree,
    expected: &WalRemoveTreeSourceEvidence,
) -> bool {
    observed_matches_causal_root(observed, expected)
        && observed.size == expected.size
        && observed.mtime_seconds == expected.mtime_seconds
        && observed.mtime_nanoseconds == expected.mtime_nanoseconds
        && observed.link_count == expected.link_count
        && observed.snapshot.as_ref().is_some_and(|snapshot| {
            snapshot.fingerprint == expected.tree_fingerprint
                && snapshot.entry_count == expected.entry_count
        })
}

fn observed_matches_causal_root(
    observed: &ObservedRemoveTree,
    expected: &WalRemoveTreeSourceEvidence,
) -> bool {
    observed.is_directory
        && observed.identity == expected.identity
        && observed.raw_mode == expected.raw_mode
        && observed.owner_uid == expected.owner_uid
        && observed.owner_gid == expected.owner_gid
        && observed.raw_device == expected.raw_device
        && observed.mount_id == Some(expected.mount_id)
}

fn require_tree(
    observed: Option<ObservedRemoveTree>,
    diagnostic: &str,
) -> Result<ObservedRemoveTree, String> {
    match observed {
        Some(value) if value.is_directory => Ok(value),
        _ => Err(diagnostic.to_string()),
    }
}
