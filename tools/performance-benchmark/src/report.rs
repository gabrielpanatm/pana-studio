use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, BufWriter, Write},
    path::Path,
};

use crate::model::{
    BenchmarkEnvironmentIdentity, BenchmarkIdentity, BudgetVerdict, ComparisonVerdict,
    Distribution, MetricSample, MetricSummary, PerformanceReport, RunManifest, RUN_SCHEMA_VERSION,
    SAMPLE_SCHEMA_VERSION,
};

const COMPARISON_MIN_SAMPLES: usize = 10;
const BENCHMARK_PROTOCOL_VERSION: u32 = 2;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComparableReport {
    benchmark_identity: BenchmarkIdentity,
    summaries: Vec<MetricSummary>,
}

pub struct RawSampleWriter {
    writer: BufWriter<File>,
}

impl RawSampleWriter {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|error| error.to_string())?;
        Ok(Self {
            writer: BufWriter::new(file),
        })
    }

    pub fn record(&mut self, sample: &MetricSample) -> Result<(), String> {
        serde_json::to_writer(&mut self.writer, sample).map_err(|error| error.to_string())?;
        self.writer
            .write_all(b"\n")
            .map_err(|error| error.to_string())?;
        self.writer.flush().map_err(|error| error.to_string())
    }
}

fn percentile(sorted: &[f64], ratio: f64) -> f64 {
    let index = ((sorted.len() as f64 * ratio).ceil() as usize)
        .saturating_sub(1)
        .min(sorted.len().saturating_sub(1));
    sorted[index]
}

fn rounded(value: f64) -> f64 {
    (value * 1_000.0).round() / 1_000.0
}

pub fn distribution(values: &[f64], unit: &str) -> Option<Distribution> {
    let mut sorted = values
        .iter()
        .copied()
        .filter(|value| value.is_finite() && *value >= 0.0)
        .collect::<Vec<_>>();
    if sorted.is_empty() {
        return None;
    }
    sorted.sort_by(f64::total_cmp);
    Some(Distribution {
        sample_count: sorted.len(),
        p50: rounded(percentile(&sorted, 0.50)),
        p95: rounded(percentile(&sorted, 0.95)),
        p99: (sorted.len() >= 100).then(|| rounded(percentile(&sorted, 0.99))),
        max: rounded(*sorted.last().unwrap_or(&0.0)),
        mean: rounded(sorted.iter().sum::<f64>() / sorted.len() as f64),
        minimum: rounded(sorted[0]),
        unit: unit.to_string(),
    })
}

