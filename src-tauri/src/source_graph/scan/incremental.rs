use std::{
    collections::{BTreeSet, HashMap, HashSet, VecDeque},
    path::Path,
    time::Instant,
};

use crate::source_graph::{
    identity::{reconcile_fragment_source_node_ids, SourceChangeSet},
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
    pub(crate) template_parse_us: u64,
    pub(crate) component_graph_us: u64,
    pub(crate) block_graph_us: u64,
    pub(crate) content_model_us: u64,
    pub(crate) listing_items_us: u64,
    pub(crate) listing_items_reused: bool,
    pub(crate) dynamic_widget_us: u64,
    pub(crate) markdown_us: u64,
    pub(crate) node_index_us: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LocalTemplateDerivedInvalidationPlan<'a> {
    component_graph_file: &'a str,
    block_graph_file: &'a str,
    content_model_usages_file: &'a str,
    listing_items_reused: bool,
    dynamic_widgets_file: &'a str,
    markdown_file: &'a str,
}

impl<'a> LocalTemplateDerivedInvalidationPlan<'a> {
    fn stable_dependency_contract(template_file: &'a str) -> Self {
        Self {
            component_graph_file: template_file,
            block_graph_file: template_file,
            content_model_usages_file: template_file,
            listing_items_reused: true,
            dynamic_widgets_file: template_file,
            markdown_file: template_file,
        }
    }
}

