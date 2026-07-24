use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::{
    commands::workspace_entries::{
        current_workspace_identity, finish_mutation, mutation_metadata, require_bound_workspace,
        WorkspaceEntryMutationReceipt,
    },
    kernel::{
        file_buffer_store::FileBufferRequestIdentity,
        observability::now_ms,
        project_path::normalize_project_relative_path,
        project_workspace::{
            ProjectWorkspace, ProjectWorkspaceMutationReceipt, WorkspaceResourceDelete,
            WorkspaceResourceMutation,
        },
        source_graph_rewrite::{
            plan_template_reference_workspace_mutation_from_graph, SourceGraphRewriteOperation,
        },
    },
    source_graph::{build_source_graph_from_workspace_projection, build_template_catalog},
    state::AppState,
};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateDraftRole {
    Page,
    Layout,
    Partial,
    MacroLibrary,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTemplateInput {
    pub name: String,
    pub role: TemplateDraftRole,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateTemplateInput {
    pub source_relative_path: String,
    pub destination_name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverrideThemeTemplateInput {
    pub source_relative_path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameTemplateInput {
    pub source_relative_path: String,
    pub destination_name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteTemplateInput {
    pub relative_path: String,
}

#[tauri::command(async)]
pub fn workspace_create_template(
    input: CreateTemplateInput,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let destination = local_template_path(&input.name)?;
    let contents = template_draft(input.role);
    let (_root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = live_workspace(&mut slot)?;
    require_destination_available(workspace, &destination)?;
    let receipt_path = destination.clone();
    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        candidate.stage_resource_texts(
            &current_workspace_identity(candidate),
            mutation_metadata("Creare șablon Tera", "templates.create"),
            vec![WorkspaceResourceMutation {
                relative_path: destination,
                contents,
                create_only: true,
            }],
            now_ms(),
        )
    })
}

#[tauri::command(async)]
pub fn workspace_duplicate_template(
    input: DuplicateTemplateInput,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let source = normalize_template_source_path(&input.source_relative_path)?;
    let destination = local_template_path(&input.destination_name)?;
    let (_root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = live_workspace(&mut slot)?;
    require_destination_available(workspace, &destination)?;
    let contents = require_template_text(workspace, &source)?;
    let receipt_path = destination.clone();
    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        candidate.stage_resource_texts(
            &current_workspace_identity(candidate),
            mutation_metadata("Duplicare șablon Tera", "templates.duplicate"),
            vec![WorkspaceResourceMutation {
                relative_path: destination,
                contents,
                create_only: true,
            }],
            now_ms(),
        )
    })
}

#[tauri::command(async)]
pub fn workspace_override_theme_template(
    input: OverrideThemeTemplateInput,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let source = normalize_template_source_path(&input.source_relative_path)?;
    theme_template_name(&source).ok_or_else(|| {
        "Suprascrierea locală cere un șablon provenit din tema activă.".to_string()
    })?;
    let (root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = live_workspace(&mut slot)?;
    let projection = workspace.capture_projection_lease()?;
    let graph = build_source_graph_from_workspace_projection(&root, &projection)?;
    let catalog = build_template_catalog(&graph);
    let entry = catalog
        .entries
        .iter()
        .find(|entry| entry.file == source && entry.effective && !entry.editable)
        .ok_or_else(|| {
            format!("Catalogul Rust nu confirmă {source} drept șablon efectiv al temei active.")
        })?;
    let destination = local_template_path(&entry.name)?;
    require_destination_available(workspace, &destination)?;
    let contents = require_template_text(workspace, &source)?;
    let receipt_path = destination.clone();
    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        candidate.stage_resource_texts(
            &current_workspace_identity(candidate),
            mutation_metadata(
                "Suprascriere locală șablon Tera",
                "templates.override_theme",
            ),
            vec![WorkspaceResourceMutation {
                relative_path: destination,
                contents,
                create_only: true,
            }],
            now_ms(),
        )
    })
}

#[tauri::command(async)]
pub fn workspace_rename_template(
    input: RenameTemplateInput,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let source = normalize_template_source_path(&input.source_relative_path)?;
    if !source.starts_with("templates/") {
        return Err(
            "Șabloanele temei sunt read-only. Creează o suprascriere locală înainte de redenumire."
                .to_string(),
        );
    }
    let destination = local_template_path(&input.destination_name)?;
    if source == destination {
        return Err("Redenumirea nu schimbă numele șablonului.".to_string());
    }

    let (root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = live_workspace(&mut slot)?;
    let receipt_path = destination.clone();
    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        stage_template_rename(&root, candidate, source, destination, now_ms())
    })
}

fn stage_template_rename(
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    source: String,
    destination: String,
    changed_at_ms: u128,
) -> Result<ProjectWorkspaceMutationReceipt, String> {
    require_destination_available(workspace, &destination)?;
    let contents = require_template_text(workspace, &source)?;
    let projection = workspace.capture_projection_lease()?;
    let graph = build_source_graph_from_workspace_projection(project_root, &projection)?;
    let rewrite = plan_template_reference_workspace_mutation_from_graph(
        project_root,
        &workspace.documents,
        &graph,
        SourceGraphRewriteOperation::Rename,
        &source,
        &destination,
    )?;
    let mut mutations = vec![WorkspaceResourceMutation {
        relative_path: destination.clone(),
        contents,
        create_only: true,
    }];
    if let Some(reference_mutation) = rewrite.workspace_mutation {
        mutations.extend(
            reference_mutation
                .changes
                .into_iter()
                .filter(|change| change.relative_path != source)
                .map(|change| WorkspaceResourceMutation {
                    relative_path: change.relative_path,
                    contents: change.new_text,
                    create_only: false,
                }),
        );
    }

    workspace.stage_composite_changes(
        &current_workspace_identity(workspace),
        mutation_metadata("Redenumire șablon Tera și referințe", "templates.rename"),
        mutations,
        vec![WorkspaceResourceDelete {
            relative_path: source,
        }],
        None,
        changed_at_ms,
    )
}

#[tauri::command(async)]
pub fn workspace_delete_template(
    input: DeleteTemplateInput,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let relative_path = normalize_template_source_path(&input.relative_path)?;
    if !relative_path.starts_with("templates/") {
        return Err("Șabloanele temei nu pot fi șterse din proiect.".to_string());
    }
    let (root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = live_workspace(&mut slot)?;
    require_template_text(workspace, &relative_path)?;
    let projection = workspace.capture_projection_lease()?;
    let graph = build_source_graph_from_workspace_projection(&root, &projection)?;
    let catalog = build_template_catalog(&graph);
    let entry = catalog
        .entries
        .iter()
        .find(|entry| entry.file == relative_path)
        .ok_or_else(|| format!("Catalogul Rust nu conține șablonul {relative_path}."))?;
    if let Some(reason) = entry.delete_blocked_reason.as_ref() {
        return Err(reason.clone());
    }

    let receipt_path = relative_path.clone();
    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        candidate.stage_resource_changes(
            &current_workspace_identity(candidate),
            mutation_metadata("Ștergere șablon Tera", "templates.delete"),
            Vec::new(),
            vec![WorkspaceResourceDelete { relative_path }],
            now_ms(),
        )
    })
}

