use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use crate::kernel::write_authority::capability_open_optional_regular_file_readonly_no_follow;
use crate::project::{
    model::ProjectFile, project_disk_metadata_version_token, resolve_project_write_path,
};

use super::{
    classify::classify_project_file,
    hash::hash_text,
    model::{FileBufferBaseline, FileBufferDiagnostic, FileBufferEntry, FileBufferStoreLimits},
};

pub enum LoadTextFileOutcome {
    Loaded(FileBufferEntry),
    Skipped(FileBufferDiagnostic),
}

#[derive(Clone, Debug)]
pub(crate) struct ProjectDiskTextSnapshot {
    pub text: String,
    pub baseline: FileBufferBaseline,
    pub version_token: String,
}

#[derive(Clone, Debug)]
pub(crate) enum ProjectDiskTextReadOutcome {
    Loaded(ProjectDiskTextSnapshot),
    Missing,
    NotFile,
    Oversized(u64),
    InvalidPath(String),
    UnsafePath(String),
    Unstable(String),
    Unreadable(String),
}

/// Reads one already-known project text file without trusting a lexical join.
///
/// The read is bounded even when the file grows after the first metadata call.
/// Symlink components are rejected because a session baseline must never be
/// refreshed from a target outside the project authority root.
pub(crate) fn read_project_disk_text_snapshot(
    project_root: &Path,
    relative_path: &str,
    limits: &FileBufferStoreLimits,
) -> ProjectDiskTextReadOutcome {
    let path = match safe_project_text_path(project_root, relative_path) {
        Ok(path) => path,
        Err(SafeProjectTextPathError::Invalid(message)) => {
            return ProjectDiskTextReadOutcome::InvalidPath(message);
        }
        Err(SafeProjectTextPathError::Unsafe(message)) => {
            return ProjectDiskTextReadOutcome::UnsafePath(message);
        }
    };
    run_after_project_text_path_validation_test_hook();

    let file = match capability_open_optional_regular_file_readonly_no_follow(
        &path,
        "FileBufferStore project text read",
    ) {
        Ok(Some(file)) => file,
        Ok(None) => return ProjectDiskTextReadOutcome::Missing,
        Err(error) => return ProjectDiskTextReadOutcome::UnsafePath(error),
    };
    let metadata_before = match file.metadata() {
        Ok(metadata) => metadata,
        Err(error) => return ProjectDiskTextReadOutcome::Unreadable(error.to_string()),
    };
    if !metadata_before.is_file() {
        return ProjectDiskTextReadOutcome::NotFile;
    }
    if !metadata_has_single_link(&metadata_before) {
        return ProjectDiskTextReadOutcome::UnsafePath(
            "fișierul are mai multe hardlink-uri și nu poate demonstra ownership exclusiv în ProjectRoot"
                .to_string(),
        );
    }
    if metadata_before.len() > limits.max_file_bytes {
        return ProjectDiskTextReadOutcome::Oversized(metadata_before.len());
    }

    let read_limit = limits.max_file_bytes.saturating_add(1);
    let mut bytes = Vec::with_capacity(metadata_before.len().min(read_limit) as usize);
    let mut bounded_reader = (&file).take(read_limit);
    if let Err(error) = bounded_reader.read_to_end(&mut bytes) {
        return ProjectDiskTextReadOutcome::Unreadable(error.to_string());
    }
    if bytes.len() as u64 > limits.max_file_bytes {
        return ProjectDiskTextReadOutcome::Oversized(bytes.len() as u64);
    }

    let metadata_after = match file.metadata() {
        Ok(metadata) => metadata,
        Err(error) => {
            return ProjectDiskTextReadOutcome::Unstable(format!(
                "metadata nu mai poate fi citită după read: {error}"
            ));
        }
    };
    if !same_observed_file_version(&metadata_before, &metadata_after) {
        return ProjectDiskTextReadOutcome::Unstable(
            "fișierul s-a modificat în timpul citirii bounded".to_string(),
        );
    }

    // Re-open the named path through the same fd-relative/no-follow adapter.
    // The descriptor read above is safe even if renamed, but a session
    // baseline is published only while the project name still resolves to the
    // exact inode/version that supplied the bytes.
    let named_file = match capability_open_optional_regular_file_readonly_no_follow(
        &path,
        "FileBufferStore project text postflight",
    ) {
        Ok(Some(file)) => file,
        Ok(None) => {
            return ProjectDiskTextReadOutcome::Unstable(
                "fișierul numit a dispărut după citirea bounded".to_string(),
            );
        }
        Err(error) => return ProjectDiskTextReadOutcome::UnsafePath(error),
    };
    let named_metadata = match named_file.metadata() {
        Ok(metadata) => metadata,
        Err(error) => return ProjectDiskTextReadOutcome::Unstable(error.to_string()),
    };
    if !metadata_has_single_link(&named_metadata)
        || !same_observed_file_version(&metadata_after, &named_metadata)
    {
        return ProjectDiskTextReadOutcome::Unstable(
            "path-ul numit nu mai indică versiunea fișierului citit".to_string(),
        );
    }

    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => {
            return ProjectDiskTextReadOutcome::Unreadable(format!(
                "conținutul nu este UTF-8 valid: {error}"
            ));
        }
    };
    let baseline = baseline_from_metadata(&metadata_after, &text);
    let version_token = project_disk_metadata_version_token(&metadata_after);
    ProjectDiskTextReadOutcome::Loaded(ProjectDiskTextSnapshot {
        text,
        baseline,
        version_token,
    })
}

