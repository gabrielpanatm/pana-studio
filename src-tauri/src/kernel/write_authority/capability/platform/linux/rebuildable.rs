use super::{
    capability_error, fs, lexical_target, open_directory_strict, relative_normal_components,
    same_file_identity, sync_directory, validate_named_file_identity, validate_regular_single_link,
    verify_directory_authority_path, version_token_for_stat, AtFlags, BTreeSet,
    CapabilityGenerationCloneStats, DirectoryAuthority, DirectoryAuthorityScope, Errno, File,
    FileType, Mode, OFlags, OsStr, OsString, OwnedFd, Path, PathBuf, SeekFrom, WalkDir,
    WriteTarget, MAX_REMOVE_TREE_DEPTH,
};
use std::io::{Seek, Write};

pub(super) fn require_rebuildable_generation_authority(
    authority: &DirectoryAuthority,
    public_label: &str,
) -> Result<(), String> {
    if !matches!(
        authority.scope(),
        DirectoryAuthorityScope::ApplicationPreviewCache
    ) {
        return Err(capability_error(
            public_label,
            "calea rapidă rebuildable este permisă numai sub authority ApplicationPreviewCache",
        ));
    }
    Ok(())
}

/// Creates one private, create-only generation leaf under a sealed Preview
/// cache authority. The caller must publish or discard it explicitly.
pub(in crate::kernel::write_authority::capability) fn create_private_rebuildable_directory(
    authority: &DirectoryAuthority,
    path: &Path,
    public_label: &str,
) -> Result<(), String> {
    require_rebuildable_generation_authority(authority, public_label)?;
    verify_directory_authority_path(authority)?;
    let target = WriteTarget::new(path, authority.root_path(), public_label)
        .bind_authority(authority.clone())?;
    let lexical = lexical_target(&target, false)?;
    if lexical.relative_components.len() != 1 {
        return Err(capability_error(
            public_label,
            "generația privată trebuie să fie un singur leaf sub authority",
        ));
    }
    fs::mkdirat(
        authority.directory(),
        &lexical.relative_components[0],
        Mode::from_raw_mode(0o700),
    )
    .map_err(|error| {
        capability_error(
            public_label,
            &format!("generația privată create-only nu a putut fi creată: {error}"),
        )
    })
}

pub(super) fn open_or_create_rebuildable_generation_directory(
    parent: &OwnedFd,
    component: &OsStr,
    public_label: &str,
) -> Result<OwnedFd, String> {
    match open_directory_strict(parent, component) {
        Ok(directory) => return Ok(directory),
        Err(Errno::NOENT) => {}
        Err(error) => {
            return Err(capability_error(
                public_label,
                &format!("ancestor-ul generației rebuildable nu poate fi deschis sigur: {error}"),
            ));
        }
    }
    match fs::mkdirat(parent, component, Mode::from_raw_mode(0o700)) {
        Ok(()) | Err(Errno::EXIST) => {}
        Err(error) => {
            return Err(capability_error(
                public_label,
                &format!("directorul generației rebuildable nu a putut fi creat: {error}"),
            ));
        }
    }
    open_directory_strict(parent, component).map_err(|error| {
        capability_error(
            public_label,
            &format!("directorul generației rebuildable nu a putut fi recapturat sigur: {error}"),
        )
    })
}

pub(super) fn rebuildable_generation_components(
    authority: &DirectoryAuthority,
    relative_path: &Path,
    public_label: &str,
) -> Result<Vec<OsString>, String> {
    require_rebuildable_generation_authority(authority, public_label)?;
    let components = relative_normal_components(relative_path, public_label)?;
    if components.is_empty() {
        return Err(capability_error(
            public_label,
            "descendentul generației rebuildable nu poate fi rădăcina authority",
        ));
    }
    if components.len() > MAX_REMOVE_TREE_DEPTH {
        return Err(capability_error(
            public_label,
            &format!(
                "descendentul generației rebuildable depășește adâncimea {}",
                MAX_REMOVE_TREE_DEPTH
            ),
        ));
    }
    Ok(components)
}

