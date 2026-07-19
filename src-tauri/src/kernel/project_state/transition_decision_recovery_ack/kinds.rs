use super::super::transition_decision_recovery::KernelProjectTransitionDecisionRecoveryPlanStatus;
use super::KernelProjectTransitionDecisionRecoveryAckKind;

pub fn recovery_ack_kind_code(
    kind: KernelProjectTransitionDecisionRecoveryAckKind,
) -> &'static str {
    match kind {
        KernelProjectTransitionDecisionRecoveryAckKind::AcknowledgeIntegrityBlocked => {
            "acknowledge_integrity_blocked"
        }
        KernelProjectTransitionDecisionRecoveryAckKind::AcknowledgeRetentionReview => {
            "acknowledge_retention_review"
        }
    }
}

pub(super) fn ack_kind_for_recovery_plan_status(
    status: KernelProjectTransitionDecisionRecoveryPlanStatus,
) -> Option<KernelProjectTransitionDecisionRecoveryAckKind> {
    match status {
        KernelProjectTransitionDecisionRecoveryPlanStatus::IntegrityBlocked => {
            Some(KernelProjectTransitionDecisionRecoveryAckKind::AcknowledgeIntegrityBlocked)
        }
        KernelProjectTransitionDecisionRecoveryPlanStatus::RetentionReview => {
            Some(KernelProjectTransitionDecisionRecoveryAckKind::AcknowledgeRetentionReview)
        }
        KernelProjectTransitionDecisionRecoveryPlanStatus::CleanNoop
        | KernelProjectTransitionDecisionRecoveryPlanStatus::VerifiedAudit => None,
    }
}
