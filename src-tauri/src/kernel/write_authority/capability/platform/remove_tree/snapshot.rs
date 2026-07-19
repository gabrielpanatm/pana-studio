use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TreeSnapshot {
    pub(super) fingerprint: String,
    pub(super) entry_count: u64,
    pub(super) records: Vec<TreeFingerprintRecord>,
}

pub(super) fn capture_tree_snapshot(
    directory: &OwnedFd,
    expected_mount_id: u64,
    public_label: &str,
    stage: &str,
) -> Result<TreeSnapshot, String> {
    let first = capture_tree_snapshot_once(directory, expected_mount_id, public_label, stage)?;
    let second = capture_tree_snapshot_once(directory, expected_mount_id, public_label, stage)?;
    if first != second {
        return Err(capability_error(
            public_label,
            &format!("{stage}: arborele s-a schimbat în timpul fingerprint-ului dublu"),
        ));
    }
    Ok(second)
}

fn capture_tree_snapshot_once(
    directory: &OwnedFd,
    expected_mount_id: u64,
    public_label: &str,
    stage: &str,
) -> Result<TreeSnapshot, String> {
    if mount_id_for_fd(directory, public_label, stage)? != expected_mount_id {
        return Err(capability_error(
            public_label,
            &format!("{stage}: root mount ID diferă de plan"),
        ));
    }
    let root_before = fs::fstat(directory).map_err(|error| {
        capability_error(
            public_label,
            &format!("{stage}: root metadata nu poate fi citită: {error}"),
        )
    })?;
    let mut records = Vec::new();
    let mut entry_count = 0_u64;
    collect_tree_records(
        directory,
        "",
        0,
        &mut entry_count,
        &mut records,
        expected_mount_id,
        public_label,
        stage,
    )?;
    let root_after = fs::fstat(directory).map_err(|error| {
        capability_error(
            public_label,
            &format!("{stage}: root post-metadata nu poate fi citită: {error}"),
        )
    })?;
    if version_token_for_stat(&root_before) != version_token_for_stat(&root_after) {
        return Err(capability_error(
            public_label,
            &format!("{stage}: root-ul s-a schimbat în timpul enumerării"),
        ));
    }
    let fingerprint = tree_fingerprint_from_records(records.clone());
    Ok(TreeSnapshot {
        fingerprint,
        entry_count,
        records,
    })
}

