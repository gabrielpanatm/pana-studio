use std::path::Path;

use crate::source_graph::{
    literals::find_first_string_literal,
    model::{SourceGraphPage, SourcePageKind, SourceRelationKind},
};

use super::templates::normalize_zola_template_reference;

#[derive(Default)]
pub(crate) struct ZolaContentFrontmatter {
    pub(crate) title: Option<String>,
    pub(crate) template: Option<String>,
    pub(crate) page_template: Option<String>,
}

pub(crate) fn parse_zola_content_frontmatter(source: &str) -> ZolaContentFrontmatter {
    let trimmed = source.trim_start_matches('\u{feff}');
    let Some(marker) = ["+++", "---"]
        .iter()
        .find(|marker| trimmed.starts_with(**marker))
    else {
        return ZolaContentFrontmatter::default();
    };
    let rest = &trimmed[marker.len()..];
    let Some(end_index) = rest.find(&format!("\n{}", marker)) else {
        return ZolaContentFrontmatter::default();
    };
    let frontmatter = &rest[..end_index];
    ZolaContentFrontmatter {
        title: frontmatter_string_value(frontmatter, "title"),
        template: frontmatter_string_value(frontmatter, "template"),
        page_template: frontmatter_string_value(frontmatter, "page_template"),
    }
}

pub(crate) fn zola_content_page_kind(zola_root: &Path, path: &Path) -> SourcePageKind {
    let content_relative = zola_content_relative_path(zola_root, path);
    if content_relative == "_index.md" {
        SourcePageKind::Home
    } else if content_relative.ends_with("/_index.md") {
        SourcePageKind::Section
    } else {
        SourcePageKind::Page
    }
}

pub(crate) fn resolve_zola_page_template(
    frontmatter_template: &Option<String>,
    page_kind: &SourcePageKind,
) -> Option<String> {
    if let Some(template) = frontmatter_template
        .as_ref()
        .map(|template| normalize_zola_template_reference(template))
        .filter(|template| !template.is_empty())
    {
        return Some(template);
    }

    Some(
        match page_kind {
            SourcePageKind::Home => "index.html",
            SourcePageKind::Section => "section.html",
            SourcePageKind::Page => "page.html",
        }
        .to_string(),
    )
}

pub(crate) fn resolve_zola_section_page_template(
    frontmatter_page_template: &Option<String>,
    page_kind: &SourcePageKind,
) -> Option<String> {
    if !matches!(page_kind, SourcePageKind::Home | SourcePageKind::Section) {
        return None;
    }
    frontmatter_page_template
        .as_ref()
        .map(|template| normalize_zola_template_reference(template))
        .filter(|template| !template.is_empty())
}

pub(crate) fn normalize_zola_content_reference(target: &str) -> String {
    target.trim().replace('\\', "/")
}

pub(crate) fn zola_content_project_file_reference(file: &str) -> Option<String> {
    file.strip_prefix("content/")
        .filter(|path| !path.is_empty() && path.ends_with(".md"))
        .map(str::to_string)
}

pub(crate) fn zola_frontmatter_template_for_key<'a>(
    page: &'a SourceGraphPage,
    frontmatter_key: &str,
) -> Option<&'a String> {
    match frontmatter_key {
        "page_template" => page.frontmatter_page_template.as_ref(),
        _ => page.frontmatter_template.as_ref(),
    }
}

pub(crate) fn find_zola_frontmatter_template_literal(
    source: &str,
    frontmatter_key: &str,
    old_template_name: &str,
) -> Option<(usize, usize, String)> {
    let (frontmatter_start, frontmatter_end) = zola_frontmatter_range(source)?;
    let frontmatter = &source[frontmatter_start..frontmatter_end];
    let toml_prefix = format!("{frontmatter_key} =");
    let yaml_prefix = format!("{frontmatter_key}:");
    let mut offset = frontmatter_start;
    for line in frontmatter.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let indent = line.len().saturating_sub(trimmed.len());
        if trimmed.starts_with('#') || trimmed.is_empty() {
            offset += line.len();
            continue;
        }
        let key_value_start = offset + indent;
        if trimmed.starts_with(&toml_prefix) || trimmed.starts_with(&yaml_prefix) {
            let line_end = offset + line.len();
            if let Some((start, end, value)) =
                find_first_string_literal(source, key_value_start, line_end)
            {
                if normalize_zola_template_reference(&value) == old_template_name {
                    return Some((start, end, value));
                }
            }
        }
        offset += line.len();
    }
    None
}

pub(crate) fn zola_content_reference_for_relation(
    reference: &str,
    relation_kind: &SourceRelationKind,
) -> Option<String> {
    let normalized = normalize_zola_content_reference(reference);
    if relation_kind == &SourceRelationKind::InternalContentLink {
        return normalized
            .strip_prefix("@/")
            .filter(|content_path| !content_path.is_empty() && content_path.ends_with(".md"))
            .map(str::to_string);
    }
    Some(normalized)
}

