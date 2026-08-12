use crate::source_graph::{
    mixed_cst::{MixedCstDocument, MixedCstKind},
    zola::{
        normalize_static_asset_reference, static_asset_reference, static_asset_reference_from_style,
    },
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct AssetReferenceScan {
    pub references: Vec<String>,
    pub unanalysable: usize,
}

impl AssetReferenceScan {
    fn push_static(&mut self, value: &str) {
        if let Some(reference) = static_asset_reference(value) {
            self.references
                .push(normalize_static_asset_reference(&reference));
        }
    }

    pub(crate) fn eligible(&self) -> usize {
        self.references.len().saturating_add(self.unanalysable)
    }
}

pub(crate) fn scan_html_asset_references(
    source: &str,
    document: &MixedCstDocument,
) -> AssetReferenceScan {
    let mut result = AssetReferenceScan::default();
    for node in &document.nodes {
        let MixedCstKind::StartTag(tag) = &node.kind else {
            continue;
        };
        for attribute in &tag.attributes {
            let name = attribute.name.to_ascii_lowercase();
            if !is_asset_attribute(&tag.name, &name) {
                continue;
            }
            let Some(value) = attribute
                .value_start
                .zip(attribute.value_end)
                .and_then(|(start, end)| source.get(start..end))
            else {
                continue;
            };
            if !attribute.embedded_tera.is_empty() || contains_dynamic_template_expression(value) {
                result.unanalysable = result.unanalysable.saturating_add(1);
                continue;
            }
            if tag.name.eq_ignore_ascii_case("a") && !looks_like_resource_href(value) {
                continue;
            }
            if name == "srcset" {
                for candidate in srcset_urls(value) {
                    result.push_static(candidate);
                }
            } else {
                result.push_static(value);
            }
        }
    }
    result.references.sort();
    result.references.dedup();
    result
}

fn looks_like_resource_href(value: &str) -> bool {
    let path = value.split(['?', '#']).next().unwrap_or(value);
    path.rsplit('/')
        .find(|segment| !segment.is_empty())
        .is_some_and(|segment| segment.contains('.') && !segment.ends_with(".html"))
}

fn is_asset_attribute(tag: &str, attribute: &str) -> bool {
    match attribute {
        "src" | "srcset" | "poster" => true,
        "href" | "xlink:href" => matches!(
            tag.to_ascii_lowercase().as_str(),
            "link" | "use" | "image" | "script" | "a"
        ),
        _ => false,
    }
}

fn contains_dynamic_template_expression(value: &str) -> bool {
    value.contains("{{") || value.contains("{%") || value.contains("{#")
}

fn srcset_urls(value: &str) -> impl Iterator<Item = &str> {
    let mut urls = Vec::new();
    let bytes = value.as_bytes();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        while cursor < bytes.len() && (bytes[cursor].is_ascii_whitespace() || bytes[cursor] == b',')
        {
            cursor += 1;
        }
        if cursor >= bytes.len() {
            break;
        }
        let start = cursor;
        let data_url = value[start..].to_ascii_lowercase().starts_with("data:");
        while cursor < bytes.len()
            && !bytes[cursor].is_ascii_whitespace()
            && (data_url || bytes[cursor] != b',')
        {
            cursor += 1;
        }
        if let Some(url) = value.get(start..cursor).filter(|url| !url.is_empty()) {
            urls.push(url);
        }
        while cursor < bytes.len() && bytes[cursor] != b',' {
            cursor += 1;
        }
    }
    urls.into_iter()
}

