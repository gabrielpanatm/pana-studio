use super::{
    atomic_commit, atomic_temp_leaf, capability_error, capture_boundary, capture_existing_boundary,
    capture_target_parent, decode_component_hex, encode_component_hex, fs, identity_from_fd,
    leaf_metadata, lexical_target, open_directory_strict, run_test_hook, same_file_identity,
    same_stable_leaf_version, settle_after_implicit_parent_creation, sha256_bytes, sync_directory,
    validate_atomic_destination, validate_expected_regular_file, validate_named_directory_identity,
    validate_named_file_identity, version_token_for_stat, wal_recovery_effect, AppendOperationPlan,
    AtFlags, AtomicOperationPlan, CapabilityEffect, CapabilityReplacePolicy, CapabilityTestStage,
    CaptureFailure, CapturedParent, DurableWalGuard, Errno, ExpectedLeaf, File, FileType,
    LexicalTarget, Mode, OFlags, OsStr, OwnedFd, RecoveryReadBudget, RenameFlags, SeekFrom, Sha256,
    WalAtomicFileEvidence, WalFilesystemIdentity, WalLeafEvidence, WalParentEvidence, WriteTarget,
    DIRECTORY_MODE, FILE_MODE,
};
#[cfg(test)]
use super::{DirectoryOperationPlan, WalDirectoryEvidence};
use sha2::Digest;
use std::io::{Read, Seek, Write};

pub(in crate::kernel::write_authority::capability) fn atomic_write(
    target: &WriteTarget,
    bytes: &[u8],
    replace_policy: CapabilityReplacePolicy,
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
    let result = (|| {
        validate_atomic_destination(&parent.directory, &parent.leaf, replace_policy, &lexical)?;

        atomic_commit(
            &parent.directory,
            &parent.leaf,
            replace_policy,
            &target.expected_leaf,
            &lexical.public_label,
            |file| {
                file.write_all(bytes).map_err(|error| {
                    capability_error(
                        &lexical.public_label,
                        &format!("fișierul temporar nu a putut fi scris: {error}"),
                    )
                })?;
                Ok(bytes.len() as u64)
            },
        )
    })();
    settle_after_implicit_parent_creation(parent.created_ancestors, result, &lexical.public_label)
}

pub(in crate::kernel::write_authority::capability) fn plan_atomic_write(
    target: &WriteTarget,
    bytes: &[u8],
    replace_policy: CapabilityReplacePolicy,
    operation_id: &str,
) -> Result<AtomicOperationPlan, String> {
    let lexical = lexical_target(target, false)?;
    if lexical.authority.is_none() {
        return Err(capability_error(
            &lexical.public_label,
            "planul WAL cere authority root sigilat",
        ));
    }
    let boundary = capture_existing_boundary(&lexical)?.ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "authority root nu există pentru planul atomic",
        )
    })?;
    let (leaf, parents) = lexical.relative_components.split_last().ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "planul atomic cere un leaf sub authority root",
        )
    })?;

    let mut directory = boundary.directory;
    let mut existing_prefix_len = 0_usize;
    for component in parents {
        match open_directory_strict(&directory, component) {
            Ok(next) => {
                directory = next;
                existing_prefix_len += 1;
            }
            Err(Errno::NOENT) => break,
            Err(error) => {
                return Err(capability_error(
                    &lexical.public_label,
                    &format!("planul atomic nu poate captura un părinte: {error}"),
                ));
            }
        }
    }
    let existing_ancestor_identity = wal_identity_from_fd(&directory, &lexical.public_label)?;
    let parent_exists = existing_prefix_len == parents.len();
    let parent_identity = parent_exists
        .then(|| wal_identity_from_fd(&directory, &lexical.public_label))
        .transpose()?;

    let before = if parent_exists {
        validate_atomic_destination(&directory, leaf, replace_policy, &lexical)?;
        capture_wal_leaf_evidence(
            &directory,
            leaf,
            &target.expected_leaf,
            &lexical.public_label,
            None,
        )?
    } else {
        if matches!(target.expected_leaf, ExpectedLeaf::Present(_)) {
            return Err(capability_error(
                &lexical.public_label,
                "disk baseline-ul Present nu poate exista sub un părinte absent",
            ));
        }
        WalLeafEvidence::Absent
    };

    if matches!(target.expected_leaf, ExpectedLeaf::Present(_))
        && matches!(before, WalLeafEvidence::Absent)
    {
        return Err(capability_error(
            &lexical.public_label,
            "target-ul disk baseline Present lipsește la planificare",
        ));
    }

    let temp_leaf = atomic_temp_leaf(operation_id);
    if parent_exists && leaf_metadata(&directory, &temp_leaf, &lexical.public_label)?.is_some() {
        return Err(capability_error(
            &lexical.public_label,
            "numele temp determinist al operației există deja",
        ));
    }

    Ok(AtomicOperationPlan {
        evidence: WalAtomicFileEvidence {
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
            temp_leaf_hex: encode_component_hex(&temp_leaf),
            replace: !matches!(before, WalLeafEvidence::Absent),
            before,
            new_size: bytes.len() as u64,
            new_content_hash: sha256_bytes(bytes),
        },
    })
}

