use serde::{Deserialize, Serialize};

use crate::{
    project_model::model::{ProjectModel, ProjectModelFileKind},
    source_graph::model::{SourceGraphTemplate, SourceNode, SourceNodeKind},
};

use super::move_engine::{
    can_receive_children, content_revision, line_indent_at_offset, line_number_at_offset,
    resolve_conjunctive_anchor, same_model_path, source_location_at_offset, ProjectMovePosition,
    ProjectSourceEditLocation,
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
    match plan_tera_insert_inner(model, intent) {
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
) -> Result<ProjectTeraInsertPatch, String> {
    let target_node = resolve_tera_insert_anchor(model, intent)
        .ok_or_else(|| tera_anchor_missing_message(intent))?;
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
    let target_location =
        source_location_at_offset(&file.contents, &target_node.file, target_range.start);
    let applied = apply_tera_insert(
        &file.contents,
        &target_node.file,
        target_range.start,
        target_range.end,
        target_node.kind == SourceNodeKind::Html,
        intent.position,
        &snippet,
    )?;

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
    if matches!(kind, "extends" | "include" | "import") {
        let fallback = if kind == "include" {
            "partials/cta.html"
        } else if kind == "import" {
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
) -> Result<TeraInsertApplication, String> {
    let anchor_start_line_start = line_start_index(source, start);
    let anchor_start_line_break = line_break_index(source, start);
    let anchor_end_line_break = line_break_index(source, end);
    let insert_indent = line_indent_at_offset(source, start);
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
        ProjectMovePosition::Inside if anchor_is_html => end,
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

fn build_tera_insert_snippet(
    model: &ProjectModel,
    item: &ProjectTeraInsertItem,
) -> Result<String, String> {
    let kind = tera_item_kind(&item.kind);
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
        "with" => {
            let expression = sanitize_tera_expression(
                item.expression.as_deref().unwrap_or("value = value"),
                "value = value",
            )?;
            format!("{{% with {expression} %}}\n{{% endwith %}}")
        }
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

    if matches!(kind, "extends" | "include" | "import") {
        let target = if kind == "include" {
            item.target.as_deref().unwrap_or("partials/cta.html")
        } else if kind == "import" {
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
            | SourceNodeKind::With
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
            | SourceNodeKind::With
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
        SourceNodeKind::Set => "set",
        SourceNodeKind::With => "with",
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
        "for" => "for",
        "if" => "if",
        "set" => "set",
        "with" => "with",
        "teraVariable" => "teraVariable",
        "teraComment" => "teraComment",
        "raw" => "raw",
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
            | "for"
            | "if"
            | "set"
            | "with"
            | "teraVariable"
            | "teraComment"
            | "raw"
    )
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
                format!("{indent}{}", line.trim())
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
                },
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.diagnostic.unwrap().contains("nivel de template"));
    }

    #[test]
    fn plan_tera_insert_blocks_unspecialized_tera_anchor() {
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
            .find(|node| node.kind == SourceNodeKind::Tera && node.label.contains("filter"))
            .unwrap();

        let plan = plan_tera_insert(
            &model,
            &ProjectTeraInsertIntent {
                target_source_id: Some(filter.id.clone()),
                target_location: None,
                target_kind: Some("tera".to_string()),
                target_tag: None,
                target_selector: Some(filter.label.clone()),
                position: ProjectMovePosition::Before,
                item: ProjectTeraInsertItem {
                    kind: "include".to_string(),
                    label: Some("Include Card".to_string()),
                    target: Some("partials/card.html".to_string()),
                    name: None,
                    expression: None,
                },
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.diagnostic.unwrap().contains("ancoră sigură"));
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
