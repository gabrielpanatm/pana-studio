use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri_utils::html::{parse, serialize_node, NodeRef};
use walkdir::WalkDir;

use crate::{
    js::parse_page_js,
    project_model::model::ProjectModel,
    source_graph::model::{
        ComponentInvocationKind, RenderedBlockInstance, RenderedComponentInstance,
        SourceRelationKind,
    },
};

pub const CANVAS_PROJECTION_SCHEMA_VERSION: u32 = 1;
const MAX_CANVAS_DOCUMENTS: usize = 4096;
const MAX_CANVAS_NODES: usize = 250_000;
const MAX_CANVAS_RESOURCES: usize = 16_384;
const MAX_CANVAS_RESOURCE_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CanvasProjectionIdentity {
    pub project_root: String,
    pub runtime_session_id: String,
    pub workspace_revision: u64,
    pub transaction_id: String,
    pub preview_revision: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "camelCase")]
pub enum CanvasProjectionPhase {
    Prepared,
    ResourcesReady,
    Committed,
    StyledReady,
    CanonicalVerified,
    Failed,
}

#[derive(Clone, Copy, Debug, Serialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PreviewImpactKind {
    HtmlStructure,
    TeraRender,
    Styles,
    Assets,
    Scripts,
    Route,
    FullDocument,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreviewImpact {
    pub kinds: Vec<PreviewImpactKind>,
    pub paths: Vec<String>,
    pub requires_full_document: bool,
}

