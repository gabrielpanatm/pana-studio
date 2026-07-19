use std::{
    collections::HashMap,
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
        identity::{source_node_id, source_relation_id},
        model::{
            SourceCapabilities, SourceGraph, SourceGraphTemplate, SourceNodeKind,
            SourceRelationKind,
        },
        tera::{parse_tera_items, TeraItem, TeraItemKind},
    },
};

pub(super) fn build_tera_graph(
    source_graph: &SourceGraph,
    files: &[ProjectModelFile],
) -> TeraGraph {
    let source_by_file: HashMap<&str, &str> = files
        .iter()
        .map(|file| (file.relative_path.as_str(), file.contents.as_str()))
        .collect();
    let mut graph = TeraGraph {
        templates: Vec::new(),
        nodes: Vec::new(),
        relations: Vec::new(),
    };

    for template in &source_graph.templates {
        let source = source_by_file
            .get(template.file.as_str())
            .copied()
            .unwrap_or("");
        add_template(template, source, &mut graph);
    }

    graph
        .templates
        .sort_by(|left, right| left.file.cmp(&right.file));
    graph
        .nodes
        .sort_by(|left, right| left.file.cmp(&right.file).then(left.id.cmp(&right.id)));
    graph
}

fn add_template(template: &SourceGraphTemplate, source: &str, graph: &mut TeraGraph) {
    let root_kind = if template.is_partial {
        SourceNodeKind::Partial
    } else {
        SourceNodeKind::Template
    };
    let root_range = if source.is_empty() {
        None
    } else {
        Some(source_range(source, 0, source.len()))
    };
    let root_node_id = source_node_id(
        &template.file,
        &root_kind,
        &template.name,
        root_range.as_ref().map(|range| range.start),
        root_range.as_ref().map(|range| range.end),
    );

    graph.nodes.push(TeraGraphNode {
        id: root_node_id.clone(),
        kind: root_kind,
        file: template.file.clone(),
        label: template.name.clone(),
        target: None,
        range: root_range,
        parent: None,
        children: Vec::new(),
        capabilities: SourceCapabilities::code_only("Rădăcină template în Project Model."),
    });

    graph.templates.push(TeraGraphTemplate {
        file: template.file.clone(),
        name: template.name.clone(),
        origin: template.origin.clone(),
        theme_name: template.theme_name.clone(),
        is_partial: template.is_partial,
        source_graph_template_id: template.id.clone(),
        source_graph_node_id: template.node_id.clone(),
        root_node_id: root_node_id.clone(),
        extends: template.extends.clone(),
        includes: template.includes.clone(),
        imports: template.imports.clone(),
        blocks: template.blocks.clone(),
        macros: template.macros.clone(),
    });

    let mut stack: Vec<String> = vec![root_node_id.clone()];
    for item in parse_tera_items(source) {
        match item.kind {
            TeraItemKind::EndScope => {
                if stack.len() > 1 {
                    let Some(node_id) = stack.pop() else {
                        continue;
                    };
                    if let Some(node) = graph.nodes.iter_mut().find(|node| node.id == node_id) {
                        let start = node
                            .range
                            .as_ref()
                            .map(|range| range.start)
                            .unwrap_or(item.start);
                        node.range = Some(source_range(source, start, item.end));
                    }
                }
            }
            TeraItemKind::Node => {
                let Some(kind) = item.node_kind.clone() else {
                    continue;
                };
                let parent = stack.last().cloned();
                let node_id = source_node_id(
                    &template.file,
                    &kind,
                    &item.label,
                    Some(item.start),
                    Some(item.end),
                );
                if let Some(parent_id) = parent.as_ref() {
                    if let Some(parent_node) =
                        graph.nodes.iter_mut().find(|node| node.id == *parent_id)
                    {
                        if !parent_node.children.contains(&node_id) {
                            parent_node.children.push(node_id.clone());
                        }
                    }
                    graph.relations.push(TeraGraphRelation {
                        id: tera_relation_id(
                            parent_id,
                            &node_id,
                            &TeraGraphRelationKind::Contains,
                            "contains",
                        ),
                        from: parent_id.clone(),
                        to: node_id.clone(),
                        kind: TeraGraphRelationKind::Contains,
                        label: "contains".to_string(),
                    });
                }

                graph.nodes.push(TeraGraphNode {
                    id: node_id.clone(),
                    kind: kind.clone(),
                    file: template.file.clone(),
                    label: item.label.clone(),
                    target: item.target.clone(),
                    range: Some(source_range(source, item.start, item.end)),
                    parent,
                    children: Vec::new(),
                    capabilities: tera_capabilities(&kind, &item),
                });
                add_semantic_relation(&root_node_id, &node_id, &kind, &item, graph);

                if tera_item_opens_scope(&kind, &item) {
                    stack.push(node_id);
                }
            }
        }
    }
}

