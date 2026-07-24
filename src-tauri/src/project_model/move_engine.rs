use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
};

use serde::{Deserialize, Serialize};

use super::zola_image_engine::zola_image_contract_start;
use crate::{
    project_model::model::{ProjectModel, ProjectModelFileKind},
    source_graph::{
        identity::source_node_id,
        model::{SourceNode, SourceNodeKind},
    },
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlMoveIntent {
    pub source_source_id: Option<String>,
    pub target_source_id: Option<String>,
    pub source_location: Option<ProjectSourceEditLocation>,
    pub target_location: Option<ProjectSourceEditLocation>,
    pub source_tag: Option<String>,
    pub target_tag: Option<String>,
    pub source_selector: Option<String>,
    pub target_selector: Option<String>,
    pub position: ProjectMovePosition,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProjectMovePosition {
    Before,
    After,
    Inside,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlMovePlan {
    pub allowed: bool,
    pub diagnostic: Option<String>,
    pub model_revision: String,
    pub patch: Option<ProjectHtmlMovePatch>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlMovePatch {
    pub file: String,
    pub resolved_source_id: String,
    pub resolved_target_id: String,
    pub source_label: String,
    pub before_revision: String,
    pub after_revision: String,
    pub contents: String,
    pub source_location: ProjectSourceEditLocation,
    pub target_location: ProjectSourceEditLocation,
    pub source_start_line: usize,
    pub source_end_line: usize,
    pub new_start_line: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSourceEditLocation {
    pub file: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Copy)]
pub(super) struct Span {
    pub(super) start: usize,
    pub(super) end: usize,
}

struct MoveApplication {
    contents: String,
    source_start_line: usize,
    source_end_line: usize,
    new_start_line: usize,
}

pub(crate) struct HtmlTag {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) tag: String,
    pub(crate) is_closing: bool,
    pub(crate) is_self_closing: bool,
}

struct HtmlNodeSpan {
    id: String,
    file: String,
    fingerprint: String,
}

const VOID_TAGS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

const CONTAINER_TAGS: &[&str] = &[
    "main", "section", "article", "header", "footer", "nav", "aside", "div", "ul", "ol", "li",
    "form", "fieldset",
];

pub fn plan_html_move(
    model: &ProjectModel,
    intent: &ProjectHtmlMoveIntent,
    aliases: &HashMap<String, String>,
) -> ProjectHtmlMovePlan {
    match plan_html_move_inner(model, intent, aliases) {
        Ok(patch) => ProjectHtmlMovePlan {
            allowed: true,
            diagnostic: None,
            model_revision: model.revision.clone(),
            patch: Some(patch),
        },
        Err(message) => ProjectHtmlMovePlan {
            allowed: false,
            diagnostic: Some(message),
            model_revision: model.revision.clone(),
            patch: None,
        },
    }
}

pub fn html_identity_aliases(
    before: &ProjectModel,
    after: &ProjectModel,
) -> HashMap<String, String> {
    let before_spans = html_node_spans(before);
    let after_spans = html_node_spans(after);
    let mut after_by_key: HashMap<String, Vec<&HtmlNodeSpan>> = HashMap::new();
    for span in &after_spans {
        after_by_key
            .entry(format!("{}::{}", span.file, span.fingerprint))
            .or_default()
            .push(span);
    }

    let mut aliases = HashMap::new();
    for span in &before_spans {
        let key = format!("{}::{}", span.file, span.fingerprint);
        let Some(candidates) = after_by_key.get(&key) else {
            continue;
        };
        if candidates.len() == 1 && candidates[0].id != span.id {
            aliases.insert(span.id.clone(), candidates[0].id.clone());
        }
    }
    aliases
}

pub fn html_node_id_at_line(
    model: &ProjectModel,
    file: &str,
    label: &str,
    line: usize,
) -> Option<String> {
    let matches: Vec<&SourceNode> = model
        .source_graph
        .nodes
        .iter()
        .filter(|node| {
            node.kind == SourceNodeKind::Html
                && same_model_path(&node.file, file)
                && node.label == label
                && node.range.as_ref().is_some_and(|range| range.line == line)
        })
        .collect();
    if matches.len() == 1 {
        Some(matches[0].id.clone())
    } else {
        None
    }
}

pub fn html_node_id_at_location(
    model: &ProjectModel,
    location: &ProjectSourceEditLocation,
    tag: &str,
) -> Option<String> {
    resolve_html_node_at_location(model, location, Some(tag)).map(|node| node.id.clone())
}

fn plan_html_move_inner(
    model: &ProjectModel,
    intent: &ProjectHtmlMoveIntent,
    aliases: &HashMap<String, String>,
) -> Result<ProjectHtmlMovePatch, String> {
    let source_node = resolve_html_node_for_anchor(
        model,
        intent.source_source_id.as_deref(),
        intent.source_location.as_ref(),
        intent.source_tag.as_deref(),
        aliases,
    )
    .ok_or_else(|| {
        source_missing_message(
            "sursă",
            intent.source_source_id.as_deref(),
            intent.source_location.as_ref(),
            intent.source_selector.as_deref(),
        )
    })?;
    let target_node = resolve_html_node_for_anchor(
        model,
        intent.target_source_id.as_deref(),
        intent.target_location.as_ref(),
        intent.target_tag.as_deref(),
        aliases,
    )
    .ok_or_else(|| {
        source_missing_message(
            "destinație",
            intent.target_source_id.as_deref(),
            intent.target_location.as_ref(),
            intent.target_selector.as_deref(),
        )
    })?;

    if source_node.id == target_node.id {
        return Err("Elementul este deja pe această țintă.".to_string());
    }
    if !same_model_path(&source_node.file, &target_node.file) {
        return Err(
            "Mutarea între template-uri diferite rămâne blocată până există plan de impact."
                .to_string(),
        );
    }
    if !source_node.capabilities.can_move {
        return Err(source_node
            .capabilities
            .reason
            .clone()
            .unwrap_or_else(|| "Elementul sursă nu este mutabil vizual.".to_string()));
    }
    if !target_node.capabilities.can_move {
        return Err(target_node
            .capabilities
            .reason
            .clone()
            .unwrap_or_else(|| "Destinația nu este mutabilă vizual.".to_string()));
    }

    let file = model
        .files
        .iter()
        .find(|file| same_model_path(&file.relative_path, &source_node.file))
        .ok_or_else(|| {
            format!(
                "Nu am găsit fișierul {} în Project Model.",
                source_node.file
            )
        })?;
    if file.kind != ProjectModelFileKind::Template {
        return Err(
            "Move Engine-ul structural este activ doar pentru template-uri Zola/Tera.".to_string(),
        );
    }
    let source_range = source_node
        .range
        .as_ref()
        .ok_or_else(|| "Sursa nu are range stabil în Source Graph.".to_string())?;
    let target_range = target_node
        .range
        .as_ref()
        .ok_or_else(|| "Destinația nu are range stabil în Source Graph.".to_string())?;
    let mut source_span = resolve_html_element_span(&file.contents, source_range.start)?;
    if let Some(contract_start) = zola_image_contract_start(&file.contents, source_range.start)? {
        source_span.start = contract_start;
    }
    let mut target_span = resolve_html_element_span(&file.contents, target_range.start)?;
    if intent.position == ProjectMovePosition::Before {
        if let Some(contract_start) = zola_image_contract_start(&file.contents, target_range.start)?
        {
            target_span.start = contract_start;
        }
    }
    if source_span.start == target_span.start && source_span.end == target_span.end {
        return Err("Sursa și destinația indică același element.".to_string());
    }
    if source_span.start <= target_span.start && target_span.start < source_span.end {
        return Err("Elementul nu poate fi mutat în interiorul propriului conținut.".to_string());
    }

    let target_tag = html_tag_at(&file.contents, target_range.start)?;
    if intent.position == ProjectMovePosition::Inside && !can_receive_children(&target_tag) {
        return Err(format!("<{target_tag}> nu este container pentru copii."));
    }

    let source_location =
        source_location_at_offset(&file.contents, &source_node.file, source_span.start);
    let target_location =
        source_location_at_offset(&file.contents, &target_node.file, target_span.start);
    let applied = apply_html_move(
        &file.contents,
        source_span,
        target_span,
        &target_tag,
        intent.position,
    )?;

    Ok(ProjectHtmlMovePatch {
        file: source_node.file.clone(),
        resolved_source_id: source_node.id.clone(),
        resolved_target_id: target_node.id.clone(),
        source_label: source_node.label.clone(),
        before_revision: file.revision.clone(),
        after_revision: content_revision(&applied.contents),
        contents: applied.contents,
        source_location,
        target_location,
        source_start_line: applied.source_start_line,
        source_end_line: applied.source_end_line,
        new_start_line: applied.new_start_line,
    })
}

fn resolve_html_node<'a>(
    model: &'a ProjectModel,
    source_id: &str,
    aliases: &HashMap<String, String>,
) -> Option<&'a SourceNode> {
    let mut current = source_id.to_string();
    let mut visited = HashSet::new();
    loop {
        if !visited.insert(current.clone()) {
            break;
        }
        // An alias records the logical identity selected before a committed
        // mutation. It must win over a positional ID which may have been
        // reused by another node in the new source graph.
        if let Some(next) = aliases.get(&current) {
            if visited.contains(next) {
                break;
            }
            current = next.clone();
            continue;
        }
        if let Some(node) = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.id == current && node.kind == SourceNodeKind::Html)
        {
            return Some(node);
        }
        break;
    }

    // Alias cycles must not exist after cache publication, but recover
    // deterministically from an older in-memory map: accept the cycle only
    // when exactly one of its identities is live in the current model.
    let mut live_cycle_nodes = model
        .source_graph
        .nodes
        .iter()
        .filter(|node| node.kind == SourceNodeKind::Html && visited.contains(&node.id));
    let node = live_cycle_nodes.next()?;
    if live_cycle_nodes.next().is_some() {
        None
    } else {
        Some(node)
    }
}

