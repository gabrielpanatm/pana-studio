use super::super::anonymous_file::{
    causal_file_identity, link_anonymous_file_create_only, open_anonymous_file,
    AnonymousFileLinkError, CausalFileIdentityError,
};
use super::*;

#[derive(Clone, Copy)]
pub(super) struct ExternalLeaves<'a> {
    pub(super) target: &'a OsStr,
    pub(super) target_temp: &'a OsStr,
    pub(super) backup: Option<&'a OsStr>,
    pub(super) backup_temp: Option<&'a OsStr>,
}

pub(super) struct OwnedExternalLeaves {
    pub(super) target: OsString,
    pub(super) target_temp: OsString,
    pub(super) backup: Option<OsString>,
    pub(super) backup_temp: Option<OsString>,
}

impl OwnedExternalLeaves {
    pub(super) fn as_borrowed(&self) -> ExternalLeaves<'_> {
        ExternalLeaves {
            target: &self.target,
            target_temp: &self.target_temp,
            backup: self.backup.as_deref(),
            backup_temp: self.backup_temp.as_deref(),
        }
    }
}

pub(super) fn validate_external_plan_shape(
    lexical: &LexicalTarget,
    backup_lexical: Option<&LexicalTarget>,
    bytes: &[u8],
    previous_bytes: Option<&[u8]>,
    plan: &ExternalConfigOperationPlan,
    operation_id: &str,
) -> Result<(), String> {
    validate_external_payload_size(bytes, &lexical.public_label, "target")?;
    let (target_leaf, target_parents) =
        lexical.relative_components.split_last().ok_or_else(|| {
            capability_error(&lexical.public_label, "ExternalConfig cere target leaf")
        })?;
    let planned_parents = plan
        .evidence
        .target
        .parent
        .relative_components_hex
        .iter()
        .map(|component| decode_component_hex(component))
        .collect::<Result<Vec<_>, _>>()?;
    if planned_parents != target_parents
        || decode_component_hex(&plan.evidence.target.target_leaf_hex)? != *target_leaf
        || decode_component_hex(&plan.evidence.target.temp_leaf_hex)?
            != external_config_target_temp_leaf(operation_id)
        || plan.evidence.target.new_size != bytes.len() as u64
        || plan.evidence.target.new_content_hash != sha256_bytes(bytes)
    {
        return Err(capability_error(
            &lexical.public_label,
            "planul ExternalConfig nu corespunde targetului/payloadului executat",
        ));
    }
    match (
        backup_lexical,
        previous_bytes,
        &plan.evidence.backup,
        plan.evidence.target.replace,
    ) {
        (None, None, None, false)
            if plan.evidence.target_before_mode_bits.is_none()
                && plan.evidence.target_before_identity_digest.is_none()
                && plan.evidence.backup_mode_bits.is_none()
                && plan.evidence.target_new_mode_bits == 0o600 => {}
        (Some(backup_lexical), Some(previous), Some(backup), true) => {
            validate_external_payload_size(previous, &lexical.public_label, "backup")?;
            let (backup_leaf, backup_parents) = backup_lexical
                .relative_components
                .split_last()
                .ok_or_else(|| capability_error(&lexical.public_label, "backup-ul cere leaf"))?;
            if backup_parents != target_parents
                || backup.parent != plan.evidence.target.parent
                || decode_component_hex(&backup.target_leaf_hex)? != *backup_leaf
                || decode_component_hex(&backup.temp_leaf_hex)?
                    != external_config_backup_temp_leaf(operation_id)
                || backup.new_size != previous.len() as u64
                || backup.new_content_hash != sha256_bytes(previous)
                || plan.evidence.target_before_mode_bits != plan.evidence.backup_mode_bits
                || plan.evidence.target_before_mode_bits != Some(plan.evidence.target_new_mode_bits)
                || !plan
                    .evidence
                    .target_before_identity_digest
                    .as_deref()
                    .is_some_and(is_external_identity_digest)
            {
                return Err(capability_error(
                    &lexical.public_label,
                    "planul ExternalConfig backup/mode nu corespunde executiei",
                ));
            }
        }
        _ => {
            return Err(capability_error(
                &lexical.public_label,
                "planul ExternalConfig are forma target/backup invalida",
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_runtime_before(
    parent: &CapturedParent,
    plan: &mut ExternalConfigOperationPlan,
    leaves: ExternalLeaves<'_>,
    public_label: &str,
    backup_must_be_absent: bool,
) -> Result<(), String> {
    if plan.evidence.target.replace {
        let file = plan.existing_target.as_mut().ok_or_else(|| {
            capability_error(
                public_label,
                "planul replace nu mai detine target descriptor",
            )
        })?;
        validate_open_before_payload(
            file,
            &plan.evidence.target.before,
            plan.evidence.target_before_mode_bits.ok_or_else(|| {
                capability_error(public_label, "planul replace nu are mode baseline")
            })?,
            &parent.directory,
            leaves.target,
            public_label,
            "target baseline runtime",
            true,
        )?;
    } else {
        validate_leaf_absent_for_external(
            &parent.directory,
            leaves.target,
            public_label,
            "target baseline create-new",
        )?;
    }
    if backup_must_be_absent {
        if let Some(backup_leaf) = leaves.backup {
            validate_leaf_absent_for_external(
                &parent.directory,
                backup_leaf,
                public_label,
                "backup baseline runtime",
            )?;
        }
    }
    Ok(())
}

pub(super) fn recapture_external_public_parent(
    lexical: &LexicalTarget,
    evidence: &WalExternalConfigEvidence,
    held_parent: &OwnedFd,
) -> Result<CapturedParent, String> {
    let recaptured = capture_parent_from_wal_evidence(lexical, &evidence.target.parent)
        .map_err(CaptureFailure::into_diagnostic)?;
    if recaptured.created_ancestors {
        return Err(capability_error(
            &lexical.public_label,
            "ExternalConfig full-path CAS a creat neașteptat namespace părinte",
        ));
    }
    let expected_leaf = decode_component_hex(&evidence.target.target_leaf_hex)?;
    if recaptured.leaf != expected_leaf {
        return Err(capability_error(
            &lexical.public_label,
            "ExternalConfig full-path CAS a recapturat alt leaf public",
        ));
    }
    let held_identity = wal_identity_from_fd(held_parent, &lexical.public_label)?;
    let recaptured_identity = wal_identity_from_fd(&recaptured.directory, &lexical.public_label)?;
    if recaptured_identity != held_identity {
        return Err(capability_error(
            &lexical.public_label,
            "ExternalConfig full-path CAS a recapturat alt parent decât descriptorul autoritar",
        ));
    }
    Ok(recaptured)
}

pub(super) fn stage_external_anonymous(
    parent: &OwnedFd,
    bytes: &[u8],
    mode_bits: u32,
    public_label: &str,
    role: &str,
) -> Result<(File, fs::Stat, String), String> {
    let mut file =
        open_anonymous_file(parent, Mode::from_raw_mode(mode_bits)).map_err(|error| {
            capability_error(
                public_label,
                &format!("{role} anonim O_TMPFILE nu poate fi creat: {error}"),
            )
        })?;
    file.write_all(bytes).map_err(|error| {
        capability_error(
            public_label,
            &format!("{role} anonim write a esuat: {error}"),
        )
    })?;
    fs::fchmod(&file, Mode::from_raw_mode(mode_bits)).map_err(|error| {
        capability_error(
            public_label,
            &format!("{role} anonim fchmod a esuat: {error}"),
        )
    })?;
    file.sync_all().map_err(|error| {
        capability_error(
            public_label,
            &format!("{role} anonim fsync a esuat: {error}"),
        )
    })?;
    let stat = fs::fstat(&file).map_err(|error| {
        capability_error(
            public_label,
            &format!("{role} anonim stat a esuat: {error}"),
        )
    })?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_nlink != 0
        || u64::try_from(stat.st_size).ok() != Some(bytes.len() as u64)
        || external_mode_bits(&stat) != mode_bits
    {
        return Err(capability_error(
            public_label,
            &format!("{role} anonim nu are tip/link/size/mode planificate"),
        ));
    }
    let observed = wal_evidence_from_open_file(
        &mut file,
        &stat,
        &ExpectedLeaf::Unspecified,
        public_label,
        role,
        None,
    )?;
    if !leaf_matches_payload(&observed, bytes.len() as u64, &sha256_bytes(bytes)) {
        return Err(capability_error(
            public_label,
            &format!("{role} anonim nu are payloadul planificat"),
        ));
    }
    let identity_digest = external_stage_identity_digest(&file, role)?;
    Ok((file, stat, identity_digest))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn publish_external_anonymous(
    file: &mut File,
    anonymous_stat: &fs::Stat,
    parent: &OwnedFd,
    leaf: &OsStr,
    evidence: &WalAtomicFileEvidence,
    mode_bits: u32,
    identity_digest: &str,
    public_label: &str,
    identity_role: &str,
    stage: &str,
) -> Result<fs::Stat, String> {
    validate_leaf_absent_for_external(parent, leaf, public_label, stage)?;
    link_external_anonymous_create_only(file, parent, leaf, public_label, stage)?;
    let linked_stat = fs::fstat(&*file).map_err(|error| {
        capability_error(
            public_label,
            &format!("{stage} stat dupa link a esuat: {error}"),
        )
    })?;
    if !same_file_identity(anonymous_stat, &linked_stat) || linked_stat.st_nlink != 1 {
        return Err(capability_error(
            public_label,
            &format!("{stage} nu a publicat exact inode-ul anonim"),
        ));
    }
    validate_open_new_payload(
        file,
        &linked_stat,
        evidence,
        mode_bits,
        parent,
        leaf,
        public_label,
        stage,
    )?;
    let observed_digest = external_stage_identity_digest(&*file, identity_role)?;
    if observed_digest != identity_digest {
        return Err(capability_error(
            public_label,
            &format!("{stage} nu mai corespunde checkpointului de identitate"),
        ));
    }
    Ok(linked_stat)
}

fn link_external_anonymous_create_only(
    file: &File,
    parent: &OwnedFd,
    leaf: &OsStr,
    public_label: &str,
    stage: &str,
) -> Result<(), String> {
    if fail_external_linkat() {
        return Err(capability_error(
            public_label,
            &format!("{stage} linkat failure injectat înainte de publicare"),
        ));
    }
    link_anonymous_file_create_only(
        file,
        parent,
        leaf,
        force_external_linkat_proc_fallback(),
    )
    .map_err(|error| match error {
        AnonymousFileLinkError::Primary(error) => capability_error(
            public_label,
            &format!(
                "{stage} linkat O_TMPFILE AT_EMPTY_PATH a esuat fara fallback permis: {error}"
            ),
        ),
        AnonymousFileLinkError::Fallback {
            primary,
            proc_fd_path,
            fallback,
        } => capability_error(
            public_label,
            &format!(
                "{stage} linkat O_TMPFILE AT_EMPTY_PATH a esuat cu {primary}, iar fallback-ul exact {proc_fd_path} a esuat: {fallback}"
            ),
        ),
    })
}

pub(super) fn external_stage_identity_digest(
    descriptor: impl AsFd,
    role: &str,
) -> Result<String, String> {
    external_identity_digest(descriptor, role, role)
}

pub(super) fn external_baseline_identity_digest(descriptor: impl AsFd) -> Result<String, String> {
    external_identity_digest(descriptor, "baseline", "baseline")
}

fn external_identity_digest(
    descriptor: impl AsFd,
    domain: &str,
    diagnostic_role: &str,
) -> Result<String, String> {
    let identity = causal_file_identity(descriptor).map_err(|error| match error {
        CausalFileIdentityError::Statx(error) => {
            format!("ExternalConfig statx identity pentru {diagnostic_role} a esuat: {error}.")
        }
        CausalFileIdentityError::Incomplete => format!(
            "ExternalConfig filesystem nu furnizeaza identitate cauzala statx completa pentru {diagnostic_role}."
        ),
    })?;
    let mut digest = Sha256::new();
    digest.update(b"pana-external-stage-identity-v1\0");
    digest.update(domain.as_bytes());
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

fn is_external_identity_digest(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_open_new_payload(
    file: &mut File,
    expected_stat: &fs::Stat,
    evidence: &WalAtomicFileEvidence,
    mode_bits: u32,
    parent: &OwnedFd,
    named_leaf: &OsStr,
    public_label: &str,
    stage: &str,
) -> Result<(), String> {
    let before = fs::fstat(&*file).map_err(|error| {
        capability_error(public_label, &format!("{stage} stat a esuat: {error}"))
    })?;
    let observed_size = u64::try_from(before.st_size)
        .map_err(|_| capability_error(public_label, &format!("{stage} are dimensiune negativă")))?;
    // Fail-fast înainte de hashing: recovery rezervă dimensiunea planificată,
    // deci un competitor supradimensionat nu trebuie citit sub acel buget.
    if observed_size != evidence.new_size
        || !same_file_identity(expected_stat, &before)
        || FileType::from_raw_mode(before.st_mode) != FileType::RegularFile
        || before.st_nlink != 1
        || external_mode_bits(&before) != mode_bits
    {
        return Err(capability_error(
            public_label,
            &format!("{stage} nu mai este inode regular single-link cu mode exact"),
        ));
    }
    let observed = wal_evidence_from_open_file(
        file,
        &before,
        &ExpectedLeaf::Unspecified,
        public_label,
        stage,
        None,
    )?;
    if !leaf_matches_payload(&observed, evidence.new_size, &evidence.new_content_hash) {
        return Err(capability_error(
            public_label,
            &format!("{stage} nu mai are payloadul planificat"),
        ));
    }
    let after = fs::fstat(&*file).map_err(|error| {
        capability_error(
            public_label,
            &format!("{stage} final stat a esuat: {error}"),
        )
    })?;
    if !same_stable_leaf_version(&before, &after) || external_mode_bits(&after) != mode_bits {
        return Err(capability_error(
            public_label,
            &format!("{stage} s-a schimbat in timpul validarii"),
        ));
    }
    validate_named_file_identity(parent, named_leaf, &after, stage)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_open_before_payload(
    file: &mut File,
    expected: &WalLeafEvidence,
    mode_bits: u32,
    parent: &OwnedFd,
    named_leaf: &OsStr,
    public_label: &str,
    stage: &str,
    require_version_token: bool,
) -> Result<(), String> {
    let before = fs::fstat(&*file).map_err(|error| {
        capability_error(public_label, &format!("{stage} stat a esuat: {error}"))
    })?;
    let expected_size = match expected {
        WalLeafEvidence::Regular { size, .. } => *size,
        WalLeafEvidence::Absent => {
            return Err(capability_error(
                public_label,
                &format!("{stage} nu poate valida un baseline absent prin descriptor"),
            ));
        }
    };
    let observed_size = u64::try_from(before.st_size)
        .map_err(|_| capability_error(public_label, &format!("{stage} are dimensiune negativă")))?;
    // Ca și pentru payloadul nou, dimensiunea este verificată înainte ca
    // helperul generic să calculeze hash-ul conținutului.
    if observed_size != expected_size
        || FileType::from_raw_mode(before.st_mode) != FileType::RegularFile
        || before.st_nlink != 1
        || external_mode_bits(&before) != mode_bits
    {
        return Err(capability_error(
            public_label,
            &format!("{stage} nu mai este regular single-link cu mode exact"),
        ));
    }
    let observed = wal_evidence_from_open_file(
        file,
        &before,
        &ExpectedLeaf::Unspecified,
        public_label,
        stage,
        None,
    )?;
    let matches = if require_version_token {
        &observed == expected
    } else {
        leaf_matches_relocated_before(&observed, expected)
    };
    if !matches {
        return Err(capability_error(
            public_label,
            &format!("{stage} difera de baseline-ul WAL"),
        ));
    }
    let after = fs::fstat(&*file).map_err(|error| {
        capability_error(
            public_label,
            &format!("{stage} final stat a esuat: {error}"),
        )
    })?;
    if !same_stable_leaf_version(&before, &after) || external_mode_bits(&after) != mode_bits {
        return Err(capability_error(
            public_label,
            &format!("{stage} s-a schimbat in timpul validarii"),
        ));
    }
    validate_named_file_identity(parent, named_leaf, &after, stage)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_open_external_baseline(
    file: &mut File,
    expected: &WalLeafEvidence,
    expected_mode: u32,
    expected_identity_digest: &str,
    parent: &OwnedFd,
    named_leaf: &OsStr,
    public_label: &str,
    stage: &str,
    require_version_token: bool,
) -> Result<(), String> {
    validate_open_before_payload(
        file,
        expected,
        expected_mode,
        parent,
        named_leaf,
        public_label,
        stage,
        require_version_token,
    )?;
    let observed_identity = external_baseline_identity_digest(&*file)?;
    if observed_identity != expected_identity_digest {
        return Err(capability_error(
            public_label,
            &format!("{stage} nu corespunde checkpointului cauzal al baseline-ului"),
        ));
    }
    Ok(())
}

pub(super) fn validate_external_payload_size(
    bytes: &[u8],
    public_label: &str,
    role: &str,
) -> Result<(), String> {
    if bytes.len() as u64 > MAX_WAL_EXTERNAL_CONFIG_BYTES {
        return Err(capability_error(
            public_label,
            &format!("payloadul {role} depaseste limita de {MAX_WAL_EXTERNAL_CONFIG_BYTES} bytes"),
        ));
    }
    Ok(())
}

pub(super) fn validate_leaf_absent_for_external(
    parent: &OwnedFd,
    leaf: &OsStr,
    public_label: &str,
    role: &str,
) -> Result<(), String> {
    if leaf_metadata(parent, leaf, public_label)?.is_some() {
        return Err(capability_error(
            public_label,
            &format!("{role} nu mai este absent"),
        ));
    }
    Ok(())
}

pub(super) fn leaf_matches_payload(evidence: &WalLeafEvidence, size: u64, hash: &str) -> bool {
    matches!(
        evidence,
        WalLeafEvidence::Regular {
            size: observed_size,
            content_hash,
            ..
        } if *observed_size == size && content_hash == hash
    )
}

pub(super) fn external_mode_bits(stat: &fs::Stat) -> u32 {
    stat.st_mode & 0o7777
}

pub(super) fn owned_external_leaves(
    evidence: &WalExternalConfigEvidence,
) -> Result<OwnedExternalLeaves, String> {
    Ok(OwnedExternalLeaves {
        target: decode_component_hex(&evidence.target.target_leaf_hex)?,
        target_temp: decode_component_hex(&evidence.target.temp_leaf_hex)?,
        backup: evidence
            .backup
            .as_ref()
            .map(|backup| decode_component_hex(&backup.target_leaf_hex))
            .transpose()?,
        backup_temp: evidence
            .backup
            .as_ref()
            .map(|backup| decode_component_hex(&backup.temp_leaf_hex))
            .transpose()?,
    })
}

pub(super) fn validate_external_leaf_distinctness(
    leaves: ExternalLeaves<'_>,
    public_label: &str,
) -> Result<(), String> {
    let mut values = vec![leaves.target, leaves.target_temp];
    if let Some(backup) = leaves.backup {
        values.push(backup);
    }
    if let Some(backup_temp) = leaves.backup_temp {
        values.push(backup_temp);
    }
    for left in 0..values.len() {
        for right in (left + 1)..values.len() {
            if values[left] == values[right] {
                return Err(capability_error(
                    public_label,
                    "leaf-urile ExternalConfig se suprapun",
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn external_auxiliary_leaves(leaves: ExternalLeaves<'_>) -> Vec<(&OsStr, &'static str)> {
    let mut result = vec![(leaves.target_temp, "target temp")];
    if let Some(backup_temp) = leaves.backup_temp {
        result.push((backup_temp, "backup temp"));
    }
    result
}
