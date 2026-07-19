use std::{fs, io::ErrorKind, path::Path};

use tauri::{AppHandle, Runtime};

use crate::{
    kernel::{
        file_buffer_store::{hash_bytes, FileBufferBaseline, FileBufferSaveStamp, FileBufferStore},
        write_authority::{
            WriteAuthority, WriteCategory, WriteIntent, WriteOperationKind, WriteOwner,
            WritePolicy, WriteReceipt, WriteTarget,
        },
    },
    project::{project_disk_metadata_version_token, resolve_project_write_path},
};

#[cfg(test)]
thread_local! {
    static AFTER_TEXT_WRITE_BEFORE_FILE_BUFFER_PROJECTION_HOOK: std::cell::RefCell<
        Option<Box<dyn Fn(&mut FileBufferStore, &str)>>,
    > = std::cell::RefCell::new(None);
}

#[cfg(test)]
fn run_after_text_write_before_file_buffer_projection_hook(
    store: &mut FileBufferStore,
    relative_path: &str,
) {
    AFTER_TEXT_WRITE_BEFORE_FILE_BUFFER_PROJECTION_HOOK.with(|slot| {
        if let Some(hook) = slot.borrow().as_ref() {
            hook(store, relative_path);
        }
    });
}

#[cfg(not(test))]
fn run_after_text_write_before_file_buffer_projection_hook(
    _store: &mut FileBufferStore,
    _relative_path: &str,
) {
}

#[cfg(test)]
pub(crate) fn with_after_text_write_before_file_buffer_projection_hook_for_test<T>(
    hook: impl Fn(&mut FileBufferStore, &str) + 'static,
    operation: impl FnOnce() -> T,
) -> T {
    struct Reset;

    impl Drop for Reset {
        fn drop(&mut self) {
            AFTER_TEXT_WRITE_BEFORE_FILE_BUFFER_PROJECTION_HOOK.with(|slot| {
                slot.replace(None);
            });
        }
    }

    AFTER_TEXT_WRITE_BEFORE_FILE_BUFFER_PROJECTION_HOOK.with(|slot| {
        assert!(
            slot.replace(Some(Box::new(hook))).is_none(),
            "nested ProjectWorkspace projection hooks are not supported"
        );
    });
    let reset = Reset;
    let result = operation();
    drop(reset);
    result
}

use super::{
    disk::{read_disk_text_baseline, DiskTextBaseline},
    error::ProjectWorkspaceDiskError,
    model::{
        RemoveTextFileResult, SaveConflictDiagnostic, SaveTextFileResult, SaveTextFileStatus,
        SAVE_ENGINE_SCHEMA_VERSION,
    },
};

