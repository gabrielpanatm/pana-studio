use serde::Serialize;

use crate::kernel::{
    project_state::KernelProjectTransitionDecisionRetentionHotJournal,
    project_workspace::ProjectWorkspaceSaveHotJournal,
};

use super::summary::RecoveryJournalFamilySummary;

pub const RECOVERY_COORDINATOR_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryCoordinatorStatus {
    Clean,
    NeedsAttention,
    Unreadable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryCoordinatorDiagnosticSeverity {
    Warning,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryCoordinatorDiagnostic {
    pub severity: RecoveryCoordinatorDiagnosticSeverity,
    pub code: String,
    pub transaction_id: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryCoordinatorScan {
    pub schema_version: u32,
    pub session_id: String,
    pub project_root: String,
    pub scanned_at_ms: u128,
    pub status: RecoveryCoordinatorStatus,
    pub hot_project_workspace_save_journals: Vec<ProjectWorkspaceSaveHotJournal>,
    pub hot_project_transition_decision_retention_journals:
        Vec<KernelProjectTransitionDecisionRetentionHotJournal>,
    pub hot_journal_families: Vec<RecoveryJournalFamilySummary>,
    pub diagnostics: Vec<RecoveryCoordinatorDiagnostic>,
}

impl RecoveryCoordinatorScan {
    pub(super) fn clean(session_id: String, project_root: String, scanned_at_ms: u128) -> Self {
        Self {
            schema_version: RECOVERY_COORDINATOR_SCHEMA_VERSION,
            session_id,
            project_root,
            scanned_at_ms,
            status: RecoveryCoordinatorStatus::Clean,
            hot_project_workspace_save_journals: Vec::new(),
            hot_project_transition_decision_retention_journals: Vec::new(),
            hot_journal_families: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    pub(super) fn require_attention(&mut self) {
        if self.status == RecoveryCoordinatorStatus::Clean {
            self.status = RecoveryCoordinatorStatus::NeedsAttention;
        }
    }

    pub(super) fn mark_unreadable(&mut self) {
        self.status = RecoveryCoordinatorStatus::Unreadable;
    }
}
