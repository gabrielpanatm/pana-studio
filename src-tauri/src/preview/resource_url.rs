use std::{collections::BTreeMap, fs, path::Path};

use percent_encoding::percent_decode_str;
use sha2::{Digest, Sha256};

use super::canvas::{CanvasResourceEntry, CanvasResourceKind};

pub(super) struct StylesheetArtifactRevision {
    pub url: String,
    pub content_hash: String,
    pub size_bytes: u64,
    pub artifact_path: std::path::PathBuf,
    pub body: Vec<u8>,
}

struct ResourceIdentity<'a> {
    content_hash: &'a str,
    is_stylesheet: bool,
}

pub(super) fn revision_stylesheet_artifacts(
    artifact_root: &Path,
    preview_revision: &str,
    entries: &[CanvasResourceEntry],
) -> Result<Vec<StylesheetArtifactRevision>, String> {
    let resources = entries
        .iter()
        .map(|entry| {
            (
                entry.url.as_str(),
                ResourceIdentity {
                    content_hash: entry.content_hash.as_str(),
                    is_stylesheet: entry.kind == CanvasResourceKind::Stylesheet,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut prepared = Vec::new();

    for entry in entries.iter().filter(|entry| {
        entry.kind == CanvasResourceKind::Stylesheet
            && entry.url.to_ascii_lowercase().ends_with(".css")
    }) {
        let artifact_path = artifact_root.join(entry.url.trim_start_matches('/'));
        let body = fs::read(&artifact_path).map_err(|error| {
            format!(
                "Versionarea resurselor CSS nu a putut citi {}: {error}",
                artifact_path.display()
            )
        })?;
        let source = std::str::from_utf8(&body).map_err(|_| {
            format!(
                "Versionarea resurselor CSS cere UTF-8 pentru {}.",
                entry.url
            )
        })?;
        let revised = revise_stylesheet_source(source, &entry.url, preview_revision, &resources);
        if revised == source {
            continue;
        }
        prepared.push((artifact_path, entry.url.clone(), revised.into_bytes()));
    }

    let mut revisions = Vec::with_capacity(prepared.len());
    for (artifact_path, url, body) in prepared {
        revisions.push(StylesheetArtifactRevision {
            url,
            content_hash: format!("sha256-{}", digest_hex(&body)),
            size_bytes: body.len() as u64,
            artifact_path,
            body,
        });
    }
    Ok(revisions)
}

fn revise_stylesheet_source(
    source: &str,
    stylesheet_url: &str,
    preview_revision: &str,
    resources: &BTreeMap<&str, ResourceIdentity<'_>>,
) -> String {
    let bytes = source.as_bytes();
    let mut output = String::with_capacity(source.len());
    let mut copy_from = 0usize;
    let mut cursor = 0usize;

    while cursor < bytes.len() {
        if bytes[cursor..].starts_with(b"/*") {
            cursor = skip_css_comment(bytes, cursor);
            continue;
        }
        if matches!(bytes[cursor], b'\'' | b'"') {
            cursor = skip_css_string(bytes, cursor, bytes[cursor]);
            continue;
        }
        let Some(parsed) = parse_url_function(bytes, cursor) else {
            cursor += 1;
            continue;
        };
        let value = &source[parsed.value_start..parsed.value_end];
        if let Some(revised) =
            revise_css_resource_url(value, stylesheet_url, preview_revision, resources)
        {
            output.push_str(&source[copy_from..parsed.value_start]);
            output.push_str(&revised);
            copy_from = parsed.value_end;
        }
        cursor = parsed.function_end;
    }

    if copy_from == 0 {
        return source.to_string();
    }
    output.push_str(&source[copy_from..]);
    output
}

struct ParsedCssUrl {
    value_start: usize,
    value_end: usize,
    function_end: usize,
}

fn parse_url_function(bytes: &[u8], start: usize) -> Option<ParsedCssUrl> {
    if start > 0 && is_css_identifier_byte(bytes[start - 1]) {
        return None;
    }
    let prefix = bytes.get(start..start.checked_add(4)?)?;
    if !prefix[..3].eq_ignore_ascii_case(b"url") || prefix[3] != b'(' {
        return None;
    }
    let mut cursor = start + 4;
    while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
        cursor += 1;
    }
    let quote = bytes
        .get(cursor)
        .copied()
        .filter(|byte| matches!(byte, b'\'' | b'"'));
    let mut value_start = cursor + usize::from(quote.is_some());
    let mut value_end;

    if let Some(quote) = quote {
        cursor += 1;
        loop {
            match bytes.get(cursor).copied()? {
                b'\\' => cursor = cursor.checked_add(2)?,
                byte if byte == quote => {
                    value_end = cursor;
                    cursor += 1;
                    break;
                }
                _ => cursor += 1,
            }
        }
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b')') {
            return None;
        }
    } else {
        loop {
            match bytes.get(cursor).copied()? {
                b'\\' => cursor = cursor.checked_add(2)?,
                b')' => {
                    value_end = cursor;
                    break;
                }
                b'\'' | b'"' => return None,
                _ => cursor += 1,
            }
        }
        while value_start < value_end && bytes[value_start].is_ascii_whitespace() {
            value_start += 1;
        }
        while value_end > value_start && bytes[value_end - 1].is_ascii_whitespace() {
            value_end -= 1;
        }
    }

    Some(ParsedCssUrl {
        value_start,
        value_end,
        function_end: cursor + 1,
    })
}

fn revise_css_resource_url(
    value: &str,
    stylesheet_url: &str,
    preview_revision: &str,
    resources: &BTreeMap<&str, ResourceIdentity<'_>>,
) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.contains(['\\', '$', '{', '}']) || trimmed.starts_with('#') {
        return None;
    }
    let resource_path = local_resource_path(stylesheet_url, trimmed)?;
    let (identity_name, identity) = match resources.get(resource_path.as_str()) {
        Some(resource) if !resource.is_stylesheet => {
            ("__pana_resource_hash", resource.content_hash)
        }
        _ => ("__pana_preview_revision", preview_revision),
    };
    let revised = replace_internal_resource_query(trimmed, identity_name, identity);
    (revised != trimmed).then_some(revised)
}

fn skip_css_comment(bytes: &[u8], start: usize) -> usize {
    bytes[start + 2..]
        .windows(2)
        .position(|window| window == b"*/")
        .map(|offset| start + 2 + offset + 2)
        .unwrap_or(bytes.len())
}

fn skip_css_string(bytes: &[u8], start: usize, quote: u8) -> usize {
    let mut cursor = start + 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' => cursor = (cursor + 2).min(bytes.len()),
            byte if byte == quote => return cursor + 1,
            _ => cursor += 1,
        }
    }
    bytes.len()
}

