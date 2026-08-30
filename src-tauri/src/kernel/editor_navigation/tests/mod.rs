use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use std::{
    fs,
    path::PathBuf,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use crate::{
    preview::{CanvasBoundaryMarkerKind, CanvasDocumentGraph, CanvasNodeCapabilities},
    project_model::test_support::ProjectModelTestFixture,
    source_graph::model::MarkdownProjectionKind,
};

use super::*;

mod move_planner_tests;
mod provenance_tests;
mod runtime_tests;
mod snapshot_tests;
mod view_tests;

fn canvas_identity(workspace_revision: u64) -> CanvasProjectionIdentity {
    CanvasProjectionIdentity {
        project_root: "/project".to_string(),
        runtime_session_id: "runtime-1".to_string(),
        workspace_revision,
        transaction_id: "canvas-1".to_string(),
        preview_revision: "preview-1".to_string(),
    }
}
fn test_source_provenance(
    source_node_id: &str,
    file: &str,
    source_kind: SourceNodeKind,
) -> EditorSourceProvenance {
    EditorSourceProvenance {
        definition: Some(EditorSourceReference {
            source_node_id: Some(source_node_id.to_string()),
            source_kind: Some(source_kind),
            file: file.to_string(),
            range: None,
            label: source_node_id.to_string(),
            origin: EditorNavigationOrigin::Project,
            theme_name: None,
            can_open_in_code: true,
        }),
        composition: None,
        resolution: EditorSourceResolution::Direct,
    }
}

fn editable_boundary_node() -> EditorNavigationNode {
    EditorNavigationNode {
        id: "editor_boundary:boundary-1".to_string(),
        parent_id: None,
        children: Vec::new(),
        order: 0,
        kind: EditorNavigationNodeKind::Boundary,
        label: "content".to_string(),
        tag: None,
        source_node_id: Some("source-block-1".to_string()),
        render_instance_id: None,
        source_kind: Some(SourceNodeKind::Block),
        file: Some("templates/index.html".to_string()),
        range: None,
        origin: EditorNavigationOrigin::Project,
        theme_name: None,
        source_provenance: test_source_provenance(
            "source-block-1",
            "templates/index.html",
            SourceNodeKind::Block,
        ),
        provenance_stack: Vec::new(),
        component_definition_ids: Vec::new(),
        component_invocation_ids: Vec::new(),
        block_definition_ids: Vec::new(),
        block_source_instance_ids: Vec::new(),
        dynamic_widget_provider_ids: Vec::new(),
        dynamic_widget_source_instance_ids: Vec::new(),
        binding_key: None,
        binding_path: None,
        boundary: Some(EditorNavigationBoundary {
            kind: EditorNavigationBoundaryKind::Template,
            component_kind: None,
            boundary_instance_id: "boundary-1".to_string(),
            source_node_id: "source-block-1".to_string(),
            root_render_instance_ids: vec!["render-1".to_string()],
            atomic_when_closed: true,
            effect_scope: EditorNavigationEffectScope::SharedDefinition,
            rendered_instance_count: 1,
            target: None,
            empty: false,
        }),
        capabilities: EditorNavigationCapabilities {
            can_select: true,
            can_inspect: true,
            can_open_in_code: true,
            can_enter_boundary: true,
            can_move_atomic: true,
            can_move: true,
            can_edit_text: false,
            can_edit_attributes: false,
            read_only: false,
            requires_edit_scope_id: Some("editor_boundary:boundary-1".to_string()),
            reason_code: Some(SourceCapabilityReason::TeraBlock),
        },
        source_html_attributes: None,
    }
}
fn focused_snapshot(
    root: &std::path::Path,
    model: &ProjectModel,
    active_document_path: &str,
) -> EditorNavigationSnapshot {
    let identity = CanvasProjectionIdentity {
        project_root: root.to_string_lossy().to_string(),
        runtime_session_id: "runtime-focused".to_string(),
        workspace_revision: 17,
        transaction_id: "canvas-focused".to_string(),
        preview_revision: "preview-focused".to_string(),
    };
    let graph = CanvasGraph {
        schema_version: 1,
        workspace_revision: identity.workspace_revision,
        preview_revision: identity.preview_revision.clone(),
        model_revision: model.revision.clone(),
        documents: vec![CanvasDocumentGraph {
            route: "/".to_string(),
            nodes: Vec::new(),
            boundaries: Vec::new(),
        }],
        component_instances: Vec::new(),
        block_instances: Vec::new(),
        dynamic_widget_instances: Vec::new(),
        runtime_nodes: Vec::new(),
        diagnostics: Vec::new(),
    };
    build_editor_navigation_snapshot(
        identity,
        "/",
        model,
        &graph,
        Some(active_document_path),
        None,
    )
    .unwrap()
}

fn editor_html_node(
    source: &SourceNode,
    render_instance_id: &str,
    scope_id: Option<String>,
    order: usize,
) -> EditorNavigationNode {
    EditorNavigationNode {
        id: editor_render_node_id(render_instance_id),
        parent_id: scope_id.clone(),
        children: Vec::new(),
        order,
        kind: EditorNavigationNodeKind::HtmlElement,
        label: source.label.clone(),
        tag: source
            .label
            .strip_prefix('<')
            .and_then(|label| label.split([' ', '>', '.']).next())
            .map(str::to_string),
        source_node_id: Some(source.id.clone()),
        render_instance_id: Some(render_instance_id.to_string()),
        source_kind: Some(source.kind.clone()),
        file: Some(source.file.clone()),
        range: source.range.clone(),
        origin: EditorNavigationOrigin::Project,
        theme_name: None,
        source_provenance: EditorSourceProvenance {
            definition: Some(editor_source_reference(source)),
            composition: None,
            resolution: EditorSourceResolution::Direct,
        },
        provenance_stack: Vec::new(),
        component_definition_ids: Vec::new(),
        component_invocation_ids: Vec::new(),
        block_definition_ids: Vec::new(),
        block_source_instance_ids: Vec::new(),
        dynamic_widget_provider_ids: Vec::new(),
        dynamic_widget_source_instance_ids: Vec::new(),
        binding_key: None,
        binding_path: None,
        boundary: None,
        capabilities: EditorNavigationCapabilities {
            can_select: true,
            can_inspect: true,
            can_open_in_code: source.capabilities.can_open_in_code,
            can_enter_boundary: false,
            can_move_atomic: false,
            can_move: scope_id.is_none() && source.capabilities.can_move,
            can_edit_text: scope_id.is_none() && source.capabilities.can_edit_text,
            can_edit_attributes: scope_id.is_none() && source.capabilities.can_edit_attributes,
            read_only: scope_id.is_some(),
            requires_edit_scope_id: scope_id,
            reason_code: source.capabilities.reason_code,
        },
        source_html_attributes: None,
    }
}

fn source_node<'a>(model: &'a ProjectModel, kind: SourceNodeKind, label: &str) -> &'a SourceNode {
    model
        .source_graph
        .nodes
        .iter()
        .find(|node| node.kind == kind && node.label.contains(label))
        .unwrap_or_else(|| panic!("Lipsește nodul {kind:?} care conține {label:?}."))
}

