use std::path::Path;

use crate::kernel::{
    bounded_journal_reader::{
        read_bounded_journal_lines, BoundedJournalReadOutcome, APPEND_JOURNAL_READ_LIMITS,
    },
    project_state::{
        transition_decision_recovery::build_decision_recovery_plan,
        transition_decision_reuse::summarize_decision_reuse_guidance,
        transition_decision_summary::{
            summarize_decision_journal_by_kind, summarize_latest_decisions_by_action,
        },
    },
};

use super::{
    KernelProjectTransitionDecisionJournalHealthSnapshot,
    KernelProjectTransitionDecisionJournalSnapshot,
    KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
};

mod health;
mod integrity;
mod parse;

use health::summarize_decision_journal_health;
use integrity::decision_journal_integrity_diagnostics;
use parse::parse_decision_journal_line;

pub(super) fn read_project_transition_decision_journal_from_path(
    path: &Path,
    limit: usize,
) -> Result<KernelProjectTransitionDecisionJournalSnapshot, String> {
    let mut records = Vec::new();
    let mut diagnostics = Vec::new();
    let outcome = read_bounded_journal_lines(
        path,
        "Project Transition Decision Journal",
        APPEND_JOURNAL_READ_LIMITS,
        |line_number, line| match parse_decision_journal_line(line_number, line) {
            Ok(Some(record)) => records.push(record),
            Ok(None) => {}
            Err(diagnostic) => diagnostics.push(diagnostic),
        },
    )?;
    if outcome == BoundedJournalReadOutcome::Missing {
        return Ok(empty_decision_journal_snapshot(path));
    }
    let read_diagnostic_count = diagnostics.len();
    diagnostics.extend(decision_journal_integrity_diagnostics(&records));
    records.sort_by(|left, right| {
        right
            .decided_at_ms
            .cmp(&left.decided_at_ms)
            .then_with(|| right.id.cmp(&left.id))
    });
    let record_count = records.len();
    let health = summarize_decision_journal_health(
        &records,
        record_count,
        &diagnostics,
        read_diagnostic_count,
    );
    let latest_by_action = summarize_latest_decisions_by_action(&records);
    let by_decision_kind = summarize_decision_journal_by_kind(&records);
    let reuse_guidance = summarize_decision_reuse_guidance(
        &records,
        !diagnostics.is_empty()
            || health.invalid_evidence_hash_count > 0
            || health.duplicate_id_count > 0,
    );
    let recovery_plan =
        build_decision_recovery_plan(&records, read_diagnostic_count, &reuse_guidance);
    records.truncate(limit);
    let returned_count = records.len();

    Ok(KernelProjectTransitionDecisionJournalSnapshot {
        schema_version: KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
        path: path.to_string_lossy().to_string(),
        health: KernelProjectTransitionDecisionJournalHealthSnapshot {
            returned_count,
            ..health
        },
        latest_by_action,
        by_decision_kind,
        reuse_guidance,
        recovery_plan,
        record_count,
        returned_count,
        records,
        diagnostics,
    })
}

fn empty_decision_journal_snapshot(path: &Path) -> KernelProjectTransitionDecisionJournalSnapshot {
    let reuse_guidance = summarize_decision_reuse_guidance(&[], false);
    let recovery_plan = build_decision_recovery_plan(&[], 0, &reuse_guidance);
    KernelProjectTransitionDecisionJournalSnapshot {
        schema_version: KERNEL_PROJECT_TRANSITION_DECISION_JOURNAL_SCHEMA_VERSION,
        path: path.to_string_lossy().to_string(),
        health: summarize_decision_journal_health(&[], 0, &[], 0),
        latest_by_action: Vec::new(),
        by_decision_kind: Vec::new(),
        reuse_guidance,
        recovery_plan,
        record_count: 0,
        returned_count: 0,
        records: Vec::new(),
        diagnostics: Vec::new(),
    }
}
