use std::{
    collections::{BTreeMap, HashMap},
    sync::OnceLock,
};

use serde::{Deserialize, Serialize};

use crate::kernel::content_models::ContentModelCatalog;
use crate::kernel::dynamic_widgets::DynamicWidgetGraph;
use crate::kernel::listing_items::ListingItemCatalog;
use crate::localization::LocalizedDiagnostic;
use crate::source_graph::tera_semantics::{
    TeraComponentCall, TeraComponentDefinition, TeraSemanticDocument, TeraSemanticExpression,
};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceGraph {
    /// Runtime-only O(1) authority index. It is derived from `nodes`, never
    /// serialized, and reset whenever reconciliation changes node identities.
    #[serde(skip)]
    pub(crate) node_index: OnceLock<HashMap<String, usize>>,
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
    pub content_models: ContentModelCatalog,
    pub listing_items: ListingItemCatalog,
    pub dynamic_widget_graph: DynamicWidgetGraph,
    pub markdown_projections: Vec<MarkdownProjection>,
    pub nodes: Vec<SourceNode>,
    pub relations: Vec<SourceRelation>,
    pub asset_reference_coverage: SourceAssetReferenceCoverage,
    pub diagnostics: Vec<SourceGraphDiagnostic>,
}

impl SourceGraph {
    pub fn node_by_id(&self, source_node_id: &str) -> Option<&SourceNode> {
        let index = self.node_index.get_or_init(|| {
            self.nodes
                .iter()
                .enumerate()
                .map(|(index, node)| (node.id.clone(), index))
                .collect()
        });
        index
            .get(source_node_id)
            .and_then(|index| self.nodes.get(*index))
            .filter(|node| node.id == source_node_id)
    }

