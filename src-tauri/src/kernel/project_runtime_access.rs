use std::path::PathBuf;

use tauri::{AppHandle, Runtime};

use crate::{
    kernel::{
        project_session::ProjectSessionSnapshot,
        recovery_coordinator::{scan_recovery_coordinator, RecoveryCoordinatorStatus},
    },
    state::AppState,
};

/// Reads the active project root without exposing the runtime mutex to command
/// modules. Project lifecycle remains the only writer of this authority.
pub(crate) fn current_project_root(state: &AppState) -> Option<PathBuf> {
    state.current_root.lock().ok()?.clone()
}

pub(crate) fn require_current_project_root(state: &AppState) -> Result<PathBuf, String> {
    current_project_root(state).ok_or_else(|| "Nu există proiect deschis.".to_string())
}

pub(crate) fn current_project_session(
    state: &AppState,
) -> Result<Option<ProjectSessionSnapshot>, String> {
    Ok(state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace.".to_string())?
        .as_ref()
        .map(|workspace| workspace.session.clone()))
}

pub(crate) fn require_current_project_session(
    state: &AppState,
) -> Result<ProjectSessionSnapshot, String> {
    current_project_session(state)?
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())
}

pub(crate) fn require_recovery_coordinator_clean_for_write(
    state: &AppState,
    session: &ProjectSessionSnapshot,
    caller: &str,
) -> Result<(), String> {
    let scan = state
        .recovery_coordinator_scan
        .lock()
        .map_err(|_| "Nu am putut bloca RecoveryCoordinatorScan.".to_string())?
        .clone()
        .ok_or_else(|| {
            format!(
                "{caller} a blocat scrierea: Transaction Recovery Scan lipsește pentru sesiunea curentă."
            )
        })?;
    if scan.session_id != session.id {
        return Err(format!(
            "{caller} a blocat scrierea: Transaction Recovery Scan aparține sesiunii {}, dar sesiunea curentă este {}.",
            scan.session_id, session.id
        ));
    }
    if scan.project_root != session.project_root {
        return Err(format!(
            "{caller} a blocat scrierea: Transaction Recovery Scan aparține proiectului {}, dar sesiunea curentă este pentru {}.",
            scan.project_root, session.project_root
        ));
    }
    if scan.status != RecoveryCoordinatorStatus::Clean {
        return Err(format!(
            "{caller} a blocat scrierea: Transaction Recovery Scan este {} pentru sesiunea curentă.",
            recovery_coordinator_status_label(scan.status)
        ));
    }
    Ok(())
}

/// Validates the canonical root -> workspace lock order used by write gates.
pub(crate) fn require_project_workspace_available_for_write(
    state: &AppState,
) -> Result<(), String> {
    let root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul proiectului pentru mutație.".to_string())?;
    let root = root
        .as_ref()
        .ok_or_else(|| "Nu există proiect curent pentru mutație.".to_string())?;
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru mutație.".to_string())?;
    let workspace = workspace
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru mutație.".to_string())?;
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        root,
    )
}

/// Recomputes recovery state outside the runtime locks, then publishes it only
/// if the captured ProjectSession is still current.
pub(crate) fn refresh_recovery_coordinator_scan<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    session: &ProjectSessionSnapshot,
    command_succeeded: bool,
) -> Result<(), String> {
    match scan_recovery_coordinator(app, session) {
        Ok(scan) => {
            let live_workspace = state.project_workspace.lock().map_err(|_| {
                "Nu am putut bloca ProjectWorkspace pentru recovery CAS.".to_string()
            })?;
            let Some(live_session) = live_workspace.as_ref().map(|workspace| &workspace.session)
            else {
                return Err(
                    "Transaction Recovery Scan a refuzat publicarea după închiderea sesiunii."
                        .to_string(),
                );
            };
            if live_session.runtime_instance_id() != session.runtime_instance_id() {
                return Err(
                    "Transaction Recovery Scan a refuzat publicarea într-o altă instanță ProjectSession."
                        .to_string(),
                );
            }
            let mut recovery_slot = state
                .recovery_coordinator_scan
                .lock()
                .map_err(|_| "Nu am putut bloca RecoveryCoordinatorScan.".to_string())?;
            *recovery_slot = Some(scan);
            Ok(())
        }
        Err(error) => {
            if let Ok(live_workspace) = state.project_workspace.lock() {
                let matches_live_session = live_workspace.as_ref().is_some_and(|workspace| {
                    workspace.session.runtime_instance_id() == session.runtime_instance_id()
                });
                if matches_live_session {
                    if let Ok(mut recovery_slot) = state.recovery_coordinator_scan.lock() {
                        *recovery_slot = None;
                    }
                }
            }
            if command_succeeded {
                return Err(format!(
                    "Comanda a rulat, dar Transaction Recovery Scan nu a putut fi actualizat: {error}"
                ));
            }
            Ok(())
        }
    }
}

fn recovery_coordinator_status_label(status: RecoveryCoordinatorStatus) -> &'static str {
    match status {
        RecoveryCoordinatorStatus::Clean => "clean",
        RecoveryCoordinatorStatus::NeedsAttention => "needs_attention",
        RecoveryCoordinatorStatus::Unreadable => "unreadable",
    }
}
