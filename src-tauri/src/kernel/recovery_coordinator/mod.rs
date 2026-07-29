mod model;
mod summary;

use tauri::{AppHandle, Runtime};

use crate::{
    app_home::project_workspace_save_journal_dir,
    kernel::{
        observability::{append_event, now_ms, KernelEventKind, KernelLogEvent, KernelLogLevel},
        project_session::ProjectSessionSnapshot,
        project_state::scan_project_transition_decision_retention_hot_journals,
        project_workspace::scan_project_workspace_save_hot_journals,
    },
    localization::LocalizedDiagnostic,
};

pub use model::{
    RecoveryCoordinatorDiagnostic, RecoveryCoordinatorDiagnosticSeverity, RecoveryCoordinatorScan,
    RecoveryCoordinatorStatus, RECOVERY_COORDINATOR_SCHEMA_VERSION,
};
pub use summary::{
    RecoveryJournalFamily, RecoveryJournalFamilyStatus, RecoveryJournalFamilySummary,
    RecoveryJournalValueCount,
};

use summary::summarize_recovery_journals;

pub fn scan_recovery_coordinator<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
) -> Result<RecoveryCoordinatorScan, String> {
    let mut scan =
        RecoveryCoordinatorScan::clean(session.id.clone(), session.project_root.clone(), now_ms());

    let save_journal_dir = project_workspace_save_journal_dir(app, &session.project_root)?;
    match scan_project_workspace_save_hot_journals(session, &save_journal_dir) {
        Ok(journals) => {
            for journal in &journals {
                scan.diagnostics.push(RecoveryCoordinatorDiagnostic {
                    severity: RecoveryCoordinatorDiagnosticSeverity::Error,
                    code: "project_workspace_save_incomplete".to_string(),
                    transaction_id: Some(journal.transaction_id.clone()),
                    message_diagnostic: LocalizedDiagnostic::new(
                        "recovery-project-workspace-save-incomplete",
                    )
                    .with_argument("transaction", journal.transaction_id.clone()),
                });
            }
            if !journals.is_empty() {
                scan.require_attention();
            }
            scan.hot_project_workspace_save_journals = journals;
        }
        Err(error) => mark_scanner_unreadable(
            &mut scan,
            "project_workspace_save_journal_unreadable",
            error,
        ),
    }

    match scan_project_transition_decision_retention_hot_journals(session) {
        Ok(journals) => {
            for journal in &journals {
                scan.diagnostics.push(RecoveryCoordinatorDiagnostic {
                    severity: RecoveryCoordinatorDiagnosticSeverity::Error,
                    code: "project_transition_retention_incomplete".to_string(),
                    transaction_id: None,
                    message_diagnostic: LocalizedDiagnostic::new(
                        "recovery-project-transition-retention-incomplete",
                    )
                    .with_argument("retention", journal.retention_id.clone()),
                });
            }
            if !journals.is_empty() {
                scan.require_attention();
            }
            scan.hot_project_transition_decision_retention_journals = journals;
        }
        Err(error) => mark_scanner_unreadable(
            &mut scan,
            "project_transition_retention_journal_unreadable",
            error,
        ),
    }

    scan.hot_journal_families = summarize_recovery_journals(
        &scan.hot_project_workspace_save_journals,
        &scan.hot_project_transition_decision_retention_journals,
    );
    record_scan(app, &scan);
    Ok(scan)
}

fn mark_scanner_unreadable(scan: &mut RecoveryCoordinatorScan, code: &str, _message: String) {
    let diagnostic_code = match code {
        "project_workspace_save_journal_unreadable" => {
            "recovery-project-workspace-journal-unreadable"
        }
        "project_transition_retention_journal_unreadable" => {
            "recovery-project-transition-journal-unreadable"
        }
        _ => "recovery-journal-unreadable",
    };
    scan.diagnostics.push(RecoveryCoordinatorDiagnostic {
        severity: RecoveryCoordinatorDiagnosticSeverity::Error,
        code: code.to_string(),
        transaction_id: None,
        message_diagnostic: LocalizedDiagnostic::new(diagnostic_code),
    });
    scan.mark_unreadable();
}

fn record_scan<R: Runtime>(app: &AppHandle<R>, scan: &RecoveryCoordinatorScan) {
    let (level, kind) = match scan.status {
        RecoveryCoordinatorStatus::Clean => (
            KernelLogLevel::Info,
            KernelEventKind::RecoveryCoordinatorClean,
        ),
        RecoveryCoordinatorStatus::NeedsAttention => (
            KernelLogLevel::Warn,
            KernelEventKind::RecoveryCoordinatorNeedsAttention,
        ),
        RecoveryCoordinatorStatus::Unreadable => (
            KernelLogLevel::Error,
            KernelEventKind::RecoveryCoordinatorFailed,
        ),
    };
    let _ = append_event(
        app,
        KernelLogEvent::new(
            level,
            kind,
            "recovery_coordinator",
            "recovery",
            "scan_active_journals",
            Some(format!("session/{}", scan.session_id)),
            format!(
                "Recovery Coordinator: {:?}; {} familii active, {} diagnostice.",
                scan.status,
                scan.hot_journal_families.len(),
                scan.diagnostics.len()
            ),
            None,
        ),
    );
}
