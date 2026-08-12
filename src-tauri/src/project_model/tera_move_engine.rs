use serde::{Deserialize, Serialize};

use crate::{
    project_model::model::{ProjectModel, ProjectModelFileKind},
    source_graph::model::{SourceNode, SourceNodeKind},
};

use super::move_engine::{
    can_receive_children, content_revision, inside_prefix_for_insert, line_number_at_offset,
    parse_html_tag_at, removal_range_for_span, same_model_path, source_location_at_offset,
    ProjectMovePosition, ProjectSourceEditLocation, Span,
};
use super::structural_edit::{format_tera_fragment, StructuralPlacement};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTeraMoveIntent {
    pub source_source_id: Option<String>,
    pub target_source_id: Option<String>,
    pub source_kind: Option<String>,
    pub target_kind: Option<String>,
    pub source_label: Option<String>,
    pub target_tag: Option<String>,
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
    pub target_label: String,
    pub target_kind: String,
    pub position: ProjectMovePosition,
    pub expected_child_index: Option<usize>,
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

struct TeraMoveApplication {
    contents: String,
    source_start_line: usize,
    source_end_line: usize,
    new_start_line: usize,
    target_start_line: usize,
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
    let placement = if target_node.kind == SourceNodeKind::Html {
        StructuralPlacement::for_html_target(model, &file.contents, target_node)
    } else {
        StructuralPlacement::for_direct_target(&file.contents, target_range.start)
    };
    let applied = apply_tera_move(
        &file.contents,
        source_span,
        target_span,
        target_node.kind == SourceNodeKind::Html,
        intent.position,
        &placement,
    )?;

    Ok(ProjectTeraMovePatch {
        file: source_node.file.clone(),
        resolved_source_id: source_node.id.clone(),
        resolved_target_id: target_node.id.clone(),
        moved_label: source_node.label.clone(),
        moved_kind: tera_kind_label(&source_node.kind).to_string(),
        target_label: target_node.label.clone(),
        target_kind: tera_kind_label(&target_node.kind).to_string(),
        position: intent.position,
        expected_child_index: (intent.position == ProjectMovePosition::Inside).then_some(
            if target_node.kind == SourceNodeKind::Html {
                target_node.children.len()
            } else {
                0
            },
        ),
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

fn resolve_tera_move_source<'a>(
    model: &'a ProjectModel,
    intent: &ProjectTeraMoveIntent,
) -> Option<&'a SourceNode> {
    intent
        .source_source_id
        .as_deref()
        .and_then(|id| resolve_source_node(model, id, intent.source_kind.as_deref()))
}

fn resolve_tera_move_target<'a>(
    model: &'a ProjectModel,
    intent: &ProjectTeraMoveIntent,
) -> Option<&'a SourceNode> {
    intent
        .target_source_id
        .as_deref()
        .and_then(|id| resolve_target_node(model, id, intent.target_kind.as_deref()))
}

fn resolve_source_node<'a>(
    model: &'a ProjectModel,
    source_id: &str,
    kind: Option<&str>,
) -> Option<&'a SourceNode> {
    model
        .source_graph
        .node_by_id(source_id)
        .filter(|node| is_movable_tera_kind(&node.kind) && node_kind_matches(node, kind))
}

fn resolve_target_node<'a>(
    model: &'a ProjectModel,
    source_id: &str,
    kind: Option<&str>,
) -> Option<&'a SourceNode> {
    model
        .source_graph
        .node_by_id(source_id)
        .filter(|node| is_tera_move_anchor_kind(&node.kind) && node_kind_matches(node, kind))
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
        .node_by_id(parent_id)
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
    placement: &StructuralPlacement,
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
    let target_indent = placement.indent.as_str();
    let nested_indent = placement.child_indent();
    let formatted = format_tera_fragment(
        &moving_source,
        if position == ProjectMovePosition::Inside {
            &nested_indent
        } else {
            target_indent
        },
        &placement.style,
    )?;
    if position == ProjectMovePosition::Inside && target_is_html {
        let insert_index =
            html_inside_insert_index(&without_source, adjusted_target_start, adjusted_target_end)?;
        let opening =
            parse_html_tag_at(&without_source, adjusted_target_start).ok_or_else(|| {
                "Ancora HTML nu mai indică un tag stabil pentru mutarea Tera.".to_string()
            })?;
        let before_insert = inside_prefix_for_insert(&without_source, opening.end, insert_index);
        let inserted_fragment_offset = before_insert.len() + placement.style.line_ending().len();
        let contents = format!(
            "{}{}{}{}{}{}",
            before_insert,
            placement.style.line_ending(),
            formatted,
            placement.style.line_ending(),
            target_indent,
            &without_source[insert_index..]
        );
        return Ok(TeraMoveApplication {
            target_start_line: line_number_at_offset(&contents, adjusted_target_start),
            new_start_line: line_number_at_offset(&contents, inserted_fragment_offset),
            contents,
            source_start_line,
            source_end_line,
        });
    }

    let block = format!("{formatted}{}", placement.style.line_ending());

    let insert_index = match position {
        ProjectMovePosition::Before => line_start_index(&without_source, adjusted_target_start),
        ProjectMovePosition::After => line_break_index(&without_source, adjusted_target_end)
            .map(|index| index + 1)
            .unwrap_or(without_source.len()),
        ProjectMovePosition::Inside if target_is_html => unreachable!("handled above"),
        ProjectMovePosition::Inside => line_break_index(&without_source, adjusted_target_start)
            .map(|index| index + 1)
            .unwrap_or(adjusted_target_end),
    };
    let insertion = source_block_for_insert(
        &without_source,
        insert_index,
        &block,
        placement.style.line_ending(),
    );
    let inserted_fragment_offset = insert_index + insertion.len().saturating_sub(block.len());
    let target_offset = if insert_index <= adjusted_target_start {
        adjusted_target_start + insertion.len()
    } else {
        adjusted_target_start
    };
    let contents = format!(
        "{}{}{}",
        &without_source[..insert_index],
        insertion,
        &without_source[insert_index..]
    );
    let new_start_line = line_number_at_offset(&contents, inserted_fragment_offset);
    let target_start_line = line_number_at_offset(&contents, target_offset);

    Ok(TeraMoveApplication {
        contents,
        source_start_line,
        source_end_line,
        new_start_line,
        target_start_line,
    })
}

