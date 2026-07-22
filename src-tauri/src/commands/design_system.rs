use serde::Serialize;
use tauri::{AppHandle, State};

use crate::{
    commands::workspace_entries::{
        current_workspace_identity, finish_mutation, mutation_metadata, require_bound_workspace,
        WorkspaceEntryMutationReceipt,
    },
    kernel::{
        design_system::{
            build_design_class_inventory, plan_design_class_rename, DesignClassInventorySnapshot,
        },
        file_buffer_store::FileBufferRequestIdentity,
        observability::now_ms,
        project_workspace::WorkspaceResourceMutation,
    },
    project_model::cache::{capture_project_model_build_lease, publish_project_model_if_current},
    state::AppState,
};

pub const DESIGN_CLASS_RENAME_SCHEMA_VERSION: u32 = 1;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignClassRenameReceipt {
    pub schema_version: u32,
    pub old_name: String,
    pub new_name: String,
    pub changed_files: Vec<String>,
    pub replacement_count: usize,
    pub workspace: WorkspaceEntryMutationReceipt,
}

#[tauri::command]
pub fn read_design_class_inventory(
    state: State<AppState>,
) -> Result<DesignClassInventorySnapshot, String> {
    let (root, session, lease) = capture_project_model_build_lease(&state)?;
    let model = crate::project_model::build_project_model_from_workspace_projection(
        &root,
        lease.projection(),
    )?;
    let snapshot = build_design_class_inventory(
        &model,
        session.runtime_instance_id(),
        lease.projection().revision,
    );
    publish_project_model_if_current(&state, &lease, model)?;
    Ok(snapshot)
}

#[tauri::command(async)]
pub fn rename_design_class(
    old_name: String,
    new_name: String,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<DesignClassRenameReceipt, String> {
    let (root, _session, lease) = capture_project_model_build_lease(&state)?;
    if identity.expected_project_root != lease.projection().project_root
        || identity.expected_session_id != lease.projection().runtime_session_id
    {
        return Err("Rename clasă a refuzat un request pentru alt ProjectSession.".to_string());
    }
    let model = crate::project_model::build_project_model_from_workspace_projection(
        &root,
        lease.projection(),
    )?;
    let plan = plan_design_class_rename(&model, &old_name, &new_name)?;
    let changed_files = plan
        .changes
        .iter()
        .map(|change| change.relative_path.clone())
        .collect::<Vec<_>>();
    let mutations = plan
        .changes
        .iter()
        .map(|change| WorkspaceResourceMutation {
            relative_path: change.relative_path.clone(),
            contents: change.contents.clone(),
            create_only: false,
        })
        .collect::<Vec<_>>();
    let (_root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = slot
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru rename clasă.".to_string())?;
    if workspace.revision != lease.projection().revision {
        return Err(
            "Rename clasă a fost anulat deoarece ProjectWorkspace s-a schimbat după analiză."
                .to_string(),
        );
    }
    let receipt = finish_mutation(&app, workspace, None, |candidate| {
        candidate.stage_resource_texts(
            &current_workspace_identity(candidate),
            mutation_metadata("Redenumire clasă", "design_system.rename_class"),
            mutations,
            now_ms(),
        )
    })?;
    Ok(DesignClassRenameReceipt {
        schema_version: DESIGN_CLASS_RENAME_SCHEMA_VERSION,
        old_name: plan.old_name,
        new_name: plan.new_name,
        changed_files,
        replacement_count: plan.replacement_count,
        workspace: receipt,
    })
}
