use std::path::Path;

use serde::{Deserialize, Serialize};

use super::paths::resolve_project_write_path;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SiteTemplateWriteOrigin {
    Local,
    Theme,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SitePageStructureInput {
    pub title: String,
    pub slug: String,
    pub page_template_name: String,
    pub draft: bool,
    pub target_origin: Option<SiteTemplateWriteOrigin>,
    pub target_theme_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteArchiveStructureInput {
    pub title: String,
    pub slug: String,
    pub archive_template_name: String,
    pub target_origin: Option<SiteTemplateWriteOrigin>,
    pub target_theme_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteSingleStructureInput {
    pub section_slug: String,
    pub title: String,
    pub slug: String,
    pub single_template_name: String,
    pub target_origin: Option<SiteTemplateWriteOrigin>,
    pub target_theme_name: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SitePartialPreset {
    Cta,
    Header,
    Footer,
    Generic,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SitePartialStructureInput {
    pub name: String,
    pub preset: Option<SitePartialPreset>,
    pub target_origin: Option<SiteTemplateWriteOrigin>,
    pub target_theme_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SitePartialIncludeInput {
    pub target_file: String,
    pub partial_template_name: String,
    /// When present, creation of the partial and insertion of its include are
    /// committed by the backend as one WorkspaceMutation transaction.
    #[serde(default)]
    pub ensure_partial: Option<SitePartialStructureInput>,
}

#[derive(Clone, Debug)]
pub struct SiteTextChange {
    pub relative_path: String,
    pub new_text: String,
}

#[derive(Clone, Debug)]
pub struct PlannedSitePageStructure {
    pub slug: String,
    pub content_path: String,
    pub template_path: String,
    pub page_template: String,
    pub origin: SiteTemplateWriteOrigin,
    pub theme_name: Option<String>,
    pub created: Vec<String>,
    pub changes: Vec<SiteTextChange>,
}

#[derive(Clone, Debug)]
pub struct PlannedSiteArchiveStructure {
    pub slug: String,
    pub content_path: String,
    pub template_path: String,
    pub archive_template: String,
    pub origin: SiteTemplateWriteOrigin,
    pub theme_name: Option<String>,
    pub created: Vec<String>,
    pub changes: Vec<SiteTextChange>,
}

#[derive(Clone, Debug)]
pub struct PlannedSiteSingleStructure {
    pub section_slug: String,
    pub item_slug: String,
    pub item_path: String,
    pub template_path: String,
    pub single_template: String,
    pub origin: SiteTemplateWriteOrigin,
    pub theme_name: Option<String>,
    pub created: Vec<String>,
    pub changes: Vec<SiteTextChange>,
}

#[derive(Clone, Debug)]
pub struct PlannedSitePartialStructure {
    pub path: String,
    pub template_name: String,
    pub origin: SiteTemplateWriteOrigin,
    pub theme_name: Option<String>,
    pub created: bool,
    pub changes: Vec<SiteTextChange>,
}

#[derive(Clone, Debug)]
pub struct PlannedSitePartialInclude {
    pub target_file: String,
    pub partial_template_name: String,
    pub changed: bool,
    pub reason: String,
    pub changes: Vec<SiteTextChange>,
}

pub fn plan_site_page_structure(
    project_root: &Path,
    input: SitePageStructureInput,
) -> Result<PlannedSitePageStructure, String> {
    let slug_source = if input.slug.trim().is_empty() {
        input.title.as_str()
    } else {
        input.slug.as_str()
    };
    let slug = slugify_label(slug_source);
    let title = input.title.trim();
    let title = if title.is_empty() {
        slug.as_str()
    } else {
        title
    };
    let page_template_source = if input.page_template_name.trim().is_empty() {
        "page.html"
    } else {
        input.page_template_name.as_str()
    };
    let page_template = normalize_template_file_name(page_template_source)?;

    if slug.is_empty() || title.is_empty() || page_template.is_empty() {
        return Err("Pagina are nevoie de titlu, slug și template valid.".to_string());
    }

    let (origin, theme_name) =
        resolve_template_write_base(input.target_origin, input.target_theme_name)?;

    let content_path = format!("content/{slug}.md");
    let template_path = template_project_path(&page_template, origin, theme_name.as_deref())?;
    let mut created = Vec::new();
    let mut changes = Vec::new();

    if !project_path_exists(project_root, &template_path)? {
        created.push(template_path.clone());
        changes.push(SiteTextChange {
            relative_path: template_path.clone(),
            new_text: page_template_source_text(),
        });
    }

    if !project_path_exists(project_root, &content_path)? {
        created.push(content_path.clone());
        changes.push(SiteTextChange {
            relative_path: content_path.clone(),
            new_text: page_frontmatter(title, &page_template, input.draft),
        });
    }

    Ok(PlannedSitePageStructure {
        slug,
        content_path,
        template_path,
        page_template,
        origin,
        theme_name,
        created,
        changes,
    })
}

pub fn plan_site_archive_structure(
    project_root: &Path,
    input: SiteArchiveStructureInput,
) -> Result<PlannedSiteArchiveStructure, String> {
    let slug_source = if input.slug.trim().is_empty() {
        input.title.as_str()
    } else {
        input.slug.as_str()
    };
    let slug = slugify_label(slug_source);
    let title = input.title.trim();
    let title = if title.is_empty() {
        slug.as_str()
    } else {
        title
    };
    let archive_template_source = if input.archive_template_name.trim().is_empty() {
        format!("{slug}.html")
    } else {
        input.archive_template_name
    };
    let archive_template = normalize_template_file_name(&archive_template_source)?;

    if slug.is_empty() || archive_template.is_empty() {
        return Err("Arhiva are nevoie de nume, slug și template valid.".to_string());
    }

    let (origin, theme_name) =
        resolve_template_write_base(input.target_origin, input.target_theme_name)?;
    let content_path = format!("content/{slug}/_index.md");
    let template_path = template_project_path(&archive_template, origin, theme_name.as_deref())?;
    let mut created = Vec::new();
    let mut changes = Vec::new();

    if !project_path_exists(project_root, &content_path)? {
        created.push(content_path.clone());
        changes.push(SiteTextChange {
            relative_path: content_path.clone(),
            new_text: archive_frontmatter(title, &archive_template),
        });
    }

    if !project_path_exists(project_root, &template_path)? {
        created.push(template_path.clone());
        changes.push(SiteTextChange {
            relative_path: template_path.clone(),
            new_text: archive_template_source_text(title),
        });
    }

    Ok(PlannedSiteArchiveStructure {
        slug,
        content_path,
        template_path,
        archive_template,
        origin,
        theme_name,
        created,
        changes,
    })
}

pub fn plan_site_single_structure(
    project_root: &Path,
    input: SiteSingleStructureInput,
) -> Result<PlannedSiteSingleStructure, String> {
    let section_slug = slugify_label(&input.section_slug);
    let item_slug_source = if input.slug.trim().is_empty() {
        input.title.as_str()
    } else {
        input.slug.as_str()
    };
    let item_slug = slugify_label(item_slug_source);
    let title = input.title.trim();
    let title = if title.is_empty() {
        item_slug.as_str()
    } else {
        title
    };
    let single_template_source = if input.single_template_name.trim().is_empty() {
        format!("{section_slug}-single.html")
    } else {
        input.single_template_name
    };
    let single_template = normalize_template_file_name(&single_template_source)?;

    if section_slug.is_empty()
        || item_slug.is_empty()
        || title.is_empty()
        || single_template.is_empty()
    {
        return Err("Single-ul are nevoie de secțiune, slug și template valid.".to_string());
    }

    let (origin, theme_name) =
        resolve_template_write_base(input.target_origin, input.target_theme_name)?;
    let item_path = format!("content/{section_slug}/{item_slug}.md");
    let template_path = template_project_path(&single_template, origin, theme_name.as_deref())?;
    let mut created = Vec::new();
    let mut changes = Vec::new();

    if !project_path_exists(project_root, &template_path)? {
        created.push(template_path.clone());
        changes.push(SiteTextChange {
            relative_path: template_path.clone(),
            new_text: single_template_source_text(),
        });
    }

    if !project_path_exists(project_root, &item_path)? {
        created.push(item_path.clone());
        changes.push(SiteTextChange {
            relative_path: item_path.clone(),
            new_text: single_frontmatter(title, &single_template),
        });
    }

    Ok(PlannedSiteSingleStructure {
        section_slug,
        item_slug,
        item_path,
        template_path,
        single_template,
        origin,
        theme_name,
        created,
        changes,
    })
}

pub fn plan_site_partial_structure(
    project_root: &Path,
    input: SitePartialStructureInput,
) -> Result<PlannedSitePartialStructure, String> {
    let partial_name = slugify_label(&input.name);
    if partial_name.is_empty() {
        return Err("Numele partialului este invalid.".to_string());
    }

    let (origin, theme_name) =
        resolve_template_write_base(input.target_origin, input.target_theme_name)?;
    let template_name = format!("partials/{partial_name}.html");
    let path = template_project_path(&template_name, origin, theme_name.as_deref())?;
    let created = !project_path_exists(project_root, &path)?;
    let changes = if created {
        vec![SiteTextChange {
            relative_path: path.clone(),
            new_text: partial_template_source_text(&partial_name, input.preset),
        }]
    } else {
        Vec::new()
    };

    Ok(PlannedSitePartialStructure {
        path,
        template_name,
        origin,
        theme_name,
        created,
        changes,
    })
}

pub fn plan_site_partial_include(
    source: &str,
    input: SitePartialIncludeInput,
) -> Result<PlannedSitePartialInclude, String> {
    let target_file = normalize_site_template_project_path(&input.target_file)?;
    let partial_template_name = normalize_template_file_name(&input.partial_template_name)?;
    if !partial_template_name.starts_with("partials/") {
        return Err("Include-ul Site Editor acceptă doar template-uri din partials/.".to_string());
    }

    let statement = include_statement(&partial_template_name);
    if find_include_line(source, &partial_template_name).is_some() {
        return Ok(PlannedSitePartialInclude {
            target_file,
            partial_template_name,
            changed: false,
            reason: "Partialul este deja inclus în template.".to_string(),
            changes: Vec::new(),
        });
    }

    let (new_text, reason) = insert_include_into_template_source(source, &statement);

    Ok(PlannedSitePartialInclude {
        target_file: target_file.clone(),
        partial_template_name,
        changed: true,
        reason,
        changes: vec![SiteTextChange {
            relative_path: target_file,
            new_text,
        }],
    })
}

fn resolve_template_write_base(
    origin: Option<SiteTemplateWriteOrigin>,
    theme_name: Option<String>,
) -> Result<(SiteTemplateWriteOrigin, Option<String>), String> {
    let origin = origin.unwrap_or(SiteTemplateWriteOrigin::Local);
    let theme_name = match origin {
        SiteTemplateWriteOrigin::Local => None,
        SiteTemplateWriteOrigin::Theme => {
            let theme_name = theme_name
                .as_deref()
                .ok_or_else(|| "Tema țintă lipsește pentru scrierea template-ului.".to_string())?;
            Some(normalize_theme_name(theme_name)?)
        }
    };
    Ok((origin, theme_name))
}

fn project_path_exists(project_root: &Path, relative_path: &str) -> Result<bool, String> {
    let path = resolve_project_write_path(project_root, relative_path)?;
    path.try_exists().map_err(|error| {
        format!(
            "Nu am putut verifica existența {}: {}",
            path.to_string_lossy(),
            error
        )
    })
}

fn template_project_path(
    template_name: &str,
    origin: SiteTemplateWriteOrigin,
    theme_name: Option<&str>,
) -> Result<String, String> {
    match origin {
        SiteTemplateWriteOrigin::Local => Ok(format!("templates/{template_name}")),
        SiteTemplateWriteOrigin::Theme => {
            let theme_name = theme_name
                .ok_or_else(|| "Tema țintă lipsește pentru scrierea template-ului.".to_string())?;
            Ok(format!("themes/{theme_name}/templates/{template_name}"))
        }
    }
}

fn normalize_template_file_name(value: &str) -> Result<String, String> {
    let mut normalized = value.trim().replace('\\', "/");
    normalized = normalized.trim_start_matches('/').to_string();
    if let Some(rest) = strip_theme_template_prefix(&normalized) {
        normalized = rest.to_string();
    } else if let Some(rest) = normalized.strip_prefix("templates/") {
        normalized = rest.to_string();
    }

    if !normalized.ends_with(".html") {
        normalized.push_str(".html");
    }
    normalize_relative_segments(&normalized, "Template-ul")
}

fn normalize_site_template_project_path(value: &str) -> Result<String, String> {
    let normalized = normalize_relative_segments(&value.trim().replace('\\', "/"), "Template-ul")?;
    let local_template = normalized.starts_with("templates/");
    let theme_template = is_theme_template_project_path(&normalized);
    if !local_template && !theme_template {
        return Err("Include-ul trebuie aplicat pe un template Zola.".to_string());
    }
    if !normalized.ends_with(".html") {
        return Err("Include-ul trebuie aplicat pe un template HTML Zola.".to_string());
    }
    Ok(normalized)
}

fn is_theme_template_project_path(value: &str) -> bool {
    let mut segments = value.split('/');
    matches!(segments.next(), Some("themes"))
        && segments.next().is_some()
        && matches!(segments.next(), Some("templates"))
        && segments.next().is_some()
}

fn strip_theme_template_prefix(value: &str) -> Option<&str> {
    let mut segments = value.split('/');
    if segments.next()? != "themes" {
        return None;
    }
    let _theme_name = segments.next()?;
    if segments.next()? != "templates" {
        return None;
    }
    let offset = value.splitn(4, '/').take(3).map(str::len).sum::<usize>() + 3;
    value.get(offset..)
}

fn normalize_theme_name(value: &str) -> Result<String, String> {
    let normalized = normalize_relative_segments(value, "Tema")?;
    if normalized.contains('/') {
        return Err("Tema țintă trebuie să fie un nume de director, nu path.".to_string());
    }
    Ok(normalized)
}

fn normalize_relative_segments(value: &str, label: &str) -> Result<String, String> {
    let mut segments = Vec::new();
    for segment in value.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." || segment.contains('\0') {
            return Err(format!("{label} are path invalid."));
        }
        segments.push(segment);
    }

    if segments.is_empty() {
        return Err(format!("{label} este gol."));
    }
    Ok(segments.join("/"))
}

fn slugify_label(value: &str) -> String {
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

fn page_frontmatter(title: &str, template: &str, draft: bool) -> String {
    let draft_line = if draft { "draft = true\n" } else { "" };
    format!(
        "+++\ntitle = \"{}\"\n{}template = \"{}\"\n+++\n\nScrie conținutul aici.\n",
        escape_toml_string(title),
        draft_line,
        escape_toml_string(template)
    )
}

fn page_template_source_text() -> String {
    "{% extends \"layout.html\" %}\n\n{% block content %}\n<article class=\"pagina\">\n  <header class=\"pagina-header\">\n    <h1>{{ page.title }}</h1>\n  </header>\n  <div class=\"continut-pagina\">\n    {{ page.content | safe }}\n  </div>\n</article>\n{% endblock %}\n".to_string()
}

fn archive_frontmatter(title: &str, template: &str) -> String {
    format!(
        "+++\ntitle = \"{}\"\ntemplate = \"{}\"\nsort_by = \"date\"\n+++\n\n",
        escape_toml_string(title),
        escape_toml_string(template)
    )
}

fn archive_template_source_text(title: &str) -> String {
    format!(
        "{{% extends \"layout.html\" %}}\n\n{{% block content %}}\n<section class=\"arhiva\">\n  <h1>{{{{ section.title | default(value=\"{}\") }}}}</h1>\n  <div class=\"lista-articole\">\n    {{% for page in section.pages %}}\n      <article class=\"card-articol\">\n        <h2><a href=\"{{{{ page.permalink }}}}\">{{{{ page.title }}}}</a></h2>\n        {{% if page.description %}}<p>{{{{ page.description }}}}</p>{{% endif %}}\n      </article>\n    {{% endfor %}}\n  </div>\n</section>\n{{% endblock %}}\n",
        escape_tera_string(title)
    )
}

fn single_frontmatter(title: &str, template: &str) -> String {
    format!(
        "+++\ntitle = \"{}\"\ndate = \"2026-01-01\"\ntemplate = \"{}\"\n+++\n\nScrie conținutul aici.\n",
        escape_toml_string(title),
        escape_toml_string(template)
    )
}

fn single_template_source_text() -> String {
    "{% extends \"layout.html\" %}\n\n{% block content %}\n<article class=\"articol\">\n  <header class=\"articol-header\">\n    <h1>{{ page.title }}</h1>\n    {% if page.date %}<time datetime=\"{{ page.date }}\">{{ page.date }}</time>{% endif %}\n  </header>\n  <div class=\"continut-articol\">\n    {{ page.content | safe }}\n  </div>\n</article>\n{% endblock %}\n".to_string()
}

fn partial_template_source_text(name: &str, preset: Option<SitePartialPreset>) -> String {
    let preset = preset.unwrap_or(SitePartialPreset::Generic);
    if preset == SitePartialPreset::Cta || name.contains("cta") {
        return "<section class=\"cta\">\n  <h2>Pregătit să lucrăm împreună?</h2>\n  <p>Adaugă aici mesajul principal al apelului la acțiune.</p>\n  <a class=\"buton\" href=\"/contact/\">Contact</a>\n</section>\n".to_string();
    }
    if preset == SitePartialPreset::Header {
        return "<header class=\"site-header\">\n  <a class=\"logo\" href=\"/\">Site</a>\n  <nav aria-label=\"Navigație principală\"></nav>\n</header>\n".to_string();
    }
    if preset == SitePartialPreset::Footer {
        return "<footer class=\"site-footer\">\n  <p>&copy; {{ now() | date(format=\"%Y\") }} Site</p>\n</footer>\n".to_string();
    }
    format!("<section class=\"{name}\">\n  <h2>{name}</h2>\n</section>\n")
}

fn include_statement(template_name: &str) -> String {
    format!("{{% include \"{template_name}\" %}}")
}

fn insert_include_into_template_source(source: &str, statement: &str) -> (String, String) {
    if let Some(line) = find_include_line(source, "partials/footer.html") {
        let insert_at = insertion_index_before_line(source, line.start);
        return (
            insert_at_with_indent(source, insert_at, &line.indent, statement),
            "Partial inclus înainte de footer.".to_string(),
        );
    }

    if let Some(line) = find_last_endblock_line(source) {
        let insert_at = insertion_index_before_line(source, line.start);
        return (
            insert_at_with_indent(source, insert_at, &line.indent, statement),
            "Partial inclus înainte de închiderea block-ului.".to_string(),
        );
    }

    let suffix = if source.ends_with('\n') { "" } else { "\n" };
    (
        format!("{source}{suffix}{statement}\n"),
        "Partial inclus la finalul template-ului.".to_string(),
    )
}

#[derive(Clone, Debug)]
struct IncludeLineMatch {
    start: usize,
    indent: String,
}

fn find_include_line(source: &str, template_name: &str) -> Option<IncludeLineMatch> {
    find_lines(source)
        .into_iter()
        .find(|line| is_include_line(line.text, template_name))
        .map(|line| IncludeLineMatch {
            start: line.start,
            indent: leading_whitespace(line.text),
        })
}

fn find_last_endblock_line(source: &str) -> Option<IncludeLineMatch> {
    find_lines(source)
        .into_iter()
        .filter(|line| is_endblock_line(line.text))
        .last()
        .map(|line| IncludeLineMatch {
            start: line.start,
            indent: leading_whitespace(line.text),
        })
}

#[derive(Clone, Copy)]
struct SourceLine<'a> {
    start: usize,
    text: &'a str,
}

fn find_lines(source: &str) -> Vec<SourceLine<'_>> {
    let mut lines = Vec::new();
    let mut start = 0;
    for line in source.split_inclusive('\n') {
        let text = line.strip_suffix('\n').unwrap_or(line);
        lines.push(SourceLine { start, text });
        start += line.len();
    }
    if source.is_empty() {
        lines.push(SourceLine { start: 0, text: "" });
    }
    lines
}

fn is_include_line(line: &str, template_name: &str) -> bool {
    let trimmed = line.trim_start();
    let double_quoted = format!("\"{template_name}\"");
    let single_quoted = format!("'{template_name}'");
    trimmed.starts_with("{%")
        && trimmed.contains("include")
        && (trimmed.contains(&double_quoted) || trimmed.contains(&single_quoted))
}

fn is_endblock_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("{%") && trimmed.contains("endblock")
}

fn leading_whitespace(line: &str) -> String {
    line.chars()
        .take_while(|character| *character == ' ' || *character == '\t')
        .collect()
}

fn insertion_index_before_line(source: &str, line_start: usize) -> usize {
    if line_start > 0 && source.as_bytes().get(line_start - 1) == Some(&b'\n') {
        line_start - 1
    } else {
        line_start
    }
}

fn insert_at_with_indent(source: &str, insert_at: usize, indent: &str, statement: &str) -> String {
    format!(
        "{}\n{}{}{}",
        &source[..insert_at],
        indent,
        statement,
        &source[insert_at..]
    )
}

fn escape_toml_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn escape_tera_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
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
    fn plans_local_site_page_and_template_as_one_structure() {
        let root = unique_test_dir("local-site-page");
        fs::create_dir_all(&root).unwrap();

        let plan = plan_site_page_structure(
            &root,
            SitePageStructureInput {
                title: "Pagina Nouă".to_string(),
                slug: "Pagina Nouă".to_string(),
                page_template_name: "custom/page".to_string(),
                draft: true,
                target_origin: Some(SiteTemplateWriteOrigin::Local),
                target_theme_name: None,
            },
        )
        .unwrap();

        assert_eq!(plan.slug, "pagina-nou");
        assert_eq!(plan.content_path, "content/pagina-nou.md");
        assert_eq!(plan.template_path, "templates/custom/page.html");
        assert_eq!(
            plan.created,
            vec![plan.template_path.clone(), plan.content_path.clone()]
        );
        assert_eq!(plan.changes.len(), 2);
        assert!(plan.changes[1].new_text.contains("draft = true\n"));

        cleanup(&root);
    }

    #[test]
    fn skips_existing_template_and_creates_missing_content() {
        let root = unique_test_dir("existing-template");
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(root.join("templates/page.html"), "existing").unwrap();

        let plan = plan_site_page_structure(
            &root,
            SitePageStructureInput {
                title: "Despre".to_string(),
                slug: "despre".to_string(),
                page_template_name: "page.html".to_string(),
                draft: false,
                target_origin: Some(SiteTemplateWriteOrigin::Local),
                target_theme_name: None,
            },
        )
        .unwrap();

        assert_eq!(plan.created, vec!["content/despre.md"]);
        assert_eq!(plan.changes.len(), 1);
        assert!(!plan.changes[0].new_text.contains("draft = true"));

        cleanup(&root);
    }

    #[test]
    fn plans_theme_template_target_when_requested() {
        let root = unique_test_dir("theme-target");
        fs::create_dir_all(&root).unwrap();

        let plan = plan_site_page_structure(
            &root,
            SitePageStructureInput {
                title: "Servicii".to_string(),
                slug: "servicii".to_string(),
                page_template_name: "templates/pages/service".to_string(),
                draft: false,
                target_origin: Some(SiteTemplateWriteOrigin::Theme),
                target_theme_name: Some("tema-test".to_string()),
            },
        )
        .unwrap();

        assert_eq!(
            plan.template_path,
            "themes/tema-test/templates/pages/service.html"
        );
        assert_eq!(plan.origin, SiteTemplateWriteOrigin::Theme);
        assert_eq!(plan.theme_name.as_deref(), Some("tema-test"));

        cleanup(&root);
    }

    #[test]
    fn rejects_template_path_traversal() {
        let root = unique_test_dir("reject-template");
        fs::create_dir_all(&root).unwrap();

        let error = plan_site_page_structure(
            &root,
            SitePageStructureInput {
                title: "Bad".to_string(),
                slug: "bad".to_string(),
                page_template_name: "../bad".to_string(),
                draft: false,
                target_origin: Some(SiteTemplateWriteOrigin::Local),
                target_theme_name: None,
            },
        )
        .unwrap_err();

        assert!(error.contains("Template-ul are path invalid"));
        cleanup(&root);
    }

    #[test]
    fn plans_archive_content_and_template_as_one_structure() {
        let root = unique_test_dir("archive-structure");
        fs::create_dir_all(&root).unwrap();

        let plan = plan_site_archive_structure(
            &root,
            SiteArchiveStructureInput {
                title: "Articole".to_string(),
                slug: "Articole".to_string(),
                archive_template_name: "blog/archive".to_string(),
                target_origin: Some(SiteTemplateWriteOrigin::Local),
                target_theme_name: None,
            },
        )
        .unwrap();

        assert_eq!(plan.slug, "articole");
        assert_eq!(plan.content_path, "content/articole/_index.md");
        assert_eq!(plan.template_path, "templates/blog/archive.html");
        assert_eq!(
            plan.created,
            vec![plan.content_path.clone(), plan.template_path.clone()]
        );
        assert_eq!(plan.changes.len(), 2);
        assert!(plan.changes[0].new_text.contains("sort_by = \"date\""));
        assert!(plan.changes[1].new_text.contains("section.pages"));

        cleanup(&root);
    }

    #[test]
    fn archive_skips_existing_content_and_creates_template() {
        let root = unique_test_dir("archive-existing-content");
        fs::create_dir_all(root.join("content/blog")).unwrap();
        fs::write(root.join("content/blog/_index.md"), "existing").unwrap();

        let plan = plan_site_archive_structure(
            &root,
            SiteArchiveStructureInput {
                title: "Blog".to_string(),
                slug: "blog".to_string(),
                archive_template_name: "blog.html".to_string(),
                target_origin: Some(SiteTemplateWriteOrigin::Local),
                target_theme_name: None,
            },
        )
        .unwrap();

        assert_eq!(plan.created, vec!["templates/blog.html"]);
        assert_eq!(plan.changes.len(), 1);
        assert_eq!(plan.changes[0].relative_path, "templates/blog.html");

        cleanup(&root);
    }

    #[test]
    fn plans_single_item_and_template_as_one_structure() {
        let root = unique_test_dir("single-structure");
        fs::create_dir_all(&root).unwrap();

        let plan = plan_site_single_structure(
            &root,
            SiteSingleStructureInput {
                section_slug: "Blog".to_string(),
                title: "Primul articol".to_string(),
                slug: "Primul articol".to_string(),
                single_template_name: "blog/single".to_string(),
                target_origin: Some(SiteTemplateWriteOrigin::Local),
                target_theme_name: None,
            },
        )
        .unwrap();

        assert_eq!(plan.section_slug, "blog");
        assert_eq!(plan.item_slug, "primul-articol");
        assert_eq!(plan.item_path, "content/blog/primul-articol.md");
        assert_eq!(plan.template_path, "templates/blog/single.html");
        assert_eq!(
            plan.created,
            vec![plan.template_path.clone(), plan.item_path.clone()]
        );
        assert_eq!(plan.changes.len(), 2);
        assert!(plan.changes[0].new_text.contains("page.date"));
        assert!(plan.changes[1].new_text.contains("date = \"2026-01-01\""));

        cleanup(&root);
    }

    #[test]
    fn single_skips_existing_template_and_creates_item() {
        let root = unique_test_dir("single-existing-template");
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(root.join("templates/blog-single.html"), "existing").unwrap();

        let plan = plan_site_single_structure(
            &root,
            SiteSingleStructureInput {
                section_slug: "blog".to_string(),
                title: "Articol".to_string(),
                slug: "articol".to_string(),
                single_template_name: String::new(),
                target_origin: Some(SiteTemplateWriteOrigin::Local),
                target_theme_name: None,
            },
        )
        .unwrap();

        assert_eq!(plan.single_template, "blog-single.html");
        assert_eq!(plan.created, vec!["content/blog/articol.md"]);
        assert_eq!(plan.changes.len(), 1);
        assert_eq!(plan.changes[0].relative_path, "content/blog/articol.md");

        cleanup(&root);
    }

    #[test]
    fn plans_local_partial_as_workspace_mutation_change() {
        let root = unique_test_dir("partial-structure");
        fs::create_dir_all(&root).unwrap();

        let plan = plan_site_partial_structure(
            &root,
            SitePartialStructureInput {
                name: "Card Promo".to_string(),
                preset: Some(SitePartialPreset::Generic),
                target_origin: Some(SiteTemplateWriteOrigin::Local),
                target_theme_name: None,
            },
        )
        .unwrap();

        assert_eq!(plan.path, "templates/partials/card-promo.html");
        assert_eq!(plan.template_name, "partials/card-promo.html");
        assert_eq!(plan.origin, SiteTemplateWriteOrigin::Local);
        assert!(plan.created);
        assert_eq!(plan.changes.len(), 1);
        assert_eq!(
            plan.changes[0].relative_path,
            "templates/partials/card-promo.html"
        );
        assert!(plan.changes[0].new_text.contains("class=\"card-promo\""));

        cleanup(&root);
    }

    #[test]
    fn partial_skips_existing_theme_template() {
        let root = unique_test_dir("partial-existing-theme");
        fs::create_dir_all(root.join("themes/tema-test/templates/partials")).unwrap();
        fs::write(
            root.join("themes/tema-test/templates/partials/footer.html"),
            "existing",
        )
        .unwrap();

        let plan = plan_site_partial_structure(
            &root,
            SitePartialStructureInput {
                name: "Footer".to_string(),
                preset: Some(SitePartialPreset::Footer),
                target_origin: Some(SiteTemplateWriteOrigin::Theme),
                target_theme_name: Some("tema-test".to_string()),
            },
        )
        .unwrap();

        assert_eq!(plan.path, "themes/tema-test/templates/partials/footer.html");
        assert_eq!(plan.origin, SiteTemplateWriteOrigin::Theme);
        assert_eq!(plan.theme_name.as_deref(), Some("tema-test"));
        assert!(!plan.created);
        assert!(plan.changes.is_empty());

        cleanup(&root);
    }

    #[test]
    fn plans_partial_include_before_footer() {
        let source = "{% extends \"layout.html\" %}\n\n{% block content %}\n  <main></main>\n  {% include \"partials/footer.html\" %}\n{% endblock %}\n";

        let plan = plan_site_partial_include(
            source,
            SitePartialIncludeInput {
                target_file: "templates/page.html".to_string(),
                partial_template_name: "partials/cta.html".to_string(),
                ensure_partial: None,
            },
        )
        .unwrap();

        assert!(plan.changed);
        assert_eq!(plan.target_file, "templates/page.html");
        assert_eq!(plan.partial_template_name, "partials/cta.html");
        assert_eq!(plan.reason, "Partial inclus înainte de footer.");
        assert_eq!(plan.changes.len(), 1);
        assert!(plan.changes[0].new_text.contains(
            "{% include \"partials/cta.html\" %}\n  {% include \"partials/footer.html\" %}"
        ));
    }

    #[test]
    fn partial_include_is_noop_when_statement_exists() {
        let source = "{% block content %}\n{% include \"partials/cta.html\" %}\n{% endblock %}\n";

        let plan = plan_site_partial_include(
            source,
            SitePartialIncludeInput {
                target_file: "templates/page.html".to_string(),
                partial_template_name: "partials/cta.html".to_string(),
                ensure_partial: None,
            },
        )
        .unwrap();

        assert!(!plan.changed);
        assert_eq!(plan.reason, "Partialul este deja inclus în template.");
        assert!(plan.changes.is_empty());
    }

    #[test]
    fn partial_include_is_noop_for_single_quoted_whitespace_controlled_include() {
        let source = "{% block content %}\n{%- include 'partials/cta.html' -%}\n{% endblock %}\n";

        let plan = plan_site_partial_include(
            source,
            SitePartialIncludeInput {
                target_file: "templates/page.html".to_string(),
                partial_template_name: "partials/cta.html".to_string(),
                ensure_partial: None,
            },
        )
        .unwrap();

        assert!(!plan.changed);
        assert_eq!(plan.reason, "Partialul este deja inclus în template.");
        assert!(plan.changes.is_empty());
    }

    #[test]
    fn partial_include_rejects_non_html_target_template() {
        let error = plan_site_partial_include(
            "{% block content %}{% endblock %}",
            SitePartialIncludeInput {
                target_file: "templates/page.txt".to_string(),
                partial_template_name: "partials/cta.html".to_string(),
                ensure_partial: None,
            },
        )
        .unwrap_err();

        assert!(error.contains("template HTML Zola"));
    }

    fn cleanup(root: &PathBuf) {
        let _ = fs::remove_dir_all(root);
    }

    fn unique_test_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("panastudio-site-structure-{label}-{nanos}"))
    }
}
