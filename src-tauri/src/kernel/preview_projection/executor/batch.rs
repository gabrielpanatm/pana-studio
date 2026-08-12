use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::Path,
};

use crate::{
    kernel::{
        project_session::ProjectSessionSnapshot,
        project_workspace::{
            ProjectWorkspace, WorkspaceDocumentMutation, WorkspaceSourceTreeHistoryAction,
        },
        selection_coordinator::SelectionMutationIdentity,
    },
    project_model::{
        attribute_engine::{
            plan_html_attributes, raw_tag_attributes, ProjectGeneratedIdentityIntent,
            ProjectGeneratedIdentityKind, ProjectHtmlAttributeIntent, ProjectHtmlAttributeMutation,
        },
        cache::rebuild_project_model_from_previous_projection,
        delete_engine::{plan_html_delete, ProjectHtmlDeleteIntent},
        duplicate_engine::{plan_html_duplicate, ProjectHtmlDuplicateIntent},
        html_editor_schema::is_live_projectable_attribute,
        model::ProjectModel,
        move_engine::{parse_html_tag_at, plan_html_batch_move, ProjectMovePosition},
        ProjectModelIncrementalIntent,
    },
    source_graph::{
        identity::{
            capture_source_forest_identity, SourceChangeSet, SourceTextEdit, SourceTreeIdentity,
            SourceTreeMovePosition,
        },
        model::{SourceGraph, SourceNode, SourceNodeKind},
    },
};

use super::super::{
    model::{
        CanvasPatchAnchor, CanvasPatchOperation, PreviewSelectionBatchAction,
        PreviewSelectionBatchExecutionInput, PreviewSelectionBatchExecutionReceipt,
        PreviewSelectionBatchExecutionStatus, PREVIEW_SELECTION_BATCH_EXECUTION_SCHEMA_VERSION,
    },
    structural_write::{
        stage_preview_structural_batch_write_in_transaction, PreviewStructuralBatchWrite,
        PreviewStructuralBatchWriteCommit,
    },
};
use super::html::{attach_canvas_history_delta, attach_source_forest_history, issue_canvas_patch};

#[derive(Clone)]
struct IndependentEdit {
    source_id: String,
    file: String,
    old_start: usize,
    old_end: usize,
    replacement: String,
    inserted_relative_start: Option<usize>,
}

struct CombinedFile {
    file: String,
    contents: String,
    text_edits: Vec<SourceTextEdit>,
    inserted_starts: HashMap<String, usize>,
}

pub struct PreviewSelectionBatchExecutionOutcome {
    pub receipt: PreviewSelectionBatchExecutionReceipt,
    pub after_model: Option<ProjectModel>,
}

pub fn execute_preview_selection_batch(
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    input: PreviewSelectionBatchExecutionInput,
    selection: &SelectionMutationIdentity,
) -> Result<PreviewSelectionBatchExecutionOutcome, String> {
    if input.schema_version != PREVIEW_SELECTION_BATCH_EXECUTION_SCHEMA_VERSION {
        return Err("Executorul batch a primit o versiune incompatibilă.".to_string());
    }
    let source_ids = selection_source_ids(selection)?;
    let primary_source_id = selection
        .primary_member_id
        .as_ref()
        .and_then(|primary_id| {
            selection
                .members
                .iter()
                .find(|member| &member.member_id == primary_id)
        })
        .and_then(|member| member.source_node_id.clone())
        .ok_or_else(|| "Executorul batch cere un membru principal source-backed.".to_string())?;
    let before_model = current_project_model(project_root, workspace)?;
    require_plain_html_roots(&before_model.source_graph, &source_ids)?;

    match input.action {
        PreviewSelectionBatchAction::SetAttributes { attributes } => execute_batch_attributes(
            session,
            project_root,
            workspace,
            before_model,
            source_ids,
            attributes,
        ),
        PreviewSelectionBatchAction::MutateClasses { add, remove } => execute_batch_classes(
            session,
            project_root,
            workspace,
            before_model,
            source_ids,
            add,
            remove,
        ),
        PreviewSelectionBatchAction::GenerateSharedClass => execute_batch_generate_shared_class(
            session,
            project_root,
            workspace,
            before_model,
            source_ids,
            &primary_source_id,
        ),
        PreviewSelectionBatchAction::Delete => {
            execute_batch_delete(session, project_root, workspace, before_model, source_ids)
        }
        PreviewSelectionBatchAction::Duplicate => execute_batch_duplicate(
            session,
            project_root,
            workspace,
            before_model,
            source_ids,
            &primary_source_id,
        ),
        PreviewSelectionBatchAction::Move {
            target_source_id,
            target_tag,
            position,
        } => execute_batch_move(
            session,
            project_root,
            workspace,
            before_model,
            source_ids,
            target_source_id,
            target_tag,
            position,
        ),
    }
}

fn execute_batch_attributes(
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    before_model: ProjectModel,
    source_ids: Vec<String>,
    attributes: Vec<ProjectHtmlAttributeMutation>,
) -> Result<PreviewSelectionBatchExecutionOutcome, String> {
    if attributes.is_empty() {
        return Ok(blocked_outcome(
            Some(before_model.revision),
            source_ids,
            "Mutația batch de atribute este goală.",
        ));
    }
    if source_ids.len() > 1
        && attributes.iter().any(|mutation| {
            matches!(
                mutation,
                ProjectHtmlAttributeMutation::SetAttribute {
                    name,
                    value,
                } if name.eq_ignore_ascii_case("id") && !value.trim().is_empty()
            )
        })
    {
        return Ok(blocked_outcome(
            Some(before_model.revision),
            source_ids,
            "Atributul id nu poate primi aceeași valoare pe mai multe elemente.",
        ));
    }
    let member_attributes = source_ids
        .iter()
        .cloned()
        .map(|source_id| (source_id, attributes.clone()))
        .collect();
    execute_batch_member_attributes(
        session,
        project_root,
        workspace,
        before_model,
        source_ids,
        member_attributes,
    )
}

