use std::path::Path;

use crate::source_graph::{
    model::{SourceCapabilities, SourceNodeKind, SourceOrigin},
    scan::{builder::SourceGraphBuilder, files::relative_project_path, summary::AssetSummary},
    zola::static_asset_logical_path,
};

pub(super) fn scan_asset(
    project_root: &Path,
    zola_root: &Path,
    path: &Path,
    origin: SourceOrigin,
    theme_name: Option<String>,
    builder: &mut SourceGraphBuilder,
) -> AssetSummary {
    let file = relative_project_path(project_root, path);
    let logical_path = static_asset_logical_path(zola_root, path, theme_name.as_deref())
        .unwrap_or_else(|| file.strip_prefix("static/").unwrap_or(&file).to_string());
    let is_script = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("js"));
    let node_id = builder.add_node(
        if is_script {
            SourceNodeKind::Script
        } else {
            SourceNodeKind::Asset
        },
        file.clone(),
        origin.clone(),
        theme_name.clone(),
        logical_path.clone(),
        None,
        None,
        SourceCapabilities::code_only(if is_script {
            "Fișier JavaScript static Zola."
        } else {
            "Asset static Zola."
        }),
    );
    AssetSummary {
        file,
        node_id,
        origin,
        theme_name,
        logical_path,
        is_script,
    }
}