pub fn save_text_file<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &Path,
    store: &mut FileBufferStore,
    relative_path: &str,
    contents: String,
    expected_file_buffer: Option<&FileBufferSaveStamp>,
) -> Result<SaveTextFileResult, ProjectWorkspaceDiskError> {
    validate_store_identity(project_root, store)?;
    validate_store_limits(store, relative_path, &contents)?;
    if let Some(expected) = expected_file_buffer {
        store
            .require_save_stamp_current(relative_path, expected)
            .map_err(ProjectWorkspaceDiskError::rejected)?;
    }
    let file_buffer_before = store.save_stamp_for(relative_path);

    let path = resolve_project_write_path(project_root, relative_path)?;
    let baseline_before = store
        .files
        .get(relative_path)
        .map(|entry| entry.baseline.clone());
    let disk_before = read_disk_text_baseline(&path, &store.limits)?;
    let status = validate_save_preconditions(
        relative_path,
        baseline_before.as_ref(),
        disk_before.as_ref(),
    )
    .map_err(|diagnostic| ProjectWorkspaceDiskError::rejected(diagnostic.to_error()))?;

    let target = WriteTarget::new(
        path,
        project_root.to_path_buf(),
        format!("project/{relative_path}"),
    )
    .with_expected_runtime_session_id(store.runtime_session_id.clone());
    let target = match disk_before.as_ref() {
        Some(disk) => target
            .with_expected_present(disk.version_token.clone(), Some(disk.baseline.hash.clone())),
        None => target.with_expected_absent(),
    };
    let intent = WriteIntent::new(
        category_for_relative_path(relative_path),
        WriteOwner::ProjectWorkspace,
        WriteOperationKind::WriteText,
        target,
        WritePolicy::project_workspace_write(),
        format!("ProjectWorkspace Save text commit pentru project/{relative_path}"),
    );
    let receipt = WriteAuthority::new(app)
        .write_text(intent, &contents)
        .map_err(|error| ProjectWorkspaceDiskError::from_write_authority(relative_path, error))?;

    run_after_text_write_before_file_buffer_projection_hook(store, relative_path);
    let projection =
        match store.record_saved_text_if_current(relative_path, contents, expected_file_buffer) {
            Ok(projection) => projection,
            Err(error) => {
                return Err(
                    ProjectWorkspaceDiskError::file_buffer_projection_with_stamps(
                        relative_path,
                        receipt.clone(),
                        file_buffer_before.clone(),
                        store.save_stamp_for(relative_path),
                        format!(
                        "Save a fost comis, dar FileBufferStore nu s-a putut actualiza: {error}"
                    ),
                    ),
                );
            }
        };
    let baseline_after = store
        .files
        .get(relative_path)
        .map(|entry| entry.baseline.clone());

    Ok(SaveTextFileResult {
        schema_version: SAVE_ENGINE_SCHEMA_VERSION,
        relative_path: relative_path.to_string(),
        status,
        baseline_before,
        disk_before: disk_before.map(|disk| disk.baseline),
        baseline_after,
        file_buffer_before,
        file_buffer_after: projection.after,
        retained_newer_draft: projection.retained_newer_draft,
        bytes_written: receipt.bytes_written,
        receipt,
    })
}

pub fn save_binary_file<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &Path,
    runtime_session_id: &str,
    relative_path: &str,
    contents: &[u8],
    existed_before: bool,
    before_hash: &str,
) -> Result<WriteReceipt, ProjectWorkspaceDiskError> {
    let path = resolve_project_write_path(project_root, relative_path)?;
    let live = inspect_binary_target(&path)?;
    let target = WriteTarget::new(
        path,
        project_root.to_path_buf(),
        format!("project/{relative_path}"),
    )
    .with_expected_runtime_session_id(runtime_session_id.to_string());
    let target = match (existed_before, live) {
        (false, None) => target.with_expected_absent(),
        (false, Some(_)) => {
            return Err(ProjectWorkspaceDiskError::rejected(format!(
                "Save binar blocat pentru {relative_path}: target-ul create-only există deja."
            )))
        }
        (true, None) => {
            return Err(ProjectWorkspaceDiskError::rejected(format!(
                "Save binar blocat pentru {relative_path}: baseline-ul acceptat lipsește de pe disk."
            )))
        }
        (true, Some(live)) if live.hash != before_hash => {
            return Err(ProjectWorkspaceDiskError::rejected(format!(
                "Save binar blocat pentru {relative_path}: hash-ul disk nu mai corespunde baseline-ului tranzacției."
            )))
        }
        (true, Some(live)) => target.with_expected_present(live.version_token, Some(live.hash)),
    };
    let intent = WriteIntent::new(
        category_for_relative_path(relative_path),
        WriteOwner::ProjectWorkspace,
        WriteOperationKind::WriteBytes,
        target,
        WritePolicy::project_workspace_write(),
        format!("ProjectWorkspace Save binary commit pentru project/{relative_path}"),
    );
    WriteAuthority::new(app)
        .write_bytes(intent, contents)
        .map_err(|error| ProjectWorkspaceDiskError::from_write_authority(relative_path, error))
}

