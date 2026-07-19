use std::{fs, path::PathBuf};

use crate::kernel::{
    project_session::{ProjectRootFingerprint, ProjectSessionScanSummary, ProjectSessionSnapshot},
    project_state::transition_decision_recovery::{
        KernelProjectTransitionDecisionRecoveryPlanSnapshot,
        KernelProjectTransitionDecisionRecoveryPlanStatus,
    },
};

use super::super::{build_recovery_ack_record, KernelProjectTransitionDecisionRecoveryAckInput};
use super::read_project_transition_decision_recovery_ack_journal_from_path;

#[test]
fn recovery_ack_reader_keeps_newest_first_and_reports_integrity_diagnostics() {
    let root = temp_dir("project-transition-decision-recovery-ack");
    fs::create_dir_all(&root).unwrap();
    let path = root.join("project-transition-decision-recovery-acknowledgements.jsonl");
    let old = build_recovery_ack_record(
        &session(),
        "/tmp/session/project-transition-decisions.jsonl",
        &plan(
            KernelProjectTransitionDecisionRecoveryPlanStatus::IntegrityBlocked,
            "plan-a",
        ),
        KernelProjectTransitionDecisionRecoveryAckInput {
            recovery_plan_evidence_hash: "plan-a".to_string(),
            diagnostic: "Operatorul a inspectat jurnalul compromis vechi.".to_string(),
        },
        1,
        "old".to_string(),
    )
    .unwrap();
    let new = build_recovery_ack_record(
        &session(),
        "/tmp/session/project-transition-decisions.jsonl",
        &plan(
            KernelProjectTransitionDecisionRecoveryPlanStatus::RetentionReview,
            "plan-b",
        ),
        KernelProjectTransitionDecisionRecoveryAckInput {
            recovery_plan_evidence_hash: "plan-b".to_string(),
            diagnostic: "Operatorul a inspectat candidații noi de retention.".to_string(),
        },
        2,
        "new".to_string(),
    )
    .unwrap();
    let old_invalid = serde_json::to_string(&old)
        .unwrap()
        .replace(&old.evidence_hash, "invalid-hash");
    fs::write(
        &path,
        format!(
            "{}\nnot-json\n{}\n",
            old_invalid,
            serde_json::to_string(&new).unwrap()
        ),
    )
    .unwrap();

    let snapshot =
        read_project_transition_decision_recovery_ack_journal_from_path(&path, 1).unwrap();

    assert_eq!(snapshot.record_count, 2);
    assert_eq!(snapshot.returned_count, 1);
    assert_eq!(snapshot.records[0].id, "new");
    assert_eq!(snapshot.health.invalid_evidence_hash_count, 1);
    assert_eq!(snapshot.diagnostics.len(), 2);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn recovery_ack_reader_rejects_oversized_line_fail_closed() {
    let root = temp_dir("project-transition-decision-recovery-ack-oversized-line");
    fs::create_dir_all(&root).unwrap();
    let path = root.join("project-transition-decision-recovery-acknowledgements.jsonl");
    fs::write(&path, "x".repeat(256 * 1024 + 1)).unwrap();

    let error =
        read_project_transition_decision_recovery_ack_journal_from_path(&path, 80).unwrap_err();

    assert!(error.contains("linia 1"));
    assert!(error.contains("262144 bytes"));
    let _ = fs::remove_dir_all(root);
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

fn temp_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("pana-studio-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    path
}
