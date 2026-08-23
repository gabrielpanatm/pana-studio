use super::{
    capability_error, capture_boundary, capture_existing_boundary, capture_existing_target_parent,
    capture_existing_target_parent_from_directory, capture_target_parent,
    capture_target_parent_from_directory, decode_component_hex, fingerprint_directory_tree, fs,
    leaf_metadata, lexical_target, open_directory_strict, open_or_create_directory_component,
    run_test_hook, same_file_identity, same_stable_leaf_version,
    settle_after_implicit_parent_creation, sync_directory, validate_expected_content,
    validate_expected_regular_file, validate_named_directory_identity,
    validate_named_file_identity, validate_open_directory_identity, validate_regular_single_link,
    version_token_for_stat, wal_identity_from_fd, wal_recovery_effect, AtFlags, CapabilityEffect,
    CapabilityTestStage, CapturedParent, DirectoryOperationPlan, DurableWalGuard, Errno,
    ExpectedLeaf, ExpectedLeafVersion, File, FileType, FlockOperation, LexicalTarget, Mode, OFlags,
    Ordering, OsStr, OsString, OwnedFd, RenameFlags, WalFilesystemIdentity, WriteTarget,
    DIRECTORY_MODE, FILE_MODE, QUARANTINE_SEQUENCE,
};
use std::io::Write;

pub(in crate::kernel::write_authority::capability) fn append(
    target: &WriteTarget,
    bytes: &[u8],
) -> Result<CapabilityEffect, String> {
    let lexical = lexical_target(target, false)?;
    let parent = match capture_target_parent(&lexical, true) {
        Ok(Some(parent)) => parent,
        Ok(None) => {
            return Err(capability_error(
                &lexical.public_label,
                "folderul părinte nu a putut fi capturat",
            ));
        }
        Err(error) => return error.into_operation_result(),
    };
    run_test_hook(CapabilityTestStage::AfterTargetParentCaptured);

    let leaf_was_absent =
        match leaf_metadata(&parent.directory, &parent.leaf, &lexical.public_label) {
            Ok(metadata) => metadata.is_none(),
            Err(error) => {
                return settle_after_implicit_parent_creation(
                    parent.created_ancestors,
                    Err(error),
                    &lexical.public_label,
                );
            }
        };

    let descriptor = match fs::openat(
        &parent.directory,
        &parent.leaf,
        OFlags::WRONLY
            | OFlags::APPEND
            | OFlags::CREATE
            | OFlags::NOFOLLOW
            | OFlags::NONBLOCK
            | OFlags::CLOEXEC,
        FILE_MODE,
    ) {
        Ok(descriptor) => descriptor,
        Err(error) => {
            return settle_after_implicit_parent_creation(
                parent.created_ancestors,
                Err(capability_error(
            &lexical.public_label,
            &format!(
                "AppendText cere un fișier regular deschis fără symlink; leaf-ul a fost refuzat: {error}"
            ),
                )),
                &lexical.public_label,
            );
        }
    };
    run_test_hook(CapabilityTestStage::AfterAppendLeafOpened);
    let result = (|| {
        validate_regular_single_link(&descriptor, &lexical.public_label, "AppendText")?;
        fs::flock(&descriptor, FlockOperation::LockExclusive).map_err(|error| {
            capability_error(
                &lexical.public_label,
                &format!("append-ul nu a putut obține lock exclusiv: {error}"),
            )
        })?;
        // Revalidate after acquiring the cooperative writer lock. The lock
        // serializes Pană Studio writers; the second fstat also closes the
        // interval between the initial type check and lock acquisition.
        validate_regular_single_link(&descriptor, &lexical.public_label, "AppendText")?;

        let mut file = File::from(descriptor);
        if let Err(error) = file.write_all(bytes) {
            let sync_diagnostic = file
                .sync_data()
                .err()
                .map(|sync_error| format!("; sync_data a eșuat de asemenea: {sync_error}"))
                .unwrap_or_default();
            return Ok(CapabilityEffect::recovery_required(
                0,
                capability_error(
                    &lexical.public_label,
                    &format!(
                        "append-ul poate fi parțial după eroarea de scriere: {error}{sync_diagnostic}. Nu repeta recordul automat"
                    ),
                ),
            ));
        }
        if let Err(error) = file.sync_data() {
            return Ok(CapabilityEffect::recovery_required(
                bytes.len() as u64,
                capability_error(
                    &lexical.public_label,
                    &format!(
                        "append-ul este vizibil, dar sync_data a eșuat: {error}. Nu repeta recordul automat"
                    ),
                ),
            ));
        }
        if let Err(error) = sync_directory(&parent.directory, &lexical.public_label) {
            return Ok(CapabilityEffect::recovery_required(
                bytes.len() as u64,
                format!("{error} Append-ul este deja vizibil; nu repeta recordul automat."),
            ));
        }

        Ok(CapabilityEffect::changed(bytes.len() as u64))
    })();
    settle_after_implicit_parent_creation(
        parent.created_ancestors || leaf_was_absent,
        result,
        &lexical.public_label,
    )
}

