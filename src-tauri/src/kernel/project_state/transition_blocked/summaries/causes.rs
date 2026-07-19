use super::super::{
    KernelProjectTransitionBlockedCause, KernelProjectTransitionBlockedCauseSummary,
    KernelProjectTransitionBlockedRecord, KERNEL_PROJECT_TRANSITION_BLOCKED_AUDIT_SCHEMA_VERSION,
};
use super::{classification::classify_blocked_cause, copy::blocked_cause_profile};

pub(in crate::kernel::project_state::transition_blocked) fn summarize_blocked_causes(
    records: &[KernelProjectTransitionBlockedRecord],
) -> Vec<KernelProjectTransitionBlockedCauseSummary> {
    let mut summaries = Vec::new();
    for cause in [
        KernelProjectTransitionBlockedCause::DiskConflict,
        KernelProjectTransitionBlockedCause::WorkspaceDirty,
        KernelProjectTransitionBlockedCause::BlockedProjectState,
        KernelProjectTransitionBlockedCause::Unknown,
    ] {
        let matching = records
            .iter()
            .filter(|record| classify_blocked_cause(record) == cause)
            .collect::<Vec<_>>();
        if matching.is_empty() {
            continue;
        }
        let latest_blocked_at_ms = matching
            .iter()
            .map(|record| record.blocked_at_ms)
            .max()
            .unwrap_or_default();
        let latest_record_id = matching
            .iter()
            .max_by_key(|record| record.blocked_at_ms)
            .map(|record| record.id.clone());
        let record_ids = matching
            .iter()
            .map(|record| record.id.clone())
            .collect::<Vec<_>>();
        let profile = blocked_cause_profile(cause);
        summaries.push(KernelProjectTransitionBlockedCauseSummary {
            schema_version: KERNEL_PROJECT_TRANSITION_BLOCKED_AUDIT_SCHEMA_VERSION,
            cause,
            surface: profile.surface,
            count: matching.len(),
            latest_blocked_at_ms,
            latest_record_id,
            record_ids,
            title: profile.title.to_string(),
            detail: profile.detail.to_string(),
            recommended_action: profile.recommended_action.to_string(),
        });
    }
    summaries
}
