use super::*;
use super::{
    provenance::{editor_source_provenance, markdown_source_provenance},
    view::{build_editor_navigation_view, same_editor_document_path, same_preview_route},
};

pub(crate) fn build_editor_navigation_snapshot(
    identity: CanvasProjectionIdentity,
    route: &str,
    model: &ProjectModel,
    graph: &CanvasGraph,
    active_document_path: Option<&str>,
    preview_context_render_instance_id: Option<&str>,
) -> Result<EditorNavigationSnapshot, String> {
    if identity.workspace_revision != graph.workspace_revision
        || identity.preview_revision != graph.preview_revision
    {
        return Err(
            "EditorNavigationSnapshot a refuzat un CanvasGraph cu altă revizie Preview."
                .to_string(),
        );
    }
    if model.revision != graph.model_revision {
        return Err(format!(
            "EditorNavigationSnapshot a refuzat ProjectModel {} pentru CanvasGraph {}.",
            model.revision, graph.model_revision
        ));
    }
    let document = graph
        .documents
        .iter()
        .find(|document| same_preview_route(&document.route, route))
        .ok_or_else(|| {
            format!("EditorNavigationSnapshot nu găsește ruta {route:?} în CanvasGraph-ul activ.")
        })?;
    let route = document.route.clone();
    let source_nodes = model
        .source_graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<HashMap<_, _>>();
    let implicitly_open_boundary_source_ids =
        implicitly_open_document_boundaries(model, active_document_path, &source_nodes);
    let render_nodes = document
        .nodes
        .iter()
        .map(|node| (node.render_instance_id.as_str(), node))
        .collect::<HashMap<_, _>>();
    let boundary_ids = document
        .boundaries
        .iter()
        .map(|boundary| {
            (
                boundary.boundary_instance_id.as_str(),
                editor_boundary_node_id(&boundary.boundary_instance_id),
            )
        })
        .collect::<HashMap<_, _>>();
    let boundary_by_id = document
        .boundaries
        .iter()
        .map(|boundary| (boundary.boundary_instance_id.as_str(), boundary))
        .collect::<HashMap<_, _>>();
    let boundary_depths = document
        .boundaries
        .iter()
        .map(|boundary| {
            (
                boundary.boundary_instance_id.as_str(),
                boundary_depth(boundary, &boundary_by_id),
            )
        })
        .collect::<HashMap<_, _>>();
    let boundary_counts =
        document
            .boundaries
            .iter()
            .fold(HashMap::<&str, usize>::new(), |mut counts, boundary| {
                *counts.entry(boundary.source_node_id.as_str()).or_default() += 1;
                counts
            });

    let mut boundaries_by_render_root = HashMap::<&str, Vec<&CanvasBoundaryInstance>>::new();
    for boundary in &document.boundaries {
        for root in &boundary.root_render_instance_ids {
            boundaries_by_render_root
                .entry(root.as_str())
                .or_default()
                .push(boundary);
        }
    }
    for boundaries in boundaries_by_render_root.values_mut() {
        boundaries.sort_by(|left, right| {
            let left_depth = boundary_depths
                .get(left.boundary_instance_id.as_str())
                .copied()
                .unwrap_or_default();
            let right_depth = boundary_depths
                .get(right.boundary_instance_id.as_str())
                .copied()
                .unwrap_or_default();
            right_depth
                .cmp(&left_depth)
                .then_with(|| left.boundary_instance_id.cmp(&right.boundary_instance_id))
        });
    }

    // A render node belongs to the boundary whose root is its nearest
    // ancestor. Walking the render ancestry once avoids the previous
    // nodes × boundaries × ancestry scan on large pages.
    let mut boundary_for_render = HashMap::<&str, &CanvasBoundaryInstance>::new();
    for render_node in &document.nodes {
        let mut cursor = Some(render_node.render_instance_id.as_str());
        let mut visited = HashSet::new();
        while let Some(current) = cursor {
            if !visited.insert(current) {
                break;
            }
            if let Some(boundary) = boundaries_by_render_root
                .get(current)
                .and_then(|boundaries| boundaries.first())
            {
                boundary_for_render.insert(render_node.render_instance_id.as_str(), *boundary);
                break;
            }
            cursor = render_nodes
                .get(current)
                .and_then(|node| node.parent_render_instance_id.as_deref());
        }
    }

    let mut nodes = Vec::with_capacity(document.nodes.len() + document.boundaries.len());
    for boundary in &document.boundaries {
        let markdown = boundary.markdown.as_ref();
        let source = source_nodes.get(boundary.source_node_id.as_str()).copied();
        let parent_id = boundary
            .parent_boundary_instance_id
            .as_deref()
            .and_then(|parent| boundary_ids.get(parent).cloned())
            .or_else(|| {
                boundary
                    .root_render_instance_ids
                    .iter()
                    .filter_map(|root| render_nodes.get(root.as_str()).copied())
                    .find_map(|root| {
                        root.parent_render_instance_id
                            .as_deref()
                            .map(editor_render_node_id)
                    })
            });
        let source_kind = source.map(|node| node.kind.clone());
        let implicitly_open = markdown.is_none()
            && implicitly_open_boundary_source_ids.contains(boundary.source_node_id.as_str());
        let can_enter = markdown.is_none()
            && !implicitly_open
            && source_kind.as_ref().is_some_and(editable_boundary_kind);
        let local_source = source.is_some_and(|node| node.origin == SourceOrigin::Local);
        let can_move_atomic = markdown.is_none()
            && local_source
            && source_kind.as_ref().is_some_and(movable_boundary_kind);
        let target = source
            .and_then(SourceNode::tera_template_target)
            .map(str::to_string);
        let effect_scope = if markdown.is_some() {
            EditorNavigationEffectScope::SingleSource
        } else {
            boundary_effect_scope(source_kind.as_ref())
        };
        let node_id = editor_boundary_node_id(&boundary.boundary_instance_id);
        let source_provenance = markdown_source_provenance(model, boundary, source);
        let (boundary_kind, component_kind) =
            editor_boundary_classification(model, source, markdown.is_some());
        let markdown_resolved = markdown.is_some_and(|markdown| {
            markdown.provenance_state == CanvasMarkdownProvenanceState::Resolved
        });
        nodes.push(EditorNavigationNode {
            id: node_id.clone(),
            parent_id,
            children: Vec::new(),
            order: boundary.document_order,
            kind: EditorNavigationNodeKind::Boundary,
            label: markdown
                .map(|markdown| markdown.kind.label().to_string())
                .or_else(|| source.map(|node| node.label.clone()))
                .unwrap_or_else(|| "Boundary Tera".to_string()),
            tag: None,
            source_node_id: Some(boundary.source_node_id.clone()),
            render_instance_id: None,
            source_kind,
            file: markdown
                .and_then(|markdown| markdown.source_file.clone())
                .or_else(|| source.map(|node| node.file.clone())),
            range: markdown
                .and_then(|markdown| markdown.source_range.clone())
                .or_else(|| source.and_then(|node| node.range.clone())),
            origin: source_origin(source),
            theme_name: source.and_then(|node| node.theme_name.clone()),
            source_provenance,
            provenance_stack: vec![boundary.source_node_id.clone()],
            component_definition_ids: Vec::new(),
            component_invocation_ids: Vec::new(),
            block_definition_ids: Vec::new(),
            block_source_instance_ids: Vec::new(),
            dynamic_widget_provider_ids: Vec::new(),
            dynamic_widget_source_instance_ids: Vec::new(),
            binding_key: boundary.binding_key.clone(),
            binding_path: boundary.binding_path.clone(),
            boundary: Some(EditorNavigationBoundary {
                kind: boundary_kind,
                component_kind,
                boundary_instance_id: boundary.boundary_instance_id.clone(),
                source_node_id: boundary.source_node_id.clone(),
                root_render_instance_ids: boundary.root_render_instance_ids.clone(),
                atomic_when_closed: true,
                effect_scope,
                rendered_instance_count: boundary_counts
                    .get(boundary.source_node_id.as_str())
                    .copied()
                    .unwrap_or(1),
                target,
                empty: boundary.root_render_instance_ids.is_empty(),
            }),
            capabilities: EditorNavigationCapabilities {
                can_select: true,
                can_inspect: true,
                can_open_in_code: if markdown.is_some() {
                    markdown_resolved
                } else {
                    source.is_some_and(|node| node.capabilities.can_open_in_code)
                },
                can_enter_boundary: can_enter,
                can_move_atomic,
                can_move: can_move_atomic,
                can_edit_text: false,
                can_edit_attributes: false,
                read_only: markdown.is_some() || !local_source,
                requires_edit_scope_id: can_enter.then_some(node_id),
                reason_code: if markdown.is_some() {
                    Some(if markdown_resolved {
                        SourceCapabilityReason::MarkdownRenderedBoundary
                    } else {
                        SourceCapabilityReason::MarkdownSourceUnresolved
                    })
                } else {
                    source.and_then(|node| node.capabilities.reason_code)
                },
            },
            source_html_attributes: None,
        });
    }

    for render_node in &document.nodes {
        let source = primary_source_node(render_node, &source_nodes);
        let containing_boundary = boundary_for_render
            .get(render_node.render_instance_id.as_str())
            .copied();
        let is_boundary_root = containing_boundary.is_some_and(|boundary| {
            boundary
                .root_render_instance_ids
                .iter()
                .any(|root| root == &render_node.render_instance_id)
        });
        let parent_id = if is_boundary_root {
            containing_boundary
                .map(|boundary| editor_boundary_node_id(&boundary.boundary_instance_id))
        } else {
            render_node
                .parent_render_instance_id
                .as_deref()
                .map(editor_render_node_id)
        };
        let requires_scope_id = containing_boundary
            .filter(|boundary| {
                boundary.markdown.is_some()
                    || (!implicitly_open_boundary_source_ids
                        .contains(boundary.source_node_id.as_str())
                        && source_nodes
                            .get(boundary.source_node_id.as_str())
                            .is_some_and(|node| editable_boundary_kind(&node.kind)))
            })
            .map(|boundary| editor_boundary_node_id(&boundary.boundary_instance_id));
        let source_capabilities = source.map(|node| &node.capabilities);
        let source_provenance =
            editor_source_provenance(model, source, &render_node.component_invocation_ids);
        let unlocked = requires_scope_id.is_none();
        let can_move = unlocked
            && render_node.capabilities.editable
            && source_capabilities.is_some_and(|capabilities| capabilities.can_move);
        let source_backed = source.is_some();
        nodes.push(EditorNavigationNode {
            id: editor_render_node_id(&render_node.render_instance_id),
            parent_id,
            children: Vec::new(),
            order: render_node.document_order,
            kind: if source_backed {
                EditorNavigationNodeKind::HtmlElement
            } else if render_node.origin == CanvasNodeOrigin::ArbitraryJsRuntime {
                EditorNavigationNodeKind::RuntimeElement
            } else {
                EditorNavigationNodeKind::HtmlElement
            },
            label: source
                .map(|node| node.label.clone())
                .unwrap_or_else(|| format!("<{}>", render_node.tag)),
            tag: Some(render_node.tag.clone()),
            source_node_id: source.map(|node| node.id.clone()),
            render_instance_id: Some(render_node.render_instance_id.clone()),
            source_kind: source.map(|node| node.kind.clone()),
            file: source.map(|node| node.file.clone()),
            range: source.and_then(|node| node.range.clone()),
            origin: render_origin(render_node, source),
            theme_name: source.and_then(|node| node.theme_name.clone()),
            source_provenance,
            provenance_stack: render_node.provenance_stack.clone(),
            component_definition_ids: render_node.component_definition_ids.clone(),
            component_invocation_ids: render_node.component_invocation_ids.clone(),
            block_definition_ids: render_node.block_definition_ids.clone(),
            block_source_instance_ids: render_node.block_source_instance_ids.clone(),
            dynamic_widget_provider_ids: render_node.dynamic_widget_provider_ids.clone(),
            dynamic_widget_source_instance_ids: render_node
                .dynamic_widget_source_instance_ids
                .clone(),
            binding_key: render_node.binding_key.clone(),
            binding_path: render_node.binding_path.clone(),
            boundary: None,
            capabilities: EditorNavigationCapabilities {
                can_select: true,
                can_inspect: render_node.capabilities.inspectable,
                can_open_in_code: source_capabilities
                    .is_some_and(|capabilities| capabilities.can_open_in_code),
                can_enter_boundary: false,
                can_move_atomic: false,
                can_move,
                can_edit_text: unlocked
                    && render_node.capabilities.editable
                    && source_capabilities.is_some_and(|capabilities| capabilities.can_edit_text),
                can_edit_attributes: unlocked
                    && render_node.capabilities.editable
                    && source_capabilities
                        .is_some_and(|capabilities| capabilities.can_edit_attributes),
                read_only: render_node.capabilities.read_only || !unlocked,
                requires_edit_scope_id: requires_scope_id,
                reason_code: source_capabilities.and_then(|capabilities| capabilities.reason_code),
            },
            source_html_attributes: source_html_attributes(model, source),
        });
    }

    let live_ids = nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<HashSet<_>>();
    for node in &mut nodes {
        if node
            .parent_id
            .as_ref()
            .is_some_and(|parent| !live_ids.contains(parent))
        {
            node.parent_id = None;
        }
    }
    let mut children_by_parent = nodes.iter().fold(
        HashMap::<String, Vec<(usize, String)>>::new(),
        |mut map, node| {
            if let Some(parent) = node.parent_id.as_ref() {
                map.entry(parent.clone())
                    .or_default()
                    .push((node.order, node.id.clone()));
            }
            map
        },
    );
    for children in children_by_parent.values_mut() {
        children.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    }
    for node in &mut nodes {
        if let Some(children) = children_by_parent.get(&node.id) {
            node.children = children.iter().map(|(_, child)| child.clone()).collect();
        }
    }
    let mut root_entries = nodes
        .iter()
        .filter(|node| node.parent_id.is_none())
        .map(|node| (node.order, node.id.clone()))
        .collect::<Vec<_>>();
    root_entries.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    let root_node_ids = root_entries
        .into_iter()
        .map(|(_, node_id)| node_id)
        .collect();
    let diagnostics = graph
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic
                .route
                .as_deref()
                .is_none_or(|diagnostic_route| same_preview_route(diagnostic_route, &route))
        })
        .map(|diagnostic| EditorNavigationDiagnostic {
            code: diagnostic.code.clone(),
            message: diagnostic.message.clone(),
            source_node_id: diagnostic.source_node_id.clone(),
        })
        .collect();

    let (focused_view, source_editor_nodes) = match active_document_path {
        Some(active_document_path) => {
            let (view, editor_nodes) = build_editor_navigation_view(
                model,
                &nodes,
                active_document_path,
                preview_context_render_instance_id,
            )?;
            (Some(view), editor_nodes)
        }
        None => (None, Vec::new()),
    };
    let node_index = build_editor_navigation_node_index(&nodes, &source_editor_nodes);
    Ok(EditorNavigationSnapshot {
        schema_version: EDITOR_NAVIGATION_SCHEMA_VERSION,
        identity,
        model_revision: model.revision.clone(),
        route: route.clone(),
        surface: if route.starts_with("/__pana_workbench/") {
            EditorNavigationSurface::TemplateWorkbench
        } else {
            EditorNavigationSurface::CanonicalPreview
        },
        root_node_ids,
        nodes,
        focused_view,
        diagnostics,
        planning_nodes: source_editor_nodes,
        node_index,
    })
}

