use crate::kernel::project_state::lifecycle_policy::KernelProjectTransitionAction;

use super::super::{
    KernelProjectTransitionBlockedActionSummary, KernelProjectTransitionBlockedRecord,
    KERNEL_PROJECT_TRANSITION_BLOCKED_AUDIT_SCHEMA_VERSION,
};
use super::{
    classification::classify_blocked_cause,
    copy::{blocked_action_title, blocked_cause_profile},
};

pub(in crate::kernel::project_state::transition_blocked) fn summarize_latest_blocked_by_action(
    records: &[KernelProjectTransitionBlockedRecord],
) -> Vec<KernelProjectTransitionBlockedActionSummary> {
    let mut summaries = Vec::new();
    for action in [
        KernelProjectTransitionAction::OpenProject,
        KernelProjectTransitionAction::ReloadProject,
        KernelProjectTransitionAction::CloseProject,
    ] {
        let matching = records
            .iter()
            .filter(|record| record.action == Some(action))
            .collect::<Vec<_>>();
        let Some(latest) = matching.iter().max_by_key(|record| record.blocked_at_ms) else {
            continue;
        };
        let cause = classify_blocked_cause(latest);
        let profile = blocked_cause_profile(cause);
        summaries.push(KernelProjectTransitionBlockedActionSummary {
            schema_version: KERNEL_PROJECT_TRANSITION_BLOCKED_AUDIT_SCHEMA_VERSION,
            action,
            count: matching.len(),
            latest_record_id: latest.id.clone(),
            latest_blocked_at_ms: latest.blocked_at_ms,
            cause,
            surface: profile.surface,
            decision: latest.decision,
            reason: latest.reason,
            project_state_status: latest.project_state_status,
            project_state_reason: latest.project_state_reason,
            current_project_root: latest.current_project_root.clone(),
            target_project_root: latest.target_project_root.clone(),
            session_id: latest.session_id.clone(),
            title: blocked_action_title(action).to_string(),
            detail: profile.detail.to_string(),
            recommended_action: profile.recommended_action.to_string(),
        });
    }
    summaries
}