pub(crate) fn scan_css_asset_references(source: &str, source_file: &str) -> AssetReferenceScan {
    let bytes = source.as_bytes();
    let mut result = AssetReferenceScan::default();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        if starts_with(bytes, cursor, b"/*") {
            cursor = find_bytes(bytes, cursor + 2, b"*/")
                .map(|end| end + 2)
                .unwrap_or(bytes.len());
            continue;
        }
        if matches!(bytes[cursor], b'\'' | b'"') {
            cursor = scan_quoted(bytes, cursor).unwrap_or(bytes.len());
            continue;
        }
        if is_identifier_start(bytes[cursor]) {
            let identifier_start = cursor;
            cursor += 1;
            while cursor < bytes.len() && is_identifier_continue(bytes[cursor]) {
                cursor += 1;
            }
            let is_url = source
                .get(identifier_start..cursor)
                .is_some_and(|identifier| identifier.eq_ignore_ascii_case("url"));
            let opening = match skip_css_trivia(bytes, cursor) {
                Some(opening) => opening,
                None if is_url => {
                    result.unanalysable = result.unanalysable.saturating_add(1);
                    break;
                }
                None => bytes.len(),
            };
            if is_url && bytes.get(opening) == Some(&b'(') {
                match scan_css_url(source, opening + 1) {
                    CssUrlScan::Static { value, end } => {
                        if let Some(reference) =
                            static_asset_reference_from_style(value, source_file)
                        {
                            result.references.push(reference);
                        }
                        cursor = end;
                    }
                    CssUrlScan::Ignored { end } => cursor = end,
                    CssUrlScan::Unanalysable { end } => {
                        result.unanalysable = result.unanalysable.saturating_add(1);
                        cursor = end;
                    }
                }
            }
            continue;
        }
        cursor += char_width(source, cursor);
    }
    result.references.sort();
    result.references.dedup();
    result
}

enum CssUrlScan<'a> {
    Static { value: &'a str, end: usize },
    Ignored { end: usize },
    Unanalysable { end: usize },
}

fn scan_css_url(source: &str, mut cursor: usize) -> CssUrlScan<'_> {
    let bytes = source.as_bytes();
    let Some(next) = skip_css_trivia(bytes, cursor) else {
        return CssUrlScan::Unanalysable { end: bytes.len() };
    };
    cursor = next;
    if cursor >= bytes.len() {
        return CssUrlScan::Unanalysable { end: bytes.len() };
    }
    let (start, value_end, end) = if matches!(bytes[cursor], b'\'' | b'"') {
        let quote = bytes[cursor];
        let start = cursor + 1;
        let Some(after_quote) = scan_quoted(bytes, cursor) else {
            return CssUrlScan::Unanalysable { end: bytes.len() };
        };
        let value_end = after_quote - 1;
        cursor = after_quote;
        let Some(next) = skip_css_trivia(bytes, cursor) else {
            return CssUrlScan::Unanalysable { end: bytes.len() };
        };
        cursor = next;
        if bytes.get(cursor) != Some(&b')') || bytes.get(value_end) != Some(&quote) {
            return CssUrlScan::Unanalysable {
                end: skip_to_closing_paren(bytes, cursor),
            };
        }
        (start, value_end, cursor + 1)
    } else {
        let start = cursor;
        let mut nested = false;
        while cursor < bytes.len() && bytes[cursor] != b')' {
            if starts_with(bytes, cursor, b"/*") {
                nested = true;
                cursor = find_bytes(bytes, cursor + 2, b"*/")
                    .map(|finish| finish + 2)
                    .unwrap_or(bytes.len());
                continue;
            }
            if bytes[cursor] == b'(' {
                nested = true;
            }
            if bytes[cursor] == b'\\' {
                cursor = (cursor + 2).min(bytes.len());
            } else {
                cursor += char_width(source, cursor);
            }
        }
        if cursor >= bytes.len() || nested {
            return CssUrlScan::Unanalysable {
                end: skip_to_closing_paren(bytes, cursor),
            };
        }
        let value_end = source
            .get(start..cursor)
            .map(str::trim_end)
            .map(|value| start + value.len())
            .unwrap_or(cursor);
        (start, value_end, cursor + 1)
    };
    let Some(value) = source.get(start..value_end).map(str::trim) else {
        return CssUrlScan::Unanalysable { end };
    };
    if value.is_empty() {
        return CssUrlScan::Ignored { end };
    }
    if value.contains("#{") || value.contains("${") || value.starts_with('$') {
        return CssUrlScan::Unanalysable { end };
    }
    CssUrlScan::Static { value, end }
}

fn skip_css_trivia(bytes: &[u8], mut cursor: usize) -> Option<usize> {
    loop {
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if !starts_with(bytes, cursor, b"/*") {
            return Some(cursor);
        }
        cursor = find_bytes(bytes, cursor + 2, b"*/")?.saturating_add(2);
    }
}

