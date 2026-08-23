use super::*;

#[test]
fn focused_view_reroots_index_without_claiming_inherited_sources() {
    let root = editor_navigation_inheritance_test_project("focused-index");
    let model = editor_navigation_test_model(&root);
    let snapshot = focused_snapshot(&root, &model, "templates/index.html");
    let view = snapshot.focused_view.as_ref().expect("focused view");

    assert_eq!(view.active_document_path, "templates/index.html");
    assert_eq!(
        view.breadcrumbs
            .iter()
            .map(|entry| entry.document_path.as_str())
            .collect::<Vec<_>>(),
        vec![
            "templates/base.html",
            "templates/layout.html",
            "templates/index.html",
        ]
    );
    assert!(view
        .nodes
        .iter()
        .all(|node| node.file == "templates/index.html"));
    assert!(!view
        .nodes
        .iter()
        .any(|node| node.file == "templates/layout.html" || node.file == "templates/base.html"));

    assert!(!view.nodes.iter().any(|node| {
        matches!(
            node.source_kind,
            Some(SourceNodeKind::Extends | SourceNodeKind::Super | SourceNodeKind::TeraVariable)
        )
    }));
    assert!(!view
        .nodes
        .iter()
        .any(|node| node.kind == EditorNavigationViewNodeKind::Slot));
    assert!(view.root_node_ids.iter().all(|root_id| {
        view.nodes
            .iter()
            .find(|node| &node.id == root_id)
            .is_some_and(|node| node.source_kind != Some(SourceNodeKind::Block))
    }));

    let includes = view
        .nodes
        .iter()
        .filter(|node| node.source_kind == Some(SourceNodeKind::Include))
        .collect::<Vec<_>>();
    assert_eq!(includes.len(), 2);
    assert!(includes.iter().all(|node| {
        node.kind == EditorNavigationViewNodeKind::Boundary
            && node.capabilities.can_enter_boundary
            && node.boundary.as_ref().is_some_and(|boundary| {
                boundary.effect_scope == EditorNavigationEffectScope::SharedDefinition
                    && boundary.rendered_instance_count == 2
            })
            && node
                .relation
                .as_ref()
                .and_then(|relation| relation.target_document_path.as_deref())
                == Some("templates/partials/card.html")
    }));
    assert_ne!(includes[0].source_node_id, includes[1].source_node_id);

    for kind in [SourceNodeKind::For, SourceNodeKind::If] {
        let node = view
            .nodes
            .iter()
            .find(|node| node.source_kind == Some(kind.clone()))
            .unwrap_or_else(|| panic!("missing {kind:?}"));
        assert_eq!(node.kind, EditorNavigationViewNodeKind::Boundary);
        assert!(node.capabilities.can_enter_boundary);
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn focused_view_keeps_only_visual_layers_and_embedded_tera_gates() {
    let root = editor_navigation_inheritance_test_project("focused-visual-layers");
    let model = editor_navigation_test_model(&root);

    let index = focused_snapshot(&root, &model, "templates/index.html");
    let view = index.focused_view.as_ref().unwrap();
    for hidden_label in ["title", "description", "css_pagina", "scripts"] {
        assert!(
            !view.nodes.iter().any(|node| node.label == hidden_label),
            "blocul auxiliar {hidden_label:?} nu este strat vizual"
        );
    }
    assert!(!view.nodes.iter().any(|node| {
        matches!(
            node.source_kind,
            Some(SourceNodeKind::Extends | SourceNodeKind::Super | SourceNodeKind::TeraVariable)
        )
    }));
    assert!(view.nodes.iter().any(|node| {
        node.source_kind == Some(SourceNodeKind::Html) && node.tag.as_deref() == Some("section")
    }));

    let embedded = focused_snapshot(&root, &model, "templates/embedded.html");
    let embedded_view = embedded.focused_view.as_ref().unwrap();
    let promo = embedded_view
        .nodes
        .iter()
        .find(|node| node.source_kind == Some(SourceNodeKind::Block) && node.label == "promo")
        .expect("block-ul Tera din HTML rămâne gate vizual");
    assert_eq!(promo.kind, EditorNavigationViewNodeKind::Boundary);
    assert!(promo.capabilities.can_enter_boundary);
    assert_eq!(promo.children.len(), 1);
    let main = embedded_view
        .root_node_ids
        .iter()
        .find_map(|root_id| {
            embedded_view
                .nodes
                .iter()
                .find(|node| &node.id == root_id && node.tag.as_deref() == Some("main"))
        })
        .expect("structura HTML este rădăcina vizuală");
    assert_eq!(promo.parent_id.as_deref(), Some(main.id.as_str()));
    assert!(embedded_view
        .root_node_ids
        .iter()
        .any(|root_id| embedded_view
            .nodes
            .iter()
            .find(|node| &node.id == root_id)
            .is_some_and(|node| node.tag.as_deref() == Some("main"))));
    assert!(!embedded_view.nodes.iter().any(|node| node.label == "title"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn focused_view_keeps_complete_nested_html_after_leading_utf8_text() {
    let root = editor_navigation_inheritance_test_project("focused-leading-utf8");
    fs::write(
        root.join("templates/index.html"),
        concat!(
            "{% extends \"layout.html\" %}\n",
            "{% block content %}\n",
            "<section id=\"contact\">\n",
            "  <div class=\"container\">\n",
            "    <h2>Începe prin a modifica acest conținut.</h2>\n",
            "    <p>Selectează un element în preview.</p>\n",
            "  </div>\n",
            "</section>\n",
            "{% endblock %}\n",
        ),
    )
    .unwrap();
    let model = editor_navigation_test_model(&root);
    let snapshot = focused_snapshot(&root, &model, "templates/index.html");
    let view = snapshot.focused_view.as_ref().expect("focused view");
    let by_tag = |tag: &str| {
        view.nodes
            .iter()
            .find(|node| node.tag.as_deref() == Some(tag))
            .unwrap_or_else(|| panic!("missing <{tag}> in focused view"))
    };
    let section = by_tag("section");
    let div = by_tag("div");
    let heading = by_tag("h2");
    let paragraph = by_tag("p");

    assert_eq!(div.parent_id.as_deref(), Some(section.id.as_str()));
    assert_eq!(heading.parent_id.as_deref(), Some(div.id.as_str()));
    assert_eq!(paragraph.parent_id.as_deref(), Some(div.id.as_str()));
    assert_eq!(div.children, vec![heading.id.clone(), paragraph.id.clone()],);
    assert_eq!(view.root_node_ids, vec![section.id.clone()]);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn focused_view_changes_ownership_for_layout_base_and_partial() {
    let root = editor_navigation_inheritance_test_project("focused-documents");
    let model = editor_navigation_test_model(&root);

    let layout = focused_snapshot(&root, &model, "templates/layout.html");
    let layout_view = layout.focused_view.as_ref().unwrap();
    assert!(layout_view
        .nodes
        .iter()
        .all(|node| node.file == "templates/layout.html"));
    assert_eq!(
        layout_view
            .nodes
            .iter()
            .filter(|node| node.source_kind == Some(SourceNodeKind::Include))
            .count(),
        2
    );
    assert!(layout_view.nodes.iter().any(|node| {
        node.source_kind == Some(SourceNodeKind::Block)
            && node.kind == EditorNavigationViewNodeKind::Boundary
            && node.capabilities.can_enter_boundary
    }));

    let base = focused_snapshot(&root, &model, "templates/base.html");
    let base_view = base.focused_view.as_ref().unwrap();
    assert_eq!(base_view.breadcrumbs.len(), 1);
    assert!(base_view
        .nodes
        .iter()
        .all(|node| node.file == "templates/base.html"));

    let partial = focused_snapshot(&root, &model, "templates/partials/card.html");
    let partial_view = partial.focused_view.as_ref().unwrap();
    assert_eq!(partial_view.breadcrumbs.len(), 1);
    assert!(partial_view
        .nodes
        .iter()
        .all(|node| node.file == "templates/partials/card.html"));
    assert!(partial_view
        .nodes
        .iter()
        .filter(|node| node.source_kind == Some(SourceNodeKind::Html))
        .all(|node| node.capabilities.requires_edit_scope_id.is_none()));

    let macro_partial = focused_snapshot(&root, &model, "templates/partials/widget.html");
    let macro_view = macro_partial.focused_view.as_ref().unwrap();
    let macro_node = macro_view
        .nodes
        .iter()
        .find(|node| node.source_kind == Some(SourceNodeKind::Macro))
        .expect("macro boundary");
    assert!(macro_node.capabilities.can_enter_boundary);
    let nested_if = macro_view
        .nodes
        .iter()
        .find(|node| node.source_kind == Some(SourceNodeKind::If))
        .expect("nested if boundary");
    assert_eq!(
        nested_if.capabilities.requires_edit_scope_id,
        nested_if.editor_node_id
    );
    assert_eq!(nested_if.parent_id.as_deref(), Some(macro_node.id.as_str()));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn focused_view_preserves_theme_origin_and_local_override_resolution() {
    let root = editor_navigation_theme_test_project("focused-theme");
    let model = editor_navigation_test_model(&root);

    let index = focused_snapshot(&root, &model, "templates/index.html");
    let index_view = index.focused_view.as_ref().unwrap();
    assert_eq!(
        index_view
            .breadcrumbs
            .iter()
            .map(|entry| (entry.document_path.as_str(), entry.origin))
            .collect::<Vec<_>>(),
        vec![
            (
                "themes/test-theme/templates/base.html",
                EditorNavigationOrigin::Theme,
            ),
            ("templates/index.html", EditorNavigationOrigin::Project,),
        ]
    );
    assert!(!index_view
        .nodes
        .iter()
        .any(|node| node.source_kind == Some(SourceNodeKind::Extends)));

    let theme_base = focused_snapshot(&root, &model, "themes/test-theme/templates/base.html");
    let theme_view = theme_base.focused_view.as_ref().unwrap();
    assert!(theme_view.nodes.iter().all(|node| {
        node.origin == EditorNavigationOrigin::Theme
            && node.theme_name.as_deref() == Some("test-theme")
            && node.capabilities.read_only
            && !node.capabilities.can_enter_boundary
    }));
    let include = theme_view
        .nodes
        .iter()
        .find(|node| node.source_kind == Some(SourceNodeKind::Include))
        .unwrap();
    assert_eq!(
        include
            .relation
            .as_ref()
            .and_then(|relation| relation.target_document_path.as_deref()),
        Some("templates/partials/footer.html")
    );

    let override_partial = focused_snapshot(&root, &model, "templates/partials/footer.html");
    assert!(override_partial
        .focused_view
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .all(|node| node.origin == EditorNavigationOrigin::Project));
    fs::remove_dir_all(root).unwrap();
}
