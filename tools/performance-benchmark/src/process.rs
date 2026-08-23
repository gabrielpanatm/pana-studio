use std::{
    collections::BTreeSet,
    fs::{self, File},
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::model::CommandMeasurement;

#[derive(Clone, Copy, Debug, Default)]
struct ProcessCounters {
    rss_kib: u64,
    pss_kib: u64,
    read_bytes: u64,
    write_bytes: u64,
    cpu_ticks: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessTreeSnapshot {
    pub rss_kib: u64,
    pub pss_kib: u64,
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub cpu_ticks: u64,
}

fn process_parent_and_cpu(pid: u32) -> Option<(u32, u64)> {
    let source = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let closing = source.rfind(')')?;
    let fields = source
        .get(closing + 2..)?
        .split_whitespace()
        .collect::<Vec<_>>();
    let parent = fields.get(1)?.parse().ok()?;
    let user_ticks = fields.get(11)?.parse::<u64>().ok()?;
    let system_ticks = fields.get(12)?.parse::<u64>().ok()?;
    Some((parent, user_ticks.saturating_add(system_ticks)))
}

fn process_tree(root: u32) -> BTreeSet<u32> {
    let mut result = BTreeSet::from([root]);
    let mut pending = vec![root];
    while let Some(pid) = pending.pop() {
        let children_path = format!("/proc/{pid}/task/{pid}/children");
        let Ok(children) = fs::read_to_string(children_path) else {
            continue;
        };
        for child in children
            .split_whitespace()
            .filter_map(|value| value.parse::<u32>().ok())
        {
            if result.insert(child) {
                pending.push(child);
            }
        }
    }
    result
}

fn process_counters(pid: u32) -> ProcessCounters {
    let mut counters = ProcessCounters::default();
    if let Ok(source) = fs::read_to_string(format!("/proc/{pid}/smaps_rollup")) {
        for line in source.lines() {
            if let Some(value) = line.strip_prefix("Rss:") {
                counters.rss_kib = value
                    .split_whitespace()
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
            } else if let Some(value) = line.strip_prefix("Pss:") {
                counters.pss_kib = value
                    .split_whitespace()
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
            }
        }
    }
    if let Ok(source) = fs::read_to_string(format!("/proc/{pid}/io")) {
        for line in source.lines() {
            if let Some(value) = line.strip_prefix("read_bytes:") {
                counters.read_bytes = value.trim().parse().unwrap_or(0);
            } else if let Some(value) = line.strip_prefix("write_bytes:") {
                counters.write_bytes = value.trim().parse().unwrap_or(0);
            }
        }
    }
    counters.cpu_ticks = process_parent_and_cpu(pid)
        .map(|(_, ticks)| ticks)
        .unwrap_or(0);
    counters
}

fn tree_counters(root: u32) -> ProcessCounters {
    process_tree(root)
        .into_iter()
        .fold(ProcessCounters::default(), |mut total, pid| {
            let item = process_counters(pid);
            total.rss_kib = total.rss_kib.saturating_add(item.rss_kib);
            total.pss_kib = total.pss_kib.saturating_add(item.pss_kib);
            total.read_bytes = total.read_bytes.saturating_add(item.read_bytes);
            total.write_bytes = total.write_bytes.saturating_add(item.write_bytes);
            total.cpu_ticks = total.cpu_ticks.saturating_add(item.cpu_ticks);
            total
        })
}

pub fn sample_process_tree(root: u32) -> ProcessTreeSnapshot {
    let counters = tree_counters(root);
    ProcessTreeSnapshot {
        rss_kib: counters.rss_kib,
        pss_kib: counters.pss_kib,
        read_bytes: counters.read_bytes,
        write_bytes: counters.write_bytes,
        cpu_ticks: counters.cpu_ticks,
    }
}

pub fn run_measured(
    project_root: &Path,
    program: &str,
    arguments: &[String],
    environment: &[(String, String)],
    stdout_path: &Path,
    stderr_path: &Path,
    poll_ms: u64,
) -> Result<CommandMeasurement, String> {
    if let Some(parent) = stdout_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let stdout_file = File::create(stdout_path).map_err(|error| error.to_string())?;
    let stderr_file = File::create(stderr_path).map_err(|error| error.to_string())?;
    let started = Instant::now();
    let mut child = Command::new(program)
        .args(arguments)
        .envs(environment.iter().cloned())
        .current_dir(project_root)
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .map_err(|error| format!("Nu am putut porni `{program}`: {error}"))?;
    let mut peak = ProcessCounters::default();
    let mut first = None;
    let (status, last) = loop {
        let current = tree_counters(child.id());
        first.get_or_insert(current);
        peak.rss_kib = peak.rss_kib.max(current.rss_kib);
        peak.pss_kib = peak.pss_kib.max(current.pss_kib);
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            break (status.code().unwrap_or(-1), current);
        }
        thread::sleep(Duration::from_millis(poll_ms.max(5)));
    };
    let first = first.unwrap_or_default();
    Ok(CommandMeasurement {
        status,
        wall_ms: started.elapsed().as_secs_f64() * 1_000.0,
        peak_rss_kib: peak.rss_kib,
        peak_pss_kib: peak.pss_kib,
        read_bytes: last.read_bytes.saturating_sub(first.read_bytes),
        write_bytes: last.write_bytes.saturating_sub(first.write_bytes),
        cpu_ticks: last.cpu_ticks.saturating_sub(first.cpu_ticks),
        stdout: fs::read_to_string(stdout_path).unwrap_or_default(),
        stderr: fs::read_to_string(stderr_path).unwrap_or_default(),
    })
}
