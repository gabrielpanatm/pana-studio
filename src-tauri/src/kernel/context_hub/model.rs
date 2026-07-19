use serde::{Deserialize, Serialize};

use crate::kernel::ai_coordination::AiCoordinationSnapshot;

pub const UI_CONTEXT_PROJECTION_SCHEMA_VERSION: u32 = 1;
pub const CONTEXT_HUB_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiCenterView {
    Preview,
    Code,
    Markdown,
    Canvas,
    Site,
    Kernel,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiPreviewDevice {
    Desktop,
    Tablet,
    Mobile,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiSourceLanguage {
    Html,
    Css,
    Scss,
    Js,
    Markdown,
    Plain,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiProjectPresentation {
    pub is_zola: bool,
    pub is_empty: bool,
    pub preview_base_url: Option<String>,
    pub preview_warning: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiWorkspaceContext {
    pub center_view: UiCenterView,
    pub preview_device: UiPreviewDevice,
    pub active_file: Option<String>,
    pub active_preview_path: Option<String>,
    pub source_language: UiSourceLanguage,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiSourceEditLocation {
    pub file: String,
    pub line: u64,
    pub column: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiSelectionRect {
    pub width: String,
    pub height: String,
    pub top: String,
    pub left: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiSelectionContext {
    pub has_selection: bool,
    pub selector: Option<String>,
    pub css_selector: Option<String>,
    pub tag: Option<String>,
    pub id: Option<String>,
    #[serde(default)]
    pub classes: Vec<String>,
    pub text: Option<String>,
    pub image_src: Option<String>,
    pub source_location: Option<UiSourceEditLocation>,
    pub source_id: Option<String>,
    pub template_source_id: Option<String>,
    pub session_id: Option<String>,
    pub rect: Option<UiSelectionRect>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiCssContext {
    pub active_selector: Option<String>,
    pub target_file: Option<String>,
    pub variables_count: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiDirtyContext {
    pub dirty: bool,
    pub can_save: bool,
    #[serde(default)]
    pub areas: Vec<String>,
    pub blocked_reason: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiMoodBoardContext {
    pub available: bool,
    pub items: usize,
    pub save_state: String,
    pub tool: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiExternalDiskContext {
    pub changed: bool,
    #[serde(default)]
    pub changed_files: Vec<String>,
    pub active_file_changed: bool,
    pub preview_relevant_changed: bool,
    pub blocked_by_dirty_session: bool,
    pub last_detected_at: Option<u64>,
    #[serde(default)]
    pub last_detected_files: Vec<String>,
    pub last_detected_active_file_changed: bool,
    pub last_detected_preview_relevant_changed: bool,
    pub last_applied_at: Option<u64>,
    #[serde(default)]
    pub last_applied_files: Vec<String>,
    pub last_checked_at: Option<u64>,
    pub checking: bool,
    pub reconciling: bool,
    pub workspace_projection_recovery_required: bool,
    pub truncated: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiContextProjection {
    pub schema_version: u32,
    pub ui_revision: u64,
    pub expected_project_session_id: Option<String>,
    pub expected_project_revision: Option<u64>,
    pub project: UiProjectPresentation,
    pub workspace: UiWorkspaceContext,
    pub selection: UiSelectionContext,
    pub css: UiCssContext,
    pub ui_dirty_state: UiDirtyContext,
    pub mood_board: UiMoodBoardContext,
    pub external_disk: UiExternalDiskContext,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiContextApp {
    pub name: String,
    pub mode: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiContextProject {
    pub root: Option<String>,
    pub session_id: Option<String>,
    pub is_open: bool,
    pub is_zola: bool,
    pub is_empty: bool,
    pub project_revision: Option<u64>,
    pub disk_generation: Option<u64>,
    pub preview_base_url: Option<String>,
    pub preview_warning: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiContextDirtyState {
    pub dirty: bool,
    pub project_workspace_dirty: bool,
    pub ui_dirty: bool,
    pub can_save: bool,
    #[serde(default)]
    pub dirty_files: Vec<String>,
    #[serde(default)]
    pub ui_areas: Vec<String>,
    pub blocked_reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiContextFileInventory {
    pub tracked_text_total: usize,
    #[serde(default)]
    pub pages: Vec<String>,
    #[serde(default)]
    pub templates: Vec<String>,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default)]
    pub scripts: Vec<String>,
    #[serde(default)]
    pub config_and_data: Vec<String>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiContextCore {
    pub app: AiContextApp,
    pub project: AiContextProject,
    pub workspace: UiWorkspaceContext,
    pub selection: UiSelectionContext,
    pub css: UiCssContext,
    pub dirty_state: AiContextDirtyState,
    pub files: AiContextFileInventory,
    pub mood_board: UiMoodBoardContext,
    pub external_disk: UiExternalDiskContext,
    pub guidance: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalAiContextSnapshot {
    pub version: u32,
    pub context_revision: u64,
    pub updated_at_ms: u128,
    pub ui_revision_seen: u64,
    #[serde(flatten)]
    pub core: AiContextCore,
    pub coordination: AiCoordinationSnapshot,
}

#[derive(Clone, Debug)]
pub struct ContextHubPublication {
    pub project_session_id: Option<String>,
    pub ui_revision: u64,
    pub core: AiContextCore,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextHubPublishReceipt {
    pub changed: bool,
    pub context_revision: u64,
    pub ui_revision_seen: u64,
    pub updated_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextHubError {
    pub diagnostic: String,
}

impl ContextHubError {
    pub fn new(diagnostic: impl Into<String>) -> Self {
        Self {
            diagnostic: diagnostic.into(),
        }
    }
}

impl std::fmt::Display for ContextHubError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.diagnostic)
    }
}

impl std::error::Error for ContextHubError {}
