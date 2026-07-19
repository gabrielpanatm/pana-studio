mod assessment;
mod lifecycle_policy;
mod model;
mod transition_blocked;
mod transition_decision;
mod transition_decision_labels;
mod transition_decision_recovery;
mod transition_decision_recovery_ack;
mod transition_decision_retention;
mod transition_decision_reuse;
mod transition_decision_summary;

#[cfg(test)]
mod tests;

pub use assessment::build_kernel_project_state_snapshot;
pub use lifecycle_policy::{
    build_project_transition_policy_matrix, evaluate_project_transition_policy,
    KernelProjectTransitionAction, KernelProjectTransitionDecision, KernelProjectTransitionPolicy,
    KernelProjectTransitionPolicyMatrixSnapshot, KernelProjectTransitionReason,
    KERNEL_PROJECT_TRANSITION_POLICY_MATRIX_SCHEMA_VERSION,
    KERNEL_PROJECT_TRANSITION_POLICY_SCHEMA_VERSION,
};
pub use model::{
    KernelProjectStateReason, KernelProjectStateSnapshot, KernelProjectStateStatus,
    KERNEL_PROJECT_STATE_SCHEMA_VERSION,
};
pub use transition_blocked::{
    build_kernel_project_transition_blocked_audit_snapshot,
    read_kernel_project_transition_blocked_audit_snapshot,
    KernelProjectTransitionBlockedActionSummary, KernelProjectTransitionBlockedAuditSnapshot,
    KernelProjectTransitionBlockedCause, KernelProjectTransitionBlockedCauseSummary,
    KernelProjectTransitionBlockedHealthSnapshot, KernelProjectTransitionBlockedHealthStatus,
    KernelProjectTransitionBlockedRecord, KernelProjectTransitionResolutionSurface,
    KERNEL_PROJECT_TRANSITION_BLOCKED_AUDIT_SCHEMA_VERSION,
};
pub use transition_decision::{
    append_kernel_project_transition_decision, build_kernel_project_transition_decision_evidence,
    read_kernel_project_transition_decision_journal_snapshot,
    require_matching_kernel_project_transition_decision, transition_decision_kind_code,
    KernelProjectTransitionDecisionEvidence, KernelProjectTransitionDecisionInput,
    KernelProjectTransitionDecisionJournalHealthSnapshot,
    KernelProjectTransitionDecisionJournalHealthStatus,
    KernelProjectTransitionDecisionJournalSnapshot, KernelProjectTransitionDecisionKind,
    KernelProjectTransitionDecisionReceipt, KernelProjectTransitionDecisionRecord,
    KernelProjectTransitionDirtyFileEvidence, KernelProjectTransitionDiskFileEvidence,
    KernelProjectTransitionPageJsEvidence, KernelProjectTransitionWorkspaceEvidence,
    KERNEL_PROJECT_TRANSITION_DECISION_EVIDENCE_SCHEMA_VERSION,
    KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
    KERNEL_PROJECT_TRANSITION_DECISION_RECEIPT_SCHEMA_VERSION,
    KERNEL_PROJECT_TRANSITION_DECISION_SCHEMA_VERSION,
};
pub use transition_decision_recovery::{
    KernelProjectTransitionDecisionRecoveryIssue, KernelProjectTransitionDecisionRecoveryIssueKind,
    KernelProjectTransitionDecisionRecoveryIssueSeverity,
    KernelProjectTransitionDecisionRecoveryPlanSnapshot,
    KernelProjectTransitionDecisionRecoveryPlanStatus,
    KernelProjectTransitionDecisionRetentionCandidate,
};
pub use transition_decision_recovery_ack::{
    append_kernel_project_transition_decision_recovery_ack,
    read_kernel_project_transition_decision_recovery_ack_journal_snapshot, recovery_ack_kind_code,
    KernelProjectTransitionDecisionRecoveryAckEvidence,
    KernelProjectTransitionDecisionRecoveryAckInput,
    KernelProjectTransitionDecisionRecoveryAckJournalHealthSnapshot,
    KernelProjectTransitionDecisionRecoveryAckJournalHealthStatus,
    KernelProjectTransitionDecisionRecoveryAckJournalSnapshot,
    KernelProjectTransitionDecisionRecoveryAckKind,
    KernelProjectTransitionDecisionRecoveryAckReceipt,
    KernelProjectTransitionDecisionRecoveryAckRecord,
    KERNEL_PROJECT_TRANSITION_DECISION_RECOVERY_ACK_RECEIPT_SCHEMA_VERSION,
    KERNEL_PROJECT_TRANSITION_DECISION_RECOVERY_ACK_SCHEMA_VERSION,
};
pub use transition_decision_retention::{
    active_project_transition_decision_retention_hot_journals,
    execute_project_transition_decision_retention,
    recover_project_transition_decision_retention_hot_journal,
    scan_project_transition_decision_retention_hot_journals,
    KernelProjectTransitionDecisionRetentionHotJournal,
    KernelProjectTransitionDecisionRetentionHotJournalDiskState,
    KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
    KernelProjectTransitionDecisionRetentionHotJournalRecoveryPlan,
    KernelProjectTransitionDecisionRetentionHotJournalSnapshot,
    KernelProjectTransitionDecisionRetentionInput, KernelProjectTransitionDecisionRetentionReceipt,
    KernelProjectTransitionDecisionRetentionRecoveryReceipt,
    KernelProjectTransitionDecisionRetentionStatus,
    KERNEL_PROJECT_TRANSITION_DECISION_RETENTION_HOT_JOURNAL_SCHEMA_VERSION,
    KERNEL_PROJECT_TRANSITION_DECISION_RETENTION_RECOVERY_SCHEMA_VERSION,
    KERNEL_PROJECT_TRANSITION_DECISION_RETENTION_SCHEMA_VERSION,
};
pub use transition_decision_reuse::{
    KernelProjectTransitionDecisionReuseContextSummary,
    KernelProjectTransitionDecisionReuseGuidanceSnapshot,
    KernelProjectTransitionDecisionReuseStatus,
};
pub use transition_decision_summary::{
    KernelProjectTransitionDecisionActionSummary, KernelProjectTransitionDecisionKindSummary,
};
