use serde::{Deserialize, Serialize};

use crate::{
    project_model::model::{ProjectModel, ProjectModelFileKind},
    source_graph::model::{SourceNode, SourceNodeKind},
};

use super::move_engine::{
    can_receive_children, content_revision, line_indent_at_offset, line_number_at_offset,
    removal_range_for_span, resolve_conjunctive_anchor, same_model_path, source_location_at_offset,
    ProjectMovePosition, ProjectSourceEditLocation, Span,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTeraMoveIntent {
    pub source_source_id: Option<String>,
    pub target_source_id: Option<String>,
    pub source_location: Option<ProjectSourceEditLocation>,
    pub target_location: Option<ProjectSourceEditLocation>,
    pub source_kind: Option<String>,
    pub target_kind: Option<String>,
    pub source_label: Option<String>,
    pub target_tag: Option<String>,
    pub target_selector: Option<String>,
    pub position: ProjectMovePosition,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTeraMovePlan {
    pub allowed: bool,
    pub diagnostic: Option<String>,
    pub model_revision: String,
    pub patch: Option<ProjectTeraMovePatch>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTeraMovePatch {
    pub file: String,
    pub resolved_source_id: String,
    pub resolved_target_id: String,
    pub moved_label: String,
    pub moved_kind: String,
    pub before_revision: String,
    pub after_revision: String,
    pub contents: String,
    pub source_location: ProjectSourceEditLocation,
    pub target_location: ProjectSourceEditLocation,
    pub source_start_line: usize,
    pub source_end_line: usize,
    pub new_start_line: usize,
}

struct TeraMoveApplication {
    contents: String,
    source_start_line: usize,
    source_end_line: usize,
    new_start_line: usize,
}

pub fn plan_tera_move(model: &ProjectModel, intent: &ProjectTeraMoveIntent) -> ProjectTeraMovePlan {
    match plan_tera_move_inner(model, intent) {
        Ok(patch) => ProjectTeraMovePlan {
            allowed: true,
            diagnostic: None,
            model_revision: model.revision.clone(),
            patch: Some(patch),
        },
        Err(message) => ProjectTeraMovePlan {
            allowed: false,
            diagnostic: Some(message),
            model_revision: model.revision.clone(),
            patch: None,
        },
    }
}

fn plan_tera_move_inner(
    model: &ProjectModel,
    intent: &ProjectTeraMoveIntent,
) -> Result<ProjectTeraMovePatch, String> {
    let source_node = resolve_tera_move_source(model, intent)
        .ok_or_else(|| tera_move_missing_message("sursă", intent))?;
    let target_node = resolve_tera_move_target(model, intent)
        .ok_or_else(|| tera_move_missing_message("destinație", intent))?;

    validate_tera_move_source(source_node)?;
    validate_tera_move_destination(target_node)?;

    if source_node.id == target_node.id {
        return Err("Nodul Tera este deja pe această țintă.".to_string());
    }
    if !same_model_path(&source_node.file, &target_node.file) {
        return Err(
            "Mutarea Tera între fișiere diferite rămâne blocată până există plan de impact."
                .to_string(),
        );
    }
    if source_node.kind == SourceNodeKind::Extends {
        return Err(
            "Extends nu se mută prin drag and drop; poziția lui este politică de template."
                .to_string(),
        );
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
        return Err("Tera Move Engine este activ doar pentru template-uri Zola/Tera.".to_string());
    }

    let source_range = source_node
        .range
        .as_ref()
        .ok_or_else(|| "Sursa Tera nu are range stabil în Source Graph.".to_string())?;
    let target_range = target_node
        .range
        .as_ref()
        .ok_or_else(|| "Destinația Tera nu are range stabil în Source Graph.".to_string())?;
    if source_range.end <= source_range.start || source_range.end > file.contents.len() {
        return Err("Range-ul sursei Tera este invalid pentru sursa curentă.".to_string());
    }
    if target_range.end < target_range.start || target_range.end > file.contents.len() {
        return Err("Range-ul destinației Tera este invalid pentru sursa curentă.".to_string());
    }

    let source_span = tera_source_block_for_move(&file.contents, source_node)?;
    let target_span = Span {
        start: target_range.start,
        end: target_range.end,
    };
    if ranges_overlap(source_span, target_span) {
        return Err("Nu poți muta un nod Tera relativ la propriul conținut.".to_string());
    }
    validate_tera_move_target(model, source_node, target_node, intent)?;

    let source_location =
        source_location_at_offset(&file.contents, &source_node.file, source_span.start);
    let target_location =
        source_location_at_offset(&file.contents, &target_node.file, target_span.start);
    let applied = apply_tera_move(
        &file.contents,
        source_span,
        target_span,
        target_node.kind == SourceNodeKind::Html,
        intent.position,
    )?;

    Ok(ProjectTeraMovePatch {
        file: source_node.file.clone(),
        resolved_source_id: source_node.id.clone(),
        resolved_target_id: target_node.id.clone(),
        moved_label: source_node.label.clone(),
        moved_kind: tera_kind_label(&source_node.kind).to_string(),
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

fn resolve_tera_move_source<'a>(
    model: &'a ProjectModel,
    intent: &ProjectTeraMoveIntent,
) -> Option<&'a SourceNode> {
    let id_node = intent
        .source_source_id
        .as_deref()
        .and_then(|id| resolve_source_node(model, id, intent.source_kind.as_deref()));
    let location_node = intent.source_location.as_ref().and_then(|location| {
        resolve_source_node_at_location(model, location, intent.source_kind.as_deref())
    });

    resolve_conjunctive_anchor(
        intent.source_source_id.as_deref(),
        intent.source_location.as_ref(),
        id_node,
        location_node,
    )
}

fn resolve_tera_move_target<'a>(
    model: &'a ProjectModel,
    intent: &ProjectTeraMoveIntent,
) -> Option<&'a SourceNode> {
    let id_node = intent
        .target_source_id
        .as_deref()
        .and_then(|id| resolve_target_node(model, id, intent.target_kind.as_deref()));
    let location_node = intent.target_location.as_ref().and_then(|location| {
        resolve_target_node_at_location(model, location, intent.target_kind.as_deref())
    });

    resolve_conjunctive_anchor(
        intent.target_source_id.as_deref(),
        intent.target_location.as_ref(),
        id_node,
        location_node,
    )
}

fn resolve_source_node<'a>(
    model: &'a ProjectModel,
    source_id: &str,
    kind: Option<&str>,
) -> Option<&'a SourceNode> {
    model.source_graph.nodes.iter().find(|node| {
        node.id == source_id && is_movable_tera_kind(&node.kind) && node_kind_matches(node, kind)
    })
}

