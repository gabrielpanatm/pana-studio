use std::{collections::HashMap, path::Path};

use crate::{
    blocks::{plan_native_block_contract, NativeBlockContractRequest},
    css::page::page_scss_relative_path,
    js::{self, PageJsDraftStageInput},
    kernel::{
        file_buffer_store::now_ms,
        project_workspace::{
            ProjectWorkspace, ProjectWorkspaceIdentity, ProjectWorkspaceMutationReceipt,
            WorkspaceDocumentMutation, WorkspaceMutationMetadata, WorkspaceResourceDelete,
            WorkspaceResourceMutation,
        },
    },
    project::{strip_zola_root_prefix, zola_project_root},
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
    pub(crate) primary_contents: String,
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
    let identity = ProjectWorkspaceIdentity {
        expected_project_root: workspace.session.project_root.clone(),
        expected_session_id: workspace.runtime_session_id(),
        expected_revision: workspace.revision,
    };
    let metadata = WorkspaceMutationMetadata {
        label: write.label,
        source: "preview.structural".to_string(),
        coalesce_key: write.coalesce_key,
        transaction_id: None,
    };
    let mut candidate = workspace.clone();
    let workspace_mutation = if is_template_source(&write.file) {
        stage_structural_write_with_native_block_contract(
            project_root,
            &mut candidate,
            &identity,
            metadata,
            &write.file,
            &write.contents,
        )?
    } else {
        candidate.stage_document_texts(
            &identity,
            metadata,
            vec![WorkspaceDocumentMutation {
                relative_path: write.file.clone(),
                contents: write.contents,
            }],
            now_ms(),
        )?
    };
    let projection = candidate.capture_projection_lease()?;
    let after_model = crate::project_model::build_project_model_from_workspace_projection(
        project_root,
        &projection,
    )?;
    let primary_contents = candidate.documents.text_for(&write.file).ok_or_else(|| {
        format!(
            "Mutația structurală a pierdut sursa principală {}.",
            write.file
        )
    })?;
    *workspace = candidate;

    Ok(PreviewStructuralWriteCommit {
        workspace_mutation,
        after_model,
        primary_contents,
    })
}

fn is_template_source(relative_path: &str) -> bool {
    let normalized = relative_path
        .trim()
        .trim_start_matches('/')
        .replace('\\', "/");
    normalized.starts_with("templates/") && normalized.ends_with(".html")
}

