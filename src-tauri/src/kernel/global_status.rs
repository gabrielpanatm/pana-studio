use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

pub const GLOBAL_STATUS_SCHEMA_VERSION: u32 = 1;
const DEFAULT_TRANSIENT_MS: u64 = 4_000;
const RETAINED_RESOLVED_EVENTS: usize = 24;
const MAX_OPEN_STATUS_EVENTS: usize = 64;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GlobalStatusSeverity {
    Info,
    Success,
    Warning,
    Error,
    Blocking,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GlobalStatusPhase {
    Idle,
    Active,
    Settled,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GlobalStatusLifecycle {
    Transient,
    UntilReplaced,
    UntilResolved,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GlobalStatusEscalation {
    StatusOnly,
    Notification,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GlobalStatusResolution {
    Open,
    Resolved,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GlobalStatusNotificationLevel {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalStatusNotification {
    pub title: String,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub level: Option<GlobalStatusNotificationLevel>,
    #[serde(default)]
    pub action_label: Option<String>,
    #[serde(default)]
    pub action_id: Option<String>,
    #[serde(default)]
    pub secondary_action_label: Option<String>,
    #[serde(default)]
    pub secondary_action_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GlobalStatusInput {
    pub schema_version: u32,
    pub code: String,
    pub source: String,
    pub message: String,
    pub severity: GlobalStatusSeverity,
    #[serde(default)]
    pub phase: Option<GlobalStatusPhase>,
    #[serde(default)]
    pub priority: Option<u16>,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub lifecycle: Option<GlobalStatusLifecycle>,
    #[serde(default)]
    pub escalation: Option<GlobalStatusEscalation>,
    #[serde(default)]
    pub dedupe_key: Option<String>,
    #[serde(default)]
    pub resolution_key: Option<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub notification: Option<GlobalStatusNotification>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalStatusEvent {
    pub schema_version: u32,
    pub id: String,
    pub code: String,
    pub source: String,
    pub severity: GlobalStatusSeverity,
    pub phase: GlobalStatusPhase,
    pub priority: u16,
    pub message: String,
    pub detail: Option<String>,
    pub lifecycle: GlobalStatusLifecycle,
    pub escalation: GlobalStatusEscalation,
    pub dedupe_key: String,
    pub resolution_key: Option<String>,
    pub resolution: GlobalStatusResolution,
    pub sequence: u64,
    pub created_at: u64,
    pub updated_at: u64,
    pub expires_at: Option<u64>,
    pub resolved_at: Option<u64>,
    pub notification: Option<GlobalStatusNotification>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalStatusSnapshot {
    pub schema_version: u32,
    pub revision: u64,
    pub events: Vec<GlobalStatusEvent>,
    pub current: Option<GlobalStatusEvent>,
}

#[derive(Default)]
struct GlobalStatusState {
    revision: u64,
    sequence: u64,
    events: Vec<GlobalStatusEvent>,
}

#[derive(Default)]
pub struct GlobalStatusRuntime {
    state: Mutex<GlobalStatusState>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn validate_segment(value: &str, label: &str, maximum: usize) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(format!("GlobalStatus cere {label}."));
    }
    if value.len() > maximum {
        return Err(format!(
            "GlobalStatus {label} depășește limita de {maximum} octeți."
        ));
    }
    Ok(value.to_string())
}

fn validate_optional_segment(
    value: Option<String>,
    label: &str,
    maximum: usize,
) -> Result<Option<String>, String> {
    value
        .as_deref()
        .map(|value| validate_segment(value, label, maximum))
        .transpose()
}

fn normalize_notification(
    notification: Option<GlobalStatusNotification>,
) -> Result<Option<GlobalStatusNotification>, String> {
    let Some(notification) = notification else {
        return Ok(None);
    };
    Ok(Some(GlobalStatusNotification {
        title: validate_segment(&notification.title, "notification.title", 512)?,
        message: validate_optional_segment(notification.message, "notification.message", 8_192)?,
        level: notification.level,
        action_label: validate_optional_segment(
            notification.action_label,
            "notification.actionLabel",
            256,
        )?,
        action_id: validate_optional_segment(notification.action_id, "notification.actionId", 240)?,
        secondary_action_label: validate_optional_segment(
            notification.secondary_action_label,
            "notification.secondaryActionLabel",
            256,
        )?,
        secondary_action_id: validate_optional_segment(
            notification.secondary_action_id,
            "notification.secondaryActionId",
            240,
        )?,
    }))
}

fn default_phase(input: &GlobalStatusInput) -> GlobalStatusPhase {
    input.phase.clone().unwrap_or(GlobalStatusPhase::Settled)
}

fn default_lifecycle(
    severity: &GlobalStatusSeverity,
    phase: &GlobalStatusPhase,
) -> GlobalStatusLifecycle {
    match severity {
        GlobalStatusSeverity::Blocking | GlobalStatusSeverity::Error => {
            GlobalStatusLifecycle::UntilResolved
        }
        GlobalStatusSeverity::Warning => GlobalStatusLifecycle::UntilReplaced,
        _ if phase == &GlobalStatusPhase::Active => GlobalStatusLifecycle::UntilReplaced,
        _ => GlobalStatusLifecycle::Transient,
    }
}

fn default_escalation(severity: &GlobalStatusSeverity) -> GlobalStatusEscalation {
    match severity {
        GlobalStatusSeverity::Blocking | GlobalStatusSeverity::Error => {
            GlobalStatusEscalation::Notification
        }
        _ => GlobalStatusEscalation::StatusOnly,
    }
}

pub fn global_status_priority(severity: &GlobalStatusSeverity, phase: &GlobalStatusPhase) -> u16 {
    match severity {
        GlobalStatusSeverity::Blocking => 500,
        GlobalStatusSeverity::Error => 450,
        GlobalStatusSeverity::Warning => 350,
        _ if phase == &GlobalStatusPhase::Active => 300,
        GlobalStatusSeverity::Success => 200,
        GlobalStatusSeverity::Info => 100,
    }
}

fn normalize_input(
    input: GlobalStatusInput,
    sequence: u64,
    timestamp: u64,
) -> Result<GlobalStatusEvent, String> {
    if input.schema_version != GLOBAL_STATUS_SCHEMA_VERSION {
        return Err(format!(
            "GlobalStatus schema incompatibilă: {}, așteptat {}.",
            input.schema_version, GLOBAL_STATUS_SCHEMA_VERSION
        ));
    }
    let code = validate_segment(&input.code, "code", 160)?;
    let source = validate_segment(&input.source, "source", 80)?;
    let message = validate_segment(&input.message, "message", 4_096)?;
    let detail = validate_optional_segment(input.detail.clone(), "detail", 16_384)?;
    let notification = normalize_notification(input.notification.clone())?;
    let phase = default_phase(&input);
    let lifecycle = input
        .lifecycle
        .clone()
        .unwrap_or_else(|| default_lifecycle(&input.severity, &phase));
    let escalation = input
        .escalation
        .clone()
        .unwrap_or_else(|| default_escalation(&input.severity));
    let dedupe_key = input
        .dedupe_key
        .as_deref()
        .map(|value| validate_segment(value, "dedupeKey", 240))
        .transpose()?
        .unwrap_or_else(|| format!("{source}:{code}"));
    let resolution_key = input
        .resolution_key
        .as_deref()
        .map(|value| validate_segment(value, "resolutionKey", 240))
        .transpose()?;
    let expires_at = if lifecycle == GlobalStatusLifecycle::Transient {
        Some(timestamp.saturating_add(input.timeout_ms.unwrap_or(DEFAULT_TRANSIENT_MS)))
    } else {
        None
    };
    Ok(GlobalStatusEvent {
        schema_version: GLOBAL_STATUS_SCHEMA_VERSION,
        id: format!("global-status:{sequence}"),
        code,
        source,
        severity: input.severity.clone(),
        phase: phase.clone(),
        priority: input
            .priority
            .unwrap_or_else(|| global_status_priority(&input.severity, &phase)),
        message,
        detail,
        lifecycle,
        escalation,
        dedupe_key,
        resolution_key,
        resolution: GlobalStatusResolution::Open,
        sequence,
        created_at: timestamp,
        updated_at: timestamp,
        expires_at,
        resolved_at: None,
        notification,
    })
}

fn resolve_expired(events: &mut [GlobalStatusEvent], timestamp: u64) {
    for event in events {
        if event.resolution == GlobalStatusResolution::Open
            && event
                .expires_at
                .is_some_and(|expires_at| expires_at <= timestamp)
        {
            event.resolution = GlobalStatusResolution::Resolved;
            event.resolved_at = event.expires_at;
            event.updated_at = event.expires_at.unwrap_or(timestamp);
        }
    }
}

fn compact_events(events: &mut Vec<GlobalStatusEvent>) {
    let mut resolved = events
        .iter()
        .filter(|event| event.resolution == GlobalStatusResolution::Resolved)
        .cloned()
        .collect::<Vec<_>>();
    resolved.sort_by_key(|event| event.sequence);
    if resolved.len() > RETAINED_RESOLVED_EVENTS {
        resolved.drain(..resolved.len() - RETAINED_RESOLVED_EVENTS);
    }
    let mut open = events
        .iter()
        .filter(|event| event.resolution == GlobalStatusResolution::Open)
        .cloned()
        .collect::<Vec<_>>();
    open.sort_by_key(|event| event.sequence);
    resolved.extend(open);
    *events = resolved;
}

fn current_event(events: &[GlobalStatusEvent]) -> Option<GlobalStatusEvent> {
    events
        .iter()
        .filter(|event| event.resolution == GlobalStatusResolution::Open)
        .max_by(|left, right| match left.priority.cmp(&right.priority) {
            Ordering::Equal => left.sequence.cmp(&right.sequence),
            ordering => ordering,
        })
        .cloned()
}

fn snapshot(state: &GlobalStatusState) -> GlobalStatusSnapshot {
    GlobalStatusSnapshot {
        schema_version: GLOBAL_STATUS_SCHEMA_VERSION,
        revision: state.revision,
        events: state.events.clone(),
        current: current_event(&state.events),
    }
}

impl GlobalStatusRuntime {
    pub fn publish(&self, input: GlobalStatusInput) -> Result<GlobalStatusSnapshot, String> {
        self.publish_at(input, now_ms())
    }

    pub fn resolve(&self, key: &str) -> Result<GlobalStatusSnapshot, String> {
        self.resolve_at(key, now_ms())
    }

    pub fn read(&self) -> Result<GlobalStatusSnapshot, String> {
        self.read_at(now_ms())
    }

    fn publish_at(
        &self,
        input: GlobalStatusInput,
        timestamp: u64,
    ) -> Result<GlobalStatusSnapshot, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "GlobalStatus runtime este indisponibil.".to_string())?;
        let next_sequence = state
            .sequence
            .checked_add(1)
            .ok_or_else(|| "GlobalStatus a epuizat spațiul de secvențe.".to_string())?;
        let event = normalize_input(input, next_sequence, timestamp)?;
        let open_event_count = state
            .events
            .iter()
            .filter(|candidate| {
                candidate.resolution == GlobalStatusResolution::Open
                    && candidate
                        .expires_at
                        .is_none_or(|expires_at| expires_at > timestamp)
            })
            .count();
        let replaces_open_lane = state
            .events
            .iter()
            .filter(|candidate| {
                candidate.resolution == GlobalStatusResolution::Open
                    && candidate
                        .expires_at
                        .is_none_or(|expires_at| expires_at > timestamp)
            })
            .any(|candidate| candidate.dedupe_key == event.dedupe_key);
        if open_event_count >= MAX_OPEN_STATUS_EVENTS && !replaces_open_lane {
            return Err(format!(
                "GlobalStatus refuză mai mult de {MAX_OPEN_STATUS_EVENTS} evenimente deschise."
            ));
        }
        resolve_expired(&mut state.events, timestamp);
        state.sequence = next_sequence;
        for candidate in &mut state.events {
            if candidate.resolution == GlobalStatusResolution::Open
                && candidate.dedupe_key == event.dedupe_key
            {
                candidate.resolution = GlobalStatusResolution::Resolved;
                candidate.resolved_at = Some(timestamp);
                candidate.updated_at = timestamp;
            }
        }
        state.events.push(event);
        compact_events(&mut state.events);
        state.revision = state.revision.saturating_add(1);
        Ok(snapshot(&state))
    }

    fn resolve_at(&self, key: &str, timestamp: u64) -> Result<GlobalStatusSnapshot, String> {
        let key = validate_segment(key, "resolution key", 240)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| "GlobalStatus runtime este indisponibil.".to_string())?;
        resolve_expired(&mut state.events, timestamp);
        let mut changed = false;
        for event in &mut state.events {
            if event.resolution == GlobalStatusResolution::Open
                && (event.id == key
                    || event.dedupe_key == key
                    || event.resolution_key.as_deref() == Some(key.as_str()))
            {
                event.resolution = GlobalStatusResolution::Resolved;
                event.resolved_at = Some(timestamp);
                event.updated_at = timestamp;
                changed = true;
            }
        }
        if changed {
            state.revision = state.revision.saturating_add(1);
        }
        compact_events(&mut state.events);
        Ok(snapshot(&state))
    }

    fn read_at(&self, timestamp: u64) -> Result<GlobalStatusSnapshot, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "GlobalStatus runtime este indisponibil.".to_string())?;
        let before = state
            .events
            .iter()
            .filter(|event| event.resolution == GlobalStatusResolution::Open)
            .count();
        resolve_expired(&mut state.events, timestamp);
        let after = state
            .events
            .iter()
            .filter(|event| event.resolution == GlobalStatusResolution::Open)
            .count();
        if before != after {
            state.revision = state.revision.saturating_add(1);
        }
        compact_events(&mut state.events);
        Ok(snapshot(&state))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    fn input(code: &str, severity: GlobalStatusSeverity) -> GlobalStatusInput {
        GlobalStatusInput {
            schema_version: GLOBAL_STATUS_SCHEMA_VERSION,
            code: code.to_string(),
            source: "test".to_string(),
            message: code.to_string(),
            severity,
            phase: None,
            priority: None,
            detail: None,
            lifecycle: None,
            escalation: None,
            dedupe_key: None,
            resolution_key: None,
            timeout_ms: None,
            notification: None,
        }
    }

    #[test]
    fn priority_matches_the_global_status_contract() {
        assert!(
            global_status_priority(&GlobalStatusSeverity::Blocking, &GlobalStatusPhase::Settled)
                > global_status_priority(&GlobalStatusSeverity::Error, &GlobalStatusPhase::Settled)
        );
        assert!(
            global_status_priority(&GlobalStatusSeverity::Warning, &GlobalStatusPhase::Settled)
                > global_status_priority(&GlobalStatusSeverity::Info, &GlobalStatusPhase::Active)
        );
        assert!(
            global_status_priority(&GlobalStatusSeverity::Info, &GlobalStatusPhase::Active)
                > global_status_priority(
                    &GlobalStatusSeverity::Success,
                    &GlobalStatusPhase::Settled
                )
        );
    }

    #[test]
    fn tauri_json_contract_uses_the_canonical_rust_enum_values() {
        let input: GlobalStatusInput = serde_json::from_value(serde_json::json!({
            "schemaVersion": GLOBAL_STATUS_SCHEMA_VERSION,
            "code": "project.opening",
            "source": "project",
            "message": "Se deschide proiectul.",
            "severity": "info",
            "phase": "active",
            "lifecycle": "until_replaced",
            "escalation": "status_only",
            "dedupeKey": "project:current"
        }))
        .expect("contractul Tauri trebuie să accepte valorile canonice Rust");

        assert_eq!(input.lifecycle, Some(GlobalStatusLifecycle::UntilReplaced));
        assert_eq!(input.escalation, Some(GlobalStatusEscalation::StatusOnly));

        let event = normalize_input(input, 1, 10).expect("statusul trebuie normalizat");
        let serialized =
            serde_json::to_value(event).expect("evenimentul trebuie serializat pentru frontend");
        assert_eq!(serialized["lifecycle"], "until_replaced");
        assert_eq!(serialized["escalation"], "status_only");

        let legacy = serde_json::from_value::<GlobalStatusInput>(serde_json::json!({
            "schemaVersion": GLOBAL_STATUS_SCHEMA_VERSION,
            "code": "project.opening",
            "source": "project",
            "message": "Se deschide proiectul.",
            "severity": "info",
            "escalation": "status-only"
        }));
        assert!(
            legacy.is_err(),
            "valoarea frontend legacy nu trebuie reintrodusă în contract"
        );
    }

    #[test]
    fn latest_event_replaces_the_same_dedupe_lane() {
        let runtime = GlobalStatusRuntime::default();
        let mut first = input("first", GlobalStatusSeverity::Info);
        first.dedupe_key = Some("lane".to_string());
        let mut second = input("second", GlobalStatusSeverity::Success);
        second.dedupe_key = Some("lane".to_string());
        runtime.publish_at(first, 10).unwrap();
        let snapshot = runtime.publish_at(second, 20).unwrap();
        assert_eq!(
            snapshot.current.as_ref().map(|event| event.code.as_str()),
            Some("second")
        );
        assert_eq!(
            snapshot
                .events
                .iter()
                .find(|event| event.code == "first")
                .map(|event| &event.resolution),
            Some(&GlobalStatusResolution::Resolved)
        );
    }

    #[test]
    fn transient_events_expire_but_active_events_wait_for_replacement() {
        let runtime = GlobalStatusRuntime::default();
        let mut transient = input("transient", GlobalStatusSeverity::Info);
        transient.timeout_ms = Some(5);
        runtime.publish_at(transient, 10).unwrap();
        let mut active = input("active", GlobalStatusSeverity::Info);
        active.phase = Some(GlobalStatusPhase::Active);
        active.dedupe_key = Some("active".to_string());
        runtime.publish_at(active, 11).unwrap();
        let snapshot = runtime.read_at(20).unwrap();
        assert_eq!(
            snapshot.current.as_ref().map(|event| event.code.as_str()),
            Some("active")
        );
        assert_eq!(
            snapshot
                .events
                .iter()
                .find(|event| event.code == "transient")
                .map(|event| &event.resolution),
            Some(&GlobalStatusResolution::Resolved)
        );
    }

    #[test]
    fn errors_escalate_and_resolve_by_resolution_key() {
        let runtime = GlobalStatusRuntime::default();
        let mut error = input("failed", GlobalStatusSeverity::Error);
        error.resolution_key = Some("operation".to_string());
        let published = runtime.publish_at(error, 10).unwrap();
        assert_eq!(
            published.current.as_ref().map(|event| &event.escalation),
            Some(&GlobalStatusEscalation::Notification)
        );
        let resolved = runtime.resolve_at("operation", 11).unwrap();
        assert!(resolved.current.is_none());
    }

    #[test]
    fn concurrent_publishers_are_serialized_and_latest_sequence_wins() {
        const PUBLISHERS: usize = 16;
        let runtime = Arc::new(GlobalStatusRuntime::default());
        let barrier = Arc::new(Barrier::new(PUBLISHERS));
        let handles = (0..PUBLISHERS)
            .map(|index| {
                let runtime = Arc::clone(&runtime);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let mut status =
                        input(&format!("concurrent-{index}"), GlobalStatusSeverity::Info);
                    status.dedupe_key = Some("concurrent-lane".to_string());
                    barrier.wait();
                    runtime.publish_at(status, 10).unwrap();
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            handle.join().unwrap();
        }

        let snapshot = runtime.read_at(11).unwrap();
        let current = snapshot.current.expect("lipsește evenimentul curent");
        assert_eq!(snapshot.revision, PUBLISHERS as u64);
        assert_eq!(current.sequence, PUBLISHERS as u64);
        assert_eq!(
            snapshot
                .events
                .iter()
                .filter(|event| event.resolution == GlobalStatusResolution::Open)
                .count(),
            1
        );
    }

    #[test]
    fn notification_payload_is_validated_at_the_rust_boundary() {
        let runtime = GlobalStatusRuntime::default();
        let mut status = input("invalid-notification", GlobalStatusSeverity::Error);
        status.notification = Some(GlobalStatusNotification {
            title: " ".to_string(),
            message: None,
            level: Some(GlobalStatusNotificationLevel::Error),
            action_label: None,
            action_id: None,
            secondary_action_label: None,
            secondary_action_id: None,
        });
        let error = runtime.publish_at(status, 10).unwrap_err();
        assert!(error.contains("notification.title"));

        let accepted = runtime
            .publish_at(input("accepted", GlobalStatusSeverity::Info), 11)
            .unwrap();
        assert_eq!(accepted.revision, 1);
        assert_eq!(accepted.current.map(|event| event.sequence), Some(1));
    }
}
