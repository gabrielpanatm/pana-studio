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
        },
    },
    project_model::{
        attribute_engine::{plan_html_attributes, raw_tag_attributes},
        delete_engine::plan_html_delete,
        duplicate_engine::plan_html_duplicate,
        html_editor_schema::is_live_projectable_attribute,
        insert_engine::plan_html_insert,
        model::ProjectModel,
        move_engine::{
            html_identity_aliases, html_node_id_at_line, html_node_id_at_location,
            parse_html_tag_at, ProjectMovePosition, ProjectSourceEditLocation,
        },
        tag_engine::plan_html_tag,
        text_engine::plan_html_text,
    },
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
    runner::{run_preview_structural_plan, PreviewStructuralPlanCommitted},
    spec::{
        HTML_ATTRIBUTES_INTENT, HTML_ATTRIBUTES_PLAN, HTML_DELETE_INTENT, HTML_DELETE_PLAN,
        HTML_DUPLICATE_INTENT, HTML_DUPLICATE_PLAN, HTML_INSERT_DROP_INTENT, HTML_INSERT_DROP_PLAN,
        HTML_TAG_INTENT, HTML_TAG_PLAN, HTML_TEXT_INTENT, HTML_TEXT_PLAN,
    },
};

pub struct PreviewHtmlInsertDropExecutionOutcome {
    pub receipt: PreviewHtmlInsertDropExecutionReceipt,
    pub after_model: Option<ProjectModel>,
    pub alias_updates: HashMap<String, String>,
}

pub struct PreviewHtmlAttributesExecutionOutcome {
    pub receipt: PreviewHtmlAttributesExecutionReceipt,
    pub after_model: Option<ProjectModel>,
    pub alias_updates: HashMap<String, String>,
}

pub struct PreviewHtmlTextExecutionOutcome {
    pub receipt: PreviewHtmlTextExecutionReceipt,
    pub after_model: Option<ProjectModel>,
    pub alias_updates: HashMap<String, String>,
}

pub struct PreviewHtmlTagExecutionOutcome {
    pub receipt: PreviewHtmlTagExecutionReceipt,
    pub after_model: Option<ProjectModel>,
    pub alias_updates: HashMap<String, String>,
}

pub struct PreviewHtmlDuplicateExecutionOutcome {
    pub receipt: PreviewHtmlDuplicateExecutionReceipt,
    pub after_model: Option<ProjectModel>,
    pub alias_updates: HashMap<String, String>,
}

pub struct PreviewHtmlDeleteExecutionOutcome {
    pub receipt: PreviewHtmlDeleteExecutionReceipt,
    pub after_model: Option<ProjectModel>,
    pub alias_updates: HashMap<String, String>,
}

pub fn execute_preview_html_insert_drop(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    input: PreviewHtmlInsertDropExecutionInput,
    aliases: &HashMap<String, String>,
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
                alias_updates: HashMap::new(),
            });
        }
    };

    let committed = match run_preview_structural_plan(
        project_root,
        workspace,
        HTML_INSERT_DROP_PLAN,
        |before_model| {
            plan_html_insert(
                before_model,
                &input.insert_intent,
                aliases,
                active_document_path,
            )
        },
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
                alias_updates: HashMap::new(),
            });
        }
    };

    let PreviewStructuralPlanCommitted {
        before_model,
        patch,
        commit,
    } = committed;
    let alias_updates = html_identity_aliases(&before_model, &commit.after_model);
    let inserted_anchor =
        html_node_id_at_location(&commit.after_model, &patch.inserted_location, &patch.tag)
            .map(|source_id| CanvasPatchAnchor::source(source_id, None, Some(&patch.tag)));
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
        input.insert_intent.position
    };
    let forward_operation = CanvasPatchOperation::Insert {
        target: CanvasPatchAnchor::source(
            &patch.resolved_target_id,
            input.insert_intent.target_selector.as_deref(),
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
        alias_updates,
    })
}

