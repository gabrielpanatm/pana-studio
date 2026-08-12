use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, State};

use crate::{
    commands::config::read_project_app_config_for_bootstrap,
    deploy::{
        configuration_snapshot, run_zola_editor_check, DeployConfigurationSnapshot,
        DeployCredentialKind, DeployCredentialStatus, DeployProviderKind,
    },
    kernel::{
        audit::{
            build_audit_run, AuditBuildEvidence, AuditOutcome, AuditPolicy, AuditPolicyOverride,
            AuditRequest, AuditRunMode, AuditScope, AuditSuppression, AuditSuppressionScope,
        },
        project_workspace::WorkspaceProjectionSnapshot,
        publish_preflight::{
            build_publish_preflight_receipt, PublishBuildReceipt, PublishPreflightEvidence,
            PublishPreflightEvidenceKind, PublishPreflightGate, PublishPreflightGateOutcome,
            PublishPreflightReceipt, PublishPreflightReceiptInput, PublishPreflightRemediation,
            PublishPreflightRemediationKind, PublishPreflightTargetIdentity,
        },
    },
    localization::LocalizedDiagnostic,
    project::{read_project_disk_manifest, AcceptedProjectDiskManifest, ProjectDiskManifest},
    project_model::build_project_model_for_audit_from_workspace_projection,
    state::AppState,
};

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishPreflightRequest {
    #[serde(default)]
    pub policy_overrides: Vec<AuditPolicyOverride>,
    #[serde(default)]
    pub suppressions: Vec<AuditSuppression>,
}

struct PublishAuthorityContext {
    root: PathBuf,
    project_root: String,
    runtime_session_id: String,
    workspace_revision: u64,
    disk_generation: u64,
    dirty: bool,
    accepted_disk: AcceptedProjectDiskManifest,
    observed_disk: ProjectDiskManifest,
    projection: WorkspaceProjectionSnapshot,
    file_buffer_diagnostics: Vec<crate::kernel::file_buffer_store::FileBufferDiagnostic>,
}

#[tauri::command]
pub fn run_publish_preflight(
    request: Option<PublishPreflightRequest>,
    app: AppHandle,
    state: State<AppState>,
) -> Result<PublishPreflightReceipt, String> {
    let request = request.unwrap_or_default();
    validate_publish_policy_request(&request)?;
    let _authorization_gate = state
        .publish_authorization_gate
        .lock()
        .map_err(|_| "Publish Preflight nu a putut bloca autoritatea de publicare.".to_string())?;
    state.clear_publish_authorization()?;
    let context = capture_publish_authority_context(&state)?;
    let configuration = read_deploy_snapshot(&app, &context.root)?;
    let settings_fingerprint = deploy_settings_fingerprint(&configuration)?;
    let disk_coherent = context.observed_disk == context.accepted_disk.manifest;
    let observed_disk_fingerprint = project_disk_fingerprint(&context.observed_disk)?;

    let zola_evidence = if context.dirty {
        AuditBuildEvidence::Skipped {
            message: "Validarea Zola pe disc a fost omisă deoarece ProjectWorkspace conține modificări nesalvate."
                .to_string(),
        }
    } else if !disk_coherent {
        AuditBuildEvidence::Skipped {
            message: "Validarea Zola pe disc a fost omisă deoarece discul diferă de AcceptedDisk."
                .to_string(),
        }
    } else {
        let zola_root = crate::project::zola_project_root(&context.root);
        match run_zola_editor_check(&context.root, &zola_root) {
            Ok(message) => AuditBuildEvidence::Complete { message },
            Err(message) => AuditBuildEvidence::Failed { message },
        }
    };

    let model = build_project_model_for_audit_from_workspace_projection(
        &context.root,
        &context.projection,
    )?;
    let audit_request = AuditRequest {
        mode: AuditRunMode::Full,
        scope: AuditScope::Project,
        policy_overrides: Vec::new(),
        suppressions: Vec::new(),
    };
    let audit_receipt = build_audit_run(
        &model,
        &context.file_buffer_diagnostics,
        context.runtime_session_id.clone(),
        context.workspace_revision,
        audit_request,
        zola_evidence.clone(),
    )?;

    let active_target = active_target_identity(&configuration);
    let mut gates = vec![
        workspace_gate(context.dirty, context.workspace_revision),
        disk_gate(disk_coherent, context.disk_generation),
        audit_gate(
            &audit_receipt,
            &request.policy_overrides,
            &request.suppressions,
        ),
        zola_gate(&zola_evidence),
        deploy_target_gate(&configuration),
        deploy_credential_gate(&configuration),
        context_current_gate(),
    ];
    gates.sort_by(|left, right| left.id.cmp(&right.id));

    let receipt = build_publish_preflight_receipt(PublishPreflightReceiptInput {
        project_root: context.project_root.clone(),
        runtime_session_id: context.runtime_session_id.clone(),
        workspace_revision: context.workspace_revision,
        disk_generation: context.disk_generation,
        workspace_dirty: context.dirty,
        disk_coherent,
        observed_disk_fingerprint,
        deploy_settings_revision: configuration.settings.revision,
        deploy_settings_fingerprint: settings_fingerprint.clone(),
        active_target,
        audit_receipt,
        gates,
    })?;
    publish_preflight_receipt_if_current(
        &app,
        &state,
        &context,
        &configuration,
        &settings_fingerprint,
        receipt.clone(),
    )?;
    Ok(receipt)
}

#[tauri::command]
pub fn current_publish_preflight_receipt(
    app: AppHandle,
    state: State<AppState>,
) -> Result<Option<PublishPreflightReceipt>, String> {
    let receipt = state
        .publish_preflight_receipt
        .lock()
        .map_err(|_| "Nu am putut citi receipt-ul Publish Preflight.".to_string())?
        .clone();
    let Some(receipt) = receipt else {
        return Ok(None);
    };
    if publish_preflight_receipt_is_current(&app, &state, &receipt).is_err() {
        invalidate_publish_authorization(&state)?;
        return Ok(None);
    }
    Ok(Some(receipt))
}