fn is_css_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'\\')
}

pub(super) fn replace_internal_resource_query(value: &str, name: &str, identity: &str) -> String {
    let (without_fragment, fragment) = value
        .split_once('#')
        .map(|(head, tail)| (head, Some(tail)))
        .unwrap_or((value, None));
    let (path, query) = without_fragment
        .split_once('?')
        .map(|(path, query)| (path, Some(query)))
        .unwrap_or((without_fragment, None));
    let mut pairs = query
        .into_iter()
        .flat_map(|query| query.split('&'))
        .filter(|pair| {
            let key = pair.split_once('=').map(|(key, _)| key).unwrap_or(pair);
            !matches!(key, "__pana_preview_revision" | "__pana_resource_hash")
        })
        .filter(|pair| !pair.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    pairs.push(format!("{name}={identity}"));
    let mut revised = format!("{path}?{}", pairs.join("&"));
    if let Some(fragment) = fragment {
        revised.push('#');
        revised.push_str(fragment);
    }
    revised
}

pub(super) fn local_resource_path(document_route: &str, value: &str) -> Option<String> {
    let trimmed = value.trim();
    let without_query = trimmed.split(['?', '#']).next().unwrap_or_default();
    if without_query.is_empty() || without_query.starts_with("//") {
        return None;
    }
    let resource_path = if let Some(scheme) = without_query.find("://") {
        let origin = &without_query[..scheme].to_ascii_lowercase();
        let authority_and_path = &without_query[scheme + 3..];
        let (authority, path) = authority_and_path
            .split_once('/')
            .map(|(authority, path)| (authority, format!("/{path}")))
            .unwrap_or((authority_and_path, "/".to_string()));
        let local_http = origin == "http"
            && (authority.starts_with("127.0.0.1:") || authority.starts_with("localhost:"));
        if !local_http {
            return None;
        }
        path
    } else {
        let first_separator = without_query.find('/').unwrap_or(without_query.len());
        if without_query[..first_separator].contains(':') {
            return None;
        }
        without_query.to_string()
    };
    let decoded = percent_decode_str(&resource_path).decode_utf8().ok()?;
    let absolute = if decoded.starts_with('/') {
        decoded.into_owned()
    } else {
        let route = if document_route.starts_with('/') {
            document_route
        } else {
            "/"
        };
        let base = if route.ends_with('/') {
            route
        } else {
            route.rsplit_once('/').map(|(base, _)| base).unwrap_or("")
        };
        format!("{base}/{decoded}")
    };
    let mut segments = Vec::new();
    for segment in absolute.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            value => segments.push(value),
        }
    }
    Some(format!("/{}", segments.join("/")))
}

