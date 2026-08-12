use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
};

use tauri::AppHandle;

use crate::{
    kernel::{
        project_session::ProjectSessionSnapshot,
        project_workspace::{
            ProjectWorkspace, ProjectWorkspaceMutationReceipt, WorkspaceCanvasHistoryDelta,
            WorkspaceSourceTreeHistory, WorkspaceSourceTreeHistoryAction,
        },
    },
    project_model::{
        attribute_engine::{plan_html_attributes, raw_tag_attributes},
        delete_engine::plan_html_delete,
        duplicate_engine::plan_html_duplicate,
        html_editor_schema::is_live_projectable_attribute,
        insert_engine::plan_html_insert,
        model::ProjectModel,
        move_engine::{parse_html_tag_at, ProjectMovePosition},
        tag_engine::plan_html_tag,
        text_engine::plan_html_text,
    },
    source_graph::identity::{capture_source_tree_identity, SourceTreeIdentity},
};

use super::super::model::{
    CanvasPatch, CanvasPatchAnchor, CanvasPatchOperation, PreviewHtmlAttributesExecutionInput,
    PreviewHtmlAttributesExecutionReceipt, PreviewHtmlDeleteExecutionInput,
    PreviewHtmlDeleteExecutionReceipt, PreviewHtmlDuplicateExecutionInput,
    PreviewHtmlDuplicateExecutionReceipt, PreviewHtmlInsertDropExecutionInput,
    PreviewHtmlInsertDropExecutionReceipt, PreviewHtmlTagExecutionInput,
    PreviewHtmlTagExecutionReceipt, PreviewHtmlTextExecutionInput, PreviewHtmlTextExecutionReceipt,
};
use super::{
    events::{
        append_html_attributes_event, append_html_delete_event, append_html_duplicate_event,
        append_html_insert_drop_event, append_html_tag_event, append_html_text_event,
    },
    gate::require_preview_executor_intent,
    receipts::{
        blocked_html_attributes_receipt, blocked_html_delete_receipt,
        blocked_html_duplicate_receipt, blocked_html_insert_drop_receipt, blocked_html_tag_receipt,
        blocked_html_text_receipt, committed_html_attributes_receipt,
        committed_html_delete_receipt, committed_html_duplicate_receipt,
        committed_html_insert_drop_receipt, committed_html_tag_receipt,
        committed_html_text_receipt,
    },
    runner::{
        confirmed_html_insert_position, duplicated_html_source_node, inserted_html_source_node,
        run_preview_structural_plan, PreviewStructuralPlanCommitted,
    },
    spec::{
        HTML_ATTRIBUTES_INTENT, HTML_ATTRIBUTES_PLAN, HTML_DELETE_INTENT, HTML_DELETE_PLAN,
        HTML_DUPLICATE_INTENT, HTML_DUPLICATE_PLAN, HTML_INSERT_DROP_INTENT, HTML_INSERT_DROP_PLAN,
        HTML_TAG_INTENT, HTML_TAG_PLAN, HTML_TEXT_INTENT, HTML_TEXT_PLAN,
    },
};

pub struct PreviewHtmlInsertDropExecutionOutcome {
    pub receipt: PreviewHtmlInsertDropExecutionReceipt,
    pub after_model: Option<ProjectModel>,
}

pub struct PreviewHtmlAttributesExecutionOutcome {
    pub receipt: PreviewHtmlAttributesExecutionReceipt,
    pub after_model: Option<ProjectModel>,
}

pub struct PreviewHtmlTextExecutionOutcome {
    pub receipt: PreviewHtmlTextExecutionReceipt,
    pub after_model: Option<ProjectModel>,
}

pub struct PreviewHtmlTagExecutionOutcome {
    pub receipt: PreviewHtmlTagExecutionReceipt,
    pub after_model: Option<ProjectModel>,
}

pub struct PreviewHtmlDuplicateExecutionOutcome {
    pub receipt: PreviewHtmlDuplicateExecutionReceipt,
    pub after_model: Option<ProjectModel>,
}

