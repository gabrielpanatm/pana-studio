use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime};

use crate::{
    app_home::app_home_snapshot,
    kernel::write_authority::{
        capability_append_observability_file, capability_lock_observability_file,
        CapabilityMaintenanceLockMode, WriteAuthorityRuntime,
    },
};

mod health;
mod reader;
mod retention;

const OBSERVABILITY_SCHEMA_VERSION: u32 = 3;
const KERNEL_LOG_FILE: &str = "kernel.jsonl";
pub(crate) const KERNEL_LOG_LOCK_FILE: &str = ".kernel-log.lock";
static EVENT_COUNTER: AtomicU64 = AtomicU64::new(1);
static OBSERVABILITY_WRITE_LOCK: Mutex<()> = Mutex::new(());

pub use health::{
    KernelObservabilityHealthProblemSnapshot, KernelObservabilityHealthSnapshot,
    KernelObservabilityHealthStatus, KernelObservabilityLevelCounts,
    KernelObservabilityModuleHealthSnapshot, KernelObservabilitySourceCounts,
};
pub use reader::{
    read_kernel_observability_log_snapshot, KernelObservabilityLogEventSnapshot,
    KernelObservabilityLogRequest, KernelObservabilityLogSnapshot,
    KernelObservabilityLogSourceFilter,
};
pub use retention::{KernelLogArchiveSnapshot, KernelLogRetentionSnapshot};

use retention::{
    default_kernel_log_retention_policy, rotate_kernel_log_if_needed, KernelLogRetentionPolicy,
    KernelLogRotationReceipt,
};

