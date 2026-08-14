use tauri::{AppHandle, State};

mod app_config;
mod asset_links;
mod model;
mod project_settings;
mod toml_edit;
mod workspace;
mod zola_settings;

use crate::{
    commands::project::require_current_project_root,
    kernel::{file_buffer_store::FileBufferStore, project_workspace::WorkspaceResourceMutation},
    localization::LocalizedDiagnostic,
    project::zola_project_root,
    state::AppState,
};
use workspace::{
    execute_config_workspace_mutation, push_text_change_if_changed, read_current_project_text,
    read_current_project_text_from_state, workspace_mutation_input, zola_to_project_relative_path,
};

pub use model::{
    ApplicationSettingsPatchInput, ApplicationSettingsSnapshot, ProjectConfigurationInput,
    ProjectConfigurationSnapshot, ProjectSettingsSnapshot, ZolaProjectSettings,
};

#[tauri::command]
pub fn read_application_settings(
    app: AppHandle,
) -> Result<ApplicationSettingsSnapshot, LocalizedDiagnostic> {
    app_config::read_application_settings(&app)
}

#[tauri::command]
pub fn save_application_settings(
    app: AppHandle,
    settings: ApplicationSettingsPatchInput,
) -> Result<ApplicationSettingsSnapshot, LocalizedDiagnostic> {
    app_config::write_application_settings(&app, settings)
}

#[tauri::command]
pub fn read_project_configuration(
    state: State<AppState>,
) -> Result<ProjectConfigurationSnapshot, String> {
    require_current_project_root(&state)?;
    let slot = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace.".to_string())?;
    let workspace = slot
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    project_configuration_from_store(&workspace.documents, workspace.revision)
}

#[tauri::command]
pub fn save_project_configuration(
    config: ProjectConfigurationInput,
    app: AppHandle,
    state: State<AppState>,
) -> Result<ProjectConfigurationSnapshot, String> {
    let expected_revision = config.project_settings.expected_workspace_revision;
    let cachebust_assets = config.project_settings.cachebust_assets;
    let requested_zola_settings = config.zola_settings;
    let ((parsed_zola, mut project_settings), workspace_revision) =
        workspace::execute_config_workspace_mutation_at_revision(
            &app,
            &state,
            Some(expected_revision),
            |_project_root, zola_root, store| {
                let mut changes = plan_project_asset_link_rewrite_changes(store, cachebust_assets)?;

                let settings_path = project_settings::PROJECT_SETTINGS_PATH.to_string();
                let existing_settings =
                    read_current_project_text(store, &settings_path).unwrap_or_default();
                let updated_settings =
                    project_settings::serialize_project_settings(cachebust_assets)?;
                push_text_change_if_changed(
                    &mut changes,
                    settings_path.clone(),
                    &existing_settings,
                    updated_settings.clone(),
                );

                let zola_relative_path = zola_settings::zola_config_relative_path(zola_root, true);
                let project_relative_path = zola_to_project_relative_path(&zola_relative_path);
                let existing_zola =
                    read_current_project_text(store, &project_relative_path).unwrap_or_default();
                let updated_zola = zola_settings::write_zola_settings_to_source(
                    &existing_zola,
                    &requested_zola_settings,
                );
                let parsed_zola = zola_settings::parse_zola_project_settings_source(
                    &updated_zola,
                    &zola_relative_path,
                );
                push_text_change_if_changed(
                    &mut changes,
                    project_relative_path,
                    &existing_zola,
                    updated_zola,
                );

                let project_settings = project_settings::parse_project_settings_source(
                    Some(&updated_settings),
                    expected_revision,
                )?;
                Ok((
                    workspace_mutation_input(
                        "Project publication configuration",
                        ".panastudio/settings.toml+zola.toml+templates",
                        changes,
                    ),
                    (parsed_zola, project_settings),
                ))
            },
        )?;
    project_settings.workspace_revision = workspace_revision;
    state.clear_publish_authorization()?;
    Ok(ProjectConfigurationSnapshot {
        project_settings,
        zola_settings: parsed_zola,
    })
}

