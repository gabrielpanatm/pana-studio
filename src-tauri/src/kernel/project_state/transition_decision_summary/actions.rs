use super::super::{
    lifecycle_policy::KernelProjectTransitionAction,
    transition_decision::KernelProjectTransitionDecisionRecord,
    transition_decision_labels::{
        action_code, action_decision_title, decision_kind_title, transition_reason_code,
    },
};
use super::{
    latest::latest_record, KernelProjectTransitionDecisionActionSummary,
    KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
};

pub(super) fn summarize_latest_decisions_by_action(
    records: &[KernelProjectTransitionDecisionRecord],
) -> Vec<KernelProjectTransitionDecisionActionSummary> {
    let mut summaries = Vec::new();
    for action in [
        KernelProjectTransitionAction::OpenProject,
        KernelProjectTransitionAction::ReloadProject,
        KernelProjectTransitionAction::CloseProject,
    ] {
        let matching = records
            .iter()
            .filter(|record| record.evidence.action == action)
            .collect::<Vec<_>>();
        let Some(latest) = latest_record(&matching) else {
            continue;
        };
        let count = matching.len();
        summaries.push(KernelProjectTransitionDecisionActionSummary {
            schema_version: KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
            action,
            count,
            latest_record_id: latest.id.clone(),
            latest_decided_at_ms: latest.decided_at_ms,
            latest_decision_kind: latest.decision_kind,
            latest_transition_reason: latest.evidence.transition_reason,
            latest_project_state_status: latest.evidence.project_state_status,
            latest_project_state_reason: latest.evidence.project_state_reason,
            latest_target_project_root: latest.evidence.target_project_root.clone(),
            latest_session_id: latest.evidence.session_id.clone(),
            title: action_decision_title(action).to_string(),
            detail: format!(
                "{} decizii operator pentru {}; ultima este {} pe {}.",
                count,
                action_code(action),
                decision_kind_title(latest.decision_kind),
                transition_reason_code(latest.evidence.transition_reason)
            ),
            recommended_action:
                "Folosește această sinteză doar pentru orientare; consumul deciziei rămâne condiționat de evidence hash curent."
                    .to_string(),
        });
    }
    summaries
}
