use crate::source_graph::{
    model::{SourceNodeKind, SourceRelationKind},
    tera::{parse_tera_items, TeraItemKind},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TeraZolaPathFunction {
    GetPage,
    GetSection,
    GetUrl,
    GetHash,
    LoadData,
    GetImageMetadata,
    ResizeImage,
}

pub(crate) fn zola_path_function_for_relation(kind: &SourceRelationKind) -> TeraZolaPathFunction {
    match kind {
        SourceRelationKind::InternalContentLink | SourceRelationKind::AssetUrl => {
            TeraZolaPathFunction::GetUrl
        }
        SourceRelationKind::AssetHash => TeraZolaPathFunction::GetHash,
        SourceRelationKind::DataLoad
        | SourceRelationKind::DataFileLoad
        | SourceRelationKind::ContentDataLoad => TeraZolaPathFunction::LoadData,
        SourceRelationKind::ImageMetadata => TeraZolaPathFunction::GetImageMetadata,
        SourceRelationKind::ImageResize => TeraZolaPathFunction::ResizeImage,
        SourceRelationKind::GetsSection => TeraZolaPathFunction::GetSection,
        _ => TeraZolaPathFunction::GetPage,
    }
}

#[derive(Clone)]
pub(crate) struct TeraZolaPathCall {
    pub(crate) function: TeraZolaPathFunction,
    pub(crate) path: String,
    pub(crate) path_start: usize,
    pub(crate) path_end: usize,
}

pub(crate) fn parse_zola_path_calls(source: &str) -> Vec<TeraZolaPathCall> {
    let mut calls = Vec::new();
    for item in parse_tera_items(source) {
        if item.kind != TeraItemKind::Node {
            continue;
        }
        if matches!(
            item.node_kind,
            Some(SourceNodeKind::TeraComment | SourceNodeKind::Raw)
        ) {
            continue;
        }
        collect_zola_path_calls(
            source,
            item.start,
            item.end,
            "get_page",
            TeraZolaPathFunction::GetPage,
            &mut calls,
        );
        collect_zola_path_calls(
            source,
            item.start,
            item.end,
            "get_section",
            TeraZolaPathFunction::GetSection,
            &mut calls,
        );
        collect_zola_path_calls(
            source,
            item.start,
            item.end,
            "get_url",
            TeraZolaPathFunction::GetUrl,
            &mut calls,
        );
        collect_zola_path_calls(
            source,
            item.start,
            item.end,
            "get_hash",
            TeraZolaPathFunction::GetHash,
            &mut calls,
        );
        collect_zola_path_calls(
            source,
            item.start,
            item.end,
            "load_data",
            TeraZolaPathFunction::LoadData,
            &mut calls,
        );
        collect_zola_path_calls(
            source,
            item.start,
            item.end,
            "get_image_metadata",
            TeraZolaPathFunction::GetImageMetadata,
            &mut calls,
        );
        collect_zola_path_calls(
            source,
            item.start,
            item.end,
            "resize_image",
            TeraZolaPathFunction::ResizeImage,
            &mut calls,
        );
    }
    calls
}

fn collect_zola_path_calls(
    source: &str,
    start: usize,
    end: usize,
    function_name: &str,
    function: TeraZolaPathFunction,
    calls: &mut Vec<TeraZolaPathCall>,
) {
    let bytes = source.as_bytes();
    let mut index = start;
    while index < end && index < bytes.len() {
        if is_quote(bytes[index]) {
            index = skip_string(bytes, index, end).unwrap_or(end);
            continue;
        }
        if !identifier_at(source, index, end, function_name) {
            index += 1;
            continue;
        }
        let mut cursor = index + function_name.len();
        cursor = skip_ascii_whitespace(bytes, cursor, end);
        if cursor >= end || bytes.get(cursor) != Some(&b'(') {
            index += function_name.len();
            continue;
        }
        let Some(call_end) = matching_paren_end(bytes, cursor, end) else {
            index += function_name.len();
            continue;
        };
        if let Some((path_start, path_end, path)) =
            named_string_argument(source, cursor + 1, call_end.saturating_sub(1), "path")
        {
            calls.push(TeraZolaPathCall {
                function: function.clone(),
                path,
                path_start,
                path_end,
            });
        }
        index = call_end;
    }
}

fn identifier_at(source: &str, start: usize, end: usize, identifier: &str) -> bool {
    let Some(candidate) = source.get(start..start + identifier.len()) else {
        return false;
    };
    if candidate != identifier {
        return false;
    }
    let before_ok = if start == 0 {
        true
    } else {
        !source[..start]
            .chars()
            .next_back()
            .map(is_identifier_continue)
            .unwrap_or(false)
    };
    let after_index = start + identifier.len();
    let after_ok = if after_index >= end {
        true
    } else {
        !source[after_index..]
            .chars()
            .next()
            .map(is_identifier_continue)
            .unwrap_or(false)
    };
    before_ok && after_ok
}

fn named_string_argument(
    source: &str,
    start: usize,
    end: usize,
    argument: &str,
) -> Option<(usize, usize, String)> {
    let bytes = source.as_bytes();
    let mut index = start;
    while index < end && index < bytes.len() {
        if is_quote(bytes[index]) {
            index = skip_string(bytes, index, end).unwrap_or(end);
            continue;
        }
        if !identifier_at(source, index, end, argument) {
            index += 1;
            continue;
        }
        let mut cursor = index + argument.len();
        cursor = skip_ascii_whitespace(bytes, cursor, end);
        if cursor >= end || bytes.get(cursor) != Some(&b'=') {
            index += argument.len();
            continue;
        }
        cursor = skip_ascii_whitespace(bytes, cursor + 1, end);
        let quote = *bytes.get(cursor)?;
        if !is_quote(quote) {
            index += argument.len();
            continue;
        }
        let content_start = cursor + 1;
        let content_end = string_content_end(bytes, content_start, end, quote)?;
        let value = source.get(content_start..content_end)?.to_string();
        return Some((content_start, content_end, value));
    }
    None
}

fn matching_paren_end(bytes: &[u8], start: usize, end: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut index = start;
    while index < end && index < bytes.len() {
        if is_quote(bytes[index]) {
            index = skip_string(bytes, index, end)?;
            continue;
        }
        match bytes[index] {
            b'(' => depth += 1,
            b')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn skip_ascii_whitespace(bytes: &[u8], mut index: usize, end: usize) -> usize {
    while index < end && index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn skip_string(bytes: &[u8], start: usize, end: usize) -> Option<usize> {
    let quote = *bytes.get(start)?;
    let content_start = start + 1;
    string_content_end(bytes, content_start, end, quote).map(|string_end| string_end + 1)
}

fn string_content_end(bytes: &[u8], mut index: usize, end: usize, quote: u8) -> Option<usize> {
    while index < end && index < bytes.len() {
        if bytes[index] == quote && !is_escaped(bytes, index) {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn is_quote(byte: u8) -> bool {
    byte == b'"' || byte == b'\''
}

fn is_escaped(bytes: &[u8], index: usize) -> bool {
    if index == 0 {
        return false;
    }
    let mut slash_count = 0;
    let mut cursor = index - 1;
    loop {
        if bytes[cursor] != b'\\' {
            break;
        }
        slash_count += 1;
        if cursor == 0 {
            break;
        }
        cursor -= 1;
    }
    slash_count % 2 == 1
}

fn is_identifier_continue(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_static_zola_path_calls() {
        let source = r#"{% set post = get_page(path="blog/post.md") %}
{{ get_section(path='blog/_index.md', metadata_only=true).title }}
<a href="{{ get_url(path="@/blog/post.md") }}">Post</a>
<link href="{{ get_url(path="css/site.css") }}">
<script integrity="{{ get_hash(path="static/js/app.js") }}"></script>
{% set data = load_data(path="static/data/catalog.json") %}
{% set meta = get_image_metadata(path="static/img/hero.png") %}
{% set resized = resize_image(path="static/img/hero.png", width=640, op="fit_width") %}
{{ "get_page(path=\"ignored.md\")" }}
{% raw %}{{ get_page(path="raw.md") }}{% endraw %}
"#;

        let calls = parse_zola_path_calls(source);

        assert_eq!(calls.len(), 8);
        assert!(calls.iter().any(|call| {
            call.function == TeraZolaPathFunction::GetPage
                && call.path == "blog/post.md"
                && source[call.path_start..call.path_end] == call.path
        }));
        assert!(calls.iter().any(|call| {
            call.function == TeraZolaPathFunction::GetSection
                && call.path == "blog/_index.md"
                && source[call.path_start..call.path_end] == call.path
        }));
        assert!(calls.iter().any(|call| {
            call.function == TeraZolaPathFunction::GetUrl
                && call.path == "@/blog/post.md"
                && source[call.path_start..call.path_end] == call.path
        }));
        assert!(calls.iter().any(|call| {
            call.function == TeraZolaPathFunction::GetUrl
                && call.path == "css/site.css"
                && source[call.path_start..call.path_end] == call.path
        }));
        assert!(calls.iter().any(|call| {
            call.function == TeraZolaPathFunction::GetHash
                && call.path == "static/js/app.js"
                && source[call.path_start..call.path_end] == call.path
        }));
        assert!(calls.iter().any(|call| {
            call.function == TeraZolaPathFunction::LoadData
                && call.path == "static/data/catalog.json"
                && source[call.path_start..call.path_end] == call.path
        }));
        assert!(calls.iter().any(|call| {
            call.function == TeraZolaPathFunction::GetImageMetadata
                && call.path == "static/img/hero.png"
                && source[call.path_start..call.path_end] == call.path
        }));
        assert!(calls.iter().any(|call| {
            call.function == TeraZolaPathFunction::ResizeImage
                && call.path == "static/img/hero.png"
                && source[call.path_start..call.path_end] == call.path
        }));
    }

    #[test]
    fn maps_source_graph_relations_to_zola_path_functions() {
        assert_eq!(
            zola_path_function_for_relation(&SourceRelationKind::GetsPage),
            TeraZolaPathFunction::GetPage
        );
        assert_eq!(
            zola_path_function_for_relation(&SourceRelationKind::GetsSection),
            TeraZolaPathFunction::GetSection
        );
        assert_eq!(
            zola_path_function_for_relation(&SourceRelationKind::InternalContentLink),
            TeraZolaPathFunction::GetUrl
        );
        assert_eq!(
            zola_path_function_for_relation(&SourceRelationKind::AssetHash),
            TeraZolaPathFunction::GetHash
        );
        assert_eq!(
            zola_path_function_for_relation(&SourceRelationKind::DataLoad),
            TeraZolaPathFunction::LoadData
        );
        assert_eq!(
            zola_path_function_for_relation(&SourceRelationKind::DataFileLoad),
            TeraZolaPathFunction::LoadData
        );
        assert_eq!(
            zola_path_function_for_relation(&SourceRelationKind::ContentDataLoad),
            TeraZolaPathFunction::LoadData
        );
        assert_eq!(
            zola_path_function_for_relation(&SourceRelationKind::ImageMetadata),
            TeraZolaPathFunction::GetImageMetadata
        );
        assert_eq!(
            zola_path_function_for_relation(&SourceRelationKind::ImageResize),
            TeraZolaPathFunction::ResizeImage
        );
    }
}