#[cfg(test)]
pub(super) fn plan_legacy_directory(
    target: &WriteTarget,
) -> Result<DirectoryOperationPlan, String> {
    let lexical = lexical_target(target, true)?;
    if lexical.authority.is_none() {
        return Err(capability_error(
            &lexical.public_label,
            "planul mkdir WAL cere authority root sigilat",
        ));
    }
    let boundary = capture_existing_boundary(&lexical)?.ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "authority root nu există pentru planul mkdir",
        )
    })?;
    let mut directory = boundary.directory;
    let mut existing_prefix_len = 0_usize;
    for component in &lexical.relative_components {
        match open_directory_strict(&directory, component) {
            Ok(next) => {
                directory = next;
                existing_prefix_len += 1;
            }
            Err(Errno::NOENT) => break,
            Err(error) => {
                return Err(capability_error(
                    &lexical.public_label,
                    &format!("planul mkdir nu poate captura un component: {error}"),
                ));
            }
        }
    }
    let existing_ancestor_identity = wal_identity_from_fd(&directory, &lexical.public_label)?;
    let target_exists = existing_prefix_len == lexical.relative_components.len();
    Ok(DirectoryOperationPlan {
        evidence: WalDirectoryEvidence {
            protocol_version: 0,
            relative_components_hex: lexical
                .relative_components
                .iter()
                .map(|component| encode_component_hex(component))
                .collect(),
            existing_prefix_len,
            existing_ancestor_identity: existing_ancestor_identity.clone(),
            existing_target_identity: target_exists.then_some(existing_ancestor_identity),
            parent_identity: None,
            target_leaf_hex: None,
            existing_target_identity_digest: None,
            existing_target_version_token: None,
            desired_mode_bits: None,
        },
    })
}

pub(super) fn capture_wal_leaf_evidence(
    parent: &OwnedFd,
    leaf: &OsStr,
    expected_leaf: &ExpectedLeaf,
    public_label: &str,
    read_budget: Option<&mut RecoveryReadBudget>,
) -> Result<WalLeafEvidence, String> {
    let Some(stat) = leaf_metadata(parent, leaf, public_label)? else {
        return Ok(WalLeafEvidence::Absent);
    };
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
        return Err(capability_error(
            public_label,
            "WAL atomic baseline nu este fișier regular",
        ));
    }
    let descriptor = fs::openat(
        parent,
        leaf,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| {
        capability_error(
            public_label,
            &format!("WAL atomic baseline nu poate fi deschis: {error}"),
        )
    })?;
    let mut file = File::from(descriptor);
    let captured = fs::fstat(&file).map_err(|error| {
        capability_error(
            public_label,
            &format!("WAL atomic baseline metadata nu poate fi citită: {error}"),
        )
    })?;
    if !same_file_identity(&stat, &captured) {
        return Err(capability_error(
            public_label,
            "WAL atomic baseline s-a schimbat în timpul capturii",
        ));
    }
    let evidence = wal_evidence_from_open_file(
        &mut file,
        &captured,
        expected_leaf,
        public_label,
        "WAL plan preflight",
        read_budget,
    )?;
    validate_named_file_identity(parent, leaf, &captured, "wal-baseline")?;
    Ok(evidence)
}

