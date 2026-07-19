use serde::Serialize;

use crate::kernel::file_buffer_store::hash_text;

use super::super::transition_decision::{
    KernelProjectTransitionDecisionRecord,
    KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
};
use super::{
    KernelProjectTransitionDecisionRecoveryPlanStatus,
    KernelProjectTransitionDecisionRetentionCandidate,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DecisionRecoveryPlanEvidence {
    schema_version: u32,
    status: KernelProjectTransitionDecisionRecoveryPlanStatus,
    integrity_trusted: bool,
    read_diagnostic_count: usize,
    invalid_evidence_hash_count: usize,
    duplicate_id_count: usize,
    superseded_record_count: usize,
    retention_candidate_count: usize,
    records: Vec<DecisionRecoveryPlanRecordEvidence>,
    invalid_evidence_record_ids: Vec<String>,
    duplicate_decision_ids: Vec<DecisionRecoveryPlanDuplicateEvidence>,
    retention_candidates: Vec<DecisionRecoveryPlanRetentionEvidence>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DecisionRecoveryPlanRecordEvidence {
    record_id: String,
    decided_at_ms: u128,
    evidence_hash: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DecisionRecoveryPlanDuplicateEvidence {
    decision_id: String,
    count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DecisionRecoveryPlanRetentionEvidence {
    record_id: String,
    superseded_by_record_id: String,
}

pub(super) fn recovery_plan_evidence_hash(
    records: &[KernelProjectTransitionDecisionRecord],
    status: KernelProjectTransitionDecisionRecoveryPlanStatus,
    integrity_trusted: bool,
    read_diagnostic_count: usize,
    invalid_records: &[&KernelProjectTransitionDecisionRecord],
    duplicate_ids: &[(String, usize)],
    duplicate_id_count: usize,
    superseded_record_count: usize,
    retention_candidates: &[KernelProjectTransitionDecisionRetentionCandidate],
) -> String {
    let mut record_evidence = records
        .iter()
        .map(|record| DecisionRecoveryPlanRecordEvidence {
            record_id: record.id.clone(),
            decided_at_ms: record.decided_at_ms,
            evidence_hash: record.evidence_hash.clone(),
        })
        .collect::<Vec<_>>();
    record_evidence.sort_by(|left, right| {
        left.record_id
            .cmp(&right.record_id)
            .then_with(|| left.decided_at_ms.cmp(&right.decided_at_ms))
            .then_with(|| left.evidence_hash.cmp(&right.evidence_hash))
    });
    let mut invalid_evidence_record_ids = invalid_records
        .iter()
        .map(|record| record.id.clone())
        .collect::<Vec<_>>();
    invalid_evidence_record_ids.sort();
    let mut duplicate_decision_ids = duplicate_ids
        .iter()
        .map(|(id, count)| DecisionRecoveryPlanDuplicateEvidence {
            decision_id: id.clone(),
            count: *count,
        })
        .collect::<Vec<_>>();
    duplicate_decision_ids.sort_by(|left, right| left.decision_id.cmp(&right.decision_id));
    let mut retention_evidence = retention_candidates
        .iter()
        .map(|candidate| DecisionRecoveryPlanRetentionEvidence {
            record_id: candidate.record_id.clone(),
            superseded_by_record_id: candidate.superseded_by_record_id.clone(),
        })
        .collect::<Vec<_>>();
    retention_evidence.sort_by(|left, right| {
        left.record_id.cmp(&right.record_id).then_with(|| {
            left.superseded_by_record_id
                .cmp(&right.superseded_by_record_id)
        })
    });
    let evidence = DecisionRecoveryPlanEvidence {
        schema_version: KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
        status,
        integrity_trusted,
        read_diagnostic_count,
        invalid_evidence_hash_count: invalid_records.len(),
        duplicate_id_count,
        superseded_record_count,
        retention_candidate_count: retention_candidates.len(),
        records: record_evidence,
        invalid_evidence_record_ids,
        duplicate_decision_ids,
        retention_candidates: retention_evidence,
    };
    let serialized = serde_json::to_string(&evidence)
        .unwrap_or_else(|error| format!("decision-recovery-plan-evidence-error:{error}"));
    hash_text(&serialized)
}
