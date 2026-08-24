use super::{support::*, *};

fn integration_fixture(
    name: &str,
) -> (
    std::path::PathBuf,
    crate::project_model::test_support::ProjectModelTestFixture,
) {
    let root = fixture_root(name);
    for directory in [
        "content/services",
        "content/articles",
        "content/portfolio",
        "templates/services",
        "templates/articles",
        CONTENT_MODEL_DIRECTORY,
    ] {
        fs::create_dir_all(root.join(directory)).unwrap();
    }

    for (relative_path, source) in [
        (
            "zola.toml",
            "base_url = \"https://example.com\"\ndefault_language = \"en\"\n\n[languages.ro]\n",
        ),
        (
            "content/_index.md",
            "+++\ntitle = \"Acasă\"\ntemplate = \"index.html\"\n+++\n",
        ),
        (
            "templates/index.html",
            "<!doctype html><html><body>Acasă</body></html>",
        ),
        (
            "content/services/_index.md",
            "+++\ntitle = \"Servicii\"\ntemplate = \"services/list.html\"\npage_template = \"services/single.html\"\n+++\n",
        ),
        (
            "content/services/consultanta.md",
            "+++\ntitle = \"Consultanță\"\n[extra]\nprice = 120\n+++\nCorp\n",
        ),
        (
            "content/services/audit.md",
            "+++\ntitle = \"Audit\"\n[extra]\nprice = 80\n+++\nCorp\n",
        ),
        (
            "content/services/consultanta.ro.md",
            "+++\ntitle = \"Consultanță RO\"\n[extra]\nprice = 130\n+++\nCorp\n",
        ),
        (
            "content/articles/_index.md",
            "+++\ntitle = \"Articole\"\ntemplate = \"articles/list.html\"\npage_template = \"articles/single.html\"\n+++\n",
        ),
        (
            "content/articles/anunt.md",
            "+++\ntitle = \"Anunț\"\n[extra]\nprice = 40\n+++\nCorp\n",
        ),
        (
            "content/portfolio/_index.md",
            "+++\ntitle = \"Portofoliu\"\n+++\n",
        ),
        (
            "content/portfolio/studiu.md",
            "+++\ntitle = \"Studiu\"\n[extra]\nprice = \"necunoscut\"\n+++\nCorp\n",
        ),
        (
            "templates/services/single.html",
            "<strong>{{ page.extra.price }}</strong>",
        ),
        (
            "templates/services/list.html",
            "{% for page in section.pages %}<a href=\"{{ page.permalink }}\">{{ page.extra.price }}</a>{% endfor %}",
        ),
        (
            "templates/articles/single.html",
            "<em>{{ page.extra.price }}</em>",
        ),
        (
            "templates/articles/list.html",
            "{% for page in section.pages %}<a href=\"{{ page.permalink }}\">{{ page.extra.price }}</a>{% endfor %}",
        ),
        (
            "templates/unrelated.html",
            "<span>{{ page.extra.price }}</span>",
        ),
        (CONTENT_MODEL_PROJECT_PATH, "schema_version = 1\n"),
        (
            CONTENT_MODEL_ASSIGNMENTS_PATH,
            "schema_version = 1\n\n[[assignments]]\nsectionPath = \"content/services/_index.md\"\nmodelId = \"service\"\n\n[[assignments]]\nsectionPath = \"content/articles/_index.md\"\nmodelId = \"service\"\n",
        ),
        (
            ".panastudio/content-models/service.toml",
            "schemaVersion = 1\nid = \"service\"\nlabel = \"Serviciu\"\n\n[[fields]]\nid = \"field_price\"\nkey = \"price\"\nlabel = \"Preț\"\nkind = \"number\"\n",
        ),
        (
            ".panastudio/content-models/premium.toml",
            "schemaVersion = 1\nid = \"premium\"\nlabel = \"Serviciu premium\"\n\n[[fields]]\nid = \"field_cost\"\nkey = \"cost\"\nlabel = \"Cost\"\nkind = \"number\"\n",
        ),
    ] {
        fs::write(root.join(relative_path), source).unwrap();
    }

    let fixture =
        crate::project_model::test_support::ProjectModelTestFixture::from_integration_disk_boundary(
            &root,
        )
        .unwrap();
    (root, fixture)
}

