use crate::kernel::project_session::ProjectSessionSnapshot;

use super::{
    hot_journal::active_project_transition_decision_retention_hot_journals,
    model::{
        ProjectTransitionDecisionRetentionJournal,
        MAX_PROJECT_TRANSITION_DECISION_RETENTION_HOT_JOURNAL_BYTES,
    },
};

pub(super) fn validate_retention_plan_hash(expected: &str, provided: &str) -> Result<(), String> {
    let expected = expected.trim();
    let provided = provided.trim();
    if expected.is_empty() {
        return Err(
            "ProjectTransition Decision retention blocat: recovery plan-ul curent nu are evidence hash."
                .to_string(),
        );
    }
    if provided.is_empty() {
        return Err(
            "ProjectTransition Decision retention cere recoveryPlanEvidenceHash.".to_string(),
        );
    }
    if expected != provided {
        return Err(
            "ProjectTransition Decision retention blocat: recoveryPlanEvidenceHash este stale."
                .to_string(),
        );
    }
    Ok(())
}

pub(super) fn ensure_no_active_hot_journals(
    session: &ProjectSessionSnapshot,
) -> Result<(), String> {
    let active_journals = active_project_transition_decision_retention_hot_journals(session)?;
    if active_journals.is_empty() {
        return Ok(());
    }
    let paths = active_journals
        .iter()
        .map(|journal| journal.path.clone())
        .collect::<Vec<_>>();
    Err(format!(
        "ProjectTransition Decision retention blocat: există hot journal-uri active: {}.",
        paths.join("; ")
    ))
}

pub(super) fn validate_hot_journal_payload_budget(
    journal: &ProjectTransitionDecisionRetentionJournal,
) -> Result<(), String> {
    let body = serde_json::to_string(journal).map_err(|error| {
        format!("Nu am putut serializa hot journal-ul de retention pentru verificare: {error}")
    })?;
    let bytes = body.len() as u64;
    if bytes > MAX_PROJECT_TRANSITION_DECISION_RETENTION_HOT_JOURNAL_BYTES {
        return Err(format!(
            "ProjectTransition Decision retention blocat: hot journal-ul ar avea {} bytes, peste limita de {} bytes.",
            bytes, MAX_PROJECT_TRANSITION_DECISION_RETENTION_HOT_JOURNAL_BYTES
        ));
    }
    Ok(())
}

pub(super) fn normalize_operator_diagnostic(value: String) -> Result<String, String> {
    let value = value
        .trim()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if value.len() < 12 {
        return Err(
            "ProjectTransition Decision retention cere diagnostic operator concret, minimum 12 caractere."
                .to_string(),
        );
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retention_rejects_stale_plan_hash() {
        let error = validate_retention_plan_hash("current-plan", "old-plan").unwrap_err();

        assert!(error.contains("stale"));
    }

    #[test]
    fn operator_diagnostic_must_be_concrete() {
        let error = normalize_operator_diagnostic(" prea scurt ".to_string()).unwrap_err();

        assert!(error.contains("minimum 12"));
    }
}
