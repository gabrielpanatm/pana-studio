use std::path::Path;

use crate::project::model::{ProjectFile, ProjectFileKind, ProjectFileRole};

use super::model::{TextBufferLanguage, TextBufferRole};

pub fn classify_project_file(file: &ProjectFile) -> Option<(TextBufferLanguage, TextBufferRole)> {
    let language = language_for_path(&file.relative_path, &file.kind)?;
    let role = role_for_file(file, language);
    Some((language, role))
}

pub fn language_for_relative_path(relative_path: &str) -> Option<TextBufferLanguage> {
    language_for_path(relative_path, &ProjectFileKind::Other)
}

fn language_for_path(relative_path: &str, kind: &ProjectFileKind) -> Option<TextBufferLanguage> {
    match kind {
        ProjectFileKind::Html => return Some(TextBufferLanguage::Html),
        ProjectFileKind::Md => return Some(TextBufferLanguage::Markdown),
        ProjectFileKind::Css => return Some(TextBufferLanguage::Css),
        ProjectFileKind::Scss => return Some(TextBufferLanguage::Scss),
        ProjectFileKind::Js => return Some(TextBufferLanguage::JavaScript),
        ProjectFileKind::Dir | ProjectFileKind::Image => return None,
        ProjectFileKind::Other => {}
    }

    match extension(relative_path).as_deref() {
        Some("toml") => Some(TextBufferLanguage::Toml),
        Some("json") => Some(TextBufferLanguage::Json),
        Some("yaml" | "yml") => Some(TextBufferLanguage::Yaml),
        Some("txt" | "env" | "mdx" | "tera") => Some(TextBufferLanguage::Plain),
        _ => match file_name(relative_path).as_deref() {
            Some(".env" | "AGENTS.md" | "README.md" | "readme.md") => {
                Some(TextBufferLanguage::Plain)
            }
            _ => None,
        },
    }
}

fn role_for_file(file: &ProjectFile, language: TextBufferLanguage) -> TextBufferRole {
    match file.role {
        ProjectFileRole::Page => TextBufferRole::Page,
        ProjectFileRole::Template => TextBufferRole::Template,
        ProjectFileRole::Style => TextBufferRole::Style,
        ProjectFileRole::Script => TextBufferRole::Script,
        ProjectFileRole::Asset => role_for_asset(&file.relative_path, language),
    }
}

fn role_for_asset(relative_path: &str, language: TextBufferLanguage) -> TextBufferRole {
    if matches!(
        language,
        TextBufferLanguage::Toml | TextBufferLanguage::Json | TextBufferLanguage::Yaml
    ) || matches!(file_name(relative_path).as_deref(), Some(".env"))
    {
        return TextBufferRole::Config;
    }
    if relative_path.contains("/data/") || relative_path.starts_with("data/") {
        return TextBufferRole::Data;
    }
    TextBufferRole::Other
}

fn extension(relative_path: &str) -> Option<String> {
    Path::new(relative_path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
}

fn file_name(relative_path: &str) -> Option<String> {
    Path::new(relative_path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
}
