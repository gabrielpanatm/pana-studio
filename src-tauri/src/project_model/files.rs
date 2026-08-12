use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    path::Path,
};

use crate::project_model::model::{ProjectModelFile, ProjectModelFileKind};

const TEXT_EXTENSIONS: &[&str] = &[
    "html", "md", "toml", "scss", "css", "js", "json", "xml", "txt", "yml", "yaml", "svg",
];
pub(super) fn collect_project_model_files_from_workspace_sources(
    source_texts: &HashMap<String, String>,
    deleted_sources: &HashSet<String>,
    changed_paths: &HashSet<String>,
) -> Result<Vec<ProjectModelFile>, String> {
    require_safe_workspace_paths(source_texts.keys().chain(deleted_sources.iter()))?;

    let mut files = source_texts
        .iter()
        .filter(|(relative_path, _)| {
            !deleted_sources.contains(*relative_path)
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

pub(super) fn model_revision(files: &[ProjectModelFile]) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for file in files {
        file.relative_path.hash(&mut hasher);
        file.revision.hash(&mut hasher);
        file.from_draft.hash(&mut hasher);
    }
    format!("pm_{:016x}", hasher.finish())
}

pub(super) fn project_model_file(
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
    if path.starts_with("content/") && path.ends_with(".md") {
        return ProjectModelFileKind::Content;
    }
    if path.contains("/templates/") && path.ends_with(".html") {
        return ProjectModelFileKind::Template;
    }
    if path.starts_with("templates/") && path.ends_with(".html") {
        return ProjectModelFileKind::Template;
    }
    if path.ends_with(".scss") || path.ends_with(".css") {
        return ProjectModelFileKind::Style;
    }
    if path.ends_with(".js") {
        return ProjectModelFileKind::Script;
    }
    if path.starts_with("data/") || path.starts_with("date/") {
        return ProjectModelFileKind::Data;
    }
    if path.starts_with("static/") {
        return ProjectModelFileKind::StaticText;
    }
    ProjectModelFileKind::OtherText
}
