use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Runtime};
use walkdir::WalkDir;

use crate::{
    app_home::{app_home_snapshot, AppHomeSnapshot},
    kernel::{
        observability::clear_kernel_observability_logs,
        write_authority::{
            WriteAuthority, WriteCategory, WriteIntent, WriteOperationKind, WriteOwner,
            WritePolicy, WriteTarget,
        },
    },
    preview::preprocess::{preview_project_dir, preview_project_directory_name},
    state::AppState,
};

const STORAGE_SCHEMA_VERSION: u32 = 1;
const WEBKIT_CACHE_DIRECTORY: &str = "WebKitCache";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageAreaSnapshot {
    pub path: String,
    pub bytes: u64,
    pub entries: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageCacheSnapshot {
    pub webkit: StorageAreaSnapshot,
    pub preview: StorageAreaSnapshot,
    pub total_bytes: u64,
    pub reclaimable_bytes: u64,
    pub protected_preview_bytes: u64,
    pub webkit_cleanup_supported: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageLogsSnapshot {
    pub area: StorageAreaSnapshot,
    pub active_bytes: u64,
    pub archive_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageSessionSnapshot {
    pub id: String,
    pub project_name: String,
    pub project_root: String,
    pub bytes: u64,
    pub entries: u64,
    pub last_seen_at_ms: u64,
    pub project_exists: bool,
    pub has_recovery: bool,
    pub recovery_signals: Vec<String>,
    pub manifest_status: String,
    pub active: bool,
    pub deletable: bool,
    pub default_selected: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageSessionsSnapshot {
    pub path: String,
    pub revision: String,
    pub total_bytes: u64,
    pub reclaimable_bytes: u64,
    pub count: usize,
    pub orphan_count: usize,
    pub recovery_count: usize,
    pub active_count: usize,
    pub items: Vec<StorageSessionSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationStorageSnapshot {
    pub schema_version: u32,
    pub scanned_at_ms: u64,
    pub total_bytes: u64,
    pub reclaimable_bytes: u64,
    pub cache: StorageCacheSnapshot,
    pub logs: StorageLogsSnapshot,
    pub sessions: StorageSessionsSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteStorageSessionsRequest {
    pub expected_revision: String,
    pub session_ids: Vec<String>,
    #[serde(default)]
    pub confirmed_recovery_session_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageCleanupReceipt {
    pub schema_version: u32,
    pub operation: String,
    pub removed_items: usize,
    pub bytes_before: u64,
    pub bytes_after: u64,
    pub freed_bytes: u64,
    pub protected_bytes: u64,
    pub failures: Vec<String>,
    pub snapshot: ApplicationStorageSnapshot,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StorageCleanupEffect {
    pub removed_items: usize,
    pub protected_bytes: u64,
    pub failures: Vec<String>,
}

#[derive(Clone, Debug, Default)]
struct ActiveStorageIdentity {
    session_id: Option<String>,
    zola_root: Option<PathBuf>,
}

#[derive(Clone, Debug)]
struct StorageRoots {
    webkit_cache: PathBuf,
    preview_cache: PathBuf,
    logs: PathBuf,
    sessions: PathBuf,
}

impl StorageRoots {
    fn from_app_home(home: &AppHomeSnapshot) -> Self {
        Self {
            webkit_cache: PathBuf::from(&home.data_dir).join(WEBKIT_CACHE_DIRECTORY),
            preview_cache: PathBuf::from(&home.preview_cache_dir),
            logs: PathBuf::from(&home.app_logs_dir),
            sessions: PathBuf::from(&home.sessions_dir),
        }
    }
}

pub fn read_application_storage<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
) -> Result<ApplicationStorageSnapshot, String> {
    let roots = StorageRoots::from_app_home(&app_home_snapshot(app)?);
    let active = active_storage_identity(state)?;
    scan_application_storage(&roots, &active)
}

pub fn clear_preview_storage<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
) -> Result<StorageCleanupEffect, String> {
    let _lifecycle_gate = state
        .project_lifecycle_transition
        .lock()
        .map_err(|_| "ProjectLifecycle nu a putut proteja Preview-ul activ.".to_string())?;
    let home = app_home_snapshot(app)?;
    let roots = StorageRoots::from_app_home(&home);
    let active = active_storage_identity(state)?;
    let active_preview = active
        .zola_root
        .as_deref()
        .map(|root| preview_project_dir(app, root))
        .transpose()?;
    let _preview_gate = state
        .preview_workspace_operation
        .lock()
        .map_err(|_| "Operațiile cache-ului Preview nu au putut fi serializate.".to_string())?;
    let _source_browser_gate = state.source_browser_operation.lock().map_err(|_| {
        "Operațiile cache-ului Source Browser nu au putut fi serializate.".to_string()
    })?;
    let _version_preview_gate = state.version_preview_operation.lock().map_err(|_| {
        "Operațiile cache-ului Version Preview nu au putut fi serializate.".to_string()
    })?;

    let mut effect = StorageCleanupEffect::default();
    for entry in read_direct_children(&roots.preview_cache)? {
        let path = entry.path();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                effect.failures.push(format!(
                    "Nu am putut verifica intrarea Preview {}: {error}",
                    path.display()
                ));
                continue;
            }
        };
        let area = match scan_path(&path) {
            Ok(area) => area,
            Err(error) => {
                effect.failures.push(error);
                continue;
            }
        };
        if active_preview
            .as_ref()
            .is_some_and(|active| active == &path)
        {
            effect.protected_bytes = effect.protected_bytes.saturating_add(area.bytes);
            continue;
        }
        if !metadata.is_dir() {
            effect.failures.push(format!(
                "Intrarea Preview non-director a fost protejată: {}",
                path.display()
            ));
            effect.protected_bytes = effect.protected_bytes.saturating_add(area.bytes);
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            effect
                .failures
                .push("O intrare Preview cu nume non-UTF-8 a fost protejată.".into());
            effect.protected_bytes = effect.protected_bytes.saturating_add(area.bytes);
            continue;
        };
        if !valid_preview_storage_entry(&name) {
            effect.failures.push(format!(
                "Intrarea Preview necunoscută a fost protejată: {name}"
            ));
            effect.protected_bytes = effect.protected_bytes.saturating_add(area.bytes);
            continue;
        }
        let intent = storage_remove_tree_intent(
            path,
            roots.preview_cache.clone(),
            format!("storage/preview/{name}"),
        );
        match WriteAuthority::new(app).remove_directory_tree_if_exists(intent) {
            Ok(receipt) if receipt.status == "committed" => effect.removed_items += 1,
            Ok(_) => {}
            Err(error) => effect.failures.push(error.into_terminal_diagnostic()),
        }
    }
    Ok(effect)
}

pub fn clear_log_storage<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
) -> Result<StorageCleanupReceipt, String> {
    let before = read_application_storage(app, state)?;
    let (removed_items, _, failures) = clear_kernel_observability_logs(app)?;
    let after = read_application_storage(app, state)?;
    Ok(cleanup_receipt(
        "logs",
        removed_items,
        before.logs.area.bytes,
        after.logs.area.bytes,
        0,
        failures,
        after,
    ))
}

pub fn delete_storage_sessions<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    request: DeleteStorageSessionsRequest,
) -> Result<StorageCleanupReceipt, String> {
    let requested = request.session_ids.into_iter().collect::<BTreeSet<_>>();
    if requested.is_empty() {
        return Err("Selectează cel puțin o sesiune pentru ștergere.".to_string());
    }
    if requested.len() > 512 || requested.iter().any(|id| !valid_session_id(id)) {
        return Err("Lista sesiunilor conține un identificator invalid.".to_string());
    }
    let confirmed_recovery = request
        .confirmed_recovery_session_ids
        .into_iter()
        .collect::<BTreeSet<_>>();
    if !confirmed_recovery.is_subset(&requested) {
        return Err("Confirmarea recovery conține sesiuni care nu sunt selectate.".to_string());
    }

    let _lifecycle_gate = state
        .project_lifecycle_transition
        .lock()
        .map_err(|_| "ProjectLifecycle nu a putut proteja sesiunea activă.".to_string())?;
    let home = app_home_snapshot(app)?;
    let roots = StorageRoots::from_app_home(&home);
    let active = active_storage_identity(state)?;
    let before = scan_application_storage(&roots, &active)?;
    if before.sessions.revision != request.expected_revision {
        return Err(
            "Inventarul sesiunilor s-a schimbat. Recitește stocarea înainte de ștergere."
                .to_string(),
        );
    }
    let indexed = before
        .sessions
        .items
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    for id in &requested {
        let item = indexed
            .get(id.as_str())
            .ok_or_else(|| format!("Sesiunea {id} nu mai există în inventarul curent."))?;
        if item.active || !item.deletable {
            return Err(format!("Sesiunea activă {id} este protejată."));
        }
        if item.has_recovery && !confirmed_recovery.contains(id) {
            return Err(format!(
                "Sesiunea {id} conține recovery și cere confirmare nominală."
            ));
        }
    }

    let bytes_before = requested
        .iter()
        .filter_map(|id| indexed.get(id.as_str()))
        .fold(0_u64, |total, item| total.saturating_add(item.bytes));
    let mut removed_items = 0_usize;
    let mut failures = Vec::new();
    for id in &requested {
        let target = roots.sessions.join(id);
        let intent = storage_remove_tree_intent(
            target,
            roots.sessions.clone(),
            format!("storage/sessions/{id}"),
        );
        match WriteAuthority::new(app).remove_directory_tree_if_exists(intent) {
            Ok(receipt) if receipt.status == "committed" => removed_items += 1,
            Ok(_) => {}
            Err(error) => failures.push(error.into_terminal_diagnostic()),
        }
    }
    let after = scan_application_storage(&roots, &active_storage_identity(state)?)?;
    let bytes_after = after
        .sessions
        .items
        .iter()
        .filter(|item| requested.contains(&item.id))
        .fold(0_u64, |total, item| total.saturating_add(item.bytes));
    Ok(cleanup_receipt(
        "sessions",
        removed_items,
        bytes_before,
        bytes_after,
        0,
        failures,
        after,
    ))
}

pub fn cleanup_receipt(
    operation: &str,
    removed_items: usize,
    bytes_before: u64,
    bytes_after: u64,
    protected_bytes: u64,
    failures: Vec<String>,
    snapshot: ApplicationStorageSnapshot,
) -> StorageCleanupReceipt {
    StorageCleanupReceipt {
        schema_version: STORAGE_SCHEMA_VERSION,
        operation: operation.to_string(),
        removed_items,
        bytes_before,
        bytes_after,
        freed_bytes: bytes_before.saturating_sub(bytes_after),
        protected_bytes,
        failures,
        snapshot,
    }
}

fn scan_application_storage(
    roots: &StorageRoots,
    active: &ActiveStorageIdentity,
) -> Result<ApplicationStorageSnapshot, String> {
    let webkit = scan_path(&roots.webkit_cache)?;
    let preview = scan_path(&roots.preview_cache)?;
    let protected_preview_bytes = active
        .zola_root
        .as_deref()
        .map(preview_project_directory_name)
        .map(|name| scan_path(&roots.preview_cache.join(name)).map(|area| area.bytes))
        .transpose()?
        .unwrap_or(0);
    let webkit_cleanup_supported = cfg!(target_os = "linux");
    let webkit_reclaimable_bytes = if webkit_cleanup_supported {
        webkit.bytes
    } else {
        0
    };
    let cache = StorageCacheSnapshot {
        total_bytes: webkit.bytes.saturating_add(preview.bytes),
        reclaimable_bytes: webkit_reclaimable_bytes
            .saturating_add(preview.bytes.saturating_sub(protected_preview_bytes)),
        protected_preview_bytes,
        webkit_cleanup_supported,
        webkit,
        preview,
    };

    let logs_area = scan_path(&roots.logs)?;
    let active_log_bytes = fs::metadata(roots.logs.join("kernel.jsonl"))
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let archive_count = (1..=64)
        .take_while(|index| roots.logs.join(format!("kernel.jsonl.{index}")).is_file())
        .count();
    let logs = StorageLogsSnapshot {
        area: logs_area,
        active_bytes: active_log_bytes,
        archive_count,
    };
    let sessions = scan_sessions(&roots.sessions, active.session_id.as_deref())?;
    let total_bytes = cache
        .total_bytes
        .saturating_add(logs.area.bytes)
        .saturating_add(sessions.total_bytes);
    let reclaimable_bytes = cache
        .reclaimable_bytes
        .saturating_add(logs.area.bytes)
        .saturating_add(sessions.reclaimable_bytes);
    Ok(ApplicationStorageSnapshot {
        schema_version: STORAGE_SCHEMA_VERSION,
        scanned_at_ms: now_ms(),
        total_bytes,
        reclaimable_bytes,
        cache,
        logs,
        sessions,
    })
}

fn scan_sessions(
    root: &Path,
    active_session_id: Option<&str>,
) -> Result<StorageSessionsSnapshot, String> {
    let mut items = Vec::new();
    for entry in read_direct_children(root)? {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            format!("Nu am putut verifica sesiunea {}: {error}", path.display())
        })?;
        if !metadata.is_dir() {
            continue;
        }
        let Some(id) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !valid_session_id(&id) {
            continue;
        }
        items.push(scan_session(&path, id, active_session_id)?);
    }
    items.sort_by(|left, right| {
        right
            .active
            .cmp(&left.active)
            .then_with(|| right.last_seen_at_ms.cmp(&left.last_seen_at_ms))
            .then_with(|| left.id.cmp(&right.id))
    });
    let total_bytes = items
        .iter()
        .fold(0_u64, |total, item| total.saturating_add(item.bytes));
    let reclaimable_bytes = items
        .iter()
        .filter(|item| item.default_selected)
        .fold(0_u64, |total, item| total.saturating_add(item.bytes));
    let revision = session_revision(&items);
    Ok(StorageSessionsSnapshot {
        path: root.to_string_lossy().to_string(),
        revision,
        total_bytes,
        reclaimable_bytes,
        count: items.len(),
        orphan_count: items.iter().filter(|item| !item.project_exists).count(),
        recovery_count: items.iter().filter(|item| item.has_recovery).count(),
        active_count: items.iter().filter(|item| item.active).count(),
        items,
    })
}

