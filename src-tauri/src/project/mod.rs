mod capacity;
mod content;
mod lifecycle;
mod manifest;
mod paths;
mod scan;
mod scope;
mod starters;
mod startup;
mod watcher;

pub mod model;

pub(crate) use capacity::{require_projected_entry_capacity, PROJECT_CAPACITY};
pub use content::build_content_page_draft_with_active_theme;
pub use lifecycle::{
    ActiveProjectReadiness, ProjectLifecycleRuntime, ProjectLifecycleSnapshot,
    ProjectOpenInspectionReceipt, PROJECT_OPEN_BOOTSTRAP_SCHEMA_VERSION,
};
pub(crate) use manifest::project_disk_manifest_changed_paths;
pub(crate) use manifest::project_disk_metadata_version_token;
#[cfg(test)]
pub(crate) use manifest::{
    project_disk_manifest_traversals, reset_project_disk_manifest_traversals,
};
pub use manifest::{
    read_project_disk_manifest, AcceptedProjectDiskManifest, ProjectDiskManifest,
    ProjectDiskManifestEntry,
};
pub use model::{ProjectFile, ProjectFileKind, ProjectFileRole, ProjectScan};
pub use paths::{resolve_project_write_path, strip_zola_root_prefix, zola_project_root};
pub use scan::scan_project_workspace_projection;
pub(crate) use scan::{
    apply_project_model_preview_routes, scan_project_disk_manifest,
    scan_project_workspace_projection_full,
};
pub use scan::{is_zola_project, scan_project_root};
pub use startup::{
    apply_creation as apply_startup_creation, plan_creation as plan_startup_creation,
    read_creation_catalog as read_startup_creation_catalog, StartupCandidateKind,
    StartupCandidateSnapshot, StartupCreationApplyRequest, StartupCreationCatalog,
    StartupCreationPlan, StartupCreationPlanRequest, StartupCreationReceipt, StartupFlowRuntime,
    StartupFlowSnapshot,
};
pub(crate) use watcher::ProjectDiskWatchHandle;