pub struct PreviewHtmlDeleteExecutionOutcome {
    pub receipt: PreviewHtmlDeleteExecutionReceipt,
    pub after_model: Option<ProjectModel>,
}

pub fn execute_preview_html_insert_drop(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    input: PreviewHtmlInsertDropExecutionInput,
    active_document_path: Option<&str>,
) -> Result<PreviewHtmlInsertDropExecutionOutcome, String> {
    let intent_receipt = match require_preview_executor_intent(
        input.intent.clone(),
        session,
        HTML_INSERT_DROP_INTENT,
    ) {
        Ok(intent_receipt) => intent_receipt,
        Err(blocked) => {
            let receipt =
                blocked_html_insert_drop_receipt(blocked.intent_receipt, None, blocked.diagnostic);
            append_html_insert_drop_event(app, session, &receipt, None);
            return Ok(PreviewHtmlInsertDropExecutionOutcome {
                receipt,
                after_model: None,
            });
        }
    };

    let committed = match run_preview_structural_plan(
        project_root,
        workspace,
        HTML_INSERT_DROP_PLAN,
        |before_model| plan_html_insert(before_model, &input.insert_intent, active_document_path),
    )? {
        Ok(committed) => committed,
        Err(blocked) => {
            let receipt = blocked_html_insert_drop_receipt(
                intent_receipt,
                Some(blocked.model_revision),
                Some(blocked.diagnostic),
            );
            append_html_insert_drop_event(app, session, &receipt, None);
            return Ok(PreviewHtmlInsertDropExecutionOutcome {
                receipt,
                after_model: None,
            });
        }
    };

    let PreviewStructuralPlanCommitted {
        before_model,
        patch,
        commit,
    } = committed;
    let inserted = inserted_html_source_node(&before_model, &commit.after_model, &patch)?;
    let inserted_anchor = Some(CanvasPatchAnchor::source(&inserted.id, Some(&patch.tag)));
    let inserted_source_tree = Some(capture_source_tree_identity(
        &commit.after_model.source_graph,
        &inserted.id,
    )?);
    // În sursă, elementul este inserat în interiorul blocului documentului.
    // În DOM, rădăcina de autor este o ancoră sintetică de append între markerii
    // blocului, deci elementul se așază înaintea ei, nu ca fiu al affordance-ului.
    let canvas_position = if input
        .insert_intent
        .target_kind
        .as_deref()
        .is_some_and(|kind| matches!(kind.trim(), "empty-tera-slot" | "active-document-root"))
    {
        ProjectMovePosition::Before
    } else {
        confirmed_html_insert_position(&before_model, &commit.after_model, &patch)?.ok_or_else(
            || "Modelul confirmat nu conține o relație structurală pentru inserare.".to_string(),
        )?
    };
    let forward_operation = CanvasPatchOperation::Insert {
        target: CanvasPatchAnchor::source(
            &patch.resolved_target_id,
            input.insert_intent.target_tag.as_deref(),
        ),
        position: canvas_position,
        html: patch.html.clone(),
        inserted: inserted_anchor.clone(),
    };
    let canvas_patch = issue_canvas_patch(
        session,
        &commit.workspace_mutation,
        &patch.before_revision,
        &patch.after_revision,
        forward_operation.clone(),
    )?;
    attach_canvas_history_delta(
        workspace,
        &commit.workspace_mutation,
        &patch.before_revision,
        &patch.after_revision,
        forward_operation,
        inserted_anchor.map(|target| CanvasPatchOperation::Delete { target }),
    )?;
    attach_source_tree_history(
        workspace,
        &commit.workspace_mutation,
        WorkspaceSourceTreeHistoryAction::Inserted,
        inserted_source_tree,
    )?;
    let receipt = committed_html_insert_drop_receipt(
        intent_receipt,
        commit.after_model.revision.clone(),
        patch,
        canvas_patch,
        commit.workspace_mutation,
    );
    append_html_insert_drop_event(app, session, &receipt, None);

    Ok(PreviewHtmlInsertDropExecutionOutcome {
        receipt,
        after_model: Some(commit.after_model),
    })
}

