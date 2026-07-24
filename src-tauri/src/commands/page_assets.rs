use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::{
    commands::page_contracts::{
        apply_authoritative_page_contract, PageContractAuthorityReceipt, PageContractMutationPlan,
    },
    commands::project::require_current_project_root,
    commands::workspace_entries::{
        current_workspace_identity, finish_mutation, mutation_metadata, require_bound_workspace,
        WorkspaceEntryMutationReceipt,
    },
    js::PageJsDraftStageReceipt,
    kernel::{
        file_buffer_store::FileBufferRequestIdentity,
        observability::now_ms,
        project_path::normalize_project_relative_path,
        project_workspace::{ProjectWorkspaceMutationReceipt, WorkspaceBinaryResource},
    },
    page_assets::{
        plan_page_asset_contract as plan_contract, PageAssetContractPlan, PageAssetContractRequest,
    },
    project::strip_zola_root_prefix,
    state::AppState,
};

#[tauri::command(async)]
pub fn import_project_asset(
    source_path: String,
    destination_directory: String,
    file_name: String,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let source_path = Path::new(source_path.trim());
    let metadata = fs::symlink_metadata(source_path)
        .map_err(|error| format!("Resursa selectată nu poate fi citită: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(
            "Importul acceptă numai un fișier obișnuit, nu directoare sau symlink-uri.".to_string(),
        );
    }
    let file_name = require_asset_file_name(&file_name)?;
    let destination_directory = normalize_project_relative_path(&destination_directory)?;
    if destination_directory != "static" && !destination_directory.starts_with("static/") {
        return Err("Resursele importate trebuie păstrate sub directorul static/.".to_string());
    }
    let relative_path =
        normalize_project_relative_path(&format!("{destination_directory}/{file_name}"))?;
    let bytes = fs::read(source_path)
        .map_err(|error| format!("Resursa selectată nu a putut fi citită: {error}"))?;
    let (_root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = slot.as_mut().ok_or_else(|| {
        "ProjectWorkspace nu este inițializat pentru importul resursei.".to_string()
    })?;
    let receipt_path = relative_path.clone();
    finish_mutation(&app, workspace, Some(receipt_path), |candidate| {
        candidate.stage_binary_resource_creates(
            &current_workspace_identity(candidate),
            mutation_metadata("Import resursă", "assets.import"),
            vec![WorkspaceBinaryResource::new(relative_path, bytes)],
            now_ms(),
        )
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageAssetContractApplyReceipt {
    pub plan: PageAssetContractPlan,
    pub workspace_mutation: Option<ProjectWorkspaceMutationReceipt>,
    pub page_js: Option<PageJsDraftStageReceipt>,
    pub authority: PageContractAuthorityReceipt,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageAssetContractApplyInput {
    pub expected_project_root: String,
    pub expected_session_id: String,
    pub template_path: String,
}

#[tauri::command]
pub fn plan_page_asset_contract(
    mut input: PageAssetContractRequest,
    state: State<AppState>,
) -> Result<PageAssetContractPlan, String> {
    let _ = require_current_project_root(&state)?;
    input.template_path = strip_zola_root_prefix(&input.template_path).to_string();
    Ok(plan_contract(input))
}

#[tauri::command(async)]
pub fn apply_page_asset_contract(
    input: PageAssetContractApplyInput,
    app: AppHandle,
    state: State<AppState>,
) -> Result<PageAssetContractApplyReceipt, String> {
    apply_page_asset_contract_impl(input, &app, &state)
}

pub(crate) fn apply_page_asset_contract_impl(
    input: PageAssetContractApplyInput,
    app: &AppHandle,
    state: &State<AppState>,
) -> Result<PageAssetContractApplyReceipt, String> {
    let expected_project_root = input.expected_project_root;
    let expected_session_id = input.expected_session_id;
    let template_path = strip_zola_root_prefix(&input.template_path).to_string();
    let applied = apply_authoritative_page_contract(
        app,
        state,
        "Apply Page Asset contract",
        &expected_project_root,
        &expected_session_id,
        &template_path,
        false,
        |sources| {
            plan_contract(PageAssetContractRequest {
                template_path: sources.template_path.clone(),
                template_source: sources.template_source.clone(),
                stylesheet_source: Some(sources.stylesheet_source.clone()),
                stylesheet_known: Some(sources.stylesheet_known),
                page_js_config: Some(sources.page_js_config.clone()),
            })
        },
        |plan| PageContractMutationPlan {
            template_changed: plan.template.changed,
            template_contents: plan.template.contents.clone(),
            stylesheet_changed: plan.stylesheet.changed,
            stylesheet_contents: plan.stylesheet.contents.clone(),
            page_js_changed: plan.page_js_changed,
            page_js_config: plan.page_js_config.clone(),
        },
    )?;

    Ok(PageAssetContractApplyReceipt {
        plan: applied.plan,
        workspace_mutation: applied.workspace_mutation,
        page_js: applied.page_js,
        authority: applied.authority,
    })
}

fn require_asset_file_name(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("Numele fișierului importat este obligatoriu.".to_string());
    }
    if value.chars().count() > 180
        || Path::new(value).components().count() != 1
        || matches!(value, "." | "..")
    {
        return Err("Numele resursei trebuie să fie un singur nume de fișier valid.".to_string());
    }
    if value.chars().any(|character| {
        character.is_control()
            || matches!(
                character,
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
            )
    }) {
        return Err("Numele resursei conține caractere nepermise.".to_string());
    }
    Ok(value.to_string())
}