pub(super) fn wal_evidence_from_open_file(
    file: &mut File,
    captured: &fs::Stat,
    expected_leaf: &ExpectedLeaf,
    public_label: &str,
    stage: &str,
    read_budget: Option<&mut RecoveryReadBudget>,
) -> Result<WalLeafEvidence, String> {
    if let ExpectedLeaf::Present(expected) = expected_leaf {
        validate_expected_regular_file(file, captured, expected, public_label, stage)?;
    }
    let size = u64::try_from(captured.st_size)
        .map_err(|_| capability_error(public_label, "WAL baseline are dimensiune negativă"))?;
    const MAX_WAL_HASH_BYTES: u64 = 512 * 1024 * 1024;
    if size > MAX_WAL_HASH_BYTES {
        return Err(capability_error(
            public_label,
            &format!("WAL baseline depășește limita de {MAX_WAL_HASH_BYTES} bytes"),
        ));
    }
    if let Some(read_budget) = read_budget {
        read_budget.reserve(size, stage)?;
    }
    file.seek(SeekFrom::Start(0)).map_err(|error| {
        capability_error(
            public_label,
            &format!("WAL baseline nu poate reveni la început: {error}"),
        )
    })?;
    let mut hasher = Sha256::new();
    let mut observed = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| {
            capability_error(
                public_label,
                &format!("WAL baseline nu poate fi hash-uit: {error}"),
            )
        })?;
        if count == 0 {
            break;
        }
        observed = observed.saturating_add(count as u64);
        if observed > size {
            return Err(capability_error(
                public_label,
                "WAL baseline a crescut în timpul hash-ului",
            ));
        }
        hasher.update(&buffer[..count]);
    }
    let captured_after = fs::fstat(&*file).map_err(|error| {
        capability_error(
            public_label,
            &format!("WAL baseline nu poate fi reverificat: {error}"),
        )
    })?;
    if observed != size || !same_stable_leaf_version(captured, &captured_after) {
        return Err(capability_error(
            public_label,
            "WAL baseline s-a modificat în timpul hash-ului",
        ));
    }
    Ok(WalLeafEvidence::Regular {
        identity: WalFilesystemIdentity {
            device: captured.st_dev,
            inode: captured.st_ino,
        },
        size,
        version_token: version_token_for_stat(captured),
        content_hash: format!("{:x}", hasher.finalize()),
    })
}

pub(super) fn wal_identity_from_fd(
    directory: &OwnedFd,
    public_label: &str,
) -> Result<WalFilesystemIdentity, String> {
    let identity = identity_from_fd(directory, public_label)?;
    Ok(WalFilesystemIdentity {
        device: identity.device,
        inode: identity.inode,
    })
}

