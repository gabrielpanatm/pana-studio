use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use crate::{
    commands::config::{read_project_app_config_for_bootstrap, save_deploy_settings_for_root},
    commands::project::require_current_project_root,
    deploy::{
        build_deploy_artifact_manifest, configuration_snapshot,
        delete_credential as delete_stored_deploy_credential, execute_deploy_with_artifact,
        plan_deploy_with_artifact, resolve_credential, run_zola_build_cancellable, run_zola_check,
        save_credential as save_stored_deploy_credential, test_deploy_connection_with_credential,
        DeployCommandError, DeployConfigurationSnapshot, DeployConnectionTestReceipt,
        DeployCredentialStatus, DeployCredentialWriteInput, DeployErrorCode, DeployExecutionInput,
        DeployPlan, DeployPlanInput, DeployProgressEvent, DeployProgressPhase,
        DeployProgressReporter, DeployReceipt, DeploySettings, DeployTarget,
        DEPLOY_PROGRESS_SCHEMA_VERSION,
    },
    kernel::{
        file_buffer_store::FileBufferRequestIdentity,
        observability::{append_event, now_ms, KernelEventKind, KernelLogEvent, KernelLogLevel},
        publish_operation::{
            PublishOperationCancelReceipt, PublishOperationControl, PublishOperationKind,
        },
        publish_preflight::{
            build_publish_build_receipt, PublishBuildReceipt, PublishBuildReceiptInput,
        },
        write_authority::{WriteAuthorityError, WriteAuthorityRuntime},
    },
    project::zola_project_root,
    state::AppState,
};

use super::publish::{
    clear_publish_build_authorization, invalidate_publish_authorization,
    require_current_publish_build, require_current_publish_preflight,
    store_publish_build_receipt_if_current,
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
pub async fn build_for_publish(
    expected_preflight_token: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<PublishBuildReceipt, String> {
    let preflight = require_current_publish_preflight(&app, &state, &expected_preflight_token)?;
    let target_id = preflight
        .active_target
        .as_ref()
        .map(|target| target.target_id.clone())
        .ok_or_else(|| "Publish Preflight ready nu conține ținta activă.".to_string())?;
    clear_publish_build_authorization(&state)?;
    let root = std::path::PathBuf::from(&preflight.project_root);
    let runtime_session_id = preflight.runtime_session_id.clone();
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
    let worker_preflight = preflight.clone();
    let worker_expected_token = expected_preflight_token.clone();

    let worker =
        tauri::async_runtime::spawn_blocking(move || -> Result<PublishBuildReceipt, String> {
            let worker_state = worker_app.state::<AppState>();
            let _authorization_gate = worker_state
                .publish_authorization_gate
                .lock()
                .map_err(|_| "Buildul nu a putut bloca autoritatea de publicare.".to_string())?;
            let runtime = worker_app.state::<WriteAuthorityRuntime>();
            let _project_lease = runtime
                .acquire_active_project_read_lease_for_session(&root, &runtime_session_id)?;
            require_current_publish_preflight(&worker_app, &worker_state, &worker_expected_token)?;
            let log = run_zola_build_cancellable(&root, &zola_root, &cancellation_token)?;
            let artifact = build_deploy_artifact_manifest(&root, &zola_root)?;
            require_current_publish_preflight(&worker_app, &worker_state, &worker_expected_token)?;
            let receipt = build_publish_build_receipt(PublishBuildReceiptInput {
                project_root: worker_preflight.project_root,
                runtime_session_id: worker_preflight.runtime_session_id,
                workspace_revision: worker_preflight.workspace_revision,
                disk_generation: worker_preflight.disk_generation,
                project_model_revision: worker_preflight.project_model_revision,
                deploy_settings_revision: worker_preflight.deploy_settings_revision,
                deploy_settings_fingerprint: worker_preflight.deploy_settings_fingerprint,
                target_id,
                preflight_token: worker_preflight.preflight_token,
                artifact_id: artifact.artifact_id,
                artifact_files: artifact.files.len() as u64,
                artifact_bytes: artifact.total_bytes,
                completed_at_ms: now_ms(),
                log,
            });
            store_publish_build_receipt_if_current(&worker_app, &worker_state, receipt.clone())?;
            Ok(receipt)
        })
        .await;
    let result = match worker {
        Ok(result) => result,
        Err(error) => Err(format!(
            "Buildul pentru publicare a căzut în task-ul de fundal: {error}"
        )),
    };
    finish_publish_operation(
        &app,
        &state,
        &operation_id,
        PublishOperationKind::Build,
        &result,
    );
    result
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
    const PREVIEW_PENDING: &str = "PANA_WORKSPACE_PREVIEW_PENDING:";
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
        format!(
            "{PREVIEW_PENDING} Nu există încă o generație Preview embedded pentru ProjectWorkspace curent."
        )
    })?;
    if !engine.owner_matches(&crate::preview::PersistentPreviewOwner::new(
        &project_root,
        &runtime_session_id,
    )) || !engine.active_matches_revision(revision)?
    {
        return Err(format!(
            "{PREVIEW_PENDING} Generația Preview nu confirmă încă revizia ProjectWorkspace {revision}; validarea va continua după reîmprospătarea Preview-ului."
        ));
    }
    Ok(format!(
        "OK Validare Zola embedded reușită\nSursă validată: ProjectWorkspace revizia {revision}"
    ))
}

