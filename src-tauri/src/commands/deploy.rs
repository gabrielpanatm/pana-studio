use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use tauri::{AppHandle, Manager, Runtime, State};

use crate::{
    commands::{
        config::{read_project_app_config_for_root, ProjectAppConfig},
        project::require_current_project_root,
    },
    deploy::{
        deploy_project_to_bunny_cancellable, resolve_artifact_root, run_zola_build_cancellable,
        run_zola_check,
    },
    images::{optimize_output_images, ImageOptimizationOptions},
    kernel::{
        file_buffer_store::FileBufferRequestIdentity,
        observability::{append_event, now_ms, KernelEventKind, KernelLogEvent, KernelLogLevel},
        publish_operation::{
            PublishOperationCancelReceipt, PublishOperationControl, PublishOperationKind,
        },
        write_authority::{WriteAuthorityError, WriteAuthorityRuntime},
    },
    project::zola_project_root,
    state::AppState,
};

const PUBLISH_OPERATION_CANCEL_SCHEMA_VERSION: u32 = 1;
static PUBLISH_OPERATION_COUNTER: AtomicU64 = AtomicU64::new(1);

// ── Zola Build ───────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn zola_build(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    let binary = get_zola_binary(&state)?;
    let root = require_current_project_root(&state)?;
    let runtime_session_id = capture_deploy_runtime_session_id(&state, &root)?;
    let zola_root = zola_project_root(&root);
    let control = begin_publish_operation(
        &app,
        &state,
        PublishOperationKind::Build,
        &root,
        &runtime_session_id,
    )?;
    let operation_id = control.operation_id.clone();
    let cancellation_token = control.cancellation_token.clone();
    let worker_app = app.clone();

    let worker = tauri::async_runtime::spawn_blocking(move || {
        let mut log = {
            let runtime = worker_app.state::<WriteAuthorityRuntime>();
            let _project_lease = runtime
                .acquire_active_project_read_lease_for_session(&root, &runtime_session_id)?;
            run_zola_build_cancellable(&binary, &root, &zola_root, &cancellation_token)?
        };
        if cancellation_token.is_cancelled() {
            return Err(WriteAuthorityError::from(
                "[publish_cancelled] Build-ul a fost anulat înainte de optimizarea output-ului."
                    .to_string(),
            ));
        }
        // Do not hold an outer project RwLock lease while the optimizer enters
        // WriteAuthority; nested reads can deadlock behind a queued publisher.
        log.push_str(
            &maybe_optimize_output_images(&worker_app, &root, &zola_root, &runtime_session_id)
                .map_err(WriteAuthorityError::into_terminal_diagnostic)?,
        );
        if cancellation_token.is_cancelled() {
            return Err(WriteAuthorityError::from(
                "[publish_cancelled] Build-ul a fost anulat după optimizarea output-ului."
                    .to_string(),
            ));
        }
        let runtime = worker_app.state::<WriteAuthorityRuntime>();
        let _post_optimizer_lease =
            runtime.acquire_active_project_read_lease_for_session(&root, &runtime_session_id)?;
        Ok(log)
    })
    .await;
    let result: Result<String, WriteAuthorityError> = match worker {
        Ok(result) => result,
        Err(error) => Err(WriteAuthorityError::from(format!(
            "Build-ul a căzut în task-ul de fundal: {error}"
        ))),
    };
    finish_publish_operation(
        &app,
        &state,
        &operation_id,
        PublishOperationKind::Build,
        &result,
    );
    result.map_err(WriteAuthorityError::into_terminal_diagnostic)
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
    let control = begin_publish_operation(
        &app,
        &state,
        PublishOperationKind::Deploy,
        &root,
        &runtime_session_id,
    )?;
    let operation_id = control.operation_id.clone();
    let cancellation_token = control.cancellation_token.clone();
    let worker_app = app.clone();

    let worker = tauri::async_runtime::spawn_blocking(move || {
        // The optimizer owns its WriteAuthority leases. The outer read lease
        // starts only after those writes finish and then protects manifest +
        // network against an internal project publication.
        let mut prefix =
            maybe_optimize_output_images(&worker_app, &root, &zola_root, &runtime_session_id)
                .map_err(WriteAuthorityError::into_terminal_diagnostic)?;
        if cancellation_token.is_cancelled() {
            return Err(
                "[publish_cancelled] Deploy-ul a fost anulat înainte de upload.".to_string(),
            );
        }
        let runtime = worker_app.state::<WriteAuthorityRuntime>();
        let _project_lease =
            runtime.acquire_active_project_read_lease_for_session(&root, &runtime_session_id)?;
        // deploy_project_to_bunny captures its manifest only after this
        // optional optimizer has completed.
        let deploy =
            deploy_project_to_bunny_cancellable(&root, &zola_root, &root, &cancellation_token)?;
        if !prefix.is_empty() {
            prefix.push('\n');
        }
        Ok(format!("{}{}", prefix, deploy))
    })
    .await;
    let result: Result<String, String> = match worker {
        Ok(result) => result,
        Err(error) => Err(format!("Deploy-ul a căzut în task-ul de fundal: {error}")),
    };
    finish_publish_operation(
        &app,
        &state,
        &operation_id,
        PublishOperationKind::Deploy,
        &result,
    );
    result
}

