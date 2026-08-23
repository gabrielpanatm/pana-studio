use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::{json, Value};
use walkdir::WalkDir;

use crate::{
    environment,
    fixture::{materialize_profile, remove_materialized_fixtures, verify_immutable_fixture},
    model::{
        CommandMeasurement, FixtureSnapshot, MetricSample, RunManifest, RUN_SCHEMA_VERSION,
        SAMPLE_SCHEMA_VERSION,
    },
    process::{run_measured, sample_process_tree, ProcessTreeSnapshot},
    report::{validate_artifact_set, write_reports, RawSampleWriter},
    suite::Suite,
};

pub struct RunOptions {
    pub project_root: PathBuf,
    pub canonical_fixture_root: PathBuf,
    pub output_root: PathBuf,
    pub suite: Suite,
    pub keep_fixtures: bool,
    pub app_binary: Option<PathBuf>,
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn run_id(suite: &str) -> String {
    format!("{suite}-{}-{}", unix_ms(), std::process::id())
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(
        path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(value).map_err(|error| error.to_string())?
        ),
    )
    .map_err(|error| error.to_string())
}

fn sample(
    layer: &str,
    scenario: &str,
    profile: &str,
    mode: &str,
    metric: &str,
    value: f64,
    unit: &str,
    iteration: usize,
    status: &str,
    attributes: BTreeMap<String, Value>,
) -> MetricSample {
    MetricSample {
        schema_version: SAMPLE_SCHEMA_VERSION,
        recorded_unix_ms: unix_ms(),
        layer: layer.to_string(),
        scenario: scenario.to_string(),
        profile: profile.to_string(),
        mode: mode.to_string(),
        metric: metric.to_string(),
        value,
        unit: unit.to_string(),
        iteration,
        status: status.to_string(),
        attributes,
    }
}

fn record_measurement(
    raw: &mut RawSampleWriter,
    layer: &str,
    scenario: &str,
    profile: &str,
    mode: &str,
    iteration: usize,
    measurement: &CommandMeasurement,
) -> Result<(), String> {
    let status = if measurement.status == 0 {
        "ok"
    } else {
        "failed"
    };
    let attributes = BTreeMap::from([("exitStatus".to_string(), json!(measurement.status))]);
    for (metric, value, unit) in [
        ("wall", measurement.wall_ms, "ms"),
        ("peak_rss", measurement.peak_rss_kib as f64, "KiB"),
        ("peak_pss", measurement.peak_pss_kib as f64, "KiB"),
        ("read", measurement.read_bytes as f64, "bytes"),
        ("write", measurement.write_bytes as f64, "bytes"),
        ("cpu", measurement.cpu_ticks as f64, "ticks"),
    ] {
        raw.record(&sample(
            layer,
            scenario,
            profile,
            mode,
            metric,
            value,
            unit,
            iteration,
            status,
            attributes.clone(),
        ))?;
    }
    Ok(())
}

fn command_paths(run_root: &Path, label: &str, iteration: usize) -> (PathBuf, PathBuf) {
    let stem = label.replace(['/', ' ', ':'], "-");
    (
        run_root
            .join("command-logs")
            .join(format!("{stem}-{iteration:04}.stdout.log")),
        run_root
            .join("command-logs")
            .join(format!("{stem}-{iteration:04}.stderr.log")),
    )
}

fn record_artifact_inventory(
    raw: &mut RawSampleWriter,
    scenario: &str,
    profile: &str,
    root: &Path,
) -> Result<(), String> {
    let mut files = 0_u64;
    let mut bytes = 0_u64;
    if root.is_file() {
        files = 1;
        bytes = root.metadata().map_err(|error| error.to_string())?.len();
    } else {
        for entry in WalkDir::new(root).follow_links(false).into_iter().flatten() {
            if entry.file_type().is_file() {
                files += 1;
                bytes = bytes
                    .saturating_add(entry.metadata().map_err(|error| error.to_string())?.len());
            }
        }
    }
    for (metric, value, unit) in [
        ("file_count", files as f64, "count"),
        ("total_bytes", bytes as f64, "bytes"),
    ] {
        raw.record(&sample(
            "build",
            scenario,
            profile,
            "artifact",
            metric,
            value,
            unit,
            0,
            "ok",
            BTreeMap::from([("path".to_string(), json!(root))]),
        ))?;
    }
    Ok(())
}