pub fn delete_binary_file<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &Path,
    runtime_session_id: &str,
    relative_path: &str,
    expected_hash: &str,
) -> Result<WriteReceipt, ProjectWorkspaceDiskError> {
    let path = resolve_project_write_path(project_root, relative_path)?;
    let live = inspect_binary_target(&path)?.ok_or_else(|| {
        ProjectWorkspaceDiskError::rejected(format!(
            "Delete binar blocat pentru {relative_path}: fișierul lipsește de pe disk."
        ))
    })?;
    if live.hash != expected_hash {
        return Err(ProjectWorkspaceDiskError::rejected(format!(
            "Delete binar blocat pentru {relative_path}: hash-ul disk nu mai corespunde baseline-ului tranzacției."
        )));
    }
    let intent = WriteIntent::new(
        category_for_relative_path(relative_path),
        WriteOwner::ProjectWorkspace,
        WriteOperationKind::RemoveFile,
        WriteTarget::new(
            path,
            project_root.to_path_buf(),
            format!("project/{relative_path}"),
        )
        .with_expected_runtime_session_id(runtime_session_id.to_string())
        .with_expected_present(live.version_token, Some(live.hash)),
        WritePolicy::project_workspace_remove(),
        format!("ProjectWorkspace Save binary delete pentru project/{relative_path}"),
    );
    let receipt = WriteAuthority::new(app)
        .remove_file_if_exists(intent)
        .map_err(|error| ProjectWorkspaceDiskError::from_write_authority(relative_path, error))?;
    if receipt.status != "committed" {
        return Err(ProjectWorkspaceDiskError::rejected(format!(
            "Delete binar blocat pentru {relative_path}: efectul nu a fost comis."
        )));
    }
    Ok(receipt)
}

struct BinaryTargetBaseline {
    hash: String,
    version_token: String,
}

fn inspect_binary_target(
    path: &Path,
) -> Result<Option<BinaryTargetBaseline>, ProjectWorkspaceDiskError> {
    let metadata_before = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(ProjectWorkspaceDiskError::rejected(format!(
                "Nu am putut inspecta resursa binară {}: {error}",
                path.display()
            )))
        }
    };
    if metadata_before.file_type().is_symlink() || !metadata_before.is_file() {
        return Err(ProjectWorkspaceDiskError::rejected(format!(
            "Resursa binară {} nu este fișier regular.",
            path.display()
        )));
    }
    if metadata_before.permissions().readonly() {
        return Err(ProjectWorkspaceDiskError::rejected(format!(
            "Resursa binară {} este readonly.",
            path.display()
        )));
    }
    let bytes = fs::read(path).map_err(|error| {
        ProjectWorkspaceDiskError::rejected(format!(
            "Nu am putut citi resursa binară {}: {error}",
            path.display()
        ))
    })?;
    let metadata_after = fs::metadata(path).map_err(|error| {
        ProjectWorkspaceDiskError::rejected(format!(
            "Nu am putut reverifica resursa binară {}: {error}",
            path.display()
        ))
    })?;
    let version_before = project_disk_metadata_version_token(&metadata_before);
    let version_after = project_disk_metadata_version_token(&metadata_after);
    if version_before != version_after {
        return Err(ProjectWorkspaceDiskError::rejected(format!(
            "Resursa binară {} s-a schimbat în timpul preflight-ului.",
            path.display()
        )));
    }
    Ok(Some(BinaryTargetBaseline {
        hash: hash_bytes(&bytes),
        version_token: version_after,
    }))
}

