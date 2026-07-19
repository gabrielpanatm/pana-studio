use serde::{Deserialize, Serialize};

pub const GENERATED_ASSET_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeneratedAssetId {
    AnimeJsRuntime,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeneratedAssetAction {
    EnsurePresent,
    RemoveIfMatching,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeneratedAssetPlanStatus {
    Noop,
    Ready,
    Blocked,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeneratedAssetDiskState {
    Missing,
    Matching,
    Different,
    Directory,
    Symlink,
    Unreadable,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedAssetPlan {
    pub schema_version: u32,
    pub asset_id: GeneratedAssetId,
    pub asset_label: String,
    pub action: GeneratedAssetAction,
    pub zola_relative_path: String,
    pub project_relative_path: String,
    pub absolute_path: String,
    pub expected_hash: String,
    pub expected_bytes: u64,
    pub disk_state: GeneratedAssetDiskState,
    pub disk_hash: Option<String>,
    pub status: GeneratedAssetPlanStatus,
    pub diagnostics: Vec<String>,
}

impl GeneratedAssetId {
    pub fn code(self) -> &'static str {
        match self {
            GeneratedAssetId::AnimeJsRuntime => "anime_js_runtime",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            GeneratedAssetId::AnimeJsRuntime => "Anime.js runtime",
        }
    }
}