pub(super) fn build_editor_navigation_node_index(
    nodes: &[EditorNavigationNode],
    planning_nodes: &[EditorNavigationNode],
) -> HashMap<String, usize> {
    nodes.iter().chain(planning_nodes.iter()).enumerate().fold(
        HashMap::new(),
        |mut index, (position, node)| {
            index.entry(node.id.clone()).or_insert(position);
            index
        },
    )
}

fn editor_boundary_node_id(boundary_instance_id: &str) -> String {
    format!("editor_boundary:{boundary_instance_id}")
}

pub(super) fn editor_render_node_id(render_instance_id: &str) -> String {
    format!("editor_render:{render_instance_id}")
}

fn boundary_depth(
    boundary: &CanvasBoundaryInstance,
    boundaries: &HashMap<&str, &CanvasBoundaryInstance>,
) -> usize {
    let mut depth = 0usize;
    let mut cursor = boundary.parent_boundary_instance_id.as_deref();
    let mut visited = HashSet::new();
    while let Some(parent) = cursor {
        if !visited.insert(parent) {
            break;
        }
        let Some(boundary) = boundaries.get(parent) else {
            break;
        };
        depth = depth.saturating_add(1);
        cursor = boundary.parent_boundary_instance_id.as_deref();
    }
    depth
}