pub fn remove_created_text_file_for_undo<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &Path,
    store: &mut FileBufferStore,
    relative_path: &str,
    expected_hash: &str,
) -> Result<RemoveTextFileResult, ProjectWorkspaceDiskError> {
    validate_store_identity(project_root, store)?;

    let path = resolve_project_write_path(project_root, relative_path)?;
    let baseline_before = store
        .files
        .get(relative_path)
        .map(|entry| entry.baseline.clone())
        .ok_or_else(|| {
            format!(
                "Undo create blocat pentru {relative_path}: FileBufferStore nu are baseline curent."
            )
        })?;
    let disk_before = read_disk_text_baseline(&path, &store.limits)?.ok_or_else(|| {
        format!("Undo create blocat pentru {relative_path}: fișierul creat nu mai există pe disk.")
    })?;

    validate_remove_created_preconditions(
        relative_path,
        &baseline_before,
        &disk_before,
        expected_hash,
    )?;

    let intent = WriteIntent::new(
        category_for_relative_path(relative_path),
        WriteOwner::ProjectWorkspace,
        WriteOperationKind::RemoveFile,
        WriteTarget::new(
            path,
            project_root.to_path_buf(),
            format!("project/{relative_path}"),
        )
        .with_expected_runtime_session_id(store.runtime_session_id.clone())
        .with_expected_present(
            disk_before.version_token.clone(),
            Some(expected_hash.to_string()),
        ),
        WritePolicy::project_workspace_remove(),
        format!("ProjectWorkspace Save undo lifecycle remove pentru project/{relative_path}"),
    );
    let receipt = WriteAuthority::new(app)
        .remove_file_if_exists(intent)
        .map_err(|error| ProjectWorkspaceDiskError::from_write_authority(relative_path, error))?;
    if receipt.status != "committed" {
        return Err(ProjectWorkspaceDiskError::rejected(format!(
            "Undo create blocat pentru {relative_path}: fișierul a dispărut înainte ca lifecycle remove să fie comis."
        )));
    }

    if let Err(error) = store.record_removed_file(relative_path) {
        return Err(ProjectWorkspaceDiskError::file_buffer_projection(
            relative_path,
            receipt,
            format!(
                "Undo create a fost comis, dar FileBufferStore nu s-a putut actualiza: {error}"
            ),
        ));
    }

    Ok(RemoveTextFileResult {
        schema_version: SAVE_ENGINE_SCHEMA_VERSION,
        relative_path: relative_path.to_string(),
        baseline_before,
        disk_before: disk_before.baseline,
        expected_hash: expected_hash.to_string(),
        bytes_written: receipt.bytes_written,
        receipt,
    })
}

pub fn delete_text_file<R: Runtime>(
    app: &AppHandle<R>,
    project_root: &Path,
    store: &mut FileBufferStore,
    relative_path: &str,
) -> Result<RemoveTextFileResult, ProjectWorkspaceDiskError> {
    validate_store_identity(project_root, store)?;

    let path = resolve_project_write_path(project_root, relative_path)?;
    let entry = store.files.get(relative_path).ok_or_else(|| {
        format!("Delete text blocat pentru {relative_path}: FileBufferStore nu are baseline.")
    })?;
    if entry.draft.is_some() {
        return Err(ProjectWorkspaceDiskError::rejected(format!(
            "Delete text blocat pentru {relative_path}: fișierul are draft nesalvat în FileBufferStore."
        )));
    }
    let baseline_before = entry.baseline.clone();
    let disk_before = read_disk_text_baseline(&path, &store.limits)?.ok_or_else(|| {
        format!("Delete text blocat pentru {relative_path}: fișierul lipsește de pe disk.")
    })?;

    validate_delete_text_preconditions(relative_path, &baseline_before, &disk_before)?;

    let intent = WriteIntent::new(
        category_for_relative_path(relative_path),
        WriteOwner::ProjectWorkspace,
        WriteOperationKind::RemoveFile,
        WriteTarget::new(
            path,
            project_root.to_path_buf(),
            format!("project/{relative_path}"),
        )
        .with_expected_runtime_session_id(store.runtime_session_id.clone())
        .with_expected_present(
            disk_before.version_token.clone(),
            Some(baseline_before.hash.clone()),
        ),
        WritePolicy::project_workspace_remove(),
        format!("ProjectWorkspace Save delete text pentru project/{relative_path}"),
    );
    let receipt = WriteAuthority::new(app)
        .remove_file_if_exists(intent)
        .map_err(|error| ProjectWorkspaceDiskError::from_write_authority(relative_path, error))?;
    if receipt.status != "committed" {
        return Err(ProjectWorkspaceDiskError::rejected(format!(
            "Delete text blocat pentru {relative_path}: fișierul a dispărut înainte ca remove-ul să fie comis."
        )));
    }

    if let Err(error) = store.record_removed_file(relative_path) {
        return Err(ProjectWorkspaceDiskError::file_buffer_projection(
            relative_path,
            receipt,
            format!(
                "Delete text a fost comis, dar FileBufferStore nu s-a putut actualiza: {error}"
            ),
        ));
    }

    Ok(RemoveTextFileResult {
        schema_version: SAVE_ENGINE_SCHEMA_VERSION,
        relative_path: relative_path.to_string(),
        baseline_before: baseline_before.clone(),
        disk_before: disk_before.baseline,
        expected_hash: baseline_before.hash,
        bytes_written: receipt.bytes_written,
        receipt,
    })
}