pub(super) fn resolve_html_node_for_anchor<'a>(
    model: &'a ProjectModel,
    source_id: Option<&str>,
    location: Option<&ProjectSourceEditLocation>,
    tag: Option<&str>,
    aliases: &HashMap<String, String>,
) -> Option<&'a SourceNode> {
    let id_node = source_id
        .and_then(|id| resolve_html_node(model, id, aliases))
        .filter(|node| node_tag_matches(node, tag));
    let location_node =
        location.and_then(|location| resolve_html_node_at_location(model, location, tag));
    resolve_conjunctive_anchor(source_id, location, id_node, location_node)
}

pub(super) fn resolve_conjunctive_anchor<'a>(
    source_id: Option<&str>,
    location: Option<&ProjectSourceEditLocation>,
    id_node: Option<&'a SourceNode>,
    location_node: Option<&'a SourceNode>,
) -> Option<&'a SourceNode> {
    match (source_id, location) {
        (Some(_), Some(_)) => match (id_node, location_node) {
            (Some(id_node), Some(location_node)) if id_node.id == location_node.id => Some(id_node),
            _ => None,
        },
        (Some(_), None) => id_node,
        (None, Some(_)) => location_node,
        (None, None) => None,
    }
}

pub(super) fn direct_location_without_source_id<'a>(
    source_id: Option<&str>,
    location: Option<&'a ProjectSourceEditLocation>,
) -> Option<&'a ProjectSourceEditLocation> {
    match (source_id, location) {
        (None, Some(location)) if location.line > 0 && location.column > 0 => Some(location),
        _ => None,
    }
}

