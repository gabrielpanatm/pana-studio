use super::KernelProjectTransitionDecisionRecoveryPlanStatus;

pub(super) fn recovery_status_copy(
    status: KernelProjectTransitionDecisionRecoveryPlanStatus,
    record_count: usize,
    read_diagnostic_count: usize,
    invalid_evidence_hash_count: usize,
    duplicate_id_count: usize,
    retention_candidate_count: usize,
) -> (String, String, String) {
    match status {
        KernelProjectTransitionDecisionRecoveryPlanStatus::CleanNoop => (
            "Recovery plan curat".to_string(),
            "Decision Journal nu are recorduri și nu cere recovery sau retention.".to_string(),
            "Continuă monitorizarea read-only; nu există acțiune operator pentru acest jurnal."
                .to_string(),
        ),
        KernelProjectTransitionDecisionRecoveryPlanStatus::VerifiedAudit => (
            "Audit verificat, fără recovery".to_string(),
            format!(
                "{} decizii sunt verificabile și nu există candidați de retention în context repetat.",
                record_count
            ),
            "Păstrează jurnalul append-only; consumul deciziilor rămâne legat de evidence hash exact."
                .to_string(),
        ),
        KernelProjectTransitionDecisionRecoveryPlanStatus::RetentionReview => (
            "Retention review read-only".to_string(),
            format!(
                "{} recorduri superseded sunt candidate informative pentru o politică viitoare de retention.",
                retention_candidate_count
            ),
            "Nu executa purge; candidații sunt doar inventar până există executor dedicat, hot journal și receipt."
                .to_string(),
        ),
        KernelProjectTransitionDecisionRecoveryPlanStatus::IntegrityBlocked => (
            "Recovery blocat de integritate".to_string(),
            format!(
                "{} diagnostics de citire, {} hash-uri invalide și {} ID-uri duplicate blochează încrederea în jurnal.",
                read_diagnostic_count, invalid_evidence_hash_count, duplicate_id_count
            ),
            "Tratează jurnalul ca audit compromis și clarifică manual cauza înainte de tranziții dependente."
                .to_string(),
        ),
    }
}
