use std::{
    collections::{BTreeSet, HashMap, HashSet},
    hash::{Hash, Hasher},
};

use serde::{Deserialize, Serialize};

use crate::{
    blocks::{native_block_by_id, native_block_instance_id},
    project_model::model::{ProjectModel, ProjectModelFile, ProjectModelFileKind},
    source_graph::model::SourceNode,
};

use super::move_engine::{
    content_revision, direct_location_without_source_id, line_indent_at_offset,
    line_number_at_offset, offset_for_source_location, parse_html_tag_at, reindent_html_fragment,
    resolve_html_element_span, resolve_html_node_for_anchor, same_model_path,
    source_location_at_offset, source_missing_message, ProjectSourceEditLocation, Span,
};
use super::zola_image_engine::{contains_zola_image_contract, zola_image_contract_start};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlDuplicateIntent {
    pub source_source_id: Option<String>,
    pub source_location: Option<ProjectSourceEditLocation>,
    pub source_tag: Option<String>,
    pub source_selector: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlDuplicatePlan {
    pub allowed: bool,
    pub diagnostic: Option<String>,
    pub model_revision: String,
    pub patch: Option<ProjectHtmlDuplicatePatch>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlDuplicatePatch {
    pub file: String,
    pub resolved_source_id: String,
    pub duplicated_label: String,
    pub before_revision: String,
    pub after_revision: String,
    pub contents: String,
    pub source_location: ProjectSourceEditLocation,
    pub inserted_location: ProjectSourceEditLocation,
    pub source_start_line: usize,
    pub source_end_line: usize,
    pub inserted_start_line: usize,
    pub line_shift_start: usize,
    pub line_shift: isize,
    pub tag: String,
    pub html: String,
    pub block_ids: Vec<String>,
    pub data_anim_count: usize,
    pub duplicate_id_count: usize,
    pub zola_image_contract: bool,
}

struct DuplicateHtml {
    html: String,
    block_ids: Vec<String>,
    data_anim_count: usize,
    duplicate_id_count: usize,
}

struct DuplicateApplication {
    contents: String,
    inserted_location: ProjectSourceEditLocation,
    source_start_line: usize,
    source_end_line: usize,
    inserted_start_line: usize,
    line_shift_start: usize,
    line_shift: isize,
}

#[derive(Clone, Debug)]
struct TagAttribute {
    attr_start: usize,
    value_start: usize,
    value_end: usize,
    attr_end: usize,
}

const STUDIO_ATTRIBUTES: &[&str] = &[
    "data-pana-source-id",
    "data-pana-template-source-id",
    "data-pana-preview-revision",
    "data-pana-session-id",
];

pub fn plan_html_duplicate(
    model: &ProjectModel,
    intent: &ProjectHtmlDuplicateIntent,
    aliases: &HashMap<String, String>,
) -> ProjectHtmlDuplicatePlan {
    match plan_html_duplicate_inner(model, intent, aliases) {
        Ok(patch) => ProjectHtmlDuplicatePlan {
            allowed: true,
            diagnostic: None,
            model_revision: model.revision.clone(),
            patch: Some(patch),
        },
        Err(message) => ProjectHtmlDuplicatePlan {
            allowed: false,
            diagnostic: Some(message),
            model_revision: model.revision.clone(),
            patch: None,
        },
    }
}

fn plan_html_duplicate_inner(
    model: &ProjectModel,
    intent: &ProjectHtmlDuplicateIntent,
    aliases: &HashMap<String, String>,
) -> Result<ProjectHtmlDuplicatePatch, String> {
    if let Some(source_node) = resolve_html_node_for_anchor(
        model,
        intent.source_source_id.as_deref(),
        intent.source_location.as_ref(),
        intent.source_tag.as_deref(),
        aliases,
    ) {
        return plan_html_duplicate_from_source_node(model, source_node);
    }

    if let Some(location) = direct_location_without_source_id(
        intent.source_source_id.as_deref(),
        intent.source_location.as_ref(),
    ) {
        return plan_html_duplicate_from_direct_location(model, intent, location);
    }

    Err(source_missing_message(
        "sursă",
        intent.source_source_id.as_deref(),
        intent.source_location.as_ref(),
        intent.source_selector.as_deref(),
    ))
}

fn plan_html_duplicate_from_source_node(
    model: &ProjectModel,
    source_node: &SourceNode,
) -> Result<ProjectHtmlDuplicatePatch, String> {
    if !source_node.capabilities.can_edit_visual {
        return Err(source_node
            .capabilities
            .reason
            .clone()
            .unwrap_or_else(|| "Elementul sursă nu este duplicabil vizual.".to_string()));
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
            "HTML Duplicate Engine este activ doar pentru template-uri Zola/Tera.".to_string(),
        );
    }

    let source_range = source_node
        .range
        .as_ref()
        .ok_or_else(|| "Sursa nu are range stabil în Source Graph.".to_string())?;
    let mut source_span = resolve_html_element_span(&file.contents, source_range.start)?;
    if let Some(contract_start) = zola_image_contract_start(&file.contents, source_range.start)? {
        source_span.start = contract_start;
    }

    plan_html_duplicate_for_span(
        model,
        file,
        &source_node.file,
        source_span,
        source_range.start,
        source_node.id.clone(),
        source_node.label.clone(),
    )
}

fn plan_html_duplicate_from_direct_location(
    model: &ProjectModel,
    intent: &ProjectHtmlDuplicateIntent,
    location: &ProjectSourceEditLocation,
) -> Result<ProjectHtmlDuplicatePatch, String> {
    let file = model
        .files
        .iter()
        .find(|file| same_model_path(&file.relative_path, &location.file))
        .ok_or_else(|| format!("Nu am găsit fișierul {} în Project Model.", location.file))?;

    if !is_direct_html_duplicate_file(file) {
        return Err(
            "Duplicarea prin locație directă este activă doar pentru fișiere HTML 1:1 din proiect, nu pentru template-uri Tera.".to_string(),
        );
    }

    let offset = offset_for_source_location(&file.contents, location)?;
    let tag = parse_html_tag_at(&file.contents, offset)
        .ok_or_else(|| "Locația nu indică începutul unui tag HTML duplicabil.".to_string())?;
    if tag.is_closing {
        return Err("Locația indică un tag de închidere, nu un element duplicabil.".to_string());
    }
    if tag.tag == "html" || tag.tag == "body" {
        return Err("Elementul rădăcină nu poate fi duplicat.".to_string());
    }
    if let Some(expected_tag) = intent.source_tag.as_deref() {
        let expected_tag = expected_tag.trim().to_ascii_lowercase();
        if !expected_tag.is_empty() && expected_tag != tag.tag {
            return Err(format!(
                "Locația indică <{}>, dar intenția preview a cerut <{}>.",
                tag.tag, expected_tag
            ));
        }
    }

    let source_span = resolve_html_element_span(&file.contents, tag.start)?;
    let resolved_source_id = intent.source_source_id.clone().unwrap_or_else(|| {
        format!(
            "location:{}:{}:{}",
            location.file, location.line, location.column
        )
    });

    plan_html_duplicate_for_span(
        model,
        file,
        &file.relative_path,
        source_span,
        tag.start,
        resolved_source_id,
        format!("<{}>", tag.tag),
    )
}

fn plan_html_duplicate_for_span(
    model: &ProjectModel,
    file: &ProjectModelFile,
    file_path: &str,
    source_span: Span,
    opening_start: usize,
    resolved_source_id: String,
    duplicated_label: String,
) -> Result<ProjectHtmlDuplicatePatch, String> {
    let source_html = file
        .contents
        .get(source_span.start..source_span.end)
        .ok_or_else(|| "Range sursă invalid pentru duplicare.".to_string())?;
    let tag = parse_html_tag_at(&file.contents, opening_start)
        .map(|tag| tag.tag)
        .ok_or_else(|| "Nu am putut citi tag-ul HTML pentru duplicare.".to_string())?;
    let duplicate = prepare_duplicated_html(model, &tag, source_html);
    let zola_image_contract = contains_zola_image_contract(source_html);
    let applied =
        apply_html_duplicate_after(&file.contents, file_path, source_span, &duplicate.html)?;

    Ok(ProjectHtmlDuplicatePatch {
        file: file_path.to_string(),
        resolved_source_id,
        duplicated_label,
        before_revision: file.revision.clone(),
        after_revision: content_revision(&applied.contents),
        contents: applied.contents,
        source_location: source_location_at_offset(&file.contents, file_path, source_span.start),
        inserted_location: applied.inserted_location,
        source_start_line: applied.source_start_line,
        source_end_line: applied.source_end_line,
        inserted_start_line: applied.inserted_start_line,
        line_shift_start: applied.line_shift_start,
        line_shift: applied.line_shift,
        tag,
        html: duplicate.html,
        block_ids: duplicate.block_ids,
        data_anim_count: duplicate.data_anim_count,
        duplicate_id_count: duplicate.duplicate_id_count,
        zola_image_contract,
    })
}

fn is_direct_html_duplicate_file(file: &ProjectModelFile) -> bool {
    matches!(
        file.kind,
        ProjectModelFileKind::StaticText | ProjectModelFileKind::OtherText
    ) && is_html_path(&file.relative_path)
}

fn is_html_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".html") || lower.ends_with(".htm")
}

