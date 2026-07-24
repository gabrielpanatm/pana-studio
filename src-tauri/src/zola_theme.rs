use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::zola_links::public_asset_href;
use toml_edit::{value, DocumentMut, Item, Value};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ZolaTemplateOrigin {
    Local,
    Theme(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZolaThemeResolver {
    active_theme: Option<String>,
}

impl ZolaThemeResolver {
    pub fn new(active_theme: Option<String>) -> Self {
        Self { active_theme }
    }

    pub fn for_root(zola_root: &Path) -> Self {
        Self::new(read_active_theme(zola_root))
    }

    pub fn active_theme(&self) -> Option<&str> {
        self.active_theme.as_deref()
    }

    pub fn resolve_template_path(&self, zola_root: &Path, template_name: &str) -> Option<String> {
        let template_name = logical_template_name(template_name);
        if template_name.is_empty() {
            return None;
        }

        let local_rel = format!("templates/{}", template_name);
        if zola_root.join(&local_rel).is_file() {
            return Some(local_rel);
        }

        let theme = self.active_theme.as_ref()?;
        let theme_rel = format!("themes/{}/templates/{}", theme, template_name);
        if zola_root.join(&theme_rel).is_file() {
            Some(theme_rel)
        } else {
            None
        }
    }

    pub fn resolve_template_reference(
        &self,
        zola_root: &Path,
        current_template_path: &str,
        template_reference: &str,
    ) -> Option<String> {
        let template_name = logical_template_name(template_reference);
        if template_name.is_empty() {
            return None;
        }

        let local_rel = format!("templates/{}", template_name);
        if zola_root.join(&local_rel).is_file() {
            return Some(local_rel);
        }

        let theme = theme_name_for_template_path(current_template_path)
            .or_else(|| self.active_theme.clone())?;
        let theme_rel = format!("themes/{}/templates/{}", theme, template_name);
        if zola_root.join(&theme_rel).is_file() {
            Some(theme_rel)
        } else {
            None
        }
    }

    pub fn conventional_style_files_for_template(
        &self,
        template_name: &str,
        origin: &ZolaTemplateOrigin,
        project_relative: bool,
    ) -> Vec<String> {
        conventional_style_files_for_template(template_name, origin, project_relative)
    }

    pub fn conventional_script_files_for_template(
        &self,
        template_name: &str,
        origin: &ZolaTemplateOrigin,
        project_relative: bool,
    ) -> Vec<String> {
        conventional_script_files_for_template(template_name, origin, project_relative)
    }
}

pub fn read_active_theme(zola_root: &Path) -> Option<String> {
    let config_path = zola_config_path(zola_root)?;
    let source = fs::read_to_string(config_path).ok()?;

    active_theme_from_source(&source)
}

pub fn active_theme_from_source(source: &str) -> Option<String> {
    for line in source.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            break;
        }
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() == "theme" {
            return unquote_value(value).filter(|theme| is_safe_theme_directory_name(theme));
        }
    }

    None
}

pub fn set_active_theme_in_source(source: &str, theme_id: &str) -> Result<String, String> {
    if !is_safe_theme_directory_name(theme_id) {
        return Err(format!(
            "Configurația Zola a refuzat ID-ul de temă nesigur `{theme_id}`."
        ));
    }
    let mut document = source
        .parse::<DocumentMut>()
        .map_err(|error| format!("Configurația Zola nu este TOML valid: {error}."))?;
    match document.get_mut("theme") {
        Some(Item::Value(Value::String(current))) => {
            let decor = current.decor().clone();
            let mut replacement = toml_edit::Formatted::new(theme_id.to_string());
            *replacement.decor_mut() = decor;
            *current = replacement;
        }
        Some(_) => {
            return Err(
                "Cheia top-level `theme` există, dar nu este un string Zola valid.".to_string(),
            );
        }
        None => {
            document["theme"] = value(theme_id);
        }
    }
    Ok(document.to_string())
}

pub fn is_template_relative_path(relative_path: &str) -> bool {
    validate_template_relative_path(relative_path).unwrap_or(false)
}

