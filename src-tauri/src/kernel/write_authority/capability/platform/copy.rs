use super::anonymous_file::{
    causal_file_identity, link_anonymous_file_create_only, open_anonymous_file,
    AnonymousFileLinkError, CausalFileIdentityError,
};
use super::*;

#[path = "copy/recovery.rs"]
mod recovery;

pub(in crate::kernel::write_authority::capability) use recovery::{
    classify_copy_recovery, execute_copy_recovery, resolve_copy_operator,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct ObservedCopyLeaf {
    identity: WalFilesystemIdentity,
    size: u64,
    version_token: String,
    mode_bits: u32,
}

pub(in crate::kernel::write_authority::capability) fn plan_copy(
    target: &WriteTarget,
    source: &Path,
    replace_policy: CapabilityReplacePolicy,
    operation_id: &str,
) -> Result<CopyOperationPlan, String> {
    let lexical = lexical_target(target, false)?;
    let authority = lexical.authority.as_ref().ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "planul copy WAL cere authority root sigilat",
        )
    })?;
    match replace_policy {
        CapabilityReplacePolicy::CreateNew
            if !matches!(
                authority.scope(),
                DirectoryAuthorityScope::ProjectBootstrap { .. }
            ) =>
        {
            return Err(capability_error(
                &lexical.public_label,
                "Copy create-only este permis numai sub authority ProjectBootstrap",
            ));
        }
        CapabilityReplacePolicy::Replace
            if !matches!(
                authority.scope(),
                DirectoryAuthorityScope::ApplicationPreviewCache
            ) =>
        {
            return Err(capability_error(
                &lexical.public_label,
                "Copy overwrite este permis numai în ApplicationPreviewCache rebuildable",
            ));
        }
        CapabilityReplacePolicy::CreateNew | CapabilityReplacePolicy::Replace => {}
    }

    let mut source_file = open_copy_source(source)?;
    let source_evidence = capture_copy_source_evidence(&mut source_file, source)?;

    let boundary = capture_existing_boundary(&lexical)?.ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "authority root nu există pentru planul copy",
        )
    })?;
    let (leaf, parents) = lexical
        .relative_components
        .split_last()
        .ok_or_else(|| capability_error(&lexical.public_label, "planul copy cere un leaf"))?;
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
                    "copy WAL parent",
                )?;
                directory = next;
                existing_prefix_len += 1;
            }
            Err(Errno::NOENT) => break,
            Err(error) => {
                return Err(capability_error(
                    &lexical.public_label,
                    &format!("planul copy nu poate captura un părinte: {error}"),
                ));
            }
        }
    }
    let existing_ancestor_identity = wal_identity_from_fd(&directory, &lexical.public_label)?;
    let parent_exists = existing_prefix_len == parents.len();
    if !parent_exists {
        return Err(capability_error(
            &lexical.public_label,
            "Copy v2 cere parent existent integral; namespace-ul aparține operației CreateDirectory",
        ));
    }
    let parent_identity = Some(wal_identity_from_fd(&directory, &lexical.public_label)?);

    validate_copy_destination(&directory, leaf, replace_policy, &lexical)?;
    let (before, before_mode_bits) = capture_copy_before(
        &directory,
        leaf,
        &target.expected_leaf,
        &lexical.public_label,
    )?;
    if matches!(target.expected_leaf, ExpectedLeaf::Present(_))
        && matches!(before, WalLeafEvidence::Absent)
    {
        return Err(capability_error(
            &lexical.public_label,
            "target-ul copy baseline Present lipsește la planificare",
        ));
    }
    if let WalLeafEvidence::Regular { identity, .. } = &before {
        if identity == &source_evidence.identity {
            return Err(capability_error(
                &lexical.public_label,
                "source și destinația copy sunt același inode",
            ));
        }
    }

    let temp_leaf = atomic_temp_leaf(operation_id);
    if parent_exists && leaf_metadata(&directory, &temp_leaf, &lexical.public_label)?.is_some() {
        return Err(capability_error(
            &lexical.public_label,
            "numele temp determinist al copiei există deja",
        ));
    }

    Ok(CopyOperationPlan {
        evidence: WalCopyEvidence {
            protocol_version: WAL_COPY_PROTOCOL_VERSION,
            file: WalAtomicFileEvidence {
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
                replace: matches!(before, WalLeafEvidence::Regular { .. }),
                new_size: source_evidence.size,
                new_content_hash: source_evidence.content_hash.clone(),
                before,
            },
            new_mode_bits: source_evidence.mode_bits,
            destination_policy: match replace_policy {
                CapabilityReplacePolicy::CreateNew => WalCopyDestinationPolicy::CreateNew,
                CapabilityReplacePolicy::Replace => WalCopyDestinationPolicy::Replace,
            },
            before_mode_bits,
            source: source_evidence,
        },
        source_file,
    })
}

