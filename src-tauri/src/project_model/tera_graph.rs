use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
};

use crate::{
    project_model::{
        model::{
            ProjectModelFile, TeraGraph, TeraGraphNode, TeraGraphRelation, TeraGraphRelationKind,
            TeraGraphTemplate,
        },
        ranges::source_range,
    },
    source_graph::{
        identity::source_relation_id,
        model::{SourceGraph, SourceGraphTemplate, SourceNode, SourceNodeKind, SourceRelationKind},
    },
};

/// TeraGraph is a compatibility projection of the canonical SourceGraph.
///
/// Parsing and scope construction happen once, while SourceGraph is built from
/// the lossless CST plus the embedded Tera AST. This projection deliberately
/// contains no parser and no independent scope stack.
pub(super) fn build_tera_graph(
    source_graph: &SourceGraph,
    files: &[ProjectModelFile],
) -> TeraGraph {
    let source_by_file: HashMap<&str, &str> = files
        .iter()
        .map(|file| (file.relative_path.as_str(), file.contents.as_str()))
        .collect();
    let nodes_by_id = source_graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<HashMap<_, _>>();
    let mut graph = TeraGraph {
        templates: Vec::new(),
        nodes: Vec::new(),
        relations: Vec::new(),
    };

    for template in &source_graph.templates {
        add_template(
            template,
            source_by_file.get(template.file.as_str()).copied(),
            &source_graph.nodes,
            &nodes_by_id,
            &mut graph,
        );
    }

    graph
        .templates
        .sort_by(|left, right| left.file.cmp(&right.file));
    graph
        .nodes
        .sort_by(|left, right| left.file.cmp(&right.file).then(left.id.cmp(&right.id)));
    graph
        .relations
        .sort_by(|left, right| left.id.cmp(&right.id));
    graph
}