pub(super) fn create_legacy_directory_all_wal(
    target: &WriteTarget,
    plan: &DirectoryOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    let lexical = lexical_target(target, true)?;
    let planned_components = plan
        .evidence
        .relative_components_hex
        .iter()
        .map(|component| decode_component_hex(component))
        .collect::<Result<Vec<_>, _>>()?;
    if planned_components != lexical.relative_components {
        return Err(capability_error(
            &lexical.public_label,
            "path-ul mkdir nu corespunde planului WAL",
        ));
    }
    let authority = lexical.authority.as_ref().ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "execuția mkdir WAL cere authority root sigilat",
        )
    })?;
    let mut directory = rustix::io::dup(authority.directory()).map_err(|error| {
        capability_error(
            &lexical.public_label,
            &format!("authority mkdir nu poate fi duplicată: {error}"),
        )
    })?;
    for component in lexical
        .relative_components
        .iter()
        .take(plan.evidence.existing_prefix_len)
    {
        directory = open_directory_strict(&directory, component).map_err(|error| {
            capability_error(
                &lexical.public_label,
                &format!("baseline-ul mkdir nu mai poate fi capturat: {error}"),
            )
        })?;
    }
    if wal_identity_from_fd(&directory, &lexical.public_label)?
        != plan.evidence.existing_ancestor_identity
    {
        return Err(capability_error(
            &lexical.public_label,
            "ancestorul mkdir diferă de identitatea din planul WAL",
        ));
    }

    if plan.evidence.existing_prefix_len == lexical.relative_components.len() {
        let observed = wal_identity_from_fd(&directory, &lexical.public_label)?;
        if plan.evidence.existing_target_identity.as_ref() != Some(&observed) {
            return Err(capability_error(
                &lexical.public_label,
                "directorul existent s-a schimbat după planificare",
            ));
        }
        validate_directory_runtime_postflight(&lexical, &observed)?;
        return Ok(CapabilityEffect::unchanged());
    }

    let mut changed = false;
    for component in lexical
        .relative_components
        .iter()
        .skip(plan.evidence.existing_prefix_len)
    {
        match open_directory_strict(&directory, component) {
            Err(Errno::NOENT) => {}
            Ok(_) => {
                let diagnostic = capability_error(
                    &lexical.public_label,
                    "un component mkdir planificat absent a apărut înaintea efectului",
                );
                return if changed {
                    Ok(wal_recovery_effect(0, &lexical.public_label, diagnostic))
                } else {
                    Err(diagnostic)
                };
            }
            Err(error) => {
                let diagnostic = capability_error(
                    &lexical.public_label,
                    &format!("componentul mkdir nu poate fi reverificat: {error}"),
                );
                return if changed {
                    Ok(wal_recovery_effect(0, &lexical.public_label, diagnostic))
                } else {
                    Err(diagnostic)
                };
            }
        }
        if let Err(error) = fs::mkdirat(&directory, component, DIRECTORY_MODE) {
            let diagnostic = capability_error(
                &lexical.public_label,
                &format!("mkdirat protejat de WAL a eșuat: {error}"),
            );
            return if changed {
                Ok(wal_recovery_effect(0, &lexical.public_label, diagnostic))
            } else {
                Err(diagnostic)
            };
        }
        run_test_hook(CapabilityTestStage::AfterDirectoryCreateBeforePhase);
        let first_effect = !changed;
        changed = true;
        let next = match open_directory_strict(&directory, component) {
            Ok(next) => next,
            Err(error) => {
                return Ok(wal_recovery_effect(
                    0,
                    &lexical.public_label,
                    format!("directorul creat nu poate fi recapturat: {error}"),
                ));
            }
        };
        if let Err(error) = validate_named_directory_identity(
            &directory,
            component,
            &next,
            &lexical.public_label,
            "mkdir WAL component",
        ) {
            return Ok(wal_recovery_effect(0, &lexical.public_label, error));
        }
        if let Err(error) = sync_directory(&directory, &lexical.public_label) {
            return Ok(wal_recovery_effect(0, &lexical.public_label, error));
        }
        if first_effect {
            if let Err(error) = guard.mark_auxiliary_durable() {
                return Ok(wal_recovery_effect(0, &lexical.public_label, error));
            }
            if let Err(error) = guard.mark_effect_visible() {
                return Ok(wal_recovery_effect(0, &lexical.public_label, error));
            }
        }
        directory = next;
    }
    if let Err(error) = sync_directory(&directory, &lexical.public_label) {
        return Ok(wal_recovery_effect(0, &lexical.public_label, error));
    }
    run_test_hook(CapabilityTestStage::BeforeDirectoryTargetDurable);
    let final_identity = match wal_identity_from_fd(&directory, &lexical.public_label) {
        Ok(identity) => identity,
        Err(error) => {
            return Ok(wal_recovery_effect(0, &lexical.public_label, error));
        }
    };
    if let Err(error) = validate_directory_runtime_postflight(&lexical, &final_identity) {
        return Ok(wal_recovery_effect(0, &lexical.public_label, error));
    }
    if let Err(error) = guard.mark_target_durable() {
        return Ok(wal_recovery_effect(0, &lexical.public_label, error));
    }
    Ok(CapabilityEffect::changed(0))
}