fn add_semantic_relation(
    root_node_id: &str,
    node_id: &str,
    kind: &SourceNodeKind,
    item: &TeraItem,
    graph: &mut TeraGraph,
) {
    let Some((relation_kind, label)) = semantic_relation(kind, item) else {
        return;
    };
    graph.relations.push(TeraGraphRelation {
        id: tera_relation_id(root_node_id, node_id, &relation_kind, &label),
        from: root_node_id.to_string(),
        to: node_id.to_string(),
        kind: relation_kind,
        label,
    });
}

fn semantic_relation(
    kind: &SourceNodeKind,
    item: &TeraItem,
) -> Option<(TeraGraphRelationKind, String)> {
    match kind {
        SourceNodeKind::Extends => Some((
            TeraGraphRelationKind::Extends,
            item.target.clone().unwrap_or_else(|| item.label.clone()),
        )),
        SourceNodeKind::Include => Some((
            TeraGraphRelationKind::Includes,
            item.target.clone().unwrap_or_else(|| item.label.clone()),
        )),
        SourceNodeKind::Import => Some((
            TeraGraphRelationKind::Imports,
            item.target.clone().unwrap_or_else(|| item.label.clone()),
        )),
        SourceNodeKind::Block => Some((TeraGraphRelationKind::DefinesBlock, item.label.clone())),
        SourceNodeKind::Macro => Some((TeraGraphRelationKind::DefinesMacro, item.label.clone())),
        _ => None,
    }
}

fn tera_item_opens_scope(kind: &SourceNodeKind, item: &TeraItem) -> bool {
    match kind {
        SourceNodeKind::Block
        | SourceNodeKind::Macro
        | SourceNodeKind::For
        | SourceNodeKind::With => true,
        SourceNodeKind::If => !(item.label.starts_with("elif") || item.label == "else"),
        _ => false,
    }
}

fn tera_capabilities(kind: &SourceNodeKind, item: &TeraItem) -> SourceCapabilities {
    match kind {
        SourceNodeKind::Block => SourceCapabilities::code_only(
            "Block Tera; copiii HTML pot deveni editabili doar prin Move Engine.",
        ),
        SourceNodeKind::Include => SourceCapabilities::code_only(
            "Include Tera; mutarea afectează compoziția template-ului.",
        ),
        SourceNodeKind::For => SourceCapabilities::code_only(
            "Loop Tera; mutarea unei instanțe randate este blocată până există model de date.",
        ),
        SourceNodeKind::If => {
            SourceCapabilities::code_only("Ramură Tera condițională; mutarea cere plan de impact.")
        }
        SourceNodeKind::Macro => SourceCapabilities::code_only(
            "Macro Tera; modificarea poate afecta mai multe instanțe.",
        ),
        SourceNodeKind::Extends => SourceCapabilities::code_only(
            "Extends Tera; relație de layout, nu element mutabil vizual.",
        ),
        SourceNodeKind::Set => {
            SourceCapabilities::code_only("Set Tera; face parte din contextul de date.")
        }
        SourceNodeKind::With => {
            SourceCapabilities::code_only("With Tera; schimbă contextul local.")
        }
        SourceNodeKind::TeraVariable => SourceCapabilities::code_only("Variabilă Tera."),
        SourceNodeKind::TeraComment => SourceCapabilities::code_only("Comentariu Tera."),
        SourceNodeKind::Raw => SourceCapabilities::code_only("Bloc raw Tera."),
        _ => SourceCapabilities::code_only(format!("Sintaxă Tera: {}", item.label)),
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
