use std::path::Path;

use crate::source_graph::{
    identity::{source_relation_id, SourceIdentityAssigner},
    model::{
        SourceCapabilities, SourceDiagnosticSeverity, SourceGraph, SourceGraphAsset,
        SourceGraphDataFile, SourceGraphDiagnostic, SourceGraphPage, SourceGraphScript,
        SourceGraphStyle, SourceGraphTemplate, SourceNode, SourceNodeKind, SourceOrigin,
        SourceRange, SourceRelation, SourceRelationKind, SourceStructuredDocument,
    },
};

pub(super) struct SourceGraphBuilder {
    project_root: String,
    zola_root: String,
    active_theme: Option<String>,
    nodes: Vec<SourceNode>,
    relations: Vec<SourceRelation>,
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
            relations: Vec::new(),
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
            if let Some(parent_node) = self.nodes.iter_mut().find(|node| node.id == *parent_id) {
                if !parent_node.children.contains(&id) {
                    parent_node.children.push(id.clone());
                }
            }
        }

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
        if self.relations.iter().any(|relation| relation.id == id) {
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
        message: impl Into<String>,
        file: Option<String>,
        range: Option<SourceRange>,
    ) {
        self.diagnostics.push(SourceGraphDiagnostic {
            severity,
            message: message.into(),
            file,
            range,
        });
    }

    pub(super) fn update_node_range(&mut self, node_id: &str, range: SourceRange) {
        if let Some(node) = self.nodes.iter_mut().find(|node| node.id == node_id) {
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
            nodes: self.nodes,
            relations: self.relations,
            diagnostics: self.diagnostics,
        }
    }
}
