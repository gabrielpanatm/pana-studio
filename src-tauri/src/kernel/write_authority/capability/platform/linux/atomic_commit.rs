use super::{
    capability_error, cleanup_temp_after_error, create_unique_temp, fs, hash_bytes, leaf_metadata,
    run_test_hook, same_file_identity, same_stable_leaf_version, sync_directory,
    validate_named_file_identity, version_token_for_stat, AtFlags, CapabilityEffect,
    CapabilityReplacePolicy, CapabilityTestStage, ExpectedLeaf, ExpectedLeafVersion, File,
    FileType, Mode, OFlags, OsStr, OwnedFd, RenameFlags, SeekFrom,
};
use std::io::{Read, Seek};

pub(super) fn atomic_commit<F>(
    parent: &OwnedFd,
    leaf: &OsStr,
    replace_policy: CapabilityReplacePolicy,
    expected_leaf: &ExpectedLeaf,
    public_label: &str,
    writer: F,
) -> Result<CapabilityEffect, String>
where
    F: FnOnce(&mut File) -> Result<u64, String>,
{
    let (temp_name, descriptor) = create_unique_temp(parent)?;
    let mut file = File::from(descriptor);
    let write_result = writer(&mut file).and_then(|bytes_written| {
        file.sync_all()
            .map_err(|error| format!("Fișierul temporar nu a putut fi sincronizat: {error}."))?;
        Ok(bytes_written)
    });

    let bytes_written = match write_result {
        Ok(bytes_written) => bytes_written,
        Err(error) => {
            let diagnostic = cleanup_temp_after_error(parent, &temp_name, error);
            drop(file);
            return Err(diagnostic);
        }
    };
    let temp_identity = fs::fstat(&file).map_err(|error| {
        cleanup_temp_after_error(
            parent,
            &temp_name,
            format!("Identitatea descriptorului temporar nu a putut fi citită: {error}."),
        )
    })?;

    run_test_hook(CapabilityTestStage::BeforeAtomicCommit);
    if let Err(error) =
        validate_named_file_identity(parent, &temp_name, &temp_identity, "atomic-temp")
    {
        let diagnostic = cleanup_temp_after_error(parent, &temp_name, error);
        drop(file);
        return Err(diagnostic);
    }
    if let ExpectedLeaf::Present(expected) = expected_leaf {
        return conditional_atomic_replace(
            parent,
            leaf,
            &temp_name,
            &mut file,
            &temp_identity,
            expected,
            bytes_written,
            public_label,
        );
    }
    if replace_policy == CapabilityReplacePolicy::Replace {
        match leaf_metadata(parent, leaf, public_label)? {
            Some(stat) if FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile => {
                return unconditional_atomic_replace(
                    parent,
                    leaf,
                    &temp_name,
                    &mut file,
                    &temp_identity,
                    bytes_written,
                    public_label,
                );
            }
            Some(_) => {
                return Err(cleanup_temp_after_error(
                    parent,
                    &temp_name,
                    capability_error(
                        public_label,
                        "target-ul Replace s-a schimbat într-un leaf non-regular înainte de commit",
                    ),
                ));
            }
            None => {}
        }
    }
    let commit_result = fs::renameat_with(parent, &temp_name, parent, leaf, RenameFlags::NOREPLACE);
    if let Err(error) = commit_result {
        let diagnostic = cleanup_temp_after_error(
            parent,
            &temp_name,
            format!("Commit-ul atomic fd-relative a eșuat: {error}."),
        );
        drop(file);
        return Err(diagnostic);
    }
    if let Err(error) = validate_named_file_identity(parent, leaf, &temp_identity, "atomic-leaf") {
        drop(file);
        return Ok(CapabilityEffect::recovery_required(
            bytes_written,
            format!(
                "Commit-ul atomic a avut loc, dar leaf-ul a fost înlocuit imediat după rename: {error} Nu repeta operația automat."
            ),
        ));
    }
    drop(file);
    match sync_directory(parent, "atomic-commit") {
        Ok(()) => Ok(CapabilityEffect::changed(bytes_written)),
        Err(error) => Ok(CapabilityEffect::recovery_required(
            bytes_written,
            format!(
                "Commit-ul atomic este vizibil, dar folderul nu a putut fi sincronizat: {error}. Nu repeta operația automat."
            ),
        )),
    }
}