fn scan_session(
    path: &Path,
    id: String,
    active_session_id: Option<&str>,
) -> Result<StorageSessionSnapshot, String> {
    let area = scan_path(path)?;
    let manifest_path = path.join("manifest.json");
    let manifest = read_session_manifest(&manifest_path);
    let (project_root, manifest_last_seen, manifest_status) = match manifest {
        Ok(Some((manifest_id, root, last_seen))) if manifest_id.is_empty() || manifest_id == id => {
            (root, last_seen, "readable".to_string())
        }
        Ok(Some((_manifest_id, root, last_seen))) => {
            (root, last_seen, "identity_mismatch".to_string())
        }
        Ok(None) => (String::new(), 0, "missing".to_string()),
        Err(_) => (String::new(), 0, "invalid".to_string()),
    };
    let mut recovery_signals = recovery_signals(path)?;
    if manifest_status != "readable" {
        recovery_signals.push(format!("manifest:{manifest_status}"));
    }
    recovery_signals.sort();
    recovery_signals.dedup();
    let has_recovery = !recovery_signals.is_empty();
    let active = active_session_id.is_some_and(|active_id| active_id == id);
    let project_exists = !project_root.is_empty() && Path::new(&project_root).is_dir();
    let project_name = Path::new(&project_root)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("Proiect necunoscut")
        .to_string();
    let last_seen_at_ms = manifest_last_seen.max(latest_modified_ms(path)?);
    Ok(StorageSessionSnapshot {
        id,
        project_name,
        project_root,
        bytes: area.bytes,
        entries: area.entries,
        last_seen_at_ms,
        project_exists,
        has_recovery,
        recovery_signals,
        manifest_status,
        active,
        deletable: !active,
        default_selected: !active && !has_recovery && !project_exists,
    })
}

