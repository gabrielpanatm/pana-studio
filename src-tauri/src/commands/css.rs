use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::Path,
};
use tauri::{AppHandle, State};

use crate::{
    css::{
        page::{
            page_css_href, page_scss_relative_path, page_target_for_template,
            plan_page_stylesheet_link_writes_with_reader, prepare_page_stylesheet_source,
            remove_page_stylesheet_link, PageCssTarget, PageCssWriteResult, WrittenProjectFile,
        },
        rules::{
            find_class_in_sources, get_class_rules as parse_class_rules, upsert_css_rule_desktop,
            CssProperty,
        },
        validation::{validate_panel_rule_input, validate_panel_variable_value},
        variables::{
            parse_variables_from_source, update_variable_in_source, variable_value_in_source,
            ScssVariable,
        },
        viewport::{
            get_rule_context, get_rules_at_viewport, write_rule_at_viewport, CssBreakpointValues,
            CssRuleContext,
        },
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
    },
    project::{strip_zola_root_prefix, zola_project_root},
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
    pub payload: T,
    pub authority: CssMutationAuthorityReceipt,
}

impl<T> CssMutationCommandReceipt<T> {
    fn noop(session: &ProjectSessionSnapshot, payload: T, workspace: &ProjectWorkspace) -> Self {
        Self {
            project_root: session.project_root.clone(),
            runtime_session_id: session.runtime_instance_id(),
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

fn to_zola_relative_paths(paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .map(|path| to_zola_relative_path(path))
        .collect()
}

fn to_project_relative_path(path: &str) -> String {
    if path.starts_with("sursa/") {
        path.to_string()
    } else {
        format!("sursa/{path}")
    }
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

fn find_class_in_current_project_styles(
    project_root: &Path,
    store: &FileBufferStore,
    preferred_files: &[String],
    selector: &str,
) -> Result<Option<(String, Vec<CssProperty>)>, String> {
    require_complete_style_inventory(store)?;
    let mut candidates = preferred_files
        .iter()
        .map(|path| to_zola_relative_path(path))
        .collect::<BTreeSet<_>>();
    candidates.extend(current_style_paths(store));
    if candidates.len() > store.limits.max_files {
        return Err(format!(
            "[css_style_inventory_limit] Inventarul CSS/SCSS cere {} fișiere, peste limita FileBufferStore de {}.",
            candidates.len(), store.limits.max_files,
        ));
    }
    let mut total_bytes = 0u64;
    find_class_in_sources(candidates, selector, |relative_path| {
        let source = read_current_zola_text(project_root, store, relative_path)?;
        if let Some(source) = source.as_ref() {
            total_bytes = total_bytes.saturating_add(source.len() as u64);
            if total_bytes > store.limits.max_total_bytes {
                return Err(format!(
                    "[css_style_inventory_budget] Citirea CSS/SCSS depășește bugetul agregat FileBufferStore de {} bytes.",
                    store.limits.max_total_bytes,
                ));
            }
        }
        Ok(source)
    })
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

fn with_bound_css_file_buffer<T>(
    state: &AppState,
    identity: &FileBufferRequestIdentity,
    operation: impl FnOnce(&Path, &Path, &ProjectSessionSnapshot, &FileBufferStore) -> Result<T, String>,
) -> Result<FileBufferCommandReceipt<T>, String> {
    // Project Transition publică în aceeași ordine. Păstrarea prefixului până
    // după receipt împiedică redeschiderea aceluiași root să schimbe runtime-ul
    // între validarea identității și proiecția citită.
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
    let accepted_disk = &workspace.accepted_disk;
    accepted_disk.require_live_complete(
        &session.runtime_instance_id(),
        &session.project_root,
        project_root,
    )?;
    let store = &workspace.documents;
    require_file_buffer_session_binding(&current_root_string, session, store, identity)?;

    let zola_root = zola_project_root(project_root);
    let payload = operation(project_root, &zola_root, session, store)?;
    accepted_disk.require_live_complete(
        &session.runtime_instance_id(),
        &session.project_root,
        project_root,
    )?;
    Ok(FileBufferCommandReceipt::new(session, payload))
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

    let (input, result_value) = build(project_root, &zola_root, &workspace.documents)?;
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
    let (mutation, removed_files) =
        commit_project_workspace_session_mutation(app, workspace, |candidate| {
            let workspace_identity = ProjectWorkspaceIdentity {
                expected_project_root: candidate.session.project_root.clone(),
                expected_session_id: candidate.runtime_session_id(),
                expected_revision: candidate.revision,
            };
            let mutation = candidate.stage_resource_changes(
                &workspace_identity,
                WorkspaceMutationMetadata {
                    label: input.label,
                    source: "css.panel".to_string(),
                    coalesce_key: Some(format!("css.panel:{}", input.target)),
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

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassSearchResult {
    pub file: String,
    pub rules: Vec<CssProperty>,
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

#[tauri::command(async)]
pub fn resolve_page_css_target(
    template_path: Option<String>,
    selector: String,
    scss_files: Vec<String>,
    fallback_file: Option<String>,
    identity: FileBufferRequestIdentity,
    state: State<AppState>,
) -> Result<FileBufferCommandReceipt<PageCssTarget>, String> {
    with_bound_css_file_buffer(
        state.inner(),
        &identity,
        move |project_root, _root, _session, store| {
            let selector = selector.trim().to_string();
            let template_path = template_path.map(|path| to_zola_relative_path(&path));
            let scss_files = to_zola_relative_paths(&scss_files);
            let fallback_file = fallback_file.map(|path| to_zola_relative_path(&path));

            if let Some((file, _rules)) =
                find_class_in_current_project_styles(project_root, store, &scss_files, &selector)?
            {
                let page_file = template_path
                    .as_deref()
                    .map(page_scss_relative_path)
                    .unwrap_or_default();
                let href = template_path.as_deref().map(page_css_href);
                let linked = template_path
                    .as_deref()
                    .zip(href.as_deref())
                    .map(|(template, href)| {
                        read_current_zola_text(project_root, store, template).map(|source| {
                            source
                                .as_deref()
                                .is_some_and(|source| template_contains_asset_path(source, href))
                        })
                    })
                    .transpose()?
                    .unwrap_or(false);
                let page_owned = !page_file.is_empty() && file == page_file;

                return Ok(PageCssTarget {
                    exists: project_relative_exists(
                        project_root,
                        store,
                        &to_project_relative_path(&file),
                    )?,
                    file: to_project_relative_path(&file),
                    selector,
                    target_kind: "existing".to_string(),
                    linked,
                    href,
                    template_path: template_path.map(|path| to_project_relative_path(&path)),
                    page_owned,
                    reason: "Regula există deja în acest fișier.".to_string(),
                });
            }

            let mut target = page_target_for_template(
                template_path.as_deref(),
                &selector,
                fallback_file
                    .as_deref()
                    .or_else(|| scss_files.first().map(String::as_str)),
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
                        source
                            .as_deref()
                            .is_some_and(|source| template_contains_asset_path(source, href))
                    })
                })
                .transpose()?
                .unwrap_or(false);
            target.file = to_project_relative_path(&target.file);
            target.template_path = target
                .template_path
                .map(|path| to_project_relative_path(&path));
            Ok(target)
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
pub fn find_class_in_scss(
    selector: String,
    scss_files: Vec<String>,
    identity: FileBufferRequestIdentity,
    state: State<AppState>,
) -> Result<FileBufferCommandReceipt<Option<ClassSearchResult>>, String> {
    with_bound_css_file_buffer(
        state.inner(),
        &identity,
        move |project_root, _root, _session, store| {
            let scss_files = to_zola_relative_paths(&scss_files);
            Ok(
                find_class_in_current_project_styles(project_root, store, &scss_files, &selector)?
                    .map(|(file, rules)| ClassSearchResult {
                        file: to_project_relative_path(&file),
                        rules,
                    }),
            )
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
pub fn get_class_rules(
    relative_path: String,
    selector: String,
    identity: FileBufferRequestIdentity,
    state: State<AppState>,
) -> Result<FileBufferCommandReceipt<Vec<CssProperty>>, String> {
    with_bound_css_file_buffer(
        state.inner(),
        &identity,
        move |project_root, _root, _session, store| {
            let zola_relative_path = strip_zola_root_prefix(&relative_path);
            let source = read_current_zola_text(project_root, store, zola_relative_path)?
                .ok_or_else(|| format!("Nu am putut citi {relative_path}."))?;
            Ok(parse_class_rules(&source, selector.trim()))
        },
    )
}

#[tauri::command(async)]
pub fn get_class_rules_at_viewport(
    relative_path: String,
    selector: String,
    viewport: String,
    identity: FileBufferRequestIdentity,
    state: State<AppState>,
) -> Result<FileBufferCommandReceipt<Vec<CssProperty>>, String> {
    with_bound_css_file_buffer(
        state.inner(),
        &identity,
        move |project_root, _root, _session, store| {
            let zola_relative_path = strip_zola_root_prefix(&relative_path);
            let source = read_current_zola_text(project_root, store, zola_relative_path)?
                .ok_or_else(|| format!("Nu am putut citi {relative_path}."))?;
            let breakpoints = current_css_breakpoints(project_root, store)?;
            Ok(get_rules_at_viewport(
                &breakpoints,
                &source,
                &viewport,
                selector.trim(),
            ))
        },
    )
}

#[tauri::command(async)]
pub fn get_css_rule_context(
    relative_path: String,
    selector: String,
    viewport: String,
    identity: FileBufferRequestIdentity,
    state: State<AppState>,
) -> Result<FileBufferCommandReceipt<CssRuleContext>, String> {
    with_bound_css_file_buffer(
        state.inner(),
        &identity,
        move |project_root, _root, _session, store| {
            let zola_relative_path = strip_zola_root_prefix(&relative_path);
            let selector = selector.trim().to_string();
            let source = read_current_zola_text(project_root, store, zola_relative_path)?
                .ok_or_else(|| format!("Nu am putut citi {relative_path}."))?;
            let breakpoints = current_css_breakpoints(project_root, store)?;
            Ok(get_rule_context(
                &breakpoints,
                to_project_relative_path(zola_relative_path),
                &source,
                selector,
                viewport,
            ))
        },
    )
}

#[tauri::command(async)]
pub fn set_css_rule(
    relative_path: String,
    selector: String,
    properties: HashMap<String, String>,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<CssMutationCommandReceipt<()>, String> {
    set_css_rule_impl(relative_path, selector, properties, &identity, &app, &state)
}

fn set_css_rule_impl(
    relative_path: String,
    selector: String,
    properties: HashMap<String, String>,
    identity: &FileBufferRequestIdentity,
    app: &AppHandle,
    state: &State<AppState>,
) -> Result<CssMutationCommandReceipt<()>, String> {
    validate_panel_rule_input(&selector, &properties, "desktop")?;
    let zola_relative_path = strip_zola_root_prefix(&relative_path).to_string();
    execute_css_workspace_mutation(
        app,
        state,
        identity,
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
#[tauri::command(async)]
pub fn set_css_rule_at_viewport(
    relative_path: String,
    selector: String,
    properties: HashMap<String, String>,
    viewport: String,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<CssMutationCommandReceipt<()>, String> {
    set_css_rule_at_viewport_impl(
        relative_path,
        selector,
        properties,
        viewport,
        &identity,
        &app,
        &state,
    )
}

fn set_css_rule_at_viewport_impl(
    relative_path: String,
    selector: String,
    properties: HashMap<String, String>,
    viewport: String,
    identity: &FileBufferRequestIdentity,
    app: &AppHandle,
    state: &State<AppState>,
) -> Result<CssMutationCommandReceipt<()>, String> {
    validate_panel_rule_input(&selector, &properties, &viewport)?;
    let zola_relative_path = strip_zola_root_prefix(&relative_path).to_string();
    execute_css_workspace_mutation(
        app,
        state,
        identity,
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

/// Write a CSS rule in a page-owned stylesheet and make sure the page template
/// links the compiled stylesheet. Used when a selector does not already belong
/// to an existing global/framework rule.
#[tauri::command(async)]
pub fn set_page_css_rule_at_viewport(
    template_path: String,
    relative_path: String,
    selector: String,
    properties: HashMap<String, String>,
    viewport: String,
    cachebust_assets: bool,
    identity: FileBufferRequestIdentity,
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
        &app,
        &state,
    )
}

fn set_page_css_rule_at_viewport_impl(
    template_path: String,
    relative_path: String,
    selector: String,
    properties: HashMap<String, String>,
    viewport: String,
    cachebust_assets: bool,
    identity: &FileBufferRequestIdentity,
    app: &AppHandle,
    state: &State<AppState>,
) -> Result<CssMutationCommandReceipt<PageCssWriteResult>, String> {
    validate_panel_rule_input(&selector, &properties, &viewport)?;
    let template_path = to_zola_relative_path(&template_path);
    let zola_relative_path = strip_zola_root_prefix(&relative_path).to_string();
    execute_css_workspace_mutation(
        app,
        state,
        identity,
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
