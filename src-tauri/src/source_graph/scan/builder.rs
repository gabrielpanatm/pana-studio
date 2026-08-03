use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use crate::{
    localization::LocalizedDiagnostic,
    source_graph::{
        identity::{source_relation_id, SourceIdentityAssigner},
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
    identities: SourceIdentityAssigner,
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
            identities: SourceIdentityAssigner::default(),
        }
    }

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
        let id = self.identities.next(&file, &kind, &label);

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

    pub(super) fn finish(
        self,
        mut pages: Vec<SourceGraphPage>,
        mut templates: Vec<SourceGraphTemplate>,
        mut styles: Vec<SourceGraphStyle>,
        mut scripts: Vec<SourceGraphScript>,
        mut assets: Vec<SourceGraphAsset>,
        mut data_files: Vec<SourceGraphDataFile>,
        mut structured_documents: Vec<SourceStructuredDocument>,
    ) -> SourceGraph {
        pages.sort_by(|left, right| left.file.cmp(&right.file));
        templates.sort_by(|left, right| left.file.cmp(&right.file));
        styles.sort_by(|left, right| left.file.cmp(&right.file));
        scripts.sort_by(|left, right| left.file.cmp(&right.file));
        assets.sort_by(|left, right| left.file.cmp(&right.file));
        data_files.sort_by(|left, right| left.file.cmp(&right.file));
        structured_documents.sort_by(|left, right| left.file.cmp(&right.file));
        SourceGraph {
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
            diagnostics: self.diagnostics,
        }
    }
}
