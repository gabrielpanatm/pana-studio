use serde::Serialize;

use crate::source_graph::model::SourceRange;

pub const PROJECT_AUDIT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditCategory {
    Build,
    References,
    Accessibility,
    Seo,
    Assets,
    Workspace,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditDiagnostic {
    pub id: String,
    pub severity: AuditSeverity,
    pub category: AuditCategory,
    pub code: String,
    pub title: String,
    pub message: String,
    pub file: Option<String>,
    pub range: Option<SourceRange>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditSummary {
    pub total: usize,
    pub errors: usize,
    pub warnings: usize,
    pub info: usize,
    pub affected_files: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAuditSnapshot {
    pub schema_version: u32,
    pub project_root: String,
    pub runtime_session_id: String,
    pub workspace_revision: u64,
    pub project_model_revision: String,
    pub summary: AuditSummary,
    pub diagnostics: Vec<AuditDiagnostic>,
}
