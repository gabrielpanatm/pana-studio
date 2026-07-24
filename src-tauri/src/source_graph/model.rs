use serde::{Deserialize, Serialize};

use crate::source_graph::tera_semantics::{TeraSemanticDocument, TeraSemanticExpression};
use crate::source_graph::zola_shortcode::ZolaShortcodeInvocation;

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
    pub structured_documents: Vec<SourceStructuredDocument>,
    pub component_graph: ComponentGraph,
    pub block_graph: BlockGraph,
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
    pub frontmatter_format: Option<SourceDataFormat>,
    pub frontmatter_parse_error: Option<String>,
    pub frontmatter_nodes: Vec<SourceDataNode>,
    pub shortcode_parse_error: Option<String>,
    pub shortcodes: Vec<ZolaShortcodeInvocation>,
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
    pub include_groups: Vec<SourceGraphInclude>,
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
    pub semantics: Option<TeraSemanticDocument>,
    pub node_id: String,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentGraph {
    pub schema_version: u32,
    pub definitions: Vec<ComponentDefinition>,
    pub invocations: Vec<ComponentInvocation>,
    pub rendered_instances: Vec<RenderedComponentInstance>,
    pub diagnostics: Vec<ComponentDiagnostic>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockGraph {
    pub schema_version: u32,
    pub definitions: Vec<BlockDefinition>,
    pub source_instances: Vec<BlockSourceInstance>,
    pub diagnostics: Vec<BlockDiagnostic>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BlockOrigin {
    Native,
    Application,
    Theme,
    Project,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BlockScale {
    Element,
    Section,
    Composition,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BlockResolutionStatus {
    Resolved,
    UnknownProvider,
    InvalidContract,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockDefinition {
    pub id: String,
    pub schema_version: u32,
    pub provider_id: String,
    pub family_id: String,
    pub variant_id: String,
    pub display_name: String,
    pub description: String,
    pub origin: BlockOrigin,
    pub scale: BlockScale,
    pub capabilities: BlockCapabilities,
    pub requirements: Vec<BlockRequirement>,
    pub options: Vec<BlockOptionDefinition>,
    pub slots: Vec<BlockSlotDefinition>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BlockCapabilities {
    pub can_insert: bool,
    pub can_edit_properties: bool,
    pub supports_variants: bool,
    pub supports_slots: bool,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BlockRequirementKind {
    Runtime,
    Stylesheet,
    Markup,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BlockRequirement {
    pub id: String,
    pub kind: BlockRequirementKind,
    pub minimum_version: u32,
    pub required: bool,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BlockOptionControl {
    Toggle,
    Number,
    Text,
    Select,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum BlockOptionValue {
    Boolean(bool),
    Integer(i64),
    Text(String),
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BlockOptionChoice {
    pub value: String,
    pub label: String,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BlockOptionConstraints {
    pub minimum: Option<i64>,
    pub maximum: Option<i64>,
    pub step: Option<i64>,
    pub maximum_length: Option<usize>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BlockOptionDefinition {
    pub id: String,
    pub label: String,
    pub description: String,
    pub control: BlockOptionControl,
    pub attribute: String,
    pub default_value: BlockOptionValue,
    pub omit_when_default: bool,
    pub constraints: BlockOptionConstraints,
    pub choices: Vec<BlockOptionChoice>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BlockSlotDefinition {
    pub id: String,
    pub label: String,
    pub required: bool,
    pub multiple: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockSourceInstance {
    pub id: String,
    pub definition_id: Option<String>,
    pub provider_id: String,
    pub file: String,
    pub source_node_id: String,
    pub status: BlockResolutionStatus,
    pub diagnostics: Vec<BlockDiagnostic>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RenderedBlockInstance {
    pub id: String,
    pub definition_id: Option<String>,
    pub source_instance_id: Option<String>,
    pub render_instance_id: String,
    pub route: String,
    pub source_node_id: Option<String>,
    pub parent_instance_id: Option<String>,
    pub binding_key: Option<String>,
    pub binding_path: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BlockDiagnostic {
    pub code: String,
    pub message: String,
    pub severity: SourceDiagnosticSeverity,
    pub file: Option<String>,
    pub source_node_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ComponentDefinitionKind {
    TemplateFile,
    Partial,
    MacroLibrary,
    Macro,
    Shortcode,
    TemplateBlock,
    InlineRepeat,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ComponentInvocationKind {
    Include,
    MacroCall,
    Shortcode,
    Repeat,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ComponentOrigin {
    Project,
    Theme,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ComponentResolutionStatus {
    Resolved,
    FallbackResolved,
    Ambiguous,
    Dynamic,
    External,
    Unresolved,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ComponentDependencyKind {
    Template,
    Data,
    Content,
    Style,
    Script,
    Asset,
    Context,
    Runtime,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentDefinition {
    pub id: String,
    pub kind: ComponentDefinitionKind,
    pub name: String,
    pub display_name: String,
    pub origin: ComponentOrigin,
    pub theme_name: Option<String>,
    pub file: Option<String>,
    pub template_name: Option<String>,
    pub source_node_id: Option<String>,
    pub owner_definition_id: Option<String>,
    pub symbol: Option<String>,
    pub parameters: Vec<ComponentParameter>,
    pub context_dependencies: Vec<String>,
    pub data_bindings: Vec<ComponentDataBinding>,
    pub dependencies: Vec<ComponentDependency>,
    pub consumer_invocation_ids: Vec<String>,
    pub shadowed_by: Option<String>,
    pub active: bool,
    pub capabilities: ComponentCapabilities,
    pub diagnostics: Vec<ComponentDiagnostic>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentInvocation {
    pub id: String,
    pub kind: ComponentInvocationKind,
    pub name: String,
    pub file: String,
    pub source_node_id: Option<String>,
    pub owner_definition_id: Option<String>,
    pub parent_invocation_id: Option<String>,
    pub target_reference: String,
    pub resolved_definition_ids: Vec<String>,
    pub fallback_references: Vec<String>,
    pub arguments: Vec<ComponentArgument>,
    pub context_dependencies: Vec<String>,
    pub data_bindings: Vec<ComponentDataBinding>,
    pub status: ComponentResolutionStatus,
    pub diagnostics: Vec<ComponentDiagnostic>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RenderedComponentInstance {
    pub id: String,
    pub definition_id: Option<String>,
    pub invocation_id: Option<String>,
    pub render_instance_id: String,
    pub route: String,
    pub source_node_id: Option<String>,
    pub parent_instance_id: Option<String>,
    pub template_stack: Vec<String>,
    pub scope_path: Vec<String>,
    pub binding_key: Option<String>,
    pub binding_path: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentParameter {
    pub name: String,
    pub required: bool,
    pub default_value: Option<TeraSemanticExpression>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentArgument {
    pub name: String,
    pub expression: TeraSemanticExpression,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ComponentDataBinding {
    pub name: String,
    pub path: String,
    pub producer: String,
    pub source_node_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ComponentDependency {
    pub kind: ComponentDependencyKind,
    pub reference: String,
    pub source_node_id: Option<String>,
    pub target_node_id: Option<String>,
    pub resolved: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentCapabilities {
    pub can_create: bool,
    pub can_edit: bool,
    pub can_duplicate: bool,
    pub can_move: bool,
    pub can_rename: bool,
    pub can_extract: bool,
    pub can_delete: bool,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ComponentDiagnostic {
    pub code: String,
    pub message: String,
    pub severity: SourceDiagnosticSeverity,
    pub file: Option<String>,
    pub source_node_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceGraphInclude {
    pub targets: Vec<String>,
    pub ignore_missing: bool,
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
    pub format: SourceDataFormat,
    pub parse_error: Option<String>,
    pub nodes: Vec<SourceDataNode>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceStructuredDocument {
    pub id: String,
    pub file: String,
    pub kind: SourceStructuredDocumentKind,
    pub node_id: String,
    pub parse_error: Option<String>,
    pub nodes: Vec<SourceDataNode>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SourceStructuredDocumentKind {
    ZolaConfig,
    ThemeConfig,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SourceDataFormat {
    Toml,
    Json,
    Yaml,
    Csv,
    Bibtex,
    Xml,
    Unknown,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum SourceDataNodeKind {
    Document,
    Table,
    ArrayOfTables,
    TableElement,
    Array,
    ArrayElement,
    InlineTable,
    Value,
    Comment,
    Opaque,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum SourceDataPathSegment {
    Key(String),
    Index(usize),
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SourceDataValueKind {
    String,
    Integer,
    Float,
    Boolean,
    Datetime,
    Array,
    InlineTable,
    Table,
    ArrayOfTables,
    Null,
    Unknown,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceDataNode {
    pub id: String,
    pub kind: SourceDataNodeKind,
    pub path: Vec<SourceDataPathSegment>,
    pub key: Option<String>,
    pub value_kind: Option<SourceDataValueKind>,
    pub value_preview: Option<String>,
    pub range: Option<SourceRange>,
    pub key_range: Option<SourceRange>,
    pub parent_id: Option<String>,
    pub children: Vec<String>,
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

#[derive(Clone, Debug, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum SourceNodeKind {
    Page,
    Template,
    Partial,
    Style,
    Script,
    Asset,
    DataFile,
    DataTable,
    DataArray,
    DataValue,
    DataComment,
    ConfigFile,
    Html,
    BlockMarker,
    MacroCall,
    FunctionCall,
    Shortcode,
    Extends,
    Block,
    Include,
    Import,
    Macro,
    For,
    If,
    Elif,
    Else,
    Set,
    SetGlobal,
    Filter,
    Break,
    Continue,
    Super,
    TeraVariable,
    TeraComment,
    Raw,
    Tera,
}

#[derive(Clone, Debug, Serialize)]
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

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SourceDiagnosticSeverity {
    Warning,
    Error,
}