fn resolve_source_node_at_location<'a>(
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
            is_movable_tera_kind(&node.kind)
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

fn resolve_target_node<'a>(
    model: &'a ProjectModel,
    source_id: &str,
    kind: Option<&str>,
) -> Option<&'a SourceNode> {
    model.source_graph.nodes.iter().find(|node| {
        node.id == source_id
            && is_tera_move_anchor_kind(&node.kind)
            && node_kind_matches(node, kind)
    })
}

fn resolve_target_node_at_location<'a>(
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
            is_tera_move_anchor_kind(&node.kind)
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

fn validate_tera_move_source(node: &SourceNode) -> Result<(), String> {
    if node.kind == SourceNodeKind::Tera {
        return Err(
            "Sintaxa Tera nespecializată se mută din cod sau printr-o acțiune dedicată, nu prin drag and drop vizual."
                .to_string(),
        );
    }
    Ok(())
}

fn validate_tera_move_destination(node: &SourceNode) -> Result<(), String> {
    if node.kind == SourceNodeKind::Tera {
        return Err(
            "Sintaxa Tera nespecializată nu este o destinație sigură pentru mutare vizuală."
                .to_string(),
        );
    }
    Ok(())
}

fn validate_tera_move_target(
    model: &ProjectModel,
    source: &SourceNode,
    target: &SourceNode,
    intent: &ProjectTeraMoveIntent,
) -> Result<(), String> {
    if intent.position == ProjectMovePosition::Inside && !can_receive_tera_inside(target, intent) {
        return Err("Această destinație nu poate primi Tera în interior.".to_string());
    }

    let context_kind = target_context_kind(model, target, intent.position);
    if source.kind == SourceNodeKind::Block
        && !matches!(context_kind, Some(SourceNodeKind::Template))
    {
        return Err("Block-urile Tera rămân la nivel de template în DnD sigur.".to_string());
    }
    if matches!(source.kind, SourceNodeKind::Macro | SourceNodeKind::Import)
        && !matches!(
            context_kind,
            Some(SourceNodeKind::Template) | Some(SourceNodeKind::Partial)
        )
    {
        return Err(
            "Macro-urile și importurile Tera rămân la nivel de template în DnD sigur.".to_string(),
        );
    }
    if matches!(context_kind, Some(SourceNodeKind::Macro))
        && matches!(
            source.kind,
            SourceNodeKind::Block | SourceNodeKind::Macro | SourceNodeKind::Extends
        )
    {
        return Err("Macro body nu primește block, macro sau extends prin DnD.".to_string());
    }

    Ok(())
}

