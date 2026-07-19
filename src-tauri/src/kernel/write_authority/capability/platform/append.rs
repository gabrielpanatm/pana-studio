use std::io::{Read, Seek, SeekFrom, Write};

use super::anonymous_file::{
    causal_file_identity, link_anonymous_file_create_only, open_anonymous_file,
    CausalFileIdentityError,
};
use super::*;

#[path = "append/recovery.rs"]
mod recovery;

pub(in crate::kernel::write_authority::capability) use recovery::{
    classify_append_recovery, execute_append_recovery,
};

pub(in crate::kernel::write_authority::capability) fn plan_append(
    target: &WriteTarget,
    bytes: &[u8],
) -> Result<AppendOperationPlan, String> {
    validate_append_payload(bytes)?;
    let lexical = lexical_target(target, false)?;
    let authority = lexical.authority.as_ref().ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "planul Append v2 cere authority root sigilat",
        )
    })?;
    if !matches!(authority.scope(), DirectoryAuthorityScope::ApplicationData) {
        return Err(capability_error(
            &lexical.public_label,
            "Append v2 este permis numai sub ApplicationData",
        ));
    }

    let boundary = capture_existing_boundary(&lexical)?.ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "authority root nu există pentru planul Append v2",
        )
    })?;
    let (leaf, parents) = lexical
        .relative_components
        .split_last()
        .ok_or_else(|| capability_error(&lexical.public_label, "Append v2 cere un leaf"))?;
    let mut directory = boundary.directory;
    for component in parents {
        let next = open_directory_strict(&directory, component).map_err(|error| {
            capability_error(
                &lexical.public_label,
                &format!(
                    "Append v2 cere parent existent integral; componenta lipsește sau este invalidă: {error}"
                ),
            )
        })?;
        validate_named_directory_identity(
            &directory,
            component,
            &next,
            &lexical.public_label,
            "append-v2-plan-parent",
        )?;
        directory = next;
    }
    let parent_identity = wal_identity_from_fd(&directory, &lexical.public_label)?;

    let (before, before_identity_digest, before_tail_size, before_tail_hash) =
        match leaf_metadata(&directory, leaf, &lexical.public_label)? {
            None => (WalAppendBefore::Absent, None, 0, None),
            Some(stat) if FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile => {
                let descriptor = fs::openat(
                    &directory,
                    leaf,
                    OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|error| {
                    capability_error(
                        &lexical.public_label,
                        &format!("Append v2 baseline open a eșuat: {error}"),
                    )
                })?;
                validate_regular_single_link(&descriptor, &lexical.public_label, "Append v2 plan")?;
                let captured = fs::fstat(&descriptor).map_err(|error| {
                    capability_error(
                        &lexical.public_label,
                        &format!("Append v2 baseline fstat a eșuat: {error}"),
                    )
                })?;
                if !same_file_identity(&stat, &captured) {
                    return Err(capability_error(
                        &lexical.public_label,
                        "Append v2 baseline s-a schimbat în timpul capturii",
                    ));
                }
                validate_named_file_identity(&directory, leaf, &captured, "append-v2-plan-target")?;
                let size = u64::try_from(captured.st_size).map_err(|_| {
                    capability_error(&lexical.public_label, "Append v2 baseline are size negativ")
                })?;
                size.checked_add(bytes.len() as u64).ok_or_else(|| {
                    capability_error(
                        &lexical.public_label,
                        "Append v2 refuză overflow-ul dimensiunii finale",
                    )
                })?;
                let mut file = File::from(descriptor);
                let (tail_size, tail_hash) = append_tail_contract(&mut file, size)?;
                let identity_digest =
                    append_identity_digest(&file, WalAppendStageRole::ExistingTarget)?;
                (
                    WalAppendBefore::Present {
                        identity: WalFilesystemIdentity {
                            device: captured.st_dev,
                            inode: captured.st_ino,
                        },
                        size,
                        version_token: append_version_token(&captured),
                    },
                    Some(identity_digest),
                    tail_size,
                    Some(tail_hash),
                )
            }
            Some(_) => {
                return Err(capability_error(
                    &lexical.public_label,
                    "Append v2 refuză un target non-regular",
                ));
            }
        };

    let prefix_len = bytes.len().min(MAX_WAL_APPEND_PREFIX_BYTES);
    Ok(AppendOperationPlan {
        evidence: WalAppendEvidence {
            protocol_version: WAL_APPEND_PROTOCOL_VERSION,
            parent: WalParentEvidence {
                relative_components_hex: parents
                    .iter()
                    .map(|component| encode_component_hex(component))
                    .collect(),
                existing_prefix_len: parents.len(),
                existing_ancestor_identity: parent_identity.clone(),
                parent_identity: Some(parent_identity),
            },
            target_leaf_hex: encode_component_hex(leaf),
            before,
            payload_size: bytes.len() as u64,
            payload_hash: sha256_bytes(bytes),
            payload_prefix_hex: encode_bytes_hex(&bytes[..prefix_len]),
            payload_complete_in_record: bytes.len() <= MAX_WAL_APPEND_PREFIX_BYTES,
            payload_hex: Some(encode_bytes_hex(bytes)),
            before_identity_digest,
            before_tail_size,
            before_tail_hash,
        },
    })
}

