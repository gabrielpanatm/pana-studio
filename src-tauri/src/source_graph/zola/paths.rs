use std::path::{Component, Path};

use percent_encoding::percent_decode_str;

use super::content::validate_safe_zola_reference;

pub(crate) fn internal_content_path(path: &str) -> Option<String> {
    path.strip_prefix("@/")
        .filter(|content_path| !content_path.is_empty() && content_path.ends_with(".md"))
        .map(|content_path| content_path.replace('\\', "/"))
}

pub(crate) fn static_asset_reference(path: &str) -> Option<String> {
    clean_local_asset_reference(path)
}

pub(crate) fn static_asset_reference_from_style(path: &str, source_file: &str) -> Option<String> {
    let trimmed = path.trim();
    let base = if trimmed.starts_with("./") || trimmed.starts_with("../") {
        static_logical_parent(source_file)
    } else {
        None
    };
    normalize_asset_path(trimmed, base.as_deref())
}

pub(crate) fn data_file_reference(path: &str) -> Option<String> {
    let normalized = path.trim().replace('\\', "/");
    if normalized.is_empty()
        || normalized.starts_with("http://")
        || normalized.starts_with("https://")
        || normalized.starts_with("//")
    {
        return None;
    }
    Some(normalized)
}

pub(crate) fn normalize_static_asset_reference(target: &str) -> String {
    normalize_asset_path(target, None).unwrap_or_else(|| target.trim().replace('\\', "/"))
}

pub(crate) fn normalize_zola_data_file_reference(target: &str) -> String {
    target.trim().replace('\\', "/")
}

pub(crate) fn static_asset_reference_keys(logical_path: &str) -> Vec<String> {
    let normalized = normalize_static_asset_reference(logical_path);
    vec![normalized.clone(), format!("static/{normalized}")]
}

fn normalize_asset_path(path: &str, relative_base: Option<&str>) -> Option<String> {
    let normalized = clean_local_asset_reference(path)?;
    let root_relative = normalized.starts_with('/');
    let mut parts = if root_relative {
        Vec::new()
    } else {
        relative_base
            .into_iter()
            .flat_map(|base| base.split('/'))
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    let path = normalized.trim_start_matches('/');
    let path = path.strip_prefix("static/").unwrap_or(path);
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part.to_string()),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn clean_local_asset_reference(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed.starts_with("@/") || trimmed.starts_with("//") {
        return None;
    }
    let without_suffix = trimmed
        .split_once(['?', '#'])
        .map(|(path, _)| path)
        .unwrap_or(trimmed);
    let decoded = percent_decode_str(without_suffix).decode_utf8().ok()?;
    let normalized = decoded.replace('\\', "/");
    let lowercase = normalized.to_ascii_lowercase();
    if lowercase.starts_with("data:")
        || lowercase.starts_with("blob:")
        || lowercase.starts_with("http:")
        || lowercase.starts_with("https:")
        || lowercase.starts_with("mailto:")
        || lowercase.starts_with("tel:")
        || lowercase.starts_with("javascript:")
        || has_uri_scheme(&normalized)
    {
        return None;
    }
    Some(normalized)
}

fn has_uri_scheme(value: &str) -> bool {
    let Some((scheme, _)) = value.split_once(':') else {
        return false;
    };
    !scheme.is_empty()
        && scheme.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphabetic()
                || (index > 0 && (byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'.')))
        })
}

fn static_logical_parent(source_file: &str) -> Option<String> {
    let normalized = source_file.replace('\\', "/");
    let logical = normalized
        .strip_prefix("static/")
        .or_else(|| normalized.split_once("/static/").map(|(_, suffix)| suffix))?;
    logical
        .rsplit_once('/')
        .map(|(parent, _)| parent.to_string())
        .or_else(|| Some(String::new()))
}

pub(crate) fn data_file_reference_keys(logical_path: &str) -> Vec<String> {
    vec![logical_path.to_string()]
}

pub(crate) fn zola_static_asset_reference_for_rewrite(reference: &str) -> Option<String> {
    static_asset_reference(reference).map(|reference| normalize_static_asset_reference(&reference))
}

pub(crate) fn zola_data_file_reference_for_rewrite(reference: &str) -> Option<String> {
    data_file_reference(reference).map(|reference| normalize_zola_data_file_reference(&reference))
}

pub(crate) fn rewrite_zola_static_asset_reference(
    original: &str,
    new_name: &str,
) -> Result<String, String> {
    if original.trim() != original {
        return Err(format!(
            "SourceGraphRewrite blocat: referința asset '{}' conține spații la margine.",
            original
        ));
    }
    let normalized = original.replace('\\', "/");
    if normalized != original {
        return Err(format!(
            "SourceGraphRewrite blocat: referința asset '{}' folosește separatori necanonici.",
            original
        ));
    }
    if normalized.starts_with("@/") || normalized.starts_with('/') {
        return Err(format!(
            "SourceGraphRewrite blocat: referința asset '{}' nu este path static local canonic.",
            original
        ));
    }
    let had_static_prefix = normalized.starts_with("static/");
    let reference = normalized.strip_prefix("static/").unwrap_or(&normalized);
    validate_safe_zola_reference(reference, original, "asset")?;
    validate_safe_zola_reference(new_name, new_name, "asset")?;
    if had_static_prefix {
        Ok(format!("static/{new_name}"))
    } else {
        Ok(new_name.to_string())
    }
}