pub(super) fn validate_directory_runtime_postflight(
    lexical: &LexicalTarget,
    expected_target: &WalFilesystemIdentity,
) -> Result<(), String> {
    let authority = lexical.authority.as_ref().ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "mkdir postflight cere authority root sigilat",
        )
    })?;
    let mut directory = rustix::io::dup(authority.directory()).map_err(|error| {
        capability_error(
            &lexical.public_label,
            &format!("mkdir postflight nu poate duplica authority: {error}"),
        )
    })?;
    for component in &lexical.relative_components {
        let next = open_directory_strict(&directory, component).map_err(|error| {
            capability_error(
                &lexical.public_label,
                &format!("mkdir postflight nu poate recaptura path-ul: {error}"),
            )
        })?;
        validate_named_directory_identity(
            &directory,
            component,
            &next,
            &lexical.public_label,
            "mkdir postflight component",
        )?;
        directory = next;
    }
    let observed = wal_identity_from_fd(&directory, &lexical.public_label)?;
    if &observed != expected_target {
        return Err(capability_error(
            &lexical.public_label,
            "mkdir postflight path-ul nu mai numește inode-ul sincronizat",
        ));
    }
    Ok(())
}

pub(in crate::kernel::write_authority::capability) fn create_directory_all(
    target: &WriteTarget,
) -> Result<CapabilityEffect, String> {
    let lexical = lexical_target(target, true)?;
    let mut boundary = match capture_boundary(&lexical, true) {
        Ok(Some(boundary)) => boundary,
        Ok(None) => {
            return Err(capability_error(
                &lexical.public_label,
                "boundary-ul nu a putut fi capturat sau creat",
            ));
        }
        Err(error) => return error.into_operation_result(),
    };
    let mut changed = boundary.created;

    for component in &lexical.relative_components {
        let (next, created) = match open_or_create_directory_component(
            &boundary.directory,
            component,
            &lexical.public_label,
        ) {
            Ok(result) => result,
            Err(error) => {
                return if changed {
                    error.promote().into_operation_result()
                } else {
                    error.into_operation_result()
                };
            }
        };
        changed |= created;
        boundary.directory = next;
    }
    if let Err(error) = sync_directory(&boundary.directory, &lexical.public_label) {
        if changed {
            return Ok(CapabilityEffect::recovery_required(
                0,
                format!(
                    "{error} Directorul a fost creat, dar durabilitatea lui cere recovery; nu repeta operația automat."
                ),
            ));
        }
        return Err(error);
    }

    Ok(CapabilityEffect {
        changed,
        bytes_written: 0,
        recovery_required: false,
        diagnostic: None,
    })
}

pub(in crate::kernel::write_authority::capability) fn remove_file_if_exists(
    target: &WriteTarget,
) -> Result<CapabilityEffect, String> {
    let lexical = lexical_target(target, false)?;
    let Some(parent) = capture_existing_target_parent(&lexical)? else {
        return Ok(CapabilityEffect::unchanged());
    };
    run_test_hook(CapabilityTestStage::AfterTargetParentCaptured);

    if let ExpectedLeaf::Present(expected) = &target.expected_leaf {
        return remove_expected_file(&parent, expected, &lexical.public_label);
    }

    let Some(metadata) = leaf_metadata(&parent.directory, &parent.leaf, &lexical.public_label)?
    else {
        return Ok(CapabilityEffect::unchanged());
    };
    if FileType::from_raw_mode(metadata.st_mode) == FileType::Directory {
        return Err(capability_error(
            &lexical.public_label,
            "RemoveFile a primit un director; folosește RemoveDirectoryTree",
        ));
    }

    match fs::unlinkat(&parent.directory, &parent.leaf, AtFlags::empty()) {
        Ok(()) => match sync_directory(&parent.directory, &lexical.public_label) {
            Ok(()) => Ok(CapabilityEffect::changed(0)),
            Err(error) => Ok(CapabilityEffect::recovery_required(
                0,
                format!("{error} Leaf-ul a fost eliminat; nu repeta operația automat."),
            )),
        },
        Err(Errno::NOENT) => Ok(CapabilityEffect::unchanged()),
        Err(error) => Err(capability_error(
            &lexical.public_label,
            &format!("leaf-ul nu a putut fi eliminat: {error}"),
        )),
    }
}

