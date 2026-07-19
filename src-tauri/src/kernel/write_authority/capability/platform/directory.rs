use std::os::fd::AsFd;

use sha2::{Digest, Sha256};

use super::anonymous_file::{causal_file_identity, CausalFileIdentityError};
use super::*;

#[path = "directory/recovery.rs"]
mod recovery;

pub(in crate::kernel::write_authority::capability) use recovery::{
    classify_directory_recovery, execute_directory_recovery, resolve_directory_operator,
};

const DIRECTORY_V2_MODE_BITS: u32 = 0o755;

#[derive(Debug)]
pub(super) enum ObservedDirectoryLeaf {
    Absent,
    Other,
    Directory(ObservedDirectory),
}

#[derive(Debug)]
pub(super) struct ObservedDirectory {
    pub descriptor: OwnedFd,
    pub stat: fs::Stat,
    pub identity_digest: String,
    pub version_token: String,
    pub state_digest: String,
    pub empty: bool,
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn plan_legacy_directory_for_test(
    target: &WriteTarget,
) -> Result<DirectoryOperationPlan, String> {
    super::plan_legacy_directory(target)
}

pub(in crate::kernel::write_authority::capability) fn plan_directory(
    target: &WriteTarget,
) -> Result<DirectoryOperationPlan, String> {
    let lexical = lexical_target(target, false)?;
    let authority = lexical.authority.as_ref().ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "planul Directory v2 cere authority root sigilat",
        )
    })?;
    verify_directory_authority_path(authority)?;
    let boundary = capture_existing_boundary(&lexical)?.ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "authority root nu există pentru Directory v2",
        )
    })?;
    let (target_leaf, parents) = lexical
        .relative_components
        .split_last()
        .ok_or_else(|| capability_error(&lexical.public_label, "Directory v2 cere un leaf"))?;
    let mut parent = boundary.directory;
    for component in parents {
        let next = open_directory_strict(&parent, component).map_err(|error| {
            capability_error(
                &lexical.public_label,
                &format!(
                    "Directory v2 cere parent final existent integral; componenta lipsește sau este invalidă: {error}"
                ),
            )
        })?;
        validate_named_directory_identity(
            &parent,
            component,
            &next,
            &lexical.public_label,
            "directory-v2-plan-parent",
        )?;
        parent = next;
    }
    let parent_identity = wal_identity_from_fd(&parent, &lexical.public_label)?;
    let observed = observe_directory_leaf(
        &parent,
        target_leaf,
        &lexical.public_label,
        "Directory v2 plan target",
    )?;
    let (
        existing_prefix_len,
        existing_ancestor_identity,
        existing_target_identity,
        existing_target_identity_digest,
        existing_target_version_token,
    ) = match observed {
        ObservedDirectoryLeaf::Absent => (parents.len(), parent_identity.clone(), None, None, None),
        ObservedDirectoryLeaf::Other => {
            return Err(capability_error(
                &lexical.public_label,
                "Directory v2 refuză targetul existent non-directory",
            ));
        }
        ObservedDirectoryLeaf::Directory(observed) => {
            let identity = WalFilesystemIdentity {
                device: observed.stat.st_dev,
                inode: observed.stat.st_ino,
            };
            (
                lexical.relative_components.len(),
                identity.clone(),
                Some(identity),
                Some(observed.identity_digest),
                Some(observed.version_token),
            )
        }
    };

    Ok(DirectoryOperationPlan {
        evidence: WalDirectoryEvidence {
            protocol_version: WAL_DIRECTORY_PROTOCOL_VERSION,
            relative_components_hex: lexical
                .relative_components
                .iter()
                .map(|component| encode_component_hex(component))
                .collect(),
            existing_prefix_len,
            existing_ancestor_identity,
            existing_target_identity,
            parent_identity: Some(parent_identity),
            target_leaf_hex: Some(encode_component_hex(target_leaf)),
            existing_target_identity_digest,
            existing_target_version_token,
            desired_mode_bits: Some(DIRECTORY_V2_MODE_BITS),
        },
    })
}