pub(in crate::kernel::write_authority::capability) fn atomic_write_wal(
    target: &WriteTarget,
    bytes: &[u8],
    replace_policy: CapabilityReplacePolicy,
    plan: &AtomicOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    let lexical = lexical_target(target, false)?;
    validate_atomic_plan_shape(&lexical, bytes, replace_policy, plan, guard.operation_id())?;
    let parent = match capture_atomic_parent_from_plan(&lexical, plan) {
        Ok(parent) => parent,
        Err(error) => return error.into_operation_result(),
    };
    let parent_changed = parent.created_ancestors;
    run_test_hook(CapabilityTestStage::AfterTargetParentCaptured);

    let observed_before = match capture_wal_leaf_evidence(
        &parent.directory,
        &parent.leaf,
        &target.expected_leaf,
        &lexical.public_label,
        None,
    ) {
        Ok(evidence) => evidence,
        Err(error) if parent_changed => {
            return Ok(wal_recovery_effect(
                0,
                &lexical.public_label,
                format!("{error} Părinții planificați au fost deja creați."),
            ));
        }
        Err(error) => return Err(error),
    };
    if observed_before != plan.evidence.before {
        let error = capability_error(
            &lexical.public_label,
            "baseline-ul target diferă de planul WAL înainte de temp create",
        );
        return if parent_changed {
            Ok(wal_recovery_effect(0, &lexical.public_label, error))
        } else {
            Err(error)
        };
    }
    let temp_name = plan.temp_leaf()?;
    match leaf_metadata(&parent.directory, &temp_name, &lexical.public_label) {
        Ok(None) => {}
        Ok(Some(_)) => {
            let error = capability_error(
                &lexical.public_label,
                "temp leaf-ul WAL determinist există înainte de O_EXCL",
            );
            return if parent_changed {
                Ok(wal_recovery_effect(0, &lexical.public_label, error))
            } else {
                Err(error)
            };
        }
        Err(error) => {
            return if parent_changed {
                Ok(wal_recovery_effect(0, &lexical.public_label, error))
            } else {
                Err(error)
            };
        }
    }

    let descriptor = match fs::openat(
        &parent.directory,
        &temp_name,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        FILE_MODE,
    ) {
        Ok(descriptor) => descriptor,
        Err(error) => {
            let diagnostic = capability_error(
                &lexical.public_label,
                &format!("temp leaf-ul WAL nu a putut fi creat exact: {error}"),
            );
            return if parent_changed {
                Ok(wal_recovery_effect(0, &lexical.public_label, diagnostic))
            } else {
                Err(diagnostic)
            };
        }
    };
    let mut temp_file = File::from(descriptor);
    if let Err(error) = temp_file
        .write_all(bytes)
        .and_then(|()| temp_file.sync_all())
    {
        return Ok(wal_recovery_effect(
            0,
            &lexical.public_label,
            format!("temp leaf-ul WAL poate fi parțial după write/fsync: {error}"),
        ));
    }
    let temp_identity = match fs::fstat(&temp_file) {
        Ok(stat)
            if FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile
                && stat.st_nlink == 1
                && u64::try_from(stat.st_size).ok() == Some(plan.evidence.new_size) =>
        {
            stat
        }
        Ok(_) => {
            return Ok(wal_recovery_effect(
                bytes.len() as u64,
                &lexical.public_label,
                "temp leaf-ul WAL nu are tip/link/size așteptat",
            ));
        }
        Err(error) => {
            return Ok(wal_recovery_effect(
                bytes.len() as u64,
                &lexical.public_label,
                format!("temp leaf-ul WAL nu poate fi verificat: {error}"),
            ));
        }
    };
    if let Err(error) =
        validate_named_file_identity(&parent.directory, &temp_name, &temp_identity, "wal-temp")
    {
        return Ok(wal_recovery_effect(
            bytes.len() as u64,
            &lexical.public_label,
            error,
        ));
    }
    if let Err(error) = guard.mark_auxiliary_durable() {
        return Ok(wal_recovery_effect(
            bytes.len() as u64,
            &lexical.public_label,
            error,
        ));
    }

    run_test_hook(CapabilityTestStage::BeforeAtomicCommit);
    if plan.evidence.replace {
        let previous_descriptor = match fs::openat(
            &parent.directory,
            &parent.leaf,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(descriptor) => descriptor,
            Err(error) => {
                return Ok(wal_recovery_effect(
                    bytes.len() as u64,
                    &lexical.public_label,
                    format!("target-ul replace nu mai poate fi capturat: {error}"),
                ));
            }
        };
        let mut previous_file = File::from(previous_descriptor);
        let previous_before = match fs::fstat(&previous_file) {
            Ok(stat) => stat,
            Err(error) => {
                return Ok(wal_recovery_effect(
                    bytes.len() as u64,
                    &lexical.public_label,
                    format!("target-ul replace nu mai are metadata: {error}"),
                ));
            }
        };
        let previous_evidence = match wal_evidence_from_open_file(
            &mut previous_file,
            &previous_before,
            &target.expected_leaf,
            &lexical.public_label,
            "WAL replace commit preflight",
            None,
        ) {
            Ok(evidence) => evidence,
            Err(error) => {
                return Ok(wal_recovery_effect(
                    bytes.len() as u64,
                    &lexical.public_label,
                    error,
                ));
            }
        };
        if previous_evidence != plan.evidence.before {
            return Ok(wal_recovery_effect(
                bytes.len() as u64,
                &lexical.public_label,
                "target-ul replace diferă de baseline-ul WAL la commit",
            ));
        }
        if let Err(error) = fs::renameat_with(
            &parent.directory,
            &temp_name,
            &parent.directory,
            &parent.leaf,
            RenameFlags::EXCHANGE,
        ) {
            return Ok(wal_recovery_effect(
                bytes.len() as u64,
                &lexical.public_label,
                format!("WAL atomic exchange a eșuat: {error}"),
            ));
        }
        run_test_hook(CapabilityTestStage::AfterAtomicExchange);
        if let Err(error) = guard.mark_effect_visible() {
            return Ok(wal_recovery_effect(
                bytes.len() as u64,
                &lexical.public_label,
                error,
            ));
        }
        let moved_previous =
            match fs::statat(&parent.directory, &temp_name, AtFlags::SYMLINK_NOFOLLOW) {
                Ok(stat) => stat,
                Err(error) => {
                    return Ok(wal_recovery_effect(
                        bytes.len() as u64,
                        &lexical.public_label,
                        format!("versiunea veche WAL nu poate fi găsită: {error}"),
                    ));
                }
            };
        let previous_after = match fs::fstat(&previous_file) {
            Ok(stat) => stat,
            Err(error) => {
                return Ok(wal_recovery_effect(
                    bytes.len() as u64,
                    &lexical.public_label,
                    format!("versiunea veche WAL nu poate fi reverificată: {error}"),
                ));
            }
        };
        if !same_file_identity(&previous_before, &moved_previous)
            || !same_stable_leaf_version(&previous_before, &previous_after)
        {
            return Ok(wal_recovery_effect(
                bytes.len() as u64,
                &lexical.public_label,
                "WAL exchange a izolat alt inode/versiune decât baseline-ul",
            ));
        }
        if let Err(error) = validate_named_file_identity(
            &parent.directory,
            &parent.leaf,
            &temp_identity,
            "wal-replace-target",
        ) {
            return Ok(wal_recovery_effect(
                bytes.len() as u64,
                &lexical.public_label,
                error,
            ));
        }
        if let Err(error) = sync_directory(&parent.directory, &lexical.public_label) {
            return Ok(wal_recovery_effect(
                bytes.len() as u64,
                &lexical.public_label,
                format!("{error} Mappingul exchange nu este confirmat durabil."),
            ));
        }
        if let Err(error) = fs::unlinkat(&parent.directory, &temp_name, AtFlags::empty()) {
            return Ok(wal_recovery_effect(
                bytes.len() as u64,
                &lexical.public_label,
                format!("versiunea veche WAL nu poate fi curățată: {error}"),
            ));
        }
        if let Err(error) = sync_directory(&parent.directory, &lexical.public_label) {
            return Ok(wal_recovery_effect(
                bytes.len() as u64,
                &lexical.public_label,
                format!("{error} Cleanup-ul versiunii vechi nu este durabil."),
            ));
        }
    } else {
        if let Err(error) = fs::renameat_with(
            &parent.directory,
            &temp_name,
            &parent.directory,
            &parent.leaf,
            RenameFlags::NOREPLACE,
        ) {
            return Ok(wal_recovery_effect(
                bytes.len() as u64,
                &lexical.public_label,
                format!("WAL atomic create rename a eșuat: {error}"),
            ));
        }
        if let Err(error) = guard.mark_effect_visible() {
            return Ok(wal_recovery_effect(
                bytes.len() as u64,
                &lexical.public_label,
                error,
            ));
        }
        if let Err(error) = validate_named_file_identity(
            &parent.directory,
            &parent.leaf,
            &temp_identity,
            "wal-create-target",
        ) {
            return Ok(wal_recovery_effect(
                bytes.len() as u64,
                &lexical.public_label,
                error,
            ));
        }
        if let Err(error) = sync_directory(&parent.directory, &lexical.public_label) {
            return Ok(wal_recovery_effect(
                bytes.len() as u64,
                &lexical.public_label,
                format!("{error} Mappingul create nu este confirmat durabil."),
            ));
        }
    }

    drop(temp_file);
    if let Err(error) = guard.mark_target_durable() {
        return Ok(wal_recovery_effect(
            bytes.len() as u64,
            &lexical.public_label,
            error,
        ));
    }
    Ok(CapabilityEffect::changed(bytes.len() as u64))
}

