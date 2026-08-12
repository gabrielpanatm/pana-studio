use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
};

use serde::{Deserialize, Serialize};

use super::structural_edit::{
    format_html_fragment, indent_at, normalize_html_subtree, relocate_lossless_fragment,
    StructuralIndentationStyle, StructuralPlacement,
};
use super::structural_envelope::{structural_envelope_for_html_node, StructuralEnvelopeKind};
use super::zola_image_engine::zola_image_contract_start;
use crate::{
    blocks::{
        node_has_native_block_ancestor, node_is_native_block, node_is_slider_managed_scaffold,
        node_is_slider_slot_item, node_subtree_contains_native_block,
        validate_native_block_slot_move, NativeBlockSlotMutationContext,
    },
    project_model::model::{ProjectModel, ProjectModelFileKind},
    source_graph::model::{SourceNode, SourceNodeKind},
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlMoveIntent {
    pub source_source_id: Option<String>,
    pub target_source_id: Option<String>,
    pub source_tag: Option<String>,
    pub target_tag: Option<String>,
    pub position: ProjectMovePosition,
    #[serde(default)]
    pub native_block_slot: Option<NativeBlockSlotMutationContext>,
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
    pub target_label: String,
    pub position: ProjectMovePosition,
    pub before_revision: String,
    pub after_revision: String,
    pub contents: String,
    pub source_location: ProjectSourceEditLocation,
    pub target_location: ProjectSourceEditLocation,
    pub source_start_line: usize,
    pub source_end_line: usize,
    pub new_start_line: usize,
    pub target_start_line: usize,
}

#[derive(Clone, Debug)]
pub struct ProjectHtmlBatchMovePatch {
    pub file: String,
    pub resolved_source_ids: Vec<String>,
    pub resolved_target_id: String,
    pub contents: String,
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
    target_start_line: usize,
}

pub(crate) struct HtmlTag {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) tag: String,
    pub(crate) is_closing: bool,
    pub(crate) is_self_closing: bool,
}

const VOID_TAGS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

const CONTAINER_TAGS: &[&str] = &[
    "main", "section", "article", "header", "footer", "nav", "aside", "div", "ul", "ol", "li",
    "form", "fieldset",
];

pub fn plan_html_move(model: &ProjectModel, intent: &ProjectHtmlMoveIntent) -> ProjectHtmlMovePlan {
    plan_html_move_with_authority(model, intent, false)
}