pub fn execute_preview_html_attributes(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    input: PreviewHtmlAttributesExecutionInput,
    aliases: &HashMap<String, String>,
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
                alias_updates: HashMap::new(),
            });
        }
    };

    let committed = match run_preview_structural_plan(
        project_root,
        workspace,
        HTML_ATTRIBUTES_PLAN,
        |before_model| plan_html_attributes(before_model, &input.attribute_intent, aliases),
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
                alias_updates: HashMap::new(),
            });
        }
    };

    let PreviewStructuralPlanCommitted {
        before_model,
        patch,
        commit,
    } = committed;
    let alias_updates = html_target_alias_updates(
        &before_model,
        &commit.after_model,
        &patch.resolved_target_id,
        &patch.target_location,
        &patch.tag,
    );
    // A no-op has no ProjectWorkspace transaction identity and therefore must
    // never manufacture a CanvasPatch. Likewise, a source-only attribute
    // (download, target, action, ...) is committed canonically but is omitted
    // from the Editare sigură fast path as one indivisible attribute operation.
    let native_block_canvas_operation = input
        .attribute_intent
        .native_block_option
        .as_ref()
        .and_then(|intent| {
            patch.attributes.iter().next().map(|(attribute, value)| {
                CanvasPatchOperation::SetBlockOption {
                    target: CanvasPatchAnchor::source(
                        &patch.resolved_target_id,
                        input.attribute_intent.target_selector.as_deref(),
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
    let canvas_operation = native_block_canvas_operation.or_else(|| {
        generic_canvas_allowed.then(|| CanvasPatchOperation::SetAttributes {
            target: CanvasPatchAnchor::source(
                &patch.resolved_target_id,
                input.attribute_intent.target_selector.as_deref(),
                input.attribute_intent.target_tag.as_deref(),
            ),
            attributes: patch.attributes.clone(),
        })
    });
    let inverse_operation = canvas_operation.as_ref().and_then(|_| {
        let attributes = previous_attribute_values(
            &before_model,
            &patch.resolved_target_id,
            patch.attributes.keys(),
        )?;
        let target = projected_history_anchor(
            &commit.after_model,
            &patch.resolved_target_id,
            &alias_updates,
            Some(&patch.tag),
        )?;
        Some(CanvasPatchOperation::SetAttributes { target, attributes })
    });
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
        alias_updates,
    })
}

pub fn execute_preview_html_text(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    input: PreviewHtmlTextExecutionInput,
    aliases: &HashMap<String, String>,
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
                    alias_updates: HashMap::new(),
                });
            }
        };

    let committed = match super::runner::run_preview_structural_plan_in_history_group(
        project_root,
        workspace,
        HTML_TEXT_PLAN,
        input.edit_session_id.as_deref(),
        |before_model| plan_html_text(before_model, &input.text_intent, aliases),
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
                alias_updates: HashMap::new(),
            });
        }
    };

    let PreviewStructuralPlanCommitted {
        before_model,
        patch,
        commit,
    } = committed;
    let alias_updates = html_target_alias_updates(
        &before_model,
        &commit.after_model,
        &patch.resolved_target_id,
        &patch.target_location,
        &patch.tag,
    );
    let forward_operation = CanvasPatchOperation::SetText {
        target: CanvasPatchAnchor::source(
            &patch.resolved_target_id,
            input.text_intent.target_selector.as_deref(),
            input.text_intent.target_tag.as_deref(),
        ),
        text: patch.text.clone(),
    };
    let inverse_operation = projected_history_anchor(
        &commit.after_model,
        &patch.resolved_target_id,
        &alias_updates,
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
        alias_updates,
    })
}

pub fn execute_preview_html_tag(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    input: PreviewHtmlTagExecutionInput,
    aliases: &HashMap<String, String>,
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
                    alias_updates: HashMap::new(),
                });
            }
        };

    let committed =
        match run_preview_structural_plan(project_root, workspace, HTML_TAG_PLAN, |before_model| {
            plan_html_tag(before_model, &input.tag_intent, aliases)
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
                    alias_updates: HashMap::new(),
                });
            }
        };

    let PreviewStructuralPlanCommitted {
        before_model,
        patch,
        commit,
    } = committed;
    let alias_updates = html_target_alias_updates(
        &before_model,
        &commit.after_model,
        &patch.resolved_target_id,
        &patch.target_location,
        &patch.new_tag,
    );
    let forward_operation = CanvasPatchOperation::ReplaceTag {
        target: CanvasPatchAnchor::source(
            &patch.resolved_target_id,
            input.tag_intent.target_selector.as_deref(),
            input.tag_intent.target_tag.as_deref(),
        ),
        new_tag: patch.new_tag.clone(),
    };
    let inverse_operation = projected_history_anchor(
        &commit.after_model,
        &patch.resolved_target_id,
        &alias_updates,
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
        alias_updates,
    })
}

