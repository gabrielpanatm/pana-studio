use crate::kernel::project_session::ProjectSessionSnapshot;

use super::{
    project_transition_evidence_hash, read_kernel_project_transition_decision_journal_snapshot,
    KernelProjectTransitionDecisionEvidence, KernelProjectTransitionDecisionRecord,
};

pub fn require_matching_kernel_project_transition_decision(
    session: &ProjectSessionSnapshot,
    decision_id: &str,
    expected_evidence: &KernelProjectTransitionDecisionEvidence,
) -> Result<KernelProjectTransitionDecisionRecord, String> {
    let decision_id = decision_id.trim();
    if decision_id.is_empty() {
        return Err("Project Transition Decision cere decision id.".to_string());
    }
    let expected_hash = project_transition_evidence_hash(expected_evidence)?;
    let snapshot = read_kernel_project_transition_decision_journal_snapshot(session, Some(500))?;
    if !snapshot.diagnostics.is_empty()
        || snapshot.health.invalid_evidence_hash_count > 0
        || snapshot.health.duplicate_id_count > 0
    {
        return Err(format!(
            "Project Transition Decision Journal nu este verificabil: {} diagnostics, {} hash-uri invalide, {} ID-uri duplicate.",
            snapshot.diagnostics.len(),
            snapshot.health.invalid_evidence_hash_count,
            snapshot.health.duplicate_id_count
        ));
    }
    let record = snapshot
        .records
        .into_iter()
        .find(|record| record.id == decision_id)
        .ok_or_else(|| {
            format!(
                "Project Transition Decision {} nu există în jurnalul sesiunii curente.",
                decision_id
            )
        })?;
    if record.evidence_hash != expected_hash || record.evidence != *expected_evidence {
        return Err(
            "Project Transition Decision nu se mai potrivește cu evidența curentă a sesiunii."
                .to_string(),
        );
    }
    Ok(record)
}
