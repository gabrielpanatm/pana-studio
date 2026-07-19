use std::path::Path;

use crate::kernel::{
    bounded_journal_reader::{read_bounded_journal_text, APPEND_JOURNAL_READ_LIMITS},
    file_buffer_store::hash_text,
};

use super::super::model::{
    KernelProjectTransitionDecisionRetentionHotJournal,
    KernelProjectTransitionDecisionRetentionHotJournalDiskState,
    KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
    KernelProjectTransitionDecisionRetentionHotJournalRecoveryPlan,
    ProjectTransitionDecisionRetentionJournal,
};

pub(crate) fn inspect_hot_journal_disk_state(
    path: &Path,
    journal: ProjectTransitionDecisionRetentionJournal,
) -> KernelProjectTransitionDecisionRetentionHotJournal {
    let mut diagnostics = Vec::new();
    let current_journal_hash = match read_bounded_journal_text(
        Path::new(&journal.decision_journal_path),
        "ProjectTransition Decision retention recovery active journal",
        APPEND_JOURNAL_READ_LIMITS,
    ) {
        Ok(Some(body)) => Some(hash_text(&body)),
        Ok(None) => {
            diagnostics.push(format!(
                "Jurnalul activ {} lipsește.",
                journal.decision_journal_path
            ));
            None
        }
        Err(error) => {
            diagnostics.push(format!(
                "Jurnalul activ {} nu poate fi citit: {}",
                journal.decision_journal_path, error
            ));
            None
        }
    };
    let mut archive_read_failed = false;
    let archive_disk_hash = match read_bounded_journal_text(
        Path::new(&journal.archive_path),
        "ProjectTransition Decision retention recovery archive",
        APPEND_JOURNAL_READ_LIMITS,
    ) {
        Ok(Some(body)) => Some(hash_text(&body)),
        Ok(None) => None,
        Err(error) => {
            archive_read_failed = true;
            diagnostics.push(format!(
                "Arhiva {} nu poate fi citită: {}",
                journal.archive_path, error
            ));
            None
        }
    };
    let archive_matches = archive_disk_hash
        .as_ref()
        .map(|hash| hash == &journal.archive_hash)
        .unwrap_or(false);
    let archive_missing = archive_disk_hash.is_none() && !archive_read_failed;
    if archive_disk_hash.is_some() && !archive_matches {
        diagnostics.push(format!(
            "Arhiva {} există, dar hash-ul nu corespunde hot journal-ului.",
            journal.archive_path
        ));
    }

    let disk_state = match current_journal_hash.as_deref() {
        Some(current) if current == journal.before_journal_hash && archive_missing => {
            KernelProjectTransitionDecisionRetentionHotJournalDiskState::NoEffect
        }
        Some(current) if current == journal.after_journal_hash && archive_matches => {
            KernelProjectTransitionDecisionRetentionHotJournalDiskState::CompletedRetention
        }
        Some(current) if current == journal.after_journal_hash && archive_missing => {
            KernelProjectTransitionDecisionRetentionHotJournalDiskState::PartialRetention
        }
        _ => KernelProjectTransitionDecisionRetentionHotJournalDiskState::ConflictState,
    };
    let recovery_plan = recovery_plan_for_hot_journal(disk_state, journal.candidate_count);

    KernelProjectTransitionDecisionRetentionHotJournal {
        schema_version: journal.schema_version,
        retention_id: journal.retention_id,
        path: path.to_string_lossy().to_string(),
        session_id: journal.session_id,
        project_root: journal.project_root,
        decision_journal_path: journal.decision_journal_path,
        archive_path: journal.archive_path,
        created_at_ms: journal.created_at_ms,
        acknowledgement_id: journal.acknowledgement_id,
        recovery_plan_evidence_hash: journal.recovery_plan_evidence_hash,
        candidate_record_ids: journal.candidate_record_ids,
        candidate_count: journal.candidate_count,
        archived_record_count: journal.archived_record_count,
        kept_record_count: journal.kept_record_count,
        before_journal_hash: journal.before_journal_hash,
        after_journal_hash: journal.after_journal_hash,
        archive_hash: journal.archive_hash,
        current_journal_hash,
        archive_disk_hash,
        disk_state,
        recovery_plan,
        diagnostics,
    }
}

