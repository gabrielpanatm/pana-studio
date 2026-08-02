use tauri::{AppHandle, Runtime, State};

use super::{
    kernel_preview_context::{
        prepare_preview_write_command, with_preview_write_workspace, PreviewWriteCommandContext,
    },
    kernel_preview_outcome::{
        finalize_preview_structural_outcome, PreviewStructuralCommandOutcome,
    },
};

use crate::{
    kernel::{
        preview_projection::PreviewStructuralCommandIdentity,
        project_workspace::{
            commit_project_workspace_session_mutation_with_projection_measured, ProjectWorkspace,
            ProjectWorkspaceCommitTimings, ProjectWorkspacePreviewProjection,
        },
    },
    state::AppState,
};

/// Runs an HTML/Tera structural mutation against exactly one workspace
/// revision. Planning, staging, ProjectModel publication and source-alias
/// publication happen while the same Rust authority is locked.
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
    execute: impl FnOnce(&PreviewWriteCommandContext, &mut ProjectWorkspace) -> Result<O, String>,
) -> Result<(O::Receipt, ProjectWorkspaceCommitTimings), String>
where
    R: Runtime,
    O: PreviewStructuralCommandOutcome,
{
    let context = prepare_preview_write_command(state, identity)?;
    let commit = || {
        with_preview_write_workspace(state, &context, |workspace| {
            commit_project_workspace_session_mutation_with_projection_measured(
                app,
                workspace,
                preview_projection,
                |candidate| {
                    let mut outcome = execute(&context, candidate)?;
                    if outcome.command_succeeded() {
                        let publication = outcome.workspace_mutation().ok_or_else(|| {
                            format!(
                                "{operation_label} a produs o mutație fără receipt-ul tranzacției ProjectWorkspace."
                            )
                        })?;
                        let publication_revision_before = publication.revision_before;
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
                        let alias_updates = outcome.take_alias_updates();
                        let project_root = candidate.session.project_root.clone();
                        let runtime_session_id = candidate.runtime_session_id();
                        candidate.publish_project_model_for_transaction(
                            &project_root,
                            &runtime_session_id,
                            publication_revision,
                            &publication_transaction_id,
                            after_model,
                        )?;
                        candidate.publish_source_identity_alias_transition(
                            publication_revision_before,
                            publication_revision,
                            alias_updates,
                        )?;
                    } else if outcome.after_model_mut().is_some()
                        || !outcome.take_alias_updates().is_empty()
                    {
                        return Err(format!(
                            "{operation_label} blocat a încercat să publice stare derivată mutabilă."
                        ));
                    }
                    Ok(outcome)
                },
            )
        })
    };
    let outcome = if let Some(expected) = identity.expected_selection.as_ref() {
        state.selection_coordinator.with_mutation_target(
            &context.session.runtime_instance_id(),
            expected.selection_revision,
            expected.editor_node_id.as_deref(),
            expected.source_node_id.as_deref(),
            expected.render_instance_id.as_deref(),
            commit,
        )
    } else {
        commit()
    };

    let (outcome, timings) = outcome?;
    let receipt = finalize_preview_structural_outcome(Ok(outcome))?;
    Ok((receipt, timings))
}
