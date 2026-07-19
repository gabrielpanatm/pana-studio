use serde::{Deserialize, Serialize};

use super::super::{
    lifecycle_policy::{KernelProjectTransitionAction, KernelProjectTransitionReason},
    transition_decision::KernelProjectTransitionDecisionKind,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelProjectTransitionDecisionRecoveryPlanStatus {
    CleanNoop,
    VerifiedAudit,
    RetentionReview,
    IntegrityBlocked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelProjectTransitionDecisionRecoveryIssueKind {
    ReadDiagnostic,
    InvalidEvidenceHash,
    DuplicateDecisionId,
    SupersededRecord,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelProjectTransitionDecisionRecoveryIssueSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionDecisionRecoveryPlanSnapshot {
    pub schema_version: u32,
    pub evidence_hash: String,
    pub status: KernelProjectTransitionDecisionRecoveryPlanStatus,
    pub read_only: bool,
    pub mutation_allowed: bool,
    pub integrity_trusted: bool,
    pub record_count: usize,
    pub read_diagnostic_count: usize,
    pub invalid_evidence_hash_count: usize,
    pub duplicate_id_count: usize,
    pub superseded_record_count: usize,
    pub retention_candidate_count: usize,
    pub issue_count: usize,
    pub summary: String,
    pub detail: String,
    pub recommended_action: String,
    pub issues: Vec<KernelProjectTransitionDecisionRecoveryIssue>,
    pub retention_candidates: Vec<KernelProjectTransitionDecisionRetentionCandidate>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionDecisionRecoveryIssue {
    pub schema_version: u32,
    pub kind: KernelProjectTransitionDecisionRecoveryIssueKind,
    pub severity: KernelProjectTransitionDecisionRecoveryIssueSeverity,
    pub record_id: Option<String>,
    pub count: usize,
    pub title: String,
    pub detail: String,
    pub recommended_action: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionDecisionRetentionCandidate {
    pub schema_version: u32,
    pub record_id: String,
    pub superseded_by_record_id: String,
    pub action: KernelProjectTransitionAction,
    pub decision_kind: KernelProjectTransitionDecisionKind,
    pub target_project_root: String,
    pub decided_at_ms: u128,
    pub transition_reason: KernelProjectTransitionReason,
    pub title: String,
    pub detail: String,
    pub recommended_action: String,
}
