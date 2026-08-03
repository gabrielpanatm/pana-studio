use serde::{Deserialize, Serialize};

pub const WORKBENCH_SCHEMA_VERSION: u32 = 1;
pub const WORKBENCH_COMMAND_SCHEMA_VERSION: u32 = 1;
pub const WORKBENCH_MAX_OPEN_DOCUMENTS: usize = 64;
pub const WORKBENCH_DEFAULT_SPLIT_RATIO_BASIS_POINTS: u16 = 5_000;
pub const WORKBENCH_MIN_SPLIT_RATIO_BASIS_POINTS: u16 = 2_000;
pub const WORKBENCH_MAX_SPLIT_RATIO_BASIS_POINTS: u16 = 8_000;
pub const WORKBENCH_MIN_VIEWPORT_WIDTH_PX: u16 = 320;
pub const WORKBENCH_MAX_VIEWPORT_WIDTH_PX: u16 = 3_840;
pub const WORKBENCH_MIN_VIEWPORT_ZOOM_PERCENT: u16 = 25;
pub const WORKBENCH_MAX_VIEWPORT_ZOOM_PERCENT: u16 = 200;

fn default_split_ratio_basis_points() -> u16 {
    WORKBENCH_DEFAULT_SPLIT_RATIO_BASIS_POINTS
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkbenchActivity {
    #[default]
    Editor,
    Themes,
    #[serde(alias = "site")]
    Templates,
    Components,
    Blocks,
    DesignSystem,
    Assets,
    Content,
    ContentModels,
    Taxonomies,
    Data,
    Versioning,
    Audit,
    Publish,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkbenchSurface {
    #[default]
    Visual,
    #[serde(alias = "markdown")]
    Code,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentWorkspaceMode {
    #[default]
    List,
    Edit,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentWorkspaceSnapshot {
    #[serde(default)]
    pub mode: ContentWorkspaceMode,
    #[serde(default)]
    pub page_path: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkbenchProjectEntryKind {
    Directory,
    Text,
    Binary,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchProjectEntrySelection {
    pub relative_path: String,
    pub kind: WorkbenchProjectEntryKind,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchProjectEntryRemap {
    pub source_prefix: String,
    pub destination_prefix: String,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkbenchSplit {
    #[default]
    None,
    Vertical,
    Horizontal,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkbenchGroupId {
    #[default]
    Primary,
    Secondary,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkbenchBottomPanelView {
    #[default]
    #[serde(alias = "timeline")]
    Problems,
    Output,
    Terminal,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkbenchCanvasMode {
    #[default]
    Fit,
    Fixed,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkbenchCanvasPreset {
    #[default]
    Desktop,
    Tablet,
    Mobile,
    Custom,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchIdentity {
    pub expected_project_root: String,
    pub expected_runtime_session_id: String,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchDocumentSnapshot {
    pub document_id: String,
    pub relative_path: String,
    pub title: String,
    pub surface: WorkbenchSurface,
    pub pinned: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchGroupSnapshot {
    pub group_id: WorkbenchGroupId,
    pub documents: Vec<WorkbenchDocumentSnapshot>,
    pub active_document_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchBottomPanelSnapshot {
    pub open: bool,
    pub active_view: WorkbenchBottomPanelView,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchCanvasViewportSnapshot {
    pub mode: WorkbenchCanvasMode,
    pub preset: WorkbenchCanvasPreset,
    pub width_px: u16,
    pub zoom_percent: u16,
    pub show_rulers: bool,
}

impl Default for WorkbenchCanvasViewportSnapshot {
    fn default() -> Self {
        Self {
            mode: WorkbenchCanvasMode::Fit,
            preset: WorkbenchCanvasPreset::Desktop,
            width_px: 1_440,
            zoom_percent: 100,
            show_rulers: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchSnapshot {
    pub schema_version: u32,
    pub project_root: String,
    pub project_session_id: String,
    pub runtime_session_id: String,
    pub revision: u64,
    pub active_activity: WorkbenchActivity,
    pub active_group_id: WorkbenchGroupId,
    pub split: WorkbenchSplit,
    #[serde(default = "default_split_ratio_basis_points")]
    pub split_ratio_basis_points: u16,
    #[serde(default)]
    pub canvas_viewport: WorkbenchCanvasViewportSnapshot,
    pub groups: Vec<WorkbenchGroupSnapshot>,
    pub bottom_panel: WorkbenchBottomPanelSnapshot,
    #[serde(default)]
    pub content_workspace: ContentWorkspaceSnapshot,
    #[serde(default)]
    pub selected_project_entry: Option<WorkbenchProjectEntrySelection>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum WorkbenchIntent {
    OpenDocument {
        relative_path: String,
        #[serde(default)]
        group_id: WorkbenchGroupId,
        #[serde(default)]
        surface: WorkbenchSurface,
        #[serde(default)]
        pinned: bool,
    },
    SelectProjectEntry {
        relative_path: String,
        entry_kind: WorkbenchProjectEntryKind,
        #[serde(default)]
        open_surface: Option<WorkbenchSurface>,
    },
    ReconcileProjectEntries {
        #[serde(default)]
        remaps: Vec<WorkbenchProjectEntryRemap>,
        #[serde(default)]
        deleted_prefixes: Vec<String>,
        #[serde(default)]
        selection_override: Option<WorkbenchProjectEntrySelection>,
    },
    ActivateDocument {
        document_id: String,
        group_id: WorkbenchGroupId,
    },
    CloseDocument {
        document_id: String,
        group_id: WorkbenchGroupId,
    },
    MoveDocument {
        document_id: String,
        from_group_id: WorkbenchGroupId,
        to_group_id: WorkbenchGroupId,
        #[serde(default)]
        index: Option<usize>,
    },
    SetDocumentSurface {
        document_id: String,
        group_id: WorkbenchGroupId,
        surface: WorkbenchSurface,
    },
    SetSplit {
        split: WorkbenchSplit,
    },
    ConfigureSynchronizedSplit {
        split: WorkbenchSplit,
        relative_path: String,
        secondary_surface: WorkbenchSurface,
    },
    SetSplitRatio {
        ratio_basis_points: u16,
    },
    SetCanvasViewport {
        viewport: WorkbenchCanvasViewportSnapshot,
    },
    SetActivity {
        activity: WorkbenchActivity,
    },
    OpenContentPage {
        relative_path: String,
    },
    SetBottomPanel {
        open: bool,
        active_view: WorkbenchBottomPanelView,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchCommandReceipt {
    pub schema_version: u32,
    pub changed: bool,
    pub project_root: String,
    pub runtime_session_id: String,
    pub revision_before: u64,
    pub revision_after: u64,
    pub snapshot: WorkbenchSnapshot,
}

#[cfg(test)]
mod tests {
    use super::{WorkbenchActivity, WorkbenchBottomPanelView, WorkbenchSurface};

    #[test]
    fn legacy_site_activity_migrates_to_templates() {
        let activity: WorkbenchActivity =
            serde_json::from_str(r#""site""#).expect("legacy site activity");

        assert_eq!(activity, WorkbenchActivity::Templates);
        assert_eq!(
            serde_json::to_string(&activity).expect("templates activity"),
            r#""templates""#,
        );
    }

    #[test]
    fn legacy_markdown_surface_migrates_to_code() {
        let surface: WorkbenchSurface =
            serde_json::from_str(r#""markdown""#).expect("legacy Markdown surface");

        assert_eq!(surface, WorkbenchSurface::Code);
        assert_eq!(
            serde_json::to_string(&surface).expect("code surface"),
            r#""code""#,
        );
    }

    #[test]
    fn legacy_bottom_timeline_migrates_to_problems() {
        let view: WorkbenchBottomPanelView =
            serde_json::from_str(r#""timeline""#).expect("legacy timeline view");

        assert_eq!(view, WorkbenchBottomPanelView::Problems);
        assert_eq!(
            serde_json::to_string(&view).expect("problems view"),
            r#""problems""#,
        );
    }
}
