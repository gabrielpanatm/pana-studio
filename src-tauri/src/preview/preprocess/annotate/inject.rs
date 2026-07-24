use std::collections::{BTreeMap, BTreeSet};

use crate::preview::preprocess::annotate::index::SourceIdIndex;
use crate::project_model::zola_image_engine::encode_preview_presentation;
use crate::source_graph::{
    mixed_cst::{parse_mixed_cst, MixedCstKind},
    tera_cst::{TeraCstKind, TeraTagKind},
};

const SKIP_TAGS: &[&str] = &[
    "html", "head", "body", "script", "style", "meta", "link", "base", "title", "br", "hr",
    "input", "area", "col", "embed", "param", "source", "track", "wbr", "noscript", "template",
];

#[cfg(test)]
pub fn preprocess_template(
    source: &str,
    relative_path: &str,
    source_ids: Option<&SourceIdIndex>,
) -> String {
    preprocess_template_with_revision(source, relative_path, source_ids, None)
}

pub fn preprocess_template_with_revision(
    source: &str,
    relative_path: &str,
    source_ids: Option<&SourceIdIndex>,
    preview_revision: Option<&str>,
) -> String {
    let document = parse_mixed_cst(source, relative_path);
    debug_assert!(document.is_lossless());
    let mut insertions = BTreeMap::<usize, String>::new();
    project_tera_provenance_insertions(
        source,
        relative_path,
        source_ids,
        &document,
        &mut insertions,
    );
    project_html_provenance_insertions(
        source,
        relative_path,
        source_ids,
        preview_revision,
        &document,
        &mut insertions,
    );

    let mut result =
        String::with_capacity(source.len() + insertions.values().map(String::len).sum::<usize>());
    result.push_str(source);
    for (position, insertion) in insertions.into_iter().rev() {
        if position <= result.len() && result.is_char_boundary(position) {
            result.insert_str(position, &insertion);
        }
    }
    inject_empty_tera_slot_placeholders(&result, preview_revision)
}

