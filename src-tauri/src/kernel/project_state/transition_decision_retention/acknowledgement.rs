use crate::kernel::project_session::ProjectSessionSnapshot;

use super::super::{
    read_kernel_project_transition_decision_recovery_ack_journal_snapshot,
    KernelProjectTransitionDecisionRecoveryAckKind,
    KernelProjectTransitionDecisionRecoveryAckRecord,
};

pub(super) fn require_retention_acknowledgement(
    session: &ProjectSessionSnapshot,
    recovery_plan_evidence_hash: &str,
    acknowledgement_id: &str,
) -> Result<KernelProjectTransitionDecisionRecoveryAckRecord, String> {
    let acknowledgement_id = acknowledgement_id.trim();
    if acknowledgement_id.is_empty() {
        return Err(
            "ProjectTransition Decision retention cere acknowledgementId verificabil.".to_string(),
        );
    }
    let snapshot =
        read_kernel_project_transition_decision_recovery_ack_journal_snapshot(session, Some(500))?;
    if !snapshot.diagnostics.is_empty()
        || snapshot.health.invalid_evidence_hash_count > 0
        || snapshot.health.duplicate_id_count > 0
    {
        return Err(format!(
            "ProjectTransition Decision retention blocat: acknowledgement journal nu este verificabil ({} diagnostics, {} hash-uri invalide, {} duplicate).",
            snapshot.diagnostics.len(),
            snapshot.health.invalid_evidence_hash_count,
            snapshot.health.duplicate_id_count
        ));
    }
    let record = snapshot
        .records
        .into_iter()
        .find(|record| record.id == acknowledgement_id)
        .ok_or_else(|| {
            format!(
                "ProjectTransition Decision retention blocat: acknowledgement {} nu există în sesiunea curentă.",
                acknowledgement_id
            )
        })?;
    if record.ack_kind != KernelProjectTransitionDecisionRecoveryAckKind::AcknowledgeRetentionReview
    {
        return Err(format!(
            "ProjectTransition Decision retention blocat: acknowledgement {} este {:?}, nu acknowledge_retention_review.",
            record.id, record.ack_kind
        ));
    }
    if record.evidence.recovery_plan_evidence_hash != recovery_plan_evidence_hash {
        return Err(
            "ProjectTransition Decision retention blocat: acknowledgement-ul este stale față de recoveryPlan.evidenceHash curent."
                .to_string(),
        );
    }
    Ok(record)
}
