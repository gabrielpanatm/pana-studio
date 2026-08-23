use std::{
    ffi::OsStr,
    fs::File,
    os::fd::{AsFd, AsRawFd},
};

use rustix::{
    fd::OwnedFd,
    fs::{self, AtFlags, Mode, OFlags},
    io::Errno,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CausalFileIdentity {
    pub(super) device_major: u32,
    pub(super) device_minor: u32,
    pub(super) inode: u64,
    pub(super) birth_time_seconds: i64,
    pub(super) birth_time_nanoseconds: u32,
    pub(super) mount_id: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CausalFileIdentityError {
    Statx(Errno),
    Incomplete,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum AnonymousFileLinkError {
    Primary(Errno),
    Fallback {
        primary: Errno,
        proc_fd_path: String,
        fallback: Errno,
    },
}

pub(super) fn open_anonymous_file(parent: &OwnedFd, mode: Mode) -> Result<File, Errno> {
    fs::openat(
        parent,
        ".",
        OFlags::RDWR | OFlags::TMPFILE | OFlags::CLOEXEC,
        mode,
    )
    .map(File::from)
}

pub(super) fn link_anonymous_file_create_only(
    file: &File,
    parent: &OwnedFd,
    leaf: &OsStr,
    force_proc_fd_fallback: bool,
) -> Result<(), AnonymousFileLinkError> {
    let primary = if force_proc_fd_fallback {
        Err(Errno::NOENT)
    } else {
        fs::linkat(file, "", parent, leaf, AtFlags::EMPTY_PATH)
    };

    match primary {
        Ok(()) => Ok(()),
        Err(primary) if matches!(primary, Errno::NOENT | Errno::PERM) => {
            let proc_fd_path = format!("/proc/self/fd/{}", file.as_raw_fd());
            fs::linkat(
                fs::CWD,
                proc_fd_path.as_str(),
                parent,
                leaf,
                AtFlags::SYMLINK_FOLLOW,
            )
            .map_err(|fallback| AnonymousFileLinkError::Fallback {
                primary,
                proc_fd_path,
                fallback,
            })
        }
        Err(error) => Err(AnonymousFileLinkError::Primary(error)),
    }
}

pub(super) fn causal_file_identity(
    descriptor: impl AsFd,
) -> Result<CausalFileIdentity, CausalFileIdentityError> {
    let requested =
        fs::StatxFlags::TYPE | fs::StatxFlags::INO | fs::StatxFlags::BTIME | fs::StatxFlags::MNT_ID;
    let observed = fs::statx(&descriptor, "", AtFlags::EMPTY_PATH, requested)
        .map_err(CausalFileIdentityError::Statx)?;
    if observed.stx_mask & requested.bits() != requested.bits()
        || observed.stx_btime.tv_nsec >= 1_000_000_000
    {
        return Err(CausalFileIdentityError::Incomplete);
    }
    Ok(CausalFileIdentity {
        device_major: observed.stx_dev_major,
        device_minor: observed.stx_dev_minor,
        inode: observed.stx_ino,
        birth_time_seconds: observed.stx_btime.tv_sec,
        birth_time_nanoseconds: observed.stx_btime.tv_nsec,
        mount_id: observed.stx_mnt_id,
    })
}
