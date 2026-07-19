use std::{os::fd::AsFd, os::unix::ffi::OsStrExt};

use sha2::{Digest, Sha256};

use super::anonymous_file::{causal_file_identity, CausalFileIdentityError};
use super::*;

#[path = "symlink/recovery.rs"]
mod recovery;

pub(in crate::kernel::write_authority::capability) use recovery::{
    classify_symlink_recovery, execute_symlink_recovery, resolve_symlink_operator,
};

#[derive(Debug)]
pub(super) enum ObservedSymlinkLeaf {
    Absent,
    Other,
    Symlink(ObservedSymlink),
}

#[derive(Debug)]
pub(super) struct ObservedSymlink {
    pub descriptor: OwnedFd,
    pub identity: WalFilesystemIdentity,
    pub identity_digest: String,
    pub version_token: String,
    pub state_digest: String,
    pub link_target_hex: String,
}

#[cfg(test)]
pub(in crate::kernel::write_authority::capability) fn plan_legacy_symlink_for_test(
    target: &WriteTarget,
    source: &Path,
) -> Result<SymlinkOperationPlan, String> {
    super::lifecycle::plan_legacy_symlink(target, source)
}

pub(in crate::kernel::write_authority::capability) fn plan_symlink(
    target: &WriteTarget,
    source: &Path,
) -> Result<SymlinkOperationPlan, String> {
    validate_symlink_literal(source, &target.public_label)?;
    if matches!(target.expected_leaf, ExpectedLeaf::Present(_)) {
        return Err(capability_error(
            &target.public_label,
            "Symlink v2 refuză ExpectedLeaf::Present",
        ));
    }
    let lexical = lexical_target(target, false)?;
    let authority = lexical.authority.as_ref().ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "planul Symlink v2 cere authority root sigilat",
        )
    })?;
    if !matches!(
        authority.scope(),
        DirectoryAuthorityScope::ApplicationPreviewCache
    ) {
        return Err(capability_error(
            &lexical.public_label,
            "Symlink v2 este rezervat authority application_preview_cache",
        ));
    }
    verify_directory_authority_path(authority)?;
    let boundary = capture_existing_boundary(&lexical)?.ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "authority root nu există pentru Symlink v2",
        )
    })?;
    let (target_leaf, parents) = lexical
        .relative_components
        .split_last()
        .ok_or_else(|| capability_error(&lexical.public_label, "Symlink v2 cere un leaf"))?;
    let mut parent = boundary.directory;
    for component in parents {
        let next = open_directory_strict(&parent, component).map_err(|error| {
            capability_error(
                &lexical.public_label,
                &format!(
                    "Symlink v2 cere parent final existent integral; componenta este invalidă: {error}"
                ),
            )
        })?;
        validate_named_directory_identity(
            &parent,
            component,
            &next,
            &lexical.public_label,
            "symlink-v2-plan-parent",
        )?;
        parent = next;
    }
    let parent_identity = wal_identity_from_fd(&parent, &lexical.public_label)?;
    let observed = observe_symlink_leaf(
        &parent,
        target_leaf,
        &lexical.public_label,
        "Symlink v2 plan target",
    )?;
    let desired_link_target_hex = encode_path_hex(source);
    let before = match (&target.expected_leaf, observed) {
        (ExpectedLeaf::Absent, ObservedSymlinkLeaf::Absent)
        | (ExpectedLeaf::Unspecified, ObservedSymlinkLeaf::Absent) => WalSymlinkBefore::Absent,
        (ExpectedLeaf::Absent, _) => {
            return Err(capability_error(
                &lexical.public_label,
                "Symlink v2 ExpectedLeaf::Absent a găsit target existent",
            ));
        }
        (ExpectedLeaf::Unspecified, ObservedSymlinkLeaf::Symlink(observed))
            if observed.link_target_hex == desired_link_target_hex =>
        {
            WalSymlinkBefore::Exact {
                identity: observed.identity,
                version_token: observed.version_token,
                link_target_hex: observed.link_target_hex,
                identity_digest: Some(observed.identity_digest),
                state_digest: Some(observed.state_digest),
            }
        }
        (ExpectedLeaf::Unspecified, ObservedSymlinkLeaf::Symlink(_)) => {
            return Err(capability_error(
                &lexical.public_label,
                "Symlink v2 a găsit un symlink către alt literal",
            ));
        }
        (ExpectedLeaf::Unspecified, ObservedSymlinkLeaf::Other) => {
            return Err(capability_error(
                &lexical.public_label,
                "Symlink v2 a găsit target de alt tip",
            ));
        }
        (ExpectedLeaf::Present(_), _) => unreachable!("Present rejected above"),
    };

    Ok(SymlinkOperationPlan {
        evidence: WalSymlinkEvidence {
            protocol_version: WAL_SYMLINK_PROTOCOL_VERSION,
            parent: WalParentEvidence {
                relative_components_hex: parents
                    .iter()
                    .map(|component| encode_component_hex(component))
                    .collect(),
                existing_prefix_len: parents.len(),
                existing_ancestor_identity: parent_identity.clone(),
                parent_identity: Some(parent_identity),
            },
            target_leaf_hex: encode_component_hex(target_leaf),
            desired_link_target_hex,
            before,
        },
    })
}

