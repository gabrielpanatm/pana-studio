use serde::{Deserialize, Serialize};

use crate::kernel::project_workspace::{
    ProjectWorkspaceIdentity, ProjectWorkspaceMutationReceipt, ProjectWorkspaceSnapshot,
};

pub const THEME_PACK_SCHEMA_VERSION: u32 = 1;
pub const THEME_CATALOG_SCHEMA_VERSION: u32 = 1;
pub const THEME_PLAN_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeManifest {
    pub schema_version: u32,
    pub id: String,
    pub display_name: String,
    pub summary: String,
    pub version: String,
    pub category: String,
    pub preview: String,
    pub zola: ThemeZolaCompatibility,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub required_pages: Vec<String>,
    #[serde(default)]
    pub required_data: Vec<String>,
    #[serde(default)]
    pub editor_anchors: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeZolaCompatibility {
    pub minimum: String,
    pub tested: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeCompatibilitySnapshot {
    pub minimum: String,
    pub tested: String,
    pub embedded: String,
    pub compatible: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeStatus {
    Available,
    Installed,
    Active,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemePackSnapshot {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub category: String,
    pub author: Option<String>,
    pub license: Option<String>,
    pub homepage: Option<String>,
    pub preview_data_url: String,
    pub compatibility: ThemeCompatibilitySnapshot,
    pub capabilities: Vec<String>,
    pub required_pages: Vec<String>,
    pub required_data: Vec<String>,
    pub editor_anchors: Vec<String>,
    pub theme_file_count: usize,
    pub theme_bytes: u64,
    pub recipe_file_count: usize,
    pub recipe_bytes: u64,
    pub status: ThemeStatus,
    pub install_complete: bool,
    pub local_override_count: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeCatalogSnapshot {
    pub schema_version: u32,
    pub registry_version: String,
    pub embedded_zola_version: String,
    pub project_root: Option<String>,
    pub runtime_session_id: Option<String>,
    pub revision: Option<u64>,
    pub active_theme_id: Option<String>,
    pub themes: Vec<ThemePackSnapshot>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeOperation {
    Install,
    Activate,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemePlanRequest {
    pub theme_id: String,
    pub operation: ThemeOperation,
    pub identity: ProjectWorkspaceIdentity,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeApplyRequest {
    pub plan: ThemePlanRequest,
    pub expected_plan_token: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeImpactItem {
    pub code: String,
    pub message: String,
    pub relative_path: Option<String>,
    pub blocking: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemePlan {
    pub schema_version: u32,
    pub theme_id: String,
    pub operation: ThemeOperation,
    pub expected_project_root: String,
    pub expected_session_id: String,
    pub expected_revision: u64,
    pub plan_token: String,
    pub changed: bool,
    pub blocking: bool,
    pub affected_files: Vec<String>,
    pub conflicts: Vec<ThemeImpactItem>,
    pub missing_requirements: Vec<ThemeImpactItem>,
    pub local_overrides: Vec<ThemeImpactItem>,
    pub notices: Vec<ThemeImpactItem>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeApplyReceipt {
    pub schema_version: u32,
    pub plan: ThemePlan,
    pub mutation: ProjectWorkspaceMutationReceipt,
    pub workspace: ProjectWorkspaceSnapshot,
}
