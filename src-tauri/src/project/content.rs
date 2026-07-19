use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    project::paths::{normalize_optional_relative_path, resolve_project_write_path},
    zola_theme::ZolaThemeResolver,
};

pub struct ContentPageDraft {
    pub relative_path: String,
    pub contents: String,
}

pub fn build_content_page_draft_with_active_theme(
    root: &Path,
    section: &str,
    slug: &str,
    title: &str,
    active_theme: Option<String>,
) -> Result<ContentPageDraft, String> {
    build_content_page_draft_with_resolver(
        root,
        section,
        slug,
        title,
        &ZolaThemeResolver::new(active_theme),
    )
}

fn build_content_page_draft_with_resolver(
    root: &Path,
    section: &str,
    slug: &str,
    title: &str,
    resolver: &ZolaThemeResolver,
) -> Result<ContentPageDraft, String> {
    let normalized_slug = slugify(slug);
    if normalized_slug.is_empty() {
        return Err("Slug invalid. Foloseste litere mici, cifre si cratime.".to_string());
    }

    let normalized_section = normalize_optional_relative_path(section)?;
    let relative_path = if normalized_section.is_empty() {
        format!("content/{normalized_slug}.md")
    } else {
        format!("content/{normalized_section}/{normalized_slug}.md")
    };
    let path = resolve_project_write_path(root, &relative_path)?;

    if path.exists() {
        return Err(format!("Fisierul exista deja: {relative_path}"));
    }

    let page_title = title.trim();
    if page_title.is_empty() {
        return Err("Titlul paginii nu poate fi gol.".to_string());
    }

    let template_line = default_page_template_line_with_resolver(root, resolver);
    let contents = format!(
        "+++\n\
title = \"{}\"\n\
date = {}\n\
draft = true\n\
{}\
+++\n\n",
        escape_toml_basic_string(page_title),
        current_date_literal(),
        template_line,
    );

    Ok(ContentPageDraft {
        relative_path,
        contents,
    })
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_was_dash = false;

    for character in value.trim().chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            previous_was_dash = false;
            continue;
        }

        if !previous_was_dash {
            slug.push('-');
            previous_was_dash = true;
        }
    }

    slug.trim_matches('-').to_string()
}

fn escape_toml_basic_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ")
}

fn current_date_literal() -> String {
    let now = SystemTime::now();
    let duration = now
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0));
    let days_since_epoch = (duration.as_secs() / 86_400) as i64;
    let (year, month, day) = civil_from_days(days_since_epoch);
    format!("\"{year:04}-{month:02}-{day:02}\"")
}

fn default_page_template_line_with_resolver(root: &Path, resolver: &ZolaThemeResolver) -> String {
    if resolver.resolve_template_path(root, "page.html").is_some() {
        "template = \"page.html\"\n".to_string()
    } else {
        String::new()
    }
}

fn civil_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year, m, d)
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
    fn default_template_line_uses_theme_page_template() {
        let root = unique_test_dir("theme-page-template");
        fs::create_dir_all(root.join("themes/test-theme/templates")).unwrap();
        fs::write(
            root.join("zola.toml"),
            "base_url = \"http://example.test\"\ntheme = \"test-theme\"\n",
        )
        .unwrap();
        fs::write(root.join("themes/test-theme/templates/page.html"), "").unwrap();

        let active_theme = ZolaThemeResolver::for_root(&root)
            .active_theme()
            .map(str::to_string);
        let draft = build_content_page_draft_with_active_theme(
            &root,
            "",
            "despre",
            "Despre noi",
            active_theme,
        )
        .unwrap();

        assert!(draft.contents.contains("template = \"page.html\""));
        cleanup(&root);
    }

    #[test]
    fn default_template_line_ignores_theme_when_theme_is_disabled() {
        let root = unique_test_dir("disabled-theme-page-template");
        fs::create_dir_all(root.join("themes/test-theme/templates")).unwrap();
        fs::write(
            root.join("zola.toml"),
            "base_url = \"http://example.test\"\n",
        )
        .unwrap();
        fs::write(root.join("themes/test-theme/templates/page.html"), "").unwrap();

        let active_theme = ZolaThemeResolver::for_root(&root)
            .active_theme()
            .map(str::to_string);
        let draft = build_content_page_draft_with_active_theme(
            &root,
            "",
            "despre",
            "Despre noi",
            active_theme,
        )
        .unwrap();

        assert!(!draft.contents.contains("template = \"page.html\""));
        cleanup(&root);
    }

    #[test]
    fn draft_template_line_can_use_active_theme_from_current_source() {
        let root = unique_test_dir("current-theme-page-template");
        fs::create_dir_all(root.join("themes/test-theme/templates")).unwrap();
        fs::write(root.join("themes/test-theme/templates/page.html"), "").unwrap();

        let draft = build_content_page_draft_with_active_theme(
            &root,
            "",
            "despre",
            "Despre noi",
            Some("test-theme".to_string()),
        )
        .unwrap();

        assert_eq!(draft.relative_path, "content/despre.md");
        assert!(draft.contents.contains("template = \"page.html\""));
        cleanup(&root);
    }

    #[test]
    fn draft_template_line_ignores_theme_template_without_active_theme() {
        let root = unique_test_dir("current-no-theme-page-template");
        fs::create_dir_all(root.join("themes/test-theme/templates")).unwrap();
        fs::write(root.join("themes/test-theme/templates/page.html"), "").unwrap();

        let draft =
            build_content_page_draft_with_active_theme(&root, "", "despre", "Despre noi", None)
                .unwrap();

        assert!(!draft.contents.contains("template = \"page.html\""));
        cleanup(&root);
    }

    fn cleanup(root: &Path) {
        let _ = fs::remove_dir_all(root);
    }

    fn unique_test_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("panastudio-content-{label}-{nanos}"))
    }
}
