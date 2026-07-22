use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Manager, Runtime, State};

use crate::{
    commands::project::require_current_project_root,
    deploy::{deploy_project_to_bunny_cancellable, run_zola_build_cancellable, run_zola_check},
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
        let runtime = worker_app.state::<WriteAuthorityRuntime>();
        let _project_lease =
            runtime.acquire_active_project_read_lease_for_session(&root, &runtime_session_id)?;
        let log = run_zola_build_cancellable(&root, &zola_root, &cancellation_token)?;
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
    let root = require_current_project_root(&state)?;
    let runtime_session_id = capture_deploy_runtime_session_id(&state, &root)?;
    let zola_root = zola_project_root(&root);

    tauri::async_runtime::spawn_blocking(move || {
        let runtime = app.state::<WriteAuthorityRuntime>();
        let _project_lease =
            runtime.acquire_active_project_read_lease_for_session(&root, &runtime_session_id)?;
        run_zola_check(&root, &zola_root)
    })
    .await
    .map_err(|e| format!("Validarea Zola embedded a căzut în task-ul de fundal: {e}"))?
}

/// Automatic editor validation is the exact ProjectWorkspace generation that
/// the embedded Preview engine has already loaded and rendered successfully.
/// Publication preflight continues to call `zola_check`, which intentionally
/// validates only canonical bytes saved on disk.
#[tauri::command]
pub fn zola_check_workspace(state: State<'_, AppState>) -> Result<String, String> {
    let (project_root, runtime_session_id, revision) = {
        let workspace = state
            .project_workspace
            .lock()
            .map_err(|_| "Nu am putut captura ProjectWorkspace pentru validare.".to_string())?;
        let workspace = workspace
            .as_ref()
            .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru validare.".to_string())?;
        (
            workspace.session.project_root.clone(),
            workspace.runtime_session_id(),
            workspace.revision,
        )
    };
    let engine = state
        .preview_engine
        .lock()
        .map_err(|_| "Motorul Preview embedded este indisponibil pentru validare.".to_string())?;
    let engine = engine.as_ref().ok_or_else(|| {
        "Nu există o generație Preview embedded pentru ProjectWorkspace curent.".to_string()
    })?;
    if !engine.owner_matches(&crate::preview::PersistentPreviewOwner::new(
        &project_root,
        &runtime_session_id,
    )) || !engine.active_matches_revision(revision)?
    {
        return Err(format!(
            "Generația Preview nu confirmă revizia ProjectWorkspace {revision}; reîmprospătează Preview-ul."
        ));
    }
    Ok(format!(
        "OK Validare Zola embedded reușită\nSursă validată: ProjectWorkspace revizia {revision}"
    ))
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
        if cancellation_token.is_cancelled() {
            return Err(
                "[publish_cancelled] Deploy-ul a fost anulat înainte de upload.".to_string(),
            );
        }
        let runtime = worker_app.state::<WriteAuthorityRuntime>();
        let _project_lease =
            runtime.acquire_active_project_read_lease_for_session(&root, &runtime_session_id)?;
        deploy_project_to_bunny_cancellable(&root, &zola_root, &root, &cancellation_token)
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