pub(in crate::kernel::write_authority::capability) fn symlink_entry_wal(
    target: &WriteTarget,
    source: &Path,
    plan: &SymlinkOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    if plan.evidence.protocol_version == 0 {
        return super::lifecycle::symlink_entry_legacy_wal(target, source, plan, guard);
    }
    if plan.evidence.protocol_version != WAL_SYMLINK_PROTOCOL_VERSION {
        return Err("Symlink runtime refuză protocolul WAL necunoscut.".into());
    }
    symlink_entry_v2(target, source, plan, guard)
}

fn symlink_entry_v2(
    target: &WriteTarget,
    source: &Path,
    plan: &SymlinkOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    validate_symlink_literal(source, &target.public_label)?;
    let lexical = lexical_target(target, false)?;
    validate_symlink_v2_plan(&lexical, source, plan)?;
    let parent = capture_existing_target_parent(&lexical)?
        .ok_or_else(|| capability_error(&lexical.public_label, "parentul Symlink v2 lipsește"))?;
    if parent.created_ancestors {
        return Err(capability_error(
            &lexical.public_label,
            "Symlink v2 interzice crearea implicită de parent",
        ));
    }
    fs::flock(&parent.directory, FlockOperation::LockExclusive).map_err(|error| {
        capability_error(
            &lexical.public_label,
            &format!("Symlink v2 stable parent lock a eșuat: {error}"),
        )
    })?;
    let expected_parent = plan
        .evidence
        .parent
        .parent_identity
        .as_ref()
        .ok_or_else(|| {
            capability_error(&lexical.public_label, "Symlink v2 nu are parent identity")
        })?;
    if wal_identity_from_fd(&parent.directory, &lexical.public_label)? != *expected_parent {
        return Err(capability_error(
            &lexical.public_label,
            "parentul Symlink v2 diferă de plan",
        ));
    }

    if matches!(plan.evidence.before, WalSymlinkBefore::Exact { .. }) {
        validate_existing_noop(&lexical, &parent, plan)?;
        return Ok(CapabilityEffect::unchanged());
    }
    if !matches!(
        observe_symlink_leaf(
            &parent.directory,
            &parent.leaf,
            &lexical.public_label,
            "Symlink v2 pre-create target",
        )?,
        ObservedSymlinkLeaf::Absent
    ) {
        return Err(capability_error(
            &lexical.public_label,
            "targetul Symlink v2 a apărut după planificare",
        ));
    }

    let desired_target = plan.desired_target()?;
    if let Err(error) = fs::symlinkat(&desired_target, &parent.directory, &parent.leaf) {
        return Ok(symlink_recovery_effect(
            &lexical.public_label,
            format!("Symlink v2 symlinkat direct create-only a eșuat: {error}"),
        ));
    }
    // `symlinkat` nu returnează FD. Primul syscall post-create este openat cu
    // O_PATH|O_NOFOLLOW asupra leaf-ului însuși. Intervalul până la această
    // captură rămâne limita explicită a modelului cooperative-writer.
    let created = match open_symlink_strict(&parent.directory, &parent.leaf) {
        Ok(created) => created,
        Err(error) => {
            return Ok(symlink_recovery_effect(
                &lexical.public_label,
                format!("Symlink v2 targetul creat nu poate fi deschis O_PATH: {error}"),
            ));
        }
    };
    run_test_hook(CapabilityTestStage::AfterSymlinkV2FirstOpenBeforeCapture);
    let created_observed = match capture_symlink_from_fd(
        created,
        &lexical.public_label,
        "Symlink v2 first-open target",
    ) {
        Ok(observed) => observed,
        Err(error) => return Ok(symlink_recovery_effect(&lexical.public_label, error)),
    };
    if created_observed.link_target_hex != plan.evidence.desired_link_target_hex {
        return Ok(symlink_recovery_effect(
            &lexical.public_label,
            "Symlink v2 first-open nu conține literalul planificat",
        ));
    }
    if let Err(error) = validate_named_symlink_binding(
        &parent.directory,
        &parent.leaf,
        &created_observed,
        &lexical.public_label,
        "Symlink v2 first-open binding",
    ) {
        return Ok(symlink_recovery_effect(&lexical.public_label, error));
    }
    run_test_hook(CapabilityTestStage::AfterSymlinkCreateBeforePhase);
    if let Err(error) = sync_directory(&parent.directory, &lexical.public_label) {
        return Ok(symlink_recovery_effect(&lexical.public_label, error));
    }
    run_test_hook(CapabilityTestStage::BeforeSymlinkV2CheckpointCapture);
    if let Err(error) = validate_named_symlink_binding(
        &parent.directory,
        &parent.leaf,
        &created_observed,
        &lexical.public_label,
        "Symlink v2 checkpoint target vs original FD",
    ) {
        return Ok(symlink_recovery_effect(&lexical.public_label, error));
    }
    let checkpointed = match observe_symlink_leaf(
        &parent.directory,
        &parent.leaf,
        &lexical.public_label,
        "Symlink v2 checkpoint target",
    ) {
        Ok(ObservedSymlinkLeaf::Symlink(observed)) => observed,
        Ok(_) => {
            return Ok(symlink_recovery_effect(
                &lexical.public_label,
                "Symlink v2 targetul lipsește sau are alt tip înainte de checkpoint",
            ));
        }
        Err(error) => return Ok(symlink_recovery_effect(&lexical.public_label, error)),
    };
    if checkpointed.identity_digest != created_observed.identity_digest
        || checkpointed.state_digest != created_observed.state_digest
        || checkpointed.link_target_hex != plan.evidence.desired_link_target_hex
    {
        return Ok(symlink_recovery_effect(
            &lexical.public_label,
            "Symlink v2 refuză checkpointul unui target înlocuit sau modificat",
        ));
    }
    let checkpoint = match WalSymlinkStageCheckpoint::new(
        checkpointed.identity_digest.clone(),
        checkpointed.state_digest.clone(),
    ) {
        Ok(checkpoint) => checkpoint,
        Err(error) => return Ok(symlink_recovery_effect(&lexical.public_label, error)),
    };
    if let Err(error) = guard.mark_symlink_auxiliary_durable(checkpoint) {
        return Ok(symlink_recovery_effect(&lexical.public_label, error));
    }
    run_test_hook(CapabilityTestStage::AfterSymlinkV2Checkpoint);
    if let Err(error) = validate_checkpointed_target(
        &parent.directory,
        &parent.leaf,
        &created_observed,
        plan,
        &lexical.public_label,
        "Symlink v2 pre-effect target",
    ) {
        return Ok(symlink_recovery_effect(&lexical.public_label, error));
    }
    if let Err(error) = guard.mark_effect_visible() {
        return Ok(symlink_recovery_effect(&lexical.public_label, error));
    }
    run_test_hook(CapabilityTestStage::BeforeSymlinkTargetDurable);
    if let Err(error) = validate_checkpointed_target(
        &parent.directory,
        &parent.leaf,
        &created_observed,
        plan,
        &lexical.public_label,
        "Symlink v2 pre-target-durable target",
    ) {
        return Ok(symlink_recovery_effect(&lexical.public_label, error));
    }
    if let Err(error) = sync_directory(&parent.directory, &lexical.public_label) {
        return Ok(symlink_recovery_effect(&lexical.public_label, error));
    }
    if let Err(error) = validate_symlink_v2_full_path(
        &lexical,
        plan,
        &created_observed.identity_digest,
        &created_observed.state_digest,
    ) {
        return Ok(symlink_recovery_effect(&lexical.public_label, error));
    }
    if let Err(error) = guard.mark_target_durable() {
        return Ok(symlink_recovery_effect(&lexical.public_label, error));
    }
    Ok(CapabilityEffect::changed(0))
}

