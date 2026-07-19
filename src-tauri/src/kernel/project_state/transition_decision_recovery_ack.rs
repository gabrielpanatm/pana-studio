use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};

use crate::kernel::project_session::ProjectSessionSnapshot;

use super::{
    transition_decision::KernelProjectTransitionDecisionJournalSnapshot,
    transition_decision_recovery::KernelProjectTransitionDecisionRecoveryPlanStatus,
};

mod events;
mod ids;
mod journal;
mod kinds;
mod records;
#[cfg(test)]
mod tests;
mod writes;

pub use kinds::recovery_ack_kind_code;

use events::append_project_transition_decision_recovery_acknowledged_event;
use ids::{
    next_project_transition_decision_recovery_ack_id,
    project_transition_decision_recovery_ack_journal_path,
};
use journal::read_project_transition_decision_recovery_ack_journal_from_path;
use records::{build_recovery_ack_record, recovery_ack_evidence_hash};
use writes::append_project_transition_decision_recovery_ack_journal_record;

pub const KERNEL_PROJECT_TRANSITION_DECISION_RECOVERY_ACK_SCHEMA_VERSION: u32 = 1;
pub const KERNEL_PROJECT_TRANSITION_DECISION_RECOVERY_ACK_RECEIPT_SCHEMA_VERSION: u32 = 1;
const PROJECT_TRANSITION_DECISION_RECOVERY_ACK_FILE: &str =
    "project-transition-decision-recovery-acknowledgements.jsonl";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelProjectTransitionDecisionRecoveryAckKind {
    AcknowledgeIntegrityBlocked,
    AcknowledgeRetentionReview,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelProjectTransitionDecisionRecoveryAckJournalHealthStatus {
    Clean,
    HasAcknowledgements,
    IntegrityWarning,
    Degraded,
}

#[derive(Clone, Debug)]
pub struct KernelProjectTransitionDecisionRecoveryAckInput {
    pub recovery_plan_evidence_hash: String,
    pub diagnostic: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionDecisionRecoveryAckEvidence {
    pub schema_version: u32,
    pub session_id: String,
    pub project_root: String,
    pub decision_journal_path: String,
    pub recovery_plan_evidence_hash: String,
    pub recovery_plan_status: KernelProjectTransitionDecisionRecoveryPlanStatus,
    pub integrity_trusted: bool,
    pub record_count: usize,
    pub read_diagnostic_count: usize,
    pub invalid_evidence_hash_count: usize,
    pub duplicate_id_count: usize,
    pub superseded_record_count: usize,
    pub retention_candidate_count: usize,
    pub issue_count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionDecisionRecoveryAckRecord {
    pub schema_version: u32,
    pub id: String,
    pub acknowledged_at_ms: u128,
    pub ack_kind: KernelProjectTransitionDecisionRecoveryAckKind,
    pub diagnostic: String,
    pub evidence_hash: String,
    pub evidence: KernelProjectTransitionDecisionRecoveryAckEvidence,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionDecisionRecoveryAckJournalSnapshot {
    pub schema_version: u32,
    pub path: String,
    pub health: KernelProjectTransitionDecisionRecoveryAckJournalHealthSnapshot,
    pub record_count: usize,
    pub returned_count: usize,
    pub records: Vec<KernelProjectTransitionDecisionRecoveryAckRecord>,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionDecisionRecoveryAckJournalHealthSnapshot {
    pub schema_version: u32,
    pub status: KernelProjectTransitionDecisionRecoveryAckJournalHealthStatus,
    pub record_count: usize,
    pub returned_count: usize,
    pub diagnostic_count: usize,
    pub invalid_evidence_hash_count: usize,
    pub duplicate_id_count: usize,
    pub latest_record_id: Option<String>,
    pub latest_acknowledged_at_ms: Option<u128>,
    pub latest_ack_kind: Option<KernelProjectTransitionDecisionRecoveryAckKind>,
    pub latest_recovery_plan_evidence_hash: Option<String>,
    pub summary: String,
    pub detail: String,
    pub recommended_action: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionDecisionRecoveryAckReceipt {
    pub schema_version: u32,
    pub acknowledgement: KernelProjectTransitionDecisionRecoveryAckRecord,
}

pub fn append_kernel_project_transition_decision_recovery_ack<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    decision_journal: &KernelProjectTransitionDecisionJournalSnapshot,
    input: KernelProjectTransitionDecisionRecoveryAckInput,
) -> Result<KernelProjectTransitionDecisionRecoveryAckReceipt, String> {
    let record = build_recovery_ack_record(
        session,
        &decision_journal.path,
        &decision_journal.recovery_plan,
        input,
        crate::kernel::observability::now_ms(),
        next_project_transition_decision_recovery_ack_id(),
    )?;
    append_project_transition_decision_recovery_ack_journal_record(app, session, &record)?;
    append_project_transition_decision_recovery_acknowledged_event(app, &record)?;

    Ok(KernelProjectTransitionDecisionRecoveryAckReceipt {
        schema_version: KERNEL_PROJECT_TRANSITION_DECISION_RECOVERY_ACK_RECEIPT_SCHEMA_VERSION,
        acknowledgement: record,
    })
}

pub fn read_kernel_project_transition_decision_recovery_ack_journal_snapshot(
    session: &ProjectSessionSnapshot,
    limit: Option<usize>,
) -> Result<KernelProjectTransitionDecisionRecoveryAckJournalSnapshot, String> {
    read_project_transition_decision_recovery_ack_journal_from_path(
        &project_transition_decision_recovery_ack_journal_path(session),
        limit.unwrap_or(40).clamp(1, 500),
    )
}
