use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Seek, SeekFrom},
    path::Path,
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

use crate::kernel::write_authority::{
    capability_lock_observability_file, CapabilityMaintenanceLockMode, WriteAuthorityRuntime,
};

use super::{
    health::{
        is_recovery_event, KernelObservabilityHealthAccumulator, KernelObservabilityHealthSnapshot,
    },
    kernel_log_path,
    retention::{archive_path, read_kernel_log_retention_snapshot},
    KernelLogEvent, KernelLogLevel, KernelLogRetentionSnapshot, KERNEL_LOG_LOCK_FILE,
};

const KERNEL_LOG_READER_SCHEMA_VERSION: u32 = 3;
const DEFAULT_EVENT_LIMIT: usize = 80;
const MAX_EVENT_LIMIT: usize = 200;
const MAX_KERNEL_LOG_SCAN_BYTES: u64 = 2 * 1024 * 1024;
const MAX_KERNEL_LOG_LINE_BYTES: usize = 256 * 1024;
const MAX_KERNEL_LOG_DIAGNOSTICS: usize = 20;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelObservabilityLogRequest {
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub recovery_only: Option<bool>,
    #[serde(default)]
    pub include_archives: Option<bool>,
    #[serde(default)]
    pub levels: Option<Vec<KernelLogLevel>>,
    #[serde(default)]
    pub source_filter: Option<KernelObservabilityLogSourceFilter>,
    #[serde(default)]
    pub event_names: Option<Vec<String>>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelObservabilityLogSourceFilter {
    #[default]
    All,
    Active,
    Archives,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelObservabilityLogSourceSnapshot {
    pub path: String,
    pub archive_index: Option<usize>,
    pub exists: bool,
    pub truncated: bool,
    pub scanned_bytes: u64,
    pub scanned_line_count: usize,
    pub unreadable_count: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelObservabilityLogEventSourceSnapshot {
    pub path: String,
    pub archive_index: Option<usize>,
    pub label: String,
    pub active: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelObservabilityLogEventSnapshot {
    #[serde(flatten)]
    pub event: KernelLogEvent,
    pub source: KernelObservabilityLogEventSourceSnapshot,
}

impl std::ops::Deref for KernelObservabilityLogEventSnapshot {
    type Target = KernelLogEvent;

    fn deref(&self) -> &Self::Target {
        &self.event
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelObservabilityLogSnapshot {
    pub schema_version: u32,
    pub log_path: String,
    pub log_exists: bool,
    pub truncated: bool,
    pub scanned_bytes: u64,
    pub scanned_line_count: usize,
    pub returned_count: usize,
    pub unreadable_count: usize,
    pub recovery_only: bool,
    pub include_archives: bool,
    pub levels: Vec<KernelLogLevel>,
    pub event_names: Vec<String>,
    pub source_filter: KernelObservabilityLogSourceFilter,
    pub limit: usize,
    pub retention: KernelLogRetentionSnapshot,
    pub health: KernelObservabilityHealthSnapshot,
    pub sources: Vec<KernelObservabilityLogSourceSnapshot>,
    pub events: Vec<KernelObservabilityLogEventSnapshot>,
    pub diagnostics: Vec<String>,
}

pub fn read_kernel_observability_log_snapshot<R: Runtime>(
    app: &AppHandle<R>,
    request: KernelObservabilityLogRequest,
) -> Result<KernelObservabilityLogSnapshot, String> {
    let path = kernel_log_path(app)?;
    let runtime = app
        .try_state::<WriteAuthorityRuntime>()
        .ok_or_else(|| "Observability reader nu are WriteAuthorityRuntime instalat.".to_string())?;
    read_kernel_observability_log_file_authorized(Some(runtime.inner()), &path, request)
}

#[cfg(test)]
pub(crate) fn read_kernel_observability_log_file(
    path: &Path,
    request: KernelObservabilityLogRequest,
) -> Result<KernelObservabilityLogSnapshot, String> {
    read_kernel_observability_log_file_authorized(None, path, request)
}

fn read_kernel_observability_log_file_authorized(
    runtime: Option<&WriteAuthorityRuntime>,
    path: &Path,
    request: KernelObservabilityLogRequest,
) -> Result<KernelObservabilityLogSnapshot, String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Observability reader nu are boundary părinte.".to_string())?;
    let lock_path = parent.join(KERNEL_LOG_LOCK_FILE);
    let _stable_lock = capability_lock_observability_file(
        runtime,
        &lock_path,
        parent,
        "observability/kernel-log-stable-lock",
        CapabilityMaintenanceLockMode::Shared,
    )
    .map_err(|error| error.into_terminal_diagnostic())?;
    let limit = request
        .limit
        .unwrap_or(DEFAULT_EVENT_LIMIT)
        .clamp(1, MAX_EVENT_LIMIT);
    let recovery_only = request.recovery_only.unwrap_or(false);
    let include_archives = request.include_archives.unwrap_or(false);
    let levels = normalize_level_filter(request.levels);
    let event_names = normalize_event_name_filter(request.event_names);
    let source_filter = request.source_filter.unwrap_or_default();
    let log_exists = active_log_exists(path)?;
    let retention = read_kernel_log_retention_snapshot(path)?;
    let plans = read_source_plans(
        path,
        include_archives,
        retention.archive_count,
        source_filter,
    )?;
    let mut state = ReadState::new(
        path,
        log_exists,
        limit,
        recovery_only,
        include_archives,
        levels,
        event_names,
        source_filter,
        retention,
    );

    for plan in plans {
        read_observability_source(plan, &mut state)?;
    }

    state.events.reverse();
    Ok(state.into_snapshot())
}

fn read_observability_source(plan: ReadSourcePlan, state: &mut ReadState) -> Result<(), String> {
    state.last_source_scanned_line_count = 0;
    state.last_source_unreadable_count = 0;

    let mut file = match File::open(&plan.path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            state.sources.push(KernelObservabilityLogSourceSnapshot {
                path: plan.path.to_string_lossy().to_string(),
                archive_index: plan.archive_index,
                exists: false,
                truncated: false,
                scanned_bytes: 0,
                scanned_line_count: 0,
                unreadable_count: 0,
            });
            return Ok(());
        }
        Err(error) => {
            return Err(format!(
                "Observability Log nu poate fi citit din {}: {}",
                plan.path.display(),
                error
            ));
        }
    };

    if plan.archive_index.is_none() {
        state.log_exists = true;
    }

    let len = file
        .metadata()
        .map_err(|error| {
            format!(
                "Nu am putut citi metadata Observability Log {}: {}",
                plan.path.display(),
                error
            )
        })?
        .len();
    let truncated = len > MAX_KERNEL_LOG_SCAN_BYTES;
    let scanned_bytes = if truncated {
        let start = len.saturating_sub(MAX_KERNEL_LOG_SCAN_BYTES);
        file.seek(SeekFrom::Start(start)).map_err(|error| {
            format!(
                "Nu am putut poziționa citirea Observability Log {}: {}",
                plan.path.display(),
                error
            )
        })?;
        let mut reader = BufReader::new(file);
        let mut discarded = String::new();
        let _ = reader.read_line(&mut discarded);
        read_observability_lines(&plan, reader, state, true, MAX_KERNEL_LOG_SCAN_BYTES)?
    } else {
        let reader = BufReader::new(file);
        read_observability_lines(&plan, reader, state, false, len)?
    };

    state.sources.push(KernelObservabilityLogSourceSnapshot {
        path: plan.path.to_string_lossy().to_string(),
        archive_index: plan.archive_index,
        exists: true,
        truncated,
        scanned_bytes,
        scanned_line_count: state.last_source_scanned_line_count,
        unreadable_count: state.last_source_unreadable_count,
    });

    Ok(())
}

fn read_observability_lines<R: BufRead>(
    plan: &ReadSourcePlan,
    mut reader: R,
    state: &mut ReadState,
    source_truncated: bool,
    scanned_bytes: u64,
) -> Result<u64, String> {
    let mut line = String::new();
    let mut scanned_line_count = 0_usize;
    let mut unreadable_count = 0_usize;

    loop {
        line.clear();
        let bytes = reader.read_line(&mut line).map_err(|error| {
            format!(
                "Nu am putut citi Observability Log {}: {}",
                plan.path.display(),
                error
            )
        })?;
        if bytes == 0 {
            break;
        }
        scanned_line_count += 1;

        if bytes > MAX_KERNEL_LOG_LINE_BYTES {
            unreadable_count += 1;
            state.unreadable_count += 1;
            push_diagnostic(
                &mut state.diagnostics,
                format!(
                    "{}: linia {} depășește limita de {} bytes și a fost ignorată.",
                    source_label(plan),
                    scanned_line_count,
                    MAX_KERNEL_LOG_LINE_BYTES
                ),
            );
            continue;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match serde_json::from_str::<KernelLogEvent>(trimmed) {
            Ok(event) => {
                if !state.event_matches_filters(&event) {
                    continue;
                }
                let source = event_source_snapshot(plan);
                state
                    .health
                    .ingest_event(&event, source.active, source.label.as_str());
                state
                    .events
                    .push(KernelObservabilityLogEventSnapshot { event, source });
                if state.events.len() > state.limit {
                    state.events.remove(0);
                }
            }
            Err(error) => {
                unreadable_count += 1;
                state.unreadable_count += 1;
                push_diagnostic(
                    &mut state.diagnostics,
                    format!(
                        "{}: linia {} nu este eveniment kernel valid: {}",
                        source_label(plan),
                        scanned_line_count,
                        error
                    ),
                );
            }
        }
    }

    state.scanned_bytes = state.scanned_bytes.saturating_add(scanned_bytes);
    state.scanned_line_count += scanned_line_count;
    state.truncated = state.truncated || source_truncated;
    state.last_source_scanned_line_count = scanned_line_count;
    state.last_source_unreadable_count = unreadable_count;
    Ok(scanned_bytes)
}

fn read_source_plans(
    path: &Path,
    include_archives: bool,
    archive_count: usize,
    source_filter: KernelObservabilityLogSourceFilter,
) -> Result<Vec<ReadSourcePlan>, String> {
    let mut plans = Vec::new();
    if include_archives && source_filter_includes_archives(source_filter) {
        for index in (1..=archive_count).rev() {
            plans.push(ReadSourcePlan {
                path: archive_path(path, index)?,
                archive_index: Some(index),
            });
        }
    }
    if source_filter_includes_active(source_filter) {
        plans.push(ReadSourcePlan {
            path: path.to_path_buf(),
            archive_index: None,
        });
    }
    Ok(plans)
}

fn active_log_exists(path: &Path) -> Result<bool, String> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "Nu am putut citi metadata Observability Log {}: {}",
            path.display(),
            error
        )),
    }
}

fn normalize_level_filter(levels: Option<Vec<KernelLogLevel>>) -> Vec<KernelLogLevel> {
    match levels {
        Some(levels) => all_log_levels()
            .into_iter()
            .filter(|level| levels.contains(level))
            .collect(),
        None => all_log_levels(),
    }
}

fn all_log_levels() -> Vec<KernelLogLevel> {
    vec![
        KernelLogLevel::Info,
        KernelLogLevel::Warn,
        KernelLogLevel::Error,
    ]
}

fn normalize_event_name_filter(event_names: Option<Vec<String>>) -> Vec<String> {
    let mut normalized = Vec::new();
    for name in event_names.unwrap_or_default() {
        let trimmed = name.trim();
        if trimmed.is_empty() || normalized.iter().any(|known| known == trimmed) {
            continue;
        }
        normalized.push(trimmed.to_string());
    }
    normalized
}

fn source_filter_includes_active(source_filter: KernelObservabilityLogSourceFilter) -> bool {
    matches!(
        source_filter,
        KernelObservabilityLogSourceFilter::All | KernelObservabilityLogSourceFilter::Active
    )
}

fn source_filter_includes_archives(source_filter: KernelObservabilityLogSourceFilter) -> bool {
    matches!(
        source_filter,
        KernelObservabilityLogSourceFilter::All | KernelObservabilityLogSourceFilter::Archives
    )
}

fn push_diagnostic(diagnostics: &mut Vec<String>, diagnostic: String) {
    if diagnostics.len() < MAX_KERNEL_LOG_DIAGNOSTICS {
        diagnostics.push(diagnostic);
    }
}

fn source_label(plan: &ReadSourcePlan) -> String {
    match plan.archive_index {
        Some(index) => format!("kernel.jsonl.{index}"),
        None => "kernel.jsonl".to_string(),
    }
}

fn event_source_snapshot(plan: &ReadSourcePlan) -> KernelObservabilityLogEventSourceSnapshot {
    KernelObservabilityLogEventSourceSnapshot {
        path: plan.path.to_string_lossy().to_string(),
        archive_index: plan.archive_index,
        label: source_label(plan),
        active: plan.archive_index.is_none(),
    }
}

#[derive(Clone, Debug)]
struct ReadSourcePlan {
    path: std::path::PathBuf,
    archive_index: Option<usize>,
}

struct ReadState {
    log_path: String,
    log_exists: bool,
    truncated: bool,
    scanned_bytes: u64,
    scanned_line_count: usize,
    unreadable_count: usize,
    recovery_only: bool,
    include_archives: bool,
    levels: Vec<KernelLogLevel>,
    event_names: Vec<String>,
    source_filter: KernelObservabilityLogSourceFilter,
    limit: usize,
    retention: KernelLogRetentionSnapshot,
    health: KernelObservabilityHealthAccumulator,
    sources: Vec<KernelObservabilityLogSourceSnapshot>,
    events: Vec<KernelObservabilityLogEventSnapshot>,
    diagnostics: Vec<String>,
    last_source_scanned_line_count: usize,
    last_source_unreadable_count: usize,
}

impl ReadState {
    fn new(
        path: &Path,
        log_exists: bool,
        limit: usize,
        recovery_only: bool,
        include_archives: bool,
        levels: Vec<KernelLogLevel>,
        event_names: Vec<String>,
        source_filter: KernelObservabilityLogSourceFilter,
        retention: KernelLogRetentionSnapshot,
    ) -> Self {
        Self {
            log_path: path.to_string_lossy().to_string(),
            log_exists,
            truncated: false,
            scanned_bytes: 0,
            scanned_line_count: 0,
            unreadable_count: 0,
            recovery_only,
            include_archives,
            levels,
            event_names,
            source_filter,
            limit,
            retention,
            health: KernelObservabilityHealthAccumulator::default(),
            sources: Vec::new(),
            events: Vec::new(),
            diagnostics: Vec::new(),
            last_source_scanned_line_count: 0,
            last_source_unreadable_count: 0,
        }
    }

    fn event_matches_filters(&self, event: &KernelLogEvent) -> bool {
        if self.recovery_only && !is_recovery_event(event) {
            return false;
        }
        self.levels.contains(&event.level)
            && (self.event_names.is_empty() || self.event_names.contains(&event.event_name))
    }

    fn into_snapshot(self) -> KernelObservabilityLogSnapshot {
        let ReadState {
            log_path,
            log_exists,
            truncated,
            scanned_bytes,
            scanned_line_count,
            unreadable_count,
            recovery_only,
            include_archives,
            levels,
            event_names,
            source_filter,
            limit,
            retention,
            health,
            sources,
            events,
            diagnostics,
            ..
        } = self;
        let health = health.into_snapshot(unreadable_count, truncated);

        KernelObservabilityLogSnapshot {
            schema_version: KERNEL_LOG_READER_SCHEMA_VERSION,
            log_path,
            log_exists,
            truncated,
            scanned_bytes,
            scanned_line_count,
            returned_count: events.len(),
            unreadable_count,
            recovery_only,
            include_archives,
            levels,
            event_names,
            source_filter,
            limit,
            retention,
            health,
            sources,
            events,
            diagnostics,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::{
        read_kernel_observability_log_file, KernelObservabilityLogRequest,
        KernelObservabilityLogSourceFilter,
    };
    use crate::kernel::observability::{
        KernelEventKind, KernelLogEvent, KernelLogLevel, KernelObservabilityHealthStatus,
    };

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn read_log_returns_newest_events_first_and_applies_limit() {
        let path = temp_log_path("newest-first");
        write_events(
            &path,
            vec![
                event("kernel.command.started", KernelEventKind::CommandStarted),
                event(
                    "kernel.transaction.recovered",
                    KernelEventKind::ProjectTransitionDecisionRetentionRecovered,
                ),
                event(
                    "kernel.workspace_mutation.recovered",
                    KernelEventKind::RecoveryCoordinatorClean,
                ),
            ],
        );

        let snapshot = read_kernel_observability_log_file(
            &path,
            KernelObservabilityLogRequest {
                limit: Some(2),
                recovery_only: Some(false),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(snapshot.returned_count, 2);
        assert_eq!(
            snapshot.events[0].event_name,
            "kernel.workspace_mutation.recovered"
        );
        assert_eq!(
            snapshot.events[1].event_name,
            "kernel.transaction.recovered"
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn read_log_filters_recovery_events() {
        let path = temp_log_path("recovery-only");
        write_events(
            &path,
            vec![
                event("kernel.command.started", KernelEventKind::CommandStarted),
                event(
                    "kernel.transaction.recovery.needs_attention",
                    KernelEventKind::RecoveryCoordinatorNeedsAttention,
                ),
                event(
                    "kernel.transaction.recovered",
                    KernelEventKind::ProjectTransitionDecisionRetentionRecovered,
                ),
                event(
                    "kernel.write.recovery_required",
                    KernelEventKind::WriteRecoveryRequired,
                ),
            ],
        );

        let snapshot = read_kernel_observability_log_file(
            &path,
            KernelObservabilityLogRequest {
                limit: Some(10),
                recovery_only: Some(true),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(snapshot.returned_count, 3);
        assert_eq!(snapshot.health.event_count, 3);
        assert_eq!(snapshot.health.recovery_count, 3);
        assert_eq!(snapshot.health.level_counts.info, 3);
        assert!(snapshot
            .events
            .iter()
            .all(|event| event.event_name.contains("recovery")
                || event.event_name.ends_with("recovered")));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn read_log_filters_by_requested_levels() {
        let path = temp_log_path("levels");
        write_events(
            &path,
            vec![
                event_with_level("kernel.info", KernelEventKind::Boot, KernelLogLevel::Info),
                event_with_level(
                    "kernel.warn",
                    KernelEventKind::UndoFailed,
                    KernelLogLevel::Warn,
                ),
                event_with_level(
                    "kernel.error",
                    KernelEventKind::CommandFailed,
                    KernelLogLevel::Error,
                ),
            ],
        );

        let snapshot = read_kernel_observability_log_file(
            &path,
            KernelObservabilityLogRequest {
                limit: Some(10),
                recovery_only: Some(false),
                levels: Some(vec![KernelLogLevel::Warn, KernelLogLevel::Error]),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(
            snapshot.levels,
            vec![KernelLogLevel::Warn, KernelLogLevel::Error]
        );
        assert_eq!(snapshot.returned_count, 2);
        assert_eq!(snapshot.health.event_count, 2);
        assert_eq!(snapshot.health.level_counts.info, 0);
        assert_eq!(snapshot.health.level_counts.warn, 1);
        assert_eq!(snapshot.health.level_counts.error, 1);
        assert_eq!(
            snapshot.health.status,
            KernelObservabilityHealthStatus::Error
        );
        assert_eq!(snapshot.events[0].event_name, "kernel.error");
        assert_eq!(snapshot.events[1].event_name, "kernel.warn");
        assert!(snapshot
            .events
            .iter()
            .all(|event| matches!(event.level, KernelLogLevel::Warn | KernelLogLevel::Error)));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn read_log_filters_by_event_name() {
        let path = temp_log_path("event-name");
        write_events(
            &path,
            vec![
                event_with_level(
                    "kernel.project_transition.blocked",
                    KernelEventKind::ProjectTransitionBlocked,
                    KernelLogLevel::Warn,
                ),
                event_with_level(
                    "kernel.command.failed",
                    KernelEventKind::CommandFailed,
                    KernelLogLevel::Error,
                ),
                event_with_level(
                    "kernel.project_transition.blocked",
                    KernelEventKind::ProjectTransitionBlocked,
                    KernelLogLevel::Warn,
                ),
            ],
        );

        let snapshot = read_kernel_observability_log_file(
            &path,
            KernelObservabilityLogRequest {
                limit: Some(10),
                recovery_only: Some(false),
                event_names: Some(vec!["kernel.project_transition.blocked".to_string()]),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(
            snapshot.event_names,
            vec!["kernel.project_transition.blocked".to_string()]
        );
        assert_eq!(snapshot.returned_count, 2);
        assert_eq!(snapshot.health.event_count, 2);
        assert!(snapshot
            .events
            .iter()
            .all(|event| event.event_name == "kernel.project_transition.blocked"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn empty_level_filter_returns_no_events() {
        let path = temp_log_path("empty-levels");
        write_events(&path, vec![event("kernel.active", KernelEventKind::Boot)]);

        let snapshot = read_kernel_observability_log_file(
            &path,
            KernelObservabilityLogRequest {
                limit: Some(10),
                recovery_only: Some(false),
                levels: Some(Vec::new()),
                ..Default::default()
            },
        )
        .unwrap();

        assert!(snapshot.levels.is_empty());
        assert_eq!(snapshot.returned_count, 0);
        assert_eq!(snapshot.health.event_count, 0);
        assert_eq!(
            snapshot.health.status,
            KernelObservabilityHealthStatus::Clean
        );
        assert!(snapshot.events.is_empty());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn missing_log_returns_empty_snapshot() {
        let path = temp_log_path("missing");
        let _ = fs::remove_file(&path);

        let snapshot = read_kernel_observability_log_file(
            &path,
            KernelObservabilityLogRequest {
                limit: Some(10),
                recovery_only: Some(true),
                ..Default::default()
            },
        )
        .unwrap();

        assert!(!snapshot.log_exists);
        assert_eq!(snapshot.returned_count, 0);
        assert!(snapshot.events.is_empty());
        assert!(snapshot.diagnostics.is_empty());
    }

    #[test]
    fn invalid_lines_are_reported_without_aborting_the_read() {
        let path = temp_log_path("invalid-line");
        let event = serde_json::to_string(&event(
            "kernel.transaction.recovered",
            KernelEventKind::ProjectTransitionDecisionRetentionRecovered,
        ))
        .unwrap();
        fs::write(&path, format!("not-json\n{event}\n")).unwrap();

        let snapshot = read_kernel_observability_log_file(
            &path,
            KernelObservabilityLogRequest {
                limit: Some(10),
                recovery_only: Some(true),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(snapshot.returned_count, 1);
        assert_eq!(snapshot.unreadable_count, 1);
        assert_eq!(
            snapshot.health.status,
            KernelObservabilityHealthStatus::Error
        );
        assert_eq!(snapshot.diagnostics.len(), 1);
        assert_eq!(
            snapshot.events[0].event_name,
            "kernel.transaction.recovered"
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn read_log_ignores_archives_by_default() {
        let path = temp_log_path("active-only");
        write_events(
            &super::archive_path(&path, 1).unwrap(),
            vec![event("kernel.archive.old", KernelEventKind::Boot)],
        );
        write_events(
            &path,
            vec![event("kernel.active.new", KernelEventKind::Boot)],
        );

        let snapshot = read_kernel_observability_log_file(
            &path,
            KernelObservabilityLogRequest {
                limit: Some(10),
                recovery_only: Some(false),
                ..Default::default()
            },
        )
        .unwrap();

        assert!(!snapshot.include_archives);
        assert_eq!(snapshot.sources.len(), 1);
        assert_eq!(snapshot.returned_count, 1);
        assert_eq!(snapshot.events[0].event_name, "kernel.active.new");
        assert_eq!(snapshot.events[0].source.label, "kernel.jsonl");
        assert!(snapshot.events[0].source.active);
        let _ = fs::remove_file(super::archive_path(&path, 1).unwrap());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn read_log_includes_archives_when_requested_newest_first() {
        let path = temp_log_path("with-archives");
        write_events(
            &super::archive_path(&path, 2).unwrap(),
            vec![event("kernel.archive.old", KernelEventKind::Boot)],
        );
        write_events(
            &super::archive_path(&path, 1).unwrap(),
            vec![event("kernel.archive.middle", KernelEventKind::Boot)],
        );
        write_events(
            &path,
            vec![event("kernel.active.new", KernelEventKind::Boot)],
        );

        let snapshot = read_kernel_observability_log_file(
            &path,
            KernelObservabilityLogRequest {
                limit: Some(10),
                recovery_only: Some(false),
                include_archives: Some(true),
                ..Default::default()
            },
        )
        .unwrap();

        assert!(snapshot.include_archives);
        assert_eq!(snapshot.returned_count, 3);
        assert_eq!(snapshot.health.event_count, 3);
        assert_eq!(snapshot.health.source_counts.active, 1);
        assert_eq!(snapshot.health.source_counts.archived, 2);
        assert_eq!(snapshot.events[0].event_name, "kernel.active.new");
        assert_eq!(snapshot.events[1].event_name, "kernel.archive.middle");
        assert_eq!(snapshot.events[2].event_name, "kernel.archive.old");
        assert_eq!(snapshot.events[0].source.label, "kernel.jsonl");
        assert_eq!(snapshot.events[1].source.label, "kernel.jsonl.1");
        assert_eq!(snapshot.events[2].source.label, "kernel.jsonl.2");
        assert!(snapshot.events[0].source.active);
        assert!(!snapshot.events[1].source.active);
        assert_eq!(snapshot.sources.len(), snapshot.retention.archive_count + 1);
        let _ = fs::remove_file(super::archive_path(&path, 1).unwrap());
        let _ = fs::remove_file(super::archive_path(&path, 2).unwrap());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn read_log_applies_limit_across_active_and_archive_sources() {
        let path = temp_log_path("archive-limit");
        write_events(
            &super::archive_path(&path, 1).unwrap(),
            vec![event("kernel.archive.old", KernelEventKind::Boot)],
        );
        write_events(
            &path,
            vec![
                event("kernel.active.middle", KernelEventKind::Boot),
                event("kernel.active.new", KernelEventKind::Boot),
            ],
        );

        let snapshot = read_kernel_observability_log_file(
            &path,
            KernelObservabilityLogRequest {
                limit: Some(2),
                recovery_only: Some(false),
                include_archives: Some(true),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(snapshot.returned_count, 2);
        assert_eq!(snapshot.health.event_count, 3);
        assert_eq!(snapshot.events[0].event_name, "kernel.active.new");
        assert_eq!(snapshot.events[1].event_name, "kernel.active.middle");
        let _ = fs::remove_file(super::archive_path(&path, 1).unwrap());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn read_log_filters_by_active_source_even_when_archives_are_loaded() {
        let path = temp_log_path("source-active");
        write_events(
            &super::archive_path(&path, 1).unwrap(),
            vec![event("kernel.archive", KernelEventKind::Boot)],
        );
        write_events(&path, vec![event("kernel.active", KernelEventKind::Boot)]);

        let snapshot = read_kernel_observability_log_file(
            &path,
            KernelObservabilityLogRequest {
                limit: Some(10),
                recovery_only: Some(false),
                include_archives: Some(true),
                source_filter: Some(KernelObservabilityLogSourceFilter::Active),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(
            snapshot.source_filter,
            KernelObservabilityLogSourceFilter::Active
        );
        assert_eq!(snapshot.returned_count, 1);
        assert_eq!(snapshot.health.event_count, 1);
        assert_eq!(snapshot.health.source_counts.active, 1);
        assert_eq!(snapshot.health.source_counts.archived, 0);
        assert_eq!(snapshot.events[0].event_name, "kernel.active");
        assert!(snapshot.events[0].source.active);
        assert_eq!(snapshot.sources.len(), 1);
        assert!(snapshot.sources[0].archive_index.is_none());
        let _ = fs::remove_file(super::archive_path(&path, 1).unwrap());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn read_log_filters_by_archive_sources() {
        let path = temp_log_path("source-archives");
        write_events(
            &super::archive_path(&path, 1).unwrap(),
            vec![event("kernel.archive", KernelEventKind::Boot)],
        );
        write_events(&path, vec![event("kernel.active", KernelEventKind::Boot)]);

        let snapshot = read_kernel_observability_log_file(
            &path,
            KernelObservabilityLogRequest {
                limit: Some(10),
                recovery_only: Some(false),
                include_archives: Some(true),
                source_filter: Some(KernelObservabilityLogSourceFilter::Archives),
                ..Default::default()
            },
        )
        .unwrap();

        assert!(snapshot.log_exists);
        assert_eq!(
            snapshot.source_filter,
            KernelObservabilityLogSourceFilter::Archives
        );
        assert_eq!(snapshot.returned_count, 1);
        assert_eq!(snapshot.health.event_count, 1);
        assert_eq!(snapshot.health.source_counts.active, 0);
        assert_eq!(snapshot.health.source_counts.archived, 1);
        assert_eq!(snapshot.events[0].event_name, "kernel.archive");
        assert_eq!(snapshot.events[0].source.label, "kernel.jsonl.1");
        assert!(!snapshot.events[0].source.active);
        assert!(snapshot
            .sources
            .iter()
            .all(|source| source.archive_index.is_some()));
        let _ = fs::remove_file(super::archive_path(&path, 1).unwrap());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn read_log_health_summarizes_modules_sources_and_latest_problem() {
        let path = temp_log_path("health");
        write_events(
            &super::archive_path(&path, 1).unwrap(),
            vec![event_with_owner_level(
                "kernel.undo_redo.macro.canceled",
                KernelEventKind::UndoFailed,
                KernelLogLevel::Warn,
                "undo_redo",
            )],
        );
        write_events(
            &path,
            vec![
                event_with_owner_level(
                    "kernel.command.started",
                    KernelEventKind::CommandStarted,
                    KernelLogLevel::Info,
                    "command_engine",
                ),
                event_with_owner_level(
                    "kernel.command.failed",
                    KernelEventKind::CommandFailed,
                    KernelLogLevel::Error,
                    "command_engine",
                ),
            ],
        );

        let snapshot = read_kernel_observability_log_file(
            &path,
            KernelObservabilityLogRequest {
                limit: Some(10),
                recovery_only: Some(false),
                include_archives: Some(true),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(
            snapshot.health.status,
            KernelObservabilityHealthStatus::Error
        );
        assert_eq!(snapshot.health.event_count, 3);
        assert_eq!(snapshot.health.level_counts.info, 1);
        assert_eq!(snapshot.health.level_counts.warn, 1);
        assert_eq!(snapshot.health.level_counts.error, 1);
        assert_eq!(snapshot.health.source_counts.active, 2);
        assert_eq!(snapshot.health.source_counts.archived, 1);
        assert_eq!(snapshot.health.module_count, 2);
        assert_eq!(snapshot.health.modules[0].owner, "command_engine");
        assert_eq!(
            snapshot.health.modules[0].status,
            KernelObservabilityHealthStatus::Error
        );
        assert_eq!(
            snapshot.health.latest_problem.as_ref().unwrap().event_name,
            "kernel.command.failed"
        );
        assert_eq!(
            snapshot
                .health
                .latest_problem
                .as_ref()
                .unwrap()
                .source_label,
            "kernel.jsonl"
        );
        let _ = fs::remove_file(super::archive_path(&path, 1).unwrap());
        let _ = fs::remove_file(path);
    }

    fn event(event_name: &str, kind: KernelEventKind) -> KernelLogEvent {
        event_with_level(event_name, kind, KernelLogLevel::Info)
    }

    fn event_with_level(
        event_name: &str,
        kind: KernelEventKind,
        level: KernelLogLevel,
    ) -> KernelLogEvent {
        event_with_owner_level(event_name, kind, level, "test")
    }

    fn event_with_owner_level(
        event_name: &str,
        kind: KernelEventKind,
        level: KernelLogLevel,
        owner: &str,
    ) -> KernelLogEvent {
        let mut event =
            KernelLogEvent::new(level, kind, owner, "test", "test", None, "test event", None);
        event.event_name = event_name.to_string();
        event
    }

    fn write_events(path: &PathBuf, events: Vec<KernelLogEvent>) {
        let body = events
            .into_iter()
            .map(|event| serde_json::to_string(&event).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(path, format!("{body}\n")).unwrap();
    }

    fn temp_log_path(label: &str) -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("pana-kernel-observability-{id}-{label}.jsonl"))
    }
}
