use serde::{Deserialize, Serialize};

use crate::{
    commands::config::ProjectSettingsSnapshot,
    kernel::{
        preview_projection::CanvasPatch,
        project_state::KernelProjectTransitionDecisionRetentionRecoveryReceipt,
        project_workspace::{
            ProjectWorkspaceSaveRecoveryReceipt, ProjectWorkspaceSnapshot, WorkspaceUndoRedoReceipt,
        },
        recovery_coordinator::RecoveryCoordinatorScan,
        workbench::{WorkbenchCommandReceipt, WorkbenchSnapshot},
    },
    preview::CanvasProjectionPlan,
    project::{ProjectLifecycleSnapshot, ProjectScan},
    project_model::template_workbench::TemplateWorkbenchPlan,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBootstrapDocument {
    pub relative_path: String,
    pub source: String,
    pub preview_path: Option<String>,
    pub diagnostic_location: Option<ProjectBootstrapSourceLocation>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBootstrapSourceLocation {
    pub line: u32,
    pub column: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectOpenBootstrapReceipt {
    pub schema_version: u32,
    pub project: ProjectScan,
    pub lifecycle: ProjectLifecycleSnapshot,
    pub workspace: ProjectWorkspaceSnapshot,
    pub project_settings: ProjectSettingsSnapshot,
    pub deploy_settings: crate::deploy::DeploySettings,
    pub workbench: WorkbenchSnapshot,
    pub active_document: Option<ProjectBootstrapDocument>,
    pub target_css_file: Option<String>,
    pub initial_surface: Option<ProjectBootstrapInitialSurface>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBootstrapInitialSurface {
    pub document_path: String,
    pub route: String,
    pub preview_url: String,
    pub reuse_token: String,
    pub plan: TemplateWorkbenchPlan,
    pub canvas_projection: CanvasProjectionPlan,
}

pub const PROJECT_WORKSPACE_UNDO_REDO_COMMAND_SCHEMA_VERSION: u32 = 4;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkspaceUndoRedoCommandReceipt {
    pub schema_version: u32,
    pub project_root: String,
    pub runtime_session_id: String,
    pub result: WorkspaceUndoRedoReceipt,
    pub workspace: ProjectWorkspaceSnapshot,
    pub workbench: Option<WorkbenchCommandReceipt>,
    pub canvas_patch: Option<CanvasPatch>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTransitionDecisionRetentionHotJournalRecoveryCommandResult {
    pub receipt: KernelProjectTransitionDecisionRetentionRecoveryReceipt,
    pub recovery_coordinator: RecoveryCoordinatorScan,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkspaceSaveRecoveryCommandResult {
    pub receipt: ProjectWorkspaceSaveRecoveryReceipt,
    pub recovery_coordinator: RecoveryCoordinatorScan,
    pub workspace: ProjectWorkspaceSnapshot,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectDiskWatchRequest {
    pub expected_project_root: String,
    pub expected_session_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectDiskWatchStopRequest {
    pub expected_project_root: String,
    pub expected_session_id: String,
    pub expected_watch_generation: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDiskWatchReceipt {
    pub project_root: String,
    pub runtime_session_id: String,
    pub watch_generation: u64,
}
