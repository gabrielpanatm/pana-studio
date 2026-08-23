use std::path::Path;

pub mod attribute_engine;
pub(crate) mod cache;
pub mod delete_engine;
pub mod duplicate_engine;
mod files;
pub(crate) mod html_editor_schema;
pub(crate) mod incremental;
pub mod insert_engine;
pub(crate) mod managed_head_engine;
pub mod model;
pub mod move_engine;
pub(crate) mod structural_edit;
mod structural_envelope;
pub mod tag_engine;
pub mod template_workbench;
pub mod tera_delete_engine;
pub mod tera_insert_engine;
pub mod tera_move_engine;
#[cfg(test)]
pub(crate) mod test_support;
pub mod text_engine;
pub mod zola_image_engine;

use crate::{
    kernel::project_workspace::WorkspaceProjectionSnapshot,
    project::zola_project_root,
    project_model::{
        files::{collect_project_model_files_from_workspace_sources, model_revision},
        model::ProjectModel,
    },
};

pub(crate) use incremental::{
    rebuild_project_model_after_workspace_change,
    rebuild_project_model_after_workspace_change_with_source_changes,
    ProjectModelIncrementalBuildReport, ProjectModelIncrementalIntent,
};
/// Builds the editable model exclusively from one immutable ProjectWorkspace
/// projection. No clean text file is filled from the live project disk.
pub fn build_project_model_from_workspace_projection(
    project_root: &Path,
    projection: &WorkspaceProjectionSnapshot,
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
    let revision = model_revision(&files);
    let workspace_paths = workspace_paths_from_projection(projection);

    Ok(ProjectModel {
        project_root: root,
        zola_root,
        revision,
        files,
        workspace_paths,
        source_graph,
        diagnostics: Vec::new(),
    })
}

