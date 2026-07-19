use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};

use crate::kernel::project_session::ProjectSessionSnapshot;

use super::transition_decision_recovery::KernelProjectTransitionDecisionRecoveryPlanSnapshot;
use super::transition_decision_reuse::KernelProjectTransitionDecisionReuseGuidanceSnapshot;
use super::transition_decision_summary::{
    KernelProjectTransitionDecisionActionSummary, KernelProjectTransitionDecisionKindSummary,
};
use super::{
    KernelProjectStateReason, KernelProjectStateStatus, KernelProjectTransitionAction,
    KernelProjectTransitionDecision, KernelProjectTransitionReason,
};

mod events;
mod evidence;
mod gate;
mod ids;
mod journal;
mod kinds;
mod records;
mod writes;

pub use evidence::build_kernel_project_transition_decision_evidence;
pub use gate::require_matching_kernel_project_transition_decision;
pub use kinds::transition_decision_kind_code;

use events::append_project_transition_decision_recorded_event;
use ids::{next_project_transition_decision_id, project_transition_decision_journal_path};
use journal::read_project_transition_decision_journal_from_path;
use kinds::decision_kind_for_transition_reason;
use records::{build_project_transition_decision_record, project_transition_evidence_hash};
use writes::append_project_transition_decision_journal_record;

pub const KERNEL_PROJECT_TRANSITION_DECISION_SCHEMA_VERSION: u32 = 1;
pub const KERNEL_PROJECT_TRANSITION_DECISION_EVIDENCE_SCHEMA_VERSION: u32 = 3;
pub const KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION: u32 = 6;
pub const KERNEL_PROJECT_TRANSITION_DECISION_RECEIPT_SCHEMA_VERSION: u32 = 1;
const PROJECT_TRANSITION_DECISION_FILE: &str = "project-transition-decisions.jsonl";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelProjectTransitionDecisionKind {
    DiscardLocalDraftsForTransition,
    AcknowledgeDirtyHistoryForTransition,
    DiscardSessionForExternalReload,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelProjectTransitionDecisionJournalHealthStatus {
    Clean,
    HasDecisions,
    IntegrityWarning,
    Degraded,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionDirtyFileEvidence {
    pub relative_path: String,
    pub baseline_hash: String,
    pub current_hash: String,
    pub current_bytes: u64,
    pub revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionDiskFileEvidence {
    pub relative_path: String,
    pub kind: String,
    pub baseline_hash: String,
    pub disk_hash: Option<String>,
    pub revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionPageJsEvidence {
    pub session_id: String,
    pub project_root: String,
    pub revision: u64,
    pub dirty_count: usize,
    pub fingerprint: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionWorkspaceEvidence {
    pub revision: u64,
    pub disk_generation: u64,
    pub dirty: bool,
    pub dirty_document_count: usize,
    pub created_document_count: usize,
    pub deleted_document_count: usize,
    pub dirty_page_js_count: usize,
    pub undo_count: usize,
    pub redo_count: usize,
    pub fingerprint: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionDecisionEvidence {
    pub schema_version: u32,
    pub action: KernelProjectTransitionAction,
    pub target_project_root: String,
    pub session_id: String,
    pub project_root: String,
    pub project_state_status: KernelProjectStateStatus,
    pub project_state_reason: KernelProjectStateReason,
    pub transition_decision: KernelProjectTransitionDecision,
    pub transition_reason: KernelProjectTransitionReason,
    pub workspace_dirty_resource_count: usize,
    pub dirty_files: Vec<KernelProjectTransitionDirtyFileEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub disk_files: Vec<KernelProjectTransitionDiskFileEvidence>,
    pub workspace: KernelProjectTransitionWorkspaceEvidence,
}

#[derive(Clone, Debug)]
pub struct KernelProjectTransitionDecisionInput {
    pub target_project_root: String,
    pub diagnostic: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionDecisionRecord {
    pub schema_version: u32,
    pub id: String,
    pub decided_at_ms: u128,
    pub decision_kind: KernelProjectTransitionDecisionKind,
    pub diagnostic: String,
    pub evidence_hash: String,
    pub evidence: KernelProjectTransitionDecisionEvidence,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionDecisionJournalSnapshot {
    pub schema_version: u32,
    pub path: String,
    pub health: KernelProjectTransitionDecisionJournalHealthSnapshot,
    pub latest_by_action: Vec<KernelProjectTransitionDecisionActionSummary>,
    pub by_decision_kind: Vec<KernelProjectTransitionDecisionKindSummary>,
    pub reuse_guidance: KernelProjectTransitionDecisionReuseGuidanceSnapshot,
    pub recovery_plan: KernelProjectTransitionDecisionRecoveryPlanSnapshot,
    pub record_count: usize,
    pub returned_count: usize,
    pub records: Vec<KernelProjectTransitionDecisionRecord>,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionDecisionJournalHealthSnapshot {
    pub schema_version: u32,
    pub status: KernelProjectTransitionDecisionJournalHealthStatus,
    pub record_count: usize,
    pub returned_count: usize,
    pub diagnostic_count: usize,
    pub invalid_evidence_hash_count: usize,
    pub duplicate_id_count: usize,
    pub latest_record_id: Option<String>,
    pub latest_decided_at_ms: Option<u128>,
    pub latest_decision_kind: Option<KernelProjectTransitionDecisionKind>,
    pub summary: String,
    pub detail: String,
    pub recommended_action: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionDecisionReceipt {
    pub schema_version: u32,
    pub decision: KernelProjectTransitionDecisionRecord,
}

pub fn append_kernel_project_transition_decision<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    policy: &super::KernelProjectTransitionPolicy,
    evidence: KernelProjectTransitionDecisionEvidence,
    input: KernelProjectTransitionDecisionInput,
) -> Result<KernelProjectTransitionDecisionReceipt, String> {
    if input.target_project_root.trim() != evidence.target_project_root {
        return Err(
            "Project Transition Decision target-ul cerut nu se potrivește cu evidența.".to_string(),
        );
    }
    if policy.decision != KernelProjectTransitionDecision::Confirm {
        return Err(
            "Project Transition Decision se poate înregistra doar pentru politici confirm."
                .to_string(),
        );
    }
    let decision_kind = decision_kind_for_transition_reason(policy.reason).ok_or_else(|| {
        format!(
            "Project Transition Decision nu are contract pentru reason {:?}.",
            policy.reason
        )
    })?;
    let record = build_project_transition_decision_record(
        decision_kind,
        evidence,
        input.diagnostic,
        crate::kernel::observability::now_ms(),
        next_project_transition_decision_id(),
    )?;
    append_project_transition_decision_journal_record(app, session, &record)?;
    append_project_transition_decision_recorded_event(app, &record)?;
    Ok(KernelProjectTransitionDecisionReceipt {
        schema_version: KERNEL_PROJECT_TRANSITION_DECISION_RECEIPT_SCHEMA_VERSION,
        decision: record,
    })
}

pub fn read_kernel_project_transition_decision_journal_snapshot(
    session: &ProjectSessionSnapshot,
    limit: Option<usize>,
) -> Result<KernelProjectTransitionDecisionJournalSnapshot, String> {
    read_project_transition_decision_journal_from_path(
        &project_transition_decision_journal_path(session),
        limit.unwrap_or(80).clamp(1, 500),
    )
}
