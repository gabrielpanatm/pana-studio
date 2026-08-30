use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    kernel::dynamic_widgets::{
        generate_dynamic_widget_instance_id, render_dynamic_widget,
        validate_dynamic_widget_source_context, DynamicFieldScope, DynamicWidgetProperties,
    },
    project_model::model::{ProjectModel, ProjectModelFileKind},
    source_graph::{
        identity::SourceTextEdit,
        model::{SourceGraphTemplate, SourceNode, SourceNodeKind},
        tera::{parse_tera_items, TeraItemKind},
    },
};

use super::move_engine::{
    append_document_fragment, can_receive_children, content_revision, inside_prefix_for_insert,
    line_number_at_offset, parse_html_tag_at, resolve_html_element_span, same_model_path,
    source_location_at_offset, ProjectMovePosition, ProjectSourceEditLocation,
};
use super::structural_edit::{format_tera_fragment, StructuralPlacement};
use super::structural_envelope::{structural_envelope_for_html_node, StructuralEnvelopeKind};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTeraInsertIntent {
    pub target_source_id: Option<String>,
    pub target_kind: Option<String>,
    pub target_tag: Option<String>,
    pub position: ProjectMovePosition,
    pub item: ProjectTeraInsertItem,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTeraInsertItem {
    pub kind: String,
    pub label: Option<String>,
    pub target: Option<String>,
    pub name: Option<String>,
    pub expression: Option<String>,
    #[serde(default)]
    pub dynamic_widget: Option<DynamicWidgetProperties>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTeraInsertPlan {
    pub allowed: bool,
    pub diagnostic: Option<String>,
    pub model_revision: String,
    pub patch: Option<ProjectTeraInsertPatch>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTeraInsertPatch {
    pub file: String,
    pub resolved_target_id: String,
    pub inserted_label: String,
    pub inserted_kind: String,
    pub target_label: String,
    pub target_kind: String,
    pub position: ProjectMovePosition,
    pub expected_child_index: Option<usize>,
    pub before_revision: String,
    pub after_revision: String,
    pub contents: String,
    pub target_location: ProjectSourceEditLocation,
    pub inserted_location: ProjectSourceEditLocation,
    pub inserted_start_line: usize,
    pub target_start_line: usize,
    pub line_shift_start: usize,
    pub line_shift: isize,
    pub snippet: String,
    #[serde(skip)]
    exact_edit: SourceTextEdit,
    #[serde(skip)]
    inserted_offset: usize,
}

impl ProjectTeraInsertPatch {
    pub(crate) fn exact_source_edit(&self) -> SourceTextEdit {
        self.exact_edit.clone()
    }

    pub(crate) fn inserted_offset(&self) -> usize {
        self.inserted_offset
    }
}

struct TeraInsertApplication {
    contents: String,
    inserted_location: ProjectSourceEditLocation,
    inserted_start_line: usize,
    target_start_line: usize,
    line_shift_start: usize,
    line_shift: isize,
    exact_edit: SourceTextEdit,
    inserted_offset: usize,
}

pub fn plan_tera_insert_for_active_document(
    model: &ProjectModel,
    intent: &ProjectTeraInsertIntent,
    active_document_path: Option<&str>,
) -> ProjectTeraInsertPlan {
    let active_document_path = active_document_path
        .map(str::trim)
        .filter(|path| !path.is_empty());
    if active_document_path.is_none() {
        return ProjectTeraInsertPlan {
            allowed: false,
            diagnostic: Some(
                "Tera Insert Engine nu poate confirma documentul activ pentru această mutație."
                    .to_string(),
            ),
            model_revision: model.revision.clone(),
            patch: None,
        };
    }
    plan_tera_insert_with_owner(model, intent, active_document_path)
}

fn plan_tera_insert_with_owner(
    model: &ProjectModel,
    intent: &ProjectTeraInsertIntent,
    active_document_path: Option<&str>,
) -> ProjectTeraInsertPlan {
    match plan_tera_insert_inner(model, intent, active_document_path) {
        Ok(patch) => ProjectTeraInsertPlan {
            allowed: true,
            diagnostic: None,
            model_revision: model.revision.clone(),
            patch: Some(patch),
        },
        Err(message) => ProjectTeraInsertPlan {
            allowed: false,
            diagnostic: Some(message),
            model_revision: model.revision.clone(),
            patch: None,
        },
    }
}

fn plan_tera_insert_inner(
    model: &ProjectModel,
    intent: &ProjectTeraInsertIntent,
    active_document_path: Option<&str>,
) -> Result<ProjectTeraInsertPatch, String> {
    let target_node = resolve_tera_insert_anchor(model, intent)
        .ok_or_else(|| tera_anchor_missing_message(intent))?;
    if active_document_path.is_some_and(|active_document_path| {
        !same_model_path(&target_node.file, active_document_path)
    }) {
        return Err(format!(
            "Tera Insert Engine a refuzat sursa externă {}. Deschide acel document pentru a-l edita direct.",
            target_node.file
        ));
    }
    let file = model
        .files
        .iter()
        .find(|file| same_model_path(&file.relative_path, &target_node.file))
        .ok_or_else(|| {
            format!(
                "Nu am găsit fișierul {} în Project Model.",
                target_node.file
            )
        })?;
    if file.kind != ProjectModelFileKind::Template {
        return Err(
            "Tera Insert Engine este activ doar pentru template-uri Zola/Tera.".to_string(),
        );
    }

    let target_range = target_node
        .range
        .as_ref()
        .ok_or_else(|| "Ancora Tera nu are range stabil în Source Graph.".to_string())?;
    if target_range.end < target_range.start || target_range.end > file.contents.len() {
        return Err("Range-ul ancorei Tera este invalid pentru sursa curentă.".to_string());
    }

    let owner_template = template_for_node(model, target_node);
    validate_tera_insert(model, intent, target_node, owner_template)?;
    let snippet = build_tera_insert_snippet(model, &intent.item)?;
    let anchor_is_html = target_node.kind == SourceNodeKind::Html;
    let (anchor_start, anchor_end, placement) = if anchor_is_html {
        let envelope = structural_envelope_for_html_node(model, &file.contents, target_node)?;
        if intent.position == ProjectMovePosition::Inside
            && envelope.kind == StructuralEnvelopeKind::DynamicWidget
        {
            return Err(
                "Corpul unui widget dinamic este generat de contractul său. Inserează structura Tera înainte sau după widget, nu în corpul generat."
                    .to_string(),
            );
        }
        let span = if intent.position == ProjectMovePosition::Inside {
            resolve_html_element_span(&file.contents, target_range.start)?
        } else {
            envelope.span
        };
        (
            span.start,
            span.end,
            StructuralPlacement::for_html_target(model, &file.contents, target_node),
        )
    } else {
        (
            target_range.start,
            target_range.end,
            StructuralPlacement::for_direct_target(&file.contents, target_range.start),
        )
    };
    let target_location =
        source_location_at_offset(&file.contents, &target_node.file, anchor_start);
    let is_fragment_root = is_document_fragment_root(model, target_node);
    let fragment_root_application =
        if intent.position == ProjectMovePosition::Inside && is_fragment_root {
            Some(apply_tera_insert_into_document_fragment_root(
                &file.contents,
                &target_node.file,
                target_range.start,
                &snippet,
            )?)
        } else {
            None
        };
    let empty_block_application = if fragment_root_application.is_none()
        && target_node.kind == SourceNodeKind::Block
        && intent.position == ProjectMovePosition::Inside
    {
        apply_tera_insert_into_empty_block(
            &file.contents,
            &target_node.file,
            target_range.start,
            target_range.end,
            &snippet,
            &placement,
        )?
    } else {
        None
    };
    let applied = match fragment_root_application.or(empty_block_application) {
        Some(applied) => applied,
        None => apply_tera_insert(
            &file.contents,
            &target_node.file,
            anchor_start,
            anchor_end,
            anchor_is_html,
            intent.position,
            &snippet,
            &placement,
        )?,
    };

    Ok(ProjectTeraInsertPatch {
        file: target_node.file.clone(),
        resolved_target_id: target_node.id.clone(),
        inserted_label: intent
            .item
            .label
            .as_deref()
            .filter(|label| !label.trim().is_empty())
            .unwrap_or_else(|| tera_item_kind(&intent.item.kind))
            .to_string(),
        inserted_kind: tera_item_kind(&intent.item.kind).to_string(),
        target_label: target_node.label.clone(),
        target_kind: source_kind_label(&target_node.kind).to_string(),
        position: intent.position,
        expected_child_index: (intent.position == ProjectMovePosition::Inside).then_some(
            if anchor_is_html || is_fragment_root {
                target_node.children.len()
            } else {
                0
            },
        ),
        before_revision: file.revision.clone(),
        after_revision: content_revision(&applied.contents),
        contents: applied.contents,
        target_location,
        inserted_location: applied.inserted_location,
        inserted_start_line: applied.inserted_start_line,
        target_start_line: applied.target_start_line,
        line_shift_start: applied.line_shift_start,
        line_shift: applied.line_shift,
        snippet,
        exact_edit: applied.exact_edit,
        inserted_offset: applied.inserted_offset,
    })
}

fn is_document_fragment_root(model: &ProjectModel, node: &SourceNode) -> bool {
    matches!(
        node.kind,
        SourceNodeKind::Template | SourceNodeKind::Partial
    ) && node.parent.is_none()
        && model
            .source_graph
            .templates
            .iter()
            .any(|template| template.node_id == node.id && template.file == node.file)
}

fn resolve_tera_insert_anchor<'a>(
    model: &'a ProjectModel,
    intent: &ProjectTeraInsertIntent,
) -> Option<&'a SourceNode> {
    intent
        .target_source_id
        .as_deref()
        .and_then(|id| resolve_anchor_node(model, id, intent.target_kind.as_deref()))
}

fn resolve_anchor_node<'a>(
    model: &'a ProjectModel,
    source_id: &str,
    kind: Option<&str>,
) -> Option<&'a SourceNode> {
    model
        .source_graph
        .node_by_id(source_id)
        .filter(|node| is_tera_insert_anchor_kind(&node.kind) && node_kind_matches(node, kind))
}