fn validate_existing_noop(
    lexical: &LexicalTarget,
    parent: &CapturedParent,
    plan: &SymlinkOperationPlan,
) -> Result<(), String> {
    let observed = observe_symlink_leaf(
        &parent.directory,
        &parent.leaf,
        &lexical.public_label,
        "Symlink v2 existing no-op",
    )?;
    let ObservedSymlinkLeaf::Symlink(observed) = observed else {
        return Err(capability_error(
            &lexical.public_label,
            "Symlink v2 no-op lipsește sau și-a schimbat tipul",
        ));
    };
    if !matches_existing_baseline(&observed, &plan.evidence) {
        return Err(capability_error(
            &lexical.public_label,
            "Symlink v2 no-op diferă de baseline-ul exact",
        ));
    }
    run_test_hook(CapabilityTestStage::BeforeSymlinkV2NoopFullPath);
    validate_symlink_v2_full_path(
        lexical,
        plan,
        &observed.identity_digest,
        &observed.state_digest,
    )
}

fn validate_symlink_v2_plan(
    lexical: &LexicalTarget,
    source: &Path,
    plan: &SymlinkOperationPlan,
) -> Result<(), String> {
    let (leaf, parents) = lexical
        .relative_components
        .split_last()
        .ok_or_else(|| capability_error(&lexical.public_label, "Symlink v2 cere leaf"))?;
    let planned_parents = plan
        .evidence
        .parent
        .relative_components_hex
        .iter()
        .map(|component| decode_component_hex(component))
        .collect::<Result<Vec<_>, _>>()?;
    let authority = lexical.authority.as_ref().ok_or_else(|| {
        capability_error(&lexical.public_label, "Symlink v2 runtime cere authority")
    })?;
    if plan.evidence.protocol_version != WAL_SYMLINK_PROTOCOL_VERSION
        || planned_parents != parents
        || decode_component_hex(&plan.evidence.target_leaf_hex)? != *leaf
        || plan.evidence.desired_link_target_hex != encode_path_hex(source)
        || plan.evidence.parent.existing_prefix_len != parents.len()
        || plan.evidence.parent.parent_identity.is_none()
        || matches!(
            &plan.evidence.before,
            WalSymlinkBefore::Exact { link_target_hex, .. }
                if link_target_hex != &plan.evidence.desired_link_target_hex
        )
        || !matches!(
            authority.scope(),
            DirectoryAuthorityScope::ApplicationPreviewCache
        )
    {
        return Err(capability_error(
            &lexical.public_label,
            "planul Symlink v2 nu corespunde targetului/operației",
        ));
    }
    Ok(())
}

