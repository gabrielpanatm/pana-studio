use super::*;

#[path = "external_config/recovery.rs"]
mod recovery;
#[path = "external_config/snapshot.rs"]
mod snapshot;

use snapshot::*;

pub(in crate::kernel::write_authority::capability) use recovery::{
    classify_external_config_recovery, execute_external_config_recovery,
};

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn external_stage_identity_digest_for_test(
    path: &Path,
    role: &str,
) -> Result<String, String> {
    let file = File::open(path)
        .map_err(|error| format!("Testul nu poate deschide artefactul ExternalConfig: {error}."))?;
    external_stage_identity_digest(&file, role)
}

pub(in crate::kernel::write_authority::capability) fn plan_external_config(
    target: &WriteTarget,
    bytes: &[u8],
    backup: Option<(&WriteTarget, &[u8])>,
    operation_id: &str,
) -> Result<ExternalConfigOperationPlan, String> {
    validate_external_payload_size(bytes, &target.public_label, "target")?;
    let lexical = lexical_target(target, false)?;
    let authority = lexical.authority.as_ref().ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "planul ExternalConfig WAL cere authority sigilata",
        )
    })?;
    let parent = capture_existing_target_parent(&lexical)?.ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "parentul ExternalConfig trebuie sa existe integral inainte de WAL prepare",
        )
    })?;
    let (_, parent_components) = lexical
        .relative_components
        .split_last()
        .ok_or_else(|| capability_error(&lexical.public_label, "ExternalConfig cere un leaf"))?;
    let parent_identity = wal_identity_from_fd(&parent.directory, &lexical.public_label)?;
    let parent_evidence = WalParentEvidence {
        relative_components_hex: parent_components
            .iter()
            .map(|component| encode_component_hex(component))
            .collect(),
        existing_prefix_len: parent_components.len(),
        existing_ancestor_identity: parent_identity.clone(),
        parent_identity: Some(parent_identity),
    };

    let target_temp = external_config_target_temp_leaf(operation_id);
    let backup_temp = external_config_backup_temp_leaf(operation_id);
    let target_hash = sha256_bytes(bytes);
    let (
        target_before,
        target_before_mode_bits,
        target_before_identity_digest,
        target_new_mode_bits,
        backup_evidence,
        backup_mode_bits,
        existing_target,
    ) = match backup {
        None => {
            if matches!(target.expected_leaf, ExpectedLeaf::Present(_)) {
                return Err(capability_error(
                    &lexical.public_label,
                    "ExternalConfig create-new nu accepta disk baseline Present",
                ));
            }
            validate_leaf_absent_for_external(
                &parent.directory,
                &parent.leaf,
                &lexical.public_label,
                "target create-new",
            )?;
            (WalLeafEvidence::Absent, None, None, 0o600, None, None, None)
        }
        Some((backup_target, previous_bytes)) => {
            validate_external_payload_size(
                previous_bytes,
                &lexical.public_label,
                "backup baseline",
            )?;
            let backup_lexical = lexical_target(backup_target, false)?;
            let backup_authority = backup_lexical.authority.as_ref().ok_or_else(|| {
                capability_error(
                    &backup_lexical.public_label,
                    "planul backup ExternalConfig cere authority sigilata",
                )
            })?;
            if !authority.same_authority(backup_authority) {
                return Err(capability_error(
                    &lexical.public_label,
                    "targetul si backup-ul ExternalConfig nu folosesc aceeasi authority",
                ));
            }
            let (backup_leaf, backup_parents) = backup_lexical
                .relative_components
                .split_last()
                .ok_or_else(|| {
                    capability_error(&backup_lexical.public_label, "backup-ul cere un leaf")
                })?;
            if backup_parents != parent_components || backup_leaf == &parent.leaf {
                return Err(capability_error(
                    &lexical.public_label,
                    "backup-ul ExternalConfig trebuie sa fie sibling distinct al targetului",
                ));
            }
            if matches!(backup_target.expected_leaf, ExpectedLeaf::Present(_)) {
                return Err(capability_error(
                    &backup_lexical.public_label,
                    "backup-ul ExternalConfig este create-only si nu accepta baseline Present",
                ));
            }
            validate_leaf_absent_for_external(
                &parent.directory,
                backup_leaf,
                &backup_lexical.public_label,
                "backup destination",
            )?;

            let descriptor = fs::openat(
                &parent.directory,
                &parent.leaf,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|error| {
                capability_error(
                    &lexical.public_label,
                    &format!("targetul existent nu poate fi capturat: {error}"),
                )
            })?;
            validate_regular_single_link(
                &descriptor,
                &lexical.public_label,
                "ExternalConfig WAL plan target",
            )?;
            let mut target_file = File::from(descriptor);
            let target_stat = fs::fstat(&target_file).map_err(|error| {
                capability_error(
                    &lexical.public_label,
                    &format!("metadata targetului existent nu poate fi citita: {error}"),
                )
            })?;
            let target_size = u64::try_from(target_stat.st_size).map_err(|_| {
                capability_error(
                    &lexical.public_label,
                    "targetul ExternalConfig are dimensiune negativă",
                )
            })?;
            if target_size > MAX_WAL_EXTERNAL_CONFIG_BYTES {
                return Err(capability_error(
                    &lexical.public_label,
                    &format!(
                        "targetul ExternalConfig depășește limita de {MAX_WAL_EXTERNAL_CONFIG_BYTES} bytes"
                    ),
                ));
            }
            let target_before = wal_evidence_from_open_file(
                &mut target_file,
                &target_stat,
                &target.expected_leaf,
                &lexical.public_label,
                "ExternalConfig WAL plan target",
                None,
            )?;
            let expected_previous_hash = sha256_bytes(previous_bytes);
            if !leaf_matches_payload(
                &target_before,
                previous_bytes.len() as u64,
                &expected_previous_hash,
            ) {
                return Err(capability_error(
                    &lexical.public_label,
                    "bytes-ii de backup nu sunt snapshotul exact al targetului existent",
                ));
            }
            validate_named_file_identity(
                &parent.directory,
                &parent.leaf,
                &target_stat,
                "external-config-plan-target",
            )?;
            let target_before_identity_digest = external_baseline_identity_digest(&target_file)?;
            let target_after = fs::fstat(&target_file).map_err(|error| {
                capability_error(
                    &lexical.public_label,
                    &format!("targetul existent nu poate fi reverificat: {error}"),
                )
            })?;
            if !same_stable_leaf_version(&target_stat, &target_after) {
                return Err(capability_error(
                    &lexical.public_label,
                    "targetul existent s-a schimbat in timpul planificarii",
                ));
            }
            validate_named_file_identity(
                &parent.directory,
                &parent.leaf,
                &target_after,
                "external-config-plan-target-final",
            )?;
            let mode = external_mode_bits(&target_after);
            let backup_evidence = WalAtomicFileEvidence {
                parent: parent_evidence.clone(),
                target_leaf_hex: encode_component_hex(backup_leaf),
                temp_leaf_hex: encode_component_hex(&backup_temp),
                before: WalLeafEvidence::Absent,
                new_size: previous_bytes.len() as u64,
                new_content_hash: expected_previous_hash,
                replace: false,
            };
            (
                target_before,
                Some(mode),
                Some(target_before_identity_digest),
                mode,
                Some(backup_evidence),
                Some(mode),
                Some(target_file),
            )
        }
    };

    let target_evidence = WalAtomicFileEvidence {
        parent: parent_evidence,
        target_leaf_hex: encode_component_hex(&parent.leaf),
        temp_leaf_hex: encode_component_hex(&target_temp),
        replace: existing_target.is_some(),
        before: target_before,
        new_size: bytes.len() as u64,
        new_content_hash: target_hash,
    };
    let evidence = WalExternalConfigEvidence {
        protocol_version: WAL_EXTERNAL_CONFIG_PROTOCOL_VERSION,
        target: target_evidence,
        backup: backup_evidence,
        target_before_mode_bits,
        target_before_identity_digest,
        target_new_mode_bits,
        backup_mode_bits,
    };
    let leaves_owned = owned_external_leaves(&evidence)?;
    let leaves = leaves_owned.as_borrowed();
    validate_external_leaf_distinctness(leaves, &lexical.public_label)?;
    for (leaf, role) in external_auxiliary_leaves(leaves) {
        validate_leaf_absent_for_external(&parent.directory, leaf, &lexical.public_label, role)?;
    }

    Ok(ExternalConfigOperationPlan {
        evidence,
        existing_target,
    })
}

