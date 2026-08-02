use std::{
    collections::{BTreeSet, HashMap, HashSet, VecDeque},
    path::Path,
    time::Instant,
};

use crate::source_graph::{
    model::{SourceGraph, SourceGraphTemplate, SourceOrigin, SourceRelation, SourceRelationKind},
    scan::{builder::SourceGraphBuilder, graph_template_from_summary, template::scan_template},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SourceGraphIncrementalFallback {
    RootChanged,
    MissingTemplate,
    AmbiguousTemplate,
    NonLocalTemplate,
    ExistingDiagnostics,
    NewDiagnostics,
    DynamicDependency,
    DependencyContractChanged,
    IdentityCollision,
    NonContiguousNodes,
}

impl SourceGraphIncrementalFallback {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::RootChanged => "source_graph_root_changed",
            Self::MissingTemplate => "source_graph_template_missing",
            Self::AmbiguousTemplate => "source_graph_template_ambiguous",
            Self::NonLocalTemplate => "source_graph_template_non_local",
            Self::ExistingDiagnostics => "source_graph_existing_diagnostics",
            Self::NewDiagnostics => "source_graph_new_diagnostics",
            Self::DynamicDependency => "source_graph_dynamic_dependency",
            Self::DependencyContractChanged => "source_graph_dependency_contract_changed",
            Self::IdentityCollision => "source_graph_identity_collision",
            Self::NonContiguousNodes => "source_graph_nodes_non_contiguous",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceGraphIncrementalTemplateReport {
    pub(crate) invalidated_template_files: Vec<String>,
    pub(crate) invalidated_page_files: Vec<String>,
    pub(crate) replaced_nodes: usize,
    pub(crate) reused_nodes: usize,
    pub(crate) reused_relations: usize,
    pub(crate) template_parse_ms: u64,
    pub(crate) component_graph_ms: u64,
    pub(crate) block_graph_ms: u64,
}

pub(crate) fn rebuild_local_template_graph(
    mut graph: SourceGraph,
    project_root: &Path,
    zola_root: &Path,
    relative_path: &str,
    projected_sources: &HashMap<String, String>,
) -> Result<(SourceGraph, SourceGraphIncrementalTemplateReport), SourceGraphIncrementalFallback> {
    if graph.project_root != project_root.to_string_lossy()
        || graph.zola_root != zola_root.to_string_lossy()
    {
        return Err(SourceGraphIncrementalFallback::RootChanged);
    }
    let matching_templates = graph
        .templates
        .iter()
        .enumerate()
        .filter_map(|(index, template)| (template.file == relative_path).then_some(index))
        .collect::<Vec<_>>();
    let [template_index] = matching_templates.as_slice() else {
        return Err(if matching_templates.is_empty() {
            SourceGraphIncrementalFallback::MissingTemplate
        } else {
            SourceGraphIncrementalFallback::AmbiguousTemplate
        });
    };
    let template_index = *template_index;
    let previous_template = graph.templates[template_index].clone();
    if previous_template.origin != SourceOrigin::Local {
        return Err(SourceGraphIncrementalFallback::NonLocalTemplate);
    }
    if graph
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.file.as_deref() == Some(relative_path))
    {
        return Err(SourceGraphIncrementalFallback::ExistingDiagnostics);
    }

    let (invalidated_template_files, invalidated_page_files) =
        reverse_template_consumers(&graph, &previous_template);
    let template_parse_started = Instant::now();
    let mut fragment_builder =
        SourceGraphBuilder::new(project_root, zola_root, graph.active_theme.clone());
    let summary = scan_template(
        project_root,
        zola_root,
        &project_root.join(relative_path),
        SourceOrigin::Local,
        None,
        projected_sources,
        &mut fragment_builder,
    );
    let next_template = graph_template_from_summary(summary);
    let fragment = fragment_builder.finish(
        Vec::new(),
        vec![next_template.clone()],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let template_parse_ms = elapsed_ms(template_parse_started);
    if fragment
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.diagnostic.code == "source-graph-dynamic-load-data")
    {
        return Err(SourceGraphIncrementalFallback::DynamicDependency);
    }
    if !fragment.diagnostics.is_empty() {
        return Err(SourceGraphIncrementalFallback::NewDiagnostics);
    }
    if !same_dependency_contract(&previous_template, &next_template) {
        return Err(SourceGraphIncrementalFallback::DependencyContractChanged);
    }

    let old_node_indexes = graph
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| (node.file == relative_path).then_some(index))
        .collect::<Vec<_>>();
    let Some(first_node_index) = old_node_indexes.first().copied() else {
        return Err(SourceGraphIncrementalFallback::MissingTemplate);
    };
    let last_node_index = *old_node_indexes.last().expect("first node exists");
    if last_node_index - first_node_index + 1 != old_node_indexes.len() {
        return Err(SourceGraphIncrementalFallback::NonContiguousNodes);
    }
    let old_node_ids = graph.nodes[first_node_index..=last_node_index]
        .iter()
        .map(|node| node.id.as_str())
        .collect::<HashSet<_>>();
    let new_node_ids = fragment
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<HashSet<_>>();
    if graph
        .nodes
        .iter()
        .any(|node| node.file != relative_path && new_node_ids.contains(node.id.as_str()))
    {
        return Err(SourceGraphIncrementalFallback::IdentityCollision);
    }
    if graph.relations.iter().any(|relation| {
        (old_node_ids.contains(relation.from.as_str())
            && !new_node_ids.contains(relation.from.as_str()))
            || (old_node_ids.contains(relation.to.as_str())
                && !new_node_ids.contains(relation.to.as_str()))
    }) {
        return Err(SourceGraphIncrementalFallback::DependencyContractChanged);
    }
    if !same_internal_relations(&graph.relations, &fragment.relations, &old_node_ids) {
        return Err(SourceGraphIncrementalFallback::DependencyContractChanged);
    }

    let replaced_nodes = old_node_indexes.len();
    let reused_nodes = graph.nodes.len().saturating_sub(replaced_nodes);
    let reused_relations = graph.relations.len();
    graph
        .nodes
        .splice(first_node_index..=last_node_index, fragment.nodes);
    graph.templates[template_index] = next_template;
    let component_graph_started = Instant::now();
    graph.component_graph = crate::source_graph::component_graph::build_component_graph(&graph);
    let component_graph_ms = elapsed_ms(component_graph_started);
    let block_graph_started = Instant::now();
    graph.block_graph = crate::blocks::graph::build_block_graph(&graph);
    let block_graph_ms = elapsed_ms(block_graph_started);
    graph.markdown_projections = crate::source_graph::markdown::build_markdown_projections(&graph);

    Ok((
        graph,
        SourceGraphIncrementalTemplateReport {
            invalidated_template_files,
            invalidated_page_files,
            replaced_nodes,
            reused_nodes,
            reused_relations,
            template_parse_ms,
            component_graph_ms,
            block_graph_ms,
        },
    ))
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u64::MAX as u128) as u64
}