fn read_session_manifest(path: &Path) -> Result<Option<(String, String, u64)>, String> {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let value: serde_json::Value =
        serde_json::from_str(&source).map_err(|error| error.to_string())?;
    let id = value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let project_root = value
        .get("projectRoot")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let last_seen = value
        .get("lastSeenAtMs")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    Ok(Some((id, project_root, last_seen)))
}

fn recovery_signals(path: &Path) -> Result<Vec<String>, String> {
    let mut signals = Vec::new();
    for entry in WalkDir::new(path).follow_links(false).min_depth(1) {
        let entry = entry.map_err(|error| {
            format!(
                "Nu am putut verifica recovery în {}: {error}",
                path.display()
            )
        })?;
        let relative = entry
            .path()
            .strip_prefix(path)
            .map_err(|error| error.to_string())?;
        let normalized = relative.to_string_lossy().replace('\\', "/");
        let file_name = entry.file_name().to_string_lossy();
        let recovery = matches!(
            file_name.as_ref(),
            "project-workspace.json"
                | "project-workspace.journal.jsonl"
                | "project-open-recovery-decision.json"
                | "autosave.json"
                | "transactions.jsonl"
        ) || normalized.starts_with("project-workspace-save/")
            || normalized.starts_with("workspace-edit-rollback/")
            || normalized.starts_with("generated-asset-journal/")
            || normalized.starts_with("project-transition-decision-retention/");
        if recovery && entry.file_type().is_file() {
            signals.push(normalized);
        }
    }
    Ok(signals)
}