pub(in crate::kernel::write_authority::capability) fn append_wal(
    target: &WriteTarget,
    bytes: &[u8],
    plan: AppendOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    validate_append_payload(bytes)?;
    let lexical = lexical_target(target, false)?;
    validate_append_plan(&lexical, bytes, &plan)?;
    let parent = match capture_append_parent_from_plan(&lexical, &plan) {
        Ok(parent) if !parent.created_ancestors => parent,
        Ok(_) => {
            return Err(capability_error(
                &lexical.public_label,
                "Append v2 interzice crearea implicită de parent",
            ));
        }
        Err(error) => return error.into_operation_result(),
    };
    fs::flock(&parent.directory, FlockOperation::LockExclusive).map_err(|error| {
        capability_error(
            &lexical.public_label,
            &format!("Append v2 stable parent lock a eșuat: {error}"),
        )
    })?;

    match &plan.evidence.before {
        WalAppendBefore::Present { .. } => append_existing(&lexical, &parent, bytes, &plan, guard),
        WalAppendBefore::Absent => append_create(&lexical, &parent, bytes, &plan, guard),
    }
}

fn append_existing(
    lexical: &LexicalTarget,
    parent: &CapturedParent,
    bytes: &[u8],
    plan: &AppendOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    let mut file = open_existing_append_target(parent, &lexical.public_label)?;
    validate_existing_baseline(&mut file, parent, &plan.evidence, &lexical.public_label)?;
    let before_size = append_before_size(&plan.evidence.before);
    let identity = append_identity_digest(&file, WalAppendStageRole::ExistingTarget)?;
    let checkpoint = WalAppendStageCheckpoint::new(
        identity,
        &plan.evidence.payload_hash,
        plan.evidence.payload_size,
        before_size,
        WalAppendStageRole::ExistingTarget,
    )?;
    if let Err(error) = guard.mark_append_auxiliary_durable(checkpoint) {
        return Ok(wal_recovery_effect(0, &lexical.public_label, error));
    }
    run_test_hook(CapabilityTestStage::AfterAppendV2Checkpoint);

    let write_limit = append_v2_short_write_limit()
        .unwrap_or(bytes.len())
        .min(bytes.len());
    let written = match file.write(&bytes[..write_limit]) {
        Ok(written) => written,
        Err(error) => {
            let _ = file.sync_data();
            let observed = observed_append_size(&file, before_size).unwrap_or(0);
            return Ok(wal_recovery_effect(
                observed,
                &lexical.public_label,
                format!("Append v2 write poate fi parțial: {error}"),
            ));
        }
    };
    run_test_hook(CapabilityTestStage::AfterAppendV2WriteBeforePhase);
    if written != bytes.len() {
        let _ = file.sync_data();
        return Ok(wal_recovery_effect(
            written as u64,
            &lexical.public_label,
            format!(
                "Append v2 a produs short write {written}/{}; recovery va continua numai prefixul exact",
                bytes.len()
            ),
        ));
    }
    if let Err(error) = guard.mark_effect_visible() {
        return Ok(wal_recovery_effect(
            bytes.len() as u64,
            &lexical.public_label,
            error,
        ));
    }
    if let Err(error) = file.sync_data() {
        return Ok(wal_recovery_effect(
            bytes.len() as u64,
            &lexical.public_label,
            format!("Append v2 fdatasync a eșuat: {error}"),
        ));
    }
    run_test_hook(CapabilityTestStage::AfterAppendV2TargetFsync);
    if let Err(error) = validate_complete_append(
        &mut file,
        parent,
        &plan.evidence,
        &lexical.public_label,
        None,
    ) {
        return Ok(wal_recovery_effect(
            bytes.len() as u64,
            &lexical.public_label,
            error,
        ));
    }
    if let Err(error) = guard.mark_target_durable() {
        return Ok(wal_recovery_effect(
            bytes.len() as u64,
            &lexical.public_label,
            error,
        ));
    }
    run_test_hook(CapabilityTestStage::AfterAppendV2TargetDurable);
    if let Err(error) = validate_complete_append(
        &mut file,
        parent,
        &plan.evidence,
        &lexical.public_label,
        None,
    ) {
        return Ok(wal_recovery_effect(
            bytes.len() as u64,
            &lexical.public_label,
            error,
        ));
    }
    Ok(CapabilityEffect::changed(bytes.len() as u64))
}

