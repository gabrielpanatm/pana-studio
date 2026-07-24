use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
};

use crate::{
    blocks::{native_block_contract_definition, native_block_provider_definitions},
    source_graph::model::{
        BlockDiagnostic, BlockGraph, BlockResolutionStatus, BlockSourceInstance,
        SourceDiagnosticSeverity, SourceGraph, SourceNodeKind,
    },
};

pub(crate) const BLOCK_GRAPH_SCHEMA_VERSION: u32 = 1;

pub(crate) fn build_block_graph(source_graph: &SourceGraph) -> BlockGraph {
    let definitions = native_block_provider_definitions()
        .iter()
        .map(native_block_contract_definition)
        .collect::<Vec<_>>();
    let definition_by_provider = definitions
        .iter()
        .map(|definition| (definition.provider_id.as_str(), definition.id.as_str()))
        .collect::<HashMap<_, _>>();
    let mut diagnostics = Vec::new();
    let source_instances = source_graph
        .nodes
        .iter()
        .filter(|node| node.kind == SourceNodeKind::BlockMarker)
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
                    message: format!(
                        "Marcajul legacy `{provider_id}` nu corespunde niciunui provider de bloc nativ Rust."
                    ),
                    severity: SourceDiagnosticSeverity::Warning,
                    file: Some(node.file.clone()),
                    source_node_id: Some(node.id.clone()),
                }]
            };
            diagnostics.extend(instance_diagnostics.iter().cloned());
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
        .collect();

    BlockGraph {
        schema_version: BLOCK_GRAPH_SCHEMA_VERSION,
        definitions,
        source_instances,
        diagnostics,
    }
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
                capabilities: SourceCapabilities::code_only("test"),
            }],
            relations: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn native_registry_and_source_instances_are_separate_from_component_graph() {
        let graph = build_block_graph(&graph_with_marker("accordion"));

        assert_eq!(graph.definitions.len(), 6);
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
            crate::source_graph::model::BlockScale::Section
        );
        assert_eq!(accordion.options.len(), 1);
    }

    #[test]
    fn unknown_legacy_marker_is_preserved_as_a_diagnostic_instance() {
        let graph = build_block_graph(&graph_with_marker("legacy-widget"));

        assert_eq!(graph.source_instances.len(), 1);
        assert!(graph.source_instances[0].definition_id.is_none());
        assert_eq!(
            graph.source_instances[0].status,
            BlockResolutionStatus::UnknownProvider
        );
        assert_eq!(graph.diagnostics.len(), 1);
    }
}