pub(in crate::kernel::write_authority::capability) fn create_directory_all_wal(
    target: &WriteTarget,
    plan: &DirectoryOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    if plan.evidence.protocol_version == 0 {
        return super::create_legacy_directory_all_wal(target, plan, guard);
    }
    if plan.evidence.protocol_version != WAL_DIRECTORY_PROTOCOL_VERSION {
        return Err("Directory runtime refuză protocolul WAL necunoscut.".into());
    }
    create_directory_v2(target, plan, guard)
}

fn create_directory_v2(
    target: &WriteTarget,
    plan: &DirectoryOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    let lexical = lexical_target(target, false)?;
    validate_directory_v2_plan(&lexical, plan)?;
    let parent = capture_existing_target_parent(&lexical)?
        .ok_or_else(|| capability_error(&lexical.public_label, "parentul Directory v2 lipsește"))?;
    if parent.created_ancestors {
        return Err(capability_error(
            &lexical.public_label,
            "Directory v2 interzice crearea implicită de parent",
        ));
    }
    fs::flock(&parent.directory, FlockOperation::LockExclusive).map_err(|error| {
        capability_error(
            &lexical.public_label,
            &format!("Directory v2 stable parent lock a eșuat: {error}"),
        )
    })?;
    let expected_parent = plan.evidence.parent_identity.as_ref().ok_or_else(|| {
        capability_error(&lexical.public_label, "Directory v2 nu are parent identity")
    })?;
    if wal_identity_from_fd(&parent.directory, &lexical.public_label)? != *expected_parent {
        return Err(capability_error(
            &lexical.public_label,
            "parentul Directory v2 diferă de plan",
        ));
    }

    if plan.evidence.existing_target_identity.is_some() {
        validate_existing_noop(&lexical, &parent, plan)?;
        return Ok(CapabilityEffect::unchanged());
    }

    if !matches!(
        observe_directory_leaf(
            &parent.directory,
            &parent.leaf,
            &lexical.public_label,
            "Directory v2 pre-create target",
        )?,
        ObservedDirectoryLeaf::Absent
    ) {
        return Err(capability_error(
            &lexical.public_label,
            "targetul Directory v2 a apărut după planificare",
        ));
    }

    if let Err(error) = fs::mkdirat(
        &parent.directory,
        &parent.leaf,
        Mode::from_raw_mode(DIRECTORY_V2_MODE_BITS),
    ) {
        return Err(capability_error(
            &lexical.public_label,
            &format!("Directory v2 mkdirat direct create-only a eșuat: {error}"),
        ));
    }
    // `mkdirat` nu returnează FD. Primul syscall post-create este deschiderea
    // strictă a leaf-ului; aceasta micșorează intervalul fundamental în care un
    // writer necooperant ar putea substitui numele înainte de prima captură.
    let created = match open_directory_strict(&parent.directory, &parent.leaf) {
        Ok(created) => created,
        Err(error) => {
            return Ok(directory_recovery_effect(
                &lexical.public_label,
                format!("targetul creat nu poate fi deschis: {error}"),
            ));
        }
    };
    let created_fd_identity = match directory_identity_digest(&created) {
        Ok(identity) => identity,
        Err(error) => return Ok(directory_recovery_effect(&lexical.public_label, error)),
    };
    let created_named_identity = match directory_named_identity_digest(
        &parent.directory,
        &parent.leaf,
        &lexical.public_label,
    ) {
        Ok(identity) => identity,
        Err(error) => return Ok(directory_recovery_effect(&lexical.public_label, error)),
    };
    if created_fd_identity != created_named_identity {
        return Ok(directory_recovery_effect(
            &lexical.public_label,
            "Directory v2 target lifetime diferă între mkdirat și first-open",
        ));
    }
    // `mkdirat` nu returnează FD; intervalul mkdirat -> first open rămâne o
    // limită explicită a modelului cooperative-writer. Din acest punct FD-ul
    // ținut deschis împiedică reutilizarea inode-ului, iar orice substituție a
    // numelui este detectată prin named-vs-FD înainte de checkpoint.
    if let Err(error) = validate_named_directory_identity(
        &parent.directory,
        &parent.leaf,
        &created,
        &lexical.public_label,
        "Directory v2 target first-open binding",
    ) {
        return Ok(directory_recovery_effect(&lexical.public_label, error));
    }
    run_test_hook(CapabilityTestStage::AfterDirectoryCreateBeforePhase);
    if let Err(error) = fs::fchmod(&created, Mode::from_raw_mode(DIRECTORY_V2_MODE_BITS)) {
        return Ok(directory_recovery_effect(
            &lexical.public_label,
            format!("Directory v2 fchmod target a eșuat: {error}"),
        ));
    }
    let pre_checkpoint_stat = match validate_staged_directory(
        &parent.directory,
        &parent.leaf,
        &created,
        &lexical.public_label,
        "Directory v2 target pre-checkpoint",
    ) {
        Ok(stat) => stat,
        Err(error) => return Ok(directory_recovery_effect(&lexical.public_label, error)),
    };
    let pre_checkpoint_state_digest =
        directory_state_digest(&pre_checkpoint_stat, &created_named_identity, true);
    if let Err(error) = fs::fsync(&created) {
        return Ok(directory_recovery_effect(
            &lexical.public_label,
            format!("Directory v2 fsync target a eșuat: {error}"),
        ));
    }
    if let Err(error) = sync_directory(&parent.directory, &lexical.public_label) {
        return Ok(directory_recovery_effect(&lexical.public_label, error));
    }
    run_test_hook(CapabilityTestStage::BeforeDirectoryV2CheckpointCapture);
    if let Err(error) = validate_named_directory_identity(
        &parent.directory,
        &parent.leaf,
        &created,
        &lexical.public_label,
        "Directory v2 checkpoint target vs original FD",
    ) {
        return Ok(directory_recovery_effect(&lexical.public_label, error));
    }
    let checkpointed_target = match observe_directory_leaf(
        &parent.directory,
        &parent.leaf,
        &lexical.public_label,
        "Directory v2 checkpoint target",
    ) {
        Ok(ObservedDirectoryLeaf::Directory(observed))
            if observed.empty && mode_bits(&observed.stat) == DIRECTORY_V2_MODE_BITS =>
        {
            observed
        }
        Ok(_) => {
            return Ok(directory_recovery_effect(
                &lexical.public_label,
                "Directory v2 target nu mai este directorul gol planificat înainte de checkpoint",
            ));
        }
        Err(error) => return Ok(directory_recovery_effect(&lexical.public_label, error)),
    };
    if checkpointed_target.identity_digest != created_named_identity
        || checkpointed_target.state_digest != pre_checkpoint_state_digest
    {
        return Ok(directory_recovery_effect(
            &lexical.public_label,
            "Directory v2 refuză să checkpoint-eze un target înlocuit sau modificat după validarea pre-checkpoint",
        ));
    }
    let target_digest = checkpointed_target.identity_digest.clone();
    let target_state_digest = checkpointed_target.state_digest.clone();
    let checkpoint = match WalDirectoryStageCheckpoint::new(
        target_digest.clone(),
        target_state_digest.clone(),
    ) {
        Ok(checkpoint) => checkpoint,
        Err(error) => return Ok(directory_recovery_effect(&lexical.public_label, error)),
    };
    if let Err(error) = guard.mark_directory_auxiliary_durable(checkpoint) {
        return Ok(directory_recovery_effect(&lexical.public_label, error));
    }
    run_test_hook(CapabilityTestStage::AfterDirectoryV2Checkpoint);
    if let Err(error) = validate_checkpointed_target(
        &parent.directory,
        &parent.leaf,
        &created,
        &lexical.public_label,
        &target_digest,
        &target_state_digest,
        "Directory v2 pre-effect target",
    ) {
        return Ok(directory_recovery_effect(&lexical.public_label, error));
    }
    if let Err(error) = guard.mark_effect_visible() {
        return Ok(directory_recovery_effect(&lexical.public_label, error));
    }
    run_test_hook(CapabilityTestStage::BeforeDirectoryTargetDurable);
    if let Err(error) = validate_checkpointed_target(
        &parent.directory,
        &parent.leaf,
        &created,
        &lexical.public_label,
        &target_digest,
        &target_state_digest,
        "Directory v2 pre-target-durable target",
    ) {
        return Ok(directory_recovery_effect(&lexical.public_label, error));
    }
    if let Err(error) = fs::fsync(&created) {
        return Ok(directory_recovery_effect(
            &lexical.public_label,
            format!("Directory v2 fsync target a eșuat: {error}"),
        ));
    }
    if let Err(error) = sync_directory(&parent.directory, &lexical.public_label) {
        return Ok(directory_recovery_effect(&lexical.public_label, error));
    }
    if let Err(error) = validate_directory_v2_full_path(
        &lexical,
        plan,
        &target_digest,
        Some(&target_state_digest),
        true,
    ) {
        return Ok(directory_recovery_effect(&lexical.public_label, error));
    }
    if let Err(error) = guard.mark_target_durable() {
        return Ok(directory_recovery_effect(&lexical.public_label, error));
    }
    Ok(CapabilityEffect::changed(0))
}