fn append_create(
    lexical: &LexicalTarget,
    parent: &CapturedParent,
    bytes: &[u8],
    plan: &AppendOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    if leaf_metadata(&parent.directory, &parent.leaf, &lexical.public_label)?.is_some() {
        return Err(capability_error(
            &lexical.public_label,
            "Append v2 create-only a observat un target concurent",
        ));
    }
    let mut staged = open_anonymous_file(&parent.directory, FILE_MODE).map_err(|error| {
        capability_error(
            &lexical.public_label,
            &format!("Append v2 O_TMPFILE a eșuat: {error}"),
        )
    })?;
    staged.write_all(bytes).map_err(|error| {
        capability_error(
            &lexical.public_label,
            &format!("Append v2 staging write a eșuat înainte de publicare: {error}"),
        )
    })?;
    fs::fchmod(&staged, Mode::from_raw_mode(0o600)).map_err(|error| {
        capability_error(
            &lexical.public_label,
            &format!("Append v2 staging fchmod 0600 a eșuat: {error}"),
        )
    })?;
    staged.sync_all().map_err(|error| {
        capability_error(
            &lexical.public_label,
            &format!("Append v2 staging fsync a eșuat înainte de publicare: {error}"),
        )
    })?;
    validate_anonymous_payload(&mut staged, &plan.evidence, &lexical.public_label)?;
    let identity = append_identity_digest(&staged, WalAppendStageRole::CreateTarget)?;
    let checkpoint = WalAppendStageCheckpoint::new(
        identity.clone(),
        &plan.evidence.payload_hash,
        plan.evidence.payload_size,
        0,
        WalAppendStageRole::CreateTarget,
    )?;
    if let Err(error) = guard.mark_append_auxiliary_durable(checkpoint) {
        return Ok(wal_recovery_effect(0, &lexical.public_label, error));
    }
    run_test_hook(CapabilityTestStage::AfterAppendV2Checkpoint);
    if let Err(error) =
        link_anonymous_file_create_only(&staged, &parent.directory, &parent.leaf, false)
    {
        return Ok(wal_recovery_effect(
            0,
            &lexical.public_label,
            format!("Append v2 linkat create-only a eșuat: {error:?}"),
        ));
    }
    run_test_hook(CapabilityTestStage::AfterAppendV2LinkBeforePhase);
    if let Err(error) = guard.mark_effect_visible() {
        return Ok(wal_recovery_effect(
            bytes.len() as u64,
            &lexical.public_label,
            error,
        ));
    }
    if let Err(error) = staged.sync_all() {
        return Ok(wal_recovery_effect(
            bytes.len() as u64,
            &lexical.public_label,
            format!("Append v2 target fsync a eșuat: {error}"),
        ));
    }
    if let Err(error) = sync_directory(&parent.directory, &lexical.public_label) {
        return Ok(wal_recovery_effect(
            bytes.len() as u64,
            &lexical.public_label,
            error,
        ));
    }
    run_test_hook(CapabilityTestStage::AfterAppendV2TargetFsync);
    if let Err(error) = validate_complete_append(
        &mut staged,
        parent,
        &plan.evidence,
        &lexical.public_label,
        Some(&identity),
    ) {
        return Ok(wal_recovery_effect(
            bytes.len() as u64,
            &lexical.public_label,
            error,
        ));
    }
    if let Err(error) = guard.mark_target_durable() {
        return Ok(wal_recovery_effect(
            bytes.len() as u64,
            &lexical.public_label,
            error,
        ));
    }
    run_test_hook(CapabilityTestStage::AfterAppendV2TargetDurable);
    if let Err(error) = validate_complete_append(
        &mut staged,
        parent,
        &plan.evidence,
        &lexical.public_label,
        Some(&identity),
    ) {
        return Ok(wal_recovery_effect(
            bytes.len() as u64,
            &lexical.public_label,
            error,
        ));
    }
    Ok(CapabilityEffect::changed(bytes.len() as u64))
}

