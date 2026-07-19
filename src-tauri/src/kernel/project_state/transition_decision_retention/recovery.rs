use std::path::Path;

use tauri::{AppHandle, Runtime};

use crate::kernel::{
    bounded_journal_reader::{
        lock_bounded_journal_parent_exclusive, read_bounded_journal_text,
        APPEND_JOURNAL_READ_LIMITS,
    },
    project_session::ProjectSessionSnapshot,
};

use super::{
    coordination::lock_project_transition_decision_retention,
    events::{append_retention_recovered_event, append_retention_recovery_failed_event},
    guards::normalize_operator_diagnostic,
    hot_journal::{
        find_fresh_retention_hot_journal, read_hot_journal_record_from_snapshot,
        validate_requested_recovery_action,
    },
    journal_split::capture_decision_journal_baseline,
    stable_file::capture_retention_file_baseline,
    writes::{
        clear_project_transition_decision_retention_hot_journal,
        write_project_transition_decision_journal,
    },
    KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
    KernelProjectTransitionDecisionRetentionRecoveryReceipt,
    KERNEL_PROJECT_TRANSITION_DECISION_RETENTION_RECOVERY_SCHEMA_VERSION,
};

pub fn recover_project_transition_decision_retention_hot_journal<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    retention_id: &str,
    action: KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
    diagnostic: String,
) -> Result<KernelProjectTransitionDecisionRetentionRecoveryReceipt, String> {
    let operator_diagnostic = normalize_operator_diagnostic(diagnostic)?;
    let result = recover_project_transition_decision_retention_hot_journal_inner(
        app,
        session,
        retention_id,
        action,
        operator_diagnostic,
    );

    match result {
        Ok(receipt) => {
            append_retention_recovered_event(app, session, &receipt)?;
            Ok(receipt)
        }
        Err(error) => {
            let _ = append_retention_recovery_failed_event(
                app,
                session,
                retention_id,
                action,
                error.clone(),
            );
            Err(error)
        }
    }
}

fn recover_project_transition_decision_retention_hot_journal_inner<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    retention_id: &str,
    action: KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
    operator_diagnostic: String,
) -> Result<KernelProjectTransitionDecisionRetentionRecoveryReceipt, String> {
    let _retention_guard = lock_project_transition_decision_retention();
    let journal = find_fresh_retention_hot_journal(session, retention_id)?;
    validate_requested_recovery_action(&journal, action)?;
    let hot_journal_baseline = capture_retention_file_baseline(
        Path::new(&journal.path),
        "ProjectTransition Decision retention recovery hot journal",
    )?;
    let hot_record = read_hot_journal_record_from_snapshot(&journal, &hot_journal_baseline.text)?;
    if hot_record.retention_id != journal.retention_id
        || hot_record.session_id != journal.session_id
        || hot_record.project_root != journal.project_root
        || hot_record.decision_journal_path != journal.decision_journal_path
        || hot_record.archive_path != journal.archive_path
        || hot_record.before_journal_hash != journal.before_journal_hash
        || hot_record.after_journal_hash != journal.after_journal_hash
        || hot_record.archive_hash != journal.archive_hash
    {
        return Err(format!(
            "ProjectTransition Decision retention recovery blocat pentru {}: hot journal-ul s-a schimbat după scanarea fresh.",
            journal.retention_id
        ));
    }

    let active_journal_path = Path::new(&journal.decision_journal_path);
    let active_journal_lock = lock_bounded_journal_parent_exclusive(
        active_journal_path,
        "ProjectTransition Decision retention recovery active journal",
    )?;
    let active_baseline =
        capture_decision_journal_baseline(active_journal_path, &active_journal_lock)?;
    let archive_body = read_bounded_journal_text(
        Path::new(&journal.archive_path),
        "ProjectTransition Decision retention recovery archive recheck",
        APPEND_JOURNAL_READ_LIMITS,
    )?;
    let archive_missing = archive_body.is_none();
    let archive_matches = archive_body.as_ref().is_some_and(|body| {
        crate::kernel::file_buffer_store::hash_text(body) == journal.archive_hash
    });

    match action {
        KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction::ClearNoEffectJournal
            if active_baseline.content_hash != journal.before_journal_hash || !archive_missing =>
        {
            return Err(format!(
                "ProjectTransition Decision retention recovery blocat pentru {}: jurnalul activ nu mai este baseline-ul before sub lock exclusiv.",
                journal.retention_id
            ));
        }
        KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction::ClearCompletedJournal
            if active_baseline.content_hash != journal.after_journal_hash || !archive_matches =>
        {
            return Err(format!(
                "ProjectTransition Decision retention recovery blocat pentru {}: starea completed nu mai este exactă sub lock exclusiv.",
                journal.retention_id
            ));
        }
        KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction::RestoreBeforeJournal
            if active_baseline.content_hash != journal.after_journal_hash || !archive_missing =>
        {
            return Err(format!(
                "ProjectTransition Decision retention recovery blocat pentru {}: starea partial nu mai este exactă sub lock exclusiv.",
                journal.retention_id
            ));
        }
        KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction::ManualReviewConflict => {
            return Err(format!(
                "ProjectTransition Decision retention recovery blocat pentru {}: conflictul cere review manual.",
                journal.retention_id
            ));
        }
        _ => {}
    }
    // La fel ca executorul normal, recovery nu poate ține parent flock peste
    // WriteAuthority fără să inverseze ordinea WAL -> parent folosită de Append.
    // Restore-ul rămâne protejat de baseline-ul exact capturat sub acest lock.
    drop(active_journal_lock);

    let mut write_receipts = Vec::new();
    let mut restored_before_journal = false;
    if action
        == KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction::RestoreBeforeJournal
    {
        let restore_receipt = write_project_transition_decision_journal(
            app,
            session,
            active_journal_path,
            &hot_record.before_journal_text,
            retention_id,
            &active_baseline.version_token,
            &active_baseline.content_hash,
        )?;
        restored_before_journal = true;
        write_receipts.push(restore_receipt);
    }

    let clear_receipt = clear_project_transition_decision_retention_hot_journal(
        app,
        session,
        Path::new(&journal.path),
        retention_id,
        &hot_journal_baseline.version_token,
        &hot_journal_baseline.content_hash,
    )?;
    write_receipts.push(clear_receipt);

    Ok(KernelProjectTransitionDecisionRetentionRecoveryReceipt {
        schema_version: KERNEL_PROJECT_TRANSITION_DECISION_RETENTION_RECOVERY_SCHEMA_VERSION,
        retention_id: journal.retention_id,
        action,
        journal_path: journal.path,
        decision_journal_path: journal.decision_journal_path,
        archive_path: journal.archive_path,
        disk_state_before: journal.disk_state,
        journal_cleared: true,
        restored_before_journal,
        candidate_count: journal.candidate_count,
        archived_record_count: journal.archived_record_count,
        kept_record_count: journal.kept_record_count,
        operator_diagnostic,
        write_receipts,
    })
}