fn run_zola(
    project_root: &Path,
    run_root: &Path,
    fixture: &FixtureSnapshot,
    suite: Suite,
    raw: &mut RawSampleWriter,
    diagnostics: &mut Vec<String>,
) -> Result<(), String> {
    if fixture.expected_outcome != "accepted" {
        return Ok(());
    }
    let spec = suite.spec();
    let fixture_root = Path::new(&fixture.project_root);
    let mut run_command = |scenario: &str,
                           mode: &str,
                           iteration: usize,
                           arguments: Vec<String>|
     -> Result<(), String> {
        let label = format!("zola-{scenario}-{}", fixture.profile);
        let (stdout, stderr) = command_paths(run_root, &label, iteration);
        let measurement =
            run_measured(project_root, "zola", &arguments, &[], &stdout, &stderr, 10)?;
        record_measurement(
            raw,
            "build",
            scenario,
            &fixture.profile,
            mode,
            iteration,
            &measurement,
        )?;
        if measurement.status != 0 {
            diagnostics.push(format!(
                "Zola {scenario} a eșuat pentru {}: {}",
                fixture.profile,
                measurement.stderr.trim()
            ));
        }
        Ok(())
    };
    run_command(
        "check",
        "cold_process",
        0,
        vec![
            "--root".to_string(),
            fixture_root.to_string_lossy().into_owned(),
            "check".to_string(),
        ],
    )?;
    let total = spec.cold_samples + spec.warm_samples + spec.warmup_samples;
    for iteration in 0..total {
        let output = run_root
            .join("zola-output")
            .join(&fixture.profile)
            .join(format!("build-{iteration:04}"));
        let mode = if iteration < spec.warmup_samples {
            "warmup"
        } else if iteration < spec.warmup_samples + spec.cold_samples {
            "cold_process"
        } else {
            "filesystem_warm"
        };
        let measured_iteration = iteration.saturating_sub(spec.warmup_samples);
        let label = format!("zola-build-{}", fixture.profile);
        let (stdout, stderr) = command_paths(run_root, &label, iteration);
        let measurement = run_measured(
            project_root,
            "zola",
            &[
                "--root".to_string(),
                fixture_root.to_string_lossy().into_owned(),
                "build".to_string(),
                "--output-dir".to_string(),
                output.to_string_lossy().into_owned(),
                "--force".to_string(),
            ],
            &[],
            &stdout,
            &stderr,
            10,
        )?;
        if mode != "warmup" {
            record_measurement(
                raw,
                "build",
                "zola_build",
                &fixture.profile,
                mode,
                measured_iteration,
                &measurement,
            )?;
        }
        if measurement.status != 0 {
            diagnostics.push(format!(
                "Zola build a eșuat pentru {} la iterația {iteration}: {}",
                fixture.profile,
                measurement.stderr.trim()
            ));
        }
        if output.exists() {
            fs::remove_dir_all(output).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn parse_prefixed_json(source: &str) -> Vec<Value> {
    const PREFIX: &str = "[pana-performance] ";
    source
        .lines()
        .filter_map(|line| line.find(PREFIX).map(|index| &line[index + PREFIX.len()..]))
        .filter_map(|value| serde_json::from_str(value).ok())
        .collect()
}

fn record_kernel_payload(
    raw: &mut RawSampleWriter,
    fixture: &FixtureSnapshot,
    payload: &Value,
) -> Result<(), String> {
    let operation = payload
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let variant = payload
        .get("variant")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let values = payload
        .get("samplesUs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let contract_status = if operation == "startup_inspection" {
        let candidate_kind = payload
            .get("candidateKind")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let passed = if fixture.expected_outcome == "accepted" {
            candidate_kind == "valid_project"
                && payload.get("candidateTruncated").and_then(Value::as_bool) == Some(false)
        } else {
            candidate_kind == "invalid_zola_project"
                && payload.get("candidateTruncated").and_then(Value::as_bool) == Some(true)
        };
        let attributes = BTreeMap::from([
            (
                "expectedOutcome".to_string(),
                json!(fixture.expected_outcome),
            ),
            ("candidateKind".to_string(), json!(candidate_kind)),
            (
                "candidateTruncated".to_string(),
                payload
                    .get("candidateTruncated")
                    .cloned()
                    .unwrap_or(Value::Null),
            ),
            (
                "candidateEntryCount".to_string(),
                payload
                    .get("candidateEntryCount")
                    .cloned()
                    .unwrap_or(Value::Null),
            ),
            (
                "scanFileCount".to_string(),
                payload.get("scanFileCount").cloned().unwrap_or(Value::Null),
            ),
            (
                "diagnosticCodes".to_string(),
                payload
                    .get("diagnosticCodes")
                    .cloned()
                    .unwrap_or(Value::Null),
            ),
        ]);
        raw.record(&sample(
            "contract",
            "project_open_boundary",
            &fixture.profile,
            "fail_closed",
            "contract_pass",
            if passed { 1.0 } else { 0.0 },
            "boolean",
            0,
            if passed { "ok" } else { "contract_violation" },
            attributes,
        ))?;
        if candidate_kind == "valid_project" {
            let scan_file_count = payload
                .get("scanFileCount")
                .and_then(Value::as_u64)
                .unwrap_or_default();
            let projection_limited = scan_file_count < fixture.file_count as u64;
            raw.record(&sample(
                "limits",
                "file_explorer_projection",
                &fixture.profile,
                "snapshot",
                "published_entry_count",
                scan_file_count as f64,
                "count",
                0,
                if projection_limited {
                    "limit_reached"
                } else {
                    "ok"
                },
                BTreeMap::from([
                    ("fixtureFileCount".to_string(), json!(fixture.file_count)),
                    ("projectionLimit".to_string(), json!(500)),
                    ("projectionLimited".to_string(), json!(projection_limited)),
                ]),
            ))?;
        }
        if passed {
            "ok"
        } else {
            "contract_violation"
        }
    } else {
        "ok"
    };
    if !values.is_empty() {
        for (iteration, value) in values.iter().filter_map(Value::as_f64).enumerate() {
            raw.record(&sample(
                "kernel",
                operation,
                &fixture.profile,
                "warm",
                "duration",
                value / 1_000.0,
                "ms",
                iteration,
                contract_status,
                BTreeMap::from([("variant".to_string(), json!(variant))]),
            ))?;
        }
    } else if let Some(value) = payload.get("p95Us").and_then(Value::as_f64) {
        raw.record(&sample(
            "kernel",
            operation,
            &fixture.profile,
            "aggregate_only",
            "p95_duration",
            value / 1_000.0,
            "ms",
            0,
            "incomplete",
            BTreeMap::from([("variant".to_string(), json!(variant))]),
        ))?;
    }
    if let Some(full_samples) = payload.get("fullSamplesUs").and_then(Value::as_array) {
        for (iteration, value) in full_samples.iter().filter_map(Value::as_f64).enumerate() {
            raw.record(&sample(
                "kernel",
                "project_model_full_build",
                &fixture.profile,
                "warm",
                "duration",
                value / 1_000.0,
                "ms",
                iteration,
                "ok",
                BTreeMap::from([
                    ("sourceOperation".to_string(), json!(operation)),
                    ("variant".to_string(), json!("full_oracle")),
                ]),
            ))?;
        }
    }
    for (name, value) in payload.as_object().into_iter().flatten() {
        if name == "p50Us"
            || name == "p95Us"
            || name == "maxUs"
            || name == "samplesUs"
            || name == "fullSamplesUs"
            || !name.ends_with("Us")
        {
            continue;
        }
        let Some(value) = value.as_f64() else {
            continue;
        };
        raw.record(&sample(
            "kernel",
            operation,
            &fixture.profile,
            "diagnostic",
            name,
            value / 1_000.0,
            "ms",
            0,
            contract_status,
            BTreeMap::from([("variant".to_string(), json!(variant))]),
        ))?;
    }
    Ok(())
}

fn run_kernel(
    project_root: &Path,
    run_root: &Path,
    fixture: &FixtureSnapshot,
    suite: Suite,
    raw: &mut RawSampleWriter,
    diagnostics: &mut Vec<String>,
) -> Result<(), String> {
    let spec = suite.spec();
    let (stdout, stderr) = command_paths(run_root, &format!("kernel-{}", fixture.profile), 0);
    let measurement = run_measured(
        project_root,
        "cargo",
        &[
            "test".to_string(),
            "--manifest-path".to_string(),
            "src-tauri/Cargo.toml".to_string(),
            "--release".to_string(),
            if fixture.expected_outcome == "accepted" {
                "performance_baseline_".to_string()
            } else {
                "performance_baseline_startup_inspection_real_fixture".to_string()
            },
            "--".to_string(),
            "--ignored".to_string(),
            "--nocapture".to_string(),
            "--test-threads=1".to_string(),
        ],
        &[
            (
                "PANA_PERFORMANCE_BENCH_PROJECT".to_string(),
                fixture.project_root.clone(),
            ),
            (
                "PANA_PERFORMANCE_SAMPLE_COUNT".to_string(),
                spec.kernel_samples.to_string(),
            ),
            (
                "PANA_PERFORMANCE_WARMUP_COUNT".to_string(),
                spec.warmup_samples.to_string(),
            ),
        ],
        &stdout,
        &stderr,
        spec.resource_poll_ms,
    )?;
    record_measurement(
        raw,
        "kernel",
        "harness",
        &fixture.profile,
        "release",
        0,
        &measurement,
    )?;
    for payload in parse_prefixed_json(&format!("{}\n{}", measurement.stdout, measurement.stderr)) {
        record_kernel_payload(raw, fixture, &payload)?;
    }
    if measurement.status != 0 {
        diagnostics.push(format!(
            "Benchmarkul kernel a eșuat pentru {}: {}",
            fixture.profile,
            measurement.stderr.trim()
        ));
    }
    Ok(())
}

fn run_bundle_build(
    project_root: &Path,
    run_root: &Path,
    suite: Suite,
    raw: &mut RawSampleWriter,
    diagnostics: &mut Vec<String>,
) -> Result<(), String> {
    let spec = suite.spec();
    if !spec.include_bundle_build {
        return Ok(());
    }
    let (stdout, stderr) = command_paths(run_root, "frontend-production-build", 0);
    let measurement = run_measured(
        project_root,
        "npm",
        &["run".to_string(), "build".to_string()],
        &[("VITE_PANA_PERFORMANCE_PROBE".to_string(), "1".to_string())],
        &stdout,
        &stderr,
        spec.resource_poll_ms,
    )?;
    record_measurement(
        raw,
        "build",
        "frontend_production",
        "application",
        "cold_process",
        0,
        &measurement,
    )?;
    if measurement.status != 0 {
        diagnostics.push(format!(
            "Buildul frontend a eșuat: {}",
            measurement.stderr.trim()
        ));
    } else {
        record_artifact_inventory(
            raw,
            "frontend_bundle",
            "application",
            &project_root.join("build"),
        )?;
    }
    Ok(())
}

fn record_canvas_array(
    raw: &mut RawSampleWriter,
    payload: &Value,
    field: &str,
    scenario: &str,
    metric: &str,
) -> Result<(), String> {
    for (iteration, value) in payload
        .get(field)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_f64)
        .enumerate()
    {
        raw.record(&sample(
            "interactions",
            scenario,
            "canvas-contract",
            "sustained",
            metric,
            value,
            "ms",
            iteration,
            "ok",
            BTreeMap::new(),
        ))?;
    }
    Ok(())
}

fn run_canvas_runtime_contract(
    project_root: &Path,
    run_root: &Path,
    suite: Suite,
    raw: &mut RawSampleWriter,
    diagnostics: &mut Vec<String>,
) -> Result<(), String> {
    let spec = suite.spec();
    let (stdout, stderr) = command_paths(run_root, "canvas-runtime-contract", 0);
    let measurement = run_measured(
        project_root,
        "node",
        &["tests/browser/canvas-runtime-real.mjs".to_string()],
        &[],
        &stdout,
        &stderr,
        spec.resource_poll_ms,
    )?;
    record_measurement(
        raw,
        "interactions",
        "canvas_runtime_contract",
        "canvas-contract",
        "release_sources",
        0,
        &measurement,
    )?;
    let payload = measurement
        .stdout
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str::<Value>(line).ok());
    let Some(payload) = payload else {
        diagnostics.push(format!(
            "Contractul Canvas nu a emis JSON-ul de evidență; status {}: {}",
            measurement.status,
            measurement.stderr.trim()
        ));
        return Ok(());
    };
    if measurement.status != 0 {
        diagnostics.push(format!(
            "Contractul Canvas runtime a eșuat: {}",
            payload
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or_else(|| measurement.stderr.trim())
        ));
    }
    for (field, scenario, metric) in [
        ("patchRoundTripsMs", "text_edit", "input_to_patch_ack"),
        (
            "patchBridgeDurationsMs",
            "text_edit",
            "bridge_commit_duration",
        ),
        (
            "selectionOverlayDurationsMs",
            "selection_overlay",
            "render_duration",
        ),
        (
            "historyPatchRoundTripsMs",
            "undo_redo",
            "input_to_patch_ack",
        ),
        (
            "historyBridgeDurationsMs",
            "undo_redo",
            "bridge_commit_duration",
        ),
    ] {
        record_canvas_array(raw, &payload, field, scenario, metric)?;
    }
    if let Some(value) = payload
        .get("dragPreviewRoundTripMs")
        .and_then(Value::as_f64)
    {
        raw.record(&sample(
            "interactions",
            "structural_drag_drop",
            "canvas-contract",
            "warm",
            "input_to_projection",
            value,
            "ms",
            0,
            "ok",
            BTreeMap::new(),
        ))?;
    }
    for (scenario, evidence) in [
        ("attribute_edit", "historyCanvasPatch"),
        ("structural_operation", "canvasAgentDrag"),
        ("undo_redo_contract", "iconCanvasPatch"),
        ("selection_contract", "canvasAgentGesture"),
    ] {
        let passed = payload.get("ok").and_then(Value::as_bool) == Some(true)
            && payload.get(evidence).and_then(Value::as_str).is_some();
        raw.record(&sample(
            "interactions",
            scenario,
            "canvas-contract",
            "functional",
            "contract_pass",
            if passed { 1.0 } else { 0.0 },
            "boolean",
            0,
            if passed { "ok" } else { "failed" },
            BTreeMap::from([("evidence".to_string(), json!(evidence))]),
        ))?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Default)]
struct ResourceWindow {
    first: ProcessTreeSnapshot,
    last: ProcessTreeSnapshot,
    peak_rss_kib: u64,
    peak_pss_kib: u64,
}

fn start_resource_monitor(
    pid: u32,
    poll_ms: u64,
) -> (Arc<AtomicBool>, thread::JoinHandle<ResourceWindow>) {
    let running = Arc::new(AtomicBool::new(true));
    let thread_running = Arc::clone(&running);
    let handle = thread::spawn(move || {
        let mut window = ResourceWindow::default();
        let mut captured = false;
        while thread_running.load(Ordering::Relaxed) {
            let snapshot = sample_process_tree(pid);
            if !captured && (snapshot.rss_kib > 0 || snapshot.pss_kib > 0) {
                window.first = snapshot;
                captured = true;
            }
            window.last = snapshot;
            window.peak_rss_kib = window.peak_rss_kib.max(snapshot.rss_kib);
            window.peak_pss_kib = window.peak_pss_kib.max(snapshot.pss_kib);
            thread::sleep(Duration::from_millis(poll_ms.max(5)));
        }
        window
    });
    (running, handle)
}

fn record_adapter_output(raw: &mut RawSampleWriter, source: &str) -> Result<usize, String> {
    let mut count = 0;
    for line in source.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if value.get("kind").and_then(Value::as_str) != Some("sample") {
            continue;
        }
        let attributes = value
            .get("attributes")
            .and_then(Value::as_object)
            .map(|attributes| {
                attributes
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect()
            })
            .unwrap_or_default();
        raw.record(&sample(
            value.get("layer").and_then(Value::as_str).unwrap_or("ui"),
            value
                .get("scenario")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            value
                .get("profile")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            value
                .get("mode")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            value
                .get("metric")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            value.get("value").and_then(Value::as_f64).unwrap_or(0.0),
            value
                .get("unit")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            value
                .get("iteration")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or(0),
            value.get("status").and_then(Value::as_str).unwrap_or("ok"),
            attributes,
        ))?;
        count += 1;
    }
    Ok(count)
}