fn validate_append_payload(bytes: &[u8]) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > MAX_WAL_APPEND_PAYLOAD_BYTES {
        return Err(format!(
            "Append v2 refuză payloadul de {} bytes (maxim {}).",
            bytes.len(),
            MAX_WAL_APPEND_PAYLOAD_BYTES
        ));
    }
    let line = bytes
        .strip_suffix(b"\n")
        .ok_or_else(|| "Append v2 cere o linie JSONL terminată cu newline.".to_string())?;
    if line.is_empty()
        || line.contains(&b'\n')
        || line.contains(&b'\r')
        || std::str::from_utf8(line).is_err()
        || serde_json::from_slice::<serde_json::Value>(line).is_err()
    {
        return Err("Append v2 refuză framingul sau JSON-ul liniei.".into());
    }
    Ok(())
}

fn validate_append_plan(
    lexical: &LexicalTarget,
    bytes: &[u8],
    plan: &AppendOperationPlan,
) -> Result<(), String> {
    let (leaf, parents) = lexical
        .relative_components
        .split_last()
        .ok_or_else(|| capability_error(&lexical.public_label, "Append v2 cere leaf"))?;
    let planned_parents = plan
        .evidence
        .parent
        .relative_components_hex
        .iter()
        .map(|component| decode_component_hex(component))
        .collect::<Result<Vec<_>, _>>()?;
    if plan.evidence.protocol_version != WAL_APPEND_PROTOCOL_VERSION
        || planned_parents != parents
        || decode_component_hex(&plan.evidence.target_leaf_hex)? != *leaf
        || plan.evidence.payload_size != bytes.len() as u64
        || plan.evidence.payload_hash != sha256_bytes(bytes)
        || plan.evidence.payload_hex.as_deref() != Some(encode_bytes_hex(bytes).as_str())
        || plan.evidence.parent.existing_prefix_len != parents.len()
        || plan.evidence.parent.parent_identity.is_none()
    {
        return Err(capability_error(
            &lexical.public_label,
            "planul Append v2 nu corespunde targetului/payloadului executat",
        ));
    }
    Ok(())
}

