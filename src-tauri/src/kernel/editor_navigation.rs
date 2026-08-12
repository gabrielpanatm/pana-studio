use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    blocks::NativeBlockSlotMutationContext,
    kernel::{
        dynamic_widgets::DynamicWidgetProperties, preview_projection::CanvasPatch,
        project_workspace::ProjectWorkspaceMutationReceipt,
    },
    preview::{
        CanvasBoundaryInstance, CanvasGraph, CanvasMarkdownProvenanceState, CanvasNodeOrigin,
        CanvasProjectionIdentity, CanvasRenderNode,
    },
    project_model::{
        attribute_engine::raw_tag_attributes,
        model::ProjectModel,
        move_engine::{
            parse_html_tag_at, plan_html_move, plan_html_move_in_edit_scope, ProjectHtmlMoveIntent,
            ProjectMovePosition,
        },
        tera_move_engine::{plan_tera_move, ProjectTeraMoveIntent},
    },
    source_graph::model::{
        ComponentInvocation, ComponentInvocationKind, ComponentResolutionStatus,
        SourceCapabilityReason, SourceGraphTemplate, SourceNode, SourceNodeKind, SourceOrigin,
        SourceRange, SourceRelationKind,
    },
};

pub const EDITOR_NAVIGATION_SCHEMA_VERSION: u32 = 3;
pub const EDIT_SCOPE_GRANT_SCHEMA_VERSION: u32 = 2;
pub const EDITOR_MOVE_PLAN_SCHEMA_VERSION: u32 = 3;
pub const EDITOR_MOVE_EXECUTION_SCHEMA_VERSION: u32 = 1;
pub const EDITOR_MOVE_LIVE_PROJECTION_SCHEMA_VERSION: u32 = 1;
const MAX_LIVE_EDIT_SCOPE_GRANTS: usize = 64;
const MAX_LIVE_EDITOR_MOVE_PLANS: usize = 128;
const MAX_CACHED_EDITOR_NAVIGATION_SNAPSHOTS: usize = 8;

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
    TeraBoundary,
    MarkdownBoundary,
    RuntimeElement,
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
    Import,
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
    pub boundary_instance_id: String,
    pub source_node_id: String,
    pub root_render_instance_ids: Vec<String>,
    pub atomic_when_closed: bool,
    pub effect_scope: EditorNavigationEffectScope,
    pub rendered_instance_count: usize,
    pub target: Option<String>,
    pub empty: bool,
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
    planning_nodes: Vec<EditorNavigationNode>,
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
    pub project_model_template_parse_ms: u64,
    pub project_model_component_graph_ms: u64,
    pub project_model_block_graph_ms: u64,
    pub project_model_tera_graph_ms: u64,
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
    pub project_model_template_parse_ms: u64,
    pub project_model_component_graph_ms: u64,
    pub project_model_block_graph_ms: u64,
    pub project_model_tera_graph_ms: u64,
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

#[derive(Clone)]
struct StoredEditorMoveDecision {
    plan: EditorMovePlan,
    execution: Option<EditorMoveExecution>,
}

#[derive(Clone)]
struct EditorNavigationSnapshotCacheEntry {
    identity: CanvasProjectionIdentity,
    route: String,
    active_document_path: Option<String>,
    preview_context_render_instance_id: Option<String>,
    snapshot: EditorNavigationSnapshot,
}

#[derive(Default)]
pub struct EditorNavigationRuntime {
    grants: Mutex<HashMap<String, EditScopeGrant>>,
    move_plans: Mutex<HashMap<String, StoredEditorMoveDecision>>,
    snapshots: Mutex<VecDeque<EditorNavigationSnapshotCacheEntry>>,
}

impl EditorNavigationRuntime {
    pub(crate) fn cached_snapshot(
        &self,
        identity: &CanvasProjectionIdentity,
        route: &str,
        active_document_path: Option<&str>,
        preview_context_render_instance_id: Option<&str>,
    ) -> Result<Option<EditorNavigationSnapshot>, String> {
        let snapshots = self
            .snapshots
            .lock()
            .map_err(|_| "Cache-ul EditorNavigationSnapshot este indisponibil.".to_string())?;
        Ok(snapshots
            .iter()
            .rev()
            .find(|entry| {
                entry.identity == *identity
                    && same_preview_route(&entry.route, route)
                    && entry.active_document_path.as_deref() == active_document_path
                    && entry.preview_context_render_instance_id.as_deref()
                        == preview_context_render_instance_id
            })
            .map(|entry| entry.snapshot.clone()))
    }

    pub(crate) fn cache_snapshot(
        &self,
        active_document_path: Option<&str>,
        preview_context_render_instance_id: Option<&str>,
        snapshot: &EditorNavigationSnapshot,
    ) -> Result<(), String> {
        let mut snapshots = self
            .snapshots
            .lock()
            .map_err(|_| "Cache-ul EditorNavigationSnapshot este indisponibil.".to_string())?;
        snapshots.retain(|entry| {
            entry.identity != snapshot.identity
                || !same_preview_route(&entry.route, &snapshot.route)
                || entry.active_document_path.as_deref() != active_document_path
                || entry.preview_context_render_instance_id.as_deref()
                    != preview_context_render_instance_id
        });
        snapshots.push_back(EditorNavigationSnapshotCacheEntry {
            identity: snapshot.identity.clone(),
            route: snapshot.route.clone(),
            active_document_path: active_document_path.map(str::to_string),
            preview_context_render_instance_id: preview_context_render_instance_id
                .map(str::to_string),
            snapshot: snapshot.clone(),
        });
        while snapshots.len() > MAX_CACHED_EDITOR_NAVIGATION_SNAPSHOTS {
            snapshots.pop_front();
        }
        Ok(())
    }

    pub fn issue_edit_scope_grant(
        &self,
        identity: &CanvasProjectionIdentity,
        model_revision: &str,
        route: &str,
        active_document_path: &str,
        node: &EditorNavigationNode,
    ) -> Result<EditScopeGrant, String> {
        let boundary = node.boundary.as_ref().ok_or_else(|| {
            "EditScopeGrant poate fi emis numai pentru un boundary semantic.".to_string()
        })?;
        if !node.capabilities.can_enter_boundary
            || node.id
                != node
                    .capabilities
                    .requires_edit_scope_id
                    .as_deref()
                    .unwrap_or_default()
        {
            return Err("Boundary-ul nu permite intrarea în editarea structurală.".to_string());
        }
        let token = random_scope_token(identity, &node.id)?;
        let grant = EditScopeGrant {
            schema_version: EDIT_SCOPE_GRANT_SCHEMA_VERSION,
            token: token.clone(),
            scope_id: node.id.clone(),
            boundary_instance_id: boundary.boundary_instance_id.clone(),
            source_node_id: boundary.source_node_id.clone(),
            project_root: identity.project_root.clone(),
            runtime_session_id: identity.runtime_session_id.clone(),
            workspace_revision: identity.workspace_revision,
            model_revision: model_revision.to_string(),
            preview_revision: identity.preview_revision.clone(),
            canvas_transaction_id: identity.transaction_id.clone(),
            route: route.to_string(),
            active_document_path: active_document_path.to_string(),
            operations: vec![
                EditScopeOperation::MoveHtmlInside,
                EditScopeOperation::EditTextInside,
                EditScopeOperation::EditAttributesInside,
                EditScopeOperation::InspectSharedDefinition,
            ],
            issued_at_ms: now_ms(),
        };
        let mut grants = self
            .grants
            .lock()
            .map_err(|_| "Registrul EditScopeGrant este indisponibil.".to_string())?;
        grants.retain(|_, candidate| {
            candidate.project_root == identity.project_root
                && candidate.runtime_session_id == identity.runtime_session_id
                && candidate.workspace_revision == identity.workspace_revision
                && candidate.preview_revision == identity.preview_revision
                && candidate.canvas_transaction_id == identity.transaction_id
        });
        if grants.len() >= MAX_LIVE_EDIT_SCOPE_GRANTS {
            if let Some(oldest) = grants
                .values()
                .min_by_key(|candidate| candidate.issued_at_ms)
                .map(|candidate| candidate.token.clone())
            {
                grants.remove(&oldest);
            }
        }
        grants.insert(token, grant.clone());
        Ok(grant)
    }

    // Every argument is independent causal evidence checked against the stored grant.
    #[allow(clippy::too_many_arguments)]
    pub fn require_edit_scope_grant(
        &self,
        presented: &EditScopeGrant,
        identity: &CanvasProjectionIdentity,
        model_revision: &str,
        route: &str,
        active_document_path: &str,
        scope_id: &str,
        operation: EditScopeOperation,
    ) -> Result<EditScopeGrant, String> {
        if presented.schema_version != EDIT_SCOPE_GRANT_SCHEMA_VERSION {
            return Err("EditScopeGrant are altă versiune de protocol.".to_string());
        }
        let mut grants = self
            .grants
            .lock()
            .map_err(|_| "Registrul EditScopeGrant este indisponibil.".to_string())?;
        let stored = grants
            .get(&presented.token)
            .cloned()
            .ok_or_else(|| "EditScopeGrant a expirat sau a fost revocat.".to_string())?;
        if &stored != presented
            || stored.project_root != identity.project_root
            || stored.runtime_session_id != identity.runtime_session_id
            || stored.workspace_revision != identity.workspace_revision
            || stored.preview_revision != identity.preview_revision
            || stored.canvas_transaction_id != identity.transaction_id
            || stored.model_revision != model_revision
            || stored.route != route
            || stored.active_document_path != active_document_path
            || stored.scope_id != scope_id
            || !stored.operations.contains(&operation)
        {
            grants.remove(&presented.token);
            return Err(
                "EditScopeGrant nu aparține contextului, reviziei sau operației curente."
                    .to_string(),
            );
        }
        Ok(stored)
    }

    pub fn issue_editor_move_plan(&self, plan: EditorMovePlan) -> Result<EditorMovePlan, String> {
        self.issue_editor_move(plan, None)
    }

    pub(crate) fn issue_editor_move_decision(
        &self,
        decision: EditorMoveDecision,
    ) -> Result<EditorMovePlan, String> {
        self.issue_editor_move(decision.plan, decision.execution)
    }

    fn issue_editor_move(
        &self,
        mut plan: EditorMovePlan,
        execution: Option<EditorMoveExecution>,
    ) -> Result<EditorMovePlan, String> {
        if !plan.allowed || plan.operation.is_none() {
            return Ok(plan);
        }
        let token =
            random_move_plan_token(&plan.identity, &plan.source_node_id, &plan.target_node_id)?;
        plan.token = Some(token.clone());
        if let Some(projection) = plan.live_projection.as_mut() {
            projection.plan_token = Some(token.clone());
        }
        plan.issued_at_ms = now_ms();
        let mut plans = self
            .move_plans
            .lock()
            .map_err(|_| "Registrul PlanEditorMove este indisponibil.".to_string())?;
        plans.retain(|_, candidate| {
            candidate.plan.identity.project_root == plan.identity.project_root
                && candidate.plan.identity.runtime_session_id == plan.identity.runtime_session_id
                && candidate.plan.identity.workspace_revision == plan.identity.workspace_revision
                && candidate.plan.identity.preview_revision == plan.identity.preview_revision
                && candidate.plan.identity.transaction_id == plan.identity.transaction_id
                && candidate.plan.model_revision == plan.model_revision
        });
        if plans.len() >= MAX_LIVE_EDITOR_MOVE_PLANS {
            if let Some(oldest) = plans
                .values()
                .min_by_key(|candidate| candidate.plan.issued_at_ms)
                .and_then(|candidate| candidate.plan.token.clone())
            {
                plans.remove(&oldest);
            }
        }
        plans.insert(
            token,
            StoredEditorMoveDecision {
                plan: plan.clone(),
                execution,
            },
        );
        Ok(plan)
    }

    pub fn consume_editor_move_plan(
        &self,
        token: &str,
        identity: &CanvasProjectionIdentity,
        model_revision: &str,
        route: &str,
        active_document_path: &str,
    ) -> Result<EditorMovePlan, String> {
        let mut plans = self
            .move_plans
            .lock()
            .map_err(|_| "Registrul PlanEditorMove este indisponibil.".to_string())?;
        let stored = plans
            .remove(token)
            .ok_or_else(|| "PlanEditorMove a expirat sau a fost deja consumat.".to_string())?;
        if stored.plan.schema_version != EDITOR_MOVE_PLAN_SCHEMA_VERSION
            || stored.plan.token.as_deref() != Some(token)
            || stored.plan.identity != *identity
            || stored.plan.model_revision != model_revision
            || stored.plan.route != route
            || stored.plan.active_document_path != active_document_path
        {
            return Err("PlanEditorMove nu aparține contextului sau reviziei curente.".to_string());
        }
        Ok(stored.plan)
    }

    pub(crate) fn consume_editor_move_decision(
        &self,
        token: &str,
        identity: &CanvasProjectionIdentity,
        model_revision: &str,
        route: &str,
        active_document_path: &str,
    ) -> Result<EditorMoveDecision, String> {
        let mut plans = self
            .move_plans
            .lock()
            .map_err(|_| "Registrul PlanEditorMove este indisponibil.".to_string())?;
        let stored = plans
            .remove(token)
            .ok_or_else(|| "PlanEditorMove a expirat sau a fost deja consumat.".to_string())?;
        let plan = stored.plan;
        if plan.schema_version != EDITOR_MOVE_PLAN_SCHEMA_VERSION
            || plan.token.as_deref() != Some(token)
            || plan.identity != *identity
            || plan.model_revision != model_revision
            || plan.route != route
            || plan.active_document_path != active_document_path
        {
            return Err("PlanEditorMove nu aparține contextului sau reviziei curente.".to_string());
        }
        Ok(EditorMoveDecision {
            plan,
            execution: stored.execution,
        })
    }

    pub fn revoke_all(&self) {
        if let Ok(mut grants) = self.grants.lock() {
            grants.clear();
        }
        if let Ok(mut plans) = self.move_plans.lock() {
            plans.clear();
        }
        if let Ok(mut snapshots) = self.snapshots.lock() {
            snapshots.clear();
        }
    }
}

pub(crate) fn plan_editor_move(
    runtime: &EditorNavigationRuntime,
    snapshot: &EditorNavigationSnapshot,
    model: &ProjectModel,
    source_node_id: &str,
    target_node_id: &str,
    position: ProjectMovePosition,
    presented_grant: Option<&EditScopeGrant>,
) -> EditorMoveDecision {
    plan_editor_move_with_slot(
        runtime,
        snapshot,
        model,
        source_node_id,
        target_node_id,
        position,
        presented_grant,
        None,
    )
}

