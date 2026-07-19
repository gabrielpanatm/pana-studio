use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::kernel::{observability::now_ms, project_session::ProjectSessionSnapshot};

use super::PROJECT_TRANSITION_DECISION_RECOVERY_ACK_FILE;

static PROJECT_TRANSITION_DECISION_RECOVERY_ACK_COUNTER: AtomicU64 = AtomicU64::new(1);

pub(super) fn project_transition_decision_recovery_ack_journal_path(
    session: &ProjectSessionSnapshot,
) -> PathBuf {
    PathBuf::from(&session.session_dir).join(PROJECT_TRANSITION_DECISION_RECOVERY_ACK_FILE)
}

pub(super) fn next_project_transition_decision_recovery_ack_id() -> String {
    let sequence = PROJECT_TRANSITION_DECISION_RECOVERY_ACK_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(
        "project-transition-decision-recovery-ack-{}-{}-{}",
        now_ms(),
        std::process::id(),
        sequence
    )
}