fn target_context_kind(
    model: &ProjectModel,
    target: &SourceNode,
    position: ProjectMovePosition,
) -> Option<SourceNodeKind> {
    if position == ProjectMovePosition::Inside {
        return Some(target.kind.clone());
    }
    let parent_id = target.parent.as_deref()?;
    model
        .source_graph
        .nodes
        .iter()
        .find(|node| node.id == parent_id)
        .map(|node| node.kind.clone())
}

fn can_receive_tera_inside(anchor: &SourceNode, intent: &ProjectTeraMoveIntent) -> bool {
    if anchor.kind == SourceNodeKind::Html {
        return intent
            .target_tag
            .as_deref()
            .map(can_receive_children)
            .unwrap_or(false);
    }
    matches!(
        anchor.kind,
        SourceNodeKind::Block
            | SourceNodeKind::Macro
            | SourceNodeKind::For
            | SourceNodeKind::If
            | SourceNodeKind::Filter
            | SourceNodeKind::Tera
    )
}

fn tera_source_block_for_move(source: &str, node: &SourceNode) -> Result<Span, String> {
    let range = node
        .range
        .as_ref()
        .ok_or_else(|| "Nodul Tera nu are range stabil pentru mutare.".to_string())?;
    let span = Span {
        start: range.start,
        end: range.end,
    };
    if span.end <= span.start || span.end > source.len() {
        return Err("Range-ul nodului Tera este invalid pentru mutare.".to_string());
    }
    Ok(removal_range_for_span(source, span))
}

fn apply_tera_move(
    source: &str,
    source_span: Span,
    target_span: Span,
    target_is_html: bool,
    position: ProjectMovePosition,
) -> Result<TeraMoveApplication, String> {
    let removed_length = source_span.end.saturating_sub(source_span.start);
    let moving_source = source
        .get(source_span.start..source_span.end)
        .ok_or_else(|| "Range sursă Tera invalid.".to_string())?
        .trim_end()
        .to_string();
    if moving_source.trim().is_empty() {
        return Err("Nodul Tera de mutat este gol.".to_string());
    }

    let without_source = format!(
        "{}{}",
        &source[..source_span.start],
        &source[source_span.end..]
    );
    let adjust_index = |index: usize| {
        if index > source_span.start {
            index.saturating_sub(removed_length)
        } else {
            index
        }
    };
    let adjusted_target_start = adjust_index(target_span.start);
    let adjusted_target_end = adjust_index(target_span.end);
    if adjusted_target_start > without_source.len() || adjusted_target_end > without_source.len() {
        return Err("Range-ul destinației Tera nu mai este valid după eliminare.".to_string());
    }

    let source_start_line = line_number_at_offset(source, source_span.start);
    let source_end_line = line_number_at_offset(source, source_span.end);
    let target_indent = line_indent_at_offset(&without_source, adjusted_target_start);
    let nested_indent = format!("{target_indent}  ");
    let block = format_moved_tera_snippet(
        &moving_source,
        if position == ProjectMovePosition::Inside {
            &nested_indent
        } else {
            &target_indent
        },
    );

    let insert_index = match position {
        ProjectMovePosition::Before => line_start_index(&without_source, adjusted_target_start),
        ProjectMovePosition::After => line_break_index(&without_source, adjusted_target_end)
            .map(|index| index + 1)
            .unwrap_or(without_source.len()),
        ProjectMovePosition::Inside if target_is_html => adjusted_target_end,
        ProjectMovePosition::Inside => line_break_index(&without_source, adjusted_target_start)
            .map(|index| index + 1)
            .unwrap_or(adjusted_target_end),
    };
    let insertion = source_block_for_insert(&without_source, insert_index, &block);
    let new_start_line = line_number_at_offset(&without_source, insert_index)
        + if insertion.starts_with('\n') { 1 } else { 0 };
    let contents = format!(
        "{}{}{}",
        &without_source[..insert_index],
        insertion,
        &without_source[insert_index..]
    );

    Ok(TeraMoveApplication {
        contents,
        source_start_line,
        source_end_line,
        new_start_line,
    })
}