pub(in crate::kernel::write_authority::capability) fn external_config_update_wal(
    target: &WriteTarget,
    bytes: &[u8],
    backup: Option<(&WriteTarget, &[u8])>,
    mut plan: ExternalConfigOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    let lexical = lexical_target(target, false)?;
    let backup_lexical = backup
        .map(|(backup_target, _)| lexical_target(backup_target, false))
        .transpose()?;
    validate_external_plan_shape(
        &lexical,
        backup_lexical.as_ref(),
        bytes,
        backup.map(|(_, previous)| previous),
        &plan,
        guard.operation_id(),
    )?;

    let parent = match capture_parent_from_wal_evidence(&lexical, &plan.evidence.target.parent) {
        Ok(parent) => parent,
        Err(error) => return error.into_operation_result(),
    };
    if parent.created_ancestors {
        return Err(capability_error(
            &lexical.public_label,
            "ExternalConfig WAL nu poate crea namespace parinte",
        ));
    }
    let leaves_owned = owned_external_leaves(&plan.evidence)?;
    let leaves = leaves_owned.as_borrowed();
    validate_external_leaf_distinctness(leaves, &lexical.public_label)?;
    validate_runtime_before(&parent, &mut plan, leaves, &lexical.public_label, true)?;
    validate_external_auxiliary_absent(&parent.directory, leaves, &lexical.public_label)?;

    let recovery = |diagnostic: String| {
        wal_recovery_effect(
            bytes.len() as u64,
            &lexical.public_label,
            format!("{diagnostic} ExternalConfig WAL ramane pentru recovery."),
        )
    };

    // Payloadul rămâne anonim până când targetul final poate fi revendicat
    // create-only. Dacă procesul cade înainte de linkat, kernelul recuperează
    // automat inode-ul la închiderea ultimului descriptor.
    let (mut staged_target, staged_stat, target_identity_digest) = match stage_external_anonymous(
        &parent.directory,
        bytes,
        plan.evidence.target_new_mode_bits,
        &lexical.public_label,
        "target",
    ) {
        Ok(staged) => staged,
        Err(error) => return Ok(recovery(error)),
    };
    let checkpoint = match WalExternalStageCheckpoint::new(target_identity_digest.clone(), None) {
        Ok(checkpoint) => checkpoint,
        Err(error) => return Ok(recovery(error)),
    };
    if let Err(error) = guard.mark_external_auxiliary_durable(checkpoint) {
        return Ok(recovery(error));
    }

    if let Err(error) =
        validate_runtime_before(&parent, &mut plan, leaves, &lexical.public_label, true)
    {
        return Ok(recovery(error));
    }
    if let Err(error) =
        validate_external_auxiliary_absent(&parent.directory, leaves, &lexical.public_label)
    {
        return Ok(recovery(error));
    }

    if let Some((_backup_target, _previous_bytes)) = backup {
        let backup_leaf = leaves.backup.expect("validated backup leaf");
        let backup_evidence = plan
            .evidence
            .backup
            .as_ref()
            .expect("replace plan validated with backup evidence");
        let baseline_mode = plan
            .evidence
            .target_before_mode_bits
            .expect("replace plan validated with baseline mode");
        let baseline_identity = plan
            .evidence
            .target_before_identity_digest
            .as_deref()
            .expect("replace plan validated with baseline identity");
        let old_target = plan
            .existing_target
            .as_mut()
            .expect("replace plan validated with held target");

        if let Err(error) = validate_open_external_baseline(
            old_target,
            &plan.evidence.target.before,
            baseline_mode,
            baseline_identity,
            &parent.directory,
            leaves.target,
            &lexical.public_label,
            "baseline imediat pre-backup",
            true,
        ) {
            return Ok(recovery(error));
        }
        if let Err(error) = validate_leaf_absent_for_external(
            &parent.directory,
            backup_leaf,
            &lexical.public_label,
            "backup destination pre-commit",
        ) {
            return Ok(recovery(error));
        }

        // Backup-ul este chiar inode-ul baseline, nu o copie care ar cere
        // ulterior ștergerea unui al doilea inode. NOREPLACE păstrează orice
        // concurent deja prezent la destinație.
        if let Err(error) = fs::renameat_with(
            &parent.directory,
            leaves.target,
            &parent.directory,
            backup_leaf,
            RenameFlags::NOREPLACE,
        ) {
            return Ok(recovery(capability_error(
                &lexical.public_label,
                &format!("relocarea baseline-ului ExternalConfig în backup a eșuat: {error}"),
            )));
        }
        run_test_hook(CapabilityTestStage::AfterExternalBaselineRelocated);
        // Fereastra în care numele target lipsește este limitată la cele două
        // operații de namespace consecutive: rename baseline -> backup și
        // linkat O_TMPFILE -> target. Niciun fsync sau hash nu rulează între
        // ele. Publicarea este create-only și nu suprascrie un concurent.
        let linked_stat = match publish_external_anonymous(
            &mut staged_target,
            &staged_stat,
            &parent.directory,
            leaves.target,
            &plan.evidence.target,
            plan.evidence.target_new_mode_bits,
            &target_identity_digest,
            &lexical.public_label,
            "target",
            "target final publication",
        ) {
            Ok(stat) => stat,
            Err(error) => {
                let restore = restore_external_source_mapping(
                    &parent.directory,
                    backup_leaf,
                    leaves.target,
                    old_target,
                    &plan.evidence.target.before,
                    baseline_mode,
                    baseline_identity,
                    &lexical.public_label,
                );
                return Ok(recovery(format!("{error} {restore}")));
            }
        };
        if let Err(error) = guard.mark_effect_visible() {
            return Ok(recovery(error));
        }

        if let Err(error) = validate_open_new_payload(
            &mut staged_target,
            &linked_stat,
            &plan.evidence.target,
            plan.evidence.target_new_mode_bits,
            &parent.directory,
            leaves.target,
            &lexical.public_label,
            "target publicat",
        ) {
            return Ok(recovery(error));
        }
        if let Err(error) = validate_open_external_baseline(
            old_target,
            &plan.evidence.target.before,
            baseline_mode,
            baseline_identity,
            &parent.directory,
            backup_leaf,
            &lexical.public_label,
            "backup baseline relocat",
            false,
        ) {
            return Ok(recovery(error));
        }
        if !leaf_matches_payload(
            &plan.evidence.target.before,
            backup_evidence.new_size,
            &backup_evidence.new_content_hash,
        ) {
            return Ok(recovery(capability_error(
                &lexical.public_label,
                "backup evidence nu mai corespunde baseline-ului relocat",
            )));
        }
        if let Err(error) = old_target.sync_all() {
            return Ok(recovery(capability_error(
                &lexical.public_label,
                &format!("fsync backup baseline a eșuat: {error}"),
            )));
        }
        if let Err(error) = sync_directory(&parent.directory, &lexical.public_label) {
            return Ok(recovery(error));
        }

        run_test_hook(CapabilityTestStage::AfterExternalBackupCommitted);

        if let Err(error) = validate_open_new_payload(
            &mut staged_target,
            &linked_stat,
            &plan.evidence.target,
            plan.evidence.target_new_mode_bits,
            &parent.directory,
            leaves.target,
            &lexical.public_label,
            "target post-backup hook",
        ) {
            return Ok(recovery(error));
        }
        if let Err(error) = validate_open_external_baseline(
            old_target,
            &plan.evidence.target.before,
            baseline_mode,
            baseline_identity,
            &parent.directory,
            backup_leaf,
            &lexical.public_label,
            "backup baseline post-hook",
            false,
        ) {
            return Ok(recovery(error));
        }

        run_test_hook(CapabilityTestStage::AfterExternalPublication);

        if let Err(error) = validate_open_new_payload(
            &mut staged_target,
            &linked_stat,
            &plan.evidence.target,
            plan.evidence.target_new_mode_bits,
            &parent.directory,
            leaves.target,
            &lexical.public_label,
            "target committed",
        ) {
            return Ok(recovery(error));
        }
        if let Err(error) = validate_open_external_baseline(
            old_target,
            &plan.evidence.target.before,
            baseline_mode,
            baseline_identity,
            &parent.directory,
            backup_leaf,
            &lexical.public_label,
            "backup committed",
            false,
        ) {
            return Ok(recovery(error));
        }
        if let Err(error) = staged_target.sync_all() {
            return Ok(recovery(capability_error(
                &lexical.public_label,
                &format!("fsync target committed a eșuat: {error}"),
            )));
        }
        if let Err(error) = old_target.sync_all() {
            return Ok(recovery(capability_error(
                &lexical.public_label,
                &format!("fsync backup committed a eșuat: {error}"),
            )));
        }
        if let Err(error) = sync_directory(&parent.directory, &lexical.public_label) {
            return Ok(recovery(error));
        }

        run_test_hook(CapabilityTestStage::BeforeExternalTargetDurable);
        let public_parent =
            match recapture_external_public_parent(&lexical, &plan.evidence, &parent.directory) {
                Ok(parent) => parent,
                Err(error) => return Ok(recovery(error)),
            };
        if let Err(error) = validate_open_external_baseline(
            old_target,
            &plan.evidence.target.before,
            baseline_mode,
            baseline_identity,
            &public_parent.directory,
            backup_leaf,
            &lexical.public_label,
            "backup imediat pre-TargetDurable",
            false,
        ) {
            return Ok(recovery(error));
        }
        if let Err(error) = validate_external_auxiliary_absent(
            &public_parent.directory,
            leaves,
            &lexical.public_label,
        ) {
            return Ok(recovery(error));
        }
        // Targetul public este validat ultimul, după backup și leaf-urile
        // auxiliare, ca fereastra terminală pentru calea activă să fie minimă.
        if let Err(error) = validate_open_new_payload(
            &mut staged_target,
            &linked_stat,
            &plan.evidence.target,
            plan.evidence.target_new_mode_bits,
            &public_parent.directory,
            leaves.target,
            &lexical.public_label,
            "target imediat pre-TargetDurable",
        ) {
            return Ok(recovery(error));
        }
        if let Err(error) = sync_directory(&public_parent.directory, &lexical.public_label) {
            return Ok(recovery(error));
        }
        if let Err(error) = guard.mark_target_durable() {
            return Ok(recovery(error));
        }

        // Markerul terminal nu înlocuiește postflight-ul. Receipt-ul committed
        // este permis numai cât ambele nume sunt încă legate de descriptorii
        // cauzali ținuți din plan/staging.
        let public_parent =
            match recapture_external_public_parent(&lexical, &plan.evidence, &parent.directory) {
                Ok(parent) => parent,
                Err(error) => return Ok(recovery(error)),
            };
        if let Err(error) = validate_open_external_baseline(
            old_target,
            &plan.evidence.target.before,
            baseline_mode,
            baseline_identity,
            &public_parent.directory,
            backup_leaf,
            &lexical.public_label,
            "backup terminal postflight",
            false,
        ) {
            return Ok(recovery(error));
        }
        if let Err(error) = validate_external_auxiliary_absent(
            &public_parent.directory,
            leaves,
            &lexical.public_label,
        ) {
            return Ok(recovery(error));
        }
        if let Err(error) = validate_open_new_payload(
            &mut staged_target,
            &linked_stat,
            &plan.evidence.target,
            plan.evidence.target_new_mode_bits,
            &public_parent.directory,
            leaves.target,
            &lexical.public_label,
            "target terminal postflight",
        ) {
            return Ok(recovery(error));
        }
    } else {
        if let Err(error) = validate_leaf_absent_for_external(
            &parent.directory,
            leaves.target,
            &lexical.public_label,
            "target create-new pre-publication",
        ) {
            return Ok(recovery(error));
        }
        let linked_stat = match publish_external_anonymous(
            &mut staged_target,
            &staged_stat,
            &parent.directory,
            leaves.target,
            &plan.evidence.target,
            plan.evidence.target_new_mode_bits,
            &target_identity_digest,
            &lexical.public_label,
            "target",
            "target create-new publication",
        ) {
            Ok(stat) => stat,
            Err(error) => return Ok(recovery(error)),
        };
        if let Err(error) = guard.mark_effect_visible() {
            return Ok(recovery(error));
        }

        run_test_hook(CapabilityTestStage::AfterExternalPublication);

        if let Err(error) = staged_target.sync_all() {
            return Ok(recovery(capability_error(
                &lexical.public_label,
                &format!("fsync target create-new a eșuat: {error}"),
            )));
        }
        if let Err(error) = sync_directory(&parent.directory, &lexical.public_label) {
            return Ok(recovery(error));
        }
        run_test_hook(CapabilityTestStage::BeforeExternalTargetDurable);
        let public_parent =
            match recapture_external_public_parent(&lexical, &plan.evidence, &parent.directory) {
                Ok(parent) => parent,
                Err(error) => return Ok(recovery(error)),
            };
        if let Err(error) = validate_external_auxiliary_absent(
            &public_parent.directory,
            leaves,
            &lexical.public_label,
        ) {
            return Ok(recovery(error));
        }
        if let Err(error) = validate_open_new_payload(
            &mut staged_target,
            &linked_stat,
            &plan.evidence.target,
            plan.evidence.target_new_mode_bits,
            &public_parent.directory,
            leaves.target,
            &lexical.public_label,
            "target create-new pre-TargetDurable",
        ) {
            return Ok(recovery(error));
        }
        if let Err(error) = sync_directory(&public_parent.directory, &lexical.public_label) {
            return Ok(recovery(error));
        }
        if let Err(error) = guard.mark_target_durable() {
            return Ok(recovery(error));
        }
        let public_parent =
            match recapture_external_public_parent(&lexical, &plan.evidence, &parent.directory) {
                Ok(parent) => parent,
                Err(error) => return Ok(recovery(error)),
            };
        if let Err(error) = validate_external_auxiliary_absent(
            &public_parent.directory,
            leaves,
            &lexical.public_label,
        ) {
            return Ok(recovery(error));
        }
        if let Err(error) = validate_open_new_payload(
            &mut staged_target,
            &linked_stat,
            &plan.evidence.target,
            plan.evidence.target_new_mode_bits,
            &public_parent.directory,
            leaves.target,
            &lexical.public_label,
            "target create-new terminal postflight",
        ) {
            return Ok(recovery(error));
        }
    }

    Ok(CapabilityEffect::changed(bytes.len() as u64))
}

