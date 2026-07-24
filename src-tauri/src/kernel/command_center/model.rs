use serde::{Deserialize, Serialize};

use crate::kernel::workbench::{WorkbenchActivity, WorkbenchSurface};

pub const COMMAND_CENTER_SCHEMA_VERSION: u32 = 1;
pub const COMMAND_CENTER_DEFAULT_LIMIT: usize = 24;
pub const COMMAND_CENTER_MAX_LIMIT: usize = 100;
pub const COMMAND_CENTER_MAX_QUERY_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandCenterScope {
    #[default]
    All,
    Commands,
    Files,
    Symbols,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandCenterItemKind {
    Command,
    Activity,
    File,
    Page,
    Component,
    Style,
    Asset,
    Symbol,
    Diagnostic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandCenterAppCommand {
    OpenProject,
    CloseProject,
    Save,
    Undo,
    Redo,
    Validate,
    RunExternal,
    RefreshSession,
    RescanProject,
    ToggleTerminal,
    ShowProblems,
    ShowOutput,
    ShowTimeline,
    SplitVertical,
    SplitHorizontal,
    CloseSplit,
    CanvasFit,
    CanvasDesktop,
    CanvasTablet,
    CanvasMobile,
    ToggleLeftSidebar,
    ToggleInspector,
    ToggleTheme,
    OpenSettings,
    ShowVisual,
    ShowCode,
    ShowMarkdown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum CommandCenterAction {
    SetActivity {
        activity: WorkbenchActivity,
    },
    OpenDocument {
        relative_path: String,
        surface: WorkbenchSurface,
    },
    AppCommand {
        command: CommandCenterAppCommand,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandCenterItem {
    pub id: String,
    pub kind: CommandCenterItemKind,
    pub title: String,
    pub subtitle: String,
    pub shortcut: Option<String>,
    pub enabled: bool,
    pub disabled_reason: Option<String>,
    pub score: i32,
    pub action: CommandCenterAction,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandCenterSearchRequest {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub scope: CommandCenterScope,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub expected_project_root: Option<String>,
    #[serde(default)]
    pub expected_session_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandCenterSearchResponse {
    pub schema_version: u32,
    pub project_root: Option<String>,
    pub runtime_session_id: Option<String>,
    pub query: String,
    pub scope: CommandCenterScope,
    pub total_matches: usize,
    pub truncated: bool,
    pub results: Vec<CommandCenterItem>,
}