pub fn execute_preview_html_attributes(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    input: PreviewHtmlAttributesExecutionInput,
) -> Result<PreviewHtmlAttributesExecutionOutcome, String> {
    let intent_receipt = match require_preview_executor_intent(
        input.intent.clone(),
        session,
        HTML_ATTRIBUTES_INTENT,
    ) {
        Ok(intent_receipt) => intent_receipt,
        Err(blocked) => {
            let receipt =
                blocked_html_attributes_receipt(blocked.intent_receipt, None, blocked.diagnostic);
            append_html_attributes_event(app, session, &receipt, None);
            return Ok(PreviewHtmlAttributesExecutionOutcome {
                receipt,
                after_model: None,
            });
        }
    };

    let committed = match run_preview_structural_plan(
        project_root,
        workspace,
        HTML_ATTRIBUTES_PLAN,
        |before_model| plan_html_attributes(before_model, &input.attribute_intent),
    )? {
        Ok(committed) => committed,
        Err(blocked) => {
            let receipt = blocked_html_attributes_receipt(
                intent_receipt,
                Some(blocked.model_revision),
                Some(blocked.diagnostic),
            );
            append_html_attributes_event(app, session, &receipt, None);
            return Ok(PreviewHtmlAttributesExecutionOutcome {
                receipt,
                after_model: None,
            });
        }
    };

    let PreviewStructuralPlanCommitted {
        before_model,
        patch,
        commit,
    } = committed;
    // A no-op has no ProjectWorkspace transaction identity and therefore must
    // never manufacture a CanvasPatch. Likewise, a source-only attribute
    // (download, target, action, ...) is committed canonically but is omitted
    // from the Editare sigură fast path as one indivisible attribute operation.
    let managed_icon_canvas_operation = (commit.workspace_mutation.changed)
        .then_some(patch.managed_icon.as_ref())
        .flatten()
        .map(|icon| CanvasPatchOperation::SetIcon {
            target: CanvasPatchAnchor::source(
                &patch.resolved_target_id,
                input.attribute_intent.target_tag.as_deref(),
            ),
            provider_id: "icon".to_string(),
            icon_identity: icon.state.icon_identity.clone(),
            attributes: patch.attributes.clone(),
            children_html: icon.children_html.clone(),
        });
    let native_block_canvas_operation = input
        .attribute_intent
        .native_block_option
        .as_ref()
        .and_then(|intent| {
            patch.attributes.iter().next().map(|(attribute, value)| {
                CanvasPatchOperation::SetBlockOption {
                    target: CanvasPatchAnchor::source(
                        &patch.resolved_target_id,
                        input.attribute_intent.target_tag.as_deref(),
                    ),
                    provider_id: intent.provider_id.clone(),
                    option_id: intent.option_id.clone(),
                    attribute: attribute.clone(),
                    value: value.clone(),
                }
            })
        });
    let generic_canvas_allowed = html_attribute_canvas_patch_allowed(
        commit.workspace_mutation.changed,
        &patch.attributes,
        patch.zola_image_contract,
    );
    let canvas_operation = managed_icon_canvas_operation
        .or(native_block_canvas_operation)
        .or_else(|| {
            generic_canvas_allowed.then(|| CanvasPatchOperation::SetAttributes {
                target: CanvasPatchAnchor::source(
                    &patch.resolved_target_id,
                    input.attribute_intent.target_tag.as_deref(),
                ),
                attributes: patch.attributes.clone(),
            })
        });
    let inverse_operation = if let Some(icon) = patch.managed_icon.as_ref() {
        let target = projected_history_anchor(
            &commit.after_model,
            &patch.resolved_target_id,
            Some(&patch.tag),
        )
        .ok_or_else(|| "CanvasPatch Icon nu a putut ancora operația inversă.".to_string())?;
        Some(CanvasPatchOperation::SetIcon {
            target,
            provider_id: "icon".to_string(),
            icon_identity: icon.previous_state.icon_identity.clone(),
            attributes: icon.previous_attributes.clone(),
            children_html: icon.previous_children_html.clone(),
        })
    } else {
        canvas_operation.as_ref().and_then(|_| {
            let attributes = previous_attribute_values(
                &before_model,
                &patch.resolved_target_id,
                patch.attributes.keys(),
            )?;
            let target = projected_history_anchor(
                &commit.after_model,
                &patch.resolved_target_id,
                Some(&patch.tag),
            )?;
            Some(CanvasPatchOperation::SetAttributes { target, attributes })
        })
    };
    let canvas_patch = if commit.workspace_mutation.changed {
        canvas_operation
            .clone()
            .map(|operation| {
                issue_canvas_patch(
                    session,
                    &commit.workspace_mutation,
                    &patch.before_revision,
                    &patch.after_revision,
                    operation,
                )
            })
            .transpose()?
    } else {
        None
    };
    if let Some(forward_operation) = canvas_operation {
        attach_canvas_history_delta(
            workspace,
            &commit.workspace_mutation,
            &patch.before_revision,
            &patch.after_revision,
            forward_operation,
            inverse_operation,
        )?;
    }
    let receipt = committed_html_attributes_receipt(
        intent_receipt,
        commit.after_model.revision.clone(),
        patch,
        canvas_patch,
        commit.workspace_mutation,
    );
    append_html_attributes_event(app, session, &receipt, None);

    Ok(PreviewHtmlAttributesExecutionOutcome {
        receipt,
        after_model: Some(commit.after_model),
    })
}

