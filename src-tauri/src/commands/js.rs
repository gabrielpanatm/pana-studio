use serde::Serialize;
use tauri::{AppHandle, State};

use crate::{
    js::{
        self, require_page_js_draft_session_identity, require_page_js_file_buffer_identity,
        PageJsCommandReceipt, PageJsConfig, PageJsDraftStageInput, PageJsDraftStageReceipt,
        PageJsDraftStoreSnapshot, PageJsRequestIdentity,
    },
    kernel::{
        file_buffer_store::FileBufferStore,
        motion_graph::{
            apply_motion_timeline_step_timing as apply_motion_timeline_step_timing_kernel,
            MotionTimelineStepTimingInput, MotionTimelineStepTimingReceipt,
        },
        project_session::ProjectSessionSnapshot,
        project_workspace::{
            commit_project_workspace_session_mutation, ProjectWorkspaceIdentity,
            WorkspaceMutationMetadata,
        },
    },
    project::strip_zola_root_prefix,
    state::AppState,
};

fn with_bound_page_js_file_buffer<T>(
    state: &AppState,
    identity: &PageJsRequestIdentity,
    operation: impl FnOnce(
        &std::path::Path,
        &ProjectSessionSnapshot,
        &FileBufferStore,
        &crate::js::PageJsDraftStore,
    ) -> Result<T, String>,
) -> Result<PageJsCommandReceipt<T>, String> {
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul curent pentru Page JS.".to_string())?;
    let project_root = current_root
        .as_ref()
        .ok_or_else(|| "Nu există proiect curent pentru Page JS.".to_string())?;
    let current_root_string = project_root.to_string_lossy().into_owned();
    let project_workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru Page JS.".to_string())?;
    let workspace = project_workspace
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru Page JS.".to_string())?;
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        project_root,
    )?;
    require_page_js_file_buffer_identity(
        &current_root_string,
        &workspace.session,
        &workspace.documents,
        identity,
    )?;
    let payload = operation(
        project_root,
        &workspace.session,
        &workspace.documents,
        &workspace.page_js,
    )?;
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        project_root,
    )?;
    Ok(PageJsCommandReceipt::new(&workspace.session, payload))
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageJsWorkspaceState {
    pub template_path: String,
    pub accepted: PageJsConfig,
    pub current: PageJsConfig,
    pub dirty: bool,
    pub entry_revision: Option<u64>,
}

#[tauri::command(async)]
pub fn get_page_data_anims(
    template_path: String,
    identity: PageJsRequestIdentity,
    state: State<AppState>,
) -> Result<PageJsCommandReceipt<Vec<String>>, String> {
    with_bound_page_js_file_buffer(state.inner(), &identity, |project_root, _, store, _| {
        js::read_page_data_anims(project_root, store, &template_path)
    })
}

#[tauri::command(async)]
pub fn get_page_js(
    template_path: String,
    identity: PageJsRequestIdentity,
    state: State<AppState>,
) -> Result<PageJsCommandReceipt<PageJsConfig>, String> {
    with_bound_page_js_file_buffer(
        state.inner(),
        &identity,
        |project_root, _, store, drafts| {
            let template_path = strip_zola_root_prefix(&template_path);
            if let Some(draft) = drafts.drafts.get(template_path) {
                return Ok(draft.current.clone());
            }
            js::read_page_js_config(project_root, store, template_path)
        },
    )
}

/// Returns one atomic, read-only projection of the Page JS resource owned by
/// ProjectWorkspace. UI clients must use `accepted` as the stable staging
/// baseline and `current` as the value to render; no frontend draft registry is
/// involved.
#[tauri::command(async)]
pub fn get_page_js_workspace_state(
    template_path: String,
    identity: PageJsRequestIdentity,
    state: State<AppState>,
) -> Result<PageJsCommandReceipt<PageJsWorkspaceState>, String> {
    with_bound_page_js_file_buffer(
        state.inner(),
        &identity,
        |project_root, _, store, drafts| {
            let template_path = strip_zola_root_prefix(&template_path).to_string();
            if let Some(draft) = drafts.drafts.get(&template_path) {
                return Ok(PageJsWorkspaceState {
                    template_path,
                    accepted: draft.base.clone(),
                    current: draft.current.clone(),
                    dirty: true,
                    entry_revision: Some(draft.revision),
                });
            }
            let accepted = js::read_page_js_config(project_root, store, &template_path)?;
            Ok(PageJsWorkspaceState {
                template_path,
                current: accepted.clone(),
                accepted,
                dirty: false,
                entry_revision: None,
            })
        },
    )
}

