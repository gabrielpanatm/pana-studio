use std::collections::HashSet;

use serde::Serialize;
use tauri::{AppHandle, Runtime, State};

use super::{
    kernel_preview_context::{require_preview_command_identity, PreviewWriteCommandContext},
    kernel_preview_outcome::PreviewStructuralCommandOutcome,
    kernel_preview_pipeline::run_preview_structural_write_command,
};
use crate::{
    kernel::{
        file_buffer_store::FileBufferStore,
        preview_projection::PreviewStructuralCommandIdentity,
        project_workspace::{
            ProjectWorkspace, ProjectWorkspaceIdentity, ProjectWorkspaceMutationReceipt,
            WorkspaceMutationMetadata, WorkspaceResourceMutation,
        },
    },
    project::{
        plan_site_archive_structure, plan_site_page_structure, plan_site_partial_include,
        plan_site_partial_structure, plan_site_single_structure, SiteArchiveStructureInput,
        SitePageStructureInput, SitePartialIncludeInput, SitePartialStructureInput,
        SiteSingleStructureInput, SiteTemplateWriteOrigin,
    },
    project_model::model::ProjectModel,
    source_graph::{build_template_catalog, SourceGraph, TemplateCatalogSnapshot},
    state::AppState,
};

const SITE_STRUCTURE_AUTHORITY_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SiteStructureAuthorityStatus {
    Noop,
    Staged,
}

