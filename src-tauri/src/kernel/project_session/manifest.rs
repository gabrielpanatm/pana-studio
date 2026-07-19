use tauri::{AppHandle, Runtime};

use crate::{
    app_home::{project_session_dir, project_session_manifest_path},
    kernel::write_authority::{
        WriteAuthority, WriteCategory, WriteIntent, WriteOperationKind, WriteOwner, WritePolicy,
        WriteTarget,
    },
};

use super::model::ProjectSessionSnapshot;

pub fn write_session_manifest<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
) -> Result<(), String> {
    let path = project_session_manifest_path(app, &session.project_root)?;
    let boundary = project_session_dir(app, &session.project_root)?;
    let body = serde_json::to_string_pretty(session)
        .map_err(|error| format!("Nu am putut serializa manifestul ProjectSession: {}", error))?;
    let target = WriteTarget::new(
        path,
        boundary,
        format!("session/{}/manifest.json", session.id),
    );
    let intent = WriteIntent::new(
        WriteCategory::InternalAppWrite,
        WriteOwner::ProjectSession,
        WriteOperationKind::WriteText,
        target,
        WritePolicy::internal_atomic(),
        "Scriere manifest ProjectSession",
    );
    WriteAuthority::new(app)
        .write_text(intent, &format!("{}\n", body))
        .map_err(|error| error.into_terminal_diagnostic())
        .map(|_| ())
}
