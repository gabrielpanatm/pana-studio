use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::kernel::observability::KernelLogEvent;

use super::{
    KernelProjectTransitionBlockedRecord, KERNEL_PROJECT_TRANSITION_BLOCKED_AUDIT_SCHEMA_VERSION,
};

pub(super) fn blocked_record_from_event(
    event: &KernelLogEvent,
    source_label: String,
    diagnostics: &mut Vec<String>,
) -> KernelProjectTransitionBlockedRecord {
    KernelProjectTransitionBlockedRecord {
        schema_version: KERNEL_PROJECT_TRANSITION_BLOCKED_AUDIT_SCHEMA_VERSION,
        id: event.id.clone(),
        blocked_at_ms: event.timestamp_ms,
        source_label,
        action: attr_enum(event, "action", diagnostics),
        decision: attr_enum(event, "decision", diagnostics),
        reason: attr_enum(event, "reason", diagnostics),
        project_state_status: attr_enum(event, "projectStateStatus", diagnostics),
        project_state_reason: attr_enum(event, "projectStateReason", diagnostics),
        current_project_root: attr_string(event, "currentProjectRoot", diagnostics),
        target_project_root: attr_string(event, "targetProjectRoot", diagnostics)
            .or_else(|| event.target.clone()),
        session_id: attr_string(event, "sessionId", diagnostics),
        operation: event.operation.clone(),
        target: event.target.clone(),
        message: event.message.clone(),
        diagnostic: event.diagnostic.clone(),
        workspace_dirty_resource_count: attr_usize(
            event,
            "workspaceDirtyResourceCount",
            diagnostics,
        ),
        workspace_revision: attr_u64(event, "workspaceRevision", diagnostics),
        workspace_undo_count: attr_usize(event, "workspaceUndoCount", diagnostics),
        workspace_redo_count: attr_usize(event, "workspaceRedoCount", diagnostics),
        disk_conflict_count: attr_usize(event, "diskConflictCount", diagnostics),
        disk_blocking_count: attr_usize(event, "diskBlockingCount", diagnostics),
    }
}

fn attr_enum<T: DeserializeOwned>(
    event: &KernelLogEvent,
    key: &str,
    diagnostics: &mut Vec<String>,
) -> Option<T> {
    let Some(value) = event.attributes.get(key) else {
        return None;
    };
    if value.is_null() {
        return None;
    }
    match serde_json::from_value::<T>(value.clone()) {
        Ok(parsed) => Some(parsed),
        Err(error) => {
            diagnostics.push(format!(
                "{}: atributul {} nu poate fi interpretat: {}.",
                event.id, key, error
            ));
            None
        }
    }
}

fn attr_string(event: &KernelLogEvent, key: &str, diagnostics: &mut Vec<String>) -> Option<String> {
    match event.attributes.get(key) {
        Some(Value::String(value)) => Some(value.clone()),
        Some(Value::Null) | None => None,
        Some(value) => {
            diagnostics.push(format!(
                "{}: atributul {} trebuia să fie string/null, dar este {}.",
                event.id, key, value
            ));
            None
        }
    }
}

fn attr_usize(event: &KernelLogEvent, key: &str, diagnostics: &mut Vec<String>) -> usize {
    match event.attributes.get(key) {
        Some(Value::Number(value)) => value.as_u64().unwrap_or(0) as usize,
        Some(Value::Null) | None => 0,
        Some(value) => {
            diagnostics.push(format!(
                "{}: atributul {} trebuia să fie număr/null, dar este {}.",
                event.id, key, value
            ));
            0
        }
    }
}

fn attr_u64(event: &KernelLogEvent, key: &str, diagnostics: &mut Vec<String>) -> Option<u64> {
    match event.attributes.get(key) {
        Some(Value::Number(value)) => value.as_u64(),
        Some(Value::Null) | None => None,
        Some(value) => {
            diagnostics.push(format!(
                "{}: atributul {} trebuia să fie număr/null, dar este {}.",
                event.id, key, value
            ));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::kernel::{
        observability::{KernelEventKind, KernelLogLevel},
        project_state::{
            lifecycle_policy::{
                KernelProjectTransitionAction, KernelProjectTransitionDecision,
                KernelProjectTransitionReason,
            },
            model::{KernelProjectStateReason, KernelProjectStateStatus},
        },
    };

    #[test]
    fn blocked_record_extracts_policy_evidence_from_observability_attributes() {
        let event = KernelLogEvent::new(
            KernelLogLevel::Warn,
            KernelEventKind::ProjectTransitionBlocked,
            "project_state",
            "project_lifecycle",
            "open_project",
            Some("/tmp/target".to_string()),
            "blocked",
            Some("guard".to_string()),
        )
        .with_attribute("action", KernelProjectTransitionAction::OpenProject)
        .with_attribute("decision", KernelProjectTransitionDecision::Block)
        .with_attribute("reason", KernelProjectTransitionReason::DiskConflict)
        .with_attribute("projectStateStatus", KernelProjectStateStatus::Warning)
        .with_attribute("projectStateReason", KernelProjectStateReason::DiskConflict)
        .with_attribute("currentProjectRoot", "/tmp/current")
        .with_attribute("targetProjectRoot", "/tmp/target")
        .with_attribute("sessionId", "session-1")
        .with_attribute("workspaceDirtyResourceCount", 2_usize)
        .with_attribute("workspaceRevision", 7_u64)
        .with_attribute("workspaceUndoCount", 4_usize)
        .with_attribute("workspaceRedoCount", 1_usize)
        .with_attribute("diskConflictCount", 3_usize)
        .with_attribute("diskBlockingCount", 1_usize);
        let mut diagnostics = Vec::new();

        let record =
            blocked_record_from_event(&event, "kernel.jsonl".to_string(), &mut diagnostics);

        assert!(diagnostics.is_empty());
        assert_eq!(
            record.action,
            Some(KernelProjectTransitionAction::OpenProject)
        );
        assert_eq!(
            record.decision,
            Some(KernelProjectTransitionDecision::Block)
        );
        assert_eq!(
            record.reason,
            Some(KernelProjectTransitionReason::DiskConflict)
        );
        assert_eq!(
            record.project_state_status,
            Some(KernelProjectStateStatus::Warning)
        );
        assert_eq!(record.current_project_root.as_deref(), Some("/tmp/current"));
        assert_eq!(record.target_project_root.as_deref(), Some("/tmp/target"));
        assert_eq!(record.session_id.as_deref(), Some("session-1"));
        assert_eq!(record.workspace_dirty_resource_count, 2);
        assert_eq!(record.workspace_revision, Some(7));
        assert_eq!(record.workspace_undo_count, 4);
        assert_eq!(record.workspace_redo_count, 1);
        assert_eq!(record.disk_conflict_count, 3);
        assert_eq!(record.disk_blocking_count, 1);
    }
}
