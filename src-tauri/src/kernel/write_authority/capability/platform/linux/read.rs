use super::{
    absolute_normal_components, capability_error, capture_existing_target_parent,
    capture_existing_target_parent_from_directory, fs, lexical_target, open_directory_strict,
    open_filesystem_root, run_test_hook, same_file_identity, verify_directory_authority_path,
    version_token_for_stat, AtFlags, CapabilityBoundedFileSnapshot, CapabilityTestStage,
    DirectoryAuthority, Errno, File, FileType, Mode, OFlags, OsStr, OwnedFd, Path, ResolveFlags,
    WriteTarget, MAX_OPENAT2_RACE_RETRIES,
};
use std::io::Read;

pub(in crate::kernel::write_authority::capability) fn read_bounded_regular_file_from_authority(
    authority: &DirectoryAuthority,
    path: &Path,
    public_label: &str,
    max_bytes: u64,
) -> Result<Option<CapabilityBoundedFileSnapshot>, String> {
    verify_directory_authority_path(authority)?;
    let target = WriteTarget::new(path, authority.root_path(), public_label)
        .bind_authority(authority.clone())?;
    let lexical = lexical_target(&target, false)?;
    let Some(parent) = capture_existing_target_parent(&lexical)? else {
        return Ok(None);
    };

    let descriptor = match open_regular_file_strict(&parent.directory, &parent.leaf) {
        Ok(descriptor) => descriptor,
        Err(Errno::NOENT) => return Ok(None),
        Err(error) => {
            return Err(capability_error(
                public_label,
                &format!(
                    "bounded read nu poate deschide leaf-ul fd-relative fără symlink: {error}"
                ),
            ));
        }
    };
    let before = fs::fstat(&descriptor).map_err(|error| {
        capability_error(
            public_label,
            &format!("bounded read nu poate verifica descriptorul: {error}"),
        )
    })?;
    if FileType::from_raw_mode(before.st_mode) != FileType::RegularFile {
        return Err(capability_error(
            public_label,
            "bounded read cere un fișier regular",
        ));
    }
    if before.st_nlink != 1 {
        return Err(capability_error(
            public_label,
            "bounded read refuză un inode cu mai multe hardlink-uri",
        ));
    }
    let expected_size = u64::try_from(before.st_size).map_err(|_| {
        capability_error(
            public_label,
            "bounded read a observat o dimensiune negativă",
        )
    })?;
    if expected_size > max_bytes {
        return Err(capability_error(
            public_label,
            &format!("fișierul are {expected_size} bytes și depășește limita de {max_bytes} bytes"),
        ));
    }
    let capacity = usize::try_from(expected_size).map_err(|_| {
        capability_error(
            public_label,
            "dimensiunea fișierului nu încape în memoria adresabilă",
        )
    })?;

    run_test_hook(CapabilityTestStage::AfterBoundedReadLeafOpened);
    let mut file = File::from(descriptor);
    let mut bytes = Vec::with_capacity(capacity);
    std::io::Read::by_ref(&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| {
            capability_error(
                public_label,
                &format!("fișierul nu poate fi citit bounded: {error}"),
            )
        })?;
    if bytes.len() as u64 != expected_size {
        return Err(capability_error(
            public_label,
            "fișierul și-a schimbat dimensiunea în timpul citirii bounded",
        ));
    }
    let after = fs::fstat(&file).map_err(|error| {
        capability_error(
            public_label,
            &format!("bounded read nu poate face fstat postflight: {error}"),
        )
    })?;
    if version_token_for_stat(&after) != version_token_for_stat(&before)
        || after.st_nlink != before.st_nlink
    {
        return Err(capability_error(
            public_label,
            "fișierul s-a schimbat în timpul citirii bounded",
        ));
    }

    let boundary = rustix::io::dup(authority.directory()).map_err(|error| {
        capability_error(
            public_label,
            &format!("authority root nu poate fi duplicat la postflight: {error}"),
        )
    })?;
    let Some(recaptured_parent) =
        capture_existing_target_parent_from_directory(&lexical, boundary)?
    else {
        return Err(capability_error(
            public_label,
            "parentul numit a dispărut în timpul citirii bounded",
        ));
    };
    let captured_parent_stat = fs::fstat(&parent.directory).map_err(|error| {
        capability_error(
            public_label,
            &format!("parentul capturat nu poate fi verificat la postflight: {error}"),
        )
    })?;
    let recaptured_parent_stat = fs::fstat(&recaptured_parent.directory).map_err(|error| {
        capability_error(
            public_label,
            &format!("parentul recapturat nu poate fi verificat la postflight: {error}"),
        )
    })?;
    if !same_file_identity(&captured_parent_stat, &recaptured_parent_stat) {
        return Err(capability_error(
            public_label,
            "path-ul nu mai numește parentul capturat în timpul citirii bounded",
        ));
    }
    let named = fs::statat(
        &recaptured_parent.directory,
        &recaptured_parent.leaf,
        AtFlags::SYMLINK_NOFOLLOW,
    )
    .map_err(|error| {
        capability_error(
            public_label,
            &format!("leaf-ul nu poate fi verificat la postflight: {error}"),
        )
    })?;
    if FileType::from_raw_mode(named.st_mode) != FileType::RegularFile
        || named.st_nlink != 1
        || version_token_for_stat(&named) != version_token_for_stat(&after)
    {
        return Err(capability_error(
            public_label,
            "numele leaf nu mai indică versiunea fișierului citit",
        ));
    }
    verify_directory_authority_path(authority)?;

    Ok(Some(CapabilityBoundedFileSnapshot {
        bytes,
        version_token: version_token_for_stat(&after),
    }))
}