fn digest_hex(body: &[u8]) -> String {
    Sha256::digest(body)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::canvas::CanvasResourceManifest;

    fn resource(url: &str, hash_byte: char, kind: CanvasResourceKind) -> CanvasResourceEntry {
        CanvasResourceEntry {
            url: url.to_string(),
            content_hash: format!("sha256-{}", hash_byte.to_string().repeat(64)),
            size_bytes: 1,
            content_type: "application/octet-stream".to_string(),
            kind,
        }
    }

    #[test]
    fn stylesheet_urls_receive_direct_resource_identities() {
        let entries = [
            resource("/css/site.css", 'a', CanvasResourceKind::Stylesheet),
            resource("/css/theme.css", 'b', CanvasResourceKind::Stylesheet),
            resource("/fonturi/display.woff2", 'c', CanvasResourceKind::Font),
            resource("/images/hero large.webp", 'd', CanvasResourceKind::Image),
        ];
        let resources = entries
            .iter()
            .map(|entry| {
                (
                    entry.url.as_str(),
                    ResourceIdentity {
                        content_hash: entry.content_hash.as_str(),
                        is_stylesheet: entry.kind == CanvasResourceKind::Stylesheet,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let source = r#"
/* url('/ignored.woff2') */
.note::before { content: "url('/also-ignored.woff2')"; }
@font-face { src: URL('/fonturi/display.woff2') format('woff2'); }
.hero { background: url('../images/hero%20large.webp?v=1#cover'); }
@import url("theme.css");
.remote { background: url(https://cdn.example/remote.webp); }
.inline { background: url("data:image/svg+xml,%3Csvg%3E"); }
.fragment { filter: url(#shadow); }
"#;
        let revised = revise_stylesheet_source(source, "/css/site.css", "workspace-7", &resources);

        assert!(revised.contains(&format!(
            "URL('/fonturi/display.woff2?__pana_resource_hash=sha256-{}')",
            "c".repeat(64)
        )));
        assert!(revised.contains(&format!(
            "url('../images/hero%20large.webp?v=1&__pana_resource_hash=sha256-{}#cover')",
            "d".repeat(64)
        )));
        assert!(revised.contains("url(\"theme.css?__pana_preview_revision=workspace-7\")"));
        assert!(revised.contains("/* url('/ignored.woff2') */"));
        assert!(revised.contains("content: \"url('/also-ignored.woff2')\""));
        assert!(revised.contains("url(https://cdn.example/remote.webp)"));
        assert!(revised.contains("url(\"data:image/svg+xml,%3Csvg%3E\")"));
        assert!(revised.contains("url(#shadow)"));
        assert_eq!(
            revise_stylesheet_source(&revised, "/css/site.css", "workspace-7", &resources),
            revised
        );
    }

    #[test]
    fn revisioned_manifest_hashes_the_rewritten_stylesheet() {
        let root = std::env::temp_dir().join(format!(
            "pana-css-resource-revision-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("css")).unwrap();
        std::fs::create_dir_all(root.join("fonturi")).unwrap();
        let font = b"stable-test-font";
        std::fs::write(root.join("fonturi/display.woff2"), font).unwrap();
        std::fs::write(
            root.join("css/site.css"),
            "@font-face { font-family: Display; src: url('/fonturi/display.woff2') format('woff2'); }",
        )
        .unwrap();

        let manifest = CanvasResourceManifest::from_revisioned_artifact_root(
            "workspace-1",
            &root,
            |path, body| std::fs::write(path, body).map_err(|error| error.to_string()),
        )
        .unwrap();
        let stylesheet = std::fs::read(root.join("css/site.css")).unwrap();
        let stylesheet_source = std::str::from_utf8(&stylesheet).unwrap();
        let font_hash = format!("sha256-{}", digest_hex(font));
        assert!(stylesheet_source.contains(&format!(
            "url('/fonturi/display.woff2?__pana_resource_hash={font_hash}')"
        )));
        let stylesheet_entry = manifest
            .entries
            .iter()
            .find(|entry| entry.url == "/css/site.css")
            .unwrap();
        assert_eq!(
            stylesheet_entry.content_hash,
            format!("sha256-{}", digest_hex(&stylesheet))
        );
        assert_eq!(stylesheet_entry.size_bytes, stylesheet.len() as u64);

        std::fs::remove_dir_all(root).unwrap();
    }
}
