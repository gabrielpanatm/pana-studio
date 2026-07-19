use std::path::PathBuf;

use crate::kernel::project_session::ProjectSessionSnapshot;

const PROJECT_TRANSITION_DECISION_RETENTION_DIR: &str = "project-transition-decision-retention";
const PROJECT_TRANSITION_DECISION_RETENTION_ARCHIVE_DIR: &str = "archives";
pub(super) const PROJECT_TRANSITION_DECISION_FILE: &str = "project-transition-decisions.jsonl";

pub(super) fn retention_dir(session: &ProjectSessionSnapshot) -> PathBuf {
    PathBuf::from(&session.session_dir).join(PROJECT_TRANSITION_DECISION_RETENTION_DIR)
}

pub(super) fn retention_archive_dir(session: &ProjectSessionSnapshot) -> PathBuf {
    retention_dir(session).join(PROJECT_TRANSITION_DECISION_RETENTION_ARCHIVE_DIR)
}

pub(super) fn retention_hot_journal_path(
    session: &ProjectSessionSnapshot,
    retention_id: &str,
) -> PathBuf {
    retention_dir(session).join(format!("{retention_id}.json"))
}

pub(super) fn retention_archive_path(
    session: &ProjectSessionSnapshot,
    retention_id: &str,
) -> PathBuf {
    retention_archive_dir(session).join(format!("{retention_id}.jsonl"))
}

pub(super) fn decision_journal_path(session: &ProjectSessionSnapshot) -> PathBuf {
    PathBuf::from(&session.session_dir).join(PROJECT_TRANSITION_DECISION_FILE)
}
