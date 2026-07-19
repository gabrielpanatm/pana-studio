use std::collections::BTreeMap;

use serde::Serialize;

use crate::kernel::{
    project_state::{
        KernelProjectTransitionDecisionRetentionHotJournal,
        KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
    },
    project_workspace::{ProjectWorkspaceSaveHotJournal, ProjectWorkspaceSaveRecoveryAction},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryJournalFamily {
    ProjectWorkspaceSave,
    ProjectTransitionDecisionRetention,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryJournalFamilyStatus {
    NeedsAttention,
    ManualReviewRequired,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryJournalValueCount {
    pub value: String,
    pub count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryJournalFamilySummary {
    pub family: RecoveryJournalFamily,
    pub status: RecoveryJournalFamilyStatus,
    pub label: String,
    pub count: usize,
    pub clearable_count: usize,
    pub rollback_count: usize,
    pub restore_count: usize,
    pub manual_review_count: usize,
    pub newest_created_at_ms: Option<u128>,
    pub state_counts: Vec<RecoveryJournalValueCount>,
    pub action_counts: Vec<RecoveryJournalValueCount>,
}

pub(super) fn summarize_recovery_journals(
    workspace_saves: &[ProjectWorkspaceSaveHotJournal],
    project_transitions: &[KernelProjectTransitionDecisionRetentionHotJournal],
) -> Vec<RecoveryJournalFamilySummary> {
    let mut result = Vec::new();
    if !workspace_saves.is_empty() {
        let manual = workspace_saves
            .iter()
            .filter(|journal| {
                matches!(
                    journal.recovery_plan.action,
                    ProjectWorkspaceSaveRecoveryAction::ManualReviewMixedState
                        | ProjectWorkspaceSaveRecoveryAction::ManualReviewConflict
                )
            })
            .count();
        result.push(RecoveryJournalFamilySummary {
            family: RecoveryJournalFamily::ProjectWorkspaceSave,
            status: status(manual),
            label: "ProjectWorkspace Save".to_string(),
            count: workspace_saves.len(),
            clearable_count: workspace_saves
                .iter()
                .filter(|journal| journal.recovery_plan.can_clear_journal)
                .count(),
            rollback_count: workspace_saves
                .iter()
                .filter(|journal| journal.recovery_plan.can_rollback)
                .count(),
            restore_count: 0,
            manual_review_count: manual,
            newest_created_at_ms: workspace_saves
                .iter()
                .map(|journal| journal.prepared_at_ms)
                .max(),
            state_counts: counts(
                workspace_saves
                    .iter()
                    .map(|journal| format!("{:?}", journal.disk_state).to_lowercase()),
            ),
            action_counts: counts(
                workspace_saves
                    .iter()
                    .map(|journal| format!("{:?}", journal.recovery_plan.action).to_lowercase()),
            ),
        });
    }
    if !project_transitions.is_empty() {
        let manual = project_transitions.iter().filter(|journal|
            journal.recovery_plan.action == KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction::ManualReviewConflict).count();
        result.push(RecoveryJournalFamilySummary {
            family: RecoveryJournalFamily::ProjectTransitionDecisionRetention,
            status: status(manual),
            label: "ProjectTransition Decision Retention".to_string(),
            count: project_transitions.len(),
            clearable_count: project_transitions
                .iter()
                .filter(|journal| journal.recovery_plan.can_clear_journal)
                .count(),
            rollback_count: 0,
            restore_count: project_transitions
                .iter()
                .filter(|journal| journal.recovery_plan.can_restore_before_journal)
                .count(),
            manual_review_count: manual,
            newest_created_at_ms: project_transitions
                .iter()
                .map(|journal| journal.created_at_ms)
                .max(),
            state_counts: counts(
                project_transitions
                    .iter()
                    .map(|journal| format!("{:?}", journal.disk_state).to_lowercase()),
            ),
            action_counts: counts(
                project_transitions
                    .iter()
                    .map(|journal| format!("{:?}", journal.recovery_plan.action).to_lowercase()),
            ),
        });
    }
    result
}

fn status(manual_review_count: usize) -> RecoveryJournalFamilyStatus {
    if manual_review_count == 0 {
        RecoveryJournalFamilyStatus::NeedsAttention
    } else {
        RecoveryJournalFamilyStatus::ManualReviewRequired
    }
}

fn counts(values: impl Iterator<Item = String>) -> Vec<RecoveryJournalValueCount> {
    let mut counts = BTreeMap::<String, usize>::new();
    for value in values {
        *counts.entry(value).or_default() += 1;
    }
    counts
        .into_iter()
        .map(|(value, count)| RecoveryJournalValueCount { value, count })
        .collect()
}
