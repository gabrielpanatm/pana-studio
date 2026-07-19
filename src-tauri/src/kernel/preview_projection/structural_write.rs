use std::{collections::HashMap, path::Path};

use crate::{
    kernel::{
        file_buffer_store::now_ms,
        project_workspace::{
            ProjectWorkspace, ProjectWorkspaceIdentity, ProjectWorkspaceMutationReceipt,
            WorkspaceDocumentMutation, WorkspaceMutationMetadata,
        },
    },
    project_model::model::ProjectModel,
};

pub(crate) struct PreviewStructuralWrite {
    pub(crate) label: String,
    pub(crate) file: String,
    pub(crate) contents: String,
    pub(crate) coalesce_key: Option<String>,
}

pub(crate) fn source_texts_from_store(
    store: &crate::kernel::file_buffer_store::FileBufferStore,
) -> HashMap<String, String> {
    store
        .files
        .iter()
        .map(|(path, entry)| (path.clone(), entry.current_text().to_string()))
        .collect()
}

impl PreviewStructuralWrite {
    pub(crate) fn new(
        label: impl Into<String>,
        file: impl Into<String>,
        contents: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            file: file.into(),
            contents: contents.into(),
            coalesce_key: None,
        }
    }

    pub(crate) fn with_coalesce_key(mut self, coalesce_key: Option<String>) -> Self {
        self.coalesce_key = coalesce_key;
        self
    }
}

pub(crate) struct PreviewStructuralWriteCommit {
    pub(crate) workspace_mutation: ProjectWorkspaceMutationReceipt,
    pub(crate) after_model: ProjectModel,
}

