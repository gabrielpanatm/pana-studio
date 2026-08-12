use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::Path,
};
use tauri::{AppHandle, State};

use crate::{
    css::{
        page::{
            consumer_stylesheet_imports_reusable, page_css_href, page_scss_relative_path,
            page_target_for_template, plan_page_stylesheet_link_writes_with_reader,
            prepare_page_stylesheet_source, prepare_reusable_consumer_stylesheet_source,
            remove_page_stylesheet_link, reusable_scss_relative_path, PageCssTarget,
            PageCssWriteResult, ReusableCssWriteResult, WrittenProjectFile,
        },
        rules::{selector_source_target, upsert_css_rule_desktop},
        validation::{
            normalize_panel_rule_input, validate_panel_rule_input, validate_panel_variable_value,
        },
        variables::{
            parse_variables_from_source, update_variable_in_source, variable_value_in_source,
            ScssVariable,
        },
        viewport::{get_rule_context, write_rule_at_viewport, CssBreakpointValues, CssRuleContext},
    },
    kernel::{
        file_buffer_store::{
            read_project_disk_text_snapshot, require_file_buffer_session_binding,
            FileBufferCommandReceipt, FileBufferRequestIdentity, FileBufferStore,
            ProjectDiskTextReadOutcome,
        },
        project_session::ProjectSessionSnapshot,
        project_workspace::{
            commit_project_workspace_session_mutation, ProjectWorkspace, ProjectWorkspaceIdentity,
            ProjectWorkspaceMutationReceipt, WorkspaceDocumentProjection,
            WorkspaceMutationMetadata, WorkspaceResourceDelete, WorkspaceResourceMutation,
            WorkspaceTextChange, WorkspaceTextDelete, WorkspaceTextResourceMutationInput,
        },
        selection_coordinator::SelectionMutationIdentity,
    },
    project::{strip_zola_root_prefix, zola_project_root},
    project_model::{
        model::{ProjectModel, ProjectModelFileKind},
        rebuild_project_model_after_workspace_change,
        template_workbench::{
            resolve_template_workbench_plan, TemplateWorkbenchDependencyKind,
            TemplateWorkbenchPlanInput,
        },
        ProjectModelIncrementalIntent,
    },
    state::AppState,
    zola_links::template_contains_asset_path,
    zola_theme::active_theme_from_source,
};

const CSS_MUTATION_AUTHORITY_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CssMutationStatus {
    Noop,
    Staged,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CssMutationAuthorityReceipt {
    pub schema_version: u32,
    pub operation_id: String,
    pub status: CssMutationStatus,
    pub project_root: String,
    pub session_id: String,
    pub revision_before: u64,
    pub revision_after: u64,
    pub dirty: bool,
    pub touched_files: Vec<String>,
    pub written_files: Vec<WrittenProjectFile>,
    pub removed_files: Vec<String>,
    pub documents: Vec<WorkspaceDocumentProjection>,
    pub workspace_mutation: Option<ProjectWorkspaceMutationReceipt>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CssMutationCommandReceipt<T> {
    pub project_root: String,
    pub runtime_session_id: String,
    pub workspace_revision: u64,
    pub payload: T,
    pub authority: CssMutationAuthorityReceipt,
}

impl<T> CssMutationCommandReceipt<T> {
    fn noop(session: &ProjectSessionSnapshot, payload: T, workspace: &ProjectWorkspace) -> Self {
        Self {
            project_root: session.project_root.clone(),
            runtime_session_id: session.runtime_instance_id(),
            workspace_revision: workspace.revision,
            payload,
            authority: CssMutationAuthorityReceipt {
                schema_version: CSS_MUTATION_AUTHORITY_SCHEMA_VERSION,
                operation_id: format!(
                    "css-noop:{}:{}",
                    session.runtime_instance_id(),
                    workspace.revision,
                ),
                status: CssMutationStatus::Noop,
                project_root: session.project_root.clone(),
                session_id: session.runtime_instance_id(),
                revision_before: workspace.revision,
                revision_after: workspace.revision,
                dirty: workspace.is_dirty(),
                touched_files: Vec::new(),
                written_files: Vec::new(),
                removed_files: Vec::new(),
                documents: Vec::new(),
                workspace_mutation: None,
            },
        }
    }

    fn staged(
        session: &ProjectSessionSnapshot,
        payload: T,
        written_files: Vec<WrittenProjectFile>,
        removed_files: Vec<String>,
        documents: Vec<WorkspaceDocumentProjection>,
        workspace_mutation: ProjectWorkspaceMutationReceipt,
    ) -> Self {
        Self {
            project_root: session.project_root.clone(),
            runtime_session_id: session.runtime_instance_id(),
            workspace_revision: workspace_mutation.revision_after,
            payload,
            authority: CssMutationAuthorityReceipt {
                schema_version: CSS_MUTATION_AUTHORITY_SCHEMA_VERSION,
                operation_id: workspace_mutation
                    .transaction_id
                    .clone()
                    .unwrap_or_else(|| {
                        format!("css-session:{}", workspace_mutation.revision_after)
                    }),
                status: CssMutationStatus::Staged,
                project_root: session.project_root.clone(),
                session_id: session.runtime_instance_id(),
                revision_before: workspace_mutation.revision_before,
                revision_after: workspace_mutation.revision_after,
                dirty: workspace_mutation.dirty,
                touched_files: workspace_mutation.touched_files.clone(),
                written_files,
                removed_files,
                documents,
                workspace_mutation: Some(workspace_mutation),
            },
        }
    }
}

fn to_zola_relative_path(path: &str) -> String {
    strip_zola_root_prefix(path).to_string()
}

fn to_project_relative_path(path: &str) -> String {
    path.to_string()
}

fn read_current_project_text(
    project_root: &Path,
    store: &FileBufferStore,
    project_relative_path: &str,
) -> Result<Option<String>, String> {
    if let Some(text) = store.text_for(project_relative_path) {
        return Ok(Some(text));
    }

    match read_project_disk_text_snapshot(project_root, project_relative_path, &store.limits) {
        ProjectDiskTextReadOutcome::Loaded(snapshot) => Ok(Some(snapshot.text)),
        ProjectDiskTextReadOutcome::Missing => Ok(None),
        ProjectDiskTextReadOutcome::NotFile => Err(format!(
            "CSS/SCSS a refuzat {project_relative_path}: target-ul nu este fișier regulat."
        )),
        ProjectDiskTextReadOutcome::Oversized(bytes) => Err(format!(
            "CSS/SCSS a refuzat {project_relative_path}: {bytes} bytes depășesc limita FileBufferStore de {} bytes.",
            store.limits.max_file_bytes,
        )),
        ProjectDiskTextReadOutcome::InvalidPath(error)
        | ProjectDiskTextReadOutcome::UnsafePath(error)
        | ProjectDiskTextReadOutcome::Unstable(error)
        | ProjectDiskTextReadOutcome::Unreadable(error) => Err(format!(
            "CSS/SCSS nu poate citi sigur {project_relative_path}: {error}"
        )),
    }
}

fn read_current_zola_text(
    project_root: &Path,
    store: &FileBufferStore,
    zola_relative_path: &str,
) -> Result<Option<String>, String> {
    read_current_project_text(
        project_root,
        store,
        &to_project_relative_path(zola_relative_path),
    )
}

fn project_relative_exists(
    project_root: &Path,
    store: &FileBufferStore,
    project_relative_path: &str,
) -> Result<bool, String> {
    Ok(read_current_project_text(project_root, store, project_relative_path)?.is_some())
}

fn current_style_paths(store: &FileBufferStore) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for relative_path in store.files.keys() {
        let zola_relative_path = to_zola_relative_path(relative_path);
        if matches!(
            Path::new(&zola_relative_path)
                .extension()
                .and_then(|extension| extension.to_str())
                .map(|extension| extension.to_ascii_lowercase())
                .as_deref(),
            Some("css" | "sass" | "scss")
        ) {
            paths.insert(zola_relative_path);
        }
    }
    paths
}

fn require_complete_style_inventory(store: &FileBufferStore) -> Result<(), String> {
    let blocking = store.diagnostics.iter().find(|diagnostic| {
        matches!(
            diagnostic.code.as_str(),
            "max_files_reached"
                | "max_total_bytes_reached"
                | "file_too_large"
                | "unsafe_project_path"
                | "unstable_during_read"
                | "read_text_failed"
        ) && diagnostic
            .relative_path
            .as_deref()
            .map(|path| {
                matches!(
                    Path::new(path)
                        .extension()
                        .and_then(|extension| extension.to_str())
                        .map(|extension| extension.to_ascii_lowercase())
                        .as_deref(),
                    Some("css" | "sass" | "scss")
                )
            })
            .unwrap_or(true)
    });
    if let Some(diagnostic) = blocking {
        return Err(format!(
            "[css_style_inventory_incomplete] Inventarul CSS/SCSS FileBufferStore este incomplet ({}): {}",
            diagnostic.code, diagnostic.message,
        ));
    }
    Ok(())
}

fn collect_current_project_style_sources(
    project_root: &Path,
    store: &FileBufferStore,
) -> Result<Vec<(String, String)>, String> {
    require_complete_style_inventory(store)?;
    let paths = current_style_paths(store);
    if paths.len() > store.limits.max_files {
        return Err(format!(
            "[css_style_inventory_limit] Inventarul CSS/SCSS cere {} fișiere, peste limita FileBufferStore de {}.",
            paths.len(), store.limits.max_files,
        ));
    }
    let mut total_bytes = 0u64;
    let mut sources = Vec::with_capacity(paths.len());
    for relative_path in paths {
        let Some(source) = read_current_zola_text(project_root, store, &relative_path)? else {
            continue;
        };
        total_bytes = total_bytes.saturating_add(source.len() as u64);
        if total_bytes > store.limits.max_total_bytes {
            return Err(format!(
                "[css_style_inventory_budget] Citirea CSS/SCSS depășește bugetul agregat FileBufferStore de {} bytes.",
                store.limits.max_total_bytes,
            ));
        }
        sources.push((relative_path, source));
    }
    Ok(sources)
}

fn collect_current_scss_variables(
    project_root: &Path,
    store: &FileBufferStore,
) -> Result<Vec<ScssVariable>, String> {
    let mut variables = Vec::new();
    require_complete_style_inventory(store)?;
    for relative_path in current_style_paths(store) {
        if Path::new(&relative_path)
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.eq_ignore_ascii_case("scss"))
            != Some(true)
        {
            continue;
        }
        let Some(source) = read_current_zola_text(project_root, store, &relative_path)? else {
            continue;
        };
        parse_variables_from_source(&source, &relative_path, &mut variables);
    }
    Ok(variables)
}

