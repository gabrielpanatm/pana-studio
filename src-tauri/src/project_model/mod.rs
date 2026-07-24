use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

pub mod attribute_engine;
pub(crate) mod cache;
pub mod delete_engine;
pub mod duplicate_engine;
mod files;
pub(crate) mod html_editor_schema;
pub mod insert_engine;
pub mod model;
pub mod move_engine;
mod ranges;
pub mod tag_engine;
pub mod template_edit_gate;
pub mod template_workbench;
pub mod tera_delete_engine;
mod tera_graph;
pub mod tera_insert_engine;
pub mod tera_move_engine;
pub mod text_engine;
pub mod zola_image_engine;

use crate::{
    kernel::project_workspace::WorkspaceProjectionLease,
    project::{is_zola_project, zola_project_root},
    project_model::{
        files::{
            collect_project_model_files, collect_project_model_files_from_workspace_sources,
            model_revision,
        },
        model::{ProjectModel, ProjectModelDiagnostic, ProjectModelDiagnosticSeverity},
        tera_graph::build_tera_graph,
    },
};

pub use model::ProjectModelSnapshot;
pub use move_engine::{ProjectHtmlMoveIntent, ProjectHtmlMovePlan};

pub fn build_project_model(
    project_root: &Path,
    draft_sources: &HashMap<String, String>,
) -> Result<ProjectModel, String> {
    build_project_model_with_projection(project_root, draft_sources, &HashSet::new())
}

/// Builds the editable model exclusively from one immutable ProjectWorkspace
/// projection. No clean text file is filled from the live project disk.
pub fn build_project_model_from_workspace_projection(
    project_root: &Path,
    projection: &WorkspaceProjectionLease,
) -> Result<ProjectModel, String> {
    let root = project_root
        .canonicalize()
        .map_err(|error| format!("Nu am putut rezolva folderul proiectului: {error}"))?;
    if root != Path::new(&projection.project_root) {
        return Err(format!(
            "ProjectModel a refuzat proiecția pentru alt root: {} != {}.",
            root.display(),
            projection.project_root
        ));
    }
    let zola_root = zola_project_root(&root);
    let files = collect_project_model_files_from_workspace_sources(
        &projection.source_texts,
        &projection.deleted_sources,
        &projection.changed_paths,
    )?;
    let source_graph =
        crate::source_graph::build_source_graph_from_workspace_projection(&root, projection)?;
    let tera_graph = build_tera_graph(&source_graph, &files);
    let revision = model_revision(&files);

    Ok(ProjectModel {
        project_root: root,
        zola_root,
        revision,
        files,
        source_graph,
        tera_graph,
        diagnostics: Vec::new(),
    })
}

