use std::path::Path;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::{
    commands::css::{
        execute_css_workspace_mutation_with_metadata, with_bound_css_file_buffer_revision,
        CssMutationCommandReceipt,
    },
    commands::workspace_entries::{
        current_workspace_identity, finish_mutation, mutation_metadata, require_bound_workspace,
        WorkspaceEntryMutationReceipt,
    },
    kernel::{
        design_system::{
            build_design_class_inventory, build_theme_style_catalog, build_theme_style_preview,
            collect_theme_style_variables, plan_design_class_rename, plan_theme_style_update,
            resolve_theme_style_source, validate_class_name, DesignClassInventorySnapshot,
            ThemeStyleCatalogSnapshot, ThemeStyleDraftPreview, ThemeStylePropertyInput,
            ThemeStyleTargetSnapshot,
        },
        file_buffer_store::{FileBufferCommandReceipt, FileBufferRequestIdentity},
        observability::now_ms,
        project_path::normalize_project_relative_path,
        project_workspace::{
            WorkspaceResourceMutation, WorkspaceTextChange, WorkspaceTextResourceMutationInput,
        },
    },
    project_model::cache::{capture_project_model_build_lease, publish_project_model_if_current},
    state::AppState,
};

pub const DESIGN_CLASS_RENAME_SCHEMA_VERSION: u32 = 1;

#[tauri::command]
pub fn read_theme_style_catalog(
    identity: FileBufferRequestIdentity,
    state: State<AppState>,
) -> Result<FileBufferCommandReceipt<ThemeStyleCatalogSnapshot>, String> {
    with_bound_css_file_buffer_revision(
        state.inner(),
        &identity,
        |_project_root, _zola_root, session, store, workspace_revision| {
            let (source_path, source, source_origin) = resolve_theme_style_source(store)?;
            Ok(build_theme_style_catalog(
                &session.project_root,
                &session.runtime_instance_id(),
                workspace_revision,
                &source_path,
                &source_origin,
                &source,
            ))
        },
    )
}

#[tauri::command]
pub fn preview_theme_style_draft(
    target_id: String,
    properties: Vec<ThemeStylePropertyInput>,
    expected_workspace_revision: u64,
    identity: FileBufferRequestIdentity,
    state: State<AppState>,
) -> Result<FileBufferCommandReceipt<ThemeStyleDraftPreview>, String> {
    with_bound_css_file_buffer_revision(
        state.inner(),
        &identity,
        |_project_root, _zola_root, _session, store, workspace_revision| {
            if workspace_revision != expected_workspace_revision {
                return Err(format!(
                    "[theme_style_stale_workspace] Previzualizarea aștepta revizia {expected_workspace_revision}, dar ProjectWorkspace este la revizia {workspace_revision}."
                ));
            }
            let (source_path, source, _source_origin) = resolve_theme_style_source(store)?;
            let variables = collect_theme_style_variables(store);
            build_theme_style_preview(&source_path, &source, &target_id, &properties, &variables)
        },
    )
}

