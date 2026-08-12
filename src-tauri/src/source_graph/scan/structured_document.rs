use std::{collections::HashMap, path::Path};

use crate::{
    localization::LocalizedDiagnostic,
    source_graph::{
        model::{
            SourceCapabilities, SourceCapabilityReason, SourceNodeKind, SourceOrigin,
            SourceStructuredDocument, SourceStructuredDocumentKind,
        },
        scan::{
            builder::SourceGraphBuilder,
            data_file::project_data_nodes_into_source_graph,
            files::{read_source, relative_project_path},
        },
        structured_data::parse_lossless_toml,
    },
};

pub(super) fn scan_structured_toml_document(
    project_root: &Path,
    path: &Path,
    kind: SourceStructuredDocumentKind,
    draft_sources: &HashMap<String, String>,
    builder: &mut SourceGraphBuilder,
) -> SourceStructuredDocument {
    let file = relative_project_path(project_root, path);
    let node_id = builder.add_node(
        SourceNodeKind::ConfigFile,
        file.clone(),
        SourceOrigin::Local,
        None,
        structured_document_label(&kind).to_string(),
        None,
        None,
        SourceCapabilities::code_only(SourceCapabilityReason::StructuredConfig),
    );
    let source = read_source(&file, draft_sources, builder);
    let (mut nodes, parse_error) = match parse_lossless_toml(&source, &file) {
        Ok(document) => (document.nodes, None),
        Err(error) => {
            builder.add_diagnostic(
                crate::source_graph::model::SourceDiagnosticSeverity::Error,
                LocalizedDiagnostic::new("source-graph-config-toml-invalid")
                    .with_argument("details", error.clone()),
                Some(file.clone()),
                None,
            );
            (Vec::new(), Some(error))
        }
    };
    project_data_nodes_into_source_graph(
        &file,
        &node_id,
        &SourceOrigin::Local,
        None,
        &mut nodes,
        builder,
    );
    SourceStructuredDocument {
        id: node_id.clone(),
        file,
        kind,
        node_id,
        parse_error,
        nodes,
    }
}

fn structured_document_label(kind: &SourceStructuredDocumentKind) -> &'static str {
    match kind {
        SourceStructuredDocumentKind::ZolaConfig => "Configurație Zola",
        SourceStructuredDocumentKind::ThemeConfig => "Configurație temă",
    }
}
