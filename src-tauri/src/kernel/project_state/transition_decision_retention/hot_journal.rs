mod disk_state;
mod reader;
mod recovery_gate;
mod scan;

#[cfg(test)]
mod tests;

pub use scan::{
    active_project_transition_decision_retention_hot_journals,
    scan_project_transition_decision_retention_hot_journals,
};

pub(crate) use reader::read_hot_journal_record_from_snapshot;
pub(crate) use recovery_gate::{
    find_fresh_retention_hot_journal, validate_requested_recovery_action,
};