fn run_adapter(
    project_root: &Path,
    run_root: &Path,
    label: &str,
    program: &str,
    arguments: &[String],
    environment: &[(String, String)],
    raw: &mut RawSampleWriter,
) -> Result<(i32, usize, String), String> {
    let log_root = run_root.join("adapter-logs");
    fs::create_dir_all(&log_root).map_err(|error| error.to_string())?;
    let stdout_path = log_root.join(format!("{label}.stdout.jsonl"));
    let stderr_path = log_root.join(format!("{label}.stderr.log"));
    let stdout_file =
        File::create(&stdout_path).map_err(|error| format!("Log adaptor stdout: {error}"))?;
    let stderr_file = File::create(&stderr_path).map_err(|error| error.to_string())?;
    let mut child = Command::new(program)
        .args(arguments)
        .envs(environment.iter().cloned())
        .current_dir(project_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .map_err(|error| format!("Adaptorul `{program}` nu a pornit: {error}"))?;
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!("Adaptorul `{program}` nu expune stdout."));
    };
    let mut reader = BufReader::new(stdout);
    let mut stdout_log = BufWriter::new(stdout_file);
    let mut count = 0;
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = match reader.read_line(&mut line) {
            Ok(bytes) => bytes,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "Citirea stdout pentru adaptorul `{program}` a eșuat: {error}"
                ));
            }
        };
        if bytes == 0 {
            break;
        }
        if let Err(error) = stdout_log
            .write_all(line.as_bytes())
            .and_then(|_| stdout_log.flush())
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("Checkpoint stdout adaptor: {error}"));
        }
        match record_adapter_output(raw, &line) {
            Ok(recorded) => count += recorded,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        }
    }
    stdout_log
        .flush()
        .map_err(|error| format!("Flush stdout adaptor: {error}"))?;
    let status = child
        .wait()
        .map_err(|error| format!("Așteptarea adaptorului `{program}` a eșuat: {error}"))?;
    let stderr = fs::read_to_string(&stderr_path)
        .map_err(|error| format!("Citirea stderr pentru adaptorul `{program}` a eșuat: {error}"))?;
    Ok((status.code().unwrap_or(-1), count, stderr))
}

