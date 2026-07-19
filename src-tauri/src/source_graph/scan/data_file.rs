use std::path::Path;

use crate::source_graph::{
    model::{SourceCapabilities, SourceNodeKind, SourceOrigin},
    scan::{builder::SourceGraphBuilder, files::relative_project_path, summary::DataFileSummary},
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
    DataFileSummary {
        file,
        node_id,
        origin,
        theme_name,
        logical_path,
    }
}
