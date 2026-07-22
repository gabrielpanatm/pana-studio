use tauri::{AppHandle, State};

use crate::{
    kernel::workbench::{
        persist_workbench, read_persisted_workbench, WorkbenchCommandReceipt, WorkbenchIdentity,
        WorkbenchIntent, WorkbenchSnapshot,
    },
    state::AppState,
};

#[tauri::command]
pub fn read_workbench_state(state: State<AppState>) -> Result<Option<WorkbenchSnapshot>, String> {
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru Workbench.".to_string())?;
    let Some(session) = workspace
        .as_ref()
        .map(|workspace| workspace.session.clone())
    else {
        return Ok(None);
    };
    state
        .workbench
        .read_or_restore(&session, || read_persisted_workbench(&session))
        .map(Some)
}

#[tauri::command]
pub fn apply_workbench_intent(
    identity: WorkbenchIdentity,
    intent: WorkbenchIntent,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkbenchCommandReceipt, String> {
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru Workbench.".to_string())?;
    let session = workspace
        .as_ref()
        .map(|workspace| workspace.session.clone())
        .ok_or_else(|| "Workbench nu poate aplica intenții fără un proiect activ.".to_string())?;
    state
        .workbench
        .read_or_restore(&session, || read_persisted_workbench(&session))?;
    state
        .workbench
        .apply_persisted(&session, &identity, intent, |snapshot| {
            persist_workbench(&app, &session, snapshot)
        })
}
