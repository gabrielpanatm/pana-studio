import type { LocalizedDiagnostic } from "$lib/contracts/localized-diagnostic";
import type {
  FileBufferBaseline,
  FileBufferTextSnapshot,
  TextBufferLanguage,
  TextBufferRole,
} from "$lib/project/workspace-contract";

type KernelDiskConflictStatus = "clean" | "info" | "warning" | "error";

type KernelDiskConflictKind =
  | "clean"
  | "dirty_only"
  | "metadata_changed"
  | "disk_changed"
  | "missing_on_disk"
  | "readonly"
  | "not_file"
  | "oversized"
  | "unreadable"
  | "invalid_path";

type KernelDiskConflictSummary = {
  status: KernelDiskConflictStatus;
  verdictDiagnostic: LocalizedDiagnostic;
  trackedFileCount: number;
  cleanCount: number;
  dirtyOnlyCount: number;
  metadataChangedCount: number;
  diskChangedCount: number;
  missingOnDiskCount: number;
  readonlyCount: number;
  notFileCount: number;
  oversizedCount: number;
  unreadableCount: number;
  invalidPathCount: number;
  conflictCount: number;
  blockingCount: number;
};

export type KernelDiskConflictFileSnapshot = {
  relativePath: string;
  absolutePath: string;
  language: TextBufferLanguage;
  role: TextBufferRole;
  status: KernelDiskConflictStatus;
  kind: KernelDiskConflictKind;
  diagnostic: LocalizedDiagnostic;
  baseline: FileBufferBaseline;
  disk: FileBufferBaseline | null;
  hasDraft: boolean;
  dirty: boolean;
  revision: number;
};

export type KernelDiskConflictSnapshot = {
  schemaVersion: number;
  sessionId: string;
  projectRoot: string;
  scannedAtMs: number;
  maxFileBytes: number;
  summary: KernelDiskConflictSummary;
  files: KernelDiskConflictFileSnapshot[];
};

type KernelExternalDiskReconcileStatus =
  | "applied"
  | "noop"
  | "blocked"
  | "reload_required"
  | "stale_evidence";

type KernelExternalDiskReconcileItemOutcome =
  | "content_rebased"
  | "metadata_refreshed"
  | "unchanged"
  | "blocked"
  | "reload_required"
  | "stale_evidence";

export type KernelExternalDiskReconcileInput = {
  expectedProjectRoot: string;
  expectedSessionId: string;
  observedManifest: ProjectDiskManifest;
  relativePaths: string[];
  activeRelativePath?: string | null;
};

type KernelExternalDiskReconcileItemReceipt = {
  relativePath: string;
  outcome: KernelExternalDiskReconcileItemOutcome;
  beforeRevision: number | null;
  afterRevision: number | null;
  beforeBaseline: FileBufferBaseline | null;
  observedDiskBaseline: FileBufferBaseline | null;
  beforeCurrentHash: string | null;
  afterCurrentHash: string | null;
  diagnostic: string | null;
};

type KernelExternalDiskReconcileDiagnostic = {
  code: string;
  relativePath: string | null;
  messageDiagnostic: LocalizedDiagnostic;
  blocking: boolean;
};

type KernelExternalDiskProjectionHints = {
  projectRescan: boolean;
  sourceGraph: boolean;
  preview: boolean;
  pageJs: boolean;
  scss: boolean;
  history: boolean;
  selection: boolean;
};

export type KernelExternalDiskReconcileReceipt = {
  schemaVersion: number;
  operationId: string;
  sessionId: string;
  projectRoot: string;
  status: KernelExternalDiskReconcileStatus;
  verdictReason: string;
  startedAtMs: number;
  completedAtMs: number;
  requestedCount: number;
  targetCount: number;
  reconciledCount: number;
  metadataRefreshedCount: number;
  unchangedCount: number;
  totalBytesRead: number;
  requestedPaths: string[];
  effectivePaths: string[];
  invalidatedPaths: string[];
  blockedPaths: string[];
  reloadRequiredPaths: string[];
  historyInvalidated: boolean;
  sourceGraphInvalidated: boolean;
  activeFile: FileBufferTextSnapshot | null;
  acceptedDiskGeneration: number | null;
  workspaceRevision: number | null;
  acceptedManifest: ProjectDiskManifest | null;
  projectionHints: KernelExternalDiskProjectionHints;
  items: KernelExternalDiskReconcileItemReceipt[];
  diagnostics: KernelExternalDiskReconcileDiagnostic[];
};

type ProjectDiskManifestEntry = {
  relativePath: string;
  modifiedMs: number;
  size: number;
  versionToken?: string;
};

export type ProjectDiskManifest = {
  root: string;
  files: ProjectDiskManifestEntry[];
  truncated: boolean;
  maxFiles: number;
};

export type ExternalDiskState = {
  baseline: ProjectDiskManifest | null;
  reconciling: boolean;
  changed: boolean;
  changedFiles: string[];
  activeFileChanged: boolean;
  previewRelevantChanged: boolean;
  blockedByDirtySession: boolean;
  lastDetectedAt: number | null;
  lastDetectedFiles: string[];
  lastDetectedActiveFileChanged: boolean;
  lastDetectedPreviewRelevantChanged: boolean;
  lastAppliedAt: number | null;
  lastAppliedFiles: string[];
  lastCheckedAt: number | null;
  checking: boolean;
  workspaceProjectionRecoveryRequired: boolean;
  truncated: boolean;
};
