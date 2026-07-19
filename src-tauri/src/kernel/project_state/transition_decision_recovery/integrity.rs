use std::collections::BTreeMap;

use crate::kernel::file_buffer_store::hash_text;

use super::super::transition_decision::KernelProjectTransitionDecisionRecord;

pub(super) fn decision_record_evidence_hash_matches(
    record: &KernelProjectTransitionDecisionRecord,
) -> bool {
    serde_json::to_string(&record.evidence)
        .map(|serialized| hash_text(&serialized) == record.evidence_hash)
        .unwrap_or(false)
}

pub(super) fn decision_id_counts(
    records: &[KernelProjectTransitionDecisionRecord],
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for record in records {
        *counts.entry(record.id.clone()).or_insert(0) += 1;
    }
    counts
}