fn validate_webkit_document_output(
    path: &Path,
    profile: &str,
    expected_samples: usize,
) -> Result<(), String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("Citirea output-ului WebKit v2 a eșuat: {error}"))?;
    let required = [
        ("code_to_code", "input_to_tab_selected"),
        ("code_to_code", "input_to_document_ready"),
        ("canonical_template_reactivation", "input_to_tab_selected"),
        ("canonical_template_reactivation", "input_to_document_ready"),
        ("rapid_document_alternation", "input_to_tab_selected"),
        ("rapid_document_alternation", "input_to_document_ready"),
    ];
    let mut counts = BTreeMap::new();
    for line in source.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if value.get("kind").and_then(Value::as_str) != Some("sample")
            || value.get("layer").and_then(Value::as_str) != Some("interactions")
            || value.get("profile").and_then(Value::as_str) != Some(profile)
            || value.get("status").and_then(Value::as_str) != Some("ok")
            || !value
                .get("value")
                .and_then(Value::as_f64)
                .is_some_and(f64::is_finite)
        {
            continue;
        }
        let key = (
            value.get("scenario").and_then(Value::as_str).unwrap_or(""),
            value.get("metric").and_then(Value::as_str).unwrap_or(""),
        );
        if required.contains(&key) {
            *counts
                .entry((key.0.to_string(), key.1.to_string()))
                .or_insert(0) += 1;
        }
    }
    let incomplete = required
        .into_iter()
        .filter_map(|(scenario, metric)| {
            let actual = counts
                .get(&(scenario.to_string(), metric.to_string()))
                .copied()
                .unwrap_or(0);
            (actual != expected_samples)
                .then(|| format!("{scenario}/{metric}={actual}/{expected_samples}"))
        })
        .collect::<Vec<_>>();
    if incomplete.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Proba WebKit v2 {profile} este incompletă: {}.",
            incomplete.join(", ")
        ))
    }
}

