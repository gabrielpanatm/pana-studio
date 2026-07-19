pub fn public_asset_href(path: &str) -> String {
    format!("/{}", normalized_asset_path(path))
}

pub fn asset_url(path: &str, cachebust: bool) -> String {
    let path = normalized_asset_path(path);
    if cachebust {
        format!("{{{{ get_url(path='{}', cachebust=true) }}}}", path)
    } else {
        format!("/{}", path)
    }
}

pub fn stylesheet_link(path: &str, cachebust: bool) -> String {
    format!(
        r#"<link rel="stylesheet" href="{}">"#,
        asset_url(path, cachebust)
    )
}

pub fn script_tag(path: &str, cachebust: bool) -> String {
    format!(
        r#"<script src="{}" defer></script>"#,
        asset_url(path, cachebust)
    )
}

pub fn template_contains_asset_path(content: &str, path: &str) -> bool {
    let path = normalized_asset_path(path);
    if path.is_empty() {
        return false;
    }

    quoted_asset_attribute_references(content, "src", &path)
        || quoted_asset_attribute_references(content, "href", &path)
}

fn quoted_asset_attribute_references(content: &str, attr: &str, expected_path: &str) -> bool {
    let semantic_surface = mask_template_comments(content);
    let mut cursor = 0;
    while let Some(relative_attr_pos) = semantic_surface[cursor..].find(attr) {
        let attr_pos = cursor + relative_attr_pos;
        cursor = attr_pos + attr.len();
        if !is_attribute_name_at(&semantic_surface, attr_pos, attr)
            || !is_inside_html_start_tag(&semantic_surface, attr_pos)
        {
            continue;
        }
        let Some((value_start, value_end)) = locate_quoted_attribute_value(content, cursor) else {
            continue;
        };
        if asset_attribute_value_matches(&content[value_start..value_end], expected_path) {
            return true;
        }
    }
    false
}

fn asset_attribute_value_matches(value: &str, expected_path: &str) -> bool {
    if let Some(path) = extract_get_url_path(value) {
        return normalized_asset_path(&path) == expected_path;
    }
    let plain = value
        .trim()
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .trim_start_matches("./");
    normalized_asset_path(plain) == expected_path
}

pub fn rewrite_template_asset_cachebust(content: &str, cachebust: bool) -> String {
    let with_href = rewrite_asset_attribute(content, "href", cachebust);
    rewrite_asset_attribute(&with_href, "src", cachebust)
}

fn rewrite_asset_attribute(content: &str, attr: &str, cachebust: bool) -> String {
    let semantic_surface = mask_template_comments(content);
    let mut result = String::new();
    let mut index = 0;

    while let Some(relative_attr_pos) = semantic_surface[index..].find(attr) {
        let attr_pos = index + relative_attr_pos;
        if !is_attribute_name_at(&semantic_surface, attr_pos, attr)
            || !is_inside_html_start_tag(&semantic_surface, attr_pos)
        {
            result.push_str(&content[index..attr_pos + attr.len()]);
            index = attr_pos + attr.len();
            continue;
        }

        let Some((value_start, value_end)) =
            locate_quoted_attribute_value(content, attr_pos + attr.len())
        else {
            result.push_str(&content[index..attr_pos + attr.len()]);
            index = attr_pos + attr.len();
            continue;
        };

        let value = &content[value_start..value_end];
        if let Some(next_value) = rewrite_asset_value(value, cachebust) {
            result.push_str(&content[index..value_start]);
            result.push_str(&next_value);
            index = value_end;
        } else {
            result.push_str(&content[index..value_end]);
            index = value_end;
        }
    }

    result.push_str(&content[index..]);
    result
}

fn is_inside_html_start_tag(content: &str, position: usize) -> bool {
    let before = &content[..position];
    let Some(open) = before.rfind('<') else {
        return false;
    };
    if before.rfind('>').is_some_and(|close| close > open) {
        return false;
    }
    let tag = content[open + 1..position].trim_start();
    !tag.is_empty()
        && !tag.starts_with(['/', '!', '?'])
        && tag
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic())
}