pub fn read_samples(path: &Path) -> Result<Vec<MetricSample>, String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    BufReader::new(file)
        .lines()
        .enumerate()
        .filter_map(|(index, line)| match line {
            Ok(line) if line.trim().is_empty() => None,
            Ok(line) => Some(
                serde_json::from_str(&line)
                    .map_err(|error| format!("JSONL linia {}: {error}", index + 1)),
            ),
            Err(error) => Some(Err(error.to_string())),
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .enumerate()
        .map(|(index, sample)| {
            validate_sample(&sample)
                .map_err(|error| format!("JSONL linia {}: {error}", index + 1))?;
            Ok(sample)
        })
        .collect()
}

fn validate_sample(sample: &MetricSample) -> Result<(), String> {
    if sample.schema_version != SAMPLE_SCHEMA_VERSION {
        return Err(format!(
            "schemaVersion {} necunoscut",
            sample.schema_version
        ));
    }
    for (name, value) in [
        ("layer", &sample.layer),
        ("scenario", &sample.scenario),
        ("profile", &sample.profile),
        ("mode", &sample.mode),
        ("metric", &sample.metric),
        ("unit", &sample.unit),
        ("status", &sample.status),
    ] {
        if value.trim().is_empty() {
            return Err(format!("{name} este gol"));
        }
    }
    if !sample.value.is_finite() || sample.value < 0.0 {
        return Err("value trebuie să fie finit și pozitiv".to_string());
    }
    Ok(())
}

pub fn summarize(samples: &[MetricSample]) -> Vec<MetricSummary> {
    type Key = (String, String, String, String, String, String);
    let mut groups = BTreeMap::<Key, Vec<&MetricSample>>::new();
    for sample in samples {
        groups
            .entry((
                sample.layer.clone(),
                sample.scenario.clone(),
                sample.profile.clone(),
                sample.mode.clone(),
                sample.metric.clone(),
                sample.unit.clone(),
            ))
            .or_default()
            .push(sample);
    }
    groups
        .into_iter()
        .filter_map(
            |((layer, scenario, profile, mode, metric, unit), samples)| {
                let values = samples
                    .iter()
                    .map(|sample| sample.value)
                    .collect::<Vec<_>>();
                Some(MetricSummary {
                    layer,
                    scenario,
                    profile,
                    mode,
                    metric,
                    distribution: distribution(&values, &unit)?,
                    failed_samples: samples
                        .iter()
                        .filter(|sample| sample.status != "ok")
                        .count(),
                })
            },
        )
        .collect()
}

fn budget(summary: &MetricSummary) -> Option<(f64, &'static str)> {
    if summary.distribution.unit != "ms" {
        return None;
    }
    match (
        summary.layer.as_str(),
        summary.scenario.as_str(),
        summary.mode.as_str(),
        summary.metric.as_str(),
    ) {
        ("ui", "activity_switch", "warm", "action_to_accessibility_state") => {
            Some((100.0, "Schimbare activitate/document p95 ≤ 100 ms"))
        }
        ("ui", "activity_switch_sustained", "sustained", "action_to_accessibility_state") => {
            Some((50.0, "Acțiune frecventă p95 ≤ 50 ms"))
        }
        ("rendering", "activity_switch", "warm", "input_to_two_raf") => {
            Some((100.0, "Input-to-paint pentru activitate p95 ≤ 100 ms"))
        }
        ("rendering", "workspace_frames", "sustained", "frame_delta") => {
            Some((16.7, "Cadru fluid p95 ≤ 16,7 ms"))
        }
        ("interactions", "text_edit", "sustained", "input_to_patch_ack") => {
            Some((50.0, "Editare frecventă p95 ≤ 50 ms"))
        }
        ("interactions", "selection_overlay", "sustained", "render_duration") => {
            Some((16.7, "Proiecție selecție într-un cadru p95 ≤ 16,7 ms"))
        }
        ("interactions", "undo_redo", "sustained", "input_to_patch_ack") => {
            Some((100.0, "Undo/Redo p95 ≤ 100 ms"))
        }
        ("interactions", "structural_drag_drop", "warm", "input_to_projection") => {
            Some((50.0, "Proiecție drag/drop p95 ≤ 50 ms"))
        }
        ("interactions", "code_to_code", "warm", "input_to_tab_selected") => {
            Some((100.0, "Activare tab document p95 ≤ 100 ms"))
        }
        ("interactions", "code_to_code", "warm", "input_to_document_ready") => {
            Some((100.0, "Document code gata de utilizare p95 ≤ 100 ms"))
        }
        (
            "interactions",
            "canonical_template_reactivation",
            "warm",
            "input_to_tab_selected",
        ) => Some((100.0, "Activare tab template p95 ≤ 100 ms")),
        (
            "interactions",
            "canonical_template_reactivation",
            "warm",
            "input_to_document_ready",
        ) => Some((500.0, "Reactivare template canonic p95 ≤ 500 ms")),
        (
            "interactions",
            "rapid_document_alternation",
            "warm",
            "input_to_tab_selected",
        ) => Some((100.0, "Activare tab latest-wins p95 ≤ 100 ms")),
        (
            "interactions",
            "rapid_document_alternation",
            "warm",
            "input_to_document_ready",
        ) => {
            Some((500.0, "Settlement latest-wins p95 ≤ 500 ms"))
        }
        ("interactions", "pane_tab_switch", "warm", "input_to_two_raf") => {
            Some((100.0, "Schimbare tab p95 ≤ 100 ms"))
        }
        ("interactions", "inspector_toggle", "warm", "input_to_two_raf") => {
            Some((50.0, "Comutare Inspector p95 ≤ 50 ms"))
        }
        ("frontend", "reactive_workspace_layout", "warm", "flush_sync") => {
            Some((50.0, "Actualizare reactivă p95 ≤ 50 ms"))
        }
        ("ui", "project_open", "cold_process", "canvas_accessible") => {
            Some((2_000.0, "Canvas cold p95 ≤ 2 s"))
        }
        ("ui", "project_open", "warm_process", "canvas_accessible") => {
            Some((1_000.0, "Canvas warm p95 ≤ 1 s"))
        }
        _ => None,
    }
}

pub fn evaluate_budgets(summaries: &[MetricSummary]) -> Vec<BudgetVerdict> {
    summaries
        .iter()
        .filter_map(|summary| {
            let (limit, rationale) = budget(summary)?;
            let actual = summary.distribution.p95;
            Some(BudgetVerdict {
                layer: summary.layer.clone(),
                scenario: summary.scenario.clone(),
                profile: summary.profile.clone(),
                mode: summary.mode.clone(),
                metric: summary.metric.clone(),
                statistic: "p95".to_string(),
                actual,
                budget: limit,
                unit: summary.distribution.unit.clone(),
                status: if summary.failed_samples == 0 && actual <= limit {
                    "pass".to_string()
                } else {
                    "fail".to_string()
                },
                rationale: rationale.to_string(),
            })
        })
        .collect()
}

fn summary_key(summary: &MetricSummary) -> (&str, &str, &str, &str, &str, &str) {
    (
        &summary.layer,
        &summary.scenario,
        &summary.profile,
        &summary.mode,
        &summary.metric,
        &summary.distribution.unit,
    )
}

fn noise_percent(summary: &MetricSummary) -> f64 {
    if summary.distribution.p50 <= f64::EPSILON {
        return 5.0;
    }
    let spread = ((summary.distribution.p95 - summary.distribution.p50) / summary.distribution.p50
        * 100.0)
        .max(0.0);
    (spread * 0.1).clamp(2.5, 5.0)
}

pub fn compare_summaries(
    baseline: &[MetricSummary],
    candidate: &[MetricSummary],
) -> Vec<ComparisonVerdict> {
    let baseline_by_key = baseline
        .iter()
        .map(|summary| (summary_key(summary), summary))
        .collect::<BTreeMap<_, _>>();
    candidate
        .iter()
        .filter_map(|current| {
            let previous = baseline_by_key.get(&summary_key(current))?;
            if previous.distribution.sample_count < COMPARISON_MIN_SAMPLES
                || current.distribution.sample_count < COMPARISON_MIN_SAMPLES
            {
                return None;
            }
            if previous.distribution.p95 <= f64::EPSILON {
                return None;
            }
            let delta = (current.distribution.p95 - previous.distribution.p95)
                / previous.distribution.p95
                * 100.0;
            let noise = noise_percent(previous).max(noise_percent(current));
            let threshold = 10.0 + noise;
            let status = if delta > threshold {
                "regression"
            } else if delta < -threshold {
                "improvement"
            } else {
                "stable"
            };
            Some(ComparisonVerdict {
                layer: current.layer.clone(),
                scenario: current.scenario.clone(),
                profile: current.profile.clone(),
                mode: current.mode.clone(),
                metric: current.metric.clone(),
                baseline_p95: previous.distribution.p95,
                candidate_p95: current.distribution.p95,
                delta_percent: rounded(delta),
                noise_margin_percent: rounded(noise),
                regression_threshold_percent: rounded(threshold),
                status: status.to_string(),
            })
        })
        .collect()
}

fn benchmark_identity(raw_path: &Path) -> Result<BenchmarkIdentity, String> {
    let run_path = raw_path
        .parent()
        .map(|parent| parent.join("run.json"))
        .ok_or_else(|| "Calea JSONL nu are un director părinte pentru run.json.".to_string())?;
    if !run_path.is_file() {
        return Err(format!(
            "Lipsește {}: raportul necesită identitatea suitei și fixture-urilor.",
            run_path.display()
        ));
    }
    let source = fs::read_to_string(&run_path)
        .map_err(|error| format!("Identitatea benchmarkului nu poate fi citită: {error}"))?;
    let run: RunManifest = serde_json::from_str(&source)
        .map_err(|error| format!("Identitatea benchmarkului este invalidă: {error}"))?;
    Ok(BenchmarkIdentity {
        protocol_version: BENCHMARK_PROTOCOL_VERSION,
        sample_schema_version: SAMPLE_SCHEMA_VERSION,
        suite: run.suite,
        run_id: run.run_id,
        fixture_sha256: run
            .fixtures
            .into_iter()
            .map(|fixture| (fixture.profile, fixture.sha256))
            .collect(),
        environment: BenchmarkEnvironmentIdentity {
            rustc_version: run.environment.rustc_version,
            cargo_version: run.environment.cargo_version,
            node_version: run.environment.node_version,
            zola_version: run.environment.zola_version,
            kernel: run.environment.kernel,
            cpu_model: run.environment.cpu_model,
            logical_cpu_count: run.environment.logical_cpu_count,
            memory_total_kib: run.environment.memory_total_kib,
            cpu_governors: run.environment.cpu_governors,
        },
    })
}

fn validate_baseline_compatibility(
    baseline: &BenchmarkIdentity,
    candidate: &BenchmarkIdentity,
) -> Result<(), String> {
    if baseline.protocol_version != candidate.protocol_version {
        return Err(format!(
            "Baseline incompatibil: protocol v{}, candidat v{}.",
            baseline.protocol_version, candidate.protocol_version
        ));
    }
    if baseline.sample_schema_version != candidate.sample_schema_version {
        return Err(format!(
            "Baseline incompatibil: schema probelor v{}, candidat v{}.",
            baseline.sample_schema_version, candidate.sample_schema_version
        ));
    }
    if baseline.suite != candidate.suite {
        return Err(format!(
            "Baseline incompatibil: suita `{}` nu poate fi comparată cu `{}`.",
            baseline.suite, candidate.suite
        ));
    }
    if baseline.fixture_sha256 != candidate.fixture_sha256 {
        return Err("Baseline incompatibil: SHA-256 al fixture-urilor diferă.".to_string());
    }
    if baseline.environment != candidate.environment {
        return Err(
            "Baseline incompatibil: hardware-ul, toolchain-ul sau governor-ul diferă.".to_string(),
        );
    }
    Ok(())
}

pub fn write_reports(
    raw_path: &Path,
    json_path: &Path,
    markdown_path: &Path,
) -> Result<(), String> {
    write_reports_with_baseline(raw_path, json_path, markdown_path, None)
}

pub fn write_reports_with_baseline(
    raw_path: &Path,
    json_path: &Path,
    markdown_path: &Path,
    baseline_path: Option<&Path>,
) -> Result<(), String> {
    let samples = read_samples(raw_path)?;
    let summaries = summarize(&samples);
    let budgets = evaluate_budgets(&summaries);
    let identity = benchmark_identity(raw_path)?;
    let comparisons = if let Some(path) = baseline_path {
        let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
        let baseline: ComparableReport =
            serde_json::from_str(&source).map_err(|error| error.to_string())?;
        validate_baseline_compatibility(&baseline.benchmark_identity, &identity)?;
        compare_summaries(&baseline.summaries, &summaries)
    } else {
        Vec::new()
    };
    let budget_failures = budgets.iter().filter(|item| item.status == "fail").count();
    let regressions = comparisons
        .iter()
        .filter(|item| item.status == "regression")
        .count();
    let report = PerformanceReport {
        schema_version: 1,
        benchmark_identity: identity,
        raw_sample_count: samples.len(),
        summaries,
        aspirational_budgets: budgets,
        budget_failure_count: budget_failures,
        comparison_baseline: baseline_path.map(|path| path.to_string_lossy().into_owned()),
        comparisons,
        regression_count: regressions,
    };
    fs::write(
        json_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?
        ),
    )
    .map_err(|error| error.to_string())?;
    let mut markdown = String::from("# Raport benchmark Pană Studio\n\n");
    markdown.push_str(&format!(
        "Protocol v{}, suită `{}`, run `{}`. Fixture-uri verificate: {}.\n\n",
        report.benchmark_identity.protocol_version,
        report.benchmark_identity.suite,
        report.benchmark_identity.run_id,
        report.benchmark_identity.fixture_sha256.len(),
    ));
    markdown.push_str(
        "| Strat | Scenariu | Profil | Mod | Metrică | Probe | p50 | p95 | p99 | Max | Eșecuri |\n| --- | --- | --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |\n",
    );
    for item in &report.summaries {
        let p99 = item
            .distribution
            .p99
            .map(|value| format!("{value:.3}"))
            .unwrap_or_else(|| "—".to_string());
        markdown.push_str(&format!(
            "| {} | {} | {} | {} | {} ({}) | {} | {:.3} | {:.3} | {} | {:.3} | {} |\n",
            item.layer,
            item.scenario,
            item.profile,
            item.mode,
            item.metric,
            item.distribution.unit,
            item.distribution.sample_count,
            item.distribution.p50,
            item.distribution.p95,
            p99,
            item.distribution.max,
            item.failed_samples,
        ));
    }
    markdown.push_str(&format!(
        "\n## Bugete aspiraționale\n\nEșecuri: **{budget_failures}**. Aceste praguri nu invalidează probele brute.\n\n| Scenariu | Profil | Metrică | p95 | Buget | Verdict |\n| --- | --- | --- | ---: | ---: | --- |\n"
    ));
    for item in &report.aspirational_budgets {
        markdown.push_str(&format!(
            "| {} | {} | {} | {:.3} {} | {:.3} {} | {} |\n",
            item.scenario,
            item.profile,
            item.metric,
            item.actual,
            item.unit,
            item.budget,
            item.unit,
            item.status,
        ));
    }
    markdown.push_str(&format!(
        "\n## Comparație cu baseline\n\nSunt comparate numai distribuțiile cu minimum {COMPARISON_MIN_SAMPLES} probe în ambele rulări. Regresii peste 10% plus marja de zgomot: **{regressions}**.\n\n"
    ));
    if report.comparisons.is_empty() {
        markdown.push_str("Nu a fost furnizat un baseline comparabil.\n");
    } else {
        markdown.push_str("| Scenariu | Profil | Metrică | Baseline p95 | Candidat p95 | Delta | Zgomot | Verdict |\n| --- | --- | --- | ---: | ---: | ---: | ---: | --- |\n");
        for item in &report.comparisons {
            markdown.push_str(&format!(
                "| {} | {} | {} | {:.3} | {:.3} | {:+.3}% | {:.3}% | {} |\n",
                item.scenario,
                item.profile,
                item.metric,
                item.baseline_p95,
                item.candidate_p95,
                item.delta_percent,
                item.noise_margin_percent,
                item.status,
            ));
        }
    }
    fs::write(markdown_path, markdown).map_err(|error| error.to_string())?;
    let serialized = fs::read_to_string(json_path).map_err(|error| error.to_string())?;
    let decoded: PerformanceReport = serde_json::from_str(&serialized)
        .map_err(|error| format!("Raport JSON invalid: {error}"))?;
    if decoded.schema_version != 1
        || decoded.raw_sample_count != samples.len()
        || decoded.budget_failure_count
            != decoded
                .aspirational_budgets
                .iter()
                .filter(|item| item.status == "fail")
                .count()
        || decoded.regression_count
            != decoded
                .comparisons
                .iter()
                .filter(|item| item.status == "regression")
                .count()
    {
        return Err("Raportul JSON nu respectă invariabilele schemei v1.".to_string());
    }
    Ok(())
}

