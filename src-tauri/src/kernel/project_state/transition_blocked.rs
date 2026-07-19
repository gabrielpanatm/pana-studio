use serde::Serialize;
use tauri::{AppHandle, Runtime};

use crate::kernel::observability::{
    kernel_event_name, read_kernel_observability_log_snapshot, KernelEventKind, KernelLogLevel,
    KernelObservabilityLogRequest, KernelObservabilityLogSnapshot,
    KernelObservabilityLogSourceFilter,
};

mod records;
mod summaries;

use super::lifecycle_policy::{
    KernelProjectTransitionAction, KernelProjectTransitionDecision, KernelProjectTransitionReason,
};
use super::model::{KernelProjectStateReason, KernelProjectStateStatus};

use records::blocked_record_from_event;
use summaries::{
    summarize_blocked_causes, summarize_blocked_health, summarize_latest_blocked_by_action,
};

pub const KERNEL_PROJECT_TRANSITION_BLOCKED_AUDIT_SCHEMA_VERSION: u32 = 6;

const DEFAULT_BLOCKED_AUDIT_LIMIT: usize = 40;
const MAX_BLOCKED_AUDIT_LIMIT: usize = 120;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionBlockedAuditSnapshot {
    pub schema_version: u32,
    pub log_path: String,
    pub log_exists: bool,
    pub truncated: bool,
    pub scanned_line_count: usize,
    pub unreadable_count: usize,
    pub matching_event_count: usize,
    pub returned_count: usize,
    pub include_archives: bool,
    pub source_filter: KernelObservabilityLogSourceFilter,
    pub health: KernelProjectTransitionBlockedHealthSnapshot,
    pub latest_by_action: Vec<KernelProjectTransitionBlockedActionSummary>,
    pub causes: Vec<KernelProjectTransitionBlockedCauseSummary>,
    pub records: Vec<KernelProjectTransitionBlockedRecord>,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelProjectTransitionBlockedCause {
    DiskConflict,
    WorkspaceDirty,
    BlockedProjectState,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelProjectTransitionResolutionSurface {
    DiskConflict,
    ProjectWorkspace,
    Overview,
    Observability,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelProjectTransitionBlockedHealthStatus {
    Clean,
    RecentlyBlocked,
    RepeatedlyBlocked,
    Degraded,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionBlockedHealthSnapshot {
    pub schema_version: u32,
    pub status: KernelProjectTransitionBlockedHealthStatus,
    pub record_count: usize,
    pub action_count: usize,
    pub repeated_action_count: usize,
    pub cause_count: usize,
    pub repeated_cause_count: usize,
    pub latest_record_id: Option<String>,
    pub latest_action: Option<KernelProjectTransitionAction>,
    pub latest_blocked_at_ms: Option<u128>,
    pub summary: String,
    pub detail: String,
    pub recommended_action: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionBlockedCauseSummary {
    pub schema_version: u32,
    pub cause: KernelProjectTransitionBlockedCause,
    pub surface: KernelProjectTransitionResolutionSurface,
    pub count: usize,
    pub latest_blocked_at_ms: u128,
    pub latest_record_id: Option<String>,
    pub record_ids: Vec<String>,
    pub title: String,
    pub detail: String,
    pub recommended_action: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionBlockedActionSummary {
    pub schema_version: u32,
    pub action: KernelProjectTransitionAction,
    pub count: usize,
    pub latest_record_id: String,
    pub latest_blocked_at_ms: u128,
    pub cause: KernelProjectTransitionBlockedCause,
    pub surface: KernelProjectTransitionResolutionSurface,
    pub decision: Option<KernelProjectTransitionDecision>,
    pub reason: Option<KernelProjectTransitionReason>,
    pub project_state_status: Option<KernelProjectStateStatus>,
    pub project_state_reason: Option<KernelProjectStateReason>,
    pub current_project_root: Option<String>,
    pub target_project_root: Option<String>,
    pub session_id: Option<String>,
    pub title: String,
    pub detail: String,
    pub recommended_action: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionBlockedRecord {
    pub schema_version: u32,
    pub id: String,
    pub blocked_at_ms: u128,
    pub source_label: String,
    pub action: Option<KernelProjectTransitionAction>,
    pub decision: Option<KernelProjectTransitionDecision>,
    pub reason: Option<KernelProjectTransitionReason>,
    pub project_state_status: Option<KernelProjectStateStatus>,
    pub project_state_reason: Option<KernelProjectStateReason>,
    pub current_project_root: Option<String>,
    pub target_project_root: Option<String>,
    pub session_id: Option<String>,
    pub operation: String,
    pub target: Option<String>,
    pub message: String,
    pub diagnostic: Option<String>,
    pub workspace_dirty_resource_count: usize,
    pub workspace_revision: Option<u64>,
    pub workspace_undo_count: usize,
    pub workspace_redo_count: usize,
    pub disk_conflict_count: usize,
    pub disk_blocking_count: usize,
}

pub fn read_kernel_project_transition_blocked_audit_snapshot<R: Runtime>(
    app: &AppHandle<R>,
    limit: Option<usize>,
    include_archives: Option<bool>,
) -> Result<KernelProjectTransitionBlockedAuditSnapshot, String> {
    let limit = limit
        .unwrap_or(DEFAULT_BLOCKED_AUDIT_LIMIT)
        .clamp(1, MAX_BLOCKED_AUDIT_LIMIT);
    let log_snapshot = read_kernel_observability_log_snapshot(
        app,
        KernelObservabilityLogRequest {
            limit: Some(limit),
            recovery_only: Some(false),
            include_archives,
            levels: Some(vec![KernelLogLevel::Warn, KernelLogLevel::Error]),
            source_filter: Some(KernelObservabilityLogSourceFilter::All),
            event_names: Some(vec![kernel_event_name(
                KernelEventKind::ProjectTransitionBlocked,
            )
            .to_string()]),
        },
    )?;

    Ok(build_kernel_project_transition_blocked_audit_snapshot(
        log_snapshot,
    ))
}

pub fn build_kernel_project_transition_blocked_audit_snapshot(
    log_snapshot: KernelObservabilityLogSnapshot,
) -> KernelProjectTransitionBlockedAuditSnapshot {
    let mut diagnostics = log_snapshot.diagnostics.clone();
    let records = log_snapshot
        .events
        .iter()
        .filter(|event| {
            event.kind == KernelEventKind::ProjectTransitionBlocked
                || event.event_name == kernel_event_name(KernelEventKind::ProjectTransitionBlocked)
        })
        .map(|event| {
            blocked_record_from_event(&event.event, event.source.label.clone(), &mut diagnostics)
        })
        .collect::<Vec<_>>();

    let latest_by_action = summarize_latest_blocked_by_action(&records);
    let causes = summarize_blocked_causes(&records);
    let health = summarize_blocked_health(&records, &latest_by_action, &causes, &diagnostics);

    KernelProjectTransitionBlockedAuditSnapshot {
        schema_version: KERNEL_PROJECT_TRANSITION_BLOCKED_AUDIT_SCHEMA_VERSION,
        log_path: log_snapshot.log_path,
        log_exists: log_snapshot.log_exists,
        truncated: log_snapshot.truncated,
        scanned_line_count: log_snapshot.scanned_line_count,
        unreadable_count: log_snapshot.unreadable_count,
        matching_event_count: log_snapshot.health.event_count,
        returned_count: records.len(),
        include_archives: log_snapshot.include_archives,
        source_filter: log_snapshot.source_filter,
        health,
        latest_by_action,
        causes,
        records,
        diagnostics,
    }
}