fn same_dependency_contract(previous: &SourceGraphTemplate, next: &SourceGraphTemplate) -> bool {
    previous.id == next.id
        && previous.file == next.file
        && previous.name == next.name
        && previous.origin == next.origin
        && previous.theme_name == next.theme_name
        && previous.is_partial == next.is_partial
        && previous.node_id == next.node_id
        && previous.extends == next.extends
        && previous.includes == next.includes
        && previous.include_groups == next.include_groups
        && previous.imports == next.imports
        && previous.get_pages == next.get_pages
        && previous.get_sections == next.get_sections
        && previous.internal_links == next.internal_links
        && previous.asset_urls == next.asset_urls
        && previous.asset_hashes == next.asset_hashes
        && previous.data_loads == next.data_loads
        && previous.image_metadata == next.image_metadata
        && previous.image_resizes == next.image_resizes
        && previous.blocks == next.blocks
        && previous.macros == next.macros
}

fn same_internal_relations(
    existing: &[SourceRelation],
    replacement: &[SourceRelation],
    old_node_ids: &HashSet<&str>,
) -> bool {
    let current = existing
        .iter()
        .filter(|relation| {
            old_node_ids.contains(relation.from.as_str())
                && old_node_ids.contains(relation.to.as_str())
        })
        .map(relation_signature)
        .collect::<BTreeSet<_>>();
    let next = replacement
        .iter()
        .map(relation_signature)
        .collect::<BTreeSet<_>>();
    current == next
}

fn relation_signature(relation: &SourceRelation) -> (String, String, &'static str, String) {
    (
        relation.from.clone(),
        relation.to.clone(),
        relation_kind_key(&relation.kind),
        relation.label.clone(),
    )
}

fn relation_kind_key(kind: &SourceRelationKind) -> &'static str {
    match kind {
        SourceRelationKind::PageTemplate => "page_template",
        SourceRelationKind::SectionPageTemplate => "section_page_template",
        SourceRelationKind::GetsPage => "gets_page",
        SourceRelationKind::GetsSection => "gets_section",
        SourceRelationKind::InternalContentLink => "internal_content_link",
        SourceRelationKind::AssetUrl => "asset_url",
        SourceRelationKind::AssetHash => "asset_hash",
        SourceRelationKind::DataLoad => "data_load",
        SourceRelationKind::DataFileLoad => "data_file_load",
        SourceRelationKind::ContentDataLoad => "content_data_load",
        SourceRelationKind::ImageMetadata => "image_metadata",
        SourceRelationKind::ImageResize => "image_resize",
        SourceRelationKind::Extends => "extends",
        SourceRelationKind::Includes => "includes",
        SourceRelationKind::Imports => "imports",
        SourceRelationKind::DefinesBlock => "defines_block",
        SourceRelationKind::OverridesBlock => "overrides_block",
        SourceRelationKind::UsesStyle => "uses_style",
        SourceRelationKind::UsesScript => "uses_script",
    }
}

fn reverse_template_consumers(
    graph: &SourceGraph,
    changed: &SourceGraphTemplate,
) -> (Vec<String>, Vec<String>) {
    let templates_by_node = graph
        .templates
        .iter()
        .map(|template| (template.node_id.as_str(), template.file.as_str()))
        .collect::<HashMap<_, _>>();
    let mut queue = VecDeque::from([changed.node_id.clone()]);
    let mut visited = BTreeSet::new();
    let mut files = BTreeSet::new();
    while let Some(node_id) = queue.pop_front() {
        if !visited.insert(node_id.clone()) {
            continue;
        }
        if let Some(file) = templates_by_node.get(node_id.as_str()) {
            files.insert((*file).to_string());
        }
        for relation in graph.relations.iter().filter(|relation| {
            relation.to == node_id
                && matches!(
                    relation.kind,
                    SourceRelationKind::Extends
                        | SourceRelationKind::Includes
                        | SourceRelationKind::Imports
                )
        }) {
            queue.push_back(relation.from.clone());
        }
    }
    let page_files = graph
        .pages
        .iter()
        .filter(|page| {
            page.template_node_id
                .as_ref()
                .is_some_and(|node_id| visited.contains(node_id))
                || page
                    .page_template_node_id
                    .as_ref()
                    .is_some_and(|node_id| visited.contains(node_id))
        })
        .map(|page| page.file.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    (files.into_iter().collect(), page_files)
}