fn validate_existing_noop(
    lexical: &LexicalTarget,
    parent: &CapturedParent,
    plan: &DirectoryOperationPlan,
) -> Result<(), String> {
    let observed = observe_directory_leaf(
        &parent.directory,
        &parent.leaf,
        &lexical.public_label,
        "Directory v2 existing no-op",
    )?;
    let ObservedDirectoryLeaf::Directory(observed) = observed else {
        return Err(capability_error(
            &lexical.public_label,
            "targetul Directory v2 existent lipsește sau și-a schimbat tipul",
        ));
    };
    if !matches_existing_baseline(&observed, &plan.evidence) {
        return Err(capability_error(
            &lexical.public_label,
            "targetul Directory v2 existent diferă de baseline",
        ));
    }
    run_test_hook(CapabilityTestStage::BeforeDirectoryV2NoopFullPath);
    validate_directory_v2_full_path(
        lexical,
        plan,
        plan.evidence
            .existing_target_identity_digest
            .as_deref()
            .unwrap_or_default(),
        None,
        false,
    )
}

fn validate_directory_v2_plan(
    lexical: &LexicalTarget,
    plan: &DirectoryOperationPlan,
) -> Result<(), String> {
    let components = plan
        .evidence
        .relative_components_hex
        .iter()
        .map(|component| decode_component_hex(component))
        .collect::<Result<Vec<_>, _>>()?;
    let (leaf, parents) = lexical
        .relative_components
        .split_last()
        .ok_or_else(|| capability_error(&lexical.public_label, "Directory v2 cere leaf"))?;
    if components != lexical.relative_components
        || plan.evidence.protocol_version != WAL_DIRECTORY_PROTOCOL_VERSION
        || plan.evidence.parent_identity.is_none()
        || plan
            .evidence
            .target_leaf_hex
            .as_deref()
            .map(decode_component_hex)
            .transpose()?
            != Some(leaf.clone())
        || plan.evidence.desired_mode_bits != Some(DIRECTORY_V2_MODE_BITS)
        || !matches!(
            plan.evidence.existing_prefix_len,
            value if value == parents.len() || value == parents.len() + 1
        )
    {
        return Err(capability_error(
            &lexical.public_label,
            "planul Directory v2 nu corespunde targetului/operației",
        ));
    }
    Ok(())
}