fn current_active_theme(
    project_root: &Path,
    store: &FileBufferStore,
) -> Result<Option<String>, String> {
    for relative_path in ["zola.toml", "config.toml"] {
        let Some(source) = read_current_zola_text(project_root, store, relative_path)? else {
            continue;
        };
        return Ok(active_theme_from_source(&source));
    }
    Ok(None)
}

fn current_css_breakpoints(
    project_root: &Path,
    store: &FileBufferStore,
) -> Result<CssBreakpointValues, String> {
    let variables = collect_current_scss_variables(project_root, store)?;
    Ok(CssBreakpointValues {
        tablet: variables
            .iter()
            .find(|variable| variable.name == "bp-tableta")
            .map(|variable| variable.value.clone()),
        mobile: variables
            .iter()
            .find(|variable| variable.name == "bp-mobil")
            .map(|variable| variable.value.clone()),
    })
}

fn push_text_change_if_changed(
    changes: &mut Vec<WorkspaceTextChange>,
    written_files: &mut Vec<WrittenProjectFile>,
    relative_path: String,
    before: &str,
    after: String,
) {
    if after == before {
        return;
    }
    changes.push(WorkspaceTextChange {
        relative_path: relative_path.clone(),
        new_text: after.clone(),
    });
    written_files.push(WrittenProjectFile {
        relative_path,
        contents: after,
    });
}

pub(crate) fn with_bound_css_file_buffer<T>(
    state: &AppState,
    identity: &FileBufferRequestIdentity,
    operation: impl FnOnce(&Path, &Path, &ProjectSessionSnapshot, &FileBufferStore) -> Result<T, String>,
) -> Result<FileBufferCommandReceipt<T>, String> {
    with_bound_css_file_buffer_revision(
        state,
        identity,
        |project_root, zola_root, session, store, _workspace_revision| {
            operation(project_root, zola_root, session, store)
        },
    )
}

pub(crate) fn with_bound_css_file_buffer_revision<T>(
    state: &AppState,
    identity: &FileBufferRequestIdentity,
    operation: impl FnOnce(
        &Path,
        &Path,
        &ProjectSessionSnapshot,
        &FileBufferStore,
        u64,
    ) -> Result<T, String>,
) -> Result<FileBufferCommandReceipt<T>, String> {
    with_bound_css_file_buffer_revision_internal(
        state,
        identity,
        false,
        |project_root, zola_root, session, store, workspace_revision, _project_model| {
            operation(project_root, zola_root, session, store, workspace_revision)
        },
    )
}

fn with_bound_css_file_buffer_revision_and_model<T>(
    state: &AppState,
    identity: &FileBufferRequestIdentity,
    operation: impl FnOnce(
        &Path,
        &Path,
        &ProjectSessionSnapshot,
        &FileBufferStore,
        u64,
        Option<&ProjectModel>,
    ) -> Result<T, String>,
) -> Result<FileBufferCommandReceipt<T>, String> {
    with_bound_css_file_buffer_revision_internal(state, identity, true, operation)
}

fn with_bound_css_file_buffer_revision_internal<T>(
    state: &AppState,
    identity: &FileBufferRequestIdentity,
    capture_project_model: bool,
    operation: impl FnOnce(
        &Path,
        &Path,
        &ProjectSessionSnapshot,
        &FileBufferStore,
        u64,
        Option<&ProjectModel>,
    ) -> Result<T, String>,
) -> Result<FileBufferCommandReceipt<T>, String> {
    let (project_root, session, accepted_disk, store, workspace_revision, project_model) = {
        // Project Transition publică root-ul și ProjectWorkspace în aceeași
        // ordine. Capturăm o proiecție exactă sub ambele lock-uri, apoi le
        // eliberăm înaintea scanării de disk și a analizei CSS.
        let current_root = state
            .current_root
            .lock()
            .map_err(|_| "Nu am putut bloca root-ul curent pentru CSS/SCSS.".to_string())?;
        let project_root = current_root
            .as_ref()
            .ok_or_else(|| "Nu există proiect curent pentru CSS/SCSS.".to_string())?;
        let current_root_string = project_root.to_string_lossy().into_owned();
        let project_workspace = state
            .project_workspace
            .lock()
            .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru CSS/SCSS.".to_string())?;
        let workspace = project_workspace
            .as_ref()
            .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru CSS/SCSS.".to_string())?;
        let session = &workspace.session;
        workspace
            .accepted_disk
            .require_identity(&session.runtime_instance_id(), &session.project_root)?;
        workspace.accepted_disk.require_complete()?;
        require_file_buffer_session_binding(
            &current_root_string,
            session,
            &workspace.documents,
            identity,
        )?;

        (
            project_root.clone(),
            session.clone(),
            workspace.accepted_disk.clone(),
            workspace.documents.clone(),
            workspace.revision,
            (capture_project_model
                && workspace.project_model_source_revision == Some(workspace.revision))
            .then(|| workspace.project_model.clone())
            .flatten(),
        )
    };

    accepted_disk.require_live_complete(
        &session.runtime_instance_id(),
        &session.project_root,
        &project_root,
    )?;
    let zola_root = zola_project_root(&project_root);
    let payload = operation(
        &project_root,
        &zola_root,
        &session,
        &store,
        workspace_revision,
        project_model.as_ref(),
    )?;
    accepted_disk.require_live_complete(
        &session.runtime_instance_id(),
        &session.project_root,
        &project_root,
    )?;

    // A read is publishable only while its complete session/revision/disk
    // authority is still current. Reopening the same path therefore cannot
    // turn an old calculation into a valid receipt.
    {
        let current_root = state
            .current_root
            .lock()
            .map_err(|_| "Nu am putut revalida root-ul curent pentru CSS/SCSS.".to_string())?;
        if current_root.as_ref() != Some(&project_root) {
            return Err(
                "Citirea CSS/SCSS a devenit stale: proiectul activ s-a schimbat.".to_string(),
            );
        }
        let project_workspace = state
            .project_workspace
            .lock()
            .map_err(|_| "Nu am putut revalida ProjectWorkspace pentru CSS/SCSS.".to_string())?;
        let workspace = project_workspace.as_ref().ok_or_else(|| {
            "Citirea CSS/SCSS a devenit stale: ProjectWorkspace a fost închis.".to_string()
        })?;
        require_file_buffer_session_binding(
            &project_root.to_string_lossy(),
            &workspace.session,
            &workspace.documents,
            identity,
        )?;
        if workspace.revision != workspace_revision || workspace.accepted_disk != accepted_disk {
            return Err(
                "Citirea CSS/SCSS a devenit stale: revizia workspace sau autoritatea disk s-a schimbat."
                    .to_string(),
            );
        }
    }

    Ok(FileBufferCommandReceipt::new(
        &session,
        workspace_revision,
        payload,
    ))
}

