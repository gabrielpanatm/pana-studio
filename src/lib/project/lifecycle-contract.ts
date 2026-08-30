import type { DeploySettings } from "$lib/deploy/contracts";
import type { ProjectDiskManifest } from "$lib/project/external-disk-contract";
import type { TemplateWorkbenchPlan } from "$lib/project/template-workbench-contract";
import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";
import type { WorkbenchSnapshot } from "$lib/workbench/contracts";

export type ProjectSettingsSnapshot = {
  schemaVersion: typeof PROJECT_SETTINGS_SCHEMA_VERSION;
  workspaceRevision: number;
  cachebustAssets: boolean;
};

export const PROJECT_SETTINGS_SCHEMA_VERSION = 1 as const;

export type ProjectConfigurationSnapshot = {
  projectSettings: ProjectSettingsSnapshot;
  zolaSettings: ZolaProjectSettings;
};

export type ZolaProjectSettings = {
  configPath: string;
  baseUrl: string;
  title: string;
  description: string;
  defaultLanguage: string;
  author: string;
  compileSass: boolean;
  minifyHtml: boolean;
  outputDir: string;
  generateSitemap: boolean;
  generateRobotsTxt: boolean;
  excludePaginatedPagesInSitemap: boolean;
  generateFeeds: boolean;
  feedFilenames: string[];
  feedLimit: number | null;
  skipContentTemplating: string[];
  highlightDataAttrPosition: "pre" | "code" | "both" | "none" | null;
  renderEmoji: boolean;
  smartPunctuation: boolean;
  insertAnchorLinks: string;
  lazyAsyncImage: boolean;
  githubAlerts: boolean;
  bottomFootnotes: boolean;
  externalLinksTargetBlank: boolean;
  externalLinksNoFollow: boolean;
  externalLinksNoReferrer: boolean;
  buildSearchIndex: boolean;
  searchIndexFormat: string;
  searchIncludeTitle: boolean;
  searchIncludeDescription: boolean;
  searchIncludeDate: boolean;
  searchIncludePath: boolean;
  searchIncludeContent: boolean;
  searchTruncateContentLength: number | null;
};

export type ProjectFileKind = "DIR" | "HTML" | "MD" | "CSS" | "SCSS" | "JS" | "IMAGE" | "FONT" | "OTHER";

type ProjectFileRole = "page" | "template" | "style" | "script" | "asset";

export type ProjectFile = {
  name: string;
  relativePath: string;
  absolutePath: string;
  kind: ProjectFileKind;
  role: ProjectFileRole;
  previewPath: string | null;
};

export type ProjectScan = {
  root: string;
  previewBaseUrl: string | null;
  previewWarning: string | null;
  activeTheme: string | null;
  files: ProjectFile[];
  kernelSessionId?: string;
  workspaceRevision?: number;
  acceptedDiskGeneration?: number;
  acceptedDiskManifest?: ProjectDiskManifest;
};

type ProjectTransitionState =
  | "idle"
  | "inspecting"
  | "awaiting_recovery_decision"
  | "preparing"
  | "committing";

type ActiveProjectReadiness =
  | { state: "initializing_frontend" }
  | { state: "preparing_preview" }
  | { state: "awaiting_canvas" }
  | { state: "finalizing_frontend" }
  | { state: "ready" }
  | { state: "degraded"; capability: string; diagnostic: string };

type ActiveProjectLifecycleSession = {
  projectRoot: string;
  runtimeSessionId: string;
  readiness: ActiveProjectReadiness;
  committedAtMs: number;
  readinessChangedAtMs: number;
};

export type ProjectLifecycleSnapshot = {
  schemaVersion: 1;
  revision: number;
  activeSession: ActiveProjectLifecycleSession | null;
  transition: ProjectTransitionState;
  operationId: string | null;
  transitionStartedAtMs: number | null;
  reason: string;
};

type StartupStage =
  | "idle"
  | "inspecting"
  | "ready"
  | "planning"
  | "creating"
  | "error";

type StartupCandidateKind =
  | "valid_project"
  | "empty_directory"
  | "unrecognized_directory"
  | "invalid_zola_project"
  | "inaccessible";

type StartupDiagnostic = {
  code: string;
  severity: "info" | "warning" | "error";
  message: string;
  detail: string | null;
};