pub(super) fn validate_directory_v2_full_path(
    lexical: &LexicalTarget,
    plan: &DirectoryOperationPlan,
    expected_identity_digest: &str,
    expected_state_digest: Option<&str>,
    require_empty: bool,
) -> Result<(), String> {
    let authority = lexical.authority.as_ref().ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "Directory v2 postflight cere authority",
        )
    })?;
    verify_directory_authority_path(authority)?;
    let mut directory = rustix::io::dup(authority.directory()).map_err(|error| {
        capability_error(
            &lexical.public_label,
            &format!("Directory v2 postflight nu poate duplica authority: {error}"),
        )
    })?;
    let (leaf, parents) = lexical
        .relative_components
        .split_last()
        .ok_or_else(|| capability_error(&lexical.public_label, "Directory v2 cere leaf"))?;
    for component in parents {
        let next = open_directory_strict(&directory, component).map_err(|error| {
            capability_error(
                &lexical.public_label,
                &format!("Directory v2 postflight parent invalid: {error}"),
            )
        })?;
        validate_named_directory_identity(
            &directory,
            component,
            &next,
            &lexical.public_label,
            "Directory v2 postflight parent",
        )?;
        directory = next;
    }
    if wal_identity_from_fd(&directory, &lexical.public_label)?
        != *plan.evidence.parent_identity.as_ref().ok_or_else(|| {
            capability_error(
                &lexical.public_label,
                "Directory v2 parent identity lipsește",
            )
        })?
    {
        return Err(capability_error(
            &lexical.public_label,
            "Directory v2 postflight a observat alt parent",
        ));
    }
    let target = observe_directory_leaf(
        &directory,
        leaf,
        &lexical.public_label,
        "Directory v2 postflight target",
    )?;
    let ObservedDirectoryLeaf::Directory(target) = target else {
        return Err(capability_error(
            &lexical.public_label,
            "Directory v2 postflight targetul lipsește sau are alt tip",
        ));
    };
    let target_matches = if require_empty {
        target.identity_digest == expected_identity_digest
            && expected_state_digest == Some(target.state_digest.as_str())
            && target.empty
            && mode_bits(&target.stat) == DIRECTORY_V2_MODE_BITS
    } else {
        matches_existing_baseline(&target, &plan.evidence)
    };
    if !target_matches {
        return Err(capability_error(
            &lexical.public_label,
            "Directory v2 postflight targetul nu este inode-ul gol/mode checkpointed",
        ));
    }
    Ok(())
}