/// Authority record for a Site Editor structural command staged in the
/// ProjectWorkspace. It is deliberately not a durable disk receipt.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteStructureAuthorityReceipt {
    pub schema_version: u32,
    pub operation_id: String,
    pub status: SiteStructureAuthorityStatus,
    pub project_root: String,
    pub session_id: String,
    pub revision_before: u64,
    pub revision_after: u64,
    pub dirty: bool,
    pub touched_files: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SitePageStructureCommandReceipt {
    pub slug: String,
    pub content_path: String,
    pub template_path: String,
    pub page_template: String,
    pub origin: SiteTemplateWriteOrigin,
    pub theme_name: Option<String>,
    pub created: Vec<String>,
    pub workspace_mutation: Option<ProjectWorkspaceMutationReceipt>,
    pub authority: SiteStructureAuthorityReceipt,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteArchiveStructureCommandReceipt {
    pub slug: String,
    pub content_path: String,
    pub template_path: String,
    pub archive_template: String,
    pub origin: SiteTemplateWriteOrigin,
    pub theme_name: Option<String>,
    pub created: Vec<String>,
    pub workspace_mutation: Option<ProjectWorkspaceMutationReceipt>,
    pub authority: SiteStructureAuthorityReceipt,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteSingleStructureCommandReceipt {
    pub section_slug: String,
    pub item_slug: String,
    pub item_path: String,
    pub template_path: String,
    pub single_template: String,
    pub origin: SiteTemplateWriteOrigin,
    pub theme_name: Option<String>,
    pub created: Vec<String>,
    pub workspace_mutation: Option<ProjectWorkspaceMutationReceipt>,
    pub authority: SiteStructureAuthorityReceipt,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SitePartialStructureCommandReceipt {
    pub path: String,
    pub template_name: String,
    pub origin: SiteTemplateWriteOrigin,
    pub theme_name: Option<String>,
    pub created: bool,
    pub workspace_mutation: Option<ProjectWorkspaceMutationReceipt>,
    pub authority: SiteStructureAuthorityReceipt,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SitePartialIncludeCommandReceipt {
    pub target_file: String,
    pub partial_template_name: String,
    /// True when either the partial was created or the include was inserted.
    pub changed: bool,
    pub include_changed: bool,
    pub partial_created: bool,
    pub partial_path: Option<String>,
    pub reason: String,
    pub workspace_mutation: Option<ProjectWorkspaceMutationReceipt>,
    pub authority: SiteStructureAuthorityReceipt,
}

#[derive(Clone, Debug)]
struct SiteStructureTextChange {
    relative_path: String,
    new_text: String,
}

#[tauri::command(async)]
pub fn read_source_graph(
    identity: PreviewStructuralCommandIdentity,
    state: State<AppState>,
) -> Result<SourceGraph, String> {
    read_source_graph_from_accepted_project(&identity, &state)
}

#[tauri::command(async)]
pub fn read_template_catalog(
    identity: PreviewStructuralCommandIdentity,
    state: State<AppState>,
) -> Result<TemplateCatalogSnapshot, String> {
    read_source_graph_from_accepted_project(&identity, &state)
        .map(|graph| build_template_catalog(&graph))
}

pub(crate) fn read_source_graph_from_accepted_project(
    identity: &PreviewStructuralCommandIdentity,
    state: &State<AppState>,
) -> Result<SourceGraph, String> {
    use crate::project_model::cache::{
        capture_project_model_build_lease, publish_project_model_if_current,
    };

    let (root, session, lease) = capture_project_model_build_lease(state)?;
    require_preview_command_identity(&session, identity)?;
    let model = crate::project_model::build_project_model_from_workspace_projection(
        &root,
        lease.projection(),
    )?;
    let graph = model.source_graph.clone();
    publish_project_model_if_current(state, &lease, model)?;
    Ok(graph)
}

#[tauri::command(async)]
pub fn create_site_page_structure(
    input: SitePageStructureInput,
    identity: PreviewStructuralCommandIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<SitePageStructureCommandReceipt, String> {
    run_preview_structural_write_command(
        &app,
        &state,
        &identity,
        "Create Site Editor page structure",
        |context, workspace| {
            let plan = plan_site_page_structure(&context.root, input)?;
            let changes = text_changes(&plan.changes);
            let create_only_paths = change_paths(&changes);
            let target = format!("site-editor/page/{}", plan.slug);
            execute_site_structure_workspace_mutation(
                context,
                workspace,
                "Create Site Editor page structure",
                target,
                changes,
                &create_only_paths,
                move |workspace_mutation, authority| SitePageStructureCommandReceipt {
                    slug: plan.slug,
                    content_path: plan.content_path,
                    template_path: plan.template_path,
                    page_template: plan.page_template,
                    origin: plan.origin,
                    theme_name: plan.theme_name,
                    created: plan.created,
                    workspace_mutation,
                    authority,
                },
            )
        },
    )
}

#[tauri::command(async)]
pub fn create_site_archive_structure(
    input: SiteArchiveStructureInput,
    identity: PreviewStructuralCommandIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<SiteArchiveStructureCommandReceipt, String> {
    run_preview_structural_write_command(
        &app,
        &state,
        &identity,
        "Create Site Editor archive structure",
        |context, workspace| {
            let plan = plan_site_archive_structure(&context.root, input)?;
            let changes = text_changes(&plan.changes);
            let create_only_paths = change_paths(&changes);
            let target = format!("site-editor/archive/{}", plan.slug);
            execute_site_structure_workspace_mutation(
                context,
                workspace,
                "Create Site Editor archive structure",
                target,
                changes,
                &create_only_paths,
                move |workspace_mutation, authority| SiteArchiveStructureCommandReceipt {
                    slug: plan.slug,
                    content_path: plan.content_path,
                    template_path: plan.template_path,
                    archive_template: plan.archive_template,
                    origin: plan.origin,
                    theme_name: plan.theme_name,
                    created: plan.created,
                    workspace_mutation,
                    authority,
                },
            )
        },
    )
}

#[tauri::command(async)]
pub fn create_site_single_structure(
    input: SiteSingleStructureInput,
    identity: PreviewStructuralCommandIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<SiteSingleStructureCommandReceipt, String> {
    run_preview_structural_write_command(
        &app,
        &state,
        &identity,
        "Create Site Editor single structure",
        |context, workspace| {
            let plan = plan_site_single_structure(&context.root, input)?;
            let changes = text_changes(&plan.changes);
            let create_only_paths = change_paths(&changes);
            let target = format!(
                "site-editor/single/{}/{}",
                plan.section_slug, plan.item_slug
            );
            execute_site_structure_workspace_mutation(
                context,
                workspace,
                "Create Site Editor single structure",
                target,
                changes,
                &create_only_paths,
                move |workspace_mutation, authority| SiteSingleStructureCommandReceipt {
                    section_slug: plan.section_slug,
                    item_slug: plan.item_slug,
                    item_path: plan.item_path,
                    template_path: plan.template_path,
                    single_template: plan.single_template,
                    origin: plan.origin,
                    theme_name: plan.theme_name,
                    created: plan.created,
                    workspace_mutation,
                    authority,
                },
            )
        },
    )
}

#[tauri::command(async)]
pub fn create_site_partial_structure(
    input: SitePartialStructureInput,
    identity: PreviewStructuralCommandIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<SitePartialStructureCommandReceipt, String> {
    create_site_partial_structure_impl(input, identity, &app, &state)
}

fn create_site_partial_structure_impl<R: Runtime>(
    input: SitePartialStructureInput,
    identity: PreviewStructuralCommandIdentity,
    app: &AppHandle<R>,
    state: &State<AppState>,
) -> Result<SitePartialStructureCommandReceipt, String> {
    run_preview_structural_write_command(
        app,
        state,
        &identity,
        "Create Site Editor partial structure",
        |context, workspace| {
            let plan = plan_site_partial_structure(&context.root, input)?;
            let changes = text_changes(&plan.changes);
            let create_only_paths = change_paths(&changes);
            let target = format!("site-editor/partial/{}", plan.template_name);
            execute_site_structure_workspace_mutation(
                context,
                workspace,
                "Create Site Editor partial structure",
                target,
                changes,
                &create_only_paths,
                move |workspace_mutation, authority| SitePartialStructureCommandReceipt {
                    path: plan.path,
                    template_name: plan.template_name,
                    origin: plan.origin,
                    theme_name: plan.theme_name,
                    created: plan.created,
                    workspace_mutation,
                    authority,
                },
            )
        },
    )
}

#[tauri::command(async)]
pub fn include_site_partial(
    input: SitePartialIncludeInput,
    identity: PreviewStructuralCommandIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<SitePartialIncludeCommandReceipt, String> {
    run_preview_structural_write_command(
        &app,
        &state,
        &identity,
        "Include Site Editor partial",
        |context, workspace| {
            let plan = plan_authoritative_partial_include(&context.root, workspace, input)?;
            let target = format!("site-editor/include/{}", plan.target_file);
            execute_site_structure_workspace_mutation(
                context,
                workspace,
                "Include Site Editor partial",
                target,
                plan.changes,
                &plan.create_only_paths,
                move |workspace_mutation, authority| SitePartialIncludeCommandReceipt {
                    target_file: plan.target_file,
                    partial_template_name: plan.partial_template_name,
                    changed: plan.include_changed || plan.partial_created,
                    include_changed: plan.include_changed,
                    partial_created: plan.partial_created,
                    partial_path: plan.partial_path,
                    reason: plan.reason,
                    workspace_mutation,
                    authority,
                },
            )
        },
    )
}

trait SiteStructureReceiptAuthority {
    fn authority(&self) -> &SiteStructureAuthorityReceipt;
}

macro_rules! site_structure_receipt_authority {
    ($($receipt:ty),+ $(,)?) => {
        $(
            impl SiteStructureReceiptAuthority for $receipt {
                fn authority(&self) -> &SiteStructureAuthorityReceipt {
                    &self.authority
                }

            }
        )+
    };
}

site_structure_receipt_authority!(
    SitePageStructureCommandReceipt,
    SiteArchiveStructureCommandReceipt,
    SiteSingleStructureCommandReceipt,
    SitePartialStructureCommandReceipt,
    SitePartialIncludeCommandReceipt,
);

struct SiteStructureCommandOutcome<R> {
    receipt: R,
    after_model: Option<ProjectModel>,
}

impl<R> PreviewStructuralCommandOutcome for SiteStructureCommandOutcome<R>
where
    R: SiteStructureReceiptAuthority,
{
    type Receipt = R;

    fn command_succeeded(&self) -> bool {
        self.receipt.authority().status == SiteStructureAuthorityStatus::Staged
    }

    fn after_model_mut(&mut self) -> &mut Option<ProjectModel> {
        &mut self.after_model
    }

    fn into_receipt(self) -> Self::Receipt {
        self.receipt
    }
}

fn execute_site_structure_workspace_mutation<R>(
    context: &PreviewWriteCommandContext,
    workspace: &mut ProjectWorkspace,
    label: &str,
    target: String,
    changes: Vec<SiteStructureTextChange>,
    create_only_paths: &[String],
    build_receipt: impl FnOnce(
        Option<ProjectWorkspaceMutationReceipt>,
        SiteStructureAuthorityReceipt,
    ) -> R,
) -> Result<SiteStructureCommandOutcome<R>, String>
where
    R: SiteStructureReceiptAuthority,
{
    if changes.is_empty() {
        let authority = site_structure_authority_receipt(context, None, &[]);
        return Ok(SiteStructureCommandOutcome {
            receipt: build_receipt(None, authority),
            after_model: None,
        });
    }

    ensure_site_structure_create_targets_untracked(
        &workspace.documents,
        &changes,
        create_only_paths,
    )?;
    let create_only = create_only_paths.iter().cloned().collect::<HashSet<_>>();
    let mut candidate = workspace.capture_projection_lease()?;
    for change in &changes {
        candidate.deleted_sources.remove(&change.relative_path);
        candidate.changed_paths.insert(change.relative_path.clone());
        candidate
            .source_texts
            .insert(change.relative_path.clone(), change.new_text.clone());
    }
    let after_model = crate::project_model::build_project_model_from_workspace_projection(
        &context.root,
        &candidate,
    )?;
    let identity = ProjectWorkspaceIdentity {
        expected_project_root: workspace.session.project_root.clone(),
        expected_session_id: workspace.runtime_session_id(),
        expected_revision: workspace.revision,
    };
    let mutation = workspace.stage_resource_texts(
        &identity,
        WorkspaceMutationMetadata {
            label: label.to_string(),
            source: "site_editor.structure".to_string(),
            coalesce_key: None,
            transaction_id: Some(target),
        },
        changes
            .into_iter()
            .map(|change| WorkspaceResourceMutation {
                create_only: create_only.contains(&change.relative_path),
                relative_path: change.relative_path,
                contents: change.new_text,
            })
            .collect(),
        crate::kernel::file_buffer_store::now_ms(),
    )?;
    let touched_files = mutation.touched_files.clone();
    let authority = site_structure_authority_receipt(context, Some(&mutation), &touched_files);
    Ok(SiteStructureCommandOutcome {
        receipt: build_receipt(Some(mutation), authority),
        after_model: Some(after_model),
    })
}

fn site_structure_authority_receipt(
    context: &PreviewWriteCommandContext,
    mutation: Option<&ProjectWorkspaceMutationReceipt>,
    touched_files: &[String],
) -> SiteStructureAuthorityReceipt {
    let revision_before = mutation
        .map(|receipt| receipt.revision_before)
        .unwrap_or(context.workspace_revision);
    let revision_after = mutation
        .map(|receipt| receipt.revision_after)
        .unwrap_or(context.workspace_revision);
    let operation_id = mutation
        .and_then(|receipt| receipt.transaction_id.clone())
        .map(|transaction_id| format!("site-structure/{transaction_id}"))
        .unwrap_or_else(|| format!("site-structure/noop/{revision_before}"));

    SiteStructureAuthorityReceipt {
        schema_version: SITE_STRUCTURE_AUTHORITY_SCHEMA_VERSION,
        operation_id,
        status: if mutation.is_some_and(|receipt| receipt.changed) {
            SiteStructureAuthorityStatus::Staged
        } else {
            SiteStructureAuthorityStatus::Noop
        },
        project_root: context.session.project_root.clone(),
        session_id: context.session.runtime_instance_id(),
        revision_before,
        revision_after,
        dirty: mutation.map(|receipt| receipt.dirty).unwrap_or(false),
        touched_files: touched_files.to_vec(),
    }
}

fn ensure_site_structure_create_targets_untracked(
    store: &FileBufferStore,
    changes: &[SiteStructureTextChange],
    create_only_paths: &[String],
) -> Result<(), String> {
    let change_paths = changes
        .iter()
        .map(|change| change.relative_path.as_str())
        .collect::<HashSet<_>>();
    for relative_path in create_only_paths {
        if !change_paths.contains(relative_path.as_str()) {
            return Err(format!(
                "Crearea structurii Site Editor are target create-only fără schimbare: {relative_path}."
            ));
        }
        if store.files.contains_key(relative_path) {
            return Err(format!(
                "Crearea structurii Site Editor a fost blocată pentru {relative_path}: FileBufferStore urmărește deja target-ul."
            ));
        }
    }
    Ok(())
}

struct AuthoritativePartialIncludePlan {
    target_file: String,
    partial_template_name: String,
    include_changed: bool,
    partial_created: bool,
    partial_path: Option<String>,
    reason: String,
    changes: Vec<SiteStructureTextChange>,
    create_only_paths: Vec<String>,
}

fn plan_authoritative_partial_include(
    project_root: &std::path::Path,
    workspace: &ProjectWorkspace,
    input: SitePartialIncludeInput,
) -> Result<AuthoritativePartialIncludePlan, String> {
    let store = &workspace.documents;
    let normalized = plan_site_partial_include("", input.clone())?;
    let source = store.text_for(&normalized.target_file).ok_or_else(|| {
        format!(
            "FileBufferStore nu are sursa curentă pentru {}.",
            normalized.target_file
        )
    })?;
    let generic_include = plan_site_partial_include(&source, input.clone())?;

    let mut changes = Vec::new();
    let mut create_only_paths = Vec::new();
    let mut partial_created = false;
    let mut partial_path = None;
    if let Some(ensure_partial) = input.ensure_partial.clone() {
        let partial_plan = plan_site_partial_structure(project_root, ensure_partial)?;
        if partial_plan.template_name != generic_include.partial_template_name {
            return Err(format!(
                "Partialul cerut pentru creare ({}) nu corespunde include-ului ({}).",
                partial_plan.template_name, generic_include.partial_template_name
            ));
        }
        partial_created = partial_plan.created;
        partial_path = Some(partial_plan.path.clone());
        let partial_changes = text_changes(&partial_plan.changes);
        create_only_paths.extend(change_paths(&partial_changes));
        changes.extend(partial_changes);
    }

    let include_changed = generic_include.changed;
    let reason = generic_include.reason.clone();
    if include_changed {
        let include_change = generic_include
            .changes
            .first()
            .map(|change| SiteStructureTextChange {
                relative_path: change.relative_path.clone(),
                new_text: change.new_text.clone(),
            })
            .ok_or_else(|| "Planul include nu conține schimbarea promisă.".to_string())?;
        changes.push(include_change);
    }

    ensure_unique_site_changes(&changes)?;
    Ok(AuthoritativePartialIncludePlan {
        target_file: generic_include.target_file,
        partial_template_name: generic_include.partial_template_name,
        include_changed,
        partial_created,
        partial_path,
        reason,
        changes,
        create_only_paths,
    })
}

fn ensure_unique_site_changes(changes: &[SiteStructureTextChange]) -> Result<(), String> {
    let mut seen = HashSet::new();
    for change in changes {
        if !seen.insert(change.relative_path.as_str()) {
            return Err(format!(
                "Site Editor a planificat de două ori același fișier în WorkspaceMutation: {}.",
                change.relative_path
            ));
        }
    }
    Ok(())
}

fn text_changes(changes: &[crate::project::SiteTextChange]) -> Vec<SiteStructureTextChange> {
    changes
        .iter()
        .map(|change| SiteStructureTextChange {
            relative_path: change.relative_path.clone(),
            new_text: change.new_text.clone(),
        })
        .collect()
}

fn change_paths(changes: &[SiteStructureTextChange]) -> Vec<String> {
    changes
        .iter()
        .map(|change| change.relative_path.clone())
        .collect()
}
