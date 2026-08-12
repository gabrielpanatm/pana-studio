use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use crate::{
    localization::LocalizedDiagnostic,
    source_graph::{
        identity::{source_relation_id, ProvisionalSourceNodeIdAllocator},
        model::{
            SourceCapabilities, SourceDiagnosticSeverity, SourceGraph, SourceGraphAsset,
            SourceGraphDataFile, SourceGraphDiagnostic, SourceGraphPage, SourceGraphScript,
            SourceGraphStyle, SourceGraphTemplate, SourceNode, SourceNodeKind, SourceOrigin,
            SourceRange, SourceRelation, SourceRelationKind, SourceStructuredDocument,
        },
    },
};

pub(super) struct SourceGraphBuilder {
    project_root: String,
    zola_root: String,
    active_theme: Option<String>,
    nodes: Vec<SourceNode>,
    node_indexes: HashMap<String, usize>,
    relations: Vec<SourceRelation>,
    relation_ids: HashSet<String>,
    diagnostics: Vec<SourceGraphDiagnostic>,
    provisional_ids: ProvisionalSourceNodeIdAllocator,
}

impl SourceGraphBuilder {
    pub(super) fn new(project_root: &Path, zola_root: &Path, active_theme: Option<String>) -> Self {
        Self {
            project_root: project_root.to_string_lossy().to_string(),
            zola_root: zola_root.to_string_lossy().to_string(),
            active_theme,
            nodes: Vec::new(),
            node_indexes: HashMap::new(),
            relations: Vec::new(),
            relation_ids: HashSet::new(),
            diagnostics: Vec::new(),
            provisional_ids: ProvisionalSourceNodeIdAllocator::default(),
        }
    }

    // Nodes are appended as complete immutable Source Graph records; fields remain explicit.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn add_node(
        &mut self,
        kind: SourceNodeKind,
        file: String,
        origin: SourceOrigin,
        theme_name: Option<String>,
        label: String,
        range: Option<SourceRange>,
        parent: Option<String>,
        capabilities: SourceCapabilities,
    ) -> String {
        let id = self.provisional_ids.next();

        if let Some(parent_id) = parent.as_ref() {
            if let Some(parent_node) = self
                .node_indexes
                .get(parent_id)
                .and_then(|index| self.nodes.get_mut(*index))
            {
                if !parent_node.children.contains(&id) {
                    parent_node.children.push(id.clone());
                }
            }
        }

        let node_index = self.nodes.len();
        self.nodes.push(SourceNode {
            id: id.clone(),
            kind,
            file,
            origin,
            theme_name,
            label,
            range,
            parent,
            children: Vec::new(),
            capabilities,
        });
        self.node_indexes.insert(id.clone(), node_index);
        id
    }

    pub(super) fn add_relation(
        &mut self,
        from: String,
        to: String,
        kind: SourceRelationKind,
        label: impl Into<String>,
    ) {
        if from == to {
            return;
        }
        let label = label.into();
        let id = source_relation_id(&from, &to, &kind, &label);
        if !self.relation_ids.insert(id.clone()) {
            return;
        }
        self.relations.push(SourceRelation {
            id,
            from,
            to,
            kind,
            label,
        });
    }

    pub(super) fn add_diagnostic(
        &mut self,
        severity: SourceDiagnosticSeverity,
        diagnostic: LocalizedDiagnostic,
        file: Option<String>,
        range: Option<SourceRange>,
    ) {
        self.diagnostics.push(SourceGraphDiagnostic {
            severity,
            diagnostic,
            file,
            range,
        });
    }

    pub(super) fn update_node_range(&mut self, node_id: &str, range: SourceRange) {
        if let Some(node) = self
            .node_indexes
            .get(node_id)
            .and_then(|index| self.nodes.get_mut(*index))
        {
            node.range = Some(range);
        }
    }

    pub(super) fn reparent_node(&mut self, node_id: &str, parent: &str) {
        let Some(node_index) = self.node_indexes.get(node_id).copied() else {
            return;
        };
        let previous_parent = self.nodes[node_index].parent.clone();
        if previous_parent.as_deref() == Some(parent) {
            return;
        }
        if let Some(previous_parent) = previous_parent {
            if let Some(parent_node) = self
                .node_indexes
                .get(&previous_parent)
                .and_then(|index| self.nodes.get_mut(*index))
            {
                parent_node.children.retain(|child| child != node_id);
            }
        }
        if let Some(parent_node) = self
            .node_indexes
            .get(parent)
            .and_then(|index| self.nodes.get_mut(*index))
        {
            if !parent_node.children.iter().any(|child| child == node_id) {
                parent_node.children.push(node_id.to_string());
            }
            self.nodes[node_index].parent = Some(parent.to_string());
        }
    }

    fn sort_node_children_by_source_order(&mut self) {
        let positions = self
            .nodes
            .iter()
            .map(|node| {
                (
                    node.id.clone(),
                    (
                        node.file.clone(),
                        node.range
                            .as_ref()
                            .map(|range| range.start)
                            .unwrap_or(usize::MAX),
                    ),
                )
            })
            .collect::<HashMap<_, _>>();
        for node in &mut self.nodes {
            node.children.sort_by(|left, right| {
                positions
                    .get(left)
                    .cmp(&positions.get(right))
                    .then_with(|| left.cmp(right))
            });
        }
    }

    // Finish receives each independently sorted Source Graph catalog exactly once.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn finish(
        mut self,
        mut pages: Vec<SourceGraphPage>,
        mut templates: Vec<SourceGraphTemplate>,
        mut styles: Vec<SourceGraphStyle>,
        mut scripts: Vec<SourceGraphScript>,
        mut assets: Vec<SourceGraphAsset>,
        mut data_files: Vec<SourceGraphDataFile>,
        mut structured_documents: Vec<SourceStructuredDocument>,
    ) -> SourceGraph {
        self.sort_node_children_by_source_order();
        pages.sort_by(|left, right| left.file.cmp(&right.file));
        templates.sort_by(|left, right| left.file.cmp(&right.file));
        styles.sort_by(|left, right| left.file.cmp(&right.file));
        scripts.sort_by(|left, right| left.file.cmp(&right.file));
        assets.sort_by(|left, right| left.file.cmp(&right.file));
        data_files.sort_by(|left, right| left.file.cmp(&right.file));
        structured_documents.sort_by(|left, right| left.file.cmp(&right.file));
        SourceGraph {
            node_index: Default::default(),
            project_root: self.project_root,
            zola_root: self.zola_root,
            active_theme: self.active_theme,
            pages,
            templates,
            styles,
            scripts,
            assets,
            data_files,
            structured_documents,
            component_graph: Default::default(),
            block_graph: Default::default(),
            content_models: Default::default(),
            listing_items: Default::default(),
            dynamic_widget_graph: Default::default(),
            markdown_projections: Vec::new(),
            nodes: self.nodes,
            relations: self.relations,
            asset_reference_coverage: Default::default(),
            diagnostics: self.diagnostics,
        }
    }
}