    pub(crate) fn rebuild_node_index(&mut self) -> Result<(), String> {
        let mut index = HashMap::with_capacity(self.nodes.len());
        for (position, node) in self.nodes.iter().enumerate() {
            if index.insert(node.id.clone(), position).is_some() {
                return Err(format!(
                    "SourceGraph a refuzat SourceNodeId duplicat: {}.",
                    node.id
                ));
            }
        }
        self.node_index = OnceLock::new();
        let _ = self.node_index.set(index);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MarkdownProjectionKind {
    Body,
    Summary,
    Filter,
    Toc,
}

impl MarkdownProjectionKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Body => "Conținut Markdown",
            Self::Summary => "Rezumat Markdown",
            Self::Filter => "Filtru Markdown",
            Self::Toc => "Cuprins Markdown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MarkdownSourceBindingKind {
    CurrentPage,
    CurrentSection,
    StaticPage,
    StaticSection,
    RuntimePage,
    RuntimeSection,
    Unresolved,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkdownProjection {
    pub id: String,
    pub kind: MarkdownProjectionKind,
    pub template_source_node_id: String,
    pub template_file: String,
    pub template_range: Option<SourceRange>,
    pub binding_kind: MarkdownSourceBindingKind,
    pub static_content_path: Option<String>,
    pub runtime_source_expression: Option<String>,
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
    pub taxonomies: BTreeMap<String, Vec<String>>,
    pub component_calls: Vec<TeraComponentCall>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
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
    pub get_pages: Vec<String>,
    pub get_sections: Vec<String>,
    pub internal_links: Vec<String>,
    pub asset_urls: Vec<String>,
    pub asset_hashes: Vec<String>,
    pub literal_asset_references: Vec<String>,
    pub asset_reference_eligible: usize,
    pub asset_reference_unanalysable: usize,
    pub data_loads: Vec<String>,
    pub image_metadata: Vec<String>,
    pub image_resizes: Vec<String>,
    pub blocks: Vec<String>,
    pub component_definitions: Vec<TeraComponentDefinition>,
    pub component_calls: Vec<TeraComponentCall>,
    pub semantics: Option<TeraSemanticDocument>,
    pub markdown_projections: Vec<MarkdownProjection>,
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
    /// Piesă atomică, fără structură internă compusă administrată ca bloc.
    Element,
    /// Ansamblu page-level care reprezintă o zonă completă de pagină.
    Section,
    /// Ansamblu autonom de elemente care poate fi integrat într-un container.
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
    pub item_kind: String,
    pub minimum_items: usize,
    pub maximum_items: Option<usize>,
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
    pub diagnostic: LocalizedDiagnostic,
    pub severity: SourceDiagnosticSeverity,
    pub file: Option<String>,
    pub source_node_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ComponentDefinitionKind {
    TemplateFile,
    Partial,
    TeraComponent,
    TemplateBlock,
    InlineRepeat,
    InlineConditional,
    InlineTransform,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ComponentInvocationKind {
    Include,
    TeraComponent,
    Repeat,
    Conditional,
    Transform,
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
    pub range: Option<SourceRange>,
    pub body_range: Option<SourceRange>,
    pub rest_parameter: Option<String>,
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
    pub range: Option<SourceRange>,
    pub call_range: Option<SourceRange>,
    pub body_range: Option<SourceRange>,
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
    pub argument_type: Option<String>,
    pub required: bool,
    pub rest: bool,
    pub default_value: Option<TeraSemanticExpression>,
    pub range: Option<SourceRange>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentArgument {
    pub name: Option<String>,
    pub expression: TeraSemanticExpression,
    pub spread: bool,
    pub range: Option<SourceRange>,
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
    pub reason_diagnostic: Option<LocalizedDiagnostic>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ComponentDiagnostic {
    pub code: String,
    pub diagnostic: LocalizedDiagnostic,
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
    pub load_paths: Vec<String>,
    pub location: SourceDataLocation,
    pub node_id: String,
    pub format: SourceDataFormat,
    pub parse_error: Option<String>,
    pub nodes: Vec<SourceDataNode>,
    pub capabilities: SourceCapabilities,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SourceDataLocation {
    Date,
    Project,
    Static,
    Content,
    Output,
    Theme,
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

impl SourceNode {
    /// Returns the first literal template target carried by a Tera relation
    /// node. The lossless SourceGraph label is the canonical projection for
    /// unresolved targets and ordered include fallbacks as well.
    pub(crate) fn tera_template_target(&self) -> Option<&str> {
        let prefix = match self.kind {
            SourceNodeKind::Extends => "extends ",
            SourceNodeKind::Include => "include ",
            _ => return None,
        };
        self.label.strip_prefix(prefix)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
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
    ComponentCall,
    FunctionCall,
    Extends,
    Block,
    Include,
    ComponentDefinition,
    LegacyTera,
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRange {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceCapabilityReason {
    StructuredConfig,
    StructuredDataNode,
    StyleFile,
    TeraTemplateFile,
    TeraExtends,
    TeraBlock,
    TeraInclude,
    TeraComponentDefinition,
    TeraFor,
    TeraIf,
    TeraElif,
    TeraElse,
    TeraSet,
    TeraSetGlobal,
    TeraFilter,
    TeraBreak,
    TeraContinue,
    TeraSuper,
    TeraVariable,
    TeraComponentCall,
    TeraFunctionCall,
    LegacyTeraSyntax,
    NativeBlockMarker,
    TeraComment,
    TeraRaw,
    TeraSyntax,
    HtmlInTeraLoop,
    HtmlInTeraCondition,
    HtmlInTeraComponent,
    HtmlInTeraLocalScope,
    HtmlInTeraRaw,
    MarkdownPage,
    MarkdownRenderedBoundary,
    MarkdownSourceUnresolved,
    StaticJavaScript,
    StaticAsset,
    DataOutputReadOnly,
    DataThemeReadOnly,
    DataFormatVisualUnsupported,
}

impl SourceCapabilityReason {
    pub fn technical_message(self) -> &'static str {
        match self {
            Self::StructuredConfig => {
                "Lossless structured Zola configuration is mutated and validated by Rust."
            }
            Self::StructuredDataNode => {
                "Structured data node mutations are planned losslessly by Rust."
            }
            Self::StyleFile => "Style file.",
            Self::TeraTemplateFile => "Tera template file.",
            Self::TeraExtends => "Tera inheritance directive.",
            Self::TeraBlock => "Tera block.",
            Self::TeraInclude => "Tera include.",
            Self::TeraComponentDefinition => "Tera component definition.",
            Self::TeraFor => "Tera loop.",
            Self::TeraIf => "Tera condition.",
            Self::TeraElif => "Tera elif branch.",
            Self::TeraElse => "Tera else branch.",
            Self::TeraSet => "Tera assignment.",
            Self::TeraSetGlobal => "Tera global assignment.",
            Self::TeraFilter => "Tera filter block.",
            Self::TeraBreak => "Tera break.",
            Self::TeraContinue => "Tera continue.",
            Self::TeraSuper => "Tera super() call.",
            Self::TeraVariable => "Tera variable.",
            Self::TeraComponentCall => "Tera component call.",
            Self::TeraFunctionCall => "Tera or Zola function call.",
            Self::LegacyTeraSyntax => "Syntax removed by Tera 2/Zola 0.23.",
            Self::NativeBlockMarker => "Marker supplied by a native Rust block provider.",
            Self::TeraComment => "Tera comment.",
            Self::TeraRaw => "Tera raw block.",
            Self::TeraSyntax => "Tera syntax.",
            Self::HtmlInTeraLoop => {
                "The element is rendered in a Tera loop; direct visual editing is unsafe."
            }
            Self::HtmlInTeraCondition => {
                "The element is rendered conditionally by Tera; direct visual editing is unsafe."
            }
            Self::HtmlInTeraComponent => {
                "The element is defined in a Tera component; changing it may affect multiple uses."
            }
            Self::HtmlInTeraLocalScope => {
                "The element is in a local Tera scope and must be edited in code."
            }
            Self::HtmlInTeraRaw => {
                "The element is in a Tera raw block; visual editing is disabled."
            }
            Self::MarkdownPage => "Zola Markdown page.",
            Self::MarkdownRenderedBoundary => {
                "Rendered Markdown is edited only through its Markdown source."
            }
            Self::MarkdownSourceUnresolved => {
                "Rendered Markdown provenance is unresolved and remains read-only."
            }
            Self::StaticJavaScript => "Static Zola JavaScript file.",
            Self::StaticAsset => "Static Zola asset.",
            Self::DataOutputReadOnly => {
                "The file is generated in Zola's output directory and is read-only."
            }
            Self::DataThemeReadOnly => {
                "The file is supplied by the active theme and is read-only."
            }
            Self::DataFormatVisualUnsupported => {
                "The format is indexed semantically, but lossless editing currently supports TOML only."
            }
        }
    }
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
    pub reason_code: Option<SourceCapabilityReason>,
}

impl SourceCapabilities {
    pub fn code_only(reason_code: SourceCapabilityReason) -> Self {
        Self {
            can_open_in_code: true,
            can_edit_visual: false,
            can_edit_text: false,
            can_edit_attributes: false,
            can_move: false,
            can_extract_partial: false,
            reason_code: Some(reason_code),
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
            reason_code: None,
        }
    }

    pub fn technical_reason(&self) -> Option<&'static str> {
        self.reason_code
            .map(SourceCapabilityReason::technical_message)
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
    AssetReference,
    DataLoad,
    DataFileLoad,
    ContentDataLoad,
    ImageMetadata,
    ImageResize,
    Extends,
    Includes,
    DefinesBlock,
    OverridesBlock,
    UsesStyle,
    UsesScript,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceAssetReferenceCoverage {
    pub eligible: usize,
    pub analyzed: usize,
    pub unanalysable: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceGraphDiagnostic {
    pub severity: SourceDiagnosticSeverity,
    pub diagnostic: LocalizedDiagnostic,
    pub file: Option<String>,
    pub range: Option<SourceRange>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SourceDiagnosticSeverity {
    Warning,
    Error,
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::{
        SourceCapabilities, SourceCapabilityReason, SourceGraph, SourceNode, SourceNodeKind,
        SourceOrigin,
    };

    #[test]
    fn source_capabilities_serialize_semantic_reason_codes_without_user_prose() {
        let value = serde_json::to_value(SourceCapabilities::code_only(
            SourceCapabilityReason::HtmlInTeraLoop,
        ))
        .expect("source capabilities should serialize");

        assert_eq!(value["reasonCode"], "htmlInTeraLoop");
        assert!(value.get("reason").is_none());
        assert_eq!(value["canOpenInCode"], true);
        assert_eq!(value["canEditVisual"], false);
    }

    #[test]
    fn visual_html_capabilities_have_no_restriction_reason() {
        let capabilities = SourceCapabilities::visual_html();
        let value =
            serde_json::to_value(&capabilities).expect("source capabilities should serialize");

        assert_eq!(value["reasonCode"], serde_json::Value::Null);
        assert!(capabilities.technical_reason().is_none());
    }

    #[test]
    fn tera_template_target_preserves_literal_unresolved_and_fallback_head_targets() {
        let mut node = graph_with_nodes(1).nodes.remove(0);
        for (kind, label, expected) in [
            (
                SourceNodeKind::Extends,
                "extends base.html",
                Some("base.html"),
            ),
            (
                SourceNodeKind::Include,
                "include partials/missing.html",
                Some("partials/missing.html"),
            ),
        ] {
            node.kind = kind;
            node.label = label.to_string();
            assert_eq!(node.tera_template_target(), expected);
        }

        node.kind = SourceNodeKind::Include;
        node.label = "include".to_string();
        assert_eq!(node.tera_template_target(), None);
        node.kind = SourceNodeKind::Html;
        node.label = "include partials/card.html".to_string();
        assert_eq!(node.tera_template_target(), None);
    }

    #[test]
    #[ignore = "release-only warm latency budget"]
    fn source_node_index_warm_p95_is_below_one_millisecond_for_1k_and_10k_nodes() {
        for node_count in [1_000usize, 10_000] {
            let mut graph = graph_with_nodes(node_count);
            graph.rebuild_node_index().unwrap();
            assert!(graph.node_by_id("node-0").is_some());
            let mut samples = Vec::with_capacity(20_000);
            for sample in 0..20_000usize {
                let id = format!("node-{}", sample.wrapping_mul(7_919) % node_count);
                let started = Instant::now();
                assert_eq!(
                    graph.node_by_id(&id).map(|node| node.id.as_str()),
                    Some(id.as_str())
                );
                samples.push(started.elapsed().as_nanos());
            }
            samples.sort_unstable();
            let percentile = |value: usize| samples[(samples.len() - 1) * value / 100];
            eprintln!(
                "source_node_index nodes={node_count} p50_ns={} p95_ns={} p99_ns={}",
                percentile(50),
                percentile(95),
                percentile(99),
            );
            assert!(
                percentile(95) < 1_000_000,
                "target resolution p95 exceeded 1 ms for {node_count} nodes"
            );
        }
    }

    fn graph_with_nodes(count: usize) -> SourceGraph {
        SourceGraph {
            node_index: Default::default(),
            project_root: "/project".to_string(),
            zola_root: "/project".to_string(),
            active_theme: None,
            pages: Vec::new(),
            templates: Vec::new(),
            styles: Vec::new(),
            scripts: Vec::new(),
            assets: Vec::new(),
            data_files: Vec::new(),
            structured_documents: Vec::new(),
            component_graph: Default::default(),
            block_graph: Default::default(),
            content_models: Default::default(),
            listing_items: Default::default(),
            dynamic_widget_graph: Default::default(),
            markdown_projections: Vec::new(),
            nodes: (0..count)
                .map(|index| SourceNode {
                    id: format!("node-{index}"),
                    kind: SourceNodeKind::Html,
                    file: "templates/index.html".to_string(),
                    origin: SourceOrigin::Local,
                    theme_name: None,
                    label: "<div>".to_string(),
                    range: None,
                    parent: None,
                    children: Vec::new(),
                    capabilities: SourceCapabilities::visual_html(),
                })
                .collect(),
            relations: Vec::new(),
            asset_reference_coverage: Default::default(),
            diagnostics: Vec::new(),
        }
    }
}
