use tauri::{AppHandle, State};

use crate::{
    kernel::workbench::{
        persist_latest_workbench, persist_workbench, read_persisted_workbench,
        WorkbenchCommandReceipt, WorkbenchIdentity, WorkbenchIntent, WorkbenchSnapshot,
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
    let session = {
        let workspace = state
            .project_workspace
            .lock()
            .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru Workbench.".to_string())?;
        workspace
            .as_ref()
            .map(|workspace| workspace.session.clone())
            .ok_or_else(|| {
                "Workbench nu poate aplica intenții fără un proiect activ.".to_string()
            })?
    };
    state
        .workbench
        .read_or_restore(&session, || read_persisted_workbench(&session))?;
    if matches!(&intent, WorkbenchIntent::SetActivity { .. }) {
        let receipt = state.workbench.apply(&session, &identity, intent)?;
        if receipt.changed {
            let app = app.clone();
            let snapshot = receipt.snapshot.clone();
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(error) = persist_latest_workbench(&app, &session, &snapshot) {
                    eprintln!(
                        "[Pană Studio] Workbench activity write-behind failed at revision {}: {}",
                        snapshot.revision, error
                    );
                }
            });
        }
        return Ok(receipt);
    }
    state
        .workbench
        .apply_persisted(&session, &identity, intent, |snapshot| {
            persist_workbench(&app, &session, snapshot)
        })
}