fn open_existing_append_target(
    parent: &CapturedParent,
    public_label: &str,
) -> Result<File, String> {
    let descriptor = fs::openat(
        &parent.directory,
        &parent.leaf,
        OFlags::RDWR | OFlags::APPEND | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| capability_error(public_label, &format!("Append v2 open a eșuat: {error}")))?;
    validate_regular_single_link(&descriptor, public_label, "Append v2 target")?;
    Ok(File::from(descriptor))
}

fn validate_existing_baseline(
    file: &mut File,
    parent: &CapturedParent,
    evidence: &WalAppendEvidence,
    public_label: &str,
) -> Result<(), String> {
    let WalAppendBefore::Present {
        identity,
        size,
        version_token,
    } = &evidence.before
    else {
        return Err(capability_error(
            public_label,
            "Append v2 baseline nu este Present",
        ));
    };
    let stat = fs::fstat(&*file)
        .map_err(|error| capability_error(public_label, &format!("Append v2 fstat: {error}")))?;
    if stat.st_dev != identity.device
        || stat.st_ino != identity.inode
        || u64::try_from(stat.st_size).ok() != Some(*size)
        || append_version_token(&stat) != *version_token
        || stat.st_nlink != 1
    {
        return Err(capability_error(
            public_label,
            "Append v2 baseline/version diferă după WAL prepare",
        ));
    }
    validate_named_file_identity(&parent.directory, &parent.leaf, &stat, "append-v2-baseline")?;
    let expected_identity = evidence.before_identity_digest.as_deref().ok_or_else(|| {
        capability_error(public_label, "Append v2 Present nu are identity digest")
    })?;
    if append_identity_digest(&*file, WalAppendStageRole::ExistingTarget)? != expected_identity {
        return Err(capability_error(
            public_label,
            "Append v2 statx lifetime diferă de plan",
        ));
    }
    validate_tail_contract(file, *size, evidence, public_label)
}

fn validate_complete_append(
    file: &mut File,
    parent: &CapturedParent,
    evidence: &WalAppendEvidence,
    public_label: &str,
    expected_identity: Option<&str>,
) -> Result<(), String> {
    let before_size = append_before_size(&evidence.before);
    let final_size = before_size
        .checked_add(evidence.payload_size)
        .ok_or_else(|| capability_error(public_label, "Append v2 final size overflow"))?;
    let stat = fs::fstat(&*file).map_err(|error| {
        capability_error(public_label, &format!("Append v2 post fstat: {error}"))
    })?;
    if u64::try_from(stat.st_size).ok() != Some(final_size)
        || stat.st_nlink != 1
        || (matches!(evidence.before, WalAppendBefore::Absent) && stat.st_mode & 0o7777 != 0o600)
    {
        return Err(capability_error(
            public_label,
            "Append v2 postflight size/nlink diferă",
        ));
    }
    validate_named_file_identity(
        &parent.directory,
        &parent.leaf,
        &stat,
        "append-v2-postflight",
    )?;
    let role = if matches!(evidence.before, WalAppendBefore::Absent) {
        WalAppendStageRole::CreateTarget
    } else {
        WalAppendStageRole::ExistingTarget
    };
    let expected_identity = expected_identity
        .or_else(|| evidence.before_identity_digest.as_deref())
        .ok_or_else(|| capability_error(public_label, "Append v2 nu are identitate postflight"))?;
    let identity_before_hash = append_identity_digest(&*file, role)?;
    if identity_before_hash != expected_identity {
        return Err(capability_error(
            public_label,
            "Append v2 targetul nu mai este inode-ul checkpointed",
        ));
    }
    if matches!(evidence.before, WalAppendBefore::Present { .. }) {
        validate_tail_contract(file, before_size, evidence, public_label)?;
    }
    let payload = read_exact_range(file, before_size, evidence.payload_size)?;
    if sha256_bytes(&payload) != evidence.payload_hash {
        return Err(capability_error(
            public_label,
            "Append v2 postflight payload hash diferă",
        ));
    }
    let after = fs::fstat(&*file).map_err(|error| {
        capability_error(
            public_label,
            &format!("Append v2 post-hash fstat a eșuat: {error}"),
        )
    })?;
    if !same_file_identity(&stat, &after)
        || append_version_token(&stat) != append_version_token(&after)
        || u64::try_from(after.st_size).ok() != Some(final_size)
        || after.st_nlink != 1
        || append_identity_digest(&*file, role)? != identity_before_hash
    {
        return Err(capability_error(
            public_label,
            "Append v2 targetul s-a schimbat în timpul postflight hash",
        ));
    }
    validate_named_file_identity(
        &parent.directory,
        &parent.leaf,
        &after,
        "append-v2-post-hash",
    )?;
    Ok(())
}