pub fn validate_template_relative_path(relative_path: &str) -> Result<bool, String> {
    let normalized = relative_path.trim().replace('\\', "/");
    let normalized = normalized.as_str();

    if normalized.is_empty() {
        return Ok(false);
    }
    if normalized.starts_with('/') || has_windows_absolute_prefix(normalized) {
        return Err(format!(
            "Path template Zola invalid: `{relative_path}` este absolut."
        ));
    }
    if normalized.contains('\0') {
        return Err("Path template Zola invalid: conține NUL.".to_string());
    }

    let segments = normalized.split('/').collect::<Vec<_>>();
    if segments
        .iter()
        .any(|segment| segment.is_empty() || matches!(*segment, "." | ".."))
    {
        return Err(format!(
            "Path template Zola invalid: `{relative_path}` conține segmente de traversare sau goale."
        ));
    }

    if segments.first() == Some(&"templates") {
        return Ok(segments.len() >= 2);
    }
    if segments.first() == Some(&"themes") {
        if segments.len() < 2 || !is_safe_theme_directory_name(segments[1]) {
            return Err(format!(
                "Path template Zola invalid: `{relative_path}` nu are o temă sigură."
            ));
        }
        return Ok(segments.len() >= 4 && segments[2] == "templates");
    }

    Ok(false)
}

pub fn normalized_zola_path(path: &str) -> String {
    path.trim()
        .trim_start_matches('/')
        .trim_start_matches("")
        .replace('\\', "/")
}

pub fn zola_path_without_theme_root(path: &str) -> String {
    let normalized = normalized_zola_path(path);
    theme_asset_parts(&normalized)
        .map(|(_theme, rest)| rest)
        .unwrap_or(normalized)
}

pub fn theme_asset_parts(path: &str) -> Option<(String, String)> {
    let normalized = normalized_zola_path(path);
    normalized
        .strip_prefix("themes/")
        .and_then(|path| path.split_once('/'))
        .and_then(|(theme, rest)| {
            if theme.is_empty() || rest.is_empty() {
                None
            } else {
                Some((theme.to_string(), rest.to_string()))
            }
        })
}

pub fn logical_template_name(template_path: &str) -> String {
    let normalized = normalized_zola_path(template_path);
    if let Some((_theme, template)) = theme_template_parts(&normalized) {
        return template.to_string();
    }
    normalized.trim_start_matches("templates/").to_string()
}

