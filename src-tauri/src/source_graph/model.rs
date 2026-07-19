use serde::Serialize;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceGraph {
    pub project_root: String,
    pub zola_root: String,
    pub active_theme: Option<String>,
    pub pages: Vec<SourceGraphPage>,
    pub templates: Vec<SourceGraphTemplate>,
    pub styles: Vec<SourceGraphStyle>,
    pub scripts: Vec<SourceGraphScript>,
    pub assets: Vec<SourceGraphAsset>,
    pub data_files: Vec<SourceGraphDataFile>,
    pub nodes: Vec<SourceNode>,
    pub relations: Vec<SourceRelation>,
    pub diagnostics: Vec<SourceGraphDiagnostic>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceGraphPage {
    pub id: String,
    pub file: String,
    pub title: String,
    pub url: String,
    pub page_kind: SourcePageKind,
    pub frontmatter_template: Option<String>,
    pub frontmatter_page_template: Option<String>,
    pub resolved_template: Option<String>,
    pub content_node_id: String,
    pub template_node_id: Option<String>,
    pub page_template_node_id: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SourcePageKind {
    Page,
    Section,
    Home,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceGraphTemplate {
    pub id: String,
    pub file: String,
    pub name: String,
    pub origin: SourceOrigin,
    pub theme_name: Option<String>,
    pub is_partial: bool,
    pub extends: Option<String>,
    pub includes: Vec<String>,
    pub imports: Vec<String>,
    pub get_pages: Vec<String>,
    pub get_sections: Vec<String>,
    pub internal_links: Vec<String>,
    pub asset_urls: Vec<String>,
    pub asset_hashes: Vec<String>,
    pub data_loads: Vec<String>,
    pub image_metadata: Vec<String>,
    pub image_resizes: Vec<String>,
    pub blocks: Vec<String>,
    pub macros: Vec<String>,
    pub node_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceGraphStyle {
    pub id: String,
    pub file: String,
    pub origin: SourceOrigin,
    pub theme_name: Option<String>,
    pub scope: SourceStyleScope,
    pub node_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceGraphScript {
    pub id: String,
    pub file: String,
    pub origin: SourceOrigin,
    pub theme_name: Option<String>,
    pub logical_path: String,
    pub node_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceGraphAsset {
    pub id: String,
    pub file: String,
    pub origin: SourceOrigin,
    pub theme_name: Option<String>,
    pub logical_path: String,
    pub node_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceGraphDataFile {
    pub id: String,
    pub file: String,
    pub origin: SourceOrigin,
    pub theme_name: Option<String>,
    pub logical_path: String,
    pub node_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceStyleScope {
    Global,
    Page,
    Partial,
    Other,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SourceOrigin {
    Local,
    Theme,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceNode {
    pub id: String,
    pub kind: SourceNodeKind,
    pub file: String,
    pub origin: SourceOrigin,
    pub theme_name: Option<String>,
    pub label: String,
    pub range: Option<SourceRange>,
    pub parent: Option<String>,
    pub children: Vec<String>,
    pub capabilities: SourceCapabilities,
}

#[derive(Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SourceNodeKind {
    Page,
    Template,
    Partial,
    Style,
    Script,
    Asset,
    DataFile,
    Html,
    Extends,
    Block,
    Include,
    Import,
    Macro,
    For,
    If,
    Set,
    With,
    TeraVariable,
    TeraComment,
    Raw,
    Tera,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRange {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceCapabilities {
    pub can_open_in_code: bool,
    pub can_edit_visual: bool,
    pub can_edit_text: bool,
    pub can_edit_attributes: bool,
    pub can_move: bool,
    pub can_extract_partial: bool,
    pub reason: Option<String>,
}

impl SourceCapabilities {
    pub fn code_only(reason: impl Into<String>) -> Self {
        Self {
            can_open_in_code: true,
            can_edit_visual: false,
            can_edit_text: false,
            can_edit_attributes: false,
            can_move: false,
            can_extract_partial: false,
            reason: Some(reason.into()),
        }
    }

    pub fn visual_html() -> Self {
        Self {
            can_open_in_code: true,
            can_edit_visual: true,
            can_edit_text: true,
            can_edit_attributes: true,
            can_move: true,
            can_extract_partial: true,
            reason: None,
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRelation {
    pub id: String,
    pub from: String,
    pub to: String,
    pub kind: SourceRelationKind,
    pub label: String,
}

#[derive(Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SourceRelationKind {
    PageTemplate,
    SectionPageTemplate,
    GetsPage,
    GetsSection,
    InternalContentLink,
    AssetUrl,
    AssetHash,
    DataLoad,
    DataFileLoad,
    ContentDataLoad,
    ImageMetadata,
    ImageResize,
    Extends,
    Includes,
    Imports,
    DefinesBlock,
    OverridesBlock,
    UsesStyle,
    UsesScript,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceGraphDiagnostic {
    pub severity: SourceDiagnosticSeverity,
    pub message: String,
    pub file: Option<String>,
    pub range: Option<SourceRange>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceDiagnosticSeverity {
    Warning,
    Error,
}