/// Materializes a directory inside an unpublished rebuildable generation.
/// Intermediate namespace changes deliberately skip WAL/fsync because no
/// public name can observe the incomplete tree.
pub(in crate::kernel::write_authority::capability) fn create_rebuildable_generation_directory(
    authority: &DirectoryAuthority,
    relative_path: &Path,
    public_label: &str,
) -> Result<(), String> {
    let components = rebuildable_generation_components(authority, relative_path, public_label)?;
    let mut directory = rustix::io::dup(authority.directory()).map_err(|error| {
        capability_error(
            public_label,
            &format!("authority-ul generației nu a putut fi duplicat: {error}"),
        )
    })?;
    for component in components {
        directory =
            open_or_create_rebuildable_generation_directory(&directory, &component, public_label)?;
    }
    Ok(())
}

/// Writes one create-only regular file inside an unpublished rebuildable
/// generation. Publication seals the complete namespace once.
pub(in crate::kernel::write_authority::capability) fn write_rebuildable_generation_file(
    authority: &DirectoryAuthority,
    relative_path: &Path,
    bytes: &[u8],
    public_label: &str,
) -> Result<(), String> {
    let components = rebuildable_generation_components(authority, relative_path, public_label)?;
    let (leaf, parents) = components
        .split_last()
        .expect("validated generation path must contain a leaf");
    let mut directory = rustix::io::dup(authority.directory()).map_err(|error| {
        capability_error(
            public_label,
            &format!("authority-ul generației nu a putut fi duplicat: {error}"),
        )
    })?;
    for component in parents {
        directory =
            open_or_create_rebuildable_generation_directory(&directory, component, public_label)?;
    }
    let descriptor = fs::openat(
        &directory,
        leaf,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map_err(|error| {
        capability_error(
            public_label,
            &format!("fișierul generației create-only a fost refuzat: {error}"),
        )
    })?;
    validate_regular_single_link(&descriptor, public_label, "rebuildable generation file")?;
    File::from(descriptor).write_all(bytes).map_err(|error| {
        capability_error(
            public_label,
            &format!("fișierul generației nu a putut fi materializat complet: {error}"),
        )
    })
}

/// Clones an immutable published Preview tree into a private generation.
/// Linux reflinks share unchanged extents copy-on-write; filesystems
/// without FICLONE support fall back first to an in-kernel range copy and
/// finally to descriptor-bound userspace copying.
/// Excluded paths are materialized later from the current Rust projection.
pub(in crate::kernel::write_authority::capability) fn clone_rebuildable_generation_tree(
    source_authority: &DirectoryAuthority,
    target_authority: &DirectoryAuthority,
    excluded: &BTreeSet<PathBuf>,
    max_entries: usize,
    max_bytes: u64,
    public_label: &str,
) -> Result<CapabilityGenerationCloneStats, String> {
    require_rebuildable_generation_authority(source_authority, public_label)?;
    require_rebuildable_generation_authority(target_authority, public_label)?;
    verify_directory_authority_path(source_authority)?;
    verify_directory_authority_path(target_authority)?;

    let mut plan = Vec::<(PathBuf, bool, u64)>::new();
    let mut total_bytes = 0_u64;
    for entry in WalkDir::new(source_authority.root_path())
        .follow_links(false)
        .sort_by_file_name()
    {
        let entry = entry.map_err(|error| {
            capability_error(
                public_label,
                &format!("arborele publicat nu poate fi parcurs: {error}"),
            )
        })?;
        let relative = entry
            .path()
            .strip_prefix(source_authority.root_path())
            .map_err(|_| {
                capability_error(public_label, "arborele publicat a ieșit din authority")
            })?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        if excluded
            .iter()
            .any(|path| relative == path || relative.starts_with(path))
        {
            continue;
        }
        if entry.file_type().is_symlink() {
            return Err(capability_error(
                public_label,
                &format!(
                    "arborele publicat conține symlink: {}",
                    entry.path().display()
                ),
            ));
        }
        let is_directory = entry.file_type().is_dir();
        if !is_directory && !entry.file_type().is_file() {
            return Err(capability_error(
                public_label,
                &format!(
                    "arborele publicat conține un tip neacceptat: {}",
                    entry.path().display()
                ),
            ));
        }
        let size = if is_directory {
            0
        } else {
            entry
                .metadata()
                .map_err(|error| {
                    capability_error(
                        public_label,
                        &format!("fișierul publicat nu poate fi măsurat: {error}"),
                    )
                })?
                .len()
        };
        total_bytes = total_bytes.checked_add(size).ok_or_else(|| {
            capability_error(
                public_label,
                "arborele publicat a depășit contorul de bytes",
            )
        })?;
        if plan.len() >= max_entries || total_bytes > max_bytes {
            return Err(capability_error(
                public_label,
                "arborele publicat depășește bugetul generației Preview",
            ));
        }
        plan.push((relative.to_path_buf(), is_directory, size));
    }

    let mut stats = CapabilityGenerationCloneStats::default();
    for (relative, is_directory, size) in plan {
        if is_directory {
            create_rebuildable_generation_directory(target_authority, &relative, public_label)?;
        } else {
            let reflinked = clone_rebuildable_generation_file(
                source_authority,
                target_authority,
                &relative,
                size,
                public_label,
            )?;
            if reflinked {
                stats.reflinked_files = stats.reflinked_files.saturating_add(1);
            } else {
                stats.copied_files = stats.copied_files.saturating_add(1);
            }
        }
        stats.entries = stats.entries.saturating_add(1);
        stats.bytes = stats
            .bytes
            .checked_add(size)
            .ok_or_else(|| capability_error(public_label, "clone bytes overflow"))?;
    }
    Ok(stats)
}

pub(super) fn clone_rebuildable_generation_file(
    source_authority: &DirectoryAuthority,
    target_authority: &DirectoryAuthority,
    relative_path: &Path,
    expected_size: u64,
    public_label: &str,
) -> Result<bool, String> {
    let source_components =
        rebuildable_generation_components(source_authority, relative_path, public_label)?;
    let target_components =
        rebuildable_generation_components(target_authority, relative_path, public_label)?;
    let (source_leaf, source_parents) = source_components
        .split_last()
        .expect("validated clone source path has a leaf");
    let (target_leaf, target_parents) = target_components
        .split_last()
        .expect("validated clone target path has a leaf");

    let mut source_directory = rustix::io::dup(source_authority.directory()).map_err(|error| {
        capability_error(
            public_label,
            &format!("source authority nu poate fi duplicat: {error}"),
        )
    })?;
    for component in source_parents {
        source_directory =
            open_directory_strict(&source_directory, component).map_err(|error| {
                capability_error(
                    public_label,
                    &format!("source ancestor nu poate fi deschis: {error}"),
                )
            })?;
    }
    let source_descriptor = fs::openat(
        &source_directory,
        source_leaf,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| {
        capability_error(
            public_label,
            &format!("source file nu poate fi deschis: {error}"),
        )
    })?;
    validate_regular_single_link(
        &source_descriptor,
        public_label,
        "rebuildable generation clone source",
    )?;
    let source_before = fs::fstat(&source_descriptor).map_err(|error| {
        capability_error(public_label, &format!("source fstat a eșuat: {error}"))
    })?;
    if u64::try_from(source_before.st_size).ok() != Some(expected_size) {
        return Err(capability_error(
            public_label,
            "source size s-a schimbat după planificarea clonei",
        ));
    }

    let mut target_directory = rustix::io::dup(target_authority.directory()).map_err(|error| {
        capability_error(
            public_label,
            &format!("target authority nu poate fi duplicat: {error}"),
        )
    })?;
    for component in target_parents {
        target_directory = open_or_create_rebuildable_generation_directory(
            &target_directory,
            component,
            public_label,
        )?;
    }
    let target_descriptor = fs::openat(
        &target_directory,
        target_leaf,
        OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map_err(|error| {
        capability_error(
            public_label,
            &format!("target file create-only a eșuat: {error}"),
        )
    })?;
    validate_regular_single_link(
        &target_descriptor,
        public_label,
        "rebuildable generation clone target",
    )?;

    let mut source_file = File::from(source_descriptor);
    let mut target_file = File::from(target_descriptor);
    let reflinked = match fs::ioctl_ficlone(&target_file, &source_file) {
        Ok(()) => true,
        Err(_) => {
            target_file.set_len(0).map_err(|error| {
                capability_error(
                    public_label,
                    &format!("target fallback truncate a eșuat: {error}"),
                )
            })?;
            source_file.seek(SeekFrom::Start(0)).map_err(|error| {
                capability_error(
                    public_label,
                    &format!("source fallback seek a eșuat: {error}"),
                )
            })?;
            target_file.seek(SeekFrom::Start(0)).map_err(|error| {
                capability_error(
                    public_label,
                    &format!("target fallback seek a eșuat: {error}"),
                )
            })?;
            let mut source_offset = 0_u64;
            let mut target_offset = 0_u64;
            let mut kernel_copy_complete = true;
            while source_offset < expected_size {
                let remaining = expected_size.saturating_sub(source_offset);
                let chunk = usize::try_from(remaining)
                    .unwrap_or(usize::MAX)
                    .min(8 * 1024 * 1024);
                match fs::copy_file_range(
                    &source_file,
                    Some(&mut source_offset),
                    &target_file,
                    Some(&mut target_offset),
                    chunk,
                ) {
                    Ok(0) | Err(_) => {
                        kernel_copy_complete = false;
                        break;
                    }
                    Ok(_) => {}
                }
            }
            if !kernel_copy_complete {
                target_file.set_len(0).map_err(|error| {
                    capability_error(
                        public_label,
                        &format!("target userspace fallback truncate a eșuat: {error}"),
                    )
                })?;
                source_file.seek(SeekFrom::Start(0)).map_err(|error| {
                    capability_error(
                        public_label,
                        &format!("source userspace fallback seek a eșuat: {error}"),
                    )
                })?;
                target_file.seek(SeekFrom::Start(0)).map_err(|error| {
                    capability_error(
                        public_label,
                        &format!("target userspace fallback seek a eșuat: {error}"),
                    )
                })?;
                let copied =
                    std::io::copy(&mut source_file, &mut target_file).map_err(|error| {
                        capability_error(
                            public_label,
                            &format!("userspace fallback copy a eșuat: {error}"),
                        )
                    })?;
                if copied != expected_size {
                    return Err(capability_error(
                        public_label,
                        "userspace fallback copy a produs o dimensiune divergentă",
                    ));
                }
            }
            false
        }
    };

    let source_after = fs::fstat(&source_file).map_err(|error| {
        capability_error(public_label, &format!("source post-clone fstat: {error}"))
    })?;
    let target_after = fs::fstat(&target_file).map_err(|error| {
        capability_error(public_label, &format!("target post-clone fstat: {error}"))
    })?;
    if !same_file_identity(&source_before, &source_after)
        || version_token_for_stat(&source_before) != version_token_for_stat(&source_after)
        || FileType::from_raw_mode(target_after.st_mode) != FileType::RegularFile
        || target_after.st_nlink != 1
        || u64::try_from(target_after.st_size).ok() != Some(expected_size)
    {
        return Err(capability_error(
            public_label,
            "clone postflight a observat identitate sau dimensiune divergentă",
        ));
    }
    validate_named_file_identity(
        &source_directory,
        source_leaf,
        &source_after,
        "rebuildable generation clone source",
    )?;
    validate_named_file_identity(
        &target_directory,
        target_leaf,
        &target_after,
        "rebuildable generation clone target",
    )?;
    Ok(reflinked)
}

pub(in crate::kernel::write_authority::capability) fn seal_rebuildable_generation(
    authority: &DirectoryAuthority,
    public_label: &str,
) -> Result<(), String> {
    require_rebuildable_generation_authority(authority, public_label)?;
    verify_directory_authority_path(authority)?;
    sync_directory(authority.directory(), public_label)
}

pub(in crate::kernel::write_authority::capability) fn is_real_directory_leaf(
    authority: &DirectoryAuthority,
    path: &Path,
    public_label: &str,
) -> Result<bool, String> {
    require_rebuildable_generation_authority(authority, public_label)?;
    let target = WriteTarget::new(path, authority.root_path(), public_label)
        .bind_authority(authority.clone())?;
    let lexical = lexical_target(&target, false)?;
    if lexical.relative_components.len() != 1 {
        return Err(capability_error(
            public_label,
            "inspecția generației retrase cere un singur leaf",
        ));
    }
    match fs::statat(
        authority.directory(),
        &lexical.relative_components[0],
        AtFlags::SYMLINK_NOFOLLOW,
    ) {
        Ok(stat) => Ok(FileType::from_raw_mode(stat.st_mode) == FileType::Directory),
        Err(Errno::NOENT) => Ok(false),
        Err(error) => Err(capability_error(
            public_label,
            &format!("leaf-ul generației nu a putut fi inspectat: {error}"),
        )),
    }
}
