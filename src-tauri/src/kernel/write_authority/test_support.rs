use std::{fs, path::Path};

use tauri::{AppHandle, Manager, Runtime};

use super::WriteAuthorityRuntime;

pub(crate) fn install_test_project_authority<R: Runtime>(
    app: &AppHandle<R>,
    runtime_session_id: &str,
    project_root: &Path,
    _session_dir: &Path,
) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        let metadata = fs::metadata(project_root).map_err(|error| {
            format!(
                "Test ProjectSession authority nu poate citi {}: {error}",
                project_root.display()
            )
        })?;
        let runtime = app
            .try_state::<WriteAuthorityRuntime>()
            .ok_or_else(|| "Test app nu are WriteAuthorityRuntime instalat.".to_string())?;
        let pending = runtime.capture_pending_project(
            runtime_session_id,
            project_root,
            metadata.dev(),
            metadata.ino(),
        )?;
        let mut publication = runtime.project_publication()?;
        publication.publish(pending)?;
        return Ok(());
    }
    #[cfg(not(unix))]
    Err("Test ProjectSession authority este disponibilă numai pe Unix.".to_string())
}