pub(crate) fn zola_content_load_reference(reference: &str) -> Option<String> {
    let normalized = reference.trim().replace('\\', "/");
    if let Some(content_path) = normalized.strip_prefix("@/") {
        return valid_markdown_content_path(content_path).map(str::to_string);
    }
    if let Some(content_path) = normalized.strip_prefix("/content/") {
        return valid_markdown_content_path(content_path).map(str::to_string);
    }
    if let Some(content_path) = normalized.strip_prefix("content/") {
        return valid_markdown_content_path(content_path).map(str::to_string);
    }
    None
}

pub(crate) fn rewrite_zola_content_reference(
    original: &str,
    new_name: &str,
    relation_kind: &SourceRelationKind,
) -> Result<String, String> {
    if original.trim() != original {
        return Err(format!(
            "SourceGraphRewrite blocat: referința content '{}' conține spații la margine.",
            original
        ));
    }
    let normalized = original.replace('\\', "/");
    if normalized != original {
        return Err(format!(
            "SourceGraphRewrite blocat: referința content '{}' folosește separatori necanonici.",
            original
        ));
    }
    if relation_kind == &SourceRelationKind::InternalContentLink {
        if !normalized.starts_with("@/") {
            return Err(format!(
                "SourceGraphRewrite blocat: referința content '{}' nu este link intern Zola cu prefix @/.",
                original
            ));
        }
        let content_path = normalized.trim_start_matches("@/");
        validate_safe_zola_reference(content_path, original, "content")?;
        return Ok(format!("@/{new_name}"));
    }
    if normalized.starts_with('/')
        || normalized.starts_with("@/")
        || normalized.starts_with("content/")
    {
        return Err(format!(
            "SourceGraphRewrite blocat: referința content '{}' nu este path relativ la content/ în forma canonică Zola pentru get_page/get_section.",
            original
        ));
    }
    validate_safe_zola_reference(&normalized, original, "content")?;
    Ok(new_name.to_string())
}

pub(crate) fn rewrite_zola_content_load_reference(
    original: &str,
    new_name: &str,
) -> Result<String, String> {
    if original.trim() != original {
        return Err(format!(
            "SourceGraphRewrite blocat: referința content load '{}' conține spații la margine.",
            original
        ));
    }
    let normalized = original.replace('\\', "/");
    if normalized != original {
        return Err(format!(
            "SourceGraphRewrite blocat: referința content load '{}' folosește separatori necanonici.",
            original
        ));
    }
    validate_safe_zola_reference(new_name, new_name, "content load")?;
    if let Some(content_path) = normalized.strip_prefix("@/") {
        validate_safe_zola_reference(content_path, original, "content load")?;
        return Ok(format!("@/{new_name}"));
    }
    if let Some(content_path) = normalized.strip_prefix("/content/") {
        validate_safe_zola_reference(content_path, original, "content load")?;
        return Ok(format!("/content/{new_name}"));
    }
    if let Some(content_path) = normalized.strip_prefix("content/") {
        validate_safe_zola_reference(content_path, original, "content load")?;
        return Ok(format!("content/{new_name}"));
    }
    Err(format!(
        "SourceGraphRewrite blocat: referința content load '{}' nu folosește @/ sau content/.",
        original
    ))
}

pub(crate) fn zola_content_url(zola_root: &Path, path: &Path) -> String {
    let content_path = zola_content_relative_path(zola_root, path)
        .trim_end_matches(".md")
        .to_string();
    if content_path == "_index" {
        return "/".to_string();
    }
    if let Some(section_path) = content_path.strip_suffix("/_index") {
        return format!("/{}/", section_path.trim_matches('/'));
    }
    format!("/{}/", content_path.trim_matches('/'))
}

pub(crate) fn zola_content_relative_path(zola_root: &Path, path: &Path) -> String {
    path.strip_prefix(zola_root.join("content"))
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_string()
}

fn valid_markdown_content_path(path: &str) -> Option<&str> {
    (!path.trim().is_empty() && path.ends_with(".md")).then_some(path)
}

pub(crate) fn zola_frontmatter_range(source: &str) -> Option<(usize, usize)> {
    let bom_len = if source.starts_with('\u{feff}') {
        '\u{feff}'.len_utf8()
    } else {
        0
    };
    let without_bom = &source[bom_len..];
    let marker = ["+++", "---"]
        .iter()
        .find(|marker| without_bom.starts_with(**marker))?;
    let body_start = bom_len + marker.len();
    let rest = &source[body_start..];
    let marker_at_line_start = format!("\n{}", marker);
    let end_relative = rest.find(&marker_at_line_start)?;
    Some((body_start, body_start + end_relative))
}

pub(crate) fn validate_safe_zola_reference(
    reference: &str,
    original: &str,
    reference_label: &str,
) -> Result<(), String> {
    if reference
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(format!(
            "SourceGraphRewrite blocat: referința {reference_label} '{}' conține segmente nesigure.",
            original
        ));
    }
    Ok(())
}