fn resolve_html_node_at_location<'a>(
    model: &'a ProjectModel,
    location: &ProjectSourceEditLocation,
    tag: Option<&str>,
) -> Option<&'a SourceNode> {
    if location.line == 0 || location.column == 0 {
        return None;
    }

    let mut candidates: Vec<&SourceNode> = model
        .source_graph
        .nodes
        .iter()
        .filter(|node| {
            node.kind == SourceNodeKind::Html
                && same_model_path(&node.file, &location.file)
                && node
                    .range
                    .as_ref()
                    .is_some_and(|range| range.line == location.line)
                && node_tag_matches(node, tag)
        })
        .collect();

    candidates.retain(|node| {
        node.range
            .as_ref()
            .is_some_and(|range| range.column == location.column)
    });

    if candidates.len() == 1 {
        candidates.pop()
    } else {
        None
    }
}

fn normalize_model_path(path: &str) -> &str {
    path
}

pub(super) fn same_model_path(left: &str, right: &str) -> bool {
    left == right || normalize_model_path(left) == normalize_model_path(right)
}

fn node_tag_matches(node: &SourceNode, tag: Option<&str>) -> bool {
    let Some(tag) = tag else {
        return true;
    };
    let normalized = tag.trim().to_ascii_lowercase();
    normalized.is_empty()
        || node
            .label
            .to_ascii_lowercase()
            .starts_with(&format!("<{normalized}"))
}

