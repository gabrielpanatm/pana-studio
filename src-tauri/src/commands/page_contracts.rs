use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicU64, Ordering},
};

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::{
    css::page::page_scss_relative_path,
    js::{self, PageJsConfig, PageJsDraftStageInput, PageJsDraftStageReceipt},
    kernel::{
        file_buffer_store::{FileBufferStore, FileBufferTextSnapshot},
        observability::now_ms,
        project_workspace::{
            commit_project_workspace_session_mutation, ProjectWorkspaceIdentity,
            ProjectWorkspaceMutationReceipt, WorkspaceMutationMetadata, WorkspaceResourceMutation,
        },
    },
    project::{strip_zola_root_prefix, zola_project_root},
    state::AppState,
};

pub(super) const PAGE_CONTRACT_AUTHORITY_SCHEMA_VERSION: u32 = 2;
static PAGE_CONTRACT_OPERATION_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PageContractApplyStatus {
    Noop,
    Staged,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageContractConsumedSourceRevision {
    pub relative_path: String,
    pub before_revision: Option<u64>,
    pub before_hash: Option<String>,
    pub after_revision: Option<u64>,
    pub after_hash: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageContractAuthorityReceipt {
    pub schema_version: u32,
    pub operation_id: String,
    pub status: PageContractApplyStatus,
    pub project_root: String,
    pub session_id: String,
    pub revision_before: u64,
    pub revision_after: u64,
    pub dirty: bool,
    pub consumed_sources: Vec<PageContractConsumedSourceRevision>,
    pub touched_files: Vec<String>,
}

pub(super) struct PageContractSourceSnapshot {
    pub template_path: String,
    pub template_relative_path: String,
    pub template_source: String,
    pub stylesheet_path: String,
    pub stylesheet_source: String,
    pub stylesheet_known: bool,
    pub page_js_config: PageJsConfig,
    pub page_js_base_config: PageJsConfig,
    template_revision: FileBufferTextSnapshot,
    stylesheet_revision: Option<FileBufferTextSnapshot>,
}

pub(super) struct PageContractMutationPlan {
    pub template_changed: bool,
    pub template_contents: String,
    pub stylesheet_changed: bool,
    pub stylesheet_contents: String,
    pub page_js_changed: bool,
    pub page_js_config: PageJsConfig,
}

pub(super) struct AppliedPageContract<Plan> {
    pub plan: Plan,
    pub workspace_mutation: Option<ProjectWorkspaceMutationReceipt>,
    pub page_js: Option<PageJsDraftStageReceipt>,
    pub authority: PageContractAuthorityReceipt,
}

pub(super) fn project_relative_zola_path(relative_path: &str) -> String {
    let stripped = strip_zola_root_prefix(relative_path)
        .trim()
        .trim_start_matches('/');
    stripped.to_string()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn apply_authoritative_page_contract<Plan>(
    app: &AppHandle,
    state: &State<AppState>,
    operation_label: &str,
    expected_project_root: &str,
    expected_session_id: &str,
    template_path: &str,
    cachebust_assets: bool,
    build_plan: impl FnOnce(&PageContractSourceSnapshot) -> Plan,
    mutation_plan: impl FnOnce(&Plan) -> PageContractMutationPlan,
) -> Result<AppliedPageContract<Plan>, String> {
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul pentru Page Contract.".to_string())?;
    let root = current_root
        .as_ref()
        .ok_or_else(|| "Page Contract cere un proiect deschis.".to_string())?;
    let mut slot = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru Page Contract.".to_string())?;
    let workspace = slot
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru Page Contract.".to_string())?;
    require_page_contract_identity(
        &workspace.session.project_root,
        &workspace.runtime_session_id(),
        expected_project_root,
        expected_session_id,
    )?;
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        root,
    )?;

    commit_project_workspace_session_mutation(app, workspace, |workspace| {
        let operation_id = format!(
            "page-contract-{:032x}-{:016x}",
            now_ms(),
            PAGE_CONTRACT_OPERATION_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        let revision_before = workspace.revision;
        let sources =
            read_page_contract_sources(root, &workspace.documents, workspace, template_path)?;
        let plan = build_plan(&sources);
        let mutation = mutation_plan(&plan);
        preflight_page_js_mutation(
            root,
            workspace,
            &sources.template_path,
            &mutation,
            cachebust_assets,
        )?;

        let mut resource_mutations = Vec::new();
        if mutation.template_changed {
            resource_mutations.push(WorkspaceResourceMutation {
                relative_path: sources.template_relative_path.clone(),
                contents: mutation.template_contents,
                create_only: false,
            });
        }
        if mutation.stylesheet_changed {
            resource_mutations.push(WorkspaceResourceMutation {
                relative_path: sources.stylesheet_path.clone(),
                contents: mutation.stylesheet_contents,
                create_only: !sources.stylesheet_known,
            });
        }
        let page_js_input = mutation.page_js_changed.then(|| PageJsDraftStageInput {
            template_path: sources.template_path.clone(),
            expected_project_root: workspace.session.project_root.clone(),
            expected_session_id: workspace.runtime_session_id(),
            base_config: sources.page_js_base_config.clone(),
            current_config: mutation.page_js_config,
            cachebust_assets,
            source: Some("page.contract".to_string()),
            coalesce_key: None,
            transaction_id: Some(operation_id.clone()),
        });
        let composite_receipt = workspace.stage_composite_changes(
            &workspace_identity(workspace),
            WorkspaceMutationMetadata {
                label: operation_label.to_string(),
                source: "page.contract".to_string(),
                coalesce_key: None,
                transaction_id: Some(operation_id.clone()),
            },
            resource_mutations,
            Vec::new(),
            page_js_input,
            now_ms(),
        )?;
        let page_js = composite_receipt.page_js.clone();
        let mut touched_files = BTreeSet::new();
        touched_files.extend(composite_receipt.touched_files.iter().cloned());
        let workspace_mutation = composite_receipt.changed.then_some(composite_receipt);

        workspace.accepted_disk.require_live_complete(
            &workspace.runtime_session_id(),
            &workspace.session.project_root,
            root,
        )?;
        let consumed_sources = vec![
            consumed_source_revision(
                &sources.template_relative_path,
                Some(&sources.template_revision),
                workspace
                    .documents
                    .text_snapshot(&sources.template_relative_path)
                    .as_ref(),
            ),
            consumed_source_revision(
                &sources.stylesheet_path,
                sources.stylesheet_revision.as_ref(),
                workspace
                    .documents
                    .text_snapshot(&sources.stylesheet_path)
                    .as_ref(),
            ),
        ];
        let revision_after = workspace.revision;
        let dirty = workspace.is_dirty();
        let status = if revision_after == revision_before {
            PageContractApplyStatus::Noop
        } else {
            PageContractApplyStatus::Staged
        };
        Ok(AppliedPageContract {
            plan,
            workspace_mutation,
            page_js,
            authority: PageContractAuthorityReceipt {
                schema_version: PAGE_CONTRACT_AUTHORITY_SCHEMA_VERSION,
                operation_id,
                status,
                project_root: workspace.session.project_root.clone(),
                session_id: workspace.runtime_session_id(),
                revision_before,
                revision_after,
                dirty,
                consumed_sources,
                touched_files: touched_files.into_iter().collect(),
            },
        })
    })
}

fn workspace_identity(
    workspace: &crate::kernel::project_workspace::ProjectWorkspace,
) -> ProjectWorkspaceIdentity {
    ProjectWorkspaceIdentity {
        expected_project_root: workspace.session.project_root.clone(),
        expected_session_id: workspace.runtime_session_id(),
        expected_revision: workspace.revision,
    }
}

fn require_page_contract_identity(
    live_project_root: &str,
    live_session_id: &str,
    expected_project_root: &str,
    expected_session_id: &str,
) -> Result<(), String> {
    if live_project_root != expected_project_root || live_session_id != expected_session_id {
        return Err(format!(
            "Page Contract a refuzat un request stale: așteptat root/session {expected_project_root}/{expected_session_id}, activ {live_project_root}/{live_session_id}."
        ));
    }
    Ok(())
}

fn read_page_contract_sources(
    project_root: &std::path::Path,
    store: &FileBufferStore,
    workspace: &crate::kernel::project_workspace::ProjectWorkspace,
    template_path: &str,
) -> Result<PageContractSourceSnapshot, String> {
    let template_path = strip_zola_root_prefix(template_path)
        .trim()
        .trim_start_matches('/')
        .to_string();
    if template_path.is_empty() {
        return Err("Page Contract a refuzat un template path gol.".to_string());
    }
    let template_relative_path = project_relative_zola_path(&template_path);
    let template_revision = store
        .text_snapshot(&template_relative_path)
        .ok_or_else(|| {
            format!(
                "Page Contract nu are snapshot ProjectWorkspace pentru {template_relative_path}."
            )
        })?;
    let stylesheet_path = project_relative_zola_path(&page_scss_relative_path(&template_path));
    let stylesheet_revision = store.text_snapshot(&stylesheet_path);
    let stylesheet_source = stylesheet_revision
        .as_ref()
        .map(|snapshot| snapshot.text.clone())
        .unwrap_or_default();
    let page_js_entry = workspace.page_js.drafts.get(&template_path);
    let disk_config = || js::read_page_js_config(project_root, store, &template_path);
    let page_js_config = match page_js_entry {
        Some(entry) => entry.current.clone(),
        None => disk_config()?,
    };
    let page_js_base_config = match page_js_entry {
        Some(entry) => entry.base.clone(),
        None => workspace
            .accepted_page_js_config(&template_path)
            .cloned()
            .unwrap_or_else(|| page_js_config.clone()),
    };

    Ok(PageContractSourceSnapshot {
        template_path,
        template_relative_path,
        template_source: template_revision.text.clone(),
        stylesheet_path,
        stylesheet_source,
        stylesheet_known: stylesheet_revision.is_some(),
        page_js_config,
        page_js_base_config,
        template_revision,
        stylesheet_revision,
    })
}

fn preflight_page_js_mutation(
    project_root: &std::path::Path,
    workspace: &crate::kernel::project_workspace::ProjectWorkspace,
    template_path: &str,
    mutation: &PageContractMutationPlan,
    cachebust_assets: bool,
) -> Result<(), String> {
    if !mutation.page_js_changed {
        return Ok(());
    }
    let preflight = js::plan_page_js_save_for_project(
        &zola_project_root(project_root),
        &workspace.session,
        &workspace.documents,
        template_path,
        mutation.page_js_config.clone(),
        cachebust_assets,
    )?;
    if preflight.page_js_resource.blocked {
        return Err(preflight.page_js_resource.message);
    }
    Ok(())
}

fn consumed_source_revision(
    relative_path: &str,
    before: Option<&FileBufferTextSnapshot>,
    after: Option<&FileBufferTextSnapshot>,
) -> PageContractConsumedSourceRevision {
    PageContractConsumedSourceRevision {
        relative_path: relative_path.to_string(),
        before_revision: before.map(|snapshot| snapshot.revision),
        before_hash: before.map(|snapshot| snapshot.hash.clone()),
        after_revision: after.map(|snapshot| snapshot.revision),
        after_hash: after.map(|snapshot| snapshot.hash.clone()),
    }
}
