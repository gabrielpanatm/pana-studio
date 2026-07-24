use serde::{Deserialize, Serialize};

use crate::{
    project_model::model::{ProjectModel, ProjectModelFileKind},
    source_graph::model::{SourceNode, SourceNodeKind},
};

use super::move_engine::{
    content_revision, line_number_at_offset, removal_range_for_span, resolve_conjunctive_anchor,
    same_model_path, source_location_at_offset, ProjectSourceEditLocation, Span,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTeraDeleteIntent {
    pub target_source_id: Option<String>,
    pub target_location: Option<ProjectSourceEditLocation>,
    pub target_kind: Option<String>,
    pub target_label: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTeraDeletePlan {
    pub allowed: bool,
    pub diagnostic: Option<String>,
    pub model_revision: String,
    pub patch: Option<ProjectTeraDeletePatch>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTeraDeletePatch {
    pub file: String,
    pub resolved_target_id: String,
    pub deleted_label: String,
    pub deleted_kind: String,
    pub before_revision: String,
    pub after_revision: String,
    pub contents: String,
    pub target_location: ProjectSourceEditLocation,
    pub source_start_line: usize,
    pub source_end_line: usize,
    pub line_shift_start: usize,
    pub line_shift: isize,
}

pub fn plan_tera_delete(
    model: &ProjectModel,
    intent: &ProjectTeraDeleteIntent,
) -> ProjectTeraDeletePlan {
    match plan_tera_delete_inner(model, intent) {
        Ok(patch) => ProjectTeraDeletePlan {
            allowed: true,
            diagnostic: None,
            model_revision: model.revision.clone(),
            patch: Some(patch),
        },
        Err(message) => ProjectTeraDeletePlan {
            allowed: false,
            diagnostic: Some(message),
            model_revision: model.revision.clone(),
            patch: None,
        },
    }
}

fn plan_tera_delete_inner(
    model: &ProjectModel,
    intent: &ProjectTeraDeleteIntent,
) -> Result<ProjectTeraDeletePatch, String> {
    let target_node = resolve_tera_node_for_anchor(model, intent)
        .ok_or_else(|| tera_source_missing_message(intent))?;
    validate_tera_delete_target(target_node)?;

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
            "Tera Delete Engine este activ doar pentru template-uri Zola/Tera.".to_string(),
        );
    }

    let target_range = target_node
        .range
        .as_ref()
        .ok_or_else(|| "Nodul Tera nu are range stabil în Source Graph.".to_string())?;
    let span = Span {
        start: target_range.start,
        end: target_range.end,
    };
    if span.end <= span.start || span.end > file.contents.len() {
        return Err("Range-ul nodului Tera este invalid pentru sursa curentă.".to_string());
    }
    let removal = removal_range_for_span(&file.contents, span);
    let removed_text = file
        .contents
        .get(removal.start..removal.end)
        .ok_or_else(|| "Range-ul de ștergere Tera este invalid.".to_string())?;
    let removed_lines = removed_text.bytes().filter(|byte| *byte == b'\n').count() as isize;
    let target_location = source_location_at_offset(&file.contents, &target_node.file, span.start);
    let source_start_line = line_number_at_offset(&file.contents, span.start);
    let source_end_line = line_number_at_offset(&file.contents, span.end);
    let contents = format!(
        "{}{}",
        &file.contents[..removal.start],
        &file.contents[removal.end..]
    );

    Ok(ProjectTeraDeletePatch {
        file: target_node.file.clone(),
        resolved_target_id: target_node.id.clone(),
        deleted_label: target_node.label.clone(),
        deleted_kind: tera_kind_label(&target_node.kind).to_string(),
        before_revision: file.revision.clone(),
        after_revision: content_revision(&contents),
        contents,
        target_location,
        source_start_line,
        source_end_line,
        line_shift_start: source_end_line + 1,
        line_shift: -removed_lines,
    })
}

fn resolve_tera_node_for_anchor<'a>(
    model: &'a ProjectModel,
    intent: &ProjectTeraDeleteIntent,
) -> Option<&'a SourceNode> {
    let id_node = intent
        .target_source_id
        .as_deref()
        .and_then(|id| resolve_tera_node(model, id, intent.target_kind.as_deref()));
    let location_node = intent.target_location.as_ref().and_then(|location| {
        resolve_tera_node_at_location(model, location, intent.target_kind.as_deref())
    });

    resolve_conjunctive_anchor(
        intent.target_source_id.as_deref(),
        intent.target_location.as_ref(),
        id_node,
        location_node,
    )
}

fn resolve_tera_node<'a>(
    model: &'a ProjectModel,
    source_id: &str,
    kind: Option<&str>,
) -> Option<&'a SourceNode> {
    model.source_graph.nodes.iter().find(|node| {
        node.id == source_id
            && is_tera_delete_anchor_kind(&node.kind)
            && node_kind_matches(node, kind)
    })
}

