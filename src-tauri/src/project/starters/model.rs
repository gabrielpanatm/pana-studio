use serde::{Deserialize, Serialize};

pub const PROJECT_STARTER_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStarterKind {
    Minimal,
    Starter,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectStarterManifest {
    pub schema_version: u32,
    pub id: String,
    pub kind: ProjectStarterKind,
    pub display_name: String,
    pub summary: String,
    pub version: String,
    pub category: String,
    #[serde(default)]
    pub preview: Option<String>,
    pub zola: ProjectStarterZolaCompatibility,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectStarterZolaCompatibility {
    pub minimum: String,
    pub tested: String,
}
