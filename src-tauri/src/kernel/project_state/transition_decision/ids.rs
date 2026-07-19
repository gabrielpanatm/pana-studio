use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::kernel::{observability::now_ms, project_session::ProjectSessionSnapshot};

use super::PROJECT_TRANSITION_DECISION_FILE;

static PROJECT_TRANSITION_DECISION_COUNTER: AtomicU64 = AtomicU64::new(1);

pub(super) fn project_transition_decision_journal_path(
    session: &ProjectSessionSnapshot,
) -> PathBuf {
    PathBuf::from(&session.session_dir).join(PROJECT_TRANSITION_DECISION_FILE)
}

pub(super) fn next_project_transition_decision_id() -> String {
    let sequence = PROJECT_TRANSITION_DECISION_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(
        "project-transition-decision-{}-{}-{}",
        now_ms(),
        std::process::id(),
        sequence
    )
}
