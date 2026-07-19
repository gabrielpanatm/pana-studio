use super::super::{
    KernelProjectTransitionDecisionJournalHealthSnapshot,
    KernelProjectTransitionDecisionJournalHealthStatus, KernelProjectTransitionDecisionRecord,
    KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
};
use super::integrity::{decision_record_evidence_hash_matches, duplicate_decision_id_count};

pub(super) fn summarize_decision_journal_health(
    records: &[KernelProjectTransitionDecisionRecord],
    record_count: usize,
    diagnostics: &[String],
    read_diagnostic_count: usize,
) -> KernelProjectTransitionDecisionJournalHealthSnapshot {
    let invalid_evidence_hash_count = records
        .iter()
        .filter(|record| !decision_record_evidence_hash_matches(record))
        .count();
    let duplicate_id_count = duplicate_decision_id_count(records);
    let latest = records.iter().max_by(|left, right| {
        left.decided_at_ms
            .cmp(&right.decided_at_ms)
            .then_with(|| left.id.cmp(&right.id))
    });
    let status = if read_diagnostic_count > 0 {
        KernelProjectTransitionDecisionJournalHealthStatus::Degraded
    } else if invalid_evidence_hash_count > 0 || duplicate_id_count > 0 {
        KernelProjectTransitionDecisionJournalHealthStatus::IntegrityWarning
    } else if records.is_empty() {
        KernelProjectTransitionDecisionJournalHealthStatus::Clean
    } else {
        KernelProjectTransitionDecisionJournalHealthStatus::HasDecisions
    };
    let (summary, detail, recommended_action) = decision_journal_health_copy(
        status,
        record_count,
        diagnostics.len(),
        invalid_evidence_hash_count,
        duplicate_id_count,
    );

    KernelProjectTransitionDecisionJournalHealthSnapshot {
        schema_version: KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
        status,
        record_count,
        returned_count: record_count,
        diagnostic_count: diagnostics.len(),
        invalid_evidence_hash_count,
        duplicate_id_count,
        latest_record_id: latest.map(|record| record.id.clone()),
        latest_decided_at_ms: latest.map(|record| record.decided_at_ms),
        latest_decision_kind: latest.map(|record| record.decision_kind),
        summary,
        detail,
        recommended_action,
    }
}

fn decision_journal_health_copy(
    status: KernelProjectTransitionDecisionJournalHealthStatus,
    record_count: usize,
    diagnostic_count: usize,
    invalid_evidence_hash_count: usize,
    duplicate_id_count: usize,
) -> (String, String, String) {
    match status {
        KernelProjectTransitionDecisionJournalHealthStatus::Clean => (
            "Decision Journal curat".to_string(),
            "Sesiunea curentă nu are decizii operator ProjectTransition înregistrate.".to_string(),
            "Continuă folosirea dialogului operator doar când ProjectTransitionPolicy cere confirmare.".to_string(),
        ),
        KernelProjectTransitionDecisionJournalHealthStatus::HasDecisions => (
            "Decision Journal verificabil".to_string(),
            format!(
                "{} decizii operator au evidence hash valid și ID-uri unice în jurnal.",
                record_count
            ),
            "Deciziile rămân audit append-only; consumul lor este permis doar dacă evidența curentă se potrivește exact.".to_string(),
        ),
        KernelProjectTransitionDecisionJournalHealthStatus::IntegrityWarning => (
            "Decision Journal cu risc de integritate".to_string(),
            format!(
                "{} hash-uri de evidență invalide și {} ID-uri duplicate au fost detectate.",
                invalid_evidence_hash_count, duplicate_id_count
            ),
            "Nu consuma decizii din acest jurnal până când cauza este inspectată în Observability și sesiunea este clarificată.".to_string(),
        ),
        KernelProjectTransitionDecisionJournalHealthStatus::Degraded => (
            "Decision Journal degradat".to_string(),
            format!(
                "Jurnalul are {} diagnostics la citire sau integritate, deci nu este complet verificabil.",
                diagnostic_count
            ),
            "Inspectează diagnostics înainte de a continua tranzițiile care depind de decizii operator.".to_string(),
        ),
    }
}
