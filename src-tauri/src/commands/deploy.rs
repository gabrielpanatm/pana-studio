use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};

use crate::{
    commands::{
        config::{read_project_app_config_for_root, ProjectAppConfig},
        project::require_current_project_root,
    },
    deploy::{deploy_project_to_bunny, resolve_artifact_root, run_zola_build, run_zola_check},
    images::{optimize_output_images, ImageOptimizationOptions},
    kernel::write_authority::{WriteAuthorityError, WriteAuthorityRuntime},
    project::zola_project_root,
    state::AppState,
};

// ── Zola Build ───────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn zola_build(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    let binary = get_zola_binary(&state)?;
    let root = require_current_project_root(&state)?;
    let runtime_session_id = capture_deploy_runtime_session_id(&state, &root)?;
    let zola_root = zola_project_root(&root);

    tauri::async_runtime::spawn_blocking(move || {
        let mut log = {
            let runtime = app.state::<WriteAuthorityRuntime>();
            let _project_lease = runtime
                .acquire_active_project_read_lease_for_session(&root, &runtime_session_id)?;
            run_zola_build(&binary, &root, &zola_root)?
        };
        // Do not hold an outer project RwLock lease while the optimizer enters
        // WriteAuthority; nested reads can deadlock behind a queued publisher.
        log.push_str(
            &maybe_optimize_output_images(&app, &root, &zola_root, &runtime_session_id)
                .map_err(WriteAuthorityError::into_terminal_diagnostic)?,
        );
        let runtime = app.state::<WriteAuthorityRuntime>();
        let _post_optimizer_lease =
            runtime.acquire_active_project_read_lease_for_session(&root, &runtime_session_id)?;
        Ok(log)
    })
    .await
    .map_err(|e| format!("Build-ul a căzut în task-ul de fundal: {}", e))?
}

#[tauri::command]
pub async fn zola_check(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    let binary = get_zola_binary(&state)?;
    let root = require_current_project_root(&state)?;
    let runtime_session_id = capture_deploy_runtime_session_id(&state, &root)?;
    let zola_root = zola_project_root(&root);

    tauri::async_runtime::spawn_blocking(move || {
        let runtime = app.state::<WriteAuthorityRuntime>();
        let _project_lease =
            runtime.acquire_active_project_read_lease_for_session(&root, &runtime_session_id)?;
        run_zola_check(&binary, &root, &zola_root)
    })
    .await
    .map_err(|e| format!("Zola check a căzut în task-ul de fundal: {}", e))?
}

// ── Deploy ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn deploy_to_bunny(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    let root = require_current_project_root(&state)?;
    let runtime_session_id = capture_deploy_runtime_session_id(&state, &root)?;
    let zola_root = zola_project_root(&root);

    tauri::async_runtime::spawn_blocking(move || {
        // The optimizer owns its WriteAuthority leases. The outer read lease
        // starts only after those writes finish and then protects manifest +
        // network against an internal project publication.
        let mut prefix = maybe_optimize_output_images(&app, &root, &zola_root, &runtime_session_id)
            .map_err(WriteAuthorityError::into_terminal_diagnostic)?;
        let runtime = app.state::<WriteAuthorityRuntime>();
        let _project_lease =
            runtime.acquire_active_project_read_lease_for_session(&root, &runtime_session_id)?;
        // deploy_project_to_bunny captures its manifest only after this
        // optional optimizer has completed.
        let deploy = deploy_project_to_bunny(&root, &zola_root, &root)?;
        if !prefix.is_empty() {
            prefix.push('\n');
        }
        Ok(format!("{}{}", prefix, deploy))
    })
    .await
    .map_err(|e| format!("Deploy-ul a căzut în task-ul de fundal: {}", e))?
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn get_zola_binary(state: &State<AppState>) -> Result<PathBuf, String> {
    state
        .zola_binary_path
        .lock()
        .map_err(|_| "Nu am putut accesa binary-ul Zola.".to_string())?
        .clone()
        .ok_or_else(|| "Binary-ul Zola nu a fost găsit.".to_string())
}

fn maybe_optimize_output_images(
    app: &AppHandle,
    project_root: &std::path::Path,
    zola_root: &std::path::Path,
    expected_runtime_session_id: &str,
) -> Result<String, WriteAuthorityError> {
    let config = read_project_app_config_for_root(app, project_root)?;
    if !config.optimize_images_on_build {
        return Ok(String::new());
    }

    let output_dir = resolve_artifact_root(project_root, zola_root)?;
    let report = optimize_output_images(
        app,
        &output_dir,
        &image_options_from_config(&config),
        expected_runtime_session_id,
    )?;
    let mut log = format!("\n{}\n", report.summary());
    if !report.log.trim().is_empty() {
        log.push_str(&report.log);
    }
    Ok(log)
}

fn capture_deploy_runtime_session_id(
    state: &State<'_, AppState>,
    project_root: &std::path::Path,
) -> Result<String, String> {
    let session = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut captura ProjectWorkspace pentru build/deploy.".to_string())?
        .as_ref()
        .map(|workspace| workspace.session.clone())
        .ok_or_else(|| "ProjectSession nu este inițializat pentru build/deploy.".to_string())?;
    if std::path::Path::new(&session.project_root) != project_root {
        return Err(format!(
            "Build/Deploy a blocat un root stale: ProjectSession aparține {}, iar requestul a capturat {}.",
            session.project_root,
            project_root.display()
        ));
    }
    Ok(session.runtime_instance_id())
}

fn image_options_from_config(config: &ProjectAppConfig) -> ImageOptimizationOptions {
    ImageOptimizationOptions {
        max_dimension: config.image_max_dimension.max(1),
        exclude_suffix: config.image_exclude_suffix.clone(),
        replace_only_if_smaller: config.image_replace_only_if_smaller,
    }
}