#[test]
fn catalog_projects_assignments_values_and_real_zola_output() {
    let (root, fixture) = integration_fixture("catalog");
    let graph = fixture.build_source_graph().unwrap();
    assert_eq!(graph.content_models.models.len(), 2);
    assert_eq!(graph.content_models.assignments.len(), 2);
    assert_eq!(graph.content_models.page_bindings.len(), 4);
    assert_eq!(
        graph
            .content_models
            .page_bindings
            .iter()
            .find(|binding| binding.page_file == "content/services/consultanta.md")
            .unwrap()
            .values["price"],
        serde_json::json!(120)
    );
    assert!(graph
        .content_models
        .page_bindings
        .iter()
        .any(|binding| binding.page_file == "content/services/consultanta.ro.md"));
    assert_eq!(graph.content_models.template_usages.len(), 4);
    assert!(graph
        .content_models
        .template_usages
        .iter()
        .all(|usage| usage.field_id == "field_price"));

    let build_output = root.join("public");
    crate::zola_engine::with_zola_engine("test build modele de conținut", || {
        let mut site = zola_site::Site::new(&root, Path::new("zola.toml"))
            .map_err(|error| error.to_string())?;
        site.set_output_path(&build_output);
        site.load().map_err(|error| error.to_string())?;
        site.build().map_err(|error| error.to_string())
    })
    .unwrap();
    let rendered_service =
        fs::read_to_string(build_output.join("services/consultanta/index.html")).unwrap();
    assert!(rendered_service.contains("120"));
    let rendered_archive = fs::read_to_string(build_output.join("services/index.html")).unwrap();
    assert!(rendered_archive.contains("120"));
    assert!(rendered_archive.contains("80"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn attach_model_and_renames_are_scoped_to_owned_files() {
    let (root, fixture) = integration_fixture("renames");
    let projection = fixture.projection();
    let graph = fixture.build_source_graph().unwrap();

    let attach = plan_content_model_mutation(
        &root,
        &graph,
        &projection.source_texts,
        &ContentModelMutationInput {
            operation: ContentModelMutationOperation::AttachModel {
                model_id: "service".to_string(),
                section_path: "content/portfolio/_index.md".to_string(),
            },
        },
    )
    .unwrap();
    assert_eq!(attach.plan.affected_keys, ["price"]);
    assert_eq!(attach.plan.affected_pages, ["content/portfolio/studiu.md"]);
    assert_eq!(attach.plan.warnings.len(), 2);
    assert!(!attach
        .changes
        .iter()
        .any(|change| change.relative_path == "content/portfolio/studiu.md"));

    let model_rename = plan_content_model_mutation(
        &root,
        &graph,
        &projection.source_texts,
        &ContentModelMutationInput {
            operation: ContentModelMutationOperation::RenameModel {
                model_id: "service".to_string(),
                new_id: "service_entry".to_string(),
                label: "Serviciu actualizat".to_string(),
                description: "Contract redenumit".to_string(),
            },
        },
    )
    .unwrap();
    assert!(!model_rename.plan.destructive);
    assert!(model_rename
        .deletes
        .iter()
        .any(|delete| delete.relative_path == model_path("service")));
    assert!(model_rename
        .changes
        .iter()
        .any(|change| change.relative_path == model_path("service_entry")));
    assert!(model_rename
        .changes
        .iter()
        .find(|change| change.relative_path == CONTENT_MODEL_ASSIGNMENTS_PATH)
        .unwrap()
        .contents
        .contains("modelId = \"service_entry\""));
    let field_rename = plan_content_model_mutation(
        &root,
        &graph,
        &projection.source_texts,
        &ContentModelMutationInput {
            operation: ContentModelMutationOperation::UpsertField {
                model_id: "service".to_string(),
                parent_field_id: None,
                original_field_id: Some("field_price".to_string()),
                field: field("field_price", "cost", ContentFieldKind::Number, vec![]),
            },
        },
    )
    .unwrap();
    assert!(!field_rename.plan.blocked);
    assert_eq!(
        field_rename.plan.affected_pages,
        [
            "content/articles/anunt.md",
            "content/services/audit.md",
            "content/services/consultanta.md",
            "content/services/consultanta.ro.md"
        ]
    );
    assert_eq!(field_rename.plan.affected_keys, ["cost", "price"]);
    for (path, expected) in [
        ("content/services/consultanta.md", "cost = 120"),
        ("content/services/consultanta.ro.md", "cost = 130"),
        ("templates/services/single.html", "page.extra.cost"),
        ("templates/services/list.html", "page.extra.cost"),
    ] {
        assert!(field_rename
            .changes
            .iter()
            .find(|change| change.relative_path == path)
            .unwrap()
            .contents
            .contains(expected));
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn replace_detach_and_shared_template_conflicts_remain_explicit() {
    let (root, fixture) = integration_fixture("destructive");
    let projection = fixture.projection();
    let graph = fixture.build_source_graph().unwrap();
    let replacement = plan_content_model_mutation(
        &root,
        &graph,
        &projection.source_texts,
        &ContentModelMutationInput {
            operation: ContentModelMutationOperation::ReplaceModel {
                section_path: "content/services/_index.md".to_string(),
                from_model_id: "service".to_string(),
                to_model_id: "premium".to_string(),
                field_migrations: BTreeMap::from([(
                    "field_price".to_string(),
                    "field_cost".to_string(),
                )]),
            },
        },
    )
    .unwrap();
    assert!(replacement.plan.destructive);
    assert!(!replacement.plan.blocked);
    assert_eq!(replacement.plan.affected_pages.len(), 3);
    let replacement_page = replacement
        .changes
        .iter()
        .find(|change| change.relative_path == "content/services/consultanta.md")
        .unwrap();
    assert!(replacement_page.contents.contains("cost = 120"));
    assert!(!replacement_page.contents.contains("price = 120"));
    assert!(!replacement.changes.iter().any(|change| {
        change.relative_path == "content/articles/anunt.md"
            || change.relative_path == "templates/articles/single.html"
            || change.relative_path == "templates/articles/list.html"
    }));
    assert!(replacement
        .changes
        .iter()
        .find(|change| change.relative_path == "templates/services/list.html")
        .unwrap()
        .contents
        .contains("page.extra.cost"));
    assert!(replacement
        .changes
        .iter()
        .find(|change| change.relative_path == CONTENT_MODEL_ASSIGNMENTS_PATH)
        .unwrap()
        .contents
        .contains("modelId = \"premium\""));

    let detach = plan_content_model_mutation(
        &root,
        &graph,
        &projection.source_texts,
        &ContentModelMutationInput {
            operation: ContentModelMutationOperation::DetachModel {
                model_id: "service".to_string(),
                section_path: "content/services/_index.md".to_string(),
            },
        },
    )
    .unwrap();
    assert!(detach.plan.destructive);
    assert!(detach.plan.blocked);
    assert_eq!(detach.plan.affected_keys, ["price"]);
    assert_eq!(detach.plan.template_usages.len(), 2);

    fs::create_dir_all(root.join("templates/shared")).unwrap();
    fs::write(
        root.join("templates/services/single.html"),
        "{% include \"shared/value.html\" %}",
    )
    .unwrap();
    fs::write(
        root.join("templates/articles/single.html"),
        "{% include \"shared/value.html\" %}",
    )
    .unwrap();
    fs::write(
        root.join("templates/shared/value.html"),
        "{{ page.extra.price }}",
    )
    .unwrap();
    let shared_fixture =
        crate::project_model::test_support::ProjectModelTestFixture::from_integration_disk_boundary(
            &root,
        )
        .unwrap();
    let shared_projection = shared_fixture.projection();
    let shared_graph = shared_fixture.build_source_graph().unwrap();
    let shared_replacement = plan_content_model_mutation(
        &root,
        &shared_graph,
        &shared_projection.source_texts,
        &ContentModelMutationInput {
            operation: ContentModelMutationOperation::ReplaceModel {
                section_path: "content/services/_index.md".to_string(),
                from_model_id: "service".to_string(),
                to_model_id: "premium".to_string(),
                field_migrations: BTreeMap::from([(
                    "field_price".to_string(),
                    "field_cost".to_string(),
                )]),
            },
        },
    )
    .unwrap();
    assert!(shared_replacement.plan.blocked);
    assert!(shared_replacement
        .plan
        .blockers
        .iter()
        .any(|blocker| blocker.contains("șabloane comune")));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn projected_delete_and_model_file_identity_fail_closed() {
    let (root, mut fixture) = integration_fixture("identity");
    fixture.delete(model_path("service"));
    let graph = fixture.build_source_graph().unwrap();
    assert!(!graph
        .content_models
        .models
        .iter()
        .any(|model| model.id == "service"));

    fs::write(
        root.join(model_path("alias")),
        "schemaVersion = 1\nid = \"service\"\nlabel = \"Alias invalid\"\n",
    )
    .unwrap();
    let mismatched =
        crate::source_graph::build_source_graph_from_integration_disk_boundary(&root).unwrap();
    assert!(mismatched
        .content_models
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "content_model_file_identity_mismatch"));
    fs::remove_dir_all(root).unwrap();
}
