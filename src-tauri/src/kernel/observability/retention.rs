use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;

use crate::kernel::write_authority::{
    capability_remove_observability_file, capability_rename_observability_file,
    WriteAuthorityRuntime,
};

const DEFAULT_MAX_ACTIVE_BYTES: u64 = 4 * 1024 * 1024;
const DEFAULT_ARCHIVE_COUNT: usize = 5;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct KernelLogRetentionPolicy {
    pub max_active_bytes: u64,
    pub archive_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelLogArchiveSnapshot {
    pub index: usize,
    pub path: String,
    pub exists: bool,
    pub bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelLogRetentionSnapshot {
    pub max_active_bytes: u64,
    pub archive_count: usize,
    pub archived_count: usize,
    pub archived_bytes: u64,
    pub total_retained_bytes: u64,
    pub archives: Vec<KernelLogArchiveSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct KernelLogRotationReceipt {
    pub archived_path: PathBuf,
    pub active_bytes_before: u64,
    pub incoming_bytes: u64,
    pub max_active_bytes: u64,
    pub archive_count: usize,
    pub removed_oldest_archive: bool,
    pub shifted_archives: usize,
}

pub(crate) fn default_kernel_log_retention_policy() -> KernelLogRetentionPolicy {
    KernelLogRetentionPolicy {
        max_active_bytes: DEFAULT_MAX_ACTIVE_BYTES,
        archive_count: DEFAULT_ARCHIVE_COUNT,
    }
}

pub(crate) fn rotate_kernel_log_if_needed(
    runtime: Option<&WriteAuthorityRuntime>,
    path: &Path,
    incoming_bytes: u64,
    policy: &KernelLogRetentionPolicy,
) -> Result<Option<KernelLogRotationReceipt>, String> {
    if policy.max_active_bytes == 0 || policy.archive_count == 0 {
        return Ok(None);
    }

    let active_bytes = match fs::metadata(path) {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "Nu am putut citi metadata Observability Log pentru retenție: {}",
                error
            ));
        }
    };

    if active_bytes == 0 || active_bytes.saturating_add(incoming_bytes) <= policy.max_active_bytes {
        return Ok(None);
    }

    rotate_kernel_log(runtime, path, active_bytes, incoming_bytes, policy).map(Some)
}

pub(crate) fn read_kernel_log_retention_snapshot(
    path: &Path,
) -> Result<KernelLogRetentionSnapshot, String> {
    let policy = default_kernel_log_retention_policy();
    read_kernel_log_retention_snapshot_with_policy(path, &policy)
}