fn live_workspace<'a>(
    slot: &'a mut std::sync::MutexGuard<'_, Option<ProjectWorkspace>>,
) -> Result<&'a mut ProjectWorkspace, String> {
    slot.as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())
}

fn local_template_path(name: &str) -> Result<String, String> {
    let logical = name
        .trim()
        .replace('\\', "/")
        .trim_start_matches("templates/")
        .to_string();
    if logical.is_empty() {
        return Err("Numele șablonului este obligatoriu.".to_string());
    }
    let logical = if logical.ends_with(".html") {
        logical
    } else {
        format!("{logical}.html")
    };
    let path = normalize_project_relative_path(&format!("templates/{logical}"))?;
    if !path.starts_with("templates/") || !path.ends_with(".html") {
        return Err("Șablonul trebuie să fie un fișier .html din templates/.".to_string());
    }
    Ok(path)
}

fn normalize_template_source_path(path: &str) -> Result<String, String> {
    let path = normalize_project_relative_path(path)?;
    let is_local = path.starts_with("templates/");
    let is_theme = theme_template_name(&path).is_some();
    if (!is_local && !is_theme) || !path.ends_with(".html") {
        return Err(
            "Operația este permisă numai pentru fișiere .html din templates/ sau themes/*/templates/."
                .to_string(),
        );
    }
    Ok(path)
}

fn theme_template_name(path: &str) -> Option<&str> {
    let remainder = path.strip_prefix("themes/")?;
    let (_theme, remainder) = remainder.split_once('/')?;
    remainder.strip_prefix("templates/")
}