fn source_node_in_file<'a>(
    model: &'a ProjectModel,
    kind: SourceNodeKind,
    label: &str,
    file: &str,
) -> &'a SourceNode {
    model
        .source_graph
        .nodes
        .iter()
        .find(|node| node.kind == kind && node.file == file && node.label.contains(label))
        .unwrap_or_else(|| panic!("Lipsește nodul {kind:?} din {file:?} care conține {label:?}."))
}

fn editor_navigation_test_model(root: &std::path::Path) -> ProjectModel {
    ProjectModelTestFixture::from_integration_disk_boundary(root)
        .unwrap()
        .build_model()
        .unwrap()
}

fn editor_navigation_test_project(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "pana-editor-navigation-{}-{label}-{stamp}",
        std::process::id()
    ));
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
            "<main>\n",
            "{% for item in section.pages %}\n",
            "  <section class=\"grid\"></section>\n",
            "  <article class=\"card\">{{ item.title }}</article>\n",
            "{% endfor %}\n",
            "<footer></footer>\n",
            "{% block sidebar %}<aside></aside>{% endblock %}\n",
            "</main>\n",
        ),
    )
    .unwrap();
    root
}

fn editor_navigation_inheritance_test_project(label: &str) -> PathBuf {
    let root = editor_navigation_test_project(label);
    fs::create_dir_all(root.join("templates/partials")).unwrap();
    fs::write(
        root.join("templates/base.html"),
        concat!(
            "<!doctype html><html><body>\n",
            "{% block body %}{% endblock %}\n",
            "</body></html>\n",
        ),
    )
    .unwrap();
    fs::write(
        root.join("templates/layout.html"),
        concat!(
            "{% extends \"base.html\" %}\n",
            "{% block body %}\n",
            "{% include \"partials/header.html\" %}\n",
            "<main>{% block content %}{% endblock %}</main>\n",
            "{% include \"partials/footer.html\" %}\n",
            "{% endblock %}\n",
        ),
    )
    .unwrap();
    fs::write(
        root.join("templates/index.html"),
        concat!(
            "{% extends \"layout.html\" %}\n",
            "{% block title %}{{ section.title }}{% endblock %}\n",
            "{% block description %}{{ config.title }}{% endblock %}\n",
            "{% block css_pagina %}{{ super() }}{% endblock %}\n",
            "{% block scripts %}{{ super() }}{% endblock %}\n",
            "{% block content %}\n",
            "<section class=\"hero\"><h1>Acasă</h1></section>\n",
            "<p>Primul</p><p>Al doilea</p>\n",
            "{% include \"partials/card.html\" %}\n",
            "{% include \"partials/card.html\" %}\n",
            "{% for item in section.pages %}\n",
            "{% if item.title %}<span>A</span><span>B</span>{% endif %}\n",
            "{% endfor %}\n",
            "{{ super() }}\n",
            "{% endblock %}\n",
        ),
    )
    .unwrap();
    fs::write(
        root.join("templates/embedded.html"),
        concat!(
            "<main>\n",
            "{% block promo %}<section class=\"promo\"></section>{% endblock %}\n",
            "</main>\n",
            "{% block title %}{{ config.title }}{% endblock %}\n",
        ),
    )
    .unwrap();
    fs::write(
        root.join("templates/partials/header.html"),
        "<header>Antet</header>\n",
    )
    .unwrap();
    fs::write(
        root.join("templates/partials/footer.html"),
        "<footer>Subsol</footer>\n",
    )
    .unwrap();
    fs::write(
        root.join("templates/partials/card.html"),
        "<article class=\"card\"><h2>Card</h2></article>\n",
    )
    .unwrap();
    fs::write(
        root.join("templates/partials/widget.html"),
        concat!(
            "{% component widget(value) %}\n",
            "{% if value %}<span>{{ value }}</span>{% endif %}\n",
            "{% endcomponent widget %}\n",
        ),
    )
    .unwrap();
    root
}

fn editor_navigation_theme_test_project(label: &str) -> PathBuf {
    let root = editor_navigation_test_project(label);
    fs::create_dir_all(root.join("templates/partials")).unwrap();
    fs::create_dir_all(root.join("themes/test-theme/templates")).unwrap();
    fs::write(
        root.join("zola.toml"),
        "base_url = \"http://example.test\"\ntheme = \"test-theme\"\n",
    )
    .unwrap();
    fs::write(
        root.join("templates/index.html"),
        concat!(
            "{% extends \"base.html\" %}\n",
            "{% block content %}<main>Local</main>{% endblock %}\n",
        ),
    )
    .unwrap();
    fs::write(
        root.join("themes/test-theme/templates/base.html"),
        concat!(
            "<body>\n",
            "{% include \"partials/footer.html\" %}\n",
            "{% block content %}{% endblock %}\n",
            "</body>\n",
        ),
    )
    .unwrap();
    fs::write(
        root.join("themes/test-theme/theme.toml"),
        "name = \"Test Theme\"\n",
    )
    .unwrap();
    fs::write(
        root.join("templates/partials/footer.html"),
        "<footer>Override local</footer>\n",
    )
    .unwrap();
    root
}