pub fn execute_preview_html_text(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    input: PreviewHtmlTextExecutionInput,
) -> Result<PreviewHtmlTextExecutionOutcome, String> {
    let intent_receipt =
        match require_preview_executor_intent(input.intent.clone(), session, HTML_TEXT_INTENT) {
            Ok(intent_receipt) => intent_receipt,
            Err(blocked) => {
                let receipt =
                    blocked_html_text_receipt(blocked.intent_receipt, None, blocked.diagnostic);
                append_html_text_event(app, session, &receipt, None);
                return Ok(PreviewHtmlTextExecutionOutcome {
                    receipt,
                    after_model: None,
                });
            }
        };

    let committed = match super::runner::run_preview_structural_plan_in_history_group(
        project_root,
        workspace,
        HTML_TEXT_PLAN,
        input.edit_session_id.as_deref(),
        |before_model| plan_html_text(before_model, &input.text_intent),
    )? {
        Ok(committed) => committed,
        Err(blocked) => {
            let receipt = blocked_html_text_receipt(
                intent_receipt,
                Some(blocked.model_revision),
                Some(blocked.diagnostic),
            );
            append_html_text_event(app, session, &receipt, None);
            return Ok(PreviewHtmlTextExecutionOutcome {
                receipt,
                after_model: None,
            });
        }
    };

    let PreviewStructuralPlanCommitted {
        before_model: _,
        patch,
        commit,
    } = committed;
    let forward_operation = CanvasPatchOperation::SetText {
        target: CanvasPatchAnchor::source(
            &patch.resolved_target_id,
            input.text_intent.target_tag.as_deref(),
        ),
        text: patch.text.clone(),
    };
    let inverse_operation = projected_history_anchor(
        &commit.after_model,
        &patch.resolved_target_id,
        Some(&patch.tag),
    )
    .map(|target| CanvasPatchOperation::SetTextHtml {
        target,
        escaped_text: patch.previous_escaped_text.clone(),
    });
    let canvas_patch = if commit.workspace_mutation.changed {
        Some(issue_canvas_patch(
            session,
            &commit.workspace_mutation,
            &patch.before_revision,
            &patch.after_revision,
            forward_operation.clone(),
        )?)
    } else {
        None
    };
    if commit.workspace_mutation.changed {
        attach_canvas_history_delta(
            workspace,
            &commit.workspace_mutation,
            &patch.before_revision,
            &patch.after_revision,
            forward_operation,
            inverse_operation,
        )?;
    }
    let receipt = committed_html_text_receipt(
        intent_receipt,
        commit.after_model.revision.clone(),
        patch,
        canvas_patch,
        commit.workspace_mutation,
    );
    append_html_text_event(app, session, &receipt, None);

    Ok(PreviewHtmlTextExecutionOutcome {
        receipt,
        after_model: Some(commit.after_model),
    })
}