pub(super) fn observe_directory_leaf(
    parent: &OwnedFd,
    leaf: &OsStr,
    public_label: &str,
    role: &str,
) -> Result<ObservedDirectoryLeaf, String> {
    let Some(named) = leaf_metadata(parent, leaf, public_label)? else {
        return Ok(ObservedDirectoryLeaf::Absent);
    };
    if FileType::from_raw_mode(named.st_mode) != FileType::Directory {
        return Ok(ObservedDirectoryLeaf::Other);
    }
    let descriptor = open_directory_strict(parent, leaf).map_err(|error| {
        capability_error(
            public_label,
            &format!("{role} nu poate fi deschis fără symlink: {error}"),
        )
    })?;
    validate_open_directory_identity(&descriptor, &named, public_label, role)?;
    validate_named_directory_identity(parent, leaf, &descriptor, public_label, role)?;
    let stat_before = fs::fstat(&descriptor).map_err(|error| {
        capability_error(
            public_label,
            &format!("{role} nu poate citi metadata: {error}"),
        )
    })?;
    let identity_digest = directory_identity_digest(&descriptor)?;
    let empty = directory_is_empty(&descriptor, public_label, role)?;
    let stat = fs::fstat(&descriptor).map_err(|error| {
        capability_error(
            public_label,
            &format!("{role} nu poate reverifica metadata: {error}"),
        )
    })?;
    if !same_file_identity(&stat_before, &stat)
        || directory_state_digest(&stat_before, &identity_digest, empty)
            != directory_state_digest(&stat, &identity_digest, empty)
    {
        return Err(capability_error(
            public_label,
            &format!("{role} s-a schimbat în timpul capturii"),
        ));
    }
    validate_named_directory_identity(parent, leaf, &descriptor, public_label, role)?;
    let state_digest = directory_state_digest(&stat, &identity_digest, empty);
    Ok(ObservedDirectoryLeaf::Directory(ObservedDirectory {
        descriptor,
        version_token: version_token_for_stat(&stat),
        stat,
        identity_digest,
        state_digest,
        empty,
    }))
}