fn implicitly_open_document_boundaries(
    model: &ProjectModel,
    active_document_path: Option<&str>,
    source_nodes: &HashMap<&str, &SourceNode>,
) -> HashSet<String> {
    let Some(active_document_path) = active_document_path else {
        return HashSet::new();
    };
    let Some(template) = model
        .source_graph
        .templates
        .iter()
        .find(|template| same_editor_document_path(&template.file, active_document_path))
    else {
        return HashSet::new();
    };

    source_nodes
        .values()
        .filter(|source| is_document_wrapper_block(source, template, source_nodes))
        .map(|source| source.id.clone())
        .collect()
}

pub(super) fn is_document_wrapper_block(
    source: &SourceNode,
    template: &SourceGraphTemplate,
    source_nodes: &HashMap<&str, &SourceNode>,
) -> bool {
    source.kind == SourceNodeKind::Block
        && source.file == template.file
        && source.parent.as_deref() == Some(template.node_id.as_str())
        && !source_is_inside_html(source, source_nodes)
}

pub(super) fn is_document_fragment_root(
    source: &SourceNode,
    template: &SourceGraphTemplate,
) -> bool {
    source.id == template.node_id
        && source.file == template.file
        && source.parent.is_none()
        && matches!(
            source.kind,
            SourceNodeKind::Template | SourceNodeKind::Partial
        )
}