// The planner keeps source, target, snapshot and optional authorization evidence explicit.
#[allow(clippy::too_many_arguments)]
pub(crate) fn plan_editor_move_with_slot(
    runtime: &EditorNavigationRuntime,
    snapshot: &EditorNavigationSnapshot,
    model: &ProjectModel,
    source_node_id: &str,
    target_node_id: &str,
    position: ProjectMovePosition,
    presented_grant: Option<&EditScopeGrant>,
    native_block_slot: Option<NativeBlockSlotMutationContext>,
) -> EditorMoveDecision {
    let active_document_path = snapshot
        .focused_view
        .as_ref()
        .map(|view| view.active_document_path.clone())
        .unwrap_or_default();
    let empty_impact = || EditorMoveImpact {
        files: Vec::new(),
        edit_scope_id: None,
        effect_scope: EditorNavigationEffectScope::SingleSource,
        rendered_instance_count: 1,
        affects_all_rendered_instances: false,
        requires_preview_reprojection: true,
    };
    let blocked = |code: &str, reason: String, impact: EditorMoveImpact| EditorMoveDecision {
        plan: EditorMovePlan {
            schema_version: EDITOR_MOVE_PLAN_SCHEMA_VERSION,
            token: None,
            allowed: false,
            reason_code: Some(code.to_string()),
            reason: Some(reason),
            operation: None,
            identity: snapshot.identity.clone(),
            model_revision: snapshot.model_revision.clone(),
            route: snapshot.route.clone(),
            active_document_path: active_document_path.clone(),
            source_node_id: source_node_id.to_string(),
            target_node_id: target_node_id.to_string(),
            position,
            impact,
            live_projection: None,
            live_projection_reason: EditorMoveLiveProjectionReason::PlanBlocked,
            issued_at_ms: now_ms(),
        },
        execution: None,
    };
    let Some(source) = editor_navigation_node(snapshot, source_node_id) else {
        return blocked(
            "editor_move_source_missing",
            "Sursa nu mai există în EditorNavigationSnapshot.".to_string(),
            empty_impact(),
        );
    };
    let Some(target) = editor_navigation_node(snapshot, target_node_id) else {
        return blocked(
            "editor_move_target_missing",
            "Destinația nu mai există în EditorNavigationSnapshot.".to_string(),
            empty_impact(),
        );
    };
    let mut files = [source.file.clone(), target.file.clone()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    files.sort();
    files.dedup();
    let (source_scope, target_scope) = if source.kind == EditorNavigationNodeKind::HtmlElement {
        (
            enclosing_edit_scope(snapshot, source, false),
            enclosing_edit_scope(snapshot, target, position == ProjectMovePosition::Inside),
        )
    } else {
        // EditScopeGrant protects mutations of rendered HTML inside a Tera
        // boundary. Moving the Tera construct itself remains an atomic Rust
        // operation and its legal nesting is decided by the Tera planner.
        (None, None)
    };
    let required_scope = match (&source_scope, &target_scope) {
        (Some(left), Some(right)) if left == right => Some(left.clone()),
        (None, None) => None,
        _ => {
            return blocked(
                "editor_move_cross_scope",
                "Mutarea copiilor HTML peste limita unui boundary Tera este blocată; intră în același scope și păstrează mutația în interiorul lui."
                    .to_string(),
                EditorMoveImpact {
                    files,
                    edit_scope_id: source_scope.or(target_scope),
                    ..empty_impact()
                },
            )
        }
    };
    if let Some(scope_id) = required_scope.as_deref() {
        let Some(grant) = presented_grant else {
            return blocked(
                "editor_move_scope_required",
                "Destinația cere intrare explicită în boundary înainte de mutare.".to_string(),
                EditorMoveImpact {
                    files,
                    edit_scope_id: Some(scope_id.to_string()),
                    ..empty_impact()
                },
            );
        };
        if let Err(reason) = runtime.require_edit_scope_grant(
            grant,
            &snapshot.identity,
            &snapshot.model_revision,
            &snapshot.route,
            &active_document_path,
            scope_id,
            EditScopeOperation::MoveHtmlInside,
        ) {
            return blocked(
                "editor_move_scope_invalid",
                reason,
                EditorMoveImpact {
                    files,
                    edit_scope_id: Some(scope_id.to_string()),
                    ..empty_impact()
                },
            );
        }
    }

    let source_boundary = source.boundary.as_ref();
    let effect_scope = source_boundary
        .map(|boundary| boundary.effect_scope)
        .or_else(|| {
            required_scope
                .as_deref()
                .and_then(|scope| editor_navigation_node(snapshot, scope))
                .and_then(|node| node.boundary.as_ref())
                .map(|boundary| boundary.effect_scope)
        })
        .unwrap_or(EditorNavigationEffectScope::SingleSource);
    let rendered_instance_count = source_boundary
        .map(|boundary| boundary.rendered_instance_count)
        .or_else(|| {
            required_scope
                .as_deref()
                .and_then(|scope| editor_navigation_node(snapshot, scope))
                .and_then(|node| node.boundary.as_ref())
                .map(|boundary| boundary.rendered_instance_count)
        })
        .unwrap_or(1);
    let impact = EditorMoveImpact {
        files,
        edit_scope_id: required_scope.clone(),
        effect_scope,
        rendered_instance_count,
        affects_all_rendered_instances: effect_scope
            == EditorNavigationEffectScope::AllRenderedInstances,
        requires_preview_reprojection: true,
    };

    let (operation, execution, diagnostic) = match source.kind {
        EditorNavigationNodeKind::RuntimeElement => (
            None,
            None,
            Some((
                "editor_move_runtime_read_only",
                "Elementele generate numai la runtime nu au sursă structurală mutabilă.",
            )),
        ),
        EditorNavigationNodeKind::MarkdownBoundary => (
            None,
            None,
            Some((
                "editor_move_markdown_read_only",
                "Conținutul randat din Markdown este atomic și poate fi modificat numai în sursa Markdown.",
            )),
        ),
        EditorNavigationNodeKind::TeraBoundary => {
            if !source.capabilities.can_move_atomic {
                (
                    None,
                    None,
                    Some((
                        "editor_move_tera_read_only",
                        "Boundary-ul Tera nu este mutabil în sursa curentă.",
                    )),
                )
            } else {
                let Some(intent) = tera_move_intent(source, target, position) else {
                    return blocked(
                        "editor_move_anchor_missing",
                        "Boundary-ul Tera nu are ancore SourceGraph complete.".to_string(),
                        impact,
                    );
                };
                let plan = plan_tera_move(model, &intent);
                if plan.allowed {
                    (
                        Some(if source.source_kind == Some(SourceNodeKind::Block) {
                            EditorMoveOperation::BlockMove
                        } else {
                            EditorMoveOperation::AtomicTeraMove
                        }),
                        Some(EditorMoveExecution::Tera { intent }),
                        None,
                    )
                } else {
                    return blocked(
                        "editor_move_tera_plan_blocked",
                        plan.diagnostic
                            .unwrap_or_else(|| "Plannerul Tera a refuzat mutarea.".to_string()),
                        impact,
                    );
                }
            }
        }
        EditorNavigationNodeKind::HtmlElement => {
            if target.kind != EditorNavigationNodeKind::HtmlElement {
                (
                    None,
                    None,
                    Some((
                        "editor_move_html_target_kind",
                        "Un element HTML poate fi ancorat numai de un alt element HTML din același scope.",
                    )),
                )
            } else if required_scope.is_none() && !source.capabilities.can_move {
                (
                    None,
                    None,
                    Some((
                        "editor_move_html_read_only",
                        "Elementul HTML nu este mutabil conform capabilităților Rust.",
                    )),
                )
            } else if required_scope.is_none() && !target.capabilities.can_move {
                (
                    None,
                    None,
                    Some((
                        "editor_move_html_target_read_only",
                        "Destinația HTML nu este mutabilă conform capabilităților Rust.",
                    )),
                )
            } else {
                let Some(intent) = html_move_intent(
                    source,
                    target,
                    position,
                    native_block_slot.clone(),
                ) else {
                    return blocked(
                        "editor_move_anchor_missing",
                        "Elementele HTML nu au ancore SourceGraph complete.".to_string(),
                        impact,
                    );
                };
                let scoped = required_scope.is_some();
                let plan = if scoped {
                    plan_html_move_in_edit_scope(model, &intent)
                } else {
                    plan_html_move(model, &intent)
                };
                if !plan.allowed {
                    return blocked(
                        "editor_move_html_plan_blocked",
                        plan.diagnostic
                            .unwrap_or_else(|| "Plannerul HTML a refuzat mutarea.".to_string()),
                        impact,
                    );
                }
                let operation = if !source.block_source_instance_ids.is_empty() {
                    EditorMoveOperation::BlockMove
                } else if !source.component_invocation_ids.is_empty()
                    || !source.component_definition_ids.is_empty()
                {
                    EditorMoveOperation::ComponentMove
                } else {
                    EditorMoveOperation::HtmlSourceMove
                };
                (
                    Some(operation),
                    Some(EditorMoveExecution::Html {
                        intent,
                        edit_scope_authorized: scoped,
                        source_render_instance_id: source.render_instance_id.clone(),
                        target_render_instance_id: target.render_instance_id.clone(),
                    }),
                    None,
                )
            }
        }
    };
    if let Some((code, reason)) = diagnostic {
        return blocked(code, reason.to_string(), impact);
    }
    let (live_projection, live_projection_reason) =
        editor_move_live_projection(snapshot, source, target, position, execution.as_ref());
    EditorMoveDecision {
        plan: EditorMovePlan {
            schema_version: EDITOR_MOVE_PLAN_SCHEMA_VERSION,
            token: None,
            allowed: true,
            reason_code: None,
            reason: None,
            operation,
            identity: snapshot.identity.clone(),
            model_revision: snapshot.model_revision.clone(),
            route: snapshot.route.clone(),
            active_document_path,
            source_node_id: source_node_id.to_string(),
            target_node_id: target_node_id.to_string(),
            position,
            impact,
            live_projection,
            live_projection_reason,
            issued_at_ms: now_ms(),
        },
        execution,
    }
}

fn editor_move_live_projection(
    snapshot: &EditorNavigationSnapshot,
    source: &EditorNavigationNode,
    target: &EditorNavigationNode,
    position: ProjectMovePosition,
    execution: Option<&EditorMoveExecution>,
) -> (
    Option<EditorMoveLiveProjection>,
    EditorMoveLiveProjectionReason,
) {
    let Some(EditorMoveExecution::Html {
        source_render_instance_id,
        target_render_instance_id,
        ..
    }) = execution
    else {
        return (None, EditorMoveLiveProjectionReason::ExecutionNotHtml);
    };
    if !matches!(
        source.source_provenance.resolution,
        EditorSourceResolution::Direct
            | EditorSourceResolution::Resolved
            | EditorSourceResolution::FallbackResolved
    ) || !matches!(
        target.source_provenance.resolution,
        EditorSourceResolution::Direct
            | EditorSourceResolution::Resolved
            | EditorSourceResolution::FallbackResolved
    ) {
        return (
            None,
            EditorMoveLiveProjectionReason::AmbiguousSourceIdentity,
        );
    }
    let (Some(source_render_instance_id), Some(target_render_instance_id)) = (
        source_render_instance_id.as_ref(),
        target_render_instance_id.as_ref(),
    ) else {
        return (None, EditorMoveLiveProjectionReason::MissingRenderIdentity);
    };
    let rendered_source_instances = snapshot
        .nodes
        .iter()
        .chain(snapshot.planning_nodes.iter())
        .filter(|node| node.source_node_id == source.source_node_id)
        .filter_map(|node| node.render_instance_id.as_deref())
        .collect::<HashSet<_>>();
    if rendered_source_instances.len() != 1
        || !rendered_source_instances.contains(source_render_instance_id.as_str())
    {
        // Mutarea sursei unei componente randate de mai multe ori ar schimba
        // toate instanțele la commit. Până când Rust poate emite setul complet
        // de perechi DOM, o proiecție doar pentru selecție ar fi falsă.
        return (
            None,
            EditorMoveLiveProjectionReason::MultipleRenderedInstances,
        );
    }
    let source_parent_render_instance_id = source
        .parent_id
        .as_deref()
        .and_then(|parent_id| editor_navigation_node(snapshot, parent_id))
        .and_then(|parent| parent.render_instance_id.clone());
    let source_next_sibling_render_instance_id = snapshot
        .nodes
        .iter()
        .chain(snapshot.planning_nodes.iter())
        .filter(|candidate| {
            candidate.parent_id == source.parent_id
                && candidate.order > source.order
                && candidate.render_instance_id.is_some()
        })
        .min_by_key(|candidate| candidate.order)
        .and_then(|candidate| candidate.render_instance_id.clone());
    (
        Some(EditorMoveLiveProjection {
            schema_version: EDITOR_MOVE_LIVE_PROJECTION_SCHEMA_VERSION,
            operation: EditorMoveLiveProjectionOperation::Move,
            scope: EditorMoveLiveProjectionScope::SelectedInstance,
            plan_token: None,
            identity: snapshot.identity.clone(),
            source_render_instance_id: source_render_instance_id.clone(),
            target_render_instance_id: target_render_instance_id.clone(),
            position,
            rollback: EditorMoveLiveProjectionRollback {
                source_parent_render_instance_id,
                source_next_sibling_render_instance_id,
            },
        }),
        EditorMoveLiveProjectionReason::Ready,
    )
}

fn enclosing_edit_scope(
    snapshot: &EditorNavigationSnapshot,
    node: &EditorNavigationNode,
    enter_target_boundary: bool,
) -> Option<String> {
    if enter_target_boundary && node.kind == EditorNavigationNodeKind::TeraBoundary {
        return node.capabilities.requires_edit_scope_id.clone();
    }
    if node.kind != EditorNavigationNodeKind::TeraBoundary {
        if let Some(scope) = node.capabilities.requires_edit_scope_id.as_ref() {
            return Some(scope.clone());
        }
    }
    let mut parent_id = node.parent_id.as_deref();
    let mut visited = HashSet::new();
    while let Some(parent) = parent_id {
        if !visited.insert(parent) {
            break;
        }
        let parent_node = editor_navigation_node(snapshot, parent)?;
        if parent_node.kind == EditorNavigationNodeKind::TeraBoundary {
            // A boundary without a required scope is the implicitly-open
            // document wrapper. It is also a scope barrier: inherited
            // boundaries outside the active document must not lock HTML
            // authored by the active document.
            return parent_node.capabilities.requires_edit_scope_id.clone();
        }
        parent_id = parent_node.parent_id.as_deref();
    }
    None
}

pub(crate) fn editor_navigation_node<'a>(
    snapshot: &'a EditorNavigationSnapshot,
    node_id: &str,
) -> Option<&'a EditorNavigationNode> {
    snapshot
        .nodes
        .iter()
        .chain(snapshot.planning_nodes.iter())
        .find(|node| node.id == node_id)
}

fn html_move_intent(
    source: &EditorNavigationNode,
    target: &EditorNavigationNode,
    position: ProjectMovePosition,
    native_block_slot: Option<NativeBlockSlotMutationContext>,
) -> Option<ProjectHtmlMoveIntent> {
    Some(ProjectHtmlMoveIntent {
        source_source_id: source.source_node_id.clone(),
        target_source_id: target.source_node_id.clone(),
        source_tag: source.tag.clone(),
        target_tag: target.tag.clone(),
        position,
        native_block_slot,
    })
    .filter(|intent| {
        intent.source_source_id.is_some()
            && intent.target_source_id.is_some()
            && intent.source_tag.is_some()
            && intent.target_tag.is_some()
    })
}

fn tera_move_intent(
    source: &EditorNavigationNode,
    target: &EditorNavigationNode,
    position: ProjectMovePosition,
) -> Option<ProjectTeraMoveIntent> {
    Some(ProjectTeraMoveIntent {
        source_source_id: source.source_node_id.clone(),
        target_source_id: target.source_node_id.clone(),
        source_kind: None,
        target_kind: None,
        source_label: Some(source.label.clone()),
        target_tag: target.tag.clone(),
        position,
    })
    .filter(|intent| intent.source_source_id.is_some() && intent.target_source_id.is_some())
}

fn editor_source_provenance(
    model: &ProjectModel,
    selected_source: Option<&SourceNode>,
    component_invocation_ids: &[String],
) -> EditorSourceProvenance {
    let component_graph = &model.source_graph.component_graph;
    let direct_invocation = selected_source.and_then(|source| {
        component_graph.invocations.iter().find(|invocation| {
            invocation.source_node_id.as_deref() == Some(source.id.as_str())
                && matches!(
                    invocation.kind,
                    ComponentInvocationKind::Include
                        | ComponentInvocationKind::MacroCall
                        | ComponentInvocationKind::Shortcode
                )
        })
    });
    let ambient_invocation = component_invocation_ids
        .iter()
        .rev()
        .find_map(|invocation_id| {
            component_graph
                .invocations
                .iter()
                .find(|invocation| invocation.id == *invocation_id)
        });
    let invocation = direct_invocation.or(ambient_invocation);
    let composition = invocation
        .and_then(|invocation| invocation.source_node_id.as_deref())
        .and_then(|source_node_id| model.source_graph.node_by_id(source_node_id))
        .filter(|node| selected_source.is_none_or(|selected| selected.id != node.id))
        .map(editor_source_reference);

    if let Some(invocation) = direct_invocation {
        return EditorSourceProvenance {
            definition: resolved_component_definition_source(model, invocation),
            composition: selected_source.map(editor_source_reference),
            resolution: editor_source_resolution(&invocation.status),
        };
    }

    EditorSourceProvenance {
        definition: selected_source.map(editor_source_reference),
        composition,
        resolution: invocation
            .map(|invocation| editor_source_resolution(&invocation.status))
            .unwrap_or(EditorSourceResolution::Direct),
    }
}

fn markdown_source_provenance(
    model: &ProjectModel,
    boundary: &CanvasBoundaryInstance,
    source: Option<&SourceNode>,
) -> EditorSourceProvenance {
    let Some(markdown) = boundary.markdown.as_ref() else {
        return editor_source_provenance(model, source, &[]);
    };
    let resolved = markdown.provenance_state == CanvasMarkdownProvenanceState::Resolved;
    let definition = resolved.then(|| EditorSourceReference {
        source_node_id: Some(boundary.source_node_id.clone()),
        source_kind: source.map(|node| node.kind.clone()),
        file: markdown.source_file.clone().unwrap_or_default(),
        range: markdown.source_range.clone(),
        label: markdown.kind.label().to_string(),
        origin: EditorNavigationOrigin::Project,
        theme_name: None,
        can_open_in_code: true,
    });
    let composition = model
        .source_graph
        .node_by_id(&markdown.template_source_node_id)
        .map(|node| {
            let mut reference = editor_source_reference(node);
            reference.range = markdown.template_range.clone().or(reference.range);
            reference
        })
        .or_else(|| {
            Some(EditorSourceReference {
                source_node_id: Some(markdown.template_source_node_id.clone()),
                source_kind: Some(SourceNodeKind::TeraVariable),
                file: markdown.template_file.clone(),
                range: markdown.template_range.clone(),
                label: "Proiecție Markdown".to_string(),
                origin: EditorNavigationOrigin::Tera,
                theme_name: None,
                can_open_in_code: true,
            })
        });
    EditorSourceProvenance {
        definition,
        composition,
        resolution: if resolved {
            EditorSourceResolution::Resolved
        } else {
            EditorSourceResolution::Unresolved
        },
    }
}

fn resolved_component_definition_source(
    model: &ProjectModel,
    invocation: &ComponentInvocation,
) -> Option<EditorSourceReference> {
    invocation
        .resolved_definition_ids
        .iter()
        .find_map(|definition_id| {
            model
                .source_graph
                .component_graph
                .definitions
                .iter()
                .find(|definition| definition.id == *definition_id)
        })
        .and_then(|definition| definition.source_node_id.as_deref())
        .and_then(|source_node_id| model.source_graph.node_by_id(source_node_id))
        .map(editor_source_reference)
}

fn editor_source_reference(source: &SourceNode) -> EditorSourceReference {
    EditorSourceReference {
        source_node_id: Some(source.id.clone()),
        source_kind: Some(source.kind.clone()),
        file: source.file.clone(),
        range: source.range.clone(),
        label: source.label.clone(),
        origin: source_origin(Some(source)),
        theme_name: source.theme_name.clone(),
        can_open_in_code: source.capabilities.can_open_in_code,
    }
}

fn editor_source_resolution(status: &ComponentResolutionStatus) -> EditorSourceResolution {
    match status {
        ComponentResolutionStatus::Resolved => EditorSourceResolution::Resolved,
        ComponentResolutionStatus::FallbackResolved => EditorSourceResolution::FallbackResolved,
        ComponentResolutionStatus::Ambiguous => EditorSourceResolution::Ambiguous,
        ComponentResolutionStatus::Dynamic => EditorSourceResolution::Dynamic,
        ComponentResolutionStatus::External => EditorSourceResolution::External,
        ComponentResolutionStatus::Unresolved => EditorSourceResolution::Unresolved,
    }
}

