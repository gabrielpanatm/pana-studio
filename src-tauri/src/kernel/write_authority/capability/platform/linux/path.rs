#[cfg(test)]
use super::hooks::TEST_FAIL_DIRECTORY_SYNC;
use super::{
    fs, identity_from_fd, run_test_hook, tree_fingerprint_from_records,
    verify_directory_authority_path, AtFlags, CapabilityEffect, CapabilityReplacePolicy,
    CapabilityTestStage, CaptureFailure, CapturedBoundary, CapturedParent, Component, Dir, Errno,
    FileType, LexicalTarget, Mode, OFlags, Ordering, OsStr, OsString, OwnedFd, Path, ResolveFlags,
    TreeFingerprintRecord, WriteTarget, DIRECTORY_MODE, FILE_MODE, MAX_OPENAT2_RACE_RETRIES,
    MAX_REMOVE_TREE_DEPTH, MAX_REMOVE_TREE_ENTRIES, TEMP_FILE_SEQUENCE,
};
use std::os::unix::ffi::OsStringExt;

pub(super) fn version_token_for_stat(stat: &fs::Stat) -> String {
    format!(
        "unix:{}:{}:{}:{}:{}:{}:{}:{}",
        stat.st_dev,
        stat.st_ino,
        stat.st_size,
        stat.st_mtime,
        stat.st_mtime_nsec,
        stat.st_ctime,
        stat.st_ctime_nsec,
        stat.st_mode,
    )
}

pub(super) fn same_stable_leaf_version(before: &fs::Stat, after: &fs::Stat) -> bool {
    same_file_identity(before, after)
        && before.st_size == after.st_size
        && before.st_mtime == after.st_mtime
        && before.st_mtime_nsec == after.st_mtime_nsec
        && before.st_mode == after.st_mode
        && before.st_nlink == after.st_nlink
}

pub(super) fn create_unique_temp(parent: &OwnedFd) -> Result<(OsString, OwnedFd), String> {
    for _ in 0..32 {
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let name = OsString::from(format!(
            ".pana-capability-{}-{sequence}.tmp",
            std::process::id()
        ));
        match fs::openat(
            parent,
            &name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            FILE_MODE,
        ) {
            Ok(descriptor) => return Ok((name, descriptor)),
            Err(Errno::EXIST) => continue,
            Err(error) => {
                return Err(format!(
                    "Fișierul temporar fd-relative nu a putut fi creat: {error}."
                ));
            }
        }
    }
    Err("Nu a putut fi rezervat un nume temporar fd-relative unic.".to_string())
}

pub(super) fn cleanup_temp_after_error(
    parent: &OwnedFd,
    temp_name: &OsStr,
    original: String,
) -> String {
    match fs::unlinkat(parent, temp_name, AtFlags::empty()) {
        Ok(()) | Err(Errno::NOENT) => original,
        Err(cleanup_error) => format!(
            "{original} Curățarea fișierului temporar a eșuat fail-closed: {cleanup_error}."
        ),
    }
}

pub(super) fn validate_named_file_identity(
    parent: &OwnedFd,
    name: &OsStr,
    expected: &fs::Stat,
    role: &str,
) -> Result<(), String> {
    let observed = fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|error| format!("{role} nu mai poate fi verificat: {error}."))?;
    if FileType::from_raw_mode(observed.st_mode) != FileType::RegularFile
        || !same_file_identity(expected, &observed)
    {
        return Err(format!(
            "{role} nu mai numește inode-ul temporar sincronizat."
        ));
    }
    Ok(())
}

pub(super) fn validate_open_directory_identity(
    directory: &OwnedFd,
    expected: &fs::Stat,
    public_label: &str,
    role: &str,
) -> Result<(), String> {
    let observed = fs::fstat(directory).map_err(|error| {
        capability_error(
            public_label,
            &format!("{role} nu și-a putut citi identitatea: {error}"),
        )
    })?;
    if !same_file_identity(expected, &observed) {
        return Err(capability_error(
            public_label,
            &format!("{role} a fost înlocuit concurent înainte de capturare"),
        ));
    }
    Ok(())
}

