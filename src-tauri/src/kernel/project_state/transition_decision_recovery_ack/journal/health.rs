use super::super::{
    KernelProjectTransitionDecisionRecoveryAckJournalHealthSnapshot,
    KernelProjectTransitionDecisionRecoveryAckJournalHealthStatus,
    KernelProjectTransitionDecisionRecoveryAckRecord,
    KERNEL_PROJECT_TRANSITION_DECISION_RECOVERY_ACK_SCHEMA_VERSION,
};
use super::integrity::{
    duplicate_recovery_ack_id_count, recovery_ack_record_evidence_hash_matches,
};

pub(super) fn summarize_recovery_ack_journal_health(
    records: &[KernelProjectTransitionDecisionRecoveryAckRecord],
    record_count: usize,
    diagnostics: &[String],
    read_diagnostic_count: usize,
) -> KernelProjectTransitionDecisionRecoveryAckJournalHealthSnapshot {
    let invalid_evidence_hash_count = records
        .iter()
        .filter(|record| !recovery_ack_record_evidence_hash_matches(record))
        .count();
    let duplicate_id_count = duplicate_recovery_ack_id_count(records);
    let latest = records.iter().max_by(|left, right| {
        left.acknowledged_at_ms
            .cmp(&right.acknowledged_at_ms)
            .then_with(|| left.id.cmp(&right.id))
    });
    let status = if read_diagnostic_count > 0 {
        KernelProjectTransitionDecisionRecoveryAckJournalHealthStatus::Degraded
    } else if invalid_evidence_hash_count > 0 || duplicate_id_count > 0 {
        KernelProjectTransitionDecisionRecoveryAckJournalHealthStatus::IntegrityWarning
    } else if records.is_empty() {
        KernelProjectTransitionDecisionRecoveryAckJournalHealthStatus::Clean
    } else {
        KernelProjectTransitionDecisionRecoveryAckJournalHealthStatus::HasAcknowledgements
    };
    let (summary, detail, recommended_action) = recovery_ack_journal_health_copy(
        status,
        record_count,
        diagnostics.len(),
        invalid_evidence_hash_count,
        duplicate_id_count,
    );

    KernelProjectTransitionDecisionRecoveryAckJournalHealthSnapshot {
        schema_version: KERNEL_PROJECT_TRANSITION_DECISION_RECOVERY_ACK_SCHEMA_VERSION,
        status,
        record_count,
        returned_count: record_count,
        diagnostic_count: diagnostics.len(),
        invalid_evidence_hash_count,
        duplicate_id_count,
        latest_record_id: latest.map(|record| record.id.clone()),
        latest_acknowledged_at_ms: latest.map(|record| record.acknowledged_at_ms),
        latest_ack_kind: latest.map(|record| record.ack_kind),
        latest_recovery_plan_evidence_hash: latest
            .map(|record| record.evidence.recovery_plan_evidence_hash.clone()),
        summary,
        detail,
        recommended_action,
    }
}

fn recovery_ack_journal_health_copy(
    status: KernelProjectTransitionDecisionRecoveryAckJournalHealthStatus,
    record_count: usize,
    diagnostic_count: usize,
    invalid_evidence_hash_count: usize,
    duplicate_id_count: usize,
) -> (String, String, String) {
    match status {
        KernelProjectTransitionDecisionRecoveryAckJournalHealthStatus::Clean => (
            "Fără acknowledge recovery".to_string(),
            "Sesiunea curentă nu are acknowledge operator pentru recovery plan-ul Decision Journal."
                .to_string(),
            "Înregistrează acknowledge doar pentru planuri integrity_blocked sau retention_review."
                .to_string(),
        ),
        KernelProjectTransitionDecisionRecoveryAckJournalHealthStatus::HasAcknowledgements => (
            "Acknowledge recovery verificabil".to_string(),
            format!(
                "{} acknowledge-uri operator au evidence hash valid și ID-uri unice.",
                record_count
            ),
            "Păstrează jurnalul append-only; acknowledge-ul nu execută retention și nu repară jurnalul original."
                .to_string(),
        ),
        KernelProjectTransitionDecisionRecoveryAckJournalHealthStatus::IntegrityWarning => (
            "Acknowledge recovery cu risc de integritate".to_string(),
            format!(
                "{} hash-uri invalide și {} ID-uri duplicate au fost detectate.",
                invalid_evidence_hash_count, duplicate_id_count
            ),
            "Nu folosi acknowledge-urile ca audit de încredere până când jurnalul este inspectat."
                .to_string(),
        ),
        KernelProjectTransitionDecisionRecoveryAckJournalHealthStatus::Degraded => (
            "Acknowledge recovery degradat".to_string(),
            format!(
                "Jurnalul de acknowledge are {} diagnostics la citire sau integritate.",
                diagnostic_count
            ),
            "Inspectează liniile necitibile înainte de orice raport operațional dependent."
                .to_string(),
        ),
    }
}
