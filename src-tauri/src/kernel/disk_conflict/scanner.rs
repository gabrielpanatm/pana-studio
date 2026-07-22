use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::kernel::file_buffer_store::{
    read_project_disk_text_snapshot, FileBufferBaseline, FileBufferEntry, FileBufferStore,
    ProjectDiskTextReadOutcome,
};

use super::model::{
    KernelDiskConflictFileSnapshot, KernelDiskConflictKind, KernelDiskConflictSnapshot,
    KernelDiskConflictStatus, KernelDiskConflictSummary, KERNEL_DISK_CONFLICT_SCHEMA_VERSION,
};

pub fn scan_disk_conflicts(store: &FileBufferStore) -> KernelDiskConflictSnapshot {
    let mut files = store
        .files
        .values()
        .map(|entry| scan_disk_conflict_entry(store, entry))
        .collect::<Vec<_>>();
    files.sort_by(|left, right| {
        status_rank(right.status)
            .cmp(&status_rank(left.status))
            .then_with(|| left.relative_path.cmp(&right.relative_path))
    });

    let summary = summarize_files(&files);

    KernelDiskConflictSnapshot {
        schema_version: KERNEL_DISK_CONFLICT_SCHEMA_VERSION,
        session_id: store.session_id.clone(),
        project_root: store.project_root.clone(),
        scanned_at_ms: now_ms(),
        max_file_bytes: store.limits.max_file_bytes,
        summary,
        files,
    }
}

pub(crate) fn scan_disk_conflict_entry(
    store: &FileBufferStore,
    entry: &FileBufferEntry,
) -> KernelDiskConflictFileSnapshot {
    let project_root = Path::new(&store.project_root);
    let disk = match read_project_disk_text_snapshot(
        project_root,
        &entry.relative_path,
        &store.limits,
    ) {
        ProjectDiskTextReadOutcome::Missing => {
            return file_snapshot(
                entry,
                None,
                KernelDiskConflictKind::MissingOnDisk,
                "Fișierul urmărit lipsește de pe disk față de baseline-ul sesiunii.",
            );
        }
        ProjectDiskTextReadOutcome::NotFile => {
            return file_snapshot(
                entry,
                None,
                KernelDiskConflictKind::NotFile,
                "Path-ul urmărit nu mai este fișier text.",
            );
        }
        ProjectDiskTextReadOutcome::Oversized(size) => {
            return file_snapshot(
                entry,
                None,
                KernelDiskConflictKind::Oversized,
                format!(
                    "Fișierul de pe disk are {size} bytes, peste limita FileBufferStore de {} bytes.",
                    store.limits.max_file_bytes
                ),
            );
        }
        ProjectDiskTextReadOutcome::InvalidPath(error)
        | ProjectDiskTextReadOutcome::UnsafePath(error) => {
            return file_snapshot(
                entry,
                None,
                KernelDiskConflictKind::InvalidPath,
                format!("Path-ul urmărit nu poate fi citit în boundary-ul proiectului: {error}"),
            );
        }
        ProjectDiskTextReadOutcome::Unstable(error)
        | ProjectDiskTextReadOutcome::Unreadable(error) => {
            return file_snapshot(
                entry,
                None,
                KernelDiskConflictKind::Unreadable,
                format!(
                    "Fișierul de pe disk nu poate fi citit ca text pentru conflict check: {error}"
                ),
            );
        }
        ProjectDiskTextReadOutcome::Loaded(snapshot) => snapshot.baseline,
    };

    if disk.readonly {
        return file_snapshot(
            entry,
            Some(disk),
            KernelDiskConflictKind::Readonly,
            "Fișierul de pe disk este readonly; Save Engine ar bloca scrierea.",
        );
    }

    if disk.hash != entry.baseline.hash {
        return file_snapshot(
            entry,
            Some(disk),
            KernelDiskConflictKind::DiskChanged,
            "Conținutul de pe disk diferă de baseline-ul FileBufferStore.",
        );
    }

    if disk.modified_ms != entry.baseline.modified_ms || disk.size != entry.baseline.size {
        return file_snapshot(
            entry,
            Some(disk),
            KernelDiskConflictKind::MetadataChanged,
            "Metadata disk diferă, dar hash-ul text este identic cu baseline-ul.",
        );
    }

    if entry.is_dirty() {
        return file_snapshot(
            entry,
            Some(disk),
            KernelDiskConflictKind::DirtyOnly,
            "Există draft în memorie, iar disk-ul este încă la baseline-ul sesiunii.",
        );
    }

    file_snapshot(
        entry,
        Some(disk),
        KernelDiskConflictKind::Clean,
        "Disk-ul corespunde baseline-ului FileBufferStore.",
    )
}