pub fn execute_preview_html_tag(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    input: PreviewHtmlTagExecutionInput,
) -> Result<PreviewHtmlTagExecutionOutcome, String> {
    let intent_receipt =
        match require_preview_executor_intent(input.intent.clone(), session, HTML_TAG_INTENT) {
            Ok(intent_receipt) => intent_receipt,
            Err(blocked) => {
                let receipt =
                    blocked_html_tag_receipt(blocked.intent_receipt, None, blocked.diagnostic);
                append_html_tag_event(app, session, &receipt, None);
                return Ok(PreviewHtmlTagExecutionOutcome {
                    receipt,
                    after_model: None,
                });
            }
        };

    let committed =
        match run_preview_structural_plan(project_root, workspace, HTML_TAG_PLAN, |before_model| {
            plan_html_tag(before_model, &input.tag_intent)
        })? {
            Ok(committed) => committed,
            Err(blocked) => {
                let receipt = blocked_html_tag_receipt(
                    intent_receipt,
                    Some(blocked.model_revision),
                    Some(blocked.diagnostic),
                );
                append_html_tag_event(app, session, &receipt, None);
                return Ok(PreviewHtmlTagExecutionOutcome {
                    receipt,
                    after_model: None,
                });
            }
        };

    let PreviewStructuralPlanCommitted {
        before_model: _,
        patch,
        commit,
    } = committed;
    let forward_operation = CanvasPatchOperation::ReplaceTag {
        target: CanvasPatchAnchor::source(
            &patch.resolved_target_id,
            input.tag_intent.target_tag.as_deref(),
        ),
        new_tag: patch.new_tag.clone(),
    };
    let inverse_operation = projected_history_anchor(
        &commit.after_model,
        &patch.resolved_target_id,
        Some(&patch.new_tag),
    )
    .map(|target| CanvasPatchOperation::ReplaceTag {
        target,
        new_tag: patch.old_tag.clone(),
    });
    let canvas_patch = issue_canvas_patch(
        session,
        &commit.workspace_mutation,
        &patch.before_revision,
        &patch.after_revision,
        forward_operation.clone(),
    )?;
    attach_canvas_history_delta(
        workspace,
        &commit.workspace_mutation,
        &patch.before_revision,
        &patch.after_revision,
        forward_operation,
        inverse_operation,
    )?;
    let receipt = committed_html_tag_receipt(
        intent_receipt,
        commit.after_model.revision.clone(),
        patch,
        canvas_patch,
        commit.workspace_mutation,
    );
    append_html_tag_event(app, session, &receipt, None);

    Ok(PreviewHtmlTagExecutionOutcome {
        receipt,
        after_model: Some(commit.after_model),
    })
}