pub(super) fn remove_expected_file(
    parent: &CapturedParent,
    expected: &ExpectedLeafVersion,
    public_label: &str,
) -> Result<CapabilityEffect, String> {
    let descriptor = fs::openat(
        &parent.directory,
        &parent.leaf,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| {
        capability_error(
            public_label,
            &format!("leaf-CAS remove nu a putut captura target-ul: {error}"),
        )
    })?;
    let mut captured = File::from(descriptor);
    let captured_before = fs::fstat(&captured).map_err(|error| {
        capability_error(
            public_label,
            &format!("leaf-CAS remove nu a putut citi metadata: {error}"),
        )
    })?;
    validate_expected_regular_file(
        &mut captured,
        &captured_before,
        expected,
        public_label,
        "remove pre-commit",
    )?;
    run_test_hook(CapabilityTestStage::AfterExpectedLeafCaptured);

    let quarantine_name = quarantine_leaf_noreplace(&parent.directory, &parent.leaf, public_label)?;
    let quarantined = match fs::statat(
        &parent.directory,
        &quarantine_name,
        AtFlags::SYMLINK_NOFOLLOW,
    ) {
        Ok(stat) => stat,
        Err(error) => {
            return Ok(CapabilityEffect::recovery_required(
                0,
                capability_error(
                    public_label,
                    &format!(
                        "leaf-ul mutat în {} nu poate fi verificat: {error}; recovery necesar",
                        quarantine_name.to_string_lossy()
                    ),
                ),
            ));
        }
    };
    let validation = (|| {
        if FileType::from_raw_mode(quarantined.st_mode) != FileType::RegularFile
            || !same_file_identity(&captured_before, &quarantined)
        {
            return Err(capability_error(
                public_label,
                "remove ar elimina alt inode decât disk baseline-ul capturat",
            ));
        }
        let captured_after = fs::fstat(&captured).map_err(|error| {
            capability_error(
                public_label,
                &format!("leaf-ul quarantine nu mai poate fi verificat: {error}"),
            )
        })?;
        if !same_stable_leaf_version(&captured_before, &captured_after) {
            return Err(capability_error(
                public_label,
                "leaf-ul s-a modificat în timpul remove-ului condițional",
            ));
        }
        validate_expected_content(
            &mut captured,
            &captured_after,
            expected.content_hash.as_deref(),
            public_label,
            "remove post-quarantine",
        )?;
        let captured_final = fs::fstat(&captured).map_err(|error| {
            capability_error(
                public_label,
                &format!("leaf-ul quarantine nu poate fi reverificat: {error}"),
            )
        })?;
        if version_token_for_stat(&captured_after) != version_token_for_stat(&captured_final) {
            return Err(capability_error(
                public_label,
                "leaf-ul a suferit o schimbare ABA în timpul postflight-ului remove",
            ));
        }
        Ok(())
    })();

    if let Err(conflict) = validation {
        return restore_leaf_after_conflict(
            &parent.directory,
            &parent.leaf,
            &quarantine_name,
            &quarantined,
            public_label,
            conflict,
        );
    }
    if let Err(error) = validate_named_file_identity(
        &parent.directory,
        &quarantine_name,
        &captured_before,
        "remove-quarantine",
    ) {
        return Ok(CapabilityEffect::recovery_required(
            0,
            format!(
                "{error} Quarantine {} cere recovery; nu repeta remove-ul automat.",
                quarantine_name.to_string_lossy()
            ),
        ));
    }
    if let Err(error) = fs::unlinkat(&parent.directory, &quarantine_name, AtFlags::empty()) {
        return Ok(CapabilityEffect::recovery_required(
            0,
            capability_error(
                public_label,
                &format!(
                    "leaf-ul este izolat în {}, dar unlink a eșuat: {error}; nu repeta remove-ul automat",
                    quarantine_name.to_string_lossy()
                ),
            ),
        ));
    }
    match sync_directory(&parent.directory, public_label) {
        Ok(()) => Ok(CapabilityEffect::changed(0)),
        Err(error) => Ok(CapabilityEffect::recovery_required(
            0,
            format!(
                "{error} Remove-ul leaf-CAS este vizibil, dar durabilitatea este incertă; nu repeta operația automat."
            ),
        )),
    }
}

pub(super) fn quarantine_leaf_noreplace(
    parent: &OwnedFd,
    source_name: &OsStr,
    public_label: &str,
) -> Result<OsString, String> {
    for _ in 0..32 {
        let sequence = QUARANTINE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let quarantine_name = OsString::from(format!(
            ".pana-capability-leaf-{}-{sequence}.quarantine",
            std::process::id()
        ));
        match fs::renameat_with(
            parent,
            source_name,
            parent,
            &quarantine_name,
            RenameFlags::NOREPLACE,
        ) {
            Ok(()) => return Ok(quarantine_name),
            Err(Errno::EXIST) => continue,
            Err(error) => {
                return Err(capability_error(
                    public_label,
                    &format!("leaf-ul nu a putut intra în quarantine: {error}"),
                ));
            }
        }
    }
    Err(capability_error(
        public_label,
        "nu a putut fi rezervat un nume leaf quarantine unic",
    ))
}