#[tauri::command]
pub fn current_publish_build_receipt(
    app: AppHandle,
    state: State<AppState>,
) -> Result<Option<PublishBuildReceipt>, String> {
    let receipt = state
        .publish_build_receipt
        .lock()
        .map_err(|_| "Nu am putut citi receipt-ul buildului pentru publicare.".to_string())?
        .clone();
    let Some(receipt) = receipt else {
        return Ok(None);
    };
    if publish_build_receipt_is_current(&app, &state, &receipt).is_err() {
        clear_publish_build_receipt(&state)?;
        return Ok(None);
    }
    Ok(Some(receipt))
}

pub(crate) fn require_current_publish_preflight(
    app: &AppHandle,
    state: &AppState,
    expected_token: &str,
) -> Result<PublishPreflightReceipt, String> {
    let receipt = state
        .publish_preflight_receipt
        .lock()
        .map_err(|_| "Nu am putut verifica receipt-ul Publish Preflight.".to_string())?
        .clone()
        .ok_or_else(|| {
            "Rulează Publish Preflight înaintea buildului pentru publicare.".to_string()
        })?;
    if receipt.preflight_token != expected_token {
        return Err("Tokenul Publish Preflight nu corespunde receipt-ului curent.".to_string());
    }
    if !receipt.is_ready() {
        return Err(
            "Publish Preflight nu este ready; buildul pentru publicare este blocat.".to_string(),
        );
    }
    publish_preflight_receipt_is_current(app, state, &receipt)?;
    Ok(receipt)
}

pub(crate) fn require_current_publish_build(
    app: &AppHandle,
    state: &AppState,
    expected_build_token: &str,
    expected_artifact_id: &str,
    expected_target_id: &str,
) -> Result<PublishBuildReceipt, String> {
    let receipt = state
        .publish_build_receipt
        .lock()
        .map_err(|_| "Nu am putut verifica receipt-ul buildului pentru publicare.".to_string())?
        .clone()
        .ok_or_else(|| "Rulează buildul pentru publicare înaintea planului deploy.".to_string())?;
    if receipt.build_token != expected_build_token
        || receipt.artifact_id != expected_artifact_id
        || receipt.target_id != expected_target_id
    {
        return Err(
            "Dovada buildului nu corespunde tokenului, artifactului sau țintei cerute.".to_string(),
        );
    }
    publish_build_receipt_is_current(app, state, &receipt)?;
    Ok(receipt)
}

pub(crate) fn invalidate_publish_authorization(state: &AppState) -> Result<(), String> {
    state.clear_publish_authorization()
}

pub(crate) fn store_publish_build_receipt_if_current(
    app: &AppHandle,
    state: &AppState,
    receipt: PublishBuildReceipt,
) -> Result<(), String> {
    let root = current_project_root(state)?;
    let configuration = read_deploy_snapshot(app, &root)?;
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut revalida ProjectWorkspace după build.".to_string())?;
    let workspace = workspace
        .as_ref()
        .ok_or_else(|| "Buildul a devenit stale: proiectul a fost închis.".to_string())?;
    let preflight = state
        .publish_preflight_receipt
        .lock()
        .map_err(|_| "Nu am putut revalida Publish Preflight după build.".to_string())?
        .clone()
        .ok_or_else(|| "Publish Preflight a fost invalidat în timpul buildului.".to_string())?;
    if !publish_workspace_identity_matches(
        &preflight,
        root.to_string_lossy().as_ref(),
        &workspace.runtime_session_id(),
        workspace.revision,
        workspace.accepted_disk.generation,
        workspace.is_dirty(),
    ) {
        return Err("Buildul a devenit stale pentru ProjectWorkspace curent.".to_string());
    }
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        &root,
    )?;
    if !preflight.is_ready()
        || receipt.preflight_token != preflight.preflight_token
        || !publish_build_matches_preflight(&receipt, &preflight)
        || configuration.settings.revision != preflight.deploy_settings_revision
        || deploy_settings_fingerprint(&configuration)? != preflight.deploy_settings_fingerprint
        || active_target_identity(&configuration) != preflight.active_target
    {
        return Err("Buildul nu mai corespunde autorității Publish Preflight curente.".to_string());
    }
    *state
        .publish_build_receipt
        .lock()
        .map_err(|_| "Nu am putut publica receipt-ul buildului pentru publicare.".to_string())? =
        Some(receipt);
    Ok(())
}

pub(crate) fn clear_publish_build_authorization(state: &AppState) -> Result<(), String> {
    clear_publish_build_receipt(state)
}

fn clear_publish_build_receipt(state: &AppState) -> Result<(), String> {
    state
        .publish_build_receipt
        .lock()
        .map_err(|_| "Nu am putut invalida buildul pentru publicare.".to_string())?
        .take();
    Ok(())
}

fn capture_publish_authority_context(state: &AppState) -> Result<PublishAuthorityContext, String> {
    let root = current_project_root(state)?;
    let (
        project_root,
        runtime_session_id,
        workspace_revision,
        disk_generation,
        dirty,
        accepted_disk,
        projection,
        file_buffer_diagnostics,
    ) = {
        let workspace = state.project_workspace.lock().map_err(|_| {
            "Nu am putut captura ProjectWorkspace pentru Publish Preflight.".to_string()
        })?;
        let workspace = workspace.as_ref().ok_or_else(|| {
            "ProjectWorkspace nu este inițializat pentru Publish Preflight.".to_string()
        })?;
        if Path::new(&workspace.session.project_root) != root {
            return Err("Publish Preflight a refuzat un ProjectRoot stale.".to_string());
        }
        let runtime_session_id = workspace.runtime_session_id();
        workspace
            .accepted_disk
            .require_identity(&runtime_session_id, &workspace.session.project_root)?;
        workspace.accepted_disk.require_complete()?;
        (
            workspace.session.project_root.clone(),
            runtime_session_id,
            workspace.revision,
            workspace.accepted_disk.generation,
            workspace.is_dirty(),
            workspace.accepted_disk.clone(),
            workspace.capture_projection_snapshot()?,
            workspace.documents.diagnostics.clone(),
        )
    };
    let observed_disk = read_project_disk_manifest(&root)?;
    if observed_disk.truncated {
        return Err("Publish Preflight nu poate valida un manifest disk trunchiat.".to_string());
    }
    Ok(PublishAuthorityContext {
        root,
        project_root,
        runtime_session_id,
        workspace_revision,
        disk_generation,
        dirty,
        accepted_disk,
        observed_disk,
        projection,
        file_buffer_diagnostics,
    })
}