fn source_is_inside_html(source: &SourceNode, source_nodes: &HashMap<&str, &SourceNode>) -> bool {
    let Some(source_range) = source.range.as_ref() else {
        return false;
    };
    source_nodes.values().any(|candidate| {
        candidate.file == source.file
            && candidate.kind == SourceNodeKind::Html
            && candidate.range.as_ref().is_some_and(|candidate_range| {
                candidate_range.start < source_range.start && source_range.end < candidate_range.end
            })
    })
}

fn primary_source_node<'a>(
    render_node: &CanvasRenderNode,
    source_nodes: &HashMap<&str, &'a SourceNode>,
) -> Option<&'a SourceNode> {
    render_node
        .source_node_id
        .as_deref()
        .or(render_node.template_source_node_id.as_deref())
        .and_then(|source_id| source_nodes.get(source_id).copied())
        .or_else(|| {
            render_node
                .provenance_stack
                .iter()
                .rev()
                .find_map(|source_id| source_nodes.get(source_id.as_str()).copied())
        })
}

pub(super) fn source_html_attributes(
    model: &ProjectModel,
    source: Option<&SourceNode>,
) -> Option<BTreeMap<String, Option<String>>> {
    let source = source.filter(|source| source.kind == SourceNodeKind::Html)?;
    let range = source.range.as_ref()?;
    let file = model
        .files
        .iter()
        .find(|file| file.relative_path == source.file)?;
    let opening = parse_html_tag_at(&file.contents, range.start)?;
    if opening.is_closing || opening.start != range.start {
        return None;
    }
    let opening_source = file.contents.get(opening.start..opening.end)?;
    Some(
        raw_tag_attributes(opening_source)
            .into_iter()
            .map(|attribute| (attribute.name, attribute.value))
            .collect(),
    )
}

