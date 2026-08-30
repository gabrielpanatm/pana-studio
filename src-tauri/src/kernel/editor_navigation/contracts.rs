use super::*;

pub const EDITOR_NAVIGATION_SCHEMA_VERSION: u32 = 4;
pub const EDIT_SCOPE_GRANT_SCHEMA_VERSION: u32 = 2;
pub const EDITOR_MOVE_PLAN_SCHEMA_VERSION: u32 = 3;
pub const EDITOR_MOVE_EXECUTION_SCHEMA_VERSION: u32 = 3;
pub const EDITOR_MOVE_LIVE_PROJECTION_SCHEMA_VERSION: u32 = 1;
pub(super) const MAX_LIVE_EDIT_SCOPE_GRANTS: usize = 64;
pub(super) const MAX_LIVE_EDITOR_MOVE_PLANS: usize = 128;
pub(super) const MAX_CACHED_EDITOR_NAVIGATION_SNAPSHOTS: usize = 8;

#[derive(Clone, Copy, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum EditorNavigationSurface {
    CanonicalPreview,
    TemplateWorkbench,
}

#[derive(Clone, Copy, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum EditorNavigationNodeKind {
    HtmlElement,
    Boundary,
    RuntimeElement,
}

#[derive(Clone, Copy, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum EditorNavigationBoundaryKind {
    Template,
    Component,
    Markdown,
}

#[derive(Clone, Copy, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum EditorNavigationComponentKind {
    Partial,
    TeraComponent,
    Repeat,
    Conditional,
    Transform,
}

#[derive(Clone, Copy, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum EditorNavigationViewNodeKind {
    HtmlElement,
    Boundary,
    Relation,
    Slot,
    Source,
}

#[derive(Clone, Copy, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum EditorNavigationRelationKind {
    Extends,
    Include,
    BlockOverride,
}

#[derive(Clone, Copy, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum EditorNavigationOrigin {
    Project,
    Theme,
    Tera,
    PanaRuntime,
    ArbitraryRuntime,
}

#[derive(Clone, Copy, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum EditorNavigationEffectScope {
    SingleSource,
    SharedDefinition,
    AllRenderedInstances,
}

#[derive(Clone, Copy, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum EditorSourceResolution {
    Direct,
    Resolved,
    FallbackResolved,
    Ambiguous,
    Dynamic,
    External,
    Unresolved,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EditorSourceReference {
    pub source_node_id: Option<String>,
    pub source_kind: Option<SourceNodeKind>,
    pub file: String,
    pub range: Option<SourceRange>,
    pub label: String,
    pub origin: EditorNavigationOrigin,
    pub theme_name: Option<String>,
    pub can_open_in_code: bool,
}

