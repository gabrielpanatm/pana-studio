use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::{
    commands::page_contracts::{
        apply_authoritative_page_contract, PageContractAuthorityReceipt, PageContractMutationPlan,
    },
    commands::project::require_current_project_root,
    js::PageJsDraftStageReceipt,
    kernel::project_workspace::ProjectWorkspaceMutationReceipt,
    page_components::page_component_registry_snapshot,
    page_components::{
        plan_page_component_contract as plan_contract, PageComponentContractPlan,
        PageComponentContractRequest, PageComponentRegistrySnapshot,
    },
    project::strip_zola_root_prefix,
    state::AppState,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageComponentContractApplyReceipt {
    pub plan: PageComponentContractPlan,
    pub workspace_mutation: Option<ProjectWorkspaceMutationReceipt>,
    pub page_js: Option<PageJsDraftStageReceipt>,
    pub authority: PageContractAuthorityReceipt,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageComponentContractApplyInput {
    pub expected_project_root: String,
    pub expected_session_id: String,
    pub template_path: String,
    pub ensure_component_id: Option<String>,
    pub cachebust_assets: Option<bool>,
}

#[tauri::command]
pub fn read_page_component_registry() -> PageComponentRegistrySnapshot {
    page_component_registry_snapshot()
}

#[tauri::command]
pub fn plan_page_component_contract(
    mut input: PageComponentContractRequest,
    state: State<AppState>,
) -> Result<PageComponentContractPlan, String> {
    let _ = require_current_project_root(&state)?;
    input.template_path = strip_zola_root_prefix(&input.template_path).to_string();
    Ok(plan_contract(input))
}

#[tauri::command(async)]
pub fn apply_page_component_contract(
    input: PageComponentContractApplyInput,
    app: AppHandle,
    state: State<AppState>,
) -> Result<PageComponentContractApplyReceipt, String> {
    apply_page_component_contract_impl(input, &app, &state)
}

pub(crate) fn apply_page_component_contract_impl(
    input: PageComponentContractApplyInput,
    app: &AppHandle,
    state: &State<AppState>,
) -> Result<PageComponentContractApplyReceipt, String> {
    let cachebust_assets = input.cachebust_assets.unwrap_or(false);
    let expected_project_root = input.expected_project_root;
    let expected_session_id = input.expected_session_id;
    let template_path = strip_zola_root_prefix(&input.template_path).to_string();
    let ensure_component_id = input.ensure_component_id;
    let applied = apply_authoritative_page_contract(
        app,
        state,
        "Apply Page Component contract",
        &expected_project_root,
        &expected_session_id,
        &template_path,
        cachebust_assets,
        |sources| {
            plan_contract(PageComponentContractRequest {
                template_path: sources.template_path.clone(),
                template_source: sources.template_source.clone(),
                stylesheet_source: Some(sources.stylesheet_source.clone()),
                page_js_config: Some(sources.page_js_config.clone()),
                ensure_component_id,
                cachebust_assets: Some(cachebust_assets),
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

    Ok(PageComponentContractApplyReceipt {
        plan: applied.plan,
        workspace_mutation: applied.workspace_mutation,
        page_js: applied.page_js,
        authority: applied.authority,
    })
}
