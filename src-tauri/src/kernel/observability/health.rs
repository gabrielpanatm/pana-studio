use std::collections::BTreeMap;

use serde::Serialize;

use super::{KernelLogEvent, KernelLogLevel};

const MAX_HEALTH_MODULES: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelObservabilityHealthStatus {
    Clean,
    Warning,
    Error,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelObservabilityLevelCounts {
    pub info: usize,
    pub warn: usize,
    pub error: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelObservabilitySourceCounts {
    pub active: usize,
    pub archived: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelObservabilityHealthProblemSnapshot {
    pub event_id: String,
    pub event_name: String,
    pub owner: String,
    pub level: KernelLogLevel,
    pub severity_text: String,
    pub timestamp_ms: u128,
    pub message: String,
    pub source_label: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelObservabilityModuleHealthSnapshot {
    pub owner: String,
    pub status: KernelObservabilityHealthStatus,
    pub event_count: usize,
    pub recovery_count: usize,
    pub level_counts: KernelObservabilityLevelCounts,
    pub latest_event_name: Option<String>,
    pub latest_timestamp_ms: Option<u128>,
    pub latest_severity_text: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelObservabilityHealthSnapshot {
    pub status: KernelObservabilityHealthStatus,
    pub event_count: usize,
    pub recovery_count: usize,
    pub level_counts: KernelObservabilityLevelCounts,
    pub source_counts: KernelObservabilitySourceCounts,
    pub module_count: usize,
    pub modules: Vec<KernelObservabilityModuleHealthSnapshot>,
    pub latest_problem: Option<KernelObservabilityHealthProblemSnapshot>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct KernelObservabilityHealthAccumulator {
    event_count: usize,
    recovery_count: usize,
    level_counts: KernelObservabilityLevelCounts,
    source_counts: KernelObservabilitySourceCounts,
    modules: BTreeMap<String, ModuleAccumulator>,
    latest_problem: Option<KernelObservabilityHealthProblemSnapshot>,
}

impl KernelObservabilityHealthAccumulator {
    pub(crate) fn ingest_event(
        &mut self,
        event: &KernelLogEvent,
        source_active: bool,
        source_label: &str,
    ) {
        self.event_count += 1;
        self.level_counts.record(event.level);
        if source_active {
            self.source_counts.active += 1;
        } else {
            self.source_counts.archived += 1;
        }
        if is_recovery_event(event) {
            self.recovery_count += 1;
        }

        let owner = normalized_owner(event);
        self.modules
            .entry(owner.clone())
            .or_default()
            .ingest_event(event);
        self.record_problem(event, source_label);
    }

    pub(crate) fn into_snapshot(
        self,
        unreadable_count: usize,
        truncated: bool,
    ) -> KernelObservabilityHealthSnapshot {
        let mut modules = self
            .modules
            .into_iter()
            .map(|(owner, module)| module.into_snapshot(owner))
            .collect::<Vec<_>>();
        let module_count = modules.len();
        modules.sort_by(|left, right| {
            status_rank(right.status)
                .cmp(&status_rank(left.status))
                .then_with(|| right.event_count.cmp(&left.event_count))
                .then_with(|| left.owner.cmp(&right.owner))
        });
        modules.truncate(MAX_HEALTH_MODULES);

        KernelObservabilityHealthSnapshot {
            status: health_status(&self.level_counts, unreadable_count, truncated),
            event_count: self.event_count,
            recovery_count: self.recovery_count,
            level_counts: self.level_counts,
            source_counts: self.source_counts,
            module_count,
            modules,
            latest_problem: self.latest_problem,
        }
    }

    fn record_problem(&mut self, event: &KernelLogEvent, source_label: &str) {
        if !matches!(event.level, KernelLogLevel::Warn | KernelLogLevel::Error) {
            return;
        }
        let problem = KernelObservabilityHealthProblemSnapshot {
            event_id: event.id.clone(),
            event_name: event.event_name.clone(),
            owner: normalized_owner(event),
            level: event.level,
            severity_text: event.severity_text.clone(),
            timestamp_ms: event.timestamp_ms,
            message: event.message.clone(),
            source_label: source_label.to_string(),
        };
        if self
            .latest_problem
            .as_ref()
            .map(|current| problem_rank(&problem) >= problem_rank(current))
            .unwrap_or(true)
        {
            self.latest_problem = Some(problem);
        }
    }
}

#[derive(Clone, Debug, Default)]
struct ModuleAccumulator {
    event_count: usize,
    recovery_count: usize,
    level_counts: KernelObservabilityLevelCounts,
    latest_event_name: Option<String>,
    latest_timestamp_ms: Option<u128>,
    latest_severity_text: Option<String>,
}

impl ModuleAccumulator {
    fn ingest_event(&mut self, event: &KernelLogEvent) {
        self.event_count += 1;
        self.level_counts.record(event.level);
        if is_recovery_event(event) {
            self.recovery_count += 1;
        }
        if self
            .latest_timestamp_ms
            .map(|current| event.timestamp_ms >= current)
            .unwrap_or(true)
        {
            self.latest_timestamp_ms = Some(event.timestamp_ms);
            self.latest_event_name = Some(event.event_name.clone());
            self.latest_severity_text = Some(event.severity_text.clone());
        }
    }

    fn into_snapshot(self, owner: String) -> KernelObservabilityModuleHealthSnapshot {
        let status = module_status(&self.level_counts);
        KernelObservabilityModuleHealthSnapshot {
            owner,
            status,
            event_count: self.event_count,
            recovery_count: self.recovery_count,
            level_counts: self.level_counts,
            latest_event_name: self.latest_event_name,
            latest_timestamp_ms: self.latest_timestamp_ms,
            latest_severity_text: self.latest_severity_text,
        }
    }
}

impl KernelObservabilityLevelCounts {
    fn record(&mut self, level: KernelLogLevel) {
        match level {
            KernelLogLevel::Info => self.info += 1,
            KernelLogLevel::Warn => self.warn += 1,
            KernelLogLevel::Error => self.error += 1,
        }
    }
}

pub(crate) fn is_recovery_event(event: &KernelLogEvent) -> bool {
    event.event_name.contains(".recovery.")
        || event.event_name.ends_with(".recovery_required")
        || event.event_name.ends_with(".recovered")
}

fn health_status(
    counts: &KernelObservabilityLevelCounts,
    unreadable_count: usize,
    truncated: bool,
) -> KernelObservabilityHealthStatus {
    if counts.error > 0 || unreadable_count > 0 {
        KernelObservabilityHealthStatus::Error
    } else if counts.warn > 0 || truncated {
        KernelObservabilityHealthStatus::Warning
    } else {
        KernelObservabilityHealthStatus::Clean
    }
}

fn module_status(counts: &KernelObservabilityLevelCounts) -> KernelObservabilityHealthStatus {
    if counts.error > 0 {
        KernelObservabilityHealthStatus::Error
    } else if counts.warn > 0 {
        KernelObservabilityHealthStatus::Warning
    } else {
        KernelObservabilityHealthStatus::Clean
    }
}

fn status_rank(status: KernelObservabilityHealthStatus) -> u8 {
    match status {
        KernelObservabilityHealthStatus::Clean => 0,
        KernelObservabilityHealthStatus::Warning => 1,
        KernelObservabilityHealthStatus::Error => 2,
    }
}

fn level_rank(level: KernelLogLevel) -> u8 {
    match level {
        KernelLogLevel::Info => 0,
        KernelLogLevel::Warn => 1,
        KernelLogLevel::Error => 2,
    }
}

fn problem_rank(problem: &KernelObservabilityHealthProblemSnapshot) -> (u8, u128) {
    (level_rank(problem.level), problem.timestamp_ms)
}

fn normalized_owner(event: &KernelLogEvent) -> String {
    if event.owner.trim().is_empty() {
        "unknown".to_string()
    } else {
        event.owner.clone()
    }
}