fn source_location_label(location: Option<&ProjectSourceEditLocation>) -> String {
    match location {
        Some(location) if location.column > 0 => {
            format!("{}:{}:{}", location.file, location.line, location.column)
        }
        Some(location) => format!("{}:{}", location.file, location.line),
        None => "fără locație".to_string(),
    }
}

pub(super) fn source_missing_message(
    kind: &str,
    source_id: Option<&str>,
    location: Option<&ProjectSourceEditLocation>,
    selector: Option<&str>,
) -> String {
    let id = source_id.unwrap_or("fără Source ID");
    let loc = source_location_label(location);
    match selector {
        Some(selector) if !selector.is_empty() => format!(
            "Nu am putut ancora {kind} în Project Model. Source ID: {id}; locație: {loc}; selector live: {selector}."
        ),
        _ => format!("Nu am putut ancora {kind} în Project Model. Source ID: {id}; locație: {loc}."),
    }
}

fn html_node_spans(model: &ProjectModel) -> Vec<HtmlNodeSpan> {
    model
        .source_graph
        .nodes
        .iter()
        .filter(|node| node.kind == SourceNodeKind::Html)
        .filter_map(|node| {
            let file = model
                .files
                .iter()
                .find(|file| same_model_path(&file.relative_path, &node.file))?;
            let range = node.range.as_ref()?;
            let span = resolve_html_element_span(&file.contents, range.start).ok()?;
            let snippet = file.contents.get(span.start..span.end)?;
            Some(HtmlNodeSpan {
                id: node.id.clone(),
                file: node.file.clone(),
                fingerprint: html_fingerprint(&node.label, snippet),
            })
        })
        .collect()
}

fn html_fingerprint(label: &str, snippet: &str) -> String {
    let normalized = snippet
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    format!("{label}::{normalized}")
}

