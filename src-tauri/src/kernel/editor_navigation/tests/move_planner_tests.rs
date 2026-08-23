use super::*;

#[test]
fn central_planner_keeps_closed_tera_atomic_and_requires_scope_for_html_children() {
    let root = editor_navigation_test_project("central-planner");
    let model = editor_navigation_test_model(&root);
    let loop_node = source_node(&model, SourceNodeKind::For, "for");
    let article = source_node(&model, SourceNodeKind::Html, "<article");
    let section = source_node(&model, SourceNodeKind::Html, "<section");
    let footer = source_node(&model, SourceNodeKind::Html, "<footer");
    let block = source_node(&model, SourceNodeKind::Block, "sidebar");
    let identity = CanvasProjectionIdentity {
        project_root: root.to_string_lossy().to_string(),
        runtime_session_id: "runtime-central-planner".to_string(),
        workspace_revision: 9,
        transaction_id: "canvas-central-planner".to_string(),
        preview_revision: "preview-central-planner".to_string(),
    };
    let scope_id = "editor_boundary:loop-instance".to_string();
    let mut boundary = editable_boundary_node();
    boundary.id = scope_id.clone();
    boundary.label = loop_node.label.clone();
    boundary.source_node_id = Some(loop_node.id.clone());
    boundary.source_kind = Some(loop_node.kind.clone());
    boundary.file = Some(loop_node.file.clone());
    boundary.range = loop_node.range.clone();
    boundary.children = vec![
        "editor_render:section-instance".to_string(),
        "editor_render:article-instance".to_string(),
    ];
    boundary.capabilities.requires_edit_scope_id = Some(scope_id.clone());
    boundary.capabilities.reason_code = loop_node.capabilities.reason_code;
    let semantic_boundary = boundary.boundary.as_mut().unwrap();
    semantic_boundary.boundary_instance_id = "loop-instance".to_string();
    semantic_boundary.source_node_id = loop_node.id.clone();
    semantic_boundary.effect_scope = EditorNavigationEffectScope::AllRenderedInstances;
    semantic_boundary.rendered_instance_count = 2;
    let mut block_boundary = editable_boundary_node();
    block_boundary.id = "editor_boundary:block-instance".to_string();
    block_boundary.label = block.label.clone();
    block_boundary.source_node_id = Some(block.id.clone());
    block_boundary.source_kind = Some(block.kind.clone());
    block_boundary.file = Some(block.file.clone());
    block_boundary.range = block.range.clone();
    block_boundary.capabilities.requires_edit_scope_id = Some(block_boundary.id.clone());
    block_boundary.capabilities.reason_code = block.capabilities.reason_code;
    let semantic_block = block_boundary.boundary.as_mut().unwrap();
    semantic_block.boundary_instance_id = "block-instance".to_string();
    semantic_block.source_node_id = block.id.clone();
    semantic_block.effect_scope = EditorNavigationEffectScope::SharedDefinition;

    let section_node = editor_html_node(section, "section-instance", Some(scope_id.clone()), 1);
    let article_node = editor_html_node(article, "article-instance", Some(scope_id.clone()), 2);
    let footer_node = editor_html_node(footer, "footer-instance", None, 3);
    let snapshot = EditorNavigationSnapshot {
        schema_version: EDITOR_NAVIGATION_SCHEMA_VERSION,
        identity: identity.clone(),
        model_revision: model.revision.clone(),
        route: "/".to_string(),
        surface: EditorNavigationSurface::CanonicalPreview,
        root_node_ids: vec![
            scope_id.clone(),
            footer_node.id.clone(),
            block_boundary.id.clone(),
        ],
        nodes: vec![
            boundary.clone(),
            section_node.clone(),
            article_node.clone(),
            footer_node.clone(),
            block_boundary.clone(),
        ],
        focused_view: Some(EditorNavigationView {
            active_document_path: "templates/index.html".to_string(),
            active_template_name: "index.html".to_string(),
            active_source_node_id: "template:index".to_string(),
            breadcrumbs: Vec::new(),
            root_node_ids: Vec::new(),
            nodes: Vec::new(),
            preview_context_render_instance_id: None,
        }),
        diagnostics: Vec::new(),
        planning_nodes: Vec::new(),
        node_index: HashMap::new(),
    };
    let runtime = EditorNavigationRuntime::default();

    let atomic = plan_editor_move(
        &runtime,
        &snapshot,
        &model,
        &boundary.id,
        &footer_node.id,
        ProjectMovePosition::After,
        None,
    );
    assert!(atomic.plan.allowed, "{:?}", atomic.plan.reason);
    assert_eq!(
        atomic.plan.operation,
        Some(EditorMoveOperation::AtomicTeraMove)
    );
    assert!(atomic.plan.live_projection.is_none());
    assert_eq!(
        atomic.plan.live_projection_reason,
        EditorMoveLiveProjectionReason::ExecutionNotHtml
    );
    assert_eq!(
        atomic.plan.impact.effect_scope,
        EditorNavigationEffectScope::AllRenderedInstances
    );
    assert!(atomic.plan.impact.affects_all_rendered_instances);

    let nested_atomic = plan_editor_move(
        &runtime,
        &snapshot,
        &model,
        &boundary.id,
        &block_boundary.id,
        ProjectMovePosition::Inside,
        None,
    );
    assert!(
        nested_atomic.plan.allowed,
        "{:?}",
        nested_atomic.plan.reason
    );
    assert_eq!(
        nested_atomic.plan.operation,
        Some(EditorMoveOperation::AtomicTeraMove)
    );

    let closed_child = plan_editor_move(
        &runtime,
        &snapshot,
        &model,
        &article_node.id,
        &section_node.id,
        ProjectMovePosition::Before,
        None,
    );
    assert!(!closed_child.plan.allowed);
    assert_eq!(
        closed_child.plan.reason_code.as_deref(),
        Some("editor_move_scope_required")
    );

    let grant = runtime
        .issue_edit_scope_grant(
            &identity,
            &model.revision,
            "/",
            "templates/index.html",
            &boundary,
        )
        .unwrap();
    let opened_child = plan_editor_move(
        &runtime,
        &snapshot,
        &model,
        &article_node.id,
        &section_node.id,
        ProjectMovePosition::Before,
        Some(&grant),
    );
    assert!(opened_child.plan.allowed, "{:?}", opened_child.plan.reason);
    assert_eq!(
        opened_child.plan.operation,
        Some(EditorMoveOperation::HtmlSourceMove)
    );
    assert_eq!(
        opened_child.plan.impact.edit_scope_id.as_deref(),
        Some(scope_id.as_str())
    );
    assert!(opened_child.plan.live_projection.is_some());
    assert_eq!(
        opened_child.plan.live_projection_reason,
        EditorMoveLiveProjectionReason::Ready
    );

    let mut component_snapshot = snapshot.clone();
    let mut component_node = article_node.clone();
    component_node.id = "editor_render:component-instance".to_string();
    component_node.render_instance_id = Some("component-instance".to_string());
    component_node.component_invocation_ids = vec!["component-invocation-1".to_string()];
    component_snapshot
        .nodes
        .retain(|node| node.id != article_node.id);
    component_snapshot.nodes.push(component_node.clone());
    let component_move = plan_editor_move(
        &runtime,
        &component_snapshot,
        &model,
        &component_node.id,
        &section_node.id,
        ProjectMovePosition::Before,
        Some(&grant),
    );
    assert!(component_move.plan.allowed);
    assert_eq!(
        component_move.plan.operation,
        Some(EditorMoveOperation::ComponentMove)
    );
    let component_projection = component_move
        .plan
        .live_projection
        .as_ref()
        .expect("ComponentMove HTML unic trebuie proiectat live");
    assert_eq!(
        component_projection.source_render_instance_id,
        "component-instance"
    );
    assert_eq!(
        component_move.plan.live_projection_reason,
        EditorMoveLiveProjectionReason::Ready
    );
    let issued_component_move = runtime
        .issue_editor_move_decision(component_move)
        .expect("plan ComponentMove tokenizat");
    assert_eq!(
        issued_component_move
            .live_projection
            .as_ref()
            .and_then(|projection| projection.plan_token.as_deref()),
        issued_component_move.token.as_deref()
    );

    let mut repeated_component_snapshot = component_snapshot.clone();
    let mut repeated_component_node = component_node.clone();
    repeated_component_node.id = "editor_render:component-instance-2".to_string();
    repeated_component_node.render_instance_id = Some("component-instance-2".to_string());
    repeated_component_snapshot
        .nodes
        .push(repeated_component_node);
    let repeated_component_move = plan_editor_move(
        &runtime,
        &repeated_component_snapshot,
        &model,
        &component_node.id,
        &section_node.id,
        ProjectMovePosition::Before,
        Some(&grant),
    );
    assert!(repeated_component_move.plan.allowed);
    assert!(repeated_component_move.plan.live_projection.is_none());
    assert_eq!(
        repeated_component_move.plan.live_projection_reason,
        EditorMoveLiveProjectionReason::MultipleRenderedInstances
    );

    let mut block_snapshot = snapshot.clone();
    let mut native_block_node = article_node.clone();
    native_block_node.id = "editor_render:native-block-instance".to_string();
    native_block_node.render_instance_id = Some("native-block-instance".to_string());
    native_block_node.block_source_instance_ids = vec!["block-source-instance-1".to_string()];
    block_snapshot
        .nodes
        .retain(|node| node.id != article_node.id);
    block_snapshot.nodes.push(native_block_node.clone());
    let native_block_move = plan_editor_move(
        &runtime,
        &block_snapshot,
        &model,
        &native_block_node.id,
        &section_node.id,
        ProjectMovePosition::Before,
        Some(&grant),
    );
    assert!(native_block_move.plan.allowed);
    assert_eq!(
        native_block_move.plan.operation,
        Some(EditorMoveOperation::BlockMove)
    );
    assert!(native_block_move.plan.live_projection.is_some());
    assert_eq!(
        native_block_move.plan.live_projection_reason,
        EditorMoveLiveProjectionReason::Ready
    );

    let cross_scope = plan_editor_move(
        &runtime,
        &snapshot,
        &model,
        &article_node.id,
        &footer_node.id,
        ProjectMovePosition::Before,
        Some(&grant),
    );
    assert!(!cross_scope.plan.allowed);
    assert_eq!(
        cross_scope.plan.reason_code.as_deref(),
        Some("editor_move_cross_scope")
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn focused_view_ids_drive_the_central_move_planner_and_scope_grants() {
    let root = editor_navigation_inheritance_test_project("focused-move");
    let model = editor_navigation_test_model(&root);
    let snapshot = focused_snapshot(&root, &model, "templates/index.html");
    let view = snapshot.focused_view.as_ref().unwrap();
    let runtime = EditorNavigationRuntime::default();

    let direct_paragraphs = view
        .nodes
        .iter()
        .filter(|node| {
            node.source_kind == Some(SourceNodeKind::Html)
                && node.tag.as_deref() == Some("p")
                && node.capabilities.requires_edit_scope_id.is_none()
        })
        .collect::<Vec<_>>();
    assert_eq!(direct_paragraphs.len(), 2);
    let direct_move = plan_editor_move(
        &runtime,
        &snapshot,
        &model,
        direct_paragraphs[1].editor_node_id.as_deref().unwrap(),
        direct_paragraphs[0].editor_node_id.as_deref().unwrap(),
        ProjectMovePosition::Before,
        None,
    );
    assert!(direct_move.plan.allowed, "{:?}", direct_move.plan.reason);
    assert_eq!(
        direct_move.plan.active_document_path,
        "templates/index.html"
    );

    let nested_spans = view
        .nodes
        .iter()
        .filter(|node| {
            node.source_kind == Some(SourceNodeKind::Html) && node.tag.as_deref() == Some("span")
        })
        .collect::<Vec<_>>();
    assert_eq!(nested_spans.len(), 2);
    let scope_id = nested_spans[0]
        .capabilities
        .requires_edit_scope_id
        .as_deref()
        .expect("if scope");
    assert_eq!(
        nested_spans[1]
            .capabilities
            .requires_edit_scope_id
            .as_deref(),
        Some(scope_id),
    );
    let closed_move = plan_editor_move(
        &runtime,
        &snapshot,
        &model,
        nested_spans[1].editor_node_id.as_deref().unwrap(),
        nested_spans[0].editor_node_id.as_deref().unwrap(),
        ProjectMovePosition::Before,
        None,
    );
    assert_eq!(
        closed_move.plan.reason_code.as_deref(),
        Some("editor_move_scope_required")
    );
    let scope = editor_navigation_node(&snapshot, scope_id).unwrap();
    let grant = runtime
        .issue_edit_scope_grant(
            &snapshot.identity,
            &snapshot.model_revision,
            &snapshot.route,
            "templates/index.html",
            scope,
        )
        .unwrap();
    let opened_move = plan_editor_move(
        &runtime,
        &snapshot,
        &model,
        nested_spans[1].editor_node_id.as_deref().unwrap(),
        nested_spans[0].editor_node_id.as_deref().unwrap(),
        ProjectMovePosition::Before,
        Some(&grant),
    );
    assert!(opened_move.plan.allowed, "{:?}", opened_move.plan.reason);
    fs::remove_dir_all(root).unwrap();
}
