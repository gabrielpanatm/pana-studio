use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::kernel::{
    file_buffer_store::hash_text,
    project_session::{ProjectRootFingerprint, ProjectSessionScanSummary, ProjectSessionSnapshot},
};

use super::super::{
    model::{
        KernelProjectTransitionDecisionRetentionHotJournal,
        KernelProjectTransitionDecisionRetentionHotJournalDiskState,
        KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
        ProjectTransitionDecisionRetentionJournal,
        KERNEL_PROJECT_TRANSITION_DECISION_RETENTION_HOT_JOURNAL_SCHEMA_VERSION,
    },
    paths::{
        decision_journal_path, retention_archive_path, retention_hot_journal_path,
        PROJECT_TRANSITION_DECISION_FILE,
    },
};
use super::{
    disk_state::{inspect_hot_journal_disk_state, recovery_plan_for_hot_journal},
    recovery_gate::validate_requested_recovery_action,
};

#[test]
fn hot_journal_classifies_completed_no_effect_partial_and_conflict() {
    let root = temp_dir("decision-retention-hot-state");
    fs::create_dir_all(&root).unwrap();
    let session = session(&root);
    let before = "{\"id\":\"old\"}\n{\"id\":\"new\"}\n";
    let after = "{\"id\":\"new\"}\n";
    let archive = "{\"id\":\"old\"}\n";
    let active_path = root.join(PROJECT_TRANSITION_DECISION_FILE);
    let archive_path = retention_archive_path(&session, "retention-1");
    fs::create_dir_all(archive_path.parent().unwrap()).unwrap();

    fs::write(&active_path, before).unwrap();
    let no_effect = inspect_hot_journal_disk_state(
        &retention_hot_journal_path(&session, "retention-1"),
        journal(&session, "retention-1", before, after, archive),
    );
    assert_eq!(
        no_effect.disk_state,
        KernelProjectTransitionDecisionRetentionHotJournalDiskState::NoEffect
    );

    fs::write(&active_path, after).unwrap();
    fs::write(&archive_path, archive).unwrap();
    let completed = inspect_hot_journal_disk_state(
        &retention_hot_journal_path(&session, "retention-1"),
        journal(&session, "retention-1", before, after, archive),
    );
    assert_eq!(
        completed.disk_state,
        KernelProjectTransitionDecisionRetentionHotJournalDiskState::CompletedRetention
    );

    fs::remove_file(&archive_path).unwrap();
    let partial = inspect_hot_journal_disk_state(
        &retention_hot_journal_path(&session, "retention-1"),
        journal(&session, "retention-1", before, after, archive),
    );
    assert_eq!(
        partial.disk_state,
        KernelProjectTransitionDecisionRetentionHotJournalDiskState::PartialRetention
    );

    fs::write(&active_path, before).unwrap();
    fs::write(&archive_path, archive).unwrap();
    let archive_without_active_rewrite = inspect_hot_journal_disk_state(
        &retention_hot_journal_path(&session, "retention-1"),
        journal(&session, "retention-1", before, after, archive),
    );
    assert_eq!(
        archive_without_active_rewrite.disk_state,
        KernelProjectTransitionDecisionRetentionHotJournalDiskState::ConflictState
    );

    fs::write(&active_path, after).unwrap();
    fs::write(&archive_path, "corrupt\n").unwrap();
    let corrupt_archive = inspect_hot_journal_disk_state(
        &retention_hot_journal_path(&session, "retention-1"),
        journal(&session, "retention-1", before, after, archive),
    );
    assert_eq!(
        corrupt_archive.disk_state,
        KernelProjectTransitionDecisionRetentionHotJournalDiskState::ConflictState
    );

    fs::write(&active_path, "external change\n").unwrap();
    let conflict = inspect_hot_journal_disk_state(
        &retention_hot_journal_path(&session, "retention-1"),
        journal(&session, "retention-1", before, after, archive),
    );
    assert_eq!(
        conflict.disk_state,
        KernelProjectTransitionDecisionRetentionHotJournalDiskState::ConflictState
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn recovery_validation_rejects_manual_conflict_mutation() {
    let root = temp_dir("decision-retention-conflict-recovery");
    fs::create_dir_all(&root).unwrap();
    let session = session(&root);
    let hot = KernelProjectTransitionDecisionRetentionHotJournal {
        schema_version: KERNEL_PROJECT_TRANSITION_DECISION_RETENTION_HOT_JOURNAL_SCHEMA_VERSION,
        retention_id: "retention-1".to_string(),
        path: retention_hot_journal_path(&session, "retention-1")
            .to_string_lossy()
            .to_string(),
        session_id: session.id.clone(),
        project_root: session.project_root.clone(),
        decision_journal_path: decision_journal_path(&session)
            .to_string_lossy()
            .to_string(),
        archive_path: retention_archive_path(&session, "retention-1")
            .to_string_lossy()
            .to_string(),
        created_at_ms: 1,
        acknowledgement_id: "ack-1".to_string(),
        recovery_plan_evidence_hash: "plan-hash".to_string(),
        candidate_record_ids: vec!["old".to_string()],
        candidate_count: 1,
        archived_record_count: 1,
        kept_record_count: 1,
        before_journal_hash: "before".to_string(),
        after_journal_hash: "after".to_string(),
        archive_hash: "archive".to_string(),
        current_journal_hash: None,
        archive_disk_hash: None,
        disk_state: KernelProjectTransitionDecisionRetentionHotJournalDiskState::ConflictState,
        recovery_plan: recovery_plan_for_hot_journal(
            KernelProjectTransitionDecisionRetentionHotJournalDiskState::ConflictState,
            1,
        ),
        diagnostics: Vec::new(),
    };

    let error = validate_requested_recovery_action(
        &hot,
        KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction::ManualReviewConflict,
    )
    .unwrap_err();

    assert!(error.contains("review manual"));
    fs::remove_dir_all(root).unwrap();
}

fn journal(
    session: &ProjectSessionSnapshot,
    retention_id: &str,
    before: &str,
    after: &str,
    archive: &str,
) -> ProjectTransitionDecisionRetentionJournal {
    ProjectTransitionDecisionRetentionJournal {
        schema_version: KERNEL_PROJECT_TRANSITION_DECISION_RETENTION_HOT_JOURNAL_SCHEMA_VERSION,
        retention_id: retention_id.to_string(),
        session_id: session.id.clone(),
        project_root: session.project_root.clone(),
        decision_journal_path: decision_journal_path(session).to_string_lossy().to_string(),
        archive_path: retention_archive_path(session, retention_id)
            .to_string_lossy()
            .to_string(),
        created_at_ms: 1,
        acknowledgement_id: "ack-1".to_string(),
        recovery_plan_evidence_hash: "plan-hash".to_string(),
        diagnostic: "Operatorul testează recovery hot journal.".to_string(),
        candidate_record_ids: vec!["old".to_string()],
        candidate_count: 1,
        archived_record_count: 1,
        kept_record_count: 1,
        before_journal_hash: hash_text(before),
        after_journal_hash: hash_text(after),
        archive_hash: hash_text(archive),
        before_journal_text: before.to_string(),
        after_journal_text: after.to_string(),
        archive_text: archive.to_string(),
    }
}

fn session(root: &Path) -> ProjectSessionSnapshot {
    ProjectSessionSnapshot {
        schema_version: 1,
        id: "session-1".to_string(),
        project_root: root.join("project").to_string_lossy().to_string(),
        zola_root: root.join("project").to_string_lossy().to_string(),
        session_dir: root.to_string_lossy().to_string(),
        manifest_path: root.join("manifest.json").to_string_lossy().to_string(),
        opened_at_ms: 1,
        last_seen_at_ms: 1,
        root_fingerprint: ProjectRootFingerprint {
            canonical_path: root.join("project").to_string_lossy().to_string(),
            modified_ms: 1,
            size: 0,
            readonly: false,
            unix_device: None,
            unix_inode: None,
        },
        scan_summary: ProjectSessionScanSummary {
            is_zola: true,
            is_empty: false,
            active_theme: None,
            file_count: 1,
            directory_count: 1,
        },
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("pana-studio-{name}-{}-{nanos}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    path
}
