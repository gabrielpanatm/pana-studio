use super::super::KernelProjectTransitionReason;
use super::KernelProjectTransitionDecisionKind;

pub fn transition_decision_kind_code(kind: KernelProjectTransitionDecisionKind) -> &'static str {
    super::super::transition_decision_labels::decision_kind_code(kind)
}

pub(super) fn decision_kind_for_transition_reason(
    reason: KernelProjectTransitionReason,
) -> Option<KernelProjectTransitionDecisionKind> {
    match reason {
        KernelProjectTransitionReason::WorkspaceDirty => {
            Some(KernelProjectTransitionDecisionKind::DiscardLocalDraftsForTransition)
        }
        KernelProjectTransitionReason::DiskConflict => {
            Some(KernelProjectTransitionDecisionKind::DiscardSessionForExternalReload)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_conflict_confirmation_has_dedicated_reload_discard_kind() {
        assert_eq!(
            decision_kind_for_transition_reason(KernelProjectTransitionReason::DiskConflict),
            Some(KernelProjectTransitionDecisionKind::DiscardSessionForExternalReload)
        );
    }
}