// ── Deploy ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn read_deploy_configuration(
    app: AppHandle,
    state: State<AppState>,
) -> Result<DeployConfigurationSnapshot, String> {
    let root = require_current_project_root(&state)?;
    let config = read_project_app_config_for_bootstrap(&app, &root)?;
    configuration_snapshot(&app, &root, config.deploy)
}

#[tauri::command]
pub fn save_deploy_settings(
    settings: DeploySettings,
    app: AppHandle,
    state: State<AppState>,
) -> Result<DeployConfigurationSnapshot, String> {
    let _authorization_gate = state.publish_authorization_gate.lock().map_err(|_| {
        "Configurația deploy nu a putut bloca autoritatea de publicare.".to_string()
    })?;
    let root = require_current_project_root(&state)?;
    let settings = save_deploy_settings_for_root(&app, &root, settings)?;
    invalidate_publish_authorization(&state)?;
    configuration_snapshot(&app, &root, settings)
}

#[tauri::command]
pub fn save_deploy_credential(
    target_id: String,
    credential: DeployCredentialWriteInput,
    app: AppHandle,
    state: State<AppState>,
) -> Result<DeployCredentialStatus, String> {
    let _authorization_gate = state
        .publish_authorization_gate
        .lock()
        .map_err(|_| "Credentialele nu au putut bloca autoritatea de publicare.".to_string())?;
    let root = require_current_project_root(&state)?;
    let config = read_project_app_config_for_bootstrap(&app, &root)?;
    let target = config
        .deploy
        .targets
        .iter()
        .find(|target| target.id == target_id)
        .ok_or_else(|| format!("Ținta deploy '{target_id}' nu există."))?;
    let status = save_stored_deploy_credential(&app, &root, target, credential)?;
    invalidate_publish_authorization(&state)?;
    Ok(status)
}

#[tauri::command]
pub fn delete_deploy_credential(
    credential_ref: String,
    app: AppHandle,
    state: State<AppState>,
) -> Result<bool, String> {
    let _authorization_gate = state
        .publish_authorization_gate
        .lock()
        .map_err(|_| "Credentialele nu au putut bloca autoritatea de publicare.".to_string())?;
    let root = require_current_project_root(&state)?;
    let removed = delete_stored_deploy_credential(&app, &root, &credential_ref)?;
    if removed {
        invalidate_publish_authorization(&state)?;
    }
    Ok(removed)
}

