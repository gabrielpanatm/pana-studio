use std::{
    collections::{HashMap, HashSet},
    fs,
    hash::{Hash, Hasher},
    io::ErrorKind,
    path::{Path, PathBuf},
};

use crate::project_model::model::{ProjectModelFile, ProjectModelFileKind};

const TEXT_EXTENSIONS: &[&str] = &[
    "html", "md", "toml", "scss", "css", "js", "json", "xml", "txt", "yml", "yaml", "svg",
];
const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "public",
    "export",
    ".svelte-kit",
];

pub(super) fn collect_project_model_files(
    project_root: &Path,
    zola_root: &Path,
    draft_sources: &HashMap<String, String>,
    deleted_sources: &HashSet<String>,
) -> Result<Vec<ProjectModelFile>, String> {
    let mut paths = Vec::new();
    if require_safe_model_root(zola_root)? {
        collect_text_paths(zola_root, &mut paths)?;
    }

    let mut seen = HashSet::new();
    let mut files = Vec::new();

    for path in paths {
        let relative_path = relative_project_path(project_root, &path);
        if deleted_sources.contains(&relative_path) {
            continue;
        }
        seen.insert(relative_path.clone());
        let disk_contents = fs::read_to_string(&path).map_err(|error| {
            format!("ProjectModel a refuzat fișierul ilizibil {relative_path}: {error}")
        })?;
        let (contents, from_draft) = match draft_sources.get(&relative_path) {
            Some(draft) => (draft.clone(), true),
            None => (disk_contents, false),
        };
        files.push(project_model_file(relative_path, contents, from_draft));
    }

    for (relative_path, contents) in draft_sources {
        if seen.contains(relative_path) || deleted_sources.contains(relative_path) {
            continue;
        }
        if !relative_path.starts_with("sursa/") {
            continue;
        }
        files.push(project_model_file(
            relative_path.clone(),
            contents.clone(),
            true,
        ));
    }

    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(files)
}

pub(super) fn collect_project_model_files_from_workspace_sources(
    source_texts: &HashMap<String, String>,
    deleted_sources: &HashSet<String>,
    changed_paths: &HashSet<String>,
) -> Result<Vec<ProjectModelFile>, String> {
    require_safe_workspace_paths(source_texts.keys().chain(deleted_sources.iter()))?;

    let mut files = source_texts
        .iter()
        .filter(|(relative_path, _)| {
            relative_path.starts_with("sursa/")
                && !deleted_sources.contains(*relative_path)
                && Path::new(relative_path)
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| {
                        TEXT_EXTENSIONS
                            .iter()
                            .any(|allowed| extension.eq_ignore_ascii_case(allowed))
                    })
        })
        .map(|(relative_path, contents)| {
            project_model_file(
                relative_path.clone(),
                contents.clone(),
                changed_paths.contains(relative_path),
            )
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(files)
}

fn require_safe_workspace_paths<'a>(paths: impl Iterator<Item = &'a String>) -> Result<(), String> {
    for relative_path in paths {
        let normalized = relative_path.replace('\\', "/");
        let path = Path::new(&normalized);
        if path.is_absolute()
            || path
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            return Err(format!(
                "ProjectModel a refuzat path-ul workspace nesigur {relative_path}."
            ));
        }
    }
    Ok(())
}

fn require_safe_model_root(root: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(format!(
            "ProjectModel a refuzat root-ul symlink {}.",
            root.display()
        )),
        Ok(metadata) => Ok(metadata.is_dir()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "ProjectModel nu a putut inspecta root-ul {}: {error}",
            root.display()
        )),
    }
}

pub(super) fn model_revision(files: &[ProjectModelFile]) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for file in files {
        file.relative_path.hash(&mut hasher);
        file.revision.hash(&mut hasher);
        file.from_draft.hash(&mut hasher);
    }
    format!("pm_{:016x}", hasher.finish())
}

fn collect_text_paths(root: &Path, paths: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(root).map_err(|error| {
        format!(
            "ProjectModel nu a putut citi folderul {}: {error}",
            root.display()
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "ProjectModel nu a putut citi o intrare din {}: {error}",
                root.display()
            )
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "ProjectModel nu a putut citi tipul intrării {}: {error}",
                path.display()
            )
        })?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("");
            if SKIP_DIRS.iter().any(|skip| name.eq_ignore_ascii_case(skip)) {
                continue;
            }
            collect_text_paths(&path, paths)?;
        } else if file_type.is_file() && is_text_path(&path) {
            paths.push(path);
        }
    }
    Ok(())
}

fn is_text_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            TEXT_EXTENSIONS
                .iter()
                .any(|allowed| extension.eq_ignore_ascii_case(allowed))
        })
        .unwrap_or(false)
}

fn project_model_file(
    relative_path: String,
    contents: String,
    from_draft: bool,
) -> ProjectModelFile {
    let size_bytes = contents.len();
    let revision = content_revision(&contents);
    ProjectModelFile {
        kind: file_kind(&relative_path),
        relative_path,
        contents,
        size_bytes,
        revision,
        from_draft,
    }
}

fn content_revision(contents: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    contents.hash(&mut hasher);
    format!("f_{:016x}", hasher.finish())
}

fn file_kind(relative_path: &str) -> ProjectModelFileKind {
    let path = relative_path.replace('\\', "/");
    if path.ends_with("zola.toml") || path.ends_with("config.toml") {
        return ProjectModelFileKind::Config;
    }
    if path.starts_with("sursa/content/") && path.ends_with(".md") {
        return ProjectModelFileKind::Content;
    }
    if path.contains("/templates/") && path.ends_with(".html") {
        return ProjectModelFileKind::Template;
    }
    if path.starts_with("sursa/templates/") && path.ends_with(".html") {
        return ProjectModelFileKind::Template;
    }
    if path.ends_with(".scss") || path.ends_with(".css") {
        return ProjectModelFileKind::Style;
    }
    if path.ends_with(".js") {
        return ProjectModelFileKind::Script;
    }
    if path.starts_with("sursa/data/") || path.starts_with("sursa/date/") {
        return ProjectModelFileKind::Data;
    }
    if path.starts_with("sursa/static/") {
        return ProjectModelFileKind::StaticText;
    }
    ProjectModelFileKind::OtherText
}

fn relative_project_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