pub type KernelLogAttributes = BTreeMap<String, Value>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelLogLevel {
    Info,
    Warn,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelEventKind {
    Boot,
    CommandStarted,
    CommandCommitted,
    CommandFailed,
    FileBufferStoreLoaded,
    ExternalDiskReconcilePlanned,
    ExternalDiskReconcileApplied,
    ExternalDiskReconcileBlocked,
    ExternalDiskReconcileFailed,
    ProjectTransitionBlocked,
    ProjectTransitionDecisionRecorded,
    ProjectTransitionDecisionRecoveryAcknowledged,
    ProjectTransitionDecisionRetentionPlanned,
    ProjectTransitionDecisionRetentionCommitted,
    ProjectTransitionDecisionRetentionFailed,
    ProjectTransitionDecisionRetentionRecoveryAttention,
    ProjectTransitionDecisionRetentionRecovered,
    NativeWindowCloseRequested,
    SessionOpened,
    SessionClosed,
    RecoveryCoordinatorClean,
    RecoveryCoordinatorNeedsAttention,
    RecoveryCoordinatorFailed,
    UndoApplied,
    UndoFailed,
    RedoApplied,
    RedoFailed,
    ObservabilityLogRotated,
    PreviewProjectionIntentAccepted,
    PreviewProjectionIntentBlocked,
    PreviewProjectionLayerDropCommitted,
    PreviewProjectionLayerDropBlocked,
    PreviewProjectionHtmlInsertDropCommitted,
    PreviewProjectionHtmlInsertDropBlocked,
    PreviewProjectionHtmlAttributesCommitted,
    PreviewProjectionHtmlAttributesBlocked,
    PreviewProjectionHtmlTextCommitted,
    PreviewProjectionHtmlTextBlocked,
    PreviewProjectionHtmlTagCommitted,
    PreviewProjectionHtmlTagBlocked,
    PreviewProjectionHtmlDuplicateCommitted,
    PreviewProjectionHtmlDuplicateBlocked,
    PreviewProjectionHtmlDeleteCommitted,
    PreviewProjectionHtmlDeleteBlocked,
    PreviewProjectionTeraInsertDropCommitted,
    PreviewProjectionTeraInsertDropBlocked,
    PreviewProjectionTeraMoveDropCommitted,
    PreviewProjectionTeraMoveDropBlocked,
    PreviewProjectionTeraDeleteCommitted,
    PreviewProjectionTeraDeleteBlocked,
    PreviewProjectionTemplateEditGranted,
    PreviewProjectionTemplateEditBlocked,
    PreviewCanvasPrepared,
    PreviewCanvasPhaseAcknowledged,
    PreviewCanvasCanonicalVerified,
    PreviewCanvasFailed,
    PreviewCanvasStaleDiscarded,
    PreviewCanvasPatchRolledBack,
    PreviewCanvasFallback,
    PreviewCanvasCacheHit,
    PreviewCanvasCacheMiss,
    PreviewCanvasFoucGuardSatisfied,
    PreviewInteractiveJsRestarted,
    PreviewInteractiveJsFailed,
    SourceBrowserBuildStarted,
    SourceBrowserPublished,
    SourceBrowserFailed,
    SourceBrowserStaleDiscarded,
    VersioningMutationCommitted,
    VersioningMutationFailed,
    VersioningPreviewStarted,
    VersioningPreviewStopped,
    VersioningRestorePublished,
    VersioningRestoreRecoveryRequired,
    VersioningRestoreRecoveryResolved,
    VersioningRemoteCompleted,
    VersioningRemoteFailed,
    VersioningRemoteCancelled,
    VersioningIntegrationPublished,
    VersioningIntegrationConflict,
    VersioningIntegrationRecoveryRequired,
    VersioningIntegrationRecoveryResolved,
    WritePlanned,
    WriteCommitted,
    WriteRecoveryRequired,
    WriteFailed,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelLogEvent {
    pub schema_version: u32,
    pub id: String,
    pub timestamp_ms: u128,
    pub observed_timestamp_ms: u128,
    pub level: KernelLogLevel,
    pub severity_text: String,
    pub severity_number: u8,
    pub kind: KernelEventKind,
    pub event_name: String,
    pub owner: String,
    pub category: String,
    pub operation: String,
    pub target: Option<String>,
    pub message: String,
    pub diagnostic: Option<String>,
    #[serde(default)]
    pub attributes: KernelLogAttributes,
}

pub fn record_boot<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    append_event(
        app,
        KernelLogEvent::new(
            KernelLogLevel::Info,
            KernelEventKind::Boot,
            "kernel",
            "internal_app_write",
            "boot",
            None,
            "Kernel boot initialized.",
            None,
        ),
    )
}

pub fn append_event<R: Runtime>(app: &AppHandle<R>, event: KernelLogEvent) -> Result<(), String> {
    let path = kernel_log_path(app)?;
    let runtime = app
        .try_state::<WriteAuthorityRuntime>()
        .ok_or_else(|| "Observability nu are WriteAuthorityRuntime instalat.".to_string())?;
    append_event_to_log_path(
        Some(runtime.inner()),
        &path,
        event,
        &default_kernel_log_retention_policy(),
    )
}

fn append_event_to_log_path(
    runtime: Option<&WriteAuthorityRuntime>,
    path: &Path,
    event: KernelLogEvent,
    policy: &KernelLogRetentionPolicy,
) -> Result<(), String> {
    // Rotation and append form one local transaction. Recovering a poisoned
    // lock is preferable to permanently disabling the diagnostic sink after
    // an unrelated panic; all filesystem effects below remain fail-closed.
    let _write_guard = OBSERVABILITY_WRITE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let parent = path
        .parent()
        .ok_or_else(|| "Nu am putut determina boundary-ul logului de kernel.".to_string())?;
    let lock_path = parent.join(KERNEL_LOG_LOCK_FILE);
    let _stable_lock = capability_lock_observability_file(
        runtime,
        &lock_path,
        parent,
        "observability/kernel-log-stable-lock",
        CapabilityMaintenanceLockMode::Exclusive,
    )
    .map_err(|error| error.into_terminal_diagnostic())?;
    let body = serde_json::to_string(&event)
        .map_err(|error| format!("Nu am putut serializa evenimentul kernel: {}", error))?;
    let incoming_bytes = body.len() as u64 + 1;
    if let Some(receipt) = rotate_kernel_log_if_needed(runtime, path, incoming_bytes, policy)? {
        let rotation_event = rotation_event(&receipt);
        append_serialized_event(runtime, path, rotation_event)?;
    }

    append_serialized_body(runtime, path, &body)
}

fn append_serialized_event(
    runtime: Option<&WriteAuthorityRuntime>,
    path: &Path,
    event: KernelLogEvent,
) -> Result<(), String> {
    let body = serde_json::to_string(&event)
        .map_err(|error| format!("Nu am putut serializa evenimentul kernel: {}", error))?;
    append_serialized_body(runtime, path, &body)
}

fn append_serialized_body(
    runtime: Option<&WriteAuthorityRuntime>,
    path: &Path,
    body: &str,
) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Nu am putut determina boundary-ul logului de kernel.".to_string())?;
    let mut record = Vec::with_capacity(body.len().saturating_add(1));
    record.extend_from_slice(body.as_bytes());
    record.push(b'\n');
    capability_append_observability_file(runtime, path, parent, "observability/kernel-log", &record)
        .map_err(|error| error.into_terminal_diagnostic())
        .map(|_| ())
}