fn scan_quoted(bytes: &[u8], opening: usize) -> Option<usize> {
    let quote = *bytes.get(opening)?;
    let mut cursor = opening + 1;
    while cursor < bytes.len() {
        if bytes[cursor] == b'\\' {
            cursor = (cursor + 2).min(bytes.len());
        } else if bytes[cursor] == quote {
            return Some(cursor + 1);
        } else {
            cursor += 1;
        }
    }
    None
}

fn skip_to_closing_paren(bytes: &[u8], mut cursor: usize) -> usize {
    while cursor < bytes.len() && bytes[cursor] != b')' {
        cursor += 1;
    }
    (cursor + usize::from(cursor < bytes.len())).min(bytes.len())
}

fn starts_with(bytes: &[u8], cursor: usize, needle: &[u8]) -> bool {
    bytes.get(cursor..cursor.saturating_add(needle.len())) == Some(needle)
}

fn find_bytes(bytes: &[u8], mut cursor: usize, needle: &[u8]) -> Option<usize> {
    while cursor.saturating_add(needle.len()) <= bytes.len() {
        if starts_with(bytes, cursor, needle) {
            return Some(cursor);
        }
        cursor += 1;
    }
    None
}

fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'-')
}

fn is_identifier_continue(byte: u8) -> bool {
    is_identifier_start(byte) || byte.is_ascii_digit()
}

fn char_width(source: &str, cursor: usize) -> usize {
    source[cursor..]
        .chars()
        .next()
        .map(char::len_utf8)
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::source_graph::mixed_cst::parse_mixed_cst;

    #[test]
    fn html_scan_covers_literal_asset_attributes_and_dynamic_values() {
        let source = r#"<img src="/images/ș.png?v=1#x" srcset="a.webp 1x, b.webp 2x"><video poster='p.jpg'></video><link href="/css/site.css"><use xlink:href="icons.svg#ok"></use><a href="/blog/">Blog</a><img src="{{ image }}">"#;
        let document = parse_mixed_cst(source, "index.html");
        let scan = scan_html_asset_references(source, &document);
        assert_eq!(
            scan.references,
            vec![
                "a.webp",
                "b.webp",
                "css/site.css",
                "icons.svg",
                "images/ș.png",
                "p.jpg"
            ]
        );
        assert_eq!(scan.unanalysable, 1);
    }

    #[test]
    fn srcset_ignores_data_urls_without_inventing_a_local_tail() {
        let source = r#"<img srcset="data:image/svg+xml,%3Csvg%3E 1x, /images/real.png 2x">"#;
        let document = parse_mixed_cst(source, "index.html");
        let scan = scan_html_asset_references(source, &document);
        assert_eq!(scan.references, vec!["images/real.png"]);
        assert_eq!(scan.unanalysable, 0);
    }

    #[test]
    fn css_scan_is_comment_string_and_interpolation_aware() {
        let source = r#"
            /* url(fake.png) */
            .a { background: url( "../images/a b.png?x=1#y" ); }
            .b::after { content: "url(ignored.png)"; }
            .c { mask: URL('/icons/mask.svg'); }
            .d { background: url(#{dynamic}); }
            .e { background: url(data:image/png;base64,abc); }
            .f { background: url( /* source */ "../images/commented.png" /* tail */ ); }
        "#;
        let scan = scan_css_asset_references(source, "static/css/site.css");
        assert_eq!(
            scan.references,
            vec!["icons/mask.svg", "images/a b.png", "images/commented.png"]
        );
        assert_eq!(scan.unanalysable, 1);
    }

    #[test]
    fn css_asset_scanner_scales_with_references_not_assets_times_sources() {
        fn measurement(count: usize) -> (usize, u128) {
            let source = (0..count)
                .map(|index| format!(".i{index}{{background:url('/images/{index}.png')}}\n"))
                .collect::<String>();
            let started = Instant::now();
            let scan = scan_css_asset_references(&source, "static/css/site.css");
            (scan.references.len(), started.elapsed().as_micros())
        }

        let (small_count, small_us) = measurement(1_000);
        let (large_count, large_us) = measurement(10_000);
        eprintln!("[Pană Studio][perf] asset_reference_scan 1k_us={small_us} 10k_us={large_us}");
        assert_eq!(small_count, 1_000);
        assert_eq!(large_count, 10_000);
        assert!(large_us <= small_us.saturating_mul(30).max(1));
    }
}
