use std::path::Path;

use crate::zola_theme::{logical_template_name, template_name_for_path};

pub(crate) fn zola_template_name_for_path(
    zola_root: &Path,
    path: &Path,
    theme_name: Option<&str>,
) -> String {
    template_name_for_path(zola_root, path, theme_name)
}

pub(crate) fn normalize_zola_template_reference(target: &str) -> String {
    logical_template_name(target)
}

pub(crate) fn zola_template_reference_keys(template_name: &str) -> Vec<String> {
    vec![
        template_name.to_string(),
        format!("templates/{template_name}"),
    ]
}

pub(crate) fn rewrite_zola_template_reference(
    original: &str,
    new_name: &str,
) -> Result<String, String> {
    if original.trim() != original {
        return Err(format!(
            "SourceGraphRewrite blocat: referința template '{}' conține spații la margine.",
            original
        ));
    }
    let normalized = original.replace('\\', "/");
    if normalized.starts_with("themes/") {
        return Err(format!(
            "SourceGraphRewrite blocat: referința template '{}' folosește path de theme explicit.",
            original
        ));
    }
    if normalized.starts_with("templates/") {
        Ok(format!("templates/{new_name}"))
    } else {
        Ok(new_name.to_string())
    }
}

pub(crate) fn local_zola_template_project_file_reference(relative_path: &str) -> Option<String> {
    relative_path
        .strip_prefix("sursa/templates/")
        .filter(|name| !name.is_empty() && name.ends_with(".html"))
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn normalizes_zola_template_references() {
        assert_eq!(
            normalize_zola_template_reference("templates/blog/card.html"),
            "blog/card.html"
        );
        assert_eq!(
            normalize_zola_template_reference("themes/base/templates/blog/card.html"),
            "blog/card.html"
        );
    }

    #[test]
    fn builds_zola_template_reference_keys() {
        assert_eq!(
            zola_template_reference_keys("blog/card.html"),
            vec![
                "blog/card.html".to_string(),
                "templates/blog/card.html".to_string()
            ]
        );
    }

    #[test]
    fn computes_zola_template_name_for_local_and_theme_paths() {
        let zola_root = PathBuf::from("/project/sursa");
        assert_eq!(
            zola_template_name_for_path(&zola_root, &zola_root.join("templates/index.html"), None),
            "index.html"
        );
        assert_eq!(
            zola_template_name_for_path(
                &zola_root,
                &zola_root.join("themes/base/templates/partials/header.html"),
                Some("base")
            ),
            "partials/header.html"
        );
    }

    #[test]
    fn rewrites_zola_template_references_preserving_templates_prefix() {
        assert_eq!(
            rewrite_zola_template_reference("blog/card.html", "blog/item.html").as_deref(),
            Ok("blog/item.html")
        );
        assert_eq!(
            rewrite_zola_template_reference("templates/blog/card.html", "blog/item.html")
                .as_deref(),
            Ok("templates/blog/item.html")
        );
        assert!(
            rewrite_zola_template_reference("themes/base/templates/card.html", "card.html")
                .is_err()
        );
    }

    #[test]
    fn maps_local_zola_template_project_files() {
        assert_eq!(
            local_zola_template_project_file_reference("sursa/templates/blog/card.html").as_deref(),
            Some("blog/card.html")
        );
        assert_eq!(
            local_zola_template_project_file_reference(
                "sursa/themes/base/templates/blog/card.html"
            ),
            None
        );
    }
}
