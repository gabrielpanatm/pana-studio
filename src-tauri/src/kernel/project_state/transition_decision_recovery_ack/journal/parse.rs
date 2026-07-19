use super::super::KernelProjectTransitionDecisionRecoveryAckRecord;

pub(super) fn parse_recovery_ack_journal_line(
    line_number: u64,
    line: &str,
) -> Result<Option<KernelProjectTransitionDecisionRecoveryAckRecord>, String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    serde_json::from_str::<KernelProjectTransitionDecisionRecoveryAckRecord>(trimmed)
        .map(Some)
        .map_err(|error| {
            format!(
                "Linia {} din Project Transition Decision recovery acknowledgement journal nu poate fi citită: {}",
                line_number, error
            )
        })
}
