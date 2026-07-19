use tauri::{AppHandle, Runtime, State};

use super::{
    kernel_preview_context::{
        prepare_preview_read_command, prepare_preview_write_command, with_preview_read_store,
        with_preview_write_workspace, PreviewReadCommandContext, PreviewWriteCommandContext,
    },
    kernel_preview_outcome::{
        finalize_preview_structural_outcome, PreviewStructuralCommandOutcome,
    },
};

use crate::{
    kernel::{
        file_buffer_store::FileBufferStore,
        preview_projection::PreviewStructuralCommandIdentity,
        project_workspace::{
            commit_project_workspace_session_mutation_with_projection, ProjectWorkspace,
            ProjectWorkspacePreviewProjection,
        },
    },
    state::AppState,
};

pub(super) fn run_preview_read_command<R>(
    state: &State<AppState>,
    identity: &PreviewStructuralCommandIdentity,
    execute: impl FnOnce(&PreviewReadCommandContext, &FileBufferStore) -> Result<R, String>,
) -> Result<R, String> {
    let context = prepare_preview_read_command(state, identity)?;
    with_preview_read_store(state, &context, |store| execute(&context, store))
}

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
    run_preview_structural_write_command_with_projection(
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
    let context = prepare_preview_write_command(state, identity)?;
    let outcome = with_preview_write_workspace(state, &context, |workspace| {
        commit_project_workspace_session_mutation_with_projection(
            app,
            workspace,
            preview_projection,
            |candidate| {
                let mut outcome = execute(&context, candidate)?;
                if outcome.command_succeeded() {
                    let after_model = outcome.after_model_mut().take().ok_or_else(|| {
                        format!(
                            "{operation_label} a produs o mutație fără ProjectModel pentru revizia rezultată."
                        )
                    })?;
                    let alias_updates = outcome.take_alias_updates();
                    let lease = candidate.capture_projection_lease()?;
                    candidate.publish_project_model(&lease, after_model)?;
                    candidate.source_identity_aliases.extend(alias_updates);
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
    });

    finalize_preview_structural_outcome(outcome)
}
