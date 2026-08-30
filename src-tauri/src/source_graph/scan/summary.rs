use crate::source_graph::model::{
    MarkdownProjection, SourceCapabilities, SourceDataFormat, SourceDataLocation, SourceDataNode,
    SourceGraphInclude, SourceNodeKind, SourceOrigin,
};
use crate::source_graph::tera_semantics::{
    TeraComponentCall, TeraComponentDefinition, TeraSemanticDocument,
};

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
    pub(super) include_groups: Vec<SourceGraphInclude>,
    pub(super) get_pages: Vec<String>,
    pub(super) get_sections: Vec<String>,
    pub(super) internal_links: Vec<String>,
    pub(super) asset_urls: Vec<String>,
    pub(super) asset_hashes: Vec<String>,
    pub(super) literal_asset_references: Vec<String>,
    pub(super) asset_reference_eligible: usize,
    pub(super) asset_reference_unanalysable: usize,
    pub(super) data_loads: Vec<String>,
    pub(super) image_metadata: Vec<String>,
    pub(super) image_resizes: Vec<String>,
    pub(super) blocks: Vec<(String, String)>,
    pub(super) component_definitions: Vec<TeraComponentDefinition>,
    pub(super) component_calls: Vec<TeraComponentCall>,
    pub(super) semantics: Option<TeraSemanticDocument>,
    pub(super) markdown_projections: Vec<MarkdownProjection>,
}

#[derive(Clone)]
pub(super) struct StyleSummary {
    pub(super) file: String,
    pub(super) node_id: String,
    pub(super) origin: SourceOrigin,
    pub(super) theme_name: Option<String>,
    pub(super) literal_asset_references: Vec<String>,
    pub(super) asset_reference_eligible: usize,
    pub(super) asset_reference_unanalysable: usize,
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
    pub(super) load_paths: Vec<String>,
    pub(super) location: SourceDataLocation,
    pub(super) format: SourceDataFormat,
    pub(super) parse_error: Option<String>,
    pub(super) nodes: Vec<SourceDataNode>,
    pub(super) capabilities: SourceCapabilities,
}

#[derive(Clone)]
pub(super) struct TeraScopeSummary {
    pub(super) node_id: String,
    pub(super) kind: SourceNodeKind,
    pub(super) start: usize,
    pub(super) end: usize,
}