pub(super) fn validate_named_directory_identity(
    parent: &OwnedFd,
    name: &OsStr,
    captured: &OwnedFd,
    public_label: &str,
    role: &str,
) -> Result<(), String> {
    let named = fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
        capability_error(
            public_label,
            &format!("{role} nu mai poate fi verificat înainte de rmdir: {error}"),
        )
    })?;
    let opened = fs::fstat(captured).map_err(|error| {
        capability_error(
            public_label,
            &format!("{role} capturat nu mai poate fi verificat: {error}"),
        )
    })?;
    if FileType::from_raw_mode(named.st_mode) != FileType::Directory
        || !same_file_identity(&named, &opened)
    {
        return Err(capability_error(
            public_label,
            &format!("{role} a fost înlocuit concurent înainte de rmdir"),
        ));
    }
    Ok(())
}

pub(super) fn same_file_identity(left: &fs::Stat, right: &fs::Stat) -> bool {
    left.st_dev == right.st_dev && left.st_ino == right.st_ino
}

pub(super) fn validate_atomic_destination(
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
            "target-ul create-only există deja",
        ));
    }
    match FileType::from_raw_mode(metadata.st_mode) {
        FileType::Symlink => Err(capability_error(
            &lexical.public_label,
            "target-ul atomic este symlink",
        )),
        FileType::Directory => Err(capability_error(
            &lexical.public_label,
            "target-ul atomic este director",
        )),
        FileType::RegularFile => Ok(()),
        _ => Err(capability_error(
            &lexical.public_label,
            "target-ul atomic nu este fișier regular",
        )),
    }
}

pub(super) fn validate_regular_single_link(
    descriptor: &OwnedFd,
    public_label: &str,
    operation: &str,
) -> Result<(), String> {
    let metadata = fs::fstat(descriptor).map_err(|error| {
        capability_error(
            public_label,
            &format!("{operation} nu a putut verifica descriptorul: {error}"),
        )
    })?;
    if FileType::from_raw_mode(metadata.st_mode) != FileType::RegularFile {
        return Err(capability_error(
            public_label,
            &format!("{operation} cere un fișier regular"),
        ));
    }
    if metadata.st_nlink > 1 {
        return Err(capability_error(
            public_label,
            &format!("{operation} refuză un inode cu mai multe hardlink-uri"),
        ));
    }
    Ok(())
}

pub(super) fn leaf_metadata(
    parent: &OwnedFd,
    leaf: &OsStr,
    public_label: &str,
) -> Result<Option<fs::Stat>, String> {
    match fs::statat(parent, leaf, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(Errno::NOENT) => Ok(None),
        Err(error) => Err(capability_error(
            public_label,
            &format!("leaf-ul nu a putut fi verificat fd-relative: {error}"),
        )),
    }
}

pub(super) fn capture_target_parent(
    lexical: &LexicalTarget,
    create_missing: bool,
) -> Result<Option<CapturedParent>, CaptureFailure> {
    let Some(boundary) = capture_boundary(lexical, create_missing)? else {
        return Ok(None);
    };
    capture_target_parent_from_directory(
        lexical,
        boundary.directory,
        create_missing,
        boundary.created,
    )
}

pub(super) fn capture_existing_target_parent(
    lexical: &LexicalTarget,
) -> Result<Option<CapturedParent>, String> {
    capture_target_parent(lexical, false).map_err(CaptureFailure::into_diagnostic)
}

pub(super) fn capture_existing_target_parent_from_directory(
    lexical: &LexicalTarget,
    directory: OwnedFd,
) -> Result<Option<CapturedParent>, String> {
    capture_target_parent_from_directory(lexical, directory, false, false)
        .map_err(CaptureFailure::into_diagnostic)
}

pub(super) fn capture_existing_boundary(
    lexical: &LexicalTarget,
) -> Result<Option<CapturedBoundary>, String> {
    capture_boundary(lexical, false).map_err(CaptureFailure::into_diagnostic)
}

pub(super) fn settle_after_implicit_parent_creation(
    created_ancestors: bool,
    result: Result<CapabilityEffect, String>,
    public_label: &str,
) -> Result<CapabilityEffect, String> {
    if !created_ancestors {
        return result;
    }
    match result {
        Ok(effect) if effect.recovery_required => Ok(effect),
        Ok(effect) if effect.changed => Ok(effect),
        Ok(effect) => Ok(CapabilityEffect::recovery_required(
            effect.bytes_written,
            capability_error(
                public_label,
                "operația leaf a fost Noop, dar namespace-ul părinte a fost creat",
            ),
        )),
        Err(error) => Ok(CapabilityEffect::recovery_required(
            0,
            format!(
                "{error} Namespace-ul părinte a fost creat durabil înaintea refuzului; nu repeta operația automat."
            ),
        )),
    }
}