fn file_snapshot(
    entry: &FileBufferEntry,
    disk: Option<FileBufferBaseline>,
    kind: KernelDiskConflictKind,
    message: impl Into<String>,
) -> KernelDiskConflictFileSnapshot {
    KernelDiskConflictFileSnapshot {
        relative_path: entry.relative_path.clone(),
        absolute_path: entry.absolute_path.clone(),
        language: entry.language,
        role: entry.role,
        status: status_for_kind(kind),
        kind,
        message: message.into(),
        baseline: entry.baseline.clone(),
        disk,
        has_draft: entry.draft.is_some(),
        dirty: entry.is_dirty(),
        revision: entry.revision,
    }
}

fn summarize_files(files: &[KernelDiskConflictFileSnapshot]) -> KernelDiskConflictSummary {
    let clean_count = count_kind(files, KernelDiskConflictKind::Clean);
    let dirty_only_count = count_kind(files, KernelDiskConflictKind::DirtyOnly);
    let metadata_changed_count = count_kind(files, KernelDiskConflictKind::MetadataChanged);
    let disk_changed_count = count_kind(files, KernelDiskConflictKind::DiskChanged);
    let missing_on_disk_count = count_kind(files, KernelDiskConflictKind::MissingOnDisk);
    let readonly_count = count_kind(files, KernelDiskConflictKind::Readonly);
    let not_file_count = count_kind(files, KernelDiskConflictKind::NotFile);
    let oversized_count = count_kind(files, KernelDiskConflictKind::Oversized);
    let unreadable_count = count_kind(files, KernelDiskConflictKind::Unreadable);
    let invalid_path_count = count_kind(files, KernelDiskConflictKind::InvalidPath);
    let conflict_count = disk_changed_count
        + missing_on_disk_count
        + readonly_count
        + not_file_count
        + oversized_count
        + unreadable_count
        + invalid_path_count;
    let blocking_count = conflict_count;
    let status = if not_file_count + oversized_count + unreadable_count + invalid_path_count > 0 {
        KernelDiskConflictStatus::Error
    } else if disk_changed_count + missing_on_disk_count + readonly_count > 0 {
        KernelDiskConflictStatus::Warning
    } else if dirty_only_count + metadata_changed_count > 0 {
        KernelDiskConflictStatus::Info
    } else {
        KernelDiskConflictStatus::Clean
    };

    KernelDiskConflictSummary {
        status,
        verdict_reason: verdict_reason(
            files.len(),
            conflict_count,
            dirty_only_count,
            metadata_changed_count,
            status,
        ),
        tracked_file_count: files.len(),
        clean_count,
        dirty_only_count,
        metadata_changed_count,
        disk_changed_count,
        missing_on_disk_count,
        readonly_count,
        not_file_count,
        oversized_count,
        unreadable_count,
        invalid_path_count,
        conflict_count,
        blocking_count,
    }
}

fn verdict_reason(
    tracked_count: usize,
    conflict_count: usize,
    dirty_only_count: usize,
    metadata_changed_count: usize,
    status: KernelDiskConflictStatus,
) -> String {
    if tracked_count == 0 {
        return "FileBufferStore nu urmărește încă fișiere pentru conflict check.".to_string();
    }
    if matches!(status, KernelDiskConflictStatus::Error) {
        return format!("{conflict_count} fișiere nu pot fi verificate sigur față de disk.");
    }
    if matches!(status, KernelDiskConflictStatus::Warning) {
        return format!("{conflict_count} fișiere diferă de baseline sau ar bloca Save Engine.");
    }
    if dirty_only_count > 0 || metadata_changed_count > 0 {
        return format!(
            "{dirty_only_count} drafturi locale și {metadata_changed_count} schimbări metadata fără conflict de hash."
        );
    }
    format!("{tracked_count} fișiere urmărite sunt aliniate cu disk-ul.")
}

fn count_kind(files: &[KernelDiskConflictFileSnapshot], kind: KernelDiskConflictKind) -> usize {
    files.iter().filter(|file| file.kind == kind).count()
}

fn status_for_kind(kind: KernelDiskConflictKind) -> KernelDiskConflictStatus {
    match kind {
        KernelDiskConflictKind::Clean => KernelDiskConflictStatus::Clean,
        KernelDiskConflictKind::DirtyOnly | KernelDiskConflictKind::MetadataChanged => {
            KernelDiskConflictStatus::Info
        }
        KernelDiskConflictKind::DiskChanged
        | KernelDiskConflictKind::MissingOnDisk
        | KernelDiskConflictKind::Readonly => KernelDiskConflictStatus::Warning,
        KernelDiskConflictKind::NotFile
        | KernelDiskConflictKind::Oversized
        | KernelDiskConflictKind::Unreadable
        | KernelDiskConflictKind::InvalidPath => KernelDiskConflictStatus::Error,
    }
}