pub(crate) fn execute_css_workspace_mutation_with_metadata<R>(
    app: &AppHandle,
    state: &State<AppState>,
    identity: &FileBufferRequestIdentity,
    expected_workspace_revision: Option<u64>,
    source: &str,
    coalesce_prefix: Option<&str>,
    build: impl FnOnce(
        &Path,
        &Path,
        &FileBufferStore,
        Option<&ProjectModel>,
    ) -> Result<(Option<WorkspaceTextResourceMutationInput>, R), String>,
) -> Result<CssMutationCommandReceipt<R>, String> {
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul curent pentru CSS/SCSS.".to_string())?;
    let project_root = current_root
        .as_ref()
        .ok_or_else(|| "Nu există proiect curent pentru CSS/SCSS.".to_string())?;
    let current_root_string = project_root.to_string_lossy().into_owned();
    let zola_root = zola_project_root(project_root);
    let mut slot = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru CSS/SCSS.".to_string())?;
    let workspace = slot
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru CSS/SCSS.".to_string())?;
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        project_root,
    )?;
    require_file_buffer_session_binding(
        &current_root_string,
        &workspace.session,
        &workspace.documents,
        identity,
    )?;
    if expected_workspace_revision.is_some_and(|expected| expected != workspace.revision) {
        return Err(format!(
            "[theme_style_stale_workspace] Editorul de stil a pornit de la revizia {}, dar ProjectWorkspace este la revizia {}.",
            expected_workspace_revision.unwrap_or_default(),
            workspace.revision,
        ));
    }

    let current_model = (workspace.project_model_source_revision == Some(workspace.revision))
        .then_some(workspace.project_model.as_ref())
        .flatten();
    let (input, result_value) = build(
        project_root,
        &zola_root,
        &workspace.documents,
        current_model,
    )?;
    let Some(input) = input else {
        let session = workspace.session.clone();
        return Ok(CssMutationCommandReceipt::noop(
            &session,
            result_value,
            workspace,
        ));
    };
    let written_files = input
        .changes
        .iter()
        .map(|change| WrittenProjectFile {
            relative_path: change.relative_path.clone(),
            contents: change.new_text.clone(),
        })
        .collect::<Vec<_>>();
    let coalesce_key = coalesce_prefix.map(|prefix| format!("{prefix}:{}", input.target));
    let (mutation, removed_files) =
        commit_project_workspace_session_mutation(app, workspace, |candidate| {
            let previous_model = candidate.project_model.clone();
            let previous_model_source_revision = candidate.project_model_source_revision;
            let workspace_identity = ProjectWorkspaceIdentity {
                expected_project_root: candidate.session.project_root.clone(),
                expected_session_id: candidate.runtime_session_id(),
                expected_revision: candidate.revision,
            };
            let mutation = candidate.stage_resource_changes(
                &workspace_identity,
                WorkspaceMutationMetadata {
                    label: input.label,
                    source: source.to_string(),
                    coalesce_key,
                    transaction_id: None,
                },
                input
                    .changes
                    .into_iter()
                    .map(|change| WorkspaceResourceMutation {
                        relative_path: change.relative_path,
                        contents: change.new_text,
                        create_only: false,
                    })
                    .collect(),
                input
                    .deletes
                    .into_iter()
                    .map(|delete| WorkspaceResourceDelete {
                        relative_path: delete.relative_path,
                    })
                    .collect(),
                crate::kernel::file_buffer_store::now_ms(),
            )?;
            if mutation.changed
                && previous_model_source_revision == Some(mutation.revision_before)
                && previous_model.is_some()
                && !mutation.touched_files.is_empty()
            {
                let style_only = mutation.touched_files.iter().all(|relative_path| {
                    let extension = Path::new(relative_path)
                        .extension()
                        .and_then(|extension| extension.to_str());
                    matches!(extension, Some("css" | "scss" | "sass"))
                        || previous_model.as_ref().is_some_and(|model| {
                            matches!(model
                                .files
                                .iter()
                                .filter(|file| file.relative_path == *relative_path)
                                .collect::<Vec<_>>()
                                .as_slice(), [file] if file.kind == ProjectModelFileKind::Style)
                        })
                });
                let projection = candidate.capture_projection_snapshot()?;
                let outcome = rebuild_project_model_after_workspace_change(
                    project_root,
                    previous_model.as_ref(),
                    previous_model_source_revision,
                    &projection,
                    &mutation.touched_files,
                    if style_only {
                        ProjectModelIncrementalIntent::StyleDeclaration
                    } else {
                        ProjectModelIncrementalIntent::Unsupported
                    },
                )?;
                candidate.publish_project_model(&projection, outcome.model)?;
            }
            let removed_files = mutation
                .entry
                .as_ref()
                .map(|entry| {
                    entry
                        .document_paths
                        .iter()
                        .filter(|path| !candidate.documents.files.contains_key(*path))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            Ok((mutation, removed_files))
        })?;
    let session = workspace.session.clone();
    let documents = mutation
        .touched_files
        .iter()
        .map(|relative_path| WorkspaceDocumentProjection {
            relative_path: relative_path.clone(),
            snapshot: workspace.documents.text_snapshot(relative_path),
        })
        .collect();
    let _ = app;
    Ok(CssMutationCommandReceipt::staged(
        &session,
        result_value,
        written_files,
        removed_files,
        documents,
        mutation,
    ))
}

fn execute_css_workspace_mutation<R>(
    app: &AppHandle,
    state: &State<AppState>,
    identity: &FileBufferRequestIdentity,
    build: impl FnOnce(
        &Path,
        &Path,
        &FileBufferStore,
    ) -> Result<(Option<WorkspaceTextResourceMutationInput>, R), String>,
) -> Result<CssMutationCommandReceipt<R>, String> {
    execute_css_workspace_mutation_with_metadata(
        app,
        state,
        identity,
        None,
        "css.panel",
        Some("css.panel"),
        |project_root, zola_root, store, _project_model| build(project_root, zola_root, store),
    )
}

fn execute_selection_bound_css_workspace_mutation<R>(
    app: &AppHandle,
    state: &State<AppState>,
    identity: &FileBufferRequestIdentity,
    expected_selection: Option<&SelectionMutationIdentity>,
    build: impl FnOnce(
        &Path,
        &Path,
        &FileBufferStore,
    ) -> Result<(Option<WorkspaceTextResourceMutationInput>, R), String>,
) -> Result<CssMutationCommandReceipt<R>, String> {
    let execute = || execute_css_workspace_mutation(app, state, identity, build);
    let Some(expected) = expected_selection else {
        return execute();
    };
    state
        .selection_coordinator
        .with_stable_semantic_mutation_target(&identity.expected_session_id, expected, execute)
}

fn execute_selection_bound_css_workspace_mutation_with_model<R>(
    app: &AppHandle,
    state: &State<AppState>,
    identity: &FileBufferRequestIdentity,
    expected_selection: Option<&SelectionMutationIdentity>,
    build: impl FnOnce(
        &Path,
        &Path,
        &FileBufferStore,
        Option<&ProjectModel>,
    ) -> Result<(Option<WorkspaceTextResourceMutationInput>, R), String>,
) -> Result<CssMutationCommandReceipt<R>, String> {
    let execute = || {
        execute_css_workspace_mutation_with_metadata(
            app,
            state,
            identity,
            None,
            "css.panel.reusable",
            Some("css.panel.reusable"),
            build,
        )
    };
    let Some(expected) = expected_selection else {
        return execute();
    };
    state
        .selection_coordinator
        .with_stable_semantic_mutation_target(&identity.expected_session_id, expected, execute)
}

fn collect_media_query_migration_changes(
    project_root: &Path,
    store: &FileBufferStore,
    changes_by_path: &mut BTreeMap<String, String>,
    old_bp: &str,
    new_bp: &str,
) -> Result<(), String> {
    let old_needle = format!("@media (max-width: {})", old_bp);
    let new_value = format!("@media (max-width: {})", new_bp);
    collect_scss_replacements(
        project_root,
        store,
        changes_by_path,
        &old_needle,
        &new_value,
    )
}

fn collect_scss_replacements(
    project_root: &Path,
    store: &FileBufferStore,
    changes_by_path: &mut BTreeMap<String, String>,
    old: &str,
    new: &str,
) -> Result<(), String> {
    require_complete_style_inventory(store)?;
    for zola_relative in current_style_paths(store) {
        if Path::new(&zola_relative)
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.eq_ignore_ascii_case("scss"))
            != Some(true)
        {
            continue;
        }
        let project_relative = to_project_relative_path(&zola_relative);
        let source = if let Some(source) = changes_by_path.get(&project_relative) {
            source.clone()
        } else {
            read_current_project_text(project_root, store, &project_relative)?.unwrap_or_default()
        };
        if source.contains(old) {
            changes_by_path.insert(project_relative, source.replace(old, new));
        }
    }

    Ok(())
}

