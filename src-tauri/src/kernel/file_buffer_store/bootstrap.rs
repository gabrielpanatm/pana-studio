use std::path::Path;

use tauri::{AppHandle, Runtime};

use crate::{
    kernel::{
        observability::{append_event, now_ms, KernelEventKind, KernelLogEvent, KernelLogLevel},
        project_session::ProjectSessionSnapshot,
    },
    project::{model::ProjectScan, PROJECT_CAPACITY},
};

use super::{
    classify::classify_project_file,
    model::{
        FileBufferDiagnostic, FileBufferStore, FileBufferStoreLimits,
        FILE_BUFFER_STORE_SCHEMA_VERSION,
    },
    reader::{load_text_file, LoadTextFileOutcome},
};

pub fn bootstrap_file_buffer_store<R: Runtime>(
    app: &AppHandle<R>,
    session: &ProjectSessionSnapshot,
    project_root: &Path,
    scan: &ProjectScan,
) -> Result<FileBufferStore, String> {
    let limits = FileBufferStoreLimits {
        max_files: PROJECT_CAPACITY.max_resident_text_documents,
        max_file_bytes: PROJECT_CAPACITY.max_text_document_bytes,
        max_total_bytes: PROJECT_CAPACITY.max_resident_text_bytes,
    };
    let mut store = FileBufferStore::for_project_session(session, now_ms(), limits.clone());

    if store.schema_version != FILE_BUFFER_STORE_SCHEMA_VERSION {
        return Err("Schema FileBufferStore invalidă.".to_string());
    }

    let mut loaded_files = 0usize;
    let mut total_loaded_bytes = 0u64;

    for file in &scan.files {
        match load_text_file(project_root, file, &limits) {
            LoadTextFileOutcome::Loaded(entry) => {
                if loaded_files >= limits.max_files {
                    store.add_diagnostic(FileBufferDiagnostic::warning(
                        "max_files_reached",
                        Some(entry.relative_path),
                        format!(
                            "Fișierul ar depăși limita FileBufferStore de {} documente text rezidente.",
                            limits.max_files
                        ),
                    ));
                    break;
                }
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

    let mut missing_text_documents = scan
        .files
        .iter()
        .filter(|file| classify_project_file(file).is_some())
        .filter(|file| !store.files.contains_key(&file.relative_path))
        .map(|file| file.relative_path.as_str());
    if let Some(first_missing) = missing_text_documents.next() {
        let missing_count = 1usize.saturating_add(missing_text_documents.count());
        let diagnostic = store
            .diagnostics
            .first()
            .map(|diagnostic| diagnostic.message.as_str())
            .unwrap_or("nu a fost publicat niciun diagnostic");
        return Err(format!(
            "FileBufferStore a refuzat un namespace text incomplet: lipsesc {missing_count} document(e), primul este {first_missing}. Diagnostic: {diagnostic}."
        ));
    }

    Ok(store)
}