pub(in crate::kernel::write_authority::capability) fn copy_file_wal(
    target: &WriteTarget,
    source: &Path,
    replace_policy: CapabilityReplacePolicy,
    mut plan: CopyOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<CapabilityEffect, String> {
    let lexical = lexical_target(target, false)?;
    validate_copy_plan_shape(
        &lexical,
        &target.expected_leaf,
        source,
        replace_policy,
        &plan,
        guard.operation_id(),
    )?;
    validate_copy_source_descriptor(&plan.source_file, &plan.evidence.source)?;

    let parent = match capture_parent_from_wal_evidence(&lexical, &plan.evidence.file.parent) {
        Ok(parent) => parent,
        Err(error) => return error.into_operation_result(),
    };
    if parent.created_ancestors {
        return Err(capability_error(
            &lexical.public_label,
            "Copy v2 nu poate crea namespace părinte",
        ));
    }
    run_test_hook(CapabilityTestStage::AfterTargetParentCaptured);

    if let Err(error) = validate_copy_before_metadata(
        &parent.directory,
        &parent.leaf,
        &plan.evidence,
        &lexical.public_label,
    ) {
        return Err(error);
    }

    let temp_name = plan.temp_leaf()?;
    match leaf_metadata(&parent.directory, &temp_name, &lexical.public_label) {
        Ok(None) => {}
        Ok(Some(_)) => {
            return Err(capability_error(
                &lexical.public_label,
                "leaf-ul temporar rezervat Copy v2 există înainte de staging",
            ));
        }
        Err(error) => return Err(error),
    }

    let stage_role = copy_stage_role(&plan.evidence);
    let (mut staged_file, staged_stat, staged_identity, bytes_written) = match stage_copy_anonymous(
        &parent.directory,
        &mut plan,
        stage_role,
        &lexical.public_label,
    ) {
        Ok(staged) => staged,
        Err((bytes, error)) => {
            return Ok(wal_recovery_effect(bytes, &lexical.public_label, error));
        }
    };
    let checkpoint = match WalCopyStageCheckpoint::new(
        staged_identity.clone(),
        &plan.evidence.file.new_content_hash,
        plan.evidence.file.new_size,
        plan.evidence.new_mode_bits,
        stage_role,
    ) {
        Ok(checkpoint) => checkpoint,
        Err(error) => {
            return Ok(wal_recovery_effect(
                bytes_written,
                &lexical.public_label,
                error,
            ));
        }
    };
    if let Err(error) = guard.mark_copy_auxiliary_durable(checkpoint) {
        return Ok(wal_recovery_effect(
            bytes_written,
            &lexical.public_label,
            error,
        ));
    }
    run_test_hook(CapabilityTestStage::AfterCopyAnonymousStageCheckpoint);

    if let Err(error) = validate_copy_before_metadata(
        &parent.directory,
        &parent.leaf,
        &plan.evidence,
        &lexical.public_label,
    ) {
        return Ok(wal_recovery_effect(
            bytes_written,
            &lexical.public_label,
            error,
        ));
    }

    run_test_hook(CapabilityTestStage::BeforeAtomicCommit);
    let commit_result = match plan.evidence.destination_policy {
        WalCopyDestinationPolicy::CreateNew => commit_copy_create_v2(
            &lexical,
            &parent,
            &mut staged_file,
            &staged_stat,
            &staged_identity,
            &plan,
            guard,
        ),
        WalCopyDestinationPolicy::Replace => commit_copy_replace_v2(
            &lexical,
            &parent,
            &temp_name,
            &mut staged_file,
            &staged_stat,
            &staged_identity,
            &plan,
            guard,
        ),
    };
    if let Err(error) = commit_result {
        return Ok(wal_recovery_effect(
            bytes_written,
            &lexical.public_label,
            error,
        ));
    }

    Ok(CapabilityEffect::changed(bytes_written))
}

#[allow(clippy::too_many_arguments)]
fn commit_copy_replace_v2(
    lexical: &LexicalTarget,
    parent: &CapturedParent,
    temp_name: &OsStr,
    staged_file: &mut File,
    staged_stat: &fs::Stat,
    staged_identity: &str,
    plan: &CopyOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<(), String> {
    let bytes = plan.evidence.file.new_size;
    let linked_stat = publish_copy_anonymous(
        staged_file,
        staged_stat,
        staged_identity,
        &parent.directory,
        temp_name,
        plan,
        &lexical.public_label,
        "copy replace temp publication",
    )
    .map_err(|error| recovery_diagnostic(bytes, lexical, error))?;
    run_test_hook(CapabilityTestStage::AfterCopyTemporaryLinkBeforePhase);

    validate_copy_before_metadata(
        &parent.directory,
        &parent.leaf,
        &plan.evidence,
        &lexical.public_label,
    )
    .map_err(|error| recovery_diagnostic(bytes, lexical, error))?;
    // Preview cache este singurul authority unde overwrite-ul necondiționat
    // este contractul operației. Orice schimbare a targetului după acest
    // preflight este tot conținut rebuildable al aceluiași cache; aceeași
    // permisiune este refuzată de plan_copy pentru orice alt scope.
    run_test_hook(CapabilityTestStage::BeforeCopyPreviewOverwriteRename);
    fs::renameat(
        &parent.directory,
        temp_name,
        &parent.directory,
        &parent.leaf,
    )
    .map_err(|error| {
        recovery_diagnostic(
            bytes,
            lexical,
            format!("copy Preview rename atomic temp -> target a eșuat: {error}"),
        )
    })?;
    run_test_hook(CapabilityTestStage::AfterCopyRenameBeforePhase);
    guard
        .mark_effect_visible()
        .map_err(|error| recovery_diagnostic(bytes, lexical, error))?;

    let committed_stat = validate_committed_copy(
        lexical,
        parent,
        staged_file,
        &linked_stat,
        plan,
        "copy Preview target după rename",
    )
    .map_err(|error| recovery_diagnostic(bytes, lexical, error))?;
    finalize_copy_target_v2(lexical, parent, staged_file, &committed_stat, plan, guard)
}

fn commit_copy_create_v2(
    lexical: &LexicalTarget,
    parent: &CapturedParent,
    staged_file: &mut File,
    staged_stat: &fs::Stat,
    staged_identity: &str,
    plan: &CopyOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<(), String> {
    let bytes = plan.evidence.file.new_size;
    let linked_stat = publish_copy_anonymous(
        staged_file,
        staged_stat,
        staged_identity,
        &parent.directory,
        &parent.leaf,
        plan,
        &lexical.public_label,
        "copy create-only target publication",
    )
    .map_err(|error| recovery_diagnostic(bytes, lexical, error))?;
    run_test_hook(CapabilityTestStage::AfterCopyTargetLinkBeforePhase);
    guard
        .mark_effect_visible()
        .map_err(|error| recovery_diagnostic(bytes, lexical, error))?;
    let committed_stat = validate_committed_copy(
        lexical,
        parent,
        staged_file,
        &linked_stat,
        plan,
        "copy create-only target după link",
    )
    .map_err(|error| recovery_diagnostic(bytes, lexical, error))?;
    finalize_copy_target_v2(lexical, parent, staged_file, &committed_stat, plan, guard)
}

fn finalize_copy_target_v2(
    lexical: &LexicalTarget,
    parent: &CapturedParent,
    target_file: &mut File,
    target_stat: &fs::Stat,
    plan: &CopyOperationPlan,
    guard: &mut DurableWalGuard<'_>,
) -> Result<(), String> {
    let bytes = plan.evidence.file.new_size;
    target_file.sync_all().map_err(|error| {
        recovery_diagnostic(
            bytes,
            lexical,
            format!("fsync pe targetul Copy v2 a eșuat: {error}"),
        )
    })?;
    run_test_hook(CapabilityTestStage::AfterCopyTargetFsync);
    sync_directory(&parent.directory, &lexical.public_label)
        .map_err(|error| recovery_diagnostic(bytes, lexical, error))?;
    run_test_hook(CapabilityTestStage::BeforeCopyTargetDurable);
    validate_copy_runtime_postflight(lexical, parent, target_file, target_stat, plan)
        .map_err(|error| recovery_diagnostic(bytes, lexical, error))?;
    guard
        .mark_target_durable()
        .map_err(|error| recovery_diagnostic(bytes, lexical, error))?;
    run_test_hook(CapabilityTestStage::AfterCopyTargetDurable);
    validate_copy_runtime_postflight(lexical, parent, target_file, target_stat, plan)
        .map_err(|error| recovery_diagnostic(bytes, lexical, error))
}

fn copy_stage_role(evidence: &WalCopyEvidence) -> WalCopyStageRole {
    match evidence.destination_policy {
        WalCopyDestinationPolicy::CreateNew => WalCopyStageRole::CreateTarget,
        WalCopyDestinationPolicy::Replace => WalCopyStageRole::ReplaceTemporary,
    }
}

fn stage_copy_anonymous(
    parent: &OwnedFd,
    plan: &mut CopyOperationPlan,
    role: WalCopyStageRole,
    public_label: &str,
) -> Result<(File, fs::Stat, String, u64), (u64, String)> {
    let mut staged = open_anonymous_file(parent, Mode::from_raw_mode(plan.evidence.new_mode_bits))
        .map_err(|error| {
            (
                0,
                capability_error(
                    public_label,
                    &format!("Copy v2 O_TMPFILE anonim nu poate fi creat: {error}"),
                ),
            )
        })?;
    run_test_hook(CapabilityTestStage::BeforeCopyStream);
    let bytes_written = stream_copy_payload(
        &mut plan.source_file,
        &mut staged,
        &plan.evidence.source,
        public_label,
    )
    .map_err(|error| {
        let observed = fs::fstat(&staged)
            .ok()
            .and_then(|stat| u64::try_from(stat.st_size).ok())
            .unwrap_or(0);
        (observed, error)
    })?;
    fs::fchmod(&staged, Mode::from_raw_mode(plan.evidence.new_mode_bits)).map_err(|error| {
        (
            bytes_written,
            capability_error(
                public_label,
                &format!("Copy v2 O_TMPFILE fchmod a eșuat: {error}"),
            ),
        )
    })?;
    staged.sync_all().map_err(|error| {
        (
            bytes_written,
            capability_error(
                public_label,
                &format!("Copy v2 O_TMPFILE fsync a eșuat: {error}"),
            ),
        )
    })?;
    let staged_stat = validate_open_copy_staged_payload(
        &mut staged,
        &plan.evidence,
        public_label,
        "copy anonymous durable",
    )
    .map_err(|error| (bytes_written, error))?;
    let identity = copy_stage_identity_digest(&staged, role)
        .map_err(|error| (bytes_written, capability_error(public_label, &error)))?;
    Ok((staged, staged_stat, identity, bytes_written))
}

fn validate_open_copy_staged_payload(
    file: &mut File,
    evidence: &WalCopyEvidence,
    public_label: &str,
    stage: &str,
) -> Result<fs::Stat, String> {
    let before = fs::fstat(&*file).map_err(|error| {
        capability_error(public_label, &format!("{stage}: fstat a eșuat: {error}"))
    })?;
    if FileType::from_raw_mode(before.st_mode) != FileType::RegularFile
        || before.st_nlink != 0
        || u64::try_from(before.st_size).ok() != Some(evidence.file.new_size)
        || mode_bits(&before) != evidence.new_mode_bits
    {
        return Err(capability_error(
            public_label,
            &format!("{stage}: inode-ul anonim nu are tip/link/size/mode planificate"),
        ));
    }
    let hash = hash_open_file_exact(file, evidence.file.new_size, stage)?;
    let after = fs::fstat(&*file).map_err(|error| {
        capability_error(
            public_label,
            &format!("{stage}: post-hash fstat a eșuat: {error}"),
        )
    })?;
    if !same_file_identity(&before, &after)
        || version_token_for_stat(&before) != version_token_for_stat(&after)
        || after.st_nlink != 0
        || hash != evidence.file.new_content_hash
    {
        return Err(capability_error(
            public_label,
            &format!("{stage}: inode-ul anonim sau payloadul s-a schimbat în timpul validării"),
        ));
    }
    Ok(after)
}

fn copy_stage_identity_digest(
    descriptor: impl AsFd,
    role: WalCopyStageRole,
) -> Result<String, String> {
    let identity = causal_file_identity(descriptor).map_err(|error| match error {
        CausalFileIdentityError::Statx(error) => {
            format!("Copy v2 statx identity a eșuat: {error}")
        }
        CausalFileIdentityError::Incomplete => {
            "Copy v2 filesystem nu furnizează identitate statx lifetime completă".into()
        }
    })?;
    let mut digest = Sha256::new();
    digest.update(b"pana-copy-stage-identity-v2\0");
    digest.update(match role {
        WalCopyStageRole::CreateTarget => b"create-target".as_slice(),
        WalCopyStageRole::ReplaceTemporary => b"replace-temporary".as_slice(),
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

#[allow(clippy::too_many_arguments)]
fn publish_copy_anonymous(
    file: &mut File,
    anonymous_stat: &fs::Stat,
    expected_identity: &str,
    parent: &OwnedFd,
    leaf: &OsStr,
    plan: &CopyOperationPlan,
    public_label: &str,
    stage: &str,
) -> Result<fs::Stat, String> {
    if leaf_metadata(parent, leaf, public_label)?.is_some() {
        return Err(capability_error(
            public_label,
            &format!("{stage}: destinația create-only există deja"),
        ));
    }
    link_anonymous_file_create_only(file, parent, leaf, false).map_err(|error| match error {
        AnonymousFileLinkError::Primary(error) => capability_error(
            public_label,
            &format!("{stage}: linkat O_TMPFILE a eșuat: {error}"),
        ),
        AnonymousFileLinkError::Fallback {
            primary,
            proc_fd_path,
            fallback,
        } => capability_error(
            public_label,
            &format!(
                "{stage}: linkat AT_EMPTY_PATH a eșuat cu {primary}, iar fallback-ul exact {proc_fd_path} a eșuat: {fallback}"
            ),
        ),
    })?;
    let linked = fs::fstat(&*file).map_err(|error| {
        capability_error(
            public_label,
            &format!("{stage}: fstat după link a eșuat: {error}"),
        )
    })?;
    if !same_file_identity(anonymous_stat, &linked) || linked.st_nlink != 1 {
        return Err(capability_error(
            public_label,
            &format!("{stage}: numele nu indică exact inode-ul anonim staged"),
        ));
    }
    validate_named_file_identity(parent, leaf, &linked, stage)?;
    let validated = validate_open_copy_payload(file, &plan.evidence, public_label, stage)?;
    if !same_file_identity(&linked, &validated) {
        return Err(capability_error(
            public_label,
            &format!("{stage}: descriptorul s-a schimbat după publicare"),
        ));
    }
    let observed_identity = copy_stage_identity_digest(file, copy_stage_role(&plan.evidence))?;
    if observed_identity != expected_identity {
        return Err(capability_error(
            public_label,
            &format!("{stage}: inode-ul nu mai corespunde checkpointului Copy v2"),
        ));
    }
    validate_named_file_identity(parent, leaf, &validated, stage)?;
    Ok(validated)
}

fn validate_copy_runtime_postflight(
    lexical: &LexicalTarget,
    expected_parent: &CapturedParent,
    target_file: &mut File,
    target_stat: &fs::Stat,
    plan: &CopyOperationPlan,
) -> Result<(), String> {
    let authority = lexical.authority.as_ref().ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "copy postflight cere authority root sigilat",
        )
    })?;
    // Descriptorul sigilat rămâne autoritatea de execuție, dar succesul Copy
    // trebuie să confirme și că pathname-ul public încă numește exact acel
    // authority root. Altfel un root swap poate produce un commit valid numai
    // în arborele deplasat, în timp ce UI-ul vede un competitor la calea
    // publică.
    verify_directory_authority_path(authority)?;
    let (leaf, parents) = lexical
        .relative_components
        .split_last()
        .ok_or_else(|| capability_error(&lexical.public_label, "copy postflight cere leaf"))?;
    let mut directory = rustix::io::dup(authority.directory()).map_err(|error| {
        capability_error(
            &lexical.public_label,
            &format!("copy postflight nu poate duplica authority: {error}"),
        )
    })?;
    for component in parents {
        let next = open_directory_strict(&directory, component).map_err(|error| {
            capability_error(
                &lexical.public_label,
                &format!("copy postflight nu poate recaptura parentul: {error}"),
            )
        })?;
        validate_named_directory_identity(
            &directory,
            component,
            &next,
            &lexical.public_label,
            "copy postflight parent",
        )?;
        directory = next;
    }
    if wal_identity_from_fd(&directory, &lexical.public_label)?
        != wal_identity_from_fd(&expected_parent.directory, &lexical.public_label)?
    {
        return Err(capability_error(
            &lexical.public_label,
            "copy postflight path-ul nu mai numește parentul sincronizat",
        ));
    }
    validate_named_file_identity(&directory, leaf, target_stat, "copy-postflight-target")?;
    validate_open_copy_snapshot(
        target_file,
        target_stat,
        &plan.evidence,
        &lexical.public_label,
        "copy postflight payload",
    )?;
    validate_named_file_identity(&directory, leaf, target_stat, "copy-postflight-target")?;
    let temp_leaf = decode_component_hex(&plan.evidence.file.temp_leaf_hex)?;
    if leaf_metadata(&directory, &temp_leaf, &lexical.public_label)?.is_some() {
        return Err(capability_error(
            &lexical.public_label,
            "copy postflight vede încă un artefact temp neașteptat",
        ));
    }
    Ok(())
}

fn validate_committed_copy(
    lexical: &LexicalTarget,
    parent: &CapturedParent,
    target_file: &mut File,
    target_stat: &fs::Stat,
    plan: &CopyOperationPlan,
    stage: &str,
) -> Result<fs::Stat, String> {
    validate_named_file_identity(&parent.directory, &parent.leaf, target_stat, stage)?;
    let committed =
        validate_open_copy_payload(target_file, &plan.evidence, &lexical.public_label, stage)?;
    if !same_file_identity(target_stat, &committed) {
        return Err(capability_error(
            &lexical.public_label,
            &format!("{stage}: descriptorul target nu mai este temp-ul validat"),
        ));
    }
    validate_named_file_identity(&parent.directory, &parent.leaf, &committed, stage)?;
    Ok(committed)
}

fn open_copy_source(path: &Path) -> Result<File, String> {
    if !path.is_absolute() {
        return Err("Copy WAL blocat: calea sursei trebuie să fie absolută.".into());
    }
    let public_label = format!("Copy source:{}", path.display());
    let components = absolute_normal_components(path, &public_label, "source")?;
    let (leaf, parents) = components.split_last().ok_or_else(|| {
        capability_error(&public_label, "sursa trebuie să aibă un leaf sub rădăcină")
    })?;
    let mut directory = open_filesystem_root(&public_label)?;
    for component in parents {
        directory = open_directory_strict(&directory, component).map_err(|error| {
            capability_error(
                &public_label,
                &format!("un părinte al sursei nu a putut fi capturat: {error}"),
            )
        })?;
    }
    let descriptor = fs::openat(
        &directory,
        leaf,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| {
        capability_error(
            &public_label,
            &format!("leaf-ul sursei nu a putut fi deschis fără symlink: {error}"),
        )
    })?;
    let stat = fs::fstat(&descriptor).map_err(|error| {
        capability_error(
            &public_label,
            &format!("descriptorul sursei nu poate fi verificat: {error}"),
        )
    })?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
        return Err(capability_error(
            &public_label,
            "sursa nu este fișier regular",
        ));
    }
    Ok(File::from(descriptor))
}

fn capture_copy_source_evidence(
    source: &mut File,
    source_path: &Path,
) -> Result<WalCopySourceEvidence, String> {
    let before =
        fs::fstat(&*source).map_err(|error| format!("Copy WAL source fstat a eșuat: {error}."))?;
    if FileType::from_raw_mode(before.st_mode) != FileType::RegularFile {
        return Err("Copy WAL source nu mai este fișier regular.".into());
    }
    let size = u64::try_from(before.st_size)
        .map_err(|_| "Copy WAL source are dimensiune negativă.".to_string())?;
    if size > MAX_WAL_COPY_BYTES {
        return Err(format!(
            "Copy WAL source depășește limita de {MAX_WAL_COPY_BYTES} bytes."
        ));
    }
    let content_hash = hash_open_file_exact(source, size, "Copy WAL source plan")?;
    let after = fs::fstat(&*source)
        .map_err(|error| format!("Copy WAL source post-hash fstat a eșuat: {error}."))?;
    if version_token_for_stat(&before) != version_token_for_stat(&after)
        || before.st_nlink != after.st_nlink
    {
        return Err("Copy WAL source s-a schimbat în timpul planificării.".into());
    }
    source
        .seek(SeekFrom::Start(0))
        .map_err(|error| format!("Copy WAL source nu poate fi resetată: {error}."))?;
    Ok(WalCopySourceEvidence {
        path_hex: encode_path_hex(source_path),
        identity: WalFilesystemIdentity {
            device: before.st_dev,
            inode: before.st_ino,
        },
        size,
        version_token: version_token_for_stat(&before),
        content_hash,
        mode_bits: mode_bits(&before),
        link_count: before.st_nlink,
    })
}

fn validate_copy_source_descriptor(
    source: &File,
    evidence: &WalCopySourceEvidence,
) -> Result<(), String> {
    let stat = fs::fstat(source)
        .map_err(|error| format!("Copy WAL source nu poate fi reverificată: {error}."))?;
    if stat.st_dev != evidence.identity.device
        || stat.st_ino != evidence.identity.inode
        || u64::try_from(stat.st_size).ok() != Some(evidence.size)
        || version_token_for_stat(&stat) != evidence.version_token
        || mode_bits(&stat) != evidence.mode_bits
        || stat.st_nlink != evidence.link_count
    {
        return Err("Copy WAL source descriptor diferă de snapshotul planificat.".into());
    }
    Ok(())
}

fn stream_copy_payload(
    source: &mut File,
    target: &mut File,
    evidence: &WalCopySourceEvidence,
    public_label: &str,
) -> Result<u64, String> {
    validate_copy_source_descriptor(source, evidence)?;
    source.seek(SeekFrom::Start(0)).map_err(|error| {
        capability_error(
            public_label,
            &format!("source copy nu poate reveni la început: {error}"),
        )
    })?;
    let mut hasher = Sha256::new();
    let mut observed = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    {
        let mut bounded_source = (&mut *source).take(evidence.size.saturating_add(1));
        loop {
            let count = bounded_source.read(&mut buffer).map_err(|error| {
                capability_error(
                    public_label,
                    &format!("citirea source copy a eșuat: {error}"),
                )
            })?;
            if count == 0 {
                break;
            }
            observed = observed.saturating_add(count as u64);
            if observed > evidence.size {
                return Err(capability_error(
                    public_label,
                    "source copy a crescut în timpul transferului",
                ));
            }
            target.write_all(&buffer[..count]).map_err(|error| {
                capability_error(
                    public_label,
                    &format!("scrierea temp copy a eșuat: {error}"),
                )
            })?;
            hasher.update(&buffer[..count]);
        }
    }
    let observed_hash = format!("{:x}", hasher.finalize());
    validate_copy_source_descriptor(source, evidence)?;
    if observed != evidence.size || observed_hash != evidence.content_hash {
        return Err(capability_error(
            public_label,
            "source copy nu mai are size/hash-ul planificat",
        ));
    }
    Ok(observed)
}

fn validate_open_copy_payload(
    file: &mut File,
    evidence: &WalCopyEvidence,
    public_label: &str,
    stage: &str,
) -> Result<fs::Stat, String> {
    let before = fs::fstat(&*file).map_err(|error| {
        capability_error(public_label, &format!("{stage}: fstat a eșuat: {error}"))
    })?;
    if FileType::from_raw_mode(before.st_mode) != FileType::RegularFile
        || before.st_nlink != 1
        || u64::try_from(before.st_size).ok() != Some(evidence.file.new_size)
        || mode_bits(&before) != evidence.new_mode_bits
    {
        return Err(capability_error(
            public_label,
            &format!("{stage}: tip/link/size/mode diferă de plan"),
        ));
    }
    let hash = hash_open_file_exact(file, evidence.file.new_size, stage)?;
    let after = fs::fstat(&*file).map_err(|error| {
        capability_error(
            public_label,
            &format!("{stage}: post-hash fstat a eșuat: {error}"),
        )
    })?;
    if version_token_for_stat(&before) != version_token_for_stat(&after)
        || hash != evidence.file.new_content_hash
    {
        return Err(capability_error(
            public_label,
            &format!("{stage}: payloadul s-a schimbat în timpul verificării"),
        ));
    }
    Ok(after)
}

fn validate_open_copy_snapshot(
    file: &File,
    expected: &fs::Stat,
    evidence: &WalCopyEvidence,
    public_label: &str,
    stage: &str,
) -> Result<(), String> {
    let before = fs::fstat(file).map_err(|error| {
        capability_error(public_label, &format!("{stage}: fstat a eșuat: {error}"))
    })?;
    if !same_file_identity(expected, &before)
        || FileType::from_raw_mode(before.st_mode) != FileType::RegularFile
        || before.st_nlink != expected.st_nlink
        || u64::try_from(before.st_size).ok() != Some(evidence.file.new_size)
        || mode_bits(&before) != evidence.new_mode_bits
        || version_token_for_stat(&before) != version_token_for_stat(expected)
    {
        return Err(capability_error(
            public_label,
            &format!("{stage}: snapshotul target validat s-a schimbat"),
        ));
    }
    let after = fs::fstat(file).map_err(|error| {
        capability_error(public_label, &format!("{stage}: re-fstat a eșuat: {error}"))
    })?;
    if version_token_for_stat(&before) != version_token_for_stat(&after)
        || before.st_nlink != after.st_nlink
    {
        return Err(capability_error(
            public_label,
            &format!("{stage}: snapshotul target s-a schimbat în timpul verificării"),
        ));
    }
    Ok(())
}

fn hash_open_file_exact(file: &mut File, size: u64, role: &str) -> Result<String, String> {
    file.seek(SeekFrom::Start(0))
        .map_err(|error| format!("{role}: seek a eșuat: {error}."))?;
    let mut hasher = Sha256::new();
    let mut observed = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    {
        let mut bounded_file = (&mut *file).take(size.saturating_add(1));
        loop {
            let count = bounded_file
                .read(&mut buffer)
                .map_err(|error| format!("{role}: read a eșuat: {error}."))?;
            if count == 0 {
                break;
            }
            observed = observed.saturating_add(count as u64);
            if observed > size {
                return Err(format!("{role}: fișierul a crescut în timpul hash-ului."));
            }
            hasher.update(&buffer[..count]);
        }
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|error| format!("{role}: reset seek a eșuat: {error}."))?;
    if observed != size {
        return Err(format!(
            "{role}: dimensiunea citită {observed} diferă de {size}."
        ));
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn observe_copy_leaf(
    parent: &OwnedFd,
    leaf: &OsStr,
    public_label: &str,
) -> Result<Option<ObservedCopyLeaf>, String> {
    let Some(named) = leaf_metadata(parent, leaf, public_label)? else {
        return Ok(None);
    };
    if FileType::from_raw_mode(named.st_mode) != FileType::RegularFile || named.st_nlink != 1 {
        return Err(capability_error(
            public_label,
            "copy recovery leaf nu este fișier regular single-link",
        ));
    }
    let size = u64::try_from(named.st_size)
        .map_err(|_| capability_error(public_label, "copy recovery leaf are size negativ"))?;
    let descriptor = fs::openat(
        parent,
        leaf,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| capability_error(public_label, &format!("copy leaf open: {error}")))?;
    let file = File::from(descriptor);
    let before = fs::fstat(&file)
        .map_err(|error| capability_error(public_label, &format!("copy leaf fstat: {error}")))?;
    if !same_file_identity(&named, &before)
        || FileType::from_raw_mode(before.st_mode) != FileType::RegularFile
        || before.st_nlink != 1
    {
        return Err(capability_error(
            public_label,
            "copy recovery leaf s-a schimbat sau nu mai este regular single-link în timpul open",
        ));
    }
    validate_named_file_identity(parent, leaf, &before, "copy-recovery-leaf")?;
    let after = fs::fstat(&file)
        .map_err(|error| capability_error(public_label, &format!("copy leaf re-fstat: {error}")))?;
    if version_token_for_stat(&before) != version_token_for_stat(&after)
        || before.st_nlink != after.st_nlink
    {
        return Err(capability_error(
            public_label,
            "copy recovery leaf s-a schimbat în timpul capturii metadata-only",
        ));
    }
    validate_named_file_identity(parent, leaf, &after, "copy-recovery-leaf")?;
    Ok(Some(ObservedCopyLeaf {
        identity: WalFilesystemIdentity {
            device: after.st_dev,
            inode: after.st_ino,
        },
        size,
        version_token: version_token_for_stat(&after),
        mode_bits: mode_bits(&after),
    }))
}

fn capture_copy_before(
    parent: &OwnedFd,
    leaf: &OsStr,
    expected: &ExpectedLeaf,
    public_label: &str,
) -> Result<(WalLeafEvidence, Option<u32>), String> {
    let evidence = capture_wal_leaf_evidence(parent, leaf, expected, public_label, None)?;
    match &evidence {
        WalLeafEvidence::Absent => {
            if leaf_metadata(parent, leaf, public_label)?.is_some() {
                return Err(capability_error(
                    public_label,
                    "copy baseline absent a apărut în timpul capturii",
                ));
            }
            Ok((evidence, None))
        }
        WalLeafEvidence::Regular {
            identity,
            version_token,
            ..
        } => {
            let stat = leaf_metadata(parent, leaf, public_label)?.ok_or_else(|| {
                capability_error(public_label, "copy baseline a dispărut după captură")
            })?;
            if stat.st_dev != identity.device
                || stat.st_ino != identity.inode
                || version_token_for_stat(&stat) != *version_token
            {
                return Err(capability_error(
                    public_label,
                    "copy baseline s-a schimbat după captură",
                ));
            }
            Ok((evidence, Some(mode_bits(&stat))))
        }
    }
}

fn validate_copy_before_metadata(
    parent: &OwnedFd,
    leaf: &OsStr,
    evidence: &WalCopyEvidence,
    public_label: &str,
) -> Result<(), String> {
    let observed = observe_copy_leaf(parent, leaf, public_label)?;
    if observed_matches_copy_before(observed.as_ref(), evidence) {
        Ok(())
    } else {
        Err(capability_error(
            public_label,
            "baseline-ul target copy diferă de snapshotul WAL",
        ))
    }
}

fn expected_leaf_matches_copy_before(expected: &ExpectedLeaf, evidence: &WalCopyEvidence) -> bool {
    match (expected, &evidence.file.before) {
        (ExpectedLeaf::Unspecified, _) => true,
        (ExpectedLeaf::Absent, WalLeafEvidence::Absent) => true,
        (
            ExpectedLeaf::Present(expected),
            WalLeafEvidence::Regular {
                version_token,
                content_hash,
                ..
            },
        ) => {
            expected.version_token == *version_token
                && expected
                    .content_hash
                    .as_ref()
                    .is_none_or(|hash| hash == content_hash)
        }
        _ => false,
    }
}

fn validate_copy_destination(
    parent: &OwnedFd,
    leaf: &OsStr,
    replace_policy: CapabilityReplacePolicy,
    lexical: &LexicalTarget,
) -> Result<(), String> {
    let Some(metadata) = leaf_metadata(parent, leaf, &lexical.public_label)? else {
        return Ok(());
    };
    if replace_policy == CapabilityReplacePolicy::CreateNew {
        return Err(capability_error(
            &lexical.public_label,
            "destinația copy create-only există deja",
        ));
    }
    if FileType::from_raw_mode(metadata.st_mode) != FileType::RegularFile {
        return Err(capability_error(
            &lexical.public_label,
            "destinația copy nu este fișier regular sau este symlink",
        ));
    }
    if metadata.st_nlink != 1 {
        return Err(capability_error(
            &lexical.public_label,
            "destinația copy este hardlink sau are mai multe nume; este cerut un leaf single-link",
        ));
    }
    Ok(())
}

fn validate_copy_plan_shape(
    lexical: &LexicalTarget,
    expected_leaf: &ExpectedLeaf,
    source: &Path,
    replace_policy: CapabilityReplacePolicy,
    plan: &CopyOperationPlan,
    operation_id: &str,
) -> Result<(), String> {
    let (leaf, parents) = lexical
        .relative_components
        .split_last()
        .ok_or_else(|| capability_error(&lexical.public_label, "planul copy cere leaf"))?;
    let planned_parents = plan
        .evidence
        .file
        .parent
        .relative_components_hex
        .iter()
        .map(|component| decode_component_hex(component))
        .collect::<Result<Vec<_>, _>>()?;
    if planned_parents != parents
        || plan.evidence.protocol_version != WAL_COPY_PROTOCOL_VERSION
        || plan.evidence.file.parent.existing_prefix_len != parents.len()
        || plan.evidence.file.parent.parent_identity.is_none()
        || decode_component_hex(&plan.evidence.file.target_leaf_hex)? != *leaf
        || plan.temp_leaf()? != atomic_temp_leaf(operation_id)
        || plan.evidence.source.path_hex != encode_path_hex(source)
        || plan.evidence.file.new_size != plan.evidence.source.size
        || plan.evidence.file.new_content_hash != plan.evidence.source.content_hash
        || plan.evidence.new_mode_bits != plan.evidence.source.mode_bits
        || plan.evidence.destination_policy
            != match replace_policy {
                CapabilityReplacePolicy::CreateNew => WalCopyDestinationPolicy::CreateNew,
                CapabilityReplacePolicy::Replace => WalCopyDestinationPolicy::Replace,
            }
        || (replace_policy == CapabilityReplacePolicy::CreateNew && plan.evidence.file.replace)
        || !expected_leaf_matches_copy_before(expected_leaf, &plan.evidence)
    {
        return Err(capability_error(
            &lexical.public_label,
            "planul copy nu corespunde source/target/modului executat",
        ));
    }
    Ok(())
}

fn observed_matches_copy_before(
    observed: Option<&ObservedCopyLeaf>,
    evidence: &WalCopyEvidence,
) -> bool {
    match (&evidence.file.before, observed) {
        (WalLeafEvidence::Absent, None) => true,
        (
            WalLeafEvidence::Regular {
                identity,
                size,
                version_token,
                ..
            },
            Some(observed),
        ) => {
            observed.identity == *identity
                && observed.size == *size
                && observed.version_token == *version_token
                && Some(observed.mode_bits) == evidence.before_mode_bits
        }
        _ => false,
    }
}

fn observed_matches_relocated_copy_before(
    observed: Option<&ObservedCopyLeaf>,
    evidence: &WalCopyEvidence,
) -> bool {
    matches!(
        (&evidence.file.before, observed),
        (
            WalLeafEvidence::Regular {
                identity,
                size,
                ..
            },
            Some(observed),
        ) if observed.identity == *identity
            && observed.size == *size
            && Some(observed.mode_bits) == evidence.before_mode_bits
    )
}

fn observed_matches_copy_new_shape(
    observed: Option<&ObservedCopyLeaf>,
    evidence: &WalCopyEvidence,
) -> bool {
    matches!(
        observed,
        Some(ObservedCopyLeaf {
            size,
            mode_bits,
            ..
        }) if *size == evidence.file.new_size
            && *mode_bits == evidence.new_mode_bits
    )
}

fn mode_bits(stat: &fs::Stat) -> u32 {
    (stat.st_mode as u32) & 0o7777
}

fn recovery_diagnostic(
    _bytes_written: u64,
    lexical: &LexicalTarget,
    diagnostic: impl Into<String>,
) -> String {
    capability_error(
        &lexical.public_label,
        &format!(
            "{} Recordul copy WAL rămâne hot; nu repeta operația automat.",
            diagnostic.into()
        ),
    )
}