#[tauri::command]
pub fn cancel_publish_operation(
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<PublishOperationCancelReceipt, String> {
    let operation = state
        .publish_operation
        .lock()
        .map_err(|_| "Nu am putut bloca operația Publish activă.".to_string())?
        .clone()
        .ok_or_else(|| "Nu există build sau deploy activ pentru anulare.".to_string())?;
    if operation.project_root != identity.expected_project_root
        || operation.runtime_session_id != identity.expected_session_id
    {
        return Err("Anularea Publish a refuzat un request din alt ProjectSession.".to_string());
    }
    let cancellation_requested = !operation.cancellation_token.is_cancelled();
    operation.cancellation_token.cancel();
    append_publish_event(
        &app,
        KernelLogLevel::Warn,
        KernelEventKind::CommandStarted,
        &operation,
        "publish_cancel_requested",
        "Utilizatorul a cerut anularea operației Publish.",
        None,
    );
    Ok(PublishOperationCancelReceipt {
        schema_version: PUBLISH_OPERATION_CANCEL_SCHEMA_VERSION,
        operation_id: operation.operation_id,
        kind: operation.kind,
        cancellation_requested,
    })
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

fn begin_publish_operation<R: Runtime>(
    app: &AppHandle<R>,
    state: &State<'_, AppState>,
    kind: PublishOperationKind,
    project_root: &std::path::Path,
    runtime_session_id: &str,
) -> Result<PublishOperationControl, String> {
    let mut active = state
        .publish_operation
        .lock()
        .map_err(|_| "Nu am putut rezerva operația Publish.".to_string())?;
    if let Some(operation) = active.as_ref() {
        return Err(format!(
            "Operația Publish {} este deja activă; așteaptă finalizarea sau anuleaz-o.",
            operation.operation_id
        ));
    }
    let operation = PublishOperationControl {
        operation_id: format!(
            "publish-{}-{}",
            now_ms(),
            PUBLISH_OPERATION_COUNTER.fetch_add(1, Ordering::Relaxed)
        ),
        kind,
        project_root: project_root.to_string_lossy().to_string(),
        runtime_session_id: runtime_session_id.to_string(),
        cancellation_token: tokio_util::sync::CancellationToken::new(),
    };
    *active = Some(operation.clone());
    drop(active);
    append_publish_event(
        app,
        KernelLogLevel::Info,
        KernelEventKind::CommandStarted,
        &operation,
        "publish_started",
        "Operația Publish a pornit și este legată de ProjectSession.",
        None,
    );
    Ok(operation)
}

fn finish_publish_operation<R: Runtime, E: std::fmt::Display>(
    app: &AppHandle<R>,
    state: &State<'_, AppState>,
    operation_id: &str,
    kind: PublishOperationKind,
    result: &Result<String, E>,
) {
    let operation = state
        .publish_operation
        .lock()
        .ok()
        .and_then(|mut active| {
            if active
                .as_ref()
                .is_some_and(|operation| operation.operation_id == operation_id)
            {
                active.take()
            } else {
                None
            }
        })
        .unwrap_or(PublishOperationControl {
            operation_id: operation_id.to_string(),
            kind,
            project_root: String::new(),
            runtime_session_id: String::new(),
            cancellation_token: tokio_util::sync::CancellationToken::new(),
        });
    match result {
        Ok(log) => append_publish_event(
            app,
            KernelLogLevel::Info,
            KernelEventKind::CommandCommitted,
            &operation,
            "publish_completed",
            "Operația Publish s-a încheiat cu succes.",
            Some(log.clone()),
        ),
        Err(error) => {
            let diagnostic = error.to_string();
            let cancelled = diagnostic.contains("[publish_cancelled]");
            append_publish_event(
                app,
                if cancelled {
                    KernelLogLevel::Warn
                } else {
                    KernelLogLevel::Error
                },
                KernelEventKind::CommandFailed,
                &operation,
                if cancelled {
                    "publish_cancelled"
                } else {
                    "publish_failed"
                },
                "Operația Publish nu a fost finalizată.",
                Some(diagnostic),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn append_publish_event<R: Runtime>(
    app: &AppHandle<R>,
    level: KernelLogLevel,
    event_kind: KernelEventKind,
    operation: &PublishOperationControl,
    event_name: &str,
    message: &str,
    diagnostic: Option<String>,
) {
    let event = KernelLogEvent::new(
        level,
        event_kind,
        "publish_center",
        "publish_operation",
        event_name,
        (!operation.project_root.is_empty()).then(|| operation.project_root.clone()),
        message,
        diagnostic,
    )
    .with_attribute("operationId", operation.operation_id.clone())
    .with_attribute(
        "operationKind",
        match operation.kind {
            PublishOperationKind::Build => "build",
            PublishOperationKind::Deploy => "deploy",
        },
    )
    .with_attribute("runtimeSessionId", operation.runtime_session_id.clone());
    if let Err(error) = append_event(app, event) {
        eprintln!("[Pană Studio] Nu am putut jurnaliza Publish: {error}");
    }
}