fn apply_html_move(
    source: &str,
    source_span: Span,
    target_span: Span,
    target_tag: &str,
    position: ProjectMovePosition,
) -> Result<MoveApplication, String> {
    let removal = removal_range_for_span(source, source_span);
    let removed_length = removal.end.saturating_sub(removal.start);
    let moving_source = source
        .get(source_span.start..source_span.end)
        .ok_or_else(|| "Range sursă invalid.".to_string())?
        .trim_end()
        .to_string();
    let without_source = format!("{}{}", &source[..removal.start], &source[removal.end..]);
    let adjust_index = |index: usize| {
        if index > removal.start {
            index.saturating_sub(removed_length)
        } else {
            index
        }
    };
    let adjusted_target_start = adjust_index(target_span.start);
    let adjusted_target_end = adjust_index(target_span.end);
    let target_indent = line_indent_at_offset(source, target_span.start);
    let source_start_line = line_number_at_offset(source, source_span.start);
    let source_end_line = line_number_at_offset(source, source_span.end);

    match position {
        ProjectMovePosition::Before => {
            let moving = reindent_html_fragment(&moving_source, &target_indent);
            Ok(MoveApplication {
                contents: format!(
                    "{}{}\n{}",
                    &without_source[..adjusted_target_start],
                    moving,
                    &without_source[adjusted_target_start..]
                ),
                source_start_line,
                source_end_line,
                new_start_line: line_number_at_offset(&without_source, adjusted_target_start),
            })
        }
        ProjectMovePosition::After => {
            let moving = reindent_html_fragment(&moving_source, &target_indent);
            Ok(MoveApplication {
                contents: format!(
                    "{}\n{}{}",
                    &without_source[..adjusted_target_end],
                    moving,
                    &without_source[adjusted_target_end..]
                ),
                source_start_line,
                source_end_line,
                new_start_line: line_number_at_offset(&without_source, adjusted_target_end) + 1,
            })
        }
        ProjectMovePosition::Inside => {
            let target_source = without_source
                .get(adjusted_target_start..adjusted_target_end)
                .ok_or_else(|| "Range destinație invalid după eliminarea sursei.".to_string())?;
            let close_tag = format!("</{target_tag}>");
            let close_offset = target_source
                .to_ascii_lowercase()
                .rfind(&close_tag.to_ascii_lowercase())
                .ok_or_else(|| format!("Nu am găsit {close_tag} pentru mutare."))?;
            let opening =
                parse_html_tag_at(&without_source, adjusted_target_start).ok_or_else(|| {
                    "Nu am putut reciti tag-ul destinație după eliminarea sursei.".to_string()
                })?;
            let child_indent = format!("{target_indent}  ");
            let moving = reindent_html_fragment(&moving_source, &child_indent);
            let insert_at = adjusted_target_start + close_offset;
            let before_insert = inside_prefix_for_insert(&without_source, opening.end, insert_at);
            let next_contents = format!(
                "{}\n{}\n{}{}",
                before_insert,
                moving,
                target_indent,
                &without_source[insert_at..]
            );
            let contents =
                reindent_html_subtree_at(&next_contents, adjusted_target_start, &target_indent)
                    .unwrap_or(next_contents);
            Ok(MoveApplication {
                contents,
                source_start_line,
                source_end_line,
                new_start_line: line_number_at_offset(&without_source, insert_at) + 1,
            })
        }
    }
}

fn reindent_html_subtree_at(
    source: &str,
    opening_start: usize,
    base_indent: &str,
) -> Option<String> {
    let span = resolve_html_element_span(source, opening_start).ok()?;
    let fragment = source.get(span.start..span.end)?;
    let formatted = reindent_html_fragment(fragment, base_indent);
    Some(format!(
        "{}{}{}",
        &source[..span.start],
        formatted,
        &source[span.end..]
    ))
}

pub(super) fn inside_prefix_for_insert(
    source: &str,
    opening_end: usize,
    insert_at: usize,
) -> String {
    let before_insert = source.get(..insert_at).unwrap_or(source);
    let existing_content = source.get(opening_end..insert_at).unwrap_or("");
    if existing_content.trim().is_empty() {
        source
            .get(..opening_end)
            .unwrap_or(before_insert)
            .to_string()
    } else {
        before_insert.trim_end().to_string()
    }
}

pub(super) fn resolve_html_element_span(
    source: &str,
    opening_start: usize,
) -> Result<Span, String> {
    let opening = parse_html_tag_at(source, opening_start)
        .ok_or_else(|| "Range-ul Source Graph nu mai indică un tag HTML stabil.".to_string())?;
    if opening.is_closing {
        return Err("Range-ul indică un tag HTML de închidere, nu un element mutabil.".to_string());
    }
    if opening.is_self_closing || is_void_tag(&opening.tag) {
        return Ok(Span {
            start: opening.start,
            end: opening.end,
        });
    }

    let mut depth = 1usize;
    let mut cursor = opening.end;
    while let Some(tag) = next_html_tag(source, cursor) {
        cursor = tag.end;
        if tag.tag != opening.tag {
            continue;
        }
        if tag.is_closing {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Ok(Span {
                    start: opening.start,
                    end: tag.end,
                });
            }
        } else if !tag.is_self_closing && !is_void_tag(&tag.tag) {
            depth += 1;
        }
    }

    Err(format!(
        "Nu am găsit o închidere clară pentru <{}>. Mutarea este blocată.",
        opening.tag
    ))
}