const CSS_INSPECTOR_CONTEXT_SCHEMA_VERSION: u32 = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReusableCssConsumer {
    template_path: String,
    stylesheet_path: String,
    href: String,
}

fn reusable_css_consumers(
    model: &ProjectModel,
    reusable_template_path: &str,
) -> Result<Vec<ReusableCssConsumer>, String> {
    let plan = resolve_template_workbench_plan(
        model,
        &TemplateWorkbenchPlanInput {
            template_path: reusable_template_path.to_string(),
            preferred_page_path: None,
            preferred_route: None,
        },
    )?;
    let mut consumers = plan
        .consumers
        .into_iter()
        .filter(|consumer| {
            consumer.dependency_path.iter().any(|step| {
                matches!(
                    step.kind,
                    TemplateWorkbenchDependencyKind::Includes
                        | TemplateWorkbenchDependencyKind::Imports
                )
            })
        })
        .map(|consumer| {
            let template_path = consumer.root_template_file;
            ReusableCssConsumer {
                stylesheet_path: page_scss_relative_path(&template_path),
                href: page_css_href(&template_path),
                template_path,
            }
        })
        .collect::<Vec<_>>();
    consumers.sort_by(|left, right| left.template_path.cmp(&right.template_path));
    consumers.dedup_by(|left, right| left.template_path == right.template_path);
    Ok(consumers)
}

fn is_page_stylesheet_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    normalized.starts_with("sass/pagini/") || normalized.contains("/sass/pagini/")
}

fn reusable_scoped_candidates(
    candidates: Vec<CssInspectorSourceCandidate>,
    owner_file: Option<&str>,
) -> Vec<CssInspectorSourceCandidate> {
    let Some(owner_file) = owner_file else {
        return candidates;
    };
    let owner_file = to_project_relative_path(owner_file);
    let owner_candidates = candidates
        .iter()
        .filter(|candidate| candidate.file == owner_file)
        .cloned()
        .collect::<Vec<_>>();
    if !owner_candidates.is_empty() {
        return owner_candidates;
    }
    candidates
        .into_iter()
        .filter(|candidate| !is_page_stylesheet_path(&candidate.file))
        .collect()
}

