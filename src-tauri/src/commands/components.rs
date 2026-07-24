use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime};

use crate::{
    commands::workspace_entries::{
        require_bound_workspace, WorkspaceEntryMutationReceipt,
        WORKSPACE_ENTRY_MUTATION_SCHEMA_VERSION,
    },
    kernel::{
        component_mutation::{
            stage_validated_component_mutation, ComponentMutationInput, ComponentMutationPlan,
        },
        file_buffer_store::FileBufferRequestIdentity,
        observability::now_ms,
        project_workspace::commit_project_workspace_session_mutation,
    },
    state::AppState,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentMutationApplyReceipt {
    pub plan: ComponentMutationPlan,
    pub workspace: WorkspaceEntryMutationReceipt,
}

#[tauri::command]
pub async fn apply_component_mutation(
    input: ComponentMutationInput,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
) -> Result<ComponentMutationApplyReceipt, String> {
    apply_component_mutation_task(input, identity, app).await
}

pub(super) async fn apply_component_mutation_task<R: Runtime>(
    input: ComponentMutationInput,
    identity: FileBufferRequestIdentity,
    app: AppHandle<R>,
) -> Result<ComponentMutationApplyReceipt, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        apply_component_mutation_blocking(&app, state.inner(), input, identity)
    })
    .await
    .map_err(|error| format!("Mutația componentei a căzut în task-ul de fundal: {error}"))?
}

fn apply_component_mutation_blocking<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    input: ComponentMutationInput,
    identity: FileBufferRequestIdentity,
) -> Result<ComponentMutationApplyReceipt, String> {
    let (root, mut slot) = require_bound_workspace(state, &identity)?;
    let workspace = slot
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru componente.".to_string())?;
    let (plan, mutation) =
        commit_project_workspace_session_mutation(app, workspace, |candidate| {
            stage_validated_component_mutation(&root, candidate, input, now_ms())
        })?;
    let relative_path = plan
        .destination_relative_path
        .clone()
        .or_else(|| plan.source_relative_path.clone());
    let workspace_receipt = WorkspaceEntryMutationReceipt {
        schema_version: WORKSPACE_ENTRY_MUTATION_SCHEMA_VERSION,
        project_root: workspace.session.project_root.clone(),
        runtime_session_id: workspace.runtime_session_id(),
        relative_path,
        mutation,
        workspace: workspace.snapshot(),
    };
    Ok(ComponentMutationApplyReceipt {
        plan,
        workspace: workspace_receipt,
    })
}