pub(super) fn html_tag_at(source: &str, opening_start: usize) -> Result<String, String> {
    parse_html_tag_at(source, opening_start)
        .map(|tag| tag.tag)
        .ok_or_else(|| "Nu am putut citi tag-ul HTML din Source Graph.".to_string())
}

pub(crate) fn parse_html_tag_at(source: &str, start: usize) -> Option<HtmlTag> {
    let bytes = source.as_bytes();
    if bytes.get(start).copied()? != b'<' {
        return None;
    }
    let mut cursor = start + 1;
    let is_closing = bytes.get(cursor).copied() == Some(b'/');
    if is_closing {
        cursor += 1;
    }
    while bytes
        .get(cursor)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        cursor += 1;
    }
    let name_start = cursor;
    while bytes
        .get(cursor)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'-' || *byte == b':')
    {
        cursor += 1;
    }
    if cursor == name_start {
        return None;
    }
    let tag = source.get(name_start..cursor)?.to_ascii_lowercase();
    let end = opening_tag_end(source, cursor)?;
    let raw = source.get(start..end)?;
    Some(HtmlTag {
        start,
        end,
        tag,
        is_closing,
        is_self_closing: raw.trim_end().ends_with("/>"),
    })
}

fn next_html_tag(source: &str, start: usize) -> Option<HtmlTag> {
    let bytes = source.as_bytes();
    let mut cursor = start;
    while cursor < bytes.len() {
        if is_tera_start(bytes, cursor) {
            cursor = skip_tera_token(bytes, cursor).unwrap_or(cursor + 2);
            continue;
        }
        if bytes[cursor] != b'<' {
            cursor += 1;
            continue;
        }
        let after_lt = cursor + 1;
        let next = bytes.get(after_lt).copied()?;
        if next == b'!' || next == b'?' {
            cursor += 1;
            continue;
        }
        if next != b'/' && !next.is_ascii_alphabetic() {
            cursor += 1;
            continue;
        }
        if let Some(tag) = parse_html_tag_at(source, cursor) {
            return Some(tag);
        }
        cursor += 1;
    }
    None
}

fn opening_tag_end(source: &str, mut index: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut in_double_quote = false;
    let mut in_single_quote = false;

    while index < bytes.len() {
        if !in_double_quote && !in_single_quote && is_tera_start(bytes, index) {
            index = skip_tera_token(bytes, index).unwrap_or(index + 2);
            continue;
        }

        match bytes[index] {
            b'"' if !in_single_quote => in_double_quote = !in_double_quote,
            b'\'' if !in_double_quote => in_single_quote = !in_single_quote,
            b'>' if !in_double_quote && !in_single_quote => return Some(index + 1),
            _ => {}
        }
        index += 1;
    }
    None
}

fn is_tera_start(bytes: &[u8], index: usize) -> bool {
    index + 1 < bytes.len()
        && bytes[index] == b'{'
        && matches!(bytes[index + 1], b'%' | b'{' | b'#')
}

fn skip_tera_token(bytes: &[u8], index: usize) -> Option<usize> {
    let (close_a, close_b) = match bytes.get(index + 1).copied()? {
        b'%' => (b'%', b'}'),
        b'{' => (b'}', b'}'),
        b'#' => (b'#', b'}'),
        _ => return None,
    };
    let mut cursor = index + 2;
    while cursor + 1 < bytes.len() {
        if bytes[cursor] == close_a && bytes[cursor + 1] == close_b {
            return Some(cursor + 2);
        }
        cursor += 1;
    }
    None
}

