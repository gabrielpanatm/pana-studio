use super::*;

#[test]
#[ignore = "probă manuală pentru build/cache EditorNavigation"]
fn editor_navigation_cache_pipeline_probe() {
    for (label, element_count) in [("small", 16usize), ("large", 1_024usize)] {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("pana-editor-navigation-pipeline-{label}-{stamp}"));
        let elements = (0..element_count)
            .map(|index| {
                format!("<section id=\"node-{index}\"><span>Node {index}</span></section>")
            })
            .collect::<String>();
        let fixture =
            ProjectModelTestFixture::standard_zola(&root, format!("<main>{elements}</main>\n"))
                .unwrap();
        let model = fixture.build_model().unwrap();

        let build_started = Instant::now();
        let snapshot = Arc::new(focused_snapshot(
            &root.canonicalize().unwrap(),
            &model,
            "templates/index.html",
        ));
        let build_elapsed = build_started.elapsed();
        let direct_serialization_started = Instant::now();
        let direct_serialized = serde_json::to_vec(snapshot.as_ref()).unwrap();
        let direct_serialization_elapsed = direct_serialization_started.elapsed();
        let arc_serialization_started = Instant::now();
        let serialized = serde_json::to_vec(&snapshot).unwrap();
        let arc_serialization_elapsed = arc_serialization_started.elapsed();
        assert_eq!(serialized, direct_serialized);
        let snapshot_bytes = serialized.len();
        let runtime = EditorNavigationRuntime::default();
        let insert_started = Instant::now();
        runtime
            .cache_snapshot(Some("templates/index.html"), None, Arc::clone(&snapshot))
            .unwrap();
        let insert_elapsed = insert_started.elapsed();
        let hit_started = Instant::now();
        let cached = runtime
            .cached_snapshot(
                &snapshot.identity,
                &snapshot.route,
                Some("templates/index.html"),
                None,
            )
            .unwrap()
            .unwrap();
        let hit_elapsed = hit_started.elapsed();
        eprintln!(
                "EDITOR_NAVIGATION_PIPELINE label={label} source_elements={element_count} snapshot_nodes={} planning_nodes={} snapshot_bytes={snapshot_bytes} build_us={} direct_serialization_us={} arc_serialization_us={} cache_insert_us={} cache_hit_us={}",
                snapshot.nodes.len(),
                snapshot.planning_nodes.len(),
                build_elapsed.as_micros(),
                direct_serialization_elapsed.as_micros(),
                arc_serialization_elapsed.as_micros(),
                insert_elapsed.as_micros(),
                hit_elapsed.as_micros(),
            );
        assert_eq!(cached.identity, snapshot.identity);
        assert_eq!(cached.nodes.len(), snapshot.nodes.len());
        assert!(Arc::ptr_eq(&cached, &snapshot));
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn snapshot_cache_shares_exact_allocation_and_revokes_by_runtime_lifecycle() {
    let runtime = EditorNavigationRuntime::default();
    let snapshot = Arc::new(editor_navigation_snapshot_for_test(
        canvas_identity(7),
        "model-7",
        "/",
        EditorNavigationSurface::CanonicalPreview,
        Vec::new(),
        Vec::new(),
    ));
    runtime
        .cache_snapshot(None, None, Arc::clone(&snapshot))
        .unwrap();
    let cached = runtime
        .cached_snapshot(&snapshot.identity, "/", None, None)
        .unwrap()
        .unwrap();
    assert!(Arc::ptr_eq(&snapshot, &cached));

    let mut other_session = snapshot.identity.clone();
    other_session.runtime_session_id = "runtime-2".to_string();
    assert!(runtime
        .cached_snapshot(&other_session, "/", None, None)
        .unwrap()
        .is_none());
    let mut other_revision = snapshot.identity.clone();
    other_revision.workspace_revision += 1;
    assert!(runtime
        .cached_snapshot(&other_revision, "/", None, None)
        .unwrap()
        .is_none());

    drop(cached);
    assert_eq!(Arc::strong_count(&snapshot), 2);
    runtime.revoke_all();
    assert_eq!(Arc::strong_count(&snapshot), 1);
}

#[test]
fn snapshot_indexes_nodes_once_without_exposing_the_index_over_serde() {
    let first = editable_boundary_node();
    let mut second = first.clone();
    second.id = "editor_boundary:boundary-2".to_string();
    let snapshot = editor_navigation_snapshot_for_test(
        canvas_identity(9),
        "model-9",
        "/",
        EditorNavigationSurface::CanonicalPreview,
        vec![first.id.clone(), second.id.clone()],
        vec![first, second.clone()],
    );

    assert_eq!(snapshot.node_index.get(&second.id), Some(&1));
    let found = editor_navigation_node(&snapshot, &second.id).expect("indexed node");
    assert!(std::ptr::eq(found, &snapshot.nodes[1]));

    let serialized = serde_json::to_value(&snapshot).unwrap();
    assert!(serialized.get("nodeIndex").is_none());
    assert!(serialized.get("planningNodes").is_none());
}

#[test]
fn access_resolution_promotes_every_surface_to_the_nearest_closed_boundary() {
    let mut outer = editable_boundary_node();
    outer.id = "editor_boundary:outer".to_string();
    outer.capabilities.requires_edit_scope_id = Some(outer.id.clone());
    outer.boundary.as_mut().unwrap().kind = EditorNavigationBoundaryKind::Component;
    outer.boundary.as_mut().unwrap().component_kind = Some(EditorNavigationComponentKind::Partial);

    let mut inner = outer.clone();
    inner.id = "editor_boundary:inner".to_string();
    inner.parent_id = Some(outer.id.clone());
    inner.capabilities.requires_edit_scope_id = Some(inner.id.clone());
    inner.boundary.as_mut().unwrap().boundary_instance_id = "inner".to_string();
    inner.boundary.as_mut().unwrap().component_kind = Some(EditorNavigationComponentKind::Repeat);

    let mut html = inner.clone();
    html.id = "editor_render:inside".to_string();
    html.parent_id = Some(inner.id.clone());
    html.kind = EditorNavigationNodeKind::HtmlElement;
    html.boundary = None;
    html.render_instance_id = Some("inside".to_string());
    html.capabilities.can_enter_boundary = false;
    html.capabilities.requires_edit_scope_id = Some(inner.id.clone());

    let snapshot = editor_navigation_snapshot_for_test(
        canvas_identity(10),
        "model-10",
        "/",
        EditorNavigationSurface::CanonicalPreview,
        vec![outer.id.clone()],
        vec![outer.clone(), inner.clone(), html.clone()],
    );

    assert_eq!(
        editor_navigation_access_node(&snapshot, &html.id, None).map(|node| node.id.as_str()),
        Some(inner.id.as_str())
    );
    assert_eq!(
        editor_navigation_access_node(&snapshot, &html.id, Some(&outer.id))
            .map(|node| node.id.as_str()),
        Some(inner.id.as_str())
    );
    assert_eq!(
        editor_navigation_access_node(&snapshot, &html.id, Some(&inner.id))
            .map(|node| node.id.as_str()),
        Some(html.id.as_str())
    );
}

#[test]
fn route_normalization_preserves_workbench_and_accepts_index_alias() {
    assert!(same_preview_route("/", "index.html"));
    assert!(same_preview_route("/blog/", "/blog/index.html"));
    assert!(same_preview_route(
        "/__pana_workbench/source/",
        "/__pana_workbench/source/?revision=1"
    ));
    assert!(!same_preview_route("/blog/", "/contact/"));
}

#[test]
fn edit_scope_grant_is_exact_and_removed_after_stale_use() {
    let runtime = EditorNavigationRuntime::default();
    let identity = canvas_identity(7);
    let node = editable_boundary_node();
    let grant = runtime
        .issue_edit_scope_grant(&identity, "model-7", "/", "templates/index.html", &node)
        .unwrap();

    assert!(runtime
        .require_edit_scope_grant(
            &grant,
            &identity,
            "model-7",
            "/",
            "templates/index.html",
            &node.id,
            EditScopeOperation::MoveHtmlInside,
        )
        .is_ok());
    assert!(runtime
        .require_edit_scope_grant(
            &grant,
            &canvas_identity(8),
            "model-8",
            "/",
            "templates/index.html",
            &node.id,
            EditScopeOperation::MoveHtmlInside,
        )
        .is_err());
    assert!(runtime
        .require_edit_scope_grant(
            &grant,
            &identity,
            "model-7",
            "/",
            "templates/index.html",
            &node.id,
            EditScopeOperation::MoveHtmlInside,
        )
        .is_err());
}

#[test]
fn editor_move_plan_token_is_single_use_and_revision_bound() {
    let runtime = EditorNavigationRuntime::default();
    let identity = canvas_identity(11);
    let plan = runtime
        .issue_editor_move_plan(EditorMovePlan {
            schema_version: EDITOR_MOVE_PLAN_SCHEMA_VERSION,
            token: None,
            allowed: true,
            reason_code: None,
            reason: None,
            operation: Some(EditorMoveOperation::HtmlSourceMove),
            identity: identity.clone(),
            model_revision: "model-11".to_string(),
            route: "/".to_string(),
            active_document_path: "templates/index.html".to_string(),
            source_node_id: "editor_render:source".to_string(),
            target_node_id: "editor_render:target".to_string(),
            position: ProjectMovePosition::After,
            impact: EditorMoveImpact {
                files: vec!["templates/index.html".to_string()],
                edit_scope_id: None,
                effect_scope: EditorNavigationEffectScope::SingleSource,
                rendered_instance_count: 1,
                affects_all_rendered_instances: false,
                requires_preview_reprojection: true,
            },
            live_projection: None,
            live_projection_reason: EditorMoveLiveProjectionReason::ExecutionNotHtml,
            issued_at_ms: 0,
        })
        .unwrap();
    let token = plan.token.clone().unwrap();

    assert!(runtime
        .consume_editor_move_plan(&token, &identity, "model-11", "/", "templates/index.html",)
        .is_ok());
    assert!(runtime
        .consume_editor_move_plan(&token, &identity, "model-11", "/", "templates/index.html",)
        .is_err());

    let stale = runtime.issue_editor_move_plan(plan).unwrap();
    assert!(runtime
        .consume_editor_move_plan(
            stale.token.as_deref().unwrap(),
            &canvas_identity(12),
            "model-12",
            "/",
            "templates/index.html",
        )
        .is_err());
}
