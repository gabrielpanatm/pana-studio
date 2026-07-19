use std::{
    collections::{HashMap, HashSet},
    fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
};

use crate::source_graph::{
    model::SourceDiagnosticSeverity,
    scan::builder::SourceGraphBuilder,
    zola::{normalize_zola_template_reference, zola_template_name_for_path},
};

pub(super) fn collect_files_with_extension(
    root: &Path,
    extension: &str,
) -> Result<Vec<PathBuf>, String> {
    collect_files_with_extensions(root, &[extension])
}

pub(super) fn collect_files_with_extensions(
    root: &Path,
    extensions: &[&str],
) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    if !require_safe_scan_root(root)? {
        return Ok(files);
    }
    collect_files_recursive(root, extensions, &mut files)?;
    files.sort();
    Ok(files)
}

pub(super) fn collect_all_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    if !require_safe_scan_root(root)? {
        return Ok(files);
    }
    collect_all_files_recursive(root, &mut files)?;
    files.sort();
    Ok(files)
}

pub(super) fn apply_virtual_file_projection(
    project_root: &Path,
    directory_root: &Path,
    extensions: Option<&[&str]>,
    draft_sources: &HashMap<String, String>,
    deleted_sources: &HashSet<String>,
    files: &mut Vec<PathBuf>,
) -> Result<(), String> {
    require_safe_draft_source_paths(draft_sources)?;
    require_safe_deleted_source_paths(deleted_sources)?;
    files.retain(|path| !deleted_sources.contains(&relative_project_path(project_root, path)));
    for relative_path in draft_sources.keys() {
        if deleted_sources.contains(relative_path) {
            continue;
        }
        let candidate = project_root.join(relative_path);
        if !candidate.starts_with(directory_root) {
            continue;
        }
        if let Some(extensions) = extensions {
            let matches = candidate
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    extensions
                        .iter()
                        .any(|allowed| extension.eq_ignore_ascii_case(allowed))
                });
            if !matches {
                continue;
            }
        }
        if !files.contains(&candidate) {
            files.push(candidate);
        }
    }
    files.sort();
    Ok(())
}

/// Validates the complete virtual draft namespace before Source Graph can
/// return early for a non-Zola or partially initialized project. Draft input
/// is an IPC boundary and must never be accepted conditionally on disk shape.
pub(super) fn require_safe_draft_source_paths(
    draft_sources: &HashMap<String, String>,
) -> Result<(), String> {
    for relative_path in draft_sources.keys() {
        let normalized = relative_path.replace('\\', "/");
        let relative = Path::new(&normalized);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(format!(
                "Source Graph a refuzat path-ul draft nesigur {relative_path}."
            ));
        }
    }
    Ok(())
}

pub(super) fn require_safe_deleted_source_paths(
    deleted_sources: &HashSet<String>,
) -> Result<(), String> {
    for relative_path in deleted_sources {
        let normalized = relative_path.replace('\\', "/");
        let relative = Path::new(&normalized);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(format!(
                "Source Graph a refuzat path-ul șters nesigur {relative_path}."
            ));
        }
    }
    Ok(())
}

fn collect_files_recursive(
    root: &Path,
    extensions: &[&str],
    files: &mut Vec<PathBuf>,
) -> Result<(), String> {
    for entry in fs::read_dir(root).map_err(|error| {
        format!(
            "Nu am putut citi folderul pentru Source Graph {}: {}",
            root.to_string_lossy(),
            error
        )
    })? {
        let entry = entry.map_err(|error| format!("Nu am putut citi o intrare: {}", error))?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "Nu am putut citi tipul intrării Source Graph {}: {error}",
                path.display()
            )
        })?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_files_recursive(&path, extensions, files)?;
        } else if file_type.is_file()
            && path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(|extension| {
                    extensions
                        .iter()
                        .any(|allowed| extension.eq_ignore_ascii_case(allowed))
                })
                .unwrap_or(false)
        {
            files.push(path);
        }
    }
    Ok(())
}

fn collect_all_files_recursive(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(root).map_err(|error| {
        format!(
            "Nu am putut citi folderul pentru Source Graph {}: {}",
            root.to_string_lossy(),
            error
        )
    })? {
        let entry = entry.map_err(|error| format!("Nu am putut citi o intrare: {}", error))?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "Nu am putut citi tipul intrării Source Graph {}: {error}",
                path.display()
            )
        })?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_all_files_recursive(&path, files)?;
        } else if file_type.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

pub(super) fn require_safe_scan_root(root: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(format!(
            "Source Graph a refuzat root-ul symlink {}.",
            root.display()
        )),
        Ok(metadata) => Ok(metadata.is_dir()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "Source Graph nu a putut inspecta root-ul {}: {error}",
            root.display()
        )),
    }
}

pub(super) fn read_source(
    path: &Path,
    file: &str,
    draft_sources: &HashMap<String, String>,
    builder: &mut SourceGraphBuilder,
) -> String {
    if let Some(source) = draft_sources.get(file) {
        return source.clone();
    }

    match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            builder.add_diagnostic(
                SourceDiagnosticSeverity::Error,
                format!("Nu am putut citi fișierul: {}", error),
                Some(file.to_string()),
                None,
            );
            String::new()
        }
    }
}

pub(super) fn relative_project_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub(super) fn template_name(zola_root: &Path, path: &Path, theme_name: Option<&str>) -> String {
    zola_template_name_for_path(zola_root, path, theme_name)
}

pub(super) fn normalize_template_name(target: &str) -> String {
    normalize_zola_template_reference(target)
}

#[cfg(all(test, unix))]
mod tests {
    use std::{
        fs,
        os::unix::fs::symlink,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::{collect_all_files, collect_files_with_extension};

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn source_graph_collectors_do_not_follow_symlink_roots_or_entries() {
        let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!(
            "pana-source-graph-no-follow-{}-{sequence}",
            std::process::id()
        ));
        let local = base.join("local");
        let outside = base.join("outside");
        fs::create_dir_all(&local).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(local.join("local.html"), "local").unwrap();
        fs::write(outside.join("secret.html"), "secret").unwrap();
        symlink(&outside, local.join("linked-dir")).unwrap();
        symlink(outside.join("secret.html"), local.join("linked.html")).unwrap();

        let files = collect_files_with_extension(&local, "html").unwrap();
        assert_eq!(files, vec![local.join("local.html")]);
        assert_eq!(collect_all_files(&local).unwrap(), files);

        let linked_root = base.join("linked-root");
        symlink(&outside, &linked_root).unwrap();
        assert!(collect_all_files(&linked_root)
            .unwrap_err()
            .contains("root-ul symlink"));

        fs::remove_dir_all(base).unwrap();
    }
}