#[allow(clippy::too_many_arguments)]
fn collect_tree_records(
    directory: &OwnedFd,
    prefix: &str,
    depth: usize,
    entry_count: &mut u64,
    records: &mut Vec<TreeFingerprintRecord>,
    expected_mount_id: u64,
    public_label: &str,
    stage: &str,
) -> Result<(), String> {
    if depth > MAX_REMOVE_TREE_DEPTH {
        return Err(capability_error(
            public_label,
            &format!("{stage}: arborele depășește adâncimea {MAX_REMOVE_TREE_DEPTH}"),
        ));
    }
    let mut stream = Dir::read_from(directory).map_err(|error| {
        capability_error(
            public_label,
            &format!("{stage}: directorul nu poate fi enumerat: {error}"),
        )
    })?;
    let mut names = Vec::new();
    while let Some(entry) = stream.read() {
        let entry = entry.map_err(|error| {
            capability_error(
                public_label,
                &format!("{stage}: enumerarea directorului a eșuat: {error}"),
            )
        })?;
        let bytes = entry.file_name().to_bytes();
        if bytes == b"." || bytes == b".." {
            continue;
        }
        names.push(OsString::from_vec(bytes.to_vec()));
    }
    drop(stream);
    names.sort_by(|left, right| left.as_encoded_bytes().cmp(right.as_encoded_bytes()));

    for name in names {
        *entry_count = entry_count.saturating_add(1);
        if *entry_count > MAX_REMOVE_TREE_ENTRIES as u64 {
            return Err(capability_error(
                public_label,
                &format!("{stage}: arborele depășește {MAX_REMOVE_TREE_ENTRIES} intrări"),
            ));
        }
        let stat = fs::statat(directory, &name, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
            capability_error(
                public_label,
                &format!("{stage}: un descendent nu poate fi verificat: {error}"),
            )
        })?;
        let mount_id = mount_id_for_name(directory, &name, public_label, stage)?;
        if mount_id != expected_mount_id {
            return Err(capability_error(
                public_label,
                &format!("{stage}: un descendent traversează un mount/bind mount"),
            ));
        }
        let key = child_key(prefix, &name);
        let file_type = FileType::from_raw_mode(stat.st_mode);
        let kind = match file_type {
            FileType::Directory => b'd',
            FileType::RegularFile => b'f',
            FileType::Symlink => b'l',
            _ => b'o',
        };
        let expected_version = version_token_for_stat(&stat);
        records.push(TreeFingerprintRecord {
            relative_path: key.clone(),
            kind,
            version_token: expected_version.clone(),
        });
        if file_type == FileType::Directory {
            let child = open_directory_strict(directory, &name).map_err(|error| {
                capability_error(
                    public_label,
                    &format!("{stage}: un descendent director nu poate fi capturat: {error}"),
                )
            })?;
            validate_open_directory_identity(&child, &stat, public_label, stage)?;
            if mount_id_for_fd(&child, public_label, stage)? != expected_mount_id {
                return Err(capability_error(
                    public_label,
                    &format!("{stage}: descriptorul child traversează un mount"),
                ));
            }
            collect_tree_records(
                &child,
                &key,
                depth + 1,
                entry_count,
                records,
                expected_mount_id,
                public_label,
                stage,
            )?;
        }
        let after = fs::statat(directory, &name, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
            capability_error(
                public_label,
                &format!("{stage}: descendentul a dispărut în timpul fingerprint-ului: {error}"),
            )
        })?;
        if expected_version != version_token_for_stat(&after) {
            return Err(capability_error(
                public_label,
                &format!("{stage}: un descendent s-a schimbat în timpul fingerprint-ului"),
            ));
        }
    }
    Ok(())
}

pub(super) fn child_key(prefix: &str, name: &OsStr) -> String {
    let component = encode_component_hex(name);
    if prefix.is_empty() {
        component
    } else {
        format!("{prefix}/{component}")
    }
}

pub(super) fn mount_id_for_fd(
    directory: &OwnedFd,
    public_label: &str,
    role: &str,
) -> Result<u64, String> {
    let observed = fs::statx(
        directory,
        "",
        AtFlags::EMPTY_PATH | AtFlags::NO_AUTOMOUNT,
        rustix::fs::StatxFlags::MNT_ID,
    )
    .map_err(|error| {
        capability_error(
            public_label,
            &format!("{role}: statx/MNT_ID indisponibil: {error}"),
        )
    })?;
    mount_id_from_statx(&observed, public_label, role)
}

pub(super) fn mount_id_for_name(
    parent: &OwnedFd,
    name: &OsStr,
    public_label: &str,
    role: &str,
) -> Result<u64, String> {
    let observed = fs::statx(
        parent,
        name,
        AtFlags::SYMLINK_NOFOLLOW | AtFlags::NO_AUTOMOUNT,
        rustix::fs::StatxFlags::MNT_ID,
    )
    .map_err(|error| {
        capability_error(
            public_label,
            &format!("{role}: statx/MNT_ID pentru leaf a eșuat: {error}"),
        )
    })?;
    mount_id_from_statx(&observed, public_label, role)
}

fn mount_id_from_statx(
    observed: &rustix::fs::Statx,
    public_label: &str,
    role: &str,
) -> Result<u64, String> {
    if observed.stx_mask & rustix::fs::StatxFlags::MNT_ID.bits() == 0 || observed.stx_mnt_id == 0 {
        return Err(capability_error(
            public_label,
            &format!("{role}: kernelul nu a furnizat STATX_MNT_ID; ștergerea este fail-closed"),
        ));
    }
    Ok(observed.stx_mnt_id)
}