fn validate_store_identity(project_root: &Path, store: &FileBufferStore) -> Result<(), String> {
    let expected = project_root.to_string_lossy();
    if store.project_root != expected {
        return Err(format!(
            "Save blocat: FileBufferStore aparține proiectului {}, dar proiectul curent este {}.",
            store.project_root, expected
        ));
    }
    Ok(())
}

fn validate_store_limits(
    store: &FileBufferStore,
    relative_path: &str,
    contents: &str,
) -> Result<(), String> {
    let bytes = contents.len() as u64;
    if bytes > store.limits.max_file_bytes {
        return Err(format!(
            "Save blocat pentru {relative_path}: conținutul are {bytes} bytes, peste limita de {} bytes.",
            store.limits.max_file_bytes
        ));
    }

    let is_new_buffer = !store.files.contains_key(relative_path);
    if is_new_buffer && store.files.len() >= store.limits.max_files {
        return Err(format!(
            "Save blocat pentru {relative_path}: FileBufferStore a atins limita de {} fișiere.",
            store.limits.max_files
        ));
    }

    let current_total = store
        .files
        .iter()
        .filter(|(path, _)| path.as_str() != relative_path)
        .map(|(_, entry)| entry.baseline_text.len() as u64)
        .sum::<u64>();
    let next_total = current_total.saturating_add(bytes);
    if next_total > store.limits.max_total_bytes {
        return Err(format!(
            "Save blocat pentru {relative_path}: FileBufferStore ar ajunge la {next_total} bytes, peste limita de {} bytes.",
            store.limits.max_total_bytes
        ));
    }

    Ok(())
}

fn validate_save_preconditions(
    relative_path: &str,
    baseline_before: Option<&FileBufferBaseline>,
    disk_before: Option<&DiskTextBaseline>,
) -> Result<SaveTextFileStatus, SaveConflictDiagnostic> {
    match (baseline_before, disk_before) {
        (None, None) => Ok(SaveTextFileStatus::Created),
        (None, Some(disk)) => Err(SaveConflictDiagnostic::missing_tracked_baseline(
            relative_path,
            Some(disk.baseline.clone()),
        )),
        (Some(expected), None) => Err(SaveConflictDiagnostic::missing_disk_file(
            relative_path,
            expected.clone(),
        )),
        (Some(expected), Some(disk)) => {
            if disk.baseline.readonly {
                return Err(SaveConflictDiagnostic::readonly_target(
                    relative_path,
                    disk.baseline.clone(),
                ));
            }
            if disk.baseline.hash != expected.hash {
                return Err(SaveConflictDiagnostic::disk_changed(
                    relative_path,
                    expected.clone(),
                    disk.baseline.clone(),
                ));
            }
            Ok(SaveTextFileStatus::Saved)
        }
    }
}