pub fn template_name_for_path(zola_root: &Path, path: &Path, theme_name: Option<&str>) -> String {
    let template_root = theme_name
        .map(|theme| zola_root.join("themes").join(theme).join("templates"))
        .unwrap_or_else(|| zola_root.join("templates"));
    path.strip_prefix(template_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_string()
}

pub fn theme_name_for_template_path(template_path: &str) -> Option<String> {
    let normalized = normalized_zola_path(template_path);
    theme_template_parts(&normalized).map(|(theme, _template)| theme.to_string())
}

pub fn template_to_page_slug(template_path: &str) -> String {
    let normalized = logical_template_name(template_path)
        .trim_end_matches(".html")
        .replace('/', "-")
        .replace('_', "-");
    if normalized.is_empty() {
        "index".to_string()
    } else {
        normalized
    }
}

pub fn page_scss_relative_path(template_path: &str) -> String {
    format!(
        "{}/pagini/{}.scss",
        style_root_for_template_path(template_path),
        template_to_page_slug(template_path)
    )
}

pub fn page_css_href(template_path: &str) -> String {
    public_asset_href(&format!(
        "pagini/{}.css",
        template_to_page_slug(template_path)
    ))
}

pub fn supports_page_css(template_path: &str) -> bool {
    let template = logical_template_name(template_path);
    !template.starts_with("partials/") && !template.starts_with("macros/")
}

pub fn style_root_for_template_path(template_path: &str) -> String {
    let normalized = normalized_zola_path(template_path);
    if let Some((theme, _template)) = theme_template_parts(&normalized) {
        return format!("themes/{}/sass", theme);
    }
    "sass".to_string()
}

pub fn style_root_for_page_stylesheet(relative_path: &str) -> String {
    let normalized = normalized_zola_path(relative_path);
    let Some(rest) = normalized.strip_prefix("themes/") else {
        return "sass".to_string();
    };
    let Some((theme, theme_relative)) = rest.split_once('/') else {
        return "sass".to_string();
    };
    if theme_relative == "sass" || theme_relative.starts_with("sass/") {
        format!("themes/{}/sass", theme)
    } else {
        "sass".to_string()
    }
}

pub fn conventional_style_files_for_template(
    template_name: &str,
    origin: &ZolaTemplateOrigin,
    project_relative: bool,
) -> Vec<String> {
    let style_root = style_root_for_origin(origin, project_relative);
    let name = logical_template_name(template_name)
        .trim_end_matches(".html")
        .to_string();

    if let Some(partial_name) = name.strip_prefix("partials/") {
        let partial_name = partial_name.trim_start_matches('/');
        return conventional_partial_style_files(&style_root, "partials", partial_name);
    }

    if let Some(component_name) = name.strip_prefix("components/") {
        let component_name = component_name.trim_start_matches('/');
        return conventional_partial_style_files(&style_root, "componente", component_name);
    }

    vec![format!("{}/pagini/{}.scss", style_root, name)]
}

pub fn conventional_script_files_for_template(
    template_name: &str,
    origin: &ZolaTemplateOrigin,
    project_relative: bool,
) -> Vec<String> {
    let script_root = script_root_for_origin(origin, project_relative);
    let name = logical_template_name(template_name)
        .trim_end_matches(".html")
        .to_string();
    let component_name = name
        .strip_prefix("partials/")
        .or_else(|| name.strip_prefix("components/"))
        .unwrap_or(&name)
        .trim_start_matches('/');

    if component_name.is_empty() {
        Vec::new()
    } else {
        vec![format!("{}/js/{}.js", script_root, component_name)]
    }
}

fn conventional_partial_style_files(
    style_root: &str,
    scope: &str,
    component_name: &str,
) -> Vec<String> {
    let path = Path::new(component_name);
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return Vec::new();
    };
    let directory = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .and_then(Path::to_str)
        .map(|parent| format!("/{parent}"))
        .unwrap_or_default();
    let root = format!("{style_root}/{scope}{directory}");

    vec![
        format!("{root}/_{file_name}.scss"),
        format!("{root}/{file_name}.scss"),
    ]
}

fn style_root_for_origin(origin: &ZolaTemplateOrigin, _project_relative: bool) -> String {
    match origin {
        ZolaTemplateOrigin::Local => "sass".to_string(),
        ZolaTemplateOrigin::Theme(theme) => format!("themes/{}/sass", theme),
    }
}

fn script_root_for_origin(origin: &ZolaTemplateOrigin, _project_relative: bool) -> String {
    match origin {
        ZolaTemplateOrigin::Local => "static".to_string(),
        ZolaTemplateOrigin::Theme(theme) => format!("themes/{}/static", theme),
    }
}

fn zola_config_path(zola_root: &Path) -> Option<PathBuf> {
    let zola = zola_root.join("zola.toml");
    if zola.is_file() {
        return Some(zola);
    }
    let config = zola_root.join("config.toml");
    if config.is_file() {
        Some(config)
    } else {
        None
    }
}

fn theme_template_parts(path: &str) -> Option<(&str, &str)> {
    let path = path.strip_prefix("themes/")?;
    let (theme, rest) = path.split_once('/')?;
    let template = rest.strip_prefix("templates/")?;
    if !is_safe_theme_directory_name(theme)
        || template.is_empty()
        || template
            .split('/')
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
    {
        None
    } else {
        Some((theme, template))
    }
}

pub(crate) fn is_safe_theme_directory_name(theme: &str) -> bool {
    !theme.is_empty()
        && !matches!(theme, "." | "..")
        && !theme.contains(['/', '\\', ':', '\0'])
        && !has_windows_absolute_prefix(theme)
}

fn has_windows_absolute_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/'
}