#[tauri::command]
pub async fn test_deploy_connection(
    target_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeployConnectionTestReceipt, DeployCommandError> {
    let root = require_current_project_root(&state).map_err(invalid_deploy_configuration)?;
    let runtime_session_id =
        capture_deploy_runtime_session_id(&state, &root).map_err(invalid_deploy_configuration)?;
    let config =
        read_project_app_config_for_bootstrap(&app, &root).map_err(invalid_deploy_configuration)?;
    let target = configured_target(&config.deploy, &target_id)?;
    let worker_app = app.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let runtime = worker_app.state::<WriteAuthorityRuntime>();
        let _project_lease = runtime
            .acquire_active_project_read_lease_for_session(&root, &runtime_session_id)
            .map_err(|error| DeployCommandError::new(DeployErrorCode::Internal, error))?;
        let credential = resolve_credential(&worker_app, &root, &target).map_err(|message| {
            DeployCommandError::new(DeployErrorCode::MissingCredentials, message)
        })?;
        test_deploy_connection_with_credential(&target, &credential)
    })
    .await
    .map_err(|error| {
        DeployCommandError::new(
            DeployErrorCode::Internal,
            format!("Testarea conexiunii deploy a căzut în task-ul de fundal: {error}"),
        )
    })?
}

#[tauri::command]
pub async fn plan_deploy(
    input: DeployPlanInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeployPlan, DeployCommandError> {
    let root = require_current_project_root(&state).map_err(invalid_deploy_configuration)?;
    let runtime_session_id =
        capture_deploy_runtime_session_id(&state, &root).map_err(invalid_deploy_configuration)?;
    let zola_root = zola_project_root(&root);
    let config =
        read_project_app_config_for_bootstrap(&app, &root).map_err(invalid_deploy_configuration)?;
    let settings_revision = config.deploy.revision;
    let target = configured_target(&config.deploy, &input.target_id)?;
    let build = require_current_publish_build(
        &app,
        &state,
        &input.expected_build_token,
        &input.expected_artifact_id,
        &input.target_id,
    )
    .map_err(invalid_deploy_configuration)?;
    let worker_app = app.clone();
    let worker_build = build.clone();
    let worker_input = input.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let worker_state = worker_app.state::<AppState>();
        let _authorization_gate = worker_state
            .publish_authorization_gate
            .lock()
            .map_err(|_| {
                DeployCommandError::new(
                    DeployErrorCode::Internal,
                    "Planul nu a putut bloca autoritatea de publicare.",
                )
            })?;
        let runtime = worker_app.state::<WriteAuthorityRuntime>();
        let _project_lease = runtime
            .acquire_active_project_read_lease_for_session(&root, &runtime_session_id)
            .map_err(|error| DeployCommandError::new(DeployErrorCode::Internal, error))?;
        require_current_publish_build(
            &worker_app,
            &worker_state,
            &worker_input.expected_build_token,
            &worker_input.expected_artifact_id,
            &worker_input.target_id,
        )
        .map_err(invalid_deploy_configuration)?;
        let artifact = build_deploy_artifact_manifest(&root, &zola_root).map_err(|message| {
            DeployCommandError::new(DeployErrorCode::ArtifactUnavailable, message)
        })?;
        if !artifact_identity_matches(&worker_build.artifact_id, &artifact.artifact_id) {
            return Err(DeployCommandError::new(
                DeployErrorCode::ArtifactUnavailable,
                "Artifactul s-a schimbat după buildul autorizat; rulează din nou buildul pentru publicare.",
            ));
        }
        let credential = resolve_credential(&worker_app, &root, &target).map_err(|message| {
            DeployCommandError::new(DeployErrorCode::MissingCredentials, message)
        })?;
        let mut plan = plan_deploy_with_artifact(&target, settings_revision, &artifact, &credential)?;
        plan.plan_token = authorize_deploy_plan_token(
            &plan.plan_token,
            &worker_build,
            settings_revision,
        )?;
        plan.preflight_token = worker_build.preflight_token;
        plan.build_token = worker_build.build_token;
        Ok(plan)
    })
    .await
    .map_err(|error| {
        DeployCommandError::new(
            DeployErrorCode::Internal,
            format!("Planificarea deploy a căzut în task-ul de fundal: {error}"),
        )
    })?
}

