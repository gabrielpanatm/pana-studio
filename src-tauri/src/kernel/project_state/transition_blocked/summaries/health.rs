use super::super::{
    KernelProjectTransitionBlockedActionSummary, KernelProjectTransitionBlockedCauseSummary,
    KernelProjectTransitionBlockedHealthSnapshot, KernelProjectTransitionBlockedHealthStatus,
    KernelProjectTransitionBlockedRecord, KERNEL_PROJECT_TRANSITION_BLOCKED_AUDIT_SCHEMA_VERSION,
};
use super::copy::blocked_health_copy;

pub(in crate::kernel::project_state::transition_blocked) fn summarize_blocked_health(
    records: &[KernelProjectTransitionBlockedRecord],
    latest_by_action: &[KernelProjectTransitionBlockedActionSummary],
    causes: &[KernelProjectTransitionBlockedCauseSummary],
    diagnostics: &[String],
) -> KernelProjectTransitionBlockedHealthSnapshot {
    let repeated_action_count = latest_by_action
        .iter()
        .filter(|summary| summary.count > 1)
        .count();
    let repeated_cause_count = causes.iter().filter(|summary| summary.count > 1).count();
    let latest = records.iter().max_by_key(|record| record.blocked_at_ms);
    let status = if !diagnostics.is_empty() {
        KernelProjectTransitionBlockedHealthStatus::Degraded
    } else if records.is_empty() {
        KernelProjectTransitionBlockedHealthStatus::Clean
    } else if repeated_action_count > 0 || repeated_cause_count > 0 {
        KernelProjectTransitionBlockedHealthStatus::RepeatedlyBlocked
    } else {
        KernelProjectTransitionBlockedHealthStatus::RecentlyBlocked
    };
    let (summary, detail, recommended_action) = blocked_health_copy(
        status,
        records.len(),
        repeated_action_count,
        repeated_cause_count,
        diagnostics.len(),
    );

    KernelProjectTransitionBlockedHealthSnapshot {
        schema_version: KERNEL_PROJECT_TRANSITION_BLOCKED_AUDIT_SCHEMA_VERSION,
        status,
        record_count: records.len(),
        action_count: latest_by_action.len(),
        repeated_action_count,
        cause_count: causes.len(),
        repeated_cause_count,
        latest_record_id: latest.map(|record| record.id.clone()),
        latest_action: latest.and_then(|record| record.action),
        latest_blocked_at_ms: latest.map(|record| record.blocked_at_ms),
        summary,
        detail,
        recommended_action,
    }
}