fn node_kind_matches(node: &SourceNode, kind: Option<&str>) -> bool {
    let Some(kind) = kind.map(str::trim).filter(|kind| !kind.is_empty()) else {
        return true;
    };
    if matches!(kind, "preview" | "empty-tera-slot") {
        return true;
    }
    tera_kind_label(&node.kind) == kind
}

fn is_movable_tera_kind(kind: &SourceNodeKind) -> bool {
    matches!(
        kind,
        SourceNodeKind::Block
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

fn is_tera_move_anchor_kind(kind: &SourceNodeKind) -> bool {
    matches!(
        kind,
        SourceNodeKind::Html
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

fn tera_kind_label(kind: &SourceNodeKind) -> &'static str {
    match kind {
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
        SourceNodeKind::Template => "template",
        SourceNodeKind::Partial => "partial",
        _ => "unsupported",
    }
}

fn ranges_overlap(left: Span, right: Span) -> bool {
    left.start < right.end && right.start < left.end
}

fn line_start_index(source: &str, index: usize) -> usize {
    source
        .get(..index.min(source.len()))
        .and_then(|prefix| prefix.rfind('\n').map(|line| line + 1))
        .unwrap_or(0)
}

fn line_break_index(source: &str, index: usize) -> Option<usize> {
    source
        .get(index.min(source.len())..)?
        .find('\n')
        .map(|relative| index + relative)
}

fn format_moved_tera_snippet(snippet: &str, indent: &str) -> String {
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
    let lines: Vec<&str> = snippet.split('\n').collect();
    let content_lines: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let common_indent_length = content_lines
        .iter()
        .map(|line| {
            line.chars()
                .take_while(|character| *character == ' ' || *character == '\t')
                .count()
        })
        .min()
        .unwrap_or(0);
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

fn tera_move_missing_message(kind: &str, intent: &ProjectTeraMoveIntent) -> String {
    let (id, loc, node_kind) = if kind == "sursă" {
        (
            intent
                .source_source_id
                .as_deref()
                .unwrap_or("fără Source ID"),
            source_location_label(intent.source_location.as_ref()),
            intent.source_kind.as_deref().unwrap_or("fără kind"),
        )
    } else {
        (
            intent
                .target_source_id
                .as_deref()
                .unwrap_or("fără Source ID"),
            source_location_label(intent.target_location.as_ref()),
            intent.target_kind.as_deref().unwrap_or("fără kind"),
        )
    };
    let selector = intent.target_selector.as_deref().unwrap_or("fără selector");
    let source_label = intent.source_label.as_deref().unwrap_or("fără label sursă");
    format!(
        "Nu am putut ancora {kind} Tera în Project Model. Source ID: {id}; locație: {loc}; kind: {node_kind}; selector live: {selector}; sursă: {source_label}."
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
    fn plan_tera_move_moves_include_before_html_anchor() {
        let root = unique_test_dir();
        write_project(
            &root,
            concat!(
                "{% block content %}\n",
                "  {% include \"partials/a.html\" %}\n",
                "  <section class=\"hero\"></section>\n",
                "  {% include \"partials/b.html\" %}\n",
                "{% endblock %}\n",
            ),
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let source = find_node(&model, SourceNodeKind::Include, "partials/b.html");
        let target = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section .hero>")
            .unwrap();

        let plan = plan_tera_move(
            &model,
            &ProjectTeraMoveIntent {
                source_source_id: Some(source.id.clone()),
                target_source_id: Some(target.id.clone()),
                source_location: None,
                target_location: None,
                source_kind: Some("include".to_string()),
                target_kind: Some("html".to_string()),
                source_label: Some(source.label.clone()),
                target_tag: Some("section".to_string()),
                target_selector: Some(".hero".to_string()),
                position: ProjectMovePosition::Before,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.unwrap();
        let moved_before_section =
            "  {% include \"partials/b.html\" %}\n  <section class=\"hero\"></section>";
        assert!(patch.contents.contains(moved_before_section));
        assert_eq!(patch.moved_kind, "include");
    }

    #[test]
    fn plan_tera_move_blocks_move_into_own_scope() {
        let root = unique_test_dir();
        write_project(
            &root,
            concat!(
                "{% block content %}\n",
                "  <section class=\"hero\"></section>\n",
                "{% endblock %}\n",
            ),
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let source = find_node(&model, SourceNodeKind::Block, "content");
        let target = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section .hero>")
            .unwrap();

        let plan = plan_tera_move(
            &model,
            &ProjectTeraMoveIntent {
                source_source_id: Some(source.id.clone()),
                target_source_id: Some(target.id.clone()),
                source_location: None,
                target_location: None,
                source_kind: Some("block".to_string()),
                target_kind: Some("html".to_string()),
                source_label: Some(source.label.clone()),
                target_tag: Some("section".to_string()),
                target_selector: Some(".hero".to_string()),
                position: ProjectMovePosition::Inside,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.diagnostic.unwrap().contains("propriul conținut"));
    }

    #[test]
    fn plan_tera_move_blocks_cross_file_move() {
        let root = unique_test_dir();
        write_project(
            &root,
            concat!(
                "{% block content %}\n",
                "  {% include \"partials/a.html\" %}\n",
                "{% endblock %}\n",
            ),
        );
        fs::write(
            root.join("templates/partials/card.html"),
            "<article class=\"card\"></article>\n",
        )
        .unwrap();
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let source = find_node(&model, SourceNodeKind::Include, "partials/a.html");
        let target = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<article .card>")
            .unwrap();

        let plan = plan_tera_move(
            &model,
            &ProjectTeraMoveIntent {
                source_source_id: Some(source.id.clone()),
                target_source_id: Some(target.id.clone()),
                source_location: None,
                target_location: None,
                source_kind: Some("include".to_string()),
                target_kind: Some("html".to_string()),
                source_label: Some(source.label.clone()),
                target_tag: Some("article".to_string()),
                target_selector: Some(".card".to_string()),
                position: ProjectMovePosition::Before,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.diagnostic.unwrap().contains("fișiere diferite"));
    }

    #[test]
    fn plan_tera_move_blocks_macro_inside_html_anchor() {
        let root = unique_test_dir();
        write_project(
            &root,
            concat!(
                "{% block content %}\n",
                "  <section class=\"hero\"></section>\n",
                "{% endblock %}\n",
                "{% macro card() %}\n",
                "{% endmacro %}\n",
            ),
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let source = find_node(&model, SourceNodeKind::Macro, "card");
        let target = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section .hero>")
            .unwrap();

        let plan = plan_tera_move(
            &model,
            &ProjectTeraMoveIntent {
                source_source_id: Some(source.id.clone()),
                target_source_id: Some(target.id.clone()),
                source_location: None,
                target_location: None,
                source_kind: Some("macro".to_string()),
                target_kind: Some("html".to_string()),
                source_label: Some(source.label.clone()),
                target_tag: Some("section".to_string()),
                target_selector: Some(".hero".to_string()),
                position: ProjectMovePosition::Inside,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.diagnostic.unwrap().contains("nivel de template"));
    }

    #[test]
    fn plan_tera_move_blocks_block_into_nested_scope() {
        let root = unique_test_dir();
        write_project(
            &root,
            concat!(
                "{% block content %}\n",
                "  <section class=\"hero\"></section>\n",
                "{% endblock %}\n",
                "{% block sidebar %}\n",
                "  <aside></aside>\n",
                "{% endblock %}\n",
            ),
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let source = find_node(&model, SourceNodeKind::Block, "sidebar");
        let target = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section .hero>")
            .unwrap();

        let plan = plan_tera_move(
            &model,
            &ProjectTeraMoveIntent {
                source_source_id: Some(source.id.clone()),
                target_source_id: Some(target.id.clone()),
                source_location: None,
                target_location: None,
                source_kind: Some("block".to_string()),
                target_kind: Some("html".to_string()),
                source_label: Some(source.label.clone()),
                target_tag: Some("section".to_string()),
                target_selector: Some(".hero".to_string()),
                position: ProjectMovePosition::Before,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.diagnostic.unwrap().contains("nivel de template"));
    }

    #[test]
    fn plan_tera_move_handles_filter_as_a_specialized_source() {
        let root = unique_test_dir();
        write_project(
            &root,
            concat!(
                "{% block content %}\n",
                "  {% filter upper %}{{ title }}{% endfilter %}\n",
                "  <section class=\"hero\"></section>\n",
                "{% endblock %}\n",
            ),
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let source = find_node(&model, SourceNodeKind::Filter, "filter");
        let target = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section .hero>")
            .unwrap();

        let plan = plan_tera_move(
            &model,
            &ProjectTeraMoveIntent {
                source_source_id: Some(source.id.clone()),
                target_source_id: Some(target.id.clone()),
                source_location: None,
                target_location: None,
                source_kind: Some("filter".to_string()),
                target_kind: Some("html".to_string()),
                source_label: Some(source.label.clone()),
                target_tag: Some("section".to_string()),
                target_selector: Some(".hero".to_string()),
                position: ProjectMovePosition::Before,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        assert!(plan.patch.is_some());
    }

    #[test]
    fn plan_tera_move_handles_filter_as_a_specialized_destination() {
        let root = unique_test_dir();
        write_project(
            &root,
            concat!(
                "{% block content %}\n",
                "  {% include \"partials/a.html\" %}\n",
                "  {% filter upper %}{{ title }}{% endfilter %}\n",
                "{% endblock %}\n",
            ),
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let source = find_node(&model, SourceNodeKind::Include, "partials/a.html");
        let target = find_node(&model, SourceNodeKind::Filter, "filter");

        let plan = plan_tera_move(
            &model,
            &ProjectTeraMoveIntent {
                source_source_id: Some(source.id.clone()),
                target_source_id: Some(target.id.clone()),
                source_location: None,
                target_location: None,
                source_kind: Some("include".to_string()),
                target_kind: Some("filter".to_string()),
                source_label: Some(source.label.clone()),
                target_tag: None,
                target_selector: None,
                position: ProjectMovePosition::Before,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        assert!(plan.patch.is_some());
    }

    #[test]
    fn plan_tera_move_rejects_contradictory_or_stale_identity_for_same_kind_siblings() {
        let root = unique_test_dir();
        write_project(
            &root,
            concat!(
                "{% block content %}\n",
                "  {% include \"partials/a.html\" %}\n",
                "  {% include \"partials/b.html\" %}\n",
                "  <section class=\"target\"></section>\n",
                "{% endblock %}\n",
            ),
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let first = find_node(&model, SourceNodeKind::Include, "partials/a.html");
        let second = find_node(&model, SourceNodeKind::Include, "partials/b.html");
        let target = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section .target>")
            .unwrap();
        let second_location = node_location(second);

        for source_source_id in [Some(first.id.clone()), Some("stale-source-id".to_string())] {
            let plan = plan_tera_move(
                &model,
                &ProjectTeraMoveIntent {
                    source_source_id,
                    target_source_id: Some(target.id.clone()),
                    source_location: Some(second_location.clone()),
                    target_location: None,
                    source_kind: Some("include".to_string()),
                    target_kind: Some("html".to_string()),
                    source_label: Some(second.label.clone()),
                    target_tag: Some("section".to_string()),
                    target_selector: Some(".target".to_string()),
                    position: ProjectMovePosition::Before,
                },
            );

            assert!(!plan.allowed, "{:?}", plan.diagnostic);
            assert!(plan.patch.is_none());
        }

        let mut wrong_column = node_location(first);
        wrong_column.column += 1;
        let exact_column_plan = plan_tera_move(
            &model,
            &ProjectTeraMoveIntent {
                source_source_id: None,
                target_source_id: Some(target.id.clone()),
                source_location: Some(wrong_column),
                target_location: None,
                source_kind: Some("include".to_string()),
                target_kind: Some("html".to_string()),
                source_label: Some(first.label.clone()),
                target_tag: Some("section".to_string()),
                target_selector: Some(".target".to_string()),
                position: ProjectMovePosition::Before,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!exact_column_plan.allowed);
        assert!(exact_column_plan.patch.is_none());
    }

    fn find_node<'a>(
        model: &'a ProjectModel,
        kind: SourceNodeKind,
        label_contains: &str,
    ) -> &'a SourceNode {
        model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == kind && node.label.contains(label_contains))
            .unwrap()
    }

    fn node_location(node: &SourceNode) -> ProjectSourceEditLocation {
        let range = node.range.as_ref().expect("source node should have range");
        ProjectSourceEditLocation {
            file: node.file.clone(),
            line: range.line,
            column: range.column,
        }
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
        fs::write(root.join("templates/partials/a.html"), "<p>A</p>\n").unwrap();
        fs::write(root.join("templates/partials/b.html"), "<p>B</p>\n").unwrap();
        fs::write(root.join("templates/base.html"), "<body></body>\n").unwrap();
    }

    fn unique_test_dir() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "pana-studio-tera-move-engine-{}-{stamp}",
            std::process::id()
        ))
    }
}