fn stage_structural_write_with_native_block_contract(
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    identity: &ProjectWorkspaceIdentity,
    metadata: WorkspaceMutationMetadata,
    template_relative_path: &str,
    structural_contents: &str,
) -> Result<ProjectWorkspaceMutationReceipt, String> {
    let template_path = strip_zola_root_prefix(template_relative_path)
        .trim()
        .trim_start_matches('/')
        .to_string();
    let stylesheet_path = strip_zola_root_prefix(&page_scss_relative_path(&template_path))
        .trim()
        .trim_start_matches('/')
        .to_string();
    let stylesheet_snapshot = workspace.documents.text_snapshot(&stylesheet_path);
    let stylesheet_source = stylesheet_snapshot
        .as_ref()
        .map(|snapshot| snapshot.text.clone())
        .unwrap_or_default();
    let page_js_entry = workspace.page_js.drafts.get(&template_path);
    let page_js_config = match page_js_entry {
        Some(entry) => entry.current.clone(),
        None => js::read_page_js_config(project_root, &workspace.documents, &template_path)?,
    };
    let page_js_base_config = match page_js_entry {
        Some(entry) => entry.base.clone(),
        None => workspace
            .accepted_page_js_config(&template_path)
            .cloned()
            .unwrap_or_else(|| page_js_config.clone()),
    };
    let plan = plan_native_block_contract(NativeBlockContractRequest {
        template_path: template_path.clone(),
        template_source: structural_contents.to_string(),
        stylesheet_source: Some(stylesheet_source.clone()),
        page_js_config: Some(page_js_config),
        ensure_block_id: None,
        cachebust_assets: Some(false),
    });

    if plan.page_js_changed {
        let preflight = js::plan_page_js_save_for_project(
            &zola_project_root(project_root),
            &workspace.session,
            &workspace.documents,
            &template_path,
            plan.page_js_config.clone(),
            false,
        )?;
        if preflight.page_js_resource.blocked {
            return Err(preflight.page_js_resource.message);
        }
    }

    let requires_composite = plan.stylesheet.changed
        || plan.page_js_changed
        || plan.template.contents != structural_contents;
    if !requires_composite {
        return workspace.stage_document_texts(
            identity,
            metadata,
            vec![WorkspaceDocumentMutation {
                relative_path: template_relative_path.to_string(),
                contents: structural_contents.to_string(),
            }],
            now_ms(),
        );
    }

    let mut mutations = vec![WorkspaceResourceMutation {
        relative_path: template_relative_path.to_string(),
        contents: plan.template.contents.clone(),
        create_only: false,
    }];
    let mut deletes = Vec::new();
    if plan.stylesheet.changed {
        if plan.stylesheet.contents.trim().is_empty() && stylesheet_snapshot.is_some() {
            deletes.push(WorkspaceResourceDelete {
                relative_path: stylesheet_path.clone(),
            });
        } else {
            mutations.push(WorkspaceResourceMutation {
                relative_path: stylesheet_path,
                contents: plan.stylesheet.contents,
                create_only: stylesheet_snapshot.is_none(),
            });
        }
    }
    let page_js = plan.page_js_changed.then(|| PageJsDraftStageInput {
        template_path,
        expected_project_root: workspace.session.project_root.clone(),
        expected_session_id: workspace.runtime_session_id(),
        base_config: page_js_base_config,
        current_config: plan.page_js_config,
        cachebust_assets: false,
        source: Some("blocks.contract".to_string()),
        coalesce_key: None,
        transaction_id: None,
    });
    workspace.stage_composite_changes(identity, metadata, mutations, deletes, page_js, now_ms())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use crate::{
        blocks::NativeBlockOptionIntent,
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
        project_model::{
            attribute_engine::{plan_html_attributes, ProjectHtmlAttributeIntent},
            build_project_model,
            zola_image_engine::{
                apply_zola_image_contract, ProjectZolaImageIntent, ZolaImageFormat,
                ZolaImageOperation,
            },
        },
        source_graph::model::{BlockOptionValue, SourceNodeKind},
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
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(root.join("zola.toml"), "base_url = '/'\n").unwrap();
        let relative_path = "templates/index.html";
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

    #[test]
    fn zola_image_contract_is_one_undoable_workspace_transaction() {
        let root = std::env::temp_dir().join(format!(
            "pana-preview-zola-image-workspace-{}-{}",
            std::process::id(),
            NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("static/images")).unwrap();
        fs::write(root.join("zola.toml"), "base_url = '/'\n").unwrap();
        fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = 'Test'\ntemplate = 'index.html'\n+++\n",
        )
        .unwrap();
        fs::write(root.join("static/images/hero.jpg"), b"image").unwrap();
        let relative_path = "templates/index.html";
        let before = "<img src=\"/images/hero.jpg\" alt=\"Hero\">\n";
        fs::write(root.join(relative_path), before).unwrap();
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let contract = apply_zola_image_contract(
            &model,
            relative_path,
            before,
            0,
            &ProjectZolaImageIntent {
                enabled: true,
                source_url: Some("/images/hero.jpg".to_string()),
                source_path: Some("static/images/hero.jpg".to_string()),
                width: Some(960),
                height: None,
                operation: Some(ZolaImageOperation::FitWidth),
                format: Some(ZolaImageFormat::Webp),
                quality: Some(82),
            },
        )
        .unwrap();

        let session = test_session(&root);
        let runtime_session_id = session.runtime_instance_id();
        let manifest = read_project_disk_manifest(&root).unwrap();
        let accepted = AcceptedProjectDiskManifest::new(
            runtime_session_id.clone(),
            session.project_root.clone(),
            manifest,
        );
        let mut documents = FileBufferStore::new(
            runtime_session_id,
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

        let committed = stage_preview_structural_write(
            &root,
            &mut workspace,
            PreviewStructuralWrite::new("Imagine Zola", relative_path, &contract.contents)
                .with_coalesce_key(Some("preview.html.attributes:image".to_string())),
        )
        .unwrap();
        assert!(committed.workspace_mutation.changed);
        assert!(workspace
            .documents
            .text_for(relative_path)
            .unwrap()
            .contains("pana-studio:zola-image"));
        assert_eq!(
            fs::read_to_string(root.join(relative_path)).unwrap(),
            before
        );

        let undo_identity = ProjectWorkspaceIdentity {
            expected_project_root: workspace.session.project_root.clone(),
            expected_session_id: workspace.runtime_session_id(),
            expected_revision: workspace.revision,
        };
        workspace.undo(&undo_identity, 2).unwrap();
        assert_eq!(
            workspace.documents.text_for(relative_path),
            Some(before.to_string())
        );
        let redo_identity = ProjectWorkspaceIdentity {
            expected_project_root: workspace.session.project_root.clone(),
            expected_session_id: workspace.runtime_session_id(),
            expected_revision: workspace.revision,
        };
        workspace.redo(&redo_identity, 3).unwrap();
        assert_eq!(
            workspace.documents.text_for(relative_path),
            Some(contract.contents)
        );
        assert_eq!(
            fs::read_to_string(root.join(relative_path)).unwrap(),
            before
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn native_block_option_is_one_undoable_revision_and_same_value_is_noop() {
        let root = std::env::temp_dir().join(format!(
            "pana-preview-block-option-{}-{}",
            std::process::id(),
            NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::write(root.join("zola.toml"), "base_url = '/'\n").unwrap();
        fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = 'Test'\ntemplate = 'index.html'\n+++\n",
        )
        .unwrap();
        let relative_path = "templates/index.html";
        let before = concat!(
            "{% block content %}\n",
            "<div class=\"offcanvas\" data-pana-block=\"offcanvas\" ",
            "data-pana-instance=\"offcanvas-test\" data-pana-offcanvas-side=\"end\">\n",
            "  <button type=\"button\" data-offcanvas-close>Închide</button>\n",
            "</div>\n",
            "{% endblock content %}\n",
        );
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
            runtime_session_id,
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
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let block_marker = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::BlockMarker && node.label == "offcanvas")
            .expect("offcanvas marker");
        let block_root = model
            .source_graph
            .nodes
            .iter()
            .find(|node| block_marker.parent.as_deref() == Some(node.id.as_str()))
            .expect("offcanvas root");
        let plan = plan_html_attributes(
            &model,
            &ProjectHtmlAttributeIntent {
                target_source_id: Some(block_root.id.clone()),
                target_location: None,
                target_tag: Some("div".to_string()),
                target_selector: Some(".offcanvas".to_string()),
                attributes: Vec::new(),
                zola_image: None,
                native_block_option: Some(NativeBlockOptionIntent {
                    provider_id: "offcanvas".to_string(),
                    option_id: "side".to_string(),
                    value: BlockOptionValue::Text("start".to_string()),
                }),
            },
            &HashMap::new(),
        );
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.expect("native option patch");

        let committed = stage_preview_structural_write(
            &root,
            &mut workspace,
            PreviewStructuralWrite::new("Proprietate bloc", relative_path, &patch.contents),
        )
        .unwrap();
        assert!(committed.workspace_mutation.changed);
        assert_eq!(committed.workspace_mutation.revision_before, 0);
        assert_eq!(committed.workspace_mutation.revision_after, 1);
        assert_eq!(committed.workspace_mutation.history.undo_count, 1);
        assert!(workspace
            .documents
            .text_for(relative_path)
            .unwrap()
            .contains(r#"data-pana-offcanvas-side="start""#));
        assert_eq!(
            fs::read_to_string(root.join(relative_path)).unwrap(),
            before
        );

        let undo_identity = ProjectWorkspaceIdentity {
            expected_project_root: workspace.session.project_root.clone(),
            expected_session_id: workspace.runtime_session_id(),
            expected_revision: workspace.revision,
        };
        workspace.undo(&undo_identity, 2).unwrap();
        assert_eq!(
            workspace.documents.text_for(relative_path),
            Some(before.to_string())
        );
        let redo_identity = ProjectWorkspaceIdentity {
            expected_project_root: workspace.session.project_root.clone(),
            expected_session_id: workspace.runtime_session_id(),
            expected_revision: workspace.revision,
        };
        workspace.redo(&redo_identity, 3).unwrap();
        assert!(workspace
            .documents
            .text_for(relative_path)
            .unwrap()
            .contains(r#"data-pana-offcanvas-side="start""#));

        let unchanged_contents = workspace.documents.text_for(relative_path).unwrap();
        let unchanged = stage_preview_structural_write(
            &root,
            &mut workspace,
            PreviewStructuralWrite::new("Proprietate bloc", relative_path, unchanged_contents),
        )
        .unwrap();
        assert!(!unchanged.workspace_mutation.changed);
        assert_eq!(unchanged.workspace_mutation.revision_before, 3);
        assert_eq!(unchanged.workspace_mutation.revision_after, 3);
        assert!(unchanged.workspace_mutation.transaction_id.is_none());
        assert_eq!(unchanged.workspace_mutation.history.undo_count, 1);
        assert_eq!(
            fs::read_to_string(root.join(relative_path)).unwrap(),
            before
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn native_block_insert_and_last_delete_are_atomic_and_noop_safe() {
        let root = std::env::temp_dir().join(format!(
            "pana-preview-block-contract-{}-{}",
            std::process::id(),
            NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(root.join("zola.toml"), "base_url = '/'\n").unwrap();
        let relative_path = "templates/index.html";
        let before = "{% block content %}<main></main>{% endblock content %}\n";
        let with_block = r#"{% block content %}<main><span class="counter ps-counter-a" data-anim="ps-counter-a" data-pana-block="counter" data-pana-instance="counter-ps-counter-a" data-tinta="10">0</span></main>{% endblock content %}
"#;
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
            runtime_session_id,
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

        let inserted = stage_preview_structural_write(
            &root,
            &mut workspace,
            PreviewStructuralWrite::new("Adaugă bloc", relative_path, with_block),
        )
        .unwrap();

        assert!(inserted.workspace_mutation.changed);
        assert_eq!(inserted.workspace_mutation.revision_before, 0);
        assert_eq!(inserted.workspace_mutation.revision_after, 1);
        assert_eq!(inserted.workspace_mutation.history.undo_count, 1);
        assert_eq!(
            inserted
                .workspace_mutation
                .history
                .next_undo
                .as_ref()
                .unwrap()
                .document_paths,
            vec![
                "sass/pagini/index.scss".to_string(),
                relative_path.to_string()
            ]
        );
        assert!(workspace
            .documents
            .text_for(relative_path)
            .unwrap()
            .contains("data-pana-block=\"counter\""));
        assert!(workspace
            .documents
            .text_for("sass/pagini/index.scss")
            .unwrap()
            .contains("pana:block counter:start"));
        assert_eq!(
            workspace
                .page_js
                .drafts
                .get(relative_path)
                .unwrap()
                .current
                .blocks[0]
                .id,
            "counter"
        );

        let undo_identity = ProjectWorkspaceIdentity {
            expected_project_root: workspace.session.project_root.clone(),
            expected_session_id: workspace.runtime_session_id(),
            expected_revision: workspace.revision,
        };
        workspace.undo(&undo_identity, 2).unwrap();
        assert_eq!(
            workspace.documents.text_for(relative_path),
            Some(before.to_string())
        );
        assert!(workspace
            .documents
            .text_for("sass/pagini/index.scss")
            .is_none());
        assert!(!workspace.page_js.drafts.contains_key(relative_path));

        let redo_identity = ProjectWorkspaceIdentity {
            expected_project_root: workspace.session.project_root.clone(),
            expected_session_id: workspace.runtime_session_id(),
            expected_revision: workspace.revision,
        };
        workspace.redo(&redo_identity, 3).unwrap();
        assert!(workspace
            .documents
            .text_for("sass/pagini/index.scss")
            .is_some());
        assert!(workspace.page_js.drafts.contains_key(relative_path));

        let deleted = stage_preview_structural_write(
            &root,
            &mut workspace,
            PreviewStructuralWrite::new("Șterge ultimul bloc", relative_path, before),
        )
        .unwrap();
        assert!(deleted.workspace_mutation.changed);
        assert_eq!(deleted.workspace_mutation.revision_before, 3);
        assert_eq!(deleted.workspace_mutation.revision_after, 4);
        assert_eq!(
            workspace.documents.text_for(relative_path),
            Some(before.to_string())
        );
        assert!(workspace
            .documents
            .text_for("sass/pagini/index.scss")
            .is_none());
        assert!(!workspace.page_js.drafts.contains_key(relative_path));

        let unchanged = stage_preview_structural_write(
            &root,
            &mut workspace,
            PreviewStructuralWrite::new("Șterge ultimul bloc", relative_path, before),
        )
        .unwrap();
        assert!(!unchanged.workspace_mutation.changed);
        assert_eq!(unchanged.workspace_mutation.revision_before, 4);
        assert_eq!(unchanged.workspace_mutation.revision_after, 4);
        assert!(unchanged.workspace_mutation.transaction_id.is_none());
        assert_eq!(
            fs::read_to_string(root.join(relative_path)).unwrap(),
            before
        );
        fs::remove_dir_all(root).unwrap();
    }

    fn test_session(root: &Path) -> ProjectSessionSnapshot {
        let root_text = root.to_string_lossy().into_owned();
        ProjectSessionSnapshot {
            schema_version: 1,
            id: "session".to_string(),
            project_root: root_text.clone(),
            zola_root: root.to_path_buf().to_string_lossy().into_owned(),
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