fn execute_batch_classes(
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    before_model: ProjectModel,
    source_ids: Vec<String>,
    add: Vec<String>,
    remove: Vec<String>,
) -> Result<PreviewSelectionBatchExecutionOutcome, String> {
    let (add, remove) = normalize_class_delta(add, remove)?;
    if add.is_empty() && remove.is_empty() {
        return Ok(blocked_outcome(
            Some(before_model.revision),
            source_ids,
            "Mutația batch de clase este goală.",
        ));
    }
    let member_attributes = source_ids
        .iter()
        .filter_map(|source_id| {
            match class_delta_for_source(&before_model, source_id, &add, &remove) {
                Ok(Some(mutation)) => Some(Ok((source_id.clone(), vec![mutation]))),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    if member_attributes.is_empty() {
        return Ok(committed_noop(&before_model, source_ids));
    }
    execute_batch_member_attributes(
        session,
        project_root,
        workspace,
        before_model,
        source_ids,
        member_attributes,
    )
}

fn execute_batch_generate_shared_class(
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    before_model: ProjectModel,
    source_ids: Vec<String>,
    primary_source_id: &str,
) -> Result<PreviewSelectionBatchExecutionOutcome, String> {
    let generated = plan_html_attributes(
        &before_model,
        &ProjectHtmlAttributeIntent {
            target_source_id: Some(primary_source_id.to_string()),
            target_tag: None,
            attributes: Vec::new(),
            zola_image: None,
            native_block_option: None,
            native_icon: None,
            generated_identity: Some(ProjectGeneratedIdentityIntent {
                kind: ProjectGeneratedIdentityKind::Class,
            }),
        },
    );
    let Some(generated_class) = generated
        .patch
        .and_then(|patch| patch.generated_identity)
        .map(|projection| projection.value)
    else {
        return Ok(blocked_outcome(
            Some(generated.model_revision),
            source_ids,
            generated
                .diagnostic
                .as_deref()
                .unwrap_or("Rust nu a putut genera clasa comună."),
        ));
    };
    let mut outcome = execute_batch_classes(
        session,
        project_root,
        workspace,
        before_model,
        source_ids,
        vec![generated_class.clone()],
        Vec::new(),
    )?;
    if outcome.receipt.status == PreviewSelectionBatchExecutionStatus::Committed {
        outcome.receipt.generated_class = Some(generated_class);
    }
    Ok(outcome)
}

fn execute_batch_member_attributes(
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    before_model: ProjectModel,
    source_ids: Vec<String>,
    member_attributes: Vec<(String, Vec<ProjectHtmlAttributeMutation>)>,
) -> Result<PreviewSelectionBatchExecutionOutcome, String> {
    let mut patches = Vec::with_capacity(source_ids.len());
    for (source_id, attributes) in member_attributes {
        let plan = plan_html_attributes(
            &before_model,
            &ProjectHtmlAttributeIntent {
                target_source_id: Some(source_id),
                target_tag: None,
                attributes,
                zola_image: None,
                native_block_option: None,
                native_icon: None,
                generated_identity: None,
            },
        );
        let Some(patch) = plan.patch else {
            return Ok(blocked_outcome(
                Some(plan.model_revision),
                source_ids,
                plan.diagnostic
                    .as_deref()
                    .unwrap_or("Atributele batch au fost blocate."),
            ));
        };
        patches.push(patch);
    }
    let edits = patches
        .iter()
        .map(|patch| {
            independent_edit(
                &before_model,
                &patch.file,
                &patch.contents,
                &patch.resolved_target_id,
                None,
            )
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect();
    let combined = combine_edits(&before_model, edits)?;
    if combined.is_empty() {
        return Ok(committed_noop(&before_model, source_ids));
    }
    let commit = commit_combined(
        project_root,
        workspace,
        &before_model,
        &combined,
        BTreeMap::new(),
        BTreeMap::new(),
        BTreeMap::new(),
        "Selecție multiplă: atribute",
    )?;
    let all_live = patches.iter().all(|patch| {
        !patch.attributes.is_empty()
            && patch
                .attributes
                .keys()
                .all(|name| is_live_projectable_attribute(name))
    });
    let forwards = all_live.then(|| {
        patches
            .iter()
            .map(|patch| CanvasPatchOperation::SetAttributes {
                target: CanvasPatchAnchor::source(&patch.resolved_target_id, Some(&patch.tag)),
                attributes: patch.attributes.clone(),
            })
            .collect::<Vec<_>>()
    });
    let inverses = if all_live {
        Some(
            patches
                .iter()
                .map(|patch| {
                    previous_attribute_values(
                        &before_model,
                        &patch.resolved_target_id,
                        patch.attributes.keys(),
                    )
                    .ok_or_else(|| {
                        format!(
                            "CanvasPatch batch nu poate captura atributele anterioare pentru {}.",
                            patch.resolved_target_id
                        )
                    })
                    .map(|attributes| CanvasPatchOperation::SetAttributes {
                        target: CanvasPatchAnchor::source(
                            &patch.resolved_target_id,
                            Some(&patch.tag),
                        ),
                        attributes,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?,
        )
    } else {
        None
    };
    finish_batch_commit(
        session,
        workspace,
        &before_model,
        commit,
        source_ids,
        forwards,
        inverses,
        None,
    )
}

fn execute_batch_delete(
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    before_model: ProjectModel,
    source_ids: Vec<String>,
) -> Result<PreviewSelectionBatchExecutionOutcome, String> {
    let history_trees = history_forests(&before_model.source_graph, &source_ids)?;
    let mut patches = Vec::with_capacity(source_ids.len());
    for source_id in &source_ids {
        let plan = plan_html_delete(
            &before_model,
            &ProjectHtmlDeleteIntent {
                target_source_id: Some(source_id.clone()),
                target_render_instance_id: None,
                target_tag: None,
                native_block_slot: None,
            },
        );
        let Some(patch) = plan.patch else {
            return Ok(blocked_outcome(
                Some(plan.model_revision),
                source_ids,
                plan.diagnostic
                    .as_deref()
                    .unwrap_or("Ștergerea batch a fost blocată."),
            ));
        };
        patches.push(patch);
    }
    let edits = patches
        .iter()
        .map(|patch| {
            independent_edit(
                &before_model,
                &patch.file,
                &patch.contents,
                &patch.resolved_target_id,
                None,
            )
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect();
    let combined = combine_edits(&before_model, edits)?;
    let deletes = ids_by_file(&before_model.source_graph, &source_ids)?;
    let commit = commit_combined(
        project_root,
        workspace,
        &before_model,
        &combined,
        deletes,
        BTreeMap::new(),
        BTreeMap::new(),
        "Selecție multiplă: ștergere",
    )?;
    for source_id in &source_ids {
        if commit
            .after_model
            .source_graph
            .node_by_id(source_id)
            .is_some()
        {
            return Err(format!(
                "Ștergerea batch nu a eliminat SourceNodeId {source_id}."
            ));
        }
    }
    let forwards = patches
        .iter()
        .map(|patch| CanvasPatchOperation::Delete {
            target: CanvasPatchAnchor::source(
                &patch.resolved_target_id,
                source_tag(&before_model, &patch.resolved_target_id).as_deref(),
            ),
        })
        .collect::<Vec<_>>();
    let inverses = inverse_delete_operations(&before_model, &commit.after_model, &source_ids)?;
    finish_batch_commit(
        session,
        workspace,
        &before_model,
        commit,
        source_ids,
        Some(forwards),
        Some(inverses),
        Some((WorkspaceSourceTreeHistoryAction::Deleted, history_trees)),
    )
}

fn execute_batch_duplicate(
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    before_model: ProjectModel,
    source_ids: Vec<String>,
    primary_source_id: &str,
) -> Result<PreviewSelectionBatchExecutionOutcome, String> {
    let mut patches = Vec::with_capacity(source_ids.len());
    for source_id in &source_ids {
        let plan = plan_html_duplicate(
            &before_model,
            &ProjectHtmlDuplicateIntent {
                source_source_id: Some(source_id.clone()),
                source_tag: None,
                native_block_slot: None,
            },
        );
        let Some(patch) = plan.patch else {
            return Ok(blocked_outcome(
                Some(plan.model_revision),
                source_ids,
                plan.diagnostic
                    .as_deref()
                    .unwrap_or("Duplicarea batch a fost blocată."),
            ));
        };
        if patch.zola_image_contract || patch.dynamic_widget_contract {
            return Ok(blocked_outcome(
                Some(plan.model_revision),
                source_ids,
                "Duplicarea batch a refuzat un contract specializat.",
            ));
        }
        patches.push(patch);
    }
    let edits = patches
        .iter()
        .map(|patch| {
            independent_edit(
                &before_model,
                &patch.file,
                &patch.contents,
                &patch.resolved_source_id,
                Some(patch.inserted_offset),
            )
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect();
    let combined = combine_edits(&before_model, edits)?;
    let duplicates = combined
        .iter()
        .map(|file| {
            let values = file
                .inserted_starts
                .iter()
                .map(|(source_id, start)| (source_id.clone(), *start))
                .collect::<Vec<_>>();
            (file.file.clone(), values)
        })
        .collect::<BTreeMap<_, _>>();
    let commit = commit_combined(
        project_root,
        workspace,
        &before_model,
        &combined,
        BTreeMap::new(),
        duplicates,
        BTreeMap::new(),
        "Selecție multiplă: duplicare",
    )?;
    let inserted = inserted_duplicate_roots(&commit.after_model, &combined)?;
    let inserted_by_origin = combined
        .iter()
        .flat_map(|file| {
            file.inserted_starts.iter().filter_map(|(origin, start)| {
                inserted
                    .iter()
                    .find(|node| {
                        node.file == file.file
                            && node
                                .range
                                .as_ref()
                                .is_some_and(|range| range.start == *start)
                    })
                    .map(|node| (origin.clone(), node.id.clone()))
            })
        })
        .collect::<HashMap<_, _>>();
    let inserted_ids = source_ids
        .iter()
        .map(|source_id| {
            inserted_by_origin
                .get(source_id)
                .cloned()
                .ok_or_else(|| format!("Duplicarea batch nu a corelat copia pentru {source_id}."))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let history_trees = history_forests(&commit.after_model.source_graph, &inserted_ids)?;
    let primary_inserted_id = inserted_by_origin
        .get(primary_source_id)
        .cloned()
        .ok_or_else(|| "Duplicarea batch nu a corelat copia membrului principal.".to_string())?;
    let forwards = patches
        .iter()
        .map(|patch| {
            let inserted_id = inserted_by_origin
                .get(&patch.resolved_source_id)
                .ok_or_else(|| {
                    format!(
                        "Duplicarea batch nu a corelat copia pentru {}.",
                        patch.resolved_source_id
                    )
                })?;
            Ok(CanvasPatchOperation::Duplicate {
                source: CanvasPatchAnchor::source(&patch.resolved_source_id, Some(&patch.tag)),
                html: patch.html.clone(),
                inserted: Some(CanvasPatchAnchor::source(inserted_id, Some(&patch.tag))),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let inverses = inserted_ids
        .iter()
        .map(|source_id| CanvasPatchOperation::Delete {
            target: CanvasPatchAnchor::source(
                source_id,
                source_tag(&commit.after_model, source_id).as_deref(),
            ),
        })
        .rev()
        .collect::<Vec<_>>();
    let mut outcome = finish_batch_commit(
        session,
        workspace,
        &before_model,
        commit,
        inserted_ids,
        Some(forwards),
        Some(inverses),
        Some((WorkspaceSourceTreeHistoryAction::Inserted, history_trees)),
    )?;
    outcome.receipt.primary_affected_source_id = Some(primary_inserted_id);
    Ok(outcome)
}

#[allow(clippy::too_many_arguments)]
fn execute_batch_move(
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    before_model: ProjectModel,
    source_ids: Vec<String>,
    target_source_id: String,
    target_tag: Option<String>,
    position: ProjectMovePosition,
) -> Result<PreviewSelectionBatchExecutionOutcome, String> {
    let patch = match plan_html_batch_move(
        &before_model,
        &source_ids,
        &target_source_id,
        target_tag.as_deref(),
        position,
    ) {
        Ok(patch) => patch,
        Err(diagnostic) => {
            return Ok(blocked_outcome(
                Some(before_model.revision),
                source_ids,
                &diagnostic,
            ))
        }
    };
    let before = before_model
        .files
        .iter()
        .find(|file| file.relative_path == patch.file)
        .ok_or_else(|| format!("Mutarea batch nu găsește documentul {}.", patch.file))?;
    let change = SourceChangeSet::between(&patch.file, &before.contents, &patch.contents);
    let combined = vec![CombinedFile {
        file: patch.file.clone(),
        contents: patch.contents.clone(),
        text_edits: change.edits,
        inserted_starts: HashMap::new(),
    }];
    let ordered_forwards = if position == ProjectMovePosition::Before {
        patch.resolved_source_ids.clone()
    } else {
        patch.resolved_source_ids.iter().rev().cloned().collect()
    };
    let moves = BTreeMap::from([(
        patch.file.clone(),
        ordered_forwards
            .iter()
            .map(|source_id| {
                (
                    source_id.clone(),
                    patch.resolved_target_id.clone(),
                    position,
                )
            })
            .collect(),
    )]);
    let commit = commit_combined(
        project_root,
        workspace,
        &before_model,
        &combined,
        BTreeMap::new(),
        BTreeMap::new(),
        moves,
        "Selecție multiplă: mutare",
    )?;
    let forwards = ordered_forwards
        .iter()
        .map(|source_id| CanvasPatchOperation::Move {
            source: CanvasPatchAnchor::source(
                source_id,
                source_tag(&before_model, source_id).as_deref(),
            ),
            target: CanvasPatchAnchor::source(&patch.resolved_target_id, target_tag.as_deref()),
            position,
        })
        .collect::<Vec<_>>();
    let selected = patch
        .resolved_source_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let parent_id = before_model
        .source_graph
        .node_by_id(&patch.resolved_source_ids[0])
        .and_then(|node| node.parent.clone())
        .ok_or_else(|| "Mutarea batch a pierdut părintele original.".to_string())?;
    let parent = before_model
        .source_graph
        .node_by_id(&parent_id)
        .ok_or_else(|| "Mutarea batch a pierdut ordinea originală.".to_string())?;
    let mut inverses = Vec::new();
    for (index, source_id) in parent.children.iter().enumerate().rev() {
        if !selected.contains(source_id.as_str()) {
            continue;
        }
        let (target_id, inverse_position) = parent
            .children
            .get(index + 1)
            .map(|next| (next.as_str(), ProjectMovePosition::Before))
            .unwrap_or((parent_id.as_str(), ProjectMovePosition::Inside));
        inverses.push(CanvasPatchOperation::Move {
            source: CanvasPatchAnchor::source(
                source_id,
                source_tag(&before_model, source_id).as_deref(),
            ),
            target: CanvasPatchAnchor::source(
                target_id,
                source_tag(&before_model, target_id).as_deref(),
            ),
            position: inverse_position,
        });
    }
    finish_batch_commit(
        session,
        workspace,
        &before_model,
        commit,
        source_ids,
        Some(forwards),
        Some(inverses),
        None,
    )
}

fn current_project_model(
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
) -> Result<ProjectModel, String> {
    let projection = workspace.capture_projection_snapshot()?;
    if workspace.project_model_source_revision == Some(projection.revision) {
        if let Some(model) = workspace.project_model.as_ref() {
            return Ok(model.clone());
        }
    }
    let model = rebuild_project_model_from_previous_projection(
        project_root,
        workspace.project_model.as_ref(),
        workspace.project_model_source_revision,
        &projection,
    )?;
    workspace.publish_project_model(&projection, model.clone())?;
    Ok(model)
}

fn selection_source_ids(selection: &SelectionMutationIdentity) -> Result<Vec<String>, String> {
    if selection.members.is_empty() || selection.members.len() > 256 {
        return Err("Executorul batch cere un set de selecție nevid și limitat.".to_string());
    }
    let mut seen = HashSet::with_capacity(selection.members.len());
    selection
        .members
        .iter()
        .map(|member| {
            let source_id = member
                .source_node_id
                .clone()
                .ok_or_else(|| format!("Membrul {} nu are SourceNodeId.", member.member_id))?;
            if !seen.insert(source_id.clone()) {
                return Err("Executorul batch a refuzat SourceNodeId-uri duplicate.".to_string());
            }
            Ok(source_id)
        })
        .collect()
}

fn require_plain_html_roots(graph: &SourceGraph, source_ids: &[String]) -> Result<(), String> {
    let selected = source_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    for source_id in source_ids {
        let node = graph
            .node_by_id(source_id)
            .ok_or_else(|| format!("SourceGraph nu conține membrul batch {source_id}."))?;
        if node.kind != SourceNodeKind::Html
            || node.origin != crate::source_graph::model::SourceOrigin::Local
        {
            return Err("Executorul batch v1 acceptă numai elemente HTML locale.".to_string());
        }
        let mut parent = node.parent.as_deref();
        while let Some(parent_id) = parent {
            if selected.contains(parent_id) {
                return Err("Executorul batch a refuzat relația strămoș–descendent.".to_string());
            }
            parent = graph
                .node_by_id(parent_id)
                .and_then(|parent| parent.parent.as_deref());
        }
        let mut subtree = HashSet::from([source_id.as_str()]);
        let mut pending = vec![source_id.as_str()];
        while let Some(current_id) = pending.pop() {
            let Some(current) = graph.node_by_id(current_id) else {
                continue;
            };
            for child in &current.children {
                if subtree.insert(child.as_str()) {
                    pending.push(child);
                }
            }
        }
        let contains_native_block = graph
            .block_graph
            .source_instances
            .iter()
            .any(|instance| subtree.contains(instance.source_node_id.as_str()));
        let contains_dynamic_widget =
            graph
                .dynamic_widget_graph
                .source_instances
                .iter()
                .any(|instance| {
                    instance
                        .root_source_node_ids
                        .iter()
                        .any(|root| subtree.contains(root.as_str()))
                });
        if contains_native_block || contains_dynamic_widget {
            return Err(
                "Executorul batch v1 a refuzat un contract structural specializat.".to_string(),
            );
        }
    }
    Ok(())
}

fn independent_edit(
    before_model: &ProjectModel,
    file: &str,
    after: &str,
    source_id: &str,
    inserted_offset: Option<usize>,
) -> Result<Option<IndependentEdit>, String> {
    let before = before_model
        .files
        .iter()
        .find(|candidate| candidate.relative_path == file)
        .ok_or_else(|| format!("Executorul batch nu găsește sursa {file}."))?;
    let change = SourceChangeSet::between(file, &before.contents, after);
    if change.edits.is_empty() {
        return Ok(None);
    }
    let [edit] = change.edits.as_slice() else {
        return Err(format!(
            "Executorul batch cere o singură editare independentă pentru {source_id}."
        ));
    };
    let replacement = after
        .get(edit.new_start..edit.new_end)
        .ok_or_else(|| "Executorul batch a calculat un replacement UTF-8 invalid.".to_string())?
        .to_string();
    let inserted_relative_start =
        match inserted_offset {
            Some(offset) => Some(offset.checked_sub(edit.new_start).ok_or_else(|| {
                "Offsetul duplicării batch precede editarea calculată.".to_string()
            })?),
            None => None,
        };
    Ok(Some(IndependentEdit {
        source_id: source_id.to_string(),
        file: file.to_string(),
        old_start: edit.old_start,
        old_end: edit.old_end,
        replacement,
        inserted_relative_start,
    }))
}

fn combine_edits(
    before_model: &ProjectModel,
    edits: Vec<IndependentEdit>,
) -> Result<Vec<CombinedFile>, String> {
    let mut grouped = BTreeMap::<String, Vec<IndependentEdit>>::new();
    for edit in edits {
        grouped.entry(edit.file.clone()).or_default().push(edit);
    }
    grouped
        .into_iter()
        .map(|(file, mut edits)| {
            let before = before_model
                .files
                .iter()
                .find(|candidate| candidate.relative_path == file)
                .ok_or_else(|| format!("Executorul batch nu găsește documentul {file}."))?;
            edits.sort_by(|left, right| {
                left.old_start
                    .cmp(&right.old_start)
                    .then_with(|| left.old_end.cmp(&right.old_end))
                    .then_with(|| left.source_id.cmp(&right.source_id))
            });
            for pair in edits.windows(2) {
                if pair[0].old_end > pair[1].old_start
                    || (pair[0].old_start == pair[1].old_start
                        && (pair[0].old_end > pair[0].old_start
                            || pair[1].old_end > pair[1].old_start))
                {
                    return Err("Executorul batch a refuzat editări sursă suprapuse.".to_string());
                }
            }
            let mut contents = before.contents.clone();
            for edit in edits.iter().rev() {
                contents.replace_range(edit.old_start..edit.old_end, &edit.replacement);
            }
            let mut delta = 0_i128;
            let mut text_edits = Vec::with_capacity(edits.len());
            let mut inserted_starts = HashMap::new();
            for edit in &edits {
                let new_start = usize::try_from(edit.old_start as i128 + delta).map_err(|_| {
                    "Executorul batch a depășit offseturile documentului.".to_string()
                })?;
                let new_end = new_start + edit.replacement.len();
                text_edits.push(SourceTextEdit {
                    old_start: edit.old_start,
                    old_end: edit.old_end,
                    new_start,
                    new_end,
                });
                if let Some(relative) = edit.inserted_relative_start {
                    inserted_starts.insert(edit.source_id.clone(), new_start + relative);
                }
                delta += edit.replacement.len() as i128
                    - edit.old_end.saturating_sub(edit.old_start) as i128;
            }
            Ok(CombinedFile {
                file,
                contents,
                text_edits,
                inserted_starts,
            })
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn commit_combined(
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    before_model: &ProjectModel,
    combined: &[CombinedFile],
    deletes: BTreeMap<String, Vec<String>>,
    duplicates: BTreeMap<String, Vec<(String, usize)>>,
    moves: BTreeMap<String, Vec<(String, String, ProjectMovePosition)>>,
    label: &str,
) -> Result<PreviewStructuralBatchWriteCommit, String> {
    let documents = combined
        .iter()
        .map(|file| WorkspaceDocumentMutation {
            relative_path: file.file.clone(),
            contents: file.contents.clone(),
        })
        .collect::<Vec<_>>();
    let mut source_changes = Vec::with_capacity(combined.len());
    for file in combined {
        let before = before_model
            .files
            .iter()
            .find(|candidate| candidate.relative_path == file.file)
            .ok_or_else(|| format!("Executorul batch nu găsește documentul {}.", file.file))?;
        let mut change = SourceChangeSet::between(&file.file, &before.contents, &file.contents)
            .with_exact_text_edits(file.text_edits.clone());
        if let Some(source_ids) = deletes.get(&file.file) {
            change = change.with_tree_delete_many(source_ids.clone());
        }
        if let Some(items) = duplicates.get(&file.file) {
            for (source_id, inserted_start) in items {
                change = change.with_tree_duplicate(source_id, *inserted_start);
            }
        }
        if let Some(items) = moves.get(&file.file) {
            for (source_id, target_id, position) in items {
                let position = match position {
                    ProjectMovePosition::Before => SourceTreeMovePosition::Before,
                    ProjectMovePosition::After => SourceTreeMovePosition::After,
                    ProjectMovePosition::Inside => SourceTreeMovePosition::Inside,
                };
                change = change.with_tree_move(source_id, target_id, position);
            }
        }
        source_changes.push(change);
    }
    stage_preview_structural_batch_write_in_transaction(
        project_root,
        workspace,
        PreviewStructuralBatchWrite {
            label: label.to_string(),
            documents,
            project_model_incremental_intent: ProjectModelIncrementalIntent::HtmlStructural,
            source_changes: Some(source_changes),
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn finish_batch_commit(
    session: &ProjectSessionSnapshot,
    workspace: &mut ProjectWorkspace,
    before_model: &ProjectModel,
    commit: PreviewStructuralBatchWriteCommit,
    affected_source_ids: Vec<String>,
    forwards: Option<Vec<CanvasPatchOperation>>,
    inverses: Option<Vec<CanvasPatchOperation>>,
    source_history: Option<(WorkspaceSourceTreeHistoryAction, Vec<SourceTreeIdentity>)>,
) -> Result<PreviewSelectionBatchExecutionOutcome, String> {
    let forward = forwards
        .filter(|operations| !operations.is_empty())
        .map(|operations| CanvasPatchOperation::Batch { operations });
    let inverse = inverses
        .filter(|operations| !operations.is_empty())
        .map(|operations| CanvasPatchOperation::Batch { operations });
    let canvas_patch = forward
        .clone()
        .map(|operation| {
            issue_canvas_patch(
                session,
                &commit.workspace_mutation,
                &before_model.revision,
                &commit.after_model.revision,
                operation,
            )
        })
        .transpose()?;
    if let Some(forward) = forward {
        attach_canvas_history_delta(
            workspace,
            &commit.workspace_mutation,
            &before_model.revision,
            &commit.after_model.revision,
            forward,
            inverse,
        )?;
    }
    if let Some((action, trees)) = source_history {
        attach_source_forest_history(workspace, &commit.workspace_mutation, action, trees)?;
    }
    let after_model_revision = commit.after_model.revision.clone();
    Ok(PreviewSelectionBatchExecutionOutcome {
        receipt: PreviewSelectionBatchExecutionReceipt {
            schema_version: PREVIEW_SELECTION_BATCH_EXECUTION_SCHEMA_VERSION,
            status: PreviewSelectionBatchExecutionStatus::Committed,
            model_revision: Some(after_model_revision),
            affected_source_ids,
            primary_affected_source_id: None,
            generated_class: None,
            canvas_patch,
            workspace_mutation: Some(commit.workspace_mutation),
            diagnostics: Vec::new(),
        },
        after_model: Some(commit.after_model),
    })
}

fn ids_by_file(
    graph: &SourceGraph,
    source_ids: &[String],
) -> Result<BTreeMap<String, Vec<String>>, String> {
    let mut result = BTreeMap::new();
    for source_id in source_ids {
        let node = graph
            .node_by_id(source_id)
            .ok_or_else(|| format!("SourceGraph nu conține {source_id}."))?;
        result
            .entry(node.file.clone())
            .or_insert_with(Vec::new)
            .push(source_id.clone());
    }
    Ok(result)
}

fn history_forests(
    graph: &SourceGraph,
    source_ids: &[String],
) -> Result<Vec<SourceTreeIdentity>, String> {
    let mut groups = BTreeMap::<(String, String), Vec<(usize, String)>>::new();
    for source_id in source_ids {
        let node = graph
            .node_by_id(source_id)
            .ok_or_else(|| format!("SourceGraph nu conține {source_id}."))?;
        let parent_id = node
            .parent
            .as_ref()
            .ok_or_else(|| "History batch a refuzat o rădăcină fără părinte.".to_string())?;
        let parent = graph
            .node_by_id(parent_id)
            .ok_or_else(|| format!("SourceGraph nu conține părintele {parent_id}."))?;
        let index = parent
            .children
            .iter()
            .position(|child| child == source_id)
            .ok_or_else(|| "History batch nu găsește membrul între frați.".to_string())?;
        groups
            .entry((node.file.clone(), parent_id.clone()))
            .or_default()
            .push((index, source_id.clone()));
    }
    let mut forests = Vec::new();
    for (_, mut roots) in groups {
        roots.sort_by_key(|(index, _)| *index);
        let mut run = Vec::new();
        let mut previous = None;
        for (index, source_id) in roots {
            if previous.is_some_and(|previous| index != previous + 1) && !run.is_empty() {
                forests.push(capture_source_forest_identity(graph, &run)?);
                run.clear();
            }
            run.push(source_id);
            previous = Some(index);
        }
        if !run.is_empty() {
            forests.push(capture_source_forest_identity(graph, &run)?);
        }
    }
    Ok(forests)
}

fn source_tag(model: &ProjectModel, source_id: &str) -> Option<String> {
    let node = model.source_graph.node_by_id(source_id)?;
    let range = node.range.as_ref()?;
    let file = model
        .files
        .iter()
        .find(|file| file.relative_path == node.file)?;
    parse_html_tag_at(&file.contents, range.start).map(|tag| tag.tag)
}

fn normalize_class_delta(
    add: Vec<String>,
    remove: Vec<String>,
) -> Result<(Vec<String>, Vec<String>), String> {
    if add.len() > 256 || remove.len() > 256 {
        return Err("Mutația batch de clase depășește limita de 256 de token-uri.".to_string());
    }
    let normalize = |values: Vec<String>| -> Result<Vec<String>, String> {
        let mut seen = HashSet::with_capacity(values.len());
        let mut normalized = Vec::with_capacity(values.len());
        for value in values {
            let value = value.trim().to_string();
            if !valid_batch_class_token(&value) {
                return Err(format!("Clasa batch «{value}» este invalidă."));
            }
            if seen.insert(value.clone()) {
                normalized.push(value);
            }
        }
        Ok(normalized)
    };
    let add = normalize(add)?;
    let remove = normalize(remove)?;
    let remove_set = remove.iter().map(String::as_str).collect::<HashSet<_>>();
    if add.iter().any(|value| remove_set.contains(value.as_str())) {
        return Err(
            "Aceeași clasă nu poate fi adăugată și eliminată într-o mutație batch.".to_string(),
        );
    }
    Ok((add, remove))
}

fn valid_batch_class_token(value: &str) -> bool {
    if value.is_empty() || value.len() > 128 {
        return false;
    }
    let mut characters = value.chars();
    characters.next().is_some_and(|first| {
        (first.is_ascii_alphabetic() || matches!(first, '_' | '-'))
            && characters.all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
            })
    })
}

fn class_delta_for_source(
    model: &ProjectModel,
    source_id: &str,
    add: &[String],
    remove: &[String],
) -> Result<Option<ProjectHtmlAttributeMutation>, String> {
    let node = model
        .source_graph
        .node_by_id(source_id)
        .ok_or_else(|| format!("SourceGraph nu conține membrul batch {source_id}."))?;
    let range = node
        .range
        .as_ref()
        .ok_or_else(|| format!("Membrul batch {source_id} nu are range stabil."))?;
    let file = model
        .files
        .iter()
        .find(|file| file.relative_path == node.file)
        .ok_or_else(|| format!("Executorul batch nu găsește documentul {}.", node.file))?;
    let parsed = parse_html_tag_at(&file.contents, range.start)
        .ok_or_else(|| format!("Membrul batch {source_id} nu mai indică un tag HTML."))?;
    let opening = file
        .contents
        .get(parsed.start..parsed.end)
        .ok_or_else(|| format!("Tag-ul membrului batch {source_id} are range UTF-8 invalid."))?;
    let current = raw_tag_attributes(opening)
        .into_iter()
        .find(|attribute| attribute.name == "class")
        .and_then(|attribute| attribute.value)
        .unwrap_or_default()
        .split_ascii_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let remove = remove.iter().map(String::as_str).collect::<HashSet<_>>();
    let mut next = current
        .iter()
        .filter(|token| !remove.contains(token.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    for token in add {
        if !next.iter().any(|existing| existing == token) {
            next.push(token.clone());
        }
    }
    if next == current {
        return Ok(None);
    }
    if next.is_empty() {
        Ok(Some(ProjectHtmlAttributeMutation::RemoveAttribute {
            name: "class".to_string(),
        }))
    } else {
        Ok(Some(ProjectHtmlAttributeMutation::SetAttribute {
            name: "class".to_string(),
            value: next.join(" "),
        }))
    }
}

fn previous_attribute_values<'a>(
    before_model: &ProjectModel,
    source_id: &str,
    names: impl Iterator<Item = &'a String>,
) -> Option<BTreeMap<String, Option<String>>> {
    let node = before_model.source_graph.node_by_id(source_id)?;
    let range = node.range.as_ref()?;
    let file = before_model
        .files
        .iter()
        .find(|file| file.relative_path == node.file)?;
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

fn inverse_delete_operations(
    before_model: &ProjectModel,
    after_model: &ProjectModel,
    source_ids: &[String],
) -> Result<Vec<CanvasPatchOperation>, String> {
    let selected = source_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut roots = source_ids
        .iter()
        .map(|source_id| {
            let node = before_model
                .source_graph
                .node_by_id(source_id)
                .ok_or_else(|| format!("SourceGraph nu conține {source_id}."))?;
            let range = node
                .range
                .as_ref()
                .ok_or_else(|| format!("SourceNodeId {source_id} nu are range."))?;
            Ok((node.file.clone(), range.start, source_id.clone()))
        })
        .collect::<Result<Vec<_>, String>>()?;
    roots.sort();
    roots
        .into_iter()
        .map(|(_, _, source_id)| {
            let node = before_model.source_graph.node_by_id(&source_id).unwrap();
            let range = node.range.as_ref().unwrap();
            let file = before_model
                .files
                .iter()
                .find(|file| file.relative_path == node.file)
                .ok_or_else(|| format!("History batch nu găsește {}.", node.file))?;
            let html = file
                .contents
                .get(range.start..range.end)
                .ok_or_else(|| "History batch a calculat HTML invalid.".to_string())?
                .to_string();
            let parent_id = node
                .parent
                .as_ref()
                .ok_or_else(|| "History batch a refuzat rădăcina documentului.".to_string())?;
            let parent = before_model.source_graph.node_by_id(parent_id).unwrap();
            let index = parent
                .children
                .iter()
                .position(|child| child == &source_id)
                .unwrap();
            let next_survivor = parent.children[index + 1..]
                .iter()
                .find(|candidate| !selected.contains(candidate.as_str()));
            let (target_id, position) = next_survivor
                .map(|target| {
                    (
                        target.as_str(),
                        crate::project_model::move_engine::ProjectMovePosition::Before,
                    )
                })
                .unwrap_or((
                    parent_id.as_str(),
                    crate::project_model::move_engine::ProjectMovePosition::Inside,
                ));
            if after_model.source_graph.node_by_id(target_id).is_none() {
                return Err("History batch nu poate ancora restaurarea DOM.".to_string());
            }
            Ok(CanvasPatchOperation::Insert {
                target: CanvasPatchAnchor::source(target_id, None),
                position,
                html,
                inserted: Some(CanvasPatchAnchor::source(
                    &source_id,
                    source_tag(before_model, &source_id).as_deref(),
                )),
            })
        })
        .collect()
}

fn inserted_duplicate_roots<'a>(
    after_model: &'a ProjectModel,
    combined: &[CombinedFile],
) -> Result<Vec<&'a SourceNode>, String> {
    combined
        .iter()
        .flat_map(|file| {
            file.inserted_starts.values().map(move |start| {
                let candidates = after_model
                    .source_graph
                    .nodes
                    .iter()
                    .filter(|node| {
                        node.file == file.file
                            && node.kind == SourceNodeKind::Html
                            && node
                                .range
                                .as_ref()
                                .is_some_and(|range| range.start == *start)
                    })
                    .collect::<Vec<_>>();
                match candidates.as_slice() {
                    [node] => Ok(*node),
                    [] => Err(format!(
                        "Duplicarea batch nu găsește rădăcina la offset {start}."
                    )),
                    _ => Err(format!(
                        "Duplicarea batch are rădăcini ambigue la offset {start}."
                    )),
                }
            })
        })
        .collect()
}

fn committed_noop(
    model: &ProjectModel,
    affected_source_ids: Vec<String>,
) -> PreviewSelectionBatchExecutionOutcome {
    blocked_outcome(
        Some(model.revision.clone()),
        affected_source_ids,
        "Mutația batch nu produce modificări.",
    )
}

fn blocked_outcome(
    model_revision: Option<String>,
    affected_source_ids: Vec<String>,
    diagnostic: &str,
) -> PreviewSelectionBatchExecutionOutcome {
    PreviewSelectionBatchExecutionOutcome {
        receipt: blocked_receipt(model_revision, affected_source_ids, diagnostic),
        after_model: None,
    }
}

fn blocked_receipt(
    model_revision: Option<String>,
    affected_source_ids: Vec<String>,
    diagnostic: &str,
) -> PreviewSelectionBatchExecutionReceipt {
    PreviewSelectionBatchExecutionReceipt {
        schema_version: PREVIEW_SELECTION_BATCH_EXECUTION_SCHEMA_VERSION,
        status: PreviewSelectionBatchExecutionStatus::Blocked,
        model_revision,
        affected_source_ids,
        primary_affected_source_id: None,
        generated_class: None,
        canvas_patch: None,
        workspace_mutation: None,
        diagnostics: vec![diagnostic.to_string()],
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_class_delta, selection_source_ids};
    use crate::kernel::selection_coordinator::{
        SelectionMutationIdentity, SelectionMutationMemberIdentity,
    };

    #[test]
    fn class_delta_is_deduplicated_and_conflicts_fail_closed() {
        let (add, remove) = normalize_class_delta(
            vec!["shared".to_string(), "shared".to_string()],
            vec!["obsolete".to_string()],
        )
        .unwrap();
        assert_eq!(add, vec!["shared"]);
        assert_eq!(remove, vec!["obsolete"]);
        assert!(
            normalize_class_delta(vec!["same".to_string()], vec!["same".to_string()],).is_err()
        );
        assert!(normalize_class_delta(vec!["invalid class".to_string()], Vec::new()).is_err());
    }

    #[test]
    fn repeated_render_instances_of_one_source_are_rejected_before_planning() {
        let identity = SelectionMutationIdentity {
            selection_revision: 7,
            workspace_revision: 11,
            primary_member_id: Some("render:first".to_string()),
            members: vec![
                SelectionMutationMemberIdentity {
                    member_id: "render:first".to_string(),
                    editor_node_id: Some("editor_render:first".to_string()),
                    source_node_id: Some("source:shared".to_string()),
                    render_instance_id: Some("render:first".to_string()),
                },
                SelectionMutationMemberIdentity {
                    member_id: "render:second".to_string(),
                    editor_node_id: Some("editor_render:second".to_string()),
                    source_node_id: Some("source:shared".to_string()),
                    render_instance_id: Some("render:second".to_string()),
                },
            ],
        };

        let error = selection_source_ids(&identity).unwrap_err();
        assert!(error.contains("SourceNodeId-uri duplicate"));
    }
}
