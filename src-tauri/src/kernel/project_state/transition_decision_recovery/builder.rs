use super::super::{
    transition_decision::{
        KernelProjectTransitionDecisionRecord,
        KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
    },
    transition_decision_reuse::KernelProjectTransitionDecisionReuseGuidanceSnapshot,
};
use super::{
    copy::recovery_status_copy,
    evidence::recovery_plan_evidence_hash,
    integrity::{decision_id_counts, decision_record_evidence_hash_matches},
    issues::build_integrity_issues,
    model::{
        KernelProjectTransitionDecisionRecoveryIssue,
        KernelProjectTransitionDecisionRecoveryIssueKind,
        KernelProjectTransitionDecisionRecoveryIssueSeverity,
        KernelProjectTransitionDecisionRecoveryPlanSnapshot,
        KernelProjectTransitionDecisionRecoveryPlanStatus,
    },
    retention::build_retention_candidates,
};

pub(in crate::kernel::project_state) fn build_decision_recovery_plan(
    records: &[KernelProjectTransitionDecisionRecord],
    read_diagnostic_count: usize,
    reuse_guidance: &KernelProjectTransitionDecisionReuseGuidanceSnapshot,
) -> KernelProjectTransitionDecisionRecoveryPlanSnapshot {
    let invalid_records = records
        .iter()
        .filter(|record| !decision_record_evidence_hash_matches(record))
        .collect::<Vec<_>>();
    let duplicate_ids = decision_id_counts(records)
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .collect::<Vec<_>>();
    let duplicate_id_count = duplicate_ids
        .iter()
        .map(|(_, count)| *count - 1)
        .sum::<usize>();
    let integrity_blocked =
        read_diagnostic_count > 0 || !invalid_records.is_empty() || duplicate_id_count > 0;
    let retention_candidates = if integrity_blocked {
        Vec::new()
    } else {
        build_retention_candidates(records, reuse_guidance)
    };
    let superseded_record_count = reuse_guidance.superseded_record_count;
    let status = if integrity_blocked {
        KernelProjectTransitionDecisionRecoveryPlanStatus::IntegrityBlocked
    } else if !retention_candidates.is_empty() {
        KernelProjectTransitionDecisionRecoveryPlanStatus::RetentionReview
    } else if records.is_empty() {
        KernelProjectTransitionDecisionRecoveryPlanStatus::CleanNoop
    } else {
        KernelProjectTransitionDecisionRecoveryPlanStatus::VerifiedAudit
    };
    let mut issues = build_integrity_issues(
        read_diagnostic_count,
        &invalid_records,
        &duplicate_ids,
        duplicate_id_count,
    );
    if !integrity_blocked && superseded_record_count > 0 {
        issues.push(KernelProjectTransitionDecisionRecoveryIssue {
            schema_version: KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
            kind: KernelProjectTransitionDecisionRecoveryIssueKind::SupersededRecord,
            severity: KernelProjectTransitionDecisionRecoveryIssueSeverity::Info,
            record_id: None,
            count: superseded_record_count,
            title: "Recorduri istorice superseded".to_string(),
            detail: format!(
                "{} decizii sunt istorice în contexte repetate și pot deveni candidate de retention după politică explicită.",
                superseded_record_count
            ),
            recommended_action:
                "Păstrează-le ca audit până există un executor de retention verificat și aprobat."
                    .to_string(),
        });
    }
    let issue_count = issues.len();
    let (summary, detail, recommended_action) = recovery_status_copy(
        status,
        records.len(),
        read_diagnostic_count,
        invalid_records.len(),
        duplicate_id_count,
        retention_candidates.len(),
    );
    let evidence_hash = recovery_plan_evidence_hash(
        records,
        status,
        !integrity_blocked,
        read_diagnostic_count,
        &invalid_records,
        &duplicate_ids,
        duplicate_id_count,
        superseded_record_count,
        &retention_candidates,
    );

    KernelProjectTransitionDecisionRecoveryPlanSnapshot {
        schema_version: KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
        evidence_hash,
        status,
        read_only: true,
        mutation_allowed: false,
        integrity_trusted: !integrity_blocked,
        record_count: records.len(),
        read_diagnostic_count,
        invalid_evidence_hash_count: invalid_records.len(),
        duplicate_id_count,
        superseded_record_count,
        retention_candidate_count: retention_candidates.len(),
        issue_count,
        summary,
        detail,
        recommended_action,
        issues,
        retention_candidates,
    }
}