/// Applies one structural preview edit to the in-memory workspace.
///
/// The candidate ProjectModel is built before the authoritative mutation is
/// published. Therefore a malformed candidate cannot partially advance the
/// workspace. No project file is written here; Save is the only disk boundary.
pub(crate) fn stage_preview_structural_write(
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    write: PreviewStructuralWrite,
) -> Result<PreviewStructuralWriteCommit, String> {
    let lease = workspace.capture_projection_lease()?;
    let mut candidate = lease;
    candidate.deleted_sources.remove(&write.file);
    candidate.changed_paths.insert(write.file.clone());
    candidate
        .source_texts
        .insert(write.file.clone(), write.contents.clone());
    let after_model = crate::project_model::build_project_model_from_workspace_projection(
        project_root,
        &candidate,
    )?;

    let identity = ProjectWorkspaceIdentity {
        expected_project_root: workspace.session.project_root.clone(),
        expected_session_id: workspace.runtime_session_id(),
        expected_revision: workspace.revision,
    };
    let workspace_mutation = workspace.stage_document_texts(
        &identity,
        WorkspaceMutationMetadata {
            label: write.label,
            source: "preview.structural".to_string(),
            coalesce_key: write.coalesce_key,
            transaction_id: None,
        },
        vec![WorkspaceDocumentMutation {
            relative_path: write.file,
            contents: write.contents,
        }],
        now_ms(),
    )?;

    Ok(PreviewStructuralWriteCommit {
        workspace_mutation,
        after_model,
    })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use crate::{
        js::PageJsDraftStore,
        kernel::{
            file_buffer_store::{
                hash_text, FileBufferBaseline, FileBufferEntry, FileBufferStore,
                FileBufferStoreLimits, TextBufferLanguage, TextBufferRole,
            },
            project_session::{
                ProjectRootFingerprint, ProjectSessionScanSummary, ProjectSessionSnapshot,
            },
            project_workspace::ProjectWorkspace,
        },
        project::{read_project_disk_manifest, AcceptedProjectDiskManifest},
    };

    use super::*;
    use crate::kernel::preview_projection::{CanvasPatch, CanvasPatchAnchor, CanvasPatchOperation};

    static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn structural_edit_advances_workspace_without_touching_disk() {
        let root = std::env::temp_dir().join(format!(
            "pana-preview-workspace-{}-{}",
            std::process::id(),
            NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("sursa/templates")).unwrap();
        fs::write(root.join("sursa/zola.toml"), "base_url = '/'\n").unwrap();
        let relative_path = "sursa/templates/index.html";
        let before = "<main>before</main>\n";
        let after = "<main>after</main>\n";
        fs::write(root.join(relative_path), before).unwrap();

        let session = test_session(&root);
        let runtime_session_id = session.runtime_instance_id();
        let manifest = read_project_disk_manifest(&root).unwrap();
        let accepted = AcceptedProjectDiskManifest::new(
            runtime_session_id.clone(),
            session.project_root.clone(),
            manifest,
        );
        let mut documents = FileBufferStore::new(
            runtime_session_id.clone(),
            session.project_root.clone(),
            1,
            FileBufferStoreLimits {
                max_files: 16,
                max_file_bytes: 1024 * 1024,
                max_total_bytes: 4 * 1024 * 1024,
            },
        );
        documents.files.insert(
            relative_path.to_string(),
            FileBufferEntry {
                relative_path: relative_path.to_string(),
                absolute_path: root.join(relative_path).to_string_lossy().into_owned(),
                language: TextBufferLanguage::Html,
                role: TextBufferRole::Template,
                baseline: FileBufferBaseline {
                    hash: hash_text(before),
                    modified_ms: 1,
                    size: before.len() as u64,
                    readonly: false,
                },
                baseline_text: before.to_string(),
                draft: None,
                revision: 0,
            },
        );
        let page_js = PageJsDraftStore::new(&session);
        let mut workspace =
            ProjectWorkspace::new(session, accepted.unwrap(), documents, page_js).unwrap();

        let receipt = stage_preview_structural_write(
            &root,
            &mut workspace,
            PreviewStructuralWrite::new("HTML text", relative_path, after)
                .with_coalesce_key(Some("preview.html.text:index-title".to_string())),
        )
        .unwrap();

        assert!(receipt.workspace_mutation.changed);
        assert_eq!(receipt.workspace_mutation.revision_after, 1);
        assert_eq!(
            workspace.documents.text_for(relative_path),
            Some(after.to_string())
        );
        assert_eq!(
            fs::read_to_string(root.join(relative_path)).unwrap(),
            before
        );
        let canvas_patch = CanvasPatch::issued(
            &workspace.session.project_root,
            &workspace.runtime_session_id(),
            &receipt.workspace_mutation,
            "model-before",
            &receipt.after_model.revision,
            CanvasPatchOperation::SetText {
                target: CanvasPatchAnchor::source("sg_0123456789abcdef", None, Some("main")),
                text: "after".to_string(),
            },
        )
        .unwrap();
        assert_eq!(
            canvas_patch.workspace_transaction_id,
            receipt.workspace_mutation.transaction_id.clone().unwrap()
        );
        assert_eq!(canvas_patch.base_workspace_revision, 0);
        assert_eq!(canvas_patch.workspace_revision, 1);
        assert!(canvas_patch.patch_id.starts_with("canvas_patch_"));

        let latest = "<main>latest</main>\n";
        let latest_receipt = stage_preview_structural_write(
            &root,
            &mut workspace,
            PreviewStructuralWrite::new("HTML text", relative_path, latest)
                .with_coalesce_key(Some("preview.html.text:index-title".to_string())),
        )
        .unwrap();
        assert_eq!(latest_receipt.workspace_mutation.revision_after, 2);
        assert_eq!(latest_receipt.workspace_mutation.history.undo_count, 1);
        let coalesced = latest_receipt
            .workspace_mutation
            .history
            .next_undo
            .as_ref()
            .unwrap();
        assert_eq!(coalesced.mutation_count, 2);
        assert_eq!(
            coalesced.coalesce_key.as_deref(),
            Some("preview.html.text:index-title")
        );
        assert_eq!(
            workspace.documents.text_for(relative_path),
            Some(latest.to_string())
        );
        assert_eq!(
            fs::read_to_string(root.join(relative_path)).unwrap(),
            before
        );

        let unchanged_receipt = stage_preview_structural_write(
            &root,
            &mut workspace,
            PreviewStructuralWrite::new("HTML text", relative_path, latest)
                .with_coalesce_key(Some("preview.html.text:index-title".to_string())),
        )
        .unwrap();
        assert!(!unchanged_receipt.workspace_mutation.changed);
        assert_eq!(unchanged_receipt.workspace_mutation.revision_before, 2);
        assert_eq!(unchanged_receipt.workspace_mutation.revision_after, 2);
        assert!(unchanged_receipt
            .workspace_mutation
            .transaction_id
            .is_none());
        assert_eq!(unchanged_receipt.workspace_mutation.history.undo_count, 1);
        fs::remove_dir_all(root).unwrap();
    }

    fn test_session(root: &Path) -> ProjectSessionSnapshot {
        let root_text = root.to_string_lossy().into_owned();
        ProjectSessionSnapshot {
            schema_version: 1,
            id: "session".to_string(),
            project_root: root_text.clone(),
            zola_root: root.join("sursa").to_string_lossy().into_owned(),
            session_dir: root.join(".session").to_string_lossy().into_owned(),
            manifest_path: root
                .join(".session/manifest.json")
                .to_string_lossy()
                .into_owned(),
            opened_at_ms: 1,
            last_seen_at_ms: 1,
            root_fingerprint: ProjectRootFingerprint {
                canonical_path: root_text,
                modified_ms: 1,
                size: 0,
                readonly: false,
                unix_device: Some("1".to_string()),
                unix_inode: Some("1".to_string()),
            },
            scan_summary: ProjectSessionScanSummary {
                is_zola: true,
                is_empty: false,
                active_theme: None,
                file_count: 2,
                directory_count: 2,
            },
        }
    }
}
