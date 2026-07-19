use crate::kernel::{
    project_session::{ProjectRootFingerprint, ProjectSessionScanSummary, ProjectSessionSnapshot},
    project_state::transition_decision_recovery::{
        KernelProjectTransitionDecisionRecoveryPlanSnapshot,
        KernelProjectTransitionDecisionRecoveryPlanStatus,
    },
};

use super::{
    build_recovery_ack_record, KernelProjectTransitionDecisionRecoveryAckInput,
    KernelProjectTransitionDecisionRecoveryAckKind,
};

#[test]
fn recovery_ack_record_rejects_stale_plan_hash() {
    let error = build_recovery_ack_record(
        &session(),
        "/tmp/session/project-transition-decisions.jsonl",
        &plan(
            KernelProjectTransitionDecisionRecoveryPlanStatus::IntegrityBlocked,
            "plan-hash",
        ),
        KernelProjectTransitionDecisionRecoveryAckInput {
            recovery_plan_evidence_hash: "old-plan-hash".to_string(),
            diagnostic: "Operatorul a inspectat jurnalul compromis.".to_string(),
        },
        1,
        "ack-1".to_string(),
    )
    .unwrap_err();

    assert!(error.contains("stale"));
}

#[test]
fn recovery_ack_record_requires_actionable_plan_status() {
    let error = build_recovery_ack_record(
        &session(),
        "/tmp/session/project-transition-decisions.jsonl",
        &plan(
            KernelProjectTransitionDecisionRecoveryPlanStatus::VerifiedAudit,
            "plan-hash",
        ),
        KernelProjectTransitionDecisionRecoveryAckInput {
            recovery_plan_evidence_hash: "plan-hash".to_string(),
            diagnostic: "Operatorul confirmă auditul verificat.".to_string(),
        },
        1,
        "ack-1".to_string(),
    )
    .unwrap_err();

    assert!(error.contains("nu cere acknowledge"));
}

#[test]
fn recovery_ack_record_persists_plan_metadata_without_decision_payloads() {
    let record = build_recovery_ack_record(
        &session(),
        "/tmp/session/project-transition-decisions.jsonl",
        &plan(
            KernelProjectTransitionDecisionRecoveryPlanStatus::RetentionReview,
            "plan-hash",
        ),
        KernelProjectTransitionDecisionRecoveryAckInput {
            recovery_plan_evidence_hash: "plan-hash".to_string(),
            diagnostic: "Operatorul a inspectat candidații de retention.".to_string(),
        },
        1,
        "ack-1".to_string(),
    )
    .unwrap();
    let serialized = serde_json::to_string(&record).unwrap();

    assert_eq!(
        record.ack_kind,
        KernelProjectTransitionDecisionRecoveryAckKind::AcknowledgeRetentionReview
    );
    assert_eq!(record.evidence.recovery_plan_evidence_hash, "plan-hash");
    assert!(serialized.contains("retentionCandidateCount"));
    assert!(!serialized.contains("dirtyFiles"));
}

fn plan(
    status: KernelProjectTransitionDecisionRecoveryPlanStatus,
    evidence_hash: &str,
) -> KernelProjectTransitionDecisionRecoveryPlanSnapshot {
    KernelProjectTransitionDecisionRecoveryPlanSnapshot {
        schema_version: 6,
        evidence_hash: evidence_hash.to_string(),
        status,
        read_only: true,
        mutation_allowed: false,
        integrity_trusted: status
            != KernelProjectTransitionDecisionRecoveryPlanStatus::IntegrityBlocked,
        record_count: 3,
        read_diagnostic_count: if status
            == KernelProjectTransitionDecisionRecoveryPlanStatus::IntegrityBlocked
        {
            1
        } else {
            0
        },
        invalid_evidence_hash_count: 0,
        duplicate_id_count: 0,
        superseded_record_count: if status
            == KernelProjectTransitionDecisionRecoveryPlanStatus::RetentionReview
        {
            1
        } else {
            0
        },
        retention_candidate_count: if status
            == KernelProjectTransitionDecisionRecoveryPlanStatus::RetentionReview
        {
            1
        } else {
            0
        },
        issue_count: if status == KernelProjectTransitionDecisionRecoveryPlanStatus::CleanNoop {
            0
        } else {
            1
        },
        summary: "summary".to_string(),
        detail: "detail".to_string(),
        recommended_action: "recommended".to_string(),
        issues: Vec::new(),
        retention_candidates: Vec::new(),
    }
}

fn session() -> ProjectSessionSnapshot {
    ProjectSessionSnapshot {
        schema_version: 1,
        id: "session-1".to_string(),
        project_root: "/tmp/project".to_string(),
        zola_root: "/tmp/project".to_string(),
        session_dir: "/tmp/session".to_string(),
        manifest_path: "/tmp/session/manifest.json".to_string(),
        opened_at_ms: 1,
        last_seen_at_ms: 1,
        root_fingerprint: ProjectRootFingerprint {
            canonical_path: "/tmp/project".to_string(),
            modified_ms: 1,
            size: 0,
            readonly: false,
            unix_device: None,
            unix_inode: None,
        },
        scan_summary: ProjectSessionScanSummary {
            is_zola: true,
            is_empty: false,
            active_theme: None,
            file_count: 1,
            directory_count: 1,
        },
    }
}