pub fn execute_preview_html_duplicate(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    input: PreviewHtmlDuplicateExecutionInput,
    aliases: &HashMap<String, String>,
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
                    alias_updates: HashMap::new(),
                });
            }
        };

    let committed = match run_preview_structural_plan(
        project_root,
        workspace,
        HTML_DUPLICATE_PLAN,
        |before_model| plan_html_duplicate(before_model, &input.duplicate_intent, aliases),
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
                alias_updates: HashMap::new(),
            });
        }
    };

    let PreviewStructuralPlanCommitted {
        before_model,
        patch,
        commit,
    } = committed;
    let alias_updates = html_identity_aliases(&before_model, &commit.after_model);
    let inserted_anchor =
        html_node_id_at_location(&commit.after_model, &patch.inserted_location, &patch.tag)
            .map(|source_id| CanvasPatchAnchor::source(source_id, None, Some(&patch.tag)));
    let forward_operation = CanvasPatchOperation::Duplicate {
        source: CanvasPatchAnchor::source(
            &patch.resolved_source_id,
            input.duplicate_intent.source_selector.as_deref(),
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
        alias_updates,
    })
}

pub fn execute_preview_html_delete(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    input: PreviewHtmlDeleteExecutionInput,
    aliases: &HashMap<String, String>,
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
                    alias_updates: HashMap::new(),
                });
            }
        };

    let committed = match run_preview_structural_plan(
        project_root,
        workspace,
        HTML_DELETE_PLAN,
        |before_model| plan_html_delete(before_model, &input.delete_intent, aliases),
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
                alias_updates: HashMap::new(),
            });
        }
    };

    let PreviewStructuralPlanCommitted {
        before_model,
        patch,
        commit,
    } = committed;
    let alias_updates = html_identity_aliases(&before_model, &commit.after_model);
    let forward_operation = CanvasPatchOperation::Delete {
        target: CanvasPatchAnchor::source(
            &patch.resolved_target_id,
            input.delete_intent.target_selector.as_deref(),
            input.delete_intent.target_tag.as_deref(),
        ),
    };
    let inverse_operation = inverse_delete_operation(
        &before_model,
        &commit.after_model,
        &patch.resolved_target_id,
        input.delete_intent.target_tag.as_deref(),
        &alias_updates,
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
        alias_updates,
    })
}