fn resolve_tera_node_at_location<'a>(
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
            is_tera_delete_anchor_kind(&node.kind)
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

fn validate_tera_delete_target(node: &SourceNode) -> Result<(), String> {
    if is_template_level_tera_kind(&node.kind) {
        return Err(
            "Directivele Tera de nivel template se șterg din cod sau prin acțiuni dedicate, nu prin delete vizual."
                .to_string(),
        );
    }
    if node.kind == SourceNodeKind::Tera {
        return Err(
            "Sintaxa Tera nespecializată se editează din cod sau printr-o acțiune dedicată, nu prin delete vizual."
                .to_string(),
        );
    }
    if node.kind == SourceNodeKind::Raw {
        return Err(
            "Blocurile raw Tera sunt scope-uri code-only și se editează din cod sau printr-o acțiune dedicată, nu prin delete vizual."
                .to_string(),
        );
    }
    Ok(())
}

fn is_template_level_tera_kind(kind: &SourceNodeKind) -> bool {
    matches!(
        kind,
        SourceNodeKind::Extends
            | SourceNodeKind::Block
            | SourceNodeKind::Import
            | SourceNodeKind::Macro
    )
}

fn is_tera_delete_anchor_kind(kind: &SourceNodeKind) -> bool {
    matches!(
        kind,
        SourceNodeKind::Extends
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
    tera_kind_label(&node.kind) == kind
}

fn tera_kind_label(kind: &SourceNodeKind) -> &'static str {
    match kind {
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

fn source_location_label(location: Option<&ProjectSourceEditLocation>) -> String {
    match location {
        Some(location) if location.column > 0 => {
            format!("{}:{}:{}", location.file, location.line, location.column)
        }
        Some(location) => format!("{}:{}", location.file, location.line),
        None => "fără locație".to_string(),
    }
}

fn tera_source_missing_message(intent: &ProjectTeraDeleteIntent) -> String {
    let id = intent
        .target_source_id
        .as_deref()
        .unwrap_or("fără Source ID");
    let loc = source_location_label(intent.target_location.as_ref());
    let kind = intent.target_kind.as_deref().unwrap_or("fără kind");
    let label = intent.target_label.as_deref().unwrap_or("fără label");
    format!(
        "Nu am putut ancora nodul Tera în Project Model. Source ID: {id}; locație: {loc}; kind: {kind}; label: {label}."
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

    use crate::{project_model::build_project_model, source_graph::model::SourceNodeKind};

    use super::*;

    #[test]
    fn plan_tera_delete_removes_include_line() {
        let root = unique_test_dir();
        write_project(
            &root,
            concat!(
                "{% block content %}\n",
                "<main>\n",
                "  {% include \"partials/card.html\" %}\n",
                "</main>\n",
                "{% endblock %}\n",
            ),
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let partial_path = root.join("templates/partials/card.html");
        let partial_before = fs::read_to_string(&partial_path).unwrap();
        let include = tera_node(
            &model,
            SourceNodeKind::Include,
            "include partials/card.html",
        );

        let plan = plan_tera_delete(
            &model,
            &ProjectTeraDeleteIntent {
                target_source_id: Some(include.id.clone()),
                target_location: None,
                target_kind: Some("include".to_string()),
                target_label: Some(include.label.clone()),
            },
        );

        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.unwrap();
        assert!(!patch.contents.contains("partials/card.html"));
        assert_eq!(patch.deleted_kind, "include");
        assert_eq!(patch.source_start_line, 3);
        assert_eq!(patch.source_end_line, 3);
        assert_eq!(patch.line_shift, -1);
        assert_eq!(fs::read_to_string(partial_path).unwrap(), partial_before);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn plan_tera_delete_removes_loop_with_set_prelude_from_source_graph_range() {
        let root = unique_test_dir();
        write_project(
            &root,
            concat!(
                "{% block content %}\n",
                "{% set cards = section.pages %}\n",
                "{% for card in cards %}\n",
                "<article>{{ card.title }}</article>\n",
                "{% endfor %}\n",
                "{% endblock %}\n",
            ),
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let loop_node = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::For)
            .unwrap();

        let plan = plan_tera_delete(
            &model,
            &ProjectTeraDeleteIntent {
                target_source_id: Some(loop_node.id.clone()),
                target_location: Some(ProjectSourceEditLocation {
                    file: loop_node.file.clone(),
                    line: loop_node.range.as_ref().unwrap().line,
                    column: loop_node.range.as_ref().unwrap().column,
                }),
                target_kind: Some("for".to_string()),
                target_label: Some(loop_node.label.clone()),
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.unwrap();
        assert!(!patch.contents.contains("{% set cards"));
        assert!(!patch.contents.contains("{% for card"));
        assert!(!patch.contents.contains("{% endfor"));
    }

    #[test]
    fn plan_tera_delete_rejects_contradictory_or_stale_identity_for_include_siblings() {
        let root = unique_test_dir();
        write_project(
            &root,
            concat!(
                "{% block content %}\n",
                "{% include \"partials/a.html\" %}\n",
                "{% include \"partials/b.html\" %}\n",
                "{% endblock %}\n",
            ),
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let first = tera_node(&model, SourceNodeKind::Include, "include partials/a.html");
        let second = tera_node(&model, SourceNodeKind::Include, "include partials/b.html");
        let second_range = second.range.as_ref().expect("include should have range");
        let second_location = ProjectSourceEditLocation {
            file: second.file.clone(),
            line: second_range.line,
            column: second_range.column,
        };

        for target_source_id in [Some(first.id.clone()), Some("stale-source-id".to_string())] {
            let plan = plan_tera_delete(
                &model,
                &ProjectTeraDeleteIntent {
                    target_source_id,
                    target_location: Some(second_location.clone()),
                    target_kind: Some("include".to_string()),
                    target_label: Some(second.label.clone()),
                },
            );

            assert!(!plan.allowed, "{:?}", plan.diagnostic);
            assert!(plan.patch.is_none());
        }

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn plan_tera_delete_blocks_missing_anchor() {
        let root = unique_test_dir();
        write_project(&root, "{% block content %}<main></main>{% endblock %}\n");
        let model = build_project_model(&root, &HashMap::new()).unwrap();

        let plan = plan_tera_delete(
            &model,
            &ProjectTeraDeleteIntent {
                target_source_id: Some("missing".to_string()),
                target_location: None,
                target_kind: Some("include".to_string()),
                target_label: Some("include missing.html".to_string()),
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan
            .diagnostic
            .unwrap()
            .contains("Nu am putut ancora nodul Tera"));
    }

    #[test]
    fn plan_tera_delete_blocks_template_level_directives() {
        let root = unique_test_dir();
        write_project(
            &root,
            concat!(
                "{% extends \"base.html\" %}\n",
                "{% import \"macros.html\" as macros %}\n",
                "{% macro card() %}{% endmacro %}\n",
                "{% block content %}<main></main>{% endblock %}\n",
            ),
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let cases = [
            (SourceNodeKind::Extends, "extends base.html", "extends"),
            (SourceNodeKind::Import, "import macros.html", "import"),
            (SourceNodeKind::Macro, "card", "macro"),
            (SourceNodeKind::Block, "content", "block"),
        ];

        for (kind, label, kind_label) in cases {
            let node = tera_node(&model, kind, label);
            let plan = plan_tera_delete(
                &model,
                &ProjectTeraDeleteIntent {
                    target_source_id: Some(node.id.clone()),
                    target_location: None,
                    target_kind: Some(kind_label.to_string()),
                    target_label: Some(node.label.clone()),
                },
            );

            assert!(!plan.allowed, "{kind_label} should be blocked");
            assert!(plan.diagnostic.unwrap().contains("nivel template"));
        }

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn plan_tera_delete_handles_filter_as_a_specialized_scope() {
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
        let node = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::Filter)
            .unwrap();

        let plan = plan_tera_delete(
            &model,
            &ProjectTeraDeleteIntent {
                target_source_id: Some(node.id.clone()),
                target_location: None,
                target_kind: Some("filter".to_string()),
                target_label: Some(node.label.clone()),
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        assert!(!plan
            .patch
            .expect("filter delete patch")
            .contents
            .contains("{% filter"));
    }

    #[test]
    fn plan_tera_delete_blocks_raw_code_only_scope() {
        let root = unique_test_dir();
        write_project(
            &root,
            concat!(
                "{% block content %}\n",
                "{% raw %}\n",
                "<article>{{ external_token }}</article>\n",
                "{% endraw %}\n",
                "{% endblock %}\n",
            ),
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let node = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::Raw)
            .unwrap();

        let plan = plan_tera_delete(
            &model,
            &ProjectTeraDeleteIntent {
                target_source_id: Some(node.id.clone()),
                target_location: Some(ProjectSourceEditLocation {
                    file: node.file.clone(),
                    line: node.range.as_ref().unwrap().line,
                    column: node.range.as_ref().unwrap().column,
                }),
                target_kind: Some("raw".to_string()),
                target_label: Some(node.label.clone()),
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.diagnostic.unwrap().contains("code-only"));
    }

    fn tera_node<'a>(model: &'a ProjectModel, kind: SourceNodeKind, label: &str) -> &'a SourceNode {
        model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == kind && node.label == label)
            .unwrap()
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
    }

    fn unique_test_dir() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "pana-studio-tera-delete-engine-{}-{stamp}",
            std::process::id()
        ))
    }
}
