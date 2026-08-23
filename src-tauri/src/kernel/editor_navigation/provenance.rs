use super::snapshot::source_origin;
use super::*;

pub(super) fn editor_source_provenance(
    model: &ProjectModel,
    selected_source: Option<&SourceNode>,
    component_invocation_ids: &[String],
) -> EditorSourceProvenance {
    let component_graph = &model.source_graph.component_graph;
    let direct_invocation = selected_source.and_then(|source| {
        component_graph.invocations.iter().find(|invocation| {
            invocation.source_node_id.as_deref() == Some(source.id.as_str())
                && matches!(
                    invocation.kind,
                    ComponentInvocationKind::Include
                        | ComponentInvocationKind::MacroCall
                        | ComponentInvocationKind::Shortcode
                )
        })
    });
    let ambient_invocation = component_invocation_ids
        .iter()
        .rev()
        .find_map(|invocation_id| {
            component_graph
                .invocations
                .iter()
                .find(|invocation| invocation.id == *invocation_id)
        });
    let invocation = direct_invocation.or(ambient_invocation);
    let composition = invocation
        .and_then(|invocation| invocation.source_node_id.as_deref())
        .and_then(|source_node_id| model.source_graph.node_by_id(source_node_id))
        .filter(|node| selected_source.is_none_or(|selected| selected.id != node.id))
        .map(editor_source_reference);

    if let Some(invocation) = direct_invocation {
        return EditorSourceProvenance {
            definition: resolved_component_definition_source(model, invocation),
            composition: selected_source.map(editor_source_reference),
            resolution: editor_source_resolution(&invocation.status),
        };
    }

    EditorSourceProvenance {
        definition: selected_source.map(editor_source_reference),
        composition,
        resolution: invocation
            .map(|invocation| editor_source_resolution(&invocation.status))
            .unwrap_or(EditorSourceResolution::Direct),
    }
}

pub(super) fn markdown_source_provenance(
    model: &ProjectModel,
    boundary: &CanvasBoundaryInstance,
    source: Option<&SourceNode>,
) -> EditorSourceProvenance {
    let Some(markdown) = boundary.markdown.as_ref() else {
        return editor_source_provenance(model, source, &[]);
    };
    let resolved = markdown.provenance_state == CanvasMarkdownProvenanceState::Resolved;
    let definition = resolved.then(|| EditorSourceReference {
        source_node_id: Some(boundary.source_node_id.clone()),
        source_kind: source.map(|node| node.kind.clone()),
        file: markdown.source_file.clone().unwrap_or_default(),
        range: markdown.source_range.clone(),
        label: markdown.kind.label().to_string(),
        origin: EditorNavigationOrigin::Project,
        theme_name: None,
        can_open_in_code: true,
    });
    let composition = model
        .source_graph
        .node_by_id(&markdown.template_source_node_id)
        .map(|node| {
            let mut reference = editor_source_reference(node);
            reference.range = markdown.template_range.clone().or(reference.range);
            reference
        })
        .or_else(|| {
            Some(EditorSourceReference {
                source_node_id: Some(markdown.template_source_node_id.clone()),
                source_kind: Some(SourceNodeKind::TeraVariable),
                file: markdown.template_file.clone(),
                range: markdown.template_range.clone(),
                label: "Proiecție Markdown".to_string(),
                origin: EditorNavigationOrigin::Tera,
                theme_name: None,
                can_open_in_code: true,
            })
        });
    EditorSourceProvenance {
        definition,
        composition,
        resolution: if resolved {
            EditorSourceResolution::Resolved
        } else {
            EditorSourceResolution::Unresolved
        },
    }
}

fn resolved_component_definition_source(
    model: &ProjectModel,
    invocation: &ComponentInvocation,
) -> Option<EditorSourceReference> {
    invocation
        .resolved_definition_ids
        .iter()
        .find_map(|definition_id| {
            model
                .source_graph
                .component_graph
                .definitions
                .iter()
                .find(|definition| definition.id == *definition_id)
        })
        .and_then(|definition| definition.source_node_id.as_deref())
        .and_then(|source_node_id| model.source_graph.node_by_id(source_node_id))
        .map(editor_source_reference)
}

pub(super) fn editor_source_reference(source: &SourceNode) -> EditorSourceReference {
    EditorSourceReference {
        source_node_id: Some(source.id.clone()),
        source_kind: Some(source.kind.clone()),
        file: source.file.clone(),
        range: source.range.clone(),
        label: source.label.clone(),
        origin: source_origin(Some(source)),
        theme_name: source.theme_name.clone(),
        can_open_in_code: source.capabilities.can_open_in_code,
    }
}

fn editor_source_resolution(status: &ComponentResolutionStatus) -> EditorSourceResolution {
    match status {
        ComponentResolutionStatus::Resolved => EditorSourceResolution::Resolved,
        ComponentResolutionStatus::FallbackResolved => EditorSourceResolution::FallbackResolved,
        ComponentResolutionStatus::Ambiguous => EditorSourceResolution::Ambiguous,
        ComponentResolutionStatus::Dynamic => EditorSourceResolution::Dynamic,
        ComponentResolutionStatus::External => EditorSourceResolution::External,
        ComponentResolutionStatus::Unresolved => EditorSourceResolution::Unresolved,
    }
}