pub(crate) fn build_editor_navigation_snapshot(
    identity: CanvasProjectionIdentity,
    route: &str,
    model: &ProjectModel,
    graph: &CanvasGraph,
    active_document_path: Option<&str>,
    preview_context_render_instance_id: Option<&str>,
) -> Result<EditorNavigationSnapshot, String> {
    if identity.workspace_revision != graph.workspace_revision
        || identity.preview_revision != graph.preview_revision
    {
        return Err(
            "EditorNavigationSnapshot a refuzat un CanvasGraph cu altă revizie Preview."
                .to_string(),
        );
    }
    if model.revision != graph.model_revision {
        return Err(format!(
            "EditorNavigationSnapshot a refuzat ProjectModel {} pentru CanvasGraph {}.",
            model.revision, graph.model_revision
        ));
    }
    let document = graph
        .documents
        .iter()
        .find(|document| same_preview_route(&document.route, route))
        .ok_or_else(|| {
            format!("EditorNavigationSnapshot nu găsește ruta {route:?} în CanvasGraph-ul activ.")
        })?;
    let route = document.route.clone();
    let source_nodes = model
        .source_graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<HashMap<_, _>>();
    let implicitly_open_boundary_source_ids =
        implicitly_open_document_boundaries(model, active_document_path, &source_nodes);
    let render_nodes = document
        .nodes
        .iter()
        .map(|node| (node.render_instance_id.as_str(), node))
        .collect::<HashMap<_, _>>();
    let boundary_ids = document
        .boundaries
        .iter()
        .map(|boundary| {
            (
                boundary.boundary_instance_id.as_str(),
                editor_boundary_node_id(&boundary.boundary_instance_id),
            )
        })
        .collect::<HashMap<_, _>>();
    let boundary_by_id = document
        .boundaries
        .iter()
        .map(|boundary| (boundary.boundary_instance_id.as_str(), boundary))
        .collect::<HashMap<_, _>>();
    let boundary_depths = document
        .boundaries
        .iter()
        .map(|boundary| {
            (
                boundary.boundary_instance_id.as_str(),
                boundary_depth(boundary, &boundary_by_id),
            )
        })
        .collect::<HashMap<_, _>>();
    let boundary_counts =
        document
            .boundaries
            .iter()
            .fold(HashMap::<&str, usize>::new(), |mut counts, boundary| {
                *counts.entry(boundary.source_node_id.as_str()).or_default() += 1;
                counts
            });

    let mut boundaries_by_render_root = HashMap::<&str, Vec<&CanvasBoundaryInstance>>::new();
    for boundary in &document.boundaries {
        for root in &boundary.root_render_instance_ids {
            boundaries_by_render_root
                .entry(root.as_str())
                .or_default()
                .push(boundary);
        }
    }
    for boundaries in boundaries_by_render_root.values_mut() {
        boundaries.sort_by(|left, right| {
            let left_depth = boundary_depths
                .get(left.boundary_instance_id.as_str())
                .copied()
                .unwrap_or_default();
            let right_depth = boundary_depths
                .get(right.boundary_instance_id.as_str())
                .copied()
                .unwrap_or_default();
            right_depth
                .cmp(&left_depth)
                .then_with(|| left.boundary_instance_id.cmp(&right.boundary_instance_id))
        });
    }

    // A render node belongs to the boundary whose root is its nearest
    // ancestor. Walking the render ancestry once avoids the previous
    // nodes × boundaries × ancestry scan on large pages.
    let mut boundary_for_render = HashMap::<&str, &CanvasBoundaryInstance>::new();
    for render_node in &document.nodes {
        let mut cursor = Some(render_node.render_instance_id.as_str());
        let mut visited = HashSet::new();
        while let Some(current) = cursor {
            if !visited.insert(current) {
                break;
            }
            if let Some(boundary) = boundaries_by_render_root
                .get(current)
                .and_then(|boundaries| boundaries.first())
            {
                boundary_for_render.insert(render_node.render_instance_id.as_str(), *boundary);
                break;
            }
            cursor = render_nodes
                .get(current)
                .and_then(|node| node.parent_render_instance_id.as_deref());
        }
    }

    let mut nodes = Vec::with_capacity(document.nodes.len() + document.boundaries.len());
    for boundary in &document.boundaries {
        let markdown = boundary.markdown.as_ref();
        let source = source_nodes.get(boundary.source_node_id.as_str()).copied();
        let parent_id = boundary
            .parent_boundary_instance_id
            .as_deref()
            .and_then(|parent| boundary_ids.get(parent).cloned())
            .or_else(|| {
                boundary
                    .root_render_instance_ids
                    .iter()
                    .filter_map(|root| render_nodes.get(root.as_str()).copied())
                    .find_map(|root| {
                        root.parent_render_instance_id
                            .as_deref()
                            .map(editor_render_node_id)
                    })
            });
        let source_kind = source.map(|node| node.kind.clone());
        let implicitly_open = markdown.is_none()
            && implicitly_open_boundary_source_ids.contains(boundary.source_node_id.as_str());
        let can_enter = markdown.is_none()
            && !implicitly_open
            && source_kind.as_ref().is_some_and(editable_boundary_kind);
        let local_source = source.is_some_and(|node| node.origin == SourceOrigin::Local);
        let can_move_atomic = markdown.is_none()
            && local_source
            && source_kind.as_ref().is_some_and(movable_boundary_kind);
        let target = model
            .tera_graph
            .nodes
            .iter()
            .find(|node| node.id == boundary.source_node_id)
            .and_then(|node| node.target.clone());
        let effect_scope = if markdown.is_some() {
            EditorNavigationEffectScope::SingleSource
        } else {
            boundary_effect_scope(source_kind.as_ref())
        };
        let node_id = editor_boundary_node_id(&boundary.boundary_instance_id);
        let source_provenance = markdown_source_provenance(model, boundary, source);
        let markdown_resolved = markdown.is_some_and(|markdown| {
            markdown.provenance_state == CanvasMarkdownProvenanceState::Resolved
        });
        nodes.push(EditorNavigationNode {
            id: node_id.clone(),
            parent_id,
            children: Vec::new(),
            order: boundary.document_order,
            kind: if markdown.is_some() {
                EditorNavigationNodeKind::MarkdownBoundary
            } else {
                EditorNavigationNodeKind::TeraBoundary
            },
            label: markdown
                .map(|markdown| markdown.kind.label().to_string())
                .or_else(|| source.map(|node| node.label.clone()))
                .unwrap_or_else(|| "Boundary Tera".to_string()),
            tag: None,
            source_node_id: Some(boundary.source_node_id.clone()),
            render_instance_id: None,
            source_kind,
            file: markdown
                .and_then(|markdown| markdown.source_file.clone())
                .or_else(|| source.map(|node| node.file.clone())),
            range: markdown
                .and_then(|markdown| markdown.source_range.clone())
                .or_else(|| source.and_then(|node| node.range.clone())),
            origin: source_origin(source),
            theme_name: source.and_then(|node| node.theme_name.clone()),
            source_provenance,
            provenance_stack: vec![boundary.source_node_id.clone()],
            component_definition_ids: Vec::new(),
            component_invocation_ids: Vec::new(),
            block_definition_ids: Vec::new(),
            block_source_instance_ids: Vec::new(),
            dynamic_widget_provider_ids: Vec::new(),
            dynamic_widget_source_instance_ids: Vec::new(),
            binding_key: boundary.binding_key.clone(),
            binding_path: boundary.binding_path.clone(),
            boundary: Some(EditorNavigationBoundary {
                boundary_instance_id: boundary.boundary_instance_id.clone(),
                source_node_id: boundary.source_node_id.clone(),
                root_render_instance_ids: boundary.root_render_instance_ids.clone(),
                atomic_when_closed: true,
                effect_scope,
                rendered_instance_count: boundary_counts
                    .get(boundary.source_node_id.as_str())
                    .copied()
                    .unwrap_or(1),
                target,
                empty: boundary.root_render_instance_ids.is_empty(),
            }),
            capabilities: EditorNavigationCapabilities {
                can_select: true,
                can_inspect: true,
                can_open_in_code: if markdown.is_some() {
                    markdown_resolved
                } else {
                    source.is_some_and(|node| node.capabilities.can_open_in_code)
                },
                can_enter_boundary: can_enter,
                can_move_atomic,
                can_move: can_move_atomic,
                can_edit_text: false,
                can_edit_attributes: false,
                read_only: markdown.is_some() || !local_source,
                requires_edit_scope_id: can_enter.then_some(node_id),
                reason_code: if markdown.is_some() {
                    Some(if markdown_resolved {
                        SourceCapabilityReason::MarkdownRenderedBoundary
                    } else {
                        SourceCapabilityReason::MarkdownSourceUnresolved
                    })
                } else {
                    source.and_then(|node| node.capabilities.reason_code)
                },
            },
            source_html_attributes: None,
        });
    }

    for render_node in &document.nodes {
        let source = primary_source_node(render_node, &source_nodes);
        let containing_boundary = boundary_for_render
            .get(render_node.render_instance_id.as_str())
            .copied();
        let is_boundary_root = containing_boundary.is_some_and(|boundary| {
            boundary
                .root_render_instance_ids
                .iter()
                .any(|root| root == &render_node.render_instance_id)
        });
        let parent_id = if is_boundary_root {
            containing_boundary
                .map(|boundary| editor_boundary_node_id(&boundary.boundary_instance_id))
        } else {
            render_node
                .parent_render_instance_id
                .as_deref()
                .map(editor_render_node_id)
        };
        let requires_scope_id = containing_boundary
            .filter(|boundary| {
                boundary.markdown.is_some()
                    || (!implicitly_open_boundary_source_ids
                        .contains(boundary.source_node_id.as_str())
                        && source_nodes
                            .get(boundary.source_node_id.as_str())
                            .is_some_and(|node| editable_boundary_kind(&node.kind)))
            })
            .map(|boundary| editor_boundary_node_id(&boundary.boundary_instance_id));
        let source_capabilities = source.map(|node| &node.capabilities);
        let source_provenance =
            editor_source_provenance(model, source, &render_node.component_invocation_ids);
        let unlocked = requires_scope_id.is_none();
        let can_move = unlocked
            && render_node.capabilities.editable
            && source_capabilities.is_some_and(|capabilities| capabilities.can_move);
        let source_backed = source.is_some();
        nodes.push(EditorNavigationNode {
            id: editor_render_node_id(&render_node.render_instance_id),
            parent_id,
            children: Vec::new(),
            order: render_node.document_order,
            kind: if source_backed {
                EditorNavigationNodeKind::HtmlElement
            } else if render_node.origin == CanvasNodeOrigin::ArbitraryJsRuntime {
                EditorNavigationNodeKind::RuntimeElement
            } else {
                EditorNavigationNodeKind::HtmlElement
            },
            label: source
                .map(|node| node.label.clone())
                .unwrap_or_else(|| format!("<{}>", render_node.tag)),
            tag: Some(render_node.tag.clone()),
            source_node_id: source.map(|node| node.id.clone()),
            render_instance_id: Some(render_node.render_instance_id.clone()),
            source_kind: source.map(|node| node.kind.clone()),
            file: source.map(|node| node.file.clone()),
            range: source.and_then(|node| node.range.clone()),
            origin: render_origin(render_node, source),
            theme_name: source.and_then(|node| node.theme_name.clone()),
            source_provenance,
            provenance_stack: render_node.provenance_stack.clone(),
            component_definition_ids: render_node.component_definition_ids.clone(),
            component_invocation_ids: render_node.component_invocation_ids.clone(),
            block_definition_ids: render_node.block_definition_ids.clone(),
            block_source_instance_ids: render_node.block_source_instance_ids.clone(),
            dynamic_widget_provider_ids: render_node.dynamic_widget_provider_ids.clone(),
            dynamic_widget_source_instance_ids: render_node
                .dynamic_widget_source_instance_ids
                .clone(),
            binding_key: render_node.binding_key.clone(),
            binding_path: render_node.binding_path.clone(),
            boundary: None,
            capabilities: EditorNavigationCapabilities {
                can_select: true,
                can_inspect: render_node.capabilities.inspectable,
                can_open_in_code: source_capabilities
                    .is_some_and(|capabilities| capabilities.can_open_in_code),
                can_enter_boundary: false,
                can_move_atomic: false,
                can_move,
                can_edit_text: unlocked
                    && render_node.capabilities.editable
                    && source_capabilities.is_some_and(|capabilities| capabilities.can_edit_text),
                can_edit_attributes: unlocked
                    && render_node.capabilities.editable
                    && source_capabilities
                        .is_some_and(|capabilities| capabilities.can_edit_attributes),
                read_only: render_node.capabilities.read_only || !unlocked,
                requires_edit_scope_id: requires_scope_id,
                reason_code: source_capabilities.and_then(|capabilities| capabilities.reason_code),
            },
            source_html_attributes: source_html_attributes(model, source),
        });
    }

    let live_ids = nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<HashSet<_>>();
    for node in &mut nodes {
        if node
            .parent_id
            .as_ref()
            .is_some_and(|parent| !live_ids.contains(parent))
        {
            node.parent_id = None;
        }
    }
    let mut children_by_parent = nodes.iter().fold(
        HashMap::<String, Vec<(usize, String)>>::new(),
        |mut map, node| {
            if let Some(parent) = node.parent_id.as_ref() {
                map.entry(parent.clone())
                    .or_default()
                    .push((node.order, node.id.clone()));
            }
            map
        },
    );
    for children in children_by_parent.values_mut() {
        children.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    }
    for node in &mut nodes {
        if let Some(children) = children_by_parent.get(&node.id) {
            node.children = children.iter().map(|(_, child)| child.clone()).collect();
        }
    }
    let mut root_entries = nodes
        .iter()
        .filter(|node| node.parent_id.is_none())
        .map(|node| (node.order, node.id.clone()))
        .collect::<Vec<_>>();
    root_entries.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    let root_node_ids = root_entries
        .into_iter()
        .map(|(_, node_id)| node_id)
        .collect();
    let diagnostics = graph
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic
                .route
                .as_deref()
                .is_none_or(|diagnostic_route| same_preview_route(diagnostic_route, &route))
        })
        .map(|diagnostic| EditorNavigationDiagnostic {
            code: diagnostic.code.clone(),
            message: diagnostic.message.clone(),
            source_node_id: diagnostic.source_node_id.clone(),
        })
        .collect();

    let (focused_view, source_editor_nodes) = match active_document_path {
        Some(active_document_path) => {
            let (view, editor_nodes) = build_editor_navigation_view(
                model,
                &nodes,
                active_document_path,
                preview_context_render_instance_id,
            )?;
            (Some(view), editor_nodes)
        }
        None => (None, Vec::new()),
    };
    Ok(EditorNavigationSnapshot {
        schema_version: EDITOR_NAVIGATION_SCHEMA_VERSION,
        identity,
        model_revision: model.revision.clone(),
        route: route.clone(),
        surface: if route.starts_with("/__pana_workbench/") {
            EditorNavigationSurface::TemplateWorkbench
        } else {
            EditorNavigationSurface::CanonicalPreview
        },
        root_node_ids,
        nodes,
        focused_view,
        diagnostics,
        planning_nodes: source_editor_nodes,
    })
}

struct EditorNavigationViewBuilder<'a> {
    model: &'a ProjectModel,
    template: &'a SourceGraphTemplate,
    source_nodes: HashMap<&'a str, &'a SourceNode>,
    global_nodes_by_source: HashMap<&'a str, Vec<&'a EditorNavigationNode>>,
    markdown_nodes_by_template_source: HashMap<&'a str, Vec<&'a EditorNavigationNode>>,
    view_nodes: Vec<EditorNavigationViewNode>,
    view_ranges: HashMap<String, SourceRange>,
    editor_nodes: Vec<EditorNavigationNode>,
}

impl<'a> EditorNavigationViewBuilder<'a> {
    fn new(
        model: &'a ProjectModel,
        template: &'a SourceGraphTemplate,
        global_nodes: &'a [EditorNavigationNode],
    ) -> Self {
        let source_nodes = model
            .source_graph
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect::<HashMap<_, _>>();
        let mut global_nodes_by_source = HashMap::<&str, Vec<&EditorNavigationNode>>::new();
        for node in global_nodes {
            if let Some(source_node_id) = node.source_node_id.as_deref() {
                global_nodes_by_source
                    .entry(source_node_id)
                    .or_default()
                    .push(node);
            }
        }
        for nodes in global_nodes_by_source.values_mut() {
            nodes.sort_by(|left, right| {
                left.order
                    .cmp(&right.order)
                    .then_with(|| left.id.cmp(&right.id))
            });
        }
        let mut markdown_nodes_by_template_source =
            HashMap::<&str, Vec<&EditorNavigationNode>>::new();
        for node in global_nodes
            .iter()
            .filter(|node| node.kind == EditorNavigationNodeKind::MarkdownBoundary)
        {
            if let Some(template_source_node_id) = node
                .source_provenance
                .composition
                .as_ref()
                .and_then(|reference| reference.source_node_id.as_deref())
            {
                markdown_nodes_by_template_source
                    .entry(template_source_node_id)
                    .or_default()
                    .push(node);
            }
        }
        Self {
            model,
            template,
            source_nodes,
            global_nodes_by_source,
            markdown_nodes_by_template_source,
            view_nodes: Vec::new(),
            view_ranges: HashMap::new(),
            editor_nodes: Vec::new(),
        }
    }

    fn build(
        mut self,
    ) -> (
        Vec<String>,
        Vec<EditorNavigationViewNode>,
        Vec<EditorNavigationNode>,
    ) {
        let mut roots = self
            .source_nodes
            .get(self.template.node_id.as_str())
            .map(|root| root.children.clone())
            .unwrap_or_default();
        self.sort_source_ids(&mut roots);
        for source_node_id in roots {
            self.add_source_node(&source_node_id, None, None);
        }
        let mut root_node_ids = self.rebuild_visual_hierarchy();
        if root_node_ids.is_empty() {
            if let Some(authoring_root_id) = self.add_empty_document_authoring_root() {
                root_node_ids.push(authoring_root_id);
            }
        }
        (root_node_ids, self.view_nodes, self.editor_nodes)
    }