fn read_kernel_log_retention_snapshot_with_policy(
    path: &Path,
    policy: &KernelLogRetentionPolicy,
) -> Result<KernelLogRetentionSnapshot, String> {
    let active_bytes = match fs::metadata(path) {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => {
            return Err(format!(
                "Nu am putut citi metadata Observability Log pentru retenție: {}",
                error
            ));
        }
    };
    let mut archived_count = 0_usize;
    let mut archived_bytes = 0_u64;
    let mut archives = Vec::new();

    for index in 1..=policy.archive_count {
        let archive = archive_path(path, index)?;
        let (exists, bytes) = match fs::metadata(&archive) {
            Ok(metadata) => (true, metadata.len()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (false, 0),
            Err(error) => {
                return Err(format!(
                    "Nu am putut citi metadata arhivei Observability Log {}: {}",
                    archive.display(),
                    error
                ));
            }
        };
        if exists {
            archived_count += 1;
            archived_bytes = archived_bytes.saturating_add(bytes);
        }
        archives.push(KernelLogArchiveSnapshot {
            index,
            path: archive.to_string_lossy().to_string(),
            exists,
            bytes,
        });
    }

    Ok(KernelLogRetentionSnapshot {
        max_active_bytes: policy.max_active_bytes,
        archive_count: policy.archive_count,
        archived_count,
        archived_bytes,
        total_retained_bytes: active_bytes.saturating_add(archived_bytes),
        archives,
    })
}

fn rotate_kernel_log(
    runtime: Option<&WriteAuthorityRuntime>,
    path: &Path,
    active_bytes: u64,
    incoming_bytes: u64,
    policy: &KernelLogRetentionPolicy,
) -> Result<KernelLogRotationReceipt, String> {
    let boundary = path
        .parent()
        .ok_or_else(|| "Observability Log nu are boundary părinte.".to_string())?;
    let oldest = archive_path(path, policy.archive_count)?;
    let removed_oldest_archive = capability_remove_observability_file(
        runtime,
        &oldest,
        boundary,
        "observability/kernel-log-oldest-archive",
    )
    .map_err(|error| error.into_terminal_diagnostic())?;

    let mut shifted_archives = 0_usize;
    for index in (1..policy.archive_count).rev() {
        let source = archive_path(path, index)?;
        if !source.exists() {
            continue;
        }
        let destination = archive_path(path, index + 1)?;
        capability_rename_observability_file(
            runtime,
            &source,
            &destination,
            boundary,
            "observability/kernel-log-archive-source",
            "observability/kernel-log-archive-destination",
        )
        .map_err(|error| error.into_terminal_diagnostic())?;
        shifted_archives += 1;
    }

    let archived_path = archive_path(path, 1)?;
    capability_rename_observability_file(
        runtime,
        path,
        &archived_path,
        boundary,
        "observability/kernel-log-active",
        "observability/kernel-log-archive-1",
    )
    .map_err(|error| error.into_terminal_diagnostic())?;

    Ok(KernelLogRotationReceipt {
        archived_path,
        active_bytes_before: active_bytes,
        incoming_bytes,
        max_active_bytes: policy.max_active_bytes,
        archive_count: policy.archive_count,
        removed_oldest_archive,
        shifted_archives,
    })
}

pub(crate) fn archive_path(path: &Path, index: usize) -> Result<PathBuf, String> {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "Observability Log nu are nume de fișier valid.".to_string())?;
    Ok(path.with_file_name(format!("{name}.{index}")))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::{
        archive_path, read_kernel_log_retention_snapshot_with_policy, rotate_kernel_log_if_needed,
        KernelLogRetentionPolicy,
    };

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn rotation_moves_active_log_and_shifts_archives() {
        let root = temp_dir("rotate-shift");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("kernel.jsonl");
        fs::write(&path, "active-active").unwrap();
        fs::write(archive_path(&path, 1).unwrap(), "archive-1").unwrap();
        fs::write(archive_path(&path, 2).unwrap(), "archive-2").unwrap();
        let policy = KernelLogRetentionPolicy {
            max_active_bytes: 12,
            archive_count: 3,
        };

        let receipt = rotate_kernel_log_if_needed(None, &path, 4, &policy)
            .unwrap()
            .unwrap();

        assert!(!path.exists());
        assert_eq!(receipt.active_bytes_before, 13);
        assert_eq!(receipt.shifted_archives, 2);
        assert!(!receipt.removed_oldest_archive);
        assert_eq!(
            fs::read_to_string(archive_path(&path, 1).unwrap()).unwrap(),
            "active-active"
        );
        assert_eq!(
            fs::read_to_string(archive_path(&path, 2).unwrap()).unwrap(),
            "archive-1"
        );
        assert_eq!(
            fs::read_to_string(archive_path(&path, 3).unwrap()).unwrap(),
            "archive-2"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rotation_removes_oldest_archive_at_retention_limit() {
        let root = temp_dir("rotate-limit");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("kernel.jsonl");
        fs::write(&path, "active-active").unwrap();
        fs::write(archive_path(&path, 1).unwrap(), "archive-1").unwrap();
        fs::write(archive_path(&path, 2).unwrap(), "archive-2").unwrap();
        let policy = KernelLogRetentionPolicy {
            max_active_bytes: 12,
            archive_count: 2,
        };

        let receipt = rotate_kernel_log_if_needed(None, &path, 4, &policy)
            .unwrap()
            .unwrap();

        assert!(receipt.removed_oldest_archive);
        assert_eq!(
            fs::read_to_string(archive_path(&path, 1).unwrap()).unwrap(),
            "active-active"
        );
        assert_eq!(
            fs::read_to_string(archive_path(&path, 2).unwrap()).unwrap(),
            "archive-1"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn retention_snapshot_reports_archived_and_total_bytes() {
        let root = temp_dir("snapshot");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("kernel.jsonl");
        fs::write(&path, "active").unwrap();
        fs::write(archive_path(&path, 1).unwrap(), "archive").unwrap();
        let policy = KernelLogRetentionPolicy {
            max_active_bytes: 16,
            archive_count: 2,
        };

        let snapshot = read_kernel_log_retention_snapshot_with_policy(&path, &policy).unwrap();

        assert_eq!(snapshot.archived_count, 1);
        assert_eq!(snapshot.archived_bytes, 7);
        assert_eq!(snapshot.total_retained_bytes, 13);
        assert_eq!(snapshot.archives.len(), 2);
        assert!(snapshot.archives[0].exists);
        assert!(!snapshot.archives[1].exists);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_active_log_does_not_rotate() {
        let root = temp_dir("missing");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("kernel.jsonl");
        let policy = KernelLogRetentionPolicy {
            max_active_bytes: 16,
            archive_count: 2,
        };

        let receipt = rotate_kernel_log_if_needed(None, &path, 4, &policy).unwrap();

        assert!(receipt.is_none());
        fs::remove_dir_all(root).unwrap();
    }

    fn temp_dir(label: &str) -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("pana-observability-retention-{id}-{label}"))
    }
}
