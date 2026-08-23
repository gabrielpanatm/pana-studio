import type { LocalizedDiagnostic } from "$lib/contracts/localized-diagnostic";
import type {
  PageJsDraftStageReceipt,
  PageJsDraftStoreSnapshot,
} from "$lib/js/contracts";
import type { WriteReceipt } from "$lib/kernel/recovery-contract";
import type { CanvasPatch } from "$lib/preview/contracts";
import type { ProjectDiskManifest } from "$lib/project/external-disk-contract";
import type { WorkbenchCommandReceipt } from "$lib/workbench/contracts";

export type TextBufferLanguage =
  | "html"
  | "markdown"
  | "css"
  | "scss"
  | "java_script"
  | "toml"
  | "json"
  | "yaml"
  | "plain";

export type TextBufferRole =
  | "page"
  | "template"
  | "style"
  | "script"
  | "config"
  | "data"
  | "other";

export type FileBufferBaseline = {
  hash: string;
  modifiedMs: number;
  size: number;
  readonly: boolean;
};

type FileBufferStoreLimits = {
  maxFiles: number;
  maxFileBytes: number;
  maxTotalBytes: number;
};

export type FileBufferRequestIdentity = {
  expectedProjectRoot: string;
  expectedSessionId: string;
};

export type ProjectWorkspaceIdentity = {
  expectedProjectRoot: string;
  expectedSessionId: string;
  expectedRevision: number;
};

export type ProjectWorkspaceHistoryIdentity = ProjectWorkspaceIdentity & {
  expectedTransactionId: string;
};

type WorkspaceHistoryEntrySnapshot = {
  transactionId: string;
  label: string;
  source: string;
  coalesceKey: string | null;
  createdAtMs: number;
  updatedAtMs: number;
  mutationCount: number;
  documentPaths: string[];
  topologyPaths: string[];
  pageJsPaths: string[];
  retainedBytes: number;
};

type WorkspaceHistorySnapshot = {
  undoCount: number;
  redoCount: number;
  canUndo: boolean;
  canRedo: boolean;
  retainedBytes: number;
  retainedBytesLimit: number;
  entryLimit: number;
  nextUndo: WorkspaceHistoryEntrySnapshot | null;
  nextRedo: WorkspaceHistoryEntrySnapshot | null;
  undoEntries: WorkspaceHistoryEntrySnapshot[];
  redoEntries: WorkspaceHistoryEntrySnapshot[];
};

export type FileBufferMutationExpectation = {
  expectedRevision: number;
  expectedHash: string;
};

export type FileBufferCommandReceipt<T> = {
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number;
  payload: T;
};

type FileBufferDiagnosticSeverity = "warning" | "error";

type FileBufferDiagnostic = {
  severity: FileBufferDiagnosticSeverity;
  code: string;
  relativePath: string | null;
  messageDiagnostic: LocalizedDiagnostic;
};

export type FileBufferFileSnapshot = {
  relativePath: string;
  absolutePath: string;
  language: TextBufferLanguage;
  role: TextBufferRole;
  baseline: FileBufferBaseline;
  hasDraft: boolean;
  dirty: boolean;
  currentHash: string;
  currentBytes: number;
  revision: number;
};

export type FileBufferTextSnapshot = {
  relativePath: string;
  text: string;
  dirty: boolean;
  hash: string;
  bytes: number;
  revision: number;
};

type FileBufferChangeCoordinateSpace = "utf16";

export type FileBufferTextChange = {
  from: number;
  to: number;
  insert: string;
};

export type FileBufferChangeSetInput = {
  relativePath: string;
  baseRevision?: number | null;
  baseHash?: string | null;
  coordinateSpace?: FileBufferChangeCoordinateSpace;
  source?: string | null;
  changes: FileBufferTextChange[];
};

export type FileBufferChangeSetResult = {
  relativePath: string;
  source: string | null;
  previousRevision: number;
  revision: number;
  previousHash: string;
  currentHash: string;
  changeCount: number;
  applied: boolean;
  file: FileBufferFileSnapshot;
};