#[tauri::command]
pub async fn execute_deploy(
    input: DeployExecutionInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeployReceipt, DeployCommandError> {
    let root = require_current_project_root(&state).map_err(invalid_deploy_configuration)?;
    let runtime_session_id =
        capture_deploy_runtime_session_id(&state, &root).map_err(invalid_deploy_configuration)?;
    let zola_root = zola_project_root(&root);
    let config =
        read_project_app_config_for_bootstrap(&app, &root).map_err(invalid_deploy_configuration)?;
    if config.deploy.revision != input.expected_settings_revision {
        return Err(DeployCommandError::new(
            DeployErrorCode::InvalidConfiguration,
            format!(
                "Configurația deploy s-a schimbat: planul folosește revizia {}, iar revizia curentă este {}.",
                input.expected_settings_revision, config.deploy.revision
            ),
        ));
    }
    let target = configured_target(&config.deploy, &input.target_id)?;
    let build = require_current_publish_build(
        &app,
        &state,
        &input.expected_build_token,
        &input.expected_artifact_id,
        &input.target_id,
    )
    .map_err(invalid_deploy_configuration)?;
    if build.preflight_token != input.expected_preflight_token {
        return Err(invalid_deploy_configuration(
            "Planul deploy nu aparține Publish Preflight curent.",
        ));
    }
    let settings_revision = config.deploy.revision;
    let provider_plan_token = require_authorized_deploy_plan_token(
        &input.expected_plan_token,
        &build,
        settings_revision,
    )?;
    let control = begin_publish_operation(
        &app,
        &state,
        PublishOperationKind::Deploy,
        &root,
        &runtime_session_id,
    )
    .map_err(invalid_deploy_configuration)?;
    let operation_id = control.operation_id.clone();
    let cancellation_token = control.cancellation_token.clone();
    let worker_app = app.clone();
    let worker_operation_id = operation_id.clone();
    let worker_target = target.clone();
    let worker_build = build.clone();
    let worker_input = input.clone();
    let worker_provider_plan_token = provider_plan_token.clone();

    let worker = tauri::async_runtime::spawn_blocking(move || {
        if cancellation_token.is_cancelled() {
            return Err(DeployCommandError::new(
                DeployErrorCode::Cancelled,
                "Deploy-ul a fost anulat înainte de capturarea artifactului.",
            ));
        }
        let worker_state = worker_app.state::<AppState>();
        let _authorization_gate = worker_state
            .publish_authorization_gate
            .lock()
            .map_err(|_| {
                DeployCommandError::new(
                    DeployErrorCode::Internal,
                    "Deploy-ul nu a putut bloca autoritatea de publicare.",
                )
            })?;
        let runtime = worker_app.state::<WriteAuthorityRuntime>();
        let _project_lease = runtime
            .acquire_active_project_read_lease_for_session(&root, &runtime_session_id)
            .map_err(|error| DeployCommandError::new(DeployErrorCode::Internal, error))?;
        require_current_publish_build(
            &worker_app,
            &worker_state,
            &worker_input.expected_build_token,
            &worker_input.expected_artifact_id,
            &worker_input.target_id,
        )
        .map_err(invalid_deploy_configuration)?;
        require_authorized_deploy_plan_token(
            &worker_input.expected_plan_token,
            &worker_build,
            settings_revision,
        )?;
        let artifact = build_deploy_artifact_manifest(&root, &zola_root).map_err(|message| {
            DeployCommandError::new(DeployErrorCode::ArtifactUnavailable, message)
        })?;
        if !artifact_identity_matches(&worker_build.artifact_id, &artifact.artifact_id) {
            return Err(DeployCommandError::new(
                DeployErrorCode::ArtifactUnavailable,
                "Artifactul s-a schimbat după planul autorizat; deploy-ul remote nu a pornit.",
            ));
        }
        let credential =
            resolve_credential(&worker_app, &root, &worker_target).map_err(|message| {
                DeployCommandError::new(DeployErrorCode::MissingCredentials, message)
            })?;
        let event_app = worker_app.clone();
        let progress_sink = move |event: DeployProgressEvent| {
            if let Err(error) = event_app.emit("deploy-progress", event) {
                eprintln!("[Pană Studio] Evenimentul deploy-progress nu a putut fi emis: {error}");
            }
        };
        let reporter =
            DeployProgressReporter::new(&worker_operation_id, &worker_target, &progress_sink);
        execute_deploy_with_artifact(
            &worker_operation_id,
            &worker_target,
            settings_revision,
            &worker_provider_plan_token,
            artifact,
            credential,
            &|| cancellation_token.is_cancelled(),
            &reporter,
        )
    })
    .await;
    let result: Result<DeployReceipt, DeployCommandError> = match worker {
        Ok(result) => result,
        Err(error) => Err(DeployCommandError::new(
            DeployErrorCode::Internal,
            format!("Deploy-ul a căzut în task-ul de fundal: {error}"),
        )),
    };
    if let Err(error) = &result {
        emit_deploy_terminal_progress(&app, &operation_id, &target, error);
    }
    finish_publish_operation(
        &app,
        &state,
        &operation_id,
        PublishOperationKind::Deploy,
        &result,
    );
    result
}

fn artifact_identity_matches(expected_artifact_id: &str, observed_artifact_id: &str) -> bool {
    !expected_artifact_id.is_empty()
        && expected_artifact_id.starts_with("sha256:")
        && expected_artifact_id == observed_artifact_id
}

fn authorize_deploy_plan_token(
    provider_plan_token: &str,
    build: &PublishBuildReceipt,
    settings_revision: u64,
) -> Result<String, DeployCommandError> {
    if !provider_plan_token.starts_with("plan:") {
        return Err(invalid_deploy_configuration(
            "Providerul deploy a emis un planToken invalid.",
        ));
    }
    let digest = deploy_plan_authorization_digest(provider_plan_token, build, settings_revision);
    Ok(format!("publish-plan-v1:{digest}:{provider_plan_token}"))
}

fn require_authorized_deploy_plan_token(
    token: &str,
    build: &PublishBuildReceipt,
    settings_revision: u64,
) -> Result<String, DeployCommandError> {
    let payload = token
        .strip_prefix("publish-plan-v1:")
        .ok_or_else(|| invalid_deploy_configuration("Planul deploy nu are dovada Publish v1."))?;
    let (observed_digest, provider_plan_token) = payload.split_once(':').ok_or_else(|| {
        invalid_deploy_configuration("Planul deploy are o dovadă Publish incompletă.")
    })?;
    if !provider_plan_token.starts_with("plan:")
        || observed_digest
            != deploy_plan_authorization_digest(provider_plan_token, build, settings_revision)
    {
        return Err(invalid_deploy_configuration(
            "Planul deploy nu corespunde Preflightului, buildului sau sesiunii curente.",
        ));
    }
    Ok(provider_plan_token.to_string())
}

fn deploy_plan_authorization_digest(
    provider_plan_token: &str,
    build: &PublishBuildReceipt,
    settings_revision: u64,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"pana-publish-deploy-plan-v1\0");
    hash_publish_plan_field(&mut digest, build.project_root.as_bytes());
    hash_publish_plan_field(&mut digest, build.runtime_session_id.as_bytes());
    digest.update(build.workspace_revision.to_be_bytes());
    digest.update(build.disk_generation.to_be_bytes());
    hash_publish_plan_field(&mut digest, build.project_model_revision.as_bytes());
    digest.update(settings_revision.to_be_bytes());
    hash_publish_plan_field(&mut digest, build.deploy_settings_fingerprint.as_bytes());
    hash_publish_plan_field(&mut digest, build.target_id.as_bytes());
    hash_publish_plan_field(&mut digest, build.preflight_token.as_bytes());
    hash_publish_plan_field(&mut digest, build.build_token.as_bytes());
    hash_publish_plan_field(&mut digest, build.artifact_id.as_bytes());
    hash_publish_plan_field(&mut digest, provider_plan_token.as_bytes());
    format!("{:x}", digest.finalize())
}

