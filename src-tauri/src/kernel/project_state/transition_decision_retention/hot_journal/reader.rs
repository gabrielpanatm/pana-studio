use std::path::{Path, PathBuf};

use crate::kernel::{file_buffer_store::hash_text, project_session::ProjectSessionSnapshot};

use super::super::model::{
    KernelProjectTransitionDecisionRetentionHotJournal,
    KernelProjectTransitionDecisionRetentionHotJournalSnapshot,
    ProjectTransitionDecisionRetentionJournal,
    KERNEL_PROJECT_TRANSITION_DECISION_RETENTION_HOT_JOURNAL_SCHEMA_VERSION,
};
use super::super::stable_file::capture_retention_file_baseline;
use super::disk_state::inspect_hot_journal_disk_state;

pub(crate) fn read_hot_journal_record_from_snapshot(
    journal: &KernelProjectTransitionDecisionRetentionHotJournal,
    body: &str,
) -> Result<ProjectTransitionDecisionRetentionJournal, String> {
    let path = PathBuf::from(&journal.path);
    let record = serde_json::from_str::<ProjectTransitionDecisionRetentionJournal>(body).map_err(
        |error| {
            format!(
                "ProjectTransition Decision retention hot journal {} nu este JSON valid la recovery: {}",
                path.display(),
                error
            )
        },
    )?;
    if hash_text(&record.before_journal_text) != record.before_journal_hash
        || hash_text(&record.after_journal_text) != record.after_journal_hash
        || hash_text(&record.archive_text) != record.archive_hash
    {
        return Err(format!(
            "ProjectTransition Decision retention hot journal {} are payload/hash invalid la recovery.",
            path.display()
        ));
    }
    Ok(record)
}

pub(super) fn hot_journal_snapshot(
    path: PathBuf,
) -> KernelProjectTransitionDecisionRetentionHotJournalSnapshot {
    let body = match capture_retention_file_baseline(
        &path,
        "ProjectTransition Decision retention hot journal snapshot",
    ) {
        Ok(baseline) => baseline.text,
        Err(error) => {
            return KernelProjectTransitionDecisionRetentionHotJournalSnapshot {
                path: path.to_string_lossy().to_string(),
                retention_id: None,
                created_at_ms: None,
                candidate_count: None,
                diagnostic: Some(format!("Hot journal unreadable: {error}")),
            };
        }
    };
    let value = match serde_json::from_str::<serde_json::Value>(&body) {
        Ok(value) => value,
        Err(error) => {
            return KernelProjectTransitionDecisionRetentionHotJournalSnapshot {
                path: path.to_string_lossy().to_string(),
                retention_id: None,
                created_at_ms: None,
                candidate_count: None,
                diagnostic: Some(format!("Hot journal JSON invalid: {error}")),
            };
        }
    };

    KernelProjectTransitionDecisionRetentionHotJournalSnapshot {
        path: path.to_string_lossy().to_string(),
        retention_id: value
            .get("retentionId")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        created_at_ms: value
            .get("createdAtMs")
            .and_then(serde_json::Value::as_u64)
            .map(u128::from),
        candidate_count: value
            .get("candidateCount")
            .and_then(serde_json::Value::as_u64)
            .map(|count| count as usize),
        diagnostic: None,
    }
}

pub(super) fn read_project_transition_decision_retention_hot_journal(
    session: &ProjectSessionSnapshot,
    path: &Path,
) -> Result<KernelProjectTransitionDecisionRetentionHotJournal, String> {
    let journal = read_hot_journal_record(session, path)?;
    Ok(inspect_hot_journal_disk_state(path, journal))
}

fn read_hot_journal_record(
    session: &ProjectSessionSnapshot,
    path: &Path,
) -> Result<ProjectTransitionDecisionRetentionJournal, String> {
    let baseline = capture_retention_file_baseline(
        path,
        "ProjectTransition Decision retention hot journal read",
    )?;
    let journal = serde_json::from_str::<ProjectTransitionDecisionRetentionJournal>(&baseline.text)
        .map_err(|error| {
            format!(
                "ProjectTransition Decision retention hot journal {} nu este JSON valid: {}",
                path.display(),
                error
            )
        })?;
    validate_hot_journal_record(session, path, &journal)?;
    Ok(journal)
}

fn validate_hot_journal_record(
    session: &ProjectSessionSnapshot,
    path: &Path,
    journal: &ProjectTransitionDecisionRetentionJournal,
) -> Result<(), String> {
    if journal.schema_version
        != KERNEL_PROJECT_TRANSITION_DECISION_RETENTION_HOT_JOURNAL_SCHEMA_VERSION
    {
        return Err(format!(
            "ProjectTransition Decision retention hot journal {} are schemaVersion {}, dar nucleul suportă {}.",
            path.display(),
            journal.schema_version,
            KERNEL_PROJECT_TRANSITION_DECISION_RETENTION_HOT_JOURNAL_SCHEMA_VERSION
        ));
    }
    if journal.session_id != session.id {
        return Err(format!(
            "ProjectTransition Decision retention hot journal {} aparține sesiunii {}, dar sesiunea curentă este {}.",
            path.display(),
            journal.session_id,
            session.id
        ));
    }
    if journal.project_root != session.project_root {
        return Err(format!(
            "ProjectTransition Decision retention hot journal {} aparține proiectului {}, dar sesiunea curentă este {}.",
            path.display(),
            journal.project_root,
            session.project_root
        ));
    }
    if journal.candidate_count != journal.candidate_record_ids.len()
        || journal.candidate_count != journal.archived_record_count
    {
        return Err(format!(
            "ProjectTransition Decision retention hot journal {} declară candidați incoerenți: candidateCount={}, ids={}, archived={}.",
            path.display(),
            journal.candidate_count,
            journal.candidate_record_ids.len(),
            journal.archived_record_count
        ));
    }
    if journal.candidate_count == 0 {
        return Err(format!(
            "ProjectTransition Decision retention hot journal {} nu conține candidați.",
            path.display()
        ));
    }
    if hash_text(&journal.before_journal_text) != journal.before_journal_hash {
        return Err(format!(
            "ProjectTransition Decision retention hot journal {} are beforeJournalHash invalid.",
            path.display()
        ));
    }
    if hash_text(&journal.after_journal_text) != journal.after_journal_hash {
        return Err(format!(
            "ProjectTransition Decision retention hot journal {} are afterJournalHash invalid.",
            path.display()
        ));
    }
    if hash_text(&journal.archive_text) != journal.archive_hash {
        return Err(format!(
            "ProjectTransition Decision retention hot journal {} are archiveHash invalid.",
            path.display()
        ));
    }
    Ok(())
}
