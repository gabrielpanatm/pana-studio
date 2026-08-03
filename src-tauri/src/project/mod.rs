mod content;
mod lifecycle;
mod manifest;
mod paths;
mod scan;
mod scope;
mod site_structure;
mod startup;
mod watcher;

pub mod model;

pub use content::build_content_page_draft_with_active_theme;
pub use lifecycle::{
    ActiveProjectReadiness, ProjectLifecycleRuntime, ProjectLifecycleSnapshot,
    ProjectOpenInspectionReceipt, PROJECT_OPEN_BOOTSTRAP_SCHEMA_VERSION,
};
pub(crate) use manifest::project_disk_manifest_changed_paths;
pub(crate) use manifest::project_disk_metadata_version_token;
pub use manifest::{
    read_project_disk_manifest, AcceptedProjectDiskManifest, ProjectDiskManifest,
    ProjectDiskManifestEntry,
};
pub use model::{ProjectFile, ProjectFileKind, ProjectFileRole, ProjectScan};
pub use paths::{resolve_project_write_path, strip_zola_root_prefix, zola_project_root};
pub use scan::scan_project_workspace_projection;
pub(crate) use scan::MAX_SCAN_FILES as PROJECT_SCAN_MAX_ENTRIES;
pub(crate) use scan::{
    apply_project_model_preview_routes, scan_project_disk_manifest,
    scan_project_workspace_projection_full,
};
pub use scan::{is_zola_project, scan_project_root};
#[allow(unused_imports)]
pub use site_structure::{
    plan_site_archive_structure, plan_site_page_structure, plan_site_partial_include,
    plan_site_partial_structure, plan_site_single_structure, PlannedSiteArchiveStructure,
    PlannedSitePageStructure, PlannedSitePartialInclude, PlannedSitePartialStructure,
    PlannedSiteSingleStructure, SiteArchiveStructureInput, SitePageStructureInput,
    SitePartialIncludeInput, SitePartialStructureInput, SiteSingleStructureInput,
    SiteTemplateWriteOrigin, SiteTextChange, DEFAULT_ARCHIVE_PAGINATE_BY,
    DEFAULT_ARCHIVE_PAGINATE_PATH,
};
pub use startup::{
    apply_creation as apply_startup_creation, plan_creation as plan_startup_creation,
    read_creation_catalog as read_startup_creation_catalog, StartupCandidateKind,
    StartupCandidateSnapshot, StartupCreationApplyRequest, StartupCreationCatalog,
    StartupCreationPlan, StartupCreationPlanRequest, StartupCreationReceipt, StartupFlowRuntime,
    StartupFlowSnapshot,
};
pub(crate) use watcher::ProjectDiskWatchHandle;
