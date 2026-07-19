use crate::source_graph::model::{SourceNodeKind, SourceOrigin};

#[derive(Clone)]
pub(super) struct TemplateSummary {
    pub(super) id: String,
    pub(super) file: String,
    pub(super) name: String,
    pub(super) node_id: String,
    pub(super) origin: SourceOrigin,
    pub(super) theme_name: Option<String>,
    pub(super) is_partial: bool,
    pub(super) extends: Option<String>,
    pub(super) includes: Vec<String>,
    pub(super) imports: Vec<String>,
    pub(super) get_pages: Vec<String>,
    pub(super) get_sections: Vec<String>,
    pub(super) internal_links: Vec<String>,
    pub(super) asset_urls: Vec<String>,
    pub(super) asset_hashes: Vec<String>,
    pub(super) data_loads: Vec<String>,
    pub(super) image_metadata: Vec<String>,
    pub(super) image_resizes: Vec<String>,
    pub(super) blocks: Vec<(String, String)>,
    pub(super) macros: Vec<String>,
}

#[derive(Clone)]
pub(super) struct StyleSummary {
    pub(super) file: String,
    pub(super) node_id: String,
    pub(super) origin: SourceOrigin,
    pub(super) theme_name: Option<String>,
}

#[derive(Clone)]
pub(super) struct AssetSummary {
    pub(super) file: String,
    pub(super) node_id: String,
    pub(super) origin: SourceOrigin,
    pub(super) theme_name: Option<String>,
    pub(super) logical_path: String,
    pub(super) is_script: bool,
}

#[derive(Clone)]
pub(super) struct DataFileSummary {
    pub(super) file: String,
    pub(super) node_id: String,
    pub(super) origin: SourceOrigin,
    pub(super) theme_name: Option<String>,
    pub(super) logical_path: String,
}

#[derive(Clone)]
pub(super) struct TeraScopeSummary {
    pub(super) node_id: String,
    pub(super) kind: SourceNodeKind,
    pub(super) start: usize,
    pub(super) end: usize,
}
