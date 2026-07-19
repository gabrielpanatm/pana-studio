use tauri::{AppHandle, Runtime};

use crate::kernel::observability::{append_event, KernelEventKind, KernelLogEvent, KernelLogLevel};

use super::{recovery_ack_kind_code, KernelProjectTransitionDecisionRecoveryAckRecord};

pub(super) fn append_project_transition_decision_recovery_acknowledged_event<R: Runtime>(
    app: &AppHandle<R>,
    record: &KernelProjectTransitionDecisionRecoveryAckRecord,
) -> Result<(), String> {
    append_event(
        app,
        KernelLogEvent::new(
            KernelLogLevel::Warn,
            KernelEventKind::ProjectTransitionDecisionRecoveryAcknowledged,
            "project_state",
            "project_lifecycle",
            "recovery_plan_acknowledge",
            Some(record.evidence.decision_journal_path.clone()),
            "Project transition decision recovery plan acknowledgement was recorded.",
            Some(record.diagnostic.clone()),
        )
        .with_attribute("acknowledgementId", record.id.clone())
        .with_attribute("ackKind", recovery_ack_kind_code(record.ack_kind))
        .with_attribute(
            "recoveryPlanEvidenceHash",
            record.evidence.recovery_plan_evidence_hash.clone(),
        )
        .with_attribute("evidenceHash", record.evidence_hash.clone())
        .with_attribute("sessionId", record.evidence.session_id.clone())
        .with_attribute("projectRoot", record.evidence.project_root.clone())
        .with_attribute("recoveryPlanStatus", record.evidence.recovery_plan_status)
        .with_attribute("integrityTrusted", record.evidence.integrity_trusted)
        .with_attribute("recordCount", record.evidence.record_count)
        .with_attribute("issueCount", record.evidence.issue_count)
        .with_attribute(
            "retentionCandidateCount",
            record.evidence.retention_candidate_count,
        ),
    )
}
