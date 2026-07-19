use std::collections::BTreeMap;

use super::super::{project_transition_evidence_hash, KernelProjectTransitionDecisionRecord};

pub(super) fn decision_journal_integrity_diagnostics(
    records: &[KernelProjectTransitionDecisionRecord],
) -> Vec<String> {
    let mut diagnostics = Vec::new();
    for record in records {
        if !decision_record_evidence_hash_matches(record) {
            diagnostics.push(format!(
                "Project Transition Decision {} are evidenceHash invalid față de evidența serializată.",
                record.id
            ));
        }
    }
    for (id, count) in decision_id_counts(records) {
        if count > 1 {
            diagnostics.push(format!(
                "Project Transition Decision Journal conține ID duplicat {} de {} ori.",
                id, count
            ));
        }
    }
    diagnostics
}

pub(super) fn decision_record_evidence_hash_matches(
    record: &KernelProjectTransitionDecisionRecord,
) -> bool {
    project_transition_evidence_hash(&record.evidence)
        .map(|hash| hash == record.evidence_hash)
        .unwrap_or(false)
}

pub(super) fn duplicate_decision_id_count(
    records: &[KernelProjectTransitionDecisionRecord],
) -> usize {
    decision_id_counts(records)
        .values()
        .filter(|count| **count > 1)
        .map(|count| *count - 1)
        .sum()
}

fn decision_id_counts(
    records: &[KernelProjectTransitionDecisionRecord],
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for record in records {
        *counts.entry(record.id.clone()).or_insert(0) += 1;
    }
    counts
}
