use std::path::{Path, PathBuf};

/// The folder selected by the user is the Zola project root.  Keeping this
/// helper avoids duplicating that invariant at call sites which still need to
/// distinguish the logical project root from generated artifacts.
pub fn zola_project_root(project_root: &Path) -> PathBuf {
    project_root.to_path_buf()
}

pub fn strip_zola_root_prefix(relative_path: &str) -> &str {
    relative_path
}

pub fn resolve_project_write_path(root: &Path, relative_path: &str) -> Result<PathBuf, String> {
    let normalized_path = normalize_relative_path(relative_path)?;
    let mut path = root.to_path_buf();

    for segment in normalized_path.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }

        if segment == ".." {
            return Err("Path invalid pentru scriere.".to_string());
        }

        path.push(segment);
    }

    Ok(path)
}

pub(super) fn normalize_relative_path(relative_path: &str) -> Result<String, String> {
    let mut normalized = Vec::new();

    for segment in relative_path.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }

        if segment == ".." {
            return Err("Path invalid pentru proiect.".to_string());
        }

        normalized.push(segment);
    }

    if normalized.is_empty() {
        return Err("Path relativ gol.".to_string());
    }

    Ok(normalized.join("/"))
}

pub(super) fn normalize_optional_relative_path(relative_path: &str) -> Result<String, String> {
    if relative_path.trim().is_empty() {
        return Ok(String::new());
    }

    normalize_relative_path(relative_path)
}