fn validate_tera_insert(
    model: &ProjectModel,
    intent: &ProjectTeraInsertIntent,
    anchor: &SourceNode,
    template: Option<&SourceGraphTemplate>,
) -> Result<(), String> {
    let kind = tera_item_kind(&intent.item.kind);
    if !is_known_tera_item_kind(kind) {
        return Err(format!(
            "Tera Insert Engine a primit kind necunoscut: {kind}."
        ));
    }
    if let Some(widget) = &intent.item.dynamic_widget {
        if let DynamicWidgetProperties::DynamicField(field) = widget {
            validate_dynamic_widget_source_context(&anchor.file, field, &model.source_graph)?;
        }
        validate_dynamic_widget_anchor(model, anchor, intent.position, widget)?;
    }
    if anchor.kind == SourceNodeKind::Tera {
        return Err(
            "Sintaxa Tera nespecializată nu este o ancoră sigură pentru inserare vizuală."
                .to_string(),
        );
    }

    if kind == "extends" && intent.position == ProjectMovePosition::Inside {
        return Err(
            "Extends trebuie inserat la nivel de template, nu în interiorul unei ancore."
                .to_string(),
        );
    }
    if kind == "extends" && template.is_some_and(|template| template.extends.is_some()) {
        return Err("Template-ul are deja extends.".to_string());
    }
    if template.is_some_and(|template| template.is_partial) && kind == "extends" {
        return Err(
            "Partialurile nu folosesc extends. Creează un template de pagină/layout pentru extends."
                .to_string(),
        );
    }
    if template.is_some_and(|template| template.is_partial) && kind == "block" {
        return Err(
            "Partialurile nu definesc block-uri Tera. Pune HTML-ul direct în partial și include partialul în pagina dorită."
                .to_string(),
        );
    }
    if kind == "block" {
        let name = sanitize_identifier(
            intent.item.name.as_deref().unwrap_or("content"),
            "content",
            "nume block",
        )?;
        if template.is_some_and(|template| template.blocks.contains(&name)) {
            return Err(format!("Block-ul {name} există deja în template."));
        }
    }
    if kind == "componentDefinition" {
        let name = sanitize_component_name(
            intent.item.name.as_deref().unwrap_or("componenta"),
            "componenta",
            "nume componentă",
        )?;
        if model
            .source_graph
            .component_graph
            .definitions
            .iter()
            .any(|definition| {
                definition.kind
                    == crate::source_graph::model::ComponentDefinitionKind::TeraComponent
                    && definition.active
                    && definition.name == name
            })
        {
            return Err(format!("Componenta {name} există deja în proiect."));
        }
    }
    if kind == "componentCall" {
        let name = sanitize_component_name(
            intent.item.name.as_deref().unwrap_or("componenta"),
            "componenta",
            "nume componentă apelată",
        )?;
        if !model
            .source_graph
            .component_graph
            .definitions
            .iter()
            .any(|definition| {
                definition.kind
                    == crate::source_graph::model::ComponentDefinitionKind::TeraComponent
                    && definition.active
                    && definition.name == name
            })
        {
            return Err(format!("Componenta {name} nu există în proiect."));
        }
    }
    let context_kind = target_context_kind(model, anchor, intent.position);
    if kind == "extends" && !matches!(context_kind, Some(SourceNodeKind::Template)) {
        return Err("Extends se inserează la nivel de template în DnD sigur.".to_string());
    }
    if kind == "block" && !matches!(context_kind, Some(SourceNodeKind::Template)) {
        return Err("Block-urile Tera rămân la nivel de template în DnD sigur.".to_string());
    }
    if kind == "componentDefinition"
        && !matches!(
            context_kind,
            Some(SourceNodeKind::Template) | Some(SourceNodeKind::Partial)
        )
    {
        return Err(
            "Definițiile de componente Tera rămân la nivel de template în DnD sigur.".to_string(),
        );
    }
    if matches!(kind, "extends" | "include") {
        let fallback = if kind == "include" {
            "partials/cta.html"
        } else {
            "base.html"
        };
        let target =
            normalize_template_reference(intent.item.target.as_deref().unwrap_or(fallback))?;
        if !template_target_exists(model, &target) {
            return Err(format!(
                "Template-ul țintă nu există în Source Graph: {target}."
            ));
        }
        if kind == "include"
            && template.is_some_and(|template| template_has_reference(&template.includes, &target))
        {
            return Err(format!("Include-ul {target} există deja în template."));
        }
    }
    if intent.position == ProjectMovePosition::Inside && !can_receive_tera_inside(anchor, intent) {
        return Err("Această destinație nu poate primi Tera în interior.".to_string());
    }

    Ok(())
}