pub(crate) fn rewrite_zola_data_file_reference(
    original: &str,
    new_name: &str,
) -> Result<String, String> {
    if original.trim() != original {
        return Err(format!(
            "SourceGraphRewrite blocat: referința data '{}' conține spații la margine.",
            original
        ));
    }
    let normalized = original.replace('\\', "/");
    if normalized != original {
        return Err(format!(
            "SourceGraphRewrite blocat: referința data '{}' folosește separatori necanonici.",
            original
        ));
    }
    validate_safe_zola_reference(
        normalized.strip_prefix('/').unwrap_or(&normalized),
        original,
        "data",
    )?;
    validate_safe_zola_reference(
        new_name.strip_prefix('/').unwrap_or(new_name),
        new_name,
        "data",
    )?;
    Ok(new_name.to_string())
}

pub(crate) fn local_static_asset_project_file_reference(relative_path: &str) -> Option<String> {
    relative_path
        .strip_prefix("static/")
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

pub(crate) fn local_zola_data_project_file_reference(relative_path: &str) -> Option<String> {
    let normalized = relative_path.trim().replace('\\', "/");
    let path = Path::new(&normalized);
    if normalized.is_empty()
        || normalized.starts_with("@output/")
        || matches!(normalized.as_str(), "zola.toml" | "config.toml")
        || normalized.starts_with("themes/")
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return None;
    }
    Some(normalized)
}

pub(crate) fn static_asset_logical_path(
    zola_root: &Path,
    path: &Path,
    theme_name: Option<&str>,
) -> Option<String> {
    let static_root = match theme_name {
        Some(theme) => zola_root.join("themes").join(theme).join("static"),
        None => zola_root.join("static"),
    };
    path.strip_prefix(static_root)
        .ok()
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .filter(|relative| !relative.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_static_asset_reference_for_rewrite() {
        assert_eq!(
            zola_static_asset_reference_for_rewrite("static/js/app.js").as_deref(),
            Some("js/app.js")
        );
        assert_eq!(
            zola_static_asset_reference_for_rewrite("@/blog/post.md"),
            None
        );
    }

    #[test]
    fn normalizes_runtime_asset_urls_without_losing_unicode() {
        assert_eq!(
            static_asset_reference("/images/Captur%C4%83%20de%20ecran.png?v=1#preview").as_deref(),
            Some("/images/Captură de ecran.png")
        );
        assert_eq!(static_asset_reference("data:image/png;base64,x"), None);
        assert_eq!(static_asset_reference("blob:local"), None);
        assert_eq!(static_asset_reference("https://example.com/a.png"), None);
    }

    #[test]
    fn resolves_relative_css_asset_against_static_file() {
        assert_eq!(
            static_asset_reference_from_style("../images/a.png", "static/css/site.css").as_deref(),
            Some("images/a.png")
        );
        assert_eq!(
            static_asset_reference_from_style("../images/a.png", "themes/pana/static/css/site.css")
                .as_deref(),
            Some("images/a.png")
        );
    }

    #[test]
    fn rewrites_static_asset_reference_preserving_static_prefix() {
        assert_eq!(
            rewrite_zola_static_asset_reference("js/app.js", "js/main.js").as_deref(),
            Ok("js/main.js")
        );
        assert_eq!(
            rewrite_zola_static_asset_reference("static/js/app.js", "js/main.js").as_deref(),
            Ok("static/js/main.js")
        );
    }

    #[test]
    fn maps_local_static_asset_project_files() {
        assert_eq!(
            local_static_asset_project_file_reference("static/js/app.js").as_deref(),
            Some("js/app.js")
        );
        assert_eq!(
            local_static_asset_project_file_reference("content/blog/post.md"),
            None
        );
    }

    #[test]
    fn rewrites_data_file_reference_under_zola_root() {
        assert_eq!(
            zola_data_file_reference_for_rewrite("date/meniu.toml").as_deref(),
            Some("date/meniu.toml")
        );
        assert_eq!(
            rewrite_zola_data_file_reference("date/meniu.toml", "date/navigatie.toml").as_deref(),
            Ok("date/navigatie.toml")
        );
        assert_eq!(
            rewrite_zola_data_file_reference(
                "static/data/meniu.toml",
                "static/data/navigatie.toml"
            )
            .as_deref(),
            Ok("static/data/navigatie.toml")
        );
    }
}
