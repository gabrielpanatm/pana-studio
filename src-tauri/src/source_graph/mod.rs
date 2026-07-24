pub(crate) mod component_graph;
pub(crate) mod html;
pub(crate) mod identity;
pub(crate) mod literals;
pub(crate) mod mixed_cst;
pub mod model;
mod scan;
pub(crate) mod structured_data;
pub mod template_catalog;
pub(crate) mod tera;
pub(crate) mod tera_cst;
pub mod tera_semantics;
pub(crate) mod zola;
pub mod zola_shortcode;

pub use model::SourceGraph;
pub use scan::{
    build_source_graph, build_source_graph_from_workspace_projection,
    build_source_graph_with_projection,
};
pub use template_catalog::{build_template_catalog, TemplateCatalogSnapshot};
