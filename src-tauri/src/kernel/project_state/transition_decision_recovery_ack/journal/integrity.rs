use std::collections::BTreeMap;

use super::super::{recovery_ack_evidence_hash, KernelProjectTransitionDecisionRecoveryAckRecord};

pub(super) fn recovery_ack_journal_integrity_diagnostics(
    records: &[KernelProjectTransitionDecisionRecoveryAckRecord],
) -> Vec<String> {
    let mut diagnostics = Vec::new();
    for record in records {
        if !recovery_ack_record_evidence_hash_matches(record) {
            diagnostics.push(format!(
                "Project Transition Decision recovery acknowledgement {} are evidenceHash invalid față de evidența serializată.",
                record.id
            ));
        }
    }
    for (id, count) in recovery_ack_id_counts(records) {
        if count > 1 {
            diagnostics.push(format!(
                "Project Transition Decision recovery acknowledgement journal conține ID duplicat {} de {} ori.",
                id, count
            ));
        }
    }
    diagnostics
}

pub(super) fn recovery_ack_record_evidence_hash_matches(
    record: &KernelProjectTransitionDecisionRecoveryAckRecord,
) -> bool {
    recovery_ack_evidence_hash(&record.evidence)
        .map(|hash| hash == record.evidence_hash)
        .unwrap_or(false)
}

pub(super) fn duplicate_recovery_ack_id_count(
    records: &[KernelProjectTransitionDecisionRecoveryAckRecord],
) -> usize {
    recovery_ack_id_counts(records)
        .values()
        .filter(|count| **count > 1)
        .map(|count| *count - 1)
        .sum()
}

fn recovery_ack_id_counts(
    records: &[KernelProjectTransitionDecisionRecoveryAckRecord],
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for record in records {
        *counts.entry(record.id.clone()).or_insert(0) += 1;
    }
    counts
}