fn unquote_value(value: &str) -> Option<String> {
    let value = strip_inline_comment(value)
        .trim()
        .trim_end_matches(',')
        .trim();
    let quoted = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        });
    let result = quoted.unwrap_or(value).trim();
    if result.is_empty() {
        None
    } else {
        Some(result.to_string())
    }
}

fn strip_inline_comment(value: &str) -> &str {
    let mut quote = None;
    let mut escaped = false;

    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' && quote == Some('"') {
            escaped = true;
            continue;
        }
        if matches!(character, '"' | '\'') {
            if quote == Some(character) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(character);
            }
            continue;
        }
        if character == '#' && quote.is_none() {
            return &value[..index];
        }
    }

    value
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    #[test]
    fn resolves_local_only_templates_and_styles() {
        let root = unique_test_dir("local-only");
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(
            root.join("zola.toml"),
            "base_url = \"http://example.test\"\n",
        )
        .unwrap();
        fs::write(root.join("templates/base.html"), "").unwrap();

        let resolver = ZolaThemeResolver::for_root(&root);

        assert_eq!(resolver.active_theme(), None);
        assert_eq!(
            resolver
                .resolve_template_path(&root, "base.html")
                .as_deref(),
            Some("templates/base.html")
        );
        assert_eq!(
            page_scss_relative_path("templates/index.html"),
            "sass/pagini/index.scss"
        );
        cleanup(&root);
    }

    #[test]
    fn resolves_theme_only_templates_and_styles() {
        let root = unique_test_dir("theme-only");
        fs::create_dir_all(root.join("themes/test-theme/templates")).unwrap();
        fs::write(
            root.join("zola.toml"),
            "base_url = \"http://example.test\"\ntheme = \"test-theme\"\n",
        )
        .unwrap();
        fs::write(root.join("themes/test-theme/templates/base.html"), "").unwrap();

        let resolver = ZolaThemeResolver::for_root(&root);

        assert_eq!(resolver.active_theme(), Some("test-theme"));
        assert_eq!(
            resolver
                .resolve_template_path(&root, "base.html")
                .as_deref(),
            Some("themes/test-theme/templates/base.html")
        );
        assert_eq!(
            page_scss_relative_path("themes/test-theme/templates/index.html"),
            "themes/test-theme/sass/pagini/index.scss"
        );
        cleanup(&root);
    }

    #[test]
    fn reads_theme_with_inline_comment() {
        let root = unique_test_dir("theme-inline-comment");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("zola.toml"),
            "base_url = \"http://example.test\"\ntheme = \"test-theme\" # local starter\n",
        )
        .unwrap();

        let resolver = ZolaThemeResolver::for_root(&root);

        assert_eq!(resolver.active_theme(), Some("test-theme"));
        cleanup(&root);
    }

    #[test]
    fn parses_active_theme_from_current_source() {
        let source = r#"
base_url = "http://example.test"
theme = "test-theme" # local starter

[extra]
theme = "ignored-after-table"
"#;

        assert_eq!(
            active_theme_from_source(source),
            Some("test-theme".to_string())
        );
    }

    #[test]
    fn rejects_unsafe_active_theme_paths() {
        assert_eq!(active_theme_from_source("theme = \"../outside\"\n"), None);
        assert_eq!(active_theme_from_source("theme = \"nested/theme\"\n"), None);
        assert_eq!(
            active_theme_from_source("theme = \"nested\\\\theme\"\n"),
            None
        );
        assert_eq!(active_theme_from_source("theme = \"C:\"\n"), None);
    }

    #[test]
    fn validates_template_paths_component_by_component() {
        assert_eq!(
            validate_template_relative_path("templates/pages/index.html"),
            Ok(true)
        );
        assert_eq!(
            validate_template_relative_path("themes/test-theme/templates/index.html"),
            Ok(true)
        );
        assert_eq!(
            validate_template_relative_path("static/js/site.js"),
            Ok(false)
        );
        assert!(validate_template_relative_path("templates/new/../../../escape.html").is_err());
        assert!(validate_template_relative_path("templates\\new\\..\\escape.html").is_err());
        assert!(validate_template_relative_path("/templates/index.html").is_err());
        assert!(validate_template_relative_path("C:\\templates\\index.html").is_err());
        assert!(validate_template_relative_path("themes/../templates/index.html").is_err());
    }

    #[test]
    fn prefers_local_template_over_theme_template() {
        let root = unique_test_dir("local-override");
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::create_dir_all(root.join("themes/test-theme/templates")).unwrap();
        fs::write(
            root.join("zola.toml"),
            "base_url = \"http://example.test\"\ntheme = \"test-theme\"\n",
        )
        .unwrap();
        fs::write(root.join("templates/base.html"), "").unwrap();
        fs::write(root.join("themes/test-theme/templates/base.html"), "").unwrap();

        let resolver = ZolaThemeResolver::for_root(&root);

        assert_eq!(
            resolver
                .resolve_template_reference(
                    &root,
                    "themes/test-theme/templates/index.html",
                    "base.html"
                )
                .as_deref(),
            Some("templates/base.html")
        );
        cleanup(&root);
    }

    #[test]
    fn ignores_theme_templates_when_theme_is_removed() {
        let root = unique_test_dir("theme-removed");
        fs::create_dir_all(root.join("themes/test-theme/templates")).unwrap();
        fs::write(
            root.join("zola.toml"),
            "base_url = \"http://example.test\"\n",
        )
        .unwrap();
        fs::write(root.join("themes/test-theme/templates/base.html"), "").unwrap();

        let resolver = ZolaThemeResolver::for_root(&root);

        assert_eq!(resolver.active_theme(), None);
        assert_eq!(resolver.resolve_template_path(&root, "base.html"), None);
        cleanup(&root);
    }

    #[test]
    fn builds_project_relative_style_paths_for_template_origin() {
        let resolver = ZolaThemeResolver::new(Some("test-theme".to_string()));

        assert_eq!(
            resolver.conventional_style_files_for_template(
                "index.html",
                &ZolaTemplateOrigin::Theme("test-theme".to_string()),
                true,
            ),
            vec!["themes/test-theme/sass/pagini/index.scss"]
        );
        assert_eq!(
            resolver.conventional_style_files_for_template(
                "partials/header.html",
                &ZolaTemplateOrigin::Local,
                true,
            ),
            vec!["sass/partials/_header.scss", "sass/partials/header.scss"]
        );
        assert_eq!(
            resolver.conventional_style_files_for_template(
                "partials/product/card.html",
                &ZolaTemplateOrigin::Local,
                true,
            ),
            vec![
                "sass/partials/product/_card.scss",
                "sass/partials/product/card.scss"
            ]
        );
    }

    #[test]
    fn builds_project_relative_script_paths_for_nested_components() {
        let resolver = ZolaThemeResolver::new(Some("test-theme".to_string()));

        assert_eq!(
            resolver.conventional_script_files_for_template(
                "partials/product/card.html",
                &ZolaTemplateOrigin::Local,
                true,
            ),
            vec!["static/js/product/card.js"]
        );
        assert_eq!(
            resolver.conventional_script_files_for_template(
                "partials/product/card.html",
                &ZolaTemplateOrigin::Theme("test-theme".to_string()),
                true,
            ),
            vec!["themes/test-theme/static/js/product/card.js"]
        );
    }

    #[test]
    fn strips_theme_root_from_zola_asset_paths() {
        assert_eq!(
            zola_path_without_theme_root("themes/test-theme/sass/pagini/index.scss"),
            "sass/pagini/index.scss"
        );
        assert_eq!(
            zola_path_without_theme_root("sass/pagini/index.scss"),
            "sass/pagini/index.scss"
        );
        assert_eq!(
            theme_asset_parts("themes/test-theme/static/css/site.css"),
            Some(("test-theme".to_string(), "static/css/site.css".to_string()))
        );
    }

    fn cleanup(root: &Path) {
        let _ = fs::remove_dir_all(root);
    }

    fn unique_test_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("panastudio-zola-theme-{label}-{nanos}"))
    }
}
