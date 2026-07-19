use std::path::{Path, PathBuf};

use tauri::{AppHandle, Runtime};

use crate::kernel::{
    project_session::ProjectSessionSnapshot,
    write_authority::{
        WriteAuthority, WriteCategory, WriteIntent, WriteOperationKind, WriteOwner, WritePolicy,
        WriteReceipt, WriteTarget,
    },
};

use super::{
    model::ProjectTransitionDecisionRetentionJournal,
    paths::{retention_archive_dir, retention_dir, retention_hot_journal_path},
};

pub(super) fn write_project_transition_decision_retention_hot_journal<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    journal: &ProjectTransitionDecisionRetentionJournal,
) -> Result<WriteReceipt, String> {
    let path = retention_hot_journal_path(session, &journal.retention_id);
    let body = serde_json::to_string_pretty(journal).map_err(|error| {
        format!("Nu am putut serializa ProjectTransition Decision retention hot journal: {error}")
    })?;
    let target = WriteTarget::new(
        path,
        retention_dir(session),
        format!(
            "session/{}/project-transition-decision-retention/{}.json",
            session.id, journal.retention_id
        ),
    )
    .with_expected_absent();
    let intent = WriteIntent::new(
        WriteCategory::InternalAppWrite,
        WriteOwner::Kernel,
        WriteOperationKind::WriteText,
        target,
        WritePolicy::project_transition_decision_retention_journal(),
        format!(
            "Write ProjectTransition Decision retention hot journal {}",
            journal.retention_id
        ),
    );
    WriteAuthority::new(app)
        .write_text(intent, &body)
        .map_err(|error| error.into_terminal_diagnostic())
}

pub(super) fn write_project_transition_decision_retention_archive<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    archive_path: &Path,
    journal: &ProjectTransitionDecisionRetentionJournal,
) -> Result<WriteReceipt, String> {
    let target = WriteTarget::new(
        archive_path.to_path_buf(),
        retention_archive_dir(session),
        format!(
            "session/{}/project-transition-decision-retention/archives/{}.jsonl",
            session.id, journal.retention_id
        ),
    )
    .with_expected_absent();
    let intent = WriteIntent::new(
        WriteCategory::InternalAppWrite,
        WriteOwner::Kernel,
        WriteOperationKind::WriteText,
        target,
        WritePolicy::project_transition_decision_retention_journal(),
        format!(
            "Archive ProjectTransition Decision retention candidates {}",
            journal.retention_id
        ),
    );
    WriteAuthority::new(app)
        .write_text(intent, &journal.archive_text)
        .map_err(|error| error.into_terminal_diagnostic())
}

pub(super) fn write_project_transition_decision_journal<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    path: &Path,
    body: &str,
    retention_id: &str,
    expected_version_token: &str,
    expected_content_hash: &str,
) -> Result<WriteReceipt, String> {
    let target = WriteTarget::new(
        path.to_path_buf(),
        PathBuf::from(&session.session_dir),
        format!("session/{}/project-transition-decisions.jsonl", session.id),
    )
    .with_expected_present(
        expected_version_token.to_string(),
        Some(expected_content_hash.to_string()),
    );
    let intent = WriteIntent::new(
        WriteCategory::InternalAppWrite,
        WriteOwner::Kernel,
        WriteOperationKind::WriteText,
        target,
        WritePolicy::project_transition_decision_retention_journal(),
        format!("Rewrite ProjectTransition Decision Journal after retention {retention_id}"),
    );
    WriteAuthority::new(app)
        .write_text(intent, body)
        .map_err(|error| error.into_terminal_diagnostic())
}

pub(super) fn clear_project_transition_decision_retention_hot_journal<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    journal_path: &Path,
    retention_id: &str,
    expected_version_token: &str,
    expected_content_hash: &str,
) -> Result<WriteReceipt, String> {
    let target = WriteTarget::new(
        journal_path.to_path_buf(),
        retention_dir(session),
        format!(
            "session/{}/project-transition-decision-retention/{retention_id}.json",
            session.id
        ),
    )
    .with_expected_present(
        expected_version_token.to_string(),
        Some(expected_content_hash.to_string()),
    );
    let intent = WriteIntent::new(
        WriteCategory::InternalAppWrite,
        WriteOwner::Kernel,
        WriteOperationKind::RemoveFile,
        target,
        WritePolicy::project_transition_decision_retention_lifecycle(),
        format!("Clear ProjectTransition Decision retention hot journal {retention_id}"),
    );
    WriteAuthority::new(app)
        .remove_file_if_exists(intent)
        .map_err(|error| error.into_terminal_diagnostic())
}