fn mask_template_comments(content: &str) -> String {
    let mut bytes = content.as_bytes().to_vec();
    mask_delimited_ranges(content, &mut bytes, "<!--", "-->");
    mask_delimited_ranges(content, &mut bytes, "{#", "#}");
    // Replacing every byte in a comment with ASCII space preserves all byte
    // offsets used to slice the original UTF-8 source.
    String::from_utf8(bytes).expect("comment masking preserves UTF-8")
}

fn mask_delimited_ranges(content: &str, bytes: &mut [u8], start: &str, end: &str) {
    let mut cursor = 0;
    while let Some(relative_start) = content[cursor..].find(start) {
        let range_start = cursor + relative_start;
        let after_start = range_start + start.len();
        let range_end = content[after_start..]
            .find(end)
            .map(|relative_end| after_start + relative_end + end.len())
            .unwrap_or(content.len());
        bytes[range_start..range_end].fill(b' ');
        cursor = range_end;
    }
}

fn is_attribute_name_at(content: &str, attr_pos: usize, attr: &str) -> bool {
    let before_ok = content[..attr_pos]
        .chars()
        .last()
        .map(|character| !is_attribute_name_character(character))
        .unwrap_or(true);
    let after_pos = attr_pos + attr.len();
    let after_ok = content[after_pos..]
        .chars()
        .next()
        .map(|character| character.is_whitespace() || character == '=')
        .unwrap_or(false);
    before_ok && after_ok
}

fn is_attribute_name_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | ':')
}

fn locate_quoted_attribute_value(content: &str, after_attr_pos: usize) -> Option<(usize, usize)> {
    let mut cursor = skip_ascii_whitespace(content, after_attr_pos);
    if content[cursor..].chars().next()? != '=' {
        return None;
    }
    cursor += 1;
    cursor = skip_ascii_whitespace(content, cursor);
    let quote = content[cursor..].chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }

    let value_start = cursor + quote.len_utf8();
    if content[value_start..].starts_with("{{") {
        if let Some(end_expression) = content[value_start..].find("}}") {
            let expression_end = value_start + end_expression + 2;
            let after_expression = skip_ascii_whitespace(content, expression_end);
            if content[after_expression..].chars().next() == Some(quote) {
                return Some((value_start, expression_end));
            }
        }
    }

    content[value_start..]
        .find(quote)
        .map(|relative_end| (value_start, value_start + relative_end))
}

fn skip_ascii_whitespace(content: &str, mut cursor: usize) -> usize {
    while let Some(character) = content[cursor..].chars().next() {
        if !character.is_ascii_whitespace() {
            break;
        }
        cursor += character.len_utf8();
    }
    cursor
}

fn rewrite_asset_value(value: &str, cachebust: bool) -> Option<String> {
    if let Some(path) = extract_get_url_path(value) {
        if !is_css_or_js_asset_path(&path) {
            return None;
        }
        return if cachebust {
            let next = asset_url(&path, true);
            (next != value).then_some(next)
        } else {
            Some(public_asset_href(&path))
        };
    }

    if cachebust && is_plain_local_css_or_js_asset(value) {
        return Some(asset_url(value, true));
    }

    None
}

fn extract_get_url_path(value: &str) -> Option<String> {
    if !value.contains("get_url") {
        return None;
    }
    let path_pos = value.find("path")?;
    let after_path = path_pos + "path".len();
    let equal_relative = value[after_path..].find('=')?;
    let mut cursor = after_path + equal_relative + 1;
    cursor = skip_ascii_whitespace(value, cursor);
    let quote = value[cursor..].chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let path_start = cursor + quote.len_utf8();
    let path_end = value[path_start..].find(quote)? + path_start;
    Some(value[path_start..path_end].to_string())
}

fn is_plain_local_css_or_js_asset(value: &str) -> bool {
    let trimmed = value.trim();
    if !trimmed.starts_with('/') || trimmed.starts_with("//") {
        return false;
    }
    if trimmed.contains("{{")
        || trimmed.contains("}}")
        || trimmed.contains('?')
        || trimmed.contains('#')
        || trimmed.contains("://")
    {
        return false;
    }
    is_css_or_js_asset_path(trimmed)
}

fn is_css_or_js_asset_path(path: &str) -> bool {
    let normalized = normalized_asset_path(path);
    normalized.ends_with(".css") || normalized.ends_with(".js")
}

