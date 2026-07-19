use std::collections::BTreeMap;

use super::super::{
    transition_decision::{
        KernelProjectTransitionDecisionRecord,
        KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
    },
    transition_decision_reuse::KernelProjectTransitionDecisionReuseGuidanceSnapshot,
};
use super::KernelProjectTransitionDecisionRetentionCandidate;

pub(super) fn build_retention_candidates(
    records: &[KernelProjectTransitionDecisionRecord],
    reuse_guidance: &KernelProjectTransitionDecisionReuseGuidanceSnapshot,
) -> Vec<KernelProjectTransitionDecisionRetentionCandidate> {
    let records_by_id = records
        .iter()
        .map(|record| (record.id.as_str(), record))
        .collect::<BTreeMap<_, _>>();
    let mut candidates = Vec::new();
    for context in &reuse_guidance.contexts {
        for record_id in &context.superseded_record_ids {
            let Some(record) = records_by_id.get(record_id.as_str()) else {
                continue;
            };
            candidates.push(KernelProjectTransitionDecisionRetentionCandidate {
                schema_version: KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
                record_id: record.id.clone(),
                superseded_by_record_id: context.latest_record_id.clone(),
                action: record.evidence.action,
                decision_kind: record.decision_kind,
                target_project_root: record.evidence.target_project_root.clone(),
                decided_at_ms: record.decided_at_ms,
                transition_reason: record.evidence.transition_reason,
                title: "Candidat retention read-only".to_string(),
                detail: format!(
                    "Decizia {} este superseded de {} pentru același context.",
                    record.id, context.latest_record_id
                ),
                recommended_action:
                    "Nu șterge automat; păstrează candidatul până există politică de retention cu receipt și audit."
                        .to_string(),
            });
        }
    }
    candidates.sort_by(|left, right| {
        right
            .decided_at_ms
            .cmp(&left.decided_at_ms)
            .then_with(|| right.record_id.cmp(&left.record_id))
    });
    candidates
}