/// Proveniența semantică a unei ținte editabile.
///
/// `definition` este sursa conținutului selectat. `composition` este locul
/// în care acea definiție a fost invocată. Pentru o sursă directă,
/// `composition` lipsește; pentru un include nerezolvat, `definition`
/// lipsește și call-site-ul rămâne disponibil în `composition`.
#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EditorSourceProvenance {
    pub definition: Option<EditorSourceReference>,
    pub composition: Option<EditorSourceReference>,
    pub resolution: EditorSourceResolution,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorNavigationCapabilities {
    pub can_select: bool,
    pub can_inspect: bool,
    pub can_open_in_code: bool,
    pub can_enter_boundary: bool,
    pub can_move_atomic: bool,
    pub can_move: bool,
    pub can_edit_text: bool,
    pub can_edit_attributes: bool,
    pub read_only: bool,
    pub requires_edit_scope_id: Option<String>,
    pub reason_code: Option<SourceCapabilityReason>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorNavigationBoundary {
    pub kind: EditorNavigationBoundaryKind,
    pub component_kind: Option<EditorNavigationComponentKind>,
    pub boundary_instance_id: String,
    pub source_node_id: String,
    pub root_render_instance_ids: Vec<String>,
    pub atomic_when_closed: bool,
    pub effect_scope: EditorNavigationEffectScope,
    pub rendered_instance_count: usize,
    pub target: Option<String>,
    pub empty: bool,
}

pub(super) fn editor_boundary_classification(
    model: &ProjectModel,
    source: Option<&SourceNode>,
    markdown: bool,
) -> (
    EditorNavigationBoundaryKind,
    Option<EditorNavigationComponentKind>,
) {
    if markdown {
        return (EditorNavigationBoundaryKind::Markdown, None);
    }
    let component_kind = source.and_then(|source| editor_component_kind(model, source));
    if component_kind.is_some() {
        (EditorNavigationBoundaryKind::Component, component_kind)
    } else {
        (EditorNavigationBoundaryKind::Template, None)
    }
}

fn editor_component_kind(
    model: &ProjectModel,
    source: &SourceNode,
) -> Option<EditorNavigationComponentKind> {
    let graph = &model.source_graph.component_graph;
    let invocation_kind = graph
        .invocations
        .iter()
        .find(|invocation| invocation.source_node_id.as_deref() == Some(source.id.as_str()))
        .map(|invocation| &invocation.kind);
    match invocation_kind {
        Some(ComponentInvocationKind::Include) => {
            return Some(EditorNavigationComponentKind::Partial)
        }
        Some(ComponentInvocationKind::TeraComponent) => {
            return Some(EditorNavigationComponentKind::TeraComponent)
        }
        Some(ComponentInvocationKind::Repeat) => {
            return Some(EditorNavigationComponentKind::Repeat)
        }
        Some(ComponentInvocationKind::Conditional) => {
            return Some(EditorNavigationComponentKind::Conditional)
        }
        Some(ComponentInvocationKind::Transform) => {
            return Some(EditorNavigationComponentKind::Transform)
        }
        None => {}
    }
    let definition_kind = graph
        .definitions
        .iter()
        .find(|definition| definition.source_node_id.as_deref() == Some(source.id.as_str()))
        .map(|definition| &definition.kind);
    match definition_kind {
        Some(ComponentDefinitionKind::Partial) => Some(EditorNavigationComponentKind::Partial),
        Some(ComponentDefinitionKind::TeraComponent) => {
            Some(EditorNavigationComponentKind::TeraComponent)
        }
        Some(ComponentDefinitionKind::InlineRepeat) => Some(EditorNavigationComponentKind::Repeat),
        Some(ComponentDefinitionKind::InlineConditional) => {
            Some(EditorNavigationComponentKind::Conditional)
        }
        Some(ComponentDefinitionKind::InlineTransform) => {
            Some(EditorNavigationComponentKind::Transform)
        }
        _ => match source.kind {
            SourceNodeKind::Include => Some(EditorNavigationComponentKind::Partial),
            SourceNodeKind::ComponentDefinition | SourceNodeKind::ComponentCall => {
                Some(EditorNavigationComponentKind::TeraComponent)
            }
            SourceNodeKind::For => Some(EditorNavigationComponentKind::Repeat),
            SourceNodeKind::If => Some(EditorNavigationComponentKind::Conditional),
            SourceNodeKind::Filter => Some(EditorNavigationComponentKind::Transform),
            _ => None,
        },
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorNavigationRelation {
    pub kind: EditorNavigationRelationKind,
    pub target_document_path: Option<String>,
    pub target_source_node_id: Option<String>,
    pub target_template_name: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorNavigationNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub children: Vec<String>,
    pub order: usize,
    pub kind: EditorNavigationNodeKind,
    pub label: String,
    pub tag: Option<String>,
    pub source_node_id: Option<String>,
    pub render_instance_id: Option<String>,
    pub source_kind: Option<SourceNodeKind>,
    pub file: Option<String>,
    pub range: Option<SourceRange>,
    pub origin: EditorNavigationOrigin,
    pub theme_name: Option<String>,
    pub source_provenance: EditorSourceProvenance,
    pub provenance_stack: Vec<String>,
    pub component_definition_ids: Vec<String>,
    pub component_invocation_ids: Vec<String>,
    pub block_definition_ids: Vec<String>,
    pub block_source_instance_ids: Vec<String>,
    pub dynamic_widget_provider_ids: Vec<String>,
    pub dynamic_widget_source_instance_ids: Vec<String>,
    pub binding_key: Option<String>,
    pub binding_path: Option<String>,
    pub boundary: Option<EditorNavigationBoundary>,
    pub capabilities: EditorNavigationCapabilities,
    /// Source-derived only; never crosses the command boundary. SelectionCoordinator
    /// uses it to build one bounded aggregate for Inspector multi-select.
    #[serde(skip)]
    pub(crate) source_html_attributes: Option<BTreeMap<String, Option<String>>>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorNavigationViewNode {
    pub id: String,
    pub editor_node_id: Option<String>,
    pub parent_id: Option<String>,
    pub children: Vec<String>,
    pub order: usize,
    pub kind: EditorNavigationViewNodeKind,
    pub label: String,
    pub tag: Option<String>,
    pub source_node_id: Option<String>,
    pub source_kind: Option<SourceNodeKind>,
    pub file: String,
    pub origin: EditorNavigationOrigin,
    pub theme_name: Option<String>,
    pub render_instance_ids: Vec<String>,
    pub boundary: Option<EditorNavigationBoundary>,
    pub relation: Option<EditorNavigationRelation>,
    pub capabilities: EditorNavigationCapabilities,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorNavigationBreadcrumb {
    pub document_path: String,
    pub template_name: String,
    pub source_node_id: String,
    pub origin: EditorNavigationOrigin,
    pub theme_name: Option<String>,
    pub current: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorNavigationView {
    pub active_document_path: String,
    pub active_template_name: String,
    pub active_source_node_id: String,
    pub breadcrumbs: Vec<EditorNavigationBreadcrumb>,
    pub root_node_ids: Vec<String>,
    pub nodes: Vec<EditorNavigationViewNode>,
    pub preview_context_render_instance_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorNavigationDiagnostic {
    pub code: String,
    pub message: String,
    pub source_node_id: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorNavigationSnapshot {
    pub schema_version: u32,
    pub identity: CanvasProjectionIdentity,
    pub model_revision: String,
    pub route: String,
    pub surface: EditorNavigationSurface,
    pub root_node_ids: Vec<String>,
    pub nodes: Vec<EditorNavigationNode>,
    pub focused_view: Option<EditorNavigationView>,
    pub diagnostics: Vec<EditorNavigationDiagnostic>,
    #[serde(skip)]
    pub(super) planning_nodes: Vec<EditorNavigationNode>,
    #[serde(skip)]
    pub(super) node_index: HashMap<String, usize>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum EditScopeOperation {
    MoveHtmlInside,
    EditTextInside,
    EditAttributesInside,
    InspectSharedDefinition,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EditScopeGrant {
    pub schema_version: u32,
    pub token: String,
    pub scope_id: String,
    pub boundary_instance_id: String,
    pub source_node_id: String,
    pub project_root: String,
    pub runtime_session_id: String,
    pub workspace_revision: u64,
    pub model_revision: String,
    pub preview_revision: String,
    pub canvas_transaction_id: String,
    pub route: String,
    pub active_document_path: String,
    pub operations: Vec<EditScopeOperation>,
    pub issued_at_ms: u128,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum EditorMoveOperation {
    HtmlSourceMove,
    AtomicTeraMove,
    ComponentMove,
    BlockMove,
}

#[derive(Clone, Copy, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum EditorMoveLiveProjectionOperation {
    Move,
}

#[derive(Clone, Copy, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum EditorMoveLiveProjectionScope {
    SelectedInstance,
}

#[derive(Clone, Copy, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum EditorMoveLiveProjectionReason {
    Ready,
    PlanBlocked,
    ExecutionNotHtml,
    MissingRenderIdentity,
    AmbiguousSourceIdentity,
    MultipleRenderedInstances,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EditorMoveLiveProjectionRollback {
    pub source_parent_render_instance_id: Option<String>,
    pub source_next_sibling_render_instance_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EditorMoveLiveProjection {
    pub schema_version: u32,
    pub operation: EditorMoveLiveProjectionOperation,
    pub scope: EditorMoveLiveProjectionScope,
    pub plan_token: Option<String>,
    pub identity: CanvasProjectionIdentity,
    pub source_render_instance_id: String,
    pub target_render_instance_id: String,
    pub position: ProjectMovePosition,
    pub rollback: EditorMoveLiveProjectionRollback,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorMoveImpact {
    pub files: Vec<String>,
    pub edit_scope_id: Option<String>,
    pub effect_scope: EditorNavigationEffectScope,
    pub rendered_instance_count: usize,
    pub affects_all_rendered_instances: bool,
    pub requires_preview_reprojection: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorMovePlan {
    pub schema_version: u32,
    pub token: Option<String>,
    pub allowed: bool,
    pub reason_code: Option<String>,
    pub reason: Option<String>,
    pub operation: Option<EditorMoveOperation>,
    pub identity: CanvasProjectionIdentity,
    pub model_revision: String,
    pub route: String,
    pub active_document_path: String,
    pub source_node_id: String,
    pub target_node_id: String,
    pub position: ProjectMovePosition,
    pub impact: EditorMoveImpact,
    pub live_projection: Option<EditorMoveLiveProjection>,
    pub live_projection_reason: EditorMoveLiveProjectionReason,
    pub issued_at_ms: u128,
}

#[derive(Clone, Copy, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EditorMoveExecutionStatus {
    Committed,
    Blocked,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorMoveExecutionReceipt {
    pub schema_version: u32,
    pub plan_token: String,
    pub project_root: String,
    pub runtime_session_id: String,
    pub status: EditorMoveExecutionStatus,
    pub operation: EditorMoveOperation,
    pub model_revision: Option<String>,
    pub projected_source_id: Option<String>,
    pub canvas_patch: Option<CanvasPatch>,
    pub workspace_mutation: Option<ProjectWorkspaceMutationReceipt>,
    pub touched_files: Vec<String>,
    pub diagnostic: Option<String>,
    pub timings: Option<EditorMoveTimings>,
    #[serde(skip)]
    pub(crate) internal_timings: EditorMoveInternalTimings,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorMoveTimings {
    pub input_emitted_at_ms: u64,
    pub plan_issued_at_ms: u64,
    pub rust_received_at_ms: u64,
    pub rust_completed_at_ms: u64,
    pub input_to_receipt_ms: u64,
    pub pointer_up_to_commit_receipt_ms: u64,
    pub plan_to_receipt_ms: u64,
    pub rust_command_ms: u64,
    pub patch_issued_to_receipt_ms: Option<u64>,
    pub candidate_clone_ms: u64,
    pub mutation_ms: u64,
    pub recovery_persist_ms: u64,
    pub authority_publish_ms: u64,
    pub authority_transaction_ms: u64,
    pub plan_revalidation_ms: u64,
    pub native_block_contract_ms: u64,
    pub workspace_stage_ms: u64,
    pub after_project_model_build_ms: u64,
    pub project_model_build_mode: String,
    pub project_model_fallback_reason: Option<String>,
    pub project_model_changed_path_count: usize,
    pub project_model_invalidated_template_count: usize,
    pub project_model_invalidated_page_count: usize,
    pub project_model_replaced_nodes: usize,
    pub project_model_reused_nodes: usize,
    pub project_model_reused_relations: usize,
    pub project_model_clone_ms: u64,
    pub project_model_template_parse_us: u64,
    pub project_model_component_graph_us: u64,
    pub project_model_block_graph_us: u64,
    pub project_model_content_model_us: u64,
    pub project_model_listing_items_us: u64,
    pub project_model_listing_items_reused: bool,
    pub project_model_dynamic_widget_us: u64,
    pub project_model_markdown_us: u64,
    pub project_model_node_index_us: u64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct EditorMoveInternalTimings {
    pub native_block_contract_ms: u64,
    pub workspace_stage_ms: u64,
    pub after_project_model_build_ms: u64,
    pub project_model_build_mode: String,
    pub project_model_fallback_reason: Option<String>,
    pub project_model_changed_path_count: usize,
    pub project_model_invalidated_template_count: usize,
    pub project_model_invalidated_page_count: usize,
    pub project_model_replaced_nodes: usize,
    pub project_model_reused_nodes: usize,
    pub project_model_reused_relations: usize,
    pub project_model_clone_ms: u64,
    pub project_model_template_parse_us: u64,
    pub project_model_component_graph_us: u64,
    pub project_model_block_graph_us: u64,
    pub project_model_content_model_us: u64,
    pub project_model_listing_items_us: u64,
    pub project_model_listing_items_reused: bool,
    pub project_model_dynamic_widget_us: u64,
    pub project_model_markdown_us: u64,
    pub project_model_node_index_us: u64,
}

#[derive(Clone)]
pub(crate) enum EditorMoveExecution {
    Html {
        intent: ProjectHtmlMoveIntent,
        edit_scope_authorized: bool,
        source_render_instance_id: Option<String>,
        target_render_instance_id: Option<String>,
    },
    Tera {
        intent: ProjectTeraMoveIntent,
    },
}

pub(crate) struct EditorMoveDecision {
    pub plan: EditorMovePlan,
    pub execution: Option<EditorMoveExecution>,
}
