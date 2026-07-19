use super::super::transition_decision::KernelProjectTransitionDecisionRecord;

pub(super) fn latest_record<'a>(
    records: &[&'a KernelProjectTransitionDecisionRecord],
) -> Option<&'a KernelProjectTransitionDecisionRecord> {
    records.iter().copied().max_by(|left, right| {
        left.decided_at_ms
            .cmp(&right.decided_at_ms)
            .then_with(|| left.id.cmp(&right.id))
    })
}
