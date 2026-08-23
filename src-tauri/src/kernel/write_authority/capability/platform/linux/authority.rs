use super::{
    capability_error, capture_boundary_from_path, capture_existing_target_parent,
    create_directory_all, fs, lexical_target, open_directory_strict, sync_directory,
    validate_regular_single_link, CapabilityLockMode, CaptureFailure, DirectoryAuthority,
    DirectoryAuthorityScope, FileType, FilesystemIdentity, FlockOperation, OFlags, OwnedFd, Path,
    PathBuf, WriteTarget, FILE_MODE,
};
#[cfg(test)]
use super::{capture_existing_boundary, Dir};
use std::os::fd::AsRawFd;

pub(in crate::kernel::write_authority::capability) struct CapabilityFileLock {
    _descriptor: OwnedFd,
}

pub(in crate::kernel::write_authority::capability) struct CapabilityDirectoryLease {
    directory: OwnedFd,
    #[cfg_attr(not(test), allow(dead_code))]
    public_label: String,
}

impl CapabilityDirectoryLease {
    pub(in crate::kernel::write_authority::capability) fn current_dir_path(&self) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", self.directory.as_raw_fd()))
    }

    #[cfg(test)]
    pub(in crate::kernel::write_authority::capability) fn require_empty(
        &self,
    ) -> Result<(), String> {
        let mut stream = Dir::read_from(&self.directory).map_err(|error| {
            capability_error(
                &self.public_label,
                &format!("directorul capturat nu a putut fi enumerat: {error}"),
            )
        })?;
        while let Some(entry) = stream.read() {
            let entry = entry.map_err(|error| {
                capability_error(
                    &self.public_label,
                    &format!("enumerarea directorului capturat a eșuat: {error}"),
                )
            })?;
            let name = entry.file_name().to_bytes();
            if name != b"." && name != b".." {
                return Err(capability_error(
                    &self.public_label,
                    "directorul capturat nu mai este gol",
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub(super) fn capture_directory_lease(
    path: &Path,
    public_label: &str,
) -> Result<CapabilityDirectoryLease, String> {
    let target = WriteTarget::new(path, path, public_label);
    let lexical = lexical_target(&target, true)?;
    let captured = capture_existing_boundary(&lexical)?
        .ok_or_else(|| capability_error(public_label, "directorul subprocess nu există"))?;
    let metadata = fs::fstat(&captured.directory).map_err(|error| {
        capability_error(
            public_label,
            &format!("identitatea directorului subprocess nu poate fi citită: {error}"),
        )
    })?;
    if FileType::from_raw_mode(metadata.st_mode) != FileType::Directory {
        return Err(capability_error(
            public_label,
            "capability-ul subprocess nu este director",
        ));
    }
    Ok(CapabilityDirectoryLease {
        directory: captured.directory,
        public_label: public_label.to_string(),
    })
}

pub(in crate::kernel::write_authority::capability) fn capture_directory_lease_from_authority(
    authority: &DirectoryAuthority,
    path: &Path,
    public_label: &str,
) -> Result<CapabilityDirectoryLease, String> {
    verify_directory_authority_path(authority)?;
    let target = WriteTarget::new(path, authority.root_path(), public_label)
        .bind_authority(authority.clone())?;
    let lexical = lexical_target(&target, true)?;
    let mut directory = rustix::io::dup(authority.directory()).map_err(|error| {
        capability_error(
            public_label,
            &format!("authority subprocess nu a putut fi duplicată: {error}"),
        )
    })?;
    for component in &lexical.relative_components {
        directory = open_directory_strict(&directory, component).map_err(|error| {
            capability_error(
                public_label,
                &format!("directorul subprocess nu a putut fi derivat: {error}"),
            )
        })?;
    }
    Ok(CapabilityDirectoryLease {
        directory,
        public_label: public_label.to_string(),
    })
}

pub(in crate::kernel::write_authority::capability) fn capture_directory_authority(
    path: &Path,
    public_label: &str,
    scope: DirectoryAuthorityScope,
) -> Result<DirectoryAuthority, String> {
    let target = WriteTarget::new(path, path, public_label);
    let lexical = lexical_target(&target, true)?;
    let captured = capture_boundary_from_path(&lexical, false)
        .map_err(CaptureFailure::into_diagnostic)?
        .ok_or_else(|| capability_error(public_label, "authority root nu există"))?;
    authority_from_captured(path, public_label, scope, captured.directory)
}

pub(in crate::kernel::write_authority::capability) fn bootstrap_directory_authority(
    path: &Path,
    public_label: &str,
    scope: DirectoryAuthorityScope,
) -> Result<DirectoryAuthority, String> {
    let target = WriteTarget::new(path, path, public_label);
    let lexical = lexical_target(&target, true)?;
    let captured = capture_boundary_from_path(&lexical, true)
        .map_err(CaptureFailure::into_diagnostic)?
        .ok_or_else(|| {
            capability_error(public_label, "authority root nu a putut fi creat/capturat")
        })?;
    sync_directory(&captured.directory, public_label)?;
    authority_from_captured(path, public_label, scope, captured.directory)
}

pub(in crate::kernel::write_authority::capability) fn create_directory_from_authority(
    authority: &DirectoryAuthority,
    path: &Path,
    public_label: &str,
) -> Result<(), String> {
    let target = WriteTarget::new(path, authority.root_path(), public_label)
        .bind_authority(authority.clone())?;
    let effect = create_directory_all(&target)?;
    if effect.recovery_required {
        return Err(effect.diagnostic.unwrap_or_else(|| {
            format!(
                "Capability filesystem a creat {public_label}, dar durabilitatea cere recovery."
            )
        }));
    }
    Ok(())
}

pub(in crate::kernel::write_authority::capability) fn capture_descendant_authority(
    parent: &DirectoryAuthority,
    path: &Path,
    public_label: &str,
    scope: DirectoryAuthorityScope,
) -> Result<DirectoryAuthority, String> {
    verify_directory_authority_path(parent)?;
    let target =
        WriteTarget::new(path, parent.root_path(), public_label).bind_authority(parent.clone())?;
    let lexical = lexical_target(&target, true)?;
    let mut directory = rustix::io::dup(parent.directory()).map_err(|error| {
        capability_error(
            public_label,
            &format!("authority parent nu a putut fi duplicată: {error}"),
        )
    })?;
    for component in &lexical.relative_components {
        directory = open_directory_strict(&directory, component).map_err(|error| {
            capability_error(
                public_label,
                &format!("authority descendant nu a putut fi capturată: {error}"),
            )
        })?;
    }
    authority_from_captured(path, public_label, scope, directory)
}

pub(in crate::kernel::write_authority::capability) fn verify_directory_authority_path(
    authority: &DirectoryAuthority,
) -> Result<(), String> {
    let target = WriteTarget::new(
        authority.root_path(),
        authority.root_path(),
        "authority/path-binding",
    );
    let lexical = lexical_target(&target, true)?;
    let captured = capture_boundary_from_path(&lexical, false)
        .map_err(CaptureFailure::into_diagnostic)?
        .ok_or_else(|| {
            capability_error(
                "authority/path-binding",
                &format!(
                    "pathname-ul authority {} nu mai există",
                    authority.root_path().display()
                ),
            )
        })?;
    let observed = identity_from_fd(&captured.directory, "authority/path-binding")?;
    if observed != authority.identity() {
        return Err(capability_error(
            "authority/path-binding",
            &format!(
                "pathname-ul {} a fost înlocuit (expected dev={} ino={}, observed dev={} ino={})",
                authority.root_path().display(),
                authority.identity().device,
                authority.identity().inode,
                observed.device,
                observed.inode
            ),
        ));
    }
    Ok(())
}

pub(super) fn authority_from_captured(
    path: &Path,
    public_label: &str,
    scope: DirectoryAuthorityScope,
    directory: OwnedFd,
) -> Result<DirectoryAuthority, String> {
    let identity = identity_from_fd(&directory, public_label)?;
    Ok(DirectoryAuthority::from_opened_directory(
        path.to_path_buf(),
        identity,
        scope,
        directory,
    ))
}

pub(super) fn identity_from_fd(
    directory: &OwnedFd,
    public_label: &str,
) -> Result<FilesystemIdentity, String> {
    let metadata = fs::fstat(directory).map_err(|error| {
        capability_error(
            public_label,
            &format!("identitatea authority nu poate fi citită: {error}"),
        )
    })?;
    if FileType::from_raw_mode(metadata.st_mode) != FileType::Directory {
        return Err(capability_error(
            public_label,
            "authority handle nu desemnează un director",
        ));
    }
    Ok(FilesystemIdentity {
        device: metadata.st_dev,
        inode: metadata.st_ino,
    })
}

pub(in crate::kernel::write_authority::capability) fn lock_file(
    target: &WriteTarget,
    mode: CapabilityLockMode,
) -> Result<CapabilityFileLock, String> {
    let lexical = lexical_target(target, false)?;
    let parent = capture_existing_target_parent(&lexical)?.ok_or_else(|| {
        capability_error(
            &lexical.public_label,
            "folderul părinte al lock-ului nu a putut fi capturat",
        )
    })?;
    let descriptor = fs::openat(
        &parent.directory,
        &parent.leaf,
        OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        FILE_MODE,
    )
    .map_err(|error| {
        capability_error(
            &lexical.public_label,
            &format!("fișierul lock stabil nu a putut fi deschis: {error}"),
        )
    })?;
    validate_regular_single_link(&descriptor, &lexical.public_label, "StableLock")?;
    let operation = match mode {
        CapabilityLockMode::Shared => FlockOperation::LockShared,
        CapabilityLockMode::Exclusive => FlockOperation::LockExclusive,
    };
    fs::flock(&descriptor, operation).map_err(|error| {
        capability_error(
            &lexical.public_label,
            &format!("lock-ul stabil nu a putut fi obținut: {error}"),
        )
    })?;
    Ok(CapabilityFileLock {
        _descriptor: descriptor,
    })
}
