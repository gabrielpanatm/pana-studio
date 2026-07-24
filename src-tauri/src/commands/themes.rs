use tauri::{AppHandle, Manager, Runtime, State};

use crate::{
    kernel::{
        observability::now_ms,
        project_workspace::{
            commit_project_workspace_session_mutation, ProjectWorkspace, ProjectWorkspaceIdentity,
        },
        themes::{
            apply_theme_plan, plan_theme_operation, ThemeApplyReceipt, ThemeApplyRequest,
            ThemeCatalogSnapshot, ThemePlan, ThemePlanRequest, ThemeRegistry,
            THEME_CATALOG_SCHEMA_VERSION,
        },
    },
    state::AppState,
};

#[tauri::command]
pub fn read_theme_catalog(
    identity: Option<ProjectWorkspaceIdentity>,
    app: AppHandle,
    state: State<AppState>,
) -> Result<ThemeCatalogSnapshot, String> {
    let registry = ThemeRegistry::load(&app).map_err(|error| error.to_string())?;
    let Some(identity) = identity else {
        return registry.snapshot(None);
    };
    let root = current_theme_root(state.inner())?;
    let slot = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru catalogul de teme.".to_string())?;
    let workspace = slot
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru teme.".to_string())?;
    workspace.require_identity(&identity)?;
    require_root_matches(&root, workspace)?;
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        &root,
    )?;
    registry.snapshot(Some(workspace))
}

#[tauri::command]
pub fn plan_theme_change(
    request: ThemePlanRequest,
    app: AppHandle,
    state: State<AppState>,
) -> Result<ThemePlan, String> {
    let registry = ThemeRegistry::load(&app).map_err(|error| error.to_string())?;
    let root = current_theme_root(state.inner())?;
    let slot = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru planul temei.".to_string())?;
    let workspace = slot
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru teme.".to_string())?;
    workspace.require_identity(&request.identity)?;
    require_root_matches(&root, workspace)?;
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        &root,
    )?;
    plan_theme_operation(&registry, workspace, &request)
}

#[tauri::command]
pub async fn apply_theme_change(
    request: ThemeApplyRequest,
    app: AppHandle,
) -> Result<ThemeApplyReceipt, String> {
    tauri::async_runtime::spawn_blocking(move || apply_theme_change_blocking(&app, request))
        .await
        .map_err(|error| format!("Mutația temei a căzut în task-ul de fundal: {error}"))?
}

fn apply_theme_change_blocking<R: Runtime>(
    app: &AppHandle<R>,
    request: ThemeApplyRequest,
) -> Result<ThemeApplyReceipt, String> {
    let registry = ThemeRegistry::load(app).map_err(|error| error.to_string())?;
    let state = app.state::<AppState>();
    let root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul pentru mutația temei.".to_string())?
        .clone()
        .ok_or_else(|| "Nu există proiect curent pentru mutația temei.".to_string())?;
    let mut slot = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru mutația temei.".to_string())?;
    let workspace = slot
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru teme.".to_string())?;
    workspace.require_identity(&request.plan.identity)?;
    require_root_matches(&root, workspace)?;
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        &root,
    )?;

    let expected_plan_token = request.expected_plan_token;
    let plan_request = request.plan;
    let (plan, mutation) =
        commit_project_workspace_session_mutation(app, workspace, |candidate| {
            apply_theme_plan(
                &registry,
                &root,
                candidate,
                plan_request,
                &expected_plan_token,
                now_ms(),
            )
        })?;
    Ok(ThemeApplyReceipt {
        schema_version: THEME_CATALOG_SCHEMA_VERSION,
        plan,
        mutation,
        workspace: workspace.snapshot(),
    })
}

fn current_theme_root(state: &AppState) -> Result<std::path::PathBuf, String> {
    state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul pentru teme.".to_string())?
        .clone()
        .ok_or_else(|| "Nu există proiect curent pentru teme.".to_string())
}

fn require_root_matches(
    root: &std::path::PathBuf,
    workspace: &ProjectWorkspace,
) -> Result<(), String> {
    if root.to_string_lossy() != workspace.session.project_root {
        return Err(format!(
            "ThemeRegistry a refuzat root-uri divergente: runtime={}, workspace={}.",
            root.display(),
            workspace.session.project_root
        ));
    }
    Ok(())
}
