use serde::{Deserialize, Serialize};

use crate::{
    project_model::model::{ProjectModel, ProjectModelFileKind},
    source_graph::model::{SourceNode, SourceNodeKind},
};

use super::move_engine::{same_model_path, ProjectSourceEditLocation};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTemplateEditPermissionIntent {
    pub target_source_id: Option<String>,
    pub target_selector: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTemplateEditPermissionPlan {
    pub allowed: bool,
    pub diagnostic: Option<String>,
    pub model_revision: String,
    pub grant: Option<ProjectTemplateEditPermissionGrant>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTemplateEditPermissionGrant {
    pub file: String,
    pub resolved_target_id: String,
    pub target_kind: String,
    pub target_label: String,
    pub target_location: Option<ProjectSourceEditLocation>,
    pub selector: String,
    pub scope: TemplateEditPermissionScope,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateEditPermissionScope {
    Template,
    Partial,
    TeraScope,
}

pub fn plan_template_edit_permission(
    model: &ProjectModel,
    intent: &ProjectTemplateEditPermissionIntent,
) -> ProjectTemplateEditPermissionPlan {
    match plan_template_edit_permission_inner(model, intent) {
        Ok(grant) => ProjectTemplateEditPermissionPlan {
            allowed: true,
            diagnostic: None,
            model_revision: model.revision.clone(),
            grant: Some(grant),
        },
        Err(message) => ProjectTemplateEditPermissionPlan {
            allowed: false,
            diagnostic: Some(message),
            model_revision: model.revision.clone(),
            grant: None,
        },
    }
}

fn plan_template_edit_permission_inner(
    model: &ProjectModel,
    intent: &ProjectTemplateEditPermissionIntent,
) -> Result<ProjectTemplateEditPermissionGrant, String> {
    let source_id = intent
        .target_source_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Template Edit Gate nu a primit Source ID.".to_string())?;
    let selector = intent
        .target_selector
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Template Edit Gate nu a primit selector preview.".to_string())?;
    let node = model
        .source_graph
        .nodes
        .iter()
        .find(|node| node.id == source_id)
        .ok_or_else(|| {
            format!("Template Edit Gate nu a găsit Source ID-ul {source_id} în Project Model.")
        })?;
    let file = model
        .files
        .iter()
        .find(|file| same_model_path(&file.relative_path, &node.file))
        .ok_or_else(|| format!("Nu am găsit fișierul {} în Project Model.", node.file))?;
    if file.kind != ProjectModelFileKind::Template {
        return Err(
            "Template Edit Gate este activ doar pentru template-uri Zola/Tera.".to_string(),
        );
    }

    let scope = template_edit_permission_scope(node)?;
    let location = node.range.as_ref().map(|range| ProjectSourceEditLocation {
        file: node.file.clone(),
        line: range.line,
        column: range.column,
    });

    Ok(ProjectTemplateEditPermissionGrant {
        file: node.file.clone(),
        resolved_target_id: node.id.clone(),
        target_kind: source_kind_label(&node.kind).to_string(),
        target_label: node.label.clone(),
        target_location: location,
        selector: selector.to_string(),
        scope,
    })
}

fn template_edit_permission_scope(
    node: &SourceNode,
) -> Result<TemplateEditPermissionScope, String> {
    match node.kind {
        SourceNodeKind::Template => Ok(TemplateEditPermissionScope::Template),
        SourceNodeKind::Partial => Ok(TemplateEditPermissionScope::Partial),
        SourceNodeKind::Block
        | SourceNodeKind::Include
        | SourceNodeKind::Macro
        | SourceNodeKind::For
        | SourceNodeKind::If
        | SourceNodeKind::With => Ok(TemplateEditPermissionScope::TeraScope),
        SourceNodeKind::Extends
        | SourceNodeKind::Import
        | SourceNodeKind::Set
        | SourceNodeKind::TeraVariable
        | SourceNodeKind::TeraComment
        | SourceNodeKind::Raw
        | SourceNodeKind::Tera => Err(format!(
            "{} nu poate debloca editare HTML vizuală.",
            source_kind_label(&node.kind)
        )),
        _ => Err("Nodul selectat nu este un gate Tera/template editabil.".to_string()),
    }
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
    fn plan_template_edit_permission_grants_for_block_scope() {
        let root = unique_test_dir();
        write_project(
            &root,
            concat!(
                "{% block content %}\n",
                "  <main></main>\n",
                "{% endblock %}\n",
            ),
        );
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let block = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::Block && node.label == "content")
            .unwrap();

        let plan = plan_template_edit_permission(
            &model,
            &ProjectTemplateEditPermissionIntent {
                target_source_id: Some(block.id.clone()),
                target_selector: Some("main".to_string()),
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let grant = plan.grant.unwrap();
        assert_eq!(grant.target_kind, "block");
        assert_eq!(
            grant.scope as u8,
            TemplateEditPermissionScope::TeraScope as u8
        );
    }

    #[test]
    fn plan_template_edit_permission_blocks_variable_scope() {
        let root = unique_test_dir();
        write_project(&root, "{{ page.title }}\n<main></main>\n");
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let variable = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::TeraVariable)
            .unwrap();

        let plan = plan_template_edit_permission(
            &model,
            &ProjectTemplateEditPermissionIntent {
                target_source_id: Some(variable.id.clone()),
                target_selector: Some("main".to_string()),
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.diagnostic.unwrap().contains("nu poate debloca"));
    }

    #[test]
    fn plan_template_edit_permission_blocks_raw_scope() {
        let root = unique_test_dir();
        write_project(&root, "{% raw %}\n<main></main>\n{% endraw %}\n");
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let raw = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::Raw)
            .unwrap();

        let plan = plan_template_edit_permission(
            &model,
            &ProjectTemplateEditPermissionIntent {
                target_source_id: Some(raw.id.clone()),
                target_selector: Some("main".to_string()),
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.diagnostic.unwrap().contains("nu poate debloca"));
    }

    #[test]
    fn plan_template_edit_permission_requires_selector() {
        let root = unique_test_dir();
        write_project(&root, "{% include \"partials/card.html\" %}\n");
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let include = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::Include)
            .unwrap();

        let plan = plan_template_edit_permission(
            &model,
            &ProjectTemplateEditPermissionIntent {
                target_source_id: Some(include.id.clone()),
                target_selector: None,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.diagnostic.unwrap().contains("selector"));
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
            "pana-studio-template-edit-gate-{}-{stamp}",
            std::process::id()
        ))
    }
}
