use serde::Serialize;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WrittenProjectFile {
    pub relative_path: String,
    pub contents: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageCssTarget {
    pub file: String,
    pub selector: String,
    pub target_kind: String,
    pub exists: bool,
    pub linked: bool,
    pub href: Option<String>,
    pub template_path: Option<String>,
    pub page_owned: bool,
    pub reason: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageCssWriteResult {
    pub file: String,
    pub href: String,
    pub stylesheet_created: bool,
    pub template_updated: bool,
    pub written_files: Vec<WrittenProjectFile>,
}