pub(super) fn validate_symlink_v2_full_path(
    lexical: &LexicalTarget,
    plan: &SymlinkOperationPlan,
    expected_identity_digest: &str,
    expected_state_digest: &str,
) -> Result<(), String> {
    let authority = lexical.authority.as_ref().ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "Symlink v2 postflight cere authority",
        )
    })?;
    verify_directory_authority_path(authority)?;
    let mut parent = rustix::io::dup(authority.directory()).map_err(|error| {
        capability_error(
            &lexical.public_label,
            &format!("Symlink v2 postflight nu poate duplica authority: {error}"),
        )
    })?;
    let (leaf, parents) = lexical
        .relative_components
        .split_last()
        .ok_or_else(|| capability_error(&lexical.public_label, "Symlink v2 cere leaf"))?;
    for component in parents {
        let next = open_directory_strict(&parent, component).map_err(|error| {
            capability_error(
                &lexical.public_label,
                &format!("Symlink v2 postflight parent invalid: {error}"),
            )
        })?;
        validate_named_directory_identity(
            &parent,
            component,
            &next,
            &lexical.public_label,
            "Symlink v2 postflight parent",
        )?;
        parent = next;
    }
    if wal_identity_from_fd(&parent, &lexical.public_label)?
        != *plan
            .evidence
            .parent
            .parent_identity
            .as_ref()
            .ok_or_else(|| {
                capability_error(&lexical.public_label, "Symlink v2 parent identity lipsește")
            })?
    {
        return Err(capability_error(
            &lexical.public_label,
            "Symlink v2 postflight a observat alt parent",
        ));
    }
    let observed = observe_symlink_leaf(
        &parent,
        leaf,
        &lexical.public_label,
        "Symlink v2 full-path target",
    )?;
    let ObservedSymlinkLeaf::Symlink(observed) = observed else {
        return Err(capability_error(
            &lexical.public_label,
            "Symlink v2 full-path target lipsește sau are alt tip",
        ));
    };
    if observed.identity_digest != expected_identity_digest
        || observed.state_digest != expected_state_digest
        || observed.link_target_hex != plan.evidence.desired_link_target_hex
    {
        return Err(capability_error(
            &lexical.public_label,
            "Symlink v2 full-path target diferă de lifetime/state/literal",
        ));
    }
    Ok(())
}

