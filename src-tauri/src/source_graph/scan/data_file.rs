use std::{collections::HashMap, path::Path};

use crate::source_graph::{
    model::{
        SourceCapabilities, SourceDataNode, SourceDataNodeKind, SourceDataPathSegment,
        SourceNodeKind, SourceOrigin,
    },
    scan::{
        builder::SourceGraphBuilder,
        files::{read_source, relative_project_path},
        ranges::source_range,
        summary::DataFileSummary,
    },
    structured_data::{data_format_for_file, parse_lossless_toml, parse_zola_data_adapter},
    zola::zola_data_file_logical_path,
};

pub(super) const ZOLA_DATA_FILE_EXTENSIONS: &[&str] =
    &["toml", "json", "yaml", "yml", "csv", "bib", "bibtex", "xml"];

pub(super) fn scan_data_file(
    project_root: &Path,
    zola_root: &Path,
    path: &Path,
    origin: SourceOrigin,
    theme_name: Option<String>,
    draft_sources: &HashMap<String, String>,
    builder: &mut SourceGraphBuilder,
) -> DataFileSummary {
    let file = relative_project_path(project_root, path);
    let logical_path = zola_data_file_logical_path(zola_root, path).unwrap_or_else(|| file.clone());
    let node_id = builder.add_node(
        SourceNodeKind::DataFile,
        file.clone(),
        origin.clone(),
        theme_name.clone(),
        logical_path.clone(),
        None,
        None,
        SourceCapabilities::code_only("Fișier de date local Zola."),
    );
    let source = read_source(path, &file, draft_sources, builder);
    let format = data_format_for_file(&file);
    let (mut nodes, parse_error) =
        if matches!(format, crate::source_graph::model::SourceDataFormat::Toml) {
            match parse_lossless_toml(&source, &file) {
                Ok(document) => (document.nodes, None),
                Err(error) => {
                    builder.add_diagnostic(
                        crate::source_graph::model::SourceDiagnosticSeverity::Error,
                        format!("Document TOML invalid: {error}"),
                        Some(file.clone()),
                        None,
                    );
                    (opaque_document_node(&file, &source), Some(error))
                }
            }
        } else if matches!(
            format,
            crate::source_graph::model::SourceDataFormat::Unknown
        ) {
            (opaque_document_node(&file, &source), None)
        } else {
            match parse_zola_data_adapter(&source, &file, &format) {
                Ok(nodes) => (nodes, None),
                Err(error) => {
                    builder.add_diagnostic(
                        crate::source_graph::model::SourceDiagnosticSeverity::Error,
                        format!("Document de date {:?} invalid: {error}", format),
                        Some(file.clone()),
                        None,
                    );
                    (opaque_document_node(&file, &source), Some(error))
                }
            }
        };
    project_data_nodes_into_source_graph(
        &file,
        &node_id,
        &origin,
        theme_name.as_ref(),
        &mut nodes,
        builder,
    );
    DataFileSummary {
        file,
        node_id,
        origin,
        theme_name,
        logical_path,
        format,
        parse_error,
        nodes,
    }
}

fn opaque_document_node(file: &str, source: &str) -> Vec<SourceDataNode> {
    vec![SourceDataNode {
        id: format!("opaque:{file}"),
        kind: SourceDataNodeKind::Opaque,
        path: Vec::new(),
        key: None,
        value_kind: None,
        value_preview: None,
        range: Some(source_range(source, 0, source.len())),
        key_range: None,
        parent_id: None,
        children: Vec::new(),
    }]
}

pub(super) fn project_data_nodes_into_source_graph(
    file: &str,
    data_file_node_id: &str,
    origin: &SourceOrigin,
    theme_name: Option<&String>,
    nodes: &mut [SourceDataNode],
    builder: &mut SourceGraphBuilder,
) {
    let mut graph_ids = HashMap::<String, String>::new();
    if let Some(root) = nodes.first() {
        graph_ids.insert(root.id.clone(), data_file_node_id.to_string());
    }

    for node in nodes.iter().skip(1) {
        let parent = node
            .parent_id
            .as_ref()
            .and_then(|parent| graph_ids.get(parent))
            .cloned()
            .unwrap_or_else(|| data_file_node_id.to_string());
        let graph_id = builder.add_node(
            source_kind_for_data_node(&node.kind),
            file.to_string(),
            origin.clone(),
            theme_name.cloned(),
            data_node_label(node),
            node.range.clone(),
            Some(parent),
            SourceCapabilities::code_only(
                "Nod de date structurat; editarea este planificată lossless în Rust.",
            ),
        );
        graph_ids.insert(node.id.clone(), graph_id);
    }

    for node in nodes {
        if let Some(graph_id) = graph_ids.get(&node.id) {
            node.id = graph_id.clone();
        }
        node.parent_id = node
            .parent_id
            .as_ref()
            .and_then(|parent| graph_ids.get(parent))
            .cloned();
        node.children = node
            .children
            .iter()
            .filter_map(|child| graph_ids.get(child).cloned())
            .collect();
    }
}

fn source_kind_for_data_node(kind: &SourceDataNodeKind) -> SourceNodeKind {
    match kind {
        SourceDataNodeKind::Table
        | SourceDataNodeKind::TableElement
        | SourceDataNodeKind::InlineTable => SourceNodeKind::DataTable,
        SourceDataNodeKind::ArrayOfTables
        | SourceDataNodeKind::Array
        | SourceDataNodeKind::ArrayElement => SourceNodeKind::DataArray,
        SourceDataNodeKind::Comment => SourceNodeKind::DataComment,
        SourceDataNodeKind::Value => SourceNodeKind::DataValue,
        SourceDataNodeKind::Document | SourceDataNodeKind::Opaque => SourceNodeKind::DataFile,
    }
}

fn data_node_label(node: &SourceDataNode) -> String {
    if let Some(key) = node.key.as_ref() {
        return key.clone();
    }
    if node.path.is_empty() {
        return "document".to_string();
    }
    node.path
        .iter()
        .map(|segment| match segment {
            SourceDataPathSegment::Key(key) => key.clone(),
            SourceDataPathSegment::Index(index) => format!("[{index}]"),
        })
        .collect::<Vec<_>>()
        .join(".")
}