fn rotation_event(receipt: &KernelLogRotationReceipt) -> KernelLogEvent {
    KernelLogEvent::new(
        KernelLogLevel::Info,
        KernelEventKind::ObservabilityLogRotated,
        "observability",
        "internal_app_write",
        "rotate_kernel_log",
        Some(receipt.archived_path.to_string_lossy().to_string()),
        "Observability Log rotated by retention policy.",
        None,
    )
    .with_attribute(
        "archivedPath",
        receipt.archived_path.to_string_lossy().to_string(),
    )
    .with_attribute("activeBytesBefore", receipt.active_bytes_before)
    .with_attribute("incomingBytes", receipt.incoming_bytes)
    .with_attribute("maxActiveBytes", receipt.max_active_bytes)
    .with_attribute("archiveCount", receipt.archive_count)
    .with_attribute("removedOldestArchive", receipt.removed_oldest_archive)
    .with_attribute("shiftedArchives", receipt.shifted_archives)
}

impl KernelLogEvent {
    pub fn new(
        level: KernelLogLevel,
        kind: KernelEventKind,
        owner: impl Into<String>,
        category: impl Into<String>,
        operation: impl Into<String>,
        target: Option<String>,
        message: impl Into<String>,
        diagnostic: Option<String>,
    ) -> Self {
        let timestamp_ms = now_ms();
        let sequence = EVENT_COUNTER.fetch_add(1, Ordering::Relaxed);
        Self {
            schema_version: OBSERVABILITY_SCHEMA_VERSION,
            id: format!("kernel-{timestamp_ms}-{}-{sequence}", std::process::id()),
            timestamp_ms,
            observed_timestamp_ms: timestamp_ms,
            level,
            severity_text: severity_text(level).to_string(),
            severity_number: severity_number(level),
            kind,
            event_name: event_name(kind).to_string(),
            owner: owner.into(),
            category: category.into(),
            operation: operation.into(),
            target,
            message: message.into(),
            diagnostic,
            attributes: BTreeMap::new(),
        }
    }

    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        self.attributes.insert(key.into(), attribute_value(value));
        self
    }

    pub fn with_attributes(mut self, attributes: KernelLogAttributes) -> Self {
        self.attributes.extend(attributes);
        self
    }
}

pub fn kernel_event_name(kind: KernelEventKind) -> &'static str {
    event_name(kind)
}

fn attribute_value(value: impl Serialize) -> Value {
    serde_json::to_value(value)
        .unwrap_or_else(|error| Value::String(format!("attribute serialization failed: {error}")))
}

fn severity_text(level: KernelLogLevel) -> &'static str {
    match level {
        KernelLogLevel::Info => "INFO",
        KernelLogLevel::Warn => "WARN",
        KernelLogLevel::Error => "ERROR",
    }
}

fn severity_number(level: KernelLogLevel) -> u8 {
    match level {
        KernelLogLevel::Info => 9,
        KernelLogLevel::Warn => 13,
        KernelLogLevel::Error => 17,
    }
}