enum SafeProjectTextPathError {
    Invalid(String),
    Unsafe(String),
}

fn safe_project_text_path(
    project_root: &Path,
    relative_path: &str,
) -> Result<PathBuf, SafeProjectTextPathError> {
    if relative_path.trim().is_empty()
        || relative_path.contains('\0')
        || relative_path.contains('\\')
        || relative_path.starts_with('/')
        || relative_path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == ".." || part.contains(':'))
    {
        return Err(SafeProjectTextPathError::Invalid(format!(
            "path relativ invalid: {relative_path:?}"
        )));
    }

    let path = resolve_project_write_path(project_root, relative_path)
        .map_err(SafeProjectTextPathError::Invalid)?;
    if !path.starts_with(project_root) {
        return Err(SafeProjectTextPathError::Unsafe(
            "path-ul rezolvat iese din root-ul proiectului".to_string(),
        ));
    }

    Ok(path)
}

#[cfg(unix)]
fn metadata_has_single_link(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    metadata.nlink() == 1
}

#[cfg(not(unix))]
fn metadata_has_single_link(_metadata: &fs::Metadata) -> bool {
    true
}

fn same_observed_file_version(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    before.is_file()
        && after.is_file()
        && before.len() == after.len()
        && metadata_modified_ms(before) == metadata_modified_ms(after)
        && project_disk_metadata_version_token(before) == project_disk_metadata_version_token(after)
        && before.permissions().readonly() == after.permissions().readonly()
}

fn metadata_modified_ms(metadata: &fs::Metadata) -> u128 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

#[cfg(test)]
thread_local! {
    static AFTER_PROJECT_TEXT_PATH_VALIDATION_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        std::cell::RefCell::new(None);
}

#[cfg(test)]
fn run_after_project_text_path_validation_test_hook() {
    AFTER_PROJECT_TEXT_PATH_VALIDATION_HOOK.with(|hook| {
        if let Some(hook) = hook.borrow_mut().take() {
            hook();
        }
    });
}

#[cfg(not(test))]
fn run_after_project_text_path_validation_test_hook() {}

#[cfg(test)]
fn with_after_project_text_path_validation_hook<T>(
    hook: impl FnOnce() + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    AFTER_PROJECT_TEXT_PATH_VALIDATION_HOOK.with(|slot| {
        assert!(slot.borrow().is_none());
        *slot.borrow_mut() = Some(Box::new(hook));
    });
    let result = operation();
    AFTER_PROJECT_TEXT_PATH_VALIDATION_HOOK.with(|slot| {
        slot.borrow_mut().take();
    });
    result
}

