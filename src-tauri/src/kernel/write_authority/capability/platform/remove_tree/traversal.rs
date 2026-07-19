use std::collections::{HashMap, HashSet};

use super::{
    snapshot::{child_key, mount_id_for_fd, mount_id_for_name},
    *,
};

pub(super) fn records_by_key(
    records: &[TreeFingerprintRecord],
) -> HashMap<String, TreeFingerprintRecord> {
    records
        .iter()
        .cloned()
        .map(|record| (record.relative_path.clone(), record))
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn remove_planned_tree_contents(
    directory: &OwnedFd,
    prefix: &str,
    depth: usize,
    removed: &mut u64,
    expected_mount_id: u64,
    planned: &HashMap<String, TreeFingerprintRecord>,
    public_label: &str,
) -> Result<(), String> {
    if depth > MAX_REMOVE_TREE_DEPTH {
        return Err(capability_error(
            public_label,
            &format!("RemoveDirectoryTree depășește adâncimea {MAX_REMOVE_TREE_DEPTH}"),
        ));
    }
    if mount_id_for_fd(directory, public_label, "RemoveDirectoryTree traversal")?
        != expected_mount_id
    {
        return Err(capability_error(
            public_label,
            "RemoveDirectoryTree traversal a traversat un mount",
        ));
    }
    let mut stream = Dir::read_from(directory).map_err(|error| {
        capability_error(
            public_label,
            &format!("RemoveDirectoryTree directorul nu poate fi enumerat: {error}"),
        )
    })?;
    let mut names = Vec::new();
    while let Some(entry) = stream.read() {
        let entry = entry.map_err(|error| {
            capability_error(
                public_label,
                &format!("RemoveDirectoryTree enumerarea a eșuat: {error}"),
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

    let current_keys = names
        .iter()
        .map(|name| child_key(prefix, name))
        .collect::<HashSet<_>>();
    let expected_keys = planned
        .keys()
        .filter(|key| is_direct_child(prefix, key))
        .cloned()
        .collect::<HashSet<_>>();
    if current_keys != expected_keys {
        return Err(capability_error(
            public_label,
            "RemoveDirectoryTree a observat descendenți adăugați sau dispăruți după plan",
        ));
    }

    for name in names {
        let key = child_key(prefix, &name);
        let expected = planned.get(&key).ok_or_else(|| {
            capability_error(
                public_label,
                "RemoveDirectoryTree a observat o intrare fără evidence în plan",
            )
        })?;
        let stat = fs::statat(directory, &name, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
            capability_error(
                public_label,
                &format!("RemoveDirectoryTree intrarea nu poate fi verificată: {error}"),
            )
        })?;
        if mount_id_for_name(
            directory,
            &name,
            public_label,
            "RemoveDirectoryTree traversal",
        )? != expected_mount_id
        {
            return Err(capability_error(
                public_label,
                "RemoveDirectoryTree a refuzat un descendent pe alt mount",
            ));
        }
        let file_type = FileType::from_raw_mode(stat.st_mode);
        let kind = match file_type {
            FileType::Directory => b'd',
            FileType::RegularFile => b'f',
            FileType::Symlink => b'l',
            _ => b'o',
        };
        if kind != expected.kind || version_token_for_stat(&stat) != expected.version_token {
            return Err(capability_error(
                public_label,
                "RemoveDirectoryTree a observat un descendent înlocuit/modificat după plan",
            ));
        }
        if file_type == FileType::Directory {
            let child = open_directory_strict(directory, &name).map_err(|error| {
                capability_error(
                    public_label,
                    &format!("RemoveDirectoryTree child nu poate fi capturat: {error}"),
                )
            })?;
            validate_open_directory_identity(
                &child,
                &stat,
                public_label,
                "RemoveDirectoryTree child",
            )?;
            remove_planned_tree_contents(
                &child,
                &key,
                depth + 1,
                removed,
                expected_mount_id,
                planned,
                public_label,
            )?;
            validate_named_directory_identity(
                directory,
                &name,
                &child,
                public_label,
                "RemoveDirectoryTree child gol",
            )?;
            fs::unlinkat(directory, &name, AtFlags::REMOVEDIR).map_err(|error| {
                capability_error(
                    public_label,
                    &format!("RemoveDirectoryTree child gol nu poate fi eliminat: {error}"),
                )
            })?;
        } else {
            let before_unlink =
                fs::statat(directory, &name, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
                    capability_error(
                        public_label,
                        &format!(
                            "RemoveDirectoryTree leaf nu poate fi reverificat înainte de unlink: {error}"
                        ),
                    )
                })?;
            if version_token_for_stat(&before_unlink) != expected.version_token {
                return Err(capability_error(
                    public_label,
                    "RemoveDirectoryTree leaf s-a schimbat înainte de unlink",
                ));
            }
            fs::unlinkat(directory, &name, AtFlags::empty()).map_err(|error| {
                capability_error(
                    public_label,
                    &format!("RemoveDirectoryTree leaf nu poate fi eliminat: {error}"),
                )
            })?;
        }
        *removed = removed.saturating_add(1);
        if *removed > MAX_REMOVE_TREE_ENTRIES as u64 {
            return Err(capability_error(
                public_label,
                &format!("RemoveDirectoryTree depășește {MAX_REMOVE_TREE_ENTRIES} intrări"),
            ));
        }
    }
    sync_directory(directory, public_label)
}

fn is_direct_child(prefix: &str, key: &str) -> bool {
    if prefix.is_empty() {
        !key.contains('/')
    } else {
        key.strip_prefix(prefix)
            .and_then(|suffix| suffix.strip_prefix('/'))
            .is_some_and(|suffix| !suffix.contains('/'))
    }
}
