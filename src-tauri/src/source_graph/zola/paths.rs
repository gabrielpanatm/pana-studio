use std::path::Path;

use super::content::validate_safe_zola_reference;

pub(crate) fn internal_content_path(path: &str) -> Option<String> {
    path.strip_prefix("@/")
        .filter(|content_path| !content_path.is_empty() && content_path.ends_with(".md"))
        .map(|content_path| content_path.replace('\\', "/"))
}

pub(crate) fn static_asset_reference(path: &str) -> Option<String> {
    let normalized = path.trim().replace('\\', "/");
    if normalized.is_empty()
        || normalized.starts_with("@/")
        || normalized.starts_with("http://")
        || normalized.starts_with("https://")
        || normalized.starts_with("//")
    {
        return None;
    }
    Some(normalized)
}

pub(crate) fn data_file_reference(path: &str) -> Option<String> {
    let normalized = path.trim().replace('\\', "/");
    if normalized.is_empty()
        || normalized.starts_with("@/")
        || normalized.starts_with('/')
        || normalized.starts_with("static/")
        || normalized.starts_with("content/")
        || normalized.starts_with("public/")
        || normalized.starts_with("themes/")
        || normalized.starts_with("http://")
        || normalized.starts_with("https://")
        || normalized.starts_with("//")
    {
        return None;
    }
    Some(normalized)
}

pub(crate) fn normalize_static_asset_reference(target: &str) -> String {
    let normalized = target.trim().replace('\\', "/");
    normalized
        .strip_prefix("static/")
        .unwrap_or(&normalized)
        .to_string()
}

pub(crate) fn normalize_zola_data_file_reference(target: &str) -> String {
    target.trim().replace('\\', "/")
}

pub(crate) fn static_asset_reference_keys(logical_path: &str) -> Vec<String> {
    vec![logical_path.to_string(), format!("static/{logical_path}")]
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
    if normalized.starts_with("@/")
        || normalized.starts_with('/')
        || normalized.starts_with("static/")
        || normalized.starts_with("content/")
        || normalized.starts_with("public/")
        || normalized.starts_with("themes/")
    {
        return Err(format!(
            "SourceGraphRewrite blocat: referința data '{}' nu este path local canonic sub rădăcina Zola.",
            original
        ));
    }
    validate_safe_zola_reference(&normalized, original, "data")?;
    validate_safe_zola_reference(new_name, new_name, "data")?;
    Ok(new_name.to_string())
}

pub(crate) fn local_static_asset_project_file_reference(relative_path: &str) -> Option<String> {
    relative_path
        .strip_prefix("sursa/static/")
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

pub(crate) fn local_zola_data_project_file_reference(relative_path: &str) -> Option<String> {
    relative_path
        .strip_prefix("sursa/date/")
        .filter(|name| !name.is_empty())
        .map(|name| format!("date/{name}"))
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

pub(crate) fn zola_data_file_logical_path(zola_root: &Path, path: &Path) -> Option<String> {
    let data_root = zola_root.join("date");
    path.strip_prefix(data_root)
        .ok()
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .filter(|relative| !relative.is_empty())
        .map(|relative| format!("date/{relative}"))
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
            local_static_asset_project_file_reference("sursa/static/js/app.js").as_deref(),
            Some("js/app.js")
        );
        assert_eq!(
            local_static_asset_project_file_reference("sursa/content/blog/post.md"),
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
        assert!(
            rewrite_zola_data_file_reference("static/data/meniu.toml", "date/meniu.toml").is_err()
        );
    }
}