pub(super) fn capture_target_parent_from_directory(
    lexical: &LexicalTarget,
    mut directory: OwnedFd,
    create_missing: bool,
    mut created_ancestors: bool,
) -> Result<Option<CapturedParent>, CaptureFailure> {
    let (leaf, parents) = lexical.relative_components.split_last().ok_or_else(|| {
        CaptureFailure::no_effect(capability_error(
            &lexical.public_label,
            "operația cere un leaf sub boundary",
        ))
    })?;

    for component in parents {
        match open_directory_strict(&directory, component) {
            Ok(next) => directory = next,
            Err(Errno::NOENT) if create_missing => {
                match open_or_create_directory_component(
                    &directory,
                    component,
                    &lexical.public_label,
                ) {
                    Ok((next, created)) => {
                        created_ancestors |= created;
                        directory = next;
                    }
                    Err(error) => {
                        return Err(if created_ancestors {
                            error.promote()
                        } else {
                            error
                        });
                    }
                }
            }
            Err(Errno::NOENT) => return Ok(None),
            Err(error) => {
                let diagnostic = capability_error(
                    &lexical.public_label,
                    &format!("un părinte nu a putut fi capturat fără symlink: {error}"),
                );
                return Err(if created_ancestors {
                    CaptureFailure::after_effect(diagnostic)
                } else {
                    CaptureFailure::no_effect(diagnostic)
                });
            }
        }
    }

    Ok(Some(CapturedParent {
        directory,
        leaf: leaf.clone(),
        created_ancestors,
    }))
}

pub(super) fn capture_boundary(
    lexical: &LexicalTarget,
    create_missing: bool,
) -> Result<Option<CapturedBoundary>, CaptureFailure> {
    if let Some(authority) = lexical.authority.as_ref() {
        if create_missing && !authority.root_path().exists() {
            return Err(CaptureFailure::no_effect(capability_error(
                &lexical.public_label,
                "authority root ținut nu mai are pathname; root-ul nu poate fi recreat implicit",
            )));
        }
        verify_directory_authority_path(authority).map_err(CaptureFailure::no_effect)?;
        run_test_hook(CapabilityTestStage::AfterAuthorityPathVerified);
        let directory = rustix::io::dup(authority.directory()).map_err(|error| {
            CaptureFailure::no_effect(capability_error(
                &lexical.public_label,
                &format!("authority handle nu a putut fi duplicat: {error}"),
            ))
        })?;
        let observed = identity_from_fd(&directory, &lexical.public_label)
            .map_err(CaptureFailure::no_effect)?;
        if observed != authority.identity() {
            return Err(CaptureFailure::no_effect(capability_error(
                &lexical.public_label,
                "authority handle nu mai corespunde identității instalate",
            )));
        }
        return Ok(Some(CapturedBoundary {
            directory,
            created: false,
        }));
    }
    capture_boundary_from_path(lexical, create_missing)
}

pub(super) fn capture_boundary_from_path(
    lexical: &LexicalTarget,
    create_missing: bool,
) -> Result<Option<CapturedBoundary>, CaptureFailure> {
    let mut current =
        open_filesystem_root(&lexical.public_label).map_err(CaptureFailure::no_effect)?;
    let mut created = false;

    for component in &lexical.boundary_components {
        match open_directory_strict(&current, component) {
            Ok(next) => current = next,
            Err(Errno::NOENT) if create_missing => {
                match open_or_create_directory_component(&current, component, &lexical.public_label)
                {
                    Ok((next, component_created)) => {
                        created |= component_created;
                        current = next;
                    }
                    Err(error) => {
                        return Err(if created { error.promote() } else { error });
                    }
                }
            }
            Err(Errno::NOENT) => return Ok(None),
            Err(error) => {
                let diagnostic = capability_error(
                    &lexical.public_label,
                    &format!("boundary-ul nu a putut fi capturat fără symlink: {error}"),
                );
                return Err(if created {
                    CaptureFailure::after_effect(diagnostic)
                } else {
                    CaptureFailure::no_effect(diagnostic)
                });
            }
        }
    }

    Ok(Some(CapturedBoundary {
        directory: current,
        created,
    }))
}

pub(super) fn open_filesystem_root(public_label: &str) -> Result<OwnedFd, String> {
    fs::openat(
        fs::CWD,
        Path::new("/"),
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| {
        capability_error(
            public_label,
            &format!("rădăcina filesystem nu a putut fi capturată: {error}"),
        )
    })
}