fn publish_preflight_receipt_if_current(
    app: &AppHandle,
    state: &AppState,
    expected: &PublishAuthorityContext,
    expected_configuration: &DeployConfigurationSnapshot,
    expected_settings_fingerprint: &str,
    receipt: PublishPreflightReceipt,
) -> Result<(), String> {
    let root = current_project_root(state)?;
    if root != expected.root {
        return Err("Publish Preflight a devenit stale: ProjectRoot s-a schimbat.".to_string());
    }
    let observed_disk = read_project_disk_manifest(&root)?;
    if observed_disk != expected.observed_disk {
        return Err("Discul proiectului s-a schimbat în timpul Publish Preflight.".to_string());
    }
    let current_configuration = read_deploy_snapshot(app, &root)?;
    if current_configuration != *expected_configuration
        || deploy_settings_fingerprint(&current_configuration)? != expected_settings_fingerprint
    {
        return Err("Configurația deploy s-a schimbat în timpul Publish Preflight.".to_string());
    }
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut revalida ProjectWorkspace după Preflight.".to_string())?;
    let workspace = workspace
        .as_ref()
        .ok_or_else(|| "Publish Preflight a devenit stale: proiectul a fost închis.".to_string())?;
    if workspace.runtime_session_id() != expected.runtime_session_id
        || workspace.session.project_root != expected.project_root
        || workspace.revision != expected.workspace_revision
        || workspace.accepted_disk != expected.accepted_disk
        || workspace.is_dirty() != expected.dirty
    {
        return Err("Publish Preflight a devenit stale în timpul analizei.".to_string());
    }
    let mut current = state
        .publish_preflight_receipt
        .lock()
        .map_err(|_| "Nu am putut publica receipt-ul Preflight în AppState.".to_string())?;
    *current = Some(receipt);
    Ok(())
}

fn publish_preflight_receipt_is_current(
    app: &AppHandle,
    state: &AppState,
    receipt: &PublishPreflightReceipt,
) -> Result<(), String> {
    let root = current_project_root(state)?;
    let (runtime_session_id, workspace_revision, disk_generation, dirty, accepted_disk) = {
        let workspace = state
            .project_workspace
            .lock()
            .map_err(|_| "Nu am putut valida ProjectWorkspace pentru Publish.".to_string())?;
        let workspace = workspace
            .as_ref()
            .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru Publish.".to_string())?;
        (
            workspace.runtime_session_id(),
            workspace.revision,
            workspace.accepted_disk.generation,
            workspace.is_dirty(),
            workspace.accepted_disk.clone(),
        )
    };
    if !publish_workspace_identity_matches(
        receipt,
        root.to_string_lossy().as_ref(),
        &runtime_session_id,
        workspace_revision,
        disk_generation,
        dirty,
    ) {
        return Err(
            "Publish Preflight receipt este stale pentru ProjectWorkspace curent.".to_string(),
        );
    }
    accepted_disk.require_identity(&runtime_session_id, root.to_string_lossy().as_ref())?;
    accepted_disk.require_complete()?;
    let observed_disk = read_project_disk_manifest(&root)?;
    if observed_disk.truncated
        || project_disk_fingerprint(&observed_disk)? != receipt.observed_disk_fingerprint
        || (observed_disk == accepted_disk.manifest) != receipt.disk_coherent
    {
        return Err("Publish Preflight receipt este stale pentru discul proiectului.".to_string());
    }
    let configuration = read_deploy_snapshot(app, &root)?;
    if configuration.settings.revision != receipt.deploy_settings_revision
        || deploy_settings_fingerprint(&configuration)? != receipt.deploy_settings_fingerprint
        || active_target_identity(&configuration) != receipt.active_target
    {
        return Err(
            "Publish Preflight receipt este stale pentru configurația deploy curentă.".to_string(),
        );
    }
    Ok(())
}

fn publish_build_receipt_is_current(
    app: &AppHandle,
    state: &AppState,
    receipt: &PublishBuildReceipt,
) -> Result<(), String> {
    let preflight = require_current_publish_preflight(app, state, &receipt.preflight_token)?;
    if !publish_build_matches_preflight(receipt, &preflight) {
        return Err("PublishBuildReceipt nu mai corespunde Publish Preflight curent.".to_string());
    }
    Ok(())
}

fn publish_workspace_identity_matches(
    receipt: &PublishPreflightReceipt,
    project_root: &str,
    runtime_session_id: &str,
    workspace_revision: u64,
    disk_generation: u64,
    dirty: bool,
) -> bool {
    receipt.project_root == project_root
        && receipt.runtime_session_id == runtime_session_id
        && receipt.workspace_revision == workspace_revision
        && receipt.disk_generation == disk_generation
        && receipt.workspace_dirty == dirty
}

fn publish_build_matches_preflight(
    receipt: &PublishBuildReceipt,
    preflight: &PublishPreflightReceipt,
) -> bool {
    receipt.project_root == preflight.project_root
        && receipt.runtime_session_id == preflight.runtime_session_id
        && receipt.workspace_revision == preflight.workspace_revision
        && receipt.disk_generation == preflight.disk_generation
        && receipt.project_model_revision == preflight.project_model_revision
        && receipt.deploy_settings_revision == preflight.deploy_settings_revision
        && receipt.deploy_settings_fingerprint == preflight.deploy_settings_fingerprint
        && preflight
            .active_target
            .as_ref()
            .is_some_and(|target| target.target_id == receipt.target_id)
}

fn read_deploy_snapshot(
    app: &AppHandle,
    root: &std::path::Path,
) -> Result<DeployConfigurationSnapshot, String> {
    let config = read_project_app_config_for_bootstrap(app, root)?;
    configuration_snapshot(app, root, config.deploy)
}