pub fn load_text_file(
    project_root: &Path,
    file: &ProjectFile,
    limits: &FileBufferStoreLimits,
) -> LoadTextFileOutcome {
    let Some((language, role)) = classify_project_file(file) else {
        return LoadTextFileOutcome::Skipped(FileBufferDiagnostic::warning(
            "not_text_file",
            Some(file.relative_path.clone()),
            "Fișierul nu este text relevant pentru FileBufferStore.",
        ));
    };

    let snapshot = match read_project_disk_text_snapshot(project_root, &file.relative_path, limits)
    {
        ProjectDiskTextReadOutcome::Loaded(snapshot) => snapshot,
        ProjectDiskTextReadOutcome::Missing => {
            return LoadTextFileOutcome::Skipped(FileBufferDiagnostic::warning(
                "open_failed",
                Some(file.relative_path.clone()),
                "Fișierul a dispărut înainte de bootstrap-ul FileBufferStore.",
            ));
        }
        ProjectDiskTextReadOutcome::NotFile => {
            return LoadTextFileOutcome::Skipped(FileBufferDiagnostic::warning(
                "not_file",
                Some(file.relative_path.clone()),
                "Path-ul nu mai este fișier.",
            ));
        }
        ProjectDiskTextReadOutcome::Oversized(bytes) => {
            return LoadTextFileOutcome::Skipped(FileBufferDiagnostic::warning(
                "file_too_large",
                Some(file.relative_path.clone()),
                format!(
                    "Fișierul are {bytes} bytes, peste limita FileBufferStore de {} bytes.",
                    limits.max_file_bytes
                ),
            ));
        }
        ProjectDiskTextReadOutcome::InvalidPath(error) => {
            return LoadTextFileOutcome::Skipped(FileBufferDiagnostic::error(
                "invalid_relative_path",
                Some(file.relative_path.clone()),
                error,
            ));
        }
        ProjectDiskTextReadOutcome::UnsafePath(error) => {
            return LoadTextFileOutcome::Skipped(FileBufferDiagnostic::error(
                "unsafe_project_path",
                Some(file.relative_path.clone()),
                error,
            ));
        }
        ProjectDiskTextReadOutcome::Unstable(error) => {
            return LoadTextFileOutcome::Skipped(FileBufferDiagnostic::warning(
                "unstable_during_read",
                Some(file.relative_path.clone()),
                error,
            ));
        }
        ProjectDiskTextReadOutcome::Unreadable(error) => {
            return LoadTextFileOutcome::Skipped(FileBufferDiagnostic::warning(
                "read_text_failed",
                Some(file.relative_path.clone()),
                error,
            ));
        }
    };
    let path = match resolve_project_write_path(project_root, &file.relative_path) {
        Ok(path) => path,
        Err(error) => {
            return LoadTextFileOutcome::Skipped(FileBufferDiagnostic::error(
                "invalid_relative_path",
                Some(file.relative_path.clone()),
                error,
            ));
        }
    };

    LoadTextFileOutcome::Loaded(FileBufferEntry {
        relative_path: file.relative_path.clone(),
        absolute_path: path.to_string_lossy().to_string(),
        language,
        role,
        baseline: snapshot.baseline,
        baseline_text: snapshot.text,
        draft: None,
        revision: 1,
    })
}

pub fn baseline_from_text_path(path: &Path, text: &str) -> Result<FileBufferBaseline, String> {
    let metadata = fs::metadata(path).map_err(|error| {
        format!(
            "Nu am putut citi metadata pentru {}: {}",
            path.display(),
            error
        )
    })?;
    Ok(baseline_from_metadata(&metadata, text))
}

pub fn project_path(project_root: &Path, relative_path: &str) -> Result<PathBuf, String> {
    resolve_project_write_path(project_root, relative_path)
}

fn baseline_from_metadata(metadata: &fs::Metadata, text: &str) -> FileBufferBaseline {
    FileBufferBaseline {
        hash: hash_text(text),
        modified_ms: metadata_modified_ms(metadata),
        size: metadata.len(),
        readonly: metadata.permissions().readonly(),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::{
        read_project_disk_text_snapshot, with_after_project_text_path_validation_hook,
        ProjectDiskTextReadOutcome,
    };
    use crate::kernel::file_buffer_store::FileBufferStoreLimits;

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    fn test_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "pana-file-buffer-reader-{label}-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn ancestor_swapped_to_outside_symlink_after_validation_is_never_read() {
        use std::os::unix::fs::symlink;

        let workspace = test_root("ancestor-swap");
        let project = workspace.join("project");
        let templates = project.join("templates");
        let held_templates = project.join("templates-held");
        let outside = workspace.join("outside");
        fs::create_dir_all(&templates).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(templates.join("index.html"), "inside").unwrap();
        fs::write(outside.join("index.html"), "outside-secret").unwrap();
        let hook_templates = templates.clone();
        let hook_held = held_templates.clone();
        let hook_outside = outside.clone();

        let outcome = with_after_project_text_path_validation_hook(
            move || {
                fs::rename(&hook_templates, &hook_held).unwrap();
                symlink(&hook_outside, &hook_templates).unwrap();
            },
            || {
                read_project_disk_text_snapshot(
                    &project,
                    "templates/index.html",
                    &FileBufferStoreLimits {
                        max_files: 10,
                        max_file_bytes: 1024,
                        max_total_bytes: 4096,
                    },
                )
            },
        );

        assert!(matches!(outcome, ProjectDiskTextReadOutcome::UnsafePath(_)));
        fs::remove_file(&templates).unwrap();
        fs::rename(&held_templates, &templates).unwrap();
        fs::remove_dir_all(&workspace).unwrap();
    }

    #[test]
    fn hardlinked_project_text_is_rejected_fail_closed() {
        let workspace = test_root("hardlink");
        let project = workspace.join("project");
        let outside = workspace.join("outside.html");
        let target = project.join("templates/index.html");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&outside, "outside-secret").unwrap();
        fs::hard_link(&outside, &target).unwrap();

        let outcome = read_project_disk_text_snapshot(
            &project,
            "templates/index.html",
            &FileBufferStoreLimits {
                max_files: 10,
                max_file_bytes: 1024,
                max_total_bytes: 4096,
            },
        );

        assert!(matches!(outcome, ProjectDiskTextReadOutcome::UnsafePath(_)));
        fs::remove_dir_all(&workspace).unwrap();
    }
}