pub(super) fn observe_symlink_leaf(
    parent: &OwnedFd,
    leaf: &OsStr,
    public_label: &str,
    role: &str,
) -> Result<ObservedSymlinkLeaf, String> {
    let Some(named) = leaf_metadata(parent, leaf, public_label)? else {
        return Ok(ObservedSymlinkLeaf::Absent);
    };
    if FileType::from_raw_mode(named.st_mode) != FileType::Symlink {
        return Ok(ObservedSymlinkLeaf::Other);
    }
    let descriptor = open_symlink_strict(parent, leaf).map_err(|error| {
        capability_error(
            public_label,
            &format!("{role} nu poate deschide leaf-ul O_PATH|O_NOFOLLOW: {error}"),
        )
    })?;
    let observed = capture_symlink_from_fd(descriptor, public_label, role)?;
    validate_named_symlink_binding(parent, leaf, &observed, public_label, role)?;
    Ok(ObservedSymlinkLeaf::Symlink(observed))
}

fn open_symlink_strict(parent: &OwnedFd, leaf: &OsStr) -> Result<OwnedFd, Errno> {
    fs::openat(
        parent,
        leaf,
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
}

fn capture_symlink_from_fd(
    descriptor: OwnedFd,
    public_label: &str,
    role: &str,
) -> Result<ObservedSymlink, String> {
    let before = fs::fstat(&descriptor).map_err(|error| {
        capability_error(
            public_label,
            &format!("{role} fstat inițial a eșuat: {error}"),
        )
    })?;
    if FileType::from_raw_mode(before.st_mode) != FileType::Symlink || before.st_nlink != 1 {
        return Err(capability_error(
            public_label,
            &format!("{role} cere un symlink cu exact un link"),
        ));
    }
    let identity_digest = symlink_identity_digest(&descriptor)?;
    let literal = readlink_fd_raw(&descriptor, public_label, role)?;
    let after = fs::fstat(&descriptor).map_err(|error| {
        capability_error(
            public_label,
            &format!("{role} fstat final a eșuat: {error}"),
        )
    })?;
    if !same_stable_leaf_version(&before, &after)
        || symlink_state_digest(&before, &identity_digest, &literal)
            != symlink_state_digest(&after, &identity_digest, &literal)
    {
        return Err(capability_error(
            public_label,
            &format!("{role} s-a schimbat în timpul capturii FD"),
        ));
    }
    if after.st_size < 0 || after.st_size as usize != literal.len() {
        return Err(capability_error(
            public_label,
            &format!("{role} are size diferit de literalul raw"),
        ));
    }
    let link_target_hex = encode_bytes_hex(&literal);
    Ok(ObservedSymlink {
        descriptor,
        identity: WalFilesystemIdentity {
            device: after.st_dev,
            inode: after.st_ino,
        },
        version_token: version_token_for_stat(&after),
        state_digest: symlink_state_digest(&after, &identity_digest, &literal),
        identity_digest,
        link_target_hex,
    })
}

fn validate_named_symlink_binding(
    parent: &OwnedFd,
    leaf: &OsStr,
    expected: &ObservedSymlink,
    public_label: &str,
    role: &str,
) -> Result<(), String> {
    let named_before = fs::statat(parent, leaf, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
        capability_error(
            public_label,
            &format!("{role} named stat inițial a eșuat: {error}"),
        )
    })?;
    let descriptor_stat = fs::fstat(&expected.descriptor).map_err(|error| {
        capability_error(public_label, &format!("{role} FD fstat a eșuat: {error}"))
    })?;
    if FileType::from_raw_mode(named_before.st_mode) != FileType::Symlink
        || named_before.st_nlink != 1
        || !same_file_identity(&named_before, &descriptor_stat)
        || symlink_named_identity_digest(parent, leaf, public_label)? != expected.identity_digest
    {
        return Err(capability_error(
            public_label,
            &format!("{role} numele nu mai indică FD-ul symlink capturat"),
        ));
    }
    let named_literal = readlink_named_raw(parent, leaf, public_label, role)?;
    let named_after = fs::statat(parent, leaf, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
        capability_error(
            public_label,
            &format!("{role} named stat final a eșuat: {error}"),
        )
    })?;
    if !same_stable_leaf_version(&named_before, &named_after)
        || symlink_state_digest(&named_after, &expected.identity_digest, &named_literal)
            != expected.state_digest
        || encode_bytes_hex(&named_literal) != expected.link_target_hex
    {
        return Err(capability_error(
            public_label,
            &format!("{role} named state/literal diferă de FD-ul capturat"),
        ));
    }
    Ok(())
}

