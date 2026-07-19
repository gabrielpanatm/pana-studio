use std::path::Path;

use crate::kernel::bounded_journal_reader::{
    read_bounded_journal_lines, BoundedJournalReadOutcome, APPEND_JOURNAL_READ_LIMITS,
};

use super::{
    KernelProjectTransitionDecisionRecoveryAckJournalHealthSnapshot,
    KernelProjectTransitionDecisionRecoveryAckJournalSnapshot,
    KERNEL_PROJECT_TRANSITION_DECISION_RECOVERY_ACK_SCHEMA_VERSION,
};

mod health;
mod integrity;
mod parse;
#[cfg(test)]
mod tests;

use health::summarize_recovery_ack_journal_health;
use integrity::recovery_ack_journal_integrity_diagnostics;
use parse::parse_recovery_ack_journal_line;

pub(super) fn read_project_transition_decision_recovery_ack_journal_from_path(
    path: &Path,
    limit: usize,
) -> Result<KernelProjectTransitionDecisionRecoveryAckJournalSnapshot, String> {
    let mut records = Vec::new();
    let mut diagnostics = Vec::new();
    let outcome = read_bounded_journal_lines(
        path,
        "Project Transition Decision recovery acknowledgement journal",
        APPEND_JOURNAL_READ_LIMITS,
        |line_number, line| match parse_recovery_ack_journal_line(line_number, line) {
            Ok(Some(record)) => records.push(record),
            Ok(None) => {}
            Err(diagnostic) => diagnostics.push(diagnostic),
        },
    )?;
    if outcome == BoundedJournalReadOutcome::Missing {
        return Ok(empty_recovery_ack_journal_snapshot(path));
    }
    let read_diagnostic_count = diagnostics.len();
    diagnostics.extend(recovery_ack_journal_integrity_diagnostics(&records));
    records.sort_by(|left, right| {
        right
            .acknowledged_at_ms
            .cmp(&left.acknowledged_at_ms)
            .then_with(|| right.id.cmp(&left.id))
    });
    let record_count = records.len();
    let health = summarize_recovery_ack_journal_health(
        &records,
        record_count,
        &diagnostics,
        read_diagnostic_count,
    );
    records.truncate(limit);
    let returned_count = records.len();

    Ok(KernelProjectTransitionDecisionRecoveryAckJournalSnapshot {
        schema_version: KERNEL_PROJECT_TRANSITION_DECISION_RECOVERY_ACK_SCHEMA_VERSION,
        path: path.to_string_lossy().to_string(),
        health: KernelProjectTransitionDecisionRecoveryAckJournalHealthSnapshot {
            returned_count,
            ..health
        },
        record_count,
        returned_count,
        records,
        diagnostics,
    })
}

fn empty_recovery_ack_journal_snapshot(
    path: &Path,
) -> KernelProjectTransitionDecisionRecoveryAckJournalSnapshot {
    KernelProjectTransitionDecisionRecoveryAckJournalSnapshot {
        schema_version: KERNEL_PROJECT_TRANSITION_DECISION_RECOVERY_ACK_SCHEMA_VERSION,
        path: path.to_string_lossy().to_string(),
        health: summarize_recovery_ack_journal_health(&[], 0, &[], 0),
        record_count: 0,
        returned_count: 0,
        records: Vec::new(),
        diagnostics: Vec::new(),
    }
}