pub(crate) fn recovery_plan_for_hot_journal(
    disk_state: KernelProjectTransitionDecisionRetentionHotJournalDiskState,
    candidate_count: usize,
) -> KernelProjectTransitionDecisionRetentionHotJournalRecoveryPlan {
    match disk_state {
        KernelProjectTransitionDecisionRetentionHotJournalDiskState::NoEffect => {
            KernelProjectTransitionDecisionRetentionHotJournalRecoveryPlan {
                action: KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction::ClearNoEffectJournal,
                title: "Retention fără efect pe jurnalul activ".to_string(),
                summary: format!(
                    "Jurnalul activ este încă în starea before pentru cei {candidate_count} candidați; retention-ul poate fi replanificat după clear."
                ),
                required_checks: vec![
                    "Verifică event-ul failed care a lăsat hot journal-ul activ.".to_string(),
                    "Confirmă că jurnalul activ nu a fost rescris extern între timp.".to_string(),
                    "Curăță hot journal-ul doar prin recovery auditat.".to_string(),
                ],
                can_clear_journal: true,
                can_restore_before_journal: false,
            }
        }
        KernelProjectTransitionDecisionRetentionHotJournalDiskState::CompletedRetention => {
            KernelProjectTransitionDecisionRetentionHotJournalRecoveryPlan {
                action: KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction::ClearCompletedJournal,
                title: "Retention complet, hot journal necurățat".to_string(),
                summary: format!(
                    "Jurnalul activ este în starea after, iar arhiva corespunde pentru cei {candidate_count} candidați."
                ),
                required_checks: vec![
                    "Confirmă că arhiva există și corespunde hash-ului din hot journal.".to_string(),
                    "Verifică Observability pentru recovery_attention sau failed asociat retention-ului.".to_string(),
                    "Curăță hot journal-ul ca marker terminal auditat.".to_string(),
                ],
                can_clear_journal: true,
                can_restore_before_journal: false,
            }
        }
        KernelProjectTransitionDecisionRetentionHotJournalDiskState::PartialRetention => {
            KernelProjectTransitionDecisionRetentionHotJournalRecoveryPlan {
                action: KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction::RestoreBeforeJournal,
                title: "Retention parțial, restaurare posibilă".to_string(),
                summary: format!(
                    "Jurnalul activ este în starea after, dar arhiva lipsește pentru cei {candidate_count} candidați."
                ),
                required_checks: vec![
                    "Confirmă că hot journal-ul are beforeJournalHash și beforeJournalText valide.".to_string(),
                    "Restaurează jurnalul activ la before prin WriteAuthority, apoi replănuiește retention-ul.".to_string(),
                    "Nu șterge hot journal-ul înainte ca restaurarea să fie comisă.".to_string(),
                ],
                can_clear_journal: false,
                can_restore_before_journal: true,
            }
        }
        KernelProjectTransitionDecisionRetentionHotJournalDiskState::ConflictState => {
            KernelProjectTransitionDecisionRetentionHotJournalRecoveryPlan {
                action: KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction::ManualReviewConflict,
                title: "Conflict în hot journal-ul de retention".to_string(),
                summary: format!(
                    "Jurnalul activ nu corespunde nici stării before, nici stării after sigure pentru cei {candidate_count} candidați."
                ),
                required_checks: vec![
                    "Inspectează manual jurnalul activ, arhiva și Observability înainte de orice mutație.".to_string(),
                    "Nu aplica restaurare automată peste o stare neclasificată.".to_string(),
                    "Reia scanarea recovery după clarificarea stării de disk.".to_string(),
                ],
                can_clear_journal: false,
                can_restore_before_journal: false,
            }
        }
    }
}
