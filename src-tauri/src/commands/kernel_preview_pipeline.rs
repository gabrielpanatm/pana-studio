use std::time::Instant;

use tauri::{AppHandle, Runtime, State};

use super::{
    kernel_preview_context::{
        capture_preview_write_workspace_candidate, prepare_preview_write_command,
        with_preview_write_workspace_cas, PreviewWriteCommandContext,
    },
    kernel_preview_outcome::{
        finalize_preview_structural_outcome, PreviewStructuralCommandOutcome,
    },
};

use crate::{
    kernel::{
        observability::append_events,
        performance::{
            elapsed_us, performance_event, project_model_performance_event,
            with_project_model_sample,
        },
        preview_projection::PreviewStructuralCommandIdentity,
        project_workspace::{
            publish_prepared_project_workspace_candidate, ProjectWorkspace,
            ProjectWorkspaceCommitTimings, ProjectWorkspacePreviewProjection,
        },
    },
    state::AppState,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PreviewStructuralSelectionEffect {
    Preserve,
    ClearCommittedTarget,
    ReplaceCommittedTarget,
}

pub(super) fn require_preview_structural_target_matches_selection(
    identity: &PreviewStructuralCommandIdentity,
    target_source_id: Option<&str>,
    target_render_instance_id: Option<&str>,
    operation_label: &str,
) -> Result<(), String> {
    let expected = identity.expected_selection.as_ref().ok_or_else(|| {
        format!(
            "{operation_label} cere o selecție semantică Rust rezolvată pentru ținta explicită."
        )
    })?;
    let target_source_id = target_source_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let target_render_instance_id = target_render_instance_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let primary = expected
        .primary_member_id
        .as_deref()
        .and_then(|member_id| {
            expected
                .members
                .iter()
                .find(|member| member.member_id == member_id)
        })
        .ok_or_else(|| {
            format!("{operation_label} a fost blocată: tokenul nu are membru primar.")
        })?;
    if target_source_id.is_none() && target_render_instance_id.is_none() {
        return Err(format!(
            "{operation_label} a fost blocată: ținta explicită nu are identitate Source Graph sau render."
        ));
    }
    if target_source_id
        .is_some_and(|target| primary.source_node_id.as_deref().map(str::trim) != Some(target))
    {
        return Err(format!(
            "{operation_label} a fost anulată deoarece ținta Source Graph nu corespunde selecției Rust confirmate."
        ));
    }
    if target_render_instance_id
        .is_some_and(|target| primary.render_instance_id.as_deref().map(str::trim) != Some(target))
    {
        return Err(format!(
            "{operation_label} a fost anulată deoarece instanța randată nu corespunde selecției Rust confirmate."
        ));
    }
    Ok(())
}

/// Runs an HTML/Tera structural mutation against exactly one workspace
/// revision. Planning and ProjectModel construction happen on a detached
/// candidate; the live authority is held only for final revision CAS.
pub(super) fn run_preview_structural_write_command<R, O>(
    app: &AppHandle<R>,
    state: &State<AppState>,
    identity: &PreviewStructuralCommandIdentity,
    operation_label: &str,
    execute: impl FnOnce(&PreviewWriteCommandContext, &mut ProjectWorkspace) -> Result<O, String>,
) -> Result<O::Receipt, String>
where
    R: Runtime,
    O: PreviewStructuralCommandOutcome,
{
    run_preview_structural_write_command_measured(app, state, identity, operation_label, execute)
        .map(|(receipt, _)| receipt)
}

pub(super) fn run_preview_structural_delete_write_command<R, O>(
    app: &AppHandle<R>,
    state: &State<AppState>,
    identity: &PreviewStructuralCommandIdentity,
    operation_label: &str,
    execute: impl FnOnce(&PreviewWriteCommandContext, &mut ProjectWorkspace) -> Result<O, String>,
) -> Result<O::Receipt, String>
where
    R: Runtime,
    O: PreviewStructuralCommandOutcome,
{
    run_preview_structural_write_command_with_projection_measured(
        app,
        state,
        identity,
        operation_label,
        ProjectWorkspacePreviewProjection::Required,
        PreviewStructuralSelectionEffect::ClearCommittedTarget,
        execute,
    )
    .map(|(receipt, _)| receipt)
}

pub(super) fn run_preview_structural_replace_write_command<R, O>(
    app: &AppHandle<R>,
    state: &State<AppState>,
    identity: &PreviewStructuralCommandIdentity,
    operation_label: &str,
    execute: impl FnOnce(&PreviewWriteCommandContext, &mut ProjectWorkspace) -> Result<O, String>,
) -> Result<O::Receipt, String>
where
    R: Runtime,
    O: PreviewStructuralCommandOutcome,
{
    run_preview_structural_write_command_with_projection_measured(
        app,
        state,
        identity,
        operation_label,
        ProjectWorkspacePreviewProjection::Required,
        PreviewStructuralSelectionEffect::ReplaceCommittedTarget,
        execute,
    )
    .map(|(receipt, _)| receipt)
}

pub(super) fn run_preview_structural_write_command_measured<R, O>(
    app: &AppHandle<R>,
    state: &State<AppState>,
    identity: &PreviewStructuralCommandIdentity,
    operation_label: &str,
    execute: impl FnOnce(&PreviewWriteCommandContext, &mut ProjectWorkspace) -> Result<O, String>,
) -> Result<(O::Receipt, ProjectWorkspaceCommitTimings), String>
where
    R: Runtime,
    O: PreviewStructuralCommandOutcome,
{
    run_preview_structural_write_command_with_projection_measured(
        app,
        state,
        identity,
        operation_label,
        ProjectWorkspacePreviewProjection::Required,
        PreviewStructuralSelectionEffect::Preserve,
        execute,
    )
}

pub(super) fn run_preview_structural_write_command_with_projection<R, O>(
    app: &AppHandle<R>,
    state: &State<AppState>,
    identity: &PreviewStructuralCommandIdentity,
    operation_label: &str,
    preview_projection: ProjectWorkspacePreviewProjection,
    execute: impl FnOnce(&PreviewWriteCommandContext, &mut ProjectWorkspace) -> Result<O, String>,
) -> Result<O::Receipt, String>
where
    R: Runtime,
    O: PreviewStructuralCommandOutcome,
{
    run_preview_structural_write_command_with_projection_measured(
        app,
        state,
        identity,
        operation_label,
        preview_projection,
        PreviewStructuralSelectionEffect::Preserve,
        execute,
    )
    .map(|(receipt, _)| receipt)
}

fn run_preview_structural_write_command_with_projection_measured<R, O>(
    app: &AppHandle<R>,
    state: &State<AppState>,
    identity: &PreviewStructuralCommandIdentity,
    operation_label: &str,
    preview_projection: ProjectWorkspacePreviewProjection,
    selection_effect: PreviewStructuralSelectionEffect,
    execute: impl FnOnce(&PreviewWriteCommandContext, &mut ProjectWorkspace) -> Result<O, String>,
) -> Result<(O::Receipt, ProjectWorkspaceCommitTimings), String>
where
    R: Runtime,
    O: PreviewStructuralCommandOutcome,
{
    let total_started = Instant::now();
    let context = prepare_preview_write_command(state, identity)?;
    let commit = || {
        state
            .ai_coordination
            .require_user_source_mutation()
            .map_err(|error| error.to_string())?;
        let (mut candidate, candidate_timings) =
            capture_preview_write_workspace_candidate(state, &context)?;
        let mutation_started = Instant::now();
        let mut outcome = execute(&context, &mut candidate)?;
        let mut selection_replacement = None;
        if outcome.command_succeeded() {
            let publication = outcome.workspace_mutation().ok_or_else(|| {
                format!(
                    "{operation_label} a produs o mutație fără receipt-ul tranzacției ProjectWorkspace."
                )
            })?;
            let publication_revision = publication.revision_after;
            let publication_transaction_id = publication
                .transaction_id
                .clone()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    format!("{operation_label} a produs o mutație fără transaction ID.")
                })?;
            let after_model = outcome.after_model_mut().take().ok_or_else(|| {
                format!(
                    "{operation_label} a produs o mutație fără ProjectModel pentru revizia rezultată."
                )
            })?;
            if selection_effect == PreviewStructuralSelectionEffect::ReplaceCommittedTarget {
                let source_ids = outcome
                    .selection_replacement_source_ids()
                    .filter(|source_ids| !source_ids.is_empty())
                    .ok_or_else(|| {
                        format!(
                            "{operation_label} nu a returnat copiile pentru înlocuirea selecției."
                        )
                    })?
                    .to_vec();
                selection_replacement = Some((
                    after_model.source_graph.clone(),
                    source_ids,
                    outcome
                        .selection_replacement_primary_source_id()
                        .map(str::to_string),
                ));
            }
            let project_root = candidate.session.project_root.clone();
            let runtime_session_id = candidate.runtime_session_id();
            candidate.publish_project_model_for_transaction(
                &project_root,
                &runtime_session_id,
                publication_revision,
                &publication_transaction_id,
                after_model,
            )?;
        } else if outcome.after_model_mut().is_some() {
            return Err(format!(
                "{operation_label} blocat a încercat să publice stare derivată mutabilă."
            ));
        }
        let mutation_ms = mutation_started.elapsed().as_millis().min(u64::MAX as u128) as u64;
        let (timings, cas_timings) =
            with_preview_write_workspace_cas(state, &context, |live_workspace| {
                publish_prepared_project_workspace_candidate(
                    app,
                    live_workspace,
                    context.workspace_revision,
                    candidate,
                    preview_projection,
                    candidate_timings.candidate_clone_us / 1_000,
                    mutation_ms,
                    total_started,
                )
            })?;
        Ok((
            outcome,
            timings,
            selection_replacement,
            candidate_timings,
            cas_timings,
        ))
    };
    let outcome = if let Some(expected) = identity.expected_selection.as_ref() {
        state.selection_coordinator.with_mutation_target(
            &context.session.runtime_instance_id(),
            expected,
            commit,
        )
    } else {
        commit()
    };

    let (outcome, timings, selection_replacement, candidate_timings, cas_timings) = outcome?;
    if selection_effect == PreviewStructuralSelectionEffect::ClearCommittedTarget
        && outcome.command_succeeded()
    {
        if let Some(expected) = identity.expected_selection.as_ref() {
            // The workspace commit is already authoritative. Clearing is a
            // compare-and-swap projection: a newer selection must win, while
            // a poisoned coordinator must not turn a commit into a false
            // failure receipt.
            let _ = state
                .selection_coordinator
                .clear_mutation_target_if_current(&context.session.runtime_instance_id(), expected);
        }
    }
    if selection_effect == PreviewStructuralSelectionEffect::ReplaceCommittedTarget
        && outcome.command_succeeded()
    {
        if let (Some(expected), Some((source_graph, source_ids, primary_source_id))) =
            (identity.expected_selection.as_ref(), selection_replacement)
        {
            // The committed workspace remains authoritative if selection
            // projection is concurrently superseded or the coordinator is
            // unavailable. A newer user selection always wins this CAS.
            let _ = state
                .selection_coordinator
                .replace_mutation_target_with_sources_if_current(
                    &context.session.runtime_instance_id(),
                    expected,
                    &source_graph,
                    &source_ids,
                    primary_source_id.as_deref(),
                );
        }
    }
    if outcome.command_succeeded() {
        let project_model_sample = outcome
            .workspace_mutation()
            .and_then(|mutation| mutation.project_model_performance.as_ref());
        let event = performance_event(
            "project_workspace",
            "performance",
            "html_edit",
            operation_label,
            Some(context.session.runtime_instance_id()),
            elapsed_us(total_started),
        )
        .with_attribute("currentRootLockWaitUs", context.current_root_lock_wait_us)
        .with_attribute(
            "projectWorkspaceAuthorityLockWaitUs",
            context.project_workspace_lock_wait_us,
        )
        .with_attribute("authorityLocksHeldUs", context.authority_locks_held_us)
        .with_attribute(
            "projectWorkspaceCandidateLockWaitUs",
            candidate_timings.project_workspace_lock_wait_us,
        )
        .with_attribute(
            "projectWorkspaceCandidateLockHeldUs",
            candidate_timings.project_workspace_lock_held_us,
        )
        .with_attribute("candidateCloneUs", candidate_timings.candidate_clone_us)
        .with_attribute("mutationUs", timings.mutation_us)
        .with_attribute("recoveryPersistUs", timings.recovery_persist_us)
        .with_attribute("authorityPublishUs", timings.authority_publish_us)
        .with_attribute(
            "projectWorkspaceCasLockWaitUs",
            cas_timings.project_workspace_lock_wait_us,
        )
        .with_attribute(
            "projectWorkspaceCasLockHeldUs",
            cas_timings.project_workspace_lock_held_us,
        );
        let mut events = vec![with_project_model_sample(event, project_model_sample)];
        if let Some(sample) = project_model_sample {
            events.push(project_model_performance_event(
                "project_workspace",
                Some(context.session.runtime_instance_id()),
                sample,
            ));
        }
        let _ = append_events(app, events);
    }
    let receipt = finalize_preview_structural_outcome(Ok(outcome))?;
    Ok((receipt, timings))
}

