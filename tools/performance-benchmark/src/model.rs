use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const RUN_SCHEMA_VERSION: u32 = 1;
pub const SAMPLE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentSnapshot {
    pub schema_version: u32,
    pub captured_unix_ms: u128,
    pub git_commit: Option<String>,
    pub git_worktree_dirty: bool,
    pub git_worktree_digest_sha256: String,
    pub rustc_version: Option<String>,
    pub cargo_version: Option<String>,
    pub node_version: Option<String>,
    pub zola_version: Option<String>,
    pub kernel: Option<String>,
    pub cpu_model: Option<String>,
    pub logical_cpu_count: usize,
    pub memory_total_kib: Option<u64>,
    pub memory_available_kib: Option<u64>,
    pub swap_total_kib: Option<u64>,
    pub swap_free_kib: Option<u64>,
    pub load_average: Option<String>,
    pub cpu_governors: Vec<String>,
    pub temperatures_millidegrees_celsius: Vec<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixtureSnapshot {
    pub schema_version: u32,
    pub profile: String,
    pub source_root: String,
    pub project_root: String,
    pub sha256: String,
    pub file_count: usize,
    pub directory_count: usize,
    pub total_bytes: u64,
    pub expected_outcome: String,
    pub source_manifest: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricSample {
    pub schema_version: u32,
    pub recorded_unix_ms: u128,
    pub layer: String,
    pub scenario: String,
    pub profile: String,
    pub mode: String,
    pub metric: String,
    pub value: f64,
    pub unit: String,
    pub iteration: usize,
    pub status: String,
    #[serde(default)]
    pub attributes: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Distribution {
    pub sample_count: usize,
    pub p50: f64,
    pub p95: f64,
    pub p99: Option<f64>,
    pub max: f64,
    pub mean: f64,
    pub minimum: f64,
    pub unit: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricSummary {
    pub layer: String,
    pub scenario: String,
    pub profile: String,
    pub mode: String,
    pub metric: String,
    pub distribution: Distribution,
    pub failed_samples: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetVerdict {
    pub layer: String,
    pub scenario: String,
    pub profile: String,
    pub mode: String,
    pub metric: String,
    pub statistic: String,
    pub actual: f64,
    pub budget: f64,
    pub unit: String,
    pub status: String,
    pub rationale: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComparisonVerdict {
    pub layer: String,
    pub scenario: String,
    pub profile: String,
    pub mode: String,
    pub metric: String,
    pub baseline_p95: f64,
    pub candidate_p95: f64,
    pub delta_percent: f64,
    pub noise_margin_percent: f64,
    pub regression_threshold_percent: f64,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkEnvironmentIdentity {
    pub rustc_version: Option<String>,
    pub cargo_version: Option<String>,
    pub node_version: Option<String>,
    pub zola_version: Option<String>,
    pub kernel: Option<String>,
    pub cpu_model: Option<String>,
    pub logical_cpu_count: usize,
    pub memory_total_kib: Option<u64>,
    pub cpu_governors: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkIdentity {
    pub protocol_version: u32,
    pub sample_schema_version: u32,
    pub suite: String,
    pub run_id: String,
    pub fixture_sha256: BTreeMap<String, String>,
    pub environment: BenchmarkEnvironmentIdentity,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandMeasurement {
    pub status: i32,
    pub wall_ms: f64,
    pub peak_rss_kib: u64,
    pub peak_pss_kib: u64,
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub cpu_ticks: u64,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunManifest {
    pub schema_version: u32,
    pub run_id: String,
    pub suite: String,
    pub started_unix_ms: u128,
    pub completed_unix_ms: Option<u128>,
    pub status: String,
    pub environment: EnvironmentSnapshot,
    pub fixtures: Vec<FixtureSnapshot>,
    pub raw_samples_path: String,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceReport {
    pub schema_version: u32,
    pub benchmark_identity: BenchmarkIdentity,
    pub raw_sample_count: usize,
    pub summaries: Vec<MetricSummary>,
    pub aspirational_budgets: Vec<BudgetVerdict>,
    pub budget_failure_count: usize,
    pub comparison_baseline: Option<String>,
    pub comparisons: Vec<ComparisonVerdict>,
    pub regression_count: usize,
}