fn frontmatter_string_value(frontmatter: &str, key: &str) -> Option<String> {
    for line in frontmatter.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(value) = line.strip_prefix(&format!("{key} =")) {
            return unquote_frontmatter_value(value);
        }
        if let Some(value) = line.strip_prefix(&format!("{key}:")) {
            return unquote_frontmatter_value(value);
        }
    }
    None
}

fn unquote_frontmatter_value(value: &str) -> Option<String> {
    let value = value.trim().trim_end_matches(',').trim();
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parses_zola_content_frontmatter_values() {
        let source =
            "+++\ntitle = \"About\"\ntemplate = \"templates/about.html\"\npage_template = 'blog/page.html'\n+++\n";

        let frontmatter = parse_zola_content_frontmatter(source);

        assert_eq!(frontmatter.title.as_deref(), Some("About"));
        assert_eq!(
            frontmatter.template.as_deref(),
            Some("templates/about.html")
        );
        assert_eq!(frontmatter.page_template.as_deref(), Some("blog/page.html"));
    }

    #[test]
    fn resolves_zola_content_kind_templates_and_urls() {
        let zola_root = PathBuf::from("/project");
        let home = zola_root.join("content/_index.md");
        let section = zola_root.join("content/blog/_index.md");
        let page = zola_root.join("content/blog/post.md");

        assert!(matches!(
            zola_content_page_kind(&zola_root, &home),
            SourcePageKind::Home
        ));
        assert!(matches!(
            zola_content_page_kind(&zola_root, &section),
            SourcePageKind::Section
        ));
        assert!(matches!(
            zola_content_page_kind(&zola_root, &page),
            SourcePageKind::Page
        ));
        assert_eq!(
            resolve_zola_page_template(&None, &SourcePageKind::Home).as_deref(),
            Some("index.html")
        );
        assert_eq!(
            resolve_zola_page_template(&None, &SourcePageKind::Section).as_deref(),
            Some("section.html")
        );
        assert_eq!(
            resolve_zola_page_template(&None, &SourcePageKind::Page).as_deref(),
            Some("page.html")
        );
        assert_eq!(
            resolve_zola_page_template(
                &Some("templates/custom/page.html".to_string()),
                &SourcePageKind::Page
            )
            .as_deref(),
            Some("custom/page.html")
        );
        assert_eq!(zola_content_url(&zola_root, &home), "/");
        assert_eq!(zola_content_url(&zola_root, &section), "/blog/");
        assert_eq!(zola_content_url(&zola_root, &page), "/blog/post/");
    }

    #[test]
    fn normalizes_zola_content_references_and_project_files() {
        assert_eq!(
            normalize_zola_content_reference(" blog\\post.md "),
            "blog/post.md"
        );
        assert_eq!(
            zola_content_project_file_reference("content/blog/post.md").as_deref(),
            Some("blog/post.md")
        );
        assert_eq!(
            zola_content_project_file_reference("templates/blog.html"),
            None
        );
    }

    #[test]
    fn finds_zola_frontmatter_template_literal() {
        let source = "+++\ntitle = \"Blog\"\npage_template = \"templates/blog/card.html\"\n+++\n";

        let literal =
            find_zola_frontmatter_template_literal(source, "page_template", "blog/card.html");

        assert_eq!(
            literal,
            Some((36, 60, "templates/blog/card.html".to_string()))
        );
    }

    #[test]
    fn rewrites_zola_content_references_by_relation() {
        assert_eq!(
            rewrite_zola_content_reference(
                "blog/post.md",
                "blog/new.md",
                &SourceRelationKind::GetsPage
            )
            .as_deref(),
            Ok("blog/new.md")
        );
        assert_eq!(
            rewrite_zola_content_reference(
                "@/blog/post.md",
                "blog/new.md",
                &SourceRelationKind::InternalContentLink
            )
            .as_deref(),
            Ok("@/blog/new.md")
        );
        assert!(rewrite_zola_content_reference(
            "content/blog/post.md",
            "blog/new.md",
            &SourceRelationKind::GetsPage
        )
        .is_err());
    }

    #[test]
    fn rewrites_content_load_reference_preserving_prefix() {
        assert_eq!(
            zola_content_load_reference("@/blog/post.md").as_deref(),
            Some("blog/post.md")
        );
        assert_eq!(
            zola_content_load_reference("content/blog/post.md").as_deref(),
            Some("blog/post.md")
        );
        assert_eq!(
            zola_content_load_reference("/content/blog/post.md").as_deref(),
            Some("blog/post.md")
        );
        assert_eq!(zola_content_load_reference("blog/post.md"), None);
        assert_eq!(
            rewrite_zola_content_load_reference("@/blog/post.md", "blog/new.md").as_deref(),
            Ok("@/blog/new.md")
        );
        assert_eq!(
            rewrite_zola_content_load_reference("content/blog/post.md", "blog/new.md").as_deref(),
            Ok("content/blog/new.md")
        );
        assert_eq!(
            rewrite_zola_content_load_reference("/content/blog/post.md", "blog/new.md").as_deref(),
            Ok("/content/blog/new.md")
        );
        assert!(rewrite_zola_content_load_reference("blog/post.md", "blog/new.md").is_err());
    }
}