fn status_rank(status: KernelDiskConflictStatus) -> u8 {
    match status {
        KernelDiskConflictStatus::Clean => 0,
        KernelDiskConflictStatus::Info => 1,
        KernelDiskConflictStatus::Warning => 2,
        KernelDiskConflictStatus::Error => 3,
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, time::UNIX_EPOCH};

    use crate::kernel::file_buffer_store::{
        hash_text, FileBufferBaseline, FileBufferEntry, FileBufferStore, FileBufferStoreLimits,
        TextBufferLanguage, TextBufferRole,
    };

    use super::*;

    #[test]
    fn scan_reports_clean_dirty_and_changed_files() {
        let root = test_root("scan-reports-clean-dirty-and-changed-files");
        write_text(&root, "templates/clean.html", "clean");
        write_text(&root, "templates/dirty.html", "dirty");
        write_text(&root, "templates/changed.html", "disk changed");
        let mut store = store(&root);
        store.insert_loaded_file(entry(&root, "templates/clean.html", "clean"));
        store.insert_loaded_file(entry(&root, "templates/dirty.html", "dirty"));
        store
            .set_draft("templates/dirty.html", "draft".to_string(), 10)
            .unwrap();
        store.insert_loaded_file(entry(&root, "templates/changed.html", "baseline"));

        let snapshot = scan_disk_conflicts(&store);

        assert_eq!(snapshot.summary.tracked_file_count, 3);
        assert_eq!(snapshot.summary.clean_count, 1);
        assert_eq!(snapshot.summary.dirty_only_count, 1);
        assert_eq!(snapshot.summary.disk_changed_count, 1);
        assert_eq!(snapshot.summary.status, KernelDiskConflictStatus::Warning);
        assert!(snapshot.files.iter().any(|file| {
            file.relative_path == "templates/changed.html"
                && file.kind == KernelDiskConflictKind::DiskChanged
        }));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn scan_reports_missing_file_as_conflict() {
        let root = test_root("scan-reports-missing-file-as-conflict");
        let mut store = store(&root);
        store.insert_loaded_file(entry(&root, "templates/missing.html", "baseline"));

        let snapshot = scan_disk_conflicts(&store);

        assert_eq!(snapshot.summary.missing_on_disk_count, 1);
        assert_eq!(snapshot.summary.status, KernelDiskConflictStatus::Warning);
        assert_eq!(
            snapshot.files[0].kind,
            KernelDiskConflictKind::MissingOnDisk
        );

        let _ = fs::remove_dir_all(root);
    }

    fn test_root(name: &str) -> PathBuf {
        let mut root = std::env::temp_dir();
        root.push(format!("pana-studio-disk-conflict-{name}-{}", now_ms()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn store(root: &Path) -> FileBufferStore {
        FileBufferStore::new(
            "session-1",
            root.to_string_lossy(),
            1,
            FileBufferStoreLimits {
                max_files: 20,
                max_file_bytes: 1024,
                max_total_bytes: 4096,
            },
        )
    }

    fn entry(root: &Path, relative_path: &str, baseline_text: &str) -> FileBufferEntry {
        let absolute_path = root.join(relative_path);
        FileBufferEntry {
            relative_path: relative_path.to_string(),
            absolute_path: absolute_path.to_string_lossy().to_string(),
            language: TextBufferLanguage::Html,
            role: TextBufferRole::Template,
            baseline: baseline_from_disk_or_text(root, relative_path, baseline_text),
            baseline_text: baseline_text.to_string(),
            draft: None,
            revision: 1,
        }
    }

    fn baseline(text: &str) -> FileBufferBaseline {
        FileBufferBaseline {
            hash: hash_text(text),
            modified_ms: 0,
            size: text.len() as u64,
            readonly: false,
        }
    }

    fn baseline_from_disk_or_text(
        root: &Path,
        relative_path: &str,
        text: &str,
    ) -> FileBufferBaseline {
        let path = root.join(relative_path);
        let Ok(metadata) = fs::metadata(path) else {
            return baseline(text);
        };
        FileBufferBaseline {
            hash: hash_text(text),
            modified_ms: metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_millis())
                .unwrap_or(0),
            size: metadata.len(),
            readonly: metadata.permissions().readonly(),
        }
    }

    fn write_text(root: &Path, relative_path: &str, text: &str) {
        let path = root.join(relative_path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
}