pub fn build_project_model_with_projection(
    project_root: &Path,
    draft_sources: &HashMap<String, String>,
    deleted_sources: &HashSet<String>,
) -> Result<ProjectModel, String> {
    let root = project_root
        .canonicalize()
        .map_err(|error| format!("Nu am putut rezolva folderul proiectului: {error}"))?;
    let zola_root = zola_project_root(&root);
    let mut diagnostics = Vec::new();

    if !is_zola_project(&root) {
        diagnostics.push(ProjectModelDiagnostic {
            severity: ProjectModelDiagnosticSeverity::Warning,
            message: "Proiectul curent nu pare să fie un proiect Zola valid.".to_string(),
            file: None,
            range: None,
        });
    }

    let files = collect_project_model_files(&root, &zola_root, draft_sources, deleted_sources)?;
    let source_graph = crate::source_graph::build_source_graph_with_projection(
        &root,
        draft_sources,
        deleted_sources,
    )?;
    let tera_graph = build_tera_graph(&source_graph, &files);
    let revision = model_revision(&files);

    Ok(ProjectModel {
        project_root: root,
        zola_root,
        revision,
        files,
        source_graph,
        tera_graph,
        diagnostics,
    })
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        kernel::project_workspace::WorkspaceProjectionLease,
        project::{AcceptedProjectDiskManifest, ProjectDiskManifest},
        project_model::move_engine::{
            html_identity_aliases, html_node_id_at_line, plan_html_move, ProjectHtmlMoveIntent,
            ProjectMovePosition, ProjectSourceEditLocation,
        },
        source_graph::model::SourceNodeKind,
    };

    use super::*;

    #[test]
    fn builds_project_model_with_tera_graph_from_drafts() {
        let root = unique_test_dir();
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
        fs::write(
            root.join("templates/index.html"),
            "{% extends \"base.html\" %}{% block content %}<main></main>{% endblock %}",
        )
        .unwrap();
        fs::write(root.join("templates/base.html"), "<body></body>").unwrap();

        let mut drafts = HashMap::new();
        drafts.insert(
            "templates/index.html".to_string(),
            "{% extends \"base.html\" %}{% block content %}{% include \"partials/header.html\" %}{% for card in cards %}<article></article>{% endfor %}{% endblock %}".to_string(),
        );

        let model = build_project_model(&root, &drafts).unwrap();
        fs::remove_dir_all(&root).unwrap();

        assert!(model
            .files
            .iter()
            .any(|file| { file.relative_path == "templates/index.html" && file.from_draft }));
        assert!(model.tera_graph.templates.iter().any(|template| {
            template.name == "index.html"
                && template
                    .includes
                    .contains(&"partials/header.html".to_string())
        }));
        assert!(model
            .tera_graph
            .nodes
            .iter()
            .any(|node| node.kind == SourceNodeKind::For));
        assert!(model.revision.starts_with("pm_"));
    }

    #[test]
    fn workspace_projection_never_imports_external_text_from_disk() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(root.join("zola.toml"), "base_url = '/'\n").unwrap();
        fs::write(
            root.join("templates/index.html"),
            "<main>External replacement</main>",
        )
        .unwrap();
        fs::write(
            root.join("templates/external.html"),
            "<aside>External addition</aside>",
        )
        .unwrap();
        let canonical = root.canonicalize().unwrap().to_string_lossy().to_string();
        let session_id = "workspace-projection-test".to_string();
        let projection = WorkspaceProjectionLease {
            project_root: canonical.clone(),
            runtime_session_id: session_id.clone(),
            revision: 7,
            workspace_transaction_id: Some("workspace-test-7".to_string()),
            source_texts: HashMap::from([
                ("zola.toml".to_string(), "base_url = '/'\n".to_string()),
                (
                    "templates/index.html".to_string(),
                    "<main>Workspace snapshot</main>".to_string(),
                ),
            ]),
            resource_bytes: HashMap::new(),
            deleted_sources: HashSet::new(),
            changed_paths: HashSet::from(["templates/index.html".to_string()]),
            accepted_disk: AcceptedProjectDiskManifest::new(
                session_id,
                canonical.clone(),
                ProjectDiskManifest {
                    root: canonical,
                    files: Vec::new(),
                    truncated: false,
                    max_files: 1000,
                },
            )
            .unwrap(),
        };

        let model = build_project_model_from_workspace_projection(&root, &projection).unwrap();
        let index = model
            .files
            .iter()
            .find(|file| file.relative_path == "templates/index.html")
            .unwrap();
        assert_eq!(index.contents, "<main>Workspace snapshot</main>");
        assert!(index.from_draft);
        assert!(!model
            .files
            .iter()
            .any(|file| file.relative_path == "templates/external.html"));
        assert!(!model
            .source_graph
            .templates
            .iter()
            .any(|template| template.file == "templates/external.html"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn move_engine_resolves_stale_source_ids_through_aliases() {
        let root = unique_test_dir();
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
        fs::write(
            root.join("templates/index.html"),
            concat!(
                "{% block content %}\n",
                "<section class=\"hero\">\n",
                "  <h1 class=\"hero-title\">Titlu</h1>\n",
                "  <p class=\"hero-subtitle\">Subtitlu</p>\n",
                "  <div class=\"hero-actions\"><a class=\"btn\">A</a><a class=\"btn\">B</a></div>\n",
                "</section>\n",
                "{% endblock %}\n",
            ),
        )
        .unwrap();

        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let title_id = html_node_id(&model, "<h1 .hero-title>");
        let subtitle_id = html_node_id(&model, "<p .hero-subtitle>");
        let first_plan = plan_html_move(
            &model,
            &ProjectHtmlMoveIntent {
                source_source_id: Some(title_id.clone()),
                target_source_id: Some(subtitle_id.clone()),
                source_location: None,
                target_location: None,
                source_tag: Some("h1".to_string()),
                target_tag: Some("p".to_string()),
                source_selector: Some(".hero-title".to_string()),
                target_selector: Some(".hero-subtitle".to_string()),
                position: ProjectMovePosition::After,
            },
            &HashMap::new(),
        );
        assert!(first_plan.allowed, "{:?}", first_plan.diagnostic);
        let first_patch = first_plan.patch.unwrap();

        let mut drafts = HashMap::new();
        drafts.insert(first_patch.file.clone(), first_patch.contents.clone());
        let after_model = build_project_model(&root, &drafts).unwrap();
        let mut aliases = html_identity_aliases(&model, &after_model);
        let moved_after_id = html_node_id_at_line(
            &after_model,
            &first_patch.file,
            &first_patch.source_label,
            first_patch.new_start_line,
        )
        .unwrap();
        aliases.insert(first_patch.resolved_source_id.clone(), moved_after_id);

        let second_plan = plan_html_move(
            &after_model,
            &ProjectHtmlMoveIntent {
                source_source_id: Some(title_id),
                target_source_id: Some(subtitle_id),
                source_location: None,
                target_location: None,
                source_tag: Some("h1".to_string()),
                target_tag: Some("p".to_string()),
                source_selector: Some(".hero-title".to_string()),
                target_selector: Some(".hero-subtitle".to_string()),
                position: ProjectMovePosition::Before,
            },
            &aliases,
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(second_plan.allowed, "{:?}", second_plan.diagnostic);
    }

    #[test]
    fn move_engine_moves_inserted_session_element_by_source_location() {
        let root = unique_test_dir();
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
        fs::write(
            root.join("templates/index.html"),
            concat!(
                "{% block content %}\n",
                "<section class=\"hero\">\n",
                "  <h1 class=\"hero-title\">Titlu</h1>\n",
                "  <p class=\"hero-subtitle\">Subtitlu</p>\n",
                "</section>\n",
                "{% endblock %}\n",
            ),
        )
        .unwrap();

        let mut drafts = HashMap::new();
        drafts.insert(
            "templates/index.html".to_string(),
            concat!(
                "{% block content %}\n",
                "<section class=\"hero\">\n",
                "  <h1 class=\"hero-title\">Titlu</h1>\n",
                "  <button class=\"new-button\">Nou</button>\n",
                "  <p class=\"hero-subtitle\">Subtitlu</p>\n",
                "</section>\n",
                "{% endblock %}\n",
            )
            .to_string(),
        );

        let model = build_project_model(&root, &drafts).unwrap();
        let subtitle_id = html_node_id(&model, "<p .hero-subtitle>");
        let plan = plan_html_move(
            &model,
            &ProjectHtmlMoveIntent {
                source_source_id: None,
                target_source_id: Some(subtitle_id),
                source_location: Some(ProjectSourceEditLocation {
                    file: "templates/index.html".to_string(),
                    line: 4,
                    column: 3,
                }),
                target_location: None,
                source_tag: Some("button".to_string()),
                target_tag: Some("p".to_string()),
                source_selector: Some(".new-button".to_string()),
                target_selector: Some(".hero-subtitle".to_string()),
                position: ProjectMovePosition::After,
            },
            &HashMap::new(),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
    }

    #[test]
    fn move_engine_reindents_when_moving_element_inside_parent() {
        let root = unique_test_dir();
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
        fs::write(
            root.join("templates/index.html"),
            concat!(
                "{% block content %}\n",
                "<section class=\"hero\">\n",
                "  <div class=\"gallery\">\n",
                "<img class=\"first\" src=\"/a.jpg\">\n",
                "  </div>\n",
                "  <div class=\"card\">\n",
                "    <img class=\"second\" src=\"/b.jpg\">\n",
                "  </div>\n",
                "</section>\n",
                "{% endblock %}\n",
            ),
        )
        .unwrap();

        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let card_id = html_node_id(&model, "<div .card>");
        let gallery_id = html_node_id(&model, "<div .gallery>");
        let plan = plan_html_move(
            &model,
            &ProjectHtmlMoveIntent {
                source_source_id: Some(card_id),
                target_source_id: Some(gallery_id),
                source_location: None,
                target_location: None,
                source_tag: Some("div".to_string()),
                target_tag: Some("div".to_string()),
                source_selector: Some(".card".to_string()),
                target_selector: Some(".gallery".to_string()),
                position: ProjectMovePosition::Inside,
            },
            &HashMap::new(),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let contents = plan.patch.unwrap().contents;
        assert!(
            contents.contains(concat!(
                "  <div class=\"gallery\">\n",
                "    <img class=\"first\" src=\"/a.jpg\">\n",
                "    <div class=\"card\">\n",
                "      <img class=\"second\" src=\"/b.jpg\">\n",
                "    </div>\n",
                "  </div>"
            )),
            "{contents}"
        );
    }

    #[test]
    fn move_engine_rejects_contradictory_or_stale_html_identity_for_same_tag_siblings() {
        let root = unique_test_dir();
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
        fs::write(
            root.join("templates/index.html"),
            concat!(
                "{% block content %}\n",
                "<div><p class=\"first\">A</p><p class=\"second\">B</p></div>\n",
                "<section class=\"target\"></section>\n",
                "{% endblock %}\n",
            ),
        )
        .unwrap();

        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let first_id = html_node_id(&model, "<p .first>");
        let second_location = html_node_location(&model, "<p .second>");
        let target_id = html_node_id(&model, "<section .target>");

        for source_source_id in [Some(first_id), Some("stale-source-id".to_string())] {
            let plan = plan_html_move(
                &model,
                &ProjectHtmlMoveIntent {
                    source_source_id,
                    target_source_id: Some(target_id.clone()),
                    source_location: Some(second_location.clone()),
                    target_location: None,
                    source_tag: Some("p".to_string()),
                    target_tag: Some("section".to_string()),
                    source_selector: Some(".second".to_string()),
                    target_selector: Some(".target".to_string()),
                    position: ProjectMovePosition::Before,
                },
                &HashMap::new(),
            );

            assert!(!plan.allowed, "{:?}", plan.diagnostic);
            assert!(plan.patch.is_none());
        }

        let mut line_only_target_location = html_node_location(&model, "<section .target>");
        line_only_target_location.column = 0;
        let line_only_plan = plan_html_move(
            &model,
            &ProjectHtmlMoveIntent {
                source_source_id: Some(html_node_id(&model, "<p .first>")),
                target_source_id: None,
                source_location: None,
                target_location: Some(line_only_target_location),
                source_tag: Some("p".to_string()),
                target_tag: Some("section".to_string()),
                source_selector: Some(".first".to_string()),
                target_selector: Some(".target".to_string()),
                position: ProjectMovePosition::Before,
            },
            &HashMap::new(),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!line_only_plan.allowed);
        assert!(line_only_plan.patch.is_none());
    }

    #[test]
    fn move_engine_keeps_semantic_ids_valid_after_unrelated_rebasing() {
        let root = unique_test_dir();
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
        fs::write(
            root.join("templates/index.html"),
            concat!(
                "{% block content %}\n",
                "<section class=\"hero\">\n",
                "  <h1 class=\"hero-title\">Titlu</h1>\n",
                "  <p class=\"hero-subtitle\">Subtitlu</p>\n",
                "  <div class=\"hero-actions\"><a class=\"btn\">A</a><a class=\"btn\">B</a></div>\n",
                "</section>\n",
                "{% endblock %}\n",
            ),
        )
        .unwrap();

        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let title_id = html_node_id(&model, "<h1 .hero-title>");
        let subtitle_id = html_node_id(&model, "<p .hero-subtitle>");
        let first_plan = plan_html_move(
            &model,
            &ProjectHtmlMoveIntent {
                source_source_id: Some(title_id.clone()),
                target_source_id: Some(subtitle_id.clone()),
                source_location: None,
                target_location: None,
                source_tag: Some("h1".to_string()),
                target_tag: Some("p".to_string()),
                source_selector: Some(".hero-title".to_string()),
                target_selector: Some(".hero-subtitle".to_string()),
                position: ProjectMovePosition::After,
            },
            &HashMap::new(),
        );
        assert!(first_plan.allowed, "{:?}", first_plan.diagnostic);
        let first_patch = first_plan.patch.unwrap();

        let mut drafts = HashMap::new();
        drafts.insert(first_patch.file.clone(), first_patch.contents.clone());
        let after_model = build_project_model(&root, &drafts).unwrap();
        let title_location = html_node_location(&after_model, "<h1 .hero-title>");
        let subtitle_location = html_node_location(&after_model, "<p .hero-subtitle>");

        let second_plan = plan_html_move(
            &after_model,
            &ProjectHtmlMoveIntent {
                source_source_id: Some(title_id),
                target_source_id: Some(subtitle_id),
                source_location: Some(title_location),
                target_location: Some(subtitle_location),
                source_tag: Some("h1".to_string()),
                target_tag: Some("p".to_string()),
                source_selector: Some(".hero-title".to_string()),
                target_selector: Some(".hero-subtitle".to_string()),
                position: ProjectMovePosition::Before,
            },
            &HashMap::new(),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(second_plan.allowed, "{:?}", second_plan.diagnostic);
        assert!(second_plan.patch.is_some());
    }

    fn unique_test_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("panastudio-project-model-{nanos}"))
    }

    fn html_node_id(model: &ProjectModel, label: &str) -> String {
        model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::Html && node.label == label)
            .map(|node| node.id.clone())
            .unwrap_or_else(|| panic!("missing html node {label}"))
    }

    fn html_node_location(model: &ProjectModel, label: &str) -> ProjectSourceEditLocation {
        let node = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::Html && node.label == label)
            .unwrap_or_else(|| panic!("missing html node {label}"));
        let range = node.range.as_ref().expect("html node should have range");
        ProjectSourceEditLocation {
            file: node.file.clone(),
            line: range.line,
            column: range.column,
        }
    }
}
