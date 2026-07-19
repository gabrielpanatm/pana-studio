use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSessionSnapshot {
    pub schema_version: u32,
    pub id: String,
    pub project_root: String,
    pub zola_root: String,
    pub session_dir: String,
    pub manifest_path: String,
    pub opened_at_ms: u128,
    pub last_seen_at_ms: u128,
    pub root_fingerprint: ProjectRootFingerprint,
    pub scan_summary: ProjectSessionScanSummary,
}

impl ProjectSessionSnapshot {
    /// Identifies one live opening of a project, while `id` remains the stable
    /// per-project identity used by persistent session storage.
    pub fn runtime_instance_id(&self) -> String {
        format!("{}:{:032x}", self.id, self.opened_at_ms)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRootFingerprint {
    pub canonical_path: String,
    pub modified_ms: u128,
    pub size: u64,
    pub readonly: bool,
    pub unix_device: Option<String>,
    pub unix_inode: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSessionScanSummary {
    pub is_zola: bool,
    pub is_empty: bool,
    pub active_theme: Option<String>,
    pub file_count: usize,
    pub directory_count: usize,
}
