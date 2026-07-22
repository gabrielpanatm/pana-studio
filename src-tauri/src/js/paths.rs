use std::path::{Component, Path};

use crate::zola_theme::{template_to_page_slug, validate_template_relative_path};

const PAGE_JS_TEMPLATE_PATH_MAX_BYTES: usize = 4096;

/// Canonicalizes the project-relative Zola source identity used by every
/// Page JS boundary. The returned path is relative to `` and can be
/// joined only after this validation has succeeded.
pub(super) fn normalize_template_path(input: &str) -> Result<String, String> {
    let mut normalized = input.trim().replace('\\', "/");
    while let Some(next) = normalized.strip_prefix("./") {
        normalized = next.to_string();
    }
    if normalized.is_empty() {
        return Err("Page JS a refuzat un template path gol.".to_string());
    }
    if normalized.contains('\0')
        || normalized.starts_with('/')
        || normalized.as_bytes().get(1) == Some(&b':')
    {
        return Err(format!(
            "Page JS a refuzat template path absolut sau invalid: {normalized}."
        ));
    }
    if normalized
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
        || Path::new(&normalized).components().any(|component| {
            matches!(
                component,
                Component::CurDir
                    | Component::ParentDir
                    | Component::RootDir
                    | Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "Page JS a refuzat template path non-canonic sau cu traversal: {normalized}."
        ));
    }

    if normalized.is_empty() || normalized.len() > PAGE_JS_TEMPLATE_PATH_MAX_BYTES {
        return Err(format!(
            "Page JS a refuzat template path invalid sau prea lung: {normalized}."
        ));
    }
    if !validate_template_relative_path(&normalized)? || !normalized.ends_with(".html") {
        return Err(format!(
            "Page JS a refuzat path-ul din afara domeniului template HTML Zola: {normalized}."
        ));
    }
    Ok(normalized)
}

pub fn template_to_slug(template_path: &str) -> String {
    format!("pana-{}", template_to_page_slug(template_path))
}

pub fn js_relative_path(template_path: &str) -> String {
    format!("static/js/{}.js", template_to_slug(template_path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_same_slug_for_local_and_theme_template_paths() {
        assert_eq!(template_to_slug("templates/index.html"), "pana-index");
        assert_eq!(
            template_to_slug("themes/pana-studio/templates/index.html"),
            "pana-index"
        );
        assert_eq!(
            js_relative_path("themes/pana-studio/templates/atelier/home_page.html"),
            "static/js/pana-atelier-home-page.js"
        );
    }

    #[test]
    fn legacy_slug_collision_is_explicitly_guarded_by_save_preflight() {
        assert_eq!(
            js_relative_path("templates/foo/bar.html"),
            js_relative_path("templates/foo-bar.html")
        );
        assert_eq!(
            js_relative_path("templates/foo_bar.html"),
            js_relative_path("templates/foo-bar.html")
        );
    }

    #[test]
    fn template_path_normalization_is_canonical_and_blocks_traversal() {
        assert_eq!(
            normalize_template_path("./templates/index.html").unwrap(),
            "templates/index.html"
        );
        assert!(normalize_template_path("templates/../secret.html").is_err());
        assert!(normalize_template_path(r"templates\..\secret.html").is_err());
        assert!(normalize_template_path("/tmp/secret.html").is_err());
        assert!(normalize_template_path("templates//index.html").is_err());
        assert!(normalize_template_path("config.toml").is_err());
        assert!(normalize_template_path("content/post.md").is_err());
        assert!(normalize_template_path("sass/app.scss").is_err());
        assert!(normalize_template_path("templates/feed.xml").is_err());
        assert_eq!(
            normalize_template_path("themes/pana-studio/templates/index.html").unwrap(),
            "themes/pana-studio/templates/index.html"
        );
    }
}