// Source span, placement and indentation are independent patch-safety inputs.
#[allow(clippy::too_many_arguments)]
fn apply_tera_insert(
    source: &str,
    file: &str,
    start: usize,
    end: usize,
    anchor_is_html: bool,
    position: ProjectMovePosition,
    snippet: &str,
    placement: &StructuralPlacement,
) -> Result<TeraInsertApplication, String> {
    let anchor_start_line_start = line_start_index(source, start);
    let anchor_start_line_break = line_break_index(source, start);
    let anchor_end_line_break = line_break_index(source, end);
    let insert_indent = placement.indent.as_str();
    let nested_indent = placement.child_indent();
    let formatted = format_tera_fragment(
        snippet,
        if position == ProjectMovePosition::Inside {
            &nested_indent
        } else {
            insert_indent
        },
        &placement.style,
    )?;
    if position == ProjectMovePosition::Inside && anchor_is_html {
        let insert_index = html_inside_insert_index(source, start, end)?;
        let opening = parse_html_tag_at(source, start).ok_or_else(|| {
            "Ancora HTML nu mai indică un tag stabil pentru inserarea Tera.".to_string()
        })?;
        let before_insert = inside_prefix_for_insert(source, opening.end, insert_index);
        let inserted_fragment_offset = before_insert.len() + placement.style.line_ending().len();
        let replacement_length = placement.style.line_ending().len()
            + formatted.len()
            + placement.style.line_ending().len()
            + insert_indent.len();
        let contents = format!(
            "{}{}{}{}{}{}",
            before_insert,
            placement.style.line_ending(),
            formatted,
            placement.style.line_ending(),
            insert_indent,
            &source[insert_index..]
        );
        let inserted_start_line = line_number_at_offset(&contents, inserted_fragment_offset);
        let line_shift = contents.bytes().filter(|byte| *byte == b'\n').count() as isize
            - source.bytes().filter(|byte| *byte == b'\n').count() as isize;
        return Ok(TeraInsertApplication {
            target_start_line: line_number_at_offset(&contents, start),
            contents,
            inserted_location: ProjectSourceEditLocation {
                file: file.to_string(),
                line: inserted_start_line,
                column: nested_indent.chars().count() + 1,
            },
            inserted_start_line,
            line_shift_start: inserted_start_line,
            line_shift,
            exact_edit: SourceTextEdit {
                old_start: before_insert.len(),
                old_end: insert_index,
                new_start: before_insert.len(),
                new_end: before_insert.len() + replacement_length,
            },
            inserted_offset: inserted_fragment_offset + nested_indent.len(),
        });
    }
    let block = format!("{formatted}{}", placement.style.line_ending());

    let insert_index = match position {
        ProjectMovePosition::Before => anchor_start_line_start,
        ProjectMovePosition::After => anchor_end_line_break
            .map(|index| index + 1)
            .unwrap_or(source.len()),
        ProjectMovePosition::Inside if anchor_is_html => unreachable!("handled above"),
        ProjectMovePosition::Inside => anchor_start_line_break
            .map(|index| index + 1)
            .unwrap_or(end),
    };
    let insertion =
        source_block_for_insert(source, insert_index, &block, placement.style.line_ending());
    let inserted_fragment_offset = insert_index + insertion.len().saturating_sub(block.len());
    let target_offset = if insert_index <= start {
        start + insertion.len()
    } else {
        start
    };
    let contents = format!(
        "{}{}{}",
        &source[..insert_index],
        insertion,
        &source[insert_index..]
    );
    let inserted_start_line = line_number_at_offset(&contents, inserted_fragment_offset);
    let target_start_line = line_number_at_offset(&contents, target_offset);
    let column_indent = if position == ProjectMovePosition::Inside {
        &nested_indent
    } else {
        insert_indent
    };

    Ok(TeraInsertApplication {
        contents,
        inserted_location: ProjectSourceEditLocation {
            file: file.to_string(),
            line: inserted_start_line,
            column: column_indent.chars().count() + 1,
        },
        inserted_start_line,
        target_start_line,
        line_shift_start: inserted_start_line,
        line_shift: insertion.bytes().filter(|byte| *byte == b'\n').count() as isize,
        exact_edit: SourceTextEdit {
            old_start: insert_index,
            old_end: insert_index,
            new_start: insert_index,
            new_end: insert_index + insertion.len(),
        },
        inserted_offset: inserted_fragment_offset + column_indent.len(),
    })
}

fn apply_tera_insert_into_document_fragment_root(
    source: &str,
    file: &str,
    target_start: usize,
    snippet: &str,
) -> Result<TeraInsertApplication, String> {
    let placement = StructuralPlacement::for_direct_target(source, 0);
    let block = format_tera_fragment(snippet, "", &placement.style)?;
    let appended = append_document_fragment(source, &block);
    let inserted_start_line = appended.inserted_start_line;
    let exact_edit = SourceTextEdit {
        old_start: appended.insertion_offset,
        old_end: appended.insertion_offset,
        new_start: appended.insertion_offset,
        new_end: appended.insertion_offset + appended.inserted_length,
    };
    let target_start_line = line_number_at_offset(&appended.contents, target_start);
    Ok(TeraInsertApplication {
        contents: appended.contents,
        inserted_location: ProjectSourceEditLocation {
            file: file.to_string(),
            line: inserted_start_line,
            column: 1,
        },
        inserted_start_line,
        target_start_line,
        line_shift_start: inserted_start_line,
        line_shift: appended.line_shift,
        exact_edit,
        inserted_offset: appended.inserted_offset,
    })
}