fn normalized_asset_path(path: &str) -> String {
    path.trim().trim_start_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_plain_and_cachebusted_asset_urls() {
        assert_eq!(asset_url("/pagini/index.css", false), "/pagini/index.css");
        assert_eq!(
            asset_url("/pagini/index.css", true),
            "{{ get_url(path='pagini/index.css', cachebust=true) }}"
        );
    }

    #[test]
    fn detects_plain_and_get_url_asset_paths() {
        assert!(template_contains_asset_path(
            r#"<link rel="stylesheet" href="/pagini/index.css">"#,
            "/pagini/index.css"
        ));
        assert!(template_contains_asset_path(
            r#"<link rel="stylesheet" href="{{ get_url(path="pagini/index.css", cachebust=true) }}">"#,
            "/pagini/index.css"
        ));
        assert!(template_contains_asset_path(
            r#"<script src="{{ get_url(path='js/pana-index.js', cachebust=true) }}" defer></script>"#,
            "js/pana-index.js"
        ));
        assert!(template_contains_asset_path(
            r#"<script src="{{ get_url(path = 'js/pana-index.js', cachebust=true) }}"></script>"#,
            "js/pana-index.js"
        ));
        assert!(template_contains_asset_path(
            r#"<script src="js/pana-index.js?rev=2"></script>"#,
            "js/pana-index.js"
        ));
        assert!(!template_contains_asset_path(
            r#"<script src="js/not-pana-index.js"></script>"#,
            "js/pana-index.js"
        ));
        assert!(!template_contains_asset_path(
            r#"<script src="js/pana-index.js.map"></script>"#,
            "js/pana-index.js"
        ));
        assert!(!template_contains_asset_path(
            "{# js/pana-index.js #}<p>js/pana-index.js</p>",
            "js/pana-index.js"
        ));
        assert!(!template_contains_asset_path(
            r#"{# <script src="/js/pana-index.js"></script> #}"#,
            "js/pana-index.js"
        ));
        assert!(!template_contains_asset_path(
            r#"<!-- <script src="/js/pana-index.js"></script> -->"#,
            "js/pana-index.js"
        ));
        assert!(!template_contains_asset_path(
            r#"{% set example = 'src="/js/pana-index.js"' %}"#,
            "js/pana-index.js"
        ));
    }

    #[test]
    fn rewrites_plain_css_and_js_assets_to_cachebusted_urls() {
        let source = r#"<link rel="stylesheet" href="/pagini/index.css">
<script src="/js/pana-index.js" defer></script>
<img src="/imagini/logo.png">"#;
        let result = rewrite_template_asset_cachebust(source, true);
        assert!(result.contains(r#"href="{{ get_url(path='pagini/index.css', cachebust=true) }}""#));
        assert!(result.contains(r#"src="{{ get_url(path='js/pana-index.js', cachebust=true) }}""#));
        assert!(result.contains(r#"<img src="/imagini/logo.png">"#));
    }

    #[test]
    fn rewrites_cachebusted_css_and_js_assets_to_plain_urls() {
        let source = r#"<link rel="stylesheet" href="{{ get_url(path="pagini/index.css", cachebust=true) }}">
<script src="{{ get_url(path='js/pana-index.js', cachebust=true) }}" defer></script>"#;
        let result = rewrite_template_asset_cachebust(source, false);
        assert!(result.contains(r#"href="/pagini/index.css""#));
        assert!(result.contains(r#"src="/js/pana-index.js""#));
    }

    #[test]
    fn does_not_rewrite_external_or_non_asset_links() {
        let source = r#"<a href="/contact">Contact</a>
<script src="https://cdn.example.test/app.js"></script>
<link rel="stylesheet" href="/style.css?v=1">"#;
        let result = rewrite_template_asset_cachebust(source, true);
        assert_eq!(result, source);
    }

    #[test]
    fn does_not_rewrite_asset_markup_inside_html_or_tera_comments() {
        let source = r#"{# <script src="/js/pana-index.js"></script> #}
<!-- <link href="/pagini/index.css"> -->"#;
        assert_eq!(rewrite_template_asset_cachebust(source, true), source);
    }
}