fn available_inspector_port() -> Result<u16, String> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("Nu am putut rezerva portul inspectorului: {error}"))?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(|error| error.to_string())
}

fn build_native_application(
    project_root: &Path,
    run_root: &Path,
    suite: Suite,
    raw: &mut RawSampleWriter,
    diagnostics: &mut Vec<String>,
    explicit_binary: Option<&Path>,
) -> Result<PathBuf, String> {
    if let Some(binary) = explicit_binary {
        return binary
            .canonicalize()
            .map_err(|error| format!("Binarul explicit nu poate fi folosit: {error}"));
    }
    let spec = suite.spec();
    let (stdout, stderr) = command_paths(run_root, "native-release-build", 0);
    let measurement = run_measured(
        project_root,
        "scripts/tauri-with-local-appimage-runtime.sh",
        &[
            "build".to_string(),
            "--no-bundle".to_string(),
            "--ci".to_string(),
            "--config".to_string(),
            "benchmarks/tauri-benchmark.conf.json".to_string(),
        ],
        &[],
        &stdout,
        &stderr,
        spec.resource_poll_ms,
    )?;
    record_measurement(
        raw,
        "build",
        "native_application",
        "application",
        "release",
        0,
        &measurement,
    )?;
    if measurement.status != 0 {
        diagnostics.push(format!(
            "Buildul aplicației native a eșuat: {}",
            measurement.stderr.trim()
        ));
        return Err("Buildul aplicației native a eșuat.".to_string());
    }
    let binary = project_root
        .join("src-tauri/target/release/pana-studio")
        .canonicalize()
        .map_err(|error| format!("Binarul release lipsește după build: {error}"))?;
    record_artifact_inventory(raw, "native_binary", "application", &binary)?;
    Ok(binary)
}

fn record_resource_snapshot(
    raw: &mut RawSampleWriter,
    fixture: &FixtureSnapshot,
    mode: &str,
    iteration: usize,
    snapshot: ProcessTreeSnapshot,
) -> Result<(), String> {
    for (metric, value, unit) in [
        ("rss", snapshot.rss_kib as f64, "KiB"),
        ("pss", snapshot.pss_kib as f64, "KiB"),
        ("read", snapshot.read_bytes as f64, "bytes"),
        ("write", snapshot.write_bytes as f64, "bytes"),
        ("cpu", snapshot.cpu_ticks as f64, "ticks"),
    ] {
        raw.record(&sample(
            "resources",
            "application_process_tree",
            &fixture.profile,
            mode,
            metric,
            value,
            unit,
            iteration,
            "ok",
            BTreeMap::new(),
        ))?;
    }
    Ok(())
}

fn ingest_kernel_runtime_log(
    raw: &mut RawSampleWriter,
    fixture: &FixtureSnapshot,
    iteration: usize,
    log_path: &Path,
) -> Result<(), String> {
    let source = fs::read_to_string(log_path)
        .map_err(|error| format!("Jurnalul kernel runtime nu poate fi citit: {error}"))?;
    let mut performance_count = 0_usize;
    for (event_index, event) in source
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .enumerate()
    {
        let Some(attributes) = event.get("attributes").and_then(Value::as_object) else {
            continue;
        };
        if attributes
            .get("performanceSchemaVersion")
            .and_then(Value::as_u64)
            != Some(3)
        {
            continue;
        }
        let operation = attributes
            .get("performanceOperation")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let variant = attributes
            .get("performanceVariant")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let sample_attributes = BTreeMap::from([
            ("variant".to_string(), json!(variant)),
            (
                "projectModelBuildMode".to_string(),
                attributes
                    .get("projectModelBuildMode")
                    .cloned()
                    .unwrap_or(Value::Null),
            ),
            (
                "projectModelFallbackReason".to_string(),
                attributes
                    .get("projectModelFallbackReason")
                    .cloned()
                    .unwrap_or(Value::Null),
            ),
        ]);
        if let Some(total) = attributes.get("performanceTotalUs").and_then(Value::as_f64) {
            raw.record(&sample(
                "kernel_runtime",
                operation,
                &fixture.profile,
                "native_release",
                "duration",
                total / 1_000.0,
                "ms",
                event_index,
                "ok",
                sample_attributes.clone(),
            ))?;
        }
        for (name, value) in attributes {
            if name == "performanceTotalUs" || !name.ends_with("Us") {
                continue;
            }
            let Some(value) = value.as_f64() else {
                continue;
            };
            raw.record(&sample(
                "kernel_runtime",
                operation,
                &fixture.profile,
                "native_release",
                name,
                value / 1_000.0,
                "ms",
                event_index,
                "ok",
                sample_attributes.clone(),
            ))?;
        }
        performance_count += 1;
    }
    for (metric, value, unit) in [
        ("event_count", source.lines().count() as f64, "count"),
        ("log_bytes", source.len() as f64, "bytes"),
        (
            "performance_command_count",
            performance_count as f64,
            "count",
        ),
    ] {
        raw.record(&sample(
            "observability",
            "kernel_runtime_log",
            &fixture.profile,
            "native_release",
            metric,
            value,
            unit,
            iteration,
            "ok",
            BTreeMap::new(),
        ))?;
    }
    Ok(())
}