    /// Proiectează rădăcina goală a documentului activ ca suprafață de autor,
    /// nu ca gate Tera. Pentru pagini, ancora este block-ul local; pentru un
    /// fragment deschis direct, ancora este chiar rădăcina Template/Partial.
    /// Straturi primește aceeași identitate Rust pe care o folosește Canvas.
    fn add_empty_document_authoring_root(&mut self) -> Option<String> {
        let candidate = self
            .source_nodes
            .values()
            .copied()
            .filter(|source| {
                source.origin == SourceOrigin::Local
                    && (is_document_wrapper_block(source, self.template, &self.source_nodes)
                        || is_document_fragment_root(source, self.template))
            })
            .filter_map(|source| {
                let matches = self.global_nodes_by_source.get(source.id.as_str())?;
                let representative = matches.iter().copied().find(|node| {
                    node.kind == EditorNavigationNodeKind::TeraBoundary
                        && is_empty_document_authoring_boundary(node, matches)
                        && node.capabilities.requires_edit_scope_id.is_none()
                })?;
                let order = source.range.as_ref().map(|range| range.start).unwrap_or(0);
                Some((
                    order,
                    source.id.clone(),
                    source.clone(),
                    representative.clone(),
                ))
            })
            .min_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)))?;
        let (order, source_id, source, representative) = candidate;
        let view_node_id = format!("editor_view_authoring_root:{source_id}");
        let label = self
            .template
            .file
            .replace('\\', "/")
            .rsplit('/')
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or(self.template.file.as_str())
            .to_string();
        let mut capabilities = representative.capabilities.clone();
        capabilities.can_select = true;
        capabilities.can_inspect = true;
        capabilities.can_enter_boundary = false;
        capabilities.can_move_atomic = false;
        capabilities.can_move = false;
        capabilities.can_edit_text = false;
        capabilities.can_edit_attributes = false;
        capabilities.read_only = false;
        capabilities.requires_edit_scope_id = None;

        self.view_nodes.push(EditorNavigationViewNode {
            id: view_node_id.clone(),
            editor_node_id: Some(representative.id),
            parent_id: None,
            children: Vec::new(),
            order,
            kind: EditorNavigationViewNodeKind::Slot,
            label,
            tag: None,
            source_node_id: Some(source_id),
            source_kind: Some(source.kind),
            file: source.file,
            origin: EditorNavigationOrigin::Project,
            theme_name: None,
            render_instance_ids: representative
                .boundary
                .as_ref()
                .map(|boundary| boundary.root_render_instance_ids.clone())
                .unwrap_or_default(),
            boundary: representative.boundary,
            relation: None,
            capabilities,
        });
        Some(view_node_id)
    }

    fn add_source_node(
        &mut self,
        source_node_id: &str,
        parent_view_id: Option<&str>,
        inherited_scope_id: Option<&str>,
    ) -> Vec<String> {
        let Some(source) = self.source_nodes.get(source_node_id).copied().cloned() else {
            return Vec::new();
        };
        if source.file != self.template.file {
            return Vec::new();
        }
        if source.kind == SourceNodeKind::BlockMarker {
            return Vec::new();
        }
        if let Some(markdown) = self.add_markdown_projection(&source, parent_view_id) {
            return markdown;
        }

        let document_wrapper_block =
            is_document_wrapper_block(&source, self.template, &self.source_nodes);
        if document_wrapper_block || !source_kind_is_visual_layer(&source.kind) {
            let mut children = source.children.clone();
            self.sort_source_ids(&mut children);
            let mut promoted = Vec::new();
            for child_id in children {
                promoted.extend(self.add_source_node(
                    &child_id,
                    parent_view_id,
                    inherited_scope_id,
                ));
            }
            return promoted;
        }

        let view_kind = view_node_kind(&source.kind);
        let view_node_id = editor_view_node_id(&source.id);
        let source_editor_node_id = editor_source_node_id(&source.id);
        let is_relation = matches!(
            source.kind,
            SourceNodeKind::Extends | SourceNodeKind::Import
        );
        let is_gate = source_kind_is_gate(&source.kind);
        let relation = self.navigation_relation(&source);
        let matches = self
            .global_nodes_by_source
            .get(source.id.as_str())
            .cloned()
            .unwrap_or_default();
        let representative = matches.first().copied();
        let mut render_instance_ids = matches
            .iter()
            .flat_map(|node| {
                node.render_instance_id.iter().cloned().chain(
                    node.boundary
                        .iter()
                        .flat_map(|boundary| boundary.root_render_instance_ids.iter().cloned()),
                )
            })
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        render_instance_ids.sort();
        let boundary = view_boundary(self.model, &source, &matches);
        let local_source = source.origin == SourceOrigin::Local;
        let editor_node_id = (!is_relation).then_some(source_editor_node_id.clone());
        let requires_scope_id = inherited_scope_id.map(str::to_string);
        let can_move_atomic = local_source
            && source_kind_is_atomic(&source.kind)
            && !matches!(view_kind, EditorNavigationViewNodeKind::Relation);
        let can_move = match view_kind {
            EditorNavigationViewNodeKind::HtmlElement => {
                local_source && inherited_scope_id.is_none() && source.capabilities.can_move
            }
            EditorNavigationViewNodeKind::Boundary | EditorNavigationViewNodeKind::Slot => {
                can_move_atomic
            }
            EditorNavigationViewNodeKind::Source => {
                local_source && source_kind_is_atomic(&source.kind)
            }
            EditorNavigationViewNodeKind::Relation => false,
        };
        let capabilities = EditorNavigationCapabilities {
            can_select: representative.is_some(),
            can_inspect: representative.is_some_and(|node| node.capabilities.can_inspect),
            can_open_in_code: source.capabilities.can_open_in_code,
            can_enter_boundary: is_gate && local_source,
            can_move_atomic,
            can_move,
            can_edit_text: local_source
                && inherited_scope_id.is_none()
                && source.capabilities.can_edit_text,
            can_edit_attributes: local_source
                && inherited_scope_id.is_none()
                && source.capabilities.can_edit_attributes,
            read_only: !local_source || inherited_scope_id.is_some(),
            requires_edit_scope_id: if is_gate {
                Some(source_editor_node_id.clone())
            } else {
                requires_scope_id.clone()
            },
            reason_code: source.capabilities.reason_code,
        };
        let order = source.range.as_ref().map(|range| range.start).unwrap_or(0);
        let tag = (source.kind == SourceNodeKind::Html)
            .then(|| source_html_tag(&source.label))
            .flatten();
        let display_label = dynamic_widget_navigation_label(self.model, &source.id)
            .unwrap_or_else(|| source.label.clone());
        if let Some(range) = source.range.clone() {
            self.view_ranges.insert(view_node_id.clone(), range);
        }

        self.view_nodes.push(EditorNavigationViewNode {
            id: view_node_id.clone(),
            editor_node_id: editor_node_id.clone(),
            parent_id: parent_view_id.map(str::to_string),
            children: Vec::new(),
            order,
            kind: view_kind,
            label: display_label.clone(),
            tag: tag.clone(),
            source_node_id: Some(source.id.clone()),
            source_kind: Some(source.kind.clone()),
            file: source.file.clone(),
            origin: source_origin(Some(&source)),
            theme_name: source.theme_name.clone(),
            render_instance_ids: render_instance_ids.clone(),
            boundary: boundary.clone(),
            relation,
            capabilities: capabilities.clone(),
        });

        if let Some(editor_node_id) = editor_node_id {
            let component_definition_ids =
                union_editor_ids(&matches, |node| &node.component_definition_ids);
            let component_invocation_ids =
                union_editor_ids(&matches, |node| &node.component_invocation_ids);
            let source_provenance =
                editor_source_provenance(self.model, Some(&source), &component_invocation_ids);
            self.editor_nodes.push(EditorNavigationNode {
                id: editor_node_id,
                parent_id: None,
                children: Vec::new(),
                order,
                kind: if source.kind == SourceNodeKind::Html {
                    EditorNavigationNodeKind::HtmlElement
                } else {
                    EditorNavigationNodeKind::TeraBoundary
                },
                label: display_label,
                tag,
                source_node_id: Some(source.id.clone()),
                render_instance_id: render_instance_ids.first().cloned(),
                source_kind: Some(source.kind.clone()),
                file: Some(source.file.clone()),
                range: source.range.clone(),
                origin: source_origin(Some(&source)),
                theme_name: source.theme_name.clone(),
                source_provenance,
                provenance_stack: representative
                    .map(|node| node.provenance_stack.clone())
                    .unwrap_or_else(|| vec![source.id.clone()]),
                component_definition_ids,
                component_invocation_ids,
                block_definition_ids: union_editor_ids(&matches, |node| &node.block_definition_ids),
                block_source_instance_ids: union_editor_ids(&matches, |node| {
                    &node.block_source_instance_ids
                }),
                dynamic_widget_provider_ids: union_editor_ids(&matches, |node| {
                    &node.dynamic_widget_provider_ids
                }),
                dynamic_widget_source_instance_ids: union_editor_ids(&matches, |node| {
                    &node.dynamic_widget_source_instance_ids
                }),
                binding_key: representative.and_then(|node| node.binding_key.clone()),
                binding_path: representative.and_then(|node| node.binding_path.clone()),
                boundary,
                capabilities,
                source_html_attributes: source_html_attributes(self.model, Some(&source)),
            });
        }

        let child_scope_id = if is_gate {
            Some(source_editor_node_id.as_str())
        } else {
            inherited_scope_id
        };
        let mut children = source.children.clone();
        self.sort_source_ids(&mut children);
        let mut child_view_ids = Vec::new();
        for child_id in children {
            child_view_ids.extend(self.add_source_node(
                &child_id,
                Some(&view_node_id),
                child_scope_id,
            ));
        }
        if let Some(node) = self
            .view_nodes
            .iter_mut()
            .find(|node| node.id == view_node_id)
        {
            node.children = child_view_ids;
        }
        vec![view_node_id]
    }

    fn add_markdown_projection(
        &mut self,
        template_source: &SourceNode,
        parent_view_id: Option<&str>,
    ) -> Option<Vec<String>> {
        let matches = self
            .markdown_nodes_by_template_source
            .get(template_source.id.as_str())?
            .clone();
        let representative = matches.first().copied()?;
        let view_node_id = format!("editor_view_markdown:{}", template_source.id);
        let mut render_instance_ids = matches
            .iter()
            .flat_map(|node| {
                node.boundary
                    .iter()
                    .flat_map(|boundary| boundary.root_render_instance_ids.iter().cloned())
            })
            .collect::<Vec<_>>();
        render_instance_ids.sort();
        render_instance_ids.dedup();
        let mut boundary = representative.boundary.clone();
        if let Some(boundary) = boundary.as_mut() {
            boundary.root_render_instance_ids = render_instance_ids.clone();
            boundary.rendered_instance_count = matches.len();
        }
        let order = template_source
            .range
            .as_ref()
            .map(|range| range.start)
            .unwrap_or(representative.order);
        if let Some(range) = template_source.range.clone() {
            self.view_ranges.insert(view_node_id.clone(), range);
        }
        self.view_nodes.push(EditorNavigationViewNode {
            id: view_node_id.clone(),
            editor_node_id: Some(representative.id.clone()),
            parent_id: parent_view_id.map(str::to_string),
            children: Vec::new(),
            order,
            kind: EditorNavigationViewNodeKind::Boundary,
            label: representative.label.clone(),
            tag: None,
            source_node_id: representative.source_node_id.clone(),
            source_kind: representative.source_kind.clone(),
            file: representative
                .file
                .clone()
                .unwrap_or_else(|| template_source.file.clone()),
            origin: representative.origin,
            theme_name: representative.theme_name.clone(),
            render_instance_ids,
            boundary,
            relation: None,
            capabilities: representative.capabilities.clone(),
        });
        Some(vec![view_node_id])
    }

    fn sort_source_ids(&self, source_ids: &mut [String]) {
        source_ids.sort_by(|left, right| {
            let left_node = self.source_nodes.get(left.as_str()).copied();
            let right_node = self.source_nodes.get(right.as_str()).copied();
            source_order(left_node)
                .cmp(&source_order(right_node))
                .then_with(|| left.cmp(right))
        });
    }

    fn rebuild_visual_hierarchy(&mut self) -> Vec<String> {
        let parent_by_id = self
            .view_nodes
            .iter()
            .filter_map(|node| {
                let range = self.view_ranges.get(&node.id)?;
                let parent = self
                    .view_nodes
                    .iter()
                    .filter(|candidate| {
                        candidate.id != node.id
                            && matches!(
                                candidate.kind,
                                EditorNavigationViewNodeKind::HtmlElement
                                    | EditorNavigationViewNodeKind::Boundary
                            )
                    })
                    .filter_map(|candidate| {
                        let candidate_range = self.view_ranges.get(&candidate.id)?;
                        (candidate_range.start <= range.start
                            && range.end <= candidate_range.end
                            && (candidate_range.start < range.start
                                || range.end < candidate_range.end))
                            .then_some((
                                candidate_range.end.saturating_sub(candidate_range.start),
                                candidate.order,
                                candidate.id.clone(),
                            ))
                    })
                    .min_by(|left, right| {
                        left.0
                            .cmp(&right.0)
                            .then_with(|| right.1.cmp(&left.1))
                            .then_with(|| left.2.cmp(&right.2))
                    })
                    .map(|(_, _, id)| id);
                Some((node.id.clone(), parent))
            })
            .collect::<HashMap<_, _>>();

        for node in &mut self.view_nodes {
            if let Some(parent) = parent_by_id.get(&node.id) {
                node.parent_id = parent.clone();
            }
            node.children.clear();
        }

        let mut children_by_parent = HashMap::<String, Vec<(usize, String)>>::new();
        for node in &self.view_nodes {
            if let Some(parent_id) = node.parent_id.as_ref() {
                children_by_parent
                    .entry(parent_id.clone())
                    .or_default()
                    .push((node.order, node.id.clone()));
            }
        }
        for children in children_by_parent.values_mut() {
            children.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
        }
        for node in &mut self.view_nodes {
            if let Some(children) = children_by_parent.get(&node.id) {
                node.children = children.iter().map(|(_, child)| child.clone()).collect();
            }
        }

        let mut roots = self
            .view_nodes
            .iter()
            .filter(|node| node.parent_id.is_none())
            .map(|node| (node.order, node.id.clone()))
            .collect::<Vec<_>>();
        roots.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
        roots.into_iter().map(|(_, id)| id).collect()
    }

    fn navigation_relation(&self, source: &SourceNode) -> Option<EditorNavigationRelation> {
        let relation_kind = match source.kind {
            SourceNodeKind::Extends => EditorNavigationRelationKind::Extends,
            SourceNodeKind::Include => EditorNavigationRelationKind::Include,
            SourceNodeKind::Import => EditorNavigationRelationKind::Import,
            SourceNodeKind::Block => {
                return self.block_override_relation(source);
            }
            _ => return None,
        };
        let target_template_name = self
            .model
            .tera_graph
            .nodes
            .iter()
            .find(|node| node.id == source.id)
            .and_then(|node| node.target.clone())
            .or_else(|| match source.kind {
                SourceNodeKind::Extends => self.template.extends.clone(),
                _ => None,
            });
        let target = resolved_template_target(
            self.model,
            self.template,
            source_relation_kind(&source.kind)?,
            target_template_name.as_deref(),
        );
        Some(EditorNavigationRelation {
            kind: relation_kind,
            target_document_path: target.map(|template| template.file.clone()),
            target_source_node_id: target.map(|template| template.node_id.clone()),
            target_template_name: target_template_name
                .or_else(|| target.map(|template| template.name.clone())),
        })
    }

    fn block_override_relation(&self, source: &SourceNode) -> Option<EditorNavigationRelation> {
        let relation = self.model.source_graph.relations.iter().find(|relation| {
            relation.from == source.id && relation.kind == SourceRelationKind::OverridesBlock
        })?;
        let target_node = self.source_nodes.get(relation.to.as_str()).copied();
        let target_template = target_node.and_then(|node| {
            self.model
                .source_graph
                .templates
                .iter()
                .find(|template| template.file == node.file)
        });
        Some(EditorNavigationRelation {
            kind: EditorNavigationRelationKind::BlockOverride,
            target_document_path: target_template.map(|template| template.file.clone()),
            target_source_node_id: Some(relation.to.clone()),
            target_template_name: target_template.map(|template| template.name.clone()),
        })
    }
}

fn is_empty_document_authoring_boundary(
    boundary_node: &EditorNavigationNode,
    source_matches: &[&EditorNavigationNode],
) -> bool {
    let Some(boundary) = boundary_node.boundary.as_ref() else {
        return false;
    };
    if boundary.empty {
        return true;
    }
    !boundary.root_render_instance_ids.is_empty()
        && boundary
            .root_render_instance_ids
            .iter()
            .all(|render_instance_id| {
                let render_node_id = format!("editor_render:{render_instance_id}");
                source_matches.iter().copied().any(|render_node| {
                    render_node.id == render_node_id
                        && render_node.parent_id.as_deref() == Some(boundary_node.id.as_str())
                        && render_node.source_node_id == boundary_node.source_node_id
                        && render_node.source_kind == Some(SourceNodeKind::Block)
                        && render_node.tag.as_deref() == Some("div")
                })
            })
}

fn build_editor_navigation_view(
    model: &ProjectModel,
    global_nodes: &[EditorNavigationNode],
    active_document_path: &str,
    preview_context_render_instance_id: Option<&str>,
) -> Result<(EditorNavigationView, Vec<EditorNavigationNode>), String> {
    let active_document_path = normalize_editor_document_path(active_document_path)?;
    let template = model
        .source_graph
        .templates
        .iter()
        .find(|template| same_editor_document_path(&template.file, &active_document_path))
        .ok_or_else(|| {
            format!(
                "EditorNavigationView nu găsește documentul activ {active_document_path:?} în SourceGraph."
            )
        })?;
    let breadcrumbs = editor_navigation_breadcrumbs(model, template);
    let builder = EditorNavigationViewBuilder::new(model, template, global_nodes);
    let (root_node_ids, nodes, editor_nodes) = builder.build();
    let preview_context_render_instance_id = preview_context_render_instance_id
        .filter(|render_instance_id| {
            global_nodes.iter().any(|node| {
                node.render_instance_id.as_deref() == Some(*render_instance_id)
                    || node.boundary.as_ref().is_some_and(|boundary| {
                        boundary
                            .root_render_instance_ids
                            .iter()
                            .any(|candidate| candidate == render_instance_id)
                    })
            })
        })
        .map(str::to_string);
    Ok((
        EditorNavigationView {
            active_document_path,
            active_template_name: template.name.clone(),
            active_source_node_id: template.node_id.clone(),
            breadcrumbs,
            root_node_ids,
            nodes,
            preview_context_render_instance_id,
        },
        editor_nodes,
    ))
}

fn editor_navigation_breadcrumbs(
    model: &ProjectModel,
    active: &SourceGraphTemplate,
) -> Vec<EditorNavigationBreadcrumb> {
    let mut chain = vec![active];
    let mut current = active;
    let mut visited = HashSet::from([active.node_id.as_str()]);
    while let Some(parent) = resolved_template_target(
        model,
        current,
        SourceRelationKind::Extends,
        current.extends.as_deref(),
    ) {
        if !visited.insert(parent.node_id.as_str()) {
            break;
        }
        chain.push(parent);
        current = parent;
    }
    chain.reverse();
    chain
        .into_iter()
        .map(|template| EditorNavigationBreadcrumb {
            document_path: template.file.clone(),
            template_name: template.name.clone(),
            source_node_id: template.node_id.clone(),
            origin: match template.origin {
                SourceOrigin::Local => EditorNavigationOrigin::Project,
                SourceOrigin::Theme => EditorNavigationOrigin::Theme,
            },
            theme_name: template.theme_name.clone(),
            current: template.node_id == active.node_id,
        })
        .collect()
}

fn resolved_template_target<'a>(
    model: &'a ProjectModel,
    from: &SourceGraphTemplate,
    kind: SourceRelationKind,
    target_name: Option<&str>,
) -> Option<&'a SourceGraphTemplate> {
    let target_name = target_name.map(normalized_template_name);
    let relation = model.source_graph.relations.iter().find(|relation| {
        relation.from == from.node_id
            && relation.kind == kind
            && target_name
                .as_ref()
                .is_none_or(|target| normalized_template_name(&relation.label) == *target)
    })?;
    model
        .source_graph
        .templates
        .iter()
        .find(|template| template.node_id == relation.to)
}

fn source_relation_kind(kind: &SourceNodeKind) -> Option<SourceRelationKind> {
    match kind {
        SourceNodeKind::Extends => Some(SourceRelationKind::Extends),
        SourceNodeKind::Include => Some(SourceRelationKind::Includes),
        SourceNodeKind::Import => Some(SourceRelationKind::Imports),
        _ => None,
    }
}

fn view_boundary(
    model: &ProjectModel,
    source: &SourceNode,
    matching_nodes: &[&EditorNavigationNode],
) -> Option<EditorNavigationBoundary> {
    if source.kind == SourceNodeKind::Html
        || matches!(
            source.kind,
            SourceNodeKind::Extends | SourceNodeKind::Import | SourceNodeKind::BlockMarker
        )
    {
        return None;
    }
    let boundaries = matching_nodes
        .iter()
        .filter_map(|node| node.boundary.as_ref())
        .collect::<Vec<_>>();
    let mut roots = boundaries
        .iter()
        .flat_map(|boundary| boundary.root_render_instance_ids.iter().cloned())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    roots.sort();
    let target = model
        .tera_graph
        .nodes
        .iter()
        .find(|node| node.id == source.id)
        .and_then(|node| node.target.clone());
    let effect_scope = match source.kind {
        SourceNodeKind::Include | SourceNodeKind::Macro => {
            EditorNavigationEffectScope::SharedDefinition
        }
        _ => boundary_effect_scope(Some(&source.kind)),
    };
    let rendered_instance_count = if source.kind == SourceNodeKind::Include {
        target
            .as_deref()
            .map(|target| include_consumer_count(model, target))
            .unwrap_or(boundaries.len())
            .max(boundaries.len())
    } else {
        boundaries.len()
    };
    Some(EditorNavigationBoundary {
        boundary_instance_id: boundaries
            .first()
            .map(|boundary| boundary.boundary_instance_id.clone())
            .unwrap_or_else(|| format!("source_boundary:{}", source.id)),
        source_node_id: source.id.clone(),
        root_render_instance_ids: roots.clone(),
        atomic_when_closed: true,
        effect_scope,
        rendered_instance_count,
        target,
        empty: roots.is_empty(),
    })
}

fn include_consumer_count(model: &ProjectModel, target: &str) -> usize {
    let target = normalized_template_name(target);
    model
        .source_graph
        .templates
        .iter()
        .flat_map(|template| template.include_groups.iter())
        .filter(|group| {
            group
                .targets
                .iter()
                .any(|candidate| normalized_template_name(candidate) == target)
        })
        .count()
}

fn union_editor_ids(
    nodes: &[&EditorNavigationNode],
    values: impl Fn(&EditorNavigationNode) -> &[String],
) -> Vec<String> {
    let mut result = nodes
        .iter()
        .flat_map(|node| values(node).iter().cloned())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    result.sort();
    result
}