fn hash_publish_plan_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_be_bytes());
    digest.update(bytes);
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

fn finish_publish_operation<R: Runtime, T: std::fmt::Display, E: std::fmt::Display>(
    app: &AppHandle<R>,
    state: &State<'_, AppState>,
    operation_id: &str,
    kind: PublishOperationKind,
    result: &Result<T, E>,
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
            Some(log.to_string()),
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

fn configured_target(
    settings: &DeploySettings,
    target_id: &str,
) -> Result<DeployTarget, DeployCommandError> {
    settings.validate().map_err(invalid_deploy_configuration)?;
    settings
        .targets
        .iter()
        .find(|target| target.id == target_id)
        .cloned()
        .ok_or_else(|| {
            DeployCommandError::new(
                DeployErrorCode::InvalidConfiguration,
                format!("Ținta deploy '{target_id}' nu există."),
            )
        })
}

fn invalid_deploy_configuration(message: impl Into<String>) -> DeployCommandError {
    DeployCommandError::new(DeployErrorCode::InvalidConfiguration, message)
}

fn emit_deploy_terminal_progress<R: Runtime>(
    app: &AppHandle<R>,
    operation_id: &str,
    target: &DeployTarget,
    error: &DeployCommandError,
) {
    let receipt = error.receipt.as_ref();
    let phase = if error.code == DeployErrorCode::Cancelled {
        DeployProgressPhase::Cancelled
    } else {
        DeployProgressPhase::Failed
    };
    let event = DeployProgressEvent {
        schema_version: DEPLOY_PROGRESS_SCHEMA_VERSION,
        operation_id: operation_id.to_string(),
        target_id: target.id.clone(),
        provider: target.provider_kind(),
        phase,
        current_path: None,
        completed_files: receipt
            .map_or(0, |receipt| receipt.uploaded_files + receipt.deleted_files),
        total_files: receipt.map_or(0, |receipt| {
            receipt.uploaded_files + receipt.skipped_files + receipt.deleted_files
        }),
        completed_bytes: receipt.map_or(0, |receipt| receipt.uploaded_bytes),
        total_bytes: receipt.map_or(0, |receipt| receipt.uploaded_bytes),
        timestamp_ms: now_ms(),
    };
    if let Err(emit_error) = app.emit("deploy-progress", event) {
        eprintln!("[Pană Studio] Evenimentul terminal deploy nu a putut fi emis: {emit_error}");
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

#[cfg(test)]
mod tests {
    use super::{
        artifact_identity_matches, authorize_deploy_plan_token,
        require_authorized_deploy_plan_token,
    };
    use crate::kernel::publish_preflight::{build_publish_build_receipt, PublishBuildReceiptInput};

    fn build_receipt(session: &str) -> crate::kernel::publish_preflight::PublishBuildReceipt {
        build_publish_build_receipt(PublishBuildReceiptInput {
            project_root: "/project".to_string(),
            runtime_session_id: session.to_string(),
            workspace_revision: 7,
            disk_generation: 3,
            project_model_revision: "sha256:model".to_string(),
            deploy_settings_revision: 4,
            deploy_settings_fingerprint: "sha256:settings".to_string(),
            target_id: "production".to_string(),
            preflight_token: "sha256:preflight".to_string(),
            artifact_id: "sha256:artifact".to_string(),
            artifact_files: 2,
            artifact_bytes: 42,
            completed_at_ms: 1,
            log: "ok".to_string(),
        })
    }

    #[test]
    fn authorized_artifact_identity_is_exact_and_fail_closed() {
        assert!(artifact_identity_matches("sha256:abc", "sha256:abc"));
        assert!(!artifact_identity_matches("sha256:abc", "sha256:def"));
        assert!(!artifact_identity_matches("", ""));
        assert!(!artifact_identity_matches("artifact", "artifact"));
    }

    #[test]
    fn deploy_plan_authorization_is_bound_to_build_session_and_provider_plan() {
        let build = build_receipt("session-a");
        let authorized = authorize_deploy_plan_token("plan:provider", &build, 4).unwrap();
        assert_eq!(
            require_authorized_deploy_plan_token(&authorized, &build, 4).unwrap(),
            "plan:provider"
        );
        assert!(
            require_authorized_deploy_plan_token(&authorized, &build_receipt("session-b"), 4,)
                .is_err()
        );
        assert!(require_authorized_deploy_plan_token(&authorized, &build, 5).is_err());
        assert!(require_authorized_deploy_plan_token(
            &authorized.replace("plan:provider", "plan:changed"),
            &build,
            4,
        )
        .is_err());
        assert!(authorize_deploy_plan_token("invalid", &build, 4).is_err());
    }
}