fn event_name(kind: KernelEventKind) -> &'static str {
    match kind {
        KernelEventKind::Boot => "kernel.boot",
        KernelEventKind::CommandStarted => "kernel.command.started",
        KernelEventKind::CommandCommitted => "kernel.command.committed",
        KernelEventKind::CommandFailed => "kernel.command.failed",
        KernelEventKind::FileBufferStoreLoaded => "kernel.file_buffer_store.loaded",
        KernelEventKind::ExternalDiskReconcilePlanned => {
            "kernel.file_buffer_store.external_reconcile.planned"
        }
        KernelEventKind::ExternalDiskReconcileApplied => {
            "kernel.file_buffer_store.external_reconcile.applied"
        }
        KernelEventKind::ExternalDiskReconcileBlocked => {
            "kernel.file_buffer_store.external_reconcile.blocked"
        }
        KernelEventKind::ExternalDiskReconcileFailed => {
            "kernel.file_buffer_store.external_reconcile.failed"
        }
        KernelEventKind::ProjectTransitionBlocked => "kernel.project_transition.blocked",
        KernelEventKind::ProjectTransitionDecisionRecorded => {
            "kernel.project_transition.decision_recorded"
        }
        KernelEventKind::ProjectTransitionDecisionRecoveryAcknowledged => {
            "kernel.project_transition.decision_recovery_acknowledged"
        }
        KernelEventKind::ProjectTransitionDecisionRetentionPlanned => {
            "kernel.project_transition.decision_retention.planned"
        }
        KernelEventKind::ProjectTransitionDecisionRetentionCommitted => {
            "kernel.project_transition.decision_retention.committed"
        }
        KernelEventKind::ProjectTransitionDecisionRetentionFailed => {
            "kernel.project_transition.decision_retention.failed"
        }
        KernelEventKind::ProjectTransitionDecisionRetentionRecoveryAttention => {
            "kernel.project_transition.decision_retention.recovery_attention"
        }
        KernelEventKind::ProjectTransitionDecisionRetentionRecovered => {
            "kernel.project_transition.decision_retention.recovered"
        }
        KernelEventKind::NativeWindowCloseRequested => "kernel.window.close_requested",
        KernelEventKind::SessionOpened => "kernel.session.opened",
        KernelEventKind::SessionClosed => "kernel.session.closed",
        KernelEventKind::RecoveryCoordinatorClean => "kernel.recovery_coordinator.clean",
        KernelEventKind::RecoveryCoordinatorNeedsAttention => {
            "kernel.recovery_coordinator.needs_attention"
        }
        KernelEventKind::RecoveryCoordinatorFailed => "kernel.recovery_coordinator.failed",
        KernelEventKind::UndoApplied => "kernel.undo_redo.undo.applied",
        KernelEventKind::UndoFailed => "kernel.undo_redo.undo.failed",
        KernelEventKind::RedoApplied => "kernel.undo_redo.redo.applied",
        KernelEventKind::RedoFailed => "kernel.undo_redo.redo.failed",
        KernelEventKind::ObservabilityLogRotated => "kernel.observability_log.rotated",
        KernelEventKind::PreviewProjectionIntentAccepted => {
            "kernel.preview_projection.intent.accepted"
        }
        KernelEventKind::PreviewProjectionIntentBlocked => {
            "kernel.preview_projection.intent.blocked"
        }
        KernelEventKind::PreviewProjectionLayerDropCommitted => {
            "kernel.preview_projection.layer_drop.committed"
        }
        KernelEventKind::PreviewProjectionLayerDropBlocked => {
            "kernel.preview_projection.layer_drop.blocked"
        }
        KernelEventKind::PreviewProjectionHtmlInsertDropCommitted => {
            "kernel.preview_projection.html_insert_drop.committed"
        }
        KernelEventKind::PreviewProjectionHtmlInsertDropBlocked => {
            "kernel.preview_projection.html_insert_drop.blocked"
        }
        KernelEventKind::PreviewProjectionHtmlAttributesCommitted => {
            "kernel.preview_projection.html_attributes.committed"
        }
        KernelEventKind::PreviewProjectionHtmlAttributesBlocked => {
            "kernel.preview_projection.html_attributes.blocked"
        }
        KernelEventKind::PreviewProjectionHtmlTextCommitted => {
            "kernel.preview_projection.html_text.committed"
        }
        KernelEventKind::PreviewProjectionHtmlTextBlocked => {
            "kernel.preview_projection.html_text.blocked"
        }
        KernelEventKind::PreviewProjectionHtmlTagCommitted => {
            "kernel.preview_projection.html_tag.committed"
        }
        KernelEventKind::PreviewProjectionHtmlTagBlocked => {
            "kernel.preview_projection.html_tag.blocked"
        }
        KernelEventKind::PreviewProjectionHtmlDuplicateCommitted => {
            "kernel.preview_projection.html_duplicate.committed"
        }
        KernelEventKind::PreviewProjectionHtmlDuplicateBlocked => {
            "kernel.preview_projection.html_duplicate.blocked"
        }
        KernelEventKind::PreviewProjectionHtmlDeleteCommitted => {
            "kernel.preview_projection.html_delete.committed"
        }
        KernelEventKind::PreviewProjectionHtmlDeleteBlocked => {
            "kernel.preview_projection.html_delete.blocked"
        }
        KernelEventKind::PreviewProjectionTeraInsertDropCommitted => {
            "kernel.preview_projection.tera_insert_drop.committed"
        }
        KernelEventKind::PreviewProjectionTeraInsertDropBlocked => {
            "kernel.preview_projection.tera_insert_drop.blocked"
        }
        KernelEventKind::PreviewProjectionTeraMoveDropCommitted => {
            "kernel.preview_projection.tera_move_drop.committed"
        }
        KernelEventKind::PreviewProjectionTeraMoveDropBlocked => {
            "kernel.preview_projection.tera_move_drop.blocked"
        }
        KernelEventKind::PreviewProjectionTeraDeleteCommitted => {
            "kernel.preview_projection.tera_delete.committed"
        }
        KernelEventKind::PreviewProjectionTeraDeleteBlocked => {
            "kernel.preview_projection.tera_delete.blocked"
        }
        KernelEventKind::PreviewProjectionTemplateEditGranted => {
            "kernel.preview_projection.template_edit.granted"
        }
        KernelEventKind::PreviewProjectionTemplateEditBlocked => {
            "kernel.preview_projection.template_edit.blocked"
        }
        KernelEventKind::PreviewCanvasPrepared => "kernel.preview.canvas.prepared",
        KernelEventKind::PreviewCanvasPhaseAcknowledged => {
            "kernel.preview.canvas.phase_acknowledged"
        }
        KernelEventKind::PreviewCanvasCanonicalVerified => {
            "kernel.preview.canvas.canonical_verified"
        }
        KernelEventKind::PreviewCanvasFailed => "kernel.preview.canvas.failed",
        KernelEventKind::PreviewCanvasStaleDiscarded => "kernel.preview.canvas.stale_discarded",
        KernelEventKind::PreviewCanvasPatchRolledBack => "kernel.preview.canvas.patch_rolled_back",
        KernelEventKind::PreviewCanvasFallback => "kernel.preview.canvas.fallback",
        KernelEventKind::PreviewCanvasCacheHit => "kernel.preview.canvas.cache_hit",
        KernelEventKind::PreviewCanvasCacheMiss => "kernel.preview.canvas.cache_miss",
        KernelEventKind::PreviewCanvasFoucGuardSatisfied => {
            "kernel.preview.canvas.fouc_guard_satisfied"
        }
        KernelEventKind::PreviewInteractiveJsRestarted => "kernel.preview.interactive_js.restarted",
        KernelEventKind::PreviewInteractiveJsFailed => "kernel.preview.interactive_js.failed",
        KernelEventKind::SourceBrowserBuildStarted => "kernel.source_browser.build_started",
        KernelEventKind::SourceBrowserPublished => "kernel.source_browser.published",
        KernelEventKind::SourceBrowserFailed => "kernel.source_browser.failed",
        KernelEventKind::SourceBrowserStaleDiscarded => "kernel.source_browser.stale_discarded",
        KernelEventKind::VersioningMutationCommitted => "kernel.versioning.mutation.committed",
        KernelEventKind::VersioningMutationFailed => "kernel.versioning.mutation.failed",
        KernelEventKind::VersioningPreviewStarted => "kernel.versioning.preview.started",
        KernelEventKind::VersioningPreviewStopped => "kernel.versioning.preview.stopped",
        KernelEventKind::VersioningRestorePublished => "kernel.versioning.restore.published",
        KernelEventKind::VersioningRestoreRecoveryRequired => {
            "kernel.versioning.restore.recovery_required"
        }
        KernelEventKind::VersioningRestoreRecoveryResolved => {
            "kernel.versioning.restore.recovery_resolved"
        }
        KernelEventKind::VersioningRemoteCompleted => "kernel.versioning.remote.completed",
        KernelEventKind::VersioningRemoteFailed => "kernel.versioning.remote.failed",
        KernelEventKind::VersioningRemoteCancelled => "kernel.versioning.remote.cancelled",
        KernelEventKind::VersioningIntegrationPublished => {
            "kernel.versioning.integration.published"
        }
        KernelEventKind::VersioningIntegrationConflict => "kernel.versioning.integration.conflict",
        KernelEventKind::VersioningIntegrationRecoveryRequired => {
            "kernel.versioning.integration.recovery_required"
        }
        KernelEventKind::VersioningIntegrationRecoveryResolved => {
            "kernel.versioning.integration.recovery_resolved"
        }
        KernelEventKind::WritePlanned => "kernel.write.planned",
        KernelEventKind::WriteCommitted => "kernel.write.committed",
        KernelEventKind::WriteRecoveryRequired => "kernel.write.recovery_required",
        KernelEventKind::WriteFailed => "kernel.write.failed",
    }
}