fn html_inside_insert_index(source: &str, start: usize, end: usize) -> Result<usize, String> {
    let opening = parse_html_tag_at(source, start).ok_or_else(|| {
        "Ancora HTML nu mai indică un tag stabil pentru mutarea Tera.".to_string()
    })?;
    if opening.is_closing || opening.is_self_closing {
        return Err("Ancora HTML nu poate primi conținut Tera în interior.".to_string());
    }
    let element = source
        .get(start..end)
        .ok_or_else(|| "Range-ul ancorei HTML este invalid pentru mutarea Tera.".to_string())?;
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

fn node_kind_matches(node: &SourceNode, kind: Option<&str>) -> bool {
    let Some(kind) = kind.map(str::trim).filter(|kind| !kind.is_empty()) else {
        return true;
    };
    if matches!(kind, "preview" | "empty-tera-slot" | "active-document-root") {
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

fn source_block_for_insert(source: &str, index: usize, block: &str, line_ending: &str) -> String {
    if index > 0 && source.as_bytes().get(index - 1) != Some(&b'\n') {
        format!("{line_ending}{block}")
    } else {
        block.to_string()
    }
}

fn tera_move_missing_message(kind: &str, intent: &ProjectTeraMoveIntent) -> String {
    let (id, node_kind) = if kind == "sursă" {
        (
            intent
                .source_source_id
                .as_deref()
                .unwrap_or("fără Source ID"),
            intent.source_kind.as_deref().unwrap_or("fără kind"),
        )
    } else {
        (
            intent
                .target_source_id
                .as_deref()
                .unwrap_or("fără Source ID"),
            intent.target_kind.as_deref().unwrap_or("fără kind"),
        )
    };
    let source_label = intent.source_label.as_deref().unwrap_or("fără label sursă");
    format!(
        "Nu am putut ancora {kind} Tera în Project Model. SourceNodeId: {id}; kind: {node_kind}; sursă: {source_label}."
    )
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
    fn plan_tera_move_moves_include_before_html_anchor() {
        let root = unique_test_dir();
        let fixture = project_fixture(
            root.clone(),
            concat!(
                "{% block content %}\n",
                "  {% include \"partials/a.html\" %}\n",
                "  <section class=\"hero\"></section>\n",
                "  {% include \"partials/b.html\" %}\n",
                "{% endblock %}\n",
            ),
        );
        let model = fixture.build_model().unwrap();
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
                source_kind: Some("include".to_string()),
                target_kind: Some("html".to_string()),
                source_label: Some(source.label.clone()),
                target_tag: Some("section".to_string()),
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
        let fixture = project_fixture(
            root.clone(),
            concat!(
                "{% block content %}\n",
                "  <section class=\"hero\"></section>\n",
                "{% endblock %}\n",
            ),
        );
        let model = fixture.build_model().unwrap();
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
                source_kind: Some("block".to_string()),
                target_kind: Some("html".to_string()),
                source_label: Some(source.label.clone()),
                target_tag: Some("section".to_string()),
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
        let mut fixture = project_fixture(
            root.clone(),
            concat!(
                "{% block content %}\n",
                "  {% include \"partials/a.html\" %}\n",
                "{% endblock %}\n",
            ),
        );
        fixture.source(
            "templates/partials/card.html",
            "<article class=\"card\"></article>\n",
        );
        let model = fixture.build_model().unwrap();
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
                source_kind: Some("include".to_string()),
                target_kind: Some("html".to_string()),
                source_label: Some(source.label.clone()),
                target_tag: Some("article".to_string()),
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
        let fixture = project_fixture(
            root.clone(),
            concat!(
                "{% block content %}\n",
                "  <section class=\"hero\"></section>\n",
                "{% endblock %}\n",
                "{% macro card() %}\n",
                "{% endmacro %}\n",
            ),
        );
        let model = fixture.build_model().unwrap();
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
                source_kind: Some("macro".to_string()),
                target_kind: Some("html".to_string()),
                source_label: Some(source.label.clone()),
                target_tag: Some("section".to_string()),
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
        let fixture = project_fixture(
            root.clone(),
            concat!(
                "{% block content %}\n",
                "  <section class=\"hero\"></section>\n",
                "{% endblock %}\n",
                "{% block sidebar %}\n",
                "  <aside></aside>\n",
                "{% endblock %}\n",
            ),
        );
        let model = fixture.build_model().unwrap();
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
                source_kind: Some("block".to_string()),
                target_kind: Some("html".to_string()),
                source_label: Some(source.label.clone()),
                target_tag: Some("section".to_string()),
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
        let fixture = project_fixture(
            root.clone(),
            concat!(
                "{% block content %}\n",
                "  {% filter upper %}{{ title }}{% endfilter %}\n",
                "  <section class=\"hero\"></section>\n",
                "{% endblock %}\n",
            ),
        );
        let model = fixture.build_model().unwrap();
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
                source_kind: Some("filter".to_string()),
                target_kind: Some("html".to_string()),
                source_label: Some(source.label.clone()),
                target_tag: Some("section".to_string()),
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
        let fixture = project_fixture(
            root.clone(),
            concat!(
                "{% block content %}\n",
                "  {% include \"partials/a.html\" %}\n",
                "  {% filter upper %}{{ title }}{% endfilter %}\n",
                "{% endblock %}\n",
            ),
        );
        let model = fixture.build_model().unwrap();
        let source = find_node(&model, SourceNodeKind::Include, "partials/a.html");
        let target = find_node(&model, SourceNodeKind::Filter, "filter");

        let plan = plan_tera_move(
            &model,
            &ProjectTeraMoveIntent {
                source_source_id: Some(source.id.clone()),
                target_source_id: Some(target.id.clone()),
                source_kind: Some("include".to_string()),
                target_kind: Some("filter".to_string()),
                source_label: Some(source.label.clone()),
                target_tag: None,
                position: ProjectMovePosition::Before,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        assert!(plan.patch.is_some());
    }

    #[test]
    fn plan_tera_move_uses_only_exact_source_id_and_rejects_stale_identity() {
        let root = unique_test_dir();
        let fixture = project_fixture(
            root.clone(),
            concat!(
                "{% block content %}\n",
                "  {% include \"partials/a.html\" %}\n",
                "  {% include \"partials/b.html\" %}\n",
                "  <section class=\"target\"></section>\n",
                "{% endblock %}\n",
            ),
        );
        let model = fixture.build_model().unwrap();
        let first = find_node(&model, SourceNodeKind::Include, "partials/a.html");
        let second = find_node(&model, SourceNodeKind::Include, "partials/b.html");
        let target = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section .target>")
            .unwrap();
        let exact = plan_tera_move(
            &model,
            &ProjectTeraMoveIntent {
                source_source_id: Some(first.id.clone()),
                target_source_id: Some(target.id.clone()),
                source_kind: Some("include".to_string()),
                target_kind: Some("html".to_string()),
                source_label: Some(second.label.clone()),
                target_tag: Some("section".to_string()),
                position: ProjectMovePosition::Before,
            },
        );
        assert!(exact.allowed, "{:?}", exact.diagnostic);
        assert_eq!(
            exact
                .patch
                .expect("exact SourceNodeId move patch")
                .resolved_source_id,
            first.id
        );

        let stale = plan_tera_move(
            &model,
            &ProjectTeraMoveIntent {
                source_source_id: Some("stale-source-id".to_string()),
                target_source_id: Some(target.id.clone()),
                source_kind: Some("include".to_string()),
                target_kind: Some("html".to_string()),
                source_label: Some(second.label.clone()),
                target_tag: Some("section".to_string()),
                position: ProjectMovePosition::Before,
            },
        );
        assert!(!stale.allowed, "{:?}", stale.diagnostic);
        assert!(stale.patch.is_none());

        let missing_source_id = plan_tera_move(
            &model,
            &ProjectTeraMoveIntent {
                source_source_id: None,
                target_source_id: Some(target.id.clone()),
                source_kind: Some("include".to_string()),
                target_kind: Some("html".to_string()),
                source_label: Some(first.label.clone()),
                target_tag: Some("section".to_string()),
                position: ProjectMovePosition::Before,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!missing_source_id.allowed);
        assert!(missing_source_id.patch.is_none());
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

    fn project_fixture(root: PathBuf, template: &str) -> ProjectModelTestFixture {
        let mut fixture = ProjectModelTestFixture::standard_zola(root, template).unwrap();
        fixture.source("templates/partials/a.html", "<p>A</p>\n");
        fixture.source("templates/partials/b.html", "<p>B</p>\n");
        fixture.source("templates/base.html", "<body></body>\n");
        fixture
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