fn require_template_text(
    workspace: &ProjectWorkspace,
    relative_path: &str,
) -> Result<String, String> {
    workspace
        .documents
        .text_for(relative_path)
        .ok_or_else(|| format!("ProjectWorkspace nu urmărește textul șablonului {relative_path}."))
}

fn require_destination_available(
    workspace: &ProjectWorkspace,
    relative_path: &str,
) -> Result<(), String> {
    if workspace.documents.files.contains_key(relative_path) {
        return Err(format!("Șablonul {relative_path} există deja în sesiune."));
    }
    Ok(())
}

fn template_draft(role: TemplateDraftRole) -> String {
    match role {
        TemplateDraftRole::Page => {
            "{% block content %}\n<main>\n  <h1>Șablon nou</h1>\n</main>\n{% endblock content %}\n"
                .to_string()
        }
        TemplateDraftRole::Layout => {
            "<!doctype html>\n<html lang=\"ro\">\n<head>\n  <meta charset=\"utf-8\">\n  <title>{% block title %}{{ config.title }}{% endblock title %}</title>\n</head>\n<body>\n  {% block content %}{% endblock content %}\n</body>\n</html>\n"
                .to_string()
        }
        TemplateDraftRole::Partial => "<div>\n  Fragment nou\n</div>\n".to_string(),
        TemplateDraftRole::MacroLibrary => {
            "{% macro exemplu(text) %}\n  <span>{{ text }}</span>\n{% endmacro exemplu %}\n"
                .to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

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
            project_workspace::{ProjectWorkspaceIdentity, WorkspaceHistoryDirection},
        },
        project::{AcceptedProjectDiskManifest, ProjectDiskManifest, ProjectDiskManifestEntry},
    };

    use super::*;

    #[test]
    fn local_template_path_is_canonical_and_adds_html_extension() {
        assert_eq!(
            local_template_path("partials/card").unwrap(),
            "templates/partials/card.html"
        );
        assert_eq!(
            local_template_path("templates/page.html").unwrap(),
            "templates/page.html"
        );
        assert!(local_template_path("../outside.html").is_err());
    }

    #[test]
    fn theme_template_name_extracts_only_theme_template_paths() {
        assert_eq!(
            theme_template_name("themes/pana/templates/base.html"),
            Some("base.html")
        );
        assert_eq!(theme_template_name("templates/base.html"), None);
    }

    #[test]
    fn rename_is_one_atomic_history_entry_and_round_trips_through_undo_redo() {
        let root = std::env::temp_dir().join(format!(
            "pana-template-rename-{}-{}",
            std::process::id(),
            now_ms()
        ));
        fs::create_dir_all(root.join("templates/partials")).unwrap();
        fs::create_dir_all(root.join("content")).unwrap();
        let config = "base_url = \"https://example.test\"\n";
        fs::write(root.join("zola.toml"), config).unwrap();
        let base = r#"{% include "partials/header.html" %}"#;
        let header = "<header>Header</header>";
        fs::write(root.join("templates/base.html"), base).unwrap();
        fs::write(root.join("templates/partials/header.html"), header).unwrap();

        let session = test_session(&root);
        let mut documents = FileBufferStore::for_project_session(
            &session,
            1,
            FileBufferStoreLimits {
                max_files: 32,
                max_file_bytes: 1024 * 1024,
                max_total_bytes: 4 * 1024 * 1024,
            },
        );
        insert_text(
            &mut documents,
            &root,
            "zola.toml",
            config,
            TextBufferLanguage::Toml,
            TextBufferRole::Config,
        );
        insert_template(&mut documents, &root, "templates/base.html", base);
        insert_template(
            &mut documents,
            &root,
            "templates/partials/header.html",
            header,
        );
        let accepted = AcceptedProjectDiskManifest::new(
            session.runtime_instance_id(),
            session.project_root.clone(),
            ProjectDiskManifest {
                root: session.project_root.clone(),
                files: vec![
                    ProjectDiskManifestEntry {
                        relative_path: "templates/base.html".to_string(),
                        modified_ms: 1,
                        size: base.len() as u64,
                        version_token: String::new(),
                    },
                    ProjectDiskManifestEntry {
                        relative_path: "templates/partials/header.html".to_string(),
                        modified_ms: 1,
                        size: header.len() as u64,
                        version_token: String::new(),
                    },
                    ProjectDiskManifestEntry {
                        relative_path: "zola.toml".to_string(),
                        modified_ms: 1,
                        size: config.len() as u64,
                        version_token: String::new(),
                    },
                ],
                truncated: false,
                max_files: 100,
            },
        )
        .unwrap();
        let page_js = PageJsDraftStore::new(&session);
        let mut workspace = ProjectWorkspace::new(session, accepted, documents, page_js).unwrap();

        let receipt = stage_template_rename(
            &root,
            &mut workspace,
            "templates/partials/header.html".to_string(),
            "templates/partials/site-header.html".to_string(),
            2,
        )
        .unwrap();
        assert_eq!(receipt.history.undo_count, 1);
        assert_eq!(receipt.history.redo_count, 0);
        assert!(workspace
            .documents
            .text_for("templates/partials/header.html")
            .is_none());
        assert_eq!(
            workspace
                .documents
                .text_for("templates/partials/site-header.html")
                .as_deref(),
            Some(header)
        );
        assert!(workspace
            .documents
            .text_for("templates/base.html")
            .unwrap()
            .contains("partials/site-header.html"));

        let undo = workspace.undo(&workspace_identity(&workspace), 3).unwrap();
        assert!(matches!(undo.direction, WorkspaceHistoryDirection::Undo));
        assert_eq!(undo.history.undo_count, 0);
        assert_eq!(undo.history.redo_count, 1);
        assert_eq!(
            workspace
                .documents
                .text_for("templates/partials/header.html")
                .as_deref(),
            Some(header)
        );
        assert!(workspace
            .documents
            .text_for("templates/partials/site-header.html")
            .is_none());
        assert_eq!(
            workspace
                .documents
                .text_for("templates/base.html")
                .as_deref(),
            Some(base)
        );

        let redo = workspace.redo(&workspace_identity(&workspace), 4).unwrap();
        assert!(matches!(redo.direction, WorkspaceHistoryDirection::Redo));
        assert_eq!(redo.history.undo_count, 1);
        assert_eq!(redo.history.redo_count, 0);
        assert!(workspace
            .documents
            .text_for("templates/partials/header.html")
            .is_none());
        assert_eq!(
            workspace
                .documents
                .text_for("templates/partials/site-header.html")
                .as_deref(),
            Some(header)
        );
        assert!(workspace
            .documents
            .text_for("templates/base.html")
            .unwrap()
            .contains("partials/site-header.html"));

        fs::remove_dir_all(root).unwrap();
    }

    fn test_session(root: &Path) -> ProjectSessionSnapshot {
        ProjectSessionSnapshot {
            schema_version: 1,
            id: "templates-operation-test".to_string(),
            project_root: root.to_string_lossy().to_string(),
            zola_root: root.to_string_lossy().to_string(),
            session_dir: root.join("session").to_string_lossy().to_string(),
            manifest_path: root.join("session.json").to_string_lossy().to_string(),
            opened_at_ms: 1,
            last_seen_at_ms: 1,
            root_fingerprint: ProjectRootFingerprint {
                canonical_path: root.to_string_lossy().to_string(),
                modified_ms: 1,
                size: 0,
                readonly: false,
                unix_device: None,
                unix_inode: None,
            },
            scan_summary: ProjectSessionScanSummary {
                is_zola: true,
                is_empty: false,
                active_theme: None,
                file_count: 2,
                directory_count: 3,
            },
        }
    }

    fn insert_template(store: &mut FileBufferStore, root: &Path, relative_path: &str, text: &str) {
        insert_text(
            store,
            root,
            relative_path,
            text,
            TextBufferLanguage::Html,
            TextBufferRole::Template,
        );
    }

    fn insert_text(
        store: &mut FileBufferStore,
        root: &Path,
        relative_path: &str,
        text: &str,
        language: TextBufferLanguage,
        role: TextBufferRole,
    ) {
        store.insert_loaded_file(FileBufferEntry {
            relative_path: relative_path.to_string(),
            absolute_path: root.join(relative_path).to_string_lossy().to_string(),
            language,
            role,
            baseline: FileBufferBaseline {
                hash: hash_text(text),
                modified_ms: 1,
                size: text.len() as u64,
                readonly: false,
            },
            baseline_text: text.to_string(),
            draft: None,
            revision: 1,
        });
    }

    fn workspace_identity(workspace: &ProjectWorkspace) -> ProjectWorkspaceIdentity {
        ProjectWorkspaceIdentity {
            expected_project_root: workspace.session.project_root.clone(),
            expected_session_id: workspace.runtime_session_id(),
            expected_revision: workspace.revision,
        }
    }
}
