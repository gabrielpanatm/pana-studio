pub(crate) mod html;
pub(crate) mod identity;
pub(crate) mod literals;
pub mod model;
mod scan;
pub(crate) mod tera;
pub(crate) mod zola;

pub use model::SourceGraph;
pub use scan::{
    build_source_graph, build_source_graph_from_workspace_projection,
    build_source_graph_with_projection,
};
