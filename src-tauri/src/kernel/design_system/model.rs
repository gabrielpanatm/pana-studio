use serde::Serialize;

use crate::source_graph::model::SourceRange;

pub const DESIGN_CLASS_INVENTORY_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DesignClassOccurrenceKind {
    Markup,
    Style,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignClassOccurrence {
    pub file: String,
    pub kind: DesignClassOccurrenceKind,
    pub range: SourceRange,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignClassEntry {
    pub name: String,
    pub markup_occurrences: usize,
    pub selector_occurrences: usize,
    pub files: Vec<String>,
    pub occurrences: Vec<DesignClassOccurrence>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignClassInventorySnapshot {
    pub schema_version: u32,
    pub project_root: String,
    pub runtime_session_id: String,
    pub workspace_revision: u64,
    pub project_model_revision: String,
    pub classes: Vec<DesignClassEntry>,
}

#[derive(Clone, Debug)]
pub struct DesignClassRenameChange {
    pub relative_path: String,
    pub contents: String,
    pub replacement_count: usize,
}

#[derive(Clone, Debug)]
pub struct DesignClassRenamePlan {
    pub old_name: String,
    pub new_name: String,
    pub changes: Vec<DesignClassRenameChange>,
    pub replacement_count: usize,
}
