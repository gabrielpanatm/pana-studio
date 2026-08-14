pub mod ai_coordination;
pub mod audit;
mod bounded_journal_reader;
pub mod canvas_interaction;
pub mod command_center;
pub mod component_legacy_migration;
pub mod component_mutation;
pub mod content_models;
pub mod context_hub;
pub mod data_mutation;
pub mod design_system;
pub mod disk_conflict;
pub mod dynamic_widgets;
pub mod editor_navigation;
pub mod file_buffer_store;
pub mod file_explorer;
pub mod generated_assets;
pub mod global_status;
pub mod insert_catalog;
pub mod listing_items;
pub mod motion_graph;
pub mod observability;
pub mod preview_projection;
pub mod project_env_store;
pub mod project_path;
pub mod project_session;
pub mod project_state;
pub mod project_workspace;
pub mod publish_operation;
pub mod publish_preflight;
pub mod recovery_coordinator;
pub mod scratch_state;
pub mod selection_coordinator;
pub mod source_graph_rewrite;
pub mod taxonomy_mutation;
pub mod themes;
pub mod workbench;
pub mod write_authority;

use tauri::{AppHandle, Manager, Runtime};

pub fn boot<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    app.state::<write_authority::WriteAuthorityRuntime>()
        .boot_recovery()?;
    observability::record_boot(app)
}
