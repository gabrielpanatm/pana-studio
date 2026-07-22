use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

use tauri::{AppHandle, Runtime};

use crate::{
    app_home::{project_session_dir, project_session_id, project_session_manifest_path},
    kernel::observability::{
        append_event, now_ms, KernelEventKind, KernelLogEvent, KernelLogLevel,
    },
    project::{
        model::{ProjectFileKind, ProjectScan},
        zola_project_root,
    },
};

use super::{
    fingerprint::fingerprint_project_root,
    manifest::write_session_manifest,
    model::{ProjectSessionScanSummary, ProjectSessionSnapshot},
};

const PROJECT_SESSION_SCHEMA_VERSION: u32 = 2;
static LAST_SESSION_OPENED_AT_MS: AtomicU64 = AtomicU64::new(0);

pub fn open_project_session<R: Runtime>(
    app: &AppHandle<R>,
    root: &Path,
    scan: &ProjectScan,
) -> Result<ProjectSessionSnapshot, String> {
    let session = prepare_project_session(app, root, scan)?;
    persist_project_session_open(app, &session)?;
    record_project_session_opened(app, &session);
    Ok(session)
}

pub fn prepare_project_session<R: Runtime>(
    app: &AppHandle<R>,
    root: &Path,
    scan: &ProjectScan,
) -> Result<ProjectSessionSnapshot, String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("Nu am putut rezolva rădăcina ProjectSession: {}", error))?;
    let project_root = root.to_string_lossy().to_string();
    let id = project_session_id(&project_root);
    let opened_at_ms = next_session_opened_at_ms();
    let session = ProjectSessionSnapshot {
        schema_version: PROJECT_SESSION_SCHEMA_VERSION,
        id: id.clone(),
        project_root: project_root.clone(),
        zola_root: zola_project_root(&root).to_string_lossy().to_string(),
        session_dir: project_session_dir(app, &project_root)?
            .to_string_lossy()
            .to_string(),
        manifest_path: project_session_manifest_path(app, &project_root)?
            .to_string_lossy()
            .to_string(),
        opened_at_ms,
        last_seen_at_ms: opened_at_ms,
        root_fingerprint: fingerprint_project_root(&root)?,
        scan_summary: ProjectSessionScanSummary {
            is_zola: scan.is_zola,
            is_empty: scan.is_empty,
            active_theme: scan.active_theme.clone(),
            file_count: scan
                .files
                .iter()
                .filter(|file| !matches!(&file.kind, ProjectFileKind::Dir))
                .count(),
            directory_count: scan
                .files
                .iter()
                .filter(|file| matches!(&file.kind, ProjectFileKind::Dir))
                .count(),
        },
    };

    Ok(session)
}

pub fn persist_project_session_open<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
) -> Result<(), String> {
    write_session_manifest(app, session)?;
    Ok(())
}

pub fn record_project_session_opened<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
) {
    if let Err(error) = append_event(
        app,
        KernelLogEvent::new(
            KernelLogLevel::Info,
            KernelEventKind::SessionOpened,
            "project_session",
            "internal_app_write",
            "open_project_session",
            Some(format!("session/{}", session.id)),
            format!("ProjectSession deschis pentru {}", session.project_root),
            None,
        )
        .with_attribute("sessionInstanceId", session.runtime_instance_id()),
    ) {
        eprintln!("[Pană Studio] session opened observability append failed: {error}");
    }
}

fn next_session_opened_at_ms() -> u128 {
    let wall_clock_ms = now_ms().min(u64::MAX as u128) as u64;
    let mut previous = LAST_SESSION_OPENED_AT_MS.load(Ordering::Relaxed);
    loop {
        let candidate = wall_clock_ms.max(previous.saturating_add(1));
        match LAST_SESSION_OPENED_AT_MS.compare_exchange_weak(
            previous,
            candidate,
            Ordering::SeqCst,
            Ordering::Relaxed,
        ) {
            Ok(_) => return candidate as u128,
            Err(observed) => previous = observed,
        }
    }
}
