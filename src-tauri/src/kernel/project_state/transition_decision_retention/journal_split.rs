use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use crate::{
    kernel::{
        bounded_journal_reader::{
            read_bounded_journal_text_under_exclusive_lock, BoundedJournalExclusiveParentLock,
            APPEND_JOURNAL_READ_LIMITS,
        },
        file_buffer_store::hash_text,
    },
    project::project_disk_metadata_version_token,
};

use super::super::KernelProjectTransitionDecisionRecord;

#[derive(Clone, Debug)]
pub(super) struct ParsedDecisionJournal {
    pub before_text: String,
    pub after_text: String,
    pub archive_text: String,
    pub before_hash: String,
    pub before_version_token: String,
    pub after_hash: String,
    pub archive_hash: String,
    pub candidate_record_ids: Vec<String>,
    pub archived_record_count: usize,
    pub kept_record_count: usize,
}

#[derive(Clone, Debug)]
pub(super) struct DecisionJournalBaseline {
    pub text: String,
    pub content_hash: String,
    pub version_token: String,
}

pub(super) fn capture_decision_journal_baseline(
    path: &Path,
    parent_lock: &BoundedJournalExclusiveParentLock,
) -> Result<DecisionJournalBaseline, String> {
    let text = read_bounded_journal_text_under_exclusive_lock(
        parent_lock,
        path,
        "ProjectTransition Decision Journal retention",
        APPEND_JOURNAL_READ_LIMITS,
    )?
    .ok_or_else(|| {
        format!(
            "ProjectTransition Decision retention blocat: jurnalul activ {} lipsește.",
            path.display()
        )
    })?;
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "ProjectTransition Decision retention blocat: metadata jurnalului activ {} nu poate fi recapturată după citirea stabilă: {error}",
            path.display()
        )
    })?;
    if !metadata.file_type().is_file() {
        return Err(format!(
            "ProjectTransition Decision retention blocat: jurnalul activ {} nu mai este fișier regular.",
            path.display()
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        if metadata.nlink() != 1 {
            return Err(format!(
                "ProjectTransition Decision retention blocat: jurnalul activ {} nu mai are exact un singur hard link.",
                path.display()
            ));
        }
    }
    if metadata.len() != text.len() as u64 {
        return Err(format!(
            "ProjectTransition Decision retention blocat: jurnalul activ {} s-a schimbat între citirea stabilă și capturarea baseline-ului.",
            path.display()
        ));
    }

    Ok(DecisionJournalBaseline {
        content_hash: hash_text(&text),
        version_token: project_disk_metadata_version_token(&metadata),
        text,
    })
}

pub(super) fn parse_decision_journal_for_retention(
    path: &Path,
    candidate_record_ids: &[String],
    parent_lock: &BoundedJournalExclusiveParentLock,
) -> Result<ParsedDecisionJournal, String> {
    let baseline = capture_decision_journal_baseline(path, parent_lock)?;
    let before_text = baseline.text;
    let candidate_ids = candidate_record_ids
        .iter()
        .map(|id| id.trim().to_string())
        .collect::<BTreeSet<_>>();
    if candidate_ids.is_empty() {
        return Err(
            "ProjectTransition Decision retention blocat: lista de candidate este goală."
                .to_string(),
        );
    }
    if candidate_ids.iter().any(|id| id.is_empty()) {
        return Err(
            "ProjectTransition Decision retention blocat: lista de candidate conține ID gol."
                .to_string(),
        );
    }

    let mut kept_lines = Vec::new();
    let mut archived_lines = Vec::new();
    let mut seen_candidate_counts = BTreeMap::<String, usize>::new();
    let mut kept_record_count = 0usize;
    let mut archived_record_count = 0usize;

    for (index, line) in before_text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            kept_lines.push(line.to_string());
            continue;
        }
        let record = serde_json::from_str::<KernelProjectTransitionDecisionRecord>(trimmed)
            .map_err(|error| {
                format!(
                    "ProjectTransition Decision retention blocat: linia {} din jurnal nu poate fi citită fresh: {}",
                    index + 1,
                    error
                )
            })?;
        if candidate_ids.contains(&record.id) {
            *seen_candidate_counts.entry(record.id).or_insert(0) += 1;
            archived_record_count += 1;
            archived_lines.push(line.to_string());
        } else {
            kept_record_count += 1;
            kept_lines.push(line.to_string());
        }
    }

    let missing = candidate_ids
        .iter()
        .filter(|id| !seen_candidate_counts.contains_key(*id))
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!(
            "ProjectTransition Decision retention blocat: candidații nu mai există în jurnalul fresh: {}.",
            missing.join(", ")
        ));
    }
    let duplicate_seen = seen_candidate_counts
        .iter()
        .filter(|(_, count)| **count != 1)
        .map(|(id, count)| format!("{id} de {count} ori"))
        .collect::<Vec<_>>();
    if !duplicate_seen.is_empty() {
        return Err(format!(
            "ProjectTransition Decision retention blocat: candidații nu sunt unici în jurnalul fresh: {}.",
            duplicate_seen.join(", ")
        ));
    }

    let after_text = join_jsonl_lines(kept_lines);
    let archive_text = join_jsonl_lines(archived_lines);
    Ok(ParsedDecisionJournal {
        before_hash: baseline.content_hash,
        before_version_token: baseline.version_token,
        after_hash: hash_text(&after_text),
        archive_hash: hash_text(&archive_text),
        before_text,
        after_text,
        archive_text,
        candidate_record_ids: candidate_ids.into_iter().collect(),
        archived_record_count,
        kept_record_count,
    })
}

fn join_jsonl_lines(lines: Vec<String>) -> String {
    if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    }
}