fn apply_html_duplicate_after(
    source: &str,
    file: &str,
    source_span: Span,
    snippet: &str,
) -> Result<DuplicateApplication, String> {
    let target_indent = line_indent_at_offset(source, source_span.start);
    let inserted = reindent_html_fragment(snippet, &target_indent);
    let source_start_line = line_number_at_offset(source, source_span.start);
    let source_end_line = line_number_at_offset(source, source_span.end);
    let inserted_start_line = line_number_at_offset(source, source_span.end) + 1;
    Ok(DuplicateApplication {
        contents: format!(
            "{}\n{}{}",
            &source[..source_span.end],
            inserted,
            &source[source_span.end..]
        ),
        inserted_location: ProjectSourceEditLocation {
            file: file.to_string(),
            line: inserted_start_line,
            column: target_indent.chars().count() + 1,
        },
        source_start_line,
        source_end_line,
        inserted_start_line,
        line_shift_start: inserted_start_line,
        line_shift: snippet_line_count(&inserted) as isize,
    })
}

fn prepare_duplicated_html(model: &ProjectModel, tag: &str, html: &str) -> DuplicateHtml {
    let mut identity_texts = model
        .files
        .iter()
        .map(|file| file.contents.clone())
        .collect::<Vec<_>>();
    identity_texts.push(html.to_string());

    let mut next = strip_studio_attributes(html);
    let data_anim_replacements = build_data_anim_replacements(&next, tag, &mut identity_texts);
    let id_replacements = build_id_replacements(&next, &identity_texts);
    let duplicate_id_count = id_replacements.len();
    let mut block_ids = BTreeSet::new();

    next = replace_class_tokens(&next, &data_anim_replacements);
    next = replace_attribute_values(&next, "data-anim", &data_anim_replacements);
    next = replace_attribute_values(&next, "id", &id_replacements);
    next = replace_whitespace_token_references(&next, &id_replacements);
    next = replace_hash_references(&next, &id_replacements);
    next = update_block_instances(&next, &mut block_ids);

    DuplicateHtml {
        html: next.trim_end().to_string(),
        block_ids: block_ids.into_iter().collect(),
        data_anim_count: data_anim_replacements.len(),
        duplicate_id_count,
    }
}