fn validate_remove_created_preconditions(
    relative_path: &str,
    baseline_before: &FileBufferBaseline,
    disk_before: &DiskTextBaseline,
    expected_hash: &str,
) -> Result<(), String> {
    if baseline_before.hash != expected_hash {
        return Err(format!(
            "Undo create blocat pentru {relative_path}: baseline-ul din sesiune nu mai corespunde intrării undo (baseline hash {}, expected hash {}).",
            baseline_before.hash, expected_hash
        ));
    }
    if disk_before.baseline.readonly {
        return Err(format!(
            "Undo create blocat pentru {relative_path}: fișierul de pe disk este readonly."
        ));
    }
    if disk_before.baseline.hash != expected_hash {
        return Err(format!(
            "Undo create blocat pentru {relative_path}: disk-ul s-a schimbat de la fișierul creat de tranzacție (expected hash {}, disk hash {}).",
            expected_hash, disk_before.baseline.hash
        ));
    }
    Ok(())
}

fn validate_delete_text_preconditions(
    relative_path: &str,
    baseline_before: &FileBufferBaseline,
    disk_before: &DiskTextBaseline,
) -> Result<(), String> {
    if disk_before.baseline.readonly {
        return Err(format!(
            "Delete text blocat pentru {relative_path}: fișierul de pe disk este readonly."
        ));
    }
    if disk_before.baseline.hash != baseline_before.hash {
        return Err(format!(
            "Delete text blocat pentru {relative_path}: disk-ul s-a schimbat față de baseline-ul sesiunii (baseline hash {}, disk hash {}).",
            baseline_before.hash, disk_before.baseline.hash
        ));
    }
    Ok(())
}

fn category_for_relative_path(relative_path: &str) -> WriteCategory {
    if relative_path.starts_with("design/") {
        return WriteCategory::ProjectDesignWrite;
    }
    WriteCategory::ProjectSourceWrite
}

#[cfg(test)]
mod tests {
    use std::{env, fs, path::Path};

    use crate::{
        app_home::{ensure_app_home, TEST_APP_ENV_LOCK},
        kernel::{
            file_buffer_store::{
                hash_text, FileBufferBaseline, FileBufferStore, FileBufferStoreLimits,
            },
            write_authority::{test_support::install_test_project_authority, WriteCategory},
        },
    };

    use super::{
        super::{
            disk::DiskTextBaseline,
            model::{SaveConflictReason, SaveTextFileStatus},
        },
        category_for_relative_path, save_text_file, validate_remove_created_preconditions,
        validate_save_preconditions,
        with_after_text_write_before_file_buffer_projection_hook_for_test,
    };

    #[test]
    fn preconditions_allow_new_file_without_disk_baseline() {
        let status = validate_save_preconditions("sursa/templates/nou.html", None, None).unwrap();

        assert_eq!(status, SaveTextFileStatus::Created);
    }

    #[test]
    fn preconditions_block_existing_untracked_file() {
        let disk = DiskTextBaseline {
            baseline: baseline("disk"),
            version_token: "test-version".to_string(),
        };
        let diagnostic =
            validate_save_preconditions("sursa/templates/index.html", None, Some(&disk))
                .unwrap_err();

        assert_eq!(
            diagnostic.reason,
            SaveConflictReason::MissingTrackedBaseline
        );
    }

    #[test]
    fn preconditions_block_changed_disk_hash() {
        let expected = baseline("session");
        let disk = DiskTextBaseline {
            baseline: baseline("disk"),
            version_token: "test-version".to_string(),
        };
        let diagnostic =
            validate_save_preconditions("sursa/templates/index.html", Some(&expected), Some(&disk))
                .unwrap_err();

        assert_eq!(diagnostic.reason, SaveConflictReason::DiskChanged);
    }

    #[test]
    fn preconditions_allow_same_hash_even_when_metadata_changed() {
        let expected = baseline("same");
        let mut actual = baseline("same");
        actual.modified_ms = actual.modified_ms.saturating_add(1000);
        let disk = DiskTextBaseline {
            baseline: actual,
            version_token: "test-version".to_string(),
        };
        let status =
            validate_save_preconditions("sursa/templates/index.html", Some(&expected), Some(&disk))
                .unwrap();

        assert_eq!(status, SaveTextFileStatus::Saved);
    }

