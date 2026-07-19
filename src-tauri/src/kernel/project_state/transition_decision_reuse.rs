use serde::Serialize;

use super::{
    lifecycle_policy::{KernelProjectTransitionAction, KernelProjectTransitionReason},
    transition_decision::{
        KernelProjectTransitionDecisionKind, KernelProjectTransitionDecisionRecord,
        KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
    },
};

mod contexts;
mod copy;

use contexts::summarize_reuse_contexts;
use copy::reuse_status_copy;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelProjectTransitionDecisionReuseStatus {
    NoDecisions,
    ExactEvidenceOnly,
    RepeatedContext,
    BlockedByIntegrity,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionDecisionReuseGuidanceSnapshot {
    pub schema_version: u32,
    pub status: KernelProjectTransitionDecisionReuseStatus,
    pub exact_evidence_only: bool,
    pub blocked_by_integrity: bool,
    pub record_count: usize,
    pub context_count: usize,
    pub repeated_context_count: usize,
    pub superseded_record_count: usize,
    pub latest_context_record_id: Option<String>,
    pub latest_decided_at_ms: Option<u128>,
    pub summary: String,
    pub detail: String,
    pub recommended_action: String,
    pub contexts: Vec<KernelProjectTransitionDecisionReuseContextSummary>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionDecisionReuseContextSummary {
    pub schema_version: u32,
    pub action: KernelProjectTransitionAction,
    pub decision_kind: KernelProjectTransitionDecisionKind,
    pub target_project_root: String,
    pub count: usize,
    pub latest_record_id: String,
    pub latest_decided_at_ms: u128,
    pub latest_transition_reason: KernelProjectTransitionReason,
    pub superseded_record_ids: Vec<String>,
    pub title: String,
    pub detail: String,
    pub recommended_action: String,
}

pub(super) fn summarize_decision_reuse_guidance(
    records: &[KernelProjectTransitionDecisionRecord],
    blocked_by_integrity: bool,
) -> KernelProjectTransitionDecisionReuseGuidanceSnapshot {
    let contexts = summarize_reuse_contexts(records);
    let repeated_context_count = contexts.iter().filter(|context| context.count > 1).count();
    let superseded_record_count = contexts
        .iter()
        .map(|context| context.superseded_record_ids.len())
        .sum();
    let latest_context = contexts.iter().max_by(|left, right| {
        left.latest_decided_at_ms
            .cmp(&right.latest_decided_at_ms)
            .then_with(|| left.latest_record_id.cmp(&right.latest_record_id))
    });
    let status = if blocked_by_integrity {
        KernelProjectTransitionDecisionReuseStatus::BlockedByIntegrity
    } else if records.is_empty() {
        KernelProjectTransitionDecisionReuseStatus::NoDecisions
    } else if superseded_record_count > 0 {
        KernelProjectTransitionDecisionReuseStatus::RepeatedContext
    } else {
        KernelProjectTransitionDecisionReuseStatus::ExactEvidenceOnly
    };
    let (summary, detail, recommended_action) = reuse_status_copy(
        status,
        records.len(),
        contexts.len(),
        repeated_context_count,
        superseded_record_count,
    );

    KernelProjectTransitionDecisionReuseGuidanceSnapshot {
        schema_version: KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
        status,
        exact_evidence_only: !blocked_by_integrity,
        blocked_by_integrity,
        record_count: records.len(),
        context_count: contexts.len(),
        repeated_context_count,
        superseded_record_count,
        latest_context_record_id: latest_context.map(|context| context.latest_record_id.clone()),
        latest_decided_at_ms: latest_context.map(|context| context.latest_decided_at_ms),
        summary,
        detail,
        recommended_action,
        contexts,
    }
}
