use crate::kernel::file_buffer_store::hash_text;

use super::super::KernelProjectTransitionDecision;
use super::{
    KernelProjectTransitionDecisionEvidence, KernelProjectTransitionDecisionKind,
    KernelProjectTransitionDecisionRecord, KERNEL_PROJECT_TRANSITION_DECISION_SCHEMA_VERSION,
};

pub(super) fn build_project_transition_decision_record(
    decision_kind: KernelProjectTransitionDecisionKind,
    evidence: KernelProjectTransitionDecisionEvidence,
    diagnostic: String,
    decided_at_ms: u128,
    id: String,
) -> Result<KernelProjectTransitionDecisionRecord, String> {
    let diagnostic = diagnostic.trim();
    if diagnostic.len() < 12 {
        return Err(
            "Project Transition Decision cere diagnostic operator concret, minimum 12 caractere."
                .to_string(),
        );
    }
    if evidence.transition_decision != KernelProjectTransitionDecision::Confirm {
        return Err(
            "Project Transition Decision refuză evidență care nu cere confirmare operator."
                .to_string(),
        );
    }
    Ok(KernelProjectTransitionDecisionRecord {
        schema_version: KERNEL_PROJECT_TRANSITION_DECISION_SCHEMA_VERSION,
        id,
        decided_at_ms,
        decision_kind,
        diagnostic: diagnostic.to_string(),
        evidence_hash: project_transition_evidence_hash(&evidence)?,
        evidence,
    })
}

pub(super) fn project_transition_evidence_hash(
    evidence: &KernelProjectTransitionDecisionEvidence,
) -> Result<String, String> {
    serde_json::to_string(evidence)
        .map(|serialized| hash_text(&serialized))
        .map_err(|error| format!("Nu am putut calcula hash-ul evidenței tranziției: {error}"))
}