fn apply_tera_insert_into_empty_block(
    source: &str,
    file: &str,
    start: usize,
    end: usize,
    snippet: &str,
    placement: &StructuralPlacement,
) -> Result<Option<TeraInsertApplication>, String> {
    let target_source = source
        .get(start..end)
        .ok_or_else(|| "Range-ul blocului Tera activ este invalid.".to_string())?;
    let items = parse_tera_items(target_source);
    let Some(opening) = items.iter().find(|item| {
        item.kind == TeraItemKind::Node
            && item.node_kind == Some(SourceNodeKind::Block)
            && item.start == 0
    }) else {
        return Err("Range-ul activ nu mai începe cu un bloc Tera.".to_string());
    };
    let Some(closing) = items
        .iter()
        .rev()
        .find(|item| item.kind == TeraItemKind::EndScope)
    else {
        return Err("Blocul Tera activ nu mai are închidere stabilă.".to_string());
    };
    let body = target_source
        .get(opening.end..closing.start)
        .ok_or_else(|| "Interiorul blocului Tera activ are un range invalid.".to_string())?;
    if !body.trim().is_empty() {
        return Ok(None);
    }

    let block_indent = placement.indent.as_str();
    let child_indent = placement.child_indent();
    let formatted = format_tera_fragment(snippet, &child_indent, &placement.style)?;
    let block = format!("{formatted}{}", placement.style.line_ending());
    let opening_end = start + opening.end;
    let insert_at = start + closing.start;
    let existing_body = source
        .get(opening_end..insert_at)
        .ok_or_else(|| "Interiorul blocului Tera activ are un range invalid.".to_string())?;
    let replacement = format!("{}{block}{block_indent}", placement.style.line_ending());
    let inserted_start_line = line_number_at_offset(source, opening_end) + 1;
    let contents = format!(
        "{}{}{}",
        &source[..opening_end],
        replacement,
        &source[insert_at..]
    );
    let replacement_lines = replacement.bytes().filter(|byte| *byte == b'\n').count() as isize;
    let existing_lines = existing_body.bytes().filter(|byte| *byte == b'\n').count() as isize;
    let target_start_line = line_number_at_offset(&contents, start);

    Ok(Some(TeraInsertApplication {
        contents,
        inserted_location: ProjectSourceEditLocation {
            file: file.to_string(),
            line: inserted_start_line,
            column: child_indent.chars().count() + 1,
        },
        inserted_start_line,
        target_start_line,
        line_shift_start: inserted_start_line,
        line_shift: replacement_lines - existing_lines,
        exact_edit: SourceTextEdit {
            old_start: opening_end,
            old_end: insert_at,
            new_start: opening_end,
            new_end: opening_end + replacement.len(),
        },
        inserted_offset: opening_end + placement.style.line_ending().len() + child_indent.len(),
    }))
}

fn html_inside_insert_index(source: &str, start: usize, end: usize) -> Result<usize, String> {
    let opening = parse_html_tag_at(source, start).ok_or_else(|| {
        "Ancora HTML nu mai indică un tag stabil pentru inserarea Tera.".to_string()
    })?;
    if opening.is_closing || opening.is_self_closing {
        return Err("Ancora HTML nu poate primi conținut Tera în interior.".to_string());
    }
    let element = source
        .get(start..end)
        .ok_or_else(|| "Range-ul ancorei HTML este invalid pentru inserarea Tera.".to_string())?;
    let closing = format!("</{}", opening.tag);
    let relative = element
        .to_ascii_lowercase()
        .rfind(&closing)
        .ok_or_else(|| {
            format!(
                "Nu am găsit tagul de închidere </{}> al ancorei.",
                opening.tag
            )
        })?;
    Ok(start + relative)
}

fn build_tera_insert_snippet(
    model: &ProjectModel,
    item: &ProjectTeraInsertItem,
) -> Result<String, String> {
    let kind = tera_item_kind(&item.kind);
    if let Some(properties) = &item.dynamic_widget {
        if kind != "dynamicWidget" {
            return Err(
                "Proprietățile widgetului dinamic cer item kind dynamicWidget.".to_string(),
            );
        }
        let instance_id = generate_dynamic_widget_instance_id(
            properties.provider_kind(),
            &format!(
                "{}:{}",
                model.revision,
                item.label.as_deref().unwrap_or_default()
            ),
            model
                .source_graph
                .dynamic_widget_graph
                .source_instances
                .iter()
                .map(|instance| instance.instance_id.as_str()),
        );
        return render_dynamic_widget(&instance_id, properties, &model.source_graph);
    }
    let snippet = match kind {
        "extends" => {
            let target =
                normalize_template_reference(item.target.as_deref().unwrap_or("base.html"))?;
            format!("{{% extends \"{target}\" %}}")
        }
        "block" => {
            let name = sanitize_identifier(
                item.name.as_deref().unwrap_or("content"),
                "content",
                "nume block",
            )?;
            format!("{{% block {name} %}}\n{{% endblock %}}")
        }
        "include" => {
            let target = normalize_template_reference(
                item.target.as_deref().unwrap_or("partials/cta.html"),
            )?;
            format!("{{% include \"{target}\" %}}")
        }
        "componentDefinition" => {
            let name = sanitize_component_name(
                item.name.as_deref().unwrap_or("componenta"),
                "componenta",
                "nume componentă",
            )?;
            format!("{{% component {name}() %}}\n{{% endcomponent {name} %}}")
        }
        "componentCall" => {
            let name = sanitize_component_name(
                item.name.as_deref().unwrap_or("componenta"),
                "componenta",
                "nume componentă apelată",
            )?;
            format!("{{{{<{name} />}}}}")
        }
        "for" => {
            let expression = sanitize_tera_expression(
                item.expression.as_deref().unwrap_or("item in items"),
                "item in items",
            )?;
            format!("{{% for {expression} %}}\n{{% endfor %}}")
        }
        "if" => {
            let expression = sanitize_tera_expression(
                item.expression.as_deref().unwrap_or("condition"),
                "condition",
            )?;
            format!("{{% if {expression} %}}\n{{% endif %}}")
        }
        "set" => {
            let expression = sanitize_tera_expression(
                item.expression.as_deref().unwrap_or("name = value"),
                "name = value",
            )?;
            format!("{{% set {expression} %}}")
        }
        "setGlobal" => {
            let expression = sanitize_tera_expression(
                item.expression.as_deref().unwrap_or("name = value"),
                "name = value",
            )?;
            format!("{{% set_global {expression} %}}")
        }
        "filter" => {
            let expression =
                sanitize_tera_expression(item.expression.as_deref().unwrap_or("safe"), "safe")?;
            format!("{{% filter {expression} %}}\n{{% endfilter %}}")
        }
        "break" => "{% break %}".to_string(),
        "continue" => "{% continue %}".to_string(),
        "super" => "{{ super() }}".to_string(),
        "teraVariable" => {
            let expression =
                sanitize_tera_expression(item.expression.as_deref().unwrap_or("value"), "value")?;
            format!("{{{{ {expression} }}}}")
        }
        "teraComment" => {
            let expression =
                sanitize_tera_comment(item.expression.as_deref().unwrap_or("comentariu"))?;
            format!("{{# {expression} #}}")
        }
        "raw" => "{% raw %}\n{% endraw %}".to_string(),
        _ => {
            return Err(format!(
                "Tera Insert Engine a primit kind necunoscut: {kind}."
            ))
        }
    };

    if matches!(kind, "extends" | "include") {
        let target = if kind == "include" {
            item.target.as_deref().unwrap_or("partials/cta.html")
        } else {
            item.target.as_deref().unwrap_or("base.html")
        };
        let normalized = normalize_template_reference(target)?;
        if !template_target_exists(model, &normalized) {
            return Err(format!(
                "Template-ul țintă nu există în Source Graph: {normalized}."
            ));
        }
    }

    Ok(snippet)
}