fn dynamic_widget_navigation_label(model: &ProjectModel, source_node_id: &str) -> Option<String> {
    let instance = model
        .source_graph
        .dynamic_widget_graph
        .source_instances
        .iter()
        .rev()
        .find(|instance| {
            instance
                .root_source_node_ids
                .iter()
                .any(|candidate| candidate == source_node_id)
        })?;
    match instance.properties.as_ref()? {
        DynamicWidgetProperties::DynamicField(properties) => {
            let label = model
                .source_graph
                .dynamic_widget_graph
                .value_catalog
                .iter()
                .find(|definition| {
                    definition.source == properties.binding.source
                        && definition.contexts.contains(&properties.binding.context)
                })
                .map(|definition| definition.label.as_str())
                .filter(|label| !label.trim().is_empty())
                .unwrap_or(properties.label.as_str());
            Some(if label.trim().is_empty() {
                "Câmp dinamic".to_string()
            } else {
                format!("Câmp dinamic · {label}")
            })
        }
        DynamicWidgetProperties::Listing(properties) => {
            Some(format!("Listing · {}", properties.listing_item_template))
        }
    }
}

fn view_node_kind(kind: &SourceNodeKind) -> EditorNavigationViewNodeKind {
    match kind {
        SourceNodeKind::Html => EditorNavigationViewNodeKind::HtmlElement,
        SourceNodeKind::Extends | SourceNodeKind::Import => EditorNavigationViewNodeKind::Relation,
        kind if source_kind_is_gate(kind) => EditorNavigationViewNodeKind::Boundary,
        _ => EditorNavigationViewNodeKind::Source,
    }
}

fn source_kind_is_visual_layer(kind: &SourceNodeKind) -> bool {
    *kind == SourceNodeKind::Html || source_kind_is_gate(kind)
}

fn source_kind_is_gate(kind: &SourceNodeKind) -> bool {
    matches!(
        kind,
        SourceNodeKind::Block
            | SourceNodeKind::Include
            | SourceNodeKind::Macro
            | SourceNodeKind::For
            | SourceNodeKind::If
            | SourceNodeKind::Filter
            | SourceNodeKind::Raw
    )
}

fn source_kind_is_atomic(kind: &SourceNodeKind) -> bool {
    matches!(
        kind,
        SourceNodeKind::Block
            | SourceNodeKind::Include
            | SourceNodeKind::Import
            | SourceNodeKind::Macro
            | SourceNodeKind::For
            | SourceNodeKind::If
            | SourceNodeKind::Set
            | SourceNodeKind::SetGlobal
            | SourceNodeKind::Filter
            | SourceNodeKind::Break
            | SourceNodeKind::Continue
            | SourceNodeKind::Super
            | SourceNodeKind::TeraVariable
            | SourceNodeKind::TeraComment
            | SourceNodeKind::Raw
            | SourceNodeKind::Tera
    )
}

fn source_order(source: Option<&SourceNode>) -> usize {
    source
        .and_then(|source| source.range.as_ref())
        .map(|range| range.start)
        .unwrap_or(usize::MAX)
}

fn source_html_tag(label: &str) -> Option<String> {
    label
        .strip_prefix('<')
        .and_then(|label| label.split([' ', '>', '.']).next())
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
}

fn editor_view_node_id(source_node_id: &str) -> String {
    format!("editor_view:{source_node_id}")
}

fn editor_source_node_id(source_node_id: &str) -> String {
    format!("editor_source:{source_node_id}")
}

fn normalized_template_name(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_string()
}

fn normalize_editor_document_path(value: &str) -> Result<String, String> {
    let value = value.trim().replace('\\', "/");
    if value.is_empty()
        || value.len() > 2_048
        || value.starts_with('/')
        || value
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(
            "EditorNavigationView a refuzat calea documentului activ deoarece este invalidă."
                .to_string(),
        );
    }
    Ok(value)
}

fn same_editor_document_path(left: &str, right: &str) -> bool {
    left.trim_start_matches('/').replace('\\', "/")
        == right.trim_start_matches('/').replace('\\', "/")
}

fn same_preview_route(left: &str, right: &str) -> bool {
    normalize_preview_route(left) == normalize_preview_route(right)
}

fn normalize_preview_route(route: &str) -> String {
    let route = route.split(['?', '#']).next().unwrap_or(route).trim();
    let mut normalized = if route.is_empty() {
        "/".to_string()
    } else if route.starts_with('/') {
        route.to_string()
    } else {
        format!("/{route}")
    };
    if normalized.len() > 1 && normalized.ends_with("/index.html") {
        normalized.truncate(normalized.len() - "index.html".len());
    }
    normalized
}

fn editor_boundary_node_id(boundary_instance_id: &str) -> String {
    format!("editor_boundary:{boundary_instance_id}")
}

fn editor_render_node_id(render_instance_id: &str) -> String {
    format!("editor_render:{render_instance_id}")
}

fn boundary_depth(
    boundary: &CanvasBoundaryInstance,
    boundaries: &HashMap<&str, &CanvasBoundaryInstance>,
) -> usize {
    let mut depth = 0usize;
    let mut cursor = boundary.parent_boundary_instance_id.as_deref();
    let mut visited = HashSet::new();
    while let Some(parent) = cursor {
        if !visited.insert(parent) {
            break;
        }
        let Some(boundary) = boundaries.get(parent) else {
            break;
        };
        depth = depth.saturating_add(1);
        cursor = boundary.parent_boundary_instance_id.as_deref();
    }
    depth
}

fn implicitly_open_document_boundaries(
    model: &ProjectModel,
    active_document_path: Option<&str>,
    source_nodes: &HashMap<&str, &SourceNode>,
) -> HashSet<String> {
    let Some(active_document_path) = active_document_path else {
        return HashSet::new();
    };
    let Some(template) = model
        .source_graph
        .templates
        .iter()
        .find(|template| same_editor_document_path(&template.file, active_document_path))
    else {
        return HashSet::new();
    };

    source_nodes
        .values()
        .filter(|source| is_document_wrapper_block(source, template, source_nodes))
        .map(|source| source.id.clone())
        .collect()
}

fn is_document_wrapper_block(
    source: &SourceNode,
    template: &SourceGraphTemplate,
    source_nodes: &HashMap<&str, &SourceNode>,
) -> bool {
    source.kind == SourceNodeKind::Block
        && source.file == template.file
        && source.parent.as_deref() == Some(template.node_id.as_str())
        && !source_is_inside_html(source, source_nodes)
}

fn is_document_fragment_root(source: &SourceNode, template: &SourceGraphTemplate) -> bool {
    source.id == template.node_id
        && source.file == template.file
        && source.parent.is_none()
        && matches!(
            source.kind,
            SourceNodeKind::Template | SourceNodeKind::Partial
        )
}

fn source_is_inside_html(source: &SourceNode, source_nodes: &HashMap<&str, &SourceNode>) -> bool {
    let Some(source_range) = source.range.as_ref() else {
        return false;
    };
    source_nodes.values().any(|candidate| {
        candidate.file == source.file
            && candidate.kind == SourceNodeKind::Html
            && candidate.range.as_ref().is_some_and(|candidate_range| {
                candidate_range.start < source_range.start && source_range.end < candidate_range.end
            })
    })
}

fn primary_source_node<'a>(
    render_node: &CanvasRenderNode,
    source_nodes: &HashMap<&str, &'a SourceNode>,
) -> Option<&'a SourceNode> {
    render_node
        .source_node_id
        .as_deref()
        .or(render_node.template_source_node_id.as_deref())
        .and_then(|source_id| source_nodes.get(source_id).copied())
        .or_else(|| {
            render_node
                .provenance_stack
                .iter()
                .rev()
                .find_map(|source_id| source_nodes.get(source_id.as_str()).copied())
        })
}

fn source_html_attributes(
    model: &ProjectModel,
    source: Option<&SourceNode>,
) -> Option<BTreeMap<String, Option<String>>> {
    let source = source.filter(|source| source.kind == SourceNodeKind::Html)?;
    let range = source.range.as_ref()?;
    let file = model
        .files
        .iter()
        .find(|file| file.relative_path == source.file)?;
    let opening = parse_html_tag_at(&file.contents, range.start)?;
    if opening.is_closing || opening.start != range.start {
        return None;
    }
    let opening_source = file.contents.get(opening.start..opening.end)?;
    Some(
        raw_tag_attributes(opening_source)
            .into_iter()
            .map(|attribute| (attribute.name, attribute.value))
            .collect(),
    )
}

fn editable_boundary_kind(kind: &SourceNodeKind) -> bool {
    matches!(
        kind,
        SourceNodeKind::Block
            | SourceNodeKind::Include
            | SourceNodeKind::Macro
            | SourceNodeKind::For
            | SourceNodeKind::If
            | SourceNodeKind::Filter
    )
}

fn movable_boundary_kind(kind: &SourceNodeKind) -> bool {
    matches!(
        kind,
        SourceNodeKind::Block
            | SourceNodeKind::Include
            | SourceNodeKind::Macro
            | SourceNodeKind::For
            | SourceNodeKind::If
            | SourceNodeKind::Filter
    )
}

fn boundary_effect_scope(kind: Option<&SourceNodeKind>) -> EditorNavigationEffectScope {
    match kind {
        Some(
            SourceNodeKind::Include
            | SourceNodeKind::Macro
            | SourceNodeKind::For
            | SourceNodeKind::If,
        ) => EditorNavigationEffectScope::AllRenderedInstances,
        Some(SourceNodeKind::Block) => EditorNavigationEffectScope::SharedDefinition,
        _ => EditorNavigationEffectScope::SingleSource,
    }
}

fn source_origin(source: Option<&SourceNode>) -> EditorNavigationOrigin {
    match source.map(|source| &source.origin) {
        Some(SourceOrigin::Theme) => EditorNavigationOrigin::Theme,
        Some(SourceOrigin::Local) => EditorNavigationOrigin::Project,
        None => EditorNavigationOrigin::Tera,
    }
}

fn render_origin(
    render_node: &CanvasRenderNode,
    source: Option<&SourceNode>,
) -> EditorNavigationOrigin {
    if let Some(source) = source {
        return source_origin(Some(source));
    }
    match render_node.origin {
        CanvasNodeOrigin::Source => EditorNavigationOrigin::Project,
        CanvasNodeOrigin::Tera => EditorNavigationOrigin::Tera,
        CanvasNodeOrigin::PanaRuntime => EditorNavigationOrigin::PanaRuntime,
        CanvasNodeOrigin::ArbitraryJsRuntime => EditorNavigationOrigin::ArbitraryRuntime,
    }
}

fn random_scope_token(
    identity: &CanvasProjectionIdentity,
    scope_id: &str,
) -> Result<String, String> {
    let mut entropy = [0u8; 32];
    getrandom::fill(&mut entropy)
        .map_err(|error| format!("EditScopeGrant nu poate genera un token sigur: {error}"))?;
    let mut hasher = Sha256::new();
    hasher.update(b"pana-edit-scope");
    hasher.update([0]);
    hasher.update(entropy);
    hasher.update([0]);
    hasher.update(identity.project_root.as_bytes());
    hasher.update([0]);
    hasher.update(identity.runtime_session_id.as_bytes());
    hasher.update([0]);
    hasher.update(identity.workspace_revision.to_le_bytes());
    hasher.update([0]);
    hasher.update(identity.transaction_id.as_bytes());
    hasher.update([0]);
    hasher.update(scope_id.as_bytes());
    Ok(format!("edit_scope_{}", full_hex(&hasher.finalize())))
}