pub(super) fn open_or_create_directory_component(
    parent: &OwnedFd,
    component: &OsStr,
    public_label: &str,
) -> Result<(OwnedFd, bool), CaptureFailure> {
    match open_directory_strict(parent, component) {
        Ok(directory) => return Ok((directory, false)),
        Err(Errno::NOENT) => {}
        Err(error) => {
            return Err(CaptureFailure::no_effect(capability_error(
                public_label,
                &format!("directorul existent nu poate fi deschis sigur: {error}"),
            )));
        }
    }

    let created = match fs::mkdirat(parent, component, DIRECTORY_MODE) {
        Ok(()) => true,
        Err(Errno::EXIST) => false,
        Err(error) => {
            return Err(CaptureFailure::no_effect(capability_error(
                public_label,
                &format!("directorul nu a putut fi creat fd-relative: {error}"),
            )));
        }
    };
    let directory = open_directory_strict(parent, component).map_err(|error| {
        let diagnostic = capability_error(
            public_label,
            &format!("directorul creat nu a putut fi recapturat sigur: {error}"),
        );
        if created {
            CaptureFailure::after_effect(diagnostic)
        } else {
            CaptureFailure::no_effect(diagnostic)
        }
    })?;
    if created {
        sync_directory(parent, public_label).map_err(CaptureFailure::after_effect)?;
    }
    Ok((directory, created))
}

pub(super) fn open_directory_strict(parent: &OwnedFd, component: &OsStr) -> Result<OwnedFd, Errno> {
    let open_flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let resolve_flags =
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS;

    for _ in 0..MAX_OPENAT2_RACE_RETRIES {
        match fs::openat2(parent, component, open_flags, Mode::empty(), resolve_flags) {
            Ok(directory) => return Ok(directory),
            Err(Errno::AGAIN) => continue,
            // The fallback remains fd-relative and receives exactly one
            // validated normal component. It never falls back to a raw
            // absolute or boundary-relative pathname. NO_XDEV is omitted
            // intentionally because Pană Studio permits mounted project
            // and output directories.
            Err(Errno::NOSYS) => {
                return fs::openat(parent, component, open_flags, Mode::empty());
            }
            Err(error) => return Err(error),
        }
    }
    Err(Errno::AGAIN)
}

pub(super) fn fingerprint_directory_tree(
    directory: &OwnedFd,
    public_label: &str,
) -> Result<String, String> {
    let mut budget = 0_usize;
    let mut records = Vec::new();
    collect_directory_fingerprint_records(
        directory,
        "",
        0,
        &mut budget,
        &mut records,
        public_label,
    )?;
    Ok(tree_fingerprint_from_records(records))
}

pub(super) fn collect_directory_fingerprint_records(
    directory: &OwnedFd,
    relative_prefix: &str,
    depth: usize,
    budget: &mut usize,
    records: &mut Vec<TreeFingerprintRecord>,
    public_label: &str,
) -> Result<(), String> {
    if depth > MAX_REMOVE_TREE_DEPTH {
        return Err(capability_error(
            public_label,
            &format!(
                "fingerprint-ul directorului depășește adâncimea {}",
                MAX_REMOVE_TREE_DEPTH
            ),
        ));
    }
    let mut stream = Dir::read_from(directory).map_err(|error| {
        capability_error(
            public_label,
            &format!("directorul nu poate fi enumerat pentru fingerprint: {error}"),
        )
    })?;
    let mut names = Vec::new();
    while let Some(entry) = stream.read() {
        let entry = entry.map_err(|error| {
            capability_error(
                public_label,
                &format!("enumerarea fingerprint a eșuat: {error}"),
            )
        })?;
        let bytes = entry.file_name().to_bytes();
        if bytes == b"." || bytes == b".." {
            continue;
        }
        let name = std::str::from_utf8(bytes).map_err(|_| {
            capability_error(
                public_label,
                "fingerprint-ul lifecycle refuză nume descendant non-UTF-8",
            )
        })?;
        *budget = budget.saturating_add(1);
        if *budget > MAX_REMOVE_TREE_ENTRIES {
            return Err(capability_error(
                public_label,
                &format!(
                    "fingerprint-ul directorului depășește {} intrări",
                    MAX_REMOVE_TREE_ENTRIES
                ),
            ));
        }
        names.push((name.to_string(), OsString::from_vec(bytes.to_vec())));
    }
    drop(stream);
    names.sort_by(|left, right| left.0.cmp(&right.0));

    for (name, os_name) in names {
        let stat = fs::statat(directory, &os_name, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
            capability_error(
                public_label,
                &format!("un descendent nu poate fi verificat pentru fingerprint: {error}"),
            )
        })?;
        let relative_path = if relative_prefix.is_empty() {
            name
        } else {
            format!("{relative_prefix}/{name}")
        };
        let file_type = FileType::from_raw_mode(stat.st_mode);
        let kind = match file_type {
            FileType::Directory => b'd',
            FileType::RegularFile => b'f',
            FileType::Symlink => b'l',
            _ => b'o',
        };
        records.push(TreeFingerprintRecord {
            relative_path: relative_path.clone(),
            kind,
            version_token: version_token_for_stat(&stat),
        });
        if file_type == FileType::Directory {
            let child = open_directory_strict(directory, &os_name).map_err(|error| {
                capability_error(
                    public_label,
                    &format!("un descendent director nu poate fi capturat: {error}"),
                )
            })?;
            validate_open_directory_identity(
                &child,
                &stat,
                public_label,
                "tree fingerprint child",
            )?;
            collect_directory_fingerprint_records(
                &child,
                &relative_path,
                depth + 1,
                budget,
                records,
                public_label,
            )?;
        }
    }
    Ok(())
}