fn add_template(
    template: &SourceGraphTemplate,
    source: Option<&str>,
    source_nodes: &[SourceNode],
    nodes_by_id: &HashMap<&str, &SourceNode>,
    graph: &mut TeraGraph,
) {
    let included_ids = source_nodes
        .iter()
        .filter(|node| node.file == template.file && is_tera_graph_kind(&node.kind))
        .map(|node| node.id.as_str())
        .collect::<HashSet<_>>();
    let root_range = source
        .filter(|source| !source.is_empty())
        .map(|source| source_range(source, 0, source.len()));

    graph.templates.push(TeraGraphTemplate {
        file: template.file.clone(),
        name: template.name.clone(),
        origin: template.origin.clone(),
        theme_name: template.theme_name.clone(),
        is_partial: template.is_partial,
        source_graph_template_id: template.id.clone(),
        source_graph_node_id: template.node_id.clone(),
        root_node_id: template.node_id.clone(),
        extends: template.extends.clone(),
        includes: template.includes.clone(),
        imports: template.imports.clone(),
        blocks: template.blocks.clone(),
        macros: template.macros.clone(),
    });

    for node in source_nodes
        .iter()
        .filter(|node| included_ids.contains(node.id.as_str()))
    {
        let parent = node
            .parent
            .as_ref()
            .filter(|parent| included_ids.contains(parent.as_str()))
            .cloned();
        let children = node
            .children
            .iter()
            .filter(|child| included_ids.contains(child.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        let range = if node.id == template.node_id {
            node.range.clone().or_else(|| root_range.clone())
        } else {
            node.range.clone()
        };

        graph.nodes.push(TeraGraphNode {
            id: node.id.clone(),
            kind: node.kind.clone(),
            file: node.file.clone(),
            label: node.label.clone(),
            target: target_from_source_node(node),
            range,
            parent: parent.clone(),
            children,
            capabilities: node.capabilities.clone(),
        });

        if let Some(parent) = parent {
            graph.relations.push(TeraGraphRelation {
                id: tera_relation_id(
                    &parent,
                    &node.id,
                    &TeraGraphRelationKind::Contains,
                    "contains",
                ),
                from: parent,
                to: node.id.clone(),
                kind: TeraGraphRelationKind::Contains,
                label: "contains".to_string(),
            });
        }
        if node.id != template.node_id {
            add_semantic_relation(&template.node_id, node, graph);
        }
    }

    debug_assert!(
        nodes_by_id.contains_key(template.node_id.as_str()),
        "SourceGraph template root must exist"
    );
}

fn is_tera_graph_kind(kind: &SourceNodeKind) -> bool {
    matches!(
        kind,
        SourceNodeKind::Template
            | SourceNodeKind::Partial
            | SourceNodeKind::Extends
            | SourceNodeKind::Block
            | SourceNodeKind::Include
            | SourceNodeKind::Import
            | SourceNodeKind::Macro
            | SourceNodeKind::For
            | SourceNodeKind::If
            | SourceNodeKind::Elif
            | SourceNodeKind::Else
            | SourceNodeKind::Set
            | SourceNodeKind::SetGlobal
            | SourceNodeKind::Filter
            | SourceNodeKind::Break
            | SourceNodeKind::Continue
            | SourceNodeKind::Super
            | SourceNodeKind::TeraVariable
            | SourceNodeKind::TeraComment
            | SourceNodeKind::Raw
            | SourceNodeKind::Tera
    )
}

fn target_from_source_node(node: &SourceNode) -> Option<String> {
    let prefix = match node.kind {
        SourceNodeKind::Extends => "extends ",
        SourceNodeKind::Include => "include ",
        SourceNodeKind::Import => "import ",
        _ => return None,
    };
    node.label.strip_prefix(prefix).map(str::to_string)
}

fn add_semantic_relation(root_node_id: &str, node: &SourceNode, graph: &mut TeraGraph) {
    let Some(kind) = relation_kind_for_node(&node.kind) else {
        return;
    };
    let label = target_from_source_node(node).unwrap_or_else(|| node.label.clone());
    graph.relations.push(TeraGraphRelation {
        id: tera_relation_id(root_node_id, &node.id, &kind, &label),
        from: root_node_id.to_string(),
        to: node.id.clone(),
        kind,
        label,
    });
}

fn relation_kind_for_node(kind: &SourceNodeKind) -> Option<TeraGraphRelationKind> {
    match kind {
        SourceNodeKind::Extends => Some(TeraGraphRelationKind::Extends),
        SourceNodeKind::Include => Some(TeraGraphRelationKind::Includes),
        SourceNodeKind::Import => Some(TeraGraphRelationKind::Imports),
        SourceNodeKind::Block => Some(TeraGraphRelationKind::DefinesBlock),
        SourceNodeKind::Macro => Some(TeraGraphRelationKind::DefinesMacro),
        _ => None,
    }
}

fn tera_relation_id(from: &str, to: &str, kind: &TeraGraphRelationKind, label: &str) -> String {
    match kind {
        TeraGraphRelationKind::Extends => {
            source_relation_id(from, to, &SourceRelationKind::Extends, label)
        }
        TeraGraphRelationKind::Includes => {
            source_relation_id(from, to, &SourceRelationKind::Includes, label)
        }
        TeraGraphRelationKind::Imports => {
            source_relation_id(from, to, &SourceRelationKind::Imports, label)
        }
        TeraGraphRelationKind::DefinesBlock => {
            source_relation_id(from, to, &SourceRelationKind::DefinesBlock, label)
        }
        _ => {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            from.hash(&mut hasher);
            to.hash(&mut hasher);
            tera_relation_kind_key(kind).hash(&mut hasher);
            label.hash(&mut hasher);
            format!("tg_rel_{:016x}", hasher.finish())
        }
    }
}

fn tera_relation_kind_key(kind: &TeraGraphRelationKind) -> &'static str {
    match kind {
        TeraGraphRelationKind::Contains => "contains",
        TeraGraphRelationKind::Extends => "extends",
        TeraGraphRelationKind::Includes => "includes",
        TeraGraphRelationKind::Imports => "imports",
        TeraGraphRelationKind::DefinesBlock => "defines_block",
        TeraGraphRelationKind::DefinesMacro => "defines_macro",
    }
}
