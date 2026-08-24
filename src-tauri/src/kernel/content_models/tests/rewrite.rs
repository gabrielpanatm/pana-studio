use super::{support::*, *};

#[test]
fn toml_extra_rewrite_preserves_unknown_frontmatter_and_unmanaged_extra() {
    let source = "+++\ntitle = \"Serviciu\"\n[extra]\nkeep = \"da\"\nprice = 20\n+++\nCorp\n";
    let next = rewrite_extra_values(
        source,
        &BTreeSet::from(["price".to_string()]),
        &BTreeMap::from([("price".to_string(), serde_json::json!(35))]),
    )
    .unwrap();
    assert!(next.contains("title = \"Serviciu\""));
    assert!(next.contains("keep = \"da\""));
    assert!(next.contains("price = 35"));
    assert!(next.ends_with("Corp\n"));
}

#[test]
fn yaml_extra_rewrite_preserves_comments_formatting_and_unmanaged_values() {
    let source = concat!(
        "---\n",
        "# comentariu document\n",
        "title: Serviciu # titlu păstrat\n",
        "extra:\n",
        "  # comentariu independent\n",
        "  keep: da\n",
        "  price: 20\n",
        "  nested:\n",
        "    flag: true\n",
        "---\n",
        "Corp\n"
    );
    let next = rewrite_extra_values(
        source,
        &BTreeSet::from(["price".to_string()]),
        &BTreeMap::from([("price".to_string(), serde_json::json!(35))]),
    )
    .unwrap();
    assert!(next.contains("# comentariu document"));
    assert!(next.contains("title: Serviciu # titlu păstrat"));
    assert!(next.contains("  # comentariu independent"));
    assert!(next.contains("  keep: da"));
    assert!(next.contains("  nested:\n    flag: true"));
    assert!(next.contains("  price: 35"));
    assert!(!next.contains("price: 20"));
    assert!(next.ends_with("Corp\n"));
}

#[test]
fn yaml_inline_extra_is_rejected_instead_of_losing_comments() {
    let source = "---\ntitle: Serviciu\nextra: {keep: da, price: 20} # păstrează\n---\nCorp\n";
    let error = rewrite_extra_values(
        source,
        &BTreeSet::from(["price".to_string()]),
        &BTreeMap::from([("price".to_string(), serde_json::json!(35))]),
    )
    .unwrap_err();
    assert!(error.contains("forma inline"));
    assert!(error.contains("nu pierde"));
}

#[test]
fn yaml_detach_removes_empty_extra_but_preserves_document_and_body() {
    let source = concat!(
        "---\n",
        "title: Serviciu\n",
        "extra:\n",
        "  # explicație păstrată\n",
        "  price: 20\n",
        "---\n",
        "Corp\n"
    );
    let next = rewrite_extra_values(
        source,
        &BTreeSet::from(["price".to_string()]),
        &BTreeMap::new(),
    )
    .unwrap();
    assert!(next.contains("title: Serviciu"));
    assert!(next.contains("# explicație păstrată"));
    assert!(!next.contains("extra:"));
    assert!(!next.contains("price:"));
    assert!(next.ends_with("Corp\n"));
}

#[test]
fn detach_cleanup_removes_only_managed_keys() {
    let source = "+++\ntitle = \"Serviciu\"\n[extra]\nkeep = \"da\"\nprice = 20\n+++\n";
    let next = rewrite_extra_values(
        source,
        &BTreeSet::from(["price".to_string()]),
        &BTreeMap::new(),
    )
    .unwrap();
    assert!(next.contains("keep = \"da\""));
    assert!(!next.contains("price"));
}

#[test]
fn generated_field_identity_is_stable() {
    assert_eq!(
        stable_field_id("service", "price"),
        stable_field_id("service", "price")
    );
    assert_ne!(
        stable_field_id("service", "price"),
        stable_field_id("service", "color")
    );
}

#[cfg(unix)]
#[test]
fn workspace_projection_excludes_external_metadata_symlinks() {
    use std::os::unix::fs::symlink;

    let root = fixture_root("path-safety");
    let outside = fixture_root("path-safety-outside");
    fs::create_dir_all(root.join(".panastudio")).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("project.toml"), "schema_version = 1\n").unwrap();
    symlink(
        outside.join("project.toml"),
        root.join(CONTENT_MODEL_PROJECT_PATH),
    )
    .unwrap();
    symlink(&outside, root.join(CONTENT_MODEL_DIRECTORY)).unwrap();
    fs::create_dir_all(root.join("content")).unwrap();
    fs::create_dir_all(root.join("templates")).unwrap();
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
    fs::write(root.join("templates/index.html"), "<main>Acasă</main>").unwrap();

    let fixture =
            crate::project_model::test_support::ProjectModelTestFixture::from_integration_disk_boundary(
                &root,
            )
            .unwrap();
    let projection = fixture.projection();
    assert!(!projection
        .source_texts
        .contains_key(CONTENT_MODEL_PROJECT_PATH));
    assert!(!projection
        .source_texts
        .keys()
        .any(|path| path.starts_with(CONTENT_MODEL_DIRECTORY)));
    let graph = fixture.build_source_graph().unwrap();
    assert!(!graph.content_models.metadata_present);
    assert!(graph.content_models.models.is_empty());

    fs::remove_dir_all(&root).unwrap();
    fs::remove_dir_all(&outside).unwrap();
}