#[cfg(test)]
mod tests {
    use crate::kernel::{
        preview_projection::PreviewStructuralCommandIdentity,
        selection_coordinator::SelectionMutationIdentity,
    };

    use super::require_preview_structural_target_matches_selection;

    fn identity(
        source_node_id: Option<&str>,
        render_instance_id: Option<&str>,
    ) -> PreviewStructuralCommandIdentity {
        PreviewStructuralCommandIdentity {
            expected_project_root: "/project".to_string(),
            expected_session_id: "session-a".to_string(),
            expected_selection: Some(SelectionMutationIdentity {
                selection_revision: 7,
                workspace_revision: 11,
                primary_member_id: Some("editor:target".to_string()),
                members: vec![
                    crate::kernel::selection_coordinator::SelectionMutationMemberIdentity {
                        member_id: "editor:target".to_string(),
                        editor_node_id: Some("editor:target".to_string()),
                        source_node_id: source_node_id.map(str::to_string),
                        render_instance_id: render_instance_id.map(str::to_string),
                    },
                ],
            }),
        }
    }

    #[test]
    fn destructive_target_must_match_the_rust_selection() {
        let selected = identity(Some("source:section"), Some("render:section"));
        require_preview_structural_target_matches_selection(
            &selected,
            Some("source:section"),
            Some("render:section"),
            "Preview HTML delete",
        )
        .unwrap();

        let error = require_preview_structural_target_matches_selection(
            &selected,
            Some("source:sibling"),
            None,
            "Preview HTML delete",
        )
        .unwrap_err();
        assert!(error.contains("nu corespunde selecției Rust"));
    }

    #[test]
    fn destructive_target_requires_a_selection_and_a_stable_coordinate() {
        let missing_selection = PreviewStructuralCommandIdentity {
            expected_project_root: "/project".to_string(),
            expected_session_id: "session-a".to_string(),
            expected_selection: None,
        };
        assert!(require_preview_structural_target_matches_selection(
            &missing_selection,
            Some("source:section"),
            None,
            "Preview HTML delete",
        )
        .is_err());
        assert!(require_preview_structural_target_matches_selection(
            &identity(None, None),
            None,
            None,
            "Preview HTML delete",
        )
        .is_err());
    }
}