fn strip_studio_attributes(source: &str) -> String {
    rewrite_opening_tags(source, |tag| {
        STUDIO_ATTRIBUTES
            .iter()
            .fold(tag.to_string(), |next, attr| {
                remove_tag_attribute(&next, attr)
            })
    })
}

fn build_data_anim_replacements(
    html: &str,
    tag: &str,
    identity_texts: &mut Vec<String>,
) -> HashMap<String, String> {
    let mut replacements = HashMap::new();
    for value in collect_attribute_values(html, "data-anim") {
        if replacements.contains_key(&value) {
            continue;
        }
        let candidate = unique_html_identity(tag, &value, identity_texts);
        identity_texts.push(candidate.clone());
        replacements.insert(value, candidate);
    }
    replacements
}

fn build_id_replacements(html: &str, identity_texts: &[String]) -> HashMap<String, String> {
    let mut replacements = HashMap::new();
    let mut taken = HashSet::new();
    for value in collect_attribute_values(html, "id") {
        if value.trim().is_empty() || replacements.contains_key(&value) {
            continue;
        }
        replacements.insert(
            value.clone(),
            unique_duplicate_id(&value, identity_texts, &mut taken),
        );
    }
    replacements
}

fn collect_attribute_values(source: &str, attr: &str) -> Vec<String> {
    let mut values = Vec::new();
    rewrite_opening_tags(source, |tag| {
        let mut cursor = 0;
        while let Some(attribute) = find_tag_attribute(tag, attr, cursor) {
            let value = tag[attribute.value_start..attribute.value_end].trim();
            if !value.is_empty() {
                values.push(value.to_string());
            }
            cursor = attribute.attr_end;
        }
        tag.to_string()
    });
    values
}

