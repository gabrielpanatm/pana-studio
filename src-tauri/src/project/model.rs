use serde::Serialize;

use super::manifest::ProjectDiskManifest;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectScan {
    pub root: String,
    pub preview_base_url: Option<String>,
    pub preview_warning: Option<String>,
    pub active_theme: Option<String>,
    pub files: Vec<ProjectFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_revision: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_disk_manifest: Option<ProjectDiskManifest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_disk_generation: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFile {
    pub name: String,
    pub relative_path: String,
    pub absolute_path: String,
    pub kind: ProjectFileKind,
    pub role: ProjectFileRole,
    pub preview_path: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ProjectFileKind {
    Dir,
    Html,
    Md,
    Css,
    Scss,
    Js,
    Image,
    Font,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectFileRole {
    Page,
    Template,
    Style,
    Script,
    Asset,
}