impl PreviewImpact {
    pub(crate) fn from_projected_paths(paths: &[String], baseline_rebuilt: bool) -> Self {
        let mut kinds = BTreeSet::new();
        let mut normalized = paths
            .iter()
            .map(|path| path.trim().replace('\\', "/"))
            .filter(|path| !path.is_empty())
            .collect::<BTreeSet<_>>();

        if baseline_rebuilt || normalized.is_empty() {
            kinds.insert(PreviewImpactKind::FullDocument);
        }

        for zola_path in &normalized {
            if zola_path.ends_with(".js") {
                kinds.insert(PreviewImpactKind::Scripts);
                continue;
            }
            if zola_path.ends_with(".css")
                || zola_path.ends_with(".scss")
                || zola_path.ends_with(".sass")
            {
                kinds.insert(PreviewImpactKind::Styles);
                continue;
            }
            if zola_path.starts_with("templates/") && zola_path.ends_with(".html") {
                kinds.insert(PreviewImpactKind::HtmlStructure);
                kinds.insert(PreviewImpactKind::TeraRender);
                continue;
            }
            if zola_path.starts_with("static/") {
                kinds.insert(PreviewImpactKind::Assets);
                continue;
            }
            if zola_path.starts_with("content/") {
                kinds.insert(PreviewImpactKind::Route);
                kinds.insert(PreviewImpactKind::FullDocument);
                continue;
            }
            kinds.insert(PreviewImpactKind::FullDocument);
        }

        let requires_full_document = kinds.contains(&PreviewImpactKind::FullDocument);
        Self {
            kinds: kinds.into_iter().collect(),
            paths: std::mem::take(&mut normalized).into_iter().collect(),
            requires_full_document,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CanvasNodeOrigin {
    Source,
    Tera,
    PanaRuntime,
    ArbitraryJsRuntime,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CanvasNodeCapabilities {
    pub editable: bool,
    pub inspectable: bool,
    pub read_only: bool,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CanvasRenderNode {
    pub render_instance_id: String,
    pub source_node_id: Option<String>,
    pub template_source_node_id: Option<String>,
    pub parent_render_instance_id: Option<String>,
    pub provenance_stack: Vec<String>,
    pub component_definition_ids: Vec<String>,
    pub component_invocation_ids: Vec<String>,
    pub block_definition_ids: Vec<String>,
    pub block_source_instance_ids: Vec<String>,
    pub binding_key: Option<String>,
    pub binding_path: Option<String>,
    pub tag: String,
    pub occurrence: usize,
    pub origin: CanvasNodeOrigin,
    pub capabilities: CanvasNodeCapabilities,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CanvasDocumentGraph {
    pub route: String,
    pub nodes: Vec<CanvasRenderNode>,
}

#[derive(Clone, Copy, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CanvasRuntimeKind {
    NativeBlock,
    Motion,
    ArbitraryScript,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CanvasRuntimeNode {
    pub runtime_node_id: String,
    pub script_source_node_id: String,
    pub script_file: String,
    pub routes: Vec<String>,
    pub key: String,
    pub kind: CanvasRuntimeKind,
    pub origin: CanvasNodeOrigin,
    pub capabilities: CanvasNodeCapabilities,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CanvasGraphDiagnostic {
    pub code: String,
    pub message: String,
    pub route: Option<String>,
    pub source_node_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CanvasGraph {
    pub schema_version: u32,
    pub workspace_revision: u64,
    pub preview_revision: String,
    pub model_revision: String,
    pub documents: Vec<CanvasDocumentGraph>,
    pub component_instances: Vec<RenderedComponentInstance>,
    pub block_instances: Vec<RenderedBlockInstance>,
    pub runtime_nodes: Vec<CanvasRuntimeNode>,
    pub diagnostics: Vec<CanvasGraphDiagnostic>,
}

struct CanvasSemanticIndex<'a> {
    live_source_ids: HashSet<&'a str>,
    definition_ids_by_source: HashMap<String, Vec<String>>,
    invocation_ids_by_source: HashMap<String, Vec<String>>,
    definition_ids_by_invocation: HashMap<String, Vec<String>>,
    block_definition_ids_by_source: HashMap<String, Vec<String>>,
    block_source_instance_ids_by_source: HashMap<String, Vec<String>>,
    block_definition_by_source_instance: HashMap<String, Option<String>>,
    binding_path_by_source: HashMap<String, String>,
    repeated_sources: HashSet<String>,
}

impl<'a> CanvasSemanticIndex<'a> {
    fn from_model(model: &'a ProjectModel) -> Self {
        let live_source_ids = model
            .source_graph
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<HashSet<_>>();
        let mut definition_ids_by_source = HashMap::<String, Vec<String>>::new();
        let mut invocation_ids_by_source = HashMap::<String, Vec<String>>::new();
        let mut definition_ids_by_invocation = HashMap::<String, Vec<String>>::new();
        let mut block_definition_ids_by_source = HashMap::<String, Vec<String>>::new();
        let mut block_source_instance_ids_by_source = HashMap::<String, Vec<String>>::new();
        let mut block_definition_by_source_instance = HashMap::<String, Option<String>>::new();
        let mut binding_path_by_source = HashMap::<String, String>::new();
        let mut repeated_sources = HashSet::new();
        let nodes_by_id = model
            .source_graph
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect::<HashMap<_, _>>();

        for definition in &model.source_graph.component_graph.definitions {
            if let Some(source_node_id) = definition.source_node_id.as_ref() {
                push_unique_map_value(
                    &mut definition_ids_by_source,
                    source_node_id,
                    &definition.id,
                );
                if definition.kind
                    == crate::source_graph::model::ComponentDefinitionKind::InlineRepeat
                {
                    repeated_sources.insert(source_node_id.clone());
                    if let Some(binding) = definition.data_bindings.first() {
                        binding_path_by_source.insert(source_node_id.clone(), binding.path.clone());
                    }
                }
            }
        }

        for invocation in &model.source_graph.component_graph.invocations {
            definition_ids_by_invocation.insert(
                invocation.id.clone(),
                invocation.resolved_definition_ids.clone(),
            );
            let Some(source_node_id) = invocation.source_node_id.as_ref() else {
                continue;
            };
            push_unique_map_value(
                &mut invocation_ids_by_source,
                source_node_id,
                &invocation.id,
            );
            for definition_id in &invocation.resolved_definition_ids {
                push_unique_map_value(&mut definition_ids_by_source, source_node_id, definition_id);
            }
            if invocation.kind == ComponentInvocationKind::Repeat {
                repeated_sources.insert(source_node_id.clone());
                if let Some(binding) = invocation.data_bindings.first() {
                    binding_path_by_source.insert(source_node_id.clone(), binding.path.clone());
                }
            }
        }

        for instance in &model.source_graph.block_graph.source_instances {
            block_definition_by_source_instance
                .insert(instance.id.clone(), instance.definition_id.clone());
            let rendered_source_id = nodes_by_id
                .get(instance.source_node_id.as_str())
                .and_then(|node| node.parent.as_ref())
                .unwrap_or(&instance.source_node_id);
            push_unique_map_value(
                &mut block_source_instance_ids_by_source,
                rendered_source_id,
                &instance.id,
            );
            if let Some(definition_id) = instance.definition_id.as_ref() {
                push_unique_map_value(
                    &mut block_definition_ids_by_source,
                    rendered_source_id,
                    definition_id,
                );
            }
        }

        Self {
            live_source_ids,
            definition_ids_by_source,
            invocation_ids_by_source,
            definition_ids_by_invocation,
            block_definition_ids_by_source,
            block_source_instance_ids_by_source,
            block_definition_by_source_instance,
            binding_path_by_source,
            repeated_sources,
        }
    }

    fn component_ids(&self, provenance: &[String]) -> (Vec<String>, Vec<String>) {
        let mut definitions = Vec::new();
        let mut invocations = Vec::new();
        for source_id in provenance {
            if let Some(values) = self.definition_ids_by_source.get(source_id) {
                push_unique_all(&mut definitions, values);
            }
            if let Some(values) = self.invocation_ids_by_source.get(source_id) {
                push_unique_all(&mut invocations, values);
            }
        }
        (definitions, invocations)
    }

    fn block_ids(&self, provenance: &[String]) -> (Vec<String>, Vec<String>) {
        let mut definitions = Vec::new();
        let mut source_instances = Vec::new();
        for source_id in provenance {
            if let Some(values) = self.block_definition_ids_by_source.get(source_id) {
                push_unique_all(&mut definitions, values);
            }
            if let Some(values) = self.block_source_instance_ids_by_source.get(source_id) {
                push_unique_all(&mut source_instances, values);
            }
        }
        (definitions, source_instances)
    }

    fn repeated_binding_path(&self, provenance: &[String]) -> Option<String> {
        provenance
            .iter()
            .rev()
            .find(|source_id| self.repeated_sources.contains(*source_id))
            .and_then(|source_id| self.binding_path_by_source.get(source_id))
            .cloned()
    }

    fn is_repeated(&self, provenance: &[String]) -> bool {
        provenance
            .iter()
            .any(|source_id| self.repeated_sources.contains(source_id))
    }
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|candidate| candidate == value) {
        values.push(value.to_string());
    }
}

fn push_unique_all(values: &mut Vec<String>, candidates: &[String]) {
    for candidate in candidates {
        push_unique(values, candidate);
    }
}

fn push_unique_map_value(values: &mut HashMap<String, Vec<String>>, key: &str, value: &str) {
    push_unique(values.entry(key.to_string()).or_default(), value);
}

impl CanvasGraph {
    pub(crate) fn annotate_rendered_document(
        model: &ProjectModel,
        route: &str,
        html: &str,
    ) -> Result<String, String> {
        let semantic_index = CanvasSemanticIndex::from_model(model);
        let document = parse(html.to_string());
        let mut occurrences = HashMap::new();
        let mut binding_occurrences = HashMap::new();
        let mut nodes = Vec::new();
        let mut diagnostics = Vec::new();
        let mut total_nodes = 0usize;
        let mut provenance_stack = Vec::new();
        collect_render_nodes(
            &document,
            route,
            None,
            &semantic_index,
            &mut provenance_stack,
            &mut occurrences,
            &mut binding_occurrences,
            &mut nodes,
            &mut diagnostics,
            &mut total_nodes,
        )?;
        String::from_utf8(serialize_node(&document))
            .map_err(|error| format!("CanvasGraph nu a putut serializa documentul anotat: {error}"))
    }

    pub(crate) fn from_rendered_documents<'a>(
        model: &ProjectModel,
        workspace_revision: u64,
        preview_revision: &str,
        documents: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Result<Self, String> {
        require_nonempty_preview_revision(preview_revision)?;
        let semantic_index = CanvasSemanticIndex::from_model(model);
        let mut result_documents = Vec::new();
        let mut diagnostics = Vec::new();
        let mut total_nodes = 0usize;

        for (route, html) in documents {
            if result_documents.len() >= MAX_CANVAS_DOCUMENTS {
                return Err(format!(
                    "CanvasGraph depășește limita de {MAX_CANVAS_DOCUMENTS} documente."
                ));
            }
            let document = parse(html.to_string());
            let mut nodes = Vec::new();
            let mut occurrences = HashMap::new();
            let mut binding_occurrences = HashMap::new();
            let mut provenance_stack = Vec::new();
            collect_render_nodes(
                &document,
                route,
                None,
                &semantic_index,
                &mut provenance_stack,
                &mut occurrences,
                &mut binding_occurrences,
                &mut nodes,
                &mut diagnostics,
                &mut total_nodes,
            )?;
            result_documents.push(CanvasDocumentGraph {
                route: route.to_string(),
                nodes,
            });
        }
        result_documents.sort_by(|left, right| left.route.cmp(&right.route));
        let component_instances = derive_component_instances(&semantic_index, &result_documents);
        let block_instances = derive_block_instances(&semantic_index, &result_documents);
        let runtime_nodes = derive_runtime_nodes(model, &result_documents);

        Ok(Self {
            schema_version: CANVAS_PROJECTION_SCHEMA_VERSION,
            workspace_revision,
            preview_revision: preview_revision.to_string(),
            model_revision: model.revision.clone(),
            documents: result_documents,
            component_instances,
            block_instances,
            runtime_nodes,
            diagnostics,
        })
    }
}

fn derive_block_instances(
    semantic_index: &CanvasSemanticIndex<'_>,
    documents: &[CanvasDocumentGraph],
) -> Vec<RenderedBlockInstance> {
    let mut pending = Vec::<(RenderedBlockInstance, Option<String>, usize)>::new();

    for document in documents {
        let nodes_by_render_id = document
            .nodes
            .iter()
            .map(|node| (node.render_instance_id.as_str(), node))
            .collect::<HashMap<_, _>>();

        for node in &document.nodes {
            let parent_node = node
                .parent_render_instance_id
                .as_deref()
                .and_then(|parent_id| nodes_by_render_id.get(parent_id))
                .copied();
            for (depth, source_instance_id) in node.block_source_instance_ids.iter().enumerate() {
                if parent_node.is_some_and(|parent| {
                    parent
                        .block_source_instance_ids
                        .iter()
                        .any(|candidate| candidate == source_instance_id)
                }) {
                    continue;
                }
                let definition_id = semantic_index
                    .block_definition_by_source_instance
                    .get(source_instance_id)
                    .cloned()
                    .flatten();
                pending.push((
                    rendered_block_instance(
                        document,
                        node,
                        definition_id,
                        Some(source_instance_id.clone()),
                    ),
                    node.parent_render_instance_id.clone(),
                    depth,
                ));
            }
        }
    }

    let mut instance_indices_by_render_id = HashMap::<String, Vec<usize>>::new();
    for (index, (instance, _, _)) in pending.iter().enumerate() {
        instance_indices_by_render_id
            .entry(instance.render_instance_id.clone())
            .or_default()
            .push(index);
    }
    let render_parents = documents
        .iter()
        .flat_map(|document| {
            document.nodes.iter().map(|node| {
                (
                    node.render_instance_id.clone(),
                    node.parent_render_instance_id.clone(),
                )
            })
        })
        .collect::<HashMap<_, _>>();

    for index in 0..pending.len() {
        let (render_instance_id, render_parent_id, depth) = {
            let (instance, parent, depth) = &pending[index];
            (instance.render_instance_id.clone(), parent.clone(), *depth)
        };
        let local_parent = if depth > 0 {
            instance_indices_by_render_id
                .get(&render_instance_id)
                .and_then(|indices| {
                    indices
                        .iter()
                        .copied()
                        .filter(|candidate| *candidate != index && pending[*candidate].2 < depth)
                        .max_by_key(|candidate| pending[*candidate].2)
                })
        } else {
            None
        };
        let ancestor_parent = local_parent.or_else(|| {
            let mut cursor = render_parent_id;
            while let Some(render_id) = cursor {
                if let Some(candidate) = instance_indices_by_render_id
                    .get(&render_id)
                    .and_then(|indices| indices.last())
                    .copied()
                {
                    return Some(candidate);
                }
                cursor = render_parents.get(&render_id).cloned().flatten();
            }
            None
        });
        pending[index].0.parent_instance_id =
            ancestor_parent.map(|parent| pending[parent].0.id.clone());
    }

    let mut instances = pending
        .into_iter()
        .map(|(instance, _, _)| instance)
        .collect::<Vec<_>>();
    instances.sort_by(|left, right| {
        left.route
            .cmp(&right.route)
            .then_with(|| left.render_instance_id.cmp(&right.render_instance_id))
            .then_with(|| left.source_instance_id.cmp(&right.source_instance_id))
            .then_with(|| left.definition_id.cmp(&right.definition_id))
    });
    instances
}

fn rendered_block_instance(
    document: &CanvasDocumentGraph,
    node: &CanvasRenderNode,
    definition_id: Option<String>,
    source_instance_id: Option<String>,
) -> RenderedBlockInstance {
    let mut hasher = Sha256::new();
    hasher.update(b"block-instance");
    hasher.update([0]);
    hasher.update(document.route.as_bytes());
    hasher.update([0]);
    hasher.update(node.render_instance_id.as_bytes());
    hasher.update([0]);
    if let Some(source_instance_id) = source_instance_id.as_deref() {
        hasher.update(source_instance_id.as_bytes());
    }
    hasher.update([0]);
    if let Some(definition_id) = definition_id.as_deref() {
        hasher.update(definition_id.as_bytes());
    }
    RenderedBlockInstance {
        id: format!("block_instance_{}", short_hex(&hasher.finalize())),
        definition_id,
        source_instance_id,
        render_instance_id: node.render_instance_id.clone(),
        route: document.route.clone(),
        source_node_id: node
            .source_node_id
            .clone()
            .or_else(|| node.template_source_node_id.clone()),
        parent_instance_id: None,
        binding_key: node.binding_key.clone(),
        binding_path: node.binding_path.clone(),
    }
}

fn derive_component_instances(
    semantic_index: &CanvasSemanticIndex<'_>,
    documents: &[CanvasDocumentGraph],
) -> Vec<RenderedComponentInstance> {
    let mut pending = Vec::<(RenderedComponentInstance, Option<String>, usize)>::new();

    for document in documents {
        let nodes_by_render_id = document
            .nodes
            .iter()
            .map(|node| (node.render_instance_id.as_str(), node))
            .collect::<HashMap<_, _>>();

        for node in &document.nodes {
            let parent_node = node
                .parent_render_instance_id
                .as_deref()
                .and_then(|parent_id| nodes_by_render_id.get(parent_id))
                .copied();
            let mut depth = 0usize;

            for invocation_id in &node.component_invocation_ids {
                if parent_node.is_some_and(|parent| {
                    parent
                        .component_invocation_ids
                        .iter()
                        .any(|candidate| candidate == invocation_id)
                }) {
                    continue;
                }
                let definition_id = semantic_index
                    .definition_ids_by_invocation
                    .get(invocation_id)
                    .and_then(|definitions| definitions.first())
                    .cloned();
                pending.push((
                    rendered_component_instance(
                        document,
                        node,
                        definition_id,
                        Some(invocation_id.clone()),
                    ),
                    node.parent_render_instance_id.clone(),
                    depth,
                ));
                depth = depth.saturating_add(1);
            }

            if node.component_invocation_ids.is_empty() {
                for definition_id in &node.component_definition_ids {
                    if parent_node.is_some_and(|parent| {
                        parent
                            .component_definition_ids
                            .iter()
                            .any(|candidate| candidate == definition_id)
                    }) {
                        continue;
                    }
                    pending.push((
                        rendered_component_instance(
                            document,
                            node,
                            Some(definition_id.clone()),
                            None,
                        ),
                        node.parent_render_instance_id.clone(),
                        depth,
                    ));
                    depth = depth.saturating_add(1);
                }
            }
        }
    }

    let mut instance_indices_by_render_id = HashMap::<String, Vec<usize>>::new();
    for (index, (instance, _, _)) in pending.iter().enumerate() {
        instance_indices_by_render_id
            .entry(instance.render_instance_id.clone())
            .or_default()
            .push(index);
    }
    let render_parents = documents
        .iter()
        .flat_map(|document| {
            document.nodes.iter().map(|node| {
                (
                    node.render_instance_id.clone(),
                    node.parent_render_instance_id.clone(),
                )
            })
        })
        .collect::<HashMap<_, _>>();

    for index in 0..pending.len() {
        let (render_instance_id, render_parent_id, depth) = {
            let (instance, parent, depth) = &pending[index];
            (instance.render_instance_id.clone(), parent.clone(), *depth)
        };
        let local_parent = if depth > 0 {
            instance_indices_by_render_id
                .get(&render_instance_id)
                .and_then(|indices| {
                    indices
                        .iter()
                        .copied()
                        .filter(|candidate| *candidate != index && pending[*candidate].2 < depth)
                        .max_by_key(|candidate| pending[*candidate].2)
                })
        } else {
            None
        };
        let ancestor_parent = local_parent.or_else(|| {
            let mut cursor = render_parent_id;
            while let Some(render_id) = cursor {
                if let Some(candidate) = instance_indices_by_render_id
                    .get(&render_id)
                    .and_then(|indices| indices.last())
                    .copied()
                {
                    return Some(candidate);
                }
                cursor = render_parents.get(&render_id).cloned().flatten();
            }
            None
        });
        pending[index].0.parent_instance_id =
            ancestor_parent.map(|parent| pending[parent].0.id.clone());
    }

    let mut instances = pending
        .into_iter()
        .map(|(instance, _, _)| instance)
        .collect::<Vec<_>>();
    instances.sort_by(|left, right| {
        left.route
            .cmp(&right.route)
            .then_with(|| left.render_instance_id.cmp(&right.render_instance_id))
            .then_with(|| left.invocation_id.cmp(&right.invocation_id))
            .then_with(|| left.definition_id.cmp(&right.definition_id))
    });
    instances
}

fn rendered_component_instance(
    document: &CanvasDocumentGraph,
    node: &CanvasRenderNode,
    definition_id: Option<String>,
    invocation_id: Option<String>,
) -> RenderedComponentInstance {
    let mut hasher = Sha256::new();
    hasher.update(b"component-instance");
    hasher.update([0]);
    hasher.update(document.route.as_bytes());
    hasher.update([0]);
    hasher.update(node.render_instance_id.as_bytes());
    hasher.update([0]);
    if let Some(invocation_id) = invocation_id.as_deref() {
        hasher.update(invocation_id.as_bytes());
    }
    hasher.update([0]);
    if let Some(definition_id) = definition_id.as_deref() {
        hasher.update(definition_id.as_bytes());
    }
    RenderedComponentInstance {
        id: format!("component_instance_{}", short_hex(&hasher.finalize())),
        definition_id,
        invocation_id,
        render_instance_id: node.render_instance_id.clone(),
        route: document.route.clone(),
        source_node_id: node
            .source_node_id
            .clone()
            .or_else(|| node.template_source_node_id.clone()),
        parent_instance_id: None,
        template_stack: node.provenance_stack.clone(),
        scope_path: node.component_invocation_ids.clone(),
        binding_key: node.binding_key.clone(),
        binding_path: node.binding_path.clone(),
    }
}

fn derive_runtime_nodes(
    model: &ProjectModel,
    documents: &[CanvasDocumentGraph],
) -> Vec<CanvasRuntimeNode> {
    let mut runtime_nodes = Vec::new();
    for script in &model.source_graph.scripts {
        let Some(file) = model.files.iter().find(|file| {
            normalized_project_path(&file.relative_path) == normalized_project_path(&script.file)
        }) else {
            continue;
        };
        if file.contents.trim().is_empty() {
            continue;
        }
        let related_sources = model
            .source_graph
            .relations
            .iter()
            .filter(|relation| {
                relation.kind == SourceRelationKind::UsesScript && relation.to == script.node_id
            })
            .map(|relation| relation.from.as_str())
            .collect::<HashSet<_>>();
        let mut routes = documents
            .iter()
            .filter(|document| {
                related_sources.is_empty()
                    || document.nodes.iter().any(|node| {
                        node.source_node_id
                            .as_deref()
                            .is_some_and(|id| related_sources.contains(id))
                            || node
                                .template_source_node_id
                                .as_deref()
                                .is_some_and(|id| related_sources.contains(id))
                    })
            })
            .map(|document| document.route.clone())
            .collect::<Vec<_>>();
        routes.sort();
        routes.dedup();

        let config = parse_page_js(&file.contents);
        for block in &config.blocks {
            runtime_nodes.push(canvas_runtime_node(
                script,
                &routes,
                CanvasRuntimeKind::NativeBlock,
                CanvasNodeOrigin::PanaRuntime,
                &block.id,
            ));
        }
        if let Some(items) = config
            .motion
            .as_ref()
            .and_then(|motion| motion.get("items"))
            .and_then(serde_json::Value::as_array)
        {
            for (index, item) in items.iter().enumerate() {
                let key = item
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .filter(|id| !id.trim().is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("motion-{index}"));
                runtime_nodes.push(canvas_runtime_node(
                    script,
                    &routes,
                    CanvasRuntimeKind::Motion,
                    CanvasNodeOrigin::PanaRuntime,
                    &key,
                ));
            }
        }
        if !config.has_page_js() {
            runtime_nodes.push(canvas_runtime_node(
                script,
                &routes,
                CanvasRuntimeKind::ArbitraryScript,
                CanvasNodeOrigin::ArbitraryJsRuntime,
                &script.logical_path,
            ));
        }
    }
    runtime_nodes.sort_by(|left, right| left.runtime_node_id.cmp(&right.runtime_node_id));
    runtime_nodes
}

fn canvas_runtime_node(
    script: &crate::source_graph::model::SourceGraphScript,
    routes: &[String],
    kind: CanvasRuntimeKind,
    origin: CanvasNodeOrigin,
    key: &str,
) -> CanvasRuntimeNode {
    let kind_key = match kind {
        CanvasRuntimeKind::NativeBlock => "native-block",
        CanvasRuntimeKind::Motion => "motion",
        CanvasRuntimeKind::ArbitraryScript => "arbitrary",
    };
    let mut hasher = Sha256::new();
    hasher.update(script.node_id.as_bytes());
    hasher.update([0]);
    hasher.update(kind_key.as_bytes());
    hasher.update([0]);
    hasher.update(key.as_bytes());
    CanvasRuntimeNode {
        runtime_node_id: format!("runtime_{}", short_hex(&hasher.finalize())),
        script_source_node_id: script.node_id.clone(),
        script_file: script.file.clone(),
        routes: routes.to_vec(),
        key: key.to_string(),
        kind,
        origin,
        capabilities: CanvasNodeCapabilities {
            editable: false,
            inspectable: true,
            read_only: true,
        },
    }
}

fn normalized_project_path(path: &str) -> String {
    path.trim().trim_start_matches('/').replace('\\', "/")
}

fn collect_render_nodes(
    node: &NodeRef,
    route: &str,
    parent_render_instance_id: Option<String>,
    semantic_index: &CanvasSemanticIndex<'_>,
    provenance_stack: &mut Vec<String>,
    occurrences: &mut HashMap<(String, String, String), usize>,
    binding_occurrences: &mut HashMap<(String, String, String, String), usize>,
    nodes: &mut Vec<CanvasRenderNode>,
    diagnostics: &mut Vec<CanvasGraphDiagnostic>,
    total_nodes: &mut usize,
) -> Result<(), String> {
    if let Some(comment) = node.as_comment() {
        apply_provenance_marker(
            comment.borrow().as_str(),
            provenance_stack,
            route,
            diagnostics,
        );
        return Ok(());
    }

    let mut descendant_parent = parent_render_instance_id;
    if let Some(element) = node.as_element() {
        let (source_node_id, template_source_node_id, binding_key) = {
            let attributes = element.attributes.borrow();
            let binding_key = [
                "data-pana-key",
                "data-key",
                "data-id",
                "data-pana-instance",
                "id",
                "href",
                "src",
            ]
            .iter()
            .find_map(|name| {
                attributes
                    .get(*name)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|value| format!("{name}:{value}"))
            });
            (
                attributes
                    .get("data-pana-source-id")
                    .map(str::to_string)
                    .filter(|value| !value.trim().is_empty()),
                attributes
                    .get("data-pana-template-source-id")
                    .map(str::to_string)
                    .filter(|value| !value.trim().is_empty()),
                binding_key,
            )
        };
        let mut provenance_stack_snapshot = provenance_stack.clone();
        if let Some(template_source_node_id) = template_source_node_id.as_ref() {
            push_unique(&mut provenance_stack_snapshot, template_source_node_id);
        }
        if let Some(source_node_id) = source_node_id.as_ref() {
            push_unique(&mut provenance_stack_snapshot, source_node_id);
        }

        if !provenance_stack_snapshot.is_empty() {
            *total_nodes = total_nodes.saturating_add(1);
            if *total_nodes > MAX_CANVAS_NODES {
                return Err(format!(
                    "CanvasGraph depășește limita de {MAX_CANVAS_NODES} instanțe randate."
                ));
            }
            let tag = element.name.local.to_string();
            let primary_provenance = source_node_id
                .as_deref()
                .or(template_source_node_id.as_deref())
                .or_else(|| provenance_stack_snapshot.last().map(String::as_str))
                .expect("marked render node has provenance")
                .to_string();
            let key = (route.to_string(), primary_provenance.clone(), tag.clone());
            let occurrence = occurrences.entry(key).or_insert(0);
            let current_occurrence = *occurrence;
            *occurrence = occurrence.saturating_add(1);
            let binding_duplicate = binding_key.as_ref().map(|binding_key| {
                let key = (
                    route.to_string(),
                    primary_provenance.clone(),
                    tag.clone(),
                    binding_key.clone(),
                );
                let duplicate = binding_occurrences.entry(key).or_default();
                let current = *duplicate;
                *duplicate = duplicate.saturating_add(1);
                current
            });
            if binding_duplicate.is_some_and(|duplicate| duplicate > 0) {
                diagnostics.push(CanvasGraphDiagnostic {
                    code: "duplicate_render_binding".to_string(),
                    message: format!(
                        "Cheia randată {:?} apare de mai multe ori pentru aceeași proveniență; identitatea include un discriminator de coliziune.",
                        binding_key.as_deref().unwrap_or_default()
                    ),
                    route: Some(route.to_string()),
                    source_node_id: Some(primary_provenance.clone()),
                });
            }
            let render_instance_id = render_instance_id(
                route,
                &primary_provenance,
                &tag,
                binding_key.as_deref(),
                binding_duplicate.unwrap_or(current_occurrence),
                current_occurrence,
            );
            element
                .attributes
                .borrow_mut()
                .insert("data-pana-render-instance-id", render_instance_id.clone());

            for candidate in &provenance_stack_snapshot {
                if !semantic_index.live_source_ids.contains(candidate.as_str()) {
                    diagnostics.push(CanvasGraphDiagnostic {
                        code: "unknown_source_provenance".to_string(),
                        message: format!(
                            "Documentul randat conține proveniența {candidate}, absentă din ProjectModel."
                        ),
                        route: Some(route.to_string()),
                        source_node_id: Some(candidate.clone()),
                    });
                }
            }

            let source_backed = source_node_id.is_some();
            let (component_definition_ids, component_invocation_ids) =
                semantic_index.component_ids(&provenance_stack_snapshot);
            let (block_definition_ids, block_source_instance_ids) =
                semantic_index.block_ids(&provenance_stack_snapshot);
            let binding_path = semantic_index.repeated_binding_path(&provenance_stack_snapshot);
            if semantic_index.is_repeated(&provenance_stack_snapshot) && binding_key.is_none() {
                diagnostics.push(CanvasGraphDiagnostic {
                    code: "unstable_repeated_render_identity".to_string(),
                    message: "Instanța repetată nu expune data-pana-key, data-key, data-id, id, href sau src; identitatea randată folosește temporar ordinea."
                        .to_string(),
                    route: Some(route.to_string()),
                    source_node_id: Some(primary_provenance.clone()),
                });
            }
            nodes.push(CanvasRenderNode {
                render_instance_id: render_instance_id.clone(),
                source_node_id,
                template_source_node_id,
                parent_render_instance_id: descendant_parent.clone(),
                provenance_stack: provenance_stack_snapshot,
                component_definition_ids,
                component_invocation_ids,
                block_definition_ids,
                block_source_instance_ids,
                binding_key,
                binding_path,
                tag,
                occurrence: current_occurrence,
                origin: if source_backed {
                    CanvasNodeOrigin::Source
                } else {
                    CanvasNodeOrigin::Tera
                },
                capabilities: CanvasNodeCapabilities {
                    editable: source_backed,
                    inspectable: true,
                    read_only: !source_backed,
                },
            });
            descendant_parent = Some(render_instance_id);
        }
    }

    let mut descendant_provenance = provenance_stack.clone();
    for child in node.children() {
        collect_render_nodes(
            &child,
            route,
            descendant_parent.clone(),
            semantic_index,
            &mut descendant_provenance,
            occurrences,
            binding_occurrences,
            nodes,
            diagnostics,
            total_nodes,
        )?;
    }
    Ok(())
}

fn render_instance_id(
    route: &str,
    source_id: &str,
    tag: &str,
    binding_key: Option<&str>,
    binding_occurrence: usize,
    source_occurrence: usize,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(route.as_bytes());
    hasher.update([0]);
    hasher.update(source_id.as_bytes());
    hasher.update([0]);
    hasher.update(tag.as_bytes());
    hasher.update([0]);
    if let Some(binding_key) = binding_key {
        hasher.update(b"binding");
        hasher.update([0]);
        hasher.update(binding_key.as_bytes());
        hasher.update([0]);
        hasher.update(binding_occurrence.to_le_bytes());
    } else {
        hasher.update(b"occurrence");
        hasher.update([0]);
        hasher.update(source_occurrence.to_le_bytes());
    }
    format!("ri_{}", short_hex(&hasher.finalize()))
}

fn apply_provenance_marker(
    comment: &str,
    stack: &mut Vec<String>,
    route: &str,
    diagnostics: &mut Vec<CanvasGraphDiagnostic>,
) {
    let marker = comment.trim();
    let start = marker
        .strip_prefix("pana-template-source-start:")
        .or_else(|| marker.strip_prefix("pana-template-expression-start:"));
    if let Some(source_id) = start.map(str::trim).filter(|value| !value.is_empty()) {
        stack.push(source_id.to_string());
        return;
    }
    let end = marker
        .strip_prefix("pana-template-source-end:")
        .or_else(|| marker.strip_prefix("pana-template-expression-end:"));
    let Some(source_id) = end.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };
    if stack.last().is_some_and(|active| active == source_id) {
        stack.pop();
        return;
    }
    if let Some(position) = stack.iter().rposition(|active| active == source_id) {
        stack.truncate(position);
        diagnostics.push(CanvasGraphDiagnostic {
            code: "recovered_provenance_stack".to_string(),
            message: format!(
                "CanvasGraph a recuperat o închidere de proveniență neordonată pentru {source_id}."
            ),
            route: Some(route.to_string()),
            source_node_id: Some(source_id.to_string()),
        });
    } else {
        diagnostics.push(CanvasGraphDiagnostic {
            code: "unmatched_provenance_end".to_string(),
            message: format!(
                "CanvasGraph a întâlnit un marker final fără început pentru {source_id}."
            ),
            route: Some(route.to_string()),
            source_node_id: Some(source_id.to_string()),
        });
    }
}

#[derive(Clone, Copy, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CanvasResourceKind {
    Stylesheet,
    Script,
    Font,
    Image,
    Media,
    Other,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CanvasResourceEntry {
    pub url: String,
    pub content_hash: String,
    pub size_bytes: u64,
    pub content_type: String,
    pub kind: CanvasResourceKind,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CanvasResourceManifest {
    pub schema_version: u32,
    pub preview_revision: String,
    pub total_bytes: u64,
    pub entries: Vec<CanvasResourceEntry>,
}

impl CanvasResourceManifest {
    pub(crate) fn from_artifact_root(preview_revision: &str, root: &Path) -> Result<Self, String> {
        require_nonempty_preview_revision(preview_revision)?;
        let mut entries = Vec::new();
        let mut total_bytes = 0u64;

        if root.is_dir() {
            for entry in WalkDir::new(root).follow_links(false) {
                let entry = entry.map_err(|error| {
                    format!("Manifestul Canvas nu a putut parcurge resursele: {error}")
                })?;
                if entry.file_type().is_symlink() {
                    return Err(format!(
                        "Manifestul Canvas a refuzat symlink-ul {}.",
                        entry.path().display()
                    ));
                }
                if !entry.file_type().is_file() {
                    continue;
                }
                if entries.len() >= MAX_CANVAS_RESOURCES {
                    return Err(format!(
                        "Manifestul Canvas depășește limita de {MAX_CANVAS_RESOURCES} resurse."
                    ));
                }
                let body = fs::read(entry.path()).map_err(|error| {
                    format!(
                        "Manifestul Canvas nu a putut citi {}: {error}",
                        entry.path().display()
                    )
                })?;
                total_bytes = total_bytes
                    .checked_add(body.len() as u64)
                    .ok_or_else(|| "Manifestul Canvas a depășit contorul de bytes.".to_string())?;
                if total_bytes > MAX_CANVAS_RESOURCE_BYTES {
                    return Err(format!(
                        "Manifestul Canvas depășește limita de {MAX_CANVAS_RESOURCE_BYTES} bytes."
                    ));
                }
                let relative = entry.path().strip_prefix(root).map_err(|_| {
                    "Manifestul Canvas a găsit o resursă în afara artifact root.".to_string()
                })?;
                let url = format!("/{}", relative.to_string_lossy().replace('\\', "/"));
                let content_type = mime_guess::from_path(entry.path())
                    .first_or_octet_stream()
                    .essence_str()
                    .to_string();
                entries.push(CanvasResourceEntry {
                    kind: resource_kind(entry.path()),
                    url,
                    content_hash: format!("sha256-{}", full_hex(&Sha256::digest(&body))),
                    size_bytes: body.len() as u64,
                    content_type,
                });
            }
        }
        entries.sort_by(|left, right| left.url.cmp(&right.url));

        Ok(Self {
            schema_version: CANVAS_PROJECTION_SCHEMA_VERSION,
            preview_revision: preview_revision.to_string(),
            total_bytes,
            entries,
        })
    }
}

fn resource_kind(path: &Path) -> CanvasResourceKind {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("css" | "scss" | "sass") => CanvasResourceKind::Stylesheet,
        Some("js" | "mjs") => CanvasResourceKind::Script,
        Some("woff" | "woff2" | "ttf" | "otf" | "eot") => CanvasResourceKind::Font,
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "avif") => CanvasResourceKind::Image,
        Some("mp3" | "wav" | "ogg" | "mp4" | "webm") => CanvasResourceKind::Media,
        _ => CanvasResourceKind::Other,
    }
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CanvasProjectionTransaction {
    pub schema_version: u32,
    pub identity: CanvasProjectionIdentity,
    pub workspace_transaction_id: Option<String>,
    pub phase: CanvasProjectionPhase,
    pub impact: PreviewImpact,
    pub graph: CanvasGraph,
    pub resources: CanvasResourceManifest,
    pub started_at_ms: u128,
    pub phase_timings_ms: BTreeMap<String, u64>,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CanvasProjectionPlan {
    pub schema_version: u32,
    pub identity: CanvasProjectionIdentity,
    pub workspace_transaction_id: Option<String>,
    pub phase: CanvasProjectionPhase,
    pub impact: PreviewImpact,
    pub resources: CanvasResourceManifest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreviewPhaseReceipt {
    pub schema_version: u32,
    pub identity: CanvasProjectionIdentity,
    pub phase: CanvasProjectionPhase,
    #[serde(default)]
    pub phase_timings_ms: BTreeMap<String, u64>,
    #[serde(default)]
    pub diagnostic: Option<String>,
}

impl CanvasProjectionTransaction {
    pub(crate) fn prepared(
        project_root: &str,
        runtime_session_id: &str,
        workspace_revision: u64,
        preview_revision: &str,
        workspace_transaction_id: Option<String>,
        impact: PreviewImpact,
        graph: CanvasGraph,
        resources: CanvasResourceManifest,
    ) -> Result<Self, String> {
        require_nonempty_preview_revision(preview_revision)?;
        if workspace_transaction_id
            .as_deref()
            .is_some_and(|value| value.trim().is_empty() || value.len() > 256)
        {
            return Err("Canvas Runtime a refuzat workspaceTransactionId invalid.".to_string());
        }
        let transaction_id = canvas_transaction_id(
            project_root,
            runtime_session_id,
            workspace_revision,
            preview_revision,
        );
        Ok(Self {
            schema_version: CANVAS_PROJECTION_SCHEMA_VERSION,
            identity: CanvasProjectionIdentity {
                project_root: project_root.to_string(),
                runtime_session_id: runtime_session_id.to_string(),
                workspace_revision,
                transaction_id,
                preview_revision: preview_revision.to_string(),
            },
            workspace_transaction_id,
            phase: CanvasProjectionPhase::Prepared,
            impact,
            graph,
            resources,
            started_at_ms: now_ms(),
            phase_timings_ms: BTreeMap::new(),
            diagnostics: Vec::new(),
        })
    }

    pub(crate) fn transition_to(
        &mut self,
        next: CanvasProjectionPhase,
        elapsed_ms: u64,
    ) -> Result<(), String> {
        let valid = (next == CanvasProjectionPhase::Failed
            && !matches!(
                self.phase,
                CanvasProjectionPhase::CanonicalVerified | CanvasProjectionPhase::Failed
            ))
            || matches!(
                (self.phase, next),
                (
                    CanvasProjectionPhase::Prepared,
                    CanvasProjectionPhase::ResourcesReady
                ) | (
                    CanvasProjectionPhase::ResourcesReady,
                    CanvasProjectionPhase::Committed
                ) | (
                    CanvasProjectionPhase::Committed,
                    CanvasProjectionPhase::StyledReady
                ) | (
                    CanvasProjectionPhase::StyledReady,
                    CanvasProjectionPhase::CanonicalVerified
                )
            );
        if !valid {
            return Err(format!(
                "Canvas Runtime a refuzat tranziția de fază {:?} -> {:?}.",
                self.phase, next
            ));
        }
        self.phase = next;
        self.phase_timings_ms
            .insert(canvas_phase_key(next).to_string(), elapsed_ms);
        Ok(())
    }

    pub(crate) fn plan(&self) -> CanvasProjectionPlan {
        CanvasProjectionPlan {
            schema_version: self.schema_version,
            identity: self.identity.clone(),
            workspace_transaction_id: self.workspace_transaction_id.clone(),
            phase: self.phase,
            impact: self.impact.clone(),
            resources: self.resources.clone(),
        }
    }

    pub(crate) fn accept_phase_receipt(
        &self,
        receipt: &PreviewPhaseReceipt,
    ) -> Result<Self, String> {
        if receipt.schema_version != CANVAS_PROJECTION_SCHEMA_VERSION
            || receipt.schema_version != self.schema_version
        {
            return Err(
                "Canvas Runtime a refuzat ACK-ul cu altă versiune de protocol.".to_string(),
            );
        }
        if receipt.identity != self.identity {
            return Err(
                "Canvas Runtime a refuzat ACK-ul pentru altă tranzacție sau ProjectSession."
                    .to_string(),
            );
        }
        if receipt.phase_timings_ms.len() > 16
            || receipt
                .phase_timings_ms
                .values()
                .any(|duration| *duration > 600_000)
        {
            return Err("Canvas Runtime a refuzat timpi de fază neîncadrați în buget.".to_string());
        }

        if receipt
            .diagnostic
            .as_deref()
            .is_some_and(|diagnostic| diagnostic.trim().is_empty() || diagnostic.len() > 4_096)
        {
            return Err("Canvas Runtime a refuzat diagnosticul de fază invalid.".to_string());
        }

        let expected = match self.phase {
            CanvasProjectionPhase::Prepared => CanvasProjectionPhase::ResourcesReady,
            CanvasProjectionPhase::ResourcesReady => CanvasProjectionPhase::Committed,
            CanvasProjectionPhase::Committed => CanvasProjectionPhase::StyledReady,
            CanvasProjectionPhase::StyledReady
            | CanvasProjectionPhase::CanonicalVerified
            | CanvasProjectionPhase::Failed => {
                return Err(format!(
                    "Canvas Runtime nu mai acceptă ACK după faza {:?}.",
                    self.phase
                ));
            }
        };
        if receipt.phase != expected && receipt.phase != CanvasProjectionPhase::Failed {
            return Err(format!(
                "Canvas Runtime cere ACK {:?}, primit {:?}.",
                expected, receipt.phase
            ));
        }

        let allowed_keys = ["resourcesReady", "committed", "styledReady", "failed"];
        if receipt
            .phase_timings_ms
            .keys()
            .any(|key| !allowed_keys.contains(&key.as_str()))
        {
            return Err("Canvas Runtime a refuzat o cheie de timing necunoscută.".to_string());
        }

        let phase_key = canvas_phase_key(receipt.phase);
        let elapsed = receipt
            .phase_timings_ms
            .get(phase_key)
            .copied()
            .ok_or_else(|| format!("ACK-ul {:?} nu conține timing-ul propriu.", receipt.phase))?;
        for (key, previous) in &self.phase_timings_ms {
            if key == "canonicalVerified" {
                continue;
            }
            if receipt.phase != CanvasProjectionPhase::Failed
                && receipt.phase_timings_ms.get(key) != Some(previous)
            {
                return Err(format!(
                    "ACK-ul {:?} nu păstrează timing-ul confirmat pentru {key}.",
                    receipt.phase
                ));
            }
        }
        if receipt.phase != CanvasProjectionPhase::Failed {
            let mut previous_elapsed = 0;
            for phase in [
                CanvasProjectionPhase::ResourcesReady,
                CanvasProjectionPhase::Committed,
                CanvasProjectionPhase::StyledReady,
            ] {
                if phase > receipt.phase {
                    break;
                }
                let value = receipt
                    .phase_timings_ms
                    .get(canvas_phase_key(phase))
                    .copied()
                    .ok_or_else(|| {
                        format!(
                            "ACK-ul {:?} nu conține toate fazele anterioare.",
                            receipt.phase
                        )
                    })?;
                if value < previous_elapsed {
                    return Err("Timpii ACK Canvas nu sunt monotoni.".to_string());
                }
                previous_elapsed = value;
            }
        }

        let mut confirmed = self.clone();
        confirmed.transition_to(receipt.phase, elapsed)?;
        if let Some(diagnostic) = receipt.diagnostic.as_deref() {
            confirmed.diagnostics.push(diagnostic.to_string());
        }
        Ok(confirmed)
    }

    #[cfg(test)]
    pub(crate) fn test_fixture(workspace_revision: u64, preview_revision: &str) -> Self {
        let project_root = "/project";
        let runtime_session_id = "runtime";
        let graph = CanvasGraph {
            schema_version: CANVAS_PROJECTION_SCHEMA_VERSION,
            workspace_revision,
            preview_revision: preview_revision.to_string(),
            model_revision: "test-model".to_string(),
            documents: Vec::new(),
            component_instances: Vec::new(),
            block_instances: Vec::new(),
            runtime_nodes: Vec::new(),
            diagnostics: Vec::new(),
        };
        let resources = CanvasResourceManifest {
            schema_version: CANVAS_PROJECTION_SCHEMA_VERSION,
            preview_revision: preview_revision.to_string(),
            total_bytes: 0,
            entries: Vec::new(),
        };
        Self::prepared(
            project_root,
            runtime_session_id,
            workspace_revision,
            preview_revision,
            None,
            PreviewImpact::from_projected_paths(&[], true),
            graph,
            resources,
        )
        .expect("test Canvas transaction")
    }
}

fn canvas_phase_key(phase: CanvasProjectionPhase) -> &'static str {
    match phase {
        CanvasProjectionPhase::Prepared => "prepared",
        CanvasProjectionPhase::ResourcesReady => "resourcesReady",
        CanvasProjectionPhase::Committed => "committed",
        CanvasProjectionPhase::StyledReady => "styledReady",
        CanvasProjectionPhase::CanonicalVerified => "canonicalVerified",
        CanvasProjectionPhase::Failed => "failed",
    }
}

fn canvas_transaction_id(
    project_root: &str,
    runtime_session_id: &str,
    workspace_revision: u64,
    preview_revision: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(project_root.as_bytes());
    hasher.update([0]);
    hasher.update(runtime_session_id.as_bytes());
    hasher.update([0]);
    hasher.update(workspace_revision.to_le_bytes());
    hasher.update([0]);
    hasher.update(preview_revision.as_bytes());
    format!("canvas_{}", short_hex(&hasher.finalize()))
}

fn require_nonempty_preview_revision(preview_revision: &str) -> Result<(), String> {
    if preview_revision.trim().is_empty() {
        Err("Canvas Runtime cere previewRevision nenul.".to_string())
    } else {
        Ok(())
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn short_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take(12)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn full_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        kernel::project_workspace::WorkspaceProjectionLease,
        project::{AcceptedProjectDiskManifest, ProjectDiskManifest},
        project_model::build_project_model_from_workspace_projection,
        source_graph::model::SourceNodeKind,
    };
    use std::{
        collections::{HashMap, HashSet},
        fs,
        path::PathBuf,
    };

    #[test]
    fn impact_classifies_html_css_js_and_full_routes() {
        let impact = PreviewImpact::from_projected_paths(
            &[
                "templates/index.html".to_string(),
                "sass/pagini/index.scss".to_string(),
                "static/js/index.js".to_string(),
                "content/_index.md".to_string(),
            ],
            false,
        );
        assert!(impact.kinds.contains(&PreviewImpactKind::HtmlStructure));
        assert!(impact.kinds.contains(&PreviewImpactKind::Styles));
        assert!(impact.kinds.contains(&PreviewImpactKind::Scripts));
        assert!(impact.kinds.contains(&PreviewImpactKind::Route));
        assert!(impact.requires_full_document);
    }

    #[test]
    fn canvas_graph_distinguishes_repeated_render_instances() {
        let root = test_project_root("render-instances");
        let canonical = root.canonicalize().unwrap().to_string_lossy().to_string();
        let session = "canvas-test".to_string();
        let template = "<main><article>Card</article></main>";
        let lease = WorkspaceProjectionLease {
            project_root: canonical.clone(),
            runtime_session_id: session.clone(),
            revision: 3,
            workspace_transaction_id: Some("canvas-test-3".to_string()),
            source_texts: HashMap::from([
                ("zola.toml".to_string(), "base_url = '/'\n".to_string()),
                ("templates/index.html".to_string(), template.to_string()),
            ]),
            resource_bytes: HashMap::new(),
            deleted_sources: HashSet::new(),
            changed_paths: HashSet::from(["templates/index.html".to_string()]),
            accepted_disk: AcceptedProjectDiskManifest::new(
                session,
                canonical.clone(),
                ProjectDiskManifest {
                    root: canonical,
                    files: Vec::new(),
                    truncated: false,
                    max_files: 100,
                },
            )
            .unwrap(),
        };
        let model = build_project_model_from_workspace_projection(&root, &lease).unwrap();
        let source_id = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label.contains("article"))
            .map(|node| node.id.clone())
            .unwrap();
        let html = format!(
            "<html><body><article data-pana-source-id=\"{0}\">A</article><article data-pana-source-id=\"{0}\">B</article></body></html>",
            source_id,
        );
        let annotated =
            CanvasGraph::annotate_rendered_document(&model, "index.html", &html).unwrap();
        let graph = CanvasGraph::from_rendered_documents(
            &model,
            3,
            "preview-3",
            [("index.html", annotated.as_str())],
        )
        .unwrap();
        assert_eq!(graph.documents[0].nodes.len(), 2);
        assert_ne!(
            graph.documents[0].nodes[0].render_instance_id,
            graph.documents[0].nodes[1].render_instance_id
        );
        assert_eq!(graph.documents[0].nodes[0].occurrence, 0);
        assert_eq!(graph.documents[0].nodes[1].occurrence, 1);
        for node in &graph.documents[0].nodes {
            assert!(annotated.contains(&format!(
                "data-pana-render-instance-id=\"{}\"",
                node.render_instance_id
            )));
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn canvas_graph_keeps_keyed_render_identity_stable_across_reordering() {
        let root = test_project_root("stable-keyed-render-identity");
        let model = test_project_model(
            &root,
            31,
            HashMap::from([
                ("zola.toml".to_string(), "base_url = '/'\n".to_string()),
                (
                    "templates/index.html".to_string(),
                    "<main><article>Card</article></main>".to_string(),
                ),
            ]),
        );
        let source_id = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::Html && node.label.contains("article"))
            .map(|node| node.id.clone())
            .unwrap();
        let first = format!(
            "<article data-pana-source-id=\"{source_id}\" data-key=\"alpha\">A</article>\
             <article data-pana-source-id=\"{source_id}\" data-key=\"beta\">B</article>"
        );
        let reordered = format!(
            "<article data-pana-source-id=\"{source_id}\" data-key=\"beta\">B</article>\
             <article data-pana-source-id=\"{source_id}\" data-key=\"alpha\">A</article>"
        );
        let first_graph = CanvasGraph::from_rendered_documents(
            &model,
            31,
            "preview-31-a",
            [("/", first.as_str())],
        )
        .unwrap();
        let reordered_graph = CanvasGraph::from_rendered_documents(
            &model,
            31,
            "preview-31-b",
            [("/", reordered.as_str())],
        )
        .unwrap();
        let identities = |graph: &CanvasGraph| {
            graph.documents[0]
                .nodes
                .iter()
                .map(|node| {
                    (
                        node.binding_key.clone().unwrap(),
                        node.render_instance_id.clone(),
                    )
                })
                .collect::<BTreeMap<_, _>>()
        };
        assert_eq!(identities(&first_graph), identities(&reordered_graph));
        assert!(first_graph.diagnostics.is_empty());
        assert!(reordered_graph.diagnostics.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn canvas_graph_projects_include_provenance_to_component_instances() {
        let root = test_project_root("component-instance-provenance");
        let model = test_project_model(
            &root,
            32,
            HashMap::from([
                ("zola.toml".to_string(), "base_url = '/'\n".to_string()),
                (
                    "templates/index.html".to_string(),
                    "{% include \"partials/card.html\" %}".to_string(),
                ),
                (
                    "templates/partials/card.html".to_string(),
                    "<article class=\"card\">Card</article>".to_string(),
                ),
            ]),
        );
        let invocation = model
            .source_graph
            .component_graph
            .invocations
            .iter()
            .find(|invocation| invocation.kind == ComponentInvocationKind::Include)
            .unwrap();
        let invocation_source_id = invocation.source_node_id.clone().unwrap();
        let resolved_definition_id = invocation.resolved_definition_ids[0].clone();
        let article_source_id = model
            .source_graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == SourceNodeKind::Html
                    && node.file == "templates/partials/card.html"
                    && node.label.contains("article")
            })
            .map(|node| node.id.clone())
            .unwrap();
        let rendered = format!(
            "<html><body>\
             <!-- pana-template-source-start:{invocation_source_id} -->\
             <article data-pana-source-id=\"{article_source_id}\">Card</article>\
             <!-- pana-template-source-end:{invocation_source_id} -->\
             </body></html>"
        );
        let graph = CanvasGraph::from_rendered_documents(
            &model,
            32,
            "preview-32",
            [("/", rendered.as_str())],
        )
        .unwrap();

        assert_eq!(graph.documents[0].nodes.len(), 1);
        assert_eq!(graph.component_instances.len(), 1);
        let instance = &graph.component_instances[0];
        assert_eq!(
            instance.invocation_id.as_deref(),
            Some(invocation.id.as_str())
        );
        assert_eq!(
            instance.definition_id.as_deref(),
            Some(resolved_definition_id.as_str())
        );
        assert_eq!(
            instance.render_instance_id,
            graph.documents[0].nodes[0].render_instance_id
        );
        assert!(instance
            .template_stack
            .iter()
            .any(|source_id| source_id == &invocation_source_id));
        assert!(graph.diagnostics.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn canvas_graph_projects_ten_thousand_render_instances_with_bounded_identity() {
        let root = test_project_root("large-render-graph");
        let canonical = root.canonicalize().unwrap().to_string_lossy().to_string();
        let session = "canvas-large-test".to_string();
        let template = "<main><article>Card</article></main>";
        let lease = WorkspaceProjectionLease {
            project_root: canonical.clone(),
            runtime_session_id: session.clone(),
            revision: 30,
            workspace_transaction_id: Some("canvas-large-30".to_string()),
            source_texts: HashMap::from([
                ("zola.toml".to_string(), "base_url = '/'\n".to_string()),
                ("templates/index.html".to_string(), template.to_string()),
            ]),
            resource_bytes: HashMap::new(),
            deleted_sources: HashSet::new(),
            changed_paths: HashSet::from(["templates/index.html".to_string()]),
            accepted_disk: AcceptedProjectDiskManifest::new(
                session,
                canonical.clone(),
                ProjectDiskManifest {
                    root: canonical,
                    files: Vec::new(),
                    truncated: false,
                    max_files: 100,
                },
            )
            .unwrap(),
        };
        let model = build_project_model_from_workspace_projection(&root, &lease).unwrap();
        let source_id = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label.contains("article"))
            .map(|node| node.id.clone())
            .unwrap();
        let mut html = String::from("<html><body><main>");
        for index in 0..10_000 {
            html.push_str(&format!(
                "<article data-pana-source-id=\"{source_id}\">{index}</article>"
            ));
        }
        html.push_str("</main></body></html>");

        let graph = CanvasGraph::from_rendered_documents(
            &model,
            30,
            "preview-30",
            [("index.html", html.as_str())],
        )
        .unwrap();
        let nodes = &graph.documents[0].nodes;
        assert_eq!(nodes.len(), 10_000);
        assert_eq!(nodes[0].occurrence, 0);
        assert_eq!(nodes[9_999].occurrence, 9_999);
        assert_ne!(nodes[0].render_instance_id, nodes[9_999].render_instance_id);
        assert!(graph.diagnostics.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn canvas_graph_derives_pana_motion_and_arbitrary_js_runtime_nodes() {
        let root = test_project_root("runtime-nodes");
        let canonical = root.canonicalize().unwrap().to_string_lossy().to_string();
        let session = "canvas-runtime-test".to_string();
        let lease = WorkspaceProjectionLease {
            project_root: canonical.clone(),
            runtime_session_id: session.clone(),
            revision: 4,
            workspace_transaction_id: Some("canvas-runtime-4".to_string()),
            source_texts: HashMap::from([
                (
                    "zola.toml".to_string(),
                    "base_url = '/'\n".to_string(),
                ),
                (
                    "templates/index.html".to_string(),
                    "<main>Home</main><script src='/js/index.js'></script><script src='/js/custom.js'></script>".to_string(),
                ),
                (
                    "static/js/index.js".to_string(),
                    "// @pana-motion {\"version\":1,\"motion\":{\"items\":[{\"id\":\"hero-motion\",\"type\":\"animation\"}]}}\n// @pana-component id=accordion\n"
                        .to_string(),
                ),
                (
                    "static/js/custom.js".to_string(),
                    "window.customRuntime = true;".to_string(),
                ),
            ]),
            resource_bytes: HashMap::new(),
            deleted_sources: HashSet::new(),
            changed_paths: HashSet::from([
                "templates/index.html".to_string(),
                "static/js/index.js".to_string(),
                "static/js/custom.js".to_string(),
            ]),
            accepted_disk: AcceptedProjectDiskManifest::new(
                session,
                canonical.clone(),
                ProjectDiskManifest {
                    root: canonical,
                    files: Vec::new(),
                    truncated: false,
                    max_files: 100,
                },
            )
            .unwrap(),
        };
        let model = build_project_model_from_workspace_projection(&root, &lease).unwrap();
        let graph = CanvasGraph::from_rendered_documents(
            &model,
            4,
            "preview-4",
            [("index.html", "<html><body><main>Home</main></body></html>")],
        )
        .unwrap();

        assert!(graph.runtime_nodes.iter().any(|node| {
            node.kind == CanvasRuntimeKind::NativeBlock
                && node.origin == CanvasNodeOrigin::PanaRuntime
                && node.key == "accordion"
        }));
        assert!(graph.runtime_nodes.iter().any(|node| {
            node.kind == CanvasRuntimeKind::Motion
                && node.origin == CanvasNodeOrigin::PanaRuntime
                && node.key == "hero-motion"
        }));
        assert!(graph.runtime_nodes.iter().any(|node| {
            node.kind == CanvasRuntimeKind::ArbitraryScript
                && node.origin == CanvasNodeOrigin::ArbitraryJsRuntime
        }));
        assert!(graph
            .runtime_nodes
            .iter()
            .all(|node| node.capabilities.read_only && node.capabilities.inspectable));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resource_manifest_is_sorted_and_content_addressed() {
        let root = test_project_root("resources");
        fs::create_dir_all(root.join("css")).unwrap();
        fs::write(root.join("css/site.css"), "body{color:red}").unwrap();
        fs::write(root.join("app.js"), "console.log('ok')").unwrap();
        let manifest = CanvasResourceManifest::from_artifact_root("preview-a", &root).unwrap();
        assert_eq!(manifest.entries.len(), 2);
        assert_eq!(manifest.entries[0].url, "/app.js");
        assert!(manifest
            .entries
            .iter()
            .all(|entry| entry.content_hash.starts_with("sha256-")));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn transaction_phase_machine_is_monotonic_and_fail_closed() {
        let mut transaction = CanvasProjectionTransaction::test_fixture(4, "preview-4");
        assert!(transaction
            .transition_to(CanvasProjectionPhase::Committed, 1)
            .is_err());
        transaction
            .transition_to(CanvasProjectionPhase::ResourcesReady, 2)
            .unwrap();
        transaction
            .transition_to(CanvasProjectionPhase::Committed, 3)
            .unwrap();
        transaction
            .transition_to(CanvasProjectionPhase::StyledReady, 4)
            .unwrap();
        transaction
            .transition_to(CanvasProjectionPhase::CanonicalVerified, 5)
            .unwrap();
        assert_eq!(transaction.phase, CanvasProjectionPhase::CanonicalVerified);
        assert_eq!(transaction.phase_timings_ms["styledReady"], 4);
        assert!(transaction
            .transition_to(CanvasProjectionPhase::Committed, 6)
            .is_err());
    }

    #[test]
    fn projection_plan_retains_the_originating_workspace_transaction() {
        let graph = CanvasGraph {
            schema_version: CANVAS_PROJECTION_SCHEMA_VERSION,
            workspace_revision: 9,
            preview_revision: "preview-9".to_string(),
            model_revision: "model-9".to_string(),
            documents: Vec::new(),
            component_instances: Vec::new(),
            block_instances: Vec::new(),
            runtime_nodes: Vec::new(),
            diagnostics: Vec::new(),
        };
        let resources = CanvasResourceManifest {
            schema_version: CANVAS_PROJECTION_SCHEMA_VERSION,
            preview_revision: "preview-9".to_string(),
            total_bytes: 0,
            entries: Vec::new(),
        };
        let transaction = CanvasProjectionTransaction::prepared(
            "/project",
            "runtime",
            9,
            "preview-9",
            Some("workspace-edit-9".to_string()),
            PreviewImpact::from_projected_paths(&["templates/index.html".to_string()], false),
            graph,
            resources,
        )
        .unwrap();

        assert_eq!(
            transaction.plan().workspace_transaction_id.as_deref(),
            Some("workspace-edit-9")
        );
    }

    #[test]
    fn phased_receipts_require_exact_identity_and_monotonic_order() {
        let transaction = CanvasProjectionTransaction::test_fixture(7, "preview-7");
        let mut wrong_identity = transaction.identity.clone();
        wrong_identity.preview_revision = "preview-8".to_string();
        let wrong = PreviewPhaseReceipt {
            schema_version: CANVAS_PROJECTION_SCHEMA_VERSION,
            identity: wrong_identity,
            phase: CanvasProjectionPhase::ResourcesReady,
            phase_timings_ms: BTreeMap::from([("resourcesReady".to_string(), 2)]),
            diagnostic: None,
        };
        assert!(transaction.accept_phase_receipt(&wrong).is_err());

        let resources = PreviewPhaseReceipt {
            schema_version: CANVAS_PROJECTION_SCHEMA_VERSION,
            identity: transaction.identity.clone(),
            phase: CanvasProjectionPhase::ResourcesReady,
            phase_timings_ms: BTreeMap::from([("resourcesReady".to_string(), 2)]),
            diagnostic: None,
        };
        let resources_ready = transaction.accept_phase_receipt(&resources).unwrap();
        let committed = resources_ready
            .accept_phase_receipt(&PreviewPhaseReceipt {
                schema_version: CANVAS_PROJECTION_SCHEMA_VERSION,
                identity: transaction.identity.clone(),
                phase: CanvasProjectionPhase::Committed,
                phase_timings_ms: BTreeMap::from([
                    ("resourcesReady".to_string(), 2),
                    ("committed".to_string(), 3),
                ]),
                diagnostic: None,
            })
            .unwrap();
        let styled = committed
            .accept_phase_receipt(&PreviewPhaseReceipt {
                schema_version: CANVAS_PROJECTION_SCHEMA_VERSION,
                identity: transaction.identity.clone(),
                phase: CanvasProjectionPhase::StyledReady,
                phase_timings_ms: BTreeMap::from([
                    ("resourcesReady".to_string(), 2),
                    ("committed".to_string(), 3),
                    ("styledReady".to_string(), 5),
                ]),
                diagnostic: None,
            })
            .unwrap();
        assert_eq!(styled.phase, CanvasProjectionPhase::StyledReady);
    }

    #[test]
    fn phase_receipt_identity_field_property_is_fail_closed() {
        let transaction = CanvasProjectionTransaction::test_fixture(17, "preview-17");
        let base = transaction.identity.clone();
        let mutations = [
            CanvasProjectionIdentity {
                project_root: "/foreign".to_string(),
                ..base.clone()
            },
            CanvasProjectionIdentity {
                runtime_session_id: "foreign-runtime".to_string(),
                ..base.clone()
            },
            CanvasProjectionIdentity {
                workspace_revision: base.workspace_revision + 1,
                ..base.clone()
            },
            CanvasProjectionIdentity {
                transaction_id: "canvas_foreign".to_string(),
                ..base.clone()
            },
            CanvasProjectionIdentity {
                preview_revision: "preview-foreign".to_string(),
                ..base
            },
        ];

        for identity in mutations {
            let receipt = PreviewPhaseReceipt {
                schema_version: CANVAS_PROJECTION_SCHEMA_VERSION,
                identity,
                phase: CanvasProjectionPhase::ResourcesReady,
                phase_timings_ms: BTreeMap::from([("resourcesReady".to_string(), 1)]),
                diagnostic: None,
            };
            assert!(transaction.accept_phase_receipt(&receipt).is_err());
        }
    }

    #[test]
    fn phase_timing_monotonicity_property_holds_for_small_bounded_domain() {
        for resources_ms in 0..=4 {
            for committed_ms in 0..=4 {
                let transaction = CanvasProjectionTransaction::test_fixture(21, "preview-21");
                let resources = transaction
                    .accept_phase_receipt(&PreviewPhaseReceipt {
                        schema_version: CANVAS_PROJECTION_SCHEMA_VERSION,
                        identity: transaction.identity.clone(),
                        phase: CanvasProjectionPhase::ResourcesReady,
                        phase_timings_ms: BTreeMap::from([(
                            "resourcesReady".to_string(),
                            resources_ms,
                        )]),
                        diagnostic: None,
                    })
                    .unwrap();
                let committed = resources.accept_phase_receipt(&PreviewPhaseReceipt {
                    schema_version: CANVAS_PROJECTION_SCHEMA_VERSION,
                    identity: transaction.identity.clone(),
                    phase: CanvasProjectionPhase::Committed,
                    phase_timings_ms: BTreeMap::from([
                        ("resourcesReady".to_string(), resources_ms),
                        ("committed".to_string(), committed_ms),
                    ]),
                    diagnostic: None,
                });
                assert_eq!(committed.is_ok(), committed_ms >= resources_ms);

                let Ok(committed) = committed else {
                    continue;
                };
                for styled_ms in 0..=4 {
                    let styled = committed.accept_phase_receipt(&PreviewPhaseReceipt {
                        schema_version: CANVAS_PROJECTION_SCHEMA_VERSION,
                        identity: transaction.identity.clone(),
                        phase: CanvasProjectionPhase::StyledReady,
                        phase_timings_ms: BTreeMap::from([
                            ("resourcesReady".to_string(), resources_ms),
                            ("committed".to_string(), committed_ms),
                            ("styledReady".to_string(), styled_ms),
                        ]),
                        diagnostic: None,
                    });
                    assert_eq!(styled.is_ok(), styled_ms >= committed_ms);
                }
            }
        }
    }

    fn test_project_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "pana-canvas-{label}-{}-{}",
            std::process::id(),
            now_ms(),
        ));
        fs::create_dir_all(root.join("templates")).unwrap();
        root
    }

    fn test_project_model(
        root: &Path,
        revision: u64,
        source_texts: HashMap<String, String>,
    ) -> ProjectModel {
        let canonical = root.canonicalize().unwrap().to_string_lossy().to_string();
        let session = format!("canvas-test-{revision}");
        let changed_paths = source_texts.keys().cloned().collect::<HashSet<_>>();
        let lease = WorkspaceProjectionLease {
            project_root: canonical.clone(),
            runtime_session_id: session.clone(),
            revision,
            workspace_transaction_id: Some(format!("canvas-test-{revision}")),
            source_texts,
            resource_bytes: HashMap::new(),
            deleted_sources: HashSet::new(),
            changed_paths,
            accepted_disk: AcceptedProjectDiskManifest::new(
                session,
                canonical.clone(),
                ProjectDiskManifest {
                    root: canonical,
                    files: Vec::new(),
                    truncated: false,
                    max_files: 100,
                },
            )
            .unwrap(),
        };
        build_project_model_from_workspace_projection(root, &lease).unwrap()
    }
}