fn run_ui_launch(
    project_root: &Path,
    run_root: &Path,
    fixture: &FixtureSnapshot,
    suite: Suite,
    binary: &Path,
    iteration: usize,
    exercise: bool,
    raw: &mut RawSampleWriter,
    diagnostics: &mut Vec<String>,
) -> Result<(), String> {
    let spec = suite.spec();
    let app_log_root = run_root
        .join("app-runtime")
        .join(&fixture.profile)
        .join(format!("launch-{iteration:02}"));
    for name in ["config", "data", "cache"] {
        fs::create_dir_all(app_log_root.join(name)).map_err(|error| error.to_string())?;
    }
    let port = available_inspector_port()?;
    let app_stdout = app_log_root.join("application.stdout.log");
    let app_stderr = app_log_root.join("application.stderr.log");
    let started = std::time::Instant::now();
    let mut required_probe_error = None;
    let mut child = Command::new(binary)
        .env_remove("NO_AT_BRIDGE")
        .env("GDK_BACKEND", "x11")
        .env("AT_SPI_BUS_ADDRESS", "unix:path=/run/user/1000/at-spi/bus")
        .env("WEBKIT_INSPECTOR_HTTP_SERVER", format!("127.0.0.1:{port}"))
        .env("XDG_CONFIG_HOME", app_log_root.join("config"))
        .env("XDG_DATA_HOME", app_log_root.join("data"))
        .env("XDG_CACHE_HOME", app_log_root.join("cache"))
        .stdout(Stdio::from(
            File::create(&app_stdout).map_err(|error| error.to_string())?,
        ))
        .stderr(Stdio::from(
            File::create(&app_stderr).map_err(|error| error.to_string())?,
        ))
        .spawn()
        .map_err(|error| format!("Aplicația release nu a pornit: {error}"))?;
    let pid = child.id();
    let (monitor_running, monitor_handle) = start_resource_monitor(pid, spec.resource_poll_ms);
    let adapter_environment = vec![
        ("PANA_BENCHMARK_APP_PID".to_string(), pid.to_string()),
        (
            "AT_SPI_BUS_ADDRESS".to_string(),
            "unix:path=/run/user/1000/at-spi/bus".to_string(),
        ),
    ];
    let expected = if fixture.expected_outcome == "accepted" {
        "accepted"
    } else {
        "rejected"
    };
    let label = format!("atspi-open-{}-{iteration:02}", fixture.profile);
    let (open_status, open_samples, open_error) = run_adapter(
        project_root,
        run_root,
        &label,
        "python3",
        &[
            "scripts/performance-atspi-adapter.py".to_string(),
            "open".to_string(),
            "--profile".to_string(),
            fixture.profile.clone(),
            "--project".to_string(),
            fixture.project_root.clone(),
            "--expected".to_string(),
            expected.to_string(),
            "--iteration".to_string(),
            iteration.to_string(),
            "--timeout-seconds".to_string(),
            "180".to_string(),
        ],
        &adapter_environment,
        raw,
    )?;
    raw.record(&sample(
        "ui",
        "application_launch",
        &fixture.profile,
        "cold_process",
        "launch_to_adapter_complete",
        started.elapsed().as_secs_f64() * 1_000.0,
        "ms",
        iteration,
        if open_status == 0 { "ok" } else { "failed" },
        BTreeMap::from([("adapterSamples".to_string(), json!(open_samples))]),
    ))?;
    if open_status != 0 {
        diagnostics.push(format!(
            "Open UI {} iterația {iteration} a eșuat cu status {open_status}; verdictul funcțional este în {label}.stdout.jsonl.{}",
            fixture.profile,
            if open_error.trim().is_empty() {
                String::new()
            } else {
                " Adaptorul a emis avertismente pe stderr.".to_string()
            }
        ));
    }
    if exercise && open_status == 0 && fixture.expected_outcome == "accepted" {
        thread::sleep(Duration::from_secs(2));
        let before = sample_process_tree(pid);
        record_resource_snapshot(raw, fixture, "before_sustained", iteration, before)?;
        let activity_label = format!("atspi-activities-{}-{iteration:02}", fixture.profile);
        let (activity_status, _, activity_error) = run_adapter(
            project_root,
            run_root,
            &activity_label,
            "python3",
            &[
                "scripts/performance-atspi-adapter.py".to_string(),
                "activities".to_string(),
                "--profile".to_string(),
                fixture.profile.clone(),
                "--cycles".to_string(),
                spec.warm_samples.to_string(),
                "--warmup-cycles".to_string(),
                spec.warmup_samples.to_string(),
                "--sustained-operations".to_string(),
                spec.sustained_operations.to_string(),
                "Editor".to_string(),
                "Șabloane".to_string(),
                "Componente".to_string(),
            ],
            &adapter_environment,
            raw,
        )?;
        if activity_status != 0 {
            diagnostics.push(format!(
                "Interacțiunile UI {} au eșuat: {}",
                fixture.profile,
                activity_error.trim()
            ));
        }
        let webkit_label = format!("webkit-{}-{iteration:02}", fixture.profile);
        let (webkit_status, _, webkit_error) = run_adapter(
            project_root,
            run_root,
            &webkit_label,
            "node",
            &[
                "scripts/performance-webkit-adapter.mjs".to_string(),
                "--endpoint".to_string(),
                format!("http://127.0.0.1:{port}"),
                "--profile".to_string(),
                fixture.profile.clone(),
                "--samples".to_string(),
                spec.warm_samples.to_string(),
                "--warmups".to_string(),
                spec.warmup_samples.to_string(),
                "--frame-samples".to_string(),
                if matches!(suite, Suite::Soak) {
                    600
                } else {
                    180
                }
                .to_string(),
            ],
            &[],
            raw,
        )?;
        if webkit_status != 0 {
            let diagnostic = format!(
                "Proba WebKit {} a eșuat: {}",
                fixture.profile,
                webkit_error.trim()
            );
            diagnostics.push(diagnostic.clone());
            required_probe_error = Some(diagnostic);
        } else if let Err(error) = validate_webkit_document_output(
            &run_root
                .join("adapter-logs")
                .join(format!("{webkit_label}.stdout.jsonl")),
            &fixture.profile,
            spec.warm_samples,
        ) {
            diagnostics.push(error.clone());
            required_probe_error = Some(error);
        }
        let settle_seconds = if matches!(suite, Suite::Smoke) { 2 } else { 30 };
        thread::sleep(Duration::from_secs(settle_seconds));
        let after = sample_process_tree(pid);
        record_resource_snapshot(raw, fixture, "after_settle", iteration, after)?;
        for (metric, value) in [
            (
                "retained_rss_delta",
                after.rss_kib.saturating_sub(before.rss_kib),
            ),
            (
                "retained_pss_delta",
                after.pss_kib.saturating_sub(before.pss_kib),
            ),
        ] {
            raw.record(&sample(
                "resources",
                "application_retention",
                &fixture.profile,
                "after_settle",
                metric,
                value as f64,
                "KiB",
                iteration,
                "ok",
                BTreeMap::from([("settleSeconds".to_string(), json!(settle_seconds))]),
            ))?;
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    monitor_running.store(false, Ordering::Relaxed);
    let window = monitor_handle
        .join()
        .map_err(|_| "Monitorul de resurse a panicat.".to_string())?;
    for (metric, value) in [
        ("peak_rss", window.peak_rss_kib),
        ("peak_pss", window.peak_pss_kib),
        (
            "read_delta",
            window
                .last
                .read_bytes
                .saturating_sub(window.first.read_bytes),
        ),
        (
            "write_delta",
            window
                .last
                .write_bytes
                .saturating_sub(window.first.write_bytes),
        ),
        (
            "cpu_delta",
            window.last.cpu_ticks.saturating_sub(window.first.cpu_ticks),
        ),
    ] {
        raw.record(&sample(
            "resources",
            "application_lifetime",
            &fixture.profile,
            "cold_process",
            metric,
            value as f64,
            if metric.contains("rss") || metric.contains("pss") {
                "KiB"
            } else if metric.contains("cpu") {
                "ticks"
            } else {
                "bytes"
            },
            iteration,
            "ok",
            BTreeMap::new(),
        ))?;
    }
    let kernel_log = app_log_root.join("data/com.gabriel.panastudio/logs/app/kernel.jsonl");
    if kernel_log.is_file() {
        ingest_kernel_runtime_log(raw, fixture, iteration, &kernel_log)?;
    } else {
        diagnostics.push(format!(
            "Jurnalul kernel runtime lipsește pentru {} lansarea {iteration}.",
            fixture.profile
        ));
    }
    if let Some(error) = required_probe_error {
        return Err(error);
    }
    Ok(())
}

fn run_ui(
    project_root: &Path,
    run_root: &Path,
    fixtures: &[FixtureSnapshot],
    suite: Suite,
    explicit_binary: Option<&Path>,
    raw: &mut RawSampleWriter,
    diagnostics: &mut Vec<String>,
) -> Result<(), String> {
    if !suite.spec().include_ui {
        return Ok(());
    }
    let binary = build_native_application(
        project_root,
        run_root,
        suite,
        raw,
        diagnostics,
        explicit_binary,
    )?;
    for fixture in fixtures {
        for iteration in 0..suite.spec().cold_samples {
            run_ui_launch(
                project_root,
                run_root,
                fixture,
                suite,
                &binary,
                iteration,
                iteration == 0,
                raw,
                diagnostics,
            )?;
        }
    }
    Ok(())
}

pub fn run(options: RunOptions) -> Result<PathBuf, String> {
    let spec = options.suite.spec();
    let id = run_id(spec.label);
    let run_root = options.output_root.join(&id);
    fs::create_dir_all(&run_root).map_err(|error| error.to_string())?;
    let raw_path = run_root.join("samples.jsonl");
    let fixtures_root = run_root.join("fixture-workspace");
    if fixtures_root.exists() {
        return Err(format!(
            "Rădăcina temporară există deja: {}",
            fixtures_root.display()
        ));
    }
    fs::create_dir_all(&fixtures_root).map_err(|error| error.to_string())?;
    let environment = environment::capture(&options.project_root);
    let mut manifest = RunManifest {
        schema_version: RUN_SCHEMA_VERSION,
        run_id: id,
        suite: spec.label.to_string(),
        started_unix_ms: unix_ms(),
        completed_unix_ms: None,
        status: "running".to_string(),
        environment,
        fixtures: Vec::new(),
        raw_samples_path: raw_path.to_string_lossy().into_owned(),
        diagnostics: Vec::new(),
    };
    write_json(&run_root.join("run.json"), &manifest)?;
    let mut raw = RawSampleWriter::open(&raw_path)?;
    let execution = (|| -> Result<(), String> {
        for profile in spec.profiles {
            let fixture =
                materialize_profile(&options.canonical_fixture_root, &fixtures_root, profile)?;
            verify_immutable_fixture(&fixture)?;
            manifest.fixtures.push(fixture);
            write_json(&run_root.join("run.json"), &manifest)?;
        }
        run_bundle_build(
            &options.project_root,
            &run_root,
            options.suite,
            &mut raw,
            &mut manifest.diagnostics,
        )?;
        for fixture in &manifest.fixtures {
            run_zola(
                &options.project_root,
                &run_root,
                fixture,
                options.suite,
                &mut raw,
                &mut manifest.diagnostics,
            )?;
            run_kernel(
                &options.project_root,
                &run_root,
                fixture,
                options.suite,
                &mut raw,
                &mut manifest.diagnostics,
            )?;
            verify_immutable_fixture(fixture)?;
        }
        run_canvas_runtime_contract(
            &options.project_root,
            &run_root,
            options.suite,
            &mut raw,
            &mut manifest.diagnostics,
        )?;
        run_ui(
            &options.project_root,
            &run_root,
            &manifest.fixtures,
            options.suite,
            options.app_binary.as_deref(),
            &mut raw,
            &mut manifest.diagnostics,
        )?;
        Ok(())
    })();
    manifest.completed_unix_ms = Some(unix_ms());
    match execution {
        Ok(()) => {
            manifest.status = if manifest.diagnostics.is_empty() {
                "complete".to_string()
            } else {
                "complete_with_diagnostics".to_string()
            };
        }
        Err(error) => {
            manifest.status = "failed".to_string();
            manifest.diagnostics.push(error);
        }
    }
    write_json(&run_root.join("run.json"), &manifest)?;
    write_reports(
        &raw_path,
        &run_root.join("report.json"),
        &run_root.join("report.md"),
    )?;
    validate_artifact_set(
        &run_root.join("run.json"),
        &raw_path,
        &run_root.join("report.json"),
    )?;
    if !options.keep_fixtures {
        remove_materialized_fixtures(&fixtures_root)?;
    }
    if manifest.status == "failed" {
        return Err(format!(
            "Suita a eșuat; datele parțiale au fost păstrate în {}",
            run_root.display()
        ));
    }
    Ok(run_root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_output_is_checkpointed_and_preserved_on_nonzero_exit() {
        let root = std::env::temp_dir().join(format!(
            "pana-benchmark-adapter-test-{}-{}",
            std::process::id(),
            unix_ms()
        ));
        fs::create_dir_all(&root).unwrap();
        let raw_path = root.join("samples.jsonl");
        let mut raw = RawSampleWriter::open(&raw_path).unwrap();
        let payload = r#"{"schemaVersion":1,"kind":"sample","layer":"ui","scenario":"stream","profile":"control","mode":"test","metric":"checkpoint","value":1,"unit":"count","iteration":0,"status":"ok","attributes":{}}"#;
        let command = format!("printf '%s\\n' '{payload}'; printf 'diagnostic\\n' >&2; exit 7");
        let (status, count, stderr) = run_adapter(
            &root,
            &root,
            "stream-test",
            "sh",
            &["-c".to_string(), command],
            &[],
            &mut raw,
        )
        .unwrap();

        assert_eq!(status, 7);
        assert_eq!(count, 1);
        assert_eq!(stderr, "diagnostic\n");
        assert_eq!(fs::read_to_string(&raw_path).unwrap().lines().count(), 1);
        assert_eq!(
            fs::read_to_string(root.join("adapter-logs/stream-test.stdout.jsonl"))
                .unwrap()
                .lines()
                .count(),
            1
        );
        assert_eq!(
            fs::read_to_string(root.join("adapter-logs/stream-test.stderr.log")).unwrap(),
            "diagnostic\n"
        );
        drop(raw);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn webkit_v2_document_output_is_fail_closed() {
        let root = std::env::temp_dir().join(format!(
            "pana-benchmark-webkit-v2-test-{}-{}",
            std::process::id(),
            unix_ms()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("webkit.jsonl");
        let required = [
            ("code_to_code", "input_to_tab_selected"),
            ("code_to_code", "input_to_document_ready"),
            ("canonical_template_reactivation", "input_to_tab_selected"),
            ("canonical_template_reactivation", "input_to_document_ready"),
            ("rapid_document_alternation", "input_to_tab_selected"),
            ("rapid_document_alternation", "input_to_document_ready"),
        ];
        let complete = required
            .iter()
            .flat_map(|(scenario, metric)| {
                (0..2).map(move |iteration| {
                    json!({
                        "schemaVersion": 1,
                        "kind": "sample",
                        "layer": "interactions",
                        "scenario": scenario,
                        "profile": "control",
                        "mode": "warm",
                        "metric": metric,
                        "value": 1.0,
                        "unit": "ms",
                        "iteration": iteration,
                        "status": "ok",
                        "attributes": {}
                    })
                    .to_string()
                })
            })
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&path, format!("{complete}\n")).unwrap();
        validate_webkit_document_output(&path, "control", 2).unwrap();

        let incomplete = complete.lines().skip(1).collect::<Vec<_>>().join("\n");
        fs::write(&path, format!("{incomplete}\n")).unwrap();
        let error = validate_webkit_document_output(&path, "control", 2).unwrap_err();
        assert!(error.contains("code_to_code/input_to_tab_selected=1/2"));
        fs::remove_dir_all(root).unwrap();
    }
}
