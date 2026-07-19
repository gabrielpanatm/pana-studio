use serde::{Deserialize, Serialize};

use crate::kernel::write_authority::WriteReceipt;

pub const KERNEL_PROJECT_TRANSITION_DECISION_RETENTION_SCHEMA_VERSION: u32 = 1;
pub const KERNEL_PROJECT_TRANSITION_DECISION_RETENTION_HOT_JOURNAL_SCHEMA_VERSION: u32 = 1;
pub const KERNEL_PROJECT_TRANSITION_DECISION_RETENTION_RECOVERY_SCHEMA_VERSION: u32 = 1;

pub(super) const MAX_PROJECT_TRANSITION_DECISION_RETENTION_HOT_JOURNALS: usize = 128;
pub(super) const MAX_PROJECT_TRANSITION_DECISION_RETENTION_HOT_JOURNAL_BYTES: u64 =
    12 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct KernelProjectTransitionDecisionRetentionInput {
    pub recovery_plan_evidence_hash: String,
    pub acknowledgement_id: String,
    pub diagnostic: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelProjectTransitionDecisionRetentionStatus {
    CleanNoop,
    Committed,
    RecoveryAttention,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelProjectTransitionDecisionRetentionHotJournalDiskState {
    NoEffect,
    CompletedRetention,
    PartialRetention,
    ConflictState,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction {
    ClearNoEffectJournal,
    ClearCompletedJournal,
    RestoreBeforeJournal,
    ManualReviewConflict,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionDecisionRetentionReceipt {
    pub schema_version: u32,
    pub retention_id: String,
    pub session_id: String,
    pub decision_journal_path: String,
    pub archive_path: Option<String>,
    pub hot_journal_path: Option<String>,
    pub status: KernelProjectTransitionDecisionRetentionStatus,
    pub started_at_ms: u128,
    pub completed_at_ms: u128,
    pub acknowledgement_id: String,
    pub recovery_plan_evidence_hash: String,
    pub diagnostic: String,
    pub candidate_record_ids: Vec<String>,
    pub before_journal_hash: String,
    pub after_journal_hash: String,
    pub archive_hash: String,
    pub hot_journal_written: bool,
    pub archive_written: bool,
    pub active_journal_written: bool,
    pub hot_journal_cleared: bool,
    pub retention_candidate_count: usize,
    pub archived_record_count: usize,
    pub kept_record_count: usize,
    pub write_receipts: Vec<WriteReceipt>,
    pub recovery_diagnostic: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionDecisionRetentionHotJournalRecoveryPlan {
    pub action: KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
    pub title: String,
    pub summary: String,
    pub required_checks: Vec<String>,
    pub can_clear_journal: bool,
    pub can_restore_before_journal: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionDecisionRetentionHotJournal {
    pub schema_version: u32,
    pub retention_id: String,
    pub path: String,
    pub session_id: String,
    pub project_root: String,
    pub decision_journal_path: String,
    pub archive_path: String,
    pub created_at_ms: u128,
    pub acknowledgement_id: String,
    pub recovery_plan_evidence_hash: String,
    pub candidate_record_ids: Vec<String>,
    pub candidate_count: usize,
    pub archived_record_count: usize,
    pub kept_record_count: usize,
    pub before_journal_hash: String,
    pub after_journal_hash: String,
    pub archive_hash: String,
    pub current_journal_hash: Option<String>,
    pub archive_disk_hash: Option<String>,
    pub disk_state: KernelProjectTransitionDecisionRetentionHotJournalDiskState,
    pub recovery_plan: KernelProjectTransitionDecisionRetentionHotJournalRecoveryPlan,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionDecisionRetentionHotJournalSnapshot {
    pub path: String,
    pub retention_id: Option<String>,
    pub created_at_ms: Option<u128>,
    pub candidate_count: Option<usize>,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionDecisionRetentionRecoveryReceipt {
    pub schema_version: u32,
    pub retention_id: String,
    pub action: KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
    pub journal_path: String,
    pub decision_journal_path: String,
    pub archive_path: String,
    pub disk_state_before: KernelProjectTransitionDecisionRetentionHotJournalDiskState,
    pub journal_cleared: bool,
    pub restored_before_journal: bool,
    pub candidate_count: usize,
    pub archived_record_count: usize,
    pub kept_record_count: usize,
    pub operator_diagnostic: String,
    pub write_receipts: Vec<WriteReceipt>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProjectTransitionDecisionRetentionJournal {
    pub(super) schema_version: u32,
    pub(super) retention_id: String,
    pub(super) session_id: String,
    pub(super) project_root: String,
    pub(super) decision_journal_path: String,
    pub(super) archive_path: String,
    pub(super) created_at_ms: u128,
    pub(super) acknowledgement_id: String,
    pub(super) recovery_plan_evidence_hash: String,
    pub(super) diagnostic: String,
    pub(super) candidate_record_ids: Vec<String>,
    pub(super) candidate_count: usize,
    pub(super) archived_record_count: usize,
    pub(super) kept_record_count: usize,
    pub(super) before_journal_hash: String,
    pub(super) after_journal_hash: String,
    pub(super) archive_hash: String,
    pub(super) before_journal_text: String,
    pub(super) after_journal_text: String,
    pub(super) archive_text: String,
}
