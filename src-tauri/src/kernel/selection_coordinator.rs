use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};

use crate::{
    kernel::canvas_interaction::CanvasInteractionIdentity,
    kernel::editor_navigation::{
        EditorNavigationCapabilities, EditorNavigationNode, EditorNavigationNodeKind,
        EditorNavigationOrigin, EditorNavigationSnapshot, EditorSourceProvenance,
        EditorSourceReference, EditorSourceResolution,
    },
    kernel::project_workspace::SourceIdentityAliasTransition,
    preview::CanvasProjectionIdentity,
    source_graph::model::{SourceGraph, SourceNode, SourceNodeKind, SourceOrigin, SourceRange},
};

pub const SELECTION_COORDINATOR_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SelectionSubjectKind {
    HtmlElement,
    TeraBoundary,
    MarkdownBoundary,
    RuntimeElement,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionSubject {
    pub kind: SelectionSubjectKind,
    pub tag: Option<String>,
    pub label: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SelectionFocus {
    Element,
    CssRule {
        file: String,
        selector: String,
        viewport: Option<String>,
        #[serde(default, skip_deserializing)]
        range: Option<SourceRange>,
    },
    CssProperty {
        file: String,
        selector: String,
        property: String,
        viewport: Option<String>,
        #[serde(default, skip_deserializing)]
        range: Option<SourceRange>,
    },
    JsBehavior {
        file: String,
        behavior_id: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SelectionResolution {
    Cleared,
    Resolved,
    NotRendered,
    Ambiguous,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionAnchor {
    pub editor_node_id: Option<String>,
    pub source_node_id: Option<String>,
    pub render_instance_id: Option<String>,
    pub render_instance_ids: Vec<String>,
    pub boundary_instance_id: Option<String>,
    pub file: Option<String>,
    pub range: Option<SourceRange>,
    pub provenance_stack: Vec<String>,
    pub component_invocation_ids: Vec<String>,
    pub block_source_instance_ids: Vec<String>,
    pub dynamic_widget_source_instance_ids: Vec<String>,
    pub binding_key: Option<String>,
    pub binding_path: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionPreviewProjection {
    pub editor_node_id: Option<String>,
    pub target_kind: Option<SelectionSubjectKind>,
    pub primary_render_instance_id: Option<String>,
    pub render_instance_ids: Vec<String>,
    pub boundary_instance_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionLayersProjection {
    pub editor_node_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionCodeProjection {
    pub file: Option<String>,
    pub range: Option<SourceRange>,
    pub focus: SelectionFocus,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionInspectorProjection {
    pub editor_node_id: Option<String>,
    pub subject_kind: Option<SelectionSubjectKind>,
    pub can_inspect: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionStatusProjection {
    pub provenance: Option<EditorSourceProvenance>,
    pub focus: SelectionFocus,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionUiProjections {
    pub preview: SelectionPreviewProjection,
    pub layers: SelectionLayersProjection,
    pub code: SelectionCodeProjection,
    pub inspector: SelectionInspectorProjection,
    pub status: SelectionStatusProjection,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionSnapshot {
    pub schema_version: u32,
    pub selection_revision: u64,
    pub project_root: String,
    pub runtime_session_id: String,
    pub canvas_identity: CanvasProjectionIdentity,
    pub route: String,
    pub active_document_path: Option<String>,
    pub resolution: SelectionResolution,
    pub subject: Option<SelectionSubject>,
    pub focus: SelectionFocus,
    pub anchor: Option<SelectionAnchor>,
    pub provenance: Option<EditorSourceProvenance>,
    pub capabilities: Option<EditorNavigationCapabilities>,
    pub projections: SelectionUiProjections,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HoverSnapshot {
    pub schema_version: u32,
    pub hover_revision: u64,
    pub canvas_identity: CanvasProjectionIdentity,
    pub route: String,
    pub document_epoch: u64,
    pub editor_node_id: String,
    pub subject_kind: SelectionSubjectKind,
    pub primary_render_instance_id: Option<String>,
    pub render_instance_ids: Vec<String>,
    pub boundary_instance_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionCoordinatorSnapshot {
    pub schema_version: u32,
    pub selection: SelectionSnapshot,
    pub hover: Option<HoverSnapshot>,
    pub inspector_summary: InspectorSelectionSummarySnapshot,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SelectionIntent {
    SelectEditorNode {
        editor_node_id: String,
    },
    SelectSourcePosition {
        file: String,
        offset: usize,
        #[serde(default)]
        viewport: Option<String>,
    },
    SetFocus {
        focus: SelectionFocus,
        #[serde(default)]
        expected_selection_revision: Option<u64>,
    },
    ClearSelection,
    Rebase,
    SetHover {
        editor_node_id: String,
        document_epoch: u64,
    },
    ClearHover {
        document_epoch: u64,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionObservationInput {
    pub schema_version: u32,
    pub selection_revision: u64,
    pub canvas_identity: CanvasProjectionIdentity,
    pub document_epoch: u64,
    pub render_instance_id: String,
    pub inspector_facts: InspectorSelectionPhysicalFacts,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InspectorSelectionSummaryState {
    Empty,
    Resolving,
    Resolved,
    NotRendered,
    Ambiguous,
    Uninspectable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InspectorSelectionSummaryReason {
    NoSelection,
    AwaitingPhysicalFacts,
    SelectionNotRendered,
    SelectionAmbiguous,
    InspectionDisabled,
    MissingRenderInstance,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectorSelectionSummaryDiagnostic {
    pub code: InspectorSelectionSummaryReason,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InspectorSelectionBlockMarkerKind {
    Canonical,
    Legacy,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectorSelectionBlockContext {
    pub provider_id: String,
    pub marker_kind: InspectorSelectionBlockMarkerKind,
    pub root_tag: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InspectorSelectionPhysicalFacts {
    pub observed_tag: String,
    pub element_id: String,
    pub classes: Vec<String>,
    #[serde(default)]
    pub block_context: Option<InspectorSelectionBlockContext>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectorSelectionSummarySnapshot {
    pub schema_version: u32,
    pub project_root: String,
    pub runtime_session_id: String,
    pub selection_revision: u64,
    pub canvas_identity: CanvasProjectionIdentity,
    pub document_epoch: Option<u64>,
    pub render_instance_id: Option<String>,
    pub state: InspectorSelectionSummaryState,
    pub subject_kind: Option<SelectionSubjectKind>,
    pub tag: Option<String>,
    pub label: Option<String>,
    pub selector: Option<String>,
    pub element_id: Option<String>,
    pub classes: Vec<String>,
    pub block_context: Option<InspectorSelectionBlockContext>,
    pub active_css_class: Option<String>,
    pub can_inspect: bool,
    pub reason: Option<InspectorSelectionSummaryReason>,
    pub diagnostics: Vec<InspectorSelectionSummaryDiagnostic>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionObservationReceipt {
    pub schema_version: u32,
    pub selection_revision: u64,
    pub canvas_identity: CanvasProjectionIdentity,
    pub document_epoch: u64,
    pub render_instance_id: String,
    pub inspector_summary: InspectorSelectionSummarySnapshot,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SelectionMutationIdentity {
    pub selection_revision: u64,
    #[serde(default)]
    pub editor_node_id: Option<String>,
    #[serde(default)]
    pub source_node_id: Option<String>,
    #[serde(default)]
    pub render_instance_id: Option<String>,
}

#[derive(Default)]
struct SelectionCoordinatorState {
    next_selection_revision: u64,
    next_hover_revision: u64,
    selection: Option<SelectionSnapshot>,
    hover: Option<HoverSnapshot>,
    active_inspector_document: Option<CanvasInteractionIdentity>,
    inspector_facts: Option<AcceptedInspectorSelectionFacts>,
}

#[derive(Clone)]
struct AcceptedInspectorSelectionFacts {
    selection_revision: u64,
    canvas_identity: CanvasProjectionIdentity,
    document_epoch: u64,
    render_instance_id: String,
    facts: InspectorSelectionPhysicalFacts,
}

#[derive(Default)]
pub struct SelectionCoordinatorRuntime {
    state: Mutex<SelectionCoordinatorState>,
}

#[derive(Clone, Copy)]
enum SelectionRevisionPolicy {
    Exact,
    StableSemanticAnchor,
}

impl SelectionCoordinatorRuntime {
    pub fn bind_inspector_document(
        &self,
        identity: CanvasInteractionIdentity,
    ) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "SelectionCoordinator este indisponibil.".to_string())?;
        if state.active_inspector_document.as_ref() == Some(&identity) {
            return Ok(());
        }
        state.active_inspector_document = Some(identity);
        state.inspector_facts = None;
        Ok(())
    }

    pub fn require_mutation_target(
        &self,
        runtime_session_id: &str,
        selection_revision: u64,
        editor_node_id: Option<&str>,
        source_node_id: Option<&str>,
        render_instance_id: Option<&str>,
    ) -> Result<(), String> {
        self.with_mutation_target(
            runtime_session_id,
            selection_revision,
            editor_node_id,
            source_node_id,
            render_instance_id,
            || Ok(()),
        )
    }

    pub fn with_mutation_target<R>(
        &self,
        runtime_session_id: &str,
        selection_revision: u64,
        editor_node_id: Option<&str>,
        source_node_id: Option<&str>,
        render_instance_id: Option<&str>,
        execute: impl FnOnce() -> Result<R, String>,
    ) -> Result<R, String> {
        self.with_resolved_target(
            "Mutația",
            runtime_session_id,
            selection_revision,
            editor_node_id,
            source_node_id,
            render_instance_id,
            SelectionRevisionPolicy::Exact,
            execute,
        )
    }

    pub fn with_stable_semantic_mutation_target<R>(
        &self,
        runtime_session_id: &str,
        selection_revision: u64,
        editor_node_id: Option<&str>,
        source_node_id: Option<&str>,
        render_instance_id: Option<&str>,
        execute: impl FnOnce() -> Result<R, String>,
    ) -> Result<R, String> {
        self.with_resolved_target(
            "Mutația CSS",
            runtime_session_id,
            selection_revision,
            editor_node_id,
            source_node_id,
            render_instance_id,
            SelectionRevisionPolicy::StableSemanticAnchor,
            execute,
        )
    }

    pub fn with_selection_target<R>(
        &self,
        runtime_session_id: &str,
        selection_revision: u64,
        editor_node_id: Option<&str>,
        source_node_id: Option<&str>,
        render_instance_id: Option<&str>,
        execute: impl FnOnce() -> Result<R, String>,
    ) -> Result<R, String> {
        self.with_resolved_target(
            "Operația dependentă de selecție",
            runtime_session_id,
            selection_revision,
            editor_node_id,
            source_node_id,
            render_instance_id,
            SelectionRevisionPolicy::Exact,
            execute,
        )
    }

    fn with_resolved_target<R>(
        &self,
        operation: &str,
        runtime_session_id: &str,
        selection_revision: u64,
        editor_node_id: Option<&str>,
        source_node_id: Option<&str>,
        render_instance_id: Option<&str>,
        revision_policy: SelectionRevisionPolicy,
        execute: impl FnOnce() -> Result<R, String>,
    ) -> Result<R, String> {
        if selection_revision == 0
            || (editor_node_id.is_none()
                && source_node_id.is_none()
                && render_instance_id.is_none())
        {
            return Err(format!(
                "SelectionCoordinator a refuzat {operation}: amprenta selecției este incompletă."
            ));
        }
        let state = self
            .state
            .lock()
            .map_err(|_| "SelectionCoordinator este indisponibil.".to_string())?;
        let selection = state.selection.as_ref().ok_or_else(|| {
            format!("{operation} a fost anulată deoarece selecția semantică nu mai există.")
        })?;
        if selection.runtime_session_id != runtime_session_id {
            return Err(format!(
                "{operation} a fost anulată deoarece ProjectSession-ul selecției s-a schimbat."
            ));
        }
        let requires_exact_revision = matches!(revision_policy, SelectionRevisionPolicy::Exact);
        if selection.selection_revision < selection_revision
            || (requires_exact_revision && selection.selection_revision != selection_revision)
        {
            return Err(format!(
                "{operation} a fost anulată deoarece selecția s-a schimbat (revizia capturată {selection_revision}, revizia activă {}).",
                selection.selection_revision
            ));
        }
        if selection.resolution != SelectionResolution::Resolved {
            return Err(format!(
                "{operation} a fost anulată deoarece selecția nu mai are o rezoluție unică."
            ));
        }
        let anchor = selection.anchor.as_ref().ok_or_else(|| {
            format!(
                "{operation} a fost anulată deoarece selecția activă nu mai are ancoră semantică."
            )
        })?;
        require_optional_identity(
            operation,
            "EditorNavigation",
            editor_node_id,
            anchor.editor_node_id.as_deref(),
        )?;
        require_optional_identity(
            operation,
            "SourceGraph",
            source_node_id,
            anchor.source_node_id.as_deref(),
        )?;
        require_optional_identity(
            operation,
            "render",
            render_instance_id,
            anchor.render_instance_id.as_deref(),
        )?;
        execute()
    }

    pub fn apply(
        &self,
        snapshot: &EditorNavigationSnapshot,
        active_document_path: Option<&str>,
        source_graph: Option<&SourceGraph>,
        intent: SelectionIntent,
    ) -> Result<SelectionCoordinatorSnapshot, String> {
        self.apply_with_source_alias_transition(
            snapshot,
            active_document_path,
            source_graph,
            None,
            intent,
        )
    }

    pub(crate) fn apply_with_source_alias_transition(
        &self,
        snapshot: &EditorNavigationSnapshot,
        active_document_path: Option<&str>,
        source_graph: Option<&SourceGraph>,
        source_identity_alias_transition: Option<&SourceIdentityAliasTransition>,
        intent: SelectionIntent,
    ) -> Result<SelectionCoordinatorSnapshot, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "SelectionCoordinator este indisponibil.".to_string())?;
        ensure_session(&mut state, snapshot, active_document_path)?;

        match intent {
            SelectionIntent::SelectEditorNode { editor_node_id } => {
                let node = snapshot
                    .nodes
                    .iter()
                    .find(|node| node.id == editor_node_id)
                    .ok_or_else(|| {
                        "SelectionCoordinator nu găsește nodul EditorNavigation solicitat."
                            .to_string()
                    })?;
                commit_node(
                    &mut state,
                    snapshot,
                    active_document_path,
                    node,
                    SelectionFocus::Element,
                    Vec::new(),
                )?;
            }
            SelectionIntent::SelectSourcePosition {
                file,
                offset,
                viewport: _,
            } => {
                select_source_position(
                    &mut state,
                    snapshot,
                    active_document_path,
                    source_graph,
                    &file,
                    offset,
                )?;
            }
            SelectionIntent::SetFocus {
                focus,
                expected_selection_revision,
            } => {
                set_focus(
                    &mut state,
                    snapshot,
                    active_document_path,
                    focus,
                    expected_selection_revision,
                )?;
            }
            SelectionIntent::ClearSelection => {
                clear_selection(&mut state, snapshot, active_document_path)?;
            }
            SelectionIntent::Rebase => {
                rebase_selection(
                    &mut state,
                    snapshot,
                    active_document_path,
                    source_graph,
                    source_identity_alias_transition,
                )?;
            }
            SelectionIntent::SetHover {
                editor_node_id,
                document_epoch,
            } => {
                set_hover(&mut state, snapshot, &editor_node_id, document_epoch)?;
            }
            SelectionIntent::ClearHover { document_epoch } => {
                clear_hover(&mut state, snapshot, document_epoch);
            }
        }

        coordinator_snapshot(&state)
    }

    /// Proiectează exclusiv starea efemeră de hover.
    ///
    /// Lane-ul pointerului nu are nevoie de SelectionSnapshot sau de sumarul
    /// Inspectorului, care nu se modifică la hover. Întoarcerea proiecției
    /// minimale evită clonarea și serializarea întregului coordinator la
    /// fiecare tranziție fizică din Canvas.
    pub fn apply_hover(
        &self,
        snapshot: &EditorNavigationSnapshot,
        active_document_path: Option<&str>,
        editor_node_id: Option<&str>,
        document_epoch: u64,
    ) -> Result<(Option<HoverSnapshot>, bool), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "SelectionCoordinator este indisponibil.".to_string())?;
        ensure_session(&mut state, snapshot, active_document_path)?;

        let current = state.hover.as_ref().filter(|hover| {
            hover.canvas_identity == snapshot.identity && hover.document_epoch == document_epoch
        });
        let unchanged = match (editor_node_id, current) {
            (Some(editor_node_id), Some(hover)) => hover.editor_node_id == editor_node_id,
            (None, None) => true,
            _ => false,
        };
        if unchanged {
            return Ok((current.cloned(), false));
        }

        if let Some(editor_node_id) = editor_node_id {
            set_hover(&mut state, snapshot, editor_node_id, document_epoch)?;
        } else {
            clear_hover(&mut state, snapshot, document_epoch);
        }
        let hover = state.hover.as_ref().filter(|hover| {
            hover.canvas_identity == snapshot.identity && hover.document_epoch == document_epoch
        });
        Ok((hover.cloned(), true))
    }

    pub fn accept_observation(
        &self,
        input: SelectionObservationInput,
    ) -> Result<SelectionObservationReceipt, String> {
        if input.schema_version != SELECTION_COORDINATOR_SCHEMA_VERSION {
            return Err("Observația selecției folosește o versiune incompatibilă.".to_string());
        }
        if input.document_epoch == 0 || input.render_instance_id.trim().is_empty() {
            return Err("Observația selecției are o identitate fizică invalidă.".to_string());
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| "SelectionCoordinator este indisponibil.".to_string())?;
        let selection = state
            .selection
            .as_ref()
            .ok_or_else(|| "Nu există o selecție semantică activă.".to_string())?;
        if selection.selection_revision != input.selection_revision {
            return Err("Observația DOM aparține unei revizii de selecție vechi.".to_string());
        }
        if selection.canvas_identity != input.canvas_identity {
            return Err("Observația DOM aparține altei identități Canvas.".to_string());
        }
        if selection
            .anchor
            .as_ref()
            .and_then(|anchor| anchor.render_instance_id.as_deref())
            != Some(input.render_instance_id.as_str())
        {
            return Err("Observația DOM nu aparține instanței semantice selectate.".to_string());
        }
        let active_document = state
            .active_inspector_document
            .as_ref()
            .ok_or_else(|| "Inspectorul nu are un document CanvasAgent activ.".to_string())?;
        if active_document.canvas != input.canvas_identity
            || active_document.document_epoch != input.document_epoch
        {
            return Err("Observația DOM nu aparține documentului CanvasAgent activ.".to_string());
        }
        if !selection.projections.inspector.can_inspect {
            return Err("Selecția semantică nu permite inspecția DOM.".to_string());
        }
        let inspector_facts = validate_inspector_facts(selection, input.inspector_facts)?;
        state.inspector_facts = Some(AcceptedInspectorSelectionFacts {
            selection_revision: input.selection_revision,
            canvas_identity: input.canvas_identity.clone(),
            document_epoch: input.document_epoch,
            render_instance_id: input.render_instance_id.clone(),
            facts: inspector_facts,
        });
        Ok(SelectionObservationReceipt {
            schema_version: SELECTION_COORDINATOR_SCHEMA_VERSION,
            selection_revision: input.selection_revision,
            canvas_identity: input.canvas_identity,
            document_epoch: input.document_epoch,
            render_instance_id: input.render_instance_id,
            inspector_summary: inspector_summary(&state)?,
        })
    }

    pub fn revoke_all(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.selection = None;
            state.hover = None;
            state.active_inspector_document = None;
            state.inspector_facts = None;
        }
    }
}

fn require_optional_identity(
    operation: &str,
    label: &str,
    expected: Option<&str>,
    actual: Option<&str>,
) -> Result<(), String> {
    let Some(expected) = expected else {
        return Ok(());
    };
    if expected.trim().is_empty() || actual != Some(expected) {
        return Err(format!(
            "{operation} a fost anulată deoarece identitatea {label} a selecției s-a schimbat."
        ));
    }
    Ok(())
}

fn ensure_session(
    state: &mut SelectionCoordinatorState,
    snapshot: &EditorNavigationSnapshot,
    active_document_path: Option<&str>,
) -> Result<(), String> {
    let changed = state.selection.as_ref().is_some_and(|selection| {
        selection.project_root != snapshot.identity.project_root
            || selection.runtime_session_id != snapshot.identity.runtime_session_id
    });
    if changed {
        state.selection = None;
        state.hover = None;
        state.inspector_facts = None;
    }
    if state.selection.is_none() {
        let revision = next_selection_revision(state)?;
        state.selection = Some(empty_selection(
            revision,
            snapshot,
            active_document_path,
            SelectionFocus::Element,
        ));
    }
    Ok(())
}

fn commit_node(
    state: &mut SelectionCoordinatorState,
    snapshot: &EditorNavigationSnapshot,
    active_document_path: Option<&str>,
    node: &EditorNavigationNode,
    focus: SelectionFocus,
    diagnostics: Vec<String>,
) -> Result<(), String> {
    let revision = next_selection_revision(state)?;
    state.selection = Some(selection_from_node(
        revision,
        snapshot,
        active_document_path,
        node,
        focus,
        SelectionResolution::Resolved,
        diagnostics,
    ));
    state.inspector_facts = None;
    Ok(())
}

fn set_focus(
    state: &mut SelectionCoordinatorState,
    snapshot: &EditorNavigationSnapshot,
    active_document_path: Option<&str>,
    focus: SelectionFocus,
    expected_selection_revision: Option<u64>,
) -> Result<(), String> {
    validate_focus(&focus)?;
    if let Some(expected_revision) = expected_selection_revision {
        let current_revision = state
            .selection
            .as_ref()
            .map(|selection| selection.selection_revision)
            .ok_or_else(|| {
                "Focusul a fost blocat deoarece selecția semantică nu mai există.".to_string()
            })?;
        if expected_revision == 0 || current_revision != expected_revision {
            return Err(format!(
                "Focusul a fost blocat deoarece selecția s-a schimbat (revizia așteptată {expected_revision}, revizia activă {current_revision})."
            ));
        }
    }
    if !matches!(&focus, SelectionFocus::Element)
        && !state.selection.as_ref().is_some_and(|selection| {
            selection.subject.is_some()
                && selection.anchor.is_some()
                && selection.resolution != SelectionResolution::Cleared
        })
    {
        return Err("Focusul CSS/JS necesită mai întâi un subiect semantic selectat.".to_string());
    }
    let revision = next_selection_revision(state)?;
    let current = state.selection.clone().unwrap_or_else(|| {
        empty_selection(revision, snapshot, active_document_path, focus.clone())
    });
    let retained_inspector_facts = state.inspector_facts.take().filter(|facts| {
        facts.selection_revision == current.selection_revision
            && facts.canvas_identity == snapshot.identity
            && current
                .anchor
                .as_ref()
                .and_then(|anchor| anchor.render_instance_id.as_deref())
                == Some(facts.render_instance_id.as_str())
    });
    let projections = projections(
        current.resolution,
        current.subject.as_ref(),
        current.anchor.as_ref(),
        current.provenance.as_ref(),
        current.capabilities.as_ref(),
        &focus,
    );
    state.selection = Some(SelectionSnapshot {
        schema_version: SELECTION_COORDINATOR_SCHEMA_VERSION,
        selection_revision: revision,
        project_root: snapshot.identity.project_root.clone(),
        runtime_session_id: snapshot.identity.runtime_session_id.clone(),
        canvas_identity: snapshot.identity.clone(),
        route: snapshot.route.clone(),
        active_document_path: active_document_path.map(str::to_string),
        focus,
        projections,
        ..current
    });
    state.inspector_facts = retained_inspector_facts.map(|mut facts| {
        facts.selection_revision = revision;
        facts
    });
    Ok(())
}

fn clear_selection(
    state: &mut SelectionCoordinatorState,
    snapshot: &EditorNavigationSnapshot,
    active_document_path: Option<&str>,
) -> Result<(), String> {
    let revision = next_selection_revision(state)?;
    state.selection = Some(empty_selection(
        revision,
        snapshot,
        active_document_path,
        SelectionFocus::Element,
    ));
    state.hover = None;
    state.inspector_facts = None;
    Ok(())
}

fn rebase_selection(
    state: &mut SelectionCoordinatorState,
    snapshot: &EditorNavigationSnapshot,
    active_document_path: Option<&str>,
    source_graph: Option<&SourceGraph>,
    source_identity_alias_transition: Option<&SourceIdentityAliasTransition>,
) -> Result<(), String> {
    let Some(current) = state.selection.clone() else {
        return ensure_session(state, snapshot, active_document_path);
    };
    let Some(anchor) = current.anchor.as_ref() else {
        let revision = next_selection_revision(state)?;
        state.selection = Some(empty_selection(
            revision,
            snapshot,
            active_document_path,
            current.focus,
        ));
        state.inspector_facts = None;
        state.hover = None;
        return Ok(());
    };
    let source_identity_aliases = source_identity_alias_transition
        .filter(|transition| {
            transition.revision_before == current.canvas_identity.workspace_revision
                && transition.revision_after == snapshot.identity.workspace_revision
        })
        .map(|transition| &transition.aliases);

    match rebase_candidate(
        snapshot,
        anchor,
        current.provenance.as_ref(),
        source_graph,
        source_identity_aliases,
    ) {
        RebaseCandidate::Resolved(node) => {
            commit_node(
                state,
                snapshot,
                active_document_path,
                node,
                current.focus,
                Vec::new(),
            )?;
        }
        RebaseCandidate::NotRendered => {
            preserve_unresolved(
                state,
                snapshot,
                active_document_path,
                current,
                SelectionResolution::NotRendered,
                "Selecția semantică nu este randată în Canvas-ul curent.",
            )?;
        }
        RebaseCandidate::Ambiguous => {
            preserve_unresolved(
                state,
                snapshot,
                active_document_path,
                current,
                SelectionResolution::Ambiguous,
                "Selecția semantică are mai multe instanțe posibile în Canvas-ul curent.",
            )?;
        }
    }
    state.hover = None;
    Ok(())
}

fn preserve_unresolved(
    state: &mut SelectionCoordinatorState,
    snapshot: &EditorNavigationSnapshot,
    active_document_path: Option<&str>,
    current: SelectionSnapshot,
    resolution: SelectionResolution,
    diagnostic: &str,
) -> Result<(), String> {
    let revision = next_selection_revision(state)?;
    let projections = projections(
        resolution,
        current.subject.as_ref(),
        current.anchor.as_ref(),
        current.provenance.as_ref(),
        current.capabilities.as_ref(),
        &current.focus,
    );
    state.selection = Some(SelectionSnapshot {
        schema_version: SELECTION_COORDINATOR_SCHEMA_VERSION,
        selection_revision: revision,
        project_root: snapshot.identity.project_root.clone(),
        runtime_session_id: snapshot.identity.runtime_session_id.clone(),
        canvas_identity: snapshot.identity.clone(),
        route: snapshot.route.clone(),
        active_document_path: active_document_path.map(str::to_string),
        resolution,
        projections,
        diagnostics: vec![diagnostic.to_string()],
        ..current
    });
    state.inspector_facts = None;
    Ok(())
}

fn select_source_position(
    state: &mut SelectionCoordinatorState,
    snapshot: &EditorNavigationSnapshot,
    active_document_path: Option<&str>,
    source_graph: Option<&SourceGraph>,
    file: &str,
    offset: usize,
) -> Result<(), String> {
    let normalized_file = normalized_path(file);
    if let Some(graph) = source_graph {
        let containing_sources: Vec<&SourceNode> = graph
            .nodes
            .iter()
            .filter(|node| {
                normalized_path(&node.file) == normalized_file
                    && node.range.as_ref().is_some_and(|range| {
                        range.start <= offset && offset < range.end.max(range.start + 1)
                    })
            })
            .collect();
        if let Some(minimum_width) = containing_sources
            .iter()
            .filter_map(|node| {
                node.range
                    .as_ref()
                    .map(|range| range.end.saturating_sub(range.start))
            })
            .min()
        {
            let mut source_candidates: Vec<&SourceNode> = containing_sources
                .into_iter()
                .filter(|node| {
                    node.range
                        .as_ref()
                        .is_some_and(|range| range.end.saturating_sub(range.start) == minimum_width)
                })
                .collect();
            source_candidates.sort_by(|left, right| left.id.cmp(&right.id));
            source_candidates.dedup_by(|left, right| left.id == right.id);
            if let Some(source) = source_candidates.first().copied() {
                return select_source_node(
                    state,
                    snapshot,
                    active_document_path,
                    source,
                    source_candidates.len() > 1,
                );
            }
        }
    }

    let containing: Vec<&EditorNavigationNode> = snapshot
        .nodes
        .iter()
        .filter(|node| {
            node.file
                .as_deref()
                .is_some_and(|candidate| normalized_path(candidate) == normalized_file)
                && node.range.as_ref().is_some_and(|range| {
                    range.start <= offset && offset < range.end.max(range.start + 1)
                })
        })
        .collect();
    let Some(minimum_width) = containing
        .iter()
        .filter_map(|node| {
            node.range
                .as_ref()
                .map(|range| range.end.saturating_sub(range.start))
        })
        .min()
    else {
        return Err(
            "SelectionCoordinator nu găsește o țintă semantică la poziția din cod.".to_string(),
        );
    };
    let mut candidates: Vec<&EditorNavigationNode> = containing
        .into_iter()
        .filter(|node| {
            node.range
                .as_ref()
                .is_some_and(|range| range.end.saturating_sub(range.start) == minimum_width)
        })
        .collect();
    candidates.sort_by_key(|node| node.order);
    candidates.dedup_by(|left, right| left.id == right.id);

    if candidates.len() == 1 {
        return commit_node(
            state,
            snapshot,
            active_document_path,
            candidates[0],
            SelectionFocus::Element,
            Vec::new(),
        );
    }

    if let Some(current_id) = state
        .selection
        .as_ref()
        .and_then(|selection| selection.anchor.as_ref())
        .and_then(|anchor| anchor.editor_node_id.as_deref())
    {
        if let Some(current) = candidates
            .iter()
            .find(|candidate| candidate.id == current_id)
        {
            return commit_node(
                state,
                snapshot,
                active_document_path,
                current,
                SelectionFocus::Element,
                Vec::new(),
            );
        }
    }

    let Some(representative) = candidates.first().copied() else {
        return Err(
            "SelectionCoordinator nu găsește o țintă semantică la poziția din cod.".to_string(),
        );
    };
    let revision = next_selection_revision(state)?;
    let mut selection = selection_from_node(
        revision,
        snapshot,
        active_document_path,
        representative,
        SelectionFocus::Element,
        SelectionResolution::Ambiguous,
        vec!["Poziția din cod corespunde mai multor instanțe randate.".to_string()],
    );
    selection.projections = projections(
        SelectionResolution::Ambiguous,
        selection.subject.as_ref(),
        selection.anchor.as_ref(),
        selection.provenance.as_ref(),
        selection.capabilities.as_ref(),
        &selection.focus,
    );
    state.selection = Some(selection);
    state.inspector_facts = None;
    Ok(())
}

fn select_source_node(
    state: &mut SelectionCoordinatorState,
    snapshot: &EditorNavigationSnapshot,
    active_document_path: Option<&str>,
    source: &SourceNode,
    source_ambiguous: bool,
) -> Result<(), String> {
    let mut rendered: Vec<&EditorNavigationNode> = snapshot
        .nodes
        .iter()
        .filter(|node| node.source_node_id.as_deref() == Some(source.id.as_str()))
        .collect();
    rendered.sort_by_key(|node| node.order);
    rendered.dedup_by(|left, right| left.id == right.id);

    if !source_ambiguous && rendered.len() == 1 {
        return commit_node(
            state,
            snapshot,
            active_document_path,
            rendered[0],
            SelectionFocus::Element,
            Vec::new(),
        );
    }
    if !source_ambiguous {
        if let Some(current_id) = state
            .selection
            .as_ref()
            .and_then(|selection| selection.anchor.as_ref())
            .and_then(|anchor| anchor.editor_node_id.as_deref())
        {
            if let Some(current) = rendered.iter().find(|node| node.id == current_id) {
                return commit_node(
                    state,
                    snapshot,
                    active_document_path,
                    current,
                    SelectionFocus::Element,
                    Vec::new(),
                );
            }
        }
    }

    let resolution = if source_ambiguous || rendered.len() > 1 {
        SelectionResolution::Ambiguous
    } else {
        SelectionResolution::NotRendered
    };
    let diagnostic = if source_ambiguous {
        "Poziția din cod corespunde mai multor noduri SourceGraph."
    } else if rendered.len() > 1 {
        "Nodul SourceGraph are mai multe instanțe randate; instanța nu poate fi ghicită."
    } else {
        "Nodul SourceGraph selectat nu este randat în Canvas-ul curent."
    };
    let revision = next_selection_revision(state)?;
    state.selection = Some(selection_from_source_node(
        revision,
        snapshot,
        active_document_path,
        source,
        resolution,
        diagnostic,
    ));
    state.inspector_facts = None;
    Ok(())
}

fn set_hover(
    state: &mut SelectionCoordinatorState,
    snapshot: &EditorNavigationSnapshot,
    editor_node_id: &str,
    document_epoch: u64,
) -> Result<(), String> {
    if document_epoch == 0 {
        return Err("HoverSnapshot cere un documentEpoch valid.".to_string());
    }
    let node = snapshot
        .nodes
        .iter()
        .find(|node| node.id == editor_node_id)
        .ok_or_else(|| "HoverSnapshot nu găsește nodul EditorNavigation.".to_string())?;
    let revision = next_hover_revision(state)?;
    let (primary, render_ids, boundary_id) = preview_identity(node);
    state.hover = Some(HoverSnapshot {
        schema_version: SELECTION_COORDINATOR_SCHEMA_VERSION,
        hover_revision: revision,
        canvas_identity: snapshot.identity.clone(),
        route: snapshot.route.clone(),
        document_epoch,
        editor_node_id: node.id.clone(),
        subject_kind: subject_kind(node.kind),
        primary_render_instance_id: primary,
        render_instance_ids: render_ids,
        boundary_instance_id: boundary_id,
    });
    Ok(())
}

fn clear_hover(
    state: &mut SelectionCoordinatorState,
    snapshot: &EditorNavigationSnapshot,
    document_epoch: u64,
) {
    if state.hover.as_ref().is_some_and(|hover| {
        hover.canvas_identity == snapshot.identity && hover.document_epoch == document_epoch
    }) {
        state.hover = None;
    }
}

enum RebaseCandidate<'a> {
    Resolved(&'a EditorNavigationNode),
    NotRendered,
    Ambiguous,
}

fn rebase_candidate<'a>(
    snapshot: &'a EditorNavigationSnapshot,
    anchor: &SelectionAnchor,
    provenance: Option<&EditorSourceProvenance>,
    source_graph: Option<&SourceGraph>,
    source_identity_aliases: Option<&HashMap<String, String>>,
) -> RebaseCandidate<'a> {
    if let Some(source_identity_aliases) = source_identity_aliases {
        if let Some(source_node_id) = anchor.source_node_id.as_deref() {
            match resolve_rebase_source_alias(
                snapshot,
                source_graph,
                source_identity_aliases,
                source_node_id,
            ) {
                SourceAliasResolution::Resolved(resolved_source_id) => {
                    return rebase_candidate_for_source_id(snapshot, anchor, resolved_source_id);
                }
                SourceAliasResolution::Unresolved => {
                    // A Rust-published transition is authoritative. Falling back
                    // to a reused physical identity could select a sibling, so an
                    // unresolved alias fails closed as not rendered.
                    return RebaseCandidate::NotRendered;
                }
                SourceAliasResolution::NotAliased => {}
            }
        }
    }

    if let Some(editor_node_id) = anchor.editor_node_id.as_deref() {
        if let Some(node) = snapshot.nodes.iter().find(|node| node.id == editor_node_id) {
            return RebaseCandidate::Resolved(node);
        }
    }
    if let Some(render_instance_id) = anchor.render_instance_id.as_deref() {
        if let Some(node) = snapshot
            .nodes
            .iter()
            .find(|node| node.render_instance_id.as_deref() == Some(render_instance_id))
        {
            return RebaseCandidate::Resolved(node);
        }
    }

    let semantic: Vec<&EditorNavigationNode> = snapshot
        .nodes
        .iter()
        .filter(|node| {
            provenance.is_some_and(|expected| &node.source_provenance == expected)
                && node.component_invocation_ids == anchor.component_invocation_ids
                && node.block_source_instance_ids == anchor.block_source_instance_ids
                && node.dynamic_widget_source_instance_ids
                    == anchor.dynamic_widget_source_instance_ids
                && optional_anchor_matches(
                    anchor.binding_path.as_deref(),
                    node.binding_path.as_deref(),
                )
                && optional_anchor_matches(
                    anchor.binding_key.as_deref(),
                    node.binding_key.as_deref(),
                )
        })
        .collect();
    if semantic.len() == 1 {
        return RebaseCandidate::Resolved(semantic[0]);
    }
    if semantic.len() > 1 {
        return RebaseCandidate::Ambiguous;
    }

    let Some(source_node_id) = anchor.source_node_id.as_deref() else {
        return RebaseCandidate::NotRendered;
    };
    let source_matches: Vec<&EditorNavigationNode> = snapshot
        .nodes
        .iter()
        .filter(|node| node.source_node_id.as_deref() == Some(source_node_id))
        .collect();
    match source_matches.as_slice() {
        [node] => RebaseCandidate::Resolved(node),
        [] => RebaseCandidate::NotRendered,
        _ => RebaseCandidate::Ambiguous,
    }
}

fn rebase_candidate_for_source_id<'a>(
    snapshot: &'a EditorNavigationSnapshot,
    anchor: &SelectionAnchor,
    source_node_id: &str,
) -> RebaseCandidate<'a> {
    let source_matches: Vec<&EditorNavigationNode> = snapshot
        .nodes
        .iter()
        .filter(|node| {
            node.source_node_id.as_deref() == Some(source_node_id)
                && node.component_invocation_ids == anchor.component_invocation_ids
                && node.block_source_instance_ids == anchor.block_source_instance_ids
                && node.dynamic_widget_source_instance_ids
                    == anchor.dynamic_widget_source_instance_ids
                && optional_anchor_matches(
                    anchor.binding_path.as_deref(),
                    node.binding_path.as_deref(),
                )
                && optional_anchor_matches(
                    anchor.binding_key.as_deref(),
                    node.binding_key.as_deref(),
                )
        })
        .collect();
    match source_matches.as_slice() {
        [node] => RebaseCandidate::Resolved(node),
        [] => RebaseCandidate::NotRendered,
        _ => RebaseCandidate::Ambiguous,
    }
}

enum SourceAliasResolution<'a> {
    NotAliased,
    Resolved(&'a str),
    Unresolved,
}

fn resolve_rebase_source_alias<'a>(
    snapshot: &EditorNavigationSnapshot,
    source_graph: Option<&SourceGraph>,
    aliases: &'a HashMap<String, String>,
    source_node_id: &str,
) -> SourceAliasResolution<'a> {
    let Some(first) = aliases.get(source_node_id) else {
        return SourceAliasResolution::NotAliased;
    };
    if first.trim().is_empty() || first == source_node_id {
        return SourceAliasResolution::Unresolved;
    }

    let mut current = first.as_str();
    let mut visited = HashSet::from([source_node_id.to_string()]);
    loop {
        if current.trim().is_empty() || !visited.insert(current.to_string()) {
            return SourceAliasResolution::Unresolved;
        }

        let live_in_source_graph =
            source_graph.is_some_and(|graph| graph.nodes.iter().any(|node| node.id == current));
        let live_in_snapshot = snapshot
            .nodes
            .iter()
            .any(|node| node.source_node_id.as_deref() == Some(current));
        if live_in_source_graph || live_in_snapshot {
            return SourceAliasResolution::Resolved(current);
        }

        let Some(next) = aliases.get(current).map(String::as_str) else {
            return SourceAliasResolution::Unresolved;
        };
        current = next;
    }
}

fn optional_anchor_matches(expected: Option<&str>, actual: Option<&str>) -> bool {
    expected.map_or(true, |expected| actual == Some(expected))
}

fn selection_from_node(
    revision: u64,
    snapshot: &EditorNavigationSnapshot,
    active_document_path: Option<&str>,
    node: &EditorNavigationNode,
    focus: SelectionFocus,
    resolution: SelectionResolution,
    diagnostics: Vec<String>,
) -> SelectionSnapshot {
    let subject = SelectionSubject {
        kind: subject_kind(node.kind),
        tag: node.tag.clone(),
        label: node.label.clone(),
    };
    let anchor = SelectionAnchor {
        editor_node_id: Some(node.id.clone()),
        source_node_id: node.source_node_id.clone(),
        render_instance_id: node.render_instance_id.clone(),
        render_instance_ids: node
            .boundary
            .as_ref()
            .map(|boundary| boundary.root_render_instance_ids.clone())
            .unwrap_or_else(|| node.render_instance_id.iter().cloned().collect()),
        boundary_instance_id: node
            .boundary
            .as_ref()
            .map(|boundary| boundary.boundary_instance_id.clone()),
        file: node.file.clone(),
        range: node.range.clone(),
        provenance_stack: node.provenance_stack.clone(),
        component_invocation_ids: node.component_invocation_ids.clone(),
        block_source_instance_ids: node.block_source_instance_ids.clone(),
        dynamic_widget_source_instance_ids: node.dynamic_widget_source_instance_ids.clone(),
        binding_key: node.binding_key.clone(),
        binding_path: node.binding_path.clone(),
    };
    let provenance = Some(node.source_provenance.clone());
    let capabilities = Some(node.capabilities.clone());
    let projections = projections(
        resolution,
        Some(&subject),
        Some(&anchor),
        provenance.as_ref(),
        capabilities.as_ref(),
        &focus,
    );
    SelectionSnapshot {
        schema_version: SELECTION_COORDINATOR_SCHEMA_VERSION,
        selection_revision: revision,
        project_root: snapshot.identity.project_root.clone(),
        runtime_session_id: snapshot.identity.runtime_session_id.clone(),
        canvas_identity: snapshot.identity.clone(),
        route: snapshot.route.clone(),
        active_document_path: active_document_path.map(str::to_string),
        resolution,
        subject: Some(subject),
        focus,
        anchor: Some(anchor),
        provenance,
        capabilities,
        projections,
        diagnostics,
    }
}

fn selection_from_source_node(
    revision: u64,
    snapshot: &EditorNavigationSnapshot,
    active_document_path: Option<&str>,
    source: &SourceNode,
    resolution: SelectionResolution,
    diagnostic: &str,
) -> SelectionSnapshot {
    let subject = SelectionSubject {
        kind: source_subject_kind(&source.kind),
        tag: None,
        label: source.label.clone(),
    };
    let anchor = SelectionAnchor {
        editor_node_id: None,
        source_node_id: Some(source.id.clone()),
        render_instance_id: None,
        render_instance_ids: Vec::new(),
        boundary_instance_id: None,
        file: Some(source.file.clone()),
        range: source.range.clone(),
        provenance_stack: vec![source.id.clone()],
        component_invocation_ids: Vec::new(),
        block_source_instance_ids: Vec::new(),
        dynamic_widget_source_instance_ids: Vec::new(),
        binding_key: None,
        binding_path: None,
    };
    let origin = match &source.origin {
        SourceOrigin::Local => EditorNavigationOrigin::Project,
        SourceOrigin::Theme => EditorNavigationOrigin::Theme,
    };
    let provenance = EditorSourceProvenance {
        definition: Some(EditorSourceReference {
            source_node_id: Some(source.id.clone()),
            source_kind: Some(source.kind.clone()),
            file: source.file.clone(),
            range: source.range.clone(),
            label: source.label.clone(),
            origin,
            theme_name: source.theme_name.clone(),
            can_open_in_code: source.capabilities.can_open_in_code,
        }),
        composition: None,
        resolution: EditorSourceResolution::Direct,
    };
    let capabilities = EditorNavigationCapabilities {
        can_select: true,
        can_inspect: false,
        can_open_in_code: source.capabilities.can_open_in_code,
        can_enter_boundary: false,
        can_move_atomic: false,
        can_move: false,
        can_edit_text: source.capabilities.can_edit_text,
        can_edit_attributes: source.capabilities.can_edit_attributes,
        read_only: source.origin == SourceOrigin::Theme,
        requires_edit_scope_id: None,
        reason_code: source.capabilities.reason_code,
    };
    let focus = SelectionFocus::Element;
    let projections = projections(
        resolution,
        Some(&subject),
        Some(&anchor),
        Some(&provenance),
        Some(&capabilities),
        &focus,
    );
    SelectionSnapshot {
        schema_version: SELECTION_COORDINATOR_SCHEMA_VERSION,
        selection_revision: revision,
        project_root: snapshot.identity.project_root.clone(),
        runtime_session_id: snapshot.identity.runtime_session_id.clone(),
        canvas_identity: snapshot.identity.clone(),
        route: snapshot.route.clone(),
        active_document_path: active_document_path.map(str::to_string),
        resolution,
        subject: Some(subject),
        focus,
        anchor: Some(anchor),
        provenance: Some(provenance),
        capabilities: Some(capabilities),
        projections,
        diagnostics: vec![diagnostic.to_string()],
    }
}

fn empty_selection(
    revision: u64,
    snapshot: &EditorNavigationSnapshot,
    active_document_path: Option<&str>,
    focus: SelectionFocus,
) -> SelectionSnapshot {
    SelectionSnapshot {
        schema_version: SELECTION_COORDINATOR_SCHEMA_VERSION,
        selection_revision: revision,
        project_root: snapshot.identity.project_root.clone(),
        runtime_session_id: snapshot.identity.runtime_session_id.clone(),
        canvas_identity: snapshot.identity.clone(),
        route: snapshot.route.clone(),
        active_document_path: active_document_path.map(str::to_string),
        resolution: SelectionResolution::Cleared,
        subject: None,
        focus: focus.clone(),
        anchor: None,
        provenance: None,
        capabilities: None,
        projections: projections(SelectionResolution::Cleared, None, None, None, None, &focus),
        diagnostics: Vec::new(),
    }
}

fn projections(
    resolution: SelectionResolution,
    subject: Option<&SelectionSubject>,
    anchor: Option<&SelectionAnchor>,
    provenance: Option<&EditorSourceProvenance>,
    capabilities: Option<&EditorNavigationCapabilities>,
    focus: &SelectionFocus,
) -> SelectionUiProjections {
    let resolved = resolution == SelectionResolution::Resolved;
    let (primary, render_instance_ids, boundary_instance_id) = if resolved {
        anchor.map_or((None, Vec::new(), None), |anchor| {
            (
                anchor
                    .render_instance_id
                    .clone()
                    .or_else(|| anchor.render_instance_ids.first().cloned()),
                anchor.render_instance_ids.clone(),
                anchor.boundary_instance_id.clone(),
            )
        })
    } else {
        (None, Vec::new(), None)
    };
    let editor_node_id = resolved
        .then(|| anchor.and_then(|anchor| anchor.editor_node_id.clone()))
        .flatten();
    let (code_file, code_range) = focus_code_target(focus)
        .map(|(file, range)| (Some(file.to_string()), range.cloned()))
        .unwrap_or_else(|| {
            (
                anchor.and_then(|anchor| anchor.file.clone()),
                anchor.and_then(|anchor| anchor.range.clone()),
            )
        });
    SelectionUiProjections {
        preview: SelectionPreviewProjection {
            editor_node_id: editor_node_id.clone(),
            target_kind: subject.map(|subject| subject.kind),
            primary_render_instance_id: primary,
            render_instance_ids,
            boundary_instance_id,
        },
        layers: SelectionLayersProjection {
            editor_node_id: editor_node_id.clone(),
        },
        code: SelectionCodeProjection {
            file: code_file,
            range: code_range,
            focus: focus.clone(),
        },
        inspector: SelectionInspectorProjection {
            editor_node_id,
            subject_kind: subject.map(|subject| subject.kind),
            can_inspect: capabilities.is_some_and(|capabilities| capabilities.can_inspect),
        },
        status: SelectionStatusProjection {
            provenance: provenance.cloned(),
            focus: focus.clone(),
        },
    }
}

fn focus_code_target(focus: &SelectionFocus) -> Option<(&str, Option<&SourceRange>)> {
    match focus {
        SelectionFocus::Element => None,
        SelectionFocus::CssRule { file, range, .. }
        | SelectionFocus::CssProperty { file, range, .. } => Some((file, range.as_ref())),
        SelectionFocus::JsBehavior { file, .. } => Some((file, None)),
    }
}

fn validate_focus(focus: &SelectionFocus) -> Result<(), String> {
    let invalid = match focus {
        SelectionFocus::Element => false,
        SelectionFocus::CssRule { file, selector, .. } => {
            file.trim().is_empty() || selector.trim().is_empty()
        }
        SelectionFocus::CssProperty {
            file,
            selector,
            property,
            ..
        } => file.trim().is_empty() || selector.trim().is_empty() || property.trim().is_empty(),
        SelectionFocus::JsBehavior { file, .. } => file.trim().is_empty(),
    };
    if invalid {
        Err("SelectionFocus este incomplet.".to_string())
    } else {
        Ok(())
    }
}

fn subject_kind(kind: EditorNavigationNodeKind) -> SelectionSubjectKind {
    match kind {
        EditorNavigationNodeKind::HtmlElement => SelectionSubjectKind::HtmlElement,
        EditorNavigationNodeKind::TeraBoundary => SelectionSubjectKind::TeraBoundary,
        EditorNavigationNodeKind::MarkdownBoundary => SelectionSubjectKind::MarkdownBoundary,
        EditorNavigationNodeKind::RuntimeElement => SelectionSubjectKind::RuntimeElement,
    }
}

fn source_subject_kind(kind: &SourceNodeKind) -> SelectionSubjectKind {
    match kind {
        SourceNodeKind::Html => SelectionSubjectKind::HtmlElement,
        SourceNodeKind::Script => SelectionSubjectKind::RuntimeElement,
        _ => SelectionSubjectKind::TeraBoundary,
    }
}

fn preview_identity(node: &EditorNavigationNode) -> (Option<String>, Vec<String>, Option<String>) {
    if let Some(boundary) = node.boundary.as_ref() {
        return (
            boundary.root_render_instance_ids.first().cloned(),
            boundary.root_render_instance_ids.clone(),
            Some(boundary.boundary_instance_id.clone()),
        );
    }
    (
        node.render_instance_id.clone(),
        node.render_instance_id.iter().cloned().collect(),
        None,
    )
}

fn normalized_path(path: &str) -> String {
    path.trim().trim_start_matches('/').replace('\\', "/")
}

fn next_selection_revision(state: &mut SelectionCoordinatorState) -> Result<u64, String> {
    state.next_selection_revision = state
        .next_selection_revision
        .checked_add(1)
        .ok_or_else(|| "SelectionCoordinator a epuizat reviziile selecției.".to_string())?;
    Ok(state.next_selection_revision)
}

fn next_hover_revision(state: &mut SelectionCoordinatorState) -> Result<u64, String> {
    state.next_hover_revision = state
        .next_hover_revision
        .checked_add(1)
        .ok_or_else(|| "SelectionCoordinator a epuizat reviziile hover.".to_string())?;
    Ok(state.next_hover_revision)
}

fn validate_inspector_facts(
    selection: &SelectionSnapshot,
    mut facts: InspectorSelectionPhysicalFacts,
) -> Result<InspectorSelectionPhysicalFacts, String> {
    facts.observed_tag = normalize_observed_tag(&facts.observed_tag)?;
    if selection
        .subject
        .as_ref()
        .and_then(|subject| subject.tag.as_deref())
        .is_some_and(|tag| tag.to_ascii_lowercase() != facts.observed_tag)
    {
        return Err("Faptele inspectorului descriu alt tag decât selecția semantică.".to_string());
    }
    if facts.element_id.len() > 512 || facts.element_id.chars().any(char::is_control) {
        return Err("Faptele inspectorului conțin un ID invalid.".to_string());
    }
    if facts.classes.len() > 64 {
        return Err("Faptele inspectorului depășesc limita de clase.".to_string());
    }
    let mut classes = Vec::with_capacity(facts.classes.len());
    for class_name in facts.classes {
        if class_name.is_empty()
            || class_name.len() > 256
            || class_name
                .chars()
                .any(|character| character.is_control() || character.is_whitespace())
        {
            return Err("Faptele inspectorului conțin o clasă invalidă.".to_string());
        }
        if matches!(
            class_name.as_str(),
            "pana-studio-empty-editable"
                | "pana-studio-empty-tera-slot"
                | "pana-studio-active-document-root"
        ) {
            continue;
        }
        if !classes.contains(&class_name) {
            classes.push(class_name);
        }
    }
    facts.classes = classes;
    if let Some(block) = facts.block_context.as_mut() {
        if block.provider_id.trim().is_empty()
            || block.provider_id.len() > 256
            || block.provider_id.chars().any(char::is_control)
        {
            return Err("Faptele inspectorului conțin un provider de bloc invalid.".to_string());
        }
        block.provider_id = block.provider_id.trim().to_string();
        block.root_tag = normalize_observed_tag(&block.root_tag)?;
    }
    Ok(facts)
}

fn normalize_observed_tag(tag: &str) -> Result<String, String> {
    let tag = tag.trim().to_ascii_lowercase();
    if tag.is_empty()
        || tag.len() > 64
        || !tag
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-:_".contains(character))
    {
        return Err("Faptele inspectorului conțin un tag invalid.".to_string());
    }
    Ok(tag)
}

fn active_css_class(focus: &SelectionFocus) -> Option<String> {
    let selector = match focus {
        SelectionFocus::CssRule { selector, .. } | SelectionFocus::CssProperty { selector, .. } => {
            selector.trim()
        }
        _ => return None,
    };
    let selector = selector.strip_prefix('.')?;
    let class_end = selector
        .find(|character: char| {
            !character.is_ascii_alphanumeric() && character != '_' && character != '-'
        })
        .unwrap_or(selector.len());
    if class_end == 0 {
        return None;
    }
    let suffix = &selector[class_end..];
    if suffix.chars().any(|character| {
        character.is_whitespace() || matches!(character, '>' | '+' | '~' | '.' | '#' | '[')
    }) {
        return None;
    }
    Some(selector[..class_end].to_string())
}

fn display_selector(tag: &str, element_id: &str, classes: &[String]) -> String {
    if !element_id.is_empty() {
        return format!("{tag}#{}", escape_css_identifier(element_id));
    }
    if classes.is_empty() {
        return tag.to_string();
    }
    format!(
        "{tag}.{}",
        classes
            .iter()
            .map(|class_name| escape_css_identifier(class_name))
            .collect::<Vec<_>>()
            .join(".")
    )
}

fn escape_css_identifier(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
                character.to_string()
            } else {
                format!("\\{character}")
            }
        })
        .collect()
}

fn inspector_summary(
    state: &SelectionCoordinatorState,
) -> Result<InspectorSelectionSummarySnapshot, String> {
    let selection = state
        .selection
        .as_ref()
        .ok_or_else(|| "SelectionCoordinator nu are context de sesiune.".to_string())?;
    let subject = selection.subject.as_ref();
    let can_inspect = selection.projections.inspector.can_inspect;
    let active_document = state
        .active_inspector_document
        .as_ref()
        .filter(|identity| identity.canvas == selection.canvas_identity);
    let matching_facts = state.inspector_facts.as_ref().filter(|facts| {
        facts.selection_revision == selection.selection_revision
            && facts.canvas_identity == selection.canvas_identity
            && active_document
                .is_some_and(|identity| identity.document_epoch == facts.document_epoch)
            && selection
                .anchor
                .as_ref()
                .and_then(|anchor| anchor.render_instance_id.as_deref())
                == Some(facts.render_instance_id.as_str())
    });

    let mut state_value = InspectorSelectionSummaryState::Resolving;
    let mut reason = Some(InspectorSelectionSummaryReason::AwaitingPhysicalFacts);
    let mut selector = subject
        .and_then(|subject| subject.tag.clone())
        .or_else(|| subject.map(|subject| subject.label.clone()));
    let mut element_id = None;
    let mut classes = Vec::new();
    let mut block_context = None;

    match selection.resolution {
        SelectionResolution::Cleared => {
            state_value = InspectorSelectionSummaryState::Empty;
            reason = Some(InspectorSelectionSummaryReason::NoSelection);
            selector = None;
        }
        SelectionResolution::NotRendered => {
            state_value = InspectorSelectionSummaryState::NotRendered;
            reason = Some(InspectorSelectionSummaryReason::SelectionNotRendered);
        }
        SelectionResolution::Ambiguous => {
            state_value = InspectorSelectionSummaryState::Ambiguous;
            reason = Some(InspectorSelectionSummaryReason::SelectionAmbiguous);
        }
        SelectionResolution::Resolved if !can_inspect => {
            state_value = InspectorSelectionSummaryState::Uninspectable;
            reason = Some(InspectorSelectionSummaryReason::InspectionDisabled);
        }
        SelectionResolution::Resolved
            if subject.is_some_and(|subject| {
                matches!(
                    subject.kind,
                    SelectionSubjectKind::TeraBoundary | SelectionSubjectKind::MarkdownBoundary
                )
            }) =>
        {
            state_value = InspectorSelectionSummaryState::Resolved;
            reason = None;
        }
        SelectionResolution::Resolved
            if selection
                .anchor
                .as_ref()
                .and_then(|anchor| anchor.render_instance_id.as_ref())
                .is_none() =>
        {
            state_value = InspectorSelectionSummaryState::Uninspectable;
            reason = Some(InspectorSelectionSummaryReason::MissingRenderInstance);
        }
        SelectionResolution::Resolved => {
            if let Some(accepted) = matching_facts {
                state_value = InspectorSelectionSummaryState::Resolved;
                reason = None;
                element_id = (!accepted.facts.element_id.is_empty())
                    .then(|| accepted.facts.element_id.clone());
                classes = accepted.facts.classes.clone();
                block_context = accepted.facts.block_context.clone();
                if let Some(tag) = subject.and_then(|subject| subject.tag.as_deref()) {
                    selector = Some(display_selector(tag, &accepted.facts.element_id, &classes));
                }
            }
        }
    }

    let diagnostics = reason
        .map(|code| InspectorSelectionSummaryDiagnostic {
            code,
            message: selection
                .diagnostics
                .first()
                .cloned()
                .unwrap_or_else(|| inspector_reason_diagnostic(code).to_string()),
        })
        .into_iter()
        .collect();

    Ok(InspectorSelectionSummarySnapshot {
        schema_version: SELECTION_COORDINATOR_SCHEMA_VERSION,
        project_root: selection.project_root.clone(),
        runtime_session_id: selection.runtime_session_id.clone(),
        selection_revision: selection.selection_revision,
        canvas_identity: selection.canvas_identity.clone(),
        document_epoch: active_document.map(|identity| identity.document_epoch),
        render_instance_id: selection
            .anchor
            .as_ref()
            .and_then(|anchor| anchor.render_instance_id.clone()),
        state: state_value,
        subject_kind: subject.map(|subject| subject.kind),
        tag: subject.and_then(|subject| subject.tag.clone()),
        label: subject.map(|subject| subject.label.clone()),
        selector,
        element_id,
        classes,
        block_context,
        active_css_class: active_css_class(&selection.focus),
        can_inspect,
        reason,
        diagnostics,
    })
}

fn inspector_reason_diagnostic(reason: InspectorSelectionSummaryReason) -> &'static str {
    match reason {
        InspectorSelectionSummaryReason::NoSelection => "Nu există un subiect semantic selectat.",
        InspectorSelectionSummaryReason::AwaitingPhysicalFacts => {
            "Selecția semantică așteaptă faptele fizice bounded ale CanvasAgent."
        }
        InspectorSelectionSummaryReason::SelectionNotRendered => {
            "Subiectul selectat nu este randat în Canvas-ul curent."
        }
        InspectorSelectionSummaryReason::SelectionAmbiguous => {
            "Subiectul selectat corespunde mai multor instanțe posibile."
        }
        InspectorSelectionSummaryReason::InspectionDisabled => {
            "Capabilitățile Rust nu permit inspectarea acestui subiect."
        }
        InspectorSelectionSummaryReason::MissingRenderInstance => {
            "Selecția nu are o instanță fizică ce poate fi inspectată."
        }
    }
}

fn coordinator_snapshot(
    state: &SelectionCoordinatorState,
) -> Result<SelectionCoordinatorSnapshot, String> {
    Ok(SelectionCoordinatorSnapshot {
        schema_version: SELECTION_COORDINATOR_SCHEMA_VERSION,
        selection: state
            .selection
            .clone()
            .ok_or_else(|| "SelectionCoordinator nu are context de sesiune.".to_string())?,
        hover: state.hover.clone(),
        inspector_summary: inspector_summary(state)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        kernel::editor_navigation::{
            editor_navigation_snapshot_for_test, EditorNavigationOrigin, EditorNavigationSurface,
            EditorSourceReference, EditorSourceResolution,
        },
        source_graph::model::SourceNodeKind,
    };

    fn identity(transaction: &str) -> CanvasProjectionIdentity {
        CanvasProjectionIdentity {
            project_root: "/project".to_string(),
            runtime_session_id: "session:runtime-1".to_string(),
            workspace_revision: 7,
            transaction_id: transaction.to_string(),
            preview_revision: format!("preview-{transaction}"),
        }
    }

    fn provenance(source_id: &str, file: &str, range: SourceRange) -> EditorSourceProvenance {
        EditorSourceProvenance {
            definition: Some(EditorSourceReference {
                source_node_id: Some(source_id.to_string()),
                source_kind: Some(SourceNodeKind::Html),
                file: file.to_string(),
                range: Some(range),
                label: source_id.to_string(),
                origin: EditorNavigationOrigin::Project,
                theme_name: None,
                can_open_in_code: true,
            }),
            composition: None,
            resolution: EditorSourceResolution::Direct,
        }
    }

    fn node(
        id: &str,
        source_id: &str,
        render_id: &str,
        binding_key: Option<&str>,
        range: SourceRange,
    ) -> EditorNavigationNode {
        EditorNavigationNode {
            id: id.to_string(),
            parent_id: None,
            children: Vec::new(),
            order: 0,
            kind: EditorNavigationNodeKind::HtmlElement,
            label: "<h1>".to_string(),
            tag: Some("h1".to_string()),
            source_node_id: Some(source_id.to_string()),
            render_instance_id: Some(render_id.to_string()),
            source_kind: Some(SourceNodeKind::Html),
            file: Some("templates/index.html".to_string()),
            range: Some(range.clone()),
            origin: EditorNavigationOrigin::Project,
            theme_name: None,
            source_provenance: provenance(source_id, "templates/index.html", range),
            provenance_stack: vec![source_id.to_string()],
            component_definition_ids: Vec::new(),
            component_invocation_ids: vec!["include:hero".to_string()],
            block_definition_ids: Vec::new(),
            block_source_instance_ids: Vec::new(),
            dynamic_widget_provider_ids: Vec::new(),
            dynamic_widget_source_instance_ids: Vec::new(),
            binding_key: binding_key.map(str::to_string),
            binding_path: binding_key.map(|key| format!("hero.items[{key}]")),
            boundary: None,
            capabilities: EditorNavigationCapabilities {
                can_select: true,
                can_inspect: true,
                can_open_in_code: true,
                can_enter_boundary: false,
                can_move_atomic: false,
                can_move: true,
                can_edit_text: true,
                can_edit_attributes: true,
                read_only: false,
                requires_edit_scope_id: None,
                reason_code: None,
            },
        }
    }

    fn snapshot(transaction: &str, nodes: Vec<EditorNavigationNode>) -> EditorNavigationSnapshot {
        editor_navigation_snapshot_for_test(
            identity(transaction),
            "model-1",
            "/",
            EditorNavigationSurface::CanonicalPreview,
            Vec::new(),
            nodes,
        )
    }

    fn range(start: usize, end: usize) -> SourceRange {
        SourceRange {
            start,
            end,
            line: 1,
            column: start + 1,
            end_line: 1,
            end_column: end + 1,
        }
    }

    fn inspector_document(
        canvas: &CanvasProjectionIdentity,
        document_epoch: u64,
    ) -> CanvasInteractionIdentity {
        CanvasInteractionIdentity {
            canvas: canvas.clone(),
            route: "/".to_string(),
            document_epoch,
            agent_instance_id: format!("test-agent-{document_epoch}"),
        }
    }

    fn inspector_facts(
        observed_tag: &str,
        element_id: &str,
        classes: &[&str],
    ) -> InspectorSelectionPhysicalFacts {
        InspectorSelectionPhysicalFacts {
            observed_tag: observed_tag.to_string(),
            element_id: element_id.to_string(),
            classes: classes
                .iter()
                .map(|class_name| class_name.to_string())
                .collect(),
            block_context: None,
        }
    }

    #[test]
    fn selection_is_owned_by_rust_and_focus_does_not_replace_subject() {
        let runtime = SelectionCoordinatorRuntime::default();
        let snapshot = snapshot(
            "tx-1",
            vec![node(
                "editor_render:a",
                "source:h1",
                "a",
                None,
                range(10, 30),
            )],
        );
        let selected = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor_render:a".to_string(),
                },
            )
            .unwrap();
        runtime
            .bind_inspector_document(inspector_document(&snapshot.identity, 1))
            .unwrap();
        runtime
            .accept_observation(SelectionObservationInput {
                schema_version: SELECTION_COORDINATOR_SCHEMA_VERSION,
                selection_revision: selected.selection.selection_revision,
                canvas_identity: snapshot.identity.clone(),
                document_epoch: 1,
                render_instance_id: "a".to_string(),
                inspector_facts: inspector_facts("h1", "", &["hero-title"]),
            })
            .unwrap();
        let focused = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SetFocus {
                    focus: SelectionFocus::CssRule {
                        file: "sass/index.scss".to_string(),
                        selector: ".hero-title".to_string(),
                        viewport: None,
                        range: None,
                    },
                    expected_selection_revision: None,
                },
            )
            .unwrap();
        assert_eq!(
            focused.selection.subject, selected.selection.subject,
            "focusul CSS nu trebuie să înlocuiască elementul"
        );
        assert_eq!(
            focused
                .selection
                .projections
                .layers
                .editor_node_id
                .as_deref(),
            Some("editor_render:a")
        );
        assert_eq!(
            focused.selection.projections.code.file.as_deref(),
            Some("sass/index.scss")
        );
        assert!(focused.selection.selection_revision > selected.selection.selection_revision);
        assert_eq!(
            focused.inspector_summary.state,
            InspectorSelectionSummaryState::Resolved
        );
    }

    #[test]
    fn css_or_js_focus_cannot_become_an_independent_selection() {
        let runtime = SelectionCoordinatorRuntime::default();
        let snapshot = snapshot(
            "tx-1",
            vec![node(
                "editor_render:a",
                "source:h1",
                "a",
                None,
                range(10, 30),
            )],
        );
        let error = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SetFocus {
                    focus: SelectionFocus::CssRule {
                        file: "sass/index.scss".to_string(),
                        selector: ".hero-title".to_string(),
                        viewport: None,
                        range: None,
                    },
                    expected_selection_revision: None,
                },
            )
            .unwrap_err();
        assert!(error.contains("subiect semantic"));
    }

    #[test]
    fn rebase_uses_binding_identity_and_refuses_ambiguous_source_fallback() {
        let runtime = SelectionCoordinatorRuntime::default();
        let original = snapshot(
            "tx-1",
            vec![node(
                "editor_render:old",
                "source:item",
                "old",
                Some("second"),
                range(10, 30),
            )],
        );
        runtime
            .apply(
                &original,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor_render:old".to_string(),
                },
            )
            .unwrap();
        let rebased = snapshot(
            "tx-2",
            vec![
                node(
                    "editor_render:first",
                    "source:item",
                    "first",
                    Some("first"),
                    range(10, 30),
                ),
                node(
                    "editor_render:second",
                    "source:item",
                    "second",
                    Some("second"),
                    range(10, 30),
                ),
            ],
        );
        let receipt = runtime
            .apply(
                &rebased,
                Some("templates/index.html"),
                None,
                SelectionIntent::Rebase,
            )
            .unwrap();
        assert_eq!(receipt.selection.resolution, SelectionResolution::Resolved);
        assert_eq!(
            receipt.selection.anchor.unwrap().binding_key.as_deref(),
            Some("second")
        );

        let ambiguous = snapshot(
            "tx-3",
            vec![
                node("editor_render:x", "source:item", "x", None, range(10, 30)),
                node("editor_render:y", "source:item", "y", None, range(10, 30)),
            ],
        );
        let receipt = runtime
            .apply(
                &ambiguous,
                Some("templates/index.html"),
                None,
                SelectionIntent::Rebase,
            )
            .unwrap();
        assert_eq!(receipt.selection.resolution, SelectionResolution::Ambiguous);
        assert!(receipt
            .selection
            .projections
            .preview
            .render_instance_ids
            .is_empty());
    }

    #[test]
    fn rebase_prefers_source_alias_over_a_physical_identity_reused_by_the_next_sibling() {
        let runtime = SelectionCoordinatorRuntime::default();
        let original = snapshot(
            "tx-before-class",
            vec![
                node(
                    "editor_render:first",
                    "source:plain-span-0",
                    "render:first",
                    None,
                    range(10, 20),
                ),
                node(
                    "editor_render:selected",
                    "source:plain-span-1",
                    "render:selected",
                    None,
                    range(21, 31),
                ),
                node(
                    "editor_render:third",
                    "source:plain-span-2",
                    "render:third",
                    None,
                    range(32, 42),
                ),
            ],
        );
        runtime
            .apply(
                &original,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor_render:selected".to_string(),
                },
            )
            .unwrap();

        // Adăugarea clasei schimbă label-ul semantic al span-ului selectat.
        // Al treilea span preia astfel occurrence-ul, source ID-ul și ID-urile
        // fizice pe care le avea înainte al doilea span.
        let mut projected = snapshot(
            "tx-after-class",
            vec![
                node(
                    "editor_render:first",
                    "source:plain-span-0",
                    "render:first",
                    None,
                    range(10, 20),
                ),
                node(
                    "editor_render:generated-class",
                    "source:class-span-0",
                    "render:generated-class",
                    None,
                    range(21, 50),
                ),
                node(
                    "editor_render:selected",
                    "source:plain-span-1",
                    "render:selected",
                    None,
                    range(51, 61),
                ),
            ],
        );
        projected.identity.workspace_revision = 8;
        let transition = SourceIdentityAliasTransition {
            revision_before: 7,
            revision_after: 8,
            aliases: HashMap::from([
                (
                    "source:plain-span-1".to_string(),
                    "source:class-span-0".to_string(),
                ),
                (
                    "source:plain-span-2".to_string(),
                    "source:plain-span-1".to_string(),
                ),
            ]),
        };
        let receipt = runtime
            .apply_with_source_alias_transition(
                &projected,
                Some("templates/index.html"),
                None,
                Some(&transition),
                SelectionIntent::Rebase,
            )
            .unwrap();

        let anchor = receipt.selection.anchor.unwrap();
        assert_eq!(receipt.selection.resolution, SelectionResolution::Resolved);
        assert_eq!(
            anchor.source_node_id.as_deref(),
            Some("source:class-span-0")
        );
        assert_eq!(
            anchor.editor_node_id.as_deref(),
            Some("editor_render:generated-class")
        );
        assert_eq!(
            anchor.render_instance_id.as_deref(),
            Some("render:generated-class")
        );

        runtime
            .apply(
                &projected,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor_render:selected".to_string(),
                },
            )
            .unwrap();
        let same_revision = runtime
            .apply_with_source_alias_transition(
                &projected,
                Some("templates/index.html"),
                None,
                Some(&transition),
                SelectionIntent::Rebase,
            )
            .unwrap();
        let anchor = same_revision.selection.anchor.unwrap();
        assert_eq!(
            anchor.source_node_id.as_deref(),
            Some("source:plain-span-1")
        );
        assert_eq!(
            anchor.editor_node_id.as_deref(),
            Some("editor_render:selected")
        );

        let mut unrelated_revision = projected.clone();
        unrelated_revision.identity.workspace_revision = 9;
        unrelated_revision.identity.transaction_id = "tx-unrelated-change".to_string();
        unrelated_revision.identity.preview_revision = "preview-unrelated-change".to_string();
        let after_unrelated_change = runtime
            .apply_with_source_alias_transition(
                &unrelated_revision,
                Some("templates/index.html"),
                None,
                Some(&transition),
                SelectionIntent::Rebase,
            )
            .unwrap();
        let anchor = after_unrelated_change.selection.anchor.unwrap();
        assert_eq!(
            anchor.source_node_id.as_deref(),
            Some("source:plain-span-1")
        );
        assert_eq!(
            anchor.editor_node_id.as_deref(),
            Some("editor_render:selected")
        );
    }

    #[test]
    fn an_unresolved_published_alias_cannot_fall_back_to_a_reused_physical_identity() {
        let runtime = SelectionCoordinatorRuntime::default();
        let original = snapshot(
            "tx-before-transition",
            vec![node(
                "editor_render:reused",
                "source:before",
                "render:reused",
                None,
                range(10, 20),
            )],
        );
        runtime
            .apply(
                &original,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor_render:reused".to_string(),
                },
            )
            .unwrap();

        let mut projected = snapshot(
            "tx-after-transition",
            vec![node(
                "editor_render:reused",
                "source:before",
                "render:reused",
                None,
                range(30, 40),
            )],
        );
        projected.identity.workspace_revision = 8;
        let transition = SourceIdentityAliasTransition {
            revision_before: 7,
            revision_after: 8,
            aliases: HashMap::from([(
                "source:before".to_string(),
                "source:not-rendered".to_string(),
            )]),
        };
        let receipt = runtime
            .apply_with_source_alias_transition(
                &projected,
                Some("templates/index.html"),
                None,
                Some(&transition),
                SelectionIntent::Rebase,
            )
            .unwrap();

        assert_eq!(
            receipt.selection.resolution,
            SelectionResolution::NotRendered
        );
        assert!(receipt
            .selection
            .projections
            .preview
            .render_instance_ids
            .is_empty());
    }

    #[test]
    fn stale_dom_observation_is_rejected() {
        let runtime = SelectionCoordinatorRuntime::default();
        let snapshot = snapshot(
            "tx-1",
            vec![node(
                "editor_render:a",
                "source:h1",
                "a",
                None,
                range(10, 30),
            )],
        );
        let selected = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor_render:a".to_string(),
                },
            )
            .unwrap();
        let error = runtime
            .accept_observation(SelectionObservationInput {
                schema_version: SELECTION_COORDINATOR_SCHEMA_VERSION,
                selection_revision: selected.selection.selection_revision - 1,
                canvas_identity: snapshot.identity.clone(),
                document_epoch: 1,
                render_instance_id: "a".to_string(),
                inspector_facts: inspector_facts("h1", "", &[]),
            })
            .unwrap_err();
        assert!(error.contains("revizii de selecție vechi"));
    }

    #[test]
    fn inspector_summary_is_a_typed_rust_projection_of_semantic_and_physical_facts() {
        let runtime = SelectionCoordinatorRuntime::default();
        let snapshot = snapshot(
            "tx-summary",
            vec![node(
                "editor_render:title",
                "source:title",
                "render:title",
                None,
                range(10, 30),
            )],
        );
        let initial = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::Rebase,
            )
            .unwrap();
        assert_eq!(
            initial.inspector_summary.state,
            InspectorSelectionSummaryState::Empty
        );
        assert_eq!(
            initial.inspector_summary.reason,
            Some(InspectorSelectionSummaryReason::NoSelection)
        );

        runtime
            .bind_inspector_document(inspector_document(&snapshot.identity, 41))
            .unwrap();
        let selected = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor_render:title".to_string(),
                },
            )
            .unwrap();
        assert_eq!(
            selected.inspector_summary.state,
            InspectorSelectionSummaryState::Resolving
        );
        assert_eq!(
            selected.inspector_summary.reason,
            Some(InspectorSelectionSummaryReason::AwaitingPhysicalFacts)
        );

        let accepted = runtime
            .accept_observation(SelectionObservationInput {
                schema_version: SELECTION_COORDINATOR_SCHEMA_VERSION,
                selection_revision: selected.selection.selection_revision,
                canvas_identity: snapshot.identity.clone(),
                document_epoch: 41,
                render_instance_id: "render:title".to_string(),
                inspector_facts: InspectorSelectionPhysicalFacts {
                    observed_tag: "H1".to_string(),
                    element_id: "main:title".to_string(),
                    classes: vec![
                        "hero-title".to_string(),
                        "hero-title".to_string(),
                        "pana-studio-empty-editable".to_string(),
                    ],
                    block_context: Some(InspectorSelectionBlockContext {
                        provider_id: "hero".to_string(),
                        marker_kind: InspectorSelectionBlockMarkerKind::Canonical,
                        root_tag: "section".to_string(),
                    }),
                },
            })
            .unwrap();
        assert_eq!(
            accepted.inspector_summary.state,
            InspectorSelectionSummaryState::Resolved
        );
        assert_eq!(
            accepted.inspector_summary.selector.as_deref(),
            Some("h1#main\\:title")
        );
        assert_eq!(
            accepted.inspector_summary.classes,
            vec!["hero-title".to_string()]
        );
        assert_eq!(accepted.inspector_summary.document_epoch, Some(41));
        assert!(accepted.inspector_summary.reason.is_none());
        assert!(accepted.inspector_summary.diagnostics.is_empty());

        let focused = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SetFocus {
                    focus: SelectionFocus::CssRule {
                        file: "sass/index.scss".to_string(),
                        selector: ".hero-title:hover".to_string(),
                        viewport: None,
                        range: None,
                    },
                    expected_selection_revision: Some(accepted.selection_revision),
                },
            )
            .unwrap();
        assert_eq!(
            focused.inspector_summary.state,
            InspectorSelectionSummaryState::Resolved
        );
        assert_eq!(
            focused.inspector_summary.active_css_class.as_deref(),
            Some("hero-title")
        );
        assert_eq!(
            focused.inspector_summary.classes,
            vec!["hero-title".to_string()]
        );

        let stale_focus = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SetFocus {
                    focus: SelectionFocus::CssRule {
                        file: "sass/index.scss".to_string(),
                        selector: ".other".to_string(),
                        viewport: None,
                        range: None,
                    },
                    expected_selection_revision: Some(accepted.selection_revision),
                },
            )
            .unwrap_err();
        assert!(stale_focus.contains("selecția s-a schimbat"));

        let cleared = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::ClearSelection,
            )
            .unwrap();
        assert_eq!(
            cleared.inspector_summary.state,
            InspectorSelectionSummaryState::Empty
        );
        assert!(cleared.inspector_summary.classes.is_empty());
    }

    #[test]
    fn inspector_summary_rejects_stale_document_and_semantic_tag_mismatch() {
        let runtime = SelectionCoordinatorRuntime::default();
        let snapshot = snapshot(
            "tx-reject",
            vec![node(
                "editor_render:title",
                "source:title",
                "render:title",
                None,
                range(10, 30),
            )],
        );
        runtime
            .bind_inspector_document(inspector_document(&snapshot.identity, 8))
            .unwrap();
        let selected = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor_render:title".to_string(),
                },
            )
            .unwrap();
        let stale_document = runtime
            .accept_observation(SelectionObservationInput {
                schema_version: SELECTION_COORDINATOR_SCHEMA_VERSION,
                selection_revision: selected.selection.selection_revision,
                canvas_identity: snapshot.identity.clone(),
                document_epoch: 7,
                render_instance_id: "render:title".to_string(),
                inspector_facts: inspector_facts("h1", "", &[]),
            })
            .unwrap_err();
        assert!(stale_document.contains("documentului CanvasAgent activ"));

        let wrong_render_instance = runtime
            .accept_observation(SelectionObservationInput {
                schema_version: SELECTION_COORDINATOR_SCHEMA_VERSION,
                selection_revision: selected.selection.selection_revision,
                canvas_identity: snapshot.identity.clone(),
                document_epoch: 8,
                render_instance_id: "render:other".to_string(),
                inspector_facts: inspector_facts("h1", "", &[]),
            })
            .unwrap_err();
        assert!(wrong_render_instance.contains("instanței semantice selectate"));

        let wrong_tag = runtime
            .accept_observation(SelectionObservationInput {
                schema_version: SELECTION_COORDINATOR_SCHEMA_VERSION,
                selection_revision: selected.selection.selection_revision,
                canvas_identity: snapshot.identity.clone(),
                document_epoch: 8,
                render_instance_id: "render:title".to_string(),
                inspector_facts: inspector_facts("p", "", &[]),
            })
            .unwrap_err();
        assert!(wrong_tag.contains("alt tag"));
    }

    #[test]
    fn inspector_summary_distinguishes_tera_runtime_uninspectable_and_rebase_failures() {
        let tera_runtime = SelectionCoordinatorRuntime::default();
        let mut tera_node = node(
            "editor_boundary:content",
            "source:block",
            "render:content",
            None,
            range(10, 30),
        );
        tera_node.kind = EditorNavigationNodeKind::TeraBoundary;
        tera_node.label = "content".to_string();
        tera_node.tag = None;
        let tera_snapshot = snapshot("tx-tera", vec![tera_node]);
        let tera = tera_runtime
            .apply(
                &tera_snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor_boundary:content".to_string(),
                },
            )
            .unwrap();
        assert_eq!(
            tera.inspector_summary.state,
            InspectorSelectionSummaryState::Resolved
        );
        assert_eq!(
            tera.inspector_summary.subject_kind,
            Some(SelectionSubjectKind::TeraBoundary)
        );
        assert_eq!(tera.inspector_summary.selector.as_deref(), Some("content"));

        let runtime = SelectionCoordinatorRuntime::default();
        let mut runtime_node = node(
            "editor_runtime:button",
            "source:script",
            "render:button",
            None,
            range(40, 60),
        );
        runtime_node.kind = EditorNavigationNodeKind::RuntimeElement;
        runtime_node.tag = Some("button".to_string());
        let runtime_snapshot = snapshot("tx-runtime", vec![runtime_node]);
        runtime
            .bind_inspector_document(inspector_document(&runtime_snapshot.identity, 9))
            .unwrap();
        let runtime_selected = runtime
            .apply(
                &runtime_snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor_runtime:button".to_string(),
                },
            )
            .unwrap();
        let runtime_accepted = runtime
            .accept_observation(SelectionObservationInput {
                schema_version: SELECTION_COORDINATOR_SCHEMA_VERSION,
                selection_revision: runtime_selected.selection.selection_revision,
                canvas_identity: runtime_snapshot.identity.clone(),
                document_epoch: 9,
                render_instance_id: "render:button".to_string(),
                inspector_facts: inspector_facts("button", "", &["action"]),
            })
            .unwrap();
        assert_eq!(
            runtime_accepted.inspector_summary.subject_kind,
            Some(SelectionSubjectKind::RuntimeElement)
        );
        assert_eq!(
            runtime_accepted.inspector_summary.state,
            InspectorSelectionSummaryState::Resolved
        );

        let uninspectable_runtime = SelectionCoordinatorRuntime::default();
        let mut uninspectable_node = node(
            "editor_render:locked",
            "source:locked",
            "render:locked",
            None,
            range(70, 90),
        );
        uninspectable_node.capabilities.can_inspect = false;
        let uninspectable_snapshot = snapshot("tx-locked", vec![uninspectable_node]);
        let uninspectable = uninspectable_runtime
            .apply(
                &uninspectable_snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor_render:locked".to_string(),
                },
            )
            .unwrap();
        assert_eq!(
            uninspectable.inspector_summary.state,
            InspectorSelectionSummaryState::Uninspectable
        );

        let rebase_runtime = SelectionCoordinatorRuntime::default();
        let original = snapshot(
            "tx-original",
            vec![node(
                "editor_render:item",
                "source:item",
                "render:item",
                None,
                range(100, 120),
            )],
        );
        rebase_runtime
            .apply(
                &original,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor_render:item".to_string(),
                },
            )
            .unwrap();
        let missing = snapshot("tx-missing", Vec::new());
        let not_rendered = rebase_runtime
            .apply(
                &missing,
                Some("templates/index.html"),
                None,
                SelectionIntent::Rebase,
            )
            .unwrap();
        assert_eq!(
            not_rendered.inspector_summary.state,
            InspectorSelectionSummaryState::NotRendered
        );

        let ambiguous_runtime = SelectionCoordinatorRuntime::default();
        ambiguous_runtime
            .apply(
                &original,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor_render:item".to_string(),
                },
            )
            .unwrap();
        let repeated = snapshot(
            "tx-ambiguous",
            vec![
                node(
                    "editor_render:first",
                    "source:item",
                    "render:first",
                    None,
                    range(100, 120),
                ),
                node(
                    "editor_render:second",
                    "source:item",
                    "render:second",
                    None,
                    range(100, 120),
                ),
            ],
        );
        let ambiguous = ambiguous_runtime
            .apply(
                &repeated,
                Some("templates/index.html"),
                None,
                SelectionIntent::Rebase,
            )
            .unwrap();
        assert_eq!(
            ambiguous.inspector_summary.state,
            InspectorSelectionSummaryState::Ambiguous
        );
    }

    #[test]
    fn queued_mutation_cannot_follow_a_new_selection() {
        let runtime = SelectionCoordinatorRuntime::default();
        let snapshot = snapshot(
            "tx-1",
            vec![
                node("editor_render:a", "source:a", "a", None, range(10, 30)),
                node("editor_render:b", "source:b", "b", None, range(40, 60)),
            ],
        );
        let captured = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor_render:a".to_string(),
                },
            )
            .unwrap()
            .selection;
        let captured_anchor = captured.anchor.as_ref().unwrap();
        runtime
            .require_mutation_target(
                &snapshot.identity.runtime_session_id,
                captured.selection_revision,
                captured_anchor.editor_node_id.as_deref(),
                captured_anchor.source_node_id.as_deref(),
                captured_anchor.render_instance_id.as_deref(),
            )
            .unwrap();

        runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor_render:b".to_string(),
                },
            )
            .unwrap();
        let error = runtime
            .require_mutation_target(
                &snapshot.identity.runtime_session_id,
                captured.selection_revision,
                captured_anchor.editor_node_id.as_deref(),
                captured_anchor.source_node_id.as_deref(),
                captured_anchor.render_instance_id.as_deref(),
            )
            .unwrap_err();
        assert!(error.contains("selecția s-a schimbat"));
    }

    #[test]
    fn css_mutation_can_follow_a_focus_revision_for_the_same_semantic_anchor() {
        let runtime = SelectionCoordinatorRuntime::default();
        let snapshot = snapshot(
            "tx-1",
            vec![node(
                "editor_render:a",
                "source:a",
                "render:a",
                None,
                range(10, 30),
            )],
        );
        let captured = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor_render:a".to_string(),
                },
            )
            .unwrap()
            .selection;
        let captured_anchor = captured.anchor.as_ref().unwrap();

        let focused = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SetFocus {
                    focus: SelectionFocus::CssProperty {
                        file: "sass/index.scss".to_string(),
                        selector: ".hero-title".to_string(),
                        property: "font-size".to_string(),
                        viewport: None,
                        range: None,
                    },
                    expected_selection_revision: Some(captured.selection_revision),
                },
            )
            .unwrap()
            .selection;
        assert!(focused.selection_revision > captured.selection_revision);

        runtime
            .with_stable_semantic_mutation_target(
                &snapshot.identity.runtime_session_id,
                captured.selection_revision,
                captured_anchor.editor_node_id.as_deref(),
                captured_anchor.source_node_id.as_deref(),
                captured_anchor.render_instance_id.as_deref(),
                || Ok(()),
            )
            .unwrap();
    }

    #[test]
    fn css_mutation_cannot_follow_a_different_semantic_anchor() {
        let runtime = SelectionCoordinatorRuntime::default();
        let snapshot = snapshot(
            "tx-1",
            vec![
                node("editor_render:a", "source:a", "a", None, range(10, 30)),
                node("editor_render:b", "source:b", "b", None, range(40, 60)),
            ],
        );
        let captured = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor_render:a".to_string(),
                },
            )
            .unwrap()
            .selection;
        let captured_anchor = captured.anchor.as_ref().unwrap();

        runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor_render:b".to_string(),
                },
            )
            .unwrap();

        let error = runtime
            .with_stable_semantic_mutation_target(
                &snapshot.identity.runtime_session_id,
                captured.selection_revision,
                captured_anchor.editor_node_id.as_deref(),
                captured_anchor.source_node_id.as_deref(),
                captured_anchor.render_instance_id.as_deref(),
                || Ok(()),
            )
            .unwrap_err();
        assert!(error.contains("EditorNavigation"));
    }

    #[test]
    fn hover_is_ephemeral_and_never_replaces_selection() {
        let runtime = SelectionCoordinatorRuntime::default();
        let snapshot = snapshot(
            "tx-1",
            vec![
                node("editor_render:a", "source:a", "a", None, range(10, 30)),
                node("editor_render:b", "source:b", "b", None, range(40, 60)),
            ],
        );
        let selected = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor_render:a".to_string(),
                },
            )
            .unwrap()
            .selection;
        let hovered = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SetHover {
                    editor_node_id: "editor_render:b".to_string(),
                    document_epoch: 11,
                },
            )
            .unwrap();
        assert_eq!(
            hovered.selection.selection_revision,
            selected.selection_revision
        );
        assert_eq!(
            hovered.hover.unwrap().editor_node_id,
            "editor_render:b".to_string()
        );

        let wrong_epoch = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::ClearHover { document_epoch: 12 },
            )
            .unwrap();
        assert!(wrong_epoch.hover.is_some());
        let cleared = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::ClearHover { document_epoch: 11 },
            )
            .unwrap();
        assert!(cleared.hover.is_none());
        assert_eq!(
            cleared.selection.anchor.unwrap().editor_node_id.as_deref(),
            Some("editor_render:a")
        );
    }

    #[test]
    fn hover_fast_path_returns_a_minimal_deduplicated_projection() {
        let runtime = SelectionCoordinatorRuntime::default();
        let snapshot = snapshot(
            "tx-1",
            vec![
                node("editor_render:a", "source:a", "a", None, range(10, 30)),
                node("editor_render:b", "source:b", "b", None, range(40, 60)),
            ],
        );
        let selection_revision = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor_render:a".to_string(),
                },
            )
            .unwrap()
            .selection
            .selection_revision;

        let (first, first_changed) = runtime
            .apply_hover(
                &snapshot,
                Some("templates/index.html"),
                Some("editor_render:b"),
                11,
            )
            .unwrap();
        assert!(first_changed);
        assert_eq!(
            first.as_ref().map(|hover| hover.editor_node_id.as_str()),
            Some("editor_render:b")
        );

        let (duplicate, duplicate_changed) = runtime
            .apply_hover(
                &snapshot,
                Some("templates/index.html"),
                Some("editor_render:b"),
                11,
            )
            .unwrap();
        assert!(!duplicate_changed);
        assert_eq!(
            duplicate.as_ref().map(|hover| (
                hover.hover_revision,
                hover.document_epoch,
                hover.editor_node_id.as_str(),
            )),
            first.as_ref().map(|hover| (
                hover.hover_revision,
                hover.document_epoch,
                hover.editor_node_id.as_str(),
            ))
        );

        let (cleared, clear_changed) = runtime
            .apply_hover(&snapshot, Some("templates/index.html"), None, 11)
            .unwrap();
        assert!(clear_changed);
        assert!(cleared.is_none());
        let (still_clear, duplicate_clear_changed) = runtime
            .apply_hover(&snapshot, Some("templates/index.html"), None, 11)
            .unwrap();
        assert!(!duplicate_clear_changed);
        assert!(still_clear.is_none());

        let state = runtime.state.lock().unwrap();
        let after_hover = state.selection.as_ref().unwrap();
        assert_eq!(after_hover.selection_revision, selection_revision);
        assert_eq!(
            after_hover
                .anchor
                .as_ref()
                .and_then(|anchor| anchor.editor_node_id.as_deref()),
            Some("editor_render:a")
        );
    }

    #[test]
    fn code_position_selects_a_unique_node_and_refuses_repeated_instances() {
        let runtime = SelectionCoordinatorRuntime::default();
        let unique = snapshot(
            "tx-1",
            vec![node(
                "editor_render:title",
                "source:title",
                "title",
                None,
                range(10, 30),
            )],
        );
        let selected = runtime
            .apply(
                &unique,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectSourcePosition {
                    file: "templates/index.html".to_string(),
                    offset: 15,
                    viewport: None,
                },
            )
            .unwrap();
        assert_eq!(selected.selection.resolution, SelectionResolution::Resolved);
        assert_eq!(
            selected.selection.anchor.unwrap().editor_node_id.as_deref(),
            Some("editor_render:title")
        );

        let repeated_runtime = SelectionCoordinatorRuntime::default();
        let repeated = snapshot(
            "tx-2",
            vec![
                node(
                    "editor_render:first",
                    "source:item",
                    "first",
                    None,
                    range(40, 60),
                ),
                node(
                    "editor_render:second",
                    "source:item",
                    "second",
                    None,
                    range(40, 60),
                ),
            ],
        );
        let ambiguous = repeated_runtime
            .apply(
                &repeated,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectSourcePosition {
                    file: "templates/index.html".to_string(),
                    offset: 45,
                    viewport: None,
                },
            )
            .unwrap();
        assert_eq!(
            ambiguous.selection.resolution,
            SelectionResolution::Ambiguous
        );
        assert!(ambiguous
            .selection
            .projections
            .preview
            .render_instance_ids
            .is_empty());
    }

    #[test]
    fn a_new_project_session_clears_the_previous_subject() {
        let runtime = SelectionCoordinatorRuntime::default();
        let first = snapshot(
            "tx-1",
            vec![node(
                "editor_render:a",
                "source:a",
                "a",
                None,
                range(10, 30),
            )],
        );
        runtime
            .apply(
                &first,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor_render:a".to_string(),
                },
            )
            .unwrap();

        let mut next = first.clone();
        next.identity.runtime_session_id = "session:runtime-2".to_string();
        next.identity.transaction_id = "tx-2".to_string();
        let receipt = runtime
            .apply(
                &next,
                Some("templates/index.html"),
                None,
                SelectionIntent::Rebase,
            )
            .unwrap();
        assert_eq!(receipt.selection.resolution, SelectionResolution::Cleared);
        assert!(receipt.selection.subject.is_none());
        assert_eq!(
            receipt.selection.runtime_session_id,
            "session:runtime-2".to_string()
        );
    }
}
