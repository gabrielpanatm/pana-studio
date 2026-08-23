use super::*;

#[test]
fn source_provenance_separates_include_definition_from_composition_site() {
    let root = editor_navigation_inheritance_test_project("source-provenance-include");
    let model = editor_navigation_test_model(&root);
    let include = source_node_in_file(
        &model,
        SourceNodeKind::Include,
        "partials/header.html",
        "templates/layout.html",
    );

    let provenance = editor_source_provenance(&model, Some(include), &[]);
    let definition = provenance.definition.as_ref().expect("include definition");
    let composition = provenance
        .composition
        .as_ref()
        .expect("include composition");
    assert_eq!(provenance.resolution, EditorSourceResolution::Resolved);
    assert_eq!(definition.file, "templates/partials/header.html");
    assert_eq!(definition.origin, EditorNavigationOrigin::Project);
    assert_eq!(composition.file, "templates/layout.html");
    assert_eq!(
        composition.source_node_id.as_deref(),
        Some(include.id.as_str())
    );
    assert!(definition.can_open_in_code);
    assert!(composition.can_open_in_code);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn source_provenance_keeps_partial_html_definition_and_include_composition() {
    let root = editor_navigation_inheritance_test_project("source-provenance-partial-html");
    let model = editor_navigation_test_model(&root);
    let include = source_node_in_file(
        &model,
        SourceNodeKind::Include,
        "partials/header.html",
        "templates/layout.html",
    );
    let header = source_node_in_file(
        &model,
        SourceNodeKind::Html,
        "<header>",
        "templates/partials/header.html",
    );
    let invocation = model
        .source_graph
        .component_graph
        .invocations
        .iter()
        .find(|invocation| invocation.source_node_id.as_deref() == Some(include.id.as_str()))
        .expect("header include invocation");

    let provenance =
        editor_source_provenance(&model, Some(header), std::slice::from_ref(&invocation.id));
    let definition = provenance.definition.as_ref().expect("header definition");
    let composition = provenance.composition.as_ref().expect("header composition");
    assert_eq!(provenance.resolution, EditorSourceResolution::Resolved);
    assert_eq!(definition.file, "templates/partials/header.html");
    assert_eq!(
        definition.source_node_id.as_deref(),
        Some(header.id.as_str()),
    );
    assert_eq!(composition.file, "templates/layout.html");
    assert_eq!(
        composition.source_node_id.as_deref(),
        Some(include.id.as_str()),
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn source_provenance_keeps_direct_html_as_its_definition() {
    let root = editor_navigation_inheritance_test_project("source-provenance-html");
    let model = editor_navigation_test_model(&root);
    let heading = source_node_in_file(&model, SourceNodeKind::Html, "<h1>", "templates/index.html");

    let provenance = editor_source_provenance(&model, Some(heading), &[]);
    let definition = provenance.definition.as_ref().expect("html definition");
    assert_eq!(provenance.resolution, EditorSourceResolution::Direct);
    assert_eq!(
        definition.source_node_id.as_deref(),
        Some(heading.id.as_str())
    );
    assert_eq!(definition.file, "templates/index.html");
    assert!(provenance.composition.is_none());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn source_provenance_respects_theme_origin_and_project_shadowing() {
    let root = editor_navigation_theme_test_project("source-provenance-theme");
    let model = editor_navigation_test_model(&root);
    let include = source_node_in_file(
        &model,
        SourceNodeKind::Include,
        "partials/footer.html",
        "themes/test-theme/templates/base.html",
    );

    let provenance = editor_source_provenance(&model, Some(include), &[]);
    let definition = provenance
        .definition
        .as_ref()
        .expect("shadowing definition");
    let composition = provenance.composition.as_ref().expect("theme composition");
    assert_eq!(definition.file, "templates/partials/footer.html");
    assert_eq!(definition.origin, EditorNavigationOrigin::Project);
    assert_eq!(composition.file, "themes/test-theme/templates/base.html",);
    assert_eq!(composition.origin, EditorNavigationOrigin::Theme);
    assert_eq!(composition.theme_name.as_deref(), Some("test-theme"));

    let theme_source = source_node_in_file(
        &model,
        SourceNodeKind::Block,
        "content",
        "themes/test-theme/templates/base.html",
    );
    let direct = editor_source_provenance(&model, Some(theme_source), &[]);
    assert_eq!(
        direct.definition.as_ref().map(|source| source.origin),
        Some(EditorNavigationOrigin::Theme),
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn source_provenance_reports_fallback_and_unresolved_includes() {
    let root = editor_navigation_test_project("source-provenance-resolution");
    fs::create_dir_all(root.join("templates/partials")).unwrap();
    fs::write(
        root.join("templates/index.html"),
        concat!(
            "{% include [\"partials/missing.html\", \"partials/fallback.html\"] %}\n",
            "{% include \"partials/unresolved.html\" %}\n",
        ),
    )
    .unwrap();
    fs::write(
        root.join("templates/partials/fallback.html"),
        "<aside>Fallback</aside>\n",
    )
    .unwrap();
    let model = editor_navigation_test_model(&root);
    let fallback = source_node_in_file(
        &model,
        SourceNodeKind::Include,
        "partials/missing.html",
        "templates/index.html",
    );
    let unresolved = source_node_in_file(
        &model,
        SourceNodeKind::Include,
        "partials/unresolved.html",
        "templates/index.html",
    );

    let fallback_provenance = editor_source_provenance(&model, Some(fallback), &[]);
    assert_eq!(
        fallback_provenance.resolution,
        EditorSourceResolution::FallbackResolved,
    );
    assert_eq!(
        fallback_provenance
            .definition
            .as_ref()
            .map(|source| source.file.as_str()),
        Some("templates/partials/fallback.html"),
    );
    assert_eq!(
        fallback_provenance
            .composition
            .as_ref()
            .map(|source| source.file.as_str()),
        Some("templates/index.html"),
    );

    let unresolved_provenance = editor_source_provenance(&model, Some(unresolved), &[]);
    assert_eq!(
        unresolved_provenance.resolution,
        EditorSourceResolution::Unresolved,
    );
    assert!(unresolved_provenance.definition.is_none());
    assert_eq!(
        unresolved_provenance
            .composition
            .as_ref()
            .and_then(|source| source.source_node_id.as_deref()),
        Some(unresolved.id.as_str()),
    );
    fs::remove_dir_all(root).unwrap();
}
