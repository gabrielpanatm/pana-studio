use std::collections::BTreeMap;

use super::super::transition_decision_labels::{
    action_code, action_title, decision_kind_code, decision_kind_title, transition_reason_code,
};
use super::{
    KernelProjectTransitionDecisionRecord, KernelProjectTransitionDecisionReuseContextSummary,
    KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
};

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct DecisionReuseContextKey {
    action: &'static str,
    decision_kind: &'static str,
    target_project_root: String,
}

pub(super) fn summarize_reuse_contexts(
    records: &[KernelProjectTransitionDecisionRecord],
) -> Vec<KernelProjectTransitionDecisionReuseContextSummary> {
    let mut grouped: BTreeMap<
        DecisionReuseContextKey,
        Vec<&KernelProjectTransitionDecisionRecord>,
    > = BTreeMap::new();
    for record in records {
        grouped
            .entry(DecisionReuseContextKey {
                action: action_code(record.evidence.action),
                decision_kind: decision_kind_code(record.decision_kind),
                target_project_root: record.evidence.target_project_root.clone(),
            })
            .or_default()
            .push(record);
    }

    let mut contexts = grouped
        .into_iter()
        .filter_map(|(_, records)| summarize_reuse_context(&records))
        .collect::<Vec<_>>();
    contexts.sort_by(|left, right| {
        right
            .latest_decided_at_ms
            .cmp(&left.latest_decided_at_ms)
            .then_with(|| right.latest_record_id.cmp(&left.latest_record_id))
    });
    contexts
}

fn summarize_reuse_context(
    records: &[&KernelProjectTransitionDecisionRecord],
) -> Option<KernelProjectTransitionDecisionReuseContextSummary> {
    let latest = records.iter().copied().max_by(|left, right| {
        left.decided_at_ms
            .cmp(&right.decided_at_ms)
            .then_with(|| left.id.cmp(&right.id))
    })?;
    let mut superseded_record_ids = records
        .iter()
        .filter(|record| record.id != latest.id)
        .map(|record| record.id.clone())
        .collect::<Vec<_>>();
    superseded_record_ids.sort();

    Some(KernelProjectTransitionDecisionReuseContextSummary {
        schema_version: KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
        action: latest.evidence.action,
        decision_kind: latest.decision_kind,
        target_project_root: latest.evidence.target_project_root.clone(),
        count: records.len(),
        latest_record_id: latest.id.clone(),
        latest_decided_at_ms: latest.decided_at_ms,
        latest_transition_reason: latest.evidence.transition_reason,
        superseded_record_ids,
        title: format!(
            "{} / {}",
            action_title(latest.evidence.action),
            decision_kind_title(latest.decision_kind)
        ),
        detail: format!(
            "{} decizii pentru același target; ultima este legată de {}.",
            records.len(),
            transition_reason_code(latest.evidence.transition_reason)
        ),
        recommended_action:
            "Nu reutiliza automat contextul; cere o decizie nouă dacă evidence hash-ul curent diferă."
                .to_string(),
    })
}
