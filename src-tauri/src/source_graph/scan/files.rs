use std::{
    collections::{HashMap, HashSet},
    fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
};

use crate::{
    localization::LocalizedDiagnostic,
    source_graph::{
        model::SourceDiagnosticSeverity,
        scan::builder::SourceGraphBuilder,
        zola::{normalize_zola_template_reference, zola_template_name_for_path},
    },
};

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
    file: &str,
    draft_sources: &HashMap<String, String>,
    builder: &mut SourceGraphBuilder,
) -> String {
    if let Some(source) = draft_sources.get(file) {
        return source.clone();
    }

    builder.add_diagnostic(
        SourceDiagnosticSeverity::Error,
        LocalizedDiagnostic::new("source-graph-projection-source-missing")
            .with_argument("path", file.to_string()),
        Some(file.to_string()),
        None,
    );
    String::new()
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