/// Builds the same immutable workspace model for Audit, but keeps source
/// conformance failures as diagnostics so independent providers can continue.
/// Authority, root and projection failures remain terminal.
pub(crate) fn build_project_model_for_audit_from_workspace_projection(
    project_root: &Path,
    projection: &WorkspaceProjectionSnapshot,
) -> Result<ProjectModel, String> {
    let root = project_root
        .canonicalize()
        .map_err(|error| format!("Nu am putut rezolva folderul proiectului: {error}"))?;
    if root != Path::new(&projection.project_root) {
        return Err(format!(
            "ProjectModel Audit a refuzat proiecția pentru alt root: {} != {}.",
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
    let source_graph = crate::source_graph::build_source_graph_for_audit_from_workspace_projection(
        &root, projection,
    )?;
    let revision = model_revision(&files);
    let workspace_paths = workspace_paths_from_projection(projection);

    Ok(ProjectModel {
        project_root: root,
        zola_root,
        revision,
        files,
        workspace_paths,
        source_graph,
        diagnostics: Vec::new(),
    })
}

fn workspace_paths_from_projection(
    projection: &WorkspaceProjectionSnapshot,
) -> std::collections::HashSet<String> {
    projection
        .accepted_disk
        .manifest
        .files
        .iter()
        .map(|entry| entry.relative_path.clone())
        .chain(projection.source_texts.keys().cloned())
        .chain(projection.resource_bytes.keys().cloned())
        .filter(|path| !projection.deleted_sources.contains(path))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        kernel::project_workspace::WorkspaceProjectionSnapshot,
        project::{AcceptedProjectDiskManifest, ProjectDiskManifest},
        project_model::move_engine::{plan_html_move, ProjectHtmlMoveIntent, ProjectMovePosition},
        project_model::test_support::ProjectModelTestFixture,
        source_graph::model::SourceNodeKind,
    };

    use super::*;

    #[test]
    fn builds_project_model_source_graph_from_drafts() {
        let root = unique_test_dir();
        let mut fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            "{% extends \"base.html\" %}{% block content %}<main></main>{% endblock %}",
        )
        .unwrap();
        fixture.source("templates/base.html", "<body></body>");
        fixture.draft(
            "templates/index.html",
            "{% extends \"base.html\" %}{% block content %}{% include \"partials/header.html\" %}{% for card in cards %}<article></article>{% endfor %}{% endblock %}".to_string(),
        );

        let model = fixture.build_model().unwrap();
        fs::remove_dir_all(&root).unwrap();

        assert!(model
            .files
            .iter()
            .any(|file| { file.relative_path == "templates/index.html" && file.from_draft }));
        assert!(model.source_graph.templates.iter().any(|template| {
            template.name == "index.html"
                && template
                    .includes
                    .contains(&"partials/header.html".to_string())
        }));
        assert!(model
            .source_graph
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
        let projection = WorkspaceProjectionSnapshot {
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
            ])
            .into(),
            resource_bytes: HashMap::new().into(),
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
            .unwrap()
            .into(),
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
    fn move_engine_moves_inserted_session_element_by_source_id() {
        let root = unique_test_dir();
        let mut fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
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
        fixture.draft(
            "templates/index.html",
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

        let model = fixture.build_model().unwrap();
        let button_id = html_node_id(&model, "<button .new-button>");
        let subtitle_id = html_node_id(&model, "<p .hero-subtitle>");
        let plan = plan_html_move(
            &model,
            &ProjectHtmlMoveIntent {
                source_source_id: Some(button_id),
                target_source_id: Some(subtitle_id),
                source_tag: Some("button".to_string()),
                target_tag: Some("p".to_string()),
                position: ProjectMovePosition::After,
                native_block_slot: None,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
    }

    #[test]
    fn move_engine_reindents_when_moving_element_inside_parent() {
        let root = unique_test_dir();
        let fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
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

        let model = fixture.build_model().unwrap();
        let card_id = html_node_id(&model, "<div .card>");
        let gallery_id = html_node_id(&model, "<div .gallery>");
        let plan = plan_html_move(
            &model,
            &ProjectHtmlMoveIntent {
                source_source_id: Some(card_id),
                target_source_id: Some(gallery_id),
                source_tag: Some("div".to_string()),
                target_tag: Some("div".to_string()),
                position: ProjectMovePosition::Inside,
                native_block_slot: None,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let contents = plan.patch.unwrap().contents;
        assert_eq!(
            contents,
            concat!(
                "{% block content %}\n",
                "<section class=\"hero\">\n",
                "  <div class=\"gallery\">\n",
                "    <img class=\"first\" src=\"/a.jpg\">\n",
                "    <div class=\"card\">\n",
                "      <img class=\"second\" src=\"/b.jpg\">\n",
                "    </div>\n",
                "  </div>\n",
                "</section>\n",
                "{% endblock %}\n",
            )
        );
    }

    #[test]
    fn repeated_inside_after_inside_moves_are_exact_and_do_not_accumulate_indent() {
        let root = unique_test_dir();
        let original = concat!(
            "{% block content %}\n",
            "<section class=\"hero\">\n",
            "  <div class=\"gallery\">\n",
            "    <img class=\"first\" src=\"/a.jpg\">\n",
            "  </div>\n",
            "  <div class=\"card\">\n",
            "    <img class=\"second\" src=\"/b.jpg\">\n",
            "  </div>\n",
            "</section>\n",
            "{% endblock %}\n",
        );
        let nested = concat!(
            "{% block content %}\n",
            "<section class=\"hero\">\n",
            "  <div class=\"gallery\">\n",
            "    <img class=\"first\" src=\"/a.jpg\">\n",
            "    <div class=\"card\">\n",
            "      <img class=\"second\" src=\"/b.jpg\">\n",
            "    </div>\n",
            "  </div>\n",
            "</section>\n",
            "{% endblock %}\n",
        );
        let mut fixture = ProjectModelTestFixture::standard_zola(root.clone(), original).unwrap();

        let first_model = fixture.build_model().unwrap();
        let first = plan_html_move(
            &first_model,
            &ProjectHtmlMoveIntent {
                source_source_id: Some(html_node_id(&first_model, "<div .card>")),
                target_source_id: Some(html_node_id(&first_model, "<div .gallery>")),
                source_tag: Some("div".to_string()),
                target_tag: Some("div".to_string()),
                position: ProjectMovePosition::Inside,
                native_block_slot: None,
            },
        );
        assert!(first.allowed, "{:?}", first.diagnostic);
        let first_contents = first.patch.unwrap().contents;
        assert_eq!(first_contents, nested);

        fixture.draft("templates/index.html", first_contents);
        let second_model = fixture.build_model().unwrap();
        let second = plan_html_move(
            &second_model,
            &ProjectHtmlMoveIntent {
                source_source_id: Some(html_node_id(&second_model, "<div .card>")),
                target_source_id: Some(html_node_id(&second_model, "<div .gallery>")),
                source_tag: Some("div".to_string()),
                target_tag: Some("div".to_string()),
                position: ProjectMovePosition::After,
                native_block_slot: None,
            },
        );
        assert!(second.allowed, "{:?}", second.diagnostic);
        let second_contents = second.patch.unwrap().contents;
        assert_eq!(second_contents, original);

        fixture.draft("templates/index.html", second_contents);
        let third_model = fixture.build_model().unwrap();
        let third = plan_html_move(
            &third_model,
            &ProjectHtmlMoveIntent {
                source_source_id: Some(html_node_id(&third_model, "<div .card>")),
                target_source_id: Some(html_node_id(&third_model, "<div .gallery>")),
                source_tag: Some("div".to_string()),
                target_tag: Some("div".to_string()),
                position: ProjectMovePosition::Inside,
                native_block_slot: None,
            },
        );
        fs::remove_dir_all(&root).unwrap();
        assert!(third.allowed, "{:?}", third.diagnostic);
        assert_eq!(third.patch.unwrap().contents, nested);
    }

    #[test]
    fn move_engine_preserves_tabs_crlf_utf8_and_exact_untouched_zones() {
        let root = unique_test_dir();
        let original = concat!(
            "<!-- înainte: Știre -->\r\n",
            "<main>\r\n",
            "\t<section class=\"target\">\r\n",
            "\t\t<span>Țintă</span>\r\n",
            "\t</section>\r\n",
            "\t<article class=\"source\">\r\n",
            "\t\t<p>Conținut</p>\r\n",
            "\t</article>\r\n",
            "</main>\r\n",
            "<!-- după: neschimbat -->\r\n",
        );
        let expected = concat!(
            "<!-- înainte: Știre -->\r\n",
            "<main>\r\n",
            "\t<section class=\"target\">\r\n",
            "\t\t<span>Țintă</span>\r\n",
            "\t\t<article class=\"source\">\r\n",
            "\t\t\t<p>Conținut</p>\r\n",
            "\t\t</article>\r\n",
            "\t</section>\r\n",
            "</main>\r\n",
            "<!-- după: neschimbat -->\r\n",
        );
        let fixture = ProjectModelTestFixture::standard_zola(root.clone(), original).unwrap();
        let model = fixture.build_model().unwrap();
        let plan = plan_html_move(
            &model,
            &ProjectHtmlMoveIntent {
                source_source_id: Some(html_node_id(&model, "<article .source>")),
                target_source_id: Some(html_node_id(&model, "<section .target>")),
                source_tag: Some("article".to_string()),
                target_tag: Some("section".to_string()),
                position: ProjectMovePosition::Inside,
                native_block_slot: None,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let contents = plan.patch.unwrap().contents;
        assert_eq!(contents, expected);
        assert!(!contents.replace("\r\n", "").contains('\n'));
    }

    #[test]
    fn move_engine_uses_only_exact_source_id_and_rejects_stale_identity() {
        let root = unique_test_dir();
        let fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            concat!(
                "{% block content %}\n",
                "<div><p class=\"first\">A</p><p class=\"second\">B</p></div>\n",
                "<section class=\"target\"></section>\n",
                "{% endblock %}\n",
            ),
        )
        .unwrap();

        let model = fixture.build_model().unwrap();
        let first_id = html_node_id(&model, "<p .first>");
        let target_id = html_node_id(&model, "<section .target>");

        let intent = |source_source_id, target_source_id| ProjectHtmlMoveIntent {
            source_source_id,
            target_source_id,
            source_tag: Some("p".to_string()),
            target_tag: Some("section".to_string()),
            position: ProjectMovePosition::Before,
            native_block_slot: None,
        };
        let exact = plan_html_move(
            &model,
            &intent(Some(first_id.clone()), Some(target_id.clone())),
        );
        assert!(exact.allowed, "{:?}", exact.diagnostic);
        assert_eq!(
            exact
                .patch
                .expect("exact SourceNodeId move patch")
                .resolved_source_id,
            first_id
        );

        let stale = plan_html_move(
            &model,
            &intent(Some("stale-source-id".to_string()), Some(target_id.clone())),
        );
        assert!(!stale.allowed, "{:?}", stale.diagnostic);
        assert!(stale.patch.is_none());

        let missing_target_id = plan_html_move(
            &model,
            &intent(Some(html_node_id(&model, "<p .first>")), None),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!missing_target_id.allowed);
        assert!(missing_target_id.patch.is_none());
    }

    #[test]
    fn move_engine_rejects_stale_ids_after_an_unreconciled_full_rebuild() {
        let root = unique_test_dir();
        let mut fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
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

        let model = fixture.build_model().unwrap();
        let title_id = html_node_id(&model, "<h1 .hero-title>");
        let subtitle_id = html_node_id(&model, "<p .hero-subtitle>");
        let first_plan = plan_html_move(
            &model,
            &ProjectHtmlMoveIntent {
                source_source_id: Some(title_id.clone()),
                target_source_id: Some(subtitle_id.clone()),
                source_tag: Some("h1".to_string()),
                target_tag: Some("p".to_string()),
                position: ProjectMovePosition::After,
                native_block_slot: None,
            },
        );
        assert!(first_plan.allowed, "{:?}", first_plan.diagnostic);
        let first_patch = first_plan.patch.unwrap();

        fixture.draft(first_patch.file.clone(), first_patch.contents.clone());
        let after_model = fixture.build_model().unwrap();
        let second_plan = plan_html_move(
            &after_model,
            &ProjectHtmlMoveIntent {
                source_source_id: Some(title_id),
                target_source_id: Some(subtitle_id),
                source_tag: Some("h1".to_string()),
                target_tag: Some("p".to_string()),
                position: ProjectMovePosition::Before,
                native_block_slot: None,
            },
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!second_plan.allowed);
        assert!(second_plan.patch.is_none());
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
}