pub(super) fn unconditional_atomic_replace(
    parent: &OwnedFd,
    leaf: &OsStr,
    temp_name: &OsStr,
    _temp_file: &mut File,
    temp_identity: &fs::Stat,
    bytes_written: u64,
    public_label: &str,
) -> Result<CapabilityEffect, String> {
    if let Err(error) = fs::renameat_with(parent, temp_name, parent, leaf, RenameFlags::EXCHANGE) {
        return Err(cleanup_temp_after_error(
            parent,
            temp_name,
            capability_error(
                public_label,
                &format!("atomic Replace exchange a eșuat fără commit: {error}"),
            ),
        ));
    }
    run_test_hook(CapabilityTestStage::AfterAtomicExchange);
    if let Err(error) =
        validate_named_file_identity(parent, leaf, temp_identity, "atomic-replace-leaf")
    {
        return Ok(CapabilityEffect::recovery_required(
            bytes_written,
            format!(
                "Atomic Replace a făcut exchange, dar target-ul nu mai este temp-ul sincronizat: {error} Leaf-ul anterior este păstrat în {}; nu repeta operația automat.",
                temp_name.to_string_lossy()
            ),
        ));
    }
    let previous = match fs::statat(parent, temp_name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => stat,
        Err(error) => {
            return Ok(CapabilityEffect::recovery_required(
                bytes_written,
                capability_error(
                    public_label,
                    &format!(
                        "atomic Replace este vizibil, dar leaf-ul anterior {} nu poate fi verificat: {error}; recovery necesar",
                        temp_name.to_string_lossy()
                    ),
                ),
            ));
        }
    };
    if FileType::from_raw_mode(previous.st_mode) != FileType::RegularFile {
        return Ok(CapabilityEffect::recovery_required(
            bytes_written,
            capability_error(
                public_label,
                &format!(
                    "atomic Replace este vizibil, dar leaf-ul anterior {} nu este regular și a fost păstrat pentru recovery",
                    temp_name.to_string_lossy()
                ),
            ),
        ));
    }
    if let Err(error) =
        validate_named_file_identity(parent, temp_name, &previous, "atomic-replace-old")
    {
        return Ok(CapabilityEffect::recovery_required(
            bytes_written,
            format!(
                "{error} Atomic Replace păstrează leaf-ul anterior pentru recovery; nu repeta operația automat."
            ),
        ));
    }
    if let Err(error) = fs::unlinkat(parent, temp_name, AtFlags::empty()) {
        return Ok(CapabilityEffect::recovery_required(
            bytes_written,
            capability_error(
                public_label,
                &format!(
                    "atomic Replace este vizibil, dar cleanup-ul {} a eșuat: {error}; recovery necesar",
                    temp_name.to_string_lossy()
                ),
            ),
        ));
    }
    match sync_directory(parent, public_label) {
        Ok(()) => Ok(CapabilityEffect::changed(bytes_written)),
        Err(error) => Ok(CapabilityEffect::recovery_required(
            bytes_written,
            format!(
                "{error} Atomic Replace este vizibil, dar durabilitatea este incertă; nu repeta operația automat."
            ),
        )),
    }
}

