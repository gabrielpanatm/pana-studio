use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};

use crate::{
    kernel::canvas_interaction::CanvasInteractionIdentity,
    kernel::editor_navigation::{
        EditorNavigationBoundaryKind, EditorNavigationCapabilities, EditorNavigationComponentKind,
        EditorNavigationNode, EditorNavigationNodeKind, EditorNavigationOrigin,
        EditorNavigationSnapshot, EditorSourceProvenance, EditorSourceReference,
        EditorSourceResolution,
    },
    preview::CanvasProjectionIdentity,
    source_graph::model::{SourceGraph, SourceNode, SourceNodeKind, SourceOrigin, SourceRange},
};

pub const SELECTION_COORDINATOR_SCHEMA_VERSION: u32 = 3;
pub const MAX_SELECTION_MEMBERS: usize = 256;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SelectionSubjectKind {
    HtmlElement,
    Boundary,
    RuntimeElement,
    CssRule,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionSubject {
    pub kind: SelectionSubjectKind,
    pub boundary_kind: Option<EditorNavigationBoundaryKind>,
    pub component_kind: Option<EditorNavigationComponentKind>,
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
pub struct SelectionEntry {
    pub member_id: String,
    pub resolution: SelectionResolution,
    pub subject: SelectionSubject,
    pub anchor: SelectionAnchor,
    pub provenance: EditorSourceProvenance,
    pub capabilities: EditorNavigationCapabilities,
    pub diagnostics: Vec<String>,
    #[serde(skip)]
    pub(crate) source_html_attributes: Option<BTreeMap<String, Option<String>>>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionAggregateHtmlFacts {
    pub complete: bool,
    pub common_classes: Vec<String>,
    pub mixed_classes: Vec<String>,
    pub common_attributes: BTreeMap<String, Option<String>>,
    pub mixed_attribute_names: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionAggregateCapabilities {
    pub member_count: usize,
    pub all_resolved: bool,
    pub all_source_backed: bool,
    pub same_file: bool,
    pub same_parent: bool,
    pub has_ancestor_descendant: bool,
    pub has_duplicate_source_targets: bool,
    pub can_batch_attributes: bool,
    pub can_batch_duplicate: bool,
    pub can_batch_delete: bool,
    pub can_batch_move: bool,
    pub primary_only_edits_allowed: bool,
    pub primary_only_reason_code: Option<String>,
    pub reasons: Vec<String>,
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
    pub primary_member_id: Option<String>,
    pub range_origin_member_id: Option<String>,
    pub members: Vec<SelectionEntry>,
    pub aggregate_capabilities: SelectionAggregateCapabilities,
    pub aggregate_html_facts: SelectionAggregateHtmlFacts,
    pub focus: SelectionFocus,
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
    pub boundary_kind: Option<EditorNavigationBoundaryKind>,
    pub component_kind: Option<EditorNavigationComponentKind>,
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
    ToggleEditorNode {
        editor_node_id: String,
    },
    ExtendRangeToEditorNode {
        editor_node_id: String,
    },
    SetPrimaryEditorNode {
        editor_node_id: String,
    },
    SelectSourcePosition {
        file: String,
        offset: usize,
        #[serde(default)]
        viewport: Option<String>,
    },
    #[serde(skip_deserializing)]
    SelectCssSourceRule {
        file: String,
        selector: String,
        #[serde(default)]
        viewport: Option<String>,
        #[serde(default, skip_deserializing)]
        range: Option<SourceRange>,
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectorSelectionBlockContext {
    pub provider_id: String,
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
    pub boundary_kind: Option<EditorNavigationBoundaryKind>,
    pub component_kind: Option<EditorNavigationComponentKind>,
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
    pub workspace_revision: u64,
    #[serde(default)]
    pub primary_member_id: Option<String>,
    pub members: Vec<SelectionMutationMemberIdentity>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SelectionMutationMemberIdentity {
    pub member_id: String,
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
    /// Returns the bounded, ordered opaque selection set owned by Rust for
    /// publication surfaces such as Code, status and AI context. Callers must
    /// provide the active ProjectSession; a foreign session is rejected rather
    /// than silently publishing stale member identities.
    pub fn current_opaque_selection(
        &self,
        runtime_session_id: Option<&str>,
    ) -> Result<(Option<String>, Vec<String>), String> {
        let Some(runtime_session_id) = runtime_session_id else {
            return Ok((None, Vec::new()));
        };
        let state = self
            .state
            .lock()
            .map_err(|_| "SelectionCoordinator este indisponibil.".to_string())?;
        let Some(selection) = state.selection.as_ref() else {
            return Ok((None, Vec::new()));
        };
        if selection.runtime_session_id != runtime_session_id {
            return Err(
                "Selecția opacă aparține altei sesiuni de proiect și nu poate fi publicată."
                    .to_string(),
            );
        }
        let member_ids = selection
            .members
            .iter()
            .map(|member| member.member_id.clone())
            .collect::<Vec<_>>();
        if selection
            .primary_member_id
            .as_ref()
            .is_some_and(|primary| !member_ids.contains(primary))
        {
            return Err(
                "SelectionCoordinator deține un primary care nu aparține setului opac.".to_string(),
            );
        }
        Ok((selection.primary_member_id.clone(), member_ids))
    }

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
        expected: &SelectionMutationIdentity,
    ) -> Result<(), String> {
        self.with_mutation_target(runtime_session_id, expected, || Ok(()))
    }

    pub fn with_mutation_target<R>(
        &self,
        runtime_session_id: &str,
        expected: &SelectionMutationIdentity,
        execute: impl FnOnce() -> Result<R, String>,
    ) -> Result<R, String> {
        self.with_resolved_target(
            "Mutația",
            runtime_session_id,
            expected,
            SelectionRevisionPolicy::Exact,
            execute,
        )
    }

    /// Clears a selection only when it is still the exact target that guarded
    /// a committed destructive mutation. A newer user selection wins and is
    /// never cleared by an older delete receipt.
    pub fn clear_mutation_target_if_current(
        &self,
        runtime_session_id: &str,
        expected: &SelectionMutationIdentity,
    ) -> Result<bool, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "SelectionCoordinator este indisponibil.".to_string())?;
        let Some(current) = state.selection.as_ref() else {
            return Ok(false);
        };
        if current.runtime_session_id != runtime_session_id
            || current.selection_revision != expected.selection_revision
            || aggregate_selection_resolution(&current.members) != SelectionResolution::Resolved
            || !mutation_identity_matches(current, expected)
        {
            return Ok(false);
        }

        let current = current.clone();
        let revision = next_selection_revision(&mut state)?;
        state.selection = Some(SelectionSnapshot {
            schema_version: SELECTION_COORDINATOR_SCHEMA_VERSION,
            selection_revision: revision,
            project_root: current.project_root,
            runtime_session_id: current.runtime_session_id,
            canvas_identity: current.canvas_identity,
            route: current.route,
            active_document_path: current.active_document_path,
            primary_member_id: None,
            range_origin_member_id: None,
            members: Vec::new(),
            aggregate_capabilities: SelectionAggregateCapabilities::default(),
            aggregate_html_facts: SelectionAggregateHtmlFacts::default(),
            focus: SelectionFocus::Element,
            diagnostics: Vec::new(),
        });
        state.hover = None;
        state.inspector_facts = None;
        Ok(true)
    }

    /// Atomically replaces the exact selection that guarded a committed
    /// duplicate with the SourceNodeIds allocated by the authoritative
    /// after-model. The source-only entries are intentionally re-resolved by
    /// the next canonical navigation snapshot; no DOM or similarity fallback
    /// participates in the handoff.
    pub fn replace_mutation_target_with_sources_if_current(
        &self,
        runtime_session_id: &str,
        expected: &SelectionMutationIdentity,
        source_graph: &SourceGraph,
        source_node_ids: &[String],
        primary_source_node_id: Option<&str>,
    ) -> Result<bool, String> {
        if source_node_ids.is_empty() || source_node_ids.len() > MAX_SELECTION_MEMBERS {
            return Err(
                "Înlocuirea selecției cere între 1 și 256 de SourceNodeId-uri.".to_string(),
            );
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| "SelectionCoordinator este indisponibil.".to_string())?;
        let Some(current) = state.selection.as_ref() else {
            return Ok(false);
        };
        if current.runtime_session_id != runtime_session_id
            || current.selection_revision != expected.selection_revision
            || aggregate_selection_resolution(&current.members) != SelectionResolution::Resolved
            || !mutation_identity_matches(current, expected)
        {
            return Ok(false);
        }

        let mut seen = HashSet::with_capacity(source_node_ids.len());
        let mut entries = Vec::with_capacity(source_node_ids.len());
        for source_node_id in source_node_ids {
            if !seen.insert(source_node_id.as_str()) {
                return Err("Înlocuirea selecției a primit SourceNodeId-uri duplicate.".to_string());
            }
            let source = source_graph.node_by_id(source_node_id).ok_or_else(|| {
                format!(
                    "Înlocuirea selecției nu găsește SourceNodeId {source_node_id} în after-model."
                )
            })?;
            entries.push(selection_entry_from_source_node(
                source,
                SelectionResolution::NotRendered,
                "Copia este confirmată în sursă și așteaptă proiecția canonică.",
            ));
        }
        let primary_member_id = primary_source_node_id
            .and_then(|source_id| {
                entries
                    .iter()
                    .find(|entry| entry.anchor.source_node_id.as_deref() == Some(source_id))
            })
            .or_else(|| entries.first())
            .map(|entry| entry.member_id.clone());
        let previous_capabilities = current.aggregate_capabilities.clone();
        let mut replacement = current.clone();
        let revision = next_selection_revision(&mut state)?;
        replacement.selection_revision = revision;
        replacement.primary_member_id = primary_member_id.clone();
        replacement.range_origin_member_id = primary_member_id;
        replacement.aggregate_capabilities =
            conservative_filtered_capabilities(&previous_capabilities, &entries);
        replacement.aggregate_html_facts = aggregate_selection_html_facts(&entries);
        replacement.members = entries;
        replacement.focus = SelectionFocus::Element;
        replacement.diagnostics =
            vec!["Selecția a fost mutată atomic pe copiile confirmate de after-model.".to_string()];
        state.selection = Some(replacement);
        state.hover = None;
        state.inspector_facts = None;
        Ok(true)
    }

    /// Invalidates a semantic selection after a Code transaction only when
    /// its opaque SourceNodeId no longer exists in the committed after-model.
    /// No range, selector, label or sibling is allowed to replace it.
    pub fn invalidate_missing_source_target(
        &self,
        runtime_session_id: &str,
        retained_source_node_ids: &HashSet<String>,
    ) -> Result<bool, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "SelectionCoordinator este indisponibil.".to_string())?;
        let Some(current) = state.selection.as_ref() else {
            return Ok(false);
        };
        if current.runtime_session_id != runtime_session_id {
            return Ok(false);
        }

        let members = current
            .members
            .iter()
            .filter(|member| {
                member
                    .anchor
                    .source_node_id
                    .as_ref()
                    .is_none_or(|source_node_id| retained_source_node_ids.contains(source_node_id))
            })
            .cloned()
            .collect::<Vec<_>>();
        if members.len() == current.members.len() {
            return Ok(false);
        }

        let current = current.clone();
        let revision = next_selection_revision(&mut state)?;
        state.selection = Some(selection_after_external_member_filter(
            revision,
            current,
            members,
            "Selecția a fost actualizată deoarece unul sau mai multe SourceNodeId nu mai există după editarea Cod.",
        ));
        state.hover = None;
        state.inspector_facts = None;
        Ok(true)
    }

    pub fn with_stable_semantic_mutation_target<R>(
        &self,
        runtime_session_id: &str,
        expected: &SelectionMutationIdentity,
        execute: impl FnOnce() -> Result<R, String>,
    ) -> Result<R, String> {
        self.with_resolved_target(
            "Mutația CSS",
            runtime_session_id,
            expected,
            SelectionRevisionPolicy::StableSemanticAnchor,
            execute,
        )
    }

    pub fn with_selection_target<R>(
        &self,
        runtime_session_id: &str,
        expected: &SelectionMutationIdentity,
        execute: impl FnOnce() -> Result<R, String>,
    ) -> Result<R, String> {
        self.with_resolved_target(
            "Operația dependentă de selecție",
            runtime_session_id,
            expected,
            SelectionRevisionPolicy::Exact,
            execute,
        )
    }

    fn with_resolved_target<R>(
        &self,
        operation: &str,
        runtime_session_id: &str,
        expected: &SelectionMutationIdentity,
        revision_policy: SelectionRevisionPolicy,
        execute: impl FnOnce() -> Result<R, String>,
    ) -> Result<R, String> {
        if expected.selection_revision == 0 || expected.members.is_empty() {
            return Err(format!(
                "SelectionCoordinator a refuzat {operation}: amprenta selecției este incompletă."
            ));
        }
        {
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
            if selection.canvas_identity.workspace_revision != expected.workspace_revision {
                return Err(format!(
                    "{operation} a fost anulată deoarece ProjectWorkspace s-a schimbat (revizia capturată {}, revizia selecției {}).",
                    expected.workspace_revision,
                    selection.canvas_identity.workspace_revision
                ));
            }
            let requires_exact_revision = matches!(revision_policy, SelectionRevisionPolicy::Exact);
            if selection.selection_revision < expected.selection_revision
                || (requires_exact_revision
                    && selection.selection_revision != expected.selection_revision)
            {
                return Err(format!(
                    "{operation} a fost anulată deoarece selecția s-a schimbat (revizia capturată {}, revizia activă {}).",
                    expected.selection_revision,
                    selection.selection_revision
                ));
            }
            if aggregate_selection_resolution(&selection.members) != SelectionResolution::Resolved {
                return Err(format!(
                    "{operation} a fost anulată deoarece selecția nu mai are o rezoluție unică."
                ));
            }
            if !mutation_identity_matches(selection, expected) {
                return Err(format!(
                    "{operation} a fost anulată deoarece setul de ancore al selecției s-a schimbat."
                ));
            }
        }
        // Nu ținem mutexul SelectionCoordinator peste parse, I/O sau commit.
        // ProjectWorkspace face al doilea CAS folosind revizia capturată.
        execute()
    }

    pub fn apply(
        &self,
        snapshot: &EditorNavigationSnapshot,
        active_document_path: Option<&str>,
        source_graph: Option<&SourceGraph>,
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
            SelectionIntent::ToggleEditorNode { editor_node_id } => {
                toggle_editor_node(&mut state, snapshot, active_document_path, &editor_node_id)?;
            }
            SelectionIntent::ExtendRangeToEditorNode { editor_node_id } => {
                extend_range_to_editor_node(
                    &mut state,
                    snapshot,
                    active_document_path,
                    &editor_node_id,
                )?;
            }
            SelectionIntent::SetPrimaryEditorNode { editor_node_id } => {
                set_primary_editor_node(
                    &mut state,
                    snapshot,
                    active_document_path,
                    &editor_node_id,
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
            SelectionIntent::SelectCssSourceRule {
                file,
                selector,
                viewport,
                range,
            } => {
                select_css_source_rule(
                    &mut state,
                    snapshot,
                    active_document_path,
                    source_graph,
                    SelectionFocus::CssRule {
                        file,
                        selector,
                        viewport,
                        range,
                    },
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
                    source_graph,
                    focus,
                    expected_selection_revision,
                )?;
            }
            SelectionIntent::ClearSelection => {
                clear_selection(&mut state, snapshot, active_document_path)?;
            }
            SelectionIntent::Rebase => {
                rebase_selection(&mut state, snapshot, active_document_path, source_graph)?;
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
        if primary_selection_entry(selection)
            .and_then(|entry| entry.anchor.render_instance_id.as_deref())
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
        if !primary_selection_entry(selection).is_some_and(|entry| entry.capabilities.can_inspect) {
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

fn mutation_members(selection: &SelectionSnapshot) -> Vec<SelectionMutationMemberIdentity> {
    selection
        .members
        .iter()
        .map(|member| SelectionMutationMemberIdentity {
            member_id: member.member_id.clone(),
            editor_node_id: member.anchor.editor_node_id.clone(),
            source_node_id: member.anchor.source_node_id.clone(),
            render_instance_id: member.anchor.render_instance_id.clone(),
        })
        .collect()
}

fn mutation_identity_matches(
    selection: &SelectionSnapshot,
    expected: &SelectionMutationIdentity,
) -> bool {
    expected.primary_member_id == selection.primary_member_id
        && expected.members == mutation_members(selection)
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

fn require_selectable_node<'a>(
    snapshot: &'a EditorNavigationSnapshot,
    editor_node_id: &str,
) -> Result<&'a EditorNavigationNode, String> {
    let node = snapshot
        .nodes
        .iter()
        .find(|node| node.id == editor_node_id)
        .ok_or_else(|| {
            "SelectionCoordinator nu găsește nodul EditorNavigation solicitat.".to_string()
        })?;
    if !node.capabilities.can_select {
        return Err("SelectionCoordinator a refuzat un nod care nu poate fi selectat.".to_string());
    }
    Ok(node)
}

fn toggle_editor_node(
    state: &mut SelectionCoordinatorState,
    snapshot: &EditorNavigationSnapshot,
    active_document_path: Option<&str>,
    editor_node_id: &str,
) -> Result<(), String> {
    let node = require_selectable_node(snapshot, editor_node_id)?;
    let current = state.selection.clone().ok_or_else(|| {
        "SelectionCoordinator nu are o stare de selecție inițializată.".to_string()
    })?;
    let mut members = current.members;
    let existing = members
        .iter()
        .position(|member| member.anchor.editor_node_id.as_deref() == Some(editor_node_id));
    let (primary_member_id, range_origin_member_id) = if let Some(index) = existing {
        let removed = members.remove(index);
        let primary = if current.primary_member_id.as_deref() == Some(removed.member_id.as_str()) {
            members.last().map(|member| member.member_id.clone())
        } else {
            current.primary_member_id
        };
        let range_origin =
            if current.range_origin_member_id.as_deref() == Some(removed.member_id.as_str()) {
                primary.clone()
            } else {
                current.range_origin_member_id
            };
        (primary, range_origin)
    } else {
        if members.len() >= MAX_SELECTION_MEMBERS {
            return Err(format!(
                "Selecția multiplă este limitată la {MAX_SELECTION_MEMBERS} de elemente."
            ));
        }
        let entry = selection_entry_from_node(node, SelectionResolution::Resolved, Vec::new());
        let member_id = entry.member_id.clone();
        members.push(entry);
        (Some(member_id.clone()), Some(member_id))
    };
    let revision = next_selection_revision(state)?;
    state.selection = Some(selection_from_members(
        revision,
        snapshot,
        active_document_path,
        members,
        primary_member_id,
        range_origin_member_id,
        SelectionFocus::Element,
        Vec::new(),
    ));
    state.inspector_facts = None;
    Ok(())
}

fn extend_range_to_editor_node(
    state: &mut SelectionCoordinatorState,
    snapshot: &EditorNavigationSnapshot,
    active_document_path: Option<&str>,
    editor_node_id: &str,
) -> Result<(), String> {
    let target = require_selectable_node(snapshot, editor_node_id)?;
    let current = state.selection.as_ref().ok_or_else(|| {
        "SelectionCoordinator nu are o stare de selecție inițializată.".to_string()
    })?;
    let origin_editor_node_id = current
        .range_origin_member_id
        .as_deref()
        .or(current.primary_member_id.as_deref())
        .and_then(|member_id| {
            current
                .members
                .iter()
                .find(|member| member.member_id == member_id)
        })
        .and_then(|member| member.anchor.editor_node_id.as_deref())
        .unwrap_or(target.id.as_str());

    let mut selectable = snapshot
        .nodes
        .iter()
        .filter(|node| node.capabilities.can_select)
        .collect::<Vec<_>>();
    selectable.sort_by(|left, right| {
        left.order
            .cmp(&right.order)
            .then_with(|| left.id.cmp(&right.id))
    });
    let origin_index = selectable
        .iter()
        .position(|node| node.id == origin_editor_node_id)
        .ok_or_else(|| "Originea intervalului nu mai există în EditorNavigation.".to_string())?;
    let target_index = selectable
        .iter()
        .position(|node| node.id == target.id)
        .ok_or_else(|| "Ținta intervalului nu mai există în EditorNavigation.".to_string())?;
    let start = origin_index.min(target_index);
    let end = origin_index.max(target_index);
    let count = end - start + 1;
    if count > MAX_SELECTION_MEMBERS {
        return Err(format!(
            "Intervalul depășește limita de {MAX_SELECTION_MEMBERS} de elemente selectate."
        ));
    }
    let members = selectable[start..=end]
        .iter()
        .map(|node| selection_entry_from_node(node, SelectionResolution::Resolved, Vec::new()))
        .collect::<Vec<_>>();
    let primary_member_id = target.id.clone();
    let origin_member_id = selectable[origin_index].id.clone();
    let revision = next_selection_revision(state)?;
    state.selection = Some(selection_from_members(
        revision,
        snapshot,
        active_document_path,
        members,
        Some(primary_member_id),
        Some(origin_member_id),
        SelectionFocus::Element,
        Vec::new(),
    ));
    state.inspector_facts = None;
    Ok(())
}

fn set_primary_editor_node(
    state: &mut SelectionCoordinatorState,
    snapshot: &EditorNavigationSnapshot,
    active_document_path: Option<&str>,
    editor_node_id: &str,
) -> Result<(), String> {
    let current = state.selection.clone().ok_or_else(|| {
        "SelectionCoordinator nu are o stare de selecție inițializată.".to_string()
    })?;
    let member_id = current
        .members
        .iter()
        .find(|member| member.anchor.editor_node_id.as_deref() == Some(editor_node_id))
        .map(|member| member.member_id.clone())
        .ok_or_else(|| "Elementul primar trebuie să aparțină selecției active.".to_string())?;
    if current.primary_member_id.as_deref() == Some(member_id.as_str()) {
        return Ok(());
    }
    let revision = next_selection_revision(state)?;
    state.selection = Some(selection_from_members(
        revision,
        snapshot,
        active_document_path,
        current.members,
        Some(member_id),
        current.range_origin_member_id,
        SelectionFocus::Element,
        current.diagnostics,
    ));
    state.inspector_facts = None;
    Ok(())
}

fn set_focus(
    state: &mut SelectionCoordinatorState,
    snapshot: &EditorNavigationSnapshot,
    active_document_path: Option<&str>,
    source_graph: Option<&SourceGraph>,
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
    let current_is_css_source = state
        .selection
        .as_ref()
        .and_then(primary_selection_entry)
        .is_some_and(|entry| entry.subject.kind == SelectionSubjectKind::CssRule);
    let focus_is_css = matches!(
        &focus,
        SelectionFocus::CssRule { .. } | SelectionFocus::CssProperty { .. }
    );

    if focus_is_css && current_is_css_source {
        return select_css_source_rule(state, snapshot, active_document_path, source_graph, focus);
    }

    if matches!(&focus, SelectionFocus::Element) && current_is_css_source {
        let revision = next_selection_revision(state)?;
        state.selection = Some(empty_selection(
            revision,
            snapshot,
            active_document_path,
            SelectionFocus::Element,
        ));
        state.inspector_facts = None;
        return Ok(());
    }

    let has_no_primary_subject = state
        .selection
        .as_ref()
        .is_none_or(|selection| primary_selection_entry(selection).is_none());
    if focus_is_css && has_no_primary_subject {
        return Err(
            "Focusul CSS din Inspector necesită o regulă selectată în Code sau un element selectat în Preview."
                .to_string(),
        );
    }
    if !matches!(&focus, SelectionFocus::Element)
        && (current_is_css_source || has_no_primary_subject)
    {
        return Err("Focusul JS necesită mai întâi un element semantic selectat.".to_string());
    }
    let revision = next_selection_revision(state)?;
    let current = state.selection.clone().unwrap_or_else(|| {
        empty_selection(revision, snapshot, active_document_path, focus.clone())
    });
    let retained_inspector_facts = state.inspector_facts.take().filter(|facts| {
        facts.selection_revision == current.selection_revision
            && facts.canvas_identity == snapshot.identity
            && primary_selection_entry(&current)
                .and_then(|entry| entry.anchor.render_instance_id.as_deref())
                == Some(facts.render_instance_id.as_str())
    });
    state.selection = Some(SelectionSnapshot {
        schema_version: SELECTION_COORDINATOR_SCHEMA_VERSION,
        selection_revision: revision,
        project_root: snapshot.identity.project_root.clone(),
        runtime_session_id: snapshot.identity.runtime_session_id.clone(),
        canvas_identity: snapshot.identity.clone(),
        route: snapshot.route.clone(),
        active_document_path: active_document_path.map(str::to_string),
        focus,
        ..current
    });
    state.inspector_facts = retained_inspector_facts.map(|mut facts| {
        facts.selection_revision = revision;
        facts
    });
    Ok(())
}

fn select_css_source_rule(
    state: &mut SelectionCoordinatorState,
    snapshot: &EditorNavigationSnapshot,
    active_document_path: Option<&str>,
    source_graph: Option<&SourceGraph>,
    focus: SelectionFocus,
) -> Result<(), String> {
    let source_graph = source_graph.ok_or_else(|| {
        "Selecția CSS sursă necesită Source Graph-ul ProjectModel curent.".to_string()
    })?;
    let entry = selection_entry_from_css_focus(source_graph, &focus)?;
    let member_id = entry.member_id.clone();
    let revision = next_selection_revision(state)?;
    state.selection = Some(selection_from_members(
        revision,
        snapshot,
        active_document_path,
        vec![entry],
        Some(member_id.clone()),
        Some(member_id),
        focus,
        Vec::new(),
    ));
    state.inspector_facts = None;
    state.hover = None;
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
) -> Result<(), String> {
    let Some(current) = state.selection.clone() else {
        return ensure_session(state, snapshot, active_document_path);
    };
    if primary_selection_entry(&current)
        .is_some_and(|entry| entry.subject.kind == SelectionSubjectKind::CssRule)
    {
        let source_graph = source_graph.ok_or_else(|| {
            "Rebazarea selecției CSS sursă necesită Source Graph-ul curent.".to_string()
        })?;
        let entry = selection_entry_from_css_focus(source_graph, &current.focus)?;
        let member_id = entry.member_id.clone();
        let revision = next_selection_revision(state)?;
        state.selection = Some(selection_from_members(
            revision,
            snapshot,
            active_document_path,
            vec![entry],
            Some(member_id.clone()),
            Some(member_id),
            current.focus,
            Vec::new(),
        ));
        state.inspector_facts = None;
        state.hover = None;
        return Ok(());
    }
    if current.members.is_empty() {
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
    }

    let mut members = Vec::with_capacity(current.members.len());
    let mut primary_member_id = None;
    let mut range_origin_member_id = None;
    let mut diagnostics = Vec::new();
    for member in &current.members {
        let (rebased, diagnostic) = match rebase_candidate(snapshot, &member.anchor) {
            RebaseCandidate::Resolved(node) => (
                selection_entry_from_node(node, SelectionResolution::Resolved, Vec::new()),
                None,
            ),
            RebaseCandidate::NotRendered => {
                let message = format!(
                    "Membrul {} nu este randat în Canvas-ul curent.",
                    member.member_id
                );
                let mut unresolved = member.clone();
                unresolved.resolution = SelectionResolution::NotRendered;
                unresolved.diagnostics = vec![message.clone()];
                (unresolved, Some(message))
            }
            RebaseCandidate::Ambiguous => {
                let message = format!(
                    "Membrul {} are mai multe instanțe exacte posibile în Canvas-ul curent.",
                    member.member_id
                );
                let mut unresolved = member.clone();
                unresolved.resolution = SelectionResolution::Ambiguous;
                unresolved.diagnostics = vec![message.clone()];
                (unresolved, Some(message))
            }
        };
        if current.primary_member_id.as_deref() == Some(member.member_id.as_str()) {
            primary_member_id = Some(rebased.member_id.clone());
        }
        if current.range_origin_member_id.as_deref() == Some(member.member_id.as_str()) {
            range_origin_member_id = Some(rebased.member_id.clone());
        }
        if let Some(diagnostic) = diagnostic {
            diagnostics.push(diagnostic);
        }
        members.push(rebased);
    }
    let revision = next_selection_revision(state)?;
    state.selection = Some(selection_from_members(
        revision,
        snapshot,
        active_document_path,
        members,
        primary_member_id,
        range_origin_member_id,
        current.focus,
        diagnostics,
    ));
    state.inspector_facts = None;
    state.hover = None;
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
        .and_then(primary_selection_entry)
        .and_then(|entry| entry.anchor.editor_node_id.as_deref())
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
    let selection = selection_from_node(
        revision,
        snapshot,
        active_document_path,
        representative,
        SelectionFocus::Element,
        SelectionResolution::Ambiguous,
        vec!["Poziția din cod corespunde mai multor instanțe randate.".to_string()],
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
            .and_then(primary_selection_entry)
            .and_then(|entry| entry.anchor.editor_node_id.as_deref())
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
        boundary_kind: node.boundary.as_ref().map(|boundary| boundary.kind),
        component_kind: node
            .boundary
            .as_ref()
            .and_then(|boundary| boundary.component_kind),
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
) -> RebaseCandidate<'a> {
    if let Some(editor_node_id) = anchor.editor_node_id.as_deref() {
        let matches = snapshot
            .nodes
            .iter()
            .filter(|node| node.id == editor_node_id)
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [node] => return RebaseCandidate::Resolved(node),
            [] => {}
            _ => return RebaseCandidate::Ambiguous,
        }
    }
    if let Some(render_instance_id) = anchor.render_instance_id.as_deref() {
        let matches = snapshot
            .nodes
            .iter()
            .filter(|node| node.render_instance_id.as_deref() == Some(render_instance_id))
            .collect::<Vec<_>>();
        return match matches.as_slice() {
            [node] => RebaseCandidate::Resolved(node),
            [] => RebaseCandidate::NotRendered,
            _ => RebaseCandidate::Ambiguous,
        };
    }

    for boundary_render_id in &anchor.render_instance_ids {
        let matches = snapshot
            .nodes
            .iter()
            .filter(|node| {
                node.boundary.as_ref().is_some_and(|boundary| {
                    boundary
                        .root_render_instance_ids
                        .iter()
                        .any(|candidate| candidate == boundary_render_id)
                })
            })
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [node] => return RebaseCandidate::Resolved(node),
            [] => {}
            _ => return RebaseCandidate::Ambiguous,
        }
    }

    if let Some(boundary_instance_id) = anchor.boundary_instance_id.as_deref() {
        let matches = snapshot
            .nodes
            .iter()
            .filter(|node| {
                node.boundary
                    .as_ref()
                    .is_some_and(|boundary| boundary.boundary_instance_id == boundary_instance_id)
            })
            .collect::<Vec<_>>();
        return match matches.as_slice() {
            [node] => RebaseCandidate::Resolved(node),
            [] => RebaseCandidate::NotRendered,
            _ => RebaseCandidate::Ambiguous,
        };
    }

    if anchor.editor_node_id.is_some() || !anchor.render_instance_ids.is_empty() {
        return RebaseCandidate::NotRendered;
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

fn selection_from_node(
    revision: u64,
    snapshot: &EditorNavigationSnapshot,
    active_document_path: Option<&str>,
    node: &EditorNavigationNode,
    focus: SelectionFocus,
    resolution: SelectionResolution,
    diagnostics: Vec<String>,
) -> SelectionSnapshot {
    let entry = selection_entry_from_node(node, resolution, diagnostics.clone());
    let member_id = entry.member_id.clone();
    selection_from_members(
        revision,
        snapshot,
        active_document_path,
        vec![entry],
        Some(member_id.clone()),
        Some(member_id),
        focus,
        diagnostics,
    )
}

fn selection_entry_from_node(
    node: &EditorNavigationNode,
    resolution: SelectionResolution,
    diagnostics: Vec<String>,
) -> SelectionEntry {
    let subject = SelectionSubject {
        kind: subject_kind(node.kind),
        boundary_kind: node.boundary.as_ref().map(|boundary| boundary.kind),
        component_kind: node
            .boundary
            .as_ref()
            .and_then(|boundary| boundary.component_kind),
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
    SelectionEntry {
        member_id: selection_member_id(&anchor),
        resolution,
        subject,
        anchor,
        provenance: node.source_provenance.clone(),
        capabilities: node.capabilities.clone(),
        diagnostics,
        source_html_attributes: node.source_html_attributes.clone(),
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
    let entry = selection_entry_from_source_node(source, resolution, diagnostic);
    let member_id = entry.member_id.clone();
    selection_from_members(
        revision,
        snapshot,
        active_document_path,
        vec![entry],
        Some(member_id.clone()),
        Some(member_id),
        SelectionFocus::Element,
        vec![diagnostic.to_string()],
    )
}

fn selection_entry_from_source_node(
    source: &SourceNode,
    resolution: SelectionResolution,
    diagnostic: &str,
) -> SelectionEntry {
    let subject = SelectionSubject {
        kind: source_subject_kind(&source.kind),
        boundary_kind: None,
        component_kind: None,
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
    SelectionEntry {
        member_id: selection_member_id(&anchor),
        resolution,
        subject,
        anchor,
        provenance,
        capabilities,
        diagnostics: vec![diagnostic.to_string()],
        source_html_attributes: None,
    }
}

fn selection_entry_from_css_focus(
    source_graph: &SourceGraph,
    focus: &SelectionFocus,
) -> Result<SelectionEntry, String> {
    let (file, selector, range) = match focus {
        SelectionFocus::CssRule {
            file,
            selector,
            range,
            ..
        }
        | SelectionFocus::CssProperty {
            file,
            selector,
            range,
            ..
        } => (file.trim(), selector.trim(), range.clone()),
        _ => return Err("Selecția CSS sursă a primit un focus incompatibil.".to_string()),
    };
    let normalized_file = normalized_path(file);
    let source = source_graph
        .styles
        .iter()
        .find(|style| normalized_path(&style.file) == normalized_file)
        .and_then(|style| source_graph.node_by_id(&style.node_id))
        .filter(|source| source.kind == SourceNodeKind::Style)
        .ok_or_else(|| {
            format!("SelectionCoordinator nu găsește fișierul CSS/SCSS {file} în Source Graph.")
        })?;

    let mut entry = selection_entry_from_source_node(
        source,
        SelectionResolution::Resolved,
        "Selector CSS selectat direct în sursă.",
    );
    entry.subject = SelectionSubject {
        kind: SelectionSubjectKind::CssRule,
        boundary_kind: None,
        component_kind: None,
        tag: None,
        label: selector.to_string(),
    };
    entry.anchor.range = range.clone();
    entry.provenance.definition = Some(EditorSourceReference {
        source_node_id: Some(source.id.clone()),
        source_kind: Some(SourceNodeKind::Style),
        file: source.file.clone(),
        range,
        label: selector.to_string(),
        origin: match &source.origin {
            SourceOrigin::Local => EditorNavigationOrigin::Project,
            SourceOrigin::Theme => EditorNavigationOrigin::Theme,
        },
        theme_name: source.theme_name.clone(),
        can_open_in_code: true,
    });
    entry.capabilities.can_select = true;
    entry.capabilities.can_inspect = true;
    entry.capabilities.can_open_in_code = true;
    entry.diagnostics.clear();
    entry.member_id = css_rule_member_id(&source.id, selector);
    Ok(entry)
}

fn css_rule_member_id(style_source_node_id: &str, selector: &str) -> String {
    let selector_hash = blake3::hash(selector.as_bytes()).to_hex();
    format!("css-rule:{style_source_node_id}:{}", &selector_hash[..16])
}

fn empty_selection(
    revision: u64,
    snapshot: &EditorNavigationSnapshot,
    active_document_path: Option<&str>,
    focus: SelectionFocus,
) -> SelectionSnapshot {
    selection_from_members(
        revision,
        snapshot,
        active_document_path,
        Vec::new(),
        None,
        None,
        focus,
        Vec::new(),
    )
}

#[allow(clippy::too_many_arguments)]
fn selection_from_members(
    revision: u64,
    snapshot: &EditorNavigationSnapshot,
    active_document_path: Option<&str>,
    mut members: Vec<SelectionEntry>,
    primary_member_id: Option<String>,
    range_origin_member_id: Option<String>,
    focus: SelectionFocus,
    diagnostics: Vec<String>,
) -> SelectionSnapshot {
    let order_by_editor_id = snapshot
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node.order))
        .collect::<std::collections::HashMap<_, _>>();
    members.sort_by(|left, right| {
        let left_order = left
            .anchor
            .editor_node_id
            .as_deref()
            .and_then(|id| order_by_editor_id.get(id).copied())
            .unwrap_or(usize::MAX);
        let right_order = right
            .anchor
            .editor_node_id
            .as_deref()
            .and_then(|id| order_by_editor_id.get(id).copied())
            .unwrap_or(usize::MAX);
        left_order
            .cmp(&right_order)
            .then_with(|| left.member_id.cmp(&right.member_id))
    });
    members.dedup_by(|left, right| left.member_id == right.member_id);

    let primary_member_id = primary_member_id
        .filter(|id| members.iter().any(|member| member.member_id == *id))
        .or_else(|| members.first().map(|member| member.member_id.clone()));
    let range_origin_member_id = range_origin_member_id
        .filter(|id| members.iter().any(|member| member.member_id == *id))
        .or_else(|| primary_member_id.clone());
    let aggregate_capabilities = aggregate_selection_capabilities(snapshot, &members);
    let aggregate_html_facts = aggregate_selection_html_facts(&members);
    SelectionSnapshot {
        schema_version: SELECTION_COORDINATOR_SCHEMA_VERSION,
        selection_revision: revision,
        project_root: snapshot.identity.project_root.clone(),
        runtime_session_id: snapshot.identity.runtime_session_id.clone(),
        canvas_identity: snapshot.identity.clone(),
        route: snapshot.route.clone(),
        active_document_path: active_document_path.map(str::to_string),
        primary_member_id,
        range_origin_member_id,
        members,
        aggregate_capabilities,
        aggregate_html_facts,
        focus,
        diagnostics,
    }
}

fn selection_after_external_member_filter(
    revision: u64,
    current: SelectionSnapshot,
    members: Vec<SelectionEntry>,
    diagnostic: &str,
) -> SelectionSnapshot {
    let primary_member_id = current
        .primary_member_id
        .filter(|id| members.iter().any(|member| member.member_id == *id))
        .or_else(|| members.first().map(|member| member.member_id.clone()));
    let range_origin_member_id = current
        .range_origin_member_id
        .filter(|id| members.iter().any(|member| member.member_id == *id))
        .or_else(|| primary_member_id.clone());
    let focus = if members.is_empty() {
        SelectionFocus::Element
    } else {
        current.focus
    };
    let aggregate_capabilities =
        conservative_filtered_capabilities(&current.aggregate_capabilities, &members);
    let aggregate_html_facts = aggregate_selection_html_facts(&members);
    SelectionSnapshot {
        schema_version: SELECTION_COORDINATOR_SCHEMA_VERSION,
        selection_revision: revision,
        project_root: current.project_root,
        runtime_session_id: current.runtime_session_id,
        canvas_identity: current.canvas_identity,
        route: current.route,
        active_document_path: current.active_document_path,
        primary_member_id,
        range_origin_member_id,
        members,
        aggregate_capabilities,
        aggregate_html_facts,
        focus,
        diagnostics: vec![diagnostic.to_string()],
    }
}

fn conservative_filtered_capabilities(
    previous: &SelectionAggregateCapabilities,
    members: &[SelectionEntry],
) -> SelectionAggregateCapabilities {
    if members.is_empty() {
        return SelectionAggregateCapabilities::default();
    }
    let all_resolved = members
        .iter()
        .all(|member| member.resolution == SelectionResolution::Resolved);
    let all_source_backed = members
        .iter()
        .all(|member| member.anchor.source_node_id.is_some());
    let same_file = members
        .first()
        .and_then(|member| member.anchor.file.as_deref())
        .is_some_and(|file| {
            members
                .iter()
                .all(|member| member.anchor.file.as_deref() == Some(file))
        });
    let mut source_ids = HashSet::with_capacity(members.len());
    let has_duplicate_source_targets = members.iter().any(|member| {
        member
            .anchor
            .source_node_id
            .as_deref()
            .is_some_and(|id| !source_ids.insert(id))
    });
    let all_mutable = members.iter().all(|member| !member.capabilities.read_only);
    let all_structural = members.iter().all(|member| {
        matches!(
            member.subject.kind,
            SelectionSubjectKind::HtmlElement | SelectionSubjectKind::Boundary
        )
    });
    let base_batch = all_resolved
        && all_source_backed
        && all_mutable
        && all_structural
        && !has_duplicate_source_targets;
    let same_parent = previous.same_parent;
    let has_ancestor_descendant = previous.has_ancestor_descendant;
    let can_batch_attributes = base_batch
        && members.iter().all(|member| {
            member.subject.kind == SelectionSubjectKind::HtmlElement
                && member.capabilities.can_edit_attributes
        });
    let can_batch_duplicate = base_batch && !has_ancestor_descendant;
    let can_batch_delete = can_batch_duplicate;
    let can_batch_move = base_batch
        && same_file
        && same_parent
        && !has_ancestor_descendant
        && members.iter().all(|member| member.capabilities.can_move);
    let primary_only_edits_allowed = members.len() == 1 && all_resolved;
    SelectionAggregateCapabilities {
        member_count: members.len(),
        all_resolved,
        all_source_backed,
        same_file,
        same_parent,
        has_ancestor_descendant,
        has_duplicate_source_targets,
        can_batch_attributes,
        can_batch_duplicate,
        can_batch_delete,
        can_batch_move,
        primary_only_edits_allowed,
        primary_only_reason_code: (!primary_only_edits_allowed)
            .then(|| "selection_primary_only_operations".to_string()),
        reasons: vec!["selection_requires_navigation_rebase".to_string()],
    }
}

fn selection_member_id(anchor: &SelectionAnchor) -> String {
    if let Some(editor_node_id) = anchor.editor_node_id.as_deref() {
        return editor_node_id.to_string();
    }
    if let Some(source_node_id) = anchor.source_node_id.as_deref() {
        return format!("source:{source_node_id}");
    }
    if let Some(render_instance_id) = anchor.render_instance_id.as_deref() {
        return format!("render:{render_instance_id}");
    }
    "selection:unresolved".to_string()
}

fn aggregate_selection_resolution(members: &[SelectionEntry]) -> SelectionResolution {
    if members.is_empty() {
        return SelectionResolution::Cleared;
    }
    if members
        .iter()
        .any(|member| member.resolution == SelectionResolution::Ambiguous)
    {
        return SelectionResolution::Ambiguous;
    }
    if members
        .iter()
        .any(|member| member.resolution == SelectionResolution::NotRendered)
    {
        return SelectionResolution::NotRendered;
    }
    SelectionResolution::Resolved
}

fn primary_selection_entry(selection: &SelectionSnapshot) -> Option<&SelectionEntry> {
    let primary_member_id = selection.primary_member_id.as_deref()?;
    selection
        .members
        .iter()
        .find(|member| member.member_id == primary_member_id)
}

fn aggregate_selection_html_facts(members: &[SelectionEntry]) -> SelectionAggregateHtmlFacts {
    let attribute_sets = members
        .iter()
        .map(|member| member.source_html_attributes.as_ref())
        .collect::<Option<Vec<_>>>();
    let Some(attribute_sets) = attribute_sets else {
        return SelectionAggregateHtmlFacts::default();
    };
    let Some(first) = attribute_sets.first() else {
        return SelectionAggregateHtmlFacts::default();
    };

    let class_sets = attribute_sets
        .iter()
        .map(|attributes| {
            attributes
                .get("class")
                .and_then(|value| value.as_deref())
                .unwrap_or_default()
                .split_ascii_whitespace()
                .map(str::to_string)
                .collect::<BTreeSet<_>>()
        })
        .collect::<Vec<_>>();
    let common_class_set = class_sets
        .iter()
        .skip(1)
        .fold(class_sets[0].clone(), |common, classes| {
            common.intersection(classes).cloned().collect()
        });
    let all_class_set = class_sets
        .iter()
        .flat_map(|classes| classes.iter().cloned())
        .collect::<BTreeSet<_>>();

    let attribute_names = attribute_sets
        .iter()
        .flat_map(|attributes| attributes.keys())
        .filter(|name| name.as_str() != "class")
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut common_attributes = BTreeMap::new();
    let mut mixed_attribute_names = Vec::new();
    for name in attribute_names {
        let first_value = first.get(&name);
        if first_value.is_some()
            && attribute_sets
                .iter()
                .skip(1)
                .all(|attributes| attributes.get(&name) == first_value)
        {
            common_attributes.insert(name, first_value.cloned().unwrap_or(None));
        } else {
            mixed_attribute_names.push(name);
        }
    }

    SelectionAggregateHtmlFacts {
        complete: true,
        common_classes: common_class_set.iter().cloned().collect(),
        mixed_classes: all_class_set
            .difference(&common_class_set)
            .cloned()
            .collect(),
        common_attributes,
        mixed_attribute_names,
    }
}

fn aggregate_selection_capabilities(
    snapshot: &EditorNavigationSnapshot,
    members: &[SelectionEntry],
) -> SelectionAggregateCapabilities {
    if members.is_empty() {
        return SelectionAggregateCapabilities::default();
    }
    let nodes_by_id = snapshot
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<std::collections::HashMap<_, _>>();
    let selected_editor_ids = members
        .iter()
        .filter_map(|member| member.anchor.editor_node_id.as_deref())
        .collect::<HashSet<_>>();
    let all_resolved = members
        .iter()
        .all(|member| member.resolution == SelectionResolution::Resolved);
    let all_source_backed = members
        .iter()
        .all(|member| member.anchor.source_node_id.is_some());
    let same_file = members
        .first()
        .and_then(|member| member.anchor.file.as_deref())
        .is_some_and(|file| {
            members
                .iter()
                .all(|member| member.anchor.file.as_deref() == Some(file))
        });
    let editor_nodes = members
        .iter()
        .filter_map(|member| {
            member
                .anchor
                .editor_node_id
                .as_deref()
                .and_then(|id| nodes_by_id.get(id).copied())
        })
        .collect::<Vec<_>>();
    let same_parent = editor_nodes.len() == members.len()
        && editor_nodes.first().is_some_and(|first| {
            editor_nodes
                .iter()
                .all(|node| node.parent_id == first.parent_id)
        });
    let has_ancestor_descendant = editor_nodes.iter().any(|node| {
        let mut parent_id = node.parent_id.as_deref();
        while let Some(parent) = parent_id {
            if selected_editor_ids.contains(parent) {
                return true;
            }
            parent_id = nodes_by_id
                .get(parent)
                .and_then(|candidate| candidate.parent_id.as_deref());
        }
        false
    });
    let mut source_ids = HashSet::with_capacity(members.len());
    let has_duplicate_source_targets = members.iter().any(|member| {
        member
            .anchor
            .source_node_id
            .as_deref()
            .is_some_and(|source_node_id| !source_ids.insert(source_node_id))
    });
    let all_mutable = members.iter().all(|member| !member.capabilities.read_only);
    let all_structural = members.iter().all(|member| {
        matches!(
            member.subject.kind,
            SelectionSubjectKind::HtmlElement | SelectionSubjectKind::Boundary
        )
    });
    let base_batch = all_resolved
        && all_source_backed
        && all_mutable
        && all_structural
        && !has_duplicate_source_targets;
    let can_batch_attributes = base_batch
        && members.iter().all(|member| {
            member.subject.kind == SelectionSubjectKind::HtmlElement
                && member.capabilities.can_edit_attributes
        });
    let can_batch_duplicate = base_batch && !has_ancestor_descendant;
    let can_batch_delete = base_batch && !has_ancestor_descendant;
    let can_batch_move = base_batch
        && same_file
        && same_parent
        && !has_ancestor_descendant
        && members.iter().all(|member| member.capabilities.can_move);
    let primary_only_edits_allowed = members.len() == 1 && all_resolved;
    let mut reasons = Vec::new();
    if !all_resolved {
        reasons.push("selection_members_unresolved".to_string());
    }
    if !all_source_backed {
        reasons.push("selection_members_without_source".to_string());
    }
    if !all_mutable {
        reasons.push("selection_members_read_only".to_string());
    }
    if has_duplicate_source_targets {
        reasons.push("selection_duplicate_source_targets".to_string());
    }
    if has_ancestor_descendant {
        reasons.push("selection_ancestor_descendant_conflict".to_string());
    }
    if !same_file {
        reasons.push("selection_multiple_files".to_string());
    }
    if !same_parent {
        reasons.push("selection_multiple_parents".to_string());
    }
    if !primary_only_edits_allowed {
        reasons.push("selection_primary_only_operations".to_string());
    }
    SelectionAggregateCapabilities {
        member_count: members.len(),
        all_resolved,
        all_source_backed,
        same_file,
        same_parent,
        has_ancestor_descendant,
        has_duplicate_source_targets,
        can_batch_attributes,
        can_batch_duplicate,
        can_batch_delete,
        can_batch_move,
        primary_only_edits_allowed,
        primary_only_reason_code: (!primary_only_edits_allowed)
            .then(|| "selection_primary_only_operations".to_string()),
        reasons,
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
        EditorNavigationNodeKind::Boundary => SelectionSubjectKind::Boundary,
        EditorNavigationNodeKind::RuntimeElement => SelectionSubjectKind::RuntimeElement,
    }
}

fn source_subject_kind(kind: &SourceNodeKind) -> SelectionSubjectKind {
    match kind {
        SourceNodeKind::Html => SelectionSubjectKind::HtmlElement,
        SourceNodeKind::Script => SelectionSubjectKind::RuntimeElement,
        _ => SelectionSubjectKind::Boundary,
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
    if primary_selection_entry(selection)
        .and_then(|entry| entry.subject.tag.as_deref())
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
    let primary = primary_selection_entry(selection);
    let subject = primary.map(|entry| &entry.subject);
    let can_inspect = primary.is_some_and(|entry| entry.capabilities.can_inspect);
    let active_document = state
        .active_inspector_document
        .as_ref()
        .filter(|identity| identity.canvas == selection.canvas_identity);
    let matching_facts = state.inspector_facts.as_ref().filter(|facts| {
        facts.selection_revision == selection.selection_revision
            && facts.canvas_identity == selection.canvas_identity
            && active_document
                .is_some_and(|identity| identity.document_epoch == facts.document_epoch)
            && primary.and_then(|entry| entry.anchor.render_instance_id.as_deref())
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

    match aggregate_selection_resolution(&selection.members) {
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
            if subject.is_some_and(|subject| subject.kind == SelectionSubjectKind::Boundary) =>
        {
            state_value = InspectorSelectionSummaryState::Resolved;
            reason = None;
        }
        SelectionResolution::Resolved
            if subject.is_some_and(|subject| subject.kind == SelectionSubjectKind::CssRule) =>
        {
            state_value = InspectorSelectionSummaryState::Resolved;
            reason = None;
            selector = match &selection.focus {
                SelectionFocus::CssRule { selector, .. }
                | SelectionFocus::CssProperty { selector, .. } => Some(selector.trim().to_string()),
                _ => subject.map(|subject| subject.label.clone()),
            };
            classes = active_css_class(&selection.focus).into_iter().collect();
        }
        SelectionResolution::Resolved
            if primary
                .and_then(|entry| entry.anchor.render_instance_id.as_ref())
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
        render_instance_id: primary.and_then(|entry| entry.anchor.render_instance_id.clone()),
        state: state_value,
        subject_kind: subject.map(|subject| subject.kind),
        boundary_kind: subject.and_then(|subject| subject.boundary_kind),
        component_kind: subject.and_then(|subject| subject.component_kind),
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
    use std::{
        fs,
        hint::black_box,
        time::{Instant, SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use crate::{
        kernel::editor_navigation::{
            editor_navigation_snapshot_for_test, EditorNavigationOrigin, EditorNavigationSurface,
            EditorSourceReference, EditorSourceResolution,
        },
        project_model::test_support::ProjectModelTestFixture,
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
            source_html_attributes: None,
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

    fn mutation_identity(
        selection: &SelectionSnapshot,
        workspace_revision: u64,
    ) -> SelectionMutationIdentity {
        SelectionMutationIdentity {
            selection_revision: selection.selection_revision,
            workspace_revision,
            primary_member_id: selection.primary_member_id.clone(),
            members: mutation_members(selection),
        }
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
            primary_selection_entry(&focused.selection).map(|entry| &entry.subject),
            primary_selection_entry(&selected.selection).map(|entry| &entry.subject),
            "focusul CSS nu trebuie să înlocuiască elementul"
        );
        assert_eq!(
            primary_selection_entry(&focused.selection)
                .and_then(|entry| entry.anchor.editor_node_id.as_deref()),
            Some("editor_render:a")
        );
        assert!(matches!(
            &focused.selection.focus,
            SelectionFocus::CssRule { file, .. } if file == "sass/index.scss"
        ));
        assert!(focused.selection.selection_revision > selected.selection.selection_revision);
        assert_eq!(
            focused.inspector_summary.state,
            InspectorSelectionSummaryState::Resolved
        );
    }

    #[test]
    fn css_source_focus_is_resolved_without_a_preselected_canvas_element() {
        let fixture_root = std::env::temp_dir().join(format!(
            "pana-css-source-selection-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        let mut fixture = ProjectModelTestFixture::standard_zola(
            &fixture_root,
            "<main class=\"hero\">Acasă</main>\n",
        )
        .unwrap();
        fixture.source(
            "sass/pagini/index.scss",
            ".hero { color: red; }\n.card { display: grid; }\n",
        );
        let source_graph = fixture.build_source_graph().unwrap();
        let runtime = SelectionCoordinatorRuntime::default();
        let snapshot = snapshot("tx-css-source", Vec::new());

        let selected = runtime
            .apply(
                &snapshot,
                Some("sass/pagini/index.scss"),
                Some(&source_graph),
                SelectionIntent::SelectCssSourceRule {
                    file: "sass/pagini/index.scss".to_string(),
                    selector: ".hero".to_string(),
                    viewport: None,
                    range: Some(range(0, 5)),
                },
            )
            .unwrap();

        let primary = primary_selection_entry(&selected.selection).unwrap();
        assert_eq!(primary.subject.kind, SelectionSubjectKind::CssRule);
        assert_eq!(primary.subject.label, ".hero");
        assert_eq!(
            primary.anchor.file.as_deref(),
            Some("sass/pagini/index.scss")
        );
        assert!(primary.anchor.render_instance_id.is_none());
        assert_eq!(
            aggregate_selection_resolution(&selected.selection.members),
            SelectionResolution::Resolved,
        );
        assert_eq!(
            selected.inspector_summary.state,
            InspectorSelectionSummaryState::Resolved,
        );
        assert_eq!(
            selected.inspector_summary.subject_kind,
            Some(SelectionSubjectKind::CssRule),
        );
        assert_eq!(
            selected.inspector_summary.selector.as_deref(),
            Some(".hero"),
        );
        assert_eq!(selected.inspector_summary.classes, vec!["hero".to_string()]);

        let mutation = mutation_identity(
            &selected.selection,
            selected.selection.canvas_identity.workspace_revision,
        );
        runtime
            .with_stable_semantic_mutation_target(
                &snapshot.identity.runtime_session_id,
                &mutation,
                || Ok(()),
            )
            .unwrap();

        let previous_member_id = selected.selection.primary_member_id.clone();
        let card = runtime
            .apply(
                &snapshot,
                Some("sass/pagini/index.scss"),
                Some(&source_graph),
                SelectionIntent::SelectCssSourceRule {
                    file: "sass/pagini/index.scss".to_string(),
                    selector: ".card".to_string(),
                    viewport: None,
                    range: Some(range(23, 28)),
                },
            )
            .unwrap();
        assert_ne!(card.selection.primary_member_id, previous_member_id);
        assert_eq!(card.inspector_summary.classes, vec!["card".to_string()]);

        fs::remove_dir_all(fixture_root).unwrap();
    }

    #[test]
    fn opaque_publication_set_is_ordered_primary_bound_and_session_exact() {
        let runtime = SelectionCoordinatorRuntime::default();
        let snapshot = snapshot(
            "tx-opaque-publication",
            vec![
                node("editor_render:a", "source:h1-a", "a", None, range(10, 30)),
                node("editor_render:b", "source:h1-b", "b", None, range(31, 50)),
            ],
        );
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
        let selected = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::ToggleEditorNode {
                    editor_node_id: "editor_render:a".to_string(),
                },
            )
            .unwrap();

        let (primary, member_ids) = runtime
            .current_opaque_selection(Some(&snapshot.identity.runtime_session_id))
            .unwrap();
        assert_eq!(primary, selected.selection.primary_member_id);
        assert_eq!(
            member_ids,
            selected
                .selection
                .members
                .iter()
                .map(|member| member.member_id.clone())
                .collect::<Vec<_>>()
        );
        assert!(runtime
            .current_opaque_selection(Some("session:foreign"))
            .is_err());
        assert_eq!(
            runtime.current_opaque_selection(None).unwrap(),
            (None, vec![])
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
        assert!(error.contains("regulă selectată în Code sau un element selectat în Preview"));
    }

    #[test]
    fn rebase_refuses_binding_and_ambiguous_source_fallbacks() {
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
        assert_eq!(
            aggregate_selection_resolution(&receipt.selection.members),
            SelectionResolution::NotRendered
        );
        assert_eq!(
            primary_selection_entry(&receipt.selection)
                .unwrap()
                .anchor
                .binding_key
                .as_deref(),
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
        assert_eq!(
            aggregate_selection_resolution(&receipt.selection.members),
            SelectionResolution::NotRendered
        );
        assert!(receipt
            .selection
            .members
            .iter()
            .all(|member| member.resolution == SelectionResolution::NotRendered));
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
        tera_node.kind = EditorNavigationNodeKind::Boundary;
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
            Some(SelectionSubjectKind::Boundary)
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
            InspectorSelectionSummaryState::NotRendered
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
        let captured_identity = mutation_identity(&captured, snapshot.identity.workspace_revision);
        runtime
            .require_mutation_target(&snapshot.identity.runtime_session_id, &captured_identity)
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
            .require_mutation_target(&snapshot.identity.runtime_session_id, &captured_identity)
            .unwrap_err();
        assert!(error.contains("selecția s-a schimbat"));
    }

    #[test]
    fn committed_delete_clears_only_the_exact_captured_target() {
        let runtime = SelectionCoordinatorRuntime::default();
        let snapshot = snapshot(
            "tx-delete",
            vec![
                node(
                    "editor_render:child",
                    "source:child",
                    "render:child",
                    None,
                    range(10, 30),
                ),
                node(
                    "editor_render:parent",
                    "source:parent",
                    "render:parent",
                    None,
                    range(5, 40),
                ),
            ],
        );
        let child = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor_render:child".to_string(),
                },
            )
            .unwrap()
            .selection;
        let child_identity = mutation_identity(&child, snapshot.identity.workspace_revision);

        assert!(runtime
            .clear_mutation_target_if_current(
                &snapshot.identity.runtime_session_id,
                &child_identity,
            )
            .unwrap());
        let cleared = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::Rebase,
            )
            .unwrap()
            .selection;
        assert_eq!(
            aggregate_selection_resolution(&cleared.members),
            SelectionResolution::Cleared
        );
        assert!(cleared.members.is_empty());

        let parent = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor_render:parent".to_string(),
                },
            )
            .unwrap()
            .selection;
        assert!(!runtime
            .clear_mutation_target_if_current(
                &snapshot.identity.runtime_session_id,
                &child_identity,
            )
            .unwrap());
        assert_eq!(
            runtime
                .state
                .lock()
                .unwrap()
                .selection
                .as_ref()
                .unwrap()
                .selection_revision,
            parent.selection_revision,
        );
    }

    #[test]
    fn code_edit_never_retargets_a_missing_source_id_to_a_similar_node() {
        let runtime = SelectionCoordinatorRuntime::default();
        let snapshot = snapshot(
            "tx-code-delete",
            vec![
                node(
                    "editor_render:first",
                    "source:first",
                    "render:first",
                    None,
                    range(10, 20),
                ),
                node(
                    "editor_render:second",
                    "source:second",
                    "render:second",
                    None,
                    range(21, 31),
                ),
            ],
        );
        runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor_render:first".to_string(),
                },
            )
            .unwrap();

        assert!(runtime
            .invalidate_missing_source_target(
                &snapshot.identity.runtime_session_id,
                &HashSet::from(["source:second".to_string()]),
            )
            .unwrap());
        let selection = runtime.state.lock().unwrap().selection.clone().unwrap();
        assert_eq!(
            aggregate_selection_resolution(&selection.members),
            SelectionResolution::Cleared
        );
        assert!(selection.members.is_empty());
        assert!(selection.diagnostics[0].contains("SourceNodeId"));
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
        let captured_identity = mutation_identity(&captured, snapshot.identity.workspace_revision);

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
                &captured_identity,
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
        let captured_identity = mutation_identity(&captured, snapshot.identity.workspace_revision);

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
                &captured_identity,
                || Ok(()),
            )
            .unwrap_err();
        assert!(error.contains("setul de ancore"));
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
            primary_selection_entry(&cleared.selection)
                .unwrap()
                .anchor
                .editor_node_id
                .as_deref(),
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
            primary_selection_entry(after_hover)
                .and_then(|entry| entry.anchor.editor_node_id.as_deref()),
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
        assert_eq!(
            aggregate_selection_resolution(&selected.selection.members),
            SelectionResolution::Resolved
        );
        assert_eq!(
            primary_selection_entry(&selected.selection)
                .unwrap()
                .anchor
                .editor_node_id
                .as_deref(),
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
            aggregate_selection_resolution(&ambiguous.selection.members),
            SelectionResolution::Ambiguous
        );
        assert!(ambiguous
            .selection
            .members
            .iter()
            .any(|member| member.resolution == SelectionResolution::Ambiguous));
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
        assert_eq!(
            aggregate_selection_resolution(&receipt.selection.members),
            SelectionResolution::Cleared
        );
        assert!(receipt.selection.members.is_empty());
        assert_eq!(
            receipt.selection.runtime_session_id,
            "session:runtime-2".to_string()
        );
    }

    #[test]
    fn toggle_keeps_canonical_order_and_an_explicit_primary_member() {
        let runtime = SelectionCoordinatorRuntime::default();
        let mut a = node("editor:a", "source:a", "render:a", None, range(10, 20));
        let mut b = node("editor:b", "source:b", "render:b", None, range(30, 40));
        a.order = 1;
        b.order = 2;
        let snapshot = snapshot("tx-multi-toggle", vec![b, a]);

        runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor:b".to_string(),
                },
            )
            .unwrap();
        let selected = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::ToggleEditorNode {
                    editor_node_id: "editor:a".to_string(),
                },
            )
            .unwrap()
            .selection;
        assert_eq!(
            selected.schema_version,
            SELECTION_COORDINATOR_SCHEMA_VERSION
        );
        assert_eq!(
            selected
                .members
                .iter()
                .map(|member| member.member_id.as_str())
                .collect::<Vec<_>>(),
            vec!["editor:a", "editor:b"]
        );
        assert_eq!(selected.primary_member_id.as_deref(), Some("editor:a"));

        let removed = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::ToggleEditorNode {
                    editor_node_id: "editor:a".to_string(),
                },
            )
            .unwrap()
            .selection;
        assert_eq!(removed.members.len(), 1);
        assert_eq!(removed.primary_member_id.as_deref(), Some("editor:b"));
    }

    #[test]
    fn range_selection_is_inclusive_and_uses_editor_navigation_order() {
        let runtime = SelectionCoordinatorRuntime::default();
        let mut nodes = [
            ("editor:a", "source:a", "render:a"),
            ("editor:b", "source:b", "render:b"),
            ("editor:c", "source:c", "render:c"),
            ("editor:d", "source:d", "render:d"),
        ]
        .into_iter()
        .enumerate()
        .map(|(order, (id, source, render))| {
            let mut node = node(id, source, render, None, range(order * 20, order * 20 + 10));
            node.order = order;
            node
        })
        .collect::<Vec<_>>();
        nodes.reverse();
        let snapshot = snapshot("tx-multi-range", nodes);
        runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor:b".to_string(),
                },
            )
            .unwrap();
        let selected = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::ExtendRangeToEditorNode {
                    editor_node_id: "editor:d".to_string(),
                },
            )
            .unwrap()
            .selection;
        assert_eq!(
            selected
                .members
                .iter()
                .map(|member| member.member_id.as_str())
                .collect::<Vec<_>>(),
            vec!["editor:b", "editor:c", "editor:d"]
        );
        assert_eq!(selected.primary_member_id.as_deref(), Some("editor:d"));
        assert_eq!(selected.range_origin_member_id.as_deref(), Some("editor:b"));
    }

    #[test]
    fn aggregate_capabilities_detect_identity_and_tree_conflicts() {
        let runtime = SelectionCoordinatorRuntime::default();
        let mut parent = node(
            "editor:parent",
            "source:shared",
            "render:parent",
            None,
            range(10, 80),
        );
        let mut child = node(
            "editor:child",
            "source:shared",
            "render:child",
            None,
            range(20, 40),
        );
        parent.order = 0;
        parent.children = vec![child.id.clone()];
        child.order = 1;
        child.parent_id = Some(parent.id.clone());
        let snapshot = snapshot("tx-multi-conflicts", vec![parent, child]);
        runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor:parent".to_string(),
                },
            )
            .unwrap();
        let selected = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::ToggleEditorNode {
                    editor_node_id: "editor:child".to_string(),
                },
            )
            .unwrap()
            .selection;
        assert!(selected.aggregate_capabilities.has_ancestor_descendant);
        assert!(selected.aggregate_capabilities.has_duplicate_source_targets);
        assert!(!selected.aggregate_capabilities.can_batch_delete);
        assert!(!selected.aggregate_capabilities.can_batch_move);
    }

    #[test]
    fn aggregate_html_facts_are_source_derived_common_and_mixed_values() {
        let runtime = SelectionCoordinatorRuntime::default();
        let mut first = node(
            "editor:first",
            "source:first",
            "render:first",
            None,
            range(10, 20),
        );
        first.order = 0;
        first.source_html_attributes = Some(BTreeMap::from([
            (
                "class".to_string(),
                Some("card featured shared".to_string()),
            ),
            ("aria-label".to_string(), Some("Card".to_string())),
            ("hidden".to_string(), None),
        ]));
        let mut second = node(
            "editor:second",
            "source:second",
            "render:second",
            None,
            range(30, 40),
        );
        second.order = 1;
        second.source_html_attributes = Some(BTreeMap::from([
            ("class".to_string(), Some("card shared quiet".to_string())),
            ("aria-label".to_string(), Some("Card".to_string())),
            ("hidden".to_string(), Some("hidden".to_string())),
        ]));
        let snapshot = snapshot("tx-multi-html-facts", vec![first, second]);
        runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor:first".to_string(),
                },
            )
            .unwrap();
        let selected = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::ToggleEditorNode {
                    editor_node_id: "editor:second".to_string(),
                },
            )
            .unwrap()
            .selection;

        assert!(selected.aggregate_html_facts.complete);
        assert_eq!(
            selected.aggregate_html_facts.common_classes,
            vec!["card", "shared"]
        );
        assert_eq!(
            selected.aggregate_html_facts.mixed_classes,
            vec!["featured", "quiet"]
        );
        assert_eq!(
            selected
                .aggregate_html_facts
                .common_attributes
                .get("aria-label"),
            Some(&Some("Card".to_string()))
        );
        assert_eq!(
            selected.aggregate_html_facts.mixed_attribute_names,
            vec!["hidden"]
        );
    }

    #[test]
    fn mutation_token_guards_the_complete_set_and_primary_member() {
        let runtime = SelectionCoordinatorRuntime::default();
        let snapshot = snapshot(
            "tx-multi-token",
            vec![
                node("editor:a", "source:a", "render:a", None, range(10, 20)),
                node("editor:b", "source:b", "render:b", None, range(30, 40)),
            ],
        );
        runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor:a".to_string(),
                },
            )
            .unwrap();
        let selected = runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::ToggleEditorNode {
                    editor_node_id: "editor:b".to_string(),
                },
            )
            .unwrap()
            .selection;
        let token = mutation_identity(&selected, 7);
        runtime
            .apply(
                &snapshot,
                Some("templates/index.html"),
                None,
                SelectionIntent::SetPrimaryEditorNode {
                    editor_node_id: "editor:a".to_string(),
                },
            )
            .unwrap();
        let error = runtime
            .with_stable_semantic_mutation_target("session:runtime-1", &token, || Ok(()))
            .unwrap_err();
        assert!(error.contains("setul de ancore"));
    }

    #[test]
    fn rebase_never_substitutes_a_new_node_that_only_shares_the_source() {
        let runtime = SelectionCoordinatorRuntime::default();
        let before = snapshot(
            "tx-before-exact-rebase",
            vec![node(
                "editor:old",
                "source:stable",
                "render:old",
                None,
                range(10, 20),
            )],
        );
        runtime
            .apply(
                &before,
                Some("templates/index.html"),
                None,
                SelectionIntent::SelectEditorNode {
                    editor_node_id: "editor:old".to_string(),
                },
            )
            .unwrap();
        let after = snapshot(
            "tx-after-exact-rebase",
            vec![node(
                "editor:new",
                "source:stable",
                "render:new",
                None,
                range(10, 20),
            )],
        );
        let rebased = runtime
            .apply(
                &after,
                Some("templates/index.html"),
                None,
                SelectionIntent::Rebase,
            )
            .unwrap()
            .selection;
        assert_eq!(
            aggregate_selection_resolution(&rebased.members),
            SelectionResolution::NotRendered
        );
        assert_eq!(rebased.primary_member_id.as_deref(), Some("editor:old"));
    }

    #[test]
    #[ignore = "release performance budget"]
    fn selection_update_for_1_to_256_members_has_warm_p95_below_four_milliseconds() {
        for universe_size in [1_000usize, 10_000] {
            let nodes = (0..universe_size)
                .map(|index| {
                    let mut benchmark_node = node(
                        &format!("editor:{index}"),
                        &format!("source:{index}"),
                        &format!("render:{index}"),
                        None,
                        range(index * 10, index * 10 + 5),
                    );
                    benchmark_node.order = index;
                    benchmark_node.source_html_attributes = Some(BTreeMap::from([
                        (
                            "class".to_string(),
                            Some(format!("shared cohort-{}", index % 4)),
                        ),
                        ("aria-label".to_string(), Some("Card".to_string())),
                        ("data-index".to_string(), Some(index.to_string())),
                    ]));
                    benchmark_node
                })
                .collect::<Vec<_>>();
            let snapshot = snapshot(&format!("tx-benchmark-{universe_size}"), nodes);

            for member_count in [1usize, 10, 100, 256] {
                let runtime = SelectionCoordinatorRuntime::default();
                let mut samples = Vec::with_capacity(64);
                for sample in 0..72 {
                    runtime
                        .apply(
                            &snapshot,
                            Some("templates/index.html"),
                            None,
                            SelectionIntent::SelectEditorNode {
                                editor_node_id: "editor:0".to_string(),
                            },
                        )
                        .unwrap();
                    let started = Instant::now();
                    let selected = runtime
                        .apply(
                            &snapshot,
                            Some("templates/index.html"),
                            None,
                            SelectionIntent::ExtendRangeToEditorNode {
                                editor_node_id: format!("editor:{}", member_count - 1),
                            },
                        )
                        .unwrap()
                        .selection;
                    let elapsed = started.elapsed().as_nanos();
                    assert_eq!(selected.members.len(), member_count);
                    black_box(selected.selection_revision);
                    black_box(selected.aggregate_html_facts);
                    if sample >= 8 {
                        samples.push(elapsed);
                    }
                }
                samples.sort_unstable();
                let p95 = samples[(samples.len() * 95).div_ceil(100).saturating_sub(1)];
                eprintln!(
                    "selection_update universe={universe_size} members={member_count} p95_ns={p95}"
                );
                assert!(
                    p95 < 4_000_000,
                    "Selection update p95 {} ms depășește bugetul de 4 ms pentru {member_count}/{universe_size}",
                    p95 as f64 / 1_000_000.0
                );
            }
        }
    }
}