fn editable_boundary_kind(kind: &SourceNodeKind) -> bool {
    matches!(
        kind,
        SourceNodeKind::Block
            | SourceNodeKind::Include
            | SourceNodeKind::ComponentDefinition
            | SourceNodeKind::ComponentCall
            | SourceNodeKind::For
            | SourceNodeKind::If
            | SourceNodeKind::Filter
    )
}

fn movable_boundary_kind(kind: &SourceNodeKind) -> bool {
    matches!(
        kind,
        SourceNodeKind::Block
            | SourceNodeKind::Include
            | SourceNodeKind::ComponentDefinition
            | SourceNodeKind::ComponentCall
            | SourceNodeKind::For
            | SourceNodeKind::If
            | SourceNodeKind::Filter
    )
}

pub(super) fn boundary_effect_scope(kind: Option<&SourceNodeKind>) -> EditorNavigationEffectScope {
    match kind {
        Some(
            SourceNodeKind::Include
            | SourceNodeKind::ComponentDefinition
            | SourceNodeKind::ComponentCall
            | SourceNodeKind::For
            | SourceNodeKind::If,
        ) => EditorNavigationEffectScope::AllRenderedInstances,
        Some(SourceNodeKind::Block) => EditorNavigationEffectScope::SharedDefinition,
        _ => EditorNavigationEffectScope::SingleSource,
    }
}

pub(super) fn source_origin(source: Option<&SourceNode>) -> EditorNavigationOrigin {
    match source.map(|source| &source.origin) {
        Some(SourceOrigin::Theme) => EditorNavigationOrigin::Theme,
        Some(SourceOrigin::Local) => EditorNavigationOrigin::Project,
        None => EditorNavigationOrigin::Tera,
    }
}

fn render_origin(
    render_node: &CanvasRenderNode,
    source: Option<&SourceNode>,
) -> EditorNavigationOrigin {
    if let Some(source) = source {
        return source_origin(Some(source));
    }
    match render_node.origin {
        CanvasNodeOrigin::Source => EditorNavigationOrigin::Project,
        CanvasNodeOrigin::Tera => EditorNavigationOrigin::Tera,
        CanvasNodeOrigin::PanaRuntime => EditorNavigationOrigin::PanaRuntime,
        CanvasNodeOrigin::ArbitraryJsRuntime => EditorNavigationOrigin::ArbitraryRuntime,
    }
}