fn validate_anonymous_payload(
    file: &mut File,
    evidence: &WalAppendEvidence,
    public_label: &str,
) -> Result<(), String> {
    let stat = fs::fstat(&*file).map_err(|error| {
        capability_error(public_label, &format!("Append v2 staged fstat: {error}"))
    })?;
    if stat.st_nlink != 0
        || stat.st_mode & 0o7777 != 0o600
        || u64::try_from(stat.st_size).ok() != Some(evidence.payload_size)
    {
        return Err(capability_error(
            public_label,
            "Append v2 staged inode are nlink/size invalid",
        ));
    }
    let payload = read_exact_range(file, 0, evidence.payload_size)?;
    if sha256_bytes(&payload) != evidence.payload_hash {
        return Err(capability_error(
            public_label,
            "Append v2 staged hash diferă",
        ));
    }
    Ok(())
}

pub(super) fn append_identity_digest(
    descriptor: impl AsFd,
    role: WalAppendStageRole,
) -> Result<String, String> {
    let identity = causal_file_identity(descriptor).map_err(|error| match error {
        CausalFileIdentityError::Statx(error) => {
            format!("Append v2 statx identity a eșuat: {error}")
        }
        CausalFileIdentityError::Incomplete => {
            "Append v2 filesystem nu furnizează identitate statx lifetime completă".into()
        }
    })?;
    let mut digest = Sha256::new();
    digest.update(b"pana-append-target-identity-v2\0");
    digest.update(match role {
        WalAppendStageRole::CreateTarget => b"create-target".as_slice(),
        WalAppendStageRole::ExistingTarget => b"existing-target".as_slice(),
    });
    digest.update(b"\0");
    digest.update(identity.device_major.to_le_bytes());
    digest.update(identity.device_minor.to_le_bytes());
    digest.update(identity.inode.to_le_bytes());
    digest.update(identity.birth_time_seconds.to_le_bytes());
    digest.update(identity.birth_time_nanoseconds.to_le_bytes());
    digest.update(identity.mount_id.to_le_bytes());
    let encoded = format!("{:x}", digest.finalize());
    Ok(encoded[..32].to_string())
}

pub(super) fn append_version_token(stat: &fs::Stat) -> String {
    format!(
        "append-v2:{}:{}:{}:{}:{}:{}:{}:{}:{}",
        stat.st_dev,
        stat.st_ino,
        stat.st_size,
        stat.st_mtime,
        stat.st_mtime_nsec,
        stat.st_ctime,
        stat.st_ctime_nsec,
        stat.st_mode,
        stat.st_nlink,
    )
}

pub(super) fn append_before_size(before: &WalAppendBefore) -> u64 {
    match before {
        WalAppendBefore::Absent => 0,
        WalAppendBefore::Present { size, .. } => *size,
    }
}

pub(super) fn validate_tail_contract(
    file: &mut File,
    before_size: u64,
    evidence: &WalAppendEvidence,
    public_label: &str,
) -> Result<(), String> {
    let (tail_size, tail_hash) = append_tail_contract(file, before_size)?;
    if tail_size != evidence.before_tail_size
        || Some(tail_hash.as_str()) != evidence.before_tail_hash.as_deref()
    {
        return Err(capability_error(
            public_label,
            "Append v2 baseline tail s-a schimbat",
        ));
    }
    Ok(())
}

