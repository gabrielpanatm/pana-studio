use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};

use crate::{
    kernel::editor_navigation::{
        EditorNavigationEffectScope, EditorNavigationNode, EditorNavigationNodeKind,
        EditorNavigationOrigin, EditorNavigationSnapshot, EditorSourceProvenance,
    },
    preview::CanvasProjectionIdentity,
    project_model::model::ProjectModel,
    source_graph::model::{SourceCapabilityReason, SourceRange},
};

pub const CANVAS_INTERACTION_SCHEMA_VERSION: u32 = 2;
const MAX_AGENT_INSTANCE_ID_BYTES: usize = 128;
const MAX_LIVE_CANVAS_AGENTS: usize = 8;
const MAX_HIT_CANDIDATES: usize = 64;
const MAX_INSTRUMENTED_ID_BYTES: usize = 512;
const MAX_ROUTE_BYTES: usize = 2_048;

/// Identitatea completă a documentului fizic care emite gesturi.
///
/// `canvas` leagă gestul de proiecția semantică Rust, iar `document_epoch`
/// și `agent_instance_id` separă două încărcări ale aceluiași URL. O
/// reîncărcare de iframe trebuie să primească o identitate nouă chiar dacă
/// proiecția Canvas nu s-a schimbat.
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanvasInteractionIdentity {
    pub canvas: CanvasProjectionIdentity,
    pub route: String,
    pub document_epoch: u64,
    pub agent_instance_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum CanvasInteractionGesture {
    PointerMove,
    PointerDown,
    Click,
    ContextMenu,
    DragStart,
    DragOver,
    Drop,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum CanvasPointerButton {
    None,
    Primary,
    Auxiliary,
    Secondary,
    Back,
    Forward,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanvasPointerModifiers {
    pub alt: bool,
    pub control: bool,
    pub meta: bool,
    pub shift: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanvasPointerSample {
    pub client_x: f64,
    pub client_y: f64,
    pub button: CanvasPointerButton,
    pub buttons: u16,
    pub modifiers: CanvasPointerModifiers,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, Hash, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum CanvasHitCandidateKind {
    RenderInstance,
    BoundaryInstance,
}

/// Un fapt observabil în DOM, ordonat de la nodul cel mai adânc spre exterior.
///
/// Agentul nu declară tipul semantic al sursei. El transmite doar ID-ul
/// instrumentat și categoria markerului fizic, iar Rust îl rezolvă în
/// `EditorNavigationSnapshot`.
#[derive(Clone, Debug, Deserialize, Serialize, Eq, Hash, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanvasHitCandidate {
    pub kind: CanvasHitCandidateKind,
    pub id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum CanvasDragPosition {
    Before,
    After,
    Inside,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanvasDragSample {
    pub session_id: String,
    pub position: Option<CanvasDragPosition>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanvasInteractionRequest {
    pub schema_version: u32,
    pub identity: CanvasInteractionIdentity,
    /// Timestamp-ul capturat de agent la emiterea faptului fizic. Rust îl
    /// folosește exclusiv pentru telemetrie end-to-end; nu participă la
    /// autorizare sau la ordonarea gesturilor.
    #[serde(default)]
    pub emitted_at_ms: u64,
    pub gesture_sequence: u64,
    pub gesture: CanvasInteractionGesture,
    pub pointer: CanvasPointerSample,
    pub hit_path: Vec<CanvasHitCandidate>,
    #[serde(default)]
    pub drag: Option<CanvasDragSample>,
}

#[derive(Clone)]
pub struct CanvasInteractionProjection {
    identity: CanvasProjectionIdentity,
    model_revision: String,
    route: String,
    nodes: Vec<EditorNavigationNode>,
    render_nodes: HashMap<String, usize>,
    boundary_nodes: HashMap<String, usize>,
    editor_nodes: HashMap<String, usize>,
}

impl CanvasInteractionProjection {
    pub fn from_snapshot(snapshot: &EditorNavigationSnapshot) -> Self {
        let mut render_nodes = HashMap::new();
        let mut boundary_nodes = HashMap::new();
        let mut editor_nodes = HashMap::new();
        for (index, node) in snapshot.nodes.iter().enumerate() {
            editor_nodes.entry(node.id.clone()).or_insert(index);
            if let Some(render_instance_id) = node.render_instance_id.as_ref() {
                render_nodes
                    .entry(render_instance_id.clone())
                    .or_insert(index);
            }
            if let Some(boundary_instance_id) = node
                .boundary
                .as_ref()
                .map(|boundary| boundary.boundary_instance_id.clone())
            {
                boundary_nodes.entry(boundary_instance_id).or_insert(index);
            }
        }
        Self {
            identity: snapshot.identity.clone(),
            model_revision: snapshot.model_revision.clone(),
            route: snapshot.route.clone(),
            nodes: snapshot.nodes.clone(),
            render_nodes,
            boundary_nodes,
            editor_nodes,
        }
    }

    fn node(&self, index: usize) -> Option<&EditorNavigationNode> {
        self.nodes.get(index)
    }
}

/// Contextul autoritativ furnizat de backend resolverului pur.
///
/// `authorized_edit_scope_id` trebuie să provină dintr-un `EditScopeGrant`
/// verificat de Rust. Nu se preia niciodată direct din mesajul agentului.
pub struct CanvasInteractionContext<'a> {
    pub binding: &'a CanvasInteractionIdentity,
    pub projection: &'a CanvasInteractionProjection,
    pub authorized_edit_scope_id: Option<&'a str>,
    pub last_accepted_sequence: u64,
}

#[derive(Clone, Copy, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum CanvasInteractionStatus {
    Resolved,
    NoTarget,
    Stale,
    Rejected,
}

#[derive(Clone, Copy, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum CanvasInteractionTargetKind {
    HtmlElement,
    TeraBoundary,
    MarkdownBoundary,
    RuntimeElement,
}

#[derive(Clone, Copy, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum CanvasInteractionScopeState {
    Unscoped,
    Locked,
    Authorized,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CanvasInteractionActions {
    pub can_select: bool,
    pub can_inspect: bool,
    pub can_open_in_code: bool,
    pub can_enter_boundary: bool,
    pub can_move_atomic: bool,
    pub can_move: bool,
    pub can_edit_text: bool,
    pub can_edit_attributes: bool,
    pub read_only: bool,
    pub reason_code: Option<SourceCapabilityReason>,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CanvasInteractionTarget {
    pub editor_node_id: String,
    pub kind: CanvasInteractionTargetKind,
    pub label: String,
    pub tag: Option<String>,
    pub source_node_id: Option<String>,
    pub file: Option<String>,
    pub range: Option<SourceRange>,
    pub render_instance_id: Option<String>,
    pub boundary_instance_id: Option<String>,
    pub origin: EditorNavigationOrigin,
    pub theme_name: Option<String>,
    pub source_provenance: EditorSourceProvenance,
    pub required_edit_scope_id: Option<String>,
    pub scope_state: CanvasInteractionScopeState,
    pub effect_scope: EditorNavigationEffectScope,
    pub rendered_instance_count: usize,
    pub actions: CanvasInteractionActions,
}

/// O țintă semantică de geometrie. Agentul măsoară aceste ID-uri în DOM, dar
/// Rust decide ce set reprezintă selecția.
#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CanvasOverlayProjection {
    pub primary_render_instance_id: Option<String>,
    pub render_instance_ids: Vec<String>,
    pub boundary_instance_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CanvasInteractionDiagnosticCode {
    ProtocolVersionMismatch,
    SnapshotBindingMismatch,
    CanvasIdentityMismatch,
    RouteMismatch,
    DocumentEpochMismatch,
    AgentInstanceMismatch,
    AgentBindingMissing,
    GestureSequenceStale,
    InvalidPointer,
    InvalidIdentity,
    HitPathTooLarge,
    InvalidHitCandidate,
    DuplicateHitCandidate,
    InvalidDragSample,
    UnknownHitCandidate,
    CandidateNotSelectable,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CanvasInteractionDiagnostic {
    pub code: CanvasInteractionDiagnosticCode,
    pub message: String,
    pub candidate_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CanvasInteractionReceipt {
    pub schema_version: u32,
    pub identity: CanvasInteractionIdentity,
    pub gesture_sequence: u64,
    pub gesture: CanvasInteractionGesture,
    pub status: CanvasInteractionStatus,
    pub target: Option<CanvasInteractionTarget>,
    pub overlay: Option<CanvasOverlayProjection>,
    /// Poziția indicatorului este proiectată numai pentru un `DragOver`
    /// rezolvat. Bridge-ul nu decide singur dacă un fapt fizic devine
    /// prezentare de mutare.
    pub drag_position: Option<CanvasDragPosition>,
    pub diagnostics: Vec<CanvasInteractionDiagnostic>,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CanvasInteractionBindingReceipt {
    pub schema_version: u32,
    pub identity: CanvasInteractionIdentity,
    pub last_accepted_sequence: u64,
    pub active_document_path: Option<String>,
}

struct LiveCanvasAgent {
    identity: CanvasInteractionIdentity,
    snapshot: EditorNavigationSnapshot,
    model: Option<ProjectModel>,
    projection: CanvasInteractionProjection,
    active_document_path: Option<String>,
    last_accepted_ordered_sequence: u64,
    last_accepted_hover_sequence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanvasInteractionScopeContext {
    pub identity: CanvasProjectionIdentity,
    pub model_revision: String,
    pub route: String,
    pub active_document_path: Option<String>,
}

#[derive(Clone)]
pub struct CanvasInteractionSelectionContext {
    pub snapshot: EditorNavigationSnapshot,
    pub active_document_path: Option<String>,
}

#[derive(Clone)]
pub struct CanvasInteractionPlanningContext {
    pub snapshot: EditorNavigationSnapshot,
    pub model: ProjectModel,
    pub active_document_path: Option<String>,
}

#[derive(Default)]
pub struct CanvasInteractionRuntime {
    agents: Mutex<Vec<LiveCanvasAgent>>,
}

impl CanvasInteractionRuntime {
    /// Instalează autoritatea fizică pentru o suprafață Canvas deja validată.
    ///
    /// Repetarea aceluiași bind este idempotentă și nu resetează secvența.
    /// Pentru aceeași suprafață, un document nou trebuie să aibă un epoch
    /// strict mai mare decât documentul pe care îl înlocuiește.
    pub fn bind_agent(
        &self,
        snapshot: &EditorNavigationSnapshot,
        active_document_path: Option<&str>,
        identity: CanvasInteractionIdentity,
    ) -> Result<CanvasInteractionBindingReceipt, String> {
        self.bind_agent_inner(snapshot, None, active_document_path, identity)
    }

    /// Leagă agentul de snapshot-ul și modelul deja validate de Rust.
    ///
    /// Modelul rămâne în registrul efemer al documentului fizic și permite
    /// ca DragOver să rezolve ținta plus PlanEditorMove într-o singură
    /// secțiune atomică, fără o nouă captură ProjectWorkspace.
    pub fn bind_agent_with_model(
        &self,
        snapshot: &EditorNavigationSnapshot,
        model: &ProjectModel,
        active_document_path: Option<&str>,
        identity: CanvasInteractionIdentity,
    ) -> Result<CanvasInteractionBindingReceipt, String> {
        self.bind_agent_inner(
            snapshot,
            Some(model.clone()),
            active_document_path,
            identity,
        )
    }

    fn bind_agent_inner(
        &self,
        snapshot: &EditorNavigationSnapshot,
        model: Option<ProjectModel>,
        active_document_path: Option<&str>,
        identity: CanvasInteractionIdentity,
    ) -> Result<CanvasInteractionBindingReceipt, String> {
        if snapshot.identity != identity.canvas || snapshot.route != identity.route {
            return Err(
                "CanvasAgent nu poate fi legat de alt EditorNavigationSnapshot.".to_string(),
            );
        }
        if let Some((_, message)) = validate_binding(&identity) {
            return Err(message.to_string());
        }

        let mut agents = self
            .agents
            .lock()
            .map_err(|_| "Registrul CanvasAgent este indisponibil.".to_string())?;
        if let Some(existing) = agents.iter().find(|agent| agent.identity == identity) {
            return Ok(CanvasInteractionBindingReceipt {
                schema_version: CANVAS_INTERACTION_SCHEMA_VERSION,
                identity,
                last_accepted_sequence: existing
                    .last_accepted_ordered_sequence
                    .max(existing.last_accepted_hover_sequence),
                active_document_path: existing.active_document_path.clone(),
            });
        }

        if let Some(existing) = agents
            .iter()
            .find(|agent| same_canvas_surface(&agent.identity, &identity))
        {
            // Fiecare proiecție distinctă de pe aceeași suprafață primește de
            // la controller un epoch nou. Verificarea trebuie făcută înainte
            // de `retain`, astfel încât un bind superseded care termină târziu
            // să nu poată înlocui proiecția mai nouă din registrul Rust.
            if identity.document_epoch <= existing.identity.document_epoch {
                return Err(
                    "CanvasAgent a refuzat un document epoch vechi sau reutilizat.".to_string(),
                );
            }
        }

        agents.retain(|agent| !same_canvas_surface(&agent.identity, &identity));
        if agents.len() >= MAX_LIVE_CANVAS_AGENTS {
            agents.remove(0);
        }
        agents.push(LiveCanvasAgent {
            identity: identity.clone(),
            snapshot: snapshot.clone(),
            model,
            projection: CanvasInteractionProjection::from_snapshot(snapshot),
            active_document_path: active_document_path.map(str::to_string),
            last_accepted_ordered_sequence: 0,
            last_accepted_hover_sequence: 0,
        });
        Ok(CanvasInteractionBindingReceipt {
            schema_version: CANVAS_INTERACTION_SCHEMA_VERSION,
            identity,
            last_accepted_sequence: 0,
            active_document_path: active_document_path.map(str::to_string),
        })
    }

    /// Rezolvă DragOver și execută planificarea semantică sub aceeași
    /// ordonare Rust. Callback-ul primește numai snapshot-ul și ProjectModel
    /// fixate la bind; nu poate consulta o revizie implicită mai nouă.
    pub fn resolve_drag_over<T>(
        &self,
        authorized_edit_scope_id: Option<&str>,
        request: &CanvasInteractionRequest,
        project: impl FnOnce(
            &EditorNavigationSnapshot,
            &ProjectModel,
            Option<&str>,
            &CanvasInteractionReceipt,
        ) -> Result<T, String>,
    ) -> Result<(CanvasInteractionReceipt, Option<T>), String> {
        if request.gesture != CanvasInteractionGesture::DragOver {
            return Err("Lane-ul Canvas drag acceptă numai gesturi DragOver.".to_string());
        }
        let mut agents = self
            .agents
            .lock()
            .map_err(|_| "Registrul CanvasAgent este indisponibil.".to_string())?;
        let Some(agent) = agents
            .iter_mut()
            .find(|agent| agent.identity == request.identity)
        else {
            return Ok((
                receipt_with_diagnostic(
                    request,
                    CanvasInteractionStatus::Stale,
                    CanvasInteractionDiagnosticCode::AgentBindingMissing,
                    "Gestul nu aparține niciunui CanvasAgent activ.",
                    None,
                ),
                None,
            ));
        };
        let receipt = resolve_canvas_interaction(
            CanvasInteractionContext {
                binding: &agent.identity,
                projection: &agent.projection,
                authorized_edit_scope_id,
                last_accepted_sequence: agent.last_accepted_ordered_sequence,
            },
            request,
        );
        if !matches!(
            receipt.status,
            CanvasInteractionStatus::Resolved | CanvasInteractionStatus::NoTarget
        ) {
            return Ok((receipt, None));
        }
        agent.last_accepted_ordered_sequence = request.gesture_sequence;
        let Some(model) = agent.model.as_ref() else {
            return Err(
                "CanvasAgent nu are ProjectModel-ul Rust fixat pentru DragOver.".to_string(),
            );
        };
        let projection = project(
            &agent.snapshot,
            model,
            agent.active_document_path.as_deref(),
            &receipt,
        )?;
        Ok((receipt, Some(projection)))
    }

    /// Rezolvă și consumă atomic secvența gestului. Chiar și un `NoTarget`
    /// consumă secvența, deoarece reprezintă un eveniment fizic valid.
    pub fn resolve(
        &self,
        authorized_edit_scope_id: Option<&str>,
        request: &CanvasInteractionRequest,
    ) -> Result<CanvasInteractionReceipt, String> {
        if request.gesture == CanvasInteractionGesture::PointerMove {
            return Err(
                "PointerMove trebuie rezolvat exclusiv prin lane-ul Canvas hover.".to_string(),
            );
        }
        let mut agents = self
            .agents
            .lock()
            .map_err(|_| "Registrul CanvasAgent este indisponibil.".to_string())?;
        let Some(agent) = agents
            .iter_mut()
            .find(|agent| agent.identity == request.identity)
        else {
            return Ok(receipt_with_diagnostic(
                request,
                CanvasInteractionStatus::Stale,
                CanvasInteractionDiagnosticCode::AgentBindingMissing,
                "Gestul nu aparține niciunui CanvasAgent activ.",
                None,
            ));
        };
        let receipt = resolve_canvas_interaction(
            CanvasInteractionContext {
                binding: &agent.identity,
                projection: &agent.projection,
                authorized_edit_scope_id,
                last_accepted_sequence: agent.last_accepted_ordered_sequence,
            },
            request,
        );
        if matches!(
            receipt.status,
            CanvasInteractionStatus::Resolved | CanvasInteractionStatus::NoTarget
        ) {
            agent.last_accepted_ordered_sequence = request.gesture_sequence;
        }
        Ok(receipt)
    }

    /// Rezolvă un hover fizic și păstrează mutex-ul agentului până când
    /// proiecția semantică Rust a fost aplicată.
    ///
    /// Astfel, o secvență mai veche nu poate reveni după una mai nouă și
    /// suprascrie HoverSnapshot. `project` este apelat numai pentru un
    /// PointerMove acceptat (`Resolved` sau `NoTarget`).
    pub fn resolve_pointer_hover<T>(
        &self,
        authorized_edit_scope_id: Option<&str>,
        request: &CanvasInteractionRequest,
        project: impl FnOnce(
            &EditorNavigationSnapshot,
            Option<&str>,
            &CanvasInteractionReceipt,
        ) -> Result<T, String>,
    ) -> Result<(CanvasInteractionReceipt, Option<T>), String> {
        if request.gesture != CanvasInteractionGesture::PointerMove {
            return Err("Lane-ul Canvas hover acceptă numai gesturi PointerMove.".to_string());
        }
        let mut agents = self
            .agents
            .lock()
            .map_err(|_| "Registrul CanvasAgent este indisponibil.".to_string())?;
        let Some(agent) = agents
            .iter_mut()
            .find(|agent| agent.identity == request.identity)
        else {
            return Ok((
                receipt_with_diagnostic(
                    request,
                    CanvasInteractionStatus::Stale,
                    CanvasInteractionDiagnosticCode::AgentBindingMissing,
                    "Gestul nu aparține niciunui CanvasAgent activ.",
                    None,
                ),
                None,
            ));
        };
        let receipt = resolve_canvas_interaction(
            CanvasInteractionContext {
                binding: &agent.identity,
                projection: &agent.projection,
                authorized_edit_scope_id,
                last_accepted_sequence: agent.last_accepted_hover_sequence,
            },
            request,
        );
        if !matches!(
            receipt.status,
            CanvasInteractionStatus::Resolved | CanvasInteractionStatus::NoTarget
        ) {
            return Ok((receipt, None));
        }

        agent.last_accepted_hover_sequence = request.gesture_sequence;
        let projection = project(
            &agent.snapshot,
            agent.active_document_path.as_deref(),
            &receipt,
        )?;
        Ok((receipt, Some(projection)))
    }

    pub fn scope_context(
        &self,
        identity: &CanvasInteractionIdentity,
    ) -> Result<CanvasInteractionScopeContext, String> {
        let agents = self
            .agents
            .lock()
            .map_err(|_| "Registrul CanvasAgent este indisponibil.".to_string())?;
        let agent = agents
            .iter()
            .find(|agent| agent.identity == *identity)
            .ok_or_else(|| "EditScopeGrant nu aparține CanvasAgent-ului activ.".to_string())?;
        Ok(CanvasInteractionScopeContext {
            identity: agent.projection.identity.clone(),
            model_revision: agent.projection.model_revision.clone(),
            route: agent.projection.route.clone(),
            active_document_path: agent.active_document_path.clone(),
        })
    }

    pub fn selection_context(
        &self,
        identity: &CanvasProjectionIdentity,
        route: &str,
    ) -> Result<Option<CanvasInteractionSelectionContext>, String> {
        let agents = self
            .agents
            .lock()
            .map_err(|_| "Registrul CanvasAgent este indisponibil.".to_string())?;
        Ok(agents
            .iter()
            .find(|agent| {
                agent.projection.identity == *identity
                    && agent.projection.route == route
                    && agent.snapshot.identity == *identity
                    && agent.snapshot.route == route
            })
            .map(|agent| CanvasInteractionSelectionContext {
                snapshot: agent.snapshot.clone(),
                active_document_path: agent.active_document_path.clone(),
            }))
    }

    pub fn planning_context(
        &self,
        identity: &CanvasProjectionIdentity,
        route: &str,
    ) -> Result<CanvasInteractionPlanningContext, String> {
        let agents = self
            .agents
            .lock()
            .map_err(|_| "Registrul CanvasAgent este indisponibil.".to_string())?;
        let agent = agents
            .iter()
            .find(|agent| {
                agent.projection.identity == *identity
                    && agent.projection.route == route
                    && agent.snapshot.identity == *identity
                    && agent.snapshot.route == route
            })
            .ok_or_else(|| {
                "PlanEditorMove nu aparține niciunui CanvasAgent Rust activ.".to_string()
            })?;
        let model = agent.model.clone().ok_or_else(|| {
            "CanvasAgent nu are ProjectModel-ul Rust fixat pentru commit.".to_string()
        })?;
        if model.revision != agent.snapshot.model_revision {
            return Err(
                "CanvasAgent a refuzat un ProjectModel diferit de snapshot-ul fixat.".to_string(),
            );
        }
        Ok(CanvasInteractionPlanningContext {
            snapshot: agent.snapshot.clone(),
            model,
            active_document_path: agent.active_document_path.clone(),
        })
    }

    pub fn revoke_all(&self) {
        if let Ok(mut agents) = self.agents.lock() {
            agents.clear();
        }
    }
}

/// Rezolvă un gest fizic într-o proiecție semantică fără I/O și fără stare
/// mutabilă. Caller-ul actualizează `last_accepted_sequence` numai pentru
/// recepții `Resolved` sau `NoTarget`.
pub fn resolve_canvas_interaction(
    context: CanvasInteractionContext<'_>,
    request: &CanvasInteractionRequest,
) -> CanvasInteractionReceipt {
    if context.projection.identity != context.binding.canvas
        || context.projection.route != context.binding.route
    {
        return receipt_with_diagnostic(
            request,
            CanvasInteractionStatus::Stale,
            CanvasInteractionDiagnosticCode::SnapshotBindingMismatch,
            "Binding-ul CanvasAgent nu aparține EditorNavigationSnapshot-ului curent.",
            None,
        );
    }

    if request.schema_version != CANVAS_INTERACTION_SCHEMA_VERSION {
        return receipt_with_diagnostic(
            request,
            CanvasInteractionStatus::Rejected,
            CanvasInteractionDiagnosticCode::ProtocolVersionMismatch,
            "CanvasAgent folosește o versiune incompatibilă a protocolului.",
            None,
        );
    }

    if let Some((code, message)) = validate_binding(context.binding) {
        return receipt_with_diagnostic(
            request,
            CanvasInteractionStatus::Rejected,
            code,
            message,
            None,
        );
    }

    if request.identity.canvas != context.binding.canvas {
        return receipt_with_diagnostic(
            request,
            CanvasInteractionStatus::Stale,
            CanvasInteractionDiagnosticCode::CanvasIdentityMismatch,
            "Gestul aparține altei proiecții Canvas.",
            None,
        );
    }
    if request.identity.route != context.binding.route {
        return receipt_with_diagnostic(
            request,
            CanvasInteractionStatus::Stale,
            CanvasInteractionDiagnosticCode::RouteMismatch,
            "Gestul aparține altei rute Preview.",
            None,
        );
    }
    if request.identity.document_epoch != context.binding.document_epoch {
        return receipt_with_diagnostic(
            request,
            CanvasInteractionStatus::Stale,
            CanvasInteractionDiagnosticCode::DocumentEpochMismatch,
            "Gestul aparține altei încărcări a documentului Preview.",
            None,
        );
    }
    if request.identity.agent_instance_id != context.binding.agent_instance_id {
        return receipt_with_diagnostic(
            request,
            CanvasInteractionStatus::Stale,
            CanvasInteractionDiagnosticCode::AgentInstanceMismatch,
            "Gestul aparține altei instanțe CanvasAgent.",
            None,
        );
    }
    if request.gesture_sequence == 0 || request.gesture_sequence <= context.last_accepted_sequence {
        return receipt_with_diagnostic(
            request,
            CanvasInteractionStatus::Stale,
            CanvasInteractionDiagnosticCode::GestureSequenceStale,
            "Ordinea gestului este stale sau a fost deja consumată.",
            None,
        );
    }
    if !request.pointer.client_x.is_finite() || !request.pointer.client_y.is_finite() {
        return receipt_with_diagnostic(
            request,
            CanvasInteractionStatus::Rejected,
            CanvasInteractionDiagnosticCode::InvalidPointer,
            "CanvasAgent a transmis coordonate nefinite.",
            None,
        );
    }
    if request.hit_path.len() > MAX_HIT_CANDIDATES {
        return receipt_with_diagnostic(
            request,
            CanvasInteractionStatus::Rejected,
            CanvasInteractionDiagnosticCode::HitPathTooLarge,
            "Calea fizică depășește limita protocolului Canvas Interaction.",
            None,
        );
    }
    let drag_shape_valid = match request.gesture {
        CanvasInteractionGesture::DragStart => request.drag.as_ref().is_some_and(|drag| {
            !drag.session_id.trim().is_empty()
                && drag.session_id.len() <= MAX_AGENT_INSTANCE_ID_BYTES
                && drag.position.is_none()
        }),
        CanvasInteractionGesture::DragOver | CanvasInteractionGesture::Drop => {
            request.drag.as_ref().is_some_and(|drag| {
                !drag.session_id.trim().is_empty()
                    && drag.session_id.len() <= MAX_AGENT_INSTANCE_ID_BYTES
                    && drag.position.is_some()
            })
        }
        _ => request.drag.is_none(),
    };
    if !drag_shape_valid {
        return receipt_with_diagnostic(
            request,
            CanvasInteractionStatus::Rejected,
            CanvasInteractionDiagnosticCode::InvalidDragSample,
            "CanvasAgent a transmis un context drag incompatibil cu gestul.",
            None,
        );
    }

    let mut seen = HashSet::with_capacity(request.hit_path.len());
    for candidate in &request.hit_path {
        if candidate.id.trim().is_empty() || candidate.id.len() > MAX_INSTRUMENTED_ID_BYTES {
            return receipt_with_diagnostic(
                request,
                CanvasInteractionStatus::Rejected,
                CanvasInteractionDiagnosticCode::InvalidHitCandidate,
                "CanvasAgent a transmis un ID instrumentat invalid.",
                Some(candidate.id.clone()),
            );
        }
        if !seen.insert((candidate.kind, candidate.id.as_str())) {
            return receipt_with_diagnostic(
                request,
                CanvasInteractionStatus::Rejected,
                CanvasInteractionDiagnosticCode::DuplicateHitCandidate,
                "Calea fizică conține același candidat de mai multe ori.",
                Some(candidate.id.clone()),
            );
        }
    }

    let mut diagnostics = Vec::new();
    for candidate in &request.hit_path {
        let Some(node) = node_for_candidate(&context, candidate) else {
            diagnostics.push(CanvasInteractionDiagnostic {
                code: CanvasInteractionDiagnosticCode::UnknownHitCandidate,
                message: "ID-ul fizic nu există în proiecția semantică activă.".to_string(),
                candidate_id: Some(candidate.id.clone()),
            });
            continue;
        };
        let node = closed_boundary_or_node(&context, node);
        if !node.capabilities.can_select {
            diagnostics.push(CanvasInteractionDiagnostic {
                code: CanvasInteractionDiagnosticCode::CandidateNotSelectable,
                message: "Candidatul există, dar capabilitățile Rust nu permit selecția."
                    .to_string(),
                candidate_id: Some(candidate.id.clone()),
            });
            continue;
        }

        let target = project_target(&context, node);
        let overlay = project_overlay(node);
        let drag_position = match request.gesture {
            CanvasInteractionGesture::DragOver => {
                request.drag.as_ref().and_then(|drag| drag.position)
            }
            _ => None,
        };
        return CanvasInteractionReceipt {
            schema_version: CANVAS_INTERACTION_SCHEMA_VERSION,
            identity: request.identity.clone(),
            gesture_sequence: request.gesture_sequence,
            gesture: request.gesture,
            status: CanvasInteractionStatus::Resolved,
            target: Some(target),
            overlay: Some(overlay),
            drag_position,
            diagnostics,
        };
    }

    CanvasInteractionReceipt {
        schema_version: CANVAS_INTERACTION_SCHEMA_VERSION,
        identity: request.identity.clone(),
        gesture_sequence: request.gesture_sequence,
        gesture: request.gesture,
        status: CanvasInteractionStatus::NoTarget,
        target: None,
        overlay: None,
        drag_position: None,
        diagnostics,
    }
}

fn validate_binding(
    binding: &CanvasInteractionIdentity,
) -> Option<(CanvasInteractionDiagnosticCode, &'static str)> {
    if binding.document_epoch == 0
        || binding.agent_instance_id.trim().is_empty()
        || binding.agent_instance_id.len() > MAX_AGENT_INSTANCE_ID_BYTES
        || binding.route.trim().is_empty()
        || binding.route.len() > MAX_ROUTE_BYTES
    {
        return Some((
            CanvasInteractionDiagnosticCode::InvalidIdentity,
            "Binding-ul CanvasAgent conține o identitate invalidă.",
        ));
    }
    None
}

fn same_canvas_surface(
    left: &CanvasInteractionIdentity,
    right: &CanvasInteractionIdentity,
) -> bool {
    left.canvas.project_root == right.canvas.project_root
        && left.canvas.runtime_session_id == right.canvas.runtime_session_id
        && left.route == right.route
}

fn node_for_candidate<'a>(
    context: &'a CanvasInteractionContext<'_>,
    candidate: &CanvasHitCandidate,
) -> Option<&'a EditorNavigationNode> {
    let index = match candidate.kind {
        CanvasHitCandidateKind::RenderInstance => {
            context.projection.render_nodes.get(&candidate.id)
        }
        CanvasHitCandidateKind::BoundaryInstance => {
            context.projection.boundary_nodes.get(&candidate.id)
        }
    }?;
    context.projection.node(*index)
}

fn closed_boundary_or_node<'a>(
    context: &'a CanvasInteractionContext<'_>,
    node: &'a EditorNavigationNode,
) -> &'a EditorNavigationNode {
    let Some(required_scope_id) = node.capabilities.requires_edit_scope_id.as_deref() else {
        return node;
    };
    if context.authorized_edit_scope_id == Some(required_scope_id) {
        return node;
    }
    context
        .projection
        .editor_nodes
        .get(required_scope_id)
        .and_then(|index| context.projection.node(*index))
        .filter(|candidate| {
            matches!(
                candidate.kind,
                EditorNavigationNodeKind::TeraBoundary | EditorNavigationNodeKind::MarkdownBoundary
            )
        })
        .unwrap_or(node)
}

fn project_target(
    context: &CanvasInteractionContext<'_>,
    node: &EditorNavigationNode,
) -> CanvasInteractionTarget {
    let required_edit_scope_id = node.capabilities.requires_edit_scope_id.clone();
    let scope_state = match required_edit_scope_id.as_deref() {
        None => CanvasInteractionScopeState::Unscoped,
        Some(scope_id) if context.authorized_edit_scope_id == Some(scope_id) => {
            CanvasInteractionScopeState::Authorized
        }
        Some(_) => CanvasInteractionScopeState::Locked,
    };
    let scope_boundary = required_edit_scope_id
        .as_deref()
        .and_then(|scope_id| context.projection.editor_nodes.get(scope_id))
        .and_then(|index| context.projection.node(*index))
        .and_then(|item| item.boundary.as_ref());
    let boundary = node.boundary.as_ref();
    let effect_scope = boundary
        .map(|item| item.effect_scope)
        .or_else(|| scope_boundary.map(|item| item.effect_scope))
        .unwrap_or(EditorNavigationEffectScope::SingleSource);
    let rendered_instance_count = boundary
        .map(|item| item.rendered_instance_count)
        .or_else(|| scope_boundary.map(|item| item.rendered_instance_count))
        .unwrap_or(1);

    CanvasInteractionTarget {
        editor_node_id: node.id.clone(),
        kind: match node.kind {
            EditorNavigationNodeKind::HtmlElement => CanvasInteractionTargetKind::HtmlElement,
            EditorNavigationNodeKind::TeraBoundary => CanvasInteractionTargetKind::TeraBoundary,
            EditorNavigationNodeKind::MarkdownBoundary => {
                CanvasInteractionTargetKind::MarkdownBoundary
            }
            EditorNavigationNodeKind::RuntimeElement => CanvasInteractionTargetKind::RuntimeElement,
        },
        label: node.label.clone(),
        tag: node.tag.clone(),
        source_node_id: node.source_node_id.clone(),
        file: node.file.clone(),
        range: node.range.clone(),
        render_instance_id: node.render_instance_id.clone(),
        boundary_instance_id: boundary.map(|item| item.boundary_instance_id.clone()),
        origin: node.origin,
        theme_name: node.theme_name.clone(),
        source_provenance: node.source_provenance.clone(),
        required_edit_scope_id,
        scope_state,
        effect_scope,
        rendered_instance_count,
        actions: CanvasInteractionActions {
            can_select: node.capabilities.can_select,
            can_inspect: node.capabilities.can_inspect,
            can_open_in_code: node.capabilities.can_open_in_code,
            can_enter_boundary: node.capabilities.can_enter_boundary,
            can_move_atomic: node.capabilities.can_move_atomic,
            can_move: node.capabilities.can_move,
            can_edit_text: node.capabilities.can_edit_text,
            can_edit_attributes: node.capabilities.can_edit_attributes,
            read_only: node.capabilities.read_only,
            reason_code: node.capabilities.reason_code,
        },
    }
}

fn project_overlay(node: &EditorNavigationNode) -> CanvasOverlayProjection {
    if let Some(boundary) = node.boundary.as_ref() {
        return CanvasOverlayProjection {
            primary_render_instance_id: boundary.root_render_instance_ids.first().cloned(),
            render_instance_ids: boundary.root_render_instance_ids.clone(),
            boundary_instance_id: Some(boundary.boundary_instance_id.clone()),
        };
    }
    CanvasOverlayProjection {
        primary_render_instance_id: node.render_instance_id.clone(),
        render_instance_ids: node.render_instance_id.iter().cloned().collect(),
        boundary_instance_id: None,
    }
}

fn receipt_with_diagnostic(
    request: &CanvasInteractionRequest,
    status: CanvasInteractionStatus,
    code: CanvasInteractionDiagnosticCode,
    message: &str,
    candidate_id: Option<String>,
) -> CanvasInteractionReceipt {
    CanvasInteractionReceipt {
        schema_version: CANVAS_INTERACTION_SCHEMA_VERSION,
        identity: request.identity.clone(),
        gesture_sequence: request.gesture_sequence,
        gesture: request.gesture,
        status,
        target: None,
        overlay: None,
        drag_position: None,
        diagnostics: vec![CanvasInteractionDiagnostic {
            code,
            message: message.to_string(),
            candidate_id,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        kernel::editor_navigation::{
            editor_navigation_snapshot_for_test, EditorNavigationBoundary,
            EditorNavigationCapabilities, EditorNavigationSurface,
        },
        kernel::selection_coordinator::{SelectionCoordinatorRuntime, SelectionIntent},
        source_graph::model::{SourceCapabilityReason, SourceNodeKind},
    };

    fn canvas_identity() -> CanvasProjectionIdentity {
        CanvasProjectionIdentity {
            project_root: "/project".to_string(),
            runtime_session_id: "runtime-1".to_string(),
            workspace_revision: 7,
            transaction_id: "canvas-transaction-1".to_string(),
            preview_revision: "preview-7".to_string(),
        }
    }

    fn interaction_identity() -> CanvasInteractionIdentity {
        CanvasInteractionIdentity {
            canvas: canvas_identity(),
            route: "/".to_string(),
            document_epoch: 3,
            agent_instance_id: "agent-3".to_string(),
        }
    }

    fn capabilities(
        requires_edit_scope_id: Option<&str>,
        can_enter_boundary: bool,
    ) -> EditorNavigationCapabilities {
        EditorNavigationCapabilities {
            can_select: true,
            can_inspect: true,
            can_open_in_code: true,
            can_enter_boundary,
            can_move_atomic: can_enter_boundary,
            can_move: requires_edit_scope_id.is_none(),
            can_edit_text: requires_edit_scope_id.is_none(),
            can_edit_attributes: requires_edit_scope_id.is_none(),
            read_only: requires_edit_scope_id.is_some(),
            requires_edit_scope_id: requires_edit_scope_id.map(str::to_string),
            reason_code: None,
        }
    }

    fn source_provenance(
        source_node_id: &str,
        file: &str,
        source_kind: SourceNodeKind,
    ) -> EditorSourceProvenance {
        EditorSourceProvenance {
            definition: Some(crate::kernel::editor_navigation::EditorSourceReference {
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
            resolution: crate::kernel::editor_navigation::EditorSourceResolution::Direct,
        }
    }

    fn boundary_node() -> EditorNavigationNode {
        EditorNavigationNode {
            id: "editor_boundary:boundary-1".to_string(),
            parent_id: None,
            children: vec!["editor_render:render-1".to_string()],
            order: 0,
            kind: EditorNavigationNodeKind::TeraBoundary,
            label: "for card in cards".to_string(),
            tag: None,
            source_node_id: Some("source-for-1".to_string()),
            render_instance_id: None,
            source_kind: Some(SourceNodeKind::For),
            file: Some("templates/index.html".to_string()),
            range: None,
            origin: EditorNavigationOrigin::Project,
            theme_name: None,
            source_provenance: source_provenance(
                "source-for-1",
                "templates/index.html",
                SourceNodeKind::For,
            ),
            provenance_stack: vec!["source-for-1".to_string()],
            component_definition_ids: Vec::new(),
            component_invocation_ids: Vec::new(),
            block_definition_ids: Vec::new(),
            block_source_instance_ids: Vec::new(),
            binding_key: Some("card".to_string()),
            binding_path: Some("cards[0]".to_string()),
            boundary: Some(EditorNavigationBoundary {
                boundary_instance_id: "boundary-1".to_string(),
                source_node_id: "source-for-1".to_string(),
                root_render_instance_ids: vec!["render-1".to_string()],
                atomic_when_closed: true,
                effect_scope: EditorNavigationEffectScope::AllRenderedInstances,
                rendered_instance_count: 4,
                target: None,
                empty: false,
            }),
            capabilities: capabilities(Some("editor_boundary:boundary-1"), true),
        }
    }

    fn html_node() -> EditorNavigationNode {
        EditorNavigationNode {
            id: "editor_render:render-1".to_string(),
            parent_id: Some("editor_boundary:boundary-1".to_string()),
            children: Vec::new(),
            order: 1,
            kind: EditorNavigationNodeKind::HtmlElement,
            label: "<article>".to_string(),
            tag: Some("article".to_string()),
            source_node_id: Some("source-html-1".to_string()),
            render_instance_id: Some("render-1".to_string()),
            source_kind: Some(SourceNodeKind::Html),
            file: Some("templates/index.html".to_string()),
            range: None,
            origin: EditorNavigationOrigin::Project,
            theme_name: None,
            source_provenance: source_provenance(
                "source-html-1",
                "templates/index.html",
                SourceNodeKind::Html,
            ),
            provenance_stack: vec!["source-for-1".to_string(), "source-html-1".to_string()],
            component_definition_ids: Vec::new(),
            component_invocation_ids: Vec::new(),
            block_definition_ids: Vec::new(),
            block_source_instance_ids: Vec::new(),
            binding_key: Some("card".to_string()),
            binding_path: Some("cards[0]".to_string()),
            boundary: None,
            capabilities: capabilities(Some("editor_boundary:boundary-1"), false),
        }
    }

    fn plain_html_node() -> EditorNavigationNode {
        EditorNavigationNode {
            id: "editor_render:render-root".to_string(),
            parent_id: None,
            children: Vec::new(),
            order: 2,
            kind: EditorNavigationNodeKind::HtmlElement,
            label: "<main>".to_string(),
            tag: Some("main".to_string()),
            source_node_id: Some("source-main".to_string()),
            render_instance_id: Some("render-root".to_string()),
            source_kind: Some(SourceNodeKind::Html),
            file: Some("templates/index.html".to_string()),
            range: None,
            origin: EditorNavigationOrigin::Project,
            theme_name: None,
            source_provenance: source_provenance(
                "source-main",
                "templates/index.html",
                SourceNodeKind::Html,
            ),
            provenance_stack: vec!["source-main".to_string()],
            component_definition_ids: Vec::new(),
            component_invocation_ids: Vec::new(),
            block_definition_ids: Vec::new(),
            block_source_instance_ids: Vec::new(),
            binding_key: None,
            binding_path: None,
            boundary: None,
            capabilities: capabilities(None, false),
        }
    }

    fn snapshot() -> EditorNavigationSnapshot {
        editor_navigation_snapshot_for_test(
            canvas_identity(),
            "model-7",
            "/",
            EditorNavigationSurface::CanonicalPreview,
            vec![
                "editor_boundary:boundary-1".to_string(),
                "editor_render:render-root".to_string(),
            ],
            vec![boundary_node(), html_node(), plain_html_node()],
        )
    }

    fn markdown_snapshot() -> EditorNavigationSnapshot {
        let markdown_scope = "editor_boundary:markdown-1";
        let mut boundary = boundary_node();
        boundary.id = markdown_scope.to_string();
        boundary.children = vec!["editor_render:markdown-render".to_string()];
        boundary.kind = EditorNavigationNodeKind::MarkdownBoundary;
        boundary.label = "Conținut Markdown".to_string();
        boundary.file = Some("content/_index.md".to_string());
        boundary.boundary.as_mut().unwrap().boundary_instance_id = "markdown-1".to_string();
        boundary.boundary.as_mut().unwrap().root_render_instance_ids =
            vec!["markdown-render".to_string()];
        boundary.capabilities = EditorNavigationCapabilities {
            can_select: true,
            can_inspect: true,
            can_open_in_code: true,
            can_enter_boundary: false,
            can_move_atomic: false,
            can_move: false,
            can_edit_text: false,
            can_edit_attributes: false,
            read_only: true,
            requires_edit_scope_id: None,
            reason_code: Some(SourceCapabilityReason::MarkdownRenderedBoundary),
        };

        let mut descendant = html_node();
        descendant.id = "editor_render:markdown-render".to_string();
        descendant.parent_id = Some(markdown_scope.to_string());
        descendant.render_instance_id = Some("markdown-render".to_string());
        descendant.capabilities = EditorNavigationCapabilities {
            can_select: true,
            can_inspect: true,
            can_open_in_code: false,
            can_enter_boundary: false,
            can_move_atomic: false,
            can_move: false,
            can_edit_text: false,
            can_edit_attributes: false,
            read_only: true,
            requires_edit_scope_id: Some(markdown_scope.to_string()),
            reason_code: Some(SourceCapabilityReason::MarkdownRenderedBoundary),
        };

        editor_navigation_snapshot_for_test(
            canvas_identity(),
            "model-7",
            "/",
            EditorNavigationSurface::CanonicalPreview,
            vec![markdown_scope.to_string()],
            vec![boundary, descendant],
        )
    }

    fn request(id: &str) -> CanvasInteractionRequest {
        CanvasInteractionRequest {
            schema_version: CANVAS_INTERACTION_SCHEMA_VERSION,
            identity: interaction_identity(),
            emitted_at_ms: 0,
            gesture_sequence: 9,
            gesture: CanvasInteractionGesture::Click,
            pointer: CanvasPointerSample {
                client_x: 120.0,
                client_y: 80.0,
                button: CanvasPointerButton::Primary,
                buttons: 0,
                modifiers: CanvasPointerModifiers::default(),
            },
            hit_path: vec![CanvasHitCandidate {
                kind: CanvasHitCandidateKind::RenderInstance,
                id: id.to_string(),
            }],
            drag: None,
        }
    }

    fn pointer_request(id: &str, gesture_sequence: u64) -> CanvasInteractionRequest {
        let mut request = request(id);
        request.gesture = CanvasInteractionGesture::PointerMove;
        request.gesture_sequence = gesture_sequence;
        request.pointer.button = CanvasPointerButton::None;
        request
    }

    fn resolve(
        snapshot: &EditorNavigationSnapshot,
        request: &CanvasInteractionRequest,
        scope: Option<&str>,
        last_sequence: u64,
    ) -> CanvasInteractionReceipt {
        let binding = interaction_identity();
        let projection = CanvasInteractionProjection::from_snapshot(snapshot);
        resolve_canvas_interaction(
            CanvasInteractionContext {
                binding: &binding,
                projection: &projection,
                authorized_edit_scope_id: scope,
                last_accepted_sequence: last_sequence,
            },
            request,
        )
    }

    #[test]
    fn resolves_plain_render_instance_to_html_target() {
        let snapshot = snapshot();
        let receipt = resolve(&snapshot, &request("render-root"), None, 8);

        assert_eq!(receipt.status, CanvasInteractionStatus::Resolved);
        let target = receipt.target.expect("target");
        assert_eq!(target.kind, CanvasInteractionTargetKind::HtmlElement);
        assert_eq!(target.editor_node_id, "editor_render:render-root");
        assert_eq!(target.scope_state, CanvasInteractionScopeState::Unscoped);
        assert_eq!(
            target
                .source_provenance
                .definition
                .as_ref()
                .map(|source| source.file.as_str()),
            Some("templates/index.html"),
        );
        assert_eq!(
            receipt.overlay.expect("overlay").render_instance_ids,
            vec!["render-root"]
        );
    }

    #[test]
    fn closed_scope_promotes_render_hit_to_tera_boundary() {
        let snapshot = snapshot();
        let receipt = resolve(&snapshot, &request("render-1"), None, 8);

        let target = receipt.target.expect("target");
        assert_eq!(target.kind, CanvasInteractionTargetKind::TeraBoundary);
        assert_eq!(target.editor_node_id, "editor_boundary:boundary-1");
        assert_eq!(target.scope_state, CanvasInteractionScopeState::Locked);
        assert_eq!(
            target.effect_scope,
            EditorNavigationEffectScope::AllRenderedInstances
        );
        assert_eq!(target.rendered_instance_count, 4);
        assert_eq!(
            receipt.overlay.expect("overlay").render_instance_ids,
            vec!["render-1"]
        );
    }

    #[test]
    fn markdown_descendant_hit_promotes_to_read_only_atomic_boundary() {
        let snapshot = markdown_snapshot();
        let receipt = resolve(&snapshot, &request("markdown-render"), None, 8);

        let target = receipt.target.expect("markdown target");
        assert_eq!(target.kind, CanvasInteractionTargetKind::MarkdownBoundary);
        assert_eq!(target.editor_node_id, "editor_boundary:markdown-1");
        assert_eq!(target.file.as_deref(), Some("content/_index.md"));
        assert!(target.actions.can_select);
        assert!(target.actions.can_inspect);
        assert!(target.actions.can_open_in_code);
        assert!(!target.actions.can_enter_boundary);
        assert!(!target.actions.can_move_atomic);
        assert!(!target.actions.can_move);
        assert!(!target.actions.can_edit_text);
        assert!(!target.actions.can_edit_attributes);
        assert!(target.actions.read_only);
        assert_eq!(
            target.actions.reason_code,
            Some(SourceCapabilityReason::MarkdownRenderedBoundary)
        );
        assert_eq!(
            receipt
                .overlay
                .expect("markdown overlay")
                .render_instance_ids,
            vec!["markdown-render"]
        );
    }

    #[test]
    fn authorized_scope_resolves_the_inner_html_instance() {
        let snapshot = snapshot();
        let receipt = resolve(
            &snapshot,
            &request("render-1"),
            Some("editor_boundary:boundary-1"),
            8,
        );

        let target = receipt.target.expect("target");
        assert_eq!(target.kind, CanvasInteractionTargetKind::HtmlElement);
        assert_eq!(target.editor_node_id, "editor_render:render-1");
        assert_eq!(target.scope_state, CanvasInteractionScopeState::Authorized);
        assert_eq!(
            target.effect_scope,
            EditorNavigationEffectScope::AllRenderedInstances
        );
    }

    #[test]
    fn stale_agent_and_replayed_sequence_fail_closed() {
        let snapshot = snapshot();
        let mut stale_agent = request("render-root");
        stale_agent.identity.agent_instance_id = "agent-old".to_string();
        let receipt = resolve(&snapshot, &stale_agent, None, 8);
        assert_eq!(receipt.status, CanvasInteractionStatus::Stale);
        assert_eq!(
            receipt.diagnostics[0].code,
            CanvasInteractionDiagnosticCode::AgentInstanceMismatch
        );

        let receipt = resolve(&snapshot, &request("render-root"), None, 9);
        assert_eq!(receipt.status, CanvasInteractionStatus::Stale);
        assert_eq!(
            receipt.diagnostics[0].code,
            CanvasInteractionDiagnosticCode::GestureSequenceStale
        );
    }

    #[test]
    fn unknown_inner_candidate_falls_back_to_known_ancestor() {
        let snapshot = snapshot();
        let mut request = request("missing-runtime-id");
        request.hit_path.push(CanvasHitCandidate {
            kind: CanvasHitCandidateKind::RenderInstance,
            id: "render-root".to_string(),
        });
        let receipt = resolve(&snapshot, &request, None, 8);

        assert_eq!(receipt.status, CanvasInteractionStatus::Resolved);
        assert_eq!(
            receipt.target.expect("target").editor_node_id,
            "editor_render:render-root"
        );
        assert_eq!(
            receipt.diagnostics[0].code,
            CanvasInteractionDiagnosticCode::UnknownHitCandidate
        );
    }

    #[test]
    fn malformed_hit_path_is_rejected_before_resolution() {
        let snapshot = snapshot();
        let mut request = request("render-root");
        request.hit_path.push(request.hit_path[0].clone());
        let receipt = resolve(&snapshot, &request, None, 8);

        assert_eq!(receipt.status, CanvasInteractionStatus::Rejected);
        assert_eq!(
            receipt.diagnostics[0].code,
            CanvasInteractionDiagnosticCode::DuplicateHitCandidate
        );
        assert!(receipt.target.is_none());
    }

    #[test]
    fn drag_context_is_bounded_and_must_match_the_gesture_phase() {
        let snapshot = snapshot();
        let mut drag_start = request("render-root");
        drag_start.gesture = CanvasInteractionGesture::DragStart;
        drag_start.drag = Some(CanvasDragSample {
            session_id: "agent-3-drag-1".to_string(),
            position: None,
        });
        assert_eq!(
            resolve(&snapshot, &drag_start, None, 8).status,
            CanvasInteractionStatus::Resolved
        );

        let mut drag_over = request("render-root");
        drag_over.gesture = CanvasInteractionGesture::DragOver;
        drag_over.drag = Some(CanvasDragSample {
            session_id: "agent-3-drag-1".to_string(),
            position: Some(CanvasDragPosition::Before),
        });
        let drag_over_receipt = resolve(&snapshot, &drag_over, None, 8);
        assert_eq!(
            drag_over_receipt.drag_position,
            Some(CanvasDragPosition::Before)
        );

        let mut invalid_drop = request("render-root");
        invalid_drop.gesture = CanvasInteractionGesture::Drop;
        invalid_drop.drag = Some(CanvasDragSample {
            session_id: "agent-3-drag-1".to_string(),
            position: None,
        });
        let receipt = resolve(&snapshot, &invalid_drop, None, 8);
        assert_eq!(receipt.status, CanvasInteractionStatus::Rejected);
        assert_eq!(receipt.drag_position, None);
        assert_eq!(
            receipt.diagnostics[0].code,
            CanvasInteractionDiagnosticCode::InvalidDragSample
        );

        let mut drag_on_click = request("render-root");
        drag_on_click.drag = Some(CanvasDragSample {
            session_id: "agent-3-drag-1".to_string(),
            position: Some(CanvasDragPosition::Inside),
        });
        assert_eq!(
            resolve(&snapshot, &drag_on_click, None, 8).diagnostics[0].code,
            CanvasInteractionDiagnosticCode::InvalidDragSample
        );
    }

    #[test]
    fn runtime_preserves_sequence_and_replaces_only_with_newer_document() {
        let snapshot = snapshot();
        let runtime = CanvasInteractionRuntime::default();
        let identity = interaction_identity();
        runtime
            .bind_agent(&snapshot, Some("templates/index.html"), identity.clone())
            .expect("initial bind");
        let receipt = runtime
            .resolve(None, &request("render-root"))
            .expect("resolve");
        assert_eq!(receipt.status, CanvasInteractionStatus::Resolved);

        let rebound = runtime
            .bind_agent(&snapshot, Some("templates/index.html"), identity.clone())
            .expect("idempotent bind");
        assert_eq!(rebound.last_accepted_sequence, 9);
        assert_eq!(
            rebound.active_document_path.as_deref(),
            Some("templates/index.html")
        );
        let scope_context = runtime.scope_context(&identity).expect("scope context");
        assert_eq!(
            scope_context.active_document_path.as_deref(),
            Some("templates/index.html")
        );
        assert_eq!(scope_context.model_revision, "model-7");

        let mut older = identity.clone();
        older.document_epoch = 2;
        older.agent_instance_id = "agent-2".to_string();
        assert!(runtime
            .bind_agent(&snapshot, Some("templates/index.html"), older)
            .is_err());

        let mut newer = identity;
        newer.document_epoch = 4;
        newer.agent_instance_id = "agent-4".to_string();
        runtime
            .bind_agent(&snapshot, Some("templates/index.html"), newer)
            .expect("new document bind");
        let stale = runtime
            .resolve(None, &request("render-root"))
            .expect("stale receipt");
        assert_eq!(stale.status, CanvasInteractionStatus::Stale);
        assert_eq!(
            stale.diagnostics[0].code,
            CanvasInteractionDiagnosticCode::AgentBindingMissing
        );

        let mut superseded = interaction_identity();
        superseded.document_epoch = 3;
        superseded.agent_instance_id = "agent-late".to_string();
        assert!(runtime
            .bind_agent(&snapshot, Some("templates/index.html"), superseded)
            .is_err());

        let mut same_epoch_new_canvas = interaction_identity();
        same_epoch_new_canvas.document_epoch = 4;
        same_epoch_new_canvas.agent_instance_id = "agent-other-projection".to_string();
        same_epoch_new_canvas.canvas.workspace_revision = 8;
        same_epoch_new_canvas.canvas.transaction_id = "canvas-transaction-2".to_string();
        same_epoch_new_canvas.canvas.preview_revision = "preview-8".to_string();
        let mut next_snapshot = snapshot.clone();
        next_snapshot.identity = same_epoch_new_canvas.canvas.clone();
        assert!(runtime
            .bind_agent(
                &next_snapshot,
                Some("templates/index.html"),
                same_epoch_new_canvas,
            )
            .is_err());
    }

    #[test]
    fn pointer_hover_projects_only_an_accepted_latest_sequence() {
        let snapshot = snapshot();
        let runtime = CanvasInteractionRuntime::default();
        let coordinator = SelectionCoordinatorRuntime::default();
        runtime
            .bind_agent(
                &snapshot,
                Some("templates/index.html"),
                interaction_identity(),
            )
            .expect("bind");

        let first = pointer_request("render-root", 9);
        let (first_interaction, first_projection) = runtime
            .resolve_pointer_hover(None, &first, |snapshot, active_document_path, receipt| {
                coordinator.apply(
                    snapshot,
                    active_document_path,
                    None,
                    SelectionIntent::SetHover {
                        editor_node_id: receipt
                            .target
                            .as_ref()
                            .expect("target")
                            .editor_node_id
                            .clone(),
                        document_epoch: first.identity.document_epoch,
                    },
                )
            })
            .expect("first hover");
        assert_eq!(first_interaction.status, CanvasInteractionStatus::Resolved);
        assert_eq!(
            first_projection
                .and_then(|projection| projection.hover)
                .map(|hover| hover.editor_node_id),
            Some("editor_render:render-root".to_string())
        );

        let latest = pointer_request("render-1", 11);
        let (latest_interaction, latest_projection) = runtime
            .resolve_pointer_hover(None, &latest, |snapshot, active_document_path, receipt| {
                coordinator.apply(
                    snapshot,
                    active_document_path,
                    None,
                    SelectionIntent::SetHover {
                        editor_node_id: receipt
                            .target
                            .as_ref()
                            .expect("target")
                            .editor_node_id
                            .clone(),
                        document_epoch: latest.identity.document_epoch,
                    },
                )
            })
            .expect("latest hover");
        assert_eq!(latest_interaction.status, CanvasInteractionStatus::Resolved);
        assert_eq!(
            latest_projection
                .and_then(|projection| projection.hover)
                .map(|hover| hover.editor_node_id),
            Some("editor_boundary:boundary-1".to_string())
        );

        let mut ordered_click = request("render-root");
        ordered_click.gesture_sequence = 10;
        assert_eq!(
            runtime
                .resolve(None, &ordered_click)
                .expect("ordered gesture after newer hover")
                .status,
            CanvasInteractionStatus::Resolved,
            "lane-ul hover nu trebuie să invalideze click-ul semantic deja ordonat"
        );

        let stale = pointer_request("render-root", 10);
        let (stale_interaction, stale_projection) = runtime
            .resolve_pointer_hover::<()>(None, &stale, |_, _, _| {
                panic!("o secvență stale nu trebuie proiectată")
            })
            .expect("stale receipt");
        assert_eq!(stale_interaction.status, CanvasInteractionStatus::Stale);
        assert!(stale_projection.is_none());
    }
}
