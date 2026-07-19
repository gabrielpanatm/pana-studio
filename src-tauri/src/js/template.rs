use crate::zola_links::{script_tag, template_contains_asset_path};

pub fn extract_extends(content: &str) -> Option<String> {
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
        if inner.starts_with("extends") {
            let rest = inner["extends".len()..].trim();
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

pub fn page_scripts_html(js_slug: &str, has_anime: bool, cachebust: bool) -> String {
    let mut scripts_html = String::new();
    if has_anime {
        scripts_html.push_str("  ");
        scripts_html.push_str(&script_tag("js/anime.min.js", cachebust));
        scripts_html.push('\n');
    }
    scripts_html.push_str("  ");
    scripts_html.push_str(&script_tag(&format!("js/{}.js", js_slug), cachebust));
    scripts_html.push('\n');
    scripts_html
}

pub fn ensure_base_scripts_block(content: &str) -> String {
    if content.contains("{% block scripts %}") {
        return content.to_string();
    }
    let insert = "  {% block scripts %}{% endblock scripts %}\n";
    let mut result = content.to_string();
    if let Some(pos) = result.rfind("</body>") {
        result.insert_str(pos, insert);
    } else {
        result.push('\n');
        result.push_str(insert);
    }
    result
}

pub fn ensure_page_scripts_block(content: &str, scripts_html: &str) -> String {
    let new_block = String::from("{% block scripts %}\n{{ super() }}\n")
        + scripts_html
        + "{% endblock scripts %}";

    let stripped = strip_scripts_block(content);
    format!("{}\n{}\n", stripped.trim_end(), new_block)
}

pub fn ensure_script_tags(
    template: &str,
    js_slug: &str,
    has_anime: bool,
    cachebust: bool,
) -> String {
    let normalized_template = if has_anime {
        template.to_string()
    } else {
        remove_script_tags_for_asset(template, "js/anime.min.js")
    };
    let js_path = format!("js/{}.js", js_slug);
    let has_anime_tag = template_contains_asset_path(&normalized_template, "js/anime.min.js");
    let has_js_tag = template_contains_asset_path(&normalized_template, &js_path);

    if (!has_anime || has_anime_tag) && has_js_tag {
        return normalized_template;
    }

    let mut insert = String::new();
    if has_anime && !has_anime_tag {
        insert.push_str("  ");
        insert.push_str(&script_tag("js/anime.min.js", cachebust));
        insert.push('\n');
    }
    if !has_js_tag {
        insert.push_str("  ");
        insert.push_str(&script_tag(&js_path, cachebust));
        insert.push('\n');
    }

    if insert.is_empty() {
        return template.to_string();
    }

    let mut result = normalized_template;
    if let Some(pos) = result.rfind("</body>") {
        result.insert_str(pos, &insert);
        return result;
    }

    result.push('\n');
    result.push_str(&insert);
    result
}

pub fn remove_page_scripts_contract(content: &str, js_slug: &str) -> String {
    let js_path = format!("js/{}.js", js_slug);

    if let Some((_start, _end, inner_start, inner_end)) = locate_scripts_block(content) {
        let inner = &content[inner_start..inner_end];
        let without_page_js = remove_script_tags_for_asset(inner, &js_path);
        let without_anime = remove_script_tags_for_asset(&without_page_js, "js/anime.min.js");
        let remaining = without_anime.trim();
        if remaining.is_empty() || remaining == "{{ super() }}" {
            return strip_scripts_block(content);
        }
        return format!(
            "{}{}{}",
            &content[..inner_start],
            without_anime.trim_matches('\n'),
            &content[inner_end..],
        );
    }

    let without_page_js = remove_script_tags_for_asset(content, &js_path);
    remove_script_tags_for_asset(&without_page_js, "js/anime.min.js")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_plain_page_script_tags() {
        let result = page_scripts_html("pana-index", true, false);
        assert!(result.contains(r#"<script src="/js/anime.min.js" defer></script>"#));
        assert!(result.contains(r#"<script src="/js/pana-index.js" defer></script>"#));
    }

    #[test]
    fn builds_cachebusted_page_script_tags() {
        let result = page_scripts_html("pana-index", true, true);
        assert!(result.contains("{{ get_url(path='js/anime.min.js', cachebust=true) }}"));
        assert!(result.contains("{{ get_url(path='js/pana-index.js', cachebust=true) }}"));
    }

    #[test]
    fn detects_existing_cachebusted_script_tag() {
        let source = r#"<body>
  <script src="{{ get_url(path="js/pana-index.js", cachebust=true) }}" defer></script>
</body>"#;
        let result = ensure_script_tags(source, "pana-index", false, false);
        assert_eq!(result, source);
    }

    #[test]
    fn inserts_cachebusted_script_tag_when_requested() {
        let source = "<body></body>";
        let result = ensure_script_tags(source, "pana-index", false, true);
        assert!(result.contains("{{ get_url(path='js/pana-index.js', cachebust=true) }}"));
    }

    #[test]
    fn path_mentioned_only_in_text_or_comment_does_not_suppress_script_tag() {
        let source = "<body>{# js/pana-index.js #}<p>js/pana-index.js</p></body>";
        let result = ensure_script_tags(source, "pana-index", false, false);

        assert!(result.contains(r#"<script src="/js/pana-index.js" defer></script>"#));
    }

    #[test]
    fn removes_anime_tag_when_page_js_no_longer_has_motion() {
        let source = r#"<body>
  <script src="/js/anime.min.js" defer></script>
  <script src="/js/pana-index.js" defer></script>
</body>"#;
        let result = ensure_script_tags(source, "pana-index", false, false);
        assert!(!result.contains("anime.min.js"));
        assert!(result.contains("pana-index.js"));
    }

    #[test]
    fn removes_generated_scripts_block_when_contract_is_empty() {
        let source = r#"{% extends "base.html" %}
{% block scripts %}
{{ super() }}
  <script src="/js/pana-index.js" defer></script>
{% endblock scripts %}
"#;
        let result = remove_page_scripts_contract(source, "pana-index");
        assert!(!result.contains("pana-index.js"));
        assert!(!result.contains("{% block scripts %}"));
    }

    #[test]
    fn keeps_manual_scripts_when_removing_page_contract() {
        let source = r#"{% block scripts %}
{{ super() }}
  <script src="/js/pana-index.js" defer></script>
  <script src="/js/manual.js" defer></script>
{% endblock scripts %}
"#;
        let result = remove_page_scripts_contract(source, "pana-index");
        assert!(!result.contains("pana-index.js"));
        assert!(result.contains("manual.js"));
        assert!(result.contains("{% block scripts %}"));
    }

    #[test]
    fn removes_cachebusted_page_script_contract() {
        let source = r#"{% block scripts %}
{{ super() }}
  <script src="{{ get_url(path='js/pana-index.js', cachebust=true) }}" defer></script>
{% endblock scripts %}
"#;
        let result = remove_page_scripts_contract(source, "pana-index");
        assert!(!result.contains("pana-index.js"));
        assert!(!result.contains("{% block scripts %}"));
    }
}

fn strip_scripts_block(content: &str) -> String {
    let Some(start) = content.find("{% block scripts %}") else {
        return content.to_string();
    };
    let after = &content[start..];
    let Some(rel) = after.find("{% endblock") else {
        return content.to_string();
    };
    let end_tag_start = start + rel;
    let tail = &content[end_tag_start..];
    let Some(close) = tail.find("%}") else {
        return content.to_string();
    };
    let mut block_end = end_tag_start + close + 2;
    if content.get(block_end..block_end + 1) == Some("\n") {
        block_end += 1;
    }
    let prefix_end = if start > 0 && content.get(start - 1..start) == Some("\n") {
        start - 1
    } else {
        start
    };
    format!("{}{}", &content[..prefix_end], &content[block_end..])
}

fn locate_scripts_block(content: &str) -> Option<(usize, usize, usize, usize)> {
    let start_needle = "{% block scripts %}";
    let start = content.find(start_needle)?;
    let inner_start = start + start_needle.len();
    let after_start = &content[inner_start..];
    let end_rel = after_start.find("{% endblock")?;
    let end_tag_start = inner_start + end_rel;
    let tail = &content[end_tag_start..];
    let close_rel = tail.find("%}")?;
    let mut end = end_tag_start + close_rel + 2;
    if content.get(end..end + 1) == Some("\n") {
        end += 1;
    }
    Some((start, end, inner_start, end_tag_start))
}

fn remove_script_tags_for_asset(content: &str, asset_path: &str) -> String {
    let mut result = String::new();
    let mut cursor = 0;
    while let Some(relative_start) = content[cursor..].find("<script") {
        let start = cursor + relative_start;
        let Some(relative_close) = content[start..].find("</script>") else {
            break;
        };
        let mut end = start + relative_close + "</script>".len();
        if content.get(end..end + 1) == Some("\n") {
            end += 1;
        }
        let script = &content[start..end];
        if template_contains_asset_path(script, asset_path) {
            result.push_str(&content[cursor..start]);
        } else {
            result.push_str(&content[cursor..end]);
        }
        cursor = end;
    }
    result.push_str(&content[cursor..]);
    result
}