pub fn execute_preview_html_duplicate(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    input: PreviewHtmlDuplicateExecutionInput,
) -> Result<PreviewHtmlDuplicateExecutionOutcome, String> {
    let intent_receipt =
        match require_preview_executor_intent(input.intent.clone(), session, HTML_DUPLICATE_INTENT)
        {
            Ok(intent_receipt) => intent_receipt,
            Err(blocked) => {
                let receipt = blocked_html_duplicate_receipt(
                    blocked.intent_receipt,
                    None,
                    blocked.diagnostic,
                );
                append_html_duplicate_event(app, session, &receipt, None);
                return Ok(PreviewHtmlDuplicateExecutionOutcome {
                    receipt,
                    after_model: None,
                });
            }
        };

    let committed = match run_preview_structural_plan(
        project_root,
        workspace,
        HTML_DUPLICATE_PLAN,
        |before_model| plan_html_duplicate(before_model, &input.duplicate_intent),
    )? {
        Ok(committed) => committed,
        Err(blocked) => {
            let receipt = blocked_html_duplicate_receipt(
                intent_receipt,
                Some(blocked.model_revision),
                Some(blocked.diagnostic),
            );
            append_html_duplicate_event(app, session, &receipt, None);
            return Ok(PreviewHtmlDuplicateExecutionOutcome {
                receipt,
                after_model: None,
            });
        }
    };

    let PreviewStructuralPlanCommitted {
        before_model,
        patch,
        commit,
    } = committed;
    let inserted = duplicated_html_source_node(&before_model, &commit.after_model, &patch)?;
    let inserted_anchor = Some(CanvasPatchAnchor::source(&inserted.id, Some(&patch.tag)));
    let inserted_source_tree = Some(capture_source_tree_identity(
        &commit.after_model.source_graph,
        &inserted.id,
    )?);
    let forward_operation = CanvasPatchOperation::Duplicate {
        source: CanvasPatchAnchor::source(
            &patch.resolved_source_id,
            input.duplicate_intent.source_tag.as_deref(),
        ),
        html: patch.html.clone(),
        inserted: inserted_anchor.clone(),
    };
    let canvas_patch = if patch.zola_image_contract || patch.dynamic_widget_contract {
        None
    } else {
        Some(issue_canvas_patch(
            session,
            &commit.workspace_mutation,
            &patch.before_revision,
            &patch.after_revision,
            forward_operation.clone(),
        )?)
    };
    if !patch.zola_image_contract && !patch.dynamic_widget_contract {
        attach_canvas_history_delta(
            workspace,
            &commit.workspace_mutation,
            &patch.before_revision,
            &patch.after_revision,
            forward_operation,
            inserted_anchor.map(|target| CanvasPatchOperation::Delete { target }),
        )?;
    }
    attach_source_tree_history(
        workspace,
        &commit.workspace_mutation,
        WorkspaceSourceTreeHistoryAction::Inserted,
        inserted_source_tree,
    )?;
    let receipt = committed_html_duplicate_receipt(
        intent_receipt,
        commit.after_model.revision.clone(),
        patch,
        canvas_patch,
        commit.workspace_mutation,
    );
    append_html_duplicate_event(app, session, &receipt, None);

    Ok(PreviewHtmlDuplicateExecutionOutcome {
        receipt,
        after_model: Some(commit.after_model),
    })
}

pub fn execute_preview_html_delete(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    input: PreviewHtmlDeleteExecutionInput,
) -> Result<PreviewHtmlDeleteExecutionOutcome, String> {
    let intent_receipt =
        match require_preview_executor_intent(input.intent.clone(), session, HTML_DELETE_INTENT) {
            Ok(intent_receipt) => intent_receipt,
            Err(blocked) => {
                let receipt =
                    blocked_html_delete_receipt(blocked.intent_receipt, None, blocked.diagnostic);
                append_html_delete_event(app, session, &receipt, None);
                return Ok(PreviewHtmlDeleteExecutionOutcome {
                    receipt,
                    after_model: None,
                });
            }
        };

    let committed = match run_preview_structural_plan(
        project_root,
        workspace,
        HTML_DELETE_PLAN,
        |before_model| plan_html_delete(before_model, &input.delete_intent),
    )? {
        Ok(committed) => committed,
        Err(blocked) => {
            let receipt = blocked_html_delete_receipt(
                intent_receipt,
                Some(blocked.model_revision),
                Some(blocked.diagnostic),
            );
            append_html_delete_event(app, session, &receipt, None);
            return Ok(PreviewHtmlDeleteExecutionOutcome {
                receipt,
                after_model: None,
            });
        }
    };

    let PreviewStructuralPlanCommitted {
        before_model,
        patch,
        commit,
    } = committed;
    let forward_operation = CanvasPatchOperation::Delete {
        target: CanvasPatchAnchor::source(
            &patch.resolved_target_id,
            input.delete_intent.target_tag.as_deref(),
        ),
    };
    let deleted_source_tree = Some(capture_source_tree_identity(
        &before_model.source_graph,
        &patch.resolved_target_id,
    )?);
    let inverse_operation = inverse_delete_operation(
        &before_model,
        &commit.after_model,
        &patch.resolved_target_id,
        input.delete_intent.target_tag.as_deref(),
    );
    let canvas_patch = issue_canvas_patch(
        session,
        &commit.workspace_mutation,
        &patch.before_revision,
        &patch.after_revision,
        forward_operation.clone(),
    )?;
    attach_canvas_history_delta(
        workspace,
        &commit.workspace_mutation,
        &patch.before_revision,
        &patch.after_revision,
        forward_operation,
        inverse_operation,
    )?;
    attach_source_tree_history(
        workspace,
        &commit.workspace_mutation,
        WorkspaceSourceTreeHistoryAction::Deleted,
        deleted_source_tree,
    )?;
    let receipt = committed_html_delete_receipt(
        intent_receipt,
        commit.after_model.revision.clone(),
        patch,
        canvas_patch,
        commit.workspace_mutation,
    );
    append_html_delete_event(app, session, &receipt, None);

    Ok(PreviewHtmlDeleteExecutionOutcome {
        receipt,
        after_model: Some(commit.after_model),
    })
}