fn replace_attribute_values(
    source: &str,
    attr: &str,
    replacements: &HashMap<String, String>,
) -> String {
    replace_attribute_values_with(source, attr, |value| replacements.get(value).cloned())
}

fn replace_attribute_values_with<F>(source: &str, attr: &str, mut replace: F) -> String
where
    F: FnMut(&str) -> Option<String>,
{
    rewrite_opening_tags(source, |tag| {
        let mut next = tag.to_string();
        let mut cursor = 0;
        while let Some(attribute) = find_tag_attribute(&next, attr, cursor) {
            let value = next[attribute.value_start..attribute.value_end].to_string();
            let Some(replacement) = replace(&value) else {
                cursor = attribute.attr_end;
                continue;
            };
            next = replace_range(
                &next,
                attribute.value_start,
                attribute.value_end,
                &escape_attr_value(&replacement),
            );
            cursor = attribute.value_start + replacement.len();
        }
        next
    })
}

fn replace_class_tokens(source: &str, replacements: &HashMap<String, String>) -> String {
    replace_attribute_values_with(source, "class", |value| {
        let tokens = value
            .split_whitespace()
            .map(|token| replacements.get(token).map(String::as_str).unwrap_or(token))
            .filter(|token| !token.is_empty())
            .collect::<Vec<_>>();
        let next = tokens.join(" ");
        if next == value {
            None
        } else {
            Some(next)
        }
    })
}

fn replace_whitespace_token_references(source: &str, ids: &HashMap<String, String>) -> String {
    [
        "aria-controls",
        "aria-labelledby",
        "aria-describedby",
        "aria-owns",
        "aria-activedescendant",
        "for",
    ]
    .iter()
    .fold(source.to_string(), |next, attr| {
        replace_attribute_values_with(&next, attr, |value| {
            let tokens = value
                .split_whitespace()
                .map(|token| ids.get(token).map(String::as_str).unwrap_or(token))
                .collect::<Vec<_>>();
            let replaced = tokens.join(" ");
            if replaced == value {
                None
            } else {
                Some(replaced)
            }
        })
    })
}

fn replace_hash_references(source: &str, ids: &HashMap<String, String>) -> String {
    ["href", "data-target"]
        .iter()
        .fold(source.to_string(), |next, attr| {
            replace_attribute_values_with(&next, attr, |value| {
                let id = value.strip_prefix('#')?;
                ids.get(id).map(|next_id| format!("#{next_id}"))
            })
        })
}

fn update_block_instances(source: &str, block_ids: &mut BTreeSet<String>) -> String {
    rewrite_opening_tags(source, |tag| {
        let block_id = tag_attribute_value(tag, "data-pana-block")
            .or_else(|| tag_attribute_value(tag, "data-pana-component"));
        let Some(block_id) = block_id else {
            return tag.to_string();
        };
        let block_id = block_id.trim();
        if block_id.is_empty() || native_block_by_id(block_id).is_none() {
            return tag.to_string();
        }
        block_ids.insert(block_id.to_string());
        let Some(data_anim) = tag_attribute_value(tag, "data-anim") else {
            return tag.to_string();
        };
        let data_anim = data_anim.trim();
        if data_anim.is_empty() {
            return tag.to_string();
        }
        set_tag_attribute_value(
            tag,
            "data-pana-instance",
            &native_block_instance_id(block_id, data_anim),
        )
    })
}

fn rewrite_opening_tags<F>(source: &str, mut rewrite: F) -> String
where
    F: FnMut(&str) -> String,
{
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    while let Some(relative_start) = source[cursor..].find('<') {
        let tag_start = cursor + relative_start;
        output.push_str(&source[cursor..tag_start]);
        let Some(tag) = parse_html_tag_at(source, tag_start) else {
            output.push('<');
            cursor = tag_start + 1;
            continue;
        };
        let tag_source = &source[tag.start..tag.end];
        if tag.is_closing {
            output.push_str(tag_source);
        } else {
            output.push_str(&rewrite(tag_source));
        }
        cursor = tag.end;
    }
    output.push_str(&source[cursor..]);
    output
}

