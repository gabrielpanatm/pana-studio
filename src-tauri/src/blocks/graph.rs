use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
};

use crate::{
    blocks::{native_block_contract_definition, native_block_provider_definitions},
    localization::LocalizedDiagnostic,
    source_graph::model::{
        BlockDiagnostic, BlockGraph, BlockResolutionStatus, BlockSourceInstance,
        SourceDiagnosticSeverity, SourceGraph, SourceNodeKind,
    },
};

pub(crate) const BLOCK_GRAPH_SCHEMA_VERSION: u32 = 2;

pub(crate) fn build_block_graph(source_graph: &SourceGraph) -> BlockGraph {
    let definitions = native_block_provider_definitions()
        .iter()
        .map(native_block_contract_definition)
        .collect::<Vec<_>>();
    let source_instances = project_block_source_instances(source_graph, &definitions, None);
    let diagnostics = source_instances
        .iter()
        .flat_map(|instance| instance.diagnostics.iter().cloned())
        .collect();

    BlockGraph {
        schema_version: BLOCK_GRAPH_SCHEMA_VERSION,
        definitions,
        source_instances,
        diagnostics,
    }
}

pub(crate) fn upsert_block_graph_template(
    source_graph: &SourceGraph,
    graph: &mut BlockGraph,
    template_file: &str,
) {
    graph
        .source_instances
        .retain(|instance| instance.file != template_file);
    graph
        .source_instances
        .extend(project_block_source_instances(
            source_graph,
            &graph.definitions,
            Some(template_file),
        ));
    let node_order = source_graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    graph.source_instances.sort_by_key(|instance| {
        node_order
            .get(instance.source_node_id.as_str())
            .copied()
            .unwrap_or(usize::MAX)
    });
    graph.diagnostics = graph
        .source_instances
        .iter()
        .flat_map(|instance| instance.diagnostics.iter().cloned())
        .collect();
}

fn project_block_source_instances(
    source_graph: &SourceGraph,
    definitions: &[crate::source_graph::model::BlockDefinition],
    template_file: Option<&str>,
) -> Vec<BlockSourceInstance> {
    let definition_by_provider = definitions
        .iter()
        .map(|definition| (definition.provider_id.as_str(), definition.id.as_str()))
        .collect::<HashMap<_, _>>();
    source_graph
        .nodes
        .iter()
        .filter(|node| node.kind == SourceNodeKind::BlockMarker)
        .filter(|node| template_file.is_none_or(|file| node.file == file))
        .map(|node| {
            let provider_id = node.label.trim().to_string();
            let definition_id = definition_by_provider
                .get(provider_id.as_str())
                .map(|value| (*value).to_string());
            let status = if definition_id.is_some() {
                BlockResolutionStatus::Resolved
            } else {
                BlockResolutionStatus::UnknownProvider
            };
            let instance_diagnostics = if definition_id.is_some() {
                Vec::new()
            } else {
                vec![BlockDiagnostic {
                    code: "unknown_native_block_provider".to_string(),
                    diagnostic: LocalizedDiagnostic::new(
                        "blocks-diagnostic-unknown-native-provider",
                    )
                    .with_argument("provider", provider_id.clone()),
                    severity: SourceDiagnosticSeverity::Warning,
                    file: Some(node.file.clone()),
                    source_node_id: Some(node.id.clone()),
                }]
            };
            BlockSourceInstance {
                id: block_graph_id("source-instance", &[node.id.as_str()]),
                definition_id,
                provider_id,
                file: node.file.clone(),
                source_node_id: node.id.clone(),
                status,
                diagnostics: instance_diagnostics,
            }
        })
        .collect()
}

fn block_graph_id(prefix: &str, parts: &[&str]) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    "pana-block-graph-v1".hash(&mut hasher);
    prefix.hash(&mut hasher);
    for part in parts {
        part.hash(&mut hasher);
    }
    format!("{prefix}_{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use crate::source_graph::model::{
        BlockResolutionStatus, SourceCapabilities, SourceGraph, SourceNode, SourceNodeKind,
        SourceOrigin,
    };

    use super::*;

    fn graph_with_marker(provider_id: &str) -> SourceGraph {
        SourceGraph {
            node_index: Default::default(),
            project_root: "/tmp/project".to_string(),
            zola_root: "/tmp/project".to_string(),
            active_theme: None,
            pages: Vec::new(),
            templates: Vec::new(),
            styles: Vec::new(),
            scripts: Vec::new(),
            assets: Vec::new(),
            data_files: Vec::new(),
            structured_documents: Vec::new(),
            component_graph: Default::default(),
            block_graph: Default::default(),
            content_models: Default::default(),
            listing_items: Default::default(),
            dynamic_widget_graph: Default::default(),
            markdown_projections: Vec::new(),
            nodes: vec![SourceNode {
                id: "marker-1".to_string(),
                kind: SourceNodeKind::BlockMarker,
                file: "templates/index.html".to_string(),
                origin: SourceOrigin::Local,
                theme_name: None,
                label: provider_id.to_string(),
                range: None,
                parent: None,
                children: Vec::new(),
                capabilities: SourceCapabilities::code_only(
                    crate::source_graph::model::SourceCapabilityReason::NativeBlockMarker,
                ),
            }],
            relations: Vec::new(),
            asset_reference_coverage: Default::default(),
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn native_registry_and_source_instances_are_separate_from_component_graph() {
        let graph = build_block_graph(&graph_with_marker("accordion"));

        assert_eq!(graph.definitions.len(), 8);
        assert_eq!(graph.source_instances.len(), 1);
        assert_eq!(
            graph.source_instances[0].definition_id.as_deref(),
            Some("native/accordion")
        );
        assert_eq!(
            graph.source_instances[0].status,
            BlockResolutionStatus::Resolved
        );
        let accordion = graph
            .definitions
            .iter()
            .find(|definition| definition.provider_id == "accordion")
            .expect("accordion definition");
        assert!(accordion.capabilities.can_edit_properties);
        assert!(accordion.capabilities.supports_slots);
        assert_eq!(
            accordion.scale,
            crate::source_graph::model::BlockScale::Composition
        );
        assert_eq!(accordion.options.len(), 1);
    }

    #[test]
    fn unknown_provider_is_preserved_as_a_diagnostic_instance() {
        let graph = build_block_graph(&graph_with_marker("custom-widget"));

        assert_eq!(graph.source_instances.len(), 1);
        assert!(graph.source_instances[0].definition_id.is_none());
        assert_eq!(
            graph.source_instances[0].status,
            BlockResolutionStatus::UnknownProvider
        );
        assert_eq!(graph.diagnostics.len(), 1);
    }
}