fn can_receive_tera_inside(anchor: &SourceNode, intent: &ProjectTeraInsertIntent) -> bool {
    if anchor.kind == SourceNodeKind::Html {
        return intent
            .target_tag
            .as_deref()
            .map(can_receive_children)
            .unwrap_or(false);
    }
    matches!(
        anchor.kind,
        SourceNodeKind::Template
            | SourceNodeKind::Partial
            | SourceNodeKind::Block
            | SourceNodeKind::ComponentDefinition
            | SourceNodeKind::For
            | SourceNodeKind::If
            | SourceNodeKind::Filter
            | SourceNodeKind::Raw
            | SourceNodeKind::Tera
    )
}

fn target_context_kind(
    model: &ProjectModel,
    anchor: &SourceNode,
    position: ProjectMovePosition,
) -> Option<SourceNodeKind> {
    if position == ProjectMovePosition::Inside {
        return Some(anchor.kind.clone());
    }
    let parent_id = anchor.parent.as_deref()?;
    model
        .source_graph
        .node_by_id(parent_id)
        .map(|node| node.kind.clone())
}

fn template_for_node<'a>(
    model: &'a ProjectModel,
    node: &SourceNode,
) -> Option<&'a SourceGraphTemplate> {
    model
        .source_graph
        .templates
        .iter()
        .find(|template| same_model_path(&template.file, &node.file))
}

fn template_target_exists(model: &ProjectModel, target: &str) -> bool {
    model
        .source_graph
        .templates
        .iter()
        .any(|template| normalize_template_name(&template.name) == target)
}

fn template_has_reference(references: &[String], target: &str) -> bool {
    references
        .iter()
        .any(|reference| normalize_template_name(reference) == target)
}

fn normalize_template_reference(value: &str) -> Result<String, String> {
    let normalized = normalize_template_name(value);
    if normalized.is_empty()
        || normalized.starts_with('/')
        || normalized.contains('\0')
        || normalized
            .split('/')
            .any(|part| part == ".." || part.is_empty())
    {
        return Err(format!("Referință template Tera invalidă: {value}."));
    }
    Ok(normalized)
}

fn normalize_template_name(value: &str) -> String {
    let trimmed = value
        .trim()
        .trim_matches(|character| character == '"' || character == '\'')
        .replace('\\', "/");
    trimmed
        .trim_start_matches('/')
        .strip_prefix("templates/")
        .unwrap_or(trimmed.trim_start_matches('/'))
        .to_string()
}

fn sanitize_identifier(value: &str, fallback: &str, label: &str) -> Result<String, String> {
    let candidate = value.trim();
    let value = if candidate.is_empty() {
        fallback
    } else {
        candidate
    };
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return Err(format!("Tera Insert Engine a primit {label} gol."));
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return Err(format!(
            "Tera Insert Engine a primit {label} invalid: {value}."
        ));
    }
    if !chars.all(|character| character == '_' || character.is_ascii_alphanumeric()) {
        return Err(format!(
            "Tera Insert Engine a primit {label} invalid: {value}."
        ));
    }
    Ok(value.to_string())
}

fn sanitize_component_name(value: &str, fallback: &str, label: &str) -> Result<String, String> {
    let value = if value.trim().is_empty() {
        fallback
    } else {
        value.trim()
    };
    if value
        .split('.')
        .any(|segment| sanitize_identifier(segment, "", label).is_err())
    {
        return Err(format!(
            "Tera Insert Engine a primit {label} invalid: {value}."
        ));
    }
    Ok(value.to_string())
}

fn sanitize_tera_expression(value: &str, fallback: &str) -> Result<String, String> {
    let expression = if value.trim().is_empty() {
        fallback
    } else {
        value.trim()
    };
    if expression.len() > 500
        || expression.contains('\0')
        || expression.contains("{%")
        || expression.contains("%}")
        || expression.contains("{{")
        || expression.contains("}}")
        || expression.contains("{#")
        || expression.contains("#}")
    {
        return Err("Expresia Tera conține delimitere sau caractere nepermise.".to_string());
    }
    Ok(expression.to_string())
}

fn sanitize_tera_comment(value: &str) -> Result<String, String> {
    let comment = value.trim();
    if comment.len() > 500 || comment.contains('\0') || comment.contains("#}") {
        return Err("Comentariul Tera conține delimitere sau caractere nepermise.".to_string());
    }
    Ok(if comment.is_empty() {
        "comentariu"
    } else {
        comment
    }
    .to_string())
}

fn is_tera_insert_anchor_kind(kind: &SourceNodeKind) -> bool {
    matches!(
        kind,
        SourceNodeKind::Template
            | SourceNodeKind::Partial
            | SourceNodeKind::Html
            | SourceNodeKind::Extends
            | SourceNodeKind::Block
            | SourceNodeKind::Include
            | SourceNodeKind::ComponentDefinition
            | SourceNodeKind::ComponentCall
            | SourceNodeKind::For
            | SourceNodeKind::If
            | SourceNodeKind::Set
            | SourceNodeKind::SetGlobal
            | SourceNodeKind::Filter
            | SourceNodeKind::Break
            | SourceNodeKind::Continue
            | SourceNodeKind::Super
            | SourceNodeKind::TeraVariable
            | SourceNodeKind::TeraComment
            | SourceNodeKind::Raw
            | SourceNodeKind::Tera
    )
}

fn node_kind_matches(node: &SourceNode, kind: Option<&str>) -> bool {
    let Some(kind) = kind.map(str::trim).filter(|kind| !kind.is_empty()) else {
        return true;
    };
    source_kind_label(&node.kind) == kind
}

fn source_kind_label(kind: &SourceNodeKind) -> &'static str {
    match kind {
        SourceNodeKind::Template => "template",
        SourceNodeKind::Partial => "partial",
        SourceNodeKind::Html => "html",
        SourceNodeKind::Extends => "extends",
        SourceNodeKind::Block => "block",
        SourceNodeKind::Include => "include",
        SourceNodeKind::ComponentDefinition => "componentDefinition",
        SourceNodeKind::ComponentCall => "componentCall",
        SourceNodeKind::LegacyTera => "legacyTera",
        SourceNodeKind::For => "for",
        SourceNodeKind::If => "if",
        SourceNodeKind::Elif => "elif",
        SourceNodeKind::Else => "else",
        SourceNodeKind::Set => "set",
        SourceNodeKind::SetGlobal => "setGlobal",
        SourceNodeKind::Filter => "filter",
        SourceNodeKind::Break => "break",
        SourceNodeKind::Continue => "continue",
        SourceNodeKind::Super => "super",
        SourceNodeKind::TeraVariable => "teraVariable",
        SourceNodeKind::TeraComment => "teraComment",
        SourceNodeKind::Raw => "raw",
        SourceNodeKind::Tera => "tera",
        _ => "unsupported",
    }
}

fn tera_item_kind(value: &str) -> &str {
    match value.trim() {
        "extends" => "extends",
        "block" => "block",
        "include" => "include",
        "componentDefinition" => "componentDefinition",
        "componentCall" => "componentCall",
        "for" => "for",
        "if" => "if",
        "set" => "set",
        "setGlobal" => "setGlobal",
        "filter" => "filter",
        "break" => "break",
        "continue" => "continue",
        "super" => "super",
        "teraVariable" => "teraVariable",
        "teraComment" => "teraComment",
        "raw" => "raw",
        "dynamicWidget" => "dynamicWidget",
        other => other,
    }
}