pub(super) fn open_regular_file_strict(parent: &OwnedFd, leaf: &OsStr) -> Result<OwnedFd, Errno> {
    let open_flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let resolve_flags =
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS;
    for _ in 0..MAX_OPENAT2_RACE_RETRIES {
        match fs::openat2(parent, leaf, open_flags, Mode::empty(), resolve_flags) {
            Ok(descriptor) => return Ok(descriptor),
            Err(Errno::AGAIN) => continue,
            Err(Errno::NOSYS) => {
                return fs::openat(parent, leaf, open_flags, Mode::empty());
            }
            Err(error) => return Err(error),
        }
    }
    Err(Errno::AGAIN)
}

pub(in crate::kernel::write_authority::capability) fn open_optional_regular_file_readonly_no_follow(
    path: &Path,
    public_label: &str,
) -> Result<Option<File>, String> {
    let components = absolute_normal_components(path, public_label, "read-only source")?;
    let (leaf, parents) = components.split_last().ok_or_else(|| {
        capability_error(public_label, "read-only source trebuie să aibă un leaf")
    })?;
    let mut directory = open_filesystem_root(public_label)?;
    for parent in parents {
        directory = match open_directory_strict(&directory, parent) {
            Ok(directory) => directory,
            Err(Errno::NOENT) => return Ok(None),
            Err(error) => {
                return Err(capability_error(
                    public_label,
                    &format!(
                        "un părinte al read-only source nu poate fi deschis fără symlink: {error}"
                    ),
                ));
            }
        };
    }
    let descriptor = match fs::openat(
        &directory,
        leaf,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(descriptor) => descriptor,
        Err(Errno::NOENT) => return Ok(None),
        Err(error) => {
            return Err(capability_error(
                public_label,
                &format!("read-only source nu poate fi deschis fără symlink: {error}"),
            ));
        }
    };
    let stat = fs::fstat(&descriptor).map_err(|error| {
        capability_error(
            public_label,
            &format!("read-only source nu poate fi verificat: {error}"),
        )
    })?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
        return Err(capability_error(
            public_label,
            "read-only source nu este fișier regular",
        ));
    }
    Ok(Some(File::from(descriptor)))
}

pub(in crate::kernel::write_authority::capability) fn open_regular_file_readonly_no_follow(
    path: &Path,
    public_label: &str,
) -> Result<File, String> {
    open_optional_regular_file_readonly_no_follow(path, public_label)?
        .ok_or_else(|| capability_error(public_label, "read-only source nu există"))
}
