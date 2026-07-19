use crate::{
    css::page::{imports::variables_import_path, model::WrittenProjectFile},
    zola_links::{stylesheet_link, template_contains_asset_path},
    zola_theme::{logical_template_name, theme_name_for_template_path},
};

pub fn prepare_page_stylesheet_source(
    relative_path: &str,
    existing: &str,
    style_files: impl IntoIterator<Item = String>,
    active_theme: Option<&str>,
) -> String {
    let Some(import_path) = variables_import_path(relative_path, style_files, active_theme) else {
        return existing.to_string();
    };

    if existing.contains(&import_path) {
        return existing.to_string();
    }

    let import_line = format!("@import '{}';", import_path);
    if existing.trim().is_empty() {
        return format!("{}\n\n", import_line);
    }
    format!("{}\n\n{}", import_line, existing.trim_start_matches('\n'))
}

/// Planifică legătura stylesheet-ului folosind sursa canonică furnizată de
/// apelant. Comenzile editorului o leagă de FileBufferStore, astfel încât un
/// draft nesalvat nu este înlocuit de o recitire directă de pe disk.
pub fn plan_page_stylesheet_link_writes_with_reader(
    template_path: &str,
    href: &str,
    cachebust: bool,
    active_theme: Option<&str>,
    mut read_source: impl FnMut(&str) -> Result<Option<String>, String>,
) -> Result<Vec<WrittenProjectFile>, String> {
    let mut written = Vec::new();
    let Some(template_source) = read_source(template_path)? else {
        return Ok(written);
    };

    let parent_name = extract_extends(&template_source);
    let parent_source = if let Some(parent_name) = parent_name.as_ref() {
        resolve_parent_template_source(template_path, parent_name, active_theme, &mut read_source)?
    } else {
        None
    };

    let next_template = if parent_name.is_some() {
        ensure_page_css_block(&template_source, href, cachebust)
    } else {
        ensure_standalone_stylesheet_link(&template_source, href, cachebust)
    };

    if let Some((base_rel, base_source)) = parent_source {
        let next_base = ensure_base_css_block(&base_source);
        if next_base != base_source {
            written.push(WrittenProjectFile {
                relative_path: base_rel,
                contents: next_base,
            });
        }
    }

    if next_template != template_source {
        written.push(WrittenProjectFile {
            relative_path: template_path.to_string(),
            contents: next_template,
        });
    }

    Ok(written)
}

fn resolve_parent_template_source(
    current_template_path: &str,
    template_reference: &str,
    active_theme: Option<&str>,
    read_source: &mut impl FnMut(&str) -> Result<Option<String>, String>,
) -> Result<Option<(String, String)>, String> {
    let template_name = logical_template_name(template_reference);
    if template_name.is_empty() {
        return Ok(None);
    }

    let local_relative = format!("templates/{template_name}");
    if let Some(source) = read_source(&local_relative)? {
        return Ok(Some((local_relative, source)));
    }

    let theme = theme_name_for_template_path(current_template_path)
        .or_else(|| active_theme.map(ToOwned::to_owned));
    let Some(theme) = theme else {
        return Ok(None);
    };
    let theme_relative = format!("themes/{theme}/templates/{template_name}");
    Ok(read_source(&theme_relative)?.map(|source| (theme_relative, source)))
}

pub fn remove_page_stylesheet_link(content: &str, href: &str) -> String {
    if let Some((_start, _end, inner_start, inner_end)) = locate_tera_block(content, "css_pagina") {
        let inner = &content[inner_start..inner_end];
        let cleaned_inner = remove_link_tags_for_asset(inner, href);
        if cleaned_inner.trim().is_empty() {
            return strip_tera_block(content, "css_pagina");
        }
        return format!(
            "{}{}{}",
            &content[..inner_start],
            cleaned_inner.trim_matches('\n'),
            &content[inner_end..],
        );
    }

    remove_link_tags_for_asset(content, href)
}

pub fn plan_page_stylesheet_link_source(content: &str, href: &str, cachebust: bool) -> String {
    if extract_extends(content).is_some() {
        ensure_page_css_block(content, href, cachebust)
    } else {
        ensure_standalone_stylesheet_link(content, href, cachebust)
    }
}

fn ensure_base_css_block(content: &str) -> String {
    if content.contains("{% block css_pagina %}") {
        return content.to_string();
    }

    let insert = "  {% block css_pagina %}{% endblock %}\n";
    let mut result = content.to_string();
    if let Some(pos) = result.rfind("</head>") {
        result.insert_str(pos, insert);
    } else {
        result.push('\n');
        result.push_str(insert);
    }
    result
}