fn append_tail_contract(file: &mut File, before_size: u64) -> Result<(u64, String), String> {
    let tail_size = before_size.min(MAX_WAL_APPEND_TAIL_BYTES as u64);
    let tail = read_exact_range(file, before_size - tail_size, tail_size)?;
    Ok((tail_size, sha256_bytes(&tail)))
}

pub(super) fn read_exact_range(file: &mut File, offset: u64, size: u64) -> Result<Vec<u8>, String> {
    let capacity = usize::try_from(size)
        .map_err(|_| "Append v2 range depășește memoria adresabilă.".to_string())?;
    file.seek(SeekFrom::Start(offset))
        .map_err(|error| format!("Append v2 seek a eșuat: {error}."))?;
    let mut bytes = vec![0_u8; capacity];
    file.read_exact(&mut bytes)
        .map_err(|error| format!("Append v2 range read a eșuat: {error}."))?;
    Ok(bytes)
}

fn observed_append_size(file: &File, before_size: u64) -> Result<u64, String> {
    let stat =
        fs::fstat(file).map_err(|error| format!("Append v2 observed fstat a eșuat: {error}."))?;
    let size =
        u64::try_from(stat.st_size).map_err(|_| "Append v2 observed size negativ.".to_string())?;
    Ok(size.saturating_sub(before_size))
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn plan_legacy_append_for_test(
    target: &WriteTarget,
    bytes: &[u8],
) -> Result<AppendOperationPlan, String> {
    let lexical = lexical_target(target, false)?;
    let boundary = capture_existing_boundary(&lexical)?
        .ok_or_else(|| capability_error(&lexical.public_label, "legacy test boundary lipsește"))?;
    let (leaf, parents) = lexical
        .relative_components
        .split_last()
        .ok_or_else(|| capability_error(&lexical.public_label, "legacy test append cere leaf"))?;
    let mut directory = boundary.directory;
    let mut existing_prefix_len = 0_usize;
    for component in parents {
        match open_directory_strict(&directory, component) {
            Ok(next) => {
                directory = next;
                existing_prefix_len += 1;
            }
            Err(Errno::NOENT) => break,
            Err(error) => return Err(format!("legacy test parent capture: {error}")),
        }
    }
    let existing_ancestor_identity = wal_identity_from_fd(&directory, &lexical.public_label)?;
    let parent_exists = existing_prefix_len == parents.len();
    let parent_identity = parent_exists
        .then(|| wal_identity_from_fd(&directory, &lexical.public_label))
        .transpose()?;
    let before = if parent_exists {
        match leaf_metadata(&directory, leaf, &lexical.public_label)? {
            None => WalAppendBefore::Absent,
            Some(stat) if FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile => {
                WalAppendBefore::Present {
                    identity: WalFilesystemIdentity {
                        device: stat.st_dev,
                        inode: stat.st_ino,
                    },
                    size: u64::try_from(stat.st_size)
                        .map_err(|_| "legacy test size negativ".to_string())?,
                    version_token: version_token_for_stat(&stat),
                }
            }
            Some(_) => return Err("legacy test append target non-regular".into()),
        }
    } else {
        WalAppendBefore::Absent
    };
    let prefix_len = bytes.len().min(MAX_WAL_APPEND_PREFIX_BYTES);
    Ok(AppendOperationPlan {
        evidence: WalAppendEvidence {
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
            before,
            payload_size: bytes.len() as u64,
            payload_hash: sha256_bytes(bytes),
            payload_prefix_hex: encode_bytes_hex(&bytes[..prefix_len]),
            payload_complete_in_record: bytes.len() <= MAX_WAL_APPEND_PREFIX_BYTES,
            payload_hex: None,
            before_identity_digest: None,
            before_tail_size: 0,
            before_tail_hash: None,
        },
    })
}
