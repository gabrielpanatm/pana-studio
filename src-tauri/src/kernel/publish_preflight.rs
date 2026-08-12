use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    kernel::audit::{
        AuditLocation, AuditRunReceipt, AUDIT_RULESET_VERSION, AUDIT_RUN_SCHEMA_VERSION,
    },
    localization::LocalizedDiagnostic,
};

pub const PUBLISH_PREFLIGHT_SCHEMA_VERSION: u32 = 1;
pub const PUBLISH_BUILD_RECEIPT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PublishPreflightStatus {
    Ready,
    Blocked,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PublishPreflightGateOutcome {
    Passed,
    Blocked,
    Advisory,
    Skipped,
    EngineError,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PublishPreflightEvidenceKind {
    Workspace,
    Disk,
    Audit,
    Build,
    DeployConfiguration,
    Credentials,
    Runtime,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishPreflightEvidence {
    pub kind: PublishPreflightEvidenceKind,
    pub diagnostic: LocalizedDiagnostic,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PublishPreflightRemediationKind {
    SaveWorkspace,
    ReconcileDisk,
    OpenAudit,
    OpenSource,
    ConfigureDeploy,
    ConfigureCredentials,
    Retry,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishPreflightRemediation {
    pub kind: PublishPreflightRemediationKind,
    pub diagnostic: LocalizedDiagnostic,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<AuditLocation>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishPreflightGate {
    pub id: String,
    pub outcome: PublishPreflightGateOutcome,
    pub diagnostic: LocalizedDiagnostic,
    pub evidence: Vec<PublishPreflightEvidence>,
    pub audit_fingerprints: Vec<String>,
    pub remediations: Vec<PublishPreflightRemediation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishPreflightAuditIdentity {
    pub schema_version: u32,
    pub ruleset_version: u32,
    pub receipt_id: String,
    pub project_model_revision: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishPreflightTargetIdentity {
    pub target_id: String,
    pub provider: String,
    pub credential_ref: String,
    pub credential_kind: String,
    pub credential_configured: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishPreflightReceipt {
    pub schema_version: u32,
    pub project_root: String,
    pub runtime_session_id: String,
    pub workspace_revision: u64,
    pub disk_generation: u64,
    pub workspace_dirty: bool,
    pub disk_coherent: bool,
    pub observed_disk_fingerprint: String,
    pub project_model_revision: String,
    pub deploy_settings_revision: u64,
    pub deploy_settings_fingerprint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_target: Option<PublishPreflightTargetIdentity>,
    pub audit_identity: PublishPreflightAuditIdentity,
    pub audit_receipt: AuditRunReceipt,
    pub status: PublishPreflightStatus,
    pub gates: Vec<PublishPreflightGate>,
    pub preflight_token: String,
}

impl PublishPreflightReceipt {
    pub fn is_ready(&self) -> bool {
        self.status == PublishPreflightStatus::Ready
    }
}

pub struct PublishPreflightReceiptInput {
    pub project_root: String,
    pub runtime_session_id: String,
    pub workspace_revision: u64,
    pub disk_generation: u64,
    pub workspace_dirty: bool,
    pub disk_coherent: bool,
    pub observed_disk_fingerprint: String,
    pub deploy_settings_revision: u64,
    pub deploy_settings_fingerprint: String,
    pub active_target: Option<PublishPreflightTargetIdentity>,
    pub audit_receipt: AuditRunReceipt,
    pub gates: Vec<PublishPreflightGate>,
}

pub fn build_publish_preflight_receipt(
    input: PublishPreflightReceiptInput,
) -> Result<PublishPreflightReceipt, String> {
    require_stable_gate_ids(&input.gates)?;
    require_publish_gate_contract(&input.gates)?;
    let status = if input
        .gates
        .iter()
        .any(|gate| gate.outcome == PublishPreflightGateOutcome::EngineError)
    {
        PublishPreflightStatus::Failed
    } else if !publish_gates_are_ready(&input.gates) {
        PublishPreflightStatus::Blocked
    } else {
        PublishPreflightStatus::Ready
    };
    let audit_receipt_id = audit_receipt_id(&input.audit_receipt)?;
    let audit_identity = PublishPreflightAuditIdentity {
        schema_version: input.audit_receipt.schema_version,
        ruleset_version: input.audit_receipt.ruleset_version,
        receipt_id: audit_receipt_id,
        project_model_revision: input.audit_receipt.project_model_revision.clone(),
    };
    let preflight_token = preflight_token(&input, status, &audit_identity);
    Ok(PublishPreflightReceipt {
        schema_version: PUBLISH_PREFLIGHT_SCHEMA_VERSION,
        project_root: input.project_root,
        runtime_session_id: input.runtime_session_id,
        workspace_revision: input.workspace_revision,
        disk_generation: input.disk_generation,
        workspace_dirty: input.workspace_dirty,
        disk_coherent: input.disk_coherent,
        observed_disk_fingerprint: input.observed_disk_fingerprint,
        project_model_revision: input.audit_receipt.project_model_revision.clone(),
        deploy_settings_revision: input.deploy_settings_revision,
        deploy_settings_fingerprint: input.deploy_settings_fingerprint,
        active_target: input.active_target,
        audit_identity,
        audit_receipt: input.audit_receipt,
        status,
        gates: input.gates,
        preflight_token,
    })
}

const REQUIRED_PUBLISH_GATES: [&str; 7] = [
    "workspace_clean",
    "disk_coherent",
    "audit_policy",
    "zola_check",
    "deploy_target",
    "deploy_credentials",
    "context_current",
];

fn require_publish_gate_contract(gates: &[PublishPreflightGate]) -> Result<(), String> {
    if REQUIRED_PUBLISH_GATES
        .iter()
        .any(|required| !gates.iter().any(|gate| gate.id == *required))
    {
        return Err(
            "Publish Preflight a refuzat un set incomplet de gate-uri obligatorii.".to_string(),
        );
    }
    Ok(())
}

fn publish_gates_are_ready(gates: &[PublishPreflightGate]) -> bool {
    REQUIRED_PUBLISH_GATES.iter().all(|required| {
        gates
            .iter()
            .find(|gate| gate.id == *required)
            .is_some_and(|gate| {
                gate.outcome == PublishPreflightGateOutcome::Passed
                    || (*required == "audit_policy"
                        && gate.outcome == PublishPreflightGateOutcome::Advisory)
            })
    })
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishBuildReceipt {
    pub schema_version: u32,
    pub project_root: String,
    pub runtime_session_id: String,
    pub workspace_revision: u64,
    pub disk_generation: u64,
    pub project_model_revision: String,
    pub deploy_settings_revision: u64,
    pub deploy_settings_fingerprint: String,
    pub target_id: String,
    pub preflight_token: String,
    pub artifact_id: String,
    pub artifact_files: u64,
    pub artifact_bytes: u64,
    pub completed_at_ms: u128,
    pub build_token: String,
    pub log: String,
}

impl std::fmt::Display for PublishBuildReceipt {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Build pentru publicare {}: {} fișiere, {} bytes, artifact {}.",
            self.target_id, self.artifact_files, self.artifact_bytes, self.artifact_id
        )
    }
}

pub struct PublishBuildReceiptInput {
    pub project_root: String,
    pub runtime_session_id: String,
    pub workspace_revision: u64,
    pub disk_generation: u64,
    pub project_model_revision: String,
    pub deploy_settings_revision: u64,
    pub deploy_settings_fingerprint: String,
    pub target_id: String,
    pub preflight_token: String,
    pub artifact_id: String,
    pub artifact_files: u64,
    pub artifact_bytes: u64,
    pub completed_at_ms: u128,
    pub log: String,
}

pub fn build_publish_build_receipt(input: PublishBuildReceiptInput) -> PublishBuildReceipt {
    let build_token = publish_build_token(&input);
    PublishBuildReceipt {
        schema_version: PUBLISH_BUILD_RECEIPT_SCHEMA_VERSION,
        project_root: input.project_root,
        runtime_session_id: input.runtime_session_id,
        workspace_revision: input.workspace_revision,
        disk_generation: input.disk_generation,
        project_model_revision: input.project_model_revision,
        deploy_settings_revision: input.deploy_settings_revision,
        deploy_settings_fingerprint: input.deploy_settings_fingerprint,
        target_id: input.target_id,
        preflight_token: input.preflight_token,
        artifact_id: input.artifact_id,
        artifact_files: input.artifact_files,
        artifact_bytes: input.artifact_bytes,
        completed_at_ms: input.completed_at_ms,
        build_token,
        log: input.log,
    }
}

fn require_stable_gate_ids(gates: &[PublishPreflightGate]) -> Result<(), String> {
    let mut ids = std::collections::HashSet::new();
    for gate in gates {
        if gate.id.is_empty()
            || !gate
                .id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
            || !ids.insert(gate.id.as_str())
        {
            return Err(
                "Publish Preflight a refuzat un ID de gate instabil sau duplicat.".to_string(),
            );
        }
    }
    Ok(())
}

fn audit_receipt_id(receipt: &AuditRunReceipt) -> Result<String, String> {
    if receipt.schema_version != AUDIT_RUN_SCHEMA_VERSION
        || receipt.ruleset_version != AUDIT_RULESET_VERSION
    {
        return Err(
            "Publish Preflight a refuzat un AuditRunReceipt cu schemă incompatibilă.".to_string(),
        );
    }
    let bytes = serde_json::to_vec(receipt)
        .map_err(|error| format!("AuditRunReceipt nu poate fi amprentat: {error}"))?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

fn preflight_token(
    input: &PublishPreflightReceiptInput,
    status: PublishPreflightStatus,
    audit: &PublishPreflightAuditIdentity,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"pana-publish-preflight-v1\0");
    hash_field(&mut digest, input.project_root.as_bytes());
    hash_field(&mut digest, input.runtime_session_id.as_bytes());
    digest.update(input.workspace_revision.to_be_bytes());
    digest.update(input.disk_generation.to_be_bytes());
    digest.update([u8::from(input.workspace_dirty)]);
    digest.update([u8::from(input.disk_coherent)]);
    hash_field(&mut digest, input.observed_disk_fingerprint.as_bytes());
    digest.update(input.deploy_settings_revision.to_be_bytes());
    hash_field(&mut digest, input.deploy_settings_fingerprint.as_bytes());
    digest.update(audit.schema_version.to_be_bytes());
    digest.update(audit.ruleset_version.to_be_bytes());
    hash_field(&mut digest, audit.project_model_revision.as_bytes());
    hash_field(&mut digest, audit.receipt_id.as_bytes());
    digest.update([preflight_status_tag(status)]);
    if let Some(target) = &input.active_target {
        digest.update([1]);
        hash_field(&mut digest, target.target_id.as_bytes());
        hash_field(&mut digest, target.provider.as_bytes());
        hash_field(&mut digest, target.credential_ref.as_bytes());
        hash_field(&mut digest, target.credential_kind.as_bytes());
        digest.update([u8::from(target.credential_configured)]);
    } else {
        digest.update([0]);
    }
    for gate in &input.gates {
        hash_field(&mut digest, gate.id.as_bytes());
        digest.update([preflight_gate_outcome_tag(gate.outcome)]);
        for fingerprint in &gate.audit_fingerprints {
            hash_field(&mut digest, fingerprint.as_bytes());
        }
    }
    format!("sha256:{:x}", digest.finalize())
}

fn publish_build_token(input: &PublishBuildReceiptInput) -> String {
    let mut digest = Sha256::new();
    digest.update(b"pana-publish-build-v1\0");
    hash_field(&mut digest, input.preflight_token.as_bytes());
    hash_field(&mut digest, input.project_root.as_bytes());
    hash_field(&mut digest, input.runtime_session_id.as_bytes());
    digest.update(input.workspace_revision.to_be_bytes());
    digest.update(input.disk_generation.to_be_bytes());
    hash_field(&mut digest, input.project_model_revision.as_bytes());
    digest.update(input.deploy_settings_revision.to_be_bytes());
    hash_field(&mut digest, input.deploy_settings_fingerprint.as_bytes());
    hash_field(&mut digest, input.target_id.as_bytes());
    hash_field(&mut digest, input.artifact_id.as_bytes());
    format!("sha256:{:x}", digest.finalize())
}

fn preflight_status_tag(status: PublishPreflightStatus) -> u8 {
    match status {
        PublishPreflightStatus::Ready => 1,
        PublishPreflightStatus::Blocked => 2,
        PublishPreflightStatus::Failed => 3,
    }
}

fn preflight_gate_outcome_tag(outcome: PublishPreflightGateOutcome) -> u8 {
    match outcome {
        PublishPreflightGateOutcome::Passed => 1,
        PublishPreflightGateOutcome::Blocked => 2,
        PublishPreflightGateOutcome::Advisory => 3,
        PublishPreflightGateOutcome::Skipped => 4,
        PublishPreflightGateOutcome::EngineError => 5,
    }
}

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_be_bytes());
    digest.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::audit::{AuditCompleteness, AuditRunMode, AuditScope, AuditSummary};

    fn audit() -> AuditRunReceipt {
        AuditRunReceipt {
            schema_version: AUDIT_RUN_SCHEMA_VERSION,
            ruleset_version: AUDIT_RULESET_VERSION,
            project_root: "/project".to_string(),
            runtime_session_id: "session-a".to_string(),
            workspace_revision: 7,
            project_model_revision: "model-a".to_string(),
            mode: AuditRunMode::Full,
            scope: AuditScope::Project,
            completeness: AuditCompleteness::Complete,
            summary: AuditSummary::default(),
            providers: Vec::new(),
            findings: Vec::new(),
        }
    }

    fn input(outcome: PublishPreflightGateOutcome) -> PublishPreflightReceiptInput {
        PublishPreflightReceiptInput {
            project_root: "/project".to_string(),
            runtime_session_id: "session-a".to_string(),
            workspace_revision: 7,
            disk_generation: 3,
            workspace_dirty: false,
            disk_coherent: true,
            observed_disk_fingerprint: "sha256:disk".to_string(),
            deploy_settings_revision: 11,
            deploy_settings_fingerprint: "sha256:settings".to_string(),
            active_target: None,
            audit_receipt: audit(),
            gates: REQUIRED_PUBLISH_GATES
                .iter()
                .map(|id| PublishPreflightGate {
                    id: (*id).to_string(),
                    outcome: if *id == "workspace_clean" {
                        outcome
                    } else {
                        PublishPreflightGateOutcome::Passed
                    },
                    diagnostic: LocalizedDiagnostic::new("publish-preflight-gate-workspace-clean"),
                    evidence: Vec::new(),
                    audit_fingerprints: Vec::new(),
                    remediations: Vec::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn receipt_status_is_derived_only_from_rust_gates() {
        assert_eq!(
            build_publish_preflight_receipt(input(PublishPreflightGateOutcome::Passed))
                .unwrap()
                .status,
            PublishPreflightStatus::Ready
        );
        assert_eq!(
            build_publish_preflight_receipt(input(PublishPreflightGateOutcome::Blocked))
                .unwrap()
                .status,
            PublishPreflightStatus::Blocked
        );
        assert_eq!(
            build_publish_preflight_receipt(input(PublishPreflightGateOutcome::EngineError))
                .unwrap()
                .status,
            PublishPreflightStatus::Failed
        );
    }

    #[test]
    fn token_is_bound_to_session_and_all_authority_revisions() {
        let first =
            build_publish_preflight_receipt(input(PublishPreflightGateOutcome::Passed)).unwrap();
        let mut changed = input(PublishPreflightGateOutcome::Passed);
        changed.runtime_session_id = "session-b".to_string();
        let second = build_publish_preflight_receipt(changed).unwrap();
        assert_ne!(first.preflight_token, second.preflight_token);

        let mut changed = input(PublishPreflightGateOutcome::Passed);
        changed.disk_generation += 1;
        let third = build_publish_preflight_receipt(changed).unwrap();
        assert_ne!(first.preflight_token, third.preflight_token);
    }

    #[test]
    fn duplicate_or_unstable_gate_ids_are_rejected() {
        let mut candidate = input(PublishPreflightGateOutcome::Passed);
        candidate.gates[0].id = "Workspace Clean".to_string();
        assert!(build_publish_preflight_receipt(candidate).is_err());
    }

    #[test]
    fn ready_requires_every_mandatory_gate_and_confirmed_zola() {
        let mut missing = input(PublishPreflightGateOutcome::Passed);
        missing.gates.retain(|gate| gate.id != "zola_check");
        assert!(build_publish_preflight_receipt(missing).is_err());

        let mut skipped = input(PublishPreflightGateOutcome::Passed);
        skipped
            .gates
            .iter_mut()
            .find(|gate| gate.id == "zola_check")
            .unwrap()
            .outcome = PublishPreflightGateOutcome::Skipped;
        assert_eq!(
            build_publish_preflight_receipt(skipped).unwrap().status,
            PublishPreflightStatus::Blocked
        );

        let mut advisory = input(PublishPreflightGateOutcome::Passed);
        advisory
            .gates
            .iter_mut()
            .find(|gate| gate.id == "audit_policy")
            .unwrap()
            .outcome = PublishPreflightGateOutcome::Advisory;
        assert_eq!(
            build_publish_preflight_receipt(advisory).unwrap().status,
            PublishPreflightStatus::Ready
        );
    }

    #[test]
    fn zola_engine_failure_keeps_the_audit_inside_a_failed_receipt() {
        let mut candidate = input(PublishPreflightGateOutcome::Passed);
        candidate
            .gates
            .iter_mut()
            .find(|gate| gate.id == "zola_check")
            .unwrap()
            .outcome = PublishPreflightGateOutcome::EngineError;
        let receipt = build_publish_preflight_receipt(candidate).unwrap();
        assert_eq!(receipt.status, PublishPreflightStatus::Failed);
        assert_eq!(receipt.audit_receipt.project_model_revision, "model-a");
        assert_eq!(receipt.audit_identity.project_model_revision, "model-a");
    }

    #[test]
    fn target_credential_state_is_part_of_the_preflight_token() {
        let mut configured = input(PublishPreflightGateOutcome::Passed);
        configured.active_target = Some(PublishPreflightTargetIdentity {
            target_id: "production".to_string(),
            provider: "bunny".to_string(),
            credential_ref: "production-credentials".to_string(),
            credential_kind: "bunny".to_string(),
            credential_configured: true,
        });
        let first = build_publish_preflight_receipt(configured).unwrap();

        let mut missing = input(PublishPreflightGateOutcome::Passed);
        missing.active_target = Some(PublishPreflightTargetIdentity {
            target_id: "production".to_string(),
            provider: "bunny".to_string(),
            credential_ref: "production-credentials".to_string(),
            credential_kind: "bunny".to_string(),
            credential_configured: false,
        });
        let second = build_publish_preflight_receipt(missing).unwrap();
        assert_ne!(first.preflight_token, second.preflight_token);
    }

    #[test]
    fn receipt_serialization_is_versioned_camel_case_and_secret_free() {
        let mut candidate = input(PublishPreflightGateOutcome::Passed);
        candidate.active_target = Some(PublishPreflightTargetIdentity {
            target_id: "production".to_string(),
            provider: "s3".to_string(),
            credential_ref: "production-credentials".to_string(),
            credential_kind: "s3".to_string(),
            credential_configured: true,
        });
        let value =
            serde_json::to_value(build_publish_preflight_receipt(candidate).unwrap()).unwrap();
        assert_eq!(value["schemaVersion"], PUBLISH_PREFLIGHT_SCHEMA_VERSION);
        assert_eq!(value["status"], "ready");
        assert!(value["preflightToken"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
        assert_eq!(value["activeTarget"]["credentialConfigured"], true);
        let serialized = value.to_string();
        assert!(!serialized.contains("secretAccessKey"));
        assert!(!serialized.contains("storageKey"));
        assert!(!serialized.contains("apiToken"));
    }

    #[test]
    fn publish_build_token_cannot_cross_sessions_or_artifacts() {
        let make = |session: &str, artifact: &str| PublishBuildReceiptInput {
            project_root: "/project".to_string(),
            runtime_session_id: session.to_string(),
            workspace_revision: 7,
            disk_generation: 3,
            project_model_revision: "model-a".to_string(),
            deploy_settings_revision: 11,
            deploy_settings_fingerprint: "sha256:settings".to_string(),
            target_id: "production".to_string(),
            preflight_token: "sha256:preflight".to_string(),
            artifact_id: artifact.to_string(),
            artifact_files: 1,
            artifact_bytes: 10,
            completed_at_ms: 1,
            log: "ok".to_string(),
        };
        let first = build_publish_build_receipt(make("session-a", "sha256:a"));
        let other_session = build_publish_build_receipt(make("session-b", "sha256:a"));
        let other_artifact = build_publish_build_receipt(make("session-a", "sha256:b"));
        assert_ne!(first.build_token, other_session.build_token);
        assert_ne!(first.build_token, other_artifact.build_token);
    }
}
