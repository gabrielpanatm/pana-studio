use tauri::{AppHandle, Runtime};

use crate::kernel::{
    observability::{append_event, KernelEventKind, KernelLogEvent, KernelLogLevel},
    project_session::ProjectSessionSnapshot,
};

use super::{
    KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
    KernelProjectTransitionDecisionRetentionReceipt,
    KernelProjectTransitionDecisionRetentionRecoveryReceipt,
    KernelProjectTransitionDecisionRetentionStatus,
};

pub(super) fn append_retention_planned_event<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    receipt: &KernelProjectTransitionDecisionRetentionReceipt,
) -> Result<(), String> {
    append_retention_event(
        app,
        KernelLogLevel::Info,
        KernelEventKind::ProjectTransitionDecisionRetentionPlanned,
        session,
        receipt,
        None,
    )
}

pub(super) fn append_retention_completion_event<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    receipt: &KernelProjectTransitionDecisionRetentionReceipt,
) -> Result<(), String> {
    let event_kind =
        if receipt.status == KernelProjectTransitionDecisionRetentionStatus::RecoveryAttention {
            KernelEventKind::ProjectTransitionDecisionRetentionRecoveryAttention
        } else {
            KernelEventKind::ProjectTransitionDecisionRetentionCommitted
        };
    let level =
        if receipt.status == KernelProjectTransitionDecisionRetentionStatus::RecoveryAttention {
            KernelLogLevel::Warn
        } else {
            KernelLogLevel::Info
        };
    append_retention_event(app, level, event_kind, session, receipt, None)
}

pub(super) fn append_retention_failed_event<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    receipt: &KernelProjectTransitionDecisionRetentionReceipt,
    diagnostic: String,
) -> Result<(), String> {
    append_retention_event(
        app,
        KernelLogLevel::Error,
        KernelEventKind::ProjectTransitionDecisionRetentionFailed,
        session,
        receipt,
        Some(diagnostic),
    )
}

pub(super) fn append_retention_recovered_event<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    receipt: &KernelProjectTransitionDecisionRetentionRecoveryReceipt,
) -> Result<(), String> {
    append_retention_recovery_event(
        app,
        KernelLogLevel::Info,
        KernelEventKind::ProjectTransitionDecisionRetentionRecovered,
        session,
        receipt,
        None,
    )
}

pub(super) fn append_retention_recovery_failed_event<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    retention_id: &str,
    action: KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
    diagnostic: String,
) -> Result<(), String> {
    append_event(
        app,
        KernelLogEvent::new(
            KernelLogLevel::Error,
            KernelEventKind::ProjectTransitionDecisionRetentionFailed,
            "project_state",
            "kernel_recovery",
            "decision_retention_recovery_failed",
            Some(retention_id.to_string()),
            "ProjectTransition Decision retention hot journal recovery failed.",
            Some(diagnostic),
        )
        .with_attribute("sessionId", session.id.clone())
        .with_attribute("projectRoot", session.project_root.clone())
        .with_attribute("retentionId", retention_id.to_string())
        .with_attribute("action", action),
    )
}

fn append_retention_event<R: Runtime>(
    app: &AppHandle<R>,
    level: KernelLogLevel,
    kind: KernelEventKind,
    session: &ProjectSessionSnapshot,
    receipt: &KernelProjectTransitionDecisionRetentionReceipt,
    diagnostic: Option<String>,
) -> Result<(), String> {
    append_event(
        app,
        KernelLogEvent::new(
            level,
            kind,
            "project_state",
            "kernel_retention",
            "project_transition_decision_retention",
            Some(receipt.decision_journal_path.clone()),
            "ProjectTransition Decision retention lifecycle event.",
            diagnostic.or_else(|| receipt.recovery_diagnostic.clone()),
        )
        .with_attribute("retentionId", receipt.retention_id.clone())
        .with_attribute("sessionId", session.id.clone())
        .with_attribute("projectRoot", session.project_root.clone())
        .with_attribute("status", retention_event_status(kind, receipt.status))
        .with_attribute("acknowledgementId", receipt.acknowledgement_id.clone())
        .with_attribute(
            "recoveryPlanEvidenceHash",
            receipt.recovery_plan_evidence_hash.clone(),
        )
        .with_attribute("candidateCount", receipt.retention_candidate_count)
        .with_attribute("archivedRecordCount", receipt.archived_record_count)
        .with_attribute("keptRecordCount", receipt.kept_record_count)
        .with_attribute("hotJournalWritten", receipt.hot_journal_written)
        .with_attribute("archiveWritten", receipt.archive_written)
        .with_attribute("activeJournalWritten", receipt.active_journal_written)
        .with_attribute("hotJournalCleared", receipt.hot_journal_cleared)
        .with_attribute("writeReceiptCount", receipt.write_receipts.len()),
    )
}

fn retention_event_status(
    kind: KernelEventKind,
    receipt_status: KernelProjectTransitionDecisionRetentionStatus,
) -> &'static str {
    match kind {
        KernelEventKind::ProjectTransitionDecisionRetentionPlanned => "planned",
        KernelEventKind::ProjectTransitionDecisionRetentionFailed => "failed",
        KernelEventKind::ProjectTransitionDecisionRetentionRecoveryAttention => {
            "recovery_attention"
        }
        KernelEventKind::ProjectTransitionDecisionRetentionCommitted => match receipt_status {
            KernelProjectTransitionDecisionRetentionStatus::CleanNoop => "clean_noop",
            KernelProjectTransitionDecisionRetentionStatus::Committed => "committed",
            KernelProjectTransitionDecisionRetentionStatus::RecoveryAttention => {
                "recovery_attention"
            }
        },
        KernelEventKind::ProjectTransitionDecisionRetentionRecovered => "recovered",
        _ => "unknown",
    }
}

fn append_retention_recovery_event<R: Runtime>(
    app: &AppHandle<R>,
    level: KernelLogLevel,
    kind: KernelEventKind,
    session: &ProjectSessionSnapshot,
    receipt: &KernelProjectTransitionDecisionRetentionRecoveryReceipt,
    diagnostic: Option<String>,
) -> Result<(), String> {
    append_event(
        app,
        KernelLogEvent::new(
            level,
            kind,
            "project_state",
            "kernel_recovery",
            "project_transition_decision_retention_recovery",
            Some(receipt.journal_path.clone()),
            "ProjectTransition Decision retention hot journal recovered.",
            diagnostic,
        )
        .with_attribute("retentionId", receipt.retention_id.clone())
        .with_attribute("sessionId", session.id.clone())
        .with_attribute("projectRoot", session.project_root.clone())
        .with_attribute("action", receipt.action)
        .with_attribute("diskStateBefore", receipt.disk_state_before)
        .with_attribute("journalCleared", receipt.journal_cleared)
        .with_attribute("restoredBeforeJournal", receipt.restored_before_journal)
        .with_attribute("candidateCount", receipt.candidate_count)
        .with_attribute("archivedRecordCount", receipt.archived_record_count)
        .with_attribute("keptRecordCount", receipt.kept_record_count)
        .with_attribute("operatorDiagnostic", receipt.operator_diagnostic.clone())
        .with_attribute("writeReceiptCount", receipt.write_receipts.len()),
    )
}
