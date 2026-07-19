use crate::kernel::{file_buffer_store::hash_text, project_session::ProjectSessionSnapshot};

use super::super::transition_decision_recovery::KernelProjectTransitionDecisionRecoveryPlanSnapshot;
use super::{
    kinds::ack_kind_for_recovery_plan_status, KernelProjectTransitionDecisionRecoveryAckEvidence,
    KernelProjectTransitionDecisionRecoveryAckInput,
    KernelProjectTransitionDecisionRecoveryAckRecord,
    KERNEL_PROJECT_TRANSITION_DECISION_RECOVERY_ACK_SCHEMA_VERSION,
};

pub(super) fn build_recovery_ack_record(
    session: &ProjectSessionSnapshot,
    decision_journal_path: &str,
    recovery_plan: &KernelProjectTransitionDecisionRecoveryPlanSnapshot,
    input: KernelProjectTransitionDecisionRecoveryAckInput,
    acknowledged_at_ms: u128,
    id: String,
) -> Result<KernelProjectTransitionDecisionRecoveryAckRecord, String> {
    let expected_hash = recovery_plan.evidence_hash.trim();
    let provided_hash = input.recovery_plan_evidence_hash.trim();
    let diagnostic = input.diagnostic.trim();
    if expected_hash.is_empty() {
        return Err("ProjectTransition Decision recovery plan nu are evidence hash.".to_string());
    }
    if provided_hash.is_empty() {
        return Err(
            "ProjectTransition Decision recovery acknowledgement cere recoveryPlanEvidenceHash."
                .to_string(),
        );
    }
    if provided_hash != expected_hash {
        return Err(
            "ProjectTransition Decision recovery acknowledgement este stale: recoveryPlan.evidenceHash nu se mai potrivește."
                .to_string(),
        );
    }
    if diagnostic.len() < 12 {
        return Err(
            "ProjectTransition Decision recovery acknowledgement cere diagnostic operator concret, minimum 12 caractere."
                .to_string(),
        );
    }
    let ack_kind = ack_kind_for_recovery_plan_status(recovery_plan.status).ok_or_else(|| {
        format!(
            "ProjectTransition Decision recovery plan {:?} nu cere acknowledge operator.",
            recovery_plan.status
        )
    })?;
    let evidence = KernelProjectTransitionDecisionRecoveryAckEvidence {
        schema_version: KERNEL_PROJECT_TRANSITION_DECISION_RECOVERY_ACK_SCHEMA_VERSION,
        session_id: session.id.clone(),
        project_root: session.project_root.clone(),
        decision_journal_path: decision_journal_path.to_string(),
        recovery_plan_evidence_hash: expected_hash.to_string(),
        recovery_plan_status: recovery_plan.status,
        integrity_trusted: recovery_plan.integrity_trusted,
        record_count: recovery_plan.record_count,
        read_diagnostic_count: recovery_plan.read_diagnostic_count,
        invalid_evidence_hash_count: recovery_plan.invalid_evidence_hash_count,
        duplicate_id_count: recovery_plan.duplicate_id_count,
        superseded_record_count: recovery_plan.superseded_record_count,
        retention_candidate_count: recovery_plan.retention_candidate_count,
        issue_count: recovery_plan.issue_count,
    };
    let evidence_hash = recovery_ack_evidence_hash(&evidence)?;

    Ok(KernelProjectTransitionDecisionRecoveryAckRecord {
        schema_version: KERNEL_PROJECT_TRANSITION_DECISION_RECOVERY_ACK_SCHEMA_VERSION,
        id,
        acknowledged_at_ms,
        ack_kind,
        diagnostic: diagnostic.to_string(),
        evidence_hash,
        evidence,
    })
}

pub(super) fn recovery_ack_evidence_hash(
    evidence: &KernelProjectTransitionDecisionRecoveryAckEvidence,
) -> Result<String, String> {
    let serialized = serde_json::to_string(evidence).map_err(|error| {
        format!(
            "Nu am putut serializa evidența ProjectTransition Decision recovery acknowledgement: {error}"
        )
    })?;
    Ok(hash_text(&serialized))
}