fn remove_tag_attribute(tag: &str, attr: &str) -> String {
    let mut next = tag.to_string();
    let mut cursor = 0;
    while let Some(attribute) = find_tag_attribute(&next, attr, cursor) {
        let remove_start = previous_whitespace_start(&next, attribute.attr_start);
        next = replace_range(&next, remove_start, attribute.attr_end, "");
        cursor = remove_start;
    }
    next
}

fn set_tag_attribute_value(tag: &str, attr: &str, value: &str) -> String {
    if let Some(attribute) = find_tag_attribute(tag, attr, 0) {
        return replace_range(
            tag,
            attribute.value_start,
            attribute.value_end,
            &escape_attr_value(value),
        );
    }
    insert_tag_attribute(tag, attr, value)
}

fn tag_attribute_value(tag: &str, attr: &str) -> Option<String> {
    let attribute = find_tag_attribute(tag, attr, 0)?;
    Some(tag[attribute.value_start..attribute.value_end].to_string())
}

fn find_tag_attribute(tag: &str, attr: &str, start: usize) -> Option<TagAttribute> {
    let mut cursor = start.min(tag.len());
    while let Some(relative_attr) = tag[cursor..].find(attr) {
        let attr_start = cursor + relative_attr;
        let attr_end = attr_start + attr.len();
        if !is_attr_boundary_before(tag, attr_start) || !is_attr_boundary_after(tag, attr_end) {
            cursor = attr_end;
            continue;
        }
        let mut value_cursor = skip_ascii_whitespace(tag, attr_end);
        if tag[value_cursor..].chars().next()? != '=' {
            cursor = attr_end;
            continue;
        }
        value_cursor += 1;
        value_cursor = skip_ascii_whitespace(tag, value_cursor);
        let quote = tag[value_cursor..].chars().next()?;
        if quote != '"' && quote != '\'' {
            cursor = attr_end;
            continue;
        }
        let value_start = value_cursor + quote.len_utf8();
        let value_end = tag[value_start..].find(quote)? + value_start;
        return Some(TagAttribute {
            attr_start,
            value_start,
            value_end,
            attr_end: value_end + quote.len_utf8(),
        });
    }
    None
}

fn is_attr_boundary_before(source: &str, index: usize) -> bool {
    source[..index]
        .chars()
        .next_back()
        .map(|character| {
            character.is_ascii_whitespace()
                || character == '<'
                || character == '/'
                || character == '%'
        })
        .unwrap_or(true)
}

fn is_attr_boundary_after(source: &str, index: usize) -> bool {
    source[index..]
        .chars()
        .next()
        .map(|character| {
            character.is_ascii_whitespace()
                || character == '='
                || character == '/'
                || character == '>'
        })
        .unwrap_or(true)
}

fn previous_whitespace_start(source: &str, index: usize) -> usize {
    let mut cursor = index;
    while cursor > 0 {
        let Some((previous_index, character)) = source[..cursor].char_indices().next_back() else {
            break;
        };
        if !character.is_ascii_whitespace() || character == '\n' || character == '\r' {
            break;
        }
        cursor = previous_index;
    }
    cursor
}

fn insert_tag_attribute(tag: &str, attr: &str, value: &str) -> String {
    let insert_at = tag
        .rfind("/>")
        .or_else(|| tag.rfind('>'))
        .unwrap_or(tag.len());
    format!(
        "{} {}=\"{}\"{}",
        &tag[..insert_at],
        attr,
        escape_attr_value(value),
        &tag[insert_at..]
    )
}

fn replace_range(source: &str, start: usize, end: usize, replacement: &str) -> String {
    let mut next = String::with_capacity(source.len() - (end - start) + replacement.len());
    next.push_str(&source[..start]);
    next.push_str(replacement);
    next.push_str(&source[end..]);
    next
}

fn escape_attr_value(value: &str) -> String {
    value.replace('&', "&amp;").replace('"', "&quot;")
}

fn skip_ascii_whitespace(source: &str, mut cursor: usize) -> usize {
    while let Some(character) = source[cursor..].chars().next() {
        if !character.is_ascii_whitespace() {
            break;
        }
        cursor += character.len_utf8();
    }
    cursor
}