fn project_tera_provenance_insertions(
    source: &str,
    relative_path: &str,
    source_ids: Option<&SourceIdIndex>,
    document: &crate::source_graph::mixed_cst::MixedCstDocument,
    insertions: &mut BTreeMap<usize, String>,
) {
    let root_tera_nodes = document
        .nodes
        .iter()
        .filter_map(|node| match node.kind {
            MixedCstKind::Tera { tera_node } => Some(tera_node),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let mut scope_stack = Vec::<Option<String>>::new();

    for (index, node) in document.tera.nodes.iter().enumerate() {
        let (line, column) =
            crate::preview::preprocess::annotate::range::line_column(source, node.start);
        let source_location = format!("{relative_path}:{line}:{column}");

        if let Some(template_source_id) =
            source_ids.and_then(|index| index.scope_start_marker_for(&source_location))
        {
            append_insertion(
                insertions,
                node.end,
                &format!("<!-- pana-template-source-start:{} -->", template_source_id),
            );
        }

        match &node.kind {
            TeraCstKind::Variable if root_tera_nodes.contains(&index) => {
                if let Some(template_source_id) =
                    source_ids.and_then(|index| index.template_source_id_for(&source_location))
                {
                    append_insertion(
                        insertions,
                        node.start,
                        &format!(
                            "<!-- pana-template-expression-start:{} -->",
                            template_source_id
                        ),
                    );
                    append_insertion(
                        insertions,
                        node.end,
                        &format!(
                            "<!-- pana-template-expression-end:{} -->",
                            template_source_id
                        ),
                    );
                }
            }
            TeraCstKind::Tag(tag) => {
                let content = node.content(source).trim();
                match tag {
                    TeraTagKind::Include => {
                        if let Some(template_source_id) = source_ids
                            .and_then(|index| index.template_source_id_for(&source_location))
                        {
                            append_insertion(
                                insertions,
                                node.start,
                                &format!(
                                    "<!-- pana-template-source-start:{} -->",
                                    template_source_id
                                ),
                            );
                            append_insertion(
                                insertions,
                                node.end,
                                &format!(
                                    "<!-- pana-template-source-end:{} -->",
                                    template_source_id
                                ),
                            );
                        }
                    }
                    TeraTagKind::Block
                    | TeraTagKind::For
                    | TeraTagKind::If
                    | TeraTagKind::Macro
                    | TeraTagKind::Filter => {
                        let keyword = tera_keyword(content);
                        let marker_id = should_mark_tera_scope(content, keyword, relative_path)
                            .then(|| {
                                source_ids
                                    .and_then(|index| {
                                        index.template_source_id_for(&source_location)
                                    })
                                    .map(str::to_string)
                            })
                            .flatten();
                        if let Some(template_source_id) = marker_id.as_deref() {
                            let external_start = source_ids.is_some_and(|index| {
                                index.has_external_scope_start(&source_location)
                            });
                            if !external_start {
                                append_insertion(
                                    insertions,
                                    node.end,
                                    &format!(
                                        "<!-- pana-template-source-start:{} -->",
                                        template_source_id
                                    ),
                                );
                            }
                        }
                        scope_stack.push(marker_id);
                    }
                    TeraTagKind::EndBlock
                    | TeraTagKind::EndFor
                    | TeraTagKind::EndIf
                    | TeraTagKind::EndMacro
                    | TeraTagKind::EndFilter => {
                        if let Some(Some(template_source_id)) = scope_stack.pop() {
                            append_insertion(
                                insertions,
                                node.start,
                                &format!(
                                    "<!-- pana-template-source-end:{} -->",
                                    template_source_id
                                ),
                            );
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

fn project_html_provenance_insertions(
    source: &str,
    relative_path: &str,
    source_ids: Option<&SourceIdIndex>,
    preview_revision: Option<&str>,
    document: &crate::source_graph::mixed_cst::MixedCstDocument,
    insertions: &mut BTreeMap<usize, String>,
) {
    for element in &document.elements {
        let Some(opening) = document.nodes.get(element.opening_node) else {
            continue;
        };
        let MixedCstKind::StartTag(tag) = &opening.kind else {
            continue;
        };
        if SKIP_TAGS.contains(&tag.name.as_str())
            || tag
                .attributes
                .iter()
                .any(|attribute| attribute.name.eq_ignore_ascii_case("data-pana-source-id"))
        {
            continue;
        }
        let (line, column) =
            crate::preview::preprocess::annotate::range::line_column(source, opening.start);
        let source_location = format!("{relative_path}:{line}:{column}");
        let mut attributes = String::new();
        let mut has_source_marker = false;
        if let Some(source_id) = source_ids.and_then(|index| index.source_id_for(&source_location))
        {
            attributes.push_str(&format!(" data-pana-source-id=\"{}\"", source_id));
            has_source_marker = true;
        }
        if let Some(template_source_id) =
            source_ids.and_then(|index| index.template_source_id_for_html(&source_location))
        {
            attributes.push_str(&format!(
                " data-pana-template-source-id=\"{}\"",
                template_source_id
            ));
            has_source_marker = true;
        }
        if let Some(presentation) =
            source_ids.and_then(|index| index.zola_image_for(&source_location))
        {
            if let Ok(payload) = encode_preview_presentation(presentation) {
                attributes.push_str(&format!(" data-pana-zola-image=\"{}\"", payload));
            }
        }
        if has_source_marker {
            if let Some(revision) = preview_revision {
                attributes.push_str(&format!(" data-pana-preview-revision=\"{}\"", revision));
            }
        }
        if attributes.is_empty() {
            continue;
        }
        let mut insert_at = opening.end.saturating_sub(1);
        let before_close = source
            .get(opening.start..insert_at)
            .unwrap_or_default()
            .trim_end();
        if tag.self_closing && before_close.ends_with('/') {
            insert_at = opening.start
                + source[opening.start..insert_at]
                    .rfind('/')
                    .unwrap_or(insert_at.saturating_sub(opening.start));
        }
        append_insertion(insertions, insert_at, &attributes);
    }
}

fn append_insertion(insertions: &mut BTreeMap<usize, String>, position: usize, value: &str) {
    insertions.entry(position).or_default().push_str(value);
}

#[cfg(test)]
#[derive(Default)]
struct TeraScopeMarkerState {
    stack: Vec<Option<String>>,
}

fn tera_keyword(content: &str) -> &str {
    content.split_whitespace().next().unwrap_or("")
}

#[cfg(test)]
fn opens_tera_scope(keyword: &str) -> bool {
    matches!(keyword, "block" | "for" | "if" | "macro" | "filter")
}

fn tera_block_name(content: &str) -> Option<&str> {
    let mut parts = content.split_whitespace();
    match parts.next() {
        Some("block") => parts.next(),
        _ => None,
    }
}

fn is_non_visual_block_name(name: &str) -> bool {
    let normalized = name
        .trim_matches(|character| character == '"' || character == '\'')
        .to_ascii_lowercase();

    matches!(
        normalized.as_str(),
        "title"
            | "description"
            | "og_title"
            | "og_description"
            | "tw_title"
            | "tw_description"
            | "canonical"
            | "robots"
    ) || normalized.contains("script")
        || normalized.contains("style")
        || normalized.contains("css")
        || normalized.contains("head")
        || normalized.contains("meta")
        || normalized.contains("preload")
}

fn is_partial_template_relative_path(relative_path: &str) -> bool {
    let normalized = relative_path.trim_start_matches('/').replace('\\', "/");
    let logical = if let Some(after_themes) = normalized.strip_prefix("themes/") {
        after_themes
            .split_once("/templates/")
            .map(|(_theme, template_path)| template_path)
            .unwrap_or(normalized.as_str())
    } else {
        normalized
            .strip_prefix("templates/")
            .unwrap_or(normalized.as_str())
    };

    logical.starts_with("partials/")
        || logical.starts_with("macros/")
        || logical.starts_with("shortcodes/")
}

fn should_mark_tera_scope(content: &str, keyword: &str, relative_path: &str) -> bool {
    match keyword {
        "block" => {
            !is_partial_template_relative_path(relative_path)
                && tera_block_name(content).is_some_and(|name| !is_non_visual_block_name(name))
        }
        "for" | "if" | "filter" => true,
        _ => false,
    }
}

#[cfg(test)]
fn closes_tera_scope(keyword: &str) -> bool {
    matches!(
        keyword,
        "endblock" | "endfor" | "endif" | "endmacro" | "endfilter"
    )
}

#[cfg(test)]
pub(super) fn inject_tpl_src_on_line(
    line: &str,
    relative_path: &str,
    line_num: usize,
    source_ids: Option<&SourceIdIndex>,
) -> String {
    inject_tpl_src_on_line_with_state(line, relative_path, line_num, source_ids, None, None)
}

#[cfg(test)]
fn inject_tpl_src_on_line_with_state(
    line: &str,
    relative_path: &str,
    line_num: usize,
    source_ids: Option<&SourceIdIndex>,
    preview_revision: Option<&str>,
    mut tera_marker_state: Option<&mut TeraScopeMarkerState>,
) -> String {
    let bytes = line.as_bytes();
    let mut insertions: Vec<(usize, String)> = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'{' {
            let (close_a, close_b) = match bytes[i + 1] {
                b'%' => (b'%', b'}'),
                b'{' => (b'}', b'}'),
                b'#' => (b'#', b'}'),
                _ => {
                    i += 1;
                    continue;
                }
            };
            let token_start = i;
            i += 2;
            while i + 1 < bytes.len() {
                if bytes[i] == close_a && bytes[i + 1] == close_b {
                    i += 2;
                    break;
                }
                i += 1;
            }
            if close_a == b'%' {
                let token_end = i;
                let content = line[token_start + 2..token_end.saturating_sub(2)]
                    .trim()
                    .trim_start_matches('-')
                    .trim_end_matches('-')
                    .trim();
                let keyword = tera_keyword(content);
                let column = line[..token_start].chars().count() + 1;
                let src_value = format!("{}:{}:{}", relative_path, line_num, column);
                if let Some(template_source_id) =
                    source_ids.and_then(|index| index.scope_start_marker_for(&src_value))
                {
                    insertions.push((
                        token_end,
                        format!("<!-- pana-template-source-start:{} -->", template_source_id),
                    ));
                }
                if keyword == "include" {
                    if let Some(template_source_id) =
                        source_ids.and_then(|index| index.template_source_id_for(&src_value))
                    {
                        insertions.push((
                            token_start,
                            format!("<!-- pana-template-source-start:{} -->", template_source_id),
                        ));
                        insertions.push((
                            token_end,
                            format!("<!-- pana-template-source-end:{} -->", template_source_id),
                        ));
                    }
                } else if opens_tera_scope(keyword) {
                    let marker_id = if should_mark_tera_scope(content, keyword, relative_path) {
                        source_ids
                            .and_then(|index| index.template_source_id_for(&src_value))
                            .map(str::to_string)
                    } else {
                        None
                    };
                    if let Some(template_source_id) = marker_id.as_deref() {
                        let external_start = source_ids
                            .is_some_and(|index| index.has_external_scope_start(&src_value));
                        if !external_start {
                            insertions.push((
                                token_end,
                                format!(
                                    "<!-- pana-template-source-start:{} -->",
                                    template_source_id
                                ),
                            ));
                        }
                    }
                    if let Some(state) = tera_marker_state.as_deref_mut() {
                        state.stack.push(marker_id);
                    }
                } else if closes_tera_scope(keyword) {
                    let marker_id = tera_marker_state
                        .as_deref_mut()
                        .and_then(|state| state.stack.pop().flatten());
                    if let Some(template_source_id) = marker_id {
                        insertions.push((
                            token_start,
                            format!("<!-- pana-template-source-end:{} -->", template_source_id),
                        ));
                    }
                }
            }
            continue;
        }

        if bytes[i] != b'<' {
            i += 1;
            continue;
        }

        let after_lt = i + 1;
        if after_lt >= bytes.len() {
            break;
        }

        let next = bytes[after_lt];
        if next == b'/' || next == b'!' || next == b'?' || !next.is_ascii_alphabetic() {
            i += 1;
            continue;
        }

        let name_start = after_lt;
        let mut j = name_start;
        while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'-') {
            j += 1;
        }
        let tag_name = line[name_start..j].to_ascii_lowercase();

        if SKIP_TAGS.contains(&tag_name.as_str()) {
            i = j;
            continue;
        }

        let mut k = j;
        let mut in_dq = false;
        let mut in_sq = false;
        let mut found_close = false;

        while k < bytes.len() {
            if !in_dq && !in_sq && k + 1 < bytes.len() && bytes[k] == b'{' {
                let (ca, cb) = match bytes[k + 1] {
                    b'{' => (b'}', b'}'),
                    b'%' => (b'%', b'}'),
                    b'#' => (b'#', b'}'),
                    _ => {
                        k += 1;
                        continue;
                    }
                };
                k += 2;
                while k + 1 < bytes.len() {
                    if bytes[k] == ca && bytes[k + 1] == cb {
                        k += 2;
                        break;
                    }
                    k += 1;
                }
                continue;
            }

            match bytes[k] {
                b'"' if !in_sq => {
                    in_dq = !in_dq;
                }
                b'\'' if !in_dq => {
                    in_sq = !in_sq;
                }
                b'>' if !in_dq && !in_sq => {
                    found_close = true;
                    break;
                }
                _ => {}
            }
            k += 1;
        }

        if !found_close {
            break;
        }

        let attrs_text = &line[j..k];
        if attrs_text.contains("data-pana-source-id") {
            i = k + 1;
            continue;
        }

        let insert_at = if attrs_text.trim_end().ends_with('/') {
            k - 1
        } else {
            k
        };
        let column = line[..i].chars().count() + 1;
        let src_value = format!("{}:{}:{}", relative_path, line_num, column);
        let mut has_pana_source_marker = false;
        if let Some(source_id) = source_ids.and_then(|index| index.source_id_for(&src_value)) {
            insertions.push((insert_at, format!(" data-pana-source-id=\"{}\"", source_id)));
            has_pana_source_marker = true;
        }
        if let Some(template_source_id) =
            source_ids.and_then(|index| index.template_source_id_for_html(&src_value))
        {
            insertions.push((
                insert_at,
                format!(" data-pana-template-source-id=\"{}\"", template_source_id),
            ));
            has_pana_source_marker = true;
        }
        if let Some(presentation) = source_ids.and_then(|index| index.zola_image_for(&src_value)) {
            if let Ok(payload) = encode_preview_presentation(presentation) {
                insertions.push((insert_at, format!(" data-pana-zola-image=\"{}\"", payload)));
            }
        }
        if let Some(revision) = preview_revision {
            if has_pana_source_marker {
                insertions.push((
                    insert_at,
                    format!(" data-pana-preview-revision=\"{}\"", revision),
                ));
            }
        }

        i = k + 1;
    }

    if insertions.is_empty() {
        return line.to_string();
    }

    let mut result = line.to_string();
    for (pos, attr) in insertions.into_iter().rev() {
        result.insert_str(pos, &attr);
    }
    result
}

fn inject_empty_tera_slot_placeholders(source: &str, preview_revision: Option<&str>) -> String {
    const START_PREFIX: &str = "<!-- pana-template-source-start:";
    const START_SUFFIX: &str = " -->";

    let mut result = String::with_capacity(source.len());
    let mut cursor = 0;

    while let Some(relative_start) = source[cursor..].find(START_PREFIX) {
        let start_index = cursor + relative_start;
        let id_start = start_index + START_PREFIX.len();
        let Some(relative_id_end) = source[id_start..].find(START_SUFFIX) else {
            break;
        };
        let id_end = id_start + relative_id_end;
        let id = &source[id_start..id_end];
        let start_end = id_end + START_SUFFIX.len();
        let end_marker = format!("<!-- pana-template-source-end:{} -->", id);
        let Some(relative_end_index) = source[start_end..].find(&end_marker) else {
            result.push_str(&source[cursor..start_end]);
            cursor = start_end;
            continue;
        };
        let end_index = start_end + relative_end_index;
        let between = &source[start_end..end_index];

        result.push_str(&source[cursor..start_end]);
        if between.trim().is_empty() {
            result.push_str(&empty_tera_slot_placeholder(id, preview_revision));
        }
        cursor = start_end;
    }

    result.push_str(&source[cursor..]);
    result
}

fn empty_tera_slot_placeholder(source_id: &str, preview_revision: Option<&str>) -> String {
    let revision_attr = preview_revision
        .map(|revision| format!(r#" data-pana-preview-revision="{}""#, revision))
        .unwrap_or_default();
    format!(
        r#"<div class="pana-studio-empty-editable pana-studio-empty-tera-slot" data-pana-empty-tera-slot="{}" data-pana-empty-tera-slot-static="true" data-pana-source-id="{}" data-pana-template-source-id="{}"{} data-pana-empty-label="Block Tera gol"></div>"#,
        source_id, source_id, source_id, revision_attr
    )
}