fn populate_reusable_target_delivery(
    project_root: &Path,
    store: &FileBufferStore,
    model: Option<&ProjectModel>,
    target: &mut PageCssTarget,
) -> Result<Vec<ReusableCssConsumer>, String> {
    let Some(template_path) = target.template_path.as_deref() else {
        return Ok(Vec::new());
    };
    let Some(model) = model else {
        target.reason = "Proprietarul SCSS reutilizabil este determinist, dar ProjectModel-ul curent nu este încă publicat; consumatorii vor fi validați înainte de scriere.".to_string();
        return Ok(Vec::new());
    };
    let consumers = reusable_css_consumers(model, template_path)?;
    target.consumer_files = consumers
        .iter()
        .map(|consumer| consumer.stylesheet_path.clone())
        .collect();
    target.consumer_templates = consumers
        .iter()
        .map(|consumer| consumer.template_path.clone())
        .collect();
    let mut all_linked = !consumers.is_empty();
    for consumer in &consumers {
        let stylesheet_linked = read_current_zola_text(
            project_root,
            store,
            &consumer.stylesheet_path,
        )?
        .is_some_and(|source| {
            consumer_stylesheet_imports_reusable(&source, &consumer.stylesheet_path, &target.file)
        });
        let template_linked = read_current_zola_text(project_root, store, &consumer.template_path)?
            .is_some_and(|source| template_contains_asset_path(&source, &consumer.href));
        all_linked &= stylesheet_linked && template_linked;
    }
    target.linked = all_linked;
    target.reason = if consumers.is_empty() {
        "Regula aparține partialului SCSS reutilizabil. Nu există încă un consumator public real; Workbench-ul o proiectează numai în preview.".to_string()
    } else {
        format!(
            "Regula aparține partialului SCSS reutilizabil și este livrată prin {} foaie/foi de pagină consumatoare.",
            consumers.len(),
        )
    };
    Ok(consumers)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CssInspectorContextState {
    Existing,
    Creation,
    Ambiguous,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CssInspectorSourceCandidate {
    pub file: String,
    pub rule_context: CssRuleContext,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CssInspectorContextResolution {
    pub schema_version: u32,
    pub selection_revision: u64,
    pub selector: String,
    pub viewport: String,
    pub state: CssInspectorContextState,
    pub target: Option<PageCssTarget>,
    pub rule_context: Option<CssRuleContext>,
    pub candidates: Vec<CssInspectorSourceCandidate>,
}

fn css_inspector_source_candidates(
    sources: &[(String, String)],
    breakpoints: &CssBreakpointValues,
    source_selector: &str,
    requested_selector: &str,
    viewport: &str,
) -> Vec<CssInspectorSourceCandidate> {
    sources
        .iter()
        .filter_map(|(file, source)| {
            selector_source_target(source, source_selector)?;
            Some(CssInspectorSourceCandidate {
                rule_context: get_rule_context(
                    breakpoints,
                    to_project_relative_path(file),
                    source,
                    requested_selector.to_string(),
                    viewport.to_string(),
                ),
                file: to_project_relative_path(file),
            })
        })
        .collect()
}

fn base_class_selector(selector: &str) -> Option<String> {
    let selector = selector.trim();
    let bytes = selector.as_bytes();
    if bytes.first() != Some(&b'.') {
        return None;
    }
    let mut end = 1usize;
    while bytes
        .get(end)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        end += 1;
    }
    (end > 1 && end < selector.len()).then(|| selector[..end].to_string())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageCssCleanupResult {
    pub stylesheet_deleted: bool,
    pub template_updated: bool,
    pub written_files: Vec<WrittenProjectFile>,
}

fn strip_block_comments(source: &str) -> String {
    let mut result = String::new();
    let mut cursor = 0;
    while let Some(relative_start) = source[cursor..].find("/*") {
        let start = cursor + relative_start;
        result.push_str(&source[cursor..start]);
        let Some(relative_end) = source[start + 2..].find("*/") else {
            return result;
        };
        cursor = start + 2 + relative_end + 2;
    }
    result.push_str(&source[cursor..]);
    result
}

fn css_has_effective_rules(source: &str) -> bool {
    let without_comments = strip_block_comments(source);
    for line in without_comments.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("//")
            || trimmed.starts_with("@")
            || trimmed.starts_with('}')
        {
            continue;
        }
        if trimmed.contains('{') {
            return true;
        }
    }
    false
}

// Tauri derives the IPC field names from this flat signature; grouping them would break the
// established frontend command contract.
#[allow(clippy::too_many_arguments)]
#[tauri::command(async)]
pub fn resolve_css_inspector_context(
    template_path: Option<String>,
    selector: String,
    viewport: String,
    fallback_file: Option<String>,
    expected_workspace_revision: u64,
    expected_selection: SelectionMutationIdentity,
    identity: FileBufferRequestIdentity,
    state: State<AppState>,
) -> Result<FileBufferCommandReceipt<CssInspectorContextResolution>, String> {
    validate_panel_rule_input(&selector, &HashMap::new(), &viewport)?;
    let runtime_session_id = identity.expected_session_id.clone();
    state.selection_coordinator.with_selection_target(
        &runtime_session_id,
        &expected_selection,
        || {
            with_bound_css_file_buffer_revision_and_model(
                state.inner(),
                &identity,
                move |project_root, _root, _session, store, workspace_revision, project_model| {
                    if workspace_revision != expected_workspace_revision {
                        return Err(format!(
                            "[css_inspector_stale_workspace] Rezoluția CSS a cerut revizia ProjectWorkspace {expected_workspace_revision}, dar revizia activă este {workspace_revision}."
                        ));
                    }
                    let selector = selector.trim().to_string();
                    let template_path =
                        template_path.map(|path| to_zola_relative_path(&path));
                    let fallback_file =
                        fallback_file.map(|path| to_zola_relative_path(&path));
                    let reusable_owner = template_path
                        .as_deref()
                        .and_then(reusable_scss_relative_path);
                    let breakpoints = current_css_breakpoints(project_root, store)?;
                    let sources = collect_current_project_style_sources(project_root, store)?;
                    let candidates = reusable_scoped_candidates(
                        css_inspector_source_candidates(
                            &sources,
                            &breakpoints,
                            &selector,
                            &selector,
                            &viewport,
                        ),
                        reusable_owner.as_deref(),
                    );

                    if candidates.len() > 1 {
                        return Ok(CssInspectorContextResolution {
                            schema_version: CSS_INSPECTOR_CONTEXT_SCHEMA_VERSION,
                            selection_revision: expected_selection.selection_revision,
                            selector,
                            viewport,
                            state: CssInspectorContextState::Ambiguous,
                            target: None,
                            rule_context: None,
                            candidates,
                        });
                    }

                    if let Some(candidate) = candidates.first().cloned() {
                        let file = candidate.file.clone();
                        let page_file = template_path
                            .as_deref()
                            .map(page_scss_relative_path)
                            .unwrap_or_default();
                        let href = template_path.as_deref().map(page_css_href);
                        let linked = template_path
                            .as_deref()
                            .zip(href.as_deref())
                            .map(|(template, href)| {
                                read_current_zola_text(project_root, store, template).map(
                                    |source| {
                                        source.as_deref().is_some_and(|source| {
                                            template_contains_asset_path(source, href)
                                        })
                                    },
                                )
                            })
                            .transpose()?
                            .unwrap_or(false);
                        let mut target = PageCssTarget {
                            exists: project_relative_exists(project_root, store, &file)?,
                            page_owned: !page_file.is_empty()
                                && file == to_project_relative_path(&page_file),
                            file,
                            selector: selector.clone(),
                            target_kind: "existing".to_string(),
                            linked,
                            href,
                            template_path: template_path
                                .clone()
                                .map(|path| to_project_relative_path(&path)),
                            consumer_files: Vec::new(),
                            consumer_templates: Vec::new(),
                            reason: "Regula există deja în acest fișier.".to_string(),
                        };
                        if reusable_owner.as_deref() == Some(target.file.as_str()) {
                            target.target_kind = "reusable".to_string();
                            target.href = None;
                            populate_reusable_target_delivery(
                                project_root,
                                store,
                                project_model,
                                &mut target,
                            )?;
                        }
                        return Ok(CssInspectorContextResolution {
                            schema_version: CSS_INSPECTOR_CONTEXT_SCHEMA_VERSION,
                            selection_revision: expected_selection.selection_revision,
                            selector,
                            viewport,
                            state: CssInspectorContextState::Existing,
                            target: Some(target),
                            rule_context: Some(candidate.rule_context),
                            candidates,
                        });
                    }

                    if let Some(base_selector) = base_class_selector(&selector) {
                        let base_candidates = reusable_scoped_candidates(
                            css_inspector_source_candidates(
                                &sources,
                                &breakpoints,
                                &base_selector,
                                &selector,
                                &viewport,
                            ),
                            reusable_owner.as_deref(),
                        );
                        if base_candidates.len() > 1 {
                            return Ok(CssInspectorContextResolution {
                                schema_version: CSS_INSPECTOR_CONTEXT_SCHEMA_VERSION,
                                selection_revision: expected_selection.selection_revision,
                                selector,
                                viewport,
                                state: CssInspectorContextState::Ambiguous,
                                target: None,
                                rule_context: None,
                                candidates: base_candidates,
                            });
                        }
                        if let Some(candidate) = base_candidates.first().cloned() {
                            let file = candidate.file.clone();
                            let page_file = template_path
                                .as_deref()
                                .map(page_scss_relative_path)
                                .unwrap_or_default();
                            let href = template_path.as_deref().map(page_css_href);
                            let linked = template_path
                                .as_deref()
                                .zip(href.as_deref())
                                .map(|(template, href)| {
                                    read_current_zola_text(project_root, store, template).map(
                                        |source| {
                                            source.as_deref().is_some_and(|source| {
                                                template_contains_asset_path(source, href)
                                            })
                                        },
                                    )
                                })
                                .transpose()?
                                .unwrap_or(false);
                            let mut target = PageCssTarget {
                                exists: project_relative_exists(project_root, store, &file)?,
                                page_owned: !page_file.is_empty()
                                    && file == to_project_relative_path(&page_file),
                                file,
                                selector: selector.clone(),
                                target_kind: "variant".to_string(),
                                linked,
                                href,
                                template_path: template_path
                                    .clone()
                                    .map(|path| to_project_relative_path(&path)),
                                consumer_files: Vec::new(),
                                consumer_templates: Vec::new(),
                                reason: format!(
                                    "Varianta va fi creată lângă regula de bază {base_selector}."
                                ),
                            };
                            if reusable_owner.as_deref() == Some(target.file.as_str()) {
                                target.target_kind = "reusable".to_string();
                                target.href = None;
                                populate_reusable_target_delivery(
                                    project_root,
                                    store,
                                    project_model,
                                    &mut target,
                                )?;
                            }
                            return Ok(CssInspectorContextResolution {
                                schema_version: CSS_INSPECTOR_CONTEXT_SCHEMA_VERSION,
                                selection_revision: expected_selection.selection_revision,
                                selector,
                                viewport,
                                state: CssInspectorContextState::Creation,
                                target: Some(target),
                                rule_context: Some(candidate.rule_context),
                                candidates: base_candidates,
                            });
                        }
                    }

                    let mut target = page_target_for_template(
                        template_path.as_deref(),
                        &selector,
                        fallback_file.as_deref(),
                    );
                    let target_source =
                        read_current_zola_text(project_root, store, &target.file)?
                            .unwrap_or_default();
                    let rule_context = get_rule_context(
                        &breakpoints,
                        to_project_relative_path(&target.file),
                        &target_source,
                        selector.clone(),
                        viewport.clone(),
                    );
                    target.exists = project_relative_exists(
                        project_root,
                        store,
                        &to_project_relative_path(&target.file),
                    )?;
                    target.linked = template_path
                        .as_deref()
                        .zip(target.href.as_deref())
                        .map(|(template, href)| {
                            read_current_zola_text(project_root, store, template).map(|source| {
                                source.as_deref().is_some_and(|source| {
                                    template_contains_asset_path(source, href)
                                })
                            })
                        })
                        .transpose()?
                        .unwrap_or(false);
                    target.file = to_project_relative_path(&target.file);
                    target.template_path = target
                        .template_path
                        .map(|path| to_project_relative_path(&path));
                    if target.target_kind == "reusable" {
                        populate_reusable_target_delivery(
                            project_root,
                            store,
                            project_model,
                            &mut target,
                        )?;
                    }
                    Ok(CssInspectorContextResolution {
                        schema_version: CSS_INSPECTOR_CONTEXT_SCHEMA_VERSION,
                        selection_revision: expected_selection.selection_revision,
                        selector,
                        viewport,
                        state: CssInspectorContextState::Creation,
                        target: Some(target),
                        rule_context: Some(rule_context),
                        candidates,
                    })
                },
            )
        },
    )
}

#[tauri::command(async)]
pub fn cleanup_page_css_contract(
    template_path: String,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<CssMutationCommandReceipt<PageCssCleanupResult>, String> {
    let template_path = to_zola_relative_path(&template_path);
    execute_css_workspace_mutation(
        &app,
        &state,
        &identity,
        |project_root, _zola_root, store| {
            let scss_rel = page_scss_relative_path(&template_path);
            let href = page_css_href(&template_path);
            let scss_project_rel = to_project_relative_path(&scss_rel);
            let template_project_rel = to_project_relative_path(&template_path);
            let mut changes = Vec::new();
            let mut deletes = Vec::new();
            let mut stylesheet_deleted = false;
            let mut template_updated = false;
            let mut written_files = Vec::new();

            let has_effective_rules = read_current_zola_text(project_root, store, &scss_rel)?
                .as_deref()
                .map(css_has_effective_rules)
                .unwrap_or(false);

            if has_effective_rules {
                return Ok((
                    None,
                    PageCssCleanupResult {
                        stylesheet_deleted,
                        template_updated,
                        written_files,
                    },
                ));
            }

            if project_relative_exists(project_root, store, &scss_project_rel)? {
                deletes.push(WorkspaceTextDelete {
                    relative_path: scss_project_rel.clone(),
                });
                stylesheet_deleted = true;
            }

            if let Some(template_source) =
                read_current_zola_text(project_root, store, &template_path)?
            {
                let updated = remove_page_stylesheet_link(&template_source, &href);
                push_text_change_if_changed(
                    &mut changes,
                    &mut written_files,
                    template_project_rel.clone(),
                    &template_source,
                    updated,
                );
                template_updated = written_files
                    .iter()
                    .any(|file| file.relative_path == template_project_rel);
            }

            let input = if changes.is_empty() && deletes.is_empty() {
                None
            } else {
                Some(WorkspaceTextResourceMutationInput {
                    label: "Cleanup Page CSS contract".to_string(),
                    target: template_project_rel,
                    changes,
                    deletes,
                })
            };

            Ok((
                input,
                PageCssCleanupResult {
                    stylesheet_deleted,
                    template_updated,
                    written_files,
                },
            ))
        },
    )
}

#[tauri::command(async)]
pub fn get_scss_variables(
    identity: FileBufferRequestIdentity,
    state: State<AppState>,
) -> Result<FileBufferCommandReceipt<Vec<ScssVariable>>, String> {
    with_bound_css_file_buffer(
        state.inner(),
        &identity,
        |project_root, _root, _session, store| collect_current_scss_variables(project_root, store),
    )
}

#[tauri::command(async)]
pub fn set_scss_variable(
    relative_path: String,
    name: String,
    value: String,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<CssMutationCommandReceipt<()>, String> {
    validate_panel_variable_value(&value)?;
    let zola_relative_path = strip_zola_root_prefix(&relative_path).to_string();
    execute_css_workspace_mutation(
        &app,
        &state,
        &identity,
        |project_root, _zola_root, store| {
            let project_relative_path = to_project_relative_path(&zola_relative_path);
            let source = read_current_zola_text(project_root, store, &zola_relative_path)?
                .ok_or_else(|| format!("Nu am putut citi {}", relative_path))?;

            let old_value = variable_value_in_source(&source, &name);

            let updated = update_variable_in_source(&source, &name, &value).ok_or_else(|| {
                format!("Variabila ${} nu a fost gasita in {}", name, relative_path)
            })?;

            let mut changes_by_path = BTreeMap::new();
            if updated != source {
                changes_by_path.insert(project_relative_path.clone(), updated);
            }

            if matches!(name.as_str(), "bp-mobil" | "bp-tableta") {
                if let Some(old_value) = old_value {
                    if old_value != value {
                        collect_media_query_migration_changes(
                            project_root,
                            store,
                            &mut changes_by_path,
                            &old_value,
                            &value,
                        )?;
                    }
                }
            }

            let changes = changes_by_path
                .into_iter()
                .map(|(relative_path, new_text)| WorkspaceTextChange {
                    relative_path,
                    new_text,
                })
                .collect::<Vec<_>>();

            let input = if changes.is_empty() {
                None
            } else {
                Some(WorkspaceTextResourceMutationInput {
                    label: format!("SCSS variable ${name}"),
                    target: project_relative_path,
                    changes,
                    deletes: Vec::new(),
                })
            };

            Ok((input, ()))
        },
    )
}

#[tauri::command(async)]
pub fn create_scss_variable(
    relative_path: String,
    name: String,
    value: String,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<CssMutationCommandReceipt<()>, String> {
    let name = validate_scss_variable_name(&name)?;
    validate_panel_variable_value(&value)?;
    let zola_relative_path = strip_zola_root_prefix(&relative_path).to_string();
    if Path::new(&zola_relative_path)
        .extension()
        .and_then(|extension| extension.to_str())
        != Some("scss")
    {
        return Err("Tokenii noi pot fi adăugați numai într-un fișier SCSS.".to_string());
    }
    execute_css_workspace_mutation(
        &app,
        &state,
        &identity,
        |project_root, _zola_root, store| {
            let project_relative_path = to_project_relative_path(&zola_relative_path);
            let source = read_current_zola_text(project_root, store, &zola_relative_path)?
                .ok_or_else(|| format!("Nu am putut citi {relative_path}."))?;
            if variable_value_in_source(&source, &name).is_some() {
                return Err(format!("Variabila ${name} există deja în {relative_path}."));
            }
            let separator = if source.is_empty() || source.ends_with('\n') {
                ""
            } else {
                "\n"
            };
            let updated = format!("{source}{separator}\n${name}: {};\n", value.trim());
            Ok((
                Some(WorkspaceTextResourceMutationInput {
                    label: format!("Creare variabilă SCSS ${name}"),
                    target: project_relative_path.clone(),
                    changes: vec![WorkspaceTextChange {
                        relative_path: project_relative_path,
                        new_text: updated,
                    }],
                    deletes: Vec::new(),
                }),
                (),
            ))
        },
    )
}

fn validate_scss_variable_name(value: &str) -> Result<String, String> {
    let value = value.trim().trim_start_matches('$');
    if value.is_empty() {
        return Err("Numele tokenului este obligatoriu.".to_string());
    }
    if value.len() > 128 {
        return Err("Numele tokenului depășește limita de 128 de caractere.".to_string());
    }
    let mut characters = value.chars();
    let first = characters.next().unwrap_or_default();
    if !(first.is_ascii_alphabetic() || first == '_' || first == '-')
        || !characters.all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        })
    {
        return Err("Numele tokenului trebuie să conțină numai litere, cifre, _ și -.".to_string());
    }
    Ok(value.to_string())
}

#[tauri::command(async)]
pub fn set_css_rule(
    relative_path: String,
    selector: String,
    properties: HashMap<String, String>,
    identity: FileBufferRequestIdentity,
    expected_selection: Option<SelectionMutationIdentity>,
    app: AppHandle,
    state: State<AppState>,
) -> Result<CssMutationCommandReceipt<()>, String> {
    set_css_rule_impl(
        relative_path,
        selector,
        properties,
        &identity,
        expected_selection.as_ref(),
        &app,
        &state,
    )
}

fn set_css_rule_impl(
    relative_path: String,
    selector: String,
    properties: HashMap<String, String>,
    identity: &FileBufferRequestIdentity,
    expected_selection: Option<&SelectionMutationIdentity>,
    app: &AppHandle,
    state: &State<AppState>,
) -> Result<CssMutationCommandReceipt<()>, String> {
    let properties = normalize_panel_rule_input(&selector, &properties, "desktop")?;
    let zola_relative_path = strip_zola_root_prefix(&relative_path).to_string();
    execute_selection_bound_css_workspace_mutation(
        app,
        state,
        identity,
        expected_selection,
        move |project_root, _zola_root, store| {
            if properties.is_empty() {
                return Ok((None, ()));
            }
            let project_relative_path = to_project_relative_path(&zola_relative_path);
            let existing = read_current_zola_text(project_root, store, &zola_relative_path)?
                .unwrap_or_default();
            let updated = upsert_css_rule_desktop(&existing, selector.trim(), &properties);
            let changes = if updated == existing {
                Vec::new()
            } else {
                vec![WorkspaceTextChange {
                    relative_path: project_relative_path.clone(),
                    new_text: updated,
                }]
            };

            let input = if changes.is_empty() {
                None
            } else {
                Some(WorkspaceTextResourceMutationInput {
                    label: "CSS rule".to_string(),
                    target: project_relative_path,
                    changes,
                    deletes: Vec::new(),
                })
            };

            Ok((input, ()))
        },
    )
}

/// Write a CSS rule at the correct breakpoint level.
/// viewport: "desktop" → base rule (no media), "tablet" / "mobile" → inside @media block.
/// Breakpoint values are read from $bp-tableta / $bp-mobil in the project's SCSS files.
// Keep the stable Tauri IPC keys flat at the command boundary.
#[allow(clippy::too_many_arguments)]
#[tauri::command(async)]
pub fn set_css_rule_at_viewport(
    relative_path: String,
    selector: String,
    properties: HashMap<String, String>,
    viewport: String,
    identity: FileBufferRequestIdentity,
    expected_selection: Option<SelectionMutationIdentity>,
    app: AppHandle,
    state: State<AppState>,
) -> Result<CssMutationCommandReceipt<()>, String> {
    set_css_rule_at_viewport_impl(
        relative_path,
        selector,
        properties,
        viewport,
        &identity,
        expected_selection.as_ref(),
        &app,
        &state,
    )
}

// This adapter intentionally mirrors the IPC boundary while borrowing native dependencies.
#[allow(clippy::too_many_arguments)]
fn set_css_rule_at_viewport_impl(
    relative_path: String,
    selector: String,
    properties: HashMap<String, String>,
    viewport: String,
    identity: &FileBufferRequestIdentity,
    expected_selection: Option<&SelectionMutationIdentity>,
    app: &AppHandle,
    state: &State<AppState>,
) -> Result<CssMutationCommandReceipt<()>, String> {
    let properties = normalize_panel_rule_input(&selector, &properties, &viewport)?;
    let zola_relative_path = strip_zola_root_prefix(&relative_path).to_string();
    execute_selection_bound_css_workspace_mutation(
        app,
        state,
        identity,
        expected_selection,
        move |project_root, _zola_root, store| {
            if properties.is_empty() {
                return Ok((None, ()));
            }
            let project_relative_path = to_project_relative_path(&zola_relative_path);
            let existing = read_current_zola_text(project_root, store, &zola_relative_path)?
                .unwrap_or_default();
            let breakpoints = current_css_breakpoints(project_root, store)?;
            let updated = write_rule_at_viewport(
                &breakpoints,
                &existing,
                selector.trim(),
                &properties,
                &viewport,
            );
            let changes = if updated == existing {
                Vec::new()
            } else {
                vec![WorkspaceTextChange {
                    relative_path: project_relative_path.clone(),
                    new_text: updated,
                }]
            };

            let input = if changes.is_empty() {
                None
            } else {
                Some(WorkspaceTextResourceMutationInput {
                    label: "CSS rule viewport".to_string(),
                    target: project_relative_path,
                    changes,
                    deletes: Vec::new(),
                })
            };

            Ok((input, ()))
        },
    )
}

/// Writes a rule owned by an included/reusable template and atomically wires
/// that partial into every real page template which consumes it. The active
/// Code tab is never used as an ownership fallback.
// Keep the stable Tauri IPC keys flat at the command boundary.
#[allow(clippy::too_many_arguments)]
#[tauri::command(async)]
pub fn set_reusable_css_rule_at_viewport(
    template_path: String,
    relative_path: String,
    selector: String,
    properties: HashMap<String, String>,
    viewport: String,
    cachebust_assets: bool,
    identity: FileBufferRequestIdentity,
    expected_selection: Option<SelectionMutationIdentity>,
    app: AppHandle,
    state: State<AppState>,
) -> Result<CssMutationCommandReceipt<ReusableCssWriteResult>, String> {
    let properties = normalize_panel_rule_input(&selector, &properties, &viewport)?;
    let template_path = to_zola_relative_path(&template_path);
    let zola_relative_path = strip_zola_root_prefix(&relative_path).to_string();
    let expected_owner = reusable_scss_relative_path(&template_path).ok_or_else(|| {
        format!("Template-ul {template_path} nu are un proprietar SCSS reutilizabil determinist.")
    })?;
    if zola_relative_path != expected_owner {
        return Err(format!(
            "Ținta SCSS reutilizabilă a fost refuzată: {zola_relative_path}; proprietarul canonic este {expected_owner}."
        ));
    }

    execute_selection_bound_css_workspace_mutation_with_model(
        &app,
        &state,
        &identity,
        expected_selection.as_ref(),
        move |project_root, _zola_root, store, project_model| {
            let model = project_model.ok_or_else(|| {
                "ProjectModel-ul reviziei curente nu este disponibil pentru legarea SCSS reutilizabilă. Reîncearcă după resincronizarea editorului."
                    .to_string()
            })?;
            let consumers = reusable_css_consumers(model, &template_path)?;
            let consumer_files = consumers
                .iter()
                .map(|consumer| consumer.stylesheet_path.clone())
                .collect::<Vec<_>>();
            let consumer_templates = consumers
                .iter()
                .map(|consumer| consumer.template_path.clone())
                .collect::<Vec<_>>();
            let project_relative_path = to_project_relative_path(&zola_relative_path);
            let stylesheet_created =
                !project_relative_exists(project_root, store, &project_relative_path)?;

            if properties.is_empty() {
                return Ok((
                    None,
                    ReusableCssWriteResult {
                        file: project_relative_path,
                        stylesheet_created: false,
                        consumer_files,
                        consumer_templates,
                        written_files: Vec::new(),
                    },
                ));
            }

            let existing = read_current_zola_text(project_root, store, &zola_relative_path)?
                .unwrap_or_default();
            let breakpoints = current_css_breakpoints(project_root, store)?;
            let updated = write_rule_at_viewport(
                &breakpoints,
                &existing,
                selector.trim(),
                &properties,
                &viewport,
            );
            require_complete_style_inventory(store)?;
            let mut style_files = current_style_paths(store);
            style_files.insert(zola_relative_path.clone());
            let active_theme = current_active_theme(project_root, store)?;
            let mut changes_by_path = BTreeMap::new();
            if updated != existing {
                changes_by_path.insert(project_relative_path.clone(), updated);
            }

            for consumer in &consumers {
                let consumer_existing =
                    read_current_zola_text(project_root, store, &consumer.stylesheet_path)?
                        .unwrap_or_default();
                let consumer_updated = prepare_reusable_consumer_stylesheet_source(
                    &consumer.stylesheet_path,
                    &consumer_existing,
                    &zola_relative_path,
                    style_files.iter().cloned(),
                    active_theme.as_deref(),
                )?;
                if consumer_updated != consumer_existing {
                    changes_by_path.insert(consumer.stylesheet_path.clone(), consumer_updated);
                }

                for file in plan_page_stylesheet_link_writes_with_reader(
                    &consumer.template_path,
                    &consumer.href,
                    cachebust_assets,
                    active_theme.as_deref(),
                    |relative_path| read_current_zola_text(project_root, store, relative_path),
                )? {
                    changes_by_path
                        .insert(to_project_relative_path(&file.relative_path), file.contents);
                }
            }

            let written_files = changes_by_path
                .iter()
                .map(|(relative_path, contents)| WrittenProjectFile {
                    relative_path: relative_path.clone(),
                    contents: contents.clone(),
                })
                .collect::<Vec<_>>();
            let changes = changes_by_path
                .into_iter()
                .map(|(relative_path, new_text)| WorkspaceTextChange {
                    relative_path,
                    new_text,
                })
                .collect::<Vec<_>>();
            let input = (!changes.is_empty()).then_some(WorkspaceTextResourceMutationInput {
                label: format!("Reusable CSS rule {selector}"),
                target: project_relative_path.clone(),
                changes,
                deletes: Vec::new(),
            });
            Ok((
                input,
                ReusableCssWriteResult {
                    file: project_relative_path,
                    stylesheet_created,
                    consumer_files,
                    consumer_templates,
                    written_files,
                },
            ))
        },
    )
}

/// Write a CSS rule in a page-owned stylesheet and make sure the page template
/// links the compiled stylesheet. Used when a selector does not already belong
/// to an existing global/framework rule.
// Keep the stable Tauri IPC keys flat at the command boundary.
#[allow(clippy::too_many_arguments)]
#[tauri::command(async)]
pub fn set_page_css_rule_at_viewport(
    template_path: String,
    relative_path: String,
    selector: String,
    properties: HashMap<String, String>,
    viewport: String,
    cachebust_assets: bool,
    identity: FileBufferRequestIdentity,
    expected_selection: Option<SelectionMutationIdentity>,
    app: AppHandle,
    state: State<AppState>,
) -> Result<CssMutationCommandReceipt<PageCssWriteResult>, String> {
    set_page_css_rule_at_viewport_impl(
        template_path,
        relative_path,
        selector,
        properties,
        viewport,
        cachebust_assets,
        &identity,
        expected_selection.as_ref(),
        &app,
        &state,
    )
}

// This adapter intentionally mirrors the IPC boundary while borrowing native dependencies.
#[allow(clippy::too_many_arguments)]
fn set_page_css_rule_at_viewport_impl(
    template_path: String,
    relative_path: String,
    selector: String,
    properties: HashMap<String, String>,
    viewport: String,
    cachebust_assets: bool,
    identity: &FileBufferRequestIdentity,
    expected_selection: Option<&SelectionMutationIdentity>,
    app: &AppHandle,
    state: &State<AppState>,
) -> Result<CssMutationCommandReceipt<PageCssWriteResult>, String> {
    let properties = normalize_panel_rule_input(&selector, &properties, &viewport)?;
    let template_path = to_zola_relative_path(&template_path);
    let zola_relative_path = strip_zola_root_prefix(&relative_path).to_string();
    execute_selection_bound_css_workspace_mutation(
        app,
        state,
        identity,
        expected_selection,
        move |project_root, _zola_root, store| {
            if properties.is_empty() {
                return Ok((
                    None,
                    PageCssWriteResult {
                        file: to_project_relative_path(&zola_relative_path),
                        href: page_css_href(&template_path),
                        stylesheet_created: false,
                        template_updated: false,
                        written_files: Vec::new(),
                    },
                ));
            }
            let project_relative_path = to_project_relative_path(&zola_relative_path);
            let stylesheet_created =
                !project_relative_exists(project_root, store, &project_relative_path)?;
            let existing = read_current_zola_text(project_root, store, &zola_relative_path)?
                .unwrap_or_default();
            require_complete_style_inventory(store)?;
            let style_files = current_style_paths(store);
            let active_theme = current_active_theme(project_root, store)?;
            let prepared = prepare_page_stylesheet_source(
                &zola_relative_path,
                &existing,
                style_files,
                active_theme.as_deref(),
            );
            let breakpoints = current_css_breakpoints(project_root, store)?;
            let updated = write_rule_at_viewport(
                &breakpoints,
                &prepared,
                selector.trim(),
                &properties,
                &viewport,
            );

            let href = page_css_href(&template_path);
            let deleting_only = properties.values().all(|value| value.trim().is_empty());
            if deleting_only && !css_has_effective_rules(&updated) {
                let mut changes = Vec::new();
                let mut deletes = Vec::new();
                let mut written_files = Vec::new();
                if project_relative_exists(project_root, store, &project_relative_path)? {
                    deletes.push(WorkspaceTextDelete {
                        relative_path: project_relative_path.clone(),
                    });
                }
                let template_project_path = to_project_relative_path(&template_path);
                if let Some(template_source) =
                    read_current_zola_text(project_root, store, &template_path)?
                {
                    let template_updated = remove_page_stylesheet_link(&template_source, &href);
                    push_text_change_if_changed(
                        &mut changes,
                        &mut written_files,
                        template_project_path.clone(),
                        &template_source,
                        template_updated,
                    );
                }
                let template_updated = !written_files.is_empty();
                let input = if changes.is_empty() && deletes.is_empty() {
                    None
                } else {
                    Some(WorkspaceTextResourceMutationInput {
                        label: "Cleanup empty Page CSS rule".to_string(),
                        target: project_relative_path.clone(),
                        changes,
                        deletes,
                    })
                };
                return Ok((
                    input,
                    PageCssWriteResult {
                        file: project_relative_path,
                        href,
                        stylesheet_created: false,
                        template_updated,
                        written_files,
                    },
                ));
            }

            let mut changes = Vec::new();
            let mut written_files = Vec::new();
            push_text_change_if_changed(
                &mut changes,
                &mut written_files,
                project_relative_path.clone(),
                &existing,
                updated,
            );

            let template_written = plan_page_stylesheet_link_writes_with_reader(
                &template_path,
                &href,
                cachebust_assets,
                active_theme.as_deref(),
                |relative_path| read_current_zola_text(project_root, store, relative_path),
            )?;
            let template_updated = !template_written.is_empty();
            for file in template_written {
                let template_project_relative = to_project_relative_path(&file.relative_path);
                changes.push(WorkspaceTextChange {
                    relative_path: template_project_relative.clone(),
                    new_text: file.contents.clone(),
                });
                written_files.push(WrittenProjectFile {
                    relative_path: template_project_relative,
                    contents: file.contents,
                });
            }

            let input = if changes.is_empty() {
                None
            } else {
                Some(WorkspaceTextResourceMutationInput {
                    label: "Page CSS rule".to_string(),
                    target: project_relative_path.clone(),
                    changes,
                    deletes: Vec::new(),
                })
            };

            Ok((
                input,
                PageCssWriteResult {
                    file: project_relative_path,
                    href,
                    stylesheet_created,
                    template_updated,
                    written_files,
                },
            ))
        },
    )
}

#[cfg(test)]
mod css_inspector_context_tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use crate::project_model::test_support::ProjectModelTestFixture;

    #[test]
    fn atomic_candidates_keep_the_exact_file_and_viewport_context() {
        let candidates = css_inspector_source_candidates(
            &[
                (
                    "sass/_other.scss".to_string(),
                    ".other { color: black; }".to_string(),
                ),
                (
                    "sass/_hero.scss".to_string(),
                    ".hero-title { color: red; }\n@media (max-width: $bp-mobil) { .hero-title { color: blue; } }"
                        .to_string(),
                ),
            ],
            &CssBreakpointValues {
                tablet: Some("1024px".to_string()),
                mobile: Some("768px".to_string()),
            },
            ".hero-title",
            ".hero-title",
            "mobile",
        );

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].file, "sass/_hero.scss");
        assert!(candidates[0].rule_context.has_base_rule);
        assert!(candidates[0].rule_context.has_viewport_rule);
        assert_eq!(
            candidates[0].rule_context.resolved_breakpoint.as_deref(),
            Some("$bp-mobil")
        );
    }

    #[test]
    fn an_empty_exact_rule_is_existing_source_not_absent() {
        let candidates = css_inspector_source_candidates(
            &[(
                "sass/_hero.scss".to_string(),
                ".hero-title {\n}\n".to_string(),
            )],
            &CssBreakpointValues::default(),
            ".hero-title",
            ".hero-title",
            "desktop",
        );

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].file, "sass/_hero.scss");
        assert!(candidates[0].rule_context.has_base_rule);
        assert!(candidates[0].rule_context.has_viewport_rule);
    }

    #[test]
    fn duplicate_selector_sources_remain_explicitly_ambiguous() {
        let candidates = css_inspector_source_candidates(
            &[
                (
                    "sass/_framework.scss".to_string(),
                    ".hero-title { color: red; }".to_string(),
                ),
                (
                    "sass/pages/index.scss".to_string(),
                    ".hero-title { color: blue; }".to_string(),
                ),
            ],
            &CssBreakpointValues::default(),
            ".hero-title",
            ".hero-title",
            "desktop",
        );

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].file, "sass/_framework.scss");
        assert_eq!(candidates[1].file, "sass/pages/index.scss");
    }

    #[test]
    fn a_missing_variant_keeps_the_unique_base_rule_source() {
        assert_eq!(
            base_class_selector(".hero-title:hover").as_deref(),
            Some(".hero-title")
        );
        let candidates = css_inspector_source_candidates(
            &[(
                "sass/_hero.scss".to_string(),
                ".hero-title { color: red; }".to_string(),
            )],
            &CssBreakpointValues::default(),
            ".hero-title",
            ".hero-title:hover",
            "desktop",
        );

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].file, "sass/_hero.scss");
        assert_eq!(candidates[0].rule_context.selector, ".hero-title:hover");
        assert!(!candidates[0].rule_context.has_base_rule);
    }

    #[test]
    fn reusable_owner_wins_and_unrelated_page_rules_cannot_hijack() {
        let candidates = vec![
            CssInspectorSourceCandidate {
                file: "sass/pagini/despre.scss".to_string(),
                rule_context: get_rule_context(
                    &CssBreakpointValues::default(),
                    "sass/pagini/despre.scss".to_string(),
                    ".ps-card { color: red; }",
                    ".ps-card".to_string(),
                    "desktop".to_string(),
                ),
            },
            CssInspectorSourceCandidate {
                file: "sass/partials/listing-items/_card.scss".to_string(),
                rule_context: get_rule_context(
                    &CssBreakpointValues::default(),
                    "sass/partials/listing-items/_card.scss".to_string(),
                    ".ps-card { color: blue; }",
                    ".ps-card".to_string(),
                    "desktop".to_string(),
                ),
            },
        ];

        let scoped =
            reusable_scoped_candidates(candidates, Some("sass/partials/listing-items/_card.scss"));
        assert_eq!(scoped.len(), 1);
        assert_eq!(scoped[0].file, "sass/partials/listing-items/_card.scss");
    }

    #[test]
    fn reusable_consumers_are_derived_from_the_project_model_include_graph() {
        let root = std::env::temp_dir().join(format!(
            "pana-reusable-css-consumer-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut fixture = ProjectModelTestFixture::new(root.clone()).unwrap();
        fixture.source("zola.toml", "base_url = 'https://example.test'\n");
        fixture.source(
            "content/servicii/_index.md",
            "+++\ntitle = 'Servicii'\ntemplate = 'servicii/arhiva.html'\n+++\n",
        );
        fixture.source(
            "templates/layout.html",
            "<!doctype html><body>{% block content %}{% endblock %}</body>",
        );
        fixture.source(
            "templates/servicii/arhiva.html",
            "{% extends 'layout.html' %}{% block content %}{% include 'listing-items/card.html' %}{% endblock %}",
        );
        fixture.source(
            "templates/listing-items/card.html",
            "<article class='ps-card'></article>",
        );

        let model = fixture.build_model().unwrap();
        let consumers =
            reusable_css_consumers(&model, "templates/listing-items/card.html").unwrap();
        assert_eq!(consumers.len(), 1);
        assert_eq!(consumers[0].template_path, "templates/servicii/arhiva.html");
        assert_eq!(
            consumers[0].stylesheet_path,
            "sass/pagini/servicii-arhiva.scss"
        );
        assert_eq!(consumers[0].href, "/pagini/servicii-arhiva.css");

        fs::remove_dir_all(root).unwrap();
    }
}
