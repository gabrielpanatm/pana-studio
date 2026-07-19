use std::path::{Component, Path};

use super::paths::style_root_for_page_stylesheet;

pub(super) fn variables_import_path(
    page_relative_path: &str,
    style_files: impl IntoIterator<Item = String>,
    active_theme: Option<&str>,
) -> Option<String> {
    let style_files = style_files.into_iter().collect::<Vec<_>>();
    let style_root = style_root_for_page_stylesheet(page_relative_path);
    let target = find_variables_partial(&style_files, &style_root).or_else(|| {
        if style_root == "sass" {
            active_theme.and_then(|theme| {
                find_variables_partial(&style_files, &format!("themes/{theme}/sass"))
            })
        } else {
            find_variables_partial(&style_files, "sass")
        }
    })?;
    relative_scss_import_path(Path::new(page_relative_path), Path::new(&target))
}

fn find_variables_partial(style_files: &[String], style_root: &str) -> Option<String> {
    let preferred_names = [
        "_variabile.scss",
        "_variables.scss",
        "variabile.scss",
        "variables.scss",
    ];
    let prefix = format!("{}/", style_root.trim_end_matches('/'));
    style_files
        .iter()
        .filter(|path| path.starts_with(&prefix))
        .filter_map(|path| {
            let name = Path::new(path).file_name()?.to_str()?;
            let preferred_index = preferred_names
                .iter()
                .position(|preferred| preferred == &name)?;
            let depth = path.split('/').count();
            Some(((depth, preferred_index, path.as_str()), path.clone()))
        })
        .min_by(|left, right| left.0.cmp(&right.0))
        .map(|(_, path)| path)
}

pub(super) fn relative_scss_import_path(from_file: &Path, target_file: &Path) -> Option<String> {
    let from_parent = from_file.parent().unwrap_or_else(|| Path::new(""));
    let from_components = clean_components(from_parent);
    let mut target_components = clean_components(target_file);

    if let Some(last) = target_components.last_mut() {
        if let Some(stripped) = last.strip_suffix(".scss") {
            *last = stripped.to_string();
        }
        if let Some(stripped) = last.strip_prefix('_') {
            *last = stripped.to_string();
        }
    }

    let common = from_components
        .iter()
        .zip(target_components.iter())
        .take_while(|(left, right)| left == right)
        .count();
    let mut parts = vec!["..".to_string(); from_components.len().saturating_sub(common)];
    parts.extend(target_components.drain(common..));
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

fn clean_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str().map(|value| value.to_string()),
            _ => None,
        })
        .collect()
}
