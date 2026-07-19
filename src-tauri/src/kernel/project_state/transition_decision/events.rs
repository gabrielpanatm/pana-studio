use tauri::{AppHandle, Runtime};

use crate::kernel::observability::{append_event, KernelEventKind, KernelLogEvent, KernelLogLevel};

use super::{transition_decision_kind_code, KernelProjectTransitionDecisionRecord};

pub(super) fn append_project_transition_decision_recorded_event<R: Runtime>(
    app: &AppHandle<R>,
    record: &KernelProjectTransitionDecisionRecord,
) -> Result<(), String> {
    append_event(
        app,
        KernelLogEvent::new(
            KernelLogLevel::Warn,
            KernelEventKind::ProjectTransitionDecisionRecorded,
            "project_state",
            "project_lifecycle",
            "operator_decision",
            Some(record.evidence.target_project_root.clone()),
            "Project transition operator decision was recorded.",
            Some(record.diagnostic.clone()),
        )
        .with_attribute("decisionId", record.id.clone())
        .with_attribute(
            "decisionKind",
            transition_decision_kind_code(record.decision_kind),
        )
        .with_attribute("action", record.evidence.action)
        .with_attribute("transitionReason", record.evidence.transition_reason)
        .with_attribute("sessionId", record.evidence.session_id.clone())
        .with_attribute("projectRoot", record.evidence.project_root.clone())
        .with_attribute(
            "targetProjectRoot",
            record.evidence.target_project_root.clone(),
        )
        .with_attribute("evidenceHash", record.evidence_hash.clone())
        .with_attribute(
            "workspaceDirtyResourceCount",
            record.evidence.workspace_dirty_resource_count,
        )
        .with_attribute("workspaceRevision", record.evidence.workspace.revision),
    )
}