fn active_storage_identity(state: &AppState) -> Result<ActiveStorageIdentity, String> {
    let slot = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut citi identitatea ProjectSession activă.".to_string())?;
    Ok(slot
        .as_ref()
        .map(|workspace| ActiveStorageIdentity {
            session_id: Some(workspace.session.id.clone()),
            zola_root: Some(PathBuf::from(&workspace.session.zola_root)),
        })
        .unwrap_or_default())
}

fn scan_path(path: &Path) -> Result<StorageAreaSnapshot, String> {
    if fs::symlink_metadata(path).is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound) {
        return Ok(StorageAreaSnapshot {
            path: path.to_string_lossy().to_string(),
            bytes: 0,
            entries: 0,
        });
    }
    let mut bytes = 0_u64;
    let mut entries = 0_u64;
    #[cfg(unix)]
    let mut physical_files = BTreeSet::<(u64, u64)>::new();
    for entry in WalkDir::new(path).follow_links(false) {
        let entry =
            entry.map_err(|error| format!("Nu am putut inventaria {}: {error}", path.display()))?;
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| format!("Nu am putut citi {}: {error}", entry.path().display()))?;
        if entry.path() != path {
            entries = entries.saturating_add(1);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;

            // Disk usage, not logical payload size. WebKit deliberately keeps
            // many hard-linked cache records; counting len() per path can
            // report the same physical allocation dozens of times.
            if physical_files.insert((metadata.dev(), metadata.ino())) {
                bytes = bytes.saturating_add(metadata.blocks().saturating_mul(512));
            }
        }
        #[cfg(not(unix))]
        if metadata.is_file() {
            bytes = bytes.saturating_add(metadata.len());
        }
    }
    Ok(StorageAreaSnapshot {
        path: path.to_string_lossy().to_string(),
        bytes,
        entries,
    })
}