fn readlink_fd_raw(
    descriptor: impl AsFd,
    public_label: &str,
    role: &str,
) -> Result<Vec<u8>, String> {
    readlink_raw(descriptor, "", public_label, role)
}

fn readlink_named_raw(
    parent: &OwnedFd,
    leaf: &OsStr,
    public_label: &str,
    role: &str,
) -> Result<Vec<u8>, String> {
    readlink_raw(parent, leaf, public_label, role)
}

fn readlink_raw(
    directory: impl AsFd,
    path: impl rustix::path::Arg,
    public_label: &str,
    role: &str,
) -> Result<Vec<u8>, String> {
    let mut literal = vec![0_u8; MAX_WAL_SYMLINK_TARGET_BYTES + 1];
    let length = fs::readlinkat_raw(directory, path, &mut literal).map_err(|error| {
        capability_error(
            public_label,
            &format!("{role} readlinkat_raw a eșuat: {error}"),
        )
    })?;
    literal.truncate(length);
    if literal.is_empty() || literal.len() > MAX_WAL_SYMLINK_TARGET_BYTES || literal.contains(&0) {
        return Err(capability_error(
            public_label,
            &format!("{role} literalul raw depășește contractul WAL"),
        ));
    }
    Ok(literal)
}

pub(super) fn symlink_identity_digest(descriptor: impl AsFd) -> Result<String, String> {
    let identity = causal_file_identity(descriptor).map_err(|error| match error {
        CausalFileIdentityError::Statx(error) => {
            format!("Symlink v2 statx identity a eșuat: {error}")
        }
        CausalFileIdentityError::Incomplete => {
            "Symlink v2 filesystem nu furnizează identitate statx lifetime completă".into()
        }
    })?;
    Ok(symlink_identity_fields_digest(
        identity.device_major,
        identity.device_minor,
        identity.inode,
        identity.birth_time_seconds,
        identity.birth_time_nanoseconds,
        identity.mount_id,
    ))
}

