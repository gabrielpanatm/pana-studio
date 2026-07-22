pub mod ai_coordination;
pub mod audit;
mod bounded_journal_reader;
pub mod command_center;
pub mod context_hub;
pub mod design_system;
pub mod disk_conflict;
pub mod file_buffer_store;
pub mod generated_assets;
pub mod motion_graph;
pub mod observability;
pub mod preview_projection;
pub mod project_path;
pub mod project_session;
pub mod project_state;
pub mod project_workspace;
pub mod publish_operation;
pub mod recovery_coordinator;
pub mod scratch_state;
pub mod source_graph_rewrite;
pub mod workbench;
pub mod write_authority;

use tauri::{AppHandle, Manager, Runtime};

pub fn boot<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    app.state::<write_authority::WriteAuthorityRuntime>()
        .boot_recovery()?;
    observability::record_boot(app)
}