fn latest_modified_ms(path: &Path) -> Result<u64, String> {
    let mut latest = 0_u64;
    for entry in WalkDir::new(path).follow_links(false) {
        let entry = entry.map_err(|error| error.to_string())?;
        let metadata = fs::symlink_metadata(entry.path()).map_err(|error| error.to_string())?;
        let modified = metadata
            .modified()
            .ok()
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map(|value| value.as_millis().min(u64::MAX as u128) as u64)
            .unwrap_or(0);
        latest = latest.max(modified);
    }
    Ok(latest)
}

fn read_direct_children(path: &Path) -> Result<Vec<fs::DirEntry>, String> {
    match fs::read_dir(path) {
        Ok(entries) => entries
            .map(|entry| {
                entry.map_err(|error| {
                    format!("Nu am putut citi intrarea din {}: {error}", path.display())
                })
            })
            .collect(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(format!("Nu am putut citi {}: {error}", path.display())),
    }
}

fn storage_remove_tree_intent(path: PathBuf, boundary: PathBuf, label: String) -> WriteIntent {
    WriteIntent::new(
        WriteCategory::InternalAppWrite,
        WriteOwner::StorageMaintenance,
        WriteOperationKind::RemoveDirectoryTree,
        WriteTarget::new(path, boundary, label),
        WritePolicy::internal_lifecycle(),
        "Curățare controlată Application Storage",
    )
}

fn session_revision(items: &[StorageSessionSnapshot]) -> String {
    let mut digest = Sha256::new();
    for item in items {
        digest.update(item.id.as_bytes());
        digest.update(item.bytes.to_le_bytes());
        digest.update(item.last_seen_at_ms.to_le_bytes());
        digest.update([u8::from(item.active), u8::from(item.has_recovery)]);
        digest.update(item.manifest_status.as_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn valid_session_id(id: &str) -> bool {
    id.len() == 16
        && id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_preview_storage_entry(name: &str) -> bool {
    name == "export"
        || name
            .strip_prefix("project-")
            .or_else(|| name.strip_prefix("template-sandbox-"))
            .is_some_and(valid_hex_suffix)
}

fn valid_hex_suffix(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use tauri::Manager;

    use super::*;
    use crate::{
        app_home::{ensure_app_home, TEST_APP_ENV_LOCK},
        kernel::write_authority::WriteAuthorityRuntime,
    };

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

    fn fixture(label: &str) -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "pana-application-storage-{}-{}-{label}",
            std::process::id(),
            id
        ))
    }

    fn roots(root: &Path) -> StorageRoots {
        StorageRoots {
            webkit_cache: root.join("data/WebKitCache"),
            preview_cache: root.join("cache/preview"),
            logs: root.join("data/logs/app"),
            sessions: root.join("data/sessions"),
        }
    }

    #[test]
    fn inventory_separates_rebuildable_cache_and_protected_recovery() {
        let root = fixture("inventory");
        let roots = roots(&root);
        fs::create_dir_all(roots.webkit_cache.join("Version 17")).unwrap();
        fs::write(
            roots.webkit_cache.join("Version 17/cache.bin"),
            vec![1_u8; 12],
        )
        .unwrap();
        let webkit_before_link = scan_path(&roots.webkit_cache).unwrap();
        fs::hard_link(
            roots.webkit_cache.join("Version 17/cache.bin"),
            roots.webkit_cache.join("Version 17/cache-link.bin"),
        )
        .unwrap();
        let webkit_after_link = scan_path(&roots.webkit_cache).unwrap();
        assert_eq!(webkit_after_link.bytes, webkit_before_link.bytes);
        fs::create_dir_all(roots.preview_cache.join("project-deadbeef/editor")).unwrap();
        fs::write(
            roots.preview_cache.join("project-deadbeef/editor/page"),
            vec![2_u8; 8],
        )
        .unwrap();
        fs::create_dir_all(roots.logs.clone()).unwrap();
        fs::write(roots.logs.join("kernel.jsonl"), b"log").unwrap();

        let ordinary = roots.sessions.join("0123456789abcdef");
        fs::create_dir_all(&ordinary).unwrap();
        fs::write(
            ordinary.join("manifest.json"),
            r#"{"id":"0123456789abcdef","projectRoot":"/missing/project","lastSeenAtMs":10}"#,
        )
        .unwrap();
        let recovery = roots.sessions.join("fedcba9876543210");
        fs::create_dir_all(&recovery).unwrap();
        fs::write(
            recovery.join("manifest.json"),
            r#"{"id":"fedcba9876543210","projectRoot":"/missing/recovery","lastSeenAtMs":20}"#,
        )
        .unwrap();
        fs::write(recovery.join("project-workspace.json"), b"recovery").unwrap();

        let snapshot = scan_application_storage(&roots, &ActiveStorageIdentity::default()).unwrap();
        assert_eq!(snapshot.cache.webkit.bytes, webkit_after_link.bytes);
        assert!(snapshot.cache.webkit.bytes > 0);
        assert!(snapshot.cache.preview.bytes > 0);
        assert_eq!(snapshot.logs.active_bytes, 3);
        assert_eq!(snapshot.sessions.count, 2);
        assert_eq!(snapshot.sessions.orphan_count, 2);
        assert_eq!(snapshot.sessions.recovery_count, 1);
        assert!(
            snapshot
                .sessions
                .items
                .iter()
                .find(|item| item.id == "0123456789abcdef")
                .unwrap()
                .default_selected
        );
        assert!(
            !snapshot
                .sessions
                .items
                .iter()
                .find(|item| item.id == "fedcba9876543210")
                .unwrap()
                .default_selected
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn active_and_invalid_sessions_are_never_selected_by_default() {
        let root = fixture("protected");
        let roots = roots(&root);
        let active = roots.sessions.join("0123456789abcdef");
        fs::create_dir_all(&active).unwrap();
        fs::write(
            active.join("manifest.json"),
            r#"{"id":"0123456789abcdef","projectRoot":"/missing/active","lastSeenAtMs":10}"#,
        )
        .unwrap();
        let invalid = roots.sessions.join("fedcba9876543210");
        fs::create_dir_all(&invalid).unwrap();
        fs::write(invalid.join("manifest.json"), b"not-json").unwrap();

        let sessions = scan_sessions(&roots.sessions, Some("0123456789abcdef")).unwrap();
        let active = sessions.items.iter().find(|item| item.active).unwrap();
        assert!(!active.deletable);
        assert!(!active.default_selected);
        let invalid = sessions
            .items
            .iter()
            .find(|item| item.id == "fedcba9876543210")
            .unwrap();
        assert!(invalid.has_recovery);
        assert!(!invalid.default_selected);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn storage_target_identifiers_are_strict() {
        for accepted in ["0123456789abcdef", "aaaaaaaaaaaaaaaa"] {
            assert!(valid_session_id(accepted));
        }
        for rejected in [
            "",
            "abc",
            "0123456789ABCDEf",
            "../sessions",
            "project-deadbeef",
        ] {
            assert!(!valid_session_id(rejected));
        }
        for accepted in ["export", "project-deadbeef", "template-sandbox-0123abcd"] {
            assert!(valid_preview_storage_entry(accepted));
        }
        for rejected in ["project-", "project-../data", "random", "WebKitCache"] {
            assert!(!valid_preview_storage_entry(rejected));
        }
    }

    #[test]
    fn session_cleanup_requires_recovery_confirmation_and_never_touches_project_data() {
        let _environment_lock = TEST_APP_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = fixture("session-cleanup");
        let _environment = TestEnvGuard::from_root(&root.join("app-home"));
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let app_handle = app.handle().clone();
        app_handle.manage(AppState::default());
        let home = ensure_app_home(&app_handle).unwrap();
        app_handle
            .state::<WriteAuthorityRuntime>()
            .boot_recovery()
            .unwrap();

        let project = root.join("real-project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("index.html"), b"project source").unwrap();
        let ordinary_id = "0123456789abcdef";
        let recovery_id = "fedcba9876543210";
        for (id, recovery) in [(ordinary_id, false), (recovery_id, true)] {
            let session = PathBuf::from(&home.sessions_dir).join(id);
            fs::create_dir_all(&session).unwrap();
            fs::write(
                session.join("manifest.json"),
                format!(
                    r#"{{"id":"{id}","projectRoot":"{}","lastSeenAtMs":10}}"#,
                    project.display()
                ),
            )
            .unwrap();
            if recovery {
                fs::write(session.join("project-workspace.json"), b"recovery").unwrap();
            }
        }
        let state = app_handle.state::<AppState>();
        let inventory = read_application_storage(&app_handle, state.inner()).unwrap();
        let rejected = delete_storage_sessions(
            &app_handle,
            state.inner(),
            DeleteStorageSessionsRequest {
                expected_revision: inventory.sessions.revision.clone(),
                session_ids: vec![ordinary_id.into(), recovery_id.into()],
                confirmed_recovery_session_ids: Vec::new(),
            },
        )
        .unwrap_err();
        assert!(rejected.contains("cere confirmare nominală"));
        assert!(PathBuf::from(&home.sessions_dir).join(ordinary_id).is_dir());
        assert!(PathBuf::from(&home.sessions_dir).join(recovery_id).is_dir());

        let receipt = delete_storage_sessions(
            &app_handle,
            state.inner(),
            DeleteStorageSessionsRequest {
                expected_revision: inventory.sessions.revision,
                session_ids: vec![ordinary_id.into(), recovery_id.into()],
                confirmed_recovery_session_ids: vec![recovery_id.into()],
            },
        )
        .unwrap();
        assert_eq!(receipt.removed_items, 2, "{receipt:#?}");
        assert!(receipt.freed_bytes > 0);
        assert_eq!(
            fs::read(project.join("index.html")).unwrap(),
            b"project source"
        );
        assert!(PathBuf::from(&home.write_authority_wal_dir).is_dir());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn log_cleanup_removes_only_declared_logs_and_preserves_wal() {
        let _environment_lock = TEST_APP_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = fixture("log-cleanup");
        let _environment = TestEnvGuard::from_root(&root.join("app-home"));
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let app_handle = app.handle().clone();
        app_handle.manage(AppState::default());
        let home = ensure_app_home(&app_handle).unwrap();
        let log_root = PathBuf::from(&home.app_logs_dir);
        let wal_root = PathBuf::from(&home.write_authority_wal_dir);
        fs::write(log_root.join("kernel.jsonl"), b"active").unwrap();
        fs::write(log_root.join("kernel.jsonl.1"), b"archive").unwrap();
        fs::write(wal_root.join("protected.json"), b"wal").unwrap();

        let state = app_handle.state::<AppState>();
        let receipt = clear_log_storage(&app_handle, state.inner()).unwrap();

        assert_eq!(receipt.removed_items, 2);
        assert!(receipt.freed_bytes >= 13);
        assert!(!log_root.join("kernel.jsonl").exists());
        assert!(!log_root.join("kernel.jsonl.1").exists());
        assert_eq!(fs::read(wal_root.join("protected.json")).unwrap(), b"wal");
        fs::remove_dir_all(root).unwrap();
    }

    struct TestEnvGuard {
        previous_values: Vec<(&'static str, Option<String>)>,
    }

    impl TestEnvGuard {
        fn from_root(root: &Path) -> Self {
            let bindings = [
                ("XDG_CONFIG_HOME", root.join("config")),
                ("XDG_DATA_HOME", root.join("data")),
                ("XDG_CACHE_HOME", root.join("cache")),
                ("XDG_STATE_HOME", root.join("state")),
            ];
            let previous_values = bindings
                .iter()
                .map(|(key, _)| (*key, env::var(key).ok()))
                .collect::<Vec<_>>();
            for (key, path) in bindings {
                env::set_var(key, path);
            }
            Self { previous_values }
        }
    }

    impl Drop for TestEnvGuard {
        fn drop(&mut self) {
            for (key, value) in self.previous_values.drain(..) {
                match value {
                    Some(previous) => env::set_var(key, previous),
                    None => env::remove_var(key),
                }
            }
        }
    }
}