pub(super) fn restore_leaf_after_conflict(
    parent: &OwnedFd,
    original_name: &OsStr,
    quarantine_name: &OsStr,
    expected_identity: &fs::Stat,
    public_label: &str,
    conflict: String,
) -> Result<CapabilityEffect, String> {
    if let Err(error) = fs::renameat_with(
        parent,
        quarantine_name,
        parent,
        original_name,
        RenameFlags::NOREPLACE,
    ) {
        return Ok(CapabilityEffect::recovery_required(
            0,
            capability_error(
                public_label,
                &format!(
                    "{conflict} Restaurarea din {} a eșuat: {error}; recovery necesar și fără retry automat",
                    quarantine_name.to_string_lossy()
                ),
            ),
        ));
    }
    let restored = fs::statat(parent, original_name, AtFlags::SYMLINK_NOFOLLOW);
    if !matches!(restored, Ok(ref stat) if same_file_identity(stat, expected_identity)) {
        return Ok(CapabilityEffect::recovery_required(
            0,
            capability_error(
                public_label,
                &format!(
                    "{conflict} Numele original nu mai poate demonstra inode-ul restaurat; recovery necesar"
                ),
            ),
        ));
    }
    if let Err(error) = sync_directory(parent, public_label) {
        return Ok(CapabilityEffect::recovery_required(
            0,
            format!(
                "{conflict} Leaf-ul a fost restaurat, dar fsync rollback a eșuat: {error} Nu repeta operația automat."
            ),
        ));
    }
    Err(format!(
        "{conflict} Remove-ul leaf-CAS a fost anulat, iar versiunea concurentă a fost restaurată."
    ))
}

pub(in crate::kernel::write_authority::capability) fn rename_noreplace(
    source: &WriteTarget,
    destination: &WriteTarget,
) -> Result<CapabilityEffect, String> {
    let source_lexical = lexical_target(source, false)?;
    let destination_lexical = lexical_target(destination, false)?;
    let (source_parent, destination_parent) = if source.boundary_root == destination.boundary_root {
        // A rename inside one authority must resolve both names from the
        // same captured boundary object. Capturing the absolute boundary
        // twice would reopen a race in which the path is replaced between
        // source and destination acquisition.
        let boundary = capture_existing_boundary(&source_lexical)?.ok_or_else(|| {
            capability_error(&source_lexical.public_label, "boundary-ul sursei nu există")
        })?;
        let source_base = rustix::io::dup(&boundary.directory).map_err(|error| {
            capability_error(
                &source_lexical.public_label,
                &format!("boundary-ul comun nu a putut fi duplicat: {error}"),
            )
        })?;
        let source_parent =
            capture_existing_target_parent_from_directory(&source_lexical, source_base)?
                .ok_or_else(|| {
                    capability_error(
                        &source_lexical.public_label,
                        "folderul părinte al sursei nu există",
                    )
                })?;
        run_test_hook(CapabilityTestStage::AfterRenameSourceParentCaptured);
        let destination_parent = match capture_target_parent_from_directory(
            &destination_lexical,
            boundary.directory,
            true,
            false,
        ) {
            Ok(Some(parent)) => parent,
            Ok(None) => {
                return Err(capability_error(
                    &destination_lexical.public_label,
                    "folderul părinte al destinației nu a putut fi capturat",
                ));
            }
            Err(error) => return error.into_operation_result(),
        };
        (source_parent, destination_parent)
    } else {
        let source_parent = capture_existing_target_parent(&source_lexical)?.ok_or_else(|| {
            capability_error(&source_lexical.public_label, "boundary-ul sursei nu există")
        })?;
        run_test_hook(CapabilityTestStage::AfterRenameSourceParentCaptured);
        let destination_parent = match capture_target_parent(&destination_lexical, true) {
            Ok(Some(parent)) => parent,
            Ok(None) => {
                return Err(capability_error(
                    &destination_lexical.public_label,
                    "folderul părinte al destinației nu a putut fi capturat",
                ));
            }
            Err(error) => return error.into_operation_result(),
        };
        (source_parent, destination_parent)
    };
    let result = if let ExpectedLeaf::Present(expected) = &source.expected_leaf {
        rename_expected_noreplace(
            &source_parent,
            &destination_parent,
            expected,
            &source_lexical.public_label,
            &destination_lexical.public_label,
        )
    } else {
        (|| {
            run_test_hook(CapabilityTestStage::BeforeRename);

            fs::renameat_with(
                &source_parent.directory,
                &source_parent.leaf,
                &destination_parent.directory,
                &destination_parent.leaf,
                RenameFlags::NOREPLACE,
            )
            .map_err(|error| {
                capability_error(
                    &source_lexical.public_label,
                    &format!(
                        "rename către {} a fost refuzat fără suprascriere: {error}",
                        destination_lexical.public_label
                    ),
                )
            })?;
            let mut diagnostics = Vec::new();
            if let Err(error) =
                sync_directory(&source_parent.directory, &source_lexical.public_label)
            {
                diagnostics.push(error);
            }
            if let Err(error) = sync_directory(
                &destination_parent.directory,
                &destination_lexical.public_label,
            ) {
                diagnostics.push(error);
            }

            if diagnostics.is_empty() {
                Ok(CapabilityEffect::changed(0))
            } else {
                Ok(CapabilityEffect::recovery_required(
                    0,
                    format!(
                        "Rename-ul este deja vizibil, dar sincronizarea directoarelor a eșuat: {} Nu repeta operația automat.",
                        diagnostics.join(" ")
                    ),
                ))
            }
        })()
    };
    settle_after_implicit_parent_creation(
        destination_parent.created_ancestors,
        result,
        &destination_lexical.public_label,
    )
}

