use super::KernelProjectTransitionDecisionReuseStatus;

pub(super) fn reuse_status_copy(
    status: KernelProjectTransitionDecisionReuseStatus,
    record_count: usize,
    context_count: usize,
    repeated_context_count: usize,
    superseded_record_count: usize,
) -> (String, String, String) {
    match status {
        KernelProjectTransitionDecisionReuseStatus::NoDecisions => (
            "Fără decizii reutilizabile".to_string(),
            "Decision Journal nu conține decizii operator pentru sesiunea curentă.".to_string(),
            "Continuă să ceri decizie operator doar când ProjectTransitionPolicy cere confirmare."
                .to_string(),
        ),
        KernelProjectTransitionDecisionReuseStatus::ExactEvidenceOnly => (
            "Decizii strict pe evidență exactă".to_string(),
            format!(
                "{} decizii sunt grupate în {} contexte; niciun context nu are recorduri istorice înlocuite.",
                record_count, context_count
            ),
            "O decizie poate fi consumată doar dacă evidence hash-ul curent se potrivește exact cu recordul."
                .to_string(),
        ),
        KernelProjectTransitionDecisionReuseStatus::RepeatedContext => (
            "Context repetat în Decision Journal".to_string(),
            format!(
                "{} contexte sunt repetate și {} recorduri istorice sunt superseded în jurnal.",
                repeated_context_count, superseded_record_count
            ),
            "Tratează recordurile superseded ca audit istoric; pentru tranziția curentă folosește doar match exact de evidență."
                .to_string(),
        ),
        KernelProjectTransitionDecisionReuseStatus::BlockedByIntegrity => (
            "Reuse guidance blocat de integritate".to_string(),
            "Decision Journal are diagnostics, hash invalid sau ID duplicat; guidance-ul rămâne informativ și neconsumabil."
                .to_string(),
            "Clarifică integrity health înainte să depinzi de orice decizie operator din jurnal."
                .to_string(),
        ),
    }
}
