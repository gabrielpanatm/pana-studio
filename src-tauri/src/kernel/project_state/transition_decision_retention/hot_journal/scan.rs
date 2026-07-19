use std::fs;

use crate::kernel::project_session::ProjectSessionSnapshot;

use super::super::{
    model::{
        KernelProjectTransitionDecisionRetentionHotJournal,
        KernelProjectTransitionDecisionRetentionHotJournalSnapshot,
        MAX_PROJECT_TRANSITION_DECISION_RETENTION_HOT_JOURNALS,
    },
    paths::retention_dir,
};
use super::reader::{hot_journal_snapshot, read_project_transition_decision_retention_hot_journal};

pub fn active_project_transition_decision_retention_hot_journals(
    session: &ProjectSessionSnapshot,
) -> Result<Vec<KernelProjectTransitionDecisionRetentionHotJournalSnapshot>, String> {
    let dir = retention_dir(session);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    if !dir.is_dir() {
        return Err(format!(
            "ProjectTransition Decision retention recovery blocat: {} nu este director.",
            dir.display()
        ));
    }

    let mut entries = fs::read_dir(&dir)
        .map_err(|error| {
            format!(
                "Nu am putut citi hot journal-urile ProjectTransition Decision retention din {}: {}",
                dir.display(),
                error
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            format!(
                "Nu am putut parcurge hot journal-urile ProjectTransition Decision retention din {}: {}",
                dir.display(),
                error
            )
        })?;
    entries.sort_by_key(|entry| entry.path());

    Ok(entries
        .into_iter()
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "json")
        })
        .map(|entry| hot_journal_snapshot(entry.path()))
        .collect())
}

pub fn scan_project_transition_decision_retention_hot_journals(
    session: &ProjectSessionSnapshot,
) -> Result<Vec<KernelProjectTransitionDecisionRetentionHotJournal>, String> {
    let dir = retention_dir(session);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    if !dir.is_dir() {
        return Err(format!(
            "ProjectTransition Decision retention recovery blocat: {} nu este director.",
            dir.display()
        ));
    }

    let mut entries = fs::read_dir(&dir)
        .map_err(|error| {
            format!(
                "Nu am putut scana hot journal-urile ProjectTransition Decision retention din {}: {}",
                dir.display(),
                error
            )
        })?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.path());

    let mut journals = Vec::new();
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        if journals.len() >= MAX_PROJECT_TRANSITION_DECISION_RETENTION_HOT_JOURNALS {
            return Err(format!(
                "ProjectTransition Decision retention recovery a depășit limita de {} hot journal-uri.",
                MAX_PROJECT_TRANSITION_DECISION_RETENTION_HOT_JOURNALS
            ));
        }
        journals.push(read_project_transition_decision_retention_hot_journal(
            session, &path,
        )?);
    }

    Ok(journals)
}
