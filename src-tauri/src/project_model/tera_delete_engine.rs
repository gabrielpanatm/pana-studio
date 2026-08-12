use serde::{Deserialize, Serialize};

use crate::{
    project_model::model::{ProjectModel, ProjectModelFileKind},
    source_graph::model::{SourceNode, SourceNodeKind},
};

use super::move_engine::{
    content_revision, line_number_at_offset, removal_range_for_span, same_model_path,
    source_location_at_offset, ProjectSourceEditLocation, Span,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTeraDeleteIntent {
    pub target_source_id: Option<String>,
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
    intent
        .target_source_id
        .as_deref()
        .and_then(|id| resolve_tera_node(model, id, intent.target_kind.as_deref()))
}

fn resolve_tera_node<'a>(
    model: &'a ProjectModel,
    source_id: &str,
    kind: Option<&str>,
) -> Option<&'a SourceNode> {
    model
        .source_graph
        .node_by_id(source_id)
        .filter(|node| is_tera_delete_anchor_kind(&node.kind) && node_kind_matches(node, kind))
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

fn tera_source_missing_message(intent: &ProjectTeraDeleteIntent) -> String {
    let id = intent
        .target_source_id
        .as_deref()
        .unwrap_or("fără Source ID");
    let kind = intent.target_kind.as_deref().unwrap_or("fără kind");
    let label = intent.target_label.as_deref().unwrap_or("fără label");
    format!(
        "Nu am putut ancora nodul Tera în Project Model. SourceNodeId: {id}; kind: {kind}; label: {label}."
    )
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        project_model::test_support::ProjectModelTestFixture, source_graph::model::SourceNodeKind,
    };

    use super::*;

    #[test]
    fn plan_tera_delete_removes_include_line() {
        let root = unique_test_dir();
        let fixture = project_fixture(
            root.clone(),
            concat!(
                "{% block content %}\n",
                "<main>\n",
                "  {% include \"partials/card.html\" %}\n",
                "</main>\n",
                "{% endblock %}\n",
            ),
        );
        let projection_before = fixture.projection();
        let model = fixture.build_model().unwrap();
        let include = tera_node(
            &model,
            SourceNodeKind::Include,
            "include partials/card.html",
        );

        let plan = plan_tera_delete(
            &model,
            &ProjectTeraDeleteIntent {
                target_source_id: Some(include.id.clone()),
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
        assert_eq!(fixture.projection(), projection_before);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn plan_tera_delete_removes_loop_with_set_prelude_from_source_graph_range() {
        let root = unique_test_dir();
        let fixture = project_fixture(
            root.clone(),
            concat!(
                "{% block content %}\n",
                "{% set cards = section.pages %}\n",
                "{% for card in cards %}\n",
                "<article>{{ card.title }}</article>\n",
                "{% endfor %}\n",
                "{% endblock %}\n",
            ),
        );
        let model = fixture.build_model().unwrap();
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
    fn plan_tera_delete_uses_only_exact_source_id_and_rejects_stale_identity() {
        let root = unique_test_dir();
        let fixture = project_fixture(
            root.clone(),
            concat!(
                "{% block content %}\n",
                "{% include \"partials/a.html\" %}\n",
                "{% include \"partials/b.html\" %}\n",
                "{% endblock %}\n",
            ),
        );
        let model = fixture.build_model().unwrap();
        let first = tera_node(&model, SourceNodeKind::Include, "include partials/a.html");
        let second = tera_node(&model, SourceNodeKind::Include, "include partials/b.html");
        let exact = plan_tera_delete(
            &model,
            &ProjectTeraDeleteIntent {
                target_source_id: Some(first.id.clone()),
                target_kind: Some("include".to_string()),
                target_label: Some(second.label.clone()),
            },
        );
        assert!(exact.allowed, "{:?}", exact.diagnostic);
        let patch = exact.patch.expect("exact SourceNodeId delete patch");
        assert_eq!(patch.resolved_target_id, first.id);
        assert!(!patch.contents.contains("partials/a.html"));
        assert!(patch.contents.contains("partials/b.html"));

        let stale = plan_tera_delete(
            &model,
            &ProjectTeraDeleteIntent {
                target_source_id: Some("stale-source-id".to_string()),
                target_kind: Some("include".to_string()),
                target_label: Some(second.label.clone()),
            },
        );
        assert!(!stale.allowed, "{:?}", stale.diagnostic);
        assert!(stale.patch.is_none());

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn plan_tera_delete_blocks_missing_anchor() {
        let root = unique_test_dir();
        let fixture = project_fixture(
            root.clone(),
            "{% block content %}<main></main>{% endblock %}\n",
        );
        let model = fixture.build_model().unwrap();

        let plan = plan_tera_delete(
            &model,
            &ProjectTeraDeleteIntent {
                target_source_id: Some("missing".to_string()),
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
        let fixture = project_fixture(
            root.clone(),
            concat!(
                "{% extends \"base.html\" %}\n",
                "{% import \"macros.html\" as macros %}\n",
                "{% macro card() %}{% endmacro %}\n",
                "{% block content %}<main></main>{% endblock %}\n",
            ),
        );
        let model = fixture.build_model().unwrap();
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
        let fixture = project_fixture(
            root.clone(),
            concat!(
                "{% block content %}\n",
                "{% filter upper %}{{ title }}{% endfilter %}\n",
                "{% endblock %}\n",
            ),
        );
        let model = fixture.build_model().unwrap();
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
        let fixture = project_fixture(
            root.clone(),
            concat!(
                "{% block content %}\n",
                "{% raw %}\n",
                "<article>{{ external_token }}</article>\n",
                "{% endraw %}\n",
                "{% endblock %}\n",
            ),
        );
        let model = fixture.build_model().unwrap();
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

    fn project_fixture(root: PathBuf, template: &str) -> ProjectModelTestFixture {
        let mut fixture = ProjectModelTestFixture::standard_zola(root, template).unwrap();
        fixture.source("templates/partials/card.html", "<article></article>\n");
        fixture
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
