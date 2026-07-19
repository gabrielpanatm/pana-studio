use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use tauri::{AppHandle, Runtime};

use crate::{
    app_home::app_home_snapshot,
    kernel::write_authority::{
        WriteAuthority, WriteCategory, WriteIntent, WriteOperationKind, WriteOwner, WritePolicy,
        WriteTarget,
    },
};

use super::model::{
    ScratchEntrySnapshot, ScratchMutationReceipt, ScratchTextSnapshot, MAX_SCRATCH_KEY_BYTES,
    MAX_SCRATCH_NAMESPACE_BYTES, MAX_SCRATCH_TEXT_BYTES,
};

struct ScratchEntryTarget {
    path: PathBuf,
    boundary_root: PathBuf,
    snapshot: ScratchEntrySnapshot,
}

pub fn write_scratch_text<R: Runtime>(
    app: &AppHandle<R>,
    namespace: &str,
    key: &str,
    contents: &str,
) -> Result<ScratchMutationReceipt, String> {
    if contents.len() > MAX_SCRATCH_TEXT_BYTES {
        return Err(format!(
            "ScratchState blocat: intrarea depășește limita de {} bytes.",
            MAX_SCRATCH_TEXT_BYTES
        ));
    }

    let target = scratch_entry_target(app, namespace, key)?;
    validate_regular_or_missing_leaf(&target.path, &target.snapshot.public_label)?;
    let intent = scratch_intent(
        &target,
        WriteOperationKind::WriteText,
        WritePolicy::scratch_state_atomic(),
        "Scrie scratch state intern rebuildable.",
    );
    let write = WriteAuthority::new(app)
        .write_text(intent, contents)
        .map_err(|error| error.into_terminal_diagnostic())?;

    Ok(ScratchMutationReceipt {
        entry: target.snapshot,
        write,
    })
}

pub fn read_scratch_text<R: Runtime>(
    app: &AppHandle<R>,
    namespace: &str,
    key: &str,
) -> Result<Option<ScratchTextSnapshot>, String> {
    let target = scratch_entry_target(app, namespace, key)?;
    let metadata = match fs::symlink_metadata(&target.path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "Nu am putut verifica scratch state {}: {}",
                target.snapshot.public_label, error
            ));
        }
    };

    if metadata.file_type().is_symlink() {
        return Err(format!(
            "ScratchState blocat: {} este symlink.",
            target.snapshot.public_label
        ));
    }
    if metadata.is_dir() {
        return Err(format!(
            "ScratchState blocat: {} este director.",
            target.snapshot.public_label
        ));
    }
    if metadata.len() > MAX_SCRATCH_TEXT_BYTES as u64 {
        return Err(format!(
            "ScratchState blocat: {} depășește limita de {} bytes.",
            target.snapshot.public_label, MAX_SCRATCH_TEXT_BYTES
        ));
    }

    let contents = fs::read_to_string(&target.path).map_err(|error| {
        format!(
            "Nu am putut citi scratch state {}: {}",
            target.snapshot.public_label, error
        )
    })?;
    let bytes = contents.len() as u64;

    Ok(Some(ScratchTextSnapshot {
        entry: target.snapshot,
        contents,
        bytes,
    }))
}

pub fn remove_scratch_entry<R: Runtime>(
    app: &AppHandle<R>,
    namespace: &str,
    key: &str,
) -> Result<ScratchMutationReceipt, String> {
    let target = scratch_entry_target(app, namespace, key)?;
    validate_regular_or_missing_leaf(&target.path, &target.snapshot.public_label)?;
    let intent = scratch_intent(
        &target,
        WriteOperationKind::RemoveFile,
        WritePolicy::scratch_state_lifecycle(),
        "Curăță scratch state intern rebuildable.",
    );
    let write = WriteAuthority::new(app)
        .remove_file_if_exists(intent)
        .map_err(|error| error.into_terminal_diagnostic())?;

    Ok(ScratchMutationReceipt {
        entry: target.snapshot,
        write,
    })
}

fn scratch_entry_target<R: Runtime>(
    app: &AppHandle<R>,
    namespace: &str,
    key: &str,
) -> Result<ScratchEntryTarget, String> {
    let namespace = normalized_token("namespace", namespace, MAX_SCRATCH_NAMESPACE_BYTES)?;
    let key = normalized_token("key", key, MAX_SCRATCH_KEY_BYTES)?;
    let boundary_root = PathBuf::from(app_home_snapshot(app)?.scratch_dir);
    let file_name = format!("{key}.json");
    let relative_path = format!("{namespace}/{file_name}");
    let path = boundary_root.join(&namespace).join(&file_name);
    let public_label = format!("scratch:{relative_path}");

    Ok(ScratchEntryTarget {
        path,
        boundary_root,
        snapshot: ScratchEntrySnapshot {
            namespace,
            key,
            relative_path,
            public_label,
        },
    })
}

