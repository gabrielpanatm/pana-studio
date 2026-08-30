use super::*;

#[test]
fn navigation_snapshot_preserves_repeated_and_empty_boundary_instances() {
    let root = editor_navigation_test_project("snapshot-boundaries");
    let model = editor_navigation_test_model(&root);
    let loop_node = source_node(&model, SourceNodeKind::For, "for");
    let article = source_node(&model, SourceNodeKind::Html, "<article");
    let footer = source_node(&model, SourceNodeKind::Html, "<footer");
    let identity = CanvasProjectionIdentity {
        project_root: root.to_string_lossy().to_string(),
        runtime_session_id: "runtime-snapshot".to_string(),
        workspace_revision: 12,
        transaction_id: "canvas-snapshot".to_string(),
        preview_revision: "preview-snapshot".to_string(),
    };
    let render = |render_instance_id: &str, binding_key: &str, occurrence| CanvasRenderNode {
        render_instance_id: render_instance_id.to_string(),
        document_order: occurrence * 2 + 3,
        source_node_id: Some(article.id.clone()),
        template_source_node_id: Some(loop_node.id.clone()),
        parent_render_instance_id: None,
        provenance_stack: vec![loop_node.id.clone()],
        component_definition_ids: Vec::new(),
        component_invocation_ids: Vec::new(),
        block_definition_ids: Vec::new(),
        block_source_instance_ids: Vec::new(),
        dynamic_widget_provider_ids: Vec::new(),
        dynamic_widget_source_instance_ids: Vec::new(),
        binding_key: Some(binding_key.to_string()),
        binding_path: Some(format!("section.pages[{occurrence}]")),
        tag: "article".to_string(),
        occurrence,
        origin: CanvasNodeOrigin::Source,
        capabilities: CanvasNodeCapabilities {
            editable: true,
            inspectable: true,
            read_only: false,
        },
    };
    let boundary = |id: &str, root_id: Option<&str>, binding_key: Option<&str>, occurrence| {
        CanvasBoundaryInstance {
            boundary_instance_id: id.to_string(),
            document_order: occurrence * 2 + 2,
            source_node_id: loop_node.id.clone(),
            parent_boundary_instance_id: None,
            root_render_instance_ids: root_id
                .map(|root| vec![root.to_string()])
                .unwrap_or_default(),
            binding_key: binding_key.map(str::to_string),
            binding_path: binding_key.map(|key| format!("section.pages[{key}]")),
            occurrence,
            marker_kind: CanvasBoundaryMarkerKind::Source,
            markdown: None,
            closed: true,
        }
    };
    let mut footer_render = render("render-footer", "footer", 0);
    footer_render.document_order = 0;
    footer_render.source_node_id = Some(footer.id.clone());
    footer_render.template_source_node_id = None;
    footer_render.provenance_stack = vec![footer.id.clone()];
    footer_render.binding_key = None;
    footer_render.binding_path = None;
    let graph = CanvasGraph {
        schema_version: 1,
        workspace_revision: identity.workspace_revision,
        preview_revision: identity.preview_revision.clone(),
        model_revision: model.revision.clone(),
        documents: vec![CanvasDocumentGraph {
            route: "/".to_string(),
            nodes: vec![
                footer_render,
                render("render-alpha", "alpha", 0),
                render("render-beta", "beta", 1),
            ],
            boundaries: vec![
                boundary("boundary-alpha", Some("render-alpha"), Some("alpha"), 0),
                boundary("boundary-beta", Some("render-beta"), Some("beta"), 1),
                boundary("boundary-empty", None, None, 2),
            ],
        }],
        component_instances: Vec::new(),
        dynamic_widget_instances: Vec::new(),
        block_instances: Vec::new(),
        runtime_nodes: Vec::new(),
        diagnostics: Vec::new(),
    };

    let snapshot = build_editor_navigation_snapshot(
        identity,
        "/",
        &model,
        &graph,
        Some("templates/index.html"),
        None,
    )
    .unwrap();
    assert_eq!(
        snapshot.root_node_ids.first().map(String::as_str),
        Some(editor_render_node_id("render-footer").as_str())
    );
    let boundaries = snapshot
        .nodes
        .iter()
        .filter(|node| {
            node.kind == EditorNavigationNodeKind::Boundary
                && node.id.starts_with("editor_boundary:")
        })
        .collect::<Vec<_>>();
    assert_eq!(boundaries.len(), 3);
    assert_eq!(
        boundaries
            .iter()
            .map(|node| node.id.as_str())
            .collect::<HashSet<_>>()
            .len(),
        3
    );
    assert!(boundaries.iter().all(|node| {
        node.boundary.as_ref().is_some_and(|boundary| {
            boundary.kind == EditorNavigationBoundaryKind::Component
                && boundary.component_kind == Some(EditorNavigationComponentKind::Repeat)
                && boundary.rendered_instance_count == 3
                && boundary.effect_scope == EditorNavigationEffectScope::AllRenderedInstances
        })
    }));
    assert_eq!(
        boundaries
            .iter()
            .filter(|node| node
                .boundary
                .as_ref()
                .is_some_and(|boundary| boundary.empty))
            .count(),
        1
    );
    for render_node in snapshot.nodes.iter().filter(|node| {
        node.kind == EditorNavigationNodeKind::HtmlElement
            && node.id.starts_with("editor_render:")
            && node.render_instance_id.as_deref() != Some("render-footer")
    }) {
        assert!(render_node.capabilities.requires_edit_scope_id.is_some());
        assert!(!render_node.capabilities.can_move);
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn semantic_boundary_taxonomy_covers_templates_and_every_component_family() {
    let root = editor_navigation_inheritance_test_project("snapshot-boundary-taxonomy");
    let model = editor_navigation_test_model(&root);
    let template = source_node_in_file(
        &model,
        SourceNodeKind::Block,
        "content",
        "templates/index.html",
    );
    let include = source_node_in_file(
        &model,
        SourceNodeKind::Include,
        "card",
        "templates/index.html",
    );
    let component_definition = source_node_in_file(
        &model,
        SourceNodeKind::ComponentDefinition,
        "widget",
        "templates/partials/widget.html",
    );
    let repeat = source_node_in_file(&model, SourceNodeKind::For, "for", "templates/index.html");
    let conditional = source_node_in_file(&model, SourceNodeKind::If, "if", "templates/index.html");

    assert_eq!(
        editor_boundary_classification(&model, Some(template), false),
        (EditorNavigationBoundaryKind::Template, None)
    );
    for (source, component_kind) in [
        (include, EditorNavigationComponentKind::Partial),
        (
            component_definition,
            EditorNavigationComponentKind::TeraComponent,
        ),
        (repeat, EditorNavigationComponentKind::Repeat),
        (conditional, EditorNavigationComponentKind::Conditional),
    ] {
        assert_eq!(
            editor_boundary_classification(&model, Some(source), false),
            (
                EditorNavigationBoundaryKind::Component,
                Some(component_kind)
            )
        );
    }

    for (source_kind, component_kind) in [
        (
            SourceNodeKind::ComponentCall,
            EditorNavigationComponentKind::TeraComponent,
        ),
        (
            SourceNodeKind::Filter,
            EditorNavigationComponentKind::Transform,
        ),
    ] {
        let mut synthetic = template.clone();
        synthetic.kind = source_kind;
        assert_eq!(
            editor_boundary_classification(&model, Some(&synthetic), false),
            (
                EditorNavigationBoundaryKind::Component,
                Some(component_kind)
            )
        );
    }
    assert_eq!(
        editor_boundary_classification(&model, Some(template), true),
        (EditorNavigationBoundaryKind::Markdown, None)
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn navigation_snapshot_opens_only_the_active_document_wrapper_boundary() {
    let root = editor_navigation_inheritance_test_project("snapshot-active-document");
    let model = editor_navigation_test_model(&root);
    let index_content = source_node_in_file(
        &model,
        SourceNodeKind::Block,
        "content",
        "templates/index.html",
    );
    let hero = source_node_in_file(
        &model,
        SourceNodeKind::Html,
        "<section",
        "templates/index.html",
    );
    let card_include = source_node_in_file(
        &model,
        SourceNodeKind::Include,
        "card",
        "templates/index.html",
    );
    let card = source_node_in_file(
        &model,
        SourceNodeKind::Html,
        "<article",
        "templates/partials/card.html",
    );
    let layout_body = source_node_in_file(
        &model,
        SourceNodeKind::Block,
        "body",
        "templates/layout.html",
    );
    let layout_main = source_node_in_file(
        &model,
        SourceNodeKind::Html,
        "<main",
        "templates/layout.html",
    );
    let identity = CanvasProjectionIdentity {
        project_root: root.to_string_lossy().to_string(),
        runtime_session_id: "runtime-active-document".to_string(),
        workspace_revision: 21,
        transaction_id: "canvas-active-document".to_string(),
        preview_revision: "preview-active-document".to_string(),
    };
    let render = |id: &str, source: &SourceNode, order| CanvasRenderNode {
        render_instance_id: id.to_string(),
        document_order: order,
        source_node_id: Some(source.id.clone()),
        template_source_node_id: None,
        parent_render_instance_id: None,
        provenance_stack: vec![source.id.clone()],
        component_definition_ids: Vec::new(),
        component_invocation_ids: Vec::new(),
        block_definition_ids: Vec::new(),
        block_source_instance_ids: Vec::new(),
        dynamic_widget_provider_ids: Vec::new(),
        dynamic_widget_source_instance_ids: Vec::new(),
        binding_key: None,
        binding_path: None,
        tag: source_html_tag(&source.label).unwrap(),
        occurrence: 0,
        origin: CanvasNodeOrigin::Source,
        capabilities: CanvasNodeCapabilities {
            editable: true,
            inspectable: true,
            read_only: false,
        },
    };
    let boundary = |id: &str,
                    source: &SourceNode,
                    parent: Option<&str>,
                    root_render_instance_id: &str,
                    order| CanvasBoundaryInstance {
        boundary_instance_id: id.to_string(),
        document_order: order,
        source_node_id: source.id.clone(),
        parent_boundary_instance_id: parent.map(str::to_string),
        root_render_instance_ids: vec![root_render_instance_id.to_string()],
        binding_key: None,
        binding_path: None,
        occurrence: 0,
        marker_kind: CanvasBoundaryMarkerKind::Source,
        markdown: None,
        closed: true,
    };
    let graph = CanvasGraph {
        schema_version: 1,
        workspace_revision: identity.workspace_revision,
        preview_revision: identity.preview_revision.clone(),
        model_revision: model.revision.clone(),
        documents: vec![CanvasDocumentGraph {
            route: "/".to_string(),
            nodes: vec![
                render("render-hero", hero, 1),
                render("render-card", card, 3),
                render("render-layout-main", layout_main, 5),
            ],
            boundaries: vec![
                boundary("index-content", index_content, None, "render-hero", 0),
                boundary(
                    "index-card-include",
                    card_include,
                    Some("index-content"),
                    "render-card",
                    2,
                ),
                boundary("layout-body", layout_body, None, "render-layout-main", 4),
            ],
        }],
        component_instances: Vec::new(),
        dynamic_widget_instances: Vec::new(),
        block_instances: Vec::new(),
        runtime_nodes: Vec::new(),
        diagnostics: Vec::new(),
    };

    let index_snapshot = build_editor_navigation_snapshot(
        identity.clone(),
        "/",
        &model,
        &graph,
        Some("templates/index.html"),
        None,
    )
    .unwrap();
    let index_wrapper =
        editor_navigation_node(&index_snapshot, "editor_boundary:index-content").unwrap();
    assert!(!index_wrapper.capabilities.can_enter_boundary);
    assert!(index_wrapper.capabilities.requires_edit_scope_id.is_none());
    let hero_render = editor_navigation_node(&index_snapshot, "editor_render:render-hero").unwrap();
    assert!(hero_render.capabilities.requires_edit_scope_id.is_none());
    assert!(hero_render.capabilities.can_move);
    assert_eq!(
        enclosing_edit_scope(&index_snapshot, hero_render, false),
        None
    );
    let included_card =
        editor_navigation_node(&index_snapshot, "editor_render:render-card").unwrap();
    assert_eq!(
        included_card.capabilities.requires_edit_scope_id.as_deref(),
        Some("editor_boundary:index-card-include")
    );
    let foreign_layout =
        editor_navigation_node(&index_snapshot, "editor_render:render-layout-main").unwrap();
    assert_eq!(
        foreign_layout
            .capabilities
            .requires_edit_scope_id
            .as_deref(),
        Some("editor_boundary:layout-body")
    );

    let layout_snapshot = build_editor_navigation_snapshot(
        identity,
        "/",
        &model,
        &graph,
        Some("templates/layout.html"),
        None,
    )
    .unwrap();
    let layout_wrapper =
        editor_navigation_node(&layout_snapshot, "editor_boundary:layout-body").unwrap();
    assert!(!layout_wrapper.capabilities.can_enter_boundary);
    assert!(layout_wrapper.capabilities.requires_edit_scope_id.is_none());
    let layout_render =
        editor_navigation_node(&layout_snapshot, "editor_render:render-layout-main").unwrap();
    assert!(layout_render.capabilities.requires_edit_scope_id.is_none());
    assert_eq!(
        editor_navigation_node(&layout_snapshot, "editor_render:render-hero")
            .unwrap()
            .capabilities
            .requires_edit_scope_id
            .as_deref(),
        Some("editor_boundary:index-content")
    );
    assert_eq!(
        editor_navigation_node(&layout_snapshot, "editor_render:render-card")
            .unwrap()
            .capabilities
            .requires_edit_scope_id
            .as_deref(),
        Some("editor_boundary:index-card-include")
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn focused_layers_project_markdown_as_one_atomic_source_boundary() {
    let root = editor_navigation_test_project("focused-markdown");
    fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = \"Acasă\"\ntemplate = \"index.html\"\n+++\n## Titlu\n\nText cu [legătură](/).\n",
        )
        .unwrap();
    fs::write(
            root.join("templates/index.html"),
            "<main><header>Exterior</header>{{ section.content | safe }}<footer>Exterior</footer></main>",
        )
        .unwrap();
    let model = editor_navigation_test_model(&root);
    let projection = model
        .source_graph
        .markdown_projections
        .iter()
        .find(|projection| projection.kind == MarkdownProjectionKind::Body)
        .expect("section.content projection");
    let encoded_file = BASE64_STANDARD.encode("_index.md");
    let rendered = format!(
        concat!(
            "<main><header>Exterior</header>",
            "<!-- pana-markdown-start:{}:{} -->",
            "<h2>Titlu</h2><p>Text cu <a href=\"/\">legătură</a>.</p>",
            "<!-- pana-markdown-end:{} -->",
            "<footer>Exterior</footer></main>"
        ),
        projection.id, encoded_file, projection.id,
    );
    let graph = CanvasGraph::from_rendered_documents(
        &model,
        23,
        "preview-markdown-23",
        [("/", rendered.as_str())],
    )
    .unwrap();
    let identity = CanvasProjectionIdentity {
        project_root: root.to_string_lossy().to_string(),
        runtime_session_id: "runtime-focused-markdown".to_string(),
        workspace_revision: 23,
        transaction_id: "canvas-focused-markdown".to_string(),
        preview_revision: "preview-markdown-23".to_string(),
    };
    let snapshot = build_editor_navigation_snapshot(
        identity,
        "/",
        &model,
        &graph,
        Some("templates/index.html"),
        None,
    )
    .unwrap();
    let markdown = snapshot
        .nodes
        .iter()
        .find(|node| {
            node.kind == EditorNavigationNodeKind::Boundary
                && node
                    .boundary
                    .as_ref()
                    .is_some_and(|boundary| boundary.kind == EditorNavigationBoundaryKind::Markdown)
        })
        .expect("canonical Markdown boundary");
    assert_eq!(markdown.file.as_deref(), Some("content/_index.md"));
    assert!(markdown.capabilities.can_open_in_code);
    assert!(!markdown.capabilities.can_enter_boundary);
    assert!(!markdown.capabilities.can_move_atomic);
    assert!(!markdown.capabilities.can_move);
    assert!(!markdown.capabilities.can_edit_text);
    assert!(!markdown.capabilities.can_edit_attributes);
    assert!(markdown.capabilities.read_only);

    let view = snapshot.focused_view.as_ref().expect("focused Layers view");
    let markdown_layers = view
        .nodes
        .iter()
        .filter(|node| node.editor_node_id.as_deref() == Some(markdown.id.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(markdown_layers.len(), 1);
    assert_eq!(
        markdown_layers[0].kind,
        EditorNavigationViewNodeKind::Boundary
    );
    assert!(markdown_layers[0].children.is_empty());
    assert_eq!(markdown_layers[0].file, "content/_index.md");
    assert!(view
        .nodes
        .iter()
        .all(|node| { !matches!(node.tag.as_deref(), Some("h2" | "p" | "a")) }));

    let internal_render_nodes = snapshot
        .nodes
        .iter()
        .filter(|node| {
            node.kind == EditorNavigationNodeKind::HtmlElement
                && node.capabilities.requires_edit_scope_id.as_deref() == Some(markdown.id.as_str())
        })
        .count();
    assert!(internal_render_nodes >= 3);
    for exterior_tag in ["header", "footer"] {
        let exterior = view
            .nodes
            .iter()
            .find(|node| node.tag.as_deref() == Some(exterior_tag))
            .expect("exterior template HTML remains in Layers");
        assert!(exterior.capabilities.requires_edit_scope_id.is_none());
        assert!(exterior.capabilities.can_move);
    }

    let target = view
        .nodes
        .iter()
        .find(|node| node.tag.as_deref() == Some("footer"))
        .and_then(|node| node.editor_node_id.as_deref())
        .expect("exterior target");
    let blocked = plan_editor_move(
        &EditorNavigationRuntime::default(),
        &snapshot,
        &model,
        &markdown.id,
        target,
        ProjectMovePosition::Before,
        None,
    );
    assert!(!blocked.plan.allowed);
    assert_eq!(
        blocked.plan.reason_code.as_deref(),
        Some("editor_move_markdown_read_only")
    );

    let graph_after_reprojection = CanvasGraph::from_rendered_documents(
        &model,
        24,
        "preview-markdown-24",
        [("/", rendered.as_str())],
    )
    .unwrap();
    let snapshot_after_reprojection = build_editor_navigation_snapshot(
        CanvasProjectionIdentity {
            project_root: root.to_string_lossy().to_string(),
            runtime_session_id: "runtime-focused-markdown".to_string(),
            workspace_revision: 24,
            transaction_id: "canvas-focused-markdown-2".to_string(),
            preview_revision: "preview-markdown-24".to_string(),
        },
        "/",
        &model,
        &graph_after_reprojection,
        Some("templates/index.html"),
        None,
    )
    .unwrap();
    let markdown_after_reprojection = snapshot_after_reprojection
        .nodes
        .iter()
        .find(|node| {
            node.kind == EditorNavigationNodeKind::Boundary
                && node
                    .boundary
                    .as_ref()
                    .is_some_and(|boundary| boundary.kind == EditorNavigationBoundaryKind::Markdown)
        })
        .expect("reprojected Markdown boundary");
    assert_eq!(markdown.id, markdown_after_reprojection.id);
    assert_eq!(markdown.file, markdown_after_reprojection.file);
    assert_eq!(markdown.range, markdown_after_reprojection.range);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn empty_direct_fragment_projects_one_local_authoring_root() {
    let root = editor_navigation_test_project("empty-direct-fragment");
    fs::create_dir_all(root.join("templates/listing-items")).unwrap();
    fs::write(root.join("templates/listing-items/card.html"), "\n").unwrap();
    let model = editor_navigation_test_model(&root);
    let fragment = model
        .source_graph
        .nodes
        .iter()
        .find(|node| {
            node.kind == SourceNodeKind::Partial
                && node.file == "templates/listing-items/card.html"
                && node.parent.is_none()
        })
        .expect("listing item root");
    let rendered = format!(
        "<body><!-- pana-template-source-start:{} --><!-- pana-template-source-end:{} --></body>",
        fragment.id, fragment.id
    );
    let route = "/__pana_workbench/listing-item/";
    let graph = CanvasGraph::from_rendered_documents(
        &model,
        31,
        "preview-empty-fragment-31",
        [(route, rendered.as_str())],
    )
    .unwrap();
    let identity = CanvasProjectionIdentity {
        project_root: root.to_string_lossy().to_string(),
        runtime_session_id: "runtime-empty-fragment".to_string(),
        workspace_revision: 31,
        transaction_id: "canvas-empty-fragment".to_string(),
        preview_revision: "preview-empty-fragment-31".to_string(),
    };
    let snapshot = build_editor_navigation_snapshot(
        identity,
        route,
        &model,
        &graph,
        Some("templates/listing-items/card.html"),
        None,
    )
    .unwrap();

    let boundary = snapshot
        .nodes
        .iter()
        .find(|node| {
            node.kind == EditorNavigationNodeKind::Boundary
                && node.source_node_id.as_deref() == Some(fragment.id.as_str())
        })
        .expect("fragment root boundary");
    assert_eq!(boundary.source_kind, Some(SourceNodeKind::Partial));
    assert!(!boundary.capabilities.read_only);
    assert!(boundary.capabilities.requires_edit_scope_id.is_none());
    assert!(boundary
        .boundary
        .as_ref()
        .is_some_and(|boundary| boundary.empty));

    let view = snapshot
        .focused_view
        .as_ref()
        .expect("focused fragment view");
    assert_eq!(view.root_node_ids.len(), 1);
    let slot = view
        .nodes
        .iter()
        .find(|node| node.id == view.root_node_ids[0])
        .expect("empty fragment authoring slot");
    assert_eq!(slot.kind, EditorNavigationViewNodeKind::Slot);
    assert_eq!(slot.source_kind, Some(SourceNodeKind::Partial));
    assert_eq!(slot.source_node_id.as_deref(), Some(fragment.id.as_str()));
    assert!(!slot.capabilities.read_only);
    assert!(slot.capabilities.requires_edit_scope_id.is_none());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn html_attribute_facts_are_read_from_the_canonical_opening_tag() {
    let root = editor_navigation_test_project("source-html-facts");
    let model = editor_navigation_test_model(&root);
    let article = source_node_in_file(
        &model,
        SourceNodeKind::Html,
        "article",
        "templates/index.html",
    );
    let attributes = source_html_attributes(&model, Some(article)).expect("HTML facts");
    assert_eq!(attributes.get("class"), Some(&Some("card".to_string())));
    assert_eq!(attributes.len(), 1);
    fs::remove_dir_all(root).unwrap();
}