// The CAS boundary exposes each captured identity and cleanup target for auditability.
#[allow(clippy::too_many_arguments)]
pub(super) fn conditional_atomic_replace(
    parent: &OwnedFd,
    leaf: &OsStr,
    temp_name: &OsStr,
    _temp_file: &mut File,
    temp_identity: &fs::Stat,
    expected: &ExpectedLeafVersion,
    bytes_written: u64,
    public_label: &str,
) -> Result<CapabilityEffect, String> {
    let descriptor = fs::openat(
        parent,
        leaf,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| {
        cleanup_temp_after_error(
            parent,
            temp_name,
            capability_error(
                public_label,
                &format!("leaf-CAS replace nu a putut captura versiunea așteptată: {error}"),
            ),
        )
    })?;
    let mut previous_file = File::from(descriptor);
    let previous_before = fs::fstat(&previous_file).map_err(|error| {
        cleanup_temp_after_error(
            parent,
            temp_name,
            capability_error(
                public_label,
                &format!("leaf-CAS replace nu a putut citi metadata: {error}"),
            ),
        )
    })?;
    validate_expected_regular_file(
        &mut previous_file,
        &previous_before,
        expected,
        public_label,
        "replace pre-commit",
    )
    .map_err(|error| cleanup_temp_after_error(parent, temp_name, error))?;

    run_test_hook(CapabilityTestStage::AfterExpectedLeafCaptured);
    if let Err(error) =
        validate_named_file_identity(parent, temp_name, temp_identity, "atomic-temp-cas")
    {
        return Err(cleanup_temp_after_error(parent, temp_name, error));
    }

    if let Err(error) = fs::renameat_with(parent, temp_name, parent, leaf, RenameFlags::EXCHANGE) {
        return Err(cleanup_temp_after_error(
            parent,
            temp_name,
            capability_error(
                public_label,
                &format!("leaf-CAS exchange a eșuat fără commit: {error}"),
            ),
        ));
    }
    run_test_hook(CapabilityTestStage::AfterAtomicExchange);

    let moved_previous = match fs::statat(parent, temp_name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => stat,
        Err(error) => {
            return Ok(CapabilityEffect::recovery_required(
                bytes_written,
                capability_error(
                    public_label,
                    &format!(
                        "leaf-ul vechi mutat sub {} nu poate fi verificat: {error}; recovery necesar și fără retry automat",
                        temp_name.to_string_lossy()
                    ),
                ),
            ));
        }
    };
    let validation = (|| {
        if FileType::from_raw_mode(moved_previous.st_mode) != FileType::RegularFile
            || !same_file_identity(&previous_before, &moved_previous)
        {
            return Err(capability_error(
                public_label,
                "leaf-ul de la commit nu este inode-ul capturat de disk baseline",
            ));
        }
        let previous_after = fs::fstat(&previous_file).map_err(|error| {
            capability_error(
                public_label,
                &format!("versiunea veche izolată nu mai poate fi verificată: {error}"),
            )
        })?;
        if !same_stable_leaf_version(&previous_before, &previous_after) {
            return Err(capability_error(
                public_label,
                "leaf-ul vechi s-a modificat în timpul commit-ului condițional",
            ));
        }
        validate_expected_content(
            &mut previous_file,
            &previous_after,
            expected.content_hash.as_deref(),
            public_label,
            "replace post-exchange",
        )?;
        let previous_final = fs::fstat(&previous_file).map_err(|error| {
            capability_error(
                public_label,
                &format!("versiunea veche nu poate fi reverificată după hash: {error}"),
            )
        })?;
        if version_token_for_stat(&previous_after) != version_token_for_stat(&previous_final) {
            return Err(capability_error(
                public_label,
                "leaf-ul vechi a suferit o schimbare ABA în timpul postflight-ului replace",
            ));
        }
        Ok(())
    })();

    if let Err(conflict) = validation {
        return rollback_atomic_exchange(
            parent,
            leaf,
            temp_name,
            temp_identity,
            &moved_previous,
            bytes_written,
            public_label,
            conflict,
        );
    }

    if let Err(error) =
        validate_named_file_identity(parent, leaf, temp_identity, "cas-committed-leaf")
    {
        return Ok(CapabilityEffect::recovery_required(
            bytes_written,
            format!(
                "Commit-ul leaf-CAS a făcut exchange, dar target-ul nu mai este inode-ul temporar sincronizat: {error} Versiunea veche este păstrată în {}; nu repeta operația automat.",
                temp_name.to_string_lossy()
            ),
        ));
    }

    if let Err(error) =
        validate_named_file_identity(parent, temp_name, &previous_before, "cas-old-leaf")
    {
        return Ok(CapabilityEffect::recovery_required(
            bytes_written,
            format!(
                "{error} Noul conținut este deja la target, dar versiunea veche izolată cere recovery; nu repeta operația automat."
            ),
        ));
    }
    if let Err(error) = fs::unlinkat(parent, temp_name, AtFlags::empty()) {
        return Ok(CapabilityEffect::recovery_required(
            bytes_written,
            capability_error(
                public_label,
                &format!(
                    "commit-ul leaf-CAS este vizibil, dar versiunea veche izolată în {} nu a putut fi eliminată: {error}; nu repeta operația automat",
                    temp_name.to_string_lossy()
                ),
            ),
        ));
    }
    match sync_directory(parent, public_label) {
        Ok(()) => Ok(CapabilityEffect::changed(bytes_written)),
        Err(error) => Ok(CapabilityEffect::recovery_required(
            bytes_written,
            format!(
                "{error} Commit-ul leaf-CAS este vizibil, dar durabilitatea directorului este incertă; nu repeta operația automat."
            ),
        )),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn rollback_atomic_exchange(
    parent: &OwnedFd,
    leaf: &OsStr,
    temp_name: &OsStr,
    temp_identity: &fs::Stat,
    restored_identity: &fs::Stat,
    bytes_written: u64,
    public_label: &str,
    conflict: String,
) -> Result<CapabilityEffect, String> {
    if let Err(error) = fs::renameat_with(parent, temp_name, parent, leaf, RenameFlags::EXCHANGE) {
        return Ok(CapabilityEffect::recovery_required(
            bytes_written,
            capability_error(
                public_label,
                &format!(
                    "{conflict} Rollback-ul exchange a eșuat: {error}. Target-ul și {} cer recovery; nu repeta operația automat",
                    temp_name.to_string_lossy()
                ),
            ),
        ));
    }

    let restored = fs::statat(parent, leaf, AtFlags::SYMLINK_NOFOLLOW);
    let temp = fs::statat(parent, temp_name, AtFlags::SYMLINK_NOFOLLOW);
    if !matches!(restored, Ok(ref stat) if same_file_identity(stat, restored_identity))
        || !matches!(temp, Ok(ref stat) if same_file_identity(stat, temp_identity))
    {
        return Ok(CapabilityEffect::recovery_required(
            bytes_written,
            capability_error(
                public_label,
                &format!(
                    "{conflict} Rollback-ul exchange nu a putut demonstra restaurarea identităților; {} cere recovery și operația nu trebuie repetată automat",
                    temp_name.to_string_lossy()
                ),
            ),
        ));
    }
    if let Err(error) = fs::unlinkat(parent, temp_name, AtFlags::empty()) {
        return Ok(CapabilityEffect::recovery_required(
            bytes_written,
            capability_error(
                public_label,
                &format!(
                    "{conflict} Leaf-ul anterior a fost restaurat, dar temp-ul rollback {} nu a putut fi eliminat: {error}; recovery necesar",
                    temp_name.to_string_lossy()
                ),
            ),
        ));
    }
    if let Err(error) = sync_directory(parent, public_label) {
        return Ok(CapabilityEffect::recovery_required(
            bytes_written,
            format!(
                "{conflict} Leaf-ul anterior a fost restaurat, dar rollback-ul nu este confirmat durabil: {error} Nu repeta operația automat."
            ),
        ));
    }
    Err(format!(
        "{conflict} Operația leaf-CAS a fost anulată, iar versiunea concurentă a fost restaurată."
    ))
}

pub(super) fn validate_expected_regular_file(
    file: &mut File,
    stat: &fs::Stat,
    expected: &ExpectedLeafVersion,
    public_label: &str,
    stage: &str,
) -> Result<(), String> {
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
        return Err(capability_error(
            public_label,
            &format!("{stage}: expected leaf nu este fișier regular"),
        ));
    }
    let observed_token = version_token_for_stat(stat);
    if observed_token != expected.version_token {
        return Err(capability_error(
            public_label,
            &format!(
                "{stage}: disk baseline s-a schimbat înainte de commit (expected {}, observed {})",
                expected.version_token, observed_token
            ),
        ));
    }
    validate_expected_content(
        file,
        stat,
        expected.content_hash.as_deref(),
        public_label,
        stage,
    )
}

pub(super) fn validate_expected_content(
    file: &mut File,
    stat: &fs::Stat,
    expected_hash: Option<&str>,
    public_label: &str,
    stage: &str,
) -> Result<(), String> {
    let Some(expected_hash) = expected_hash else {
        return Ok(());
    };
    let expected_size = u64::try_from(stat.st_size)
        .map_err(|_| capability_error(public_label, &format!("{stage}: dimensiune negativă")))?;
    const MAX_CONDITIONAL_HASH_BYTES: u64 = 512 * 1024 * 1024;
    if expected_size > MAX_CONDITIONAL_HASH_BYTES {
        return Err(capability_error(
            public_label,
            &format!(
                "{stage}: verificarea hash depășește limita de {MAX_CONDITIONAL_HASH_BYTES} bytes"
            ),
        ));
    }
    file.seek(SeekFrom::Start(0)).map_err(|error| {
        capability_error(
            public_label,
            &format!("{stage}: descriptorul nu poate reveni la început: {error}"),
        )
    })?;
    let mut bytes = Vec::with_capacity(expected_size as usize);
    (&mut *file)
        .take(expected_size.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| {
            capability_error(
                public_label,
                &format!("{stage}: conținutul nu poate fi citit bounded: {error}"),
            )
        })?;
    file.seek(SeekFrom::Start(0)).map_err(|error| {
        capability_error(
            public_label,
            &format!("{stage}: descriptorul nu poate fi resetat: {error}"),
        )
    })?;
    let observed_hash = hash_bytes(&bytes);
    if bytes.len() as u64 != expected_size || observed_hash != expected_hash {
        return Err(capability_error(
            public_label,
            &format!(
                "{stage}: conținutul disk s-a schimbat (expected hash {expected_hash}, observed {observed_hash})"
            ),
        ));
    }
    Ok(())
}
