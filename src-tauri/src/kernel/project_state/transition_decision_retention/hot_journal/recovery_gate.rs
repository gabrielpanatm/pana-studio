use crate::kernel::project_session::ProjectSessionSnapshot;

use super::super::model::{
    KernelProjectTransitionDecisionRetentionHotJournal,
    KernelProjectTransitionDecisionRetentionHotJournalDiskState,
    KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
};
use super::scan::scan_project_transition_decision_retention_hot_journals;

pub(crate) fn find_fresh_retention_hot_journal(
    session: &ProjectSessionSnapshot,
    retention_id: &str,
) -> Result<KernelProjectTransitionDecisionRetentionHotJournal, String> {
    scan_project_transition_decision_retention_hot_journals(session)?
        .into_iter()
        .find(|journal| journal.retention_id == retention_id)
        .ok_or_else(|| {
            format!(
                "ProjectTransition Decision retention recovery blocat: hot journal-ul {retention_id} nu mai există în scanarea fresh."
            )
        })
}

pub(crate) fn validate_requested_recovery_action(
    journal: &KernelProjectTransitionDecisionRetentionHotJournal,
    action: KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
) -> Result<(), String> {
    if journal.recovery_plan.action != action {
        return Err(format!(
            "ProjectTransition Decision retention recovery blocat pentru {}: UI a cerut {:?}, dar scanarea fresh recomandă {:?}.",
            journal.retention_id, action, journal.recovery_plan.action
        ));
    }

    match action {
        KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction::ClearNoEffectJournal => {
            if journal.disk_state
                != KernelProjectTransitionDecisionRetentionHotJournalDiskState::NoEffect
                || !journal.recovery_plan.can_clear_journal
            {
                return Err(format!(
                    "ProjectTransition Decision retention recovery blocat pentru {}: clear no-effect cere jurnal activ în starea before.",
                    journal.retention_id
                ));
            }
            Ok(())
        }
        KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction::ClearCompletedJournal => {
            if journal.disk_state
                != KernelProjectTransitionDecisionRetentionHotJournalDiskState::CompletedRetention
                || !journal.recovery_plan.can_clear_journal
            {
                return Err(format!(
                    "ProjectTransition Decision retention recovery blocat pentru {}: clear completed cere jurnal activ after și arhivă verificată.",
                    journal.retention_id
                ));
            }
            Ok(())
        }
        KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction::RestoreBeforeJournal => {
            if journal.disk_state
                != KernelProjectTransitionDecisionRetentionHotJournalDiskState::PartialRetention
                || !journal.recovery_plan.can_restore_before_journal
            {
                return Err(format!(
                    "ProjectTransition Decision retention recovery blocat pentru {}: restore before este permis doar pentru partial_retention fresh.",
                    journal.retention_id
                ));
            }
            Ok(())
        }
        KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction::ManualReviewConflict => {
            Err(format!(
                "ProjectTransition Decision retention recovery blocat pentru {}: conflictul cere review manual înainte de clear sau restore.",
                journal.retention_id
            ))
        }
    }
}
