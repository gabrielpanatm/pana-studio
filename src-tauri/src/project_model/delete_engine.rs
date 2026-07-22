use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    project_model::model::{ProjectModel, ProjectModelFile, ProjectModelFileKind},
    source_graph::model::SourceNode,
};

use super::move_engine::{
    content_revision, direct_location_without_source_id, line_number_at_offset,
    offset_for_source_location, parse_html_tag_at, removal_range_for_span,
    resolve_html_element_span, resolve_html_node_for_anchor, same_model_path,
    source_location_at_offset, source_missing_message, ProjectSourceEditLocation, Span,
};
use super::zola_image_engine::zola_image_contract_start;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlDeleteIntent {
    pub target_source_id: Option<String>,
    pub target_location: Option<ProjectSourceEditLocation>,
    pub target_tag: Option<String>,
    pub target_selector: Option<String>,
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
    aliases: &HashMap<String, String>,
) -> ProjectHtmlDeletePlan {
    match plan_html_delete_inner(model, intent, aliases) {
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
    aliases: &HashMap<String, String>,
) -> Result<ProjectHtmlDeletePatch, String> {
    if let Some(target_node) = resolve_html_node_for_anchor(
        model,
        intent.target_source_id.as_deref(),
        intent.target_location.as_ref(),
        intent.target_tag.as_deref(),
        aliases,
    ) {
        return plan_html_delete_from_source_node(model, target_node);
    }

    if let Some(location) = direct_location_without_source_id(
        intent.target_source_id.as_deref(),
        intent.target_location.as_ref(),
    ) {
        return plan_html_delete_from_direct_location(model, intent, location);
    }

    Err(source_missing_message(
        "țintă",
        intent.target_source_id.as_deref(),
        intent.target_location.as_ref(),
        intent.target_selector.as_deref(),
    ))
}

fn plan_html_delete_from_source_node(
    model: &ProjectModel,
    target_node: &SourceNode,
) -> Result<ProjectHtmlDeletePatch, String> {
    if !target_node.capabilities.can_edit_visual {
        return Err(target_node
            .capabilities
            .reason
            .clone()
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
    let mut span = resolve_html_element_span(&file.contents, target_range.start)?;
    if let Some(contract_start) = zola_image_contract_start(&file.contents, target_range.start)? {
        span.start = contract_start;
    }

    plan_html_delete_for_span(
        file,
        &target_node.file,
        span,
        target_node.id.clone(),
        target_node.label.clone(),
    )
}

fn plan_html_delete_from_direct_location(
    model: &ProjectModel,
    intent: &ProjectHtmlDeleteIntent,
    location: &ProjectSourceEditLocation,
) -> Result<ProjectHtmlDeletePatch, String> {
    let file = model
        .files
        .iter()
        .find(|file| same_model_path(&file.relative_path, &location.file))
        .ok_or_else(|| format!("Nu am găsit fișierul {} în Project Model.", location.file))?;

    if !is_direct_html_delete_file(file) {
        return Err(
            "Ștergerea prin locație directă este activă doar pentru fișiere HTML 1:1 din proiect, nu pentru template-uri Tera.".to_string(),
        );
    }

    let offset = offset_for_source_location(&file.contents, location)?;
    let tag = parse_html_tag_at(&file.contents, offset)
        .ok_or_else(|| "Locația nu indică începutul unui tag HTML ștergibil.".to_string())?;
    if tag.is_closing {
        return Err("Locația indică un tag de închidere, nu un element ștergibil.".to_string());
    }
    if tag.tag == "html" || tag.tag == "body" {
        return Err("Elementul rădăcină nu poate fi șters.".to_string());
    }
    if let Some(expected_tag) = intent.target_tag.as_deref() {
        let expected_tag = expected_tag.trim().to_ascii_lowercase();
        if !expected_tag.is_empty() && expected_tag != tag.tag {
            return Err(format!(
                "Locația indică <{}>, dar intenția preview a cerut <{}>.",
                tag.tag, expected_tag
            ));
        }
    }

    let span = resolve_html_element_span(&file.contents, tag.start)?;
    let resolved_target_id = intent.target_source_id.clone().unwrap_or_else(|| {
        format!(
            "location:{}:{}:{}",
            location.file, location.line, location.column
        )
    });

    plan_html_delete_for_span(
        file,
        &file.relative_path,
        span,
        resolved_target_id,
        format!("<{}>", tag.tag),
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

fn is_direct_html_delete_file(file: &ProjectModelFile) -> bool {
    matches!(
        file.kind,
        ProjectModelFileKind::StaticText | ProjectModelFileKind::OtherText
    ) && is_html_path(&file.relative_path)
}

fn is_html_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".html") || lower.ends_with(".htm")
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
    fn plan_html_delete_removes_target_element_with_metadata() {
        let root = unique_test_dir();
        write_project(
            &root,
            concat!(
                "{% block content %}\n",
                "<section class=\"hero\">\n",
                "  <h1>Titlu</h1>\n",
                "  <p class=\"lede\">Text</p>\n",
                "</section>\n",
                "{% endblock %}\n",
            ),
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
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
                target_location: None,
                target_tag: Some("p".to_string()),
                target_selector: Some(".lede".to_string()),
            },
            &HashMap::new(),
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
    fn plan_html_delete_resolves_active_html_by_direct_location() {
        let root = unique_test_dir();
        write_project(&root, "<main></main>\n");
        fs::create_dir_all(root.join("static")).unwrap();
        fs::write(
            root.join("static/plain.html"),
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
        )
        .unwrap();
        let model = build_project_model(&root, &HashMap::new()).unwrap();

        let plan = plan_html_delete(
            &model,
            &ProjectHtmlDeleteIntent {
                target_source_id: None,
                target_location: Some(ProjectSourceEditLocation {
                    file: "static/plain.html".to_string(),
                    line: 4,
                    column: 3,
                }),
                target_tag: Some("section".to_string()),
                target_selector: Some("body:nth-of-type(1) > section:nth-of-type(1)".to_string()),
            },
            &HashMap::new(),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.unwrap();
        assert_eq!(patch.file, "static/plain.html");
        assert_eq!(patch.resolved_target_id, "location:static/plain.html:4:3");
        assert!(!patch.contents.contains("id=\"hero\""));
        assert_eq!(patch.source_start_line, 4);
        assert_eq!(patch.source_end_line, 6);
        assert_eq!(patch.line_shift, -3);
    }

    #[test]
    fn plan_html_delete_rejects_stale_source_id_instead_of_using_direct_location_fallback() {
        let root = unique_test_dir();
        write_project(&root, "<main></main>\n");
        fs::create_dir_all(root.join("static")).unwrap();
        fs::write(
            root.join("static/plain.html"),
            concat!(
                "<!DOCTYPE html>\n",
                "<html>\n",
                "<body>\n",
                "  <section id=\"first\"></section>\n",
                "  <section id=\"second\"></section>\n",
                "</body>\n",
                "</html>\n",
            ),
        )
        .unwrap();
        let model = build_project_model(&root, &HashMap::new()).unwrap();

        let plan = plan_html_delete(
            &model,
            &ProjectHtmlDeleteIntent {
                target_source_id: Some("stale-source-id".to_string()),
                target_location: Some(ProjectSourceEditLocation {
                    file: "static/plain.html".to_string(),
                    line: 5,
                    column: 3,
                }),
                target_tag: Some("section".to_string()),
                target_selector: Some("#second".to_string()),
            },
            &HashMap::new(),
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
        write_project(&root, "<section></section>\n");
        let model = build_project_model(&root, &HashMap::new()).unwrap();

        let plan = plan_html_delete(
            &model,
            &ProjectHtmlDeleteIntent {
                target_source_id: Some("missing".to_string()),
                target_location: None,
                target_tag: Some("p".to_string()),
                target_selector: Some("p".to_string()),
            },
            &HashMap::new(),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan
            .diagnostic
            .unwrap()
            .contains("Nu am putut ancora țintă"));
    }

    fn write_project(root: &PathBuf, template: &str) {
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates")).unwrap();
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