fn symlink_named_identity_digest(
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
                &format!("Symlink v2 named statx identity a eșuat: {error}"),
            )
        })?;
    if observed.stx_mask & requested.bits() != requested.bits()
        || observed.stx_btime.tv_nsec >= 1_000_000_000
    {
        return Err(capability_error(
            public_label,
            "Symlink v2 named statx identity este incompletă",
        ));
    }
    Ok(symlink_identity_fields_digest(
        observed.stx_dev_major,
        observed.stx_dev_minor,
        observed.stx_ino,
        observed.stx_btime.tv_sec,
        observed.stx_btime.tv_nsec,
        observed.stx_mnt_id,
    ))
}

fn symlink_identity_fields_digest(
    device_major: u32,
    device_minor: u32,
    inode: u64,
    birth_time_seconds: i64,
    birth_time_nanoseconds: u32,
    mount_id: u64,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"pana-symlink-direct-identity-v2\0");
    digest.update(device_major.to_le_bytes());
    digest.update(device_minor.to_le_bytes());
    digest.update(inode.to_le_bytes());
    digest.update(birth_time_seconds.to_le_bytes());
    digest.update(birth_time_nanoseconds.to_le_bytes());
    digest.update(mount_id.to_le_bytes());
    let encoded = format!("{:x}", digest.finalize());
    encoded[..32].to_string()
}

pub(super) fn symlink_state_digest(
    stat: &fs::Stat,
    identity_digest: &str,
    literal: &[u8],
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"pana-symlink-direct-state-v2\0");
    digest.update(identity_digest.as_bytes());
    digest.update(b"\0");
    digest.update(stat.st_mode.to_le_bytes());
    digest.update(stat.st_nlink.to_le_bytes());
    digest.update(stat.st_uid.to_le_bytes());
    digest.update(stat.st_gid.to_le_bytes());
    digest.update(stat.st_size.to_le_bytes());
    digest.update(stat.st_mtime.to_le_bytes());
    digest.update(stat.st_mtime_nsec.to_le_bytes());
    digest.update(stat.st_ctime.to_le_bytes());
    digest.update(stat.st_ctime_nsec.to_le_bytes());
    digest.update((literal.len() as u64).to_le_bytes());
    digest.update(literal);
    let encoded = format!("{:x}", digest.finalize());
    encoded[..32].to_string()
}

pub(super) fn matches_existing_baseline(
    observed: &ObservedSymlink,
    evidence: &WalSymlinkEvidence,
) -> bool {
    matches!(
        &evidence.before,
        WalSymlinkBefore::Exact {
            identity,
            version_token,
            link_target_hex,
            identity_digest: Some(expected_identity_digest),
            state_digest: Some(expected_state_digest),
        } if *identity == observed.identity
            && version_token == &observed.version_token
            && link_target_hex == &observed.link_target_hex
            && link_target_hex == &evidence.desired_link_target_hex
            && expected_identity_digest == &observed.identity_digest
            && expected_state_digest == &observed.state_digest
    )
}