type FileBufferStoreSnapshot = {
  schemaVersion: number;
  sessionId: string;
  runtimeSessionId: string;
  projectRoot: string;
  loadedAtMs: number;
  fileCount: number;
  loadedFileCount: number;
  skippedFileCount: number;
  dirtyFileCount: number;
  totalLoadedBytes: number;
  limits: FileBufferStoreLimits;
  files: FileBufferFileSnapshot[];
  diagnostics: FileBufferDiagnostic[];
};

export type ProjectWorkspaceMutationReceipt = {
  schemaVersion: number;
  changed: boolean;
  revisionBefore: number;
  revisionAfter: number;
  dirty: boolean;
  transactionId: string | null;
  touchedFiles: string[];
  documents: WorkspaceDocumentProjection[];
  entry: WorkspaceHistoryEntrySnapshot | null;
  files: FileBufferFileSnapshot[];
  pageJs: PageJsDraftStageReceipt | null;
  history: WorkspaceHistorySnapshot;
};

// Keep this in lockstep with
// src-tauri/src/kernel/project_workspace/model.rs::PROJECT_WORKSPACE_SCHEMA_VERSION.
export const PROJECT_WORKSPACE_SCHEMA_VERSION = 3;

export const PROJECT_WORKSPACE_UNDO_REDO_COMMAND_SCHEMA_VERSION = 4;

export type WorkspaceEntryMutationReceipt = {
  schemaVersion: 1;
  projectRoot: string;
  runtimeSessionId: string;
  relativePath: string | null;
  mutation: ProjectWorkspaceMutationReceipt;
  workspace: ProjectWorkspaceSnapshot;
};

export type ProjectWorkspaceSnapshot = {
  schemaVersion: number;
  projectRoot: string;
  runtimeSessionId: string;
  revision: number;
  diskGeneration: number;
  dirty: boolean;
  dirtyDocumentCount: number;
  createdDocumentCount: number;
  createdDocuments: string[];
  deletedDocumentCount: number;
  deletedDocuments: string[];
  stagedBinaryResourceCount: number;
  stagedBinaryResourceBytes: number;
  stagedBinaryResources: string[];
  deletedBinaryResourceCount: number;
  deletedBinaryResources: string[];
  dirtyPageJsCount: number;
  projectModelRevision: string | null;
  projectModelSourceRevision: number | null;
  lastProjectionTransactionId: string | null;
  documents: FileBufferStoreSnapshot;
  pageJs: PageJsDraftStoreSnapshot;
  history: WorkspaceHistorySnapshot;
};

type ProjectWorkspaceSaveStatus = "noop" | "saved";

export type ProjectWorkspaceSaveReceipt = {
  schemaVersion: number;
  transactionId: string | null;
  status: ProjectWorkspaceSaveStatus;
  projectRoot: string;
  runtimeSessionId: string;
  revisionBefore: number;
  revisionAfter: number;
  diskGenerationBefore: number;
  diskGenerationAfter: number;
  writtenFiles: string[];
  removedFiles: string[];
  writeReceipts: WriteReceipt[];
  acceptedManifest: ProjectDiskManifest;
  workspace: ProjectWorkspaceSnapshot;
};

export type WorkspaceHistoryDirection = "undo" | "redo";

type WorkspaceUndoRedoReceipt = {
  schemaVersion: number;
  direction: WorkspaceHistoryDirection;
  revisionBefore: number;
  revisionAfter: number;
  dirty: boolean;
  entry: WorkspaceHistoryEntrySnapshot;
  documents: WorkspaceDocumentProjection[];
  history: WorkspaceHistorySnapshot;
  applicationTransactionId: string;
};

export type WorkspaceDocumentProjection = {
  relativePath: string;
  snapshot: FileBufferTextSnapshot | null;
};

export type ProjectWorkspaceUndoRedoCommandReceipt = {
  schemaVersion: typeof PROJECT_WORKSPACE_UNDO_REDO_COMMAND_SCHEMA_VERSION;
  projectRoot: string;
  runtimeSessionId: string;
  result: WorkspaceUndoRedoReceipt;
  workspace: ProjectWorkspaceSnapshot;
  workbench: WorkbenchCommandReceipt | null;
  canvasPatch: CanvasPatch | null;
};