fn random_move_plan_token(
    identity: &CanvasProjectionIdentity,
    source_node_id: &str,
    target_node_id: &str,
) -> Result<String, String> {
    let mut entropy = [0u8; 32];
    getrandom::fill(&mut entropy)
        .map_err(|error| format!("PlanEditorMove nu poate genera un token sigur: {error}"))?;
    let mut hasher = Sha256::new();
    hasher.update(b"pana-editor-move-plan");
    hasher.update([0]);
    hasher.update(entropy);
    hasher.update([0]);
    hasher.update(identity.project_root.as_bytes());
    hasher.update([0]);
    hasher.update(identity.runtime_session_id.as_bytes());
    hasher.update([0]);
    hasher.update(identity.workspace_revision.to_le_bytes());
    hasher.update([0]);
    hasher.update(identity.transaction_id.as_bytes());
    hasher.update([0]);
    hasher.update(source_node_id.as_bytes());
    hasher.update([0]);
    hasher.update(target_node_id.as_bytes());
    Ok(format!("editor_move_{}", full_hex(&hasher.finalize())))
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
pub(crate) fn editor_navigation_snapshot_for_test(
    identity: CanvasProjectionIdentity,
    model_revision: &str,
    route: &str,
    surface: EditorNavigationSurface,
    root_node_ids: Vec<String>,
    nodes: Vec<EditorNavigationNode>,
) -> EditorNavigationSnapshot {
    EditorNavigationSnapshot {
        schema_version: EDITOR_NAVIGATION_SCHEMA_VERSION,
        identity,
        model_revision: model_revision.to_string(),
        route: route.to_string(),
        surface,
        root_node_ids,
        nodes,
        focused_view: None,
        diagnostics: Vec::new(),
        planning_nodes: Vec::new(),
    }
}

fn full_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        preview::{CanvasBoundaryMarkerKind, CanvasDocumentGraph, CanvasNodeCapabilities},
        project_model::test_support::ProjectModelTestFixture,
        source_graph::model::MarkdownProjectionKind,
    };

    use super::*;

    fn canvas_identity(workspace_revision: u64) -> CanvasProjectionIdentity {
        CanvasProjectionIdentity {
            project_root: "/project".to_string(),
            runtime_session_id: "runtime-1".to_string(),
            workspace_revision,
            transaction_id: "canvas-1".to_string(),
            preview_revision: "preview-1".to_string(),
        }
    }

    fn test_source_provenance(
        source_node_id: &str,
        file: &str,
        source_kind: SourceNodeKind,
    ) -> EditorSourceProvenance {
        EditorSourceProvenance {
            definition: Some(EditorSourceReference {
                source_node_id: Some(source_node_id.to_string()),
                source_kind: Some(source_kind),
                file: file.to_string(),
                range: None,
                label: source_node_id.to_string(),
                origin: EditorNavigationOrigin::Project,
                theme_name: None,
                can_open_in_code: true,
            }),
            composition: None,
            resolution: EditorSourceResolution::Direct,
        }
    }

    fn editable_boundary_node() -> EditorNavigationNode {
        EditorNavigationNode {
            id: "editor_boundary:boundary-1".to_string(),
            parent_id: None,
            children: Vec::new(),
            order: 0,
            kind: EditorNavigationNodeKind::TeraBoundary,
            label: "content".to_string(),
            tag: None,
            source_node_id: Some("source-block-1".to_string()),
            render_instance_id: None,
            source_kind: Some(SourceNodeKind::Block),
            file: Some("templates/index.html".to_string()),
            range: None,
            origin: EditorNavigationOrigin::Project,
            theme_name: None,
            source_provenance: test_source_provenance(
                "source-block-1",
                "templates/index.html",
                SourceNodeKind::Block,
            ),
            provenance_stack: Vec::new(),
            component_definition_ids: Vec::new(),
            component_invocation_ids: Vec::new(),
            block_definition_ids: Vec::new(),
            block_source_instance_ids: Vec::new(),
            dynamic_widget_provider_ids: Vec::new(),
            dynamic_widget_source_instance_ids: Vec::new(),
            binding_key: None,
            binding_path: None,
            boundary: Some(EditorNavigationBoundary {
                boundary_instance_id: "boundary-1".to_string(),
                source_node_id: "source-block-1".to_string(),
                root_render_instance_ids: vec!["render-1".to_string()],
                atomic_when_closed: true,
                effect_scope: EditorNavigationEffectScope::SharedDefinition,
                rendered_instance_count: 1,
                target: None,
                empty: false,
            }),
            capabilities: EditorNavigationCapabilities {
                can_select: true,
                can_inspect: true,
                can_open_in_code: true,
                can_enter_boundary: true,
                can_move_atomic: true,
                can_move: true,
                can_edit_text: false,
                can_edit_attributes: false,
                read_only: false,
                requires_edit_scope_id: Some("editor_boundary:boundary-1".to_string()),
                reason_code: Some(SourceCapabilityReason::TeraBlock),
            },
            source_html_attributes: None,
        }
    }

    #[test]
    fn route_normalization_preserves_workbench_and_accepts_index_alias() {
        assert!(same_preview_route("/", "index.html"));
        assert!(same_preview_route("/blog/", "/blog/index.html"));
        assert!(same_preview_route(
            "/__pana_workbench/source/",
            "/__pana_workbench/source/?revision=1"
        ));
        assert!(!same_preview_route("/blog/", "/contact/"));
    }

    #[test]
    fn edit_scope_grant_is_exact_and_removed_after_stale_use() {
        let runtime = EditorNavigationRuntime::default();
        let identity = canvas_identity(7);
        let node = editable_boundary_node();
        let grant = runtime
            .issue_edit_scope_grant(&identity, "model-7", "/", "templates/index.html", &node)
            .unwrap();

        assert!(runtime
            .require_edit_scope_grant(
                &grant,
                &identity,
                "model-7",
                "/",
                "templates/index.html",
                &node.id,
                EditScopeOperation::MoveHtmlInside,
            )
            .is_ok());
        assert!(runtime
            .require_edit_scope_grant(
                &grant,
                &canvas_identity(8),
                "model-8",
                "/",
                "templates/index.html",
                &node.id,
                EditScopeOperation::MoveHtmlInside,
            )
            .is_err());
        assert!(runtime
            .require_edit_scope_grant(
                &grant,
                &identity,
                "model-7",
                "/",
                "templates/index.html",
                &node.id,
                EditScopeOperation::MoveHtmlInside,
            )
            .is_err());
    }

    #[test]
    fn editor_move_plan_token_is_single_use_and_revision_bound() {
        let runtime = EditorNavigationRuntime::default();
        let identity = canvas_identity(11);
        let plan = runtime
            .issue_editor_move_plan(EditorMovePlan {
                schema_version: EDITOR_MOVE_PLAN_SCHEMA_VERSION,
                token: None,
                allowed: true,
                reason_code: None,
                reason: None,
                operation: Some(EditorMoveOperation::HtmlSourceMove),
                identity: identity.clone(),
                model_revision: "model-11".to_string(),
                route: "/".to_string(),
                active_document_path: "templates/index.html".to_string(),
                source_node_id: "editor_render:source".to_string(),
                target_node_id: "editor_render:target".to_string(),
                position: ProjectMovePosition::After,
                impact: EditorMoveImpact {
                    files: vec!["templates/index.html".to_string()],
                    edit_scope_id: None,
                    effect_scope: EditorNavigationEffectScope::SingleSource,
                    rendered_instance_count: 1,
                    affects_all_rendered_instances: false,
                    requires_preview_reprojection: true,
                },
                live_projection: None,
                live_projection_reason: EditorMoveLiveProjectionReason::ExecutionNotHtml,
                issued_at_ms: 0,
            })
            .unwrap();
        let token = plan.token.clone().unwrap();

        assert!(runtime
            .consume_editor_move_plan(&token, &identity, "model-11", "/", "templates/index.html",)
            .is_ok());
        assert!(runtime
            .consume_editor_move_plan(&token, &identity, "model-11", "/", "templates/index.html",)
            .is_err());

        let stale = runtime.issue_editor_move_plan(plan).unwrap();
        assert!(runtime
            .consume_editor_move_plan(
                stale.token.as_deref().unwrap(),
                &canvas_identity(12),
                "model-12",
                "/",
                "templates/index.html",
            )
            .is_err());
    }

    #[test]
    fn central_planner_keeps_closed_tera_atomic_and_requires_scope_for_html_children() {
        let root = editor_navigation_test_project("central-planner");
        let model = editor_navigation_test_model(&root);
        let loop_node = source_node(&model, SourceNodeKind::For, "for");
        let article = source_node(&model, SourceNodeKind::Html, "<article");
        let section = source_node(&model, SourceNodeKind::Html, "<section");
        let footer = source_node(&model, SourceNodeKind::Html, "<footer");
        let block = source_node(&model, SourceNodeKind::Block, "sidebar");
        let identity = CanvasProjectionIdentity {
            project_root: root.to_string_lossy().to_string(),
            runtime_session_id: "runtime-central-planner".to_string(),
            workspace_revision: 9,
            transaction_id: "canvas-central-planner".to_string(),
            preview_revision: "preview-central-planner".to_string(),
        };
        let scope_id = "editor_boundary:loop-instance".to_string();
        let mut boundary = editable_boundary_node();
        boundary.id = scope_id.clone();
        boundary.label = loop_node.label.clone();
        boundary.source_node_id = Some(loop_node.id.clone());
        boundary.source_kind = Some(loop_node.kind.clone());
        boundary.file = Some(loop_node.file.clone());
        boundary.range = loop_node.range.clone();
        boundary.children = vec![
            "editor_render:section-instance".to_string(),
            "editor_render:article-instance".to_string(),
        ];
        boundary.capabilities.requires_edit_scope_id = Some(scope_id.clone());
        boundary.capabilities.reason_code = loop_node.capabilities.reason_code;
        let semantic_boundary = boundary.boundary.as_mut().unwrap();
        semantic_boundary.boundary_instance_id = "loop-instance".to_string();
        semantic_boundary.source_node_id = loop_node.id.clone();
        semantic_boundary.effect_scope = EditorNavigationEffectScope::AllRenderedInstances;
        semantic_boundary.rendered_instance_count = 2;
        let mut block_boundary = editable_boundary_node();
        block_boundary.id = "editor_boundary:block-instance".to_string();
        block_boundary.label = block.label.clone();
        block_boundary.source_node_id = Some(block.id.clone());
        block_boundary.source_kind = Some(block.kind.clone());
        block_boundary.file = Some(block.file.clone());
        block_boundary.range = block.range.clone();
        block_boundary.capabilities.requires_edit_scope_id = Some(block_boundary.id.clone());
        block_boundary.capabilities.reason_code = block.capabilities.reason_code;
        let semantic_block = block_boundary.boundary.as_mut().unwrap();
        semantic_block.boundary_instance_id = "block-instance".to_string();
        semantic_block.source_node_id = block.id.clone();
        semantic_block.effect_scope = EditorNavigationEffectScope::SharedDefinition;

        let section_node = editor_html_node(section, "section-instance", Some(scope_id.clone()), 1);
        let article_node = editor_html_node(article, "article-instance", Some(scope_id.clone()), 2);
        let footer_node = editor_html_node(footer, "footer-instance", None, 3);
        let snapshot = EditorNavigationSnapshot {
            schema_version: EDITOR_NAVIGATION_SCHEMA_VERSION,
            identity: identity.clone(),
            model_revision: model.revision.clone(),
            route: "/".to_string(),
            surface: EditorNavigationSurface::CanonicalPreview,
            root_node_ids: vec![
                scope_id.clone(),
                footer_node.id.clone(),
                block_boundary.id.clone(),
            ],
            nodes: vec![
                boundary.clone(),
                section_node.clone(),
                article_node.clone(),
                footer_node.clone(),
                block_boundary.clone(),
            ],
            focused_view: Some(EditorNavigationView {
                active_document_path: "templates/index.html".to_string(),
                active_template_name: "index.html".to_string(),
                active_source_node_id: "template:index".to_string(),
                breadcrumbs: Vec::new(),
                root_node_ids: Vec::new(),
                nodes: Vec::new(),
                preview_context_render_instance_id: None,
            }),
            diagnostics: Vec::new(),
            planning_nodes: Vec::new(),
        };
        let runtime = EditorNavigationRuntime::default();

        let atomic = plan_editor_move(
            &runtime,
            &snapshot,
            &model,
            &boundary.id,
            &footer_node.id,
            ProjectMovePosition::After,
            None,
        );
        assert!(atomic.plan.allowed, "{:?}", atomic.plan.reason);
        assert_eq!(
            atomic.plan.operation,
            Some(EditorMoveOperation::AtomicTeraMove)
        );
        assert!(atomic.plan.live_projection.is_none());
        assert_eq!(
            atomic.plan.live_projection_reason,
            EditorMoveLiveProjectionReason::ExecutionNotHtml
        );
        assert_eq!(
            atomic.plan.impact.effect_scope,
            EditorNavigationEffectScope::AllRenderedInstances
        );
        assert!(atomic.plan.impact.affects_all_rendered_instances);

        let nested_atomic = plan_editor_move(
            &runtime,
            &snapshot,
            &model,
            &boundary.id,
            &block_boundary.id,
            ProjectMovePosition::Inside,
            None,
        );
        assert!(
            nested_atomic.plan.allowed,
            "{:?}",
            nested_atomic.plan.reason
        );
        assert_eq!(
            nested_atomic.plan.operation,
            Some(EditorMoveOperation::AtomicTeraMove)
        );

        let closed_child = plan_editor_move(
            &runtime,
            &snapshot,
            &model,
            &article_node.id,
            &section_node.id,
            ProjectMovePosition::Before,
            None,
        );
        assert!(!closed_child.plan.allowed);
        assert_eq!(
            closed_child.plan.reason_code.as_deref(),
            Some("editor_move_scope_required")
        );

        let grant = runtime
            .issue_edit_scope_grant(
                &identity,
                &model.revision,
                "/",
                "templates/index.html",
                &boundary,
            )
            .unwrap();
        let opened_child = plan_editor_move(
            &runtime,
            &snapshot,
            &model,
            &article_node.id,
            &section_node.id,
            ProjectMovePosition::Before,
            Some(&grant),
        );
        assert!(opened_child.plan.allowed, "{:?}", opened_child.plan.reason);
        assert_eq!(
            opened_child.plan.operation,
            Some(EditorMoveOperation::HtmlSourceMove)
        );
        assert_eq!(
            opened_child.plan.impact.edit_scope_id.as_deref(),
            Some(scope_id.as_str())
        );
        assert!(opened_child.plan.live_projection.is_some());
        assert_eq!(
            opened_child.plan.live_projection_reason,
            EditorMoveLiveProjectionReason::Ready
        );

        let mut component_snapshot = snapshot.clone();
        let mut component_node = article_node.clone();
        component_node.id = "editor_render:component-instance".to_string();
        component_node.render_instance_id = Some("component-instance".to_string());
        component_node.component_invocation_ids = vec!["component-invocation-1".to_string()];
        component_snapshot
            .nodes
            .retain(|node| node.id != article_node.id);
        component_snapshot.nodes.push(component_node.clone());
        let component_move = plan_editor_move(
            &runtime,
            &component_snapshot,
            &model,
            &component_node.id,
            &section_node.id,
            ProjectMovePosition::Before,
            Some(&grant),
        );
        assert!(component_move.plan.allowed);
        assert_eq!(
            component_move.plan.operation,
            Some(EditorMoveOperation::ComponentMove)
        );
        let component_projection = component_move
            .plan
            .live_projection
            .as_ref()
            .expect("ComponentMove HTML unic trebuie proiectat live");
        assert_eq!(
            component_projection.source_render_instance_id,
            "component-instance"
        );
        assert_eq!(
            component_move.plan.live_projection_reason,
            EditorMoveLiveProjectionReason::Ready
        );
        let issued_component_move = runtime
            .issue_editor_move_decision(component_move)
            .expect("plan ComponentMove tokenizat");
        assert_eq!(
            issued_component_move
                .live_projection
                .as_ref()
                .and_then(|projection| projection.plan_token.as_deref()),
            issued_component_move.token.as_deref()
        );

        let mut repeated_component_snapshot = component_snapshot.clone();
        let mut repeated_component_node = component_node.clone();
        repeated_component_node.id = "editor_render:component-instance-2".to_string();
        repeated_component_node.render_instance_id = Some("component-instance-2".to_string());
        repeated_component_snapshot
            .nodes
            .push(repeated_component_node);
        let repeated_component_move = plan_editor_move(
            &runtime,
            &repeated_component_snapshot,
            &model,
            &component_node.id,
            &section_node.id,
            ProjectMovePosition::Before,
            Some(&grant),
        );
        assert!(repeated_component_move.plan.allowed);
        assert!(repeated_component_move.plan.live_projection.is_none());
        assert_eq!(
            repeated_component_move.plan.live_projection_reason,
            EditorMoveLiveProjectionReason::MultipleRenderedInstances
        );

        let mut block_snapshot = snapshot.clone();
        let mut native_block_node = article_node.clone();
        native_block_node.id = "editor_render:native-block-instance".to_string();
        native_block_node.render_instance_id = Some("native-block-instance".to_string());
        native_block_node.block_source_instance_ids = vec!["block-source-instance-1".to_string()];
        block_snapshot
            .nodes
            .retain(|node| node.id != article_node.id);
        block_snapshot.nodes.push(native_block_node.clone());
        let native_block_move = plan_editor_move(
            &runtime,
            &block_snapshot,
            &model,
            &native_block_node.id,
            &section_node.id,
            ProjectMovePosition::Before,
            Some(&grant),
        );
        assert!(native_block_move.plan.allowed);
        assert_eq!(
            native_block_move.plan.operation,
            Some(EditorMoveOperation::BlockMove)
        );
        assert!(native_block_move.plan.live_projection.is_some());
        assert_eq!(
            native_block_move.plan.live_projection_reason,
            EditorMoveLiveProjectionReason::Ready
        );

        let cross_scope = plan_editor_move(
            &runtime,
            &snapshot,
            &model,
            &article_node.id,
            &footer_node.id,
            ProjectMovePosition::Before,
            Some(&grant),
        );
        assert!(!cross_scope.plan.allowed);
        assert_eq!(
            cross_scope.plan.reason_code.as_deref(),
            Some("editor_move_cross_scope")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn navigation_snapshot_preserves_repeated_and_empty_boundary_instances() {
        let root = editor_navigation_test_project("snapshot-boundaries");
        let model = editor_navigation_test_model(&root);
        let loop_node = source_node(&model, SourceNodeKind::For, "for");
        let article = source_node(&model, SourceNodeKind::Html, "<article");
        let footer = source_node(&model, SourceNodeKind::Html, "<footer");
        let identity = CanvasProjectionIdentity {
            project_root: root.to_string_lossy().to_string(),
            runtime_session_id: "runtime-snapshot".to_string(),
            workspace_revision: 12,
            transaction_id: "canvas-snapshot".to_string(),
            preview_revision: "preview-snapshot".to_string(),
        };
        let render = |render_instance_id: &str, binding_key: &str, occurrence| CanvasRenderNode {
            render_instance_id: render_instance_id.to_string(),
            document_order: occurrence * 2 + 3,
            source_node_id: Some(article.id.clone()),
            template_source_node_id: Some(loop_node.id.clone()),
            parent_render_instance_id: None,
            provenance_stack: vec![loop_node.id.clone()],
            component_definition_ids: Vec::new(),
            component_invocation_ids: Vec::new(),
            block_definition_ids: Vec::new(),
            block_source_instance_ids: Vec::new(),
            dynamic_widget_provider_ids: Vec::new(),
            dynamic_widget_source_instance_ids: Vec::new(),
            binding_key: Some(binding_key.to_string()),
            binding_path: Some(format!("section.pages[{occurrence}]")),
            tag: "article".to_string(),
            occurrence,
            origin: CanvasNodeOrigin::Source,
            capabilities: CanvasNodeCapabilities {
                editable: true,
                inspectable: true,
                read_only: false,
            },
        };
        let boundary = |id: &str, root_id: Option<&str>, binding_key: Option<&str>, occurrence| {
            CanvasBoundaryInstance {
                boundary_instance_id: id.to_string(),
                document_order: occurrence * 2 + 2,
                source_node_id: loop_node.id.clone(),
                parent_boundary_instance_id: None,
                root_render_instance_ids: root_id
                    .map(|root| vec![root.to_string()])
                    .unwrap_or_default(),
                binding_key: binding_key.map(str::to_string),
                binding_path: binding_key.map(|key| format!("section.pages[{key}]")),
                occurrence,
                marker_kind: CanvasBoundaryMarkerKind::Source,
                markdown: None,
                closed: true,
            }
        };
        let mut footer_render = render("render-footer", "footer", 0);
        footer_render.document_order = 0;
        footer_render.source_node_id = Some(footer.id.clone());
        footer_render.template_source_node_id = None;
        footer_render.provenance_stack = vec![footer.id.clone()];
        footer_render.binding_key = None;
        footer_render.binding_path = None;
        let graph = CanvasGraph {
            schema_version: 1,
            workspace_revision: identity.workspace_revision,
            preview_revision: identity.preview_revision.clone(),
            model_revision: model.revision.clone(),
            documents: vec![CanvasDocumentGraph {
                route: "/".to_string(),
                nodes: vec![
                    footer_render,
                    render("render-alpha", "alpha", 0),
                    render("render-beta", "beta", 1),
                ],
                boundaries: vec![
                    boundary("boundary-alpha", Some("render-alpha"), Some("alpha"), 0),
                    boundary("boundary-beta", Some("render-beta"), Some("beta"), 1),
                    boundary("boundary-empty", None, None, 2),
                ],
            }],
            component_instances: Vec::new(),
            dynamic_widget_instances: Vec::new(),
            block_instances: Vec::new(),
            runtime_nodes: Vec::new(),
            diagnostics: Vec::new(),
        };

        let snapshot = build_editor_navigation_snapshot(
            identity,
            "/",
            &model,
            &graph,
            Some("templates/index.html"),
            None,
        )
        .unwrap();
        assert_eq!(
            snapshot.root_node_ids.first().map(String::as_str),
            Some(editor_render_node_id("render-footer").as_str())
        );
        let boundaries = snapshot
            .nodes
            .iter()
            .filter(|node| {
                node.kind == EditorNavigationNodeKind::TeraBoundary
                    && node.id.starts_with("editor_boundary:")
            })
            .collect::<Vec<_>>();
        assert_eq!(boundaries.len(), 3);
        assert_eq!(
            boundaries
                .iter()
                .map(|node| node.id.as_str())
                .collect::<HashSet<_>>()
                .len(),
            3
        );
        assert!(boundaries.iter().all(|node| {
            node.boundary.as_ref().is_some_and(|boundary| {
                boundary.rendered_instance_count == 3
                    && boundary.effect_scope == EditorNavigationEffectScope::AllRenderedInstances
            })
        }));
        assert_eq!(
            boundaries
                .iter()
                .filter(|node| node
                    .boundary
                    .as_ref()
                    .is_some_and(|boundary| boundary.empty))
                .count(),
            1
        );
        for render_node in snapshot.nodes.iter().filter(|node| {
            node.kind == EditorNavigationNodeKind::HtmlElement
                && node.id.starts_with("editor_render:")
                && node.render_instance_id.as_deref() != Some("render-footer")
        }) {
            assert!(render_node.capabilities.requires_edit_scope_id.is_some());
            assert!(!render_node.capabilities.can_move);
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn navigation_snapshot_opens_only_the_active_document_wrapper_boundary() {
        let root = editor_navigation_inheritance_test_project("snapshot-active-document");
        let model = editor_navigation_test_model(&root);
        let index_content = source_node_in_file(
            &model,
            SourceNodeKind::Block,
            "content",
            "templates/index.html",
        );
        let hero = source_node_in_file(
            &model,
            SourceNodeKind::Html,
            "<section",
            "templates/index.html",
        );
        let card_include = source_node_in_file(
            &model,
            SourceNodeKind::Include,
            "card",
            "templates/index.html",
        );
        let card = source_node_in_file(
            &model,
            SourceNodeKind::Html,
            "<article",
            "templates/partials/card.html",
        );
        let layout_body = source_node_in_file(
            &model,
            SourceNodeKind::Block,
            "body",
            "templates/layout.html",
        );
        let layout_main = source_node_in_file(
            &model,
            SourceNodeKind::Html,
            "<main",
            "templates/layout.html",
        );
        let identity = CanvasProjectionIdentity {
            project_root: root.to_string_lossy().to_string(),
            runtime_session_id: "runtime-active-document".to_string(),
            workspace_revision: 21,
            transaction_id: "canvas-active-document".to_string(),
            preview_revision: "preview-active-document".to_string(),
        };
        let render = |id: &str, source: &SourceNode, order| CanvasRenderNode {
            render_instance_id: id.to_string(),
            document_order: order,
            source_node_id: Some(source.id.clone()),
            template_source_node_id: None,
            parent_render_instance_id: None,
            provenance_stack: vec![source.id.clone()],
            component_definition_ids: Vec::new(),
            component_invocation_ids: Vec::new(),
            block_definition_ids: Vec::new(),
            block_source_instance_ids: Vec::new(),
            dynamic_widget_provider_ids: Vec::new(),
            dynamic_widget_source_instance_ids: Vec::new(),
            binding_key: None,
            binding_path: None,
            tag: source_html_tag(&source.label).unwrap(),
            occurrence: 0,
            origin: CanvasNodeOrigin::Source,
            capabilities: CanvasNodeCapabilities {
                editable: true,
                inspectable: true,
                read_only: false,
            },
        };
        let boundary = |id: &str,
                        source: &SourceNode,
                        parent: Option<&str>,
                        root_render_instance_id: &str,
                        order| CanvasBoundaryInstance {
            boundary_instance_id: id.to_string(),
            document_order: order,
            source_node_id: source.id.clone(),
            parent_boundary_instance_id: parent.map(str::to_string),
            root_render_instance_ids: vec![root_render_instance_id.to_string()],
            binding_key: None,
            binding_path: None,
            occurrence: 0,
            marker_kind: CanvasBoundaryMarkerKind::Source,
            markdown: None,
            closed: true,
        };
        let graph = CanvasGraph {
            schema_version: 1,
            workspace_revision: identity.workspace_revision,
            preview_revision: identity.preview_revision.clone(),
            model_revision: model.revision.clone(),
            documents: vec![CanvasDocumentGraph {
                route: "/".to_string(),
                nodes: vec![
                    render("render-hero", hero, 1),
                    render("render-card", card, 3),
                    render("render-layout-main", layout_main, 5),
                ],
                boundaries: vec![
                    boundary("index-content", index_content, None, "render-hero", 0),
                    boundary(
                        "index-card-include",
                        card_include,
                        Some("index-content"),
                        "render-card",
                        2,
                    ),
                    boundary("layout-body", layout_body, None, "render-layout-main", 4),
                ],
            }],
            component_instances: Vec::new(),
            dynamic_widget_instances: Vec::new(),
            block_instances: Vec::new(),
            runtime_nodes: Vec::new(),
            diagnostics: Vec::new(),
        };

        let index_snapshot = build_editor_navigation_snapshot(
            identity.clone(),
            "/",
            &model,
            &graph,
            Some("templates/index.html"),
            None,
        )
        .unwrap();
        let index_wrapper =
            editor_navigation_node(&index_snapshot, "editor_boundary:index-content").unwrap();
        assert!(!index_wrapper.capabilities.can_enter_boundary);
        assert!(index_wrapper.capabilities.requires_edit_scope_id.is_none());
        let hero_render =
            editor_navigation_node(&index_snapshot, "editor_render:render-hero").unwrap();
        assert!(hero_render.capabilities.requires_edit_scope_id.is_none());
        assert!(hero_render.capabilities.can_move);
        assert_eq!(
            enclosing_edit_scope(&index_snapshot, hero_render, false),
            None
        );
        let included_card =
            editor_navigation_node(&index_snapshot, "editor_render:render-card").unwrap();
        assert_eq!(
            included_card.capabilities.requires_edit_scope_id.as_deref(),
            Some("editor_boundary:index-card-include")
        );
        let foreign_layout =
            editor_navigation_node(&index_snapshot, "editor_render:render-layout-main").unwrap();
        assert_eq!(
            foreign_layout
                .capabilities
                .requires_edit_scope_id
                .as_deref(),
            Some("editor_boundary:layout-body")
        );

        let layout_snapshot = build_editor_navigation_snapshot(
            identity,
            "/",
            &model,
            &graph,
            Some("templates/layout.html"),
            None,
        )
        .unwrap();
        let layout_wrapper =
            editor_navigation_node(&layout_snapshot, "editor_boundary:layout-body").unwrap();
        assert!(!layout_wrapper.capabilities.can_enter_boundary);
        assert!(layout_wrapper.capabilities.requires_edit_scope_id.is_none());
        let layout_render =
            editor_navigation_node(&layout_snapshot, "editor_render:render-layout-main").unwrap();
        assert!(layout_render.capabilities.requires_edit_scope_id.is_none());
        assert_eq!(
            editor_navigation_node(&layout_snapshot, "editor_render:render-hero")
                .unwrap()
                .capabilities
                .requires_edit_scope_id
                .as_deref(),
            Some("editor_boundary:index-content")
        );
        assert_eq!(
            editor_navigation_node(&layout_snapshot, "editor_render:render-card")
                .unwrap()
                .capabilities
                .requires_edit_scope_id
                .as_deref(),
            Some("editor_boundary:index-card-include")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn focused_view_reroots_index_without_claiming_inherited_sources() {
        let root = editor_navigation_inheritance_test_project("focused-index");
        let model = editor_navigation_test_model(&root);
        let snapshot = focused_snapshot(&root, &model, "templates/index.html");
        let view = snapshot.focused_view.as_ref().expect("focused view");

        assert_eq!(view.active_document_path, "templates/index.html");
        assert_eq!(
            view.breadcrumbs
                .iter()
                .map(|entry| entry.document_path.as_str())
                .collect::<Vec<_>>(),
            vec![
                "templates/base.html",
                "templates/layout.html",
                "templates/index.html",
            ]
        );
        assert!(view
            .nodes
            .iter()
            .all(|node| node.file == "templates/index.html"));
        assert!(
            !view
                .nodes
                .iter()
                .any(|node| node.file == "templates/layout.html"
                    || node.file == "templates/base.html")
        );

        assert!(!view.nodes.iter().any(|node| {
            matches!(
                node.source_kind,
                Some(
                    SourceNodeKind::Extends | SourceNodeKind::Super | SourceNodeKind::TeraVariable
                )
            )
        }));
        assert!(!view
            .nodes
            .iter()
            .any(|node| node.kind == EditorNavigationViewNodeKind::Slot));
        assert!(view.root_node_ids.iter().all(|root_id| {
            view.nodes
                .iter()
                .find(|node| &node.id == root_id)
                .is_some_and(|node| node.source_kind != Some(SourceNodeKind::Block))
        }));

        let includes = view
            .nodes
            .iter()
            .filter(|node| node.source_kind == Some(SourceNodeKind::Include))
            .collect::<Vec<_>>();
        assert_eq!(includes.len(), 2);
        assert!(includes.iter().all(|node| {
            node.kind == EditorNavigationViewNodeKind::Boundary
                && node.capabilities.can_enter_boundary
                && node.boundary.as_ref().is_some_and(|boundary| {
                    boundary.effect_scope == EditorNavigationEffectScope::SharedDefinition
                        && boundary.rendered_instance_count == 2
                })
                && node
                    .relation
                    .as_ref()
                    .and_then(|relation| relation.target_document_path.as_deref())
                    == Some("templates/partials/card.html")
        }));
        assert_ne!(includes[0].source_node_id, includes[1].source_node_id);

        for kind in [SourceNodeKind::For, SourceNodeKind::If] {
            let node = view
                .nodes
                .iter()
                .find(|node| node.source_kind == Some(kind.clone()))
                .unwrap_or_else(|| panic!("missing {kind:?}"));
            assert_eq!(node.kind, EditorNavigationViewNodeKind::Boundary);
            assert!(node.capabilities.can_enter_boundary);
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn focused_view_keeps_only_visual_layers_and_embedded_tera_gates() {
        let root = editor_navigation_inheritance_test_project("focused-visual-layers");
        let model = editor_navigation_test_model(&root);

        let index = focused_snapshot(&root, &model, "templates/index.html");
        let view = index.focused_view.as_ref().unwrap();
        for hidden_label in ["title", "description", "css_pagina", "scripts"] {
            assert!(
                !view.nodes.iter().any(|node| node.label == hidden_label),
                "blocul auxiliar {hidden_label:?} nu este strat vizual"
            );
        }
        assert!(!view.nodes.iter().any(|node| {
            matches!(
                node.source_kind,
                Some(
                    SourceNodeKind::Extends | SourceNodeKind::Super | SourceNodeKind::TeraVariable
                )
            )
        }));
        assert!(view.nodes.iter().any(|node| {
            node.source_kind == Some(SourceNodeKind::Html) && node.tag.as_deref() == Some("section")
        }));

        let embedded = focused_snapshot(&root, &model, "templates/embedded.html");
        let embedded_view = embedded.focused_view.as_ref().unwrap();
        let promo = embedded_view
            .nodes
            .iter()
            .find(|node| node.source_kind == Some(SourceNodeKind::Block) && node.label == "promo")
            .expect("block-ul Tera din HTML rămâne gate vizual");
        assert_eq!(promo.kind, EditorNavigationViewNodeKind::Boundary);
        assert!(promo.capabilities.can_enter_boundary);
        assert_eq!(promo.children.len(), 1);
        let main = embedded_view
            .root_node_ids
            .iter()
            .find_map(|root_id| {
                embedded_view
                    .nodes
                    .iter()
                    .find(|node| &node.id == root_id && node.tag.as_deref() == Some("main"))
            })
            .expect("structura HTML este rădăcina vizuală");
        assert_eq!(promo.parent_id.as_deref(), Some(main.id.as_str()));
        assert!(embedded_view
            .root_node_ids
            .iter()
            .any(|root_id| embedded_view
                .nodes
                .iter()
                .find(|node| &node.id == root_id)
                .is_some_and(|node| node.tag.as_deref() == Some("main"))));
        assert!(!embedded_view.nodes.iter().any(|node| node.label == "title"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn focused_view_keeps_complete_nested_html_after_leading_utf8_text() {
        let root = editor_navigation_inheritance_test_project("focused-leading-utf8");
        fs::write(
            root.join("templates/index.html"),
            concat!(
                "{% extends \"layout.html\" %}\n",
                "{% block content %}\n",
                "<section id=\"contact\">\n",
                "  <div class=\"container\">\n",
                "    <h2>Începe prin a modifica acest conținut.</h2>\n",
                "    <p>Selectează un element în preview.</p>\n",
                "  </div>\n",
                "</section>\n",
                "{% endblock %}\n",
            ),
        )
        .unwrap();
        let model = editor_navigation_test_model(&root);
        let snapshot = focused_snapshot(&root, &model, "templates/index.html");
        let view = snapshot.focused_view.as_ref().expect("focused view");
        let by_tag = |tag: &str| {
            view.nodes
                .iter()
                .find(|node| node.tag.as_deref() == Some(tag))
                .unwrap_or_else(|| panic!("missing <{tag}> in focused view"))
        };
        let section = by_tag("section");
        let div = by_tag("div");
        let heading = by_tag("h2");
        let paragraph = by_tag("p");

        assert_eq!(div.parent_id.as_deref(), Some(section.id.as_str()));
        assert_eq!(heading.parent_id.as_deref(), Some(div.id.as_str()));
        assert_eq!(paragraph.parent_id.as_deref(), Some(div.id.as_str()));
        assert_eq!(div.children, vec![heading.id.clone(), paragraph.id.clone()],);
        assert_eq!(view.root_node_ids, vec![section.id.clone()]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn focused_view_changes_ownership_for_layout_base_and_partial() {
        let root = editor_navigation_inheritance_test_project("focused-documents");
        let model = editor_navigation_test_model(&root);

        let layout = focused_snapshot(&root, &model, "templates/layout.html");
        let layout_view = layout.focused_view.as_ref().unwrap();
        assert!(layout_view
            .nodes
            .iter()
            .all(|node| node.file == "templates/layout.html"));
        assert_eq!(
            layout_view
                .nodes
                .iter()
                .filter(|node| node.source_kind == Some(SourceNodeKind::Include))
                .count(),
            2
        );
        assert!(layout_view.nodes.iter().any(|node| {
            node.source_kind == Some(SourceNodeKind::Block)
                && node.kind == EditorNavigationViewNodeKind::Boundary
                && node.capabilities.can_enter_boundary
        }));

        let base = focused_snapshot(&root, &model, "templates/base.html");
        let base_view = base.focused_view.as_ref().unwrap();
        assert_eq!(base_view.breadcrumbs.len(), 1);
        assert!(base_view
            .nodes
            .iter()
            .all(|node| node.file == "templates/base.html"));

        let partial = focused_snapshot(&root, &model, "templates/partials/card.html");
        let partial_view = partial.focused_view.as_ref().unwrap();
        assert_eq!(partial_view.breadcrumbs.len(), 1);
        assert!(partial_view
            .nodes
            .iter()
            .all(|node| node.file == "templates/partials/card.html"));
        assert!(partial_view
            .nodes
            .iter()
            .filter(|node| node.source_kind == Some(SourceNodeKind::Html))
            .all(|node| node.capabilities.requires_edit_scope_id.is_none()));

        let macro_partial = focused_snapshot(&root, &model, "templates/partials/widget.html");
        let macro_view = macro_partial.focused_view.as_ref().unwrap();
        let macro_node = macro_view
            .nodes
            .iter()
            .find(|node| node.source_kind == Some(SourceNodeKind::Macro))
            .expect("macro boundary");
        assert!(macro_node.capabilities.can_enter_boundary);
        let nested_if = macro_view
            .nodes
            .iter()
            .find(|node| node.source_kind == Some(SourceNodeKind::If))
            .expect("nested if boundary");
        assert_eq!(
            nested_if.capabilities.requires_edit_scope_id,
            nested_if.editor_node_id
        );
        assert_eq!(nested_if.parent_id.as_deref(), Some(macro_node.id.as_str()));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn focused_view_preserves_theme_origin_and_local_override_resolution() {
        let root = editor_navigation_theme_test_project("focused-theme");
        let model = editor_navigation_test_model(&root);

        let index = focused_snapshot(&root, &model, "templates/index.html");
        let index_view = index.focused_view.as_ref().unwrap();
        assert_eq!(
            index_view
                .breadcrumbs
                .iter()
                .map(|entry| (entry.document_path.as_str(), entry.origin))
                .collect::<Vec<_>>(),
            vec![
                (
                    "themes/test-theme/templates/base.html",
                    EditorNavigationOrigin::Theme,
                ),
                ("templates/index.html", EditorNavigationOrigin::Project,),
            ]
        );
        assert!(!index_view
            .nodes
            .iter()
            .any(|node| node.source_kind == Some(SourceNodeKind::Extends)));

        let theme_base = focused_snapshot(&root, &model, "themes/test-theme/templates/base.html");
        let theme_view = theme_base.focused_view.as_ref().unwrap();
        assert!(theme_view.nodes.iter().all(|node| {
            node.origin == EditorNavigationOrigin::Theme
                && node.theme_name.as_deref() == Some("test-theme")
                && node.capabilities.read_only
                && !node.capabilities.can_enter_boundary
        }));
        let include = theme_view
            .nodes
            .iter()
            .find(|node| node.source_kind == Some(SourceNodeKind::Include))
            .unwrap();
        assert_eq!(
            include
                .relation
                .as_ref()
                .and_then(|relation| relation.target_document_path.as_deref()),
            Some("templates/partials/footer.html")
        );

        let override_partial = focused_snapshot(&root, &model, "templates/partials/footer.html");
        assert!(override_partial
            .focused_view
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .all(|node| node.origin == EditorNavigationOrigin::Project));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn source_provenance_separates_include_definition_from_composition_site() {
        let root = editor_navigation_inheritance_test_project("source-provenance-include");
        let model = editor_navigation_test_model(&root);
        let include = source_node_in_file(
            &model,
            SourceNodeKind::Include,
            "partials/header.html",
            "templates/layout.html",
        );

        let provenance = editor_source_provenance(&model, Some(include), &[]);
        let definition = provenance.definition.as_ref().expect("include definition");
        let composition = provenance
            .composition
            .as_ref()
            .expect("include composition");
        assert_eq!(provenance.resolution, EditorSourceResolution::Resolved);
        assert_eq!(definition.file, "templates/partials/header.html");
        assert_eq!(definition.origin, EditorNavigationOrigin::Project);
        assert_eq!(composition.file, "templates/layout.html");
        assert_eq!(
            composition.source_node_id.as_deref(),
            Some(include.id.as_str())
        );
        assert!(definition.can_open_in_code);
        assert!(composition.can_open_in_code);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn source_provenance_keeps_partial_html_definition_and_include_composition() {
        let root = editor_navigation_inheritance_test_project("source-provenance-partial-html");
        let model = editor_navigation_test_model(&root);
        let include = source_node_in_file(
            &model,
            SourceNodeKind::Include,
            "partials/header.html",
            "templates/layout.html",
        );
        let header = source_node_in_file(
            &model,
            SourceNodeKind::Html,
            "<header>",
            "templates/partials/header.html",
        );
        let invocation = model
            .source_graph
            .component_graph
            .invocations
            .iter()
            .find(|invocation| invocation.source_node_id.as_deref() == Some(include.id.as_str()))
            .expect("header include invocation");

        let provenance =
            editor_source_provenance(&model, Some(header), std::slice::from_ref(&invocation.id));
        let definition = provenance.definition.as_ref().expect("header definition");
        let composition = provenance.composition.as_ref().expect("header composition");
        assert_eq!(provenance.resolution, EditorSourceResolution::Resolved);
        assert_eq!(definition.file, "templates/partials/header.html");
        assert_eq!(
            definition.source_node_id.as_deref(),
            Some(header.id.as_str()),
        );
        assert_eq!(composition.file, "templates/layout.html");
        assert_eq!(
            composition.source_node_id.as_deref(),
            Some(include.id.as_str()),
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn source_provenance_keeps_direct_html_as_its_definition() {
        let root = editor_navigation_inheritance_test_project("source-provenance-html");
        let model = editor_navigation_test_model(&root);
        let heading =
            source_node_in_file(&model, SourceNodeKind::Html, "<h1>", "templates/index.html");

        let provenance = editor_source_provenance(&model, Some(heading), &[]);
        let definition = provenance.definition.as_ref().expect("html definition");
        assert_eq!(provenance.resolution, EditorSourceResolution::Direct);
        assert_eq!(
            definition.source_node_id.as_deref(),
            Some(heading.id.as_str())
        );
        assert_eq!(definition.file, "templates/index.html");
        assert!(provenance.composition.is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn source_provenance_respects_theme_origin_and_project_shadowing() {
        let root = editor_navigation_theme_test_project("source-provenance-theme");
        let model = editor_navigation_test_model(&root);
        let include = source_node_in_file(
            &model,
            SourceNodeKind::Include,
            "partials/footer.html",
            "themes/test-theme/templates/base.html",
        );

        let provenance = editor_source_provenance(&model, Some(include), &[]);
        let definition = provenance
            .definition
            .as_ref()
            .expect("shadowing definition");
        let composition = provenance.composition.as_ref().expect("theme composition");
        assert_eq!(definition.file, "templates/partials/footer.html");
        assert_eq!(definition.origin, EditorNavigationOrigin::Project);
        assert_eq!(composition.file, "themes/test-theme/templates/base.html",);
        assert_eq!(composition.origin, EditorNavigationOrigin::Theme);
        assert_eq!(composition.theme_name.as_deref(), Some("test-theme"));

        let theme_source = source_node_in_file(
            &model,
            SourceNodeKind::Block,
            "content",
            "themes/test-theme/templates/base.html",
        );
        let direct = editor_source_provenance(&model, Some(theme_source), &[]);
        assert_eq!(
            direct.definition.as_ref().map(|source| source.origin),
            Some(EditorNavigationOrigin::Theme),
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn source_provenance_reports_fallback_and_unresolved_includes() {
        let root = editor_navigation_test_project("source-provenance-resolution");
        fs::create_dir_all(root.join("templates/partials")).unwrap();
        fs::write(
            root.join("templates/index.html"),
            concat!(
                "{% include [\"partials/missing.html\", \"partials/fallback.html\"] %}\n",
                "{% include \"partials/unresolved.html\" %}\n",
            ),
        )
        .unwrap();
        fs::write(
            root.join("templates/partials/fallback.html"),
            "<aside>Fallback</aside>\n",
        )
        .unwrap();
        let model = editor_navigation_test_model(&root);
        let fallback = source_node_in_file(
            &model,
            SourceNodeKind::Include,
            "partials/missing.html",
            "templates/index.html",
        );
        let unresolved = source_node_in_file(
            &model,
            SourceNodeKind::Include,
            "partials/unresolved.html",
            "templates/index.html",
        );

        let fallback_provenance = editor_source_provenance(&model, Some(fallback), &[]);
        assert_eq!(
            fallback_provenance.resolution,
            EditorSourceResolution::FallbackResolved,
        );
        assert_eq!(
            fallback_provenance
                .definition
                .as_ref()
                .map(|source| source.file.as_str()),
            Some("templates/partials/fallback.html"),
        );
        assert_eq!(
            fallback_provenance
                .composition
                .as_ref()
                .map(|source| source.file.as_str()),
            Some("templates/index.html"),
        );

        let unresolved_provenance = editor_source_provenance(&model, Some(unresolved), &[]);
        assert_eq!(
            unresolved_provenance.resolution,
            EditorSourceResolution::Unresolved,
        );
        assert!(unresolved_provenance.definition.is_none());
        assert_eq!(
            unresolved_provenance
                .composition
                .as_ref()
                .and_then(|source| source.source_node_id.as_deref()),
            Some(unresolved.id.as_str()),
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn focused_view_ids_drive_the_central_move_planner_and_scope_grants() {
        let root = editor_navigation_inheritance_test_project("focused-move");
        let model = editor_navigation_test_model(&root);
        let snapshot = focused_snapshot(&root, &model, "templates/index.html");
        let view = snapshot.focused_view.as_ref().unwrap();
        let runtime = EditorNavigationRuntime::default();

        let direct_paragraphs = view
            .nodes
            .iter()
            .filter(|node| {
                node.source_kind == Some(SourceNodeKind::Html)
                    && node.tag.as_deref() == Some("p")
                    && node.capabilities.requires_edit_scope_id.is_none()
            })
            .collect::<Vec<_>>();
        assert_eq!(direct_paragraphs.len(), 2);
        let direct_move = plan_editor_move(
            &runtime,
            &snapshot,
            &model,
            direct_paragraphs[1].editor_node_id.as_deref().unwrap(),
            direct_paragraphs[0].editor_node_id.as_deref().unwrap(),
            ProjectMovePosition::Before,
            None,
        );
        assert!(direct_move.plan.allowed, "{:?}", direct_move.plan.reason);
        assert_eq!(
            direct_move.plan.active_document_path,
            "templates/index.html"
        );

        let nested_spans = view
            .nodes
            .iter()
            .filter(|node| {
                node.source_kind == Some(SourceNodeKind::Html)
                    && node.tag.as_deref() == Some("span")
            })
            .collect::<Vec<_>>();
        assert_eq!(nested_spans.len(), 2);
        let scope_id = nested_spans[0]
            .capabilities
            .requires_edit_scope_id
            .as_deref()
            .expect("if scope");
        assert_eq!(
            nested_spans[1]
                .capabilities
                .requires_edit_scope_id
                .as_deref(),
            Some(scope_id),
        );
        let closed_move = plan_editor_move(
            &runtime,
            &snapshot,
            &model,
            nested_spans[1].editor_node_id.as_deref().unwrap(),
            nested_spans[0].editor_node_id.as_deref().unwrap(),
            ProjectMovePosition::Before,
            None,
        );
        assert_eq!(
            closed_move.plan.reason_code.as_deref(),
            Some("editor_move_scope_required")
        );
        let scope = editor_navigation_node(&snapshot, scope_id).unwrap();
        let grant = runtime
            .issue_edit_scope_grant(
                &snapshot.identity,
                &snapshot.model_revision,
                &snapshot.route,
                "templates/index.html",
                scope,
            )
            .unwrap();
        let opened_move = plan_editor_move(
            &runtime,
            &snapshot,
            &model,
            nested_spans[1].editor_node_id.as_deref().unwrap(),
            nested_spans[0].editor_node_id.as_deref().unwrap(),
            ProjectMovePosition::Before,
            Some(&grant),
        );
        assert!(opened_move.plan.allowed, "{:?}", opened_move.plan.reason);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn focused_layers_project_markdown_as_one_atomic_source_boundary() {
        let root = editor_navigation_test_project("focused-markdown");
        fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = \"Acasă\"\ntemplate = \"index.html\"\n+++\n## Titlu\n\nText cu [legătură](/).\n",
        )
        .unwrap();
        fs::write(
            root.join("templates/index.html"),
            "<main><header>Exterior</header>{{ section.content | safe }}<footer>Exterior</footer></main>",
        )
        .unwrap();
        let model = editor_navigation_test_model(&root);
        let projection = model
            .source_graph
            .markdown_projections
            .iter()
            .find(|projection| projection.kind == MarkdownProjectionKind::Body)
            .expect("section.content projection");
        let encoded_file = BASE64_STANDARD.encode("_index.md");
        let rendered = format!(
            concat!(
                "<main><header>Exterior</header>",
                "<!-- pana-markdown-start:{}:{} -->",
                "<h2>Titlu</h2><p>Text cu <a href=\"/\">legătură</a>.</p>",
                "<!-- pana-markdown-end:{} -->",
                "<footer>Exterior</footer></main>"
            ),
            projection.id, encoded_file, projection.id,
        );
        let graph = CanvasGraph::from_rendered_documents(
            &model,
            23,
            "preview-markdown-23",
            [("/", rendered.as_str())],
        )
        .unwrap();
        let identity = CanvasProjectionIdentity {
            project_root: root.to_string_lossy().to_string(),
            runtime_session_id: "runtime-focused-markdown".to_string(),
            workspace_revision: 23,
            transaction_id: "canvas-focused-markdown".to_string(),
            preview_revision: "preview-markdown-23".to_string(),
        };
        let snapshot = build_editor_navigation_snapshot(
            identity,
            "/",
            &model,
            &graph,
            Some("templates/index.html"),
            None,
        )
        .unwrap();
        let markdown = snapshot
            .nodes
            .iter()
            .find(|node| node.kind == EditorNavigationNodeKind::MarkdownBoundary)
            .expect("canonical Markdown boundary");
        assert_eq!(markdown.file.as_deref(), Some("content/_index.md"));
        assert!(markdown.capabilities.can_open_in_code);
        assert!(!markdown.capabilities.can_enter_boundary);
        assert!(!markdown.capabilities.can_move_atomic);
        assert!(!markdown.capabilities.can_move);
        assert!(!markdown.capabilities.can_edit_text);
        assert!(!markdown.capabilities.can_edit_attributes);
        assert!(markdown.capabilities.read_only);

        let view = snapshot.focused_view.as_ref().expect("focused Layers view");
        let markdown_layers = view
            .nodes
            .iter()
            .filter(|node| node.editor_node_id.as_deref() == Some(markdown.id.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(markdown_layers.len(), 1);
        assert_eq!(
            markdown_layers[0].kind,
            EditorNavigationViewNodeKind::Boundary
        );
        assert!(markdown_layers[0].children.is_empty());
        assert_eq!(markdown_layers[0].file, "content/_index.md");
        assert!(view
            .nodes
            .iter()
            .all(|node| { !matches!(node.tag.as_deref(), Some("h2" | "p" | "a")) }));

        let internal_render_nodes = snapshot
            .nodes
            .iter()
            .filter(|node| {
                node.kind == EditorNavigationNodeKind::HtmlElement
                    && node.capabilities.requires_edit_scope_id.as_deref()
                        == Some(markdown.id.as_str())
            })
            .count();
        assert!(internal_render_nodes >= 3);
        for exterior_tag in ["header", "footer"] {
            let exterior = view
                .nodes
                .iter()
                .find(|node| node.tag.as_deref() == Some(exterior_tag))
                .expect("exterior template HTML remains in Layers");
            assert!(exterior.capabilities.requires_edit_scope_id.is_none());
            assert!(exterior.capabilities.can_move);
        }

        let target = view
            .nodes
            .iter()
            .find(|node| node.tag.as_deref() == Some("footer"))
            .and_then(|node| node.editor_node_id.as_deref())
            .expect("exterior target");
        let blocked = plan_editor_move(
            &EditorNavigationRuntime::default(),
            &snapshot,
            &model,
            &markdown.id,
            target,
            ProjectMovePosition::Before,
            None,
        );
        assert!(!blocked.plan.allowed);
        assert_eq!(
            blocked.plan.reason_code.as_deref(),
            Some("editor_move_markdown_read_only")
        );

        let graph_after_reprojection = CanvasGraph::from_rendered_documents(
            &model,
            24,
            "preview-markdown-24",
            [("/", rendered.as_str())],
        )
        .unwrap();
        let snapshot_after_reprojection = build_editor_navigation_snapshot(
            CanvasProjectionIdentity {
                project_root: root.to_string_lossy().to_string(),
                runtime_session_id: "runtime-focused-markdown".to_string(),
                workspace_revision: 24,
                transaction_id: "canvas-focused-markdown-2".to_string(),
                preview_revision: "preview-markdown-24".to_string(),
            },
            "/",
            &model,
            &graph_after_reprojection,
            Some("templates/index.html"),
            None,
        )
        .unwrap();
        let markdown_after_reprojection = snapshot_after_reprojection
            .nodes
            .iter()
            .find(|node| node.kind == EditorNavigationNodeKind::MarkdownBoundary)
            .expect("reprojected Markdown boundary");
        assert_eq!(markdown.id, markdown_after_reprojection.id);
        assert_eq!(markdown.file, markdown_after_reprojection.file);
        assert_eq!(markdown.range, markdown_after_reprojection.range);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn empty_direct_fragment_projects_one_local_authoring_root() {
        let root = editor_navigation_test_project("empty-direct-fragment");
        fs::create_dir_all(root.join("templates/listing-items")).unwrap();
        fs::write(root.join("templates/listing-items/card.html"), "\n").unwrap();
        let model = editor_navigation_test_model(&root);
        let fragment = model
            .source_graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == SourceNodeKind::Partial
                    && node.file == "templates/listing-items/card.html"
                    && node.parent.is_none()
            })
            .expect("listing item root");
        let rendered = format!(
            "<body><!-- pana-template-source-start:{} --><!-- pana-template-source-end:{} --></body>",
            fragment.id, fragment.id
        );
        let route = "/__pana_workbench/listing-item/";
        let graph = CanvasGraph::from_rendered_documents(
            &model,
            31,
            "preview-empty-fragment-31",
            [(route, rendered.as_str())],
        )
        .unwrap();
        let identity = CanvasProjectionIdentity {
            project_root: root.to_string_lossy().to_string(),
            runtime_session_id: "runtime-empty-fragment".to_string(),
            workspace_revision: 31,
            transaction_id: "canvas-empty-fragment".to_string(),
            preview_revision: "preview-empty-fragment-31".to_string(),
        };
        let snapshot = build_editor_navigation_snapshot(
            identity,
            route,
            &model,
            &graph,
            Some("templates/listing-items/card.html"),
            None,
        )
        .unwrap();

        let boundary = snapshot
            .nodes
            .iter()
            .find(|node| {
                node.kind == EditorNavigationNodeKind::TeraBoundary
                    && node.source_node_id.as_deref() == Some(fragment.id.as_str())
            })
            .expect("fragment root boundary");
        assert_eq!(boundary.source_kind, Some(SourceNodeKind::Partial));
        assert!(!boundary.capabilities.read_only);
        assert!(boundary.capabilities.requires_edit_scope_id.is_none());
        assert!(boundary
            .boundary
            .as_ref()
            .is_some_and(|boundary| boundary.empty));

        let view = snapshot
            .focused_view
            .as_ref()
            .expect("focused fragment view");
        assert_eq!(view.root_node_ids.len(), 1);
        let slot = view
            .nodes
            .iter()
            .find(|node| node.id == view.root_node_ids[0])
            .expect("empty fragment authoring slot");
        assert_eq!(slot.kind, EditorNavigationViewNodeKind::Slot);
        assert_eq!(slot.source_kind, Some(SourceNodeKind::Partial));
        assert_eq!(slot.source_node_id.as_deref(), Some(fragment.id.as_str()));
        assert!(!slot.capabilities.read_only);
        assert!(slot.capabilities.requires_edit_scope_id.is_none());
        fs::remove_dir_all(root).unwrap();
    }

    fn focused_snapshot(
        root: &std::path::Path,
        model: &ProjectModel,
        active_document_path: &str,
    ) -> EditorNavigationSnapshot {
        let identity = CanvasProjectionIdentity {
            project_root: root.to_string_lossy().to_string(),
            runtime_session_id: "runtime-focused".to_string(),
            workspace_revision: 17,
            transaction_id: "canvas-focused".to_string(),
            preview_revision: "preview-focused".to_string(),
        };
        let graph = CanvasGraph {
            schema_version: 1,
            workspace_revision: identity.workspace_revision,
            preview_revision: identity.preview_revision.clone(),
            model_revision: model.revision.clone(),
            documents: vec![CanvasDocumentGraph {
                route: "/".to_string(),
                nodes: Vec::new(),
                boundaries: Vec::new(),
            }],
            component_instances: Vec::new(),
            block_instances: Vec::new(),
            dynamic_widget_instances: Vec::new(),
            runtime_nodes: Vec::new(),
            diagnostics: Vec::new(),
        };
        build_editor_navigation_snapshot(
            identity,
            "/",
            model,
            &graph,
            Some(active_document_path),
            None,
        )
        .unwrap()
    }

    fn editor_html_node(
        source: &SourceNode,
        render_instance_id: &str,
        scope_id: Option<String>,
        order: usize,
    ) -> EditorNavigationNode {
        EditorNavigationNode {
            id: editor_render_node_id(render_instance_id),
            parent_id: scope_id.clone(),
            children: Vec::new(),
            order,
            kind: EditorNavigationNodeKind::HtmlElement,
            label: source.label.clone(),
            tag: source
                .label
                .strip_prefix('<')
                .and_then(|label| label.split([' ', '>', '.']).next())
                .map(str::to_string),
            source_node_id: Some(source.id.clone()),
            render_instance_id: Some(render_instance_id.to_string()),
            source_kind: Some(source.kind.clone()),
            file: Some(source.file.clone()),
            range: source.range.clone(),
            origin: EditorNavigationOrigin::Project,
            theme_name: None,
            source_provenance: EditorSourceProvenance {
                definition: Some(editor_source_reference(source)),
                composition: None,
                resolution: EditorSourceResolution::Direct,
            },
            provenance_stack: Vec::new(),
            component_definition_ids: Vec::new(),
            component_invocation_ids: Vec::new(),
            block_definition_ids: Vec::new(),
            block_source_instance_ids: Vec::new(),
            dynamic_widget_provider_ids: Vec::new(),
            dynamic_widget_source_instance_ids: Vec::new(),
            binding_key: None,
            binding_path: None,
            boundary: None,
            capabilities: EditorNavigationCapabilities {
                can_select: true,
                can_inspect: true,
                can_open_in_code: source.capabilities.can_open_in_code,
                can_enter_boundary: false,
                can_move_atomic: false,
                can_move: scope_id.is_none() && source.capabilities.can_move,
                can_edit_text: scope_id.is_none() && source.capabilities.can_edit_text,
                can_edit_attributes: scope_id.is_none() && source.capabilities.can_edit_attributes,
                read_only: scope_id.is_some(),
                requires_edit_scope_id: scope_id,
                reason_code: source.capabilities.reason_code,
            },
            source_html_attributes: None,
        }
    }

    fn source_node<'a>(
        model: &'a ProjectModel,
        kind: SourceNodeKind,
        label: &str,
    ) -> &'a SourceNode {
        model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == kind && node.label.contains(label))
            .unwrap_or_else(|| panic!("Lipsește nodul {kind:?} care conține {label:?}."))
    }

    fn source_node_in_file<'a>(
        model: &'a ProjectModel,
        kind: SourceNodeKind,
        label: &str,
        file: &str,
    ) -> &'a SourceNode {
        model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == kind && node.file == file && node.label.contains(label))
            .unwrap_or_else(|| {
                panic!("Lipsește nodul {kind:?} din {file:?} care conține {label:?}.")
            })
    }

    fn editor_navigation_test_model(root: &std::path::Path) -> ProjectModel {
        ProjectModelTestFixture::from_integration_disk_boundary(root)
            .unwrap()
            .build_model()
            .unwrap()
    }

    #[test]
    fn html_attribute_facts_are_read_from_the_canonical_opening_tag() {
        let root = editor_navigation_test_project("source-html-facts");
        let model = editor_navigation_test_model(&root);
        let article = source_node_in_file(
            &model,
            SourceNodeKind::Html,
            "article",
            "templates/index.html",
        );
        let attributes = source_html_attributes(&model, Some(article)).expect("HTML facts");
        assert_eq!(attributes.get("class"), Some(&Some("card".to_string())));
        assert_eq!(attributes.len(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    fn editor_navigation_test_project(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "pana-editor-navigation-{}-{label}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(
            root.join("zola.toml"),
            "base_url = \"http://example.test\"\n",
        )
        .unwrap();
        fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = \"Acasă\"\ntemplate = \"index.html\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("templates/index.html"),
            concat!(
                "<main>\n",
                "{% for item in section.pages %}\n",
                "  <section class=\"grid\"></section>\n",
                "  <article class=\"card\">{{ item.title }}</article>\n",
                "{% endfor %}\n",
                "<footer></footer>\n",
                "{% block sidebar %}<aside></aside>{% endblock %}\n",
                "</main>\n",
            ),
        )
        .unwrap();
        root
    }

    fn editor_navigation_inheritance_test_project(label: &str) -> PathBuf {
        let root = editor_navigation_test_project(label);
        fs::create_dir_all(root.join("templates/partials")).unwrap();
        fs::write(
            root.join("templates/base.html"),
            concat!(
                "<!doctype html><html><body>\n",
                "{% block body %}{% endblock %}\n",
                "</body></html>\n",
            ),
        )
        .unwrap();
        fs::write(
            root.join("templates/layout.html"),
            concat!(
                "{% extends \"base.html\" %}\n",
                "{% block body %}\n",
                "{% include \"partials/header.html\" %}\n",
                "<main>{% block content %}{% endblock %}</main>\n",
                "{% include \"partials/footer.html\" %}\n",
                "{% endblock %}\n",
            ),
        )
        .unwrap();
        fs::write(
            root.join("templates/index.html"),
            concat!(
                "{% extends \"layout.html\" %}\n",
                "{% block title %}{{ section.title }}{% endblock %}\n",
                "{% block description %}{{ config.title }}{% endblock %}\n",
                "{% block css_pagina %}{{ super() }}{% endblock %}\n",
                "{% block scripts %}{{ super() }}{% endblock %}\n",
                "{% block content %}\n",
                "<section class=\"hero\"><h1>Acasă</h1></section>\n",
                "<p>Primul</p><p>Al doilea</p>\n",
                "{% include \"partials/card.html\" %}\n",
                "{% include \"partials/card.html\" %}\n",
                "{% for item in section.pages %}\n",
                "{% if item.title %}<span>A</span><span>B</span>{% endif %}\n",
                "{% endfor %}\n",
                "{{ super() }}\n",
                "{% endblock %}\n",
            ),
        )
        .unwrap();
        fs::write(
            root.join("templates/embedded.html"),
            concat!(
                "<main>\n",
                "{% block promo %}<section class=\"promo\"></section>{% endblock %}\n",
                "</main>\n",
                "{% block title %}{{ config.title }}{% endblock %}\n",
            ),
        )
        .unwrap();
        fs::write(
            root.join("templates/partials/header.html"),
            "<header>Antet</header>\n",
        )
        .unwrap();
        fs::write(
            root.join("templates/partials/footer.html"),
            "<footer>Subsol</footer>\n",
        )
        .unwrap();
        fs::write(
            root.join("templates/partials/card.html"),
            "<article class=\"card\"><h2>Card</h2></article>\n",
        )
        .unwrap();
        fs::write(
            root.join("templates/partials/widget.html"),
            concat!(
                "{% macro widget(value) %}\n",
                "{% if value %}<span>{{ value }}</span>{% endif %}\n",
                "{% endmacro %}\n",
            ),
        )
        .unwrap();
        root
    }

    fn editor_navigation_theme_test_project(label: &str) -> PathBuf {
        let root = editor_navigation_test_project(label);
        fs::create_dir_all(root.join("templates/partials")).unwrap();
        fs::create_dir_all(root.join("themes/test-theme/templates")).unwrap();
        fs::write(
            root.join("zola.toml"),
            "base_url = \"http://example.test\"\ntheme = \"test-theme\"\n",
        )
        .unwrap();
        fs::write(
            root.join("templates/index.html"),
            concat!(
                "{% extends \"base.html\" %}\n",
                "{% block content %}<main>Local</main>{% endblock %}\n",
            ),
        )
        .unwrap();
        fs::write(
            root.join("themes/test-theme/templates/base.html"),
            concat!(
                "<body>\n",
                "{% include \"partials/footer.html\" %}\n",
                "{% block content %}{% endblock %}\n",
                "</body>\n",
            ),
        )
        .unwrap();
        fs::write(
            root.join("themes/test-theme/theme.toml"),
            "name = \"Test Theme\"\n",
        )
        .unwrap();
        fs::write(
            root.join("templates/partials/footer.html"),
            "<footer>Override local</footer>\n",
        )
        .unwrap();
        root
    }
}