fn unique_html_identity(tag: &str, existing_value: &str, identity_texts: &[String]) -> String {
    let tag = normalize_identity_tag(tag);
    for attempt in 0..80u32 {
        let token = identity_token(&format!("{tag}:{existing_value}"), attempt);
        let candidate = format!("ps-{tag}-{token}");
        if !identity_texts.iter().any(|text| text.contains(&candidate)) {
            return candidate;
        }
    }
    format!(
        "ps-{tag}-{}",
        identity_token(&format!("{tag}:{existing_value}:fallback"), 80)
    )
}

fn normalize_identity_tag(value: &str) -> String {
    let mut output = String::new();
    let mut last_was_dash = false;
    for character in value.trim().chars() {
        if character.is_ascii_alphanumeric() {
            output.push(character.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash && !output.is_empty() {
            output.push('-');
            last_was_dash = true;
        }
    }
    let trimmed = output.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "el".to_string()
    } else {
        trimmed
    }
}

fn identity_token(seed: &str, attempt: u32) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    seed.hash(&mut hasher);
    attempt.hash(&mut hasher);
    let token = base36(hasher.finish());
    if token.len() >= 8 {
        token[..8].to_string()
    } else {
        format!("{token:0>8}")
    }
}

fn base36(mut value: u64) -> String {
    const ALPHABET: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if value == 0 {
        return "0".to_string();
    }
    let mut output = Vec::new();
    while value > 0 {
        output.push(ALPHABET[(value % 36) as usize] as char);
        value /= 36;
    }
    output.iter().rev().collect()
}

fn unique_duplicate_id(id: &str, source_texts: &[String], taken: &mut HashSet<String>) -> String {
    let base = normalize_duplicate_id_base(id);
    for index in 1..=120 {
        let candidate = if index == 1 {
            format!("{base}-copy")
        } else {
            format!("{base}-copy-{index}")
        };
        if taken.contains(&candidate) {
            continue;
        }
        if source_texts
            .iter()
            .any(|source| source.contains(&candidate))
        {
            continue;
        }
        taken.insert(candidate.clone());
        return candidate;
    }
    let fallback = format!("{base}-copy-{}", identity_token(id, 121));
    taken.insert(fallback.clone());
    fallback
}

fn normalize_duplicate_id_base(id: &str) -> String {
    let trimmed = id.trim();
    let without_copy = trimmed
        .strip_suffix("-copy")
        .or_else(|| copy_suffix_base(trimmed))
        .unwrap_or(trimmed);
    let mut output = String::new();
    let mut last_was_dash = false;
    for character in without_copy.chars() {
        if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
            output.push(character);
            last_was_dash = false;
        } else if !last_was_dash && !output.is_empty() {
            output.push('-');
            last_was_dash = true;
        }
    }
    let normalized = output.trim_matches('-');
    if normalized.is_empty() {
        "element".to_string()
    } else {
        normalized.to_string()
    }
}

fn copy_suffix_base(value: &str) -> Option<&str> {
    let (base, suffix) = value.rsplit_once("-copy-")?;
    if !base.is_empty() && suffix.chars().all(|character| character.is_ascii_digit()) {
        Some(base)
    } else {
        None
    }
}