/// Atomically publishes a complete rebuildable directory. Both names
/// must be sibling leaves under one sealed authority. If a previous
/// artifact exists Linux exchanges the two directory names in one syscall;
/// otherwise NOREPLACE closes the absent-destination race.
pub(in crate::kernel::write_authority::capability) fn publish_rebuildable_directory(
    source: &WriteTarget,
    destination: &WriteTarget,
) -> Result<CapabilityEffect, String> {
    let source_lexical = lexical_target(source, false)?;
    let destination_lexical = lexical_target(destination, false)?;
    if source.boundary_root != destination.boundary_root
        || source_lexical.relative_components.len() != 1
        || destination_lexical.relative_components.len() != 1
        || !matches!(
            (source.authority(), destination.authority()),
            (Some(left), Some(right)) if left.same_authority(right)
        )
    {
        return Err(capability_error(
            &source_lexical.public_label,
            "publicarea rebuildable cere două leaf-uri sibling sub aceeași authority sigilată",
        ));
    }

    let boundary = capture_existing_boundary(&source_lexical)?.ok_or_else(|| {
        capability_error(
            &source_lexical.public_label,
            "authority root pentru publicare nu mai există",
        )
    })?;
    let source_leaf = &source_lexical.relative_components[0];
    let destination_leaf = &destination_lexical.relative_components[0];
    if source_leaf == destination_leaf {
        return Err(capability_error(
            &source_lexical.public_label,
            "generația staged și artifactul public nu pot avea același nume",
        ));
    }

    let source_before = fs::statat(&boundary.directory, source_leaf, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|error| {
            capability_error(
                &source_lexical.public_label,
                &format!("generația staged nu poate fi capturată: {error}"),
            )
        })?;
    if FileType::from_raw_mode(source_before.st_mode) != FileType::Directory {
        return Err(capability_error(
            &source_lexical.public_label,
            "generația staged nu este un director real",
        ));
    }
    let source_directory =
        open_directory_strict(&boundary.directory, source_leaf).map_err(|error| {
            capability_error(
                &source_lexical.public_label,
                &format!("generația staged nu poate fi deschisă sigur: {error}"),
            )
        })?;
    validate_open_directory_identity(
        &source_directory,
        &source_before,
        &source_lexical.public_label,
        "rebuildable publication source",
    )?;

    let previous = match fs::statat(
        &boundary.directory,
        destination_leaf,
        AtFlags::SYMLINK_NOFOLLOW,
    ) {
        Ok(stat) => {
            if FileType::from_raw_mode(stat.st_mode) != FileType::Directory {
                return Err(capability_error(
                    &destination_lexical.public_label,
                    "artifactul existent nu este un director real",
                ));
            }
            let directory =
                open_directory_strict(&boundary.directory, destination_leaf).map_err(|error| {
                    capability_error(
                        &destination_lexical.public_label,
                        &format!("artifactul existent nu poate fi deschis sigur: {error}"),
                    )
                })?;
            validate_open_directory_identity(
                &directory,
                &stat,
                &destination_lexical.public_label,
                "rebuildable publication destination",
            )?;
            Some((stat, directory))
        }
        Err(Errno::NOENT) => None,
        Err(error) => {
            return Err(capability_error(
                &destination_lexical.public_label,
                &format!("artifactul existent nu poate fi inspectat: {error}"),
            ));
        }
    };

    let flags = if previous.is_some() {
        RenameFlags::EXCHANGE
    } else {
        RenameFlags::NOREPLACE
    };
    fs::renameat_with(
        &boundary.directory,
        source_leaf,
        &boundary.directory,
        destination_leaf,
        flags,
    )
    .map_err(|error| {
        capability_error(
            &destination_lexical.public_label,
            &format!(
                "commit-ul atomic al generației Zola a fost refuzat; artifactul precedent rămâne publicat: {error}"
            ),
        )
    })?;

    let postflight = (|| {
        validate_named_directory_identity(
            &boundary.directory,
            destination_leaf,
            &source_directory,
            &destination_lexical.public_label,
            "rebuildable publication committed destination",
        )?;
        if let Some((_stat, previous_directory)) = &previous {
            validate_named_directory_identity(
                &boundary.directory,
                source_leaf,
                previous_directory,
                &source_lexical.public_label,
                "rebuildable publication exchanged previous artifact",
            )?;
        } else if leaf_metadata(
            &boundary.directory,
            source_leaf,
            &source_lexical.public_label,
        )?
        .is_some()
        {
            return Err(capability_error(
                &source_lexical.public_label,
                "numele staged trebuia să fie absent după publicare",
            ));
        }
        sync_directory(&boundary.directory, &destination_lexical.public_label)
    })();
    match postflight {
        Ok(()) => Ok(CapabilityEffect::changed(0)),
        Err(error) => Ok(CapabilityEffect::recovery_required(
            0,
            format!("{error} Commit-ul poate fi deja vizibil; nu repeta publicarea automat."),
        )),
    }
}

