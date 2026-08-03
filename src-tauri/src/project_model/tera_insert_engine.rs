use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    kernel::dynamic_widgets::{
        generate_dynamic_widget_instance_id, render_dynamic_widget,
        validate_dynamic_widget_source_context, DynamicFieldScope, DynamicWidgetProperties,
    },
    project_model::model::{ProjectModel, ProjectModelFileKind},
    source_graph::{
        model::{SourceGraphTemplate, SourceNode, SourceNodeKind},
        tera::{parse_tera_items, TeraItemKind},
    },
};

use super::move_engine::{
    append_document_fragment, can_receive_children, content_revision, line_indent_at_offset,
    line_number_at_offset, parse_html_tag_at, resolve_conjunctive_anchor,
    resolve_html_element_span, same_model_path, source_location_at_offset, ProjectMovePosition,
    ProjectSourceEditLocation,
};
use super::structural_envelope::{
    semantic_html_indent, structural_envelope_for_html_node, StructuralEnvelopeKind,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTeraInsertIntent {
    pub target_source_id: Option<String>,
    pub target_location: Option<ProjectSourceEditLocation>,
    pub target_kind: Option<String>,
    pub target_tag: Option<String>,
    pub target_selector: Option<String>,
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
    pub dynamic_binding: Option<ProjectDynamicFieldBinding>,
    #[serde(default)]
    pub dynamic_widget: Option<DynamicWidgetProperties>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDynamicFieldBinding {
    pub model_id: String,
    pub field_id: String,
    pub path: String,
    pub scope: String,
    pub item_path: Option<String>,
    pub presentation: String,
    pub prefix: String,
    pub suffix: String,
    pub fallback: String,
    pub text: String,
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
    pub before_revision: String,
    pub after_revision: String,
    pub contents: String,
    pub target_location: ProjectSourceEditLocation,
    pub inserted_location: ProjectSourceEditLocation,
    pub inserted_start_line: usize,
    pub line_shift_start: usize,
    pub line_shift: isize,
    pub snippet: String,
}

struct TeraInsertApplication {
    contents: String,
    inserted_location: ProjectSourceEditLocation,
    inserted_start_line: usize,
    line_shift_start: usize,
    line_shift: isize,
}

pub fn plan_tera_insert(
    model: &ProjectModel,
    intent: &ProjectTeraInsertIntent,
) -> ProjectTeraInsertPlan {
    plan_tera_insert_with_owner(model, intent, None)
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
    let (anchor_start, anchor_end, semantic_indent) = if anchor_is_html {
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
            semantic_html_indent(model, &file.contents, target_node),
        )
    } else {
        (
            target_range.start,
            target_range.end,
            line_indent_at_offset(&file.contents, target_range.start),
        )
    };
    let target_location =
        source_location_at_offset(&file.contents, &target_node.file, anchor_start);
    let fragment_root_application = if intent.position == ProjectMovePosition::Inside
        && is_document_fragment_root(model, target_node)
    {
        Some(apply_tera_insert_into_document_fragment_root(
            &file.contents,
            &target_node.file,
            &snippet,
        ))
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
            &semantic_indent,
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
            &semantic_indent,
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
        before_revision: file.revision.clone(),
        after_revision: content_revision(&applied.contents),
        contents: applied.contents,
        target_location,
        inserted_location: applied.inserted_location,
        inserted_start_line: applied.inserted_start_line,
        line_shift_start: applied.line_shift_start,
        line_shift: applied.line_shift,
        snippet,
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
    let id_node = intent
        .target_source_id
        .as_deref()
        .and_then(|id| resolve_anchor_node(model, id, intent.target_kind.as_deref()));
    let location_node = intent.target_location.as_ref().and_then(|location| {
        resolve_anchor_node_at_location(model, location, intent.target_kind.as_deref())
    });

    resolve_conjunctive_anchor(
        intent.target_source_id.as_deref(),
        intent.target_location.as_ref(),
        id_node,
        location_node,
    )
}

fn resolve_anchor_node<'a>(
    model: &'a ProjectModel,
    source_id: &str,
    kind: Option<&str>,
) -> Option<&'a SourceNode> {
    model.source_graph.nodes.iter().find(|node| {
        node.id == source_id
            && is_tera_insert_anchor_kind(&node.kind)
            && node_kind_matches(node, kind)
    })
}

