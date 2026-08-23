use super::{support::*, *};

#[test]
fn catalog_tracks_list_and_single_templates_for_an_empty_assigned_section() {
    let root = fixture_root("empty-section-templates");
    fs::create_dir_all(root.join("content/services")).unwrap();
    fs::create_dir_all(root.join("templates/services")).unwrap();
    fs::create_dir_all(root.join(CONTENT_MODEL_DIRECTORY)).unwrap();
    fs::write(
        root.join("zola.toml"),
        "base_url = \"https://example.com\"\n",
    )
    .unwrap();
    fs::write(
        root.join("content/_index.md"),
        "+++\ntitle = \"Acasă\"\ntemplate = \"index.html\"\n+++\n",
    )
    .unwrap();
    fs::write(
        root.join("templates/index.html"),
        "<!doctype html><html><body>Acasă</body></html>",
    )
    .unwrap();
    fs::write(
            root.join("content/services/_index.md"),
            "+++\ntitle = \"Servicii\"\ntemplate = \"services/list.html\"\npage_template = \"services/single.html\"\n+++\n",
        )
        .unwrap();
    fs::write(
        root.join("templates/services/list.html"),
        "{% for page in section.pages %}{{ page.extra.price }}{% endfor %}",
    )
    .unwrap();
    fs::write(
        root.join("templates/services/single.html"),
        "{{ page.extra.price }}",
    )
    .unwrap();
    fs::write(
        root.join(CONTENT_MODEL_PROJECT_PATH),
        "schema_version = 1\n",
    )
    .unwrap();
    fs::write(
            root.join(CONTENT_MODEL_ASSIGNMENTS_PATH),
            "schema_version = 1\n\n[[assignments]]\nsectionPath = \"content/services/_index.md\"\nmodelId = \"service\"\n",
        )
        .unwrap();
    fs::write(
            root.join(model_path("service")),
            "schemaVersion = 1\nid = \"service\"\nlabel = \"Serviciu\"\n\n[[fields]]\nid = \"field_price\"\nkey = \"price\"\nlabel = \"Preț\"\nkind = \"number\"\n",
        )
        .unwrap();

    let fixture =
            crate::project_model::test_support::ProjectModelTestFixture::from_integration_disk_boundary(
                &root,
            )
            .unwrap();
    let graph = fixture.build_source_graph().unwrap();
    assert!(graph.content_models.page_bindings.is_empty());
    assert_eq!(
        graph
            .content_models
            .template_usages
            .iter()
            .map(|usage| usage.template_file.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "templates/services/list.html",
            "templates/services/single.html"
        ])
    );
    let _ = fs::remove_dir_all(root);
}