/// Plans one lossless permutation of HTML sibling subtrees. Every member is
/// preflighted through the normal move engine, but source text is rewritten
/// once and ProjectModel is rebuilt only by the caller's single transaction.
pub fn plan_html_batch_move(
    model: &ProjectModel,
    source_ids: &[String],
    target_source_id: &str,
    target_tag: Option<&str>,
    position: ProjectMovePosition,
) -> Result<ProjectHtmlBatchMovePatch, String> {
    if source_ids.len() < 2 || source_ids.len() > 256 {
        return Err("Mutarea batch cere între 2 și 256 de elemente.".to_string());
    }
    if position == ProjectMovePosition::Inside {
        return Err(
            "Mutarea batch v1 acceptă numai pozițiile before/after între frați.".to_string(),
        );
    }
    let selected = source_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    if selected.len() != source_ids.len() || selected.contains(target_source_id) {
        return Err(
            "Mutarea batch a refuzat identități duplicate sau o țintă selectată.".to_string(),
        );
    }
    let target = resolve_html_node_for_anchor(model, Some(target_source_id), target_tag)
        .ok_or_else(|| source_missing_message("destinație", Some(target_source_id)))?;
    let parent_id = target
        .parent
        .as_deref()
        .ok_or_else(|| "Mutarea batch a refuzat o destinație fără părinte.".to_string())?;
    let parent = model
        .source_graph
        .node_by_id(parent_id)
        .ok_or_else(|| "Mutarea batch nu găsește părintele comun.".to_string())?;
    let file = model
        .files
        .iter()
        .find(|file| same_model_path(&file.relative_path, &target.file))
        .ok_or_else(|| format!("Nu am găsit fișierul {} în Project Model.", target.file))?;
    if file.kind != ProjectModelFileKind::Template {
        return Err("Mutarea batch este activă numai pentru template-uri Zola/Tera.".to_string());
    }

    let ordered_ids = parent
        .children
        .iter()
        .filter(|id| selected.contains(id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if ordered_ids.len() != source_ids.len() {
        return Err("Elementele mutate nu sunt toate copiii aceluiași părinte.".to_string());
    }
    for source_id in &ordered_ids {
        let source = model
            .source_graph
            .node_by_id(source_id)
            .ok_or_else(|| source_missing_message("sursă", Some(source_id)))?;
        if source.file != target.file || source.parent.as_deref() != Some(parent_id) {
            return Err("Mutarea batch v1 cere același document și același părinte.".to_string());
        }
        let preflight = plan_html_move(
            model,
            &ProjectHtmlMoveIntent {
                source_source_id: Some(source_id.clone()),
                target_source_id: Some(target_source_id.to_string()),
                source_tag: None,
                target_tag: target_tag.map(str::to_string),
                position,
                native_block_slot: None,
            },
        );
        if !preflight.allowed {
            return Err(preflight
                .diagnostic
                .unwrap_or_else(|| "Move Engine a blocat un membru al selecției.".to_string()));
        }
    }

    let target_index = parent
        .children
        .iter()
        .position(|id| id == target_source_id)
        .ok_or_else(|| "Destinația nu aparține părintelui comun.".to_string())?;
    let mut desired = parent
        .children
        .iter()
        .filter(|id| !selected.contains(id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let target_after_removal = desired
        .iter()
        .position(|id| id == target_source_id)
        .ok_or_else(|| "Destinația a dispărut din ordinea fraților.".to_string())?;
    let insertion_index =
        target_after_removal + usize::from(position == ProjectMovePosition::After);
    desired.splice(
        insertion_index..insertion_index,
        ordered_ids.iter().cloned(),
    );
    if desired == parent.children {
        return Err("Elementele selectate sunt deja în poziția cerută.".to_string());
    }

    let mut spans = ordered_ids
        .iter()
        .map(|source_id| {
            let node = model.source_graph.node_by_id(source_id).unwrap();
            let envelope = structural_envelope_for_html_node(model, &file.contents, node)?;
            if envelope.kind != StructuralEnvelopeKind::HtmlElement
                || zola_image_contract_start(
                    &file.contents,
                    node.range
                        .as_ref()
                        .map(|range| range.start)
                        .unwrap_or(envelope.span.start),
                )?
                .is_some()
            {
                return Err(
                    "Mutarea batch v1 a refuzat un contract structural specializat.".to_string(),
                );
            }
            let removal = removal_range_for_span(&file.contents, envelope.span);
            Ok((removal.start, removal.end, source_id.clone()))
        })
        .collect::<Result<Vec<_>, String>>()?;
    spans.sort_by_key(|(start, _, _)| *start);
    if spans.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err("Mutarea batch a refuzat subarbori sursă suprapuși.".to_string());
    }
    let target_envelope = structural_envelope_for_html_node(model, &file.contents, target)?;
    if target_envelope.kind != StructuralEnvelopeKind::HtmlElement {
        return Err(
            "Mutarea batch v1 a refuzat o destinație structurală specializată.".to_string(),
        );
    }
    let target_removal = removal_range_for_span(&file.contents, target_envelope.span);
    let insertion_original = if position == ProjectMovePosition::Before {
        target_removal.start
    } else {
        target_removal.end
    };
    let fragments = spans
        .iter()
        .map(|(start, end, _)| {
            file.contents
                .get(*start..*end)
                .ok_or_else(|| "Mutarea batch a calculat un fragment UTF-8 invalid.".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?
        .concat();
    let removed_before = spans
        .iter()
        .filter(|(_, end, _)| *end <= insertion_original)
        .map(|(start, end, _)| end - start)
        .sum::<usize>();
    let insertion = insertion_original
        .checked_sub(removed_before)
        .ok_or_else(|| "Mutarea batch a calculat un offset invalid.".to_string())?;
    let mut contents = file.contents.clone();
    for (start, end, _) in spans.iter().rev() {
        contents.replace_range(*start..*end, "");
    }
    contents.insert_str(insertion, &fragments);
    if contents == file.contents {
        return Err(format!(
            "Mutarea batch nu a schimbat ordinea fraților (ținta inițială {target_index})."
        ));
    }
    Ok(ProjectHtmlBatchMovePatch {
        file: target.file.clone(),
        resolved_source_ids: ordered_ids,
        resolved_target_id: target.id.clone(),
        contents,
    })
}

/// Plans an HTML move after the caller has validated a Rust-issued
/// `EditScopeGrant` for the exact Tera boundary containing both anchors.
///
/// The unscoped planner keeps the conservative SourceGraph capability gate,
/// while an exact Rust-issued grant may authorize only the otherwise-safe
/// HTML-in-Tera reasons below.
pub fn plan_html_move_in_edit_scope(
    model: &ProjectModel,
    intent: &ProjectHtmlMoveIntent,
) -> ProjectHtmlMovePlan {
    plan_html_move_with_authority(model, intent, true)
}

fn plan_html_move_with_authority(
    model: &ProjectModel,
    intent: &ProjectHtmlMoveIntent,
    edit_scope_authorized: bool,
) -> ProjectHtmlMovePlan {
    match plan_html_move_inner(model, intent, edit_scope_authorized) {
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

fn plan_html_move_inner(
    model: &ProjectModel,
    intent: &ProjectHtmlMoveIntent,
    edit_scope_authorized: bool,
) -> Result<ProjectHtmlMovePatch, String> {
    if let Some(context) = intent.native_block_slot.as_ref() {
        validate_native_block_slot_move(
            model,
            context,
            intent.source_source_id.as_deref(),
            intent.target_source_id.as_deref(),
            intent.position,
        )?;
    }
    let source_node = resolve_html_node_for_anchor(
        model,
        intent.source_source_id.as_deref(),
        intent.source_tag.as_deref(),
    )
    .ok_or_else(|| source_missing_message("sursă", intent.source_source_id.as_deref()))?;
    let target_node = resolve_html_node_for_anchor(
        model,
        intent.target_source_id.as_deref(),
        intent.target_tag.as_deref(),
    )
    .ok_or_else(|| source_missing_message("destinație", intent.target_source_id.as_deref()))?;

    if source_node.id == target_node.id {
        return Err("Elementul este deja pe această țintă.".to_string());
    }
    if intent.native_block_slot.is_none()
        && (node_is_slider_managed_scaffold(model, source_node)
            || (node_is_slider_managed_scaffold(model, target_node)
                && !(intent.position == ProjectMovePosition::Inside
                    && node_is_slider_slot_item(model, target_node))))
    {
        return Err(
            "Structura administrată Slider se reordonează numai prin BlockPropertiesPane."
                .to_string(),
        );
    }
    if node_subtree_contains_native_block(model, source_node, "slider")
        && (node_has_native_block_ancestor(model, target_node, "slider")
            || (intent.position == ProjectMovePosition::Inside
                && node_is_native_block(model, target_node, "slider")))
    {
        return Err("Slider în slider este blocat de contractul Rust v1.".to_string());
    }
    if !same_model_path(&source_node.file, &target_node.file) {
        return Err(
            "Mutarea între template-uri diferite rămâne blocată până există plan de impact."
                .to_string(),
        );
    }
    if !html_move_capability_allowed(source_node, edit_scope_authorized) {
        return Err(source_node
            .capabilities
            .technical_reason()
            .map(str::to_string)
            .unwrap_or_else(|| "Elementul sursă nu este mutabil vizual.".to_string()));
    }
    if !html_move_capability_allowed(target_node, edit_scope_authorized) {
        return Err(target_node
            .capabilities
            .technical_reason()
            .map(str::to_string)
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
    let source_envelope = structural_envelope_for_html_node(model, &file.contents, source_node)?;
    let target_envelope = structural_envelope_for_html_node(model, &file.contents, target_node)?;
    if intent.position == ProjectMovePosition::Inside
        && target_envelope.kind == StructuralEnvelopeKind::DynamicWidget
    {
        return Err(
            "Corpul unui widget dinamic este generat de contractul său. Mută widgetul ca unitate sau editează-i proprietățile; nu insera copii direct în corpul generat."
                .to_string(),
        );
    }
    if source_envelope
        .dynamic_widget
        .zip(target_envelope.dynamic_widget)
        .is_some_and(|(source, target)| source.instance_id == target.instance_id)
    {
        return Err("Widgetul dinamic nu poate fi mutat în propriul contract.".to_string());
    }

    let mut source_span = source_envelope.span;
    if source_envelope.kind == StructuralEnvelopeKind::HtmlElement {
        if let Some(contract_start) = zola_image_contract_start(&file.contents, source_range.start)?
        {
            source_span.start = contract_start;
        }
    }
    let mut target_span = if intent.position == ProjectMovePosition::Inside {
        resolve_html_element_span(&file.contents, target_range.start)?
    } else {
        target_envelope.span
    };
    if intent.position == ProjectMovePosition::Before
        && target_envelope.kind == StructuralEnvelopeKind::HtmlElement
    {
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
    let placement = StructuralPlacement::for_html_target(model, &file.contents, target_node);
    let applied = apply_html_move(
        &file.contents,
        source_span,
        target_span,
        &target_tag,
        intent.position,
        &placement,
        source_envelope.preserves_internal_indentation(),
    )?;

    Ok(ProjectHtmlMovePatch {
        file: source_node.file.clone(),
        resolved_source_id: source_node.id.clone(),
        resolved_target_id: target_node.id.clone(),
        source_label: source_node.label.clone(),
        target_label: target_node.label.clone(),
        position: intent.position,
        before_revision: file.revision.clone(),
        after_revision: content_revision(&applied.contents),
        contents: applied.contents,
        source_location,
        target_location,
        source_start_line: applied.source_start_line,
        source_end_line: applied.source_end_line,
        new_start_line: applied.new_start_line,
        target_start_line: applied.target_start_line,
    })
}

fn html_move_capability_allowed(node: &SourceNode, edit_scope_authorized: bool) -> bool {
    node.capabilities.can_move
        || (edit_scope_authorized
            && node.kind == SourceNodeKind::Html
            && matches!(
                node.capabilities.reason_code,
                Some(
                    crate::source_graph::model::SourceCapabilityReason::HtmlInTeraLoop
                        | crate::source_graph::model::SourceCapabilityReason::HtmlInTeraCondition
                        | crate::source_graph::model::SourceCapabilityReason::HtmlInTeraMacro
                        | crate::source_graph::model::SourceCapabilityReason::HtmlInTeraLocalScope
                )
            ))
}

fn resolve_html_node<'a>(model: &'a ProjectModel, source_id: &str) -> Option<&'a SourceNode> {
    model
        .source_graph
        .node_by_id(source_id)
        .filter(|node| node.kind == SourceNodeKind::Html)
}

pub(super) fn resolve_html_node_for_anchor<'a>(
    model: &'a ProjectModel,
    source_id: Option<&str>,
    tag: Option<&str>,
) -> Option<&'a SourceNode> {
    source_id
        .and_then(|id| resolve_html_node(model, id))
        .filter(|node| node_tag_matches(node, tag))
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

pub(super) fn source_missing_message(kind: &str, source_id: Option<&str>) -> String {
    let id = source_id.unwrap_or("fără Source ID");
    format!("Nu am putut ancora {kind} în Project Model. SourceNodeId: {id}.")
}

fn apply_html_move(
    source: &str,
    source_span: Span,
    target_span: Span,
    target_tag: &str,
    position: ProjectMovePosition,
    placement: &StructuralPlacement,
    preserve_internal_indentation: bool,
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
    let source_indent = indent_at(source, source_span.start);
    let target_indent = placement.indent.as_str();
    let source_start_line = line_number_at_offset(source, source_span.start);
    let source_end_line = line_number_at_offset(source, source_span.end);

    match position {
        ProjectMovePosition::Before => {
            let moving = format_structural_fragment(
                &moving_source,
                &source_indent,
                target_indent,
                placement,
                preserve_internal_indentation,
            )?;
            let insert_at = line_block_before_index(&without_source, adjusted_target_start);
            let new_start_line = inserted_block_start_line(&without_source, insert_at);
            let contents = insert_line_block(&without_source, insert_at, &moving);
            let inserted_length = contents.len().saturating_sub(without_source.len());
            let target_offset = adjusted_target_start.saturating_add(inserted_length);
            Ok(MoveApplication {
                target_start_line: line_number_at_offset(&contents, target_offset),
                contents,
                source_start_line,
                source_end_line,
                new_start_line,
            })
        }
        ProjectMovePosition::After => {
            let moving = format_structural_fragment(
                &moving_source,
                &source_indent,
                target_indent,
                placement,
                preserve_internal_indentation,
            )?;
            let insert_at = line_block_after_index(&without_source, adjusted_target_end);
            let new_start_line = inserted_block_start_line(&without_source, insert_at);
            let contents = insert_line_block(&without_source, insert_at, &moving);
            Ok(MoveApplication {
                target_start_line: line_number_at_offset(&contents, adjusted_target_start),
                contents,
                source_start_line,
                source_end_line,
                new_start_line,
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
            let child_indent = placement.child_indent();
            let moving = format_structural_fragment(
                &moving_source,
                &source_indent,
                &child_indent,
                placement,
                preserve_internal_indentation,
            )?;
            let insert_at = adjusted_target_start + close_offset;
            let before_insert = inside_prefix_for_insert(&without_source, opening.end, insert_at);
            let moved_start_offset = before_insert.len() + placement.style.line_ending().len();
            let next_contents = format!(
                "{}{}{}{}{}{}",
                before_insert,
                placement.style.line_ending(),
                moving,
                placement.style.line_ending(),
                target_indent,
                &without_source[insert_at..]
            );
            let contents = normalize_html_subtree(
                &next_contents,
                adjusted_target_start,
                target_indent,
                &placement.style,
            )?;
            let new_start_line = line_number_at_offset(&next_contents, moved_start_offset);
            Ok(MoveApplication {
                target_start_line: line_number_at_offset(&contents, adjusted_target_start),
                contents,
                source_start_line,
                source_end_line,
                new_start_line,
            })
        }
    }
}

fn format_structural_fragment(
    source: &str,
    source_indent: &str,
    target_indent: &str,
    placement: &StructuralPlacement,
    preserve_internal_indentation: bool,
) -> Result<String, String> {
    if preserve_internal_indentation {
        relocate_lossless_fragment(source, source_indent, target_indent, &placement.style)
    } else {
        format_html_fragment(source, target_indent, &placement.style)
    }
}

pub(super) fn line_block_before_index(source: &str, opening_start: usize) -> usize {
    let opening_start = opening_start.min(source.len());
    let line_start = source
        .get(..opening_start)
        .and_then(|prefix| prefix.rfind('\n').map(|index| index + 1))
        .unwrap_or(0);
    if source
        .get(line_start..opening_start)
        .is_some_and(|prefix| prefix.trim().is_empty())
    {
        line_start
    } else {
        opening_start
    }
}

pub(super) fn line_block_after_index(source: &str, span_end: usize) -> usize {
    let span_end = span_end.min(source.len());
    let Some(relative_line_end) = source.get(span_end..).and_then(|suffix| suffix.find('\n'))
    else {
        return span_end;
    };
    let line_end = span_end + relative_line_end;
    if source
        .get(span_end..line_end)
        .is_some_and(|suffix| suffix.trim().is_empty())
    {
        line_end + 1
    } else {
        span_end
    }
}

pub(super) fn insert_line_block(source: &str, index: usize, block: &str) -> String {
    let index = index.min(source.len());
    let style = StructuralIndentationStyle::detect(source);
    let line_ending = style.line_ending();
    let needs_leading_break = index > 0 && source.as_bytes().get(index - 1) != Some(&b'\n');
    let needs_trailing_break = index < source.len() && source.as_bytes().get(index) != Some(&b'\n');
    format!(
        "{}{}{}{}{}",
        &source[..index],
        if needs_leading_break { line_ending } else { "" },
        block.trim_matches(|character| character == '\n' || character == '\r'),
        if needs_trailing_break {
            line_ending
        } else {
            ""
        },
        &source[index..]
    )
}

pub(super) struct DocumentFragmentAppend {
    pub(super) contents: String,
    pub(super) inserted_start_line: usize,
    pub(super) line_shift: isize,
    pub(super) insertion_offset: usize,
    pub(super) inserted_length: usize,
    pub(super) inserted_offset: usize,
}

/// Appends one top-level structural unit before the source's trailing
/// whitespace. This is the shared persistence rule for a directly opened
/// Template/Partial root: no synthetic Canvas wrapper is ever serialized, and
/// a second drop follows the first one instead of targeting generated DOM.
pub(super) fn append_document_fragment(source: &str, fragment: &str) -> DocumentFragmentAppend {
    let insert_at = source.trim_end_matches(char::is_whitespace).len();
    let style = StructuralIndentationStyle::detect(source);
    let needs_leading_break = insert_at > 0 && source.as_bytes().get(insert_at - 1) != Some(&b'\n');
    let inserted_offset = insert_at + usize::from(needs_leading_break) * style.line_ending().len();
    let contents = insert_line_block(source, insert_at, fragment);
    let inserted_length = contents.len().saturating_sub(source.len());
    let inserted_start_line = inserted_block_start_line(source, insert_at);
    let before_lines = source.bytes().filter(|byte| *byte == b'\n').count() as isize;
    let after_lines = contents.bytes().filter(|byte| *byte == b'\n').count() as isize;
    DocumentFragmentAppend {
        contents,
        inserted_start_line,
        line_shift: after_lines - before_lines,
        insertion_offset: insert_at,
        inserted_length,
        inserted_offset,
    }
}

pub(super) fn inserted_block_start_line(source: &str, index: usize) -> usize {
    line_number_at_offset(source, index)
        + usize::from(index > 0 && source.as_bytes().get(index - 1) != Some(&b'\n'))
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
        let close_line_start = before_insert
            .rfind('\n')
            .map(|index| index + 1)
            .unwrap_or(0);
        let close_prefix = before_insert.get(close_line_start..).unwrap_or("");
        if close_prefix
            .bytes()
            .all(|byte| matches!(byte, b' ' | b'\t'))
        {
            let content_end = if close_line_start >= 2
                && before_insert
                    .as_bytes()
                    .get(close_line_start - 2..close_line_start)
                    .is_some_and(|ending| ending == b"\r\n")
            {
                close_line_start - 2
            } else {
                close_line_start.saturating_sub(1)
            };
            before_insert[..content_end].to_string()
        } else {
            before_insert.to_string()
        }
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

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::project_model::test_support::ProjectModelTestFixture;

    use super::*;

    #[test]
    fn html_batch_move_permutates_siblings_once_and_preserves_indentation() {
        let root = unique_test_dir();
        let fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            concat!(
                "<main>\n",
                "  <p class=\"a\">A</p>\n",
                "  <p class=\"x\">X</p>\n",
                "  <p class=\"b\">B</p>\n",
                "  <p class=\"target\">T</p>\n",
                "</main>\n",
            ),
        )
        .unwrap();
        let model = fixture.build_model().unwrap();
        let template = model
            .files
            .iter()
            .find(|file| file.relative_path.ends_with("templates/index.html"))
            .unwrap();
        let source_id =
            |class_name: &str| {
                model
                    .source_graph
                    .nodes
                    .iter()
                    .filter(|node| {
                        node.kind == SourceNodeKind::Html
                            && node.range.as_ref().is_some_and(|range| {
                                template.contents.get(range.start..range.end).is_some_and(
                                    |source| source.contains(&format!("class=\"{class_name}\"")),
                                )
                            })
                    })
                    .min_by_key(|node| {
                        node.range
                            .as_ref()
                            .map(|range| range.end.saturating_sub(range.start))
                            .unwrap_or(usize::MAX)
                    })
                    .unwrap()
                    .id
                    .clone()
            };
        let patch = plan_html_batch_move(
            &model,
            &[source_id("a"), source_id("b")],
            &source_id("target"),
            Some("p"),
            ProjectMovePosition::Before,
        )
        .unwrap();

        assert!(patch.contents.contains(concat!(
            "  <p class=\"x\">X</p>\n",
            "  <p class=\"a\">A</p>\n",
            "  <p class=\"b\">B</p>\n",
            "  <p class=\"target\">T</p>\n",
        )));
        assert_eq!(patch.contents.matches("class=\"a\"").count(), 1);
        assert_eq!(patch.contents.matches("class=\"b\"").count(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn html_move_refuses_cross_source_transfer_from_partial_to_page() {
        let root = unique_test_dir();
        let fixture = project_fixture(root.clone());
        let projection_before = fixture.projection();
        let model = fixture.build_model().unwrap();
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
        let plan = plan_html_move(
            &model,
            &ProjectHtmlMoveIntent {
                source_source_id: Some(source.id.clone()),
                target_source_id: Some(target.id.clone()),
                source_tag: Some("article".to_string()),
                target_tag: Some("section".to_string()),
                position: ProjectMovePosition::Before,
                native_block_slot: None,
            },
        );

        assert!(!plan.allowed);
        assert!(plan.patch.is_none());
        assert!(plan
            .diagnostic
            .as_deref()
            .is_some_and(|message| message.contains("template-uri diferite")));
        assert_eq!(fixture.projection(), projection_before);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn html_move_inside_tera_requires_the_explicit_scoped_planner() {
        let root = unique_test_dir();
        let fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            concat!(
                "{% for item in section.pages %}\n",
                "  <section class=\"grid\"></section>\n",
                "  <article class=\"card\">{{ item.title }}</article>\n",
                "{% endfor %}\n",
            ),
        )
        .unwrap();
        let model = fixture.build_model().unwrap();
        let source = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::Html && node.label.starts_with("<article"))
            .unwrap();
        let target = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::Html && node.label.starts_with("<section"))
            .unwrap();
        assert_eq!(
            source.capabilities.reason_code,
            Some(crate::source_graph::model::SourceCapabilityReason::HtmlInTeraLoop)
        );
        let intent = ProjectHtmlMoveIntent {
            source_source_id: Some(source.id.clone()),
            target_source_id: Some(target.id.clone()),
            source_tag: Some("article".to_string()),
            target_tag: Some("section".to_string()),
            position: ProjectMovePosition::Before,
            native_block_slot: None,
        };

        let unscoped = plan_html_move(&model, &intent);
        let scoped = plan_html_move_in_edit_scope(&model, &intent);

        assert!(!unscoped.allowed);
        assert!(scoped.allowed, "{:?}", scoped.diagnostic);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn html_move_keeps_a_dynamic_widget_contract_atomic_and_repairs_indentation() {
        let root = unique_test_dir();
        let fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            concat!(
                "<main>\n",
                "  <div>\n",
                "        {# pana:widget schema=2 provider=dynamic-field instance=dynamic-field-atomic01 props=00 #}\n",
                "<h2 data-pana-widget-instance=\"dynamic-field-atomic01\">{{ page.title }}</h2>\n",
                "{# /pana:widget instance=dynamic-field-atomic01 #}\n",
                "    <p class=\"target\">Țintă</p>\n",
                "  </div>\n",
                "</main>\n",
            ),
        )
        .unwrap();
        let model = fixture.build_model().unwrap();
        let source = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::Html && node.label.starts_with("<h2"))
            .unwrap();
        let target = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::Html && node.label.starts_with("<p"))
            .unwrap();
        assert!(model
            .source_graph
            .dynamic_widget_graph
            .source_instances
            .iter()
            .any(|instance| instance.source_node_ids.contains(&source.id)));

        let plan = plan_html_move(
            &model,
            &ProjectHtmlMoveIntent {
                source_source_id: Some(source.id.clone()),
                target_source_id: Some(target.id.clone()),
                source_tag: Some("h2".to_string()),
                target_tag: Some("p".to_string()),
                position: ProjectMovePosition::After,
                native_block_slot: None,
            },
        );

        fs::remove_dir_all(root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let contents = plan.patch.unwrap().contents;
        assert_eq!(
            contents,
            concat!(
                "<main>\n",
                "  <div>\n",
                "    <p class=\"target\">Țintă</p>\n",
                "    {# pana:widget schema=2 provider=dynamic-field instance=dynamic-field-atomic01 props=00 #}\n",
                "    <h2 data-pana-widget-instance=\"dynamic-field-atomic01\">{{ page.title }}</h2>\n",
                "    {# /pana:widget instance=dynamic-field-atomic01 #}\n",
                "  </div>\n",
                "</main>\n",
            )
        );
    }

    fn project_fixture(root: PathBuf) -> ProjectModelTestFixture {
        let mut fixture = ProjectModelTestFixture::standard_zola(
            root,
            concat!(
                "<main>\n",
                "  <section class=\"hero\"></section>\n",
                "  {% include \"partials/card.html\" %}\n",
                "</main>\n",
            ),
        )
        .unwrap();
        fixture.source(
            "templates/partials/card.html",
            "<article class=\"card\"><p>Card</p></article>\n",
        );
        fixture
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
