use super::runtime::now_ms;
use super::*;

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
        EditorNavigationNodeKind::Boundary => {
            if source
                .boundary
                .as_ref()
                .is_some_and(|boundary| boundary.kind == EditorNavigationBoundaryKind::Markdown)
            {
                (
                    None,
                    None,
                    Some((
                        "editor_move_markdown_read_only",
                        "Conținutul randat din Markdown este atomic și poate fi modificat numai în sursa Markdown.",
                    )),
                )
            } else if !source.capabilities.can_move_atomic {
                (
                    None,
                    None,
                    Some((
                        "editor_move_tera_read_only",
                        "Boundary-ul semantic nu este mutabil în sursa curentă.",
                    )),
                )
            } else {
                let Some(intent) = tera_move_intent(source, target, position) else {
                    return blocked(
                        "editor_move_anchor_missing",
                        "Boundary-ul semantic nu are ancore SourceGraph complete.".to_string(),
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
                let Some(intent) =
                    html_move_intent(source, target, position, native_block_slot.clone())
                else {
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

pub(super) fn enclosing_edit_scope(
    snapshot: &EditorNavigationSnapshot,
    node: &EditorNavigationNode,
    enter_target_boundary: bool,
) -> Option<String> {
    if enter_target_boundary && node.kind == EditorNavigationNodeKind::Boundary {
        return node.capabilities.requires_edit_scope_id.clone();
    }
    if node.kind != EditorNavigationNodeKind::Boundary {
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
        if parent_node.kind == EditorNavigationNodeKind::Boundary {
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
    if let Some(position) = snapshot.node_index.get(node_id).copied() {
        let indexed = if position < snapshot.nodes.len() {
            snapshot.nodes.get(position)
        } else {
            snapshot
                .planning_nodes
                .get(position.saturating_sub(snapshot.nodes.len()))
        };
        if indexed.is_some_and(|node| node.id == node_id) {
            return indexed;
        }
    }
    // Snapshot-urile de producție sunt imuabile și ajung întotdeauna aici cu
    // indexul construit. Fallback-ul păstrează fixture-urile crate-level care
    // modifică deliberat nodurile după construire pentru teste fail-closed.
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
