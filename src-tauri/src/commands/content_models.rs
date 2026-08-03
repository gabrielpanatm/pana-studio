use serde::Serialize;
use tauri::{AppHandle, State};

use crate::{
    commands::workspace_entries::{
        require_bound_workspace, WorkspaceEntryMutationReceipt,
        WORKSPACE_ENTRY_MUTATION_SCHEMA_VERSION,
    },
    kernel::{
        content_models::{
            plan_content_model_mutation as build_mutation_plan, stage_content_model_mutation,
            ContentModelCatalog, ContentModelMutationInput, ContentModelMutationPlan,
            PlannedContentModelMutation,
        },
        file_buffer_store::{FileBufferCommandReceipt, FileBufferRequestIdentity},
        observability::now_ms,
        project_workspace::{commit_project_workspace_session_mutation, ProjectWorkspace},
    },
    source_graph::build_source_graph_from_workspace_projection,
    state::AppState,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentModelMutationApplyReceipt {
    pub plan: ContentModelMutationPlan,
    pub workspace: WorkspaceEntryMutationReceipt,
}

#[tauri::command(async)]
pub fn read_content_model_catalog(
    identity: FileBufferRequestIdentity,
    state: State<AppState>,
) -> Result<FileBufferCommandReceipt<ContentModelCatalog>, String> {
    let (root, slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = slot.as_ref().ok_or_else(|| {
        "ProjectWorkspace nu este inițializat pentru modelele de conținut.".to_string()
    })?;
    let projection = workspace.capture_projection_snapshot()?;
    let graph = build_source_graph_from_workspace_projection(&root, &projection)?;
    let catalog = graph.content_models;
    Ok(FileBufferCommandReceipt::new(
        &workspace.session,
        workspace.revision,
        catalog,
    ))
}

#[tauri::command(async)]
pub fn plan_content_model_mutation(
    input: ContentModelMutationInput,
    identity: FileBufferRequestIdentity,
    state: State<AppState>,
) -> Result<ContentModelMutationPlan, String> {
    let (root, slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = slot.as_ref().ok_or_else(|| {
        "ProjectWorkspace nu este inițializat pentru modelele de conținut.".to_string()
    })?;
    Ok(plan_for_workspace(&root, workspace, &input)?.plan)
}

#[tauri::command(async)]
pub fn apply_content_model_mutation(
    input: ContentModelMutationInput,
    expected_plan_id: String,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<ContentModelMutationApplyReceipt, String> {
    let (root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = slot.as_mut().ok_or_else(|| {
        "ProjectWorkspace nu este inițializat pentru modelele de conținut.".to_string()
    })?;
    let planned = plan_for_workspace(&root, workspace, &input)?;
    if planned.plan.plan_id != expected_plan_id {
        return Err(format!(
            "Planul modelului este stale: UI a confirmat {}, Rust a recalculat {}.",
            expected_plan_id, planned.plan.plan_id
        ));
    }
    if planned.plan.blocked {
        return Err(format!(
            "Mutația modelului este blocată: {}",
            planned.plan.blockers.join(" ")
        ));
    }
    let (plan, mutation) =
        commit_project_workspace_session_mutation(&app, workspace, |candidate| {
            stage_content_model_mutation(candidate, planned, now_ms())
        })?;
    let workspace_receipt = WorkspaceEntryMutationReceipt {
        schema_version: WORKSPACE_ENTRY_MUTATION_SCHEMA_VERSION,
        project_root: workspace.session.project_root.clone(),
        runtime_session_id: workspace.runtime_session_id(),
        relative_path: plan.touched_files.first().cloned(),
        mutation,
        workspace: workspace.snapshot(),
    };
    Ok(ContentModelMutationApplyReceipt {
        plan,
        workspace: workspace_receipt,
    })
}

fn plan_for_workspace(
    root: &std::path::Path,
    workspace: &ProjectWorkspace,
    input: &ContentModelMutationInput,
) -> Result<PlannedContentModelMutation, String> {
    let projection = workspace.capture_projection_snapshot()?;
    let graph = build_source_graph_from_workspace_projection(root, &projection)?;
    build_mutation_plan(root, &graph, &projection.source_texts, input)
}