fn is_known_tera_item_kind(kind: &str) -> bool {
    matches!(
        kind,
        "extends"
            | "block"
            | "include"
            | "componentDefinition"
            | "componentCall"
            | "for"
            | "if"
            | "set"
            | "setGlobal"
            | "filter"
            | "break"
            | "continue"
            | "super"
            | "teraVariable"
            | "teraComment"
            | "raw"
            | "dynamicWidget"
    )
}

fn validate_dynamic_widget_anchor(
    model: &ProjectModel,
    anchor: &SourceNode,
    position: ProjectMovePosition,
    properties: &DynamicWidgetProperties,
) -> Result<(), String> {
    match properties {
        DynamicWidgetProperties::DynamicField(field) => match field.binding.context {
            DynamicFieldScope::Page => {
                let owner = template_for_node(model, anchor)
                    .map(|template| template.name.as_str())
                    .unwrap_or_default();
                if owner.starts_with("listing-items/") {
                    Err(
                        "Un Listing Item fixează contextul collectionItem; contextul page nu poate fi folosit aici."
                            .to_string(),
                    )
                } else {
                    Ok(())
                }
            }
            DynamicFieldScope::CollectionItem => {
                let owner = template_for_node(model, anchor)
                    .map(|template| template.name.as_str())
                    .unwrap_or_default();
                if owner.starts_with("listing-items/")
                    || has_ancestor_kind(model, anchor, position, SourceNodeKind::For)
                {
                    Ok(())
                } else {
                    Err("Câmpul collectionItem poate fi inserat numai într-un Listing Item sau într-o buclă Tera for.".to_string())
                }
            }
            DynamicFieldScope::RepeaterItem => {
                if has_ancestor_kind(model, anchor, position, SourceNodeKind::For) {
                    Ok(())
                } else {
                    Err(
                        "Câmpul repeaterItem poate fi inserat numai într-o buclă Tera for."
                            .to_string(),
                    )
                }
            }
            DynamicFieldScope::Section => {
                let owner = template_for_node(model, anchor)
                    .map(|template| template.name.as_str())
                    .unwrap_or_default();
                if model.source_graph.pages.iter().any(|page| {
                    matches!(
                        page.page_kind,
                        crate::source_graph::model::SourcePageKind::Section
                    ) && page.resolved_template.as_deref() == Some(owner)
                }) {
                    Ok(())
                } else {
                    Err(
                        "Contextul section cere un template consumat de o secțiune Zola."
                            .to_string(),
                    )
                }
            }
            DynamicFieldScope::Site => Ok(()),
            DynamicFieldScope::TaxonomyTerm => {
                if has_ancestor_kind(model, anchor, position, SourceNodeKind::For) {
                    Ok(())
                } else {
                    Err(
                        "Câmpul taxonomyTerm poate fi inserat numai într-o buclă de termeni."
                            .to_string(),
                    )
                }
            }
        },
        DynamicWidgetProperties::Listing(listing) => {
            if has_ancestor_kind(model, anchor, position, SourceNodeKind::For) {
                return Err(
                    "Un Listing nu poate fi inserat în interiorul altei bucle Tera.".to_string(),
                );
            }
            if !model.source_graph.pages.iter().any(|page| {
                page.file.trim_start_matches("content/")
                    == listing.section_path.trim_start_matches("content/")
            }) {
                return Err(format!("Secțiunea {} nu există.", listing.section_path));
            }
            let item = model
                .source_graph
                .listing_items
                .items
                .iter()
                .find(|item| item.id == listing.listing_item_id)
                .ok_or_else(|| format!("Listing Item-ul {} nu există.", listing.listing_item_id))?;
            if item.template_name != listing.listing_item_template {
                return Err("Template-ul Listing Item nu corespunde contractului Rust.".to_string());
            }
            Ok(())
        }
    }
}

fn has_ancestor_kind(
    model: &ProjectModel,
    anchor: &SourceNode,
    position: ProjectMovePosition,
    kind: SourceNodeKind,
) -> bool {
    let mut current = if position == ProjectMovePosition::Inside {
        Some(anchor.id.as_str())
    } else {
        anchor.parent.as_deref()
    };
    let mut visited = BTreeSet::new();
    while let Some(id) = current {
        if !visited.insert(id) {
            break;
        }
        let Some(node) = model.source_graph.node_by_id(id) else {
            break;
        };
        if node.kind == kind {
            return true;
        }
        current = node.parent.as_deref();
    }
    false
}

fn line_start_index(source: &str, index: usize) -> usize {
    source
        .get(..index.min(source.len()))
        .and_then(|prefix| prefix.rfind('\n').map(|line| line + 1))
        .unwrap_or(0)
}

fn line_break_index(source: &str, index: usize) -> Option<usize> {
    source.get(index.min(source.len())..).and_then(|suffix| {
        suffix
            .find('\n')
            .map(|offset| index.min(source.len()) + offset)
    })
}

fn source_block_for_insert(source: &str, index: usize, block: &str, line_ending: &str) -> String {
    if index > 0 && source.as_bytes().get(index - 1) != Some(&b'\n') {
        format!("{line_ending}{block}")
    } else {
        block.to_string()
    }
}