fn resolve_anchor_node_at_location<'a>(
    model: &'a ProjectModel,
    location: &ProjectSourceEditLocation,
    kind: Option<&str>,
) -> Option<&'a SourceNode> {
    if location.line == 0 || location.column == 0 {
        return None;
    }

    let mut candidates: Vec<&SourceNode> = model
        .source_graph
        .nodes
        .iter()
        .filter(|node| {
            is_tera_insert_anchor_kind(&node.kind)
                && same_model_path(&node.file, &location.file)
                && node_kind_matches(node, kind)
                && node
                    .range
                    .as_ref()
                    .is_some_and(|range| range.line == location.line)
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
    if let Some(binding) = &intent.item.dynamic_binding {
        validate_dynamic_field_binding(model, kind, binding)?;
        validate_dynamic_field_anchor(model, anchor, intent.position, binding)?;
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
    if kind == "import" && intent.position == ProjectMovePosition::Inside {
        return Err(
            "Importurile Tera se inserează la nivel de template, înainte sau după o ancoră stabilă."
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
    if kind == "macro" {
        let name = sanitize_identifier(
            intent.item.name.as_deref().unwrap_or("componenta"),
            "componenta",
            "nume macro",
        )?;
        if template.is_some_and(|template| template.macros.contains(&name)) {
            return Err(format!("Macro-ul {name} există deja în template."));
        }
    }
    let context_kind = target_context_kind(model, anchor, intent.position);
    if kind == "extends" && !matches!(context_kind, Some(SourceNodeKind::Template)) {
        return Err("Extends se inserează la nivel de template în DnD sigur.".to_string());
    }
    if kind == "block" && !matches!(context_kind, Some(SourceNodeKind::Template)) {
        return Err("Block-urile Tera rămân la nivel de template în DnD sigur.".to_string());
    }
    if matches!(kind, "macro" | "import")
        && !matches!(
            context_kind,
            Some(SourceNodeKind::Template) | Some(SourceNodeKind::Partial)
        )
    {
        return Err(
            "Macro-urile și importurile Tera rămân la nivel de template în DnD sigur.".to_string(),
        );
    }
    if matches!(kind, "extends" | "include" | "import" | "macroCall") {
        let fallback = if kind == "include" {
            "partials/cta.html"
        } else if matches!(kind, "import" | "macroCall") {
            "macros.html"
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
        if kind == "import"
            && template.is_some_and(|template| template_has_reference(&template.imports, &target))
        {
            return Err(format!("Importul {target} există deja în template."));
        }
    }
    if intent.position == ProjectMovePosition::Inside && !can_receive_tera_inside(anchor, intent) {
        return Err("Această destinație nu poate primi Tera în interior.".to_string());
    }

    Ok(())
}

fn apply_tera_insert(
    source: &str,
    file: &str,
    start: usize,
    end: usize,
    anchor_is_html: bool,
    position: ProjectMovePosition,
    snippet: &str,
    semantic_anchor_indent: &str,
) -> Result<TeraInsertApplication, String> {
    let anchor_start_line_start = line_start_index(source, start);
    let anchor_start_line_break = line_break_index(source, start);
    let anchor_end_line_break = line_break_index(source, end);
    let insert_indent = semantic_anchor_indent.to_string();
    let nested_indent = format!("{insert_indent}  ");
    let block = format_inserted_tera_snippet(
        snippet,
        if position == ProjectMovePosition::Inside {
            &nested_indent
        } else {
            &insert_indent
        },
    );

    let insert_index = match position {
        ProjectMovePosition::Before => anchor_start_line_start,
        ProjectMovePosition::After => anchor_end_line_break
            .map(|index| index + 1)
            .unwrap_or(source.len()),
        ProjectMovePosition::Inside if anchor_is_html => {
            html_inside_insert_index(source, start, end)?
        }
        ProjectMovePosition::Inside => anchor_start_line_break
            .map(|index| index + 1)
            .unwrap_or(end),
    };
    let insertion = source_block_for_insert(source, insert_index, &block);
    let inserted_start_line = line_number_at_offset(source, insert_index)
        + if insertion.starts_with('\n') { 1 } else { 0 };
    let contents = format!(
        "{}{}{}",
        &source[..insert_index],
        insertion,
        &source[insert_index..]
    );
    let column_indent = if position == ProjectMovePosition::Inside {
        &nested_indent
    } else {
        &insert_indent
    };

    Ok(TeraInsertApplication {
        contents,
        inserted_location: ProjectSourceEditLocation {
            file: file.to_string(),
            line: inserted_start_line,
            column: column_indent.chars().count() + 1,
        },
        inserted_start_line,
        line_shift_start: inserted_start_line,
        line_shift: insertion.bytes().filter(|byte| *byte == b'\n').count() as isize,
    })
}

fn apply_tera_insert_into_document_fragment_root(
    source: &str,
    file: &str,
    snippet: &str,
) -> TeraInsertApplication {
    let block = format_inserted_tera_snippet(snippet, "");
    let appended = append_document_fragment(source, &block);
    let inserted_start_line = appended.inserted_start_line;
    TeraInsertApplication {
        contents: appended.contents,
        inserted_location: ProjectSourceEditLocation {
            file: file.to_string(),
            line: inserted_start_line,
            column: 1,
        },
        inserted_start_line,
        line_shift_start: inserted_start_line,
        line_shift: appended.line_shift,
    }
}

fn apply_tera_insert_into_empty_block(
    source: &str,
    file: &str,
    start: usize,
    end: usize,
    snippet: &str,
    block_indent: &str,
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

    let child_indent = format!("{block_indent}  ");
    let block = format_inserted_tera_snippet(snippet, &child_indent);
    let opening_end = start + opening.end;
    let insert_at = start + closing.start;
    let existing_body = source
        .get(opening_end..insert_at)
        .ok_or_else(|| "Interiorul blocului Tera activ are un range invalid.".to_string())?;
    let replacement = format!("\n{block}{block_indent}");
    let inserted_start_line = line_number_at_offset(source, opening_end) + 1;
    let contents = format!(
        "{}{}{}",
        &source[..opening_end],
        replacement,
        &source[insert_at..]
    );
    let replacement_lines = replacement.bytes().filter(|byte| *byte == b'\n').count() as isize;
    let existing_lines = existing_body.bytes().filter(|byte| *byte == b'\n').count() as isize;

    Ok(Some(TeraInsertApplication {
        contents,
        inserted_location: ProjectSourceEditLocation {
            file: file.to_string(),
            line: inserted_start_line,
            column: child_indent.chars().count() + 1,
        },
        inserted_start_line,
        line_shift_start: inserted_start_line,
        line_shift: replacement_lines - existing_lines,
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
    if let Some(binding) = &item.dynamic_binding {
        return build_dynamic_field_snippet(model, kind, binding);
    }
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
        "import" => {
            let target =
                normalize_template_reference(item.target.as_deref().unwrap_or("macros.html"))?;
            let name = sanitize_identifier(
                item.name.as_deref().unwrap_or("macros"),
                "macros",
                "alias import",
            )?;
            format!("{{% import \"{target}\" as {name} %}}")
        }
        "macro" => {
            let name = sanitize_identifier(
                item.name.as_deref().unwrap_or("componenta"),
                "componenta",
                "nume macro",
            )?;
            format!("{{% macro {name}() %}}\n{{% endmacro %}}")
        }
        "macroCall" => {
            let target =
                normalize_template_reference(item.target.as_deref().unwrap_or("macros.html"))?;
            let name = sanitize_identifier(
                item.name.as_deref().unwrap_or("componenta"),
                "componenta",
                "nume macro apelat",
            )?;
            format!(
                "{{% import \"{target}\" as pana_component %}}\n{{{{ pana_component::{name}() }}}}"
            )
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

    if matches!(kind, "extends" | "include" | "import" | "macroCall") {
        let target = if kind == "include" {
            item.target.as_deref().unwrap_or("partials/cta.html")
        } else if matches!(kind, "import" | "macroCall") {
            item.target.as_deref().unwrap_or("macros.html")
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
            | SourceNodeKind::Macro
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
        .nodes
        .iter()
        .find(|node| node.id == parent_id)
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

fn validate_dynamic_field_binding(
    model: &ProjectModel,
    item_kind: &str,
    binding: &ProjectDynamicFieldBinding,
) -> Result<(), String> {
    let content_model = model
        .source_graph
        .content_models
        .models
        .iter()
        .find(|candidate| candidate.id == binding.model_id)
        .ok_or_else(|| {
            format!(
                "Binding-ul dinamic referă modelul inexistent „{}”.",
                binding.model_id
            )
        })?;
    let (field, canonical_path, canonical_item_path) =
        find_dynamic_field(&content_model.fields, &binding.field_id, "", None).ok_or_else(
            || {
                format!(
                    "Binding-ul dinamic referă câmpul inexistent „{}”.",
                    binding.field_id
                )
            },
        )?;
    if canonical_path != binding.path {
        return Err(format!(
            "Calea binding-ului nu corespunde contractului Rust: {} != {}.",
            binding.path, canonical_path
        ));
    }
    for segment in binding.path.split('.') {
        sanitize_identifier(segment, "field", "segment de cale")?;
    }
    if !matches!(binding.scope.as_str(), "page" | "item") {
        return Err("Scope-ul câmpului dinamic trebuie să fie page sau item.".to_string());
    }
    if binding.scope == "item" {
        let item_path = binding
            .item_path
            .as_deref()
            .filter(|path| !path.trim().is_empty())
            .ok_or_else(|| "Scope-ul item cere o cale relativă în repetor.".to_string())?;
        for segment in item_path.split('.') {
            sanitize_identifier(segment, "field", "segment de cale item")?;
        }
        if canonical_item_path.as_deref() != Some(item_path) {
            return Err(format!(
                "Calea item nu corespunde repetorului din contract: {} != {}.",
                item_path,
                canonical_item_path.as_deref().unwrap_or("fără repetor")
            ));
        }
    } else if canonical_item_path.is_some() {
        return Err(
            "Un subcâmp al repetorului cere scope item și nu poate fi citit direct din pagină."
                .to_string(),
        );
    }
    let expected_kind = match binding.presentation.as_str() {
        "text" | "image" | "link" | "button" => "teraVariable",
        "list" => "for",
        "condition" => "if",
        other => return Err(format!("Prezentare dinamică necunoscută: {other}.")),
    };
    if item_kind != expected_kind {
        return Err(format!(
            "Prezentarea {} cere item kind {expected_kind}, nu {item_kind}.",
            binding.presentation
        ));
    }
    use crate::kernel::content_models::ContentFieldKind;
    let compatible = match binding.presentation.as_str() {
        "image" => field.kind == ContentFieldKind::Image,
        "link" | "button" => field.kind == ContentFieldKind::Url,
        "list" => field.kind == ContentFieldKind::Repeater,
        "condition" => field.kind == ContentFieldKind::Boolean,
        "text" => !matches!(
            field.kind,
            ContentFieldKind::Group | ContentFieldKind::Repeater
        ),
        _ => false,
    };
    if !compatible {
        return Err(format!(
            "Câmpul {} de tip {:?} nu este compatibil cu prezentarea {}.",
            binding.path, field.kind, binding.presentation
        ));
    }
    for (label, value) in [
        ("prefix", binding.prefix.as_str()),
        ("suffix", binding.suffix.as_str()),
        ("fallback", binding.fallback.as_str()),
        ("text", binding.text.as_str()),
    ] {
        if value.len() > 500 || value.contains('\0') {
            return Err(format!(
                "Valoarea {label} a binding-ului este prea lungă sau invalidă."
            ));
        }
    }
    Ok(())
}

fn find_dynamic_field<'a>(
    fields: &'a [crate::kernel::content_models::ContentFieldDefinition],
    field_id: &str,
    parent_path: &str,
    item_parent: Option<&str>,
) -> Option<(
    &'a crate::kernel::content_models::ContentFieldDefinition,
    String,
    Option<String>,
)> {
    for field in fields {
        let path = if parent_path.is_empty() {
            field.key.clone()
        } else {
            format!("{parent_path}.{}", field.key)
        };
        let item_path = item_parent.map(|parent| {
            if parent.is_empty() {
                field.key.clone()
            } else {
                format!("{parent}.{}", field.key)
            }
        });
        if field.id == field_id {
            return Some((field, path, item_path));
        }
        let next_item_parent =
            if field.kind == crate::kernel::content_models::ContentFieldKind::Repeater {
                Some("")
            } else {
                item_path.as_deref()
            };
        if let Some(found) = find_dynamic_field(&field.fields, field_id, &path, next_item_parent) {
            return Some(found);
        }
    }
    None
}

fn validate_dynamic_field_anchor(
    model: &ProjectModel,
    anchor: &SourceNode,
    position: ProjectMovePosition,
    binding: &ProjectDynamicFieldBinding,
) -> Result<(), String> {
    if binding.scope != "item" {
        return Ok(());
    }
    let mut current_id = if position == ProjectMovePosition::Inside {
        Some(anchor.id.as_str())
    } else {
        anchor.parent.as_deref()
    };
    let mut visited = BTreeSet::new();
    while let Some(node_id) = current_id {
        if !visited.insert(node_id.to_string()) {
            break;
        }
        let Some(node) = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.id == node_id)
        else {
            break;
        };
        if node.kind == SourceNodeKind::For {
            return Ok(());
        }
        current_id = node.parent.as_deref();
    }
    Err("Binding-ul cu scope item trebuie inserat în interiorul unei bucle Tera `for`.".to_string())
}

fn build_dynamic_field_snippet(
    model: &ProjectModel,
    item_kind: &str,
    binding: &ProjectDynamicFieldBinding,
) -> Result<String, String> {
    validate_dynamic_field_binding(model, item_kind, binding)?;
    let expression = if binding.scope == "item" {
        format!("item.{}", binding.item_path.as_deref().unwrap_or_default())
    } else {
        format!("page.extra.{}", binding.path)
    };
    let value_expression = if binding.fallback.is_empty() {
        expression.clone()
    } else {
        format!(
            "{expression} | default(value=\"{}\")",
            escape_tera_string(&binding.fallback)
        )
    };
    let marker = format!(
        "{{# pana:dynamic model={} field={} path={} scope={} presentation={} #}}",
        binding.model_id, binding.field_id, binding.path, binding.scope, binding.presentation
    );
    let prefix = escape_html_text(&binding.prefix);
    let suffix = escape_html_text(&binding.suffix);
    let text = if binding.text.trim().is_empty() {
        "Deschide".to_string()
    } else {
        escape_html_text(binding.text.trim())
    };
    let body = match binding.presentation.as_str() {
        "text" => format!("{prefix}{{{{ {value_expression} }}}}{suffix}"),
        "image" => {
            format!("{prefix}<img src=\"{{{{ {value_expression} }}}}\" alt=\"{text}\">{suffix}")
        }
        "link" => format!("{prefix}<a href=\"{{{{ {value_expression} }}}}\">{text}</a>{suffix}"),
        "button" => format!(
            "{prefix}<a class=\"button\" href=\"{{{{ {value_expression} }}}}\">{text}</a>{suffix}"
        ),
        "list" => format!("{{% for item in {expression} %}}\n{{% endfor %}}"),
        "condition" => format!("{{% if {expression} %}}\n{{% endif %}}"),
        _ => unreachable!("validated presentation"),
    };
    Ok(format!("{marker}\n{body}"))
}

fn escape_tera_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn escape_html_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
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
            | SourceNodeKind::Import
            | SourceNodeKind::Macro
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
        SourceNodeKind::Import => "import",
        SourceNodeKind::Macro => "macro",
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
        "import" => "import",
        "macro" => "macro",
        "macroCall" => "macroCall",
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
            | "import"
            | "macro"
            | "macroCall"
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
        let Some(node) = model.source_graph.nodes.iter().find(|node| node.id == id) else {
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

fn format_inserted_tera_snippet(snippet: &str, indent: &str) -> String {
    let stripped = strip_common_indent(snippet.trim_end());
    let body = stripped
        .split('\n')
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                format!("{indent}{}", line.trim_end())
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("{body}\n")
}

fn strip_common_indent(snippet: &str) -> String {
    let lines = snippet.split('\n').collect::<Vec<_>>();
    let content_lines = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    let Some(common_indent_length) = content_lines
        .iter()
        .map(|line| {
            line.chars()
                .take_while(|character| *character == ' ' || *character == '\t')
                .count()
        })
        .min()
    else {
        return snippet.to_string();
    };
    if common_indent_length == 0 {
        return snippet.to_string();
    }
    lines
        .iter()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                line.chars().skip(common_indent_length).collect()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn source_block_for_insert(source: &str, index: usize, block: &str) -> String {
    if index > 0 && source.as_bytes().get(index - 1) != Some(&b'\n') {
        format!("\n{block}")
    } else {
        block.to_string()
    }
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

fn tera_anchor_missing_message(intent: &ProjectTeraInsertIntent) -> String {
    let id = intent
        .target_source_id
        .as_deref()
        .unwrap_or("fără Source ID");
    let loc = source_location_label(intent.target_location.as_ref());
    let kind = intent.target_kind.as_deref().unwrap_or("fără kind");
    let selector = intent.target_selector.as_deref().unwrap_or("fără selector");
    format!(
        "Nu am putut ancora drop-ul Tera în Project Model. Source ID: {id}; locație: {loc}; kind: {kind}; selector live: {selector}."
    )
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
    fn plan_tera_insert_adds_include_before_html_anchor() {
        let root = unique_test_dir();
        write_project(
            &root,
            concat!(
                "{% block content %}\n",
                "<main>\n",
                "  <section class=\"hero\"></section>\n",
                "</main>\n",
                "{% endblock %}\n",
            ),
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let section = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section .hero>")
            .unwrap();

        let plan = plan_tera_insert(
            &model,
            &ProjectTeraInsertIntent {
                target_source_id: Some(section.id.clone()),
                target_location: None,
                target_kind: Some("html".to_string()),
                target_tag: Some("section".to_string()),
                target_selector: Some(".hero".to_string()),
                position: ProjectMovePosition::Before,
                item: ProjectTeraInsertItem {
                    kind: "include".to_string(),
                    label: Some("Include Card".to_string()),
                    target: Some("partials/card.html".to_string()),
                    name: None,
                    expression: None,
                    dynamic_binding: None,
                    dynamic_widget: None,
                },
            },
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
        write_project(&root, "{% block content %}\n\n{% endblock content %}\n");
        fs::write(
            root.join("templates/other.html"),
            "{% block content %}\n\n{% endblock content %}\n",
        )
        .unwrap();
        let model = build_project_model(&root, &HashMap::new()).unwrap();
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
            target_location: None,
            target_kind: Some("block".to_string()),
            target_tag: Some("div".to_string()),
            target_selector: Some("[data-pana-empty-tera-slot]".to_string()),
            position: ProjectMovePosition::Inside,
            item: ProjectTeraInsertItem {
                kind: "teraVariable".to_string(),
                label: Some("Titlu".to_string()),
                target: None,
                name: None,
                expression: Some("page.title".to_string()),
                dynamic_binding: None,
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
        write_project(&root, "<main></main>\n");
        fs::create_dir_all(root.join("templates/listing-items")).unwrap();
        let fragment_path = root.join("templates/listing-items/card.html");
        fs::write(&fragment_path, "\n").unwrap();

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
            let range = fragment.range.as_ref().expect("fragment root range");
            plan_tera_insert_for_active_document(
                model,
                &ProjectTeraInsertIntent {
                    target_source_id: Some(fragment.id.clone()),
                    target_location: Some(ProjectSourceEditLocation {
                        file: fragment.file.clone(),
                        line: range.line,
                        column: range.column,
                    }),
                    target_kind: Some("partial".to_string()),
                    target_tag: Some("div".to_string()),
                    target_selector: Some("[data-pana-active-document-root]".to_string()),
                    position: ProjectMovePosition::Inside,
                    item: ProjectTeraInsertItem {
                        kind: "teraVariable".to_string(),
                        label: Some("Dynamic value".to_string()),
                        target: None,
                        name: None,
                        expression: Some(expression.to_string()),
                        dynamic_binding: None,
                        dynamic_widget: None,
                    },
                },
                Some("templates/listing-items/card.html"),
            )
        };

        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let first = insert(&model, "item.title");
        assert!(first.allowed, "{:?}", first.diagnostic);
        let first_contents = first.patch.expect("first patch").contents;
        assert_eq!(first_contents, "{{ item.title }}\n");
        fs::write(&fragment_path, &first_contents).unwrap();

        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let second = insert(&model, "item.description");
        fs::remove_dir_all(&root).unwrap();
        assert!(second.allowed, "{:?}", second.diagnostic);
        assert_eq!(
            second.patch.expect("second patch").contents,
            "{{ item.title }}\n{{ item.description }}\n"
        );
    }

    #[test]
    fn dynamic_image_binding_is_rust_validated_and_reconstructible() {
        let root = unique_test_dir();
        write_project(
            &root,
            "<main><section class=\"hero\"></section>{% for item in page.extra.gallery %}<div class=\"item\"></div>{% endfor %}</main>\n",
        );
        fs::create_dir_all(root.join(".panastudio/content-models")).unwrap();
        fs::write(
            root.join(".panastudio/project.toml"),
            "schema_version = 1\n",
        )
        .unwrap();
        fs::write(
            root.join(".panastudio/content-models/service.toml"),
            "schemaVersion = 1\nid = \"service\"\nlabel = \"Serviciu\"\n\n[[fields]]\nid = \"field_image\"\nkey = \"image\"\nlabel = \"Imagine\"\nkind = \"image\"\n\n[[fields]]\nid = \"field_gallery\"\nkey = \"gallery\"\nlabel = \"Galerie\"\nkind = \"repeater\"\n\n[[fields.fields]]\nid = \"field_gallery_image\"\nkey = \"image\"\nlabel = \"Imagine galerie\"\nkind = \"image\"\n",
        )
        .unwrap();
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let section = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section .hero>")
            .unwrap();
        let intent = ProjectTeraInsertIntent {
            target_source_id: Some(section.id.clone()),
            target_location: None,
            target_kind: Some("html".to_string()),
            target_tag: Some("section".to_string()),
            target_selector: Some(".hero".to_string()),
            position: ProjectMovePosition::After,
            item: ProjectTeraInsertItem {
                kind: "teraVariable".to_string(),
                label: Some("Imagine".to_string()),
                target: None,
                name: None,
                expression: Some("frontendul nu este autoritate".to_string()),
                dynamic_binding: Some(ProjectDynamicFieldBinding {
                    model_id: "service".to_string(),
                    field_id: "field_image".to_string(),
                    path: "image".to_string(),
                    scope: "page".to_string(),
                    item_path: None,
                    presentation: "image".to_string(),
                    prefix: String::new(),
                    suffix: String::new(),
                    fallback: "/fallback.jpg".to_string(),
                    text: "Fotografie serviciu".to_string(),
                }),
                dynamic_widget: None,
            },
        };
        let plan = plan_tera_insert(&model, &intent);
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let contents = plan.patch.unwrap().contents;
        assert!(contents.contains("pana:dynamic model=service field=field_image"));
        assert!(contents.contains("page.extra.image | default(value=\"/fallback.jpg\")"));
        assert!(contents.contains("alt=\"Fotografie serviciu\""));
        assert!(!contents.contains("frontendul nu este autoritate"));

        let mut invalid = intent;
        invalid.item.dynamic_binding.as_mut().unwrap().path = "other".to_string();
        let blocked = plan_tera_insert(&model, &invalid);
        assert!(!blocked.allowed);
        assert!(blocked
            .diagnostic
            .unwrap()
            .contains("nu corespunde contractului Rust"));

        let nested_binding = ProjectDynamicFieldBinding {
            model_id: "service".to_string(),
            field_id: "field_gallery_image".to_string(),
            path: "gallery.image".to_string(),
            scope: "item".to_string(),
            item_path: Some("image".to_string()),
            presentation: "image".to_string(),
            prefix: String::new(),
            suffix: String::new(),
            fallback: String::new(),
            text: "Imagine galerie".to_string(),
        };
        let outside_loop = plan_tera_insert(
            &model,
            &ProjectTeraInsertIntent {
                target_source_id: Some(section.id.clone()),
                target_location: None,
                target_kind: Some("html".to_string()),
                target_tag: Some("section".to_string()),
                target_selector: Some(".hero".to_string()),
                position: ProjectMovePosition::After,
                item: ProjectTeraInsertItem {
                    kind: "teraVariable".to_string(),
                    label: Some("Imagine galerie".to_string()),
                    target: None,
                    name: None,
                    expression: None,
                    dynamic_binding: Some(nested_binding.clone()),
                    dynamic_widget: None,
                },
            },
        );
        assert!(!outside_loop.allowed);
        assert!(outside_loop
            .diagnostic
            .unwrap()
            .contains("interiorul unei bucle Tera"));

        let loop_node = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::For)
            .unwrap();
        let inside_loop = plan_tera_insert(
            &model,
            &ProjectTeraInsertIntent {
                target_source_id: Some(loop_node.id.clone()),
                target_location: None,
                target_kind: Some("for".to_string()),
                target_tag: None,
                target_selector: None,
                position: ProjectMovePosition::Inside,
                item: ProjectTeraInsertItem {
                    kind: "teraVariable".to_string(),
                    label: Some("Imagine galerie".to_string()),
                    target: None,
                    name: None,
                    expression: None,
                    dynamic_binding: Some(nested_binding),
                    dynamic_widget: None,
                },
            },
        );
        assert!(inside_loop.allowed, "{:?}", inside_loop.diagnostic);
        assert!(inside_loop
            .patch
            .unwrap()
            .contents
            .contains("src=\"{{ item.image }}\""));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn plan_tera_insert_rejects_contradictory_or_stale_identity_for_html_siblings() {
        let root = unique_test_dir();
        write_project(
            &root,
            concat!(
                "{% block content %}\n",
                "<section class=\"first\"></section>\n",
                "<section class=\"second\"></section>\n",
                "{% endblock %}\n",
            ),
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let first = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section .first>")
            .unwrap();
        let second = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section .second>")
            .unwrap();
        let second_range = second.range.as_ref().expect("section should have range");
        let second_location = ProjectSourceEditLocation {
            file: second.file.clone(),
            line: second_range.line,
            column: second_range.column,
        };

        for target_source_id in [Some(first.id.clone()), Some("stale-source-id".to_string())] {
            let plan = plan_tera_insert(
                &model,
                &ProjectTeraInsertIntent {
                    target_source_id,
                    target_location: Some(second_location.clone()),
                    target_kind: Some("html".to_string()),
                    target_tag: Some("section".to_string()),
                    target_selector: Some(".second".to_string()),
                    position: ProjectMovePosition::Before,
                    item: ProjectTeraInsertItem {
                        kind: "include".to_string(),
                        label: Some("Include Card".to_string()),
                        target: Some("partials/card.html".to_string()),
                        name: None,
                        expression: None,
                        dynamic_binding: None,
                        dynamic_widget: None,
                    },
                },
            );

            assert!(!plan.allowed, "{:?}", plan.diagnostic);
            assert!(plan.patch.is_none());
        }

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn plan_tera_insert_blocks_duplicate_block() {
        let root = unique_test_dir();
        write_project(&root, "{% block content %}<main></main>{% endblock %}\n");
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let main = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<main>")
            .unwrap();

        let plan = plan_tera_insert(
            &model,
            &ProjectTeraInsertIntent {
                target_source_id: Some(main.id.clone()),
                target_location: None,
                target_kind: Some("html".to_string()),
                target_tag: Some("main".to_string()),
                target_selector: Some("main".to_string()),
                position: ProjectMovePosition::Before,
                item: ProjectTeraInsertItem {
                    kind: "block".to_string(),
                    label: Some("Block content".to_string()),
                    target: None,
                    name: Some("content".to_string()),
                    expression: None,
                    dynamic_binding: None,
                    dynamic_widget: None,
                },
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.diagnostic.unwrap().contains("Block-ul content există"));
    }

    #[test]
    fn plan_tera_insert_blocks_duplicate_include_with_tera_equivalent_syntax() {
        let root = unique_test_dir();
        write_project(
            &root,
            "{% block content %}\n{%- include 'partials/card.html' -%}\n<main></main>\n{% endblock %}\n",
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let main = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<main>")
            .unwrap();

        let plan = plan_tera_insert(
            &model,
            &ProjectTeraInsertIntent {
                target_source_id: Some(main.id.clone()),
                target_location: None,
                target_kind: Some("html".to_string()),
                target_tag: Some("main".to_string()),
                target_selector: Some("main".to_string()),
                position: ProjectMovePosition::Before,
                item: ProjectTeraInsertItem {
                    kind: "include".to_string(),
                    label: Some("Include Card".to_string()),
                    target: Some("partials/card.html".to_string()),
                    name: None,
                    expression: None,
                    dynamic_binding: None,
                    dynamic_widget: None,
                },
            },
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
        write_project(&root, "{% block content %}<main></main>{% endblock %}\n");
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let main = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<main>")
            .unwrap();

        let plan = plan_tera_insert(
            &model,
            &ProjectTeraInsertIntent {
                target_source_id: Some(main.id.clone()),
                target_location: None,
                target_kind: Some("html".to_string()),
                target_tag: Some("main".to_string()),
                target_selector: Some("main".to_string()),
                position: ProjectMovePosition::Inside,
                item: ProjectTeraInsertItem {
                    kind: "include".to_string(),
                    label: Some("Include Missing".to_string()),
                    target: Some("partials/missing.html".to_string()),
                    name: None,
                    expression: None,
                    dynamic_binding: None,
                    dynamic_widget: None,
                },
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan
            .diagnostic
            .unwrap()
            .contains("Template-ul țintă nu există"));
    }

    #[test]
    fn plan_tera_insert_blocks_duplicate_import_with_tera_equivalent_syntax() {
        let root = unique_test_dir();
        write_project(
            &root,
            "{%- import 'macros.html' as macros -%}\n{% block content %}<main></main>{% endblock %}\n",
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let content_block = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::Block && node.label == "content")
            .unwrap();

        let plan = plan_tera_insert(
            &model,
            &ProjectTeraInsertIntent {
                target_source_id: Some(content_block.id.clone()),
                target_location: None,
                target_kind: Some("block".to_string()),
                target_tag: None,
                target_selector: Some("content".to_string()),
                position: ProjectMovePosition::Before,
                item: ProjectTeraInsertItem {
                    kind: "import".to_string(),
                    label: Some("Import macros".to_string()),
                    target: Some("macros.html".to_string()),
                    name: Some("macros".to_string()),
                    expression: None,
                    dynamic_binding: None,
                    dynamic_widget: None,
                },
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan
            .diagnostic
            .unwrap()
            .contains("Importul macros.html există deja"));
    }

    #[test]
    fn plan_tera_insert_blocks_block_in_nested_scope() {
        let root = unique_test_dir();
        write_project(
            &root,
            "{% block content %}\n<main></main>\n{% endblock %}\n",
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let main = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<main>")
            .unwrap();

        let plan = plan_tera_insert(
            &model,
            &ProjectTeraInsertIntent {
                target_source_id: Some(main.id.clone()),
                target_location: None,
                target_kind: Some("html".to_string()),
                target_tag: Some("main".to_string()),
                target_selector: Some("main".to_string()),
                position: ProjectMovePosition::Before,
                item: ProjectTeraInsertItem {
                    kind: "block".to_string(),
                    label: Some("Block sidebar".to_string()),
                    target: None,
                    name: Some("sidebar".to_string()),
                    expression: None,
                    dynamic_binding: None,
                    dynamic_widget: None,
                },
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.diagnostic.unwrap().contains("nivel de template"));
    }

    #[test]
    fn plan_tera_insert_blocks_macro_in_nested_scope() {
        let root = unique_test_dir();
        write_project(
            &root,
            "{% block content %}\n<main></main>\n{% endblock %}\n",
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let main = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<main>")
            .unwrap();

        let plan = plan_tera_insert(
            &model,
            &ProjectTeraInsertIntent {
                target_source_id: Some(main.id.clone()),
                target_location: None,
                target_kind: Some("html".to_string()),
                target_tag: Some("main".to_string()),
                target_selector: Some("main".to_string()),
                position: ProjectMovePosition::Before,
                item: ProjectTeraInsertItem {
                    kind: "macro".to_string(),
                    label: Some("Macro card".to_string()),
                    target: None,
                    name: Some("card".to_string()),
                    expression: None,
                    dynamic_binding: None,
                    dynamic_widget: None,
                },
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.diagnostic.unwrap().contains("nivel de template"));
    }

    #[test]
    fn plan_tera_insert_uses_filter_as_a_specialized_anchor() {
        let root = unique_test_dir();
        write_project(
            &root,
            concat!(
                "{% block content %}\n",
                "{% filter upper %}{{ title }}{% endfilter %}\n",
                "{% endblock %}\n",
            ),
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let filter = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::Filter)
            .unwrap();

        let plan = plan_tera_insert(
            &model,
            &ProjectTeraInsertIntent {
                target_source_id: Some(filter.id.clone()),
                target_location: None,
                target_kind: Some("filter".to_string()),
                target_tag: None,
                target_selector: Some(filter.label.clone()),
                position: ProjectMovePosition::Before,
                item: ProjectTeraInsertItem {
                    kind: "include".to_string(),
                    label: Some("Include Card".to_string()),
                    target: Some("partials/card.html".to_string()),
                    name: None,
                    expression: None,
                    dynamic_binding: None,
                    dynamic_widget: None,
                },
            },
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
    fn plan_tera_insert_adds_safe_macro_call_from_catalog_identity() {
        let root = unique_test_dir();
        write_project(&root, "{% block content %}<main></main>{% endblock %}\n");
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let main = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<main>")
            .unwrap();
        let plan = plan_tera_insert(
            &model,
            &ProjectTeraInsertIntent {
                target_source_id: Some(main.id.clone()),
                target_location: None,
                target_kind: Some("html".to_string()),
                target_tag: Some("main".to_string()),
                target_selector: Some("main".to_string()),
                position: ProjectMovePosition::Inside,
                item: ProjectTeraInsertItem {
                    kind: "macroCall".to_string(),
                    label: Some("Card".to_string()),
                    target: Some("macros.html".to_string()),
                    name: Some("card".to_string()),
                    expression: None,
                    dynamic_binding: None,
                    dynamic_widget: None,
                },
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let contents = plan.patch.expect("macro call patch").contents;
        assert!(contents.contains("{% import \"macros.html\" as pana_component %}"));
        assert!(contents.contains("{{ pana_component::card() }}"));
        assert!(
            contents.find("{{ pana_component::card() }}").unwrap()
                < contents.find("</main>").unwrap(),
            "Inserarea Inside trebuie să rămână înaintea tagului HTML de închidere: {contents}"
        );
    }

    fn write_project(root: &PathBuf, template: &str) {
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
        fs::write(root.join("templates/index.html"), template).unwrap();
        fs::write(
            root.join("templates/partials/card.html"),
            "<article></article>\n",
        )
        .unwrap();
        fs::write(root.join("templates/base.html"), "<body></body>\n").unwrap();
        fs::write(
            root.join("templates/macros.html"),
            "{% macro card() %}{% endmacro %}\n",
        )
        .unwrap();
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
