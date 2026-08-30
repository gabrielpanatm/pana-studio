pub(crate) mod asset_references;
pub(crate) mod component_graph;
pub(crate) mod html;
pub(crate) mod identity;
pub(crate) mod literals;
pub(crate) mod markdown;
pub(crate) mod mixed_cst;
pub mod model;
mod scan;
pub(crate) mod structured_data;
pub mod taxonomy_catalog;
pub mod template_catalog;
pub(crate) mod tera;
pub(crate) mod tera_cst;
pub mod tera_semantics;
pub(crate) mod zola;

pub use model::SourceGraph;
pub use scan::build_source_graph_from_workspace_projection;

#[cfg(test)]
pub(crate) fn build_source_graph_from_integration_disk_boundary(
    project_root: &std::path::Path,
) -> Result<SourceGraph, String> {
    crate::project_model::test_support::ProjectModelTestFixture::from_integration_disk_boundary(
        project_root,
    )?
    .build_source_graph()
}
pub(crate) use scan::{
    build_source_graph_for_audit_from_workspace_projection, rebuild_local_template_graph,
    SourceGraphIncrementalFallback, SourceGraphIncrementalTemplateReport,
};
pub use taxonomy_catalog::{build_taxonomy_catalog, TaxonomyCatalogSnapshot};
pub use template_catalog::{
    build_template_catalog, build_template_catalog_with_taxonomies, TemplateCatalogSnapshot,
};