export type StartupCandidateSnapshot = {
  root: string;
  displayName: string;
  kind: StartupCandidateKind;
  snapshotToken: string;
  entryCount: number;
  truncated: boolean;
  diagnostics: StartupDiagnostic[];
};

export type StartupFlowSnapshot = {
  schemaVersion: 1;
  revision: number;
  stage: StartupStage;
  candidate: StartupCandidateSnapshot | null;
  diagnostics: StartupDiagnostic[];
};

export type StartupCreationKind = "minimal" | "starter";

type StartupCreationOption = {
  id: string;
  kind: StartupCreationKind;
  name: string;
  description: string;
  previewDataUrl: string | null;
  compatibilityLabel: string;
  capabilities: string[];
};

export type StartupCreationCatalog = {
  schemaVersion: 1;
  registryVersion: string;
  embeddedZolaVersion: string;
  expectedSnapshotToken: string;
  options: StartupCreationOption[];
};

export type StartupCreationPlanRequest = {
  expectedSnapshotToken: string;
  optionId: string;
};

export type StartupCreationPlan = {
  schemaVersion: 1;
  expectedSnapshotToken: string;
  planToken: string;
  projectRoot: string;
  optionId: string;
  optionKind: StartupCreationKind;
  optionName: string;
  affectedFiles: string[];
  totalBytes: number;
  diagnostics: StartupDiagnostic[];
};

export type StartupCreationApplyRequest = {
  expectedSnapshotToken: string;
  expectedPlanToken: string;
};

export type StartupCreationReceipt = {
  schemaVersion: 1;
  projectRoot: string;
  optionId: string;
  planToken: string;
  publishedFiles: string[];
  validation: string;
  startup: StartupFlowSnapshot;
};

type ProjectOpenRecoveryStatus =
  | "missing"
  | "restorable"
  | "decision_required"
  | "abandoned";

type ProjectOpenRecoveryConflictReason =
  | "disk_baseline_changed"
  | "project_root_replaced"
  | "recovery_invalid";

export type ProjectOpenRecoveryAssessment = {
  schemaVersion: number;
  status: ProjectOpenRecoveryStatus;
  projectRoot: string;
  assessmentToken: string | null;
  conflictReason: ProjectOpenRecoveryConflictReason | null;
  rootIdentityChanged: boolean | null;
  recoveryRevision: number | null;
  dirtyDocumentCount: number;
  stagedBinaryResourceCount: number;
  deletedBinaryResourceCount: number;
  pageJsDraftCount: number;
  undoCount: number;
  redoCount: number;
  acceptedFileCount: number;
  currentFileCount: number;
  diagnostic: string | null;
};

export type ProjectOpenRecoveryDecisionInput = {
  action: "abandon";
  assessmentToken: string;
};

export type ProjectOpenInspectionReceipt = {
  schemaVersion: 1;
  operationId: string;
  operationStartedAtMs: number;
  candidateToken: string;
  recovery: ProjectOpenRecoveryAssessment;
  lifecycle: ProjectLifecycleSnapshot;
};

export type ProjectOpenBootstrapReceipt = {
  schemaVersion: typeof PROJECT_OPEN_BOOTSTRAP_SCHEMA_VERSION;
  project: ProjectScan;
  lifecycle: ProjectLifecycleSnapshot;
  workspace: ProjectWorkspaceSnapshot;
  projectSettings: ProjectSettingsSnapshot;
  deploySettings: DeploySettings;
  workbench: WorkbenchSnapshot;
  activeDocument: ProjectBootstrapDocument | null;
  targetCssFile: string | null;
  initialSurface: ProjectBootstrapInitialSurface | null;
};

export const PROJECT_OPEN_BOOTSTRAP_SCHEMA_VERSION = 5 as const;

export type ProjectBootstrapInitialSurface = {
  documentPath: string;
  route: string;
  previewUrl: string;
  reuseToken: string;
  plan: TemplateWorkbenchPlan;
  canvasProjection: import("$lib/contracts/canvas-projection").CanvasProjectionPlan;
};

type ProjectBootstrapDocument = {
  relativePath: string;
  source: string;
  previewPath: string | null;
  diagnosticLocation: ProjectBootstrapSourceLocation | null;
};

export type ProjectBootstrapSourceLocation = {
  line: number;
  column: number;
};
