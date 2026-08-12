use serde::{Deserialize, Serialize};

use crate::{localization::LocalizedDiagnostic, source_graph::model::SourceRange};

pub const AUDIT_RUN_SCHEMA_VERSION: u32 = 4;
pub const AUDIT_RULESET_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditRunMode {
    #[default]
    Quick,
    Full,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AuditScope {
    #[default]
    Project,
    File {
        path: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    Pass,
    Violation,
    NeedsReview,
    NotApplicable,
    Skipped,
    EngineError,
    Suppressed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditImpact {
    Info,
    Minor,
    Moderate,
    Serious,
    Critical,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditPolicy {
    Off,
    Advisory,
    Blocking,
    Budget,
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
    Components,
    Content,
    Data,
    Deploy,
    Performance,
    Crawl,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditProviderKind {
    WorkspaceIntegrity,
    ProjectModel,
    SourceConformance,
    ProjectGraph,
    ComponentGraph,
    BlockGraph,
    ContentModels,
    ListingItems,
    DynamicWidgets,
    TemplateSemantics,
    ContentSemantics,
    AssetUsage,
    BuildZola,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditProviderStatus {
    Complete,
    Partial,
    Failed,
    Skipped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditPublishCoverageRequirement {
    Required,
    Advisory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditCompleteness {
    Complete,
    Partial,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditSourceOrigin {
    Project,
    Theme,
    Workspace,
    Generated,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLocation {
    pub file: String,
    pub range: Option<SourceRange>,
    pub origin: AuditSourceOrigin,
    pub source_node_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditEvidenceKind {
    Source,
    Graph,
    Parser,
    Build,
    Coverage,
    Runtime,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEvidence {
    pub kind: AuditEvidenceKind,
    pub diagnostic: LocalizedDiagnostic,
    pub value: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditFixApplicability {
    Safe,
    NeedsConfirmation,
    Manual,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditTextEdit {
    pub location: AuditLocation,
    pub replacement: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditFix {
    pub id: String,
    pub title_diagnostic: LocalizedDiagnostic,
    pub applicability: AuditFixApplicability,
    pub edits: Vec<AuditTextEdit>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditSuppressionScope {
    Finding,
    File,
    Rule,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditSuppression {
    pub rule_code: String,
    pub file: Option<String>,
    #[serde(default)]
    pub fingerprint: Option<String>,
    pub scope: AuditSuppressionScope,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditPolicyOverride {
    pub rule_code: String,
    pub policy: AuditPolicy,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditRequest {
    #[serde(default)]
    pub mode: AuditRunMode,
    #[serde(default)]
    pub scope: AuditScope,
    #[serde(default)]
    pub policy_overrides: Vec<AuditPolicyOverride>,
    #[serde(default)]
    pub suppressions: Vec<AuditSuppression>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditFinding {
    pub id: String,
    pub fingerprint: String,
    pub provider_id: String,
    pub rule_code: String,
    pub category: AuditCategory,
    pub outcome: AuditOutcome,
    pub impact: AuditImpact,
    pub policy: AuditPolicy,
    pub title_diagnostic: LocalizedDiagnostic,
    pub message_diagnostic: LocalizedDiagnostic,
    pub primary_location: Option<AuditLocation>,
    pub related_locations: Vec<AuditLocation>,
    pub evidence: Vec<AuditEvidence>,
    pub fixes: Vec<AuditFix>,
    pub suppression: Option<AuditSuppression>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditCoverage {
    pub eligible: usize,
    pub analyzed: usize,
    pub limitations: Vec<LocalizedDiagnostic>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditProviderReceipt {
    pub id: String,
    pub kind: AuditProviderKind,
    pub status: AuditProviderStatus,
    pub publish_coverage_requirement: AuditPublishCoverageRequirement,
    pub finding_count: usize,
    pub coverage: AuditCoverage,
    pub evidence: Vec<AuditEvidence>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditSummary {
    pub total: usize,
    pub violations: usize,
    pub needs_review: usize,
    pub engine_errors: usize,
    pub passed: usize,
    pub not_applicable: usize,
    pub skipped: usize,
    pub suppressed: usize,
    pub blocking: usize,
    pub affected_files: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditRunReceipt {
    pub schema_version: u32,
    pub ruleset_version: u32,
    pub project_root: String,
    pub runtime_session_id: String,
    pub workspace_revision: u64,
    pub project_model_revision: String,
    pub mode: AuditRunMode,
    pub scope: AuditScope,
    pub completeness: AuditCompleteness,
    pub summary: AuditSummary,
    pub providers: Vec<AuditProviderReceipt>,
    pub findings: Vec<AuditFinding>,
}

#[derive(Clone, Debug)]
pub enum AuditBuildEvidence {
    Complete { message: String },
    Failed { message: String },
    Skipped { message: String },
}
