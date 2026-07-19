use std::collections::HashMap;

use tauri::{AppHandle, State};

mod app_config;
mod asset_links;
mod env;
mod model;
mod toml_edit;
mod workspace;
mod zola_settings;

use crate::{
    commands::project::require_current_project_root,
    kernel::{file_buffer_store::FileBufferStore, project_workspace::WorkspaceResourceMutation},
    project::zola_project_root,
    state::AppState,
};
use workspace::{
    execute_config_workspace_mutation, push_text_change_if_changed, read_current_project_text,
    read_current_project_text_from_state, workspace_mutation_input, zola_to_project_relative_path,
};

pub(crate) use app_config::read_project_app_config_for_root;
pub use model::{ProjectAppConfig, ProjectAppConfigInput, ZolaProjectSettings};

#[tauri::command]
pub fn read_project_app_config(
    app: AppHandle,
    state: State<AppState>,
) -> Result<ProjectAppConfig, String> {
    let root = require_current_project_root(&state)?;
    app_config::read_project_app_config_for_root(&app, &root)
}

#[tauri::command]
pub fn save_project_app_config(
    app: AppHandle,
    config: ProjectAppConfigInput,
    state: State<AppState>,
) -> Result<ProjectAppConfig, String> {
    save_project_app_config_impl(&app, config, &state)
}

fn save_project_app_config_impl(
    app: &AppHandle,
    config: ProjectAppConfigInput,
    state: &State<AppState>,
) -> Result<ProjectAppConfig, String> {
    let root = require_current_project_root(state)?;
    let stored = app_config::project_app_config_from_input(&root, config);
    let cachebust_assets = stored.cachebust_assets;
    execute_config_workspace_mutation(app, state, |project_root, zola_root, store| {
        let changes = plan_project_asset_link_rewrite_changes(
            project_root,
            zola_root,
            store,
            cachebust_assets,
        )?;
        Ok((
            workspace_mutation_input("Rewrite project asset links", "sursa/templates", changes),
            (),
        ))
    })?;
    app_config::write_project_app_config_for_root(app, &root, stored)
}

#[tauri::command]
pub fn read_project_env(state: State<AppState>) -> Result<HashMap<String, String>, String> {
    let source = read_current_project_text_from_state(&state, ".env")?.unwrap_or_default();
    Ok(env::parse_env(&source))
}

#[tauri::command]
pub fn save_project_env(
    vars: HashMap<String, String>,
    app: AppHandle,
    state: State<AppState>,
) -> Result<(), String> {
    save_project_env_impl(vars, &app, &state)
}

fn save_project_env_impl(
    vars: HashMap<String, String>,
    app: &AppHandle,
    state: &State<AppState>,
) -> Result<(), String> {
    execute_config_workspace_mutation(app, state, |_project_root, _zola_root, store| {
        let relative_path = ".env".to_string();
        let existing = read_current_project_text(store, &relative_path).unwrap_or_default();
        let updated = env::upsert_env(&existing, &vars);
        let mut changes = Vec::new();
        push_text_change_if_changed(&mut changes, relative_path.clone(), &existing, updated);
        Ok((
            workspace_mutation_input("Project env", relative_path, changes),
            (),
        ))
    })
}

#[tauri::command]
pub fn read_zola_project_settings(state: State<AppState>) -> Result<ZolaProjectSettings, String> {
    let root = zola_project_root(&require_current_project_root(&state)?);
    let zola_relative_path = zola_settings::zola_config_relative_path(&root, false);
    let project_relative_path = zola_to_project_relative_path(&zola_relative_path);
    let source =
        read_current_project_text_from_state(&state, &project_relative_path)?.unwrap_or_default();
    Ok(zola_settings::parse_zola_project_settings_source(
        &source,
        &zola_relative_path,
    ))
}

#[tauri::command]
pub fn save_zola_project_settings(
    settings: ZolaProjectSettings,
    app: AppHandle,
    state: State<AppState>,
) -> Result<ZolaProjectSettings, String> {
    save_zola_project_settings_impl(settings, &app, &state)
}

fn save_zola_project_settings_impl(
    settings: ZolaProjectSettings,
    app: &AppHandle,
    state: &State<AppState>,
) -> Result<ZolaProjectSettings, String> {
    execute_config_workspace_mutation(app, state, |_project_root, zola_root, store| {
        let zola_relative_path = zola_settings::zola_config_relative_path(zola_root, true);
        let project_relative_path = zola_to_project_relative_path(&zola_relative_path);
        let existing = read_current_project_text(store, &project_relative_path).unwrap_or_default();
        let updated = zola_settings::write_zola_settings_to_source(&existing, &settings);
        let parsed =
            zola_settings::parse_zola_project_settings_source(&updated, &zola_relative_path);
        let mut changes = Vec::new();
        push_text_change_if_changed(
            &mut changes,
            project_relative_path.clone(),
            &existing,
            updated,
        );
        Ok((
            workspace_mutation_input("Zola project settings", project_relative_path, changes),
            parsed,
        ))
    })
}

#[tauri::command]
pub fn read_zola_base_url(state: State<AppState>) -> Result<String, String> {
    let root = zola_project_root(&require_current_project_root(&state)?);
    let zola_relative_path = zola_settings::zola_config_relative_path(&root, false);
    let project_relative_path = zola_to_project_relative_path(&zola_relative_path);
    let Some(source) = read_current_project_text_from_state(&state, &project_relative_path)? else {
        return Ok(String::new());
    };
    Ok(zola_settings::extract_base_url_from_source(&source))
}

#[tauri::command]
pub fn save_zola_base_url(
    url: String,
    app: AppHandle,
    state: State<AppState>,
) -> Result<(), String> {
    save_zola_base_url_impl(url, &app, &state)
}

fn save_zola_base_url_impl(
    url: String,
    app: &AppHandle,
    state: &State<AppState>,
) -> Result<(), String> {
    execute_config_workspace_mutation(app, state, |_project_root, zola_root, store| {
        let zola_relative_path = zola_settings::zola_config_relative_path(zola_root, true);
        let project_relative_path = zola_to_project_relative_path(&zola_relative_path);
        let existing = read_current_project_text(store, &project_relative_path)
            .ok_or_else(|| "zola.toml/config.toml nu există în proiect.".to_string())?;
        let updated = zola_settings::write_zola_base_url_to_source(&existing, &url);
        let mut changes = Vec::new();
        push_text_change_if_changed(
            &mut changes,
            project_relative_path.clone(),
            &existing,
            updated,
        );
        Ok((
            workspace_mutation_input("Zola base_url", project_relative_path, changes),
            (),
        ))
    })
}

fn plan_project_asset_link_rewrite_changes(
    _project_root: &std::path::Path,
    zola_root: &std::path::Path,
    store: &FileBufferStore,
    cachebust_assets: bool,
) -> Result<Vec<WorkspaceResourceMutation>, String> {
    let mut changes = Vec::new();
    for zola_relative_path in asset_links::project_template_asset_link_targets(zola_root)? {
        let project_relative_path = zola_to_project_relative_path(&zola_relative_path);
        let Some(source) = read_current_project_text(store, &project_relative_path) else {
            continue;
        };
        let updated = asset_links::rewrite_template_asset_links_source(&source, cachebust_assets);
        push_text_change_if_changed(&mut changes, project_relative_path, &source, updated);
    }
    Ok(changes)
}
