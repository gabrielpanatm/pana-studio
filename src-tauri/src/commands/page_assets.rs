use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::{
    commands::page_contracts::{
        apply_authoritative_page_contract, PageContractAuthorityReceipt, PageContractMutationPlan,
    },
    commands::project::require_current_project_root,
    js::PageJsDraftStageReceipt,
    kernel::project_workspace::ProjectWorkspaceMutationReceipt,
    page_assets::{
        plan_page_asset_contract as plan_contract, PageAssetContractPlan, PageAssetContractRequest,
    },
    project::strip_zola_root_prefix,
    state::AppState,
};

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