pub(super) fn rename_expected_noreplace(
    source: &CapturedParent,
    destination: &CapturedParent,
    expected: &ExpectedLeafVersion,
    source_label: &str,
    destination_label: &str,
) -> Result<CapabilityEffect, String> {
    let handle = fs::openat(
        &source.directory,
        &source.leaf,
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| {
        capability_error(
            source_label,
            &format!("rename leaf-CAS nu a putut captura sursa: {error}"),
        )
    })?;
    let before = fs::fstat(&handle).map_err(|error| {
        capability_error(
            source_label,
            &format!("rename leaf-CAS nu a putut citi metadata sursei: {error}"),
        )
    })?;
    let observed_token = version_token_for_stat(&before);
    if observed_token != expected.version_token {
        return Err(capability_error(
            source_label,
            &format!(
                "rename disk baseline s-a schimbat înainte de commit (expected {}, observed {})",
                expected.version_token, observed_token
            ),
        ));
    }
    let source_type = FileType::from_raw_mode(before.st_mode);
    let mut content_file = if expected.content_hash.is_some() {
        if source_type != FileType::RegularFile {
            return Err(capability_error(
                source_label,
                "rename cu content hash cere o sursă regular file",
            ));
        }
        let descriptor = fs::openat(
            &source.directory,
            &source.leaf,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| {
            capability_error(
                source_label,
                &format!("rename nu a putut deschide conținutul sursei: {error}"),
            )
        })?;
        let mut file = File::from(descriptor);
        let readable_stat = fs::fstat(&file).map_err(|error| {
            capability_error(
                source_label,
                &format!("rename nu a putut verifica descriptorul sursei: {error}"),
            )
        })?;
        if !same_file_identity(&before, &readable_stat) {
            return Err(capability_error(
                source_label,
                "rename a observat alt inode între handle și descriptorul de conținut",
            ));
        }
        validate_expected_content(
            &mut file,
            &readable_stat,
            expected.content_hash.as_deref(),
            source_label,
            "rename pre-commit",
        )?;
        Some(file)
    } else {
        None
    };
    let captured_directory = if let Some(expected_tree) = expected.tree_fingerprint.as_deref() {
        if source_type != FileType::Directory {
            return Err(capability_error(
                source_label,
                "rename cu tree fingerprint cere o sursă director",
            ));
        }
        let directory =
            open_directory_strict(&source.directory, &source.leaf).map_err(|error| {
                capability_error(
                    source_label,
                    &format!("directorul lifecycle nu poate fi capturat: {error}"),
                )
            })?;
        validate_open_directory_identity(&directory, &before, source_label, "rename tree source")?;
        let observed_tree = fingerprint_directory_tree(&directory, source_label)?;
        if observed_tree != expected_tree {
            return Err(capability_error(
                source_label,
                &format!(
                    "descendenții sursei s-au schimbat înainte de rename (expected {expected_tree}, observed {observed_tree})"
                ),
            ));
        }
        Some(directory)
    } else {
        if source_type == FileType::Directory {
            return Err(capability_error(
                source_label,
                "rename RequireDiskBaseline pentru director cere tree fingerprint",
            ));
        }
        None
    };

    run_test_hook(CapabilityTestStage::BeforeRename);
    fs::renameat_with(
        &source.directory,
        &source.leaf,
        &destination.directory,
        &destination.leaf,
        RenameFlags::NOREPLACE,
    )
    .map_err(|error| {
        capability_error(
            source_label,
            &format!(
                "rename leaf-CAS către {destination_label} a fost refuzat fără suprascriere: {error}"
            ),
        )
    })?;

    let moved = match fs::statat(
        &destination.directory,
        &destination.leaf,
        AtFlags::SYMLINK_NOFOLLOW,
    ) {
        Ok(stat) => stat,
        Err(error) => {
            return Ok(CapabilityEffect::recovery_required(
                0,
                capability_error(
                    source_label,
                    &format!(
                        "rename-ul este vizibil, dar destinația nu poate fi verificată: {error}; recovery necesar și fără retry automat"
                    ),
                ),
            ));
        }
    };
    let validation = (|| {
        if !same_file_identity(&before, &moved) {
            return Err(capability_error(
                source_label,
                "rename a mutat alt inode decât sursa capturată",
            ));
        }
        let after = fs::fstat(&handle).map_err(|error| {
            capability_error(
                source_label,
                &format!("sursa mutată nu mai poate fi verificată: {error}"),
            )
        })?;
        if !same_stable_leaf_version(&before, &after) {
            return Err(capability_error(
                source_label,
                "sursa s-a modificat în timpul rename-ului condițional",
            ));
        }
        if let Some(file) = content_file.as_mut() {
            validate_expected_content(
                file,
                &after,
                expected.content_hash.as_deref(),
                source_label,
                "rename post-commit",
            )?;
        }
        if let (Some(directory), Some(expected_tree)) = (
            captured_directory.as_ref(),
            expected.tree_fingerprint.as_deref(),
        ) {
            let observed_tree = fingerprint_directory_tree(directory, source_label)?;
            if observed_tree != expected_tree {
                return Err(capability_error(
                    source_label,
                    &format!(
                        "descendenții sursei s-au schimbat în timpul rename-ului (expected {expected_tree}, observed {observed_tree})"
                    ),
                ));
            }
        }
        let after_validation = fs::fstat(&handle).map_err(|error| {
            capability_error(
                source_label,
                &format!("sursa nu mai poate fi reverificată după postflight: {error}"),
            )
        })?;
        if version_token_for_stat(&after) != version_token_for_stat(&after_validation) {
            return Err(capability_error(
                source_label,
                "sursa a suferit o schimbare ABA în timpul postflight-ului rename",
            ));
        }
        Ok(())
    })();
    if let Err(conflict) = validation {
        return rollback_conditional_rename(source, destination, &moved, source_label, conflict);
    }

    let mut diagnostics = Vec::new();
    if let Err(error) = sync_directory(&source.directory, source_label) {
        diagnostics.push(error);
    }
    if let Err(error) = sync_directory(&destination.directory, destination_label) {
        diagnostics.push(error);
    }
    if diagnostics.is_empty() {
        Ok(CapabilityEffect::changed(0))
    } else {
        Ok(CapabilityEffect::recovery_required(
            0,
            format!(
                "Rename-ul leaf-CAS este vizibil, dar sincronizarea directoarelor a eșuat: {} Nu repeta operația automat.",
                diagnostics.join(" ")
            ),
        ))
    }
}