pub(super) fn validate_atomic_plan_shape(
    lexical: &LexicalTarget,
    bytes: &[u8],
    replace_policy: CapabilityReplacePolicy,
    plan: &AtomicOperationPlan,
    operation_id: &str,
) -> Result<(), String> {
    let (leaf, parents) = lexical.relative_components.split_last().ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "planul WAL atomic nu are target leaf",
        )
    })?;
    let planned_parents = plan
        .evidence
        .parent
        .relative_components_hex
        .iter()
        .map(|component| decode_component_hex(component))
        .collect::<Result<Vec<_>, _>>()?;
    if planned_parents != parents
        || decode_component_hex(&plan.evidence.target_leaf_hex)? != *leaf
        || plan.temp_leaf()? != atomic_temp_leaf(operation_id)
        || plan.evidence.new_size != bytes.len() as u64
        || plan.evidence.new_content_hash != sha256_bytes(bytes)
        || (replace_policy == CapabilityReplacePolicy::CreateNew && plan.evidence.replace)
    {
        return Err(capability_error(
            &lexical.public_label,
            "planul WAL atomic nu corespunde targetului/payloadului executat",
        ));
    }
    Ok(())
}

pub(super) fn capture_atomic_parent_from_plan(
    lexical: &LexicalTarget,
    plan: &AtomicOperationPlan,
) -> Result<CapturedParent, CaptureFailure> {
    capture_parent_from_wal_evidence(lexical, &plan.evidence.parent)
}