fn validate_checkpointed_target(
    parent: &OwnedFd,
    leaf: &OsStr,
    created: &ObservedSymlink,
    plan: &SymlinkOperationPlan,
    public_label: &str,
    role: &str,
) -> Result<(), String> {
    validate_named_symlink_binding(parent, leaf, created, public_label, role).map_err(|error| {
        capability_error(public_label, &format!("{role} binding invalid: {error}"))
    })?;
    let observed = observe_symlink_leaf(parent, leaf, public_label, role)?;
    let ObservedSymlinkLeaf::Symlink(observed) = observed else {
        return Err(format!("{role} lipsește sau are alt tip"));
    };
    if observed.identity_digest != created.identity_digest
        || observed.state_digest != created.state_digest
        || observed.link_target_hex != plan.evidence.desired_link_target_hex
    {
        return Err(format!(
            "{role} nu mai este lifetime/state/literalul checkpointed"
        ));
    }
    Ok(())
}

fn validate_symlink_literal(source: &Path, public_label: &str) -> Result<(), String> {
    let bytes = source.as_os_str().as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_WAL_SYMLINK_TARGET_BYTES || bytes.contains(&0) {
        return Err(capability_error(
            public_label,
            "literalul Symlink v2 este gol, conține NUL sau depășește limita WAL",
        ));
    }
    Ok(())
}

fn symlink_recovery_effect(public_label: &str, diagnostic: impl Into<String>) -> CapabilityEffect {
    CapabilityEffect::recovery_required(
        0,
        capability_error(
            public_label,
            &format!(
                "{} WAL Symlink v2 rămâne hot; zero cleanup/adoption automată",
                diagnostic.into()
            ),
        ),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn direct_runtime_has_one_symlinkat_and_zero_temp_rename_unlink_paths() {
        let source = include_str!("symlink.rs");
        let runtime = source
            .split_once("fn symlink_entry_v2(")
            .and_then(|(_, suffix)| suffix.split_once("fn validate_existing_noop("))
            .map(|(runtime, _)| runtime)
            .expect("Symlink v2 runtime source boundaries");
        assert_eq!(runtime.matches("fs::symlinkat(").count(), 1);
        for forbidden in [
            "renameat",
            "RenameFlags",
            "unlinkat",
            "fs::linkat(",
            "std::fs::rename",
            "std::fs::remove_file",
            "std::fs::remove_dir",
            "std::fs::remove_dir_all",
            "fs::rename",
            "fs::remove_file",
            "fs::remove_dir",
            "fs::remove_dir_all",
            "rmdir",
            "temp_leaf",
        ] {
            assert!(!runtime.contains(forbidden), "{forbidden}: {runtime}");
        }

        let create = runtime.find("fs::symlinkat(").unwrap();
        let first_open = runtime.find("open_symlink_strict(").unwrap();
        let first_hook = runtime.find("run_test_hook(").unwrap();
        assert!(create < first_open && first_open < first_hook);
    }

    #[test]
    fn recovery_has_zero_namespace_cleanup_primitives() {
        let source = include_str!("symlink/recovery.rs");
        for forbidden in [
            "renameat",
            "RenameFlags",
            "unlinkat",
            "symlinkat",
            "fs::linkat(",
            "std::fs::rename",
            "std::fs::remove_file",
            "std::fs::remove_dir",
            "std::fs::remove_dir_all",
            "fs::rename",
            "fs::remove_file",
            "fs::remove_dir",
            "fs::remove_dir_all",
            "rmdir",
            "temp_leaf",
        ] {
            assert!(!source.contains(forbidden), "{forbidden}: {source}");
        }
    }
}
