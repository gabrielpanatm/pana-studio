use super::super::{
    transition_decision::{
        KernelProjectTransitionDecisionKind, KernelProjectTransitionDecisionRecord,
    },
    transition_decision_labels::{action_code, decision_kind_code, decision_kind_title},
};
use super::{
    latest::latest_record, KernelProjectTransitionDecisionKindSummary,
    KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
};

pub(super) fn summarize_decision_journal_by_kind(
    records: &[KernelProjectTransitionDecisionRecord],
) -> Vec<KernelProjectTransitionDecisionKindSummary> {
    let mut summaries = Vec::new();
    for decision_kind in [
        KernelProjectTransitionDecisionKind::DiscardLocalDraftsForTransition,
        KernelProjectTransitionDecisionKind::AcknowledgeDirtyHistoryForTransition,
        KernelProjectTransitionDecisionKind::DiscardSessionForExternalReload,
    ] {
        let matching = records
            .iter()
            .filter(|record| record.decision_kind == decision_kind)
            .collect::<Vec<_>>();
        let Some(latest) = latest_record(&matching) else {
            continue;
        };
        let count = matching.len();
        summaries.push(KernelProjectTransitionDecisionKindSummary {
            schema_version: KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
            decision_kind,
            count,
            latest_record_id: latest.id.clone(),
            latest_decided_at_ms: latest.decided_at_ms,
            latest_action: latest.evidence.action,
            latest_transition_reason: latest.evidence.transition_reason,
            latest_target_project_root: latest.evidence.target_project_root.clone(),
            title: decision_kind_title(decision_kind).to_string(),
            detail: format!(
                "{} decizii de tip {}; ultima a fost pentru {}.",
                count,
                decision_kind_code(decision_kind),
                action_code(latest.evidence.action)
            ),
            recommended_action:
                "Verifică repetiția acestui tip de decizie înainte să tratezi confirmările ca rutină operațională."
                    .to_string(),
        });
    }
    summaries
}