pub(super) fn removal_range_for_span(source: &str, span: Span) -> Span {
    let line_start = source
        .get(..span.start)
        .and_then(|prefix| prefix.rfind('\n').map(|index| index + 1))
        .unwrap_or(0);
    let line_end_index = source
        .get(span.end..)
        .and_then(|suffix| suffix.find('\n').map(|index| span.end + index));
    let line_end = line_end_index.unwrap_or(source.len());
    let before_on_line = source.get(line_start..span.start).unwrap_or("");
    let after_on_line = source.get(span.end..line_end).unwrap_or("");

    if before_on_line.trim().is_empty() && after_on_line.trim().is_empty() {
        let end = line_end_index.map(|index| index + 1).unwrap_or(line_end);
        return Span {
            start: line_start,
            end,
        };
    }

    span
}

pub(super) fn line_number_at_offset(source: &str, offset: usize) -> usize {
    source
        .get(..offset.min(source.len()))
        .unwrap_or("")
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

pub(super) fn source_location_at_offset(
    source: &str,
    file: &str,
    offset: usize,
) -> ProjectSourceEditLocation {
    let prefix = source.get(..offset.min(source.len())).unwrap_or("");
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let line_start = prefix.rfind('\n').map(|index| index + 1).unwrap_or(0);
    let column = prefix[line_start..].chars().count() + 1;
    ProjectSourceEditLocation {
        file: file.to_string(),
        line,
        column,
    }
}

pub(super) fn offset_for_source_location(
    source: &str,
    location: &ProjectSourceEditLocation,
) -> Result<usize, String> {
    if location.line == 0 {
        return Err("Locația sursă are linie invalidă pentru editare.".to_string());
    }

    let mut line = 1usize;
    let mut line_start = 0usize;
    for (index, character) in source.char_indices() {
        if line == location.line {
            break;
        }
        if character == '\n' {
            line += 1;
            line_start = index + character.len_utf8();
        }
    }
    if line != location.line {
        return Err(format!(
            "Locația {}:{} nu există în sursa curentă.",
            location.file, location.line
        ));
    }

    if location.column <= 1 {
        return Ok(line_start);
    }

    let target_column = location.column;
    let mut column = 1usize;
    let mut cursor = line_start;
    for (relative_index, character) in source[line_start..].char_indices() {
        if character == '\n' || character == '\r' {
            break;
        }
        if column == target_column {
            return Ok(line_start + relative_index);
        }
        column += 1;
        cursor = line_start + relative_index + character.len_utf8();
    }
    if column == target_column {
        return Ok(cursor);
    }

    Err(format!(
        "Coloana {} nu există pe linia {} în {}.",
        location.column, location.line, location.file
    ))
}

pub(super) fn line_indent_at_offset(source: &str, offset: usize) -> String {
    let prefix = source.get(..offset.min(source.len())).unwrap_or("");
    let line_start = prefix.rfind('\n').map(|index| index + 1).unwrap_or(0);
    source
        .get(line_start..)
        .unwrap_or("")
        .chars()
        .take_while(|character| {
            character.is_whitespace() && *character != '\n' && *character != '\r'
        })
        .collect()
}

pub(super) fn reindent_html_fragment(source: &str, base_indent: &str) -> String {
    let trimmed = source.trim_matches(|character| character == '\n' || character == '\r');
    let mut level = 0usize;
    trimmed
        .split('\n')
        .map(|line| {
            let content = line.trim();
            if content.is_empty() {
                return String::new();
            }
            let leading_closes = leading_closing_tag_count(content);
            let display_level = level.saturating_sub(leading_closes);
            let (opens, closes) = html_line_depth_delta(content);
            level = display_level + opens;
            level = level.saturating_sub(closes.saturating_sub(leading_closes));
            format!("{}{}{}", base_indent, "  ".repeat(display_level), content)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn leading_closing_tag_count(line: &str) -> usize {
    let mut count = 0usize;
    let mut cursor = 0usize;
    while cursor < line.len() {
        while line
            .as_bytes()
            .get(cursor)
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            cursor += 1;
        }
        let Some(tag) = parse_html_tag_at(line, cursor) else {
            break;
        };
        if !tag.is_closing {
            break;
        }
        count += 1;
        cursor = tag.end;
    }
    count
}

fn html_line_depth_delta(line: &str) -> (usize, usize) {
    let mut opens = 0usize;
    let mut closes = 0usize;
    let mut cursor = 0usize;
    while let Some(tag) = next_html_tag(line, cursor) {
        cursor = tag.end;
        if tag.is_closing {
            closes += 1;
        } else if !tag.is_self_closing && !is_void_tag(&tag.tag) {
            opens += 1;
        }
    }
    (opens, closes)
}

pub(super) fn can_receive_children(tag: &str) -> bool {
    CONTAINER_TAGS
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(tag))
}

fn is_void_tag(tag: &str) -> bool {
    VOID_TAGS
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(tag))
}

pub(crate) fn content_revision(contents: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    contents.hash(&mut hasher);
    format!("f_{:016x}", hasher.finish())
}

#[allow(dead_code)]
fn source_id_for_html_opening(file: &str, label: &str, start: usize, end: usize) -> String {
    source_node_id(file, &SourceNodeKind::Html, label, Some(start), Some(end))
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::project_model::build_project_model;

    use super::*;

    #[test]
    fn html_move_refuses_cross_source_transfer_from_partial_to_page() {
        let root = unique_test_dir();
        write_project(&root);
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let source = model
            .source_graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == SourceNodeKind::Html
                    && node.file.ends_with("templates/partials/card.html")
                    && node.label.starts_with("<article")
            })
            .unwrap();
        let target = model
            .source_graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == SourceNodeKind::Html
                    && node.file.ends_with("templates/index.html")
                    && node.label.starts_with("<section")
            })
            .unwrap();
        let index_path = root.join("templates/index.html");
        let partial_path = root.join("templates/partials/card.html");
        let index_before = fs::read_to_string(&index_path).unwrap();
        let partial_before = fs::read_to_string(&partial_path).unwrap();

        let plan = plan_html_move(
            &model,
            &ProjectHtmlMoveIntent {
                source_source_id: Some(source.id.clone()),
                target_source_id: Some(target.id.clone()),
                source_location: None,
                target_location: None,
                source_tag: Some("article".to_string()),
                target_tag: Some("section".to_string()),
                source_selector: Some(".card".to_string()),
                target_selector: Some(".hero".to_string()),
                position: ProjectMovePosition::Before,
            },
            &HashMap::new(),
        );

        assert!(!plan.allowed);
        assert!(plan.patch.is_none());
        assert!(plan
            .diagnostic
            .as_deref()
            .is_some_and(|message| message.contains("template-uri diferite")));
        assert_eq!(fs::read_to_string(index_path).unwrap(), index_before);
        assert_eq!(fs::read_to_string(partial_path).unwrap(), partial_before);
        fs::remove_dir_all(root).unwrap();
    }

    fn write_project(root: &PathBuf) {
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates/partials")).unwrap();
        fs::write(
            root.join("zola.toml"),
            "base_url = \"http://example.test\"\n",
        )
        .unwrap();
        fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = \"Acasă\"\ntemplate = \"index.html\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("templates/index.html"),
            concat!(
                "<main>\n",
                "  <section class=\"hero\"></section>\n",
                "  {% include \"partials/card.html\" %}\n",
                "</main>\n",
            ),
        )
        .unwrap();
        fs::write(
            root.join("templates/partials/card.html"),
            "<article class=\"card\"><p>Card</p></article>\n",
        )
        .unwrap();
    }

    fn unique_test_dir() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "pana-studio-html-move-source-boundary-{}-{stamp}",
            std::process::id()
        ))
    }
}