pub(crate) fn project_settings_from_store(
    store: &FileBufferStore,
    workspace_revision: u64,
) -> Result<ProjectSettingsSnapshot, String> {
    let source = read_current_project_text(store, project_settings::PROJECT_SETTINGS_PATH);
    project_settings::parse_project_settings_source(source.as_deref(), workspace_revision)
}

pub(crate) fn serialize_default_project_settings() -> Result<String, String> {
    project_settings::serialize_project_settings(false)
}

pub(crate) fn cachebust_assets_from_store(store: &FileBufferStore) -> Result<bool, String> {
    project_settings_from_store(store, 0).map(|settings| settings.cachebust_assets)
}

pub(crate) fn read_deploy_settings_from_state(
    state: &State<AppState>,
) -> Result<crate::deploy::DeploySettings, String> {
    let root = require_current_project_root(state)?;
    let slot = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru deploy.".to_string())?;
    let workspace = slot
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru deploy.".to_string())?;
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        &root,
    )?;
    crate::deploy::read_deploy_settings_from_store(&workspace.documents, workspace.revision)
}

pub(crate) fn save_deploy_settings_in_workspace(
    mut settings: crate::deploy::DeploySettings,
    app: &AppHandle,
    state: &State<AppState>,
) -> Result<crate::deploy::DeploySettings, String> {
    settings.validate()?;
    let expected_revision = settings.revision;
    let serialized = crate::deploy::serialize_deploy_settings(&settings)?;
    let retained_prefixes = settings
        .targets
        .iter()
        .map(|target| target.credential_env_prefix.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let ((), workspace_revision) = workspace::execute_config_workspace_mutation_at_revision(
        app,
        state,
        Some(expected_revision),
        |project_root, _zola_root, store| {
            let current = crate::deploy::read_deploy_settings_from_store(store, expected_revision)?;
            for removed in current
                .targets
                .iter()
                .filter(|target| !retained_prefixes.contains(target.credential_env_prefix.as_str()))
            {
                if !crate::kernel::project_env_store::ProjectEnvStore::read_namespace(
                    project_root,
                    &removed.credential_env_prefix,
                )?
                .is_empty()
                {
                    return Err(format!(
                        "Șterge mai întâi credentialele țintei '{}' ({}) și apoi elimină ținta.",
                        removed.name, removed.credential_env_prefix
                    ));
                }
            }
            let path = crate::deploy::DEPLOY_SETTINGS_PATH.to_string();
            let existing = read_current_project_text(store, &path).unwrap_or_default();
            let mut changes = Vec::new();
            push_text_change_if_changed(&mut changes, path.clone(), &existing, serialized);
            Ok((
                workspace_mutation_input("Deploy configuration", path, changes),
                (),
            ))
        },
    )?;
    settings.revision = workspace_revision;
    Ok(settings)
}

fn project_configuration_from_store(
    store: &FileBufferStore,
    workspace_revision: u64,
) -> Result<ProjectConfigurationSnapshot, String> {
    let zola_root = zola_project_root(std::path::Path::new(&store.project_root));
    let zola_relative_path = zola_settings::zola_config_relative_path(&zola_root, false);
    let project_relative_path = zola_to_project_relative_path(&zola_relative_path);
    let zola_source = read_current_project_text(store, &project_relative_path).unwrap_or_default();
    Ok(ProjectConfigurationSnapshot {
        project_settings: project_settings_from_store(store, workspace_revision)?,
        zola_settings: zola_settings::parse_zola_project_settings_source(
            &zola_source,
            &zola_relative_path,
        ),
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
    store: &FileBufferStore,
    cachebust_assets: bool,
) -> Result<Vec<WorkspaceResourceMutation>, String> {
    let mut changes = Vec::new();
    for zola_relative_path in asset_links::project_template_asset_link_targets(store) {
        let project_relative_path = zola_to_project_relative_path(&zola_relative_path);
        let Some(source) = read_current_project_text(store, &project_relative_path) else {
            continue;
        };
        let updated = asset_links::rewrite_template_asset_links_source(&source, cachebust_assets);
        push_text_change_if_changed(&mut changes, project_relative_path, &source, updated);
    }
    Ok(changes)
}