fn attach_canvas_history_delta(
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

fn projected_history_anchor(
    after_model: &ProjectModel,
    before_source_id: &str,
    aliases: &HashMap<String, String>,
    expected_tag: Option<&str>,
) -> Option<CanvasPatchAnchor> {
    let after_source_id = aliases.get(before_source_id).cloned().or_else(|| {
        after_model
            .source_graph
            .nodes
            .iter()
            .any(|node| node.id == before_source_id)
            .then(|| before_source_id.to_string())
    })?;
    Some(
        CanvasPatchAnchor::source(after_source_id, None, expected_tag)
            .with_alternate_source_ids([before_source_id.to_string()]),
    )
}

fn previous_attribute_values<'a>(
    before_model: &ProjectModel,
    target_source_id: &str,
    names: impl Iterator<Item = &'a String>,
) -> Option<BTreeMap<String, Option<String>>> {
    let target = before_model
        .source_graph
        .nodes
        .iter()
        .find(|node| node.id == target_source_id)?;
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
    aliases: &HashMap<String, String>,
) -> Option<CanvasPatchOperation> {
    let deleted = before_model
        .source_graph
        .nodes
        .iter()
        .find(|node| node.id == deleted_source_id)?;
    let range = deleted.range.as_ref()?;
    let file = before_model
        .files
        .iter()
        .find(|file| file.relative_path == deleted.file)?;
    let html = file.contents.get(range.start..range.end)?.to_string();
    let parent_id = deleted.parent.as_deref()?;
    let parent = before_model
        .source_graph
        .nodes
        .iter()
        .find(|node| node.id == parent_id)?;
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
    let target = projected_history_anchor(after_model, target_before_id, aliases, None)?;
    Some(CanvasPatchOperation::Insert {
        target,
        position,
        html,
        inserted: Some(CanvasPatchAnchor::source(
            deleted_source_id,
            None,
            expected_tag,
        )),
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

fn html_target_alias_updates(
    before_model: &ProjectModel,
    after_model: &ProjectModel,
    resolved_target_id: &str,
    target_location: &ProjectSourceEditLocation,
    after_tag: &str,
) -> HashMap<String, String> {
    let mut updates = html_identity_aliases(before_model, after_model);
    if let Some(after_id) = html_node_id_at_location(after_model, target_location, after_tag) {
        if after_id != resolved_target_id {
            updates.insert(resolved_target_id.to_string(), after_id);
        }
    }
    updates
}

pub(super) fn html_move_alias_updates(
    before_model: &ProjectModel,
    after_model: &ProjectModel,
    patch: &crate::project_model::move_engine::ProjectHtmlMovePatch,
) -> HashMap<String, String> {
    let mut updates = html_identity_aliases(before_model, after_model);
    if let Some(after_id) = html_node_id_at_line(
        after_model,
        &patch.file,
        &patch.source_label,
        patch.new_start_line,
    ) {
        updates.insert(patch.resolved_source_id.clone(), after_id);
    }
    updates
}

pub(super) fn html_move_projected_source_id(
    after_model: &ProjectModel,
    resolved_source_id: &str,
    alias_updates: &HashMap<String, String>,
) -> Option<String> {
    // Prefer the authoritative before -> after mapping. With same-label,
    // same-width siblings, the old positional ID can legitimately be reused
    // by the sibling which moved into the vacated byte range.
    if let Some(projected) = alias_updates.get(resolved_source_id) {
        return Some(projected.clone());
    }
    if after_model
        .source_graph
        .nodes
        .iter()
        .any(|node| node.id == resolved_source_id)
    {
        return Some(resolved_source_id.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, HashMap},
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        project_model::{
            attribute_engine::{
                plan_html_attributes, ProjectHtmlAttributeIntent, ProjectHtmlAttributeMutation,
            },
            build_project_model,
            move_engine::{plan_html_move, ProjectHtmlMoveIntent, ProjectMovePosition},
        },
        source_graph::model::SourceNodeKind,
    };

    use super::{
        html_attribute_canvas_patch_allowed, html_move_alias_updates,
        html_move_projected_source_id, html_target_alias_updates,
    };

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

    #[test]
    fn consecutive_attribute_commands_keep_stable_identity_without_alias_churn() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::create_dir_all(root.join("content")).unwrap();
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
                "{% block content %}\n",
                "<main>\n",
                "  <h1 class=\"hero-title\" data-anim=\"ps-old\">Titlu</h1>\n",
                "</main>\n",
                "{% endblock %}\n",
            ),
        )
        .unwrap();

        let before = build_project_model(&root, &HashMap::new()).unwrap();
        let original_id = html_ids(&before, "<h1 .hero-title>")
            .into_iter()
            .next()
            .expect("missing h1");
        let first_plan = plan_html_attributes(
            &before,
            &ProjectHtmlAttributeIntent {
                target_source_id: Some(original_id.clone()),
                target_location: None,
                target_tag: Some("h1".to_string()),
                target_selector: Some("main > h1".to_string()),
                attributes: vec![ProjectHtmlAttributeMutation::remove("data-anim")],
                zola_image: None,
                native_block_option: None,
            },
            &HashMap::new(),
        );
        assert!(first_plan.allowed, "{:?}", first_plan.diagnostic);
        let first_patch = first_plan.patch.unwrap();
        let mut drafts = HashMap::new();
        drafts.insert(first_patch.file.clone(), first_patch.contents.clone());
        let after_first = build_project_model(&root, &drafts).unwrap();
        let aliases = html_target_alias_updates(
            &before,
            &after_first,
            &first_patch.resolved_target_id,
            &first_patch.target_location,
            &first_patch.tag,
        );
        let projected_id = html_ids(&after_first, "<h1 .hero-title>")
            .into_iter()
            .next()
            .expect("missing projected h1");
        assert_eq!(projected_id, original_id);
        assert!(!aliases.contains_key(&original_id));

        let second_plan = plan_html_attributes(
            &after_first,
            &ProjectHtmlAttributeIntent {
                target_source_id: Some(original_id.clone()),
                target_location: Some(first_patch.target_location.clone()),
                target_tag: Some("h1".to_string()),
                target_selector: Some("main > h1".to_string()),
                attributes: vec![ProjectHtmlAttributeMutation::set("data-anim", "ps-new")],
                zola_image: None,
                native_block_option: None,
            },
            &aliases,
        );

        let mut cyclic_aliases = aliases.clone();
        cyclic_aliases.insert(projected_id.clone(), original_id.clone());
        let cycle_recovery_plan = plan_html_attributes(
            &after_first,
            &ProjectHtmlAttributeIntent {
                target_source_id: Some(original_id),
                target_location: Some(first_patch.target_location.clone()),
                target_tag: Some("h1".to_string()),
                target_selector: Some("main > h1".to_string()),
                attributes: vec![ProjectHtmlAttributeMutation::set("data-anim", "ps-new")],
                zola_image: None,
                native_block_option: None,
            },
            &cyclic_aliases,
        );

        let mut chained_aliases = HashMap::new();
        let mut from = first_patch.resolved_target_id.clone();
        for index in 0..10 {
            let to = if index == 9 {
                projected_id.clone()
            } else {
                format!("intermediate-source-id-{index}")
            };
            chained_aliases.insert(from, to.clone());
            from = to;
        }
        let long_lived_selection_plan = plan_html_attributes(
            &after_first,
            &ProjectHtmlAttributeIntent {
                target_source_id: Some(first_patch.resolved_target_id),
                target_location: Some(first_patch.target_location),
                target_tag: Some("h1".to_string()),
                target_selector: Some("main > h1".to_string()),
                attributes: vec![ProjectHtmlAttributeMutation::set(
                    "title",
                    "Selecție persistentă",
                )],
                zola_image: None,
                native_block_option: None,
            },
            &chained_aliases,
        );

        fs::remove_dir_all(root).unwrap();
        assert!(second_plan.allowed, "{:?}", second_plan.diagnostic);
        assert!(second_plan
            .patch
            .expect("second attribute patch")
            .contents
            .contains("data-anim=\"ps-new\""));
        assert!(
            cycle_recovery_plan.allowed,
            "{:?}",
            cycle_recovery_plan.diagnostic
        );
        assert!(
            long_lived_selection_plan.allowed,
            "{:?}",
            long_lived_selection_plan.diagnostic
        );
    }

    #[test]
    fn same_tag_sibling_editor_move_identity_resolves_exact_moved_node() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::create_dir_all(root.join("content")).unwrap();
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
                "{% block content %}\n",
                "<main>\n",
                "  <section>A</section>\n",
                "  <section>B</section>\n",
                "</main>\n",
                "{% endblock %}\n",
            ),
        )
        .unwrap();

        let before = build_project_model(&root, &HashMap::new()).unwrap();
        let before_siblings = html_ids(&before, "<section>");
        assert_eq!(before_siblings.len(), 2);
        let source_id = before_siblings[0].clone();
        let target_id = before_siblings[1].clone();
        let plan = plan_html_move(
            &before,
            &ProjectHtmlMoveIntent {
                source_source_id: Some(source_id.clone()),
                target_source_id: Some(target_id),
                source_location: None,
                target_location: None,
                source_tag: Some("section".to_string()),
                target_tag: Some("section".to_string()),
                source_selector: Some("main > section:nth-of-type(1)".to_string()),
                target_selector: Some("main > section:nth-of-type(2)".to_string()),
                position: ProjectMovePosition::After,
            },
            &HashMap::new(),
        );
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.unwrap();
        let mut drafts = HashMap::new();
        drafts.insert(patch.file.clone(), patch.contents.clone());
        let after = build_project_model(&root, &drafts).unwrap();
        let aliases = html_move_alias_updates(&before, &after, &patch);
        let projected = html_move_projected_source_id(&after, &source_id, &aliases).unwrap();
        let after_siblings = html_ids(&after, "<section>");
        assert_eq!(after_siblings.len(), 2);
        let sibling = after_siblings[0].clone();
        let expected = after_siblings[1].clone();

        fs::remove_dir_all(root).unwrap();
        assert_eq!(projected, expected);
        assert_ne!(projected, sibling);
        assert_ne!(projected, source_id);
    }

    fn html_ids(model: &crate::project_model::model::ProjectModel, label: &str) -> Vec<String> {
        let mut nodes = model
            .source_graph
            .nodes
            .iter()
            .filter(|node| node.kind == SourceNodeKind::Html && node.label == label)
            .collect::<Vec<_>>();
        nodes.sort_by_key(|node| {
            node.range
                .as_ref()
                .map(|range| range.start)
                .unwrap_or(usize::MAX)
        });
        nodes.into_iter().map(|node| node.id.clone()).collect()
    }

    fn unique_test_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "pana-layer-projection-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ))
    }
}
