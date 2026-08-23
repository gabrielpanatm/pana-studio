use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use super::*;
use crate::{
    js::PageJsDraftStore,
    kernel::{
        file_buffer_store::{
            hash_text, FileBufferBaseline, FileBufferEntry, FileBufferStore, FileBufferStoreLimits,
            TextBufferLanguage, TextBufferRole,
        },
        project_session::{
            ProjectRootFingerprint, ProjectSessionScanSummary, ProjectSessionSnapshot,
        },
        project_workspace::ProjectWorkspace,
    },
    project::{read_project_disk_manifest, AcceptedProjectDiskManifest},
};

pub(super) fn fixture_root(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("pana-content-model-{name}-{nonce}"))
}

pub(super) fn field(
    id: &str,
    key: &str,
    kind: ContentFieldKind,
    fields: Vec<ContentFieldDefinition>,
) -> ContentFieldDefinition {
    ContentFieldDefinition {
        id: id.to_string(),
        key: key.to_string(),
        label: key.to_string(),
        kind,
        required: false,
        help: String::new(),
        default_value: None,
        choices: Vec::new(),
        minimum: None,
        maximum: None,
        pattern: None,
        fields,
    }
}

pub(super) fn test_workspace(root: &Path, sources: HashMap<String, String>) -> ProjectWorkspace {
    for (relative_path, source) in &sources {
        let absolute = root.join(relative_path);
        if let Some(parent) = absolute.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(absolute, source).unwrap();
    }
    let canonical = root.canonicalize().unwrap().to_string_lossy().to_string();
    let session = ProjectSessionSnapshot {
        schema_version: 1,
        id: "content-model-mutation-test".to_string(),
        project_root: canonical.clone(),
        zola_root: canonical.clone(),
        session_dir: root.join("session").to_string_lossy().to_string(),
        manifest_path: root.join("session.json").to_string_lossy().to_string(),
        opened_at_ms: 1,
        last_seen_at_ms: 1,
        root_fingerprint: ProjectRootFingerprint {
            canonical_path: canonical,
            modified_ms: 1,
            size: 0,
            readonly: false,
            unix_device: None,
            unix_inode: None,
        },
        scan_summary: ProjectSessionScanSummary {
            active_theme: None,
            file_count: sources.len(),
            directory_count: 2,
        },
    };
    let mut documents = FileBufferStore::for_project_session(
        &session,
        1,
        FileBufferStoreLimits {
            max_files: 64,
            max_file_bytes: 1024 * 1024,
            max_total_bytes: 4 * 1024 * 1024,
        },
    );
    for (relative_path, source) in sources {
        let (language, role) = if relative_path.ends_with(".html") {
            (TextBufferLanguage::Html, TextBufferRole::Template)
        } else if relative_path.ends_with(".md") {
            (TextBufferLanguage::Markdown, TextBufferRole::Page)
        } else {
            (TextBufferLanguage::Toml, TextBufferRole::Config)
        };
        documents.insert_loaded_file(FileBufferEntry {
            relative_path: relative_path.clone(),
            absolute_path: root.join(&relative_path).to_string_lossy().to_string(),
            language,
            role,
            baseline: FileBufferBaseline {
                hash: hash_text(&source),
                modified_ms: 1,
                size: source.len() as u64,
                readonly: false,
            },
            baseline_text: source.into(),
            draft: None,
            revision: 1,
        });
    }
    let accepted = AcceptedProjectDiskManifest::new(
        session.runtime_instance_id(),
        session.project_root.clone(),
        read_project_disk_manifest(root).unwrap(),
    )
    .unwrap();
    ProjectWorkspace::new(
        session.clone(),
        accepted,
        documents,
        PageJsDraftStore::new(&session),
    )
    .unwrap()
}