pub(crate) fn rebuild_local_template_graph(
    mut graph: SourceGraph,
    project_root: &Path,
    zola_root: &Path,
    relative_path: &str,
    previous_source: &str,
    projected_sources: &HashMap<String, String>,
    supplied_source_change: Option<SourceChangeSet>,
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

    let (mut invalidated_template_files, mut invalidated_page_files) =
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
    let mut fragment = fragment_builder.finish(
        Vec::new(),
        vec![next_template.clone()],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let next_source = projected_sources
        .get(relative_path)
        .ok_or(SourceGraphIncrementalFallback::MissingTemplate)?;
    let mut source_change_set = supplied_source_change
        .filter(|change| change.file == relative_path)
        .unwrap_or_else(|| SourceChangeSet::between(relative_path, previous_source, next_source));
    source_change_set
        .require_sources(previous_source, next_source)
        .map_err(|_| SourceGraphIncrementalFallback::IdentityCollision)?;
    reconcile_fragment_source_node_ids(&graph, &mut fragment, &mut source_change_set)
        .map_err(|_| SourceGraphIncrementalFallback::IdentityCollision)?;
    let next_template = fragment
        .templates
        .first()
        .cloned()
        .ok_or(SourceGraphIncrementalFallback::MissingTemplate)?;
    if !previous_template.component_definitions.is_empty()
        || !next_template.component_definitions.is_empty()
    {
        let (component_templates, component_pages) = component_consumers(
            &graph,
            previous_template
                .component_definitions
                .iter()
                .chain(next_template.component_definitions.iter())
                .map(|definition| definition.name.as_str()),
        );
        invalidated_template_files.extend(component_templates);
        invalidated_template_files.sort();
        invalidated_template_files.dedup();
        invalidated_page_files.extend(component_pages);
        invalidated_page_files.sort();
        invalidated_page_files.dedup();
    }
    let template_parse_us = elapsed_us(template_parse_started);
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
    let invalidation =
        LocalTemplateDerivedInvalidationPlan::stable_dependency_contract(relative_path);

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
    // Every derived graph resolves source nodes through the O(1) index. The
    // splice may shift all following nodes even when their stable IDs survive,
    // so the old positional index must not escape into component ranges.
    let node_index_started = Instant::now();
    graph
        .rebuild_node_index()
        .map_err(|_| SourceGraphIncrementalFallback::IdentityCollision)?;
    let node_index_us = elapsed_us(node_index_started);
    let component_graph_started = Instant::now();
    let previous_component_graph = std::mem::take(&mut graph.component_graph);
    graph.component_graph = crate::source_graph::component_graph::upsert_component_graph_template(
        &graph,
        previous_component_graph,
        invalidation.component_graph_file,
    );
    let component_graph_us = elapsed_us(component_graph_started);
    let block_graph_started = Instant::now();
    let mut block_graph = std::mem::take(&mut graph.block_graph);
    crate::blocks::graph::upsert_block_graph_template(
        &graph,
        &mut block_graph,
        invalidation.block_graph_file,
    );
    graph.block_graph = block_graph;
    let block_graph_us = elapsed_us(block_graph_started);
    let content_model_started = Instant::now();
    let mut content_models = std::mem::take(&mut graph.content_models);
    crate::kernel::content_models::upsert_content_model_template_usages(
        projected_sources,
        &graph,
        &mut content_models,
        invalidation.content_model_usages_file,
    );
    graph.content_models = content_models;
    let content_model_us = elapsed_us(content_model_started);
    let listing_items_started = Instant::now();
    // The dependency contract above proves the listing inputs (template
    // identity/includes/content assignments) are unchanged for this lane.
    let listing_items_us = elapsed_us(listing_items_started);
    let dynamic_widget_started = Instant::now();
    let mut dynamic_widget_graph = std::mem::take(&mut graph.dynamic_widget_graph);
    crate::kernel::dynamic_widgets::upsert_dynamic_widget_graph_template(
        projected_sources,
        &graph,
        &mut dynamic_widget_graph,
        invalidation.dynamic_widgets_file,
    );
    graph.dynamic_widget_graph = dynamic_widget_graph;
    let dynamic_widget_us = elapsed_us(dynamic_widget_started);
    let markdown_started = Instant::now();
    let mut markdown_projections = std::mem::take(&mut graph.markdown_projections);
    let markdown_template = graph
        .templates
        .iter()
        .find(|template| template.file == invalidation.markdown_file)
        .expect("invalidation plan references the replaced template");
    crate::source_graph::markdown::upsert_markdown_template(
        &mut markdown_projections,
        markdown_template,
    );
    graph.markdown_projections = markdown_projections;
    let markdown_us = elapsed_us(markdown_started);
    Ok((
        graph,
        SourceGraphIncrementalTemplateReport {
            invalidated_template_files,
            invalidated_page_files,
            replaced_nodes,
            reused_nodes,
            reused_relations,
            template_parse_us,
            component_graph_us,
            block_graph_us,
            content_model_us,
            listing_items_us,
            listing_items_reused: invalidation.listing_items_reused,
            dynamic_widget_us,
            markdown_us,
            node_index_us,
        },
    ))
}

fn elapsed_us(started: Instant) -> u64 {
    started.elapsed().as_micros().min(u64::MAX as u128) as u64
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
        && previous.get_pages == next.get_pages
        && previous.get_sections == next.get_sections
        && previous.internal_links == next.internal_links
        && previous.asset_urls == next.asset_urls
        && previous.asset_hashes == next.asset_hashes
        && previous.literal_asset_references == next.literal_asset_references
        && previous.asset_reference_eligible == next.asset_reference_eligible
        && previous.asset_reference_unanalysable == next.asset_reference_unanalysable
        && previous.data_loads == next.data_loads
        && previous.image_metadata == next.image_metadata
        && previous.image_resizes == next.image_resizes
        && previous.blocks == next.blocks
}

fn component_consumers<'a>(
    graph: &SourceGraph,
    component_names: impl Iterator<Item = &'a str>,
) -> (Vec<String>, Vec<String>) {
    let names = component_names.collect::<HashSet<_>>();
    let template_files = graph
        .templates
        .iter()
        .filter(|template| {
            template
                .component_calls
                .iter()
                .any(|call| names.contains(call.name.as_str()))
        })
        .map(|template| template.file.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let page_files = graph
        .pages
        .iter()
        .filter(|page| {
            page.component_calls
                .iter()
                .any(|call| names.contains(call.name.as_str()))
        })
        .map(|page| page.file.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    (template_files, page_files)
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
        SourceRelationKind::AssetReference => "asset_reference",
        SourceRelationKind::DataLoad => "data_load",
        SourceRelationKind::DataFileLoad => "data_file_load",
        SourceRelationKind::ContentDataLoad => "content_data_load",
        SourceRelationKind::ImageMetadata => "image_metadata",
        SourceRelationKind::ImageResize => "image_resize",
        SourceRelationKind::Extends => "extends",
        SourceRelationKind::Includes => "includes",
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
                    SourceRelationKind::Extends | SourceRelationKind::Includes
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