pub(super) fn capture_append_parent_from_plan(
    lexical: &LexicalTarget,
    plan: &AppendOperationPlan,
) -> Result<CapturedParent, CaptureFailure> {
    capture_parent_from_wal_evidence(lexical, &plan.evidence.parent)
}

pub(super) fn capture_parent_from_wal_evidence(
    lexical: &LexicalTarget,
    evidence: &WalParentEvidence,
) -> Result<CapturedParent, CaptureFailure> {
    let (leaf, parents) = lexical.relative_components.split_last().ok_or_else(|| {
        CaptureFailure::no_effect(capability_error(
            &lexical.public_label,
            "planul WAL atomic cere un leaf",
        ))
    })?;
    if evidence.existing_prefix_len > parents.len() {
        return Err(CaptureFailure::no_effect(capability_error(
            &lexical.public_label,
            "planul WAL atomic are existing prefix invalid",
        )));
    }
    let boundary = capture_boundary(lexical, false)?.ok_or_else(|| {
        CaptureFailure::no_effect(capability_error(
            &lexical.public_label,
            "authority root a dispărut înainte de execuția WAL",
        ))
    })?;
    let mut directory = boundary.directory;
    for component in parents.iter().take(evidence.existing_prefix_len) {
        directory = open_directory_strict(&directory, component).map_err(|error| {
            CaptureFailure::no_effect(capability_error(
                &lexical.public_label,
                &format!("existing prefix WAL nu poate fi recapturat: {error}"),
            ))
        })?;
    }
    let observed = wal_identity_from_fd(&directory, &lexical.public_label)
        .map_err(CaptureFailure::no_effect)?;
    if observed != evidence.existing_ancestor_identity {
        return Err(CaptureFailure::no_effect(capability_error(
            &lexical.public_label,
            "existing ancestor identity diferă de planul WAL",
        )));
    }
    if evidence.existing_prefix_len == parents.len() {
        if evidence.parent_identity.as_ref() != Some(&observed) {
            return Err(CaptureFailure::no_effect(capability_error(
                &lexical.public_label,
                "parent identity diferă de planul WAL",
            )));
        }
        return Ok(CapturedParent {
            directory,
            leaf: leaf.clone(),
            created_ancestors: false,
        });
    }
    if evidence.parent_identity.is_some() {
        return Err(CaptureFailure::no_effect(capability_error(
            &lexical.public_label,
            "planul WAL declară parent identity pentru un suffix absent",
        )));
    }

    let mut created = false;
    for component in parents.iter().skip(evidence.existing_prefix_len) {
        match open_directory_strict(&directory, component) {
            Err(Errno::NOENT) => {}
            Ok(_) => {
                let error = capability_error(
                    &lexical.public_label,
                    "un părinte planificat absent a apărut înainte de mkdirat",
                );
                return Err(if created {
                    CaptureFailure::after_effect(error)
                } else {
                    CaptureFailure::no_effect(error)
                });
            }
            Err(error) => {
                let diagnostic = capability_error(
                    &lexical.public_label,
                    &format!("un părinte WAL nu poate fi verificat: {error}"),
                );
                return Err(if created {
                    CaptureFailure::after_effect(diagnostic)
                } else {
                    CaptureFailure::no_effect(diagnostic)
                });
            }
        }
        if let Err(error) = fs::mkdirat(&directory, component, DIRECTORY_MODE) {
            let diagnostic = capability_error(
                &lexical.public_label,
                &format!("mkdirat planificat de WAL a eșuat: {error}"),
            );
            return Err(if created {
                CaptureFailure::after_effect(diagnostic)
            } else {
                CaptureFailure::no_effect(diagnostic)
            });
        }
        created = true;
        let next = open_directory_strict(&directory, component).map_err(|error| {
            CaptureFailure::after_effect(capability_error(
                &lexical.public_label,
                &format!("părintele WAL creat nu poate fi recapturat: {error}"),
            ))
        })?;
        validate_named_directory_identity(
            &directory,
            component,
            &next,
            &lexical.public_label,
            "WAL parent component",
        )
        .map_err(CaptureFailure::after_effect)?;
        sync_directory(&directory, &lexical.public_label).map_err(CaptureFailure::after_effect)?;
        directory = next;
    }
    Ok(CapturedParent {
        directory,
        leaf: leaf.clone(),
        created_ancestors: created,
    })
}