#[tauri::command(async)]
pub fn apply_theme_style_draft(
    target_id: String,
    properties: Vec<ThemeStylePropertyInput>,
    expected_workspace_revision: u64,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<CssMutationCommandReceipt<ThemeStyleTargetSnapshot>, String> {
    execute_css_workspace_mutation_with_metadata(
        &app,
        &state,
        &identity,
        Some(expected_workspace_revision),
        "design_system.theme_styles",
        None,
        |_project_root, _zola_root, store| {
            let (source_path, source, _source_origin) = resolve_theme_style_source(store)?;
            let (updated, target) =
                plan_theme_style_update(&source_path, &source, &target_id, &properties)?;
            let input = (updated != source).then_some(WorkspaceTextResourceMutationInput {
                label: format!("Stil temă: {}", target.label),
                target: source_path.clone(),
                changes: vec![WorkspaceTextChange {
                    relative_path: source_path,
                    new_text: updated,
                }],
                deletes: Vec::new(),
            });
            Ok((input, target))
        },
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignClassRenameReceipt {
    pub schema_version: u32,
    pub old_name: String,
    pub new_name: String,
    pub changed_files: Vec<String>,
    pub replacement_count: usize,
    pub workspace: WorkspaceEntryMutationReceipt,
}

#[tauri::command(async)]
pub fn create_design_class(
    name: String,
    relative_path: String,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let name = validate_class_name(&name, "Clasa nouă")?;
    let relative_path = normalize_project_relative_path(&relative_path)?;
    if !matches!(
        Path::new(&relative_path)
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("css" | "scss")
    ) {
        return Err("Clasa nouă trebuie adăugată într-un fișier CSS sau SCSS.".to_string());
    }

    let (root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = slot
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru creare clasă.".to_string())?;
    let projection = workspace.capture_projection_lease()?;
    let model =
        crate::project_model::build_project_model_from_workspace_projection(&root, &projection)?;
    let inventory =
        build_design_class_inventory(&model, workspace.runtime_session_id(), workspace.revision);
    if inventory.classes.iter().any(|entry| entry.name == name) {
        return Err(format!("Clasa .{name} există deja în sursele proiectului."));
    }
    let source = workspace
        .documents
        .text_for(&relative_path)
        .ok_or_else(|| format!("ProjectWorkspace nu urmărește stylesheet-ul {relative_path}."))?;
    let separator = if source.is_empty() || source.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    let contents = format!("{source}{separator}\n.{name} {{\n}}\n");
    let receipt_path = relative_path.clone();
    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        candidate.stage_resource_texts(
            &current_workspace_identity(candidate),
            mutation_metadata("Creare clasă", "design_system.create_class"),
            vec![WorkspaceResourceMutation {
                relative_path,
                contents,
                create_only: false,
            }],
            now_ms(),
        )
    })
}

#[tauri::command]
pub fn read_design_class_inventory(
    state: State<AppState>,
) -> Result<DesignClassInventorySnapshot, String> {
    let (root, session, lease) = capture_project_model_build_lease(&state)?;
    let model = crate::project_model::build_project_model_from_workspace_projection(
        &root,
        lease.projection(),
    )?;
    let snapshot = build_design_class_inventory(
        &model,
        session.runtime_instance_id(),
        lease.projection().revision,
    );
    publish_project_model_if_current(&state, &lease, model)?;
    Ok(snapshot)
}

#[tauri::command(async)]
pub fn rename_design_class(
    old_name: String,
    new_name: String,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<DesignClassRenameReceipt, String> {
    let (root, _session, lease) = capture_project_model_build_lease(&state)?;
    if identity.expected_project_root != lease.projection().project_root
        || identity.expected_session_id != lease.projection().runtime_session_id
    {
        return Err("Rename clasă a refuzat un request pentru alt ProjectSession.".to_string());
    }
    let model = crate::project_model::build_project_model_from_workspace_projection(
        &root,
        lease.projection(),
    )?;
    let plan = plan_design_class_rename(&model, &old_name, &new_name)?;
    let changed_files = plan
        .changes
        .iter()
        .map(|change| change.relative_path.clone())
        .collect::<Vec<_>>();
    let mutations = plan
        .changes
        .iter()
        .map(|change| WorkspaceResourceMutation {
            relative_path: change.relative_path.clone(),
            contents: change.contents.clone(),
            create_only: false,
        })
        .collect::<Vec<_>>();
    let (_root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = slot
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru rename clasă.".to_string())?;
    if workspace.revision != lease.projection().revision {
        return Err(
            "Rename clasă a fost anulat deoarece ProjectWorkspace s-a schimbat după analiză."
                .to_string(),
        );
    }
    let receipt = finish_mutation(&app, workspace, None, |candidate| {
        candidate.stage_resource_texts(
            &current_workspace_identity(candidate),
            mutation_metadata("Redenumire clasă", "design_system.rename_class"),
            mutations,
            now_ms(),
        )
    })?;
    Ok(DesignClassRenameReceipt {
        schema_version: DESIGN_CLASS_RENAME_SCHEMA_VERSION,
        old_name: plan.old_name,
        new_name: plan.new_name,
        changed_files,
        replacement_count: plan.replacement_count,
        workspace: receipt,
    })
}
