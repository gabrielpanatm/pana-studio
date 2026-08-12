use serde::{Deserialize, Serialize};

use crate::{
    blocks::{
        node_is_slider_managed_scaffold, validate_native_block_slot_delete,
        NativeBlockSlotMutationContext,
    },
    project_model::model::{ProjectModel, ProjectModelFile, ProjectModelFileKind},
    source_graph::model::SourceNode,
};

use super::move_engine::{
    content_revision, line_number_at_offset, removal_range_for_span, resolve_html_node_for_anchor,
    same_model_path, source_location_at_offset, source_missing_message, ProjectSourceEditLocation,
    Span,
};
use super::structural_envelope::{structural_envelope_for_html_node, StructuralEnvelopeKind};
use super::zola_image_engine::zola_image_contract_start;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlDeleteIntent {
    pub target_source_id: Option<String>,
    #[serde(default)]
    pub target_render_instance_id: Option<String>,
    pub target_tag: Option<String>,
    #[serde(default)]
    pub native_block_slot: Option<NativeBlockSlotMutationContext>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlDeletePlan {
    pub allowed: bool,
    pub diagnostic: Option<String>,
    pub model_revision: String,
    pub patch: Option<ProjectHtmlDeletePatch>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlDeletePatch {
    pub file: String,
    pub resolved_target_id: String,
    pub deleted_label: String,
    pub before_revision: String,
    pub after_revision: String,
    pub contents: String,
    pub target_location: ProjectSourceEditLocation,
    pub source_start_line: usize,
    pub source_end_line: usize,
    pub line_shift_start: usize,
    pub line_shift: isize,
}

pub fn plan_html_delete(
    model: &ProjectModel,
    intent: &ProjectHtmlDeleteIntent,
) -> ProjectHtmlDeletePlan {
    match plan_html_delete_inner(model, intent) {
        Ok(patch) => ProjectHtmlDeletePlan {
            allowed: true,
            diagnostic: None,
            model_revision: model.revision.clone(),
            patch: Some(patch),
        },
        Err(message) => ProjectHtmlDeletePlan {
            allowed: false,
            diagnostic: Some(message),
            model_revision: model.revision.clone(),
            patch: None,
        },
    }
}

fn plan_html_delete_inner(
    model: &ProjectModel,
    intent: &ProjectHtmlDeleteIntent,
) -> Result<ProjectHtmlDeletePatch, String> {
    if let Some(context) = intent.native_block_slot.as_ref() {
        validate_native_block_slot_delete(model, context, intent.target_source_id.as_deref())?;
    }
    if let Some(target_node) = resolve_html_node_for_anchor(
        model,
        intent.target_source_id.as_deref(),
        intent.target_tag.as_deref(),
    ) {
        if intent.native_block_slot.is_none() && node_is_slider_managed_scaffold(model, target_node)
        {
            return Err(
                "Structura administrată Slider se șterge numai prin BlockPropertiesPane."
                    .to_string(),
            );
        }
        return plan_html_delete_from_source_node(model, target_node);
    }

    Err(source_missing_message(
        "țintă",
        intent.target_source_id.as_deref(),
    ))
}

fn plan_html_delete_from_source_node(
    model: &ProjectModel,
    target_node: &SourceNode,
) -> Result<ProjectHtmlDeletePatch, String> {
    if !target_node.capabilities.can_edit_visual {
        return Err(target_node
            .capabilities
            .technical_reason()
            .map(str::to_string)
            .unwrap_or_else(|| "Elementul nu este ștergibil vizual.".to_string()));
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
            "HTML Delete Engine este activ doar pentru template-uri Zola/Tera.".to_string(),
        );
    }

    let target_range = target_node
        .range
        .as_ref()
        .ok_or_else(|| "Ținta nu are range stabil în Source Graph.".to_string())?;
    let envelope = structural_envelope_for_html_node(model, &file.contents, target_node)?;
    let mut span = envelope.span;
    if envelope.kind == StructuralEnvelopeKind::HtmlElement {
        if let Some(contract_start) = zola_image_contract_start(&file.contents, target_range.start)?
        {
            span.start = contract_start;
        }
    }

    plan_html_delete_for_span(
        file,
        &target_node.file,
        span,
        target_node.id.clone(),
        target_node.label.clone(),
    )
}

fn plan_html_delete_for_span(
    file: &ProjectModelFile,
    file_path: &str,
    span: Span,
    resolved_target_id: String,
    deleted_label: String,
) -> Result<ProjectHtmlDeletePatch, String> {
    let removal = removal_range_for_span(&file.contents, span);
    let removed_text = file
        .contents
        .get(removal.start..removal.end)
        .ok_or_else(|| "Range-ul de ștergere este invalid.".to_string())?;
    let removed_lines = removed_text.bytes().filter(|byte| *byte == b'\n').count() as isize;
    let target_location = source_location_at_offset(&file.contents, file_path, span.start);
    let source_start_line = line_number_at_offset(&file.contents, span.start);
    let source_end_line = line_number_at_offset(&file.contents, span.end);
    let contents = format!(
        "{}{}",
        &file.contents[..removal.start],
        &file.contents[removal.end..]
    );

    Ok(ProjectHtmlDeletePatch {
        file: file_path.to_string(),
        resolved_target_id,
        deleted_label,
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
    fn plan_html_delete_removes_target_element_with_metadata() {
        let root = unique_test_dir();
        let fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            concat!(
                "{% block content %}\n",
                "<section class=\"hero\">\n",
                "  <h1>Titlu</h1>\n",
                "  <p class=\"lede\">Text</p>\n",
                "</section>\n",
                "{% endblock %}\n",
            ),
        )
        .unwrap();
        let model = fixture.build_model().unwrap();
        let paragraph = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<p .lede>")
            .unwrap();

        let plan = plan_html_delete(
            &model,
            &ProjectHtmlDeleteIntent {
                target_source_id: Some(paragraph.id.clone()),
                target_render_instance_id: None,
                target_tag: Some("p".to_string()),
                native_block_slot: None,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.unwrap();
        assert!(!patch.contents.contains("class=\"lede\""));
        assert_eq!(patch.source_start_line, 4);
        assert_eq!(patch.source_end_line, 4);
        assert_eq!(patch.line_shift, -1);
    }

    #[test]
    fn consecutive_child_then_parent_delete_uses_the_rebuilt_model() {
        let root = unique_test_dir();
        let mut fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            "<section class=\"section\">\n  <div class=\"child\"></div>\n</section>\n",
        )
        .unwrap();
        let before = fixture.build_model().unwrap();
        let child = before
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<div .child>")
            .unwrap();
        let child_delete = plan_html_delete(
            &before,
            &ProjectHtmlDeleteIntent {
                target_source_id: Some(child.id.clone()),
                target_render_instance_id: None,
                target_tag: Some("div".to_string()),
                native_block_slot: None,
            },
        );
        assert!(child_delete.allowed, "{:?}", child_delete.diagnostic);
        let child_patch = child_delete.patch.unwrap();
        assert_eq!(
            child_patch.contents,
            "<section class=\"section\">\n</section>\n"
        );

        fixture.draft("templates/index.html", child_patch.contents);
        let after_child = fixture.build_model().unwrap();
        let parent = after_child
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section .section>")
            .unwrap();
        let parent_delete = plan_html_delete(
            &after_child,
            &ProjectHtmlDeleteIntent {
                target_source_id: Some(parent.id.clone()),
                target_render_instance_id: None,
                target_tag: Some("section".to_string()),
                native_block_slot: None,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(parent_delete.allowed, "{:?}", parent_delete.diagnostic);
        assert_eq!(parent_delete.patch.unwrap().contents, "");
    }

    #[test]
    fn plan_html_delete_rejects_location_without_source_id() {
        let root = unique_test_dir();
        let mut fixture =
            ProjectModelTestFixture::standard_zola(root.clone(), "<main></main>\n").unwrap();
        fixture.source(
            "static/plain.html",
            concat!(
                "<!DOCTYPE html>\n",
                "<html>\n",
                "<body>\n",
                "  <section id=\"hero\">\n",
                "    <p>Text</p>\n",
                "  </section>\n",
                "</body>\n",
                "</html>\n",
            ),
        );
        let model = fixture.build_model().unwrap();

        let plan = plan_html_delete(
            &model,
            &ProjectHtmlDeleteIntent {
                target_source_id: None,
                target_render_instance_id: None,
                target_tag: Some("section".to_string()),
                native_block_slot: None,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.patch.is_none());
    }

    #[test]
    fn plan_html_delete_rejects_stale_source_id_instead_of_using_location() {
        let root = unique_test_dir();
        let mut fixture =
            ProjectModelTestFixture::standard_zola(root.clone(), "<main></main>\n").unwrap();
        fixture.source(
            "static/plain.html",
            concat!(
                "<!DOCTYPE html>\n",
                "<html>\n",
                "<body>\n",
                "  <section id=\"first\"></section>\n",
                "  <section id=\"second\"></section>\n",
                "</body>\n",
                "</html>\n",
            ),
        );
        let model = fixture.build_model().unwrap();

        let plan = plan_html_delete(
            &model,
            &ProjectHtmlDeleteIntent {
                target_source_id: Some("stale-source-id".to_string()),
                target_render_instance_id: None,
                target_tag: Some("section".to_string()),
                native_block_slot: None,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.patch.is_none());
        assert!(plan
            .diagnostic
            .as_deref()
            .is_some_and(|diagnostic| diagnostic.contains("Nu am putut ancora țintă")));
    }

    #[test]
    fn plan_html_delete_blocks_missing_anchor() {
        let root = unique_test_dir();
        let fixture =
            ProjectModelTestFixture::standard_zola(root.clone(), "<section></section>\n").unwrap();
        let model = fixture.build_model().unwrap();

        let plan = plan_html_delete(
            &model,
            &ProjectHtmlDeleteIntent {
                target_source_id: Some("missing".to_string()),
                target_render_instance_id: None,
                target_tag: Some("p".to_string()),
                native_block_slot: None,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan
            .diagnostic
            .unwrap()
            .contains("Nu am putut ancora țintă"));
    }

    #[test]
    fn plan_html_delete_removes_the_complete_dynamic_widget_envelope() {
        let root = unique_test_dir();
        let fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            concat!(
                "<main>\n",
                "  {# pana:widget schema=2 provider=dynamic-field instance=dynamic-field-delete01 props=00 #}\n",
                "  <h2 data-pana-widget-instance=\"dynamic-field-delete01\">{{ page.title }}</h2>\n",
                "  {# /pana:widget instance=dynamic-field-delete01 #}\n",
                "  <p>Rămâne</p>\n",
                "</main>\n",
            ),
        )
        .unwrap();
        let model = fixture.build_model().unwrap();
        let heading = model
            .source_graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == crate::source_graph::model::SourceNodeKind::Html
                    && node.label.starts_with("<h2")
            })
            .unwrap();

        let plan = plan_html_delete(
            &model,
            &ProjectHtmlDeleteIntent {
                target_source_id: Some(heading.id.clone()),
                target_render_instance_id: None,
                target_tag: Some("h2".to_string()),
                native_block_slot: None,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let contents = plan.patch.unwrap().contents;
        assert!(!contents.contains("dynamic-field-delete01"));
        assert!(!contents.contains("{{ page.title }}"));
        assert!(contents.contains("<p>Rămâne</p>"));
    }

    fn unique_test_dir() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "pana-studio-delete-engine-{}-{stamp}",
            std::process::id()
        ))
    }
}