#[tauri::command(async)]
pub fn stage_page_js_draft(
    input: PageJsDraftStageInput,
    app: AppHandle,
    state: State<AppState>,
) -> Result<PageJsDraftStageReceipt, String> {
    let mut input = input;
    input.template_path = strip_zola_root_prefix(&input.template_path).to_string();
    let identity = PageJsRequestIdentity {
        expected_project_root: input.expected_project_root.clone(),
        expected_session_id: input.expected_session_id.clone(),
    };
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul curent pentru Page JS.".to_string())?;
    let current_root = current_root
        .as_ref()
        .ok_or_else(|| "Nu există proiect curent pentru Page JS.".to_string())?
        .to_string_lossy()
        .into_owned();
    let mut workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru Page JS.".to_string())?;
    let workspace = workspace
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru Page JS.".to_string())?;
    require_page_js_draft_session_identity(
        &current_root,
        &workspace.session,
        &workspace.page_js,
        &identity,
    )?;
    let metadata = WorkspaceMutationMetadata {
        label: "Editare Page JS".to_string(),
        source: input
            .source
            .clone()
            .unwrap_or_else(|| "inspector.js".to_string()),
        coalesce_key: input.coalesce_key.clone(),
        transaction_id: input.transaction_id.clone(),
    };
    let receipt = commit_project_workspace_session_mutation(&app, workspace, |candidate| {
        candidate.stage_page_js(
            &workspace_identity(candidate),
            metadata,
            input,
            crate::kernel::observability::now_ms(),
        )
    })?;
    receipt
        .page_js
        .ok_or_else(|| "ProjectWorkspace nu a returnat receipt Page JS.".to_string())
}

#[tauri::command(async)]
pub fn read_page_js_drafts(
    expected_project_root: String,
    expected_session_id: String,
    state: State<AppState>,
) -> Result<PageJsDraftStoreSnapshot, String> {
    let identity = PageJsRequestIdentity {
        expected_project_root,
        expected_session_id,
    };
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul curent pentru Page JS.".to_string())?;
    let current_root = current_root
        .as_ref()
        .ok_or_else(|| "Nu există proiect curent pentru Page JS.".to_string())?
        .to_string_lossy()
        .into_owned();
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru Page JS.".to_string())?;
    let workspace = workspace
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru Page JS.".to_string())?;
    require_page_js_draft_session_identity(
        &current_root,
        &workspace.session,
        &workspace.page_js,
        &identity,
    )?;
    Ok(workspace.page_js.snapshot())
}

#[tauri::command(async)]
pub fn clear_page_js_draft(
    template_path: String,
    expected_revision: Option<u64>,
    expected_project_root: String,
    expected_session_id: String,
    app: AppHandle,
    state: State<AppState>,
) -> Result<PageJsDraftStageReceipt, String> {
    let template_path = strip_zola_root_prefix(&template_path);
    let identity = PageJsRequestIdentity {
        expected_project_root,
        expected_session_id,
    };
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul curent pentru Page JS.".to_string())?;
    let current_root = current_root
        .as_ref()
        .ok_or_else(|| "Nu există proiect curent pentru Page JS.".to_string())?
        .to_string_lossy()
        .into_owned();
    let mut workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru Page JS.".to_string())?;
    let workspace = workspace
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru Page JS.".to_string())?;
    require_page_js_draft_session_identity(
        &current_root,
        &workspace.session,
        &workspace.page_js,
        &identity,
    )?;
    let receipt = commit_project_workspace_session_mutation(&app, workspace, |candidate| {
        candidate.clear_page_js(
            &workspace_identity(candidate),
            WorkspaceMutationMetadata {
                label: "Revenire Page JS la baseline".to_string(),
                source: "inspector.js".to_string(),
                coalesce_key: None,
                transaction_id: None,
            },
            template_path,
            expected_revision,
            crate::kernel::observability::now_ms(),
        )
    })?;
    receipt
        .page_js
        .ok_or_else(|| "ProjectWorkspace nu a returnat receipt Page JS.".to_string())
}

fn workspace_identity(
    workspace: &crate::kernel::project_workspace::ProjectWorkspace,
) -> ProjectWorkspaceIdentity {
    ProjectWorkspaceIdentity {
        expected_project_root: workspace.session.project_root.clone(),
        expected_session_id: workspace.runtime_session_id(),
        expected_revision: workspace.revision,
    }
}

#[tauri::command(async)]
pub fn apply_motion_timeline_step_timing(
    input: MotionTimelineStepTimingInput,
) -> Result<MotionTimelineStepTimingReceipt, String> {
    apply_motion_timeline_step_timing_kernel(input)
}