pub(super) fn attach_canvas_history_delta(
    workspace: &mut ProjectWorkspace,
    mutation: &ProjectWorkspaceMutationReceipt,
    before_model_revision: &str,
    after_model_revision: &str,
    forward: CanvasPatchOperation,
    inverse: Option<CanvasPatchOperation>,
) -> Result<(), String> {
    let Some(inverse) = inverse else {
        return Ok(());
    };
    let transaction_id = mutation
        .transaction_id
        .as_deref()
        .ok_or_else(|| "Canvas History cere transactionId-ul mutației.".to_string())?;
    let current_history_entry = mutation.history.next_undo.as_ref();
    let direct_match =
        current_history_entry.is_some_and(|entry| entry.transaction_id == transaction_id);
    let coalesced_group_match = mutation
        .entry
        .as_ref()
        .and_then(|entry| entry.coalesce_key.as_deref())
        .zip(current_history_entry.and_then(|entry| entry.coalesce_key.as_deref()))
        .is_some_and(|(mutation_key, current_key)| mutation_key == current_key);
    if !direct_match && !coalesced_group_match {
        // A coalesced edit can return exactly to its original source. In that
        // case History intentionally removes the net-noop entry; the forward
        // DOM patch remains valid, but there is no Undo delta to retain.
        return Ok(());
    }
    workspace.attach_latest_canvas_history_delta(
        transaction_id,
        WorkspaceCanvasHistoryDelta {
            before_model_revision: before_model_revision.to_string(),
            after_model_revision: after_model_revision.to_string(),
            forward,
            inverse,
        },
    )
}

pub(super) fn attach_source_tree_history(
    workspace: &mut ProjectWorkspace,
    mutation: &ProjectWorkspaceMutationReceipt,
    action: WorkspaceSourceTreeHistoryAction,
    source_tree: Option<SourceTreeIdentity>,
) -> Result<(), String> {
    let Some(source_tree) = source_tree else {
        return Ok(());
    };
    attach_source_forest_history(workspace, mutation, action, vec![source_tree])
}

pub(super) fn attach_source_forest_history(
    workspace: &mut ProjectWorkspace,
    mutation: &ProjectWorkspaceMutationReceipt,
    action: WorkspaceSourceTreeHistoryAction,
    trees: Vec<SourceTreeIdentity>,
) -> Result<(), String> {
    if trees.is_empty() {
        return Ok(());
    }
    let transaction_id = mutation
        .transaction_id
        .as_deref()
        .ok_or_else(|| "SourceGraph History cere transactionId-ul mutației.".to_string())?;
    workspace.attach_latest_source_tree_identity(
        transaction_id,
        WorkspaceSourceTreeHistory { action, trees },
    )
}

fn projected_history_anchor(
    after_model: &ProjectModel,
    before_source_id: &str,
    expected_tag: Option<&str>,
) -> Option<CanvasPatchAnchor> {
    after_model
        .source_graph
        .node_by_id(before_source_id)
        .is_some()
        .then(|| CanvasPatchAnchor::source(before_source_id, expected_tag))
}

