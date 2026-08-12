use std::{
    collections::{BTreeSet, HashMap, HashSet},
    hash::{Hash, Hasher},
};

use serde::{Deserialize, Serialize};

use crate::{
    blocks::{
        inspect_native_icon_source, native_block_by_id, native_block_instance_id,
        node_is_slider_managed_scaffold, validate_native_block_slot_duplicate,
        NativeBlockSlotMutationContext,
    },
    kernel::dynamic_widgets::{
        generate_dynamic_widget_instance_id, render_dynamic_widget, DynamicWidgetSourceInstance,
    },
    project_model::model::{ProjectModel, ProjectModelFile, ProjectModelFileKind},
    source_graph::model::SourceNode,
};

use super::move_engine::{
    content_revision, insert_line_block, inserted_block_start_line, line_block_after_index,
    line_number_at_offset, parse_html_tag_at, resolve_html_node_for_anchor, same_model_path,
    source_location_at_offset, source_missing_message, ProjectSourceEditLocation, Span,
};
use super::structural_edit::{
    format_html_fragment, relocate_lossless_fragment, StructuralPlacement,
};
use super::structural_envelope::{structural_envelope_for_html_node, StructuralEnvelopeKind};
use super::zola_image_engine::{contains_zola_image_contract, zola_image_contract_start};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlDuplicateIntent {
    pub source_source_id: Option<String>,
    pub source_tag: Option<String>,
    #[serde(default)]
    pub native_block_slot: Option<NativeBlockSlotMutationContext>,
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
    /// Exact UTF-8 byte offset of the duplicated root in `contents`.
    /// Rust computes it while applying the structural edit; identity
    /// reconciliation never reconstructs it from line/column metadata.
    pub inserted_offset: usize,
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
    pub dynamic_widget_contract: bool,
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
    inserted_offset: usize,
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
) -> ProjectHtmlDuplicatePlan {
    match plan_html_duplicate_inner(model, intent) {
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
) -> Result<ProjectHtmlDuplicatePatch, String> {
    if let Some(context) = intent.native_block_slot.as_ref() {
        validate_native_block_slot_duplicate(model, context, intent.source_source_id.as_deref())?;
    }
    if let Some(source_node) = resolve_html_node_for_anchor(
        model,
        intent.source_source_id.as_deref(),
        intent.source_tag.as_deref(),
    ) {
        if intent.native_block_slot.is_none() && node_is_slider_managed_scaffold(model, source_node)
        {
            return Err(
                "Structura administrată Slider se duplică numai prin BlockPropertiesPane."
                    .to_string(),
            );
        }
        return plan_html_duplicate_from_source_node(model, source_node);
    }

    Err(source_missing_message(
        "sursă",
        intent.source_source_id.as_deref(),
    ))
}

fn plan_html_duplicate_from_source_node(
    model: &ProjectModel,
    source_node: &SourceNode,
) -> Result<ProjectHtmlDuplicatePatch, String> {
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
    let envelope = structural_envelope_for_html_node(model, &file.contents, source_node)?;
    // The generated body of a dynamic widget commonly contains its own Tera
    // condition. That condition is not an independently editable HTML node,
    // but the Rust-owned widget envelope is an atomic structural unit and may
    // be duplicated safely by rebuilding it with a fresh contract identity.
    if !source_node.capabilities.can_edit_visual && envelope.dynamic_widget.is_none() {
        return Err(source_node
            .capabilities
            .technical_reason()
            .map(str::to_string)
            .unwrap_or_else(|| "Elementul sursă nu este duplicabil vizual.".to_string()));
    }
    let mut source_span = envelope.span;
    if envelope.kind == StructuralEnvelopeKind::HtmlElement {
        if let Some(contract_start) = zola_image_contract_start(&file.contents, source_range.start)?
        {
            source_span.start = contract_start;
        }
    }

    plan_html_duplicate_for_span(
        model,
        file,
        &source_node.file,
        source_span,
        envelope.opening_start,
        envelope.dynamic_widget,
        source_node.id.clone(),
        source_node.label.clone(),
    )
}

// Span, semantic identity and optional widget evidence are distinct duplication preconditions.
#[allow(clippy::too_many_arguments)]
fn plan_html_duplicate_for_span(
    model: &ProjectModel,
    file: &ProjectModelFile,
    file_path: &str,
    source_span: Span,
    opening_start: usize,
    dynamic_widget: Option<&DynamicWidgetSourceInstance>,
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
    if tag == "svg" {
        validate_duplicated_icon_source(source_html)?;
    }
    let dynamic_widget_contract = dynamic_widget.is_some();
    let duplicate_source = if let Some(instance) = dynamic_widget {
        let properties = instance.properties.as_ref().ok_or_else(|| {
            format!(
                "Widgetul dinamic {} nu are proprietăți valide și nu poate fi duplicat.",
                instance.instance_id
            )
        })?;
        let instance_id = generate_dynamic_widget_instance_id(
            properties.provider_kind(),
            &format!("duplicate:{}:{}", model.revision, instance.instance_id),
            model
                .source_graph
                .dynamic_widget_graph
                .source_instances
                .iter()
                .map(|candidate| candidate.instance_id.as_str()),
        );
        render_dynamic_widget(&instance_id, properties, &model.source_graph)?
    } else {
        source_html.to_string()
    };
    let duplicate = prepare_duplicated_html(model, &tag, &duplicate_source);
    let zola_image_contract = contains_zola_image_contract(source_html);
    let applied = apply_html_duplicate_after(
        &file.contents,
        file_path,
        source_span,
        &duplicate.html,
        dynamic_widget_contract,
    )?;

    Ok(ProjectHtmlDuplicatePatch {
        file: file_path.to_string(),
        resolved_source_id,
        duplicated_label,
        before_revision: file.revision.clone(),
        after_revision: content_revision(&applied.contents),
        contents: applied.contents,
        source_location: source_location_at_offset(&file.contents, file_path, source_span.start),
        inserted_location: applied.inserted_location,
        inserted_offset: applied.inserted_offset,
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
        dynamic_widget_contract,
    })
}

fn validate_duplicated_icon_source(source: &str) -> Result<(), String> {
    let opening = parse_html_tag_at(source, 0)
        .ok_or_else(|| "Block-ul Icon nu mai are o rădăcină SVG stabilă.".to_string())?;
    let opening_source = source
        .get(opening.start..opening.end)
        .ok_or_else(|| "Rădăcina block-ului Icon nu poate fi citită.".to_string())?;
    let Some(state) = inspect_native_icon_source(opening_source)? else {
        return Ok(());
    };
    let closing = source
        .to_ascii_lowercase()
        .rfind("</svg>")
        .ok_or_else(|| "Block-ul Icon nu mai are închiderea </svg>.".to_string())?;
    let children = source
        .get(opening.end..closing)
        .ok_or_else(|| "Geometria block-ului Icon are un range invalid.".to_string())?;
    let expected = crate::blocks::icons::render_icon_children_by_identity(&state.icon_identity)?;
    if children.trim() != expected {
        return Err(
            "Block-ul Icon conține geometrie care nu aparține registrului Rust și nu poate fi duplicat."
                .to_string(),
        );
    }
    Ok(())
}

fn apply_html_duplicate_after(
    source: &str,
    file: &str,
    source_span: Span,
    snippet: &str,
    preserve_internal_indentation: bool,
) -> Result<DuplicateApplication, String> {
    let placement = StructuralPlacement::for_direct_target(source, source_span.start);
    let target_indent = placement.indent.as_str();
    let inserted = if preserve_internal_indentation {
        relocate_lossless_fragment(snippet, "", target_indent, &placement.style)?
    } else {
        format_html_fragment(snippet, target_indent, &placement.style)?
    };
    let source_start_line = line_number_at_offset(source, source_span.start);
    let source_end_line = line_number_at_offset(source, source_span.end);
    let insert_at = line_block_after_index(source, source_span.end);
    let inserted_start_line = inserted_block_start_line(source, insert_at);
    let leading_break_bytes =
        usize::from(insert_at > 0 && source.as_bytes().get(insert_at - 1) != Some(&b'\n'))
            * placement.style.line_ending().len();
    let inserted_offset = insert_at + leading_break_bytes + target_indent.len();
    Ok(DuplicateApplication {
        contents: insert_line_block(source, insert_at, &inserted),
        inserted_location: ProjectSourceEditLocation {
            file: file.to_string(),
            line: inserted_start_line,
            column: target_indent.chars().count() + 1,
        },
        inserted_offset,
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
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        kernel::dynamic_widgets::{
            DynamicFieldEmptyBehavior, DynamicFieldPresentation, DynamicFieldScope,
            DynamicFieldWidgetProperties, DynamicValueBinding, DynamicValueFormat,
            DynamicValueSource, DynamicValueType, DynamicWidgetProperties,
        },
        project_model::test_support::ProjectModelTestFixture,
    };

    use super::*;

    #[test]
    fn plan_html_duplicate_rewrites_html_identity_and_references() {
        let root = unique_test_dir();
        let fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            concat!(
                "{% block content %}\n",
                "<section class=\"hero\">\n",
                "  <div id=\"card\" class=\"card ps-card-old\" data-anim=\"ps-card-old\" data-pana-source-id=\"stale\">\n",
                "    <button id=\"cta\" aria-controls=\"card\" data-target=\"#card\">A</button>\n",
                "  </div>\n",
                "</section>\n",
                "{% endblock %}\n",
            ),
        )
        .unwrap();
        let model = fixture.build_model().unwrap();
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
                source_tag: Some("div".to_string()),
                native_block_slot: None,
            },
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
        let fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            concat!(
                "{% block content %}\n",
                "<section>\n",
                "  <span class=\"counter ps-counter-old\" data-anim=\"ps-counter-old\" data-pana-block=\"counter\" data-pana-instance=\"counter-stale\">0</span>\n",
                "</section>\n",
                "{% endblock %}\n",
            ),
        )
        .unwrap();
        let model = fixture.build_model().unwrap();
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
                source_tag: Some("span".to_string()),
                native_block_slot: None,
            },
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
    fn plan_html_duplicate_keeps_icon_geometry_and_refreshes_root_identity() {
        let root = unique_test_dir();
        let icon = crate::blocks::icons::render_icon_block_html(
            "star",
            "ps-icon-old custom-icon",
            "ps-icon-old",
            "icon-stale",
        )
        .unwrap();
        let fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            format!("<main>\n  {icon}\n</main>\n"),
        )
        .unwrap();
        let model = fixture.build_model().unwrap();
        let marker = model
            .source_graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == crate::source_graph::model::SourceNodeKind::BlockMarker
                    && node.label == "icon"
            })
            .expect("icon marker");
        let icon_root = model
            .source_graph
            .nodes
            .iter()
            .find(|node| marker.parent.as_deref() == Some(node.id.as_str()))
            .expect("icon root");

        let plan = plan_html_duplicate(
            &model,
            &ProjectHtmlDuplicateIntent {
                source_source_id: Some(icon_root.id.clone()),
                source_tag: Some("svg".to_string()),
                native_block_slot: None,
            },
        );
        fs::remove_dir_all(&root).unwrap();

        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.expect("icon duplicate patch");
        assert_eq!(patch.block_ids, vec!["icon".to_string()]);
        assert!(patch
            .html
            .contains("data-pana-icon=\"tabler-outline:star\""));
        assert!(patch.html.contains("class=\"icon ps-svg-"));
        assert!(patch.html.contains("custom-icon"));
        assert!(patch.html.contains("data-anim=\"ps-svg-"));
        assert!(patch.html.contains("data-pana-instance=\"icon-svg-"));
        assert!(!patch.html.contains("icon-stale"));
        assert_eq!(
            patch.html.matches("<path ").count(),
            icon.matches("<path ").count()
        );
    }

    #[test]
    fn plan_html_duplicate_rejects_arbitrary_icon_geometry() {
        let root = unique_test_dir();
        let icon = crate::blocks::icons::render_icon_block_html(
            "home",
            "ps-icon-old",
            "ps-icon-old",
            "icon-old",
        )
        .unwrap()
        .replace("<path ", "<path onload=\"alert(1)\" ");
        let fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            format!("<main>\n  {icon}\n</main>\n"),
        )
        .unwrap();
        let model = fixture.build_model().unwrap();
        let marker = model
            .source_graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == crate::source_graph::model::SourceNodeKind::BlockMarker
                    && node.label == "icon"
            })
            .expect("icon marker");
        let icon_root = model
            .source_graph
            .nodes
            .iter()
            .find(|node| marker.parent.as_deref() == Some(node.id.as_str()))
            .expect("icon root");
        let plan = plan_html_duplicate(
            &model,
            &ProjectHtmlDuplicateIntent {
                source_source_id: Some(icon_root.id.clone()),
                source_tag: Some("svg".to_string()),
                native_block_slot: None,
            },
        );
        fs::remove_dir_all(&root).unwrap();

        assert!(!plan.allowed);
        assert!(plan.patch.is_none());
        assert!(plan
            .diagnostic
            .as_deref()
            .is_some_and(|message| message.contains("registrului Rust")));
    }

    #[test]
    fn plan_html_duplicate_rebuilds_a_dynamic_widget_with_a_fresh_contract_identity() {
        let root = unique_test_dir();
        let mut fixture =
            ProjectModelTestFixture::standard_zola(root.clone(), "<main></main>\n").unwrap();
        let base_model = fixture.build_model().unwrap();
        let properties = DynamicWidgetProperties::DynamicField(DynamicFieldWidgetProperties {
            binding: DynamicValueBinding {
                context: DynamicFieldScope::Section,
                source: DynamicValueSource::Builtin {
                    field: "title".to_string(),
                },
                value_type: DynamicValueType::Text,
            },
            presentation: DynamicFieldPresentation::Heading,
            tag: "h2".to_string(),
            format: DynamicValueFormat::default(),
            prefix: String::new(),
            suffix: String::new(),
            fallback: String::new(),
            label: "Titlu".to_string(),
            empty_behavior: DynamicFieldEmptyBehavior::RenderEmpty,
        });
        let widget = render_dynamic_widget(
            "dynamic-field-duplicate01",
            &properties,
            &base_model.source_graph,
        )
        .unwrap();
        fixture.source(
            "templates/index.html",
            format!("<main>\n  {}\n</main>\n", widget.replace('\n', "\n  ")),
        );
        let model = fixture.build_model().unwrap();
        let heading = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label.starts_with("<h2"))
            .unwrap();

        let plan = plan_html_duplicate(
            &model,
            &ProjectHtmlDuplicateIntent {
                source_source_id: Some(heading.id.clone()),
                source_tag: Some("h2".to_string()),
                native_block_slot: None,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.unwrap();
        assert!(patch.dynamic_widget_contract);
        assert!(!patch.html.contains("dynamic-field-duplicate01"));
        assert_eq!(
            patch.contents.matches("dynamic-field-duplicate01").count(),
            4
        );
        assert_eq!(patch.contents.matches("{# pana:widget schema=2").count(), 2);
        assert_eq!(
            patch.contents.matches("data-pana-widget-instance=").count(),
            4
        );
    }

    #[test]
    fn plan_html_duplicate_rejects_location_without_source_id() {
        let root = unique_test_dir();
        let mut fixture =
            ProjectModelTestFixture::standard_zola(root.clone(), "<main></main>\n").unwrap();
        fixture.source(
            "static/plain.html",
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
        );
        let model = fixture.build_model().unwrap();

        let plan = plan_html_duplicate(
            &model,
            &ProjectHtmlDuplicateIntent {
                source_source_id: None,
                source_tag: Some("section".to_string()),
                native_block_slot: None,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.patch.is_none());
    }

    #[test]
    fn plan_html_duplicate_blocks_missing_anchor() {
        let root = unique_test_dir();
        let fixture =
            ProjectModelTestFixture::standard_zola(root.clone(), "<section></section>\n").unwrap();
        let model = fixture.build_model().unwrap();

        let plan = plan_html_duplicate(
            &model,
            &ProjectHtmlDuplicateIntent {
                source_source_id: Some("missing".to_string()),
                source_tag: Some("section".to_string()),
                native_block_slot: None,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan
            .diagnostic
            .unwrap()
            .contains("Nu am putut ancora sursă"));
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
