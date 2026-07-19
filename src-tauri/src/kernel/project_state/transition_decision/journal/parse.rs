use super::super::KernelProjectTransitionDecisionRecord;

pub(super) fn parse_decision_journal_line(
    line_number: u64,
    line: &str,
) -> Result<Option<KernelProjectTransitionDecisionRecord>, String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    serde_json::from_str::<KernelProjectTransitionDecisionRecord>(trimmed)
        .map(Some)
        .map_err(|error| {
            format!(
                "Linia {} din Project Transition Decision Journal nu poate fi citită: {}",
                line_number, error
            )
        })
}