fn current_project_root(state: &AppState) -> Result<PathBuf, String> {
    state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut citi ProjectRoot pentru Publish Preflight.".to_string())?
        .clone()
        .ok_or_else(|| "Nu există proiect deschis pentru Publish Preflight.".to_string())
}

fn deploy_settings_fingerprint(
    configuration: &DeployConfigurationSnapshot,
) -> Result<String, String> {
    let bytes = serde_json::to_vec(&configuration.settings)
        .map_err(|error| format!("Configurația deploy nu poate fi amprentată: {error}"))?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

fn project_disk_fingerprint(manifest: &ProjectDiskManifest) -> Result<String, String> {
    let bytes = serde_json::to_vec(manifest)
        .map_err(|error| format!("Manifestul disk nu poate fi amprentat: {error}"))?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

fn active_target_identity(
    configuration: &DeployConfigurationSnapshot,
) -> Option<PublishPreflightTargetIdentity> {
    let target_id = configuration.settings.active_target_id.as_deref()?;
    let target = configuration
        .settings
        .targets
        .iter()
        .find(|target| target.id == target_id)?;
    let status = active_credential_status(configuration)?;
    Some(PublishPreflightTargetIdentity {
        target_id: target.id.clone(),
        provider: target.provider_kind().as_str().to_string(),
        credential_ref: target.credential_ref.clone(),
        credential_kind: credential_kind_name(status.kind).to_string(),
        credential_configured: status.configured,
    })
}

fn active_credential_is_configured(configuration: &DeployConfigurationSnapshot) -> bool {
    active_credential_status(configuration).is_some_and(|status| status.configured)
}

fn active_credential_status(
    configuration: &DeployConfigurationSnapshot,
) -> Option<&DeployCredentialStatus> {
    let target_id = configuration.settings.active_target_id.as_deref()?;
    let target = configuration
        .settings
        .targets
        .iter()
        .find(|target| target.id == target_id)?;
    configuration.credential_statuses.iter().find(|status| {
        status.credential_ref == target.credential_ref
            && credential_kind_supports_provider(status.kind, target.provider_kind())
    })
}

fn credential_kind_supports_provider(
    credential: DeployCredentialKind,
    provider: DeployProviderKind,
) -> bool {
    matches!(
        (credential, provider),
        (DeployCredentialKind::Bunny, DeployProviderKind::Bunny)
            | (DeployCredentialKind::Ftp, DeployProviderKind::Ftp)
            | (
                DeployCredentialKind::SftpPassword | DeployCredentialKind::SftpPrivateKey,
                DeployProviderKind::Sftp
            )
            | (DeployCredentialKind::S3, DeployProviderKind::S3)
            | (
                DeployCredentialKind::CloudflarePages,
                DeployProviderKind::CloudflarePages
            )
    )
}

fn credential_kind_name(kind: DeployCredentialKind) -> &'static str {
    match kind {
        DeployCredentialKind::Bunny => "bunny",
        DeployCredentialKind::Ftp => "ftp",
        DeployCredentialKind::SftpPassword => "sftp_password",
        DeployCredentialKind::SftpPrivateKey => "sftp_private_key",
        DeployCredentialKind::S3 => "s3",
        DeployCredentialKind::CloudflarePages => "cloudflare_pages",
    }
}

fn gate(
    id: &str,
    outcome: PublishPreflightGateOutcome,
    diagnostic: &'static str,
) -> PublishPreflightGate {
    PublishPreflightGate {
        id: id.to_string(),
        outcome,
        diagnostic: LocalizedDiagnostic::new(diagnostic),
        evidence: Vec::new(),
        audit_fingerprints: Vec::new(),
        remediations: Vec::new(),
    }
}

fn evidence(
    kind: PublishPreflightEvidenceKind,
    diagnostic: &'static str,
    value: impl Into<Option<String>>,
) -> PublishPreflightEvidence {
    PublishPreflightEvidence {
        kind,
        diagnostic: LocalizedDiagnostic::new(diagnostic),
        value: value.into(),
    }
}

fn remediation(
    kind: PublishPreflightRemediationKind,
    diagnostic: &'static str,
) -> PublishPreflightRemediation {
    PublishPreflightRemediation {
        kind,
        diagnostic: LocalizedDiagnostic::new(diagnostic),
        location: None,
    }
}

fn workspace_gate(dirty: bool, workspace_revision: u64) -> PublishPreflightGate {
    let mut result = gate(
        "workspace_clean",
        if dirty {
            PublishPreflightGateOutcome::Blocked
        } else {
            PublishPreflightGateOutcome::Passed
        },
        if dirty {
            "publish-preflight-gate-workspace-dirty"
        } else {
            "publish-preflight-gate-workspace-clean"
        },
    );
    result.evidence.push(evidence(
        PublishPreflightEvidenceKind::Workspace,
        "publish-preflight-evidence-workspace-revision",
        Some(workspace_revision.to_string()),
    ));
    if dirty {
        result.remediations.push(remediation(
            PublishPreflightRemediationKind::SaveWorkspace,
            "publish-preflight-remediation-save",
        ));
    }
    result
}

fn disk_gate(coherent: bool, disk_generation: u64) -> PublishPreflightGate {
    let mut result = gate(
        "disk_coherent",
        if coherent {
            PublishPreflightGateOutcome::Passed
        } else {
            PublishPreflightGateOutcome::Blocked
        },
        if coherent {
            "publish-preflight-gate-disk-coherent"
        } else {
            "publish-preflight-gate-disk-changed"
        },
    );
    result.evidence.push(evidence(
        PublishPreflightEvidenceKind::Disk,
        "publish-preflight-evidence-disk-generation",
        Some(disk_generation.to_string()),
    ));
    if !coherent {
        result.remediations.push(remediation(
            PublishPreflightRemediationKind::ReconcileDisk,
            "publish-preflight-remediation-reconcile-disk",
        ));
    }
    result
}

fn audit_gate(
    audit: &crate::kernel::audit::AuditRunReceipt,
    overrides: &[AuditPolicyOverride],
    suppressions: &[AuditSuppression],
) -> PublishPreflightGate {
    let explicit_blocking = overrides
        .iter()
        .filter(|item| item.policy == AuditPolicy::Blocking)
        .map(|item| item.rule_code.as_str())
        .collect::<HashSet<_>>();
    let relevant = audit.findings.iter().collect::<Vec<_>>();
    let suppressed = relevant
        .iter()
        .filter(|finding| publish_suppression_matches(finding, suppressions))
        .map(|finding| finding.fingerprint.as_str())
        .collect::<HashSet<_>>();
    let engine_error = relevant
        .iter()
        .any(|finding| finding.outcome == AuditOutcome::EngineError)
        || audit.providers.iter().any(|provider| {
            provider.publish_coverage_requirement
                == crate::kernel::audit::AuditPublishCoverageRequirement::Required
                && provider.status == crate::kernel::audit::AuditProviderStatus::Failed
        });
    let required_provider_incomplete = audit.providers.iter().any(|provider| {
        provider.publish_coverage_requirement
            == crate::kernel::audit::AuditPublishCoverageRequirement::Required
            && provider.status != crate::kernel::audit::AuditProviderStatus::Complete
    });
    let advisory_provider_incomplete = audit.providers.iter().any(|provider| {
        provider.publish_coverage_requirement
            == crate::kernel::audit::AuditPublishCoverageRequirement::Advisory
            && provider.status != crate::kernel::audit::AuditProviderStatus::Complete
    });
    let blocking = relevant.iter().filter(|finding| {
        !suppressed.contains(finding.fingerprint.as_str())
            && effective_publish_policy(finding, overrides) == AuditPolicy::Blocking
            && (matches!(
                finding.outcome,
                AuditOutcome::Violation | AuditOutcome::EngineError
            ) || finding.outcome == AuditOutcome::NeedsReview
                && explicit_blocking.contains(finding.rule_code.as_str()))
    });
    let blocking_fingerprints = blocking
        .map(|finding| finding.fingerprint.clone())
        .collect::<Vec<_>>();
    let advisory = relevant.iter().any(|finding| {
        !suppressed.contains(finding.fingerprint.as_str())
            && matches!(
                finding.outcome,
                AuditOutcome::Violation | AuditOutcome::NeedsReview
            )
            && matches!(
                effective_publish_policy(finding, overrides),
                AuditPolicy::Advisory | AuditPolicy::Budget | AuditPolicy::Blocking
            )
            && !blocking_fingerprints.contains(&finding.fingerprint)
    });
    let outcome = if engine_error {
        PublishPreflightGateOutcome::EngineError
    } else if !blocking_fingerprints.is_empty() || required_provider_incomplete {
        PublishPreflightGateOutcome::Blocked
    } else if advisory || advisory_provider_incomplete {
        PublishPreflightGateOutcome::Advisory
    } else {
        PublishPreflightGateOutcome::Passed
    };
    let mut result = gate(
        "audit_policy",
        outcome,
        match outcome {
            PublishPreflightGateOutcome::Passed => "publish-preflight-gate-audit-passed",
            PublishPreflightGateOutcome::Blocked if required_provider_incomplete => {
                "publish-preflight-gate-audit-incomplete"
            }
            PublishPreflightGateOutcome::Blocked => "publish-preflight-gate-audit-blocked",
            PublishPreflightGateOutcome::Advisory => "publish-preflight-gate-audit-advisory",
            PublishPreflightGateOutcome::EngineError => "publish-preflight-gate-audit-failed",
            PublishPreflightGateOutcome::Skipped => "publish-preflight-gate-audit-failed",
        },
    );
    result.audit_fingerprints = if outcome == PublishPreflightGateOutcome::Passed {
        Vec::new()
    } else {
        relevant
            .iter()
            .filter(|finding| {
                !suppressed.contains(finding.fingerprint.as_str())
                    && (finding.outcome == AuditOutcome::EngineError
                        || (matches!(
                            finding.outcome,
                            AuditOutcome::Violation | AuditOutcome::NeedsReview
                        ) && effective_publish_policy(finding, overrides) != AuditPolicy::Off))
            })
            .map(|finding| finding.fingerprint.clone())
            .collect()
    };
    result.evidence.push(evidence(
        PublishPreflightEvidenceKind::Audit,
        "publish-preflight-evidence-audit-model",
        Some(audit.project_model_revision.clone()),
    ));
    if outcome != PublishPreflightGateOutcome::Passed {
        result.remediations.push(remediation(
            PublishPreflightRemediationKind::OpenAudit,
            "publish-preflight-remediation-open-audit",
        ));
    }
    result
}

fn effective_publish_policy(
    finding: &crate::kernel::audit::AuditFinding,
    overrides: &[AuditPolicyOverride],
) -> AuditPolicy {
    overrides
        .iter()
        .find(|item| item.rule_code == finding.rule_code)
        .map_or(finding.policy, |item| item.policy)
}

fn publish_suppression_matches(
    finding: &crate::kernel::audit::AuditFinding,
    suppressions: &[AuditSuppression],
) -> bool {
    if !matches!(
        finding.outcome,
        AuditOutcome::Violation | AuditOutcome::NeedsReview
    ) {
        return false;
    }
    suppressions.iter().any(|suppression| {
        suppression.rule_code == finding.rule_code
            && match suppression.scope {
                AuditSuppressionScope::Rule => true,
                AuditSuppressionScope::File => finding
                    .primary_location
                    .as_ref()
                    .zip(suppression.file.as_ref())
                    .is_some_and(|(location, file)| location.file == *file),
                AuditSuppressionScope::Finding => {
                    suppression.fingerprint.as_deref() == Some(finding.fingerprint.as_str())
                }
            }
    })
}

fn validate_publish_policy_request(request: &PublishPreflightRequest) -> Result<(), String> {
    let mut rules = HashSet::new();
    for item in &request.policy_overrides {
        if item.rule_code.trim().is_empty() || !rules.insert(item.rule_code.as_str()) {
            return Err(
                "Publish Preflight a refuzat un override de policy gol sau duplicat.".to_string(),
            );
        }
    }
    for suppression in &request.suppressions {
        if suppression.rule_code.trim().is_empty() || suppression.reason.trim().is_empty() {
            return Err(
                "Publish Preflight a refuzat o suprimare fără rule code sau justificare."
                    .to_string(),
            );
        }
        if suppression.scope == AuditSuppressionScope::File
            && suppression.file.as_deref().is_none_or(str::is_empty)
        {
            return Err(
                "Publish Preflight a refuzat o suprimare de fișier fără fișier.".to_string(),
            );
        }
        if suppression.scope == AuditSuppressionScope::Finding
            && !suppression.fingerprint.as_deref().is_some_and(|value| {
                value.strip_prefix("sha256:").is_some_and(|digest| {
                    digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
            })
        {
            return Err(
                "Publish Preflight a refuzat o suprimare de constatare fără fingerprint valid."
                    .to_string(),
            );
        }
    }
    Ok(())
}

fn zola_gate(build: &AuditBuildEvidence) -> PublishPreflightGate {
    let (outcome, diagnostic, value) = match build {
        AuditBuildEvidence::Complete { message } => (
            PublishPreflightGateOutcome::Passed,
            "publish-preflight-gate-zola-passed",
            message.clone(),
        ),
        AuditBuildEvidence::Failed { message } => (
            PublishPreflightGateOutcome::EngineError,
            "publish-preflight-gate-zola-failed",
            message.clone(),
        ),
        AuditBuildEvidence::Skipped { message } => (
            PublishPreflightGateOutcome::Skipped,
            "publish-preflight-gate-zola-skipped",
            message.clone(),
        ),
    };
    let mut result = gate("zola_check", outcome, diagnostic);
    result.evidence.push(evidence(
        PublishPreflightEvidenceKind::Build,
        "publish-preflight-evidence-zola",
        Some(value),
    ));
    if outcome == PublishPreflightGateOutcome::EngineError {
        result.remediations.push(remediation(
            PublishPreflightRemediationKind::Retry,
            "publish-preflight-remediation-fix-zola",
        ));
    }
    result
}

fn deploy_target_gate(configuration: &DeployConfigurationSnapshot) -> PublishPreflightGate {
    let configured = configuration
        .settings
        .active_target_id
        .as_deref()
        .and_then(|id| {
            configuration
                .settings
                .targets
                .iter()
                .find(|target| target.id == id)
        });
    let mut result = gate(
        "deploy_target",
        if configured.is_some() {
            PublishPreflightGateOutcome::Passed
        } else {
            PublishPreflightGateOutcome::Blocked
        },
        if configured.is_some() {
            "publish-preflight-gate-target-passed"
        } else {
            "publish-preflight-gate-target-blocked"
        },
    );
    result.evidence.push(evidence(
        PublishPreflightEvidenceKind::DeployConfiguration,
        "publish-preflight-evidence-settings-revision",
        Some(configuration.settings.revision.to_string()),
    ));
    if configured.is_none() {
        result.remediations.push(remediation(
            PublishPreflightRemediationKind::ConfigureDeploy,
            "publish-preflight-remediation-configure-target",
        ));
    }
    result
}

fn deploy_credential_gate(configuration: &DeployConfigurationSnapshot) -> PublishPreflightGate {
    let configured = active_credential_is_configured(configuration);
    let mut result = gate(
        "deploy_credentials",
        if configured {
            PublishPreflightGateOutcome::Passed
        } else {
            PublishPreflightGateOutcome::Blocked
        },
        if configured {
            "publish-preflight-gate-credentials-passed"
        } else {
            "publish-preflight-gate-credentials-blocked"
        },
    );
    if let Some(target) = active_target_identity(configuration) {
        result.evidence.push(evidence(
            PublishPreflightEvidenceKind::Credentials,
            "publish-preflight-evidence-credential-reference",
            Some(format!(
                "{} ({})",
                target.credential_ref, target.credential_kind
            )),
        ));
    }
    if !configured {
        result.remediations.push(remediation(
            PublishPreflightRemediationKind::ConfigureCredentials,
            "publish-preflight-remediation-configure-credentials",
        ));
    }
    result
}

fn context_current_gate() -> PublishPreflightGate {
    let mut result = gate(
        "context_current",
        PublishPreflightGateOutcome::Passed,
        "publish-preflight-gate-context-current",
    );
    result.evidence.push(evidence(
        PublishPreflightEvidenceKind::Runtime,
        "publish-preflight-evidence-context-revalidated",
        None,
    ));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deploy::{
        BunnyTargetConfig, DeployCleanupPolicy, DeployCredentialStatus, DeploySettings,
        DeployTarget, DeployTargetProvider, S3TargetConfig,
    };
    use crate::kernel::audit::{
        AuditCategory, AuditCompleteness, AuditCoverage, AuditFinding, AuditImpact,
        AuditProviderKind, AuditProviderReceipt, AuditProviderStatus, AuditRunReceipt,
        AuditSummary, AUDIT_RULESET_VERSION, AUDIT_RUN_SCHEMA_VERSION,
    };
    use crate::kernel::publish_preflight::{
        build_publish_build_receipt, build_publish_preflight_receipt, PublishBuildReceiptInput,
        PublishPreflightReceiptInput,
    };

    fn audit_with_review() -> AuditRunReceipt {
        AuditRunReceipt {
            schema_version: AUDIT_RUN_SCHEMA_VERSION,
            ruleset_version: AUDIT_RULESET_VERSION,
            project_root: "/project".to_string(),
            runtime_session_id: "session".to_string(),
            workspace_revision: 1,
            project_model_revision: "model".to_string(),
            mode: AuditRunMode::Full,
            scope: AuditScope::Project,
            completeness: AuditCompleteness::Complete,
            summary: AuditSummary::default(),
            providers: Vec::new(),
            findings: vec![AuditFinding {
                id: "audit:review".to_string(),
                fingerprint: "sha256:review".to_string(),
                provider_id: "content_semantics".to_string(),
                rule_code: "review".to_string(),
                category: AuditCategory::Content,
                outcome: AuditOutcome::NeedsReview,
                impact: AuditImpact::Moderate,
                policy: AuditPolicy::Blocking,
                title_diagnostic: LocalizedDiagnostic::new("audit-provider-pass-title"),
                message_diagnostic: LocalizedDiagnostic::new("audit-provider-pass-message"),
                primary_location: None,
                related_locations: Vec::new(),
                evidence: Vec::new(),
                fixes: Vec::new(),
                suppression: None,
            }],
        }
    }

    #[test]
    fn needs_review_blocks_only_when_policy_is_explicitly_blocking() {
        let audit = audit_with_review();
        assert_eq!(
            audit_gate(&audit, &[], &[]).outcome,
            PublishPreflightGateOutcome::Advisory
        );
        assert_eq!(
            audit_gate(
                &audit,
                &[AuditPolicyOverride {
                    rule_code: "review".to_string(),
                    policy: AuditPolicy::Blocking,
                }],
                &[],
            )
            .outcome,
            PublishPreflightGateOutcome::Blocked
        );
        assert_eq!(
            audit_gate(
                &audit,
                &[AuditPolicyOverride {
                    rule_code: "review".to_string(),
                    policy: AuditPolicy::Off,
                }],
                &[],
            )
            .outcome,
            PublishPreflightGateOutcome::Passed
        );
        assert_eq!(
            audit_gate(
                &audit,
                &[AuditPolicyOverride {
                    rule_code: "review".to_string(),
                    policy: AuditPolicy::Budget,
                }],
                &[],
            )
            .outcome,
            PublishPreflightGateOutcome::Advisory
        );
    }

    #[test]
    fn dirty_disk_and_zola_failure_have_independent_fail_closed_gates() {
        let dirty = workspace_gate(true, 12);
        assert_eq!(dirty.outcome, PublishPreflightGateOutcome::Blocked);
        assert_eq!(
            dirty.remediations[0].kind,
            PublishPreflightRemediationKind::SaveWorkspace
        );

        let disk = disk_gate(false, 4);
        assert_eq!(disk.outcome, PublishPreflightGateOutcome::Blocked);
        assert_eq!(
            disk.remediations[0].kind,
            PublishPreflightRemediationKind::ReconcileDisk
        );

        let zola = zola_gate(&AuditBuildEvidence::Failed {
            message: "template invalid".to_string(),
        });
        assert_eq!(zola.outcome, PublishPreflightGateOutcome::EngineError);
        assert_eq!(zola.evidence[0].value.as_deref(), Some("template invalid"));
    }

    #[test]
    fn blocking_audit_violation_blocks_but_advisory_review_does_not() {
        let mut audit = audit_with_review();
        audit.findings[0].outcome = AuditOutcome::Violation;
        assert_eq!(
            audit_gate(&audit, &[], &[]).outcome,
            PublishPreflightGateOutcome::Blocked
        );

        assert_eq!(
            audit_gate(
                &audit,
                &[],
                &[AuditSuppression {
                    rule_code: "review".to_string(),
                    file: None,
                    fingerprint: None,
                    scope: AuditSuppressionScope::Rule,
                    reason: "acceptat explicit pentru publicare".to_string(),
                }],
            )
            .outcome,
            PublishPreflightGateOutcome::Passed
        );

        audit.findings[0].outcome = AuditOutcome::NeedsReview;
        assert_eq!(
            audit_gate(&audit, &[], &[]).outcome,
            PublishPreflightGateOutcome::Advisory
        );

        audit.findings.clear();
        audit.providers.push(AuditProviderReceipt {
            id: "content_semantics".to_string(),
            kind: AuditProviderKind::ProjectGraph,
            status: AuditProviderStatus::Partial,
            publish_coverage_requirement:
                crate::kernel::audit::AuditPublishCoverageRequirement::Required,
            finding_count: 0,
            coverage: AuditCoverage {
                eligible: 1,
                analyzed: 0,
                limitations: Vec::new(),
            },
            evidence: Vec::new(),
        });
        assert_eq!(
            audit_gate(&audit, &[], &[]).outcome,
            PublishPreflightGateOutcome::Blocked
        );
        audit.providers[0].publish_coverage_requirement =
            crate::kernel::audit::AuditPublishCoverageRequirement::Advisory;
        assert_eq!(
            audit_gate(&audit, &[], &[]).outcome,
            PublishPreflightGateOutcome::Advisory
        );
    }

    #[test]
    fn target_and_credential_gates_are_local_and_typed() {
        let no_target = DeployConfigurationSnapshot {
            schema_version: 1,
            settings: DeploySettings::default(),
            credential_statuses: Vec::new(),
            target_capabilities: Vec::new(),
            legacy_bunny_fallback: false,
        };
        assert_eq!(
            deploy_target_gate(&no_target).outcome,
            PublishPreflightGateOutcome::Blocked
        );

        let target = DeployTarget {
            id: "production".to_string(),
            name: "Production".to_string(),
            credential_ref: "production-credentials".to_string(),
            cleanup_policy: DeployCleanupPolicy::ManagedOnly,
            provider: DeployTargetProvider::Bunny(BunnyTargetConfig {
                storage_zone: "pana-studio".to_string(),
                storage_region: "de".to_string(),
                pull_zone_id: "1516140".to_string(),
                remote_prefix: String::new(),
            }),
        };
        let mut configuration = DeployConfigurationSnapshot {
            schema_version: 1,
            settings: DeploySettings {
                schema_version: 1,
                revision: 3,
                active_target_id: Some(target.id.clone()),
                targets: vec![target],
            },
            credential_statuses: vec![DeployCredentialStatus {
                schema_version: 1,
                credential_ref: "production-credentials".to_string(),
                kind: DeployCredentialKind::Bunny,
                configured: false,
            }],
            target_capabilities: Vec::new(),
            legacy_bunny_fallback: false,
        };
        assert_eq!(
            deploy_target_gate(&configuration).outcome,
            PublishPreflightGateOutcome::Passed
        );
        assert_eq!(
            deploy_credential_gate(&configuration).outcome,
            PublishPreflightGateOutcome::Blocked
        );
        assert!(
            !active_target_identity(&configuration)
                .unwrap()
                .credential_configured
        );

        configuration.credential_statuses[0].configured = true;
        assert_eq!(
            deploy_credential_gate(&configuration).outcome,
            PublishPreflightGateOutcome::Passed
        );

        let shared_ref_s3 = DeployTarget {
            id: "staging".to_string(),
            name: "Staging".to_string(),
            credential_ref: "production-credentials".to_string(),
            cleanup_policy: DeployCleanupPolicy::ManagedOnly,
            provider: DeployTargetProvider::S3(S3TargetConfig {
                bucket: "staging".to_string(),
                prefix: String::new(),
                region: "eu-central-1".to_string(),
                endpoint: None,
                force_path_style: false,
                allow_insecure_endpoint: false,
                cache_control: None,
            }),
        };
        configuration.settings.targets.push(shared_ref_s3);
        configuration.settings.active_target_id = Some("staging".to_string());
        configuration
            .credential_statuses
            .push(DeployCredentialStatus {
                schema_version: 1,
                credential_ref: "production-credentials".to_string(),
                kind: DeployCredentialKind::S3,
                configured: false,
            });
        assert_eq!(
            deploy_credential_gate(&configuration).outcome,
            PublishPreflightGateOutcome::Blocked
        );
        assert_eq!(
            active_target_identity(&configuration)
                .unwrap()
                .credential_kind,
            "s3"
        );
    }

    #[test]
    fn publish_policy_request_rejects_ambiguous_overrides_and_suppressions() {
        let duplicate = PublishPreflightRequest {
            policy_overrides: vec![
                AuditPolicyOverride {
                    rule_code: "review".to_string(),
                    policy: AuditPolicy::Blocking,
                },
                AuditPolicyOverride {
                    rule_code: "review".to_string(),
                    policy: AuditPolicy::Off,
                },
            ],
            suppressions: Vec::new(),
        };
        assert!(validate_publish_policy_request(&duplicate).is_err());

        let invalid_suppression = PublishPreflightRequest {
            policy_overrides: Vec::new(),
            suppressions: vec![AuditSuppression {
                rule_code: "review".to_string(),
                file: None,
                fingerprint: None,
                scope: AuditSuppressionScope::Finding,
                reason: "motiv".to_string(),
            }],
        };
        assert!(validate_publish_policy_request(&invalid_suppression).is_err());
    }

    fn ready_preflight() -> PublishPreflightReceipt {
        let audit = AuditRunReceipt {
            findings: Vec::new(),
            ..audit_with_review()
        };
        let gates = [
            "workspace_clean",
            "disk_coherent",
            "audit_policy",
            "zola_check",
            "deploy_target",
            "deploy_credentials",
            "context_current",
        ]
        .into_iter()
        .map(|id| {
            gate(
                id,
                PublishPreflightGateOutcome::Passed,
                "publish-preflight-gate-context-current",
            )
        })
        .collect();
        build_publish_preflight_receipt(PublishPreflightReceiptInput {
            project_root: "/project".to_string(),
            runtime_session_id: "session-a".to_string(),
            workspace_revision: 9,
            disk_generation: 5,
            workspace_dirty: false,
            disk_coherent: true,
            observed_disk_fingerprint: "sha256:disk".to_string(),
            deploy_settings_revision: 3,
            deploy_settings_fingerprint: "sha256:settings".to_string(),
            active_target: Some(PublishPreflightTargetIdentity {
                target_id: "production".to_string(),
                provider: "bunny".to_string(),
                credential_ref: "production-credentials".to_string(),
                credential_kind: "bunny".to_string(),
                credential_configured: true,
            }),
            audit_receipt: audit,
            gates,
        })
        .unwrap()
    }

    #[test]
    fn workspace_currency_matches_captured_dirty_state_and_rejects_changed_identity() {
        let receipt = ready_preflight();
        assert!(publish_workspace_identity_matches(
            &receipt,
            "/project",
            "session-a",
            9,
            5,
            false
        ));
        assert!(!publish_workspace_identity_matches(
            &receipt,
            "/project",
            "session-b",
            9,
            5,
            false
        ));
        assert!(!publish_workspace_identity_matches(
            &receipt,
            "/project",
            "session-a",
            9,
            6,
            false
        ));
        assert!(!publish_workspace_identity_matches(
            &receipt,
            "/project",
            "session-a",
            9,
            5,
            true
        ));
        let mut dirty_receipt = receipt;
        dirty_receipt.workspace_dirty = true;
        assert!(publish_workspace_identity_matches(
            &dirty_receipt,
            "/project",
            "session-a",
            9,
            5,
            true
        ));
    }

    #[test]
    fn build_receipt_cannot_be_reused_after_preflight_identity_changes() {
        let preflight = ready_preflight();
        let build = build_publish_build_receipt(PublishBuildReceiptInput {
            project_root: preflight.project_root.clone(),
            runtime_session_id: preflight.runtime_session_id.clone(),
            workspace_revision: preflight.workspace_revision,
            disk_generation: preflight.disk_generation,
            project_model_revision: preflight.project_model_revision.clone(),
            deploy_settings_revision: preflight.deploy_settings_revision,
            deploy_settings_fingerprint: preflight.deploy_settings_fingerprint.clone(),
            target_id: "production".to_string(),
            preflight_token: preflight.preflight_token.clone(),
            artifact_id: "sha256:artifact".to_string(),
            artifact_files: 2,
            artifact_bytes: 42,
            completed_at_ms: 1,
            log: "ok".to_string(),
        });
        assert!(publish_build_matches_preflight(&build, &preflight));

        let mut changed = preflight.clone();
        changed.workspace_revision += 1;
        assert!(!publish_build_matches_preflight(&build, &changed));
        changed = preflight.clone();
        changed.deploy_settings_revision += 1;
        assert!(!publish_build_matches_preflight(&build, &changed));
    }
}