#[test]
fn typed_values_validate_constraints_groups_repeaters_and_unknown_keys() {
    let mut amount = field("field_amount", "amount", ContentFieldKind::Number, vec![]);
    amount.required = true;
    amount.minimum = Some(1.0);
    amount.maximum = Some(100.0);
    let mut status = field("field_status", "status", ContentFieldKind::Select, vec![]);
    status.choices = vec![ContentFieldChoice {
        value: "active".to_string(),
        label: "Activ".to_string(),
    }];
    let mut caption = field("field_caption", "caption", ContentFieldKind::Text, vec![]);
    caption.required = true;
    caption.pattern = Some(r"^[A-Z]".to_string());
    let gallery = field(
        "field_gallery",
        "gallery",
        ContentFieldKind::Repeater,
        vec![caption],
    );
    let mut model = ContentModelDefinition {
        schema_version: CONTENT_MODEL_SCHEMA_VERSION,
        id: "service".to_string(),
        label: "Serviciu".to_string(),
        description: String::new(),
        fields: vec![
            amount,
            status,
            field("field_url", "url", ContentFieldKind::Url, vec![]),
            field("field_date", "date", ContentFieldKind::Date, vec![]),
            field("field_color", "color", ContentFieldKind::Color, vec![]),
            gallery,
        ],
        file: model_path("service"),
    };
    validate_model(&mut model).unwrap();
    let valid = BTreeMap::from([
        ("amount".to_string(), serde_json::json!(20)),
        ("status".to_string(), serde_json::json!("active")),
        ("url".to_string(), serde_json::json!("/servicii/")),
        ("date".to_string(), serde_json::json!("2026-08-02")),
        ("color".to_string(), serde_json::json!("#18a36f")),
        (
            "gallery".to_string(),
            serde_json::json!([{"caption": "Imagine"}]),
        ),
    ]);
    validate_page_values(&model, &valid).unwrap();

    let mut invalid_number = valid.clone();
    invalid_number.insert("amount".to_string(), serde_json::json!(101));
    assert!(validate_page_values(&model, &invalid_number)
        .unwrap_err()
        .contains("în afara limitelor"));
    let mut invalid_repeater = valid.clone();
    invalid_repeater.insert("gallery".to_string(), serde_json::json!([{}]));
    assert!(validate_page_values(&model, &invalid_repeater)
        .unwrap_err()
        .contains("obligatoriu"));
    let mut unknown = valid;
    unknown.insert("legacy".to_string(), serde_json::json!(true));
    assert!(validate_page_values(&model, &unknown)
        .unwrap_err()
        .contains("nu aparține modelului"));
}

#[test]
fn nested_cleanup_preserves_siblings_across_groups_and_repeaters() {
    let mut group = serde_json::json!({
        "heading": "Titlu",
        "items": [
            {"label": "Unu", "url": "/unu"},
            {"label": "Doi", "url": "/doi"}
        ]
    });
    assert!(remove_nested_value(
        &mut group,
        &["items".to_string(), "url".to_string()]
    ));
    assert_eq!(
        group,
        serde_json::json!({
            "heading": "Titlu",
            "items": [{"label": "Unu"}, {"label": "Doi"}]
        })
    );
}

#[test]
fn nested_rename_migrates_each_repeater_item_without_overwrite() {
    let mut value = serde_json::json!([
        {"label": "Unu"},
        {"label": "Doi"}
    ]);
    assert!(
        rename_nested_value(&mut value, &["label".to_string()], &["title".to_string()]).unwrap()
    );
    assert_eq!(
        value,
        serde_json::json!([{"title": "Unu"}, {"title": "Doi"}])
    );

    let mut collision = serde_json::json!({"label": "Unu", "title": "Existent"});
    assert!(rename_nested_value(
        &mut collision,
        &["label".to_string()],
        &["title".to_string()]
    )
    .is_err());
}

#[test]
fn nested_schema_allows_repeated_keys_in_different_containers() {
    let mut model = ContentModelDefinition {
        schema_version: CONTENT_MODEL_SCHEMA_VERSION,
        id: "service".to_string(),
        label: "Serviciu".to_string(),
        description: String::new(),
        fields: vec![
            field(
                "",
                "hero",
                ContentFieldKind::Group,
                vec![field("", "title", ContentFieldKind::Text, vec![])],
            ),
            field(
                "",
                "card",
                ContentFieldKind::Group,
                vec![field("", "title", ContentFieldKind::Text, vec![])],
            ),
        ],
        file: model_path("service"),
    };
    validate_model(&mut model).unwrap();
    assert_ne!(model.fields[0].fields[0].id, model.fields[1].fields[0].id);
}

#[test]
fn tera_usage_scanner_tracks_dotted_and_bracket_paths_with_boundaries() {
    assert_eq!(
        expression_offsets(
            "{{ page.extra.hero.title }} {{ page.extra.hero.titleSuffix }}",
            "page.extra.hero.title"
        ),
        vec![3]
    );
    assert_eq!(
        expression_offsets(
            "{{ page.extra[\"hero\"][\"title\"] }}",
            "page.extra[\"hero\"][\"title\"]"
        ),
        vec![3]
    );
    assert_eq!(
        replace_expression_prefix(
            "{{ page.extra.hero.title }} {{ page.extra.hero.titleSuffix }}",
            "page.extra.hero.title",
            "page.extra.hero.heading"
        ),
        "{{ page.extra.hero.heading }} {{ page.extra.hero.titleSuffix }}"
    );
}