fn normalized_token(label: &str, value: &str, max_bytes: usize) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("ScratchState blocat: {label} este gol."));
    }
    if trimmed.len() > max_bytes {
        return Err(format!(
            "ScratchState blocat: {label} depășește limita de {max_bytes} bytes."
        ));
    }
    if !trimmed
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(format!(
            "ScratchState blocat: {label} poate conține doar litere ASCII, cifre, '-' și '_'."
        ));
    }

    Ok(trimmed.to_string())
}

fn validate_regular_or_missing_leaf(path: &Path, public_label: &str) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(format!("ScratchState blocat: {public_label} este symlink."))
        }
        Ok(metadata) if metadata.is_dir() => Err(format!(
            "ScratchState blocat: {public_label} este director."
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "Nu am putut verifica scratch state {public_label}: {error}"
        )),
    }
}

fn scratch_intent(
    target: &ScratchEntryTarget,
    operation: WriteOperationKind,
    policy: WritePolicy,
    description: &str,
) -> WriteIntent {
    WriteIntent::new(
        WriteCategory::InternalAppWrite,
        WriteOwner::ScratchState,
        operation,
        WriteTarget::new(
            target.path.clone(),
            target.boundary_root.clone(),
            target.snapshot.public_label.clone(),
        ),
        policy,
        description,
    )
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::app_home::{ensure_app_home, TEST_APP_ENV_LOCK};

    use super::*;

    #[test]
    fn scratch_text_roundtrip_uses_application_home_cache() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = temp_dir("scratch-roundtrip");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");
        let app_handle = app.handle().clone();
        let app_home = ensure_app_home(&app_handle).expect("test app home should be available");

        let receipt = write_scratch_text(
            &app_handle,
            "preview",
            "last_projection",
            r#"{"status":"ok"}"#,
        )
        .expect("scratch write should succeed");

        assert_eq!(receipt.entry.relative_path, "preview/last_projection.json");
        assert_eq!(receipt.write.status, "committed");
        let expected_path = PathBuf::from(app_home.scratch_dir)
            .join("preview")
            .join("last_projection.json");
        assert_eq!(
            fs::read_to_string(&expected_path).unwrap(),
            r#"{"status":"ok"}"#
        );

        let snapshot = read_scratch_text(&app_handle, "preview", "last_projection")
            .expect("scratch read should succeed")
            .expect("scratch entry should exist");
        assert_eq!(snapshot.contents, r#"{"status":"ok"}"#);
        assert_eq!(snapshot.bytes, 15);

        let removal = remove_scratch_entry(&app_handle, "preview", "last_projection")
            .expect("scratch cleanup should succeed");
        assert_eq!(removal.write.status, "committed");
        assert!(!expected_path.exists());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scratch_entry_tokens_reject_path_segments() {
        let error = normalized_token("key", "../session", MAX_SCRATCH_KEY_BYTES).unwrap_err();

        assert!(error.contains("doar litere ASCII"));
    }

    #[test]
    fn scratch_text_write_enforces_size_budget() {
        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = temp_dir("scratch-size-budget");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");
        let app_handle = app.handle().clone();
        ensure_app_home(&app_handle).expect("test app home should be available");
        let contents = "x".repeat(MAX_SCRATCH_TEXT_BYTES + 1);

        let error = write_scratch_text(&app_handle, "kernel", "oversized", &contents).unwrap_err();

        assert!(error.contains("depășește limita"));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn scratch_text_write_blocks_symlink_leaf() {
        use std::os::unix::fs::symlink;

        let _lock = TEST_APP_ENV_LOCK.lock().unwrap();
        let root = temp_dir("scratch-symlink");
        let _env_guard = TestEnvGuard::from_root(&root.join("app-home"));
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("Tauri test app should build with mock context");
        let app_handle = app.handle().clone();
        let app_home = ensure_app_home(&app_handle).expect("test app home should be available");
        let namespace_dir = PathBuf::from(app_home.scratch_dir).join("preview");
        fs::create_dir_all(&namespace_dir).unwrap();
        symlink(
            root.join("outside.json"),
            namespace_dir.join("last_projection.json"),
        )
        .unwrap();

        let error = write_scratch_text(&app_handle, "preview", "last_projection", "{}")
            .expect_err("scratch write should block symlink leaf");

        assert!(error.contains("este symlink"));
        fs::remove_dir_all(root).unwrap();
    }

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("pana-studio-{name}-{stamp}"))
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