fn previous_attribute_values<'a>(
    before_model: &ProjectModel,
    target_source_id: &str,
    names: impl Iterator<Item = &'a String>,
) -> Option<BTreeMap<String, Option<String>>> {
    let target = before_model.source_graph.node_by_id(target_source_id)?;
    let range = target.range.as_ref()?;
    let file = before_model
        .files
        .iter()
        .find(|file| file.relative_path == target.file)?;
    let tag = parse_html_tag_at(&file.contents, range.start)?;
    let opening = file.contents.get(tag.start..tag.end)?;
    let current = raw_tag_attributes(opening)
        .into_iter()
        .map(|attribute| (attribute.name, Some(attribute.value.unwrap_or_default())))
        .collect::<HashMap<_, _>>();
    Some(
        names
            .map(|name| (name.clone(), current.get(name).cloned().unwrap_or(None)))
            .collect(),
    )
}

fn inverse_delete_operation(
    before_model: &ProjectModel,
    after_model: &ProjectModel,
    deleted_source_id: &str,
    expected_tag: Option<&str>,
) -> Option<CanvasPatchOperation> {
    let deleted = before_model.source_graph.node_by_id(deleted_source_id)?;
    let range = deleted.range.as_ref()?;
    let file = before_model
        .files
        .iter()
        .find(|file| file.relative_path == deleted.file)?;
    let html = file.contents.get(range.start..range.end)?.to_string();
    let parent_id = deleted.parent.as_deref()?;
    let parent = before_model.source_graph.node_by_id(parent_id)?;
    let deleted_index = parent
        .children
        .iter()
        .position(|child| child == deleted_source_id)?;
    let (target_before_id, position) =
        if let Some(next_sibling) = parent.children.get(deleted_index.saturating_add(1)) {
            (next_sibling.as_str(), ProjectMovePosition::Before)
        } else {
            (parent_id, ProjectMovePosition::Inside)
        };
    let target = projected_history_anchor(after_model, target_before_id, None)?;
    Some(CanvasPatchOperation::Insert {
        target,
        position,
        html,
        inserted: Some(CanvasPatchAnchor::source(deleted_source_id, expected_tag)),
    })
}

pub(super) fn issue_canvas_patch(
    session: &ProjectSessionSnapshot,
    workspace_mutation: &ProjectWorkspaceMutationReceipt,
    before_model_revision: &str,
    after_model_revision: &str,
    operation: CanvasPatchOperation,
) -> Result<CanvasPatch, String> {
    CanvasPatch::issued(
        &session.project_root,
        &session.runtime_instance_id(),
        workspace_mutation,
        before_model_revision,
        after_model_revision,
        operation,
    )
}

fn html_attribute_canvas_patch_allowed(
    workspace_changed: bool,
    attributes: &BTreeMap<String, Option<String>>,
    zola_image_contract: bool,
) -> bool {
    workspace_changed
        && !zola_image_contract
        && !attributes.is_empty()
        && attributes
            .keys()
            .all(|name| is_live_projectable_attribute(name))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::html_attribute_canvas_patch_allowed;

    #[test]
    fn html_attribute_canvas_patch_requires_a_real_live_projectable_transition() {
        let live = BTreeMap::from([("href".to_string(), Some("/despre".to_string()))]);
        assert!(html_attribute_canvas_patch_allowed(true, &live, false));
        assert!(!html_attribute_canvas_patch_allowed(false, &live, false));

        let source_only = BTreeMap::from([("download".to_string(), Some(String::new()))]);
        assert!(!html_attribute_canvas_patch_allowed(
            true,
            &source_only,
            false
        ));

        let mixed = BTreeMap::from([
            ("href".to_string(), Some("/despre".to_string())),
            ("target".to_string(), Some("_blank".to_string())),
        ]);
        assert!(!html_attribute_canvas_patch_allowed(true, &mixed, false));
        assert!(!html_attribute_canvas_patch_allowed(true, &live, true));
    }
}
