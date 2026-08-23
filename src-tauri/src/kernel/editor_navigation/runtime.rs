#[cfg(test)]
use super::snapshot::build_editor_navigation_node_index;
use super::*;
use super::{
    contracts::{
        MAX_CACHED_EDITOR_NAVIGATION_SNAPSHOTS, MAX_LIVE_EDITOR_MOVE_PLANS,
        MAX_LIVE_EDIT_SCOPE_GRANTS,
    },
    view::same_preview_route,
};

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
    snapshot: Arc<EditorNavigationSnapshot>,
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
    ) -> Result<Option<Arc<EditorNavigationSnapshot>>, String> {
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
            .map(|entry| Arc::clone(&entry.snapshot)))
    }

    pub(crate) fn cache_snapshot(
        &self,
        active_document_path: Option<&str>,
        preview_context_render_instance_id: Option<&str>,
        snapshot: Arc<EditorNavigationSnapshot>,
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
            snapshot,
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

pub(super) fn now_ms() -> u128 {
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
    let planning_nodes = Vec::new();
    let node_index = build_editor_navigation_node_index(&nodes, &planning_nodes);
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
        planning_nodes,
        node_index,
    }
}

fn full_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