pub(crate) fn kernel_log_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    Ok(PathBuf::from(app_home_snapshot(app)?.app_logs_dir).join(KERNEL_LOG_FILE))
}

pub fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        fs,
        path::PathBuf,
        sync::{
            atomic::{AtomicU64, Ordering},
            Arc,
        },
        thread,
    };

    use super::{
        append_event_to_log_path, kernel_event_name, retention::KernelLogRetentionPolicy,
        KernelEventKind, KernelLogEvent, KernelLogLevel,
    };

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn versioning_events_have_stable_distinct_names() {
        let names = [
            KernelEventKind::VersioningMutationCommitted,
            KernelEventKind::VersioningMutationFailed,
            KernelEventKind::VersioningPreviewStarted,
            KernelEventKind::VersioningPreviewStopped,
            KernelEventKind::VersioningRestorePublished,
            KernelEventKind::VersioningRestoreRecoveryRequired,
            KernelEventKind::VersioningRestoreRecoveryResolved,
            KernelEventKind::VersioningRemoteCompleted,
            KernelEventKind::VersioningRemoteFailed,
            KernelEventKind::VersioningRemoteCancelled,
            KernelEventKind::VersioningIntegrationPublished,
            KernelEventKind::VersioningIntegrationConflict,
            KernelEventKind::VersioningIntegrationRecoveryRequired,
            KernelEventKind::VersioningIntegrationRecoveryResolved,
        ]
        .map(kernel_event_name)
        .into_iter()
        .collect::<BTreeSet<_>>();
        assert_eq!(names.len(), 14);
        assert!(names
            .iter()
            .all(|name| name.starts_with("kernel.versioning.")));
    }

    #[test]
    fn kernel_log_event_starts_with_empty_attributes() {
        let event = KernelLogEvent::new(
            KernelLogLevel::Info,
            KernelEventKind::ProjectTransitionDecisionRetentionRecovered,
            "transaction_log",
            "editor_transaction",
            "recovered",
            Some("session/transactions.jsonl".to_string()),
            "Recovered marker written.",
            None,
        );

        assert_eq!(event.schema_version, 3);
        assert!(event.attributes.is_empty());
    }

    #[test]
    fn kernel_log_event_serializes_structured_attributes() {
        let event = KernelLogEvent::new(
            KernelLogLevel::Info,
            KernelEventKind::ProjectTransitionDecisionRetentionRecovered,
            "transaction_log",
            "editor_transaction",
            "recovered",
            Some("session/transactions.jsonl".to_string()),
            "Recovered marker written.",
            None,
        )
        .with_attribute("transactionId", "transaction-1")
        .with_attribute("bytesWritten", 42_u64);

        assert_eq!(
            event.attributes["transactionId"],
            serde_json::Value::String("transaction-1".to_string())
        );
        assert_eq!(event.attributes["bytesWritten"], serde_json::json!(42));
    }

    #[test]
    fn recovery_coordinator_attention_event_is_a_structured_warning() {
        let event = KernelLogEvent::new(
            KernelLogLevel::Warn,
            KernelEventKind::RecoveryCoordinatorNeedsAttention,
            "project_workspace",
            "session_recovery",
            "recovery_attention",
            Some("project/session".to_string()),
            "ProjectWorkspace recovery requires operator attention.",
            Some("recovery snapshot is not clean".to_string()),
        );

        assert_eq!(
            event.event_name,
            "kernel.recovery_coordinator.needs_attention"
        );
        assert_eq!(event.level, KernelLogLevel::Warn);
        assert_eq!(event.severity_text, "WARN");
        assert_eq!(event.severity_number, 13);
    }

    #[test]
    fn append_event_rotates_active_log_and_records_rotation_event() {
        let root = temp_dir("append-rotation");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("kernel.jsonl");
        fs::write(&path, "existing event line\n").unwrap();
        let policy = KernelLogRetentionPolicy {
            max_active_bytes: 18,
            archive_count: 2,
        };
        let event = KernelLogEvent::new(
            KernelLogLevel::Info,
            KernelEventKind::Boot,
            "kernel",
            "internal_app_write",
            "boot",
            None,
            "Kernel boot initialized.",
            None,
        );

        append_event_to_log_path(None, &path, event, &policy).unwrap();

        assert_eq!(
            fs::read_to_string(root.join("kernel.jsonl.1")).unwrap(),
            "existing event line\n"
        );
        let active = fs::read_to_string(&path).unwrap();
        assert!(active.contains("\"eventName\":\"kernel.observability_log.rotated\""));
        assert!(active.contains("\"eventName\":\"kernel.boot\""));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn concurrent_appends_produce_complete_parseable_jsonl_records() {
        let root = temp_dir("concurrent-jsonl");
        fs::create_dir_all(&root).unwrap();
        let path = Arc::new(root.join("kernel.jsonl"));
        let policy = Arc::new(KernelLogRetentionPolicy {
            max_active_bytes: 1024 * 1024,
            archive_count: 3,
        });
        let writers = 16usize;
        let mut threads = Vec::new();

        for index in 0..writers {
            let path = Arc::clone(&path);
            let policy = Arc::clone(&policy);
            threads.push(thread::spawn(move || {
                append_event_to_log_path(
                    None,
                    &path,
                    KernelLogEvent::new(
                        KernelLogLevel::Info,
                        KernelEventKind::CommandCommitted,
                        "concurrency-test",
                        "internal_app_write",
                        "append",
                        None,
                        format!("record-{index}"),
                        None,
                    ),
                    &policy,
                )
                .unwrap();
            }));
        }
        for writer in threads {
            writer.join().unwrap();
        }

        let source = fs::read_to_string(path.as_ref()).unwrap();
        let records = source
            .lines()
            .map(|line| serde_json::from_str::<KernelLogEvent>(line).unwrap())
            .collect::<Vec<_>>();
        let messages = records
            .iter()
            .map(|event| event.message.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(records.len(), writers);
        assert_eq!(messages.len(), writers);
        for index in 0..writers {
            assert!(messages.contains(&format!("record-{index}")));
        }
        fs::remove_dir_all(root).unwrap();
    }

    fn temp_dir(label: &str) -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("pana-observability-{id}-{label}"))
    }
}