pub(super) fn directory_is_empty(
    directory: &OwnedFd,
    public_label: &str,
    role: &str,
) -> Result<bool, String> {
    let mut stream = Dir::read_from(directory).map_err(|error| {
        capability_error(
            public_label,
            &format!("{role} nu poate fi enumerat: {error}"),
        )
    })?;
    while let Some(entry) = stream.read() {
        let entry = entry.map_err(|error| {
            capability_error(public_label, &format!("{role} enumerare eșuată: {error}"))
        })?;
        let name = entry.file_name().to_bytes();
        if name != b"." && name != b".." {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn directory_identity_digest(descriptor: impl AsFd) -> Result<String, String> {
    let identity = causal_file_identity(descriptor).map_err(|error| match error {
        CausalFileIdentityError::Statx(error) => {
            format!("Directory v2 statx identity a eșuat: {error}")
        }
        CausalFileIdentityError::Incomplete => {
            "Directory v2 filesystem nu furnizează identitate statx lifetime completă".into()
        }
    })?;
    Ok(directory_identity_fields_digest(
        identity.device_major,
        identity.device_minor,
        identity.inode,
        identity.birth_time_seconds,
        identity.birth_time_nanoseconds,
        identity.mount_id,
    ))
}

fn directory_named_identity_digest(
    parent: &OwnedFd,
    leaf: &OsStr,
    public_label: &str,
) -> Result<String, String> {
    let requested =
        fs::StatxFlags::TYPE | fs::StatxFlags::INO | fs::StatxFlags::BTIME | fs::StatxFlags::MNT_ID;
    let observed =
        fs::statx(parent, leaf, AtFlags::SYMLINK_NOFOLLOW, requested).map_err(|error| {
            capability_error(
                public_label,
                &format!("Directory v2 named statx identity a eșuat: {error}"),
            )
        })?;
    if observed.stx_mask & requested.bits() != requested.bits()
        || observed.stx_btime.tv_nsec >= 1_000_000_000
    {
        return Err(capability_error(
            public_label,
            "Directory v2 named statx identity este incompletă",
        ));
    }
    Ok(directory_identity_fields_digest(
        observed.stx_dev_major,
        observed.stx_dev_minor,
        observed.stx_ino,
        observed.stx_btime.tv_sec,
        observed.stx_btime.tv_nsec,
        observed.stx_mnt_id,
    ))
}

fn directory_identity_fields_digest(
    device_major: u32,
    device_minor: u32,
    inode: u64,
    birth_time_seconds: i64,
    birth_time_nanoseconds: u32,
    mount_id: u64,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"pana-directory-direct-identity-v3\0");
    digest.update(device_major.to_le_bytes());
    digest.update(device_minor.to_le_bytes());
    digest.update(inode.to_le_bytes());
    digest.update(birth_time_seconds.to_le_bytes());
    digest.update(birth_time_nanoseconds.to_le_bytes());
    digest.update(mount_id.to_le_bytes());
    let encoded = format!("{:x}", digest.finalize());
    encoded[..32].to_string()
}

/// State-ul cauzal al targetului direct include ctime. După fchmod nu mai există
/// nicio relocare legitimă a inode-ului, deci add/remove sau restaurările de
/// metadata prin utimensat nu pot reveni la checkpoint fără să schimbe ctime.
pub(super) fn directory_state_digest(
    stat: &fs::Stat,
    identity_digest: &str,
    empty: bool,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"pana-directory-direct-state-v3\0");
    digest.update(identity_digest.as_bytes());
    digest.update(b"\0");
    digest.update(stat.st_ctime.to_le_bytes());
    digest.update(stat.st_ctime_nsec.to_le_bytes());
    digest.update(stat.st_mtime.to_le_bytes());
    digest.update(stat.st_mtime_nsec.to_le_bytes());
    digest.update(stat.st_size.to_le_bytes());
    digest.update(stat.st_nlink.to_le_bytes());
    digest.update(stat.st_mode.to_le_bytes());
    digest.update(stat.st_uid.to_le_bytes());
    digest.update(stat.st_gid.to_le_bytes());
    digest.update([u8::from(empty)]);
    let encoded = format!("{:x}", digest.finalize());
    encoded[..32].to_string()
}

pub(super) fn matches_existing_baseline(
    observed: &ObservedDirectory,
    evidence: &WalDirectoryEvidence,
) -> bool {
    evidence
        .existing_target_identity
        .as_ref()
        .is_some_and(|identity| {
            identity.device == observed.stat.st_dev && identity.inode == observed.stat.st_ino
        })
        && evidence.existing_target_identity_digest.as_deref()
            == Some(observed.identity_digest.as_str())
        && evidence.existing_target_version_token.as_deref()
            == Some(observed.version_token.as_str())
}

fn validate_staged_directory(
    parent: &OwnedFd,
    leaf: &OsStr,
    staged: &OwnedFd,
    public_label: &str,
    role: &str,
) -> Result<fs::Stat, String> {
    validate_named_directory_identity(parent, leaf, staged, public_label, role)?;
    let stat = fs::fstat(staged).map_err(|error| {
        capability_error(public_label, &format!("{role} fstat a eșuat: {error}"))
    })?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory
        || mode_bits(&stat) != DIRECTORY_V2_MODE_BITS
        || !directory_is_empty(staged, public_label, role)?
    {
        return Err(capability_error(
            public_label,
            &format!("{role} nu este director gol cu mode 0755"),
        ));
    }
    Ok(stat)
}

fn validate_checkpointed_target(
    parent: &OwnedFd,
    leaf: &OsStr,
    created: &OwnedFd,
    public_label: &str,
    expected_identity_digest: &str,
    expected_state_digest: &str,
    role: &str,
) -> Result<(), String> {
    validate_named_directory_identity(parent, leaf, created, public_label, role)?;
    let observed = observe_directory_leaf(parent, leaf, public_label, role)?;
    let ObservedDirectoryLeaf::Directory(observed) = observed else {
        return Err(capability_error(
            public_label,
            &format!("{role} lipsește sau are alt tip"),
        ));
    };
    if observed.identity_digest != expected_identity_digest
        || observed.state_digest != expected_state_digest
        || !observed.empty
        || mode_bits(&observed.stat) != DIRECTORY_V2_MODE_BITS
    {
        return Err(capability_error(
            public_label,
            &format!("{role} nu mai este targetul gol/mode checkpointed"),
        ));
    }
    Ok(())
}

pub(super) const fn mode_bits(stat: &fs::Stat) -> u32 {
    stat.st_mode & 0o7777
}

fn directory_recovery_effect(
    public_label: &str,
    diagnostic: impl Into<String>,
) -> CapabilityEffect {
    CapabilityEffect::recovery_required(
        0,
        capability_error(
            public_label,
            &format!(
                "{} WAL Directory v2 rămâne hot; zero cleanup/adoption automată",
                diagnostic.into()
            ),
        ),
    )
}