fn snippet_line_count(value: &str) -> usize {
    value.split('\n').count()
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
    fn plan_html_duplicate_rewrites_html_identity_and_references() {
        let root = unique_test_dir();
        write_project(
            &root,
            concat!(
                "{% block content %}\n",
                "<section class=\"hero\">\n",
                "  <div id=\"card\" class=\"card ps-card-old\" data-anim=\"ps-card-old\" data-pana-source-id=\"stale\">\n",
                "    <button id=\"cta\" aria-controls=\"card\" data-target=\"#card\">A</button>\n",
                "  </div>\n",
                "</section>\n",
                "{% endblock %}\n",
            ),
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let card = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label.starts_with("<div") && node.label.contains("card"))
            .unwrap();

        let plan = plan_html_duplicate(
            &model,
            &ProjectHtmlDuplicateIntent {
                source_source_id: Some(card.id.clone()),
                source_location: None,
                source_tag: Some("div".to_string()),
                source_selector: Some(".card".to_string()),
            },
            &HashMap::new(),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.unwrap();
        assert_eq!(patch.tag, "div");
        assert_eq!(patch.duplicate_id_count, 2);
        assert_eq!(patch.data_anim_count, 1);
        assert!(!patch.html.contains("data-pana-source-id"));
        assert!(patch.html.contains("id=\"card-copy\""));
        assert!(patch.html.contains("id=\"cta-copy\""));
        assert!(patch.html.contains("aria-controls=\"card-copy\""));
        assert!(patch.html.contains("data-target=\"#card-copy\""));
        assert!(patch.html.contains("class=\"card ps-div-"));
        assert!(patch.html.contains("data-anim=\"ps-div-"));
        assert!(!patch.html.contains("ps-card-old"));
        assert!(patch.inserted_start_line > patch.source_end_line);
        assert!(patch.contents.contains("\n  <div id=\"card-copy\""));
    }

    #[test]
    fn plan_html_duplicate_normalizes_registered_block_instance() {
        let root = unique_test_dir();
        write_project(
            &root,
            concat!(
                "{% block content %}\n",
                "<section>\n",
                "  <span class=\"counter ps-counter-old\" data-anim=\"ps-counter-old\" data-pana-block=\"counter\" data-pana-instance=\"counter-stale\">0</span>\n",
                "</section>\n",
                "{% endblock %}\n",
            ),
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let counter = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label.starts_with("<span") && node.label.contains("counter"))
            .unwrap();

        let plan = plan_html_duplicate(
            &model,
            &ProjectHtmlDuplicateIntent {
                source_source_id: Some(counter.id.clone()),
                source_location: None,
                source_tag: Some("span".to_string()),
                source_selector: Some(".counter".to_string()),
            },
            &HashMap::new(),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.unwrap();
        assert_eq!(patch.block_ids, vec!["counter".to_string()]);
        assert!(patch.html.contains("data-pana-block=\"counter\""));
        assert!(patch.html.contains("data-anim=\"ps-span-"));
        assert!(patch.html.contains("data-pana-instance=\"counter-span-"));
        assert!(!patch.html.contains("counter-stale"));
    }

    #[test]
    fn plan_html_duplicate_resolves_active_html_by_direct_location() {
        let root = unique_test_dir();
        write_project(&root, "<main></main>\n");
        fs::create_dir_all(root.join("static")).unwrap();
        fs::write(
            root.join("static/plain.html"),
            concat!(
                "<!DOCTYPE html>\n",
                "<html>\n",
                "<body>\n",
                "  <section id=\"hero\" class=\"panel ps-old\" data-anim=\"ps-old\">\n",
                "    <button id=\"cta\" aria-controls=\"hero\">A</button>\n",
                "  </section>\n",
                "</body>\n",
                "</html>\n",
            ),
        )
        .unwrap();
        let model = build_project_model(&root, &HashMap::new()).unwrap();

        let plan = plan_html_duplicate(
            &model,
            &ProjectHtmlDuplicateIntent {
                source_source_id: None,
                source_location: Some(ProjectSourceEditLocation {
                    file: "static/plain.html".to_string(),
                    line: 4,
                    column: 3,
                }),
                source_tag: Some("section".to_string()),
                source_selector: Some("body:nth-of-type(1) > section:nth-of-type(1)".to_string()),
            },
            &HashMap::new(),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.unwrap();
        assert_eq!(patch.file, "static/plain.html");
        assert_eq!(patch.resolved_source_id, "location:static/plain.html:4:3");
        assert_eq!(patch.tag, "section");
        assert!(patch.html.contains("id=\"hero-copy\""));
        assert!(patch.html.contains("id=\"cta-copy\""));
        assert!(patch.html.contains("aria-controls=\"hero-copy\""));
        assert!(patch.html.contains("class=\"panel ps-section-"));
        assert!(patch.contents.contains("\n  <section id=\"hero-copy\""));
    }

    #[test]
    fn plan_html_duplicate_blocks_missing_anchor() {
        let root = unique_test_dir();
        write_project(&root, "<section></section>\n");
        let model = build_project_model(&root, &HashMap::new()).unwrap();

        let plan = plan_html_duplicate(
            &model,
            &ProjectHtmlDuplicateIntent {
                source_source_id: Some("missing".to_string()),
                source_location: None,
                source_tag: Some("section".to_string()),
                source_selector: Some("section".to_string()),
            },
            &HashMap::new(),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan
            .diagnostic
            .unwrap()
            .contains("Nu am putut ancora sursă"));
    }

    fn write_project(root: &PathBuf, template: &str) {
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates")).unwrap();
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
        fs::write(root.join("templates/index.html"), template).unwrap();
    }

    fn unique_test_dir() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "pana-studio-duplicate-engine-{}-{stamp}",
            std::process::id()
        ))
    }
}