fn tera_anchor_missing_message(intent: &ProjectTeraInsertIntent) -> String {
    let id = intent
        .target_source_id
        .as_deref()
        .unwrap_or("fără Source ID");
    let kind = intent.target_kind.as_deref().unwrap_or("fără kind");
    format!("Nu am putut ancora drop-ul Tera în Project Model. SourceNodeId: {id}; kind: {kind}.")
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
    fn plan_tera_insert_adds_include_before_html_anchor() {
        let root = unique_test_dir();
        let fixture = project_fixture(
            root.clone(),
            concat!(
                "{% block content %}\n",
                "<main>\n",
                "  <section class=\"hero\"></section>\n",
                "</main>\n",
                "{% endblock %}\n",
            ),
        );
        let model = fixture.build_model().unwrap();
        let section = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section .hero>")
            .unwrap();

        let plan = plan_tera_insert_for_active_document(
            &model,
            &ProjectTeraInsertIntent {
                target_source_id: Some(section.id.clone()),
                target_kind: Some("html".to_string()),
                target_tag: Some("section".to_string()),
                position: ProjectMovePosition::Before,
                item: ProjectTeraInsertItem {
                    kind: "include".to_string(),
                    label: Some("Include Card".to_string()),
                    target: Some("partials/card.html".to_string()),
                    name: None,
                    expression: None,
                    dynamic_widget: None,
                },
            },
            Some("templates/index.html"),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.unwrap();
        assert!(patch
            .contents
            .contains("  {% include \"partials/card.html\" %}\n  <section"));
        assert_eq!(patch.inserted_kind, "include");
    }

    #[test]
    fn preview_tera_insert_requires_the_anchor_to_belong_to_the_active_document() {
        let root = unique_test_dir();
        let mut fixture = project_fixture(
            root.clone(),
            "{% block content %}\n\n{% endblock content %}\n",
        );
        fixture.source(
            "templates/other.html",
            "{% block content %}\n\n{% endblock content %}\n",
        );
        let model = fixture.build_model().unwrap();
        let content = model
            .source_graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == SourceNodeKind::Block
                    && node.file == "templates/index.html"
                    && node.label == "content"
            })
            .unwrap();
        let intent = ProjectTeraInsertIntent {
            target_source_id: Some(content.id.clone()),
            target_kind: Some("block".to_string()),
            target_tag: Some("div".to_string()),
            position: ProjectMovePosition::Inside,
            item: ProjectTeraInsertItem {
                kind: "teraVariable".to_string(),
                label: Some("Titlu".to_string()),
                target: None,
                name: None,
                expression: Some("page.title".to_string()),
                dynamic_widget: None,
            },
        };

        let local =
            plan_tera_insert_for_active_document(&model, &intent, Some("templates/index.html"));
        let external =
            plan_tera_insert_for_active_document(&model, &intent, Some("templates/other.html"));
        let missing_owner = plan_tera_insert_for_active_document(&model, &intent, None);

        fs::remove_dir_all(&root).unwrap();
        assert!(local.allowed, "{:?}", local.diagnostic);
        let local_contents = local.patch.unwrap().contents;
        assert!(
            local_contents
                .contains("{% block content %}\n  {{ page.title }}\n{% endblock content %}"),
            "{local_contents:?}"
        );
        assert!(!external.allowed);
        assert!(external.diagnostic.unwrap().contains("sursa externă"));
        assert!(!missing_owner.allowed);
        assert!(missing_owner
            .diagnostic
            .unwrap()
            .contains("documentul activ"));
    }

    #[test]
    fn preview_tera_insert_appends_repeatedly_to_direct_fragment_root() {
        let root = unique_test_dir();
        let mut fixture = project_fixture(root.clone(), "<main></main>\n");
        fixture.source("templates/listing-items/card.html", "\n");

        let insert = |model: &ProjectModel, expression: &str| {
            let fragment = model
                .source_graph
                .nodes
                .iter()
                .find(|node| {
                    node.kind == SourceNodeKind::Partial
                        && node.file == "templates/listing-items/card.html"
                        && node.parent.is_none()
                })
                .expect("listing item root");
            plan_tera_insert_for_active_document(
                model,
                &ProjectTeraInsertIntent {
                    target_source_id: Some(fragment.id.clone()),
                    target_kind: Some("partial".to_string()),
                    target_tag: Some("div".to_string()),
                    position: ProjectMovePosition::Inside,
                    item: ProjectTeraInsertItem {
                        kind: "teraVariable".to_string(),
                        label: Some("Dynamic value".to_string()),
                        target: None,
                        name: None,
                        expression: Some(expression.to_string()),
                        dynamic_widget: None,
                    },
                },
                Some("templates/listing-items/card.html"),
            )
        };

        let model = fixture.build_model().unwrap();
        let first = insert(&model, "item.title");
        assert!(first.allowed, "{:?}", first.diagnostic);
        let first_contents = first.patch.expect("first patch").contents;
        assert_eq!(first_contents, "{{ item.title }}\n");
        fixture.draft("templates/listing-items/card.html", &first_contents);
        let model = fixture.build_model().unwrap();
        let second = insert(&model, "item.description");
        fs::remove_dir_all(&root).unwrap();
        assert!(second.allowed, "{:?}", second.diagnostic);
        assert_eq!(
            second.patch.expect("second patch").contents,
            "{{ item.title }}\n{{ item.description }}\n"
        );
    }

    #[test]
    fn plan_tera_insert_uses_only_exact_source_id_and_rejects_stale_identity() {
        let root = unique_test_dir();
        let fixture = project_fixture(
            root.clone(),
            concat!(
                "{% block content %}\n",
                "<section class=\"first\"></section>\n",
                "<section class=\"second\"></section>\n",
                "{% endblock %}\n",
            ),
        );
        let model = fixture.build_model().unwrap();
        let first = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section .first>")
            .unwrap();
        let intent = |target_source_id| ProjectTeraInsertIntent {
            target_source_id,
            target_kind: Some("html".to_string()),
            target_tag: Some("section".to_string()),
            position: ProjectMovePosition::Before,
            item: ProjectTeraInsertItem {
                kind: "include".to_string(),
                label: Some("Include Card".to_string()),
                target: Some("partials/card.html".to_string()),
                name: None,
                expression: None,
                dynamic_widget: None,
            },
        };
        let exact = plan_tera_insert_for_active_document(
            &model,
            &intent(Some(first.id.clone())),
            Some("templates/index.html"),
        );
        assert!(exact.allowed, "{:?}", exact.diagnostic);
        assert_eq!(
            exact
                .patch
                .expect("exact SourceNodeId insert patch")
                .resolved_target_id,
            first.id
        );

        let stale = plan_tera_insert_for_active_document(
            &model,
            &intent(Some("stale-source-id".to_string())),
            Some("templates/index.html"),
        );
        assert!(!stale.allowed, "{:?}", stale.diagnostic);
        assert!(stale.patch.is_none());

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn plan_tera_insert_blocks_duplicate_block() {
        let root = unique_test_dir();
        let fixture = project_fixture(
            root.clone(),
            "{% block content %}<main></main>{% endblock %}\n",
        );
        let model = fixture.build_model().unwrap();
        let main = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<main>")
            .unwrap();

        let plan = plan_tera_insert_for_active_document(
            &model,
            &ProjectTeraInsertIntent {
                target_source_id: Some(main.id.clone()),
                target_kind: Some("html".to_string()),
                target_tag: Some("main".to_string()),
                position: ProjectMovePosition::Before,
                item: ProjectTeraInsertItem {
                    kind: "block".to_string(),
                    label: Some("Block content".to_string()),
                    target: None,
                    name: Some("content".to_string()),
                    expression: None,
                    dynamic_widget: None,
                },
            },
            Some("templates/index.html"),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.diagnostic.unwrap().contains("Block-ul content există"));
    }

    #[test]
    fn plan_tera_insert_blocks_duplicate_include_with_tera_equivalent_syntax() {
        let root = unique_test_dir();
        let fixture = project_fixture(
            root.clone(),
            "{% block content %}\n{%- include 'partials/card.html' -%}\n<main></main>\n{% endblock %}\n",
        );
        let model = fixture.build_model().unwrap();
        let main = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<main>")
            .unwrap();

        let plan = plan_tera_insert_for_active_document(
            &model,
            &ProjectTeraInsertIntent {
                target_source_id: Some(main.id.clone()),
                target_kind: Some("html".to_string()),
                target_tag: Some("main".to_string()),
                position: ProjectMovePosition::Before,
                item: ProjectTeraInsertItem {
                    kind: "include".to_string(),
                    label: Some("Include Card".to_string()),
                    target: Some("partials/card.html".to_string()),
                    name: None,
                    expression: None,
                    dynamic_widget: None,
                },
            },
            Some("templates/index.html"),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan
            .diagnostic
            .unwrap()
            .contains("Include-ul partials/card.html există deja"));
    }

    #[test]
    fn plan_tera_insert_blocks_missing_include_target() {
        let root = unique_test_dir();
        let fixture = project_fixture(
            root.clone(),
            "{% block content %}<main></main>{% endblock %}\n",
        );
        let model = fixture.build_model().unwrap();
        let main = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<main>")
            .unwrap();

        let plan = plan_tera_insert_for_active_document(
            &model,
            &ProjectTeraInsertIntent {
                target_source_id: Some(main.id.clone()),
                target_kind: Some("html".to_string()),
                target_tag: Some("main".to_string()),
                position: ProjectMovePosition::Inside,
                item: ProjectTeraInsertItem {
                    kind: "include".to_string(),
                    label: Some("Include Missing".to_string()),
                    target: Some("partials/missing.html".to_string()),
                    name: None,
                    expression: None,
                    dynamic_widget: None,
                },
            },
            Some("templates/index.html"),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan
            .diagnostic
            .unwrap()
            .contains("Template-ul țintă nu există"));
    }

    #[test]
    fn plan_tera_insert_blocks_block_in_nested_scope() {
        let root = unique_test_dir();
        let fixture = project_fixture(
            root.clone(),
            "{% block content %}\n<main></main>\n{% endblock %}\n",
        );
        let model = fixture.build_model().unwrap();
        let main = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<main>")
            .unwrap();

        let plan = plan_tera_insert_for_active_document(
            &model,
            &ProjectTeraInsertIntent {
                target_source_id: Some(main.id.clone()),
                target_kind: Some("html".to_string()),
                target_tag: Some("main".to_string()),
                position: ProjectMovePosition::Before,
                item: ProjectTeraInsertItem {
                    kind: "block".to_string(),
                    label: Some("Block sidebar".to_string()),
                    target: None,
                    name: Some("sidebar".to_string()),
                    expression: None,
                    dynamic_widget: None,
                },
            },
            Some("templates/index.html"),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.diagnostic.unwrap().contains("nivel de template"));
    }

    #[test]
    fn plan_tera_insert_blocks_component_definition_in_nested_scope() {
        let root = unique_test_dir();
        let fixture = project_fixture(
            root.clone(),
            "{% block content %}\n<main></main>\n{% endblock %}\n",
        );
        let model = fixture.build_model().unwrap();
        let main = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<main>")
            .unwrap();

        let plan = plan_tera_insert_for_active_document(
            &model,
            &ProjectTeraInsertIntent {
                target_source_id: Some(main.id.clone()),
                target_kind: Some("html".to_string()),
                target_tag: Some("main".to_string()),
                position: ProjectMovePosition::Before,
                item: ProjectTeraInsertItem {
                    kind: "componentDefinition".to_string(),
                    label: Some("Component sidebar".to_string()),
                    target: None,
                    name: Some("sidebar".to_string()),
                    expression: None,
                    dynamic_widget: None,
                },
            },
            Some("templates/index.html"),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.diagnostic.unwrap().contains("nivel de template"));
    }

    #[test]
    fn plan_tera_insert_uses_filter_as_a_specialized_anchor() {
        let root = unique_test_dir();
        let fixture = project_fixture(
            root.clone(),
            concat!(
                "{% block content %}\n",
                "{% filter upper %}{{ title }}{% endfilter %}\n",
                "{% endblock %}\n",
            ),
        );
        let model = fixture.build_model().unwrap();
        let filter = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::Filter)
            .unwrap();

        let plan = plan_tera_insert_for_active_document(
            &model,
            &ProjectTeraInsertIntent {
                target_source_id: Some(filter.id.clone()),
                target_kind: Some("filter".to_string()),
                target_tag: None,
                position: ProjectMovePosition::Before,
                item: ProjectTeraInsertItem {
                    kind: "include".to_string(),
                    label: Some("Include Card".to_string()),
                    target: Some("partials/card.html".to_string()),
                    name: None,
                    expression: None,
                    dynamic_widget: None,
                },
            },
            Some("templates/index.html"),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        assert!(plan
            .patch
            .expect("filter insert patch")
            .contents
            .contains("{% include \"partials/card.html\" %}"));
    }

    #[test]
    fn plan_tera_insert_adds_component_call_at_template_level() {
        let root = unique_test_dir();
        let fixture = project_fixture(root.clone(), "<main></main>\n");
        let model = fixture.build_model().unwrap();
        let main = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<main>")
            .unwrap();
        let plan = plan_tera_insert_for_active_document(
            &model,
            &ProjectTeraInsertIntent {
                target_source_id: Some(main.id.clone()),
                target_kind: Some("html".to_string()),
                target_tag: Some("main".to_string()),
                position: ProjectMovePosition::Before,
                item: ProjectTeraInsertItem {
                    kind: "componentCall".to_string(),
                    label: Some("Card".to_string()),
                    target: None,
                    name: Some("card".to_string()),
                    expression: None,
                    dynamic_widget: None,
                },
            },
            Some("templates/index.html"),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let contents = plan.patch.expect("component call patch").contents;
        assert!(contents.contains("{{<card />}}"));
        assert!(
            contents.find("{{<card />}}").unwrap() < contents.find("<main>").unwrap(),
            "Apelul componentei trebuie inserat înaintea ancorei: {contents}"
        );
    }

    #[test]
    fn plan_tera_insert_allows_component_call_inside_html() {
        let root = unique_test_dir();
        let fixture = project_fixture(
            root.clone(),
            "{% block content %}<main></main>{% endblock %}\n",
        );
        let model = fixture.build_model().unwrap();
        let main = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<main>")
            .unwrap();
        let plan = plan_tera_insert_for_active_document(
            &model,
            &ProjectTeraInsertIntent {
                target_source_id: Some(main.id.clone()),
                target_kind: Some("html".to_string()),
                target_tag: Some("main".to_string()),
                position: ProjectMovePosition::Inside,
                item: ProjectTeraInsertItem {
                    kind: "componentCall".to_string(),
                    label: Some("Card".to_string()),
                    target: None,
                    name: Some("card".to_string()),
                    expression: None,
                    dynamic_widget: None,
                },
            },
            Some("templates/index.html"),
        );
        fs::remove_dir_all(root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        assert!(plan
            .patch
            .expect("component call patch")
            .contents
            .contains("<main>\n  {{<card />}}\n</main>"));
    }

    fn project_fixture(root: PathBuf, template: &str) -> ProjectModelTestFixture {
        let mut fixture = ProjectModelTestFixture::standard_zola(root, template).unwrap();
        fixture.source("templates/partials/card.html", "<article></article>\n");
        fixture.source("templates/base.html", "<body></body>\n");
        fixture.source(
            "templates/components.html",
            "{% component card() %}{% endcomponent card %}\n",
        );
        fixture
    }

    fn unique_test_dir() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "pana-studio-tera-insert-engine-{}-{stamp}",
            std::process::id()
        ))
    }
}
