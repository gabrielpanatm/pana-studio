use super::*;

pub(super) fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

pub(super) fn record_versioning_event(
    app: &AppHandle,
    level: KernelLogLevel,
    kind: KernelEventKind,
    operation: &str,
    target: Option<String>,
    message: &str,
    diagnostic: Option<String>,
) {
    let event = KernelLogEvent::new(
        level,
        kind,
        "versioning",
        "project_source_write",
        operation,
        target,
        message,
        diagnostic,
    );
    if let Err(error) = append_event(app, event) {
        eprintln!("[Pană Studio] Evenimentul de observabilitate Git nu a fost scris: {error}");
    }
}
