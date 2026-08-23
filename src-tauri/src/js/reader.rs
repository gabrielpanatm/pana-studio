use std::path::Path;

use crate::{
    kernel::file_buffer_store::{
        read_project_disk_text_snapshot, FileBufferStore, ProjectDiskTextReadOutcome,
    },
    project::strip_zola_root_prefix,
};

use super::{
    motion_source_relative_path, parse_motion_source, paths::normalize_template_path, PageJsConfig,
};

pub fn read_page_motion_config(
    project_root: &Path,
    store: &FileBufferStore,
    template_path: &str,
) -> Result<PageJsConfig, String> {
    let template_path = normalize_template_path(strip_zola_root_prefix(template_path))?;
    let relative_path = motion_source_relative_path(&template_path)?;
    let Some(content) = read_optional_project_text(project_root, store, &relative_path)? else {
        return Ok(PageJsConfig::default());
    };
    parse_motion_source(&content).map_err(|error| format!("{relative_path}: {error}"))
}

pub(super) fn read_optional_project_text(
    project_root: &Path,
    store: &FileBufferStore,
    relative_path: &str,
) -> Result<Option<String>, String> {
    if let Some(text) = store.text_for(relative_path) {
        return Ok(Some(text));
    }

    match read_project_disk_text_snapshot(project_root, relative_path, &store.limits) {
        ProjectDiskTextReadOutcome::Loaded(snapshot) => Ok(Some(snapshot.text)),
        ProjectDiskTextReadOutcome::Missing => Ok(None),
        ProjectDiskTextReadOutcome::NotFile => Err(format!(
            "Page JS a refuzat {relative_path}: target-ul nu este fișier regulat."
        )),
        ProjectDiskTextReadOutcome::Oversized(bytes) => Err(format!(
            "Page JS a refuzat {relative_path}: {bytes} bytes depășesc limita FileBufferStore de {} bytes.",
            store.limits.max_file_bytes
        )),
        ProjectDiskTextReadOutcome::InvalidPath(error)
        | ProjectDiskTextReadOutcome::UnsafePath(error)
        | ProjectDiskTextReadOutcome::Unstable(error)
        | ProjectDiskTextReadOutcome::Unreadable(error) => Err(format!(
            "Page JS nu poate citi sigur {relative_path}: {error}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::read_page_motion_config;
    use crate::kernel::{
        file_buffer_store::{FileBufferStore, FileBufferStoreLimits},
        project_session::{
            ProjectRootFingerprint, ProjectSessionScanSummary, ProjectSessionSnapshot,
        },
    };

    fn session(root: &std::path::Path) -> ProjectSessionSnapshot {
        let root = root.to_string_lossy().into_owned();
        ProjectSessionSnapshot {
            schema_version: 1,
            id: "page-js-reader".to_string(),
            project_root: root.clone(),
            zola_root: root.to_string(),
            session_dir: format!("{root}/session"),
            manifest_path: format!("{root}/session/session.json"),
            opened_at_ms: 1,
            last_seen_at_ms: 1,
            root_fingerprint: ProjectRootFingerprint {
                canonical_path: root,
                modified_ms: 1,
                size: 0,
                readonly: false,
                unix_device: None,
                unix_inode: None,
            },
            scan_summary: ProjectSessionScanSummary {
                active_theme: None,
                file_count: 0,
                directory_count: 0,
            },
        }
    }

    #[test]
    fn missing_page_sources_are_empty_without_creating_files() {
        let root = std::env::temp_dir().join(format!("pana-page-js-reader-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let live = session(&root);
        let store = FileBufferStore::for_project_session(
            &live,
            1,
            FileBufferStoreLimits {
                max_files: 10,
                max_file_bytes: 1024,
                max_total_bytes: 4096,
            },
        );

        assert_eq!(
            read_page_motion_config(&root, &store, "templates/index.html").unwrap(),
            Default::default()
        );
        assert!(!root.join("templates/index.html").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn portable_motion_source_reopens_after_the_project_moves() {
        let root = std::env::temp_dir().join(format!(
            "pana-motion-portable-{}-source",
            std::process::id()
        ));
        let moved =
            std::env::temp_dir().join(format!("pana-motion-portable-{}-moved", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&moved);
        fs::create_dir_all(root.join(".panastudio/motion/templates")).unwrap();
        fs::write(
            root.join(".panastudio/motion/templates/index.json"),
            r#"{
  "schemaVersion": 2,
  "customCode": [{
    "id": "portable",
    "name": "Portable",
    "code": "window.__portable=true"
  }]
}
"#,
        )
        .unwrap();
        fs::rename(&root, &moved).unwrap();

        let live = session(&moved);
        let store = FileBufferStore::for_project_session(
            &live,
            1,
            FileBufferStoreLimits {
                max_files: 10,
                max_file_bytes: 4096,
                max_total_bytes: 8192,
            },
        );
        let config = read_page_motion_config(&moved, &store, "templates/index.html").unwrap();

        assert_eq!(
            config.motion.as_ref().unwrap().custom_code[0].id,
            "portable"
        );
        assert!(!moved.join("static/js/pana-index.js").exists());
        fs::remove_dir_all(moved).unwrap();
    }

    #[test]
    fn page_js_reads_reject_parent_traversal_before_store_or_disk_access() {
        let root = std::env::temp_dir().join(format!(
            "pana-page-js-reader-traversal-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let live = session(&root);
        let store = FileBufferStore::for_project_session(
            &live,
            1,
            FileBufferStoreLimits {
                max_files: 10,
                max_file_bytes: 1024,
                max_total_bytes: 4096,
            },
        );

        let config_error = read_page_motion_config(&root, &store, r"..\secret.html").unwrap_err();

        assert!(config_error.contains("traversal"));
        fs::remove_dir_all(root).unwrap();
    }
}