pub fn validate_artifact_set(
    run_path: &Path,
    raw_path: &Path,
    report_path: &Path,
) -> Result<(), String> {
    let run_source = fs::read_to_string(run_path).map_err(|error| error.to_string())?;
    let run: RunManifest = serde_json::from_str(&run_source)
        .map_err(|error| format!("Manifest run JSON invalid: {error}"))?;
    if run.schema_version != RUN_SCHEMA_VERSION
        || run.run_id.trim().is_empty()
        || run.completed_unix_ms.is_none()
        || !matches!(
            run.status.as_str(),
            "complete" | "complete_with_diagnostics" | "failed"
        )
    {
        return Err("Manifestul run nu respectă invariabilele schemei v1.".to_string());
    }
    let samples = read_samples(raw_path)?;
    let report_source = fs::read_to_string(report_path).map_err(|error| error.to_string())?;
    let report: PerformanceReport = serde_json::from_str(&report_source)
        .map_err(|error| format!("Raport JSON invalid: {error}"))?;
    let expected_identity = BenchmarkIdentity {
        protocol_version: BENCHMARK_PROTOCOL_VERSION,
        sample_schema_version: SAMPLE_SCHEMA_VERSION,
        suite: run.suite.clone(),
        run_id: run.run_id.clone(),
        fixture_sha256: run
            .fixtures
            .iter()
            .map(|fixture| (fixture.profile.clone(), fixture.sha256.clone()))
            .collect(),
        environment: BenchmarkEnvironmentIdentity {
            rustc_version: run.environment.rustc_version.clone(),
            cargo_version: run.environment.cargo_version.clone(),
            node_version: run.environment.node_version.clone(),
            zola_version: run.environment.zola_version.clone(),
            kernel: run.environment.kernel.clone(),
            cpu_model: run.environment.cpu_model.clone(),
            logical_cpu_count: run.environment.logical_cpu_count,
            memory_total_kib: run.environment.memory_total_kib,
            cpu_governors: run.environment.cpu_governors.clone(),
        },
    };
    if report.schema_version != 1
        || report.benchmark_identity != expected_identity
        || report.raw_sample_count != samples.len()
        || report.summaries.is_empty()
        || run.raw_samples_path != raw_path.to_string_lossy()
    {
        return Err("Setul run/JSONL/report este inconsistent.".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(p50: f64, p95: f64) -> MetricSummary {
        MetricSummary {
            layer: "rendering".to_string(),
            scenario: "workspace_frames".to_string(),
            profile: "mare".to_string(),
            mode: "sustained".to_string(),
            metric: "frame_delta".to_string(),
            distribution: Distribution {
                sample_count: 100,
                p50,
                p95,
                p99: Some(p95),
                max: p95,
                mean: p50,
                minimum: p50,
                unit: "ms".to_string(),
            },
            failed_samples: 0,
        }
    }

    fn identity(suite: &str, fixture_sha256: &str) -> BenchmarkIdentity {
        BenchmarkIdentity {
            protocol_version: 1,
            sample_schema_version: 1,
            suite: suite.to_string(),
            run_id: format!("{suite}-test"),
            fixture_sha256: BTreeMap::from([("control".to_string(), fixture_sha256.to_string())]),
            environment: BenchmarkEnvironmentIdentity {
                rustc_version: Some("rustc-test".to_string()),
                cargo_version: Some("cargo-test".to_string()),
                node_version: Some("node-test".to_string()),
                zola_version: Some("zola-test".to_string()),
                kernel: Some("kernel-test".to_string()),
                cpu_model: Some("cpu-test".to_string()),
                logical_cpu_count: 12,
                memory_total_kib: Some(16_000_000),
                cpu_governors: vec!["schedutil".to_string()],
            },
        }
    }

    #[test]
    fn p99_is_only_reported_for_at_least_one_hundred_samples() {
        assert_eq!(
            distribution(&(1..=99).map(f64::from).collect::<Vec<_>>(), "ms")
                .unwrap()
                .p99,
            None
        );
        assert_eq!(
            distribution(&(1..=100).map(f64::from).collect::<Vec<_>>(), "ms")
                .unwrap()
                .p99,
            Some(99.0)
        );
    }

    #[test]
    fn budgets_are_aspirational_and_regressions_include_noise_margin() {
        let candidate = summary(16.0, 18.0);
        let verdict = evaluate_budgets(std::slice::from_ref(&candidate));
        assert_eq!(verdict.len(), 1);
        assert_eq!(verdict[0].status, "fail");

        let comparisons = compare_summaries(&[summary(10.0, 10.0)], &[summary(10.0, 12.0)]);
        assert_eq!(comparisons.len(), 1);
        assert_eq!(comparisons[0].noise_margin_percent, 2.5);
        assert_eq!(comparisons[0].status, "regression");
    }

    #[test]
    fn comparisons_ignore_singleton_and_short_distributions() {
        let mut baseline = summary(10.0, 10.0);
        let mut candidate = summary(10.0, 20.0);
        baseline.distribution.sample_count = COMPARISON_MIN_SAMPLES - 1;
        candidate.distribution.sample_count = COMPARISON_MIN_SAMPLES;
        assert!(compare_summaries(&[baseline], &[candidate]).is_empty());
    }

    #[test]
    fn baseline_requires_the_same_suite_protocol_and_fixtures() {
        let standard = identity("standard", "fixture-a");
        let same = BenchmarkIdentity {
            run_id: "standard-candidate".to_string(),
            ..standard.clone()
        };
        assert!(validate_baseline_compatibility(&standard, &same).is_ok());

        let soak = identity("soak", "fixture-a");
        assert!(validate_baseline_compatibility(&standard, &soak)
            .unwrap_err()
            .contains("suita `standard`"));

        let changed_fixture = identity("standard", "fixture-b");
        assert!(validate_baseline_compatibility(&standard, &changed_fixture)
            .unwrap_err()
            .contains("fixture-urilor"));

        let mut changed_environment = standard.clone();
        changed_environment.environment.kernel = Some("kernel-next".to_string());
        assert!(
            validate_baseline_compatibility(&standard, &changed_environment)
                .unwrap_err()
                .contains("toolchain-ul")
        );
    }

    #[test]
    fn all_versioned_schema_documents_are_valid_json() {
        for source in [
            include_str!("../../../benchmarks/schema/performance-sample-v1.schema.json"),
            include_str!("../../../benchmarks/schema/performance-run-v1.schema.json"),
            include_str!("../../../benchmarks/schema/performance-report-v1.schema.json"),
        ] {
            let schema: serde_json::Value = serde_json::from_str(source).unwrap();
            assert_eq!(
                schema.get("$schema").and_then(serde_json::Value::as_str),
                Some("https://json-schema.org/draft/2020-12/schema")
            );
            assert!(schema.get("required").is_some());
        }
    }
}