pub(super) fn rollback_conditional_rename(
    source: &CapturedParent,
    destination: &CapturedParent,
    moved_identity: &fs::Stat,
    source_label: &str,
    conflict: String,
) -> Result<CapabilityEffect, String> {
    if let Err(error) = fs::renameat_with(
        &destination.directory,
        &destination.leaf,
        &source.directory,
        &source.leaf,
        RenameFlags::NOREPLACE,
    ) {
        return Ok(CapabilityEffect::recovery_required(
            0,
            capability_error(
                source_label,
                &format!(
                    "{conflict} Rollback-ul rename a eșuat: {error}; destinația cere recovery și operația nu trebuie repetată automat"
                ),
            ),
        ));
    }
    let restored = fs::statat(&source.directory, &source.leaf, AtFlags::SYMLINK_NOFOLLOW);
    if !matches!(restored, Ok(ref stat) if same_file_identity(stat, moved_identity)) {
        return Ok(CapabilityEffect::recovery_required(
            0,
            capability_error(
                source_label,
                &format!(
                    "{conflict} Rollback-ul rename nu poate demonstra identitatea restaurată; recovery necesar"
                ),
            ),
        ));
    }
    let mut diagnostics = Vec::new();
    if let Err(error) = sync_directory(&source.directory, source_label) {
        diagnostics.push(error);
    }
    if let Err(error) = sync_directory(&destination.directory, source_label) {
        diagnostics.push(error);
    }
    if diagnostics.is_empty() {
        Err(format!(
            "{conflict} Rename-ul leaf-CAS a fost anulat, iar sursa concurentă a fost restaurată."
        ))
    } else {
        Ok(CapabilityEffect::recovery_required(
            0,
            format!(
                "{conflict} Sursa a fost restaurată după conflict, dar rollback-ul nu este confirmat durabil: {} Nu repeta operația automat.",
                diagnostics.join(" ")
            ),
        ))
    }
}
