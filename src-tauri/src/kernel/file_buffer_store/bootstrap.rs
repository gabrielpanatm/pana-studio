use std::path::Path;

use tauri::{AppHandle, Runtime};

use crate::{
    kernel::{
        observability::{append_event, now_ms, KernelEventKind, KernelLogEvent, KernelLogLevel},
        project_session::ProjectSessionSnapshot,
    },
    project::model::ProjectScan,
};

use super::{
    model::{
        FileBufferDiagnostic, FileBufferStore, FileBufferStoreLimits,
        FILE_BUFFER_STORE_SCHEMA_VERSION,
    },
    reader::{load_text_file, LoadTextFileOutcome},
};

const MAX_BUFFER_FILES: usize = 500;
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 24 * 1024 * 1024;

pub fn bootstrap_file_buffer_store<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    scan: &ProjectScan,
) -> Result<FileBufferStore, String> {
    let limits = FileBufferStoreLimits {
        max_files: MAX_BUFFER_FILES,
        max_file_bytes: MAX_FILE_BYTES,
        max_total_bytes: MAX_TOTAL_BYTES,
    };
    let mut store = FileBufferStore::for_project_session(session, now_ms(), limits.clone());

    if store.schema_version != FILE_BUFFER_STORE_SCHEMA_VERSION {
        return Err("Schema FileBufferStore invalidă.".to_string());
    }

    let mut loaded_files = 0usize;
    let mut total_loaded_bytes = 0u64;

    for file in &scan.files {
        if loaded_files >= limits.max_files {
            store.add_diagnostic(FileBufferDiagnostic::warning(
                "max_files_reached",
                None,
                format!(
                    "FileBufferStore a încărcat limita de {} fișiere.",
                    limits.max_files
                ),
            ));
            break;
        }
        if total_loaded_bytes >= limits.max_total_bytes {
            store.add_diagnostic(FileBufferDiagnostic::warning(
                "max_total_bytes_reached",
                None,
                format!(
                    "FileBufferStore a încărcat limita totală de {} bytes.",
                    limits.max_total_bytes
                ),
            ));
            break;
        }

        match load_text_file(project_root, file, &limits) {
            LoadTextFileOutcome::Loaded(entry) => {
                let next_total =
                    total_loaded_bytes.saturating_add(entry.baseline_text.len() as u64);
                if next_total > limits.max_total_bytes {
                    store.add_diagnostic(FileBufferDiagnostic::warning(
                        "max_total_bytes_reached",
                        Some(entry.relative_path),
                        format!(
                            "Fișierul ar depăși limita totală FileBufferStore de {} bytes.",
                            limits.max_total_bytes
                        ),
                    ));
                    break;
                }
                total_loaded_bytes = next_total;
                loaded_files += 1;
                store.insert_loaded_file(entry);
            }
            LoadTextFileOutcome::Skipped(diagnostic) => {
                if diagnostic.code != "not_text_file" {
                    store.add_diagnostic(diagnostic);
                }
            }
        }
    }

    let snapshot = store.snapshot();
    append_event(
        app,
        KernelLogEvent::new(
            KernelLogLevel::Info,
            KernelEventKind::FileBufferStoreLoaded,
            "file_buffer_store",
            "internal_app_write",
            "bootstrap_file_buffer_store",
            Some(format!("session/{}", session.id)),
            format!(
                "FileBufferStore încărcat: {} fișiere, {} bytes, {} diagnostic(e).",
                snapshot.loaded_file_count,
                snapshot.total_loaded_bytes,
                snapshot.diagnostics.len()
            ),
            None,
        ),
    )?;

    Ok(store)
}