pub(super) fn sync_directory(directory: &OwnedFd, public_label: &str) -> Result<(), String> {
    #[cfg(test)]
    if TEST_FAIL_DIRECTORY_SYNC.with(std::cell::Cell::get) {
        return Err(capability_error(
            public_label,
            "failure injection: fsync director refuzat după efect",
        ));
    }
    fs::fsync(directory).map_err(|error| {
        capability_error(
            public_label,
            &format!("directorul capturat nu a putut fi sincronizat: {error}"),
        )
    })
}

pub(super) fn lexical_target(
    target: &WriteTarget,
    allow_boundary_root: bool,
) -> Result<LexicalTarget, String> {
    if target.path.as_os_str().is_empty() || target.boundary_root.as_os_str().is_empty() {
        return Err(capability_error(
            &target.public_label,
            "target-ul și boundary-ul trebuie să fie ne-goale",
        ));
    }
    if !target.path.is_absolute() || !target.boundary_root.is_absolute() {
        return Err(capability_error(
            &target.public_label,
            "target-ul și boundary-ul trebuie să fie absolute",
        ));
    }

    let boundary_components =
        absolute_normal_components(&target.boundary_root, &target.public_label, "boundary")?;
    let relative = target
        .path
        .strip_prefix(&target.boundary_root)
        .map_err(|_| {
            capability_error(
                &target.public_label,
                "target-ul nu este descendent lexical al boundary-ului",
            )
        })?;
    let relative_components = relative_normal_components(relative, &target.public_label)?;
    if relative_components.is_empty() && !allow_boundary_root {
        return Err(capability_error(
            &target.public_label,
            "operația nu poate folosi boundary root drept leaf",
        ));
    }

    Ok(LexicalTarget {
        boundary_components,
        relative_components,
        public_label: target.public_label.clone(),
        authority: target.authority().cloned(),
    })
}

pub(super) fn absolute_normal_components(
    path: &Path,
    public_label: &str,
    role: &str,
) -> Result<Vec<OsString>, String> {
    let mut saw_root = false;
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::RootDir if !saw_root => saw_root = true,
            Component::Normal(value) if saw_root => components.push(value.to_os_string()),
            _ => {
                return Err(capability_error(
                    public_label,
                    &format!("{role}-ul conține componente relative sau non-canonice"),
                ));
            }
        }
    }
    if !saw_root {
        return Err(capability_error(
            public_label,
            &format!("{role}-ul nu are rădăcină absolută"),
        ));
    }
    Ok(components)
}

pub(super) fn relative_normal_components(
    path: &Path,
    public_label: &str,
) -> Result<Vec<OsString>, String> {
    path.components()
        .map(|component| match component {
            Component::Normal(value) => Ok(value.to_os_string()),
            _ => Err(capability_error(
                public_label,
                "target-ul relativ conține traversal sau componente non-canonice",
            )),
        })
        .collect()
}

pub(super) fn capability_error(public_label: &str, reason: &str) -> String {
    format!("Capability filesystem a blocat {public_label}: {reason}.")
}