    #[test]
    fn remove_created_preconditions_allow_matching_session_and_disk_hash() {
        let expected = baseline("created");
        let disk = DiskTextBaseline {
            baseline: baseline("created"),
            version_token: "test-version".to_string(),
        };

        validate_remove_created_preconditions(
            "sursa/templates/new.html",
            &expected,
            &disk,
            &expected.hash,
        )
        .unwrap();
    }

    #[test]
    fn remove_created_preconditions_block_changed_disk_hash() {
        let expected = baseline("created");
        let disk = DiskTextBaseline {
            baseline: baseline("changed"),
            version_token: "test-version".to_string(),
        };

        let error = validate_remove_created_preconditions(
            "sursa/templates/new.html",
            &expected,
            &disk,
            &expected.hash,
        )
        .unwrap_err();

        assert!(error.contains("disk-ul s-a schimbat"));
    }

    #[test]
    fn design_paths_use_design_write_category() {
        assert_eq!(
            category_for_relative_path("design/mood/shape.svg"),
            WriteCategory::ProjectDesignWrite
        );
        assert_eq!(
            category_for_relative_path("sursa/templates/index.html"),
            WriteCategory::ProjectSourceWrite
        );
    }

    #[test]
    fn committed_save_rebases_and_retains_draft_created_before_projection() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let workspace = std::env::temp_dir().join(format!(
            "pana-save-engine-file-buffer-cas-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&workspace);
        let _env_guard = TestEnvGuard::from_root(&workspace.join("app-home"));
        let project_root = workspace.join("project");
        let relative_path = "sursa/templates/index.html";
        fs::create_dir_all(project_root.join("sursa/templates")).unwrap();
        fs::write(project_root.join(relative_path), "baseline").unwrap();
        let project_root = project_root.canonicalize().unwrap();

        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock app");
        let app_home = ensure_app_home(app.handle()).unwrap();
        let session_dir = Path::new(&app_home.sessions_dir).join("save-cas-session");
        fs::create_dir_all(&session_dir).unwrap();
        install_test_project_authority(
            app.handle(),
            "save-cas/runtime-1",
            &project_root,
            &session_dir,
        )
        .unwrap();

        let mut store = FileBufferStore::new(
            "save-cas/runtime-1",
            project_root.to_string_lossy(),
            1,
            FileBufferStoreLimits {
                max_files: 10,
                max_file_bytes: 4096,
                max_total_bytes: 8192,
            },
        );
        store
            .record_saved_text(relative_path, "baseline".to_string())
            .unwrap();
        store
            .set_draft(relative_path, "captured draft".to_string(), 2)
            .unwrap();
        let captured = store.capture_dirty_save_snapshot(relative_path).unwrap();

        let result = with_after_text_write_before_file_buffer_projection_hook_for_test(
            |store, path| {
                store.set_draft(path, "newer draft".to_string(), 3).unwrap();
            },
            || {
                save_text_file(
                    app.handle(),
                    &project_root,
                    &mut store,
                    relative_path,
                    captured.contents.clone(),
                    Some(&captured.stamp),
                )
            },
        )
        .unwrap();

        assert_eq!(result.file_buffer_before, Some(captured.stamp));
        assert!(result.retained_newer_draft);
        assert!(result.file_buffer_after.dirty);
        assert_eq!(result.file_buffer_after.hash, hash_text("newer draft"));
        assert_eq!(
            store.text_for(relative_path).as_deref(),
            Some("newer draft")
        );
        assert_eq!(
            store.baseline_text_for(relative_path).as_deref(),
            Some("captured draft")
        );
        assert_eq!(
            fs::read_to_string(project_root.join(relative_path)).unwrap(),
            "captured draft"
        );

        drop(app);
        fs::remove_dir_all(workspace).unwrap();
    }

    fn baseline(text: &str) -> FileBufferBaseline {
        FileBufferBaseline {
            hash: hash_text(text),
            modified_ms: 1,
            size: text.len() as u64,
            readonly: false,
        }
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
            for (key, value) in &self.previous_values {
                if let Some(value) = value {
                    env::set_var(key, value);
                } else {
                    env::remove_var(key);
                }
            }
        }
    }
}