pub(super) fn ensure_page_css_block(content: &str, href: &str, cachebust: bool) -> String {
    if template_contains_asset_path(content, href) {
        return content.to_string();
    }

    let link = stylesheet_link(href, cachebust);
    if let Some((_start, end, inner_start, inner_end)) = locate_tera_block(content, "css_pagina") {
        let inner = &content[inner_start..inner_end];
        let next_inner = if inner.trim().is_empty() {
            link
        } else {
            format!("{}\n{}", inner.trim_end(), link)
        };
        return format!(
            "{}{}{}",
            &content[..inner_start],
            next_inner,
            &content[inner_end..end]
        ) + &content[end..];
    }

    let block = format!("{{% block css_pagina %}}{}{{% endblock %}}", link);
    if let Some(first_line_end) = content.find('\n') {
        let mut result = String::new();
        result.push_str(&content[..first_line_end + 1]);
        result.push('\n');
        result.push_str(&block);
        result.push('\n');
        result.push_str(&content[first_line_end + 1..]);
        result
    } else {
        format!("{}\n{}\n", content.trim_end(), block)
    }
}

fn ensure_standalone_stylesheet_link(content: &str, href: &str, cachebust: bool) -> String {
    if template_contains_asset_path(content, href) {
        return content.to_string();
    }
    let link = format!("  {}", stylesheet_link(href, cachebust));
    let mut result = content.to_string();
    if let Some(pos) = result.rfind("</head>") {
        result.insert_str(pos, &(link + "\n"));
    } else {
        result.push('\n');
        result.push_str(&link);
        result.push('\n');
    }
    result
}

fn locate_tera_block(content: &str, name: &str) -> Option<(usize, usize, usize, usize)> {
    let start_needle = format!("{{% block {} %}}", name);
    let start = content.find(&start_needle)?;
    let inner_start = start + start_needle.len();
    let after_start = &content[inner_start..];
    let end_rel = after_start.find("{% endblock")?;
    let end_tag_start = inner_start + end_rel;
    let tail = &content[end_tag_start..];
    let close_rel = tail.find("%}")?;
    let end = end_tag_start + close_rel + 2;
    Some((start, end, inner_start, end_tag_start))
}

fn strip_tera_block(content: &str, name: &str) -> String {
    let Some((start, mut end, _inner_start, _inner_end)) = locate_tera_block(content, name) else {
        return content.to_string();
    };
    if content.get(end..end + 1) == Some("\n") {
        end += 1;
    }
    let prefix_end = if start > 0 && content.get(start - 1..start) == Some("\n") {
        start - 1
    } else {
        start
    };
    format!("{}{}", &content[..prefix_end], &content[end..])
}

fn remove_link_tags_for_asset(content: &str, href: &str) -> String {
    let mut result = String::new();
    let mut cursor = 0;
    while let Some(relative_start) = find_ascii_case_insensitive(&content[cursor..], "<link") {
        let start = cursor + relative_start;
        let Some(relative_close) = content[start..].find('>') else {
            break;
        };
        let mut end = start + relative_close + 1;
        if content.get(end..end + 1) == Some("\n") {
            end += 1;
        }
        let tag = &content[start..end];
        result.push_str(&content[cursor..start]);
        if !template_contains_asset_path(tag, href) {
            result.push_str(tag);
        }
        cursor = end;
    }
    result.push_str(&content[cursor..]);
    result
}

fn find_ascii_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

fn extract_extends(content: &str) -> Option<String> {
    for line in content.lines().take(10) {
        let trimmed = line.trim();
        if !trimmed.starts_with("{%") {
            continue;
        }
        let inner = trimmed
            .trim_start_matches("{%-")
            .trim_start_matches("{%")
            .trim_end_matches("-%}")
            .trim_end_matches("%}")
            .trim();
        if let Some(rest) = inner.strip_prefix("extends") {
            let rest = rest.trim();
            if let Some(q) = rest.strip_prefix('"') {
                if let Some(end) = q.find('"') {
                    return Some(q[..end].to_string());
                }
            } else if let Some(q) = rest.strip_prefix('\'') {
                if let Some(end) = q.find('\'') {
                    return Some(q[..end].to_string());
                }
            }
        }
    }
    None
}
