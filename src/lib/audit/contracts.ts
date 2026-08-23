import type { LocalizedDiagnostic } from "$lib/contracts/localized-diagnostic";
import type {
  ProjectWorkspaceMutationReceipt,
  ProjectWorkspaceSnapshot,
} from "$lib/project/workspace-contract";
import type { SourceRange } from "$lib/source-graph/contracts";

export const AUDIT_RUN_SCHEMA_VERSION = 4 as const;

export const AUDIT_RULESET_VERSION = 1 as const;

export const AUDIT_FIX_APPLY_SCHEMA_VERSION = 1 as const;

export type AuditRunMode = "quick" | "full";

type AuditScope = { kind: "project" } | { kind: "file"; path: string };

export type AuditOutcome =
  | "pass"
  | "violation"
  | "needs_review"
  | "not_applicable"
  | "skipped"
  | "engine_error"
  | "suppressed";

export type AuditImpact = "info" | "minor" | "moderate" | "serious" | "critical";

export type AuditPolicy = "off" | "advisory" | "blocking" | "budget";

export type AuditCategory =
  | "build"
  | "references"
  | "accessibility"
  | "seo"
  | "assets"
  | "workspace"
  | "components"
  | "content"
  | "data"
  | "deploy"
  | "performance"
  | "crawl";

type AuditProviderKind =
  | "workspace_integrity"
  | "project_model"
  | "source_conformance"
  | "project_graph"
  | "component_graph"
  | "block_graph"
  | "content_models"
  | "listing_items"
  | "dynamic_widgets"
  | "template_semantics"
  | "content_semantics"
  | "asset_usage"
  | "build_zola";

export type AuditProviderStatus = "complete" | "partial" | "failed" | "skipped";

type AuditPublishCoverageRequirement = "required" | "advisory";

type AuditCompleteness = "complete" | "partial";

export type AuditSourceOrigin = "project" | "theme" | "workspace" | "generated";

type AuditEvidenceKind = "source" | "graph" | "parser" | "build" | "coverage" | "runtime";

export type AuditLocation = {
  file: string;
  range: SourceRange | null;
  origin: AuditSourceOrigin;
  sourceNodeId: string | null;
};

type AuditEvidence = {
  kind: AuditEvidenceKind;
  diagnostic: LocalizedDiagnostic;
  value: string | null;
};

type AuditFixApplicability = "safe" | "needs_confirmation" | "manual";

type AuditTextEdit = { location: AuditLocation; replacement: string };

type AuditFix = {
  id: string;
  titleDiagnostic: LocalizedDiagnostic;
  applicability: AuditFixApplicability;
  edits: AuditTextEdit[];
};

type AuditSuppressionScope = "finding" | "file" | "rule";

export type AuditSuppression = {
  ruleCode: string;
  file: string | null;
  fingerprint: string | null;
  scope: AuditSuppressionScope;
  reason: string;
};

export type AuditPolicyOverride = { ruleCode: string; policy: AuditPolicy };

export type AuditRequest = {
  mode: AuditRunMode;
  scope: AuditScope;
  policyOverrides: AuditPolicyOverride[];
  suppressions: AuditSuppression[];
};

export type AuditFinding = {
  id: string;
  fingerprint: string;
  providerId: string;
  ruleCode: string;
  category: AuditCategory;
  outcome: AuditOutcome;
  impact: AuditImpact;
  policy: AuditPolicy;
  titleDiagnostic: LocalizedDiagnostic;
  messageDiagnostic: LocalizedDiagnostic;
  primaryLocation: AuditLocation | null;
  relatedLocations: AuditLocation[];
  evidence: AuditEvidence[];
  fixes: AuditFix[];
  suppression: AuditSuppression | null;
};

type AuditSummary = {
  total: number;
  violations: number;
  needsReview: number;
  engineErrors: number;
  passed: number;
  notApplicable: number;
  skipped: number;
  suppressed: number;
  blocking: number;
  affectedFiles: number;
};

type AuditCoverage = {
  eligible: number;
  analyzed: number;
  limitations: LocalizedDiagnostic[];
};

export type AuditProviderReceipt = {
  id: string;
  kind: AuditProviderKind;
  status: AuditProviderStatus;
  publishCoverageRequirement: AuditPublishCoverageRequirement;
  findingCount: number;
  coverage: AuditCoverage;
  evidence: AuditEvidence[];
};

export type AuditRunReceipt = {
  schemaVersion: typeof AUDIT_RUN_SCHEMA_VERSION;
  rulesetVersion: typeof AUDIT_RULESET_VERSION;
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number;
  projectModelRevision: string;
  mode: AuditRunMode;
  scope: AuditScope;
  completeness: AuditCompleteness;
  summary: AuditSummary;
  providers: AuditProviderReceipt[];
  findings: AuditFinding[];
};

export type AuditFixApplyInput = {
  schemaVersion: typeof AUDIT_FIX_APPLY_SCHEMA_VERSION;
  expectedAuditSchemaVersion: typeof AUDIT_RUN_SCHEMA_VERSION;
  expectedRulesetVersion: typeof AUDIT_RULESET_VERSION;
  expectedProjectRoot: string;
  expectedSessionId: string;
  expectedWorkspaceRevision: number;
  expectedProjectModelRevision: string;
  findingFingerprint: string;
  fixId: string;
};

export type AuditFixApplyReceipt = {
  schemaVersion: typeof AUDIT_FIX_APPLY_SCHEMA_VERSION;
  findingFingerprint: string;
  fixId: string;
  mutation: ProjectWorkspaceMutationReceipt;
  workspace: ProjectWorkspaceSnapshot;
  audit: AuditRunReceipt;
};