fn validate_external_auxiliary_absent(
    parent: &OwnedFd,
    leaves: ExternalLeaves<'_>,
    public_label: &str,
) -> Result<(), String> {
    for (leaf, role) in external_auxiliary_leaves(leaves) {
        validate_leaf_absent_for_external(parent, leaf, public_label, role)?;
    }
    Ok(())
}

fn restore_external_source_mapping(
    parent: &OwnedFd,
    moved_leaf: &OsStr,
    original_leaf: &OsStr,
    baseline_file: &mut File,
    baseline_evidence: &WalLeafEvidence,
    baseline_mode: u32,
    baseline_identity: &str,
    public_label: &str,
) -> String {
    if let Err(error) = validate_open_external_baseline(
        baseline_file,
        baseline_evidence,
        baseline_mode,
        baseline_identity,
        parent,
        moved_leaf,
        public_label,
        "restore baseline preflight",
        false,
    ) {
        return format!(
            "Numele sursă nu a fost restaurat: backup-ul nu mai indică descriptorul baseline cauzal: {error}"
        );
    }
    if let Err(error) =
        validate_leaf_absent_for_external(parent, original_leaf, public_label, "restore target")
    {
        return format!(
            "Numele sursă nu a fost restaurat deoarece targetul nu mai este absent: {error}"
        );
    }
    if let Err(error) = fs::renameat_with(
        parent,
        moved_leaf,
        parent,
        original_leaf,
        RenameFlags::NOREPLACE,
    ) {
        return format!("Restaurarea create-only a numelui sursă a eșuat fără overwrite: {error}.");
    }
    if let Err(error) = validate_open_external_baseline(
        baseline_file,
        baseline_evidence,
        baseline_mode,
        baseline_identity,
        parent,
        original_leaf,
        public_label,
        "restore baseline postflight",
        false,
    ) {
        return format!(
            "Numele sursă a fost mutat la target, dar postflight-ul descriptor-bound a detectat substituție: {error}"
        );
    }
    if let Err(error) = baseline_file.sync_all() {
        return format!("Numele sursă a fost restaurat, dar fsync baseline a eșuat: {error}");
    }
    match sync_directory(parent, public_label) {
        Ok(()) => "Descriptorul baseline cauzal a fost restaurat create-only la target; WAL rămâne pentru audit.".into(),
        Err(error) => {
            format!("Numele sursă a fost restaurat, dar fsync directory a eșuat: {error}")
        }
    }
}
