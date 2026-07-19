use serde::Serialize;

use super::{
    lifecycle_policy::{KernelProjectTransitionAction, KernelProjectTransitionReason},
    model::{KernelProjectStateReason, KernelProjectStateStatus},
    transition_decision::{
        KernelProjectTransitionDecisionKind, KernelProjectTransitionDecisionRecord,
        KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
    },
};

mod actions;
mod kinds;
mod latest;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionDecisionActionSummary {
    pub schema_version: u32,
    pub action: KernelProjectTransitionAction,
    pub count: usize,
    pub latest_record_id: String,
    pub latest_decided_at_ms: u128,
    pub latest_decision_kind: KernelProjectTransitionDecisionKind,
    pub latest_transition_reason: KernelProjectTransitionReason,
    pub latest_project_state_status: KernelProjectStateStatus,
    pub latest_project_state_reason: KernelProjectStateReason,
    pub latest_target_project_root: String,
    pub latest_session_id: String,
    pub title: String,
    pub detail: String,
    pub recommended_action: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionDecisionKindSummary {
    pub schema_version: u32,
    pub decision_kind: KernelProjectTransitionDecisionKind,
    pub count: usize,
    pub latest_record_id: String,
    pub latest_decided_at_ms: u128,
    pub latest_action: KernelProjectTransitionAction,
    pub latest_transition_reason: KernelProjectTransitionReason,
    pub latest_target_project_root: String,
    pub title: String,
    pub detail: String,
    pub recommended_action: String,
}

pub(super) fn summarize_latest_decisions_by_action(
    records: &[KernelProjectTransitionDecisionRecord],
) -> Vec<KernelProjectTransitionDecisionActionSummary> {
    actions::summarize_latest_decisions_by_action(records)
}

pub(super) fn summarize_decision_journal_by_kind(
    records: &[KernelProjectTransitionDecisionRecord],
) -> Vec<KernelProjectTransitionDecisionKindSummary> {
    kinds::summarize_decision_journal_by_kind(records)
}
