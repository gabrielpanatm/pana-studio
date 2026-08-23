use std::{
    collections::BTreeSet,
    fs,
    path::Path,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use sha2::{Digest, Sha256};

use crate::model::{EnvironmentSnapshot, RUN_SCHEMA_VERSION};

fn command_output(root: &Path, program: &str, arguments: &[&str]) -> Option<String> {
    let output = Command::new(program)
        .args(arguments)
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn meminfo_value(key: &str) -> Option<u64> {
    let source = fs::read_to_string("/proc/meminfo").ok()?;
    source.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        (name == key)
            .then(|| value.split_whitespace().next()?.parse().ok())
            .flatten()
    })
}

fn cpu_model() -> Option<String> {
    let source = fs::read_to_string("/proc/cpuinfo").ok()?;
    source.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.trim() == "model name").then(|| value.trim().to_string())
    })
}

fn values_below(root: &Path, suffix: &str) -> Vec<String> {
    let Ok(entries) = walkdir::WalkDir::new(root)
        .max_depth(4)
        .follow_links(false)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
    else {
        return Vec::new();
    };
    entries
        .into_iter()
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().to_string_lossy().ends_with(suffix))
        .filter_map(|entry| fs::read_to_string(entry.path()).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

pub fn capture(project_root: &Path) -> EnvironmentSnapshot {
    let worktree =
        command_output(project_root, "git", &["status", "--porcelain=v1"]).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(worktree.as_bytes());
    let governors = values_below(Path::new("/sys/devices/system/cpu"), "scaling_governor")
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let temperatures_millidegrees_celsius = values_below(Path::new("/sys/class/thermal"), "temp")
        .into_iter()
        .filter_map(|value| value.parse::<i64>().ok())
        .collect();
    EnvironmentSnapshot {
        schema_version: RUN_SCHEMA_VERSION,
        captured_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        git_commit: command_output(project_root, "git", &["rev-parse", "HEAD"]),
        git_worktree_dirty: !worktree.is_empty(),
        git_worktree_digest_sha256: format!("{:x}", hasher.finalize()),
        rustc_version: command_output(project_root, "rustc", &["--version"]),
        cargo_version: command_output(project_root, "cargo", &["--version"]),
        node_version: command_output(project_root, "node", &["--version"]),
        zola_version: command_output(project_root, "zola", &["--version"]),
        kernel: command_output(project_root, "uname", &["-srmo"]),
        cpu_model: cpu_model(),
        logical_cpu_count: std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1),
        memory_total_kib: meminfo_value("MemTotal"),
        memory_available_kib: meminfo_value("MemAvailable"),
        swap_total_kib: meminfo_value("SwapTotal"),
        swap_free_kib: meminfo_value("SwapFree"),
        load_average: fs::read_to_string("/proc/loadavg")
            .ok()
            .map(|value| value.trim().to_string()),
        cpu_governors: governors,
        temperatures_millidegrees_celsius,
    }
}
