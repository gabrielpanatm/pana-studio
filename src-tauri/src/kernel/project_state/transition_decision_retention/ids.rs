use std::sync::atomic::{AtomicU64, Ordering};

use crate::kernel::observability::now_ms;

static PROJECT_TRANSITION_DECISION_RETENTION_COUNTER: AtomicU64 = AtomicU64::new(1);

pub(super) fn next_project_transition_decision_retention_id() -> String {
    let sequence = PROJECT_TRANSITION_DECISION_RETENTION_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(
        "project-transition-decision-retention-{}-{}-{}",
        now_ms(),
        std::process::id(),
        sequence
    )
}
