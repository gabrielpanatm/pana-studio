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
    if intent_uses_projection_write_behind(&intent) {
        let receipt = state.workbench.apply(&session, &identity, intent)?;
        if receipt.changed {
            let snapshot = receipt.snapshot.clone();
            let revision = snapshot.revision;
            if let Err(error) =
                state
                    .workbench_projection_persistence
                    .schedule(app.clone(), session, snapshot)
            {
                eprintln!(
                    "[Pană Studio] Workbench projection write-behind scheduling failed at revision {}: {}",
                    revision, error
                );
            }
        }
        return Ok(receipt);
    }
    state
        .workbench
        .apply_persisted(&session, &identity, intent, |snapshot| {
            persist_workbench(&app, &session, snapshot)
        })
}

fn intent_uses_projection_write_behind(intent: &WorkbenchIntent) -> bool {
    matches!(
        intent,
        WorkbenchIntent::SetActivity { .. } | WorkbenchIntent::ActivateDocument { .. }
    )
}
