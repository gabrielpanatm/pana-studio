import type { ProjectFileKind } from "$lib/project/lifecycle-contract";
import type { WorkspaceEntryMutationReceipt } from "$lib/project/workspace-contract";
import type {
  WorkbenchCommandReceipt,
  WorkbenchSurface,
} from "$lib/workbench/contracts";

export const FILE_EXPLORER_SCHEMA_VERSION = 1 as const;

type FileExplorerEntryKind = "directory" | "text" | "binary";

type FileExplorerRole = "page" | "template" | "style" | "script" | "asset";

type FileExplorerCapabilityReason =
  | "not_document"
  | "binary_editor_unavailable"
  | "binary_mutation_unavailable"
  | "directory_mutation_unavailable"
  | "root_entry"
  | "edit_authority_unavailable";

type FileExplorerCapability = {
  allowed: boolean;
  reason: FileExplorerCapabilityReason | null;
};

type FileExplorerCapabilities = {
  open: FileExplorerCapability;
  createChild: FileExplorerCapability;
  rename: FileExplorerCapability;
  moveEntry: FileExplorerCapability;
  delete: FileExplorerCapability;
};

export type FileExplorerEntry = {
  id: string;
  parentId: string | null;
  name: string;
  relativePath: string;
  absolutePath: string;
  fileKind: ProjectFileKind;
  depth: number;
  kind: FileExplorerEntryKind;
  role: FileExplorerRole;
  previewPath: string | null;
  openSurface: WorkbenchSurface | null;
  capabilities: FileExplorerCapabilities;
};

type FileExplorerSelection = {
  entryId: string;
  relativePath: string;
  kind: FileExplorerEntryKind;
};

export type FileExplorerSnapshot = {
  schemaVersion: typeof FILE_EXPLORER_SCHEMA_VERSION;
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number;
  acceptedDiskGeneration: number;
  workbenchRevision: number;
  selectionRevision: number;
  selectedEntry: FileExplorerSelection | null;
  activeDocumentPath: string | null;
  entries: FileExplorerEntry[];
  rootCapabilities: FileExplorerCapabilities;
  truncated: boolean;
  maxEntries: number;
  diagnostics: string[];
};

export type FileExplorerSelectionReceipt = {
  schemaVersion: typeof FILE_EXPLORER_SCHEMA_VERSION;
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number;
  workbench: WorkbenchCommandReceipt;
  snapshot: FileExplorerSnapshot;
};

type FileExplorerOperationReason =
  | "invalid_name"
  | "missing_source"
  | "missing_target"
  | "target_not_directory"
  | "same_parent"
  | "descendant_target"
  | "destination_conflict"
  | "protected_path"
  | "unsupported_entry_kind"
  | "truncated_snapshot"
  | "edit_authority_unavailable";

export type FileExplorerOperationRequest =
  | {
      kind: "create";
      parentEntryId: string | null;
      entryKind: FileExplorerEntryKind;
      name: string;
    }
  | { kind: "rename"; entryId: string; newName: string }
  | {
      kind: "move";
      entryId: string;
      targetDirectoryEntryId: string | null;
    }
  | { kind: "delete"; entryId: string };

export type FileExplorerOperationPlan = {
  schemaVersion: typeof FILE_EXPLORER_SCHEMA_VERSION;
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number;
  acceptedDiskGeneration: number;
  allowed: boolean;
  reason: FileExplorerOperationReason | null;
  diagnostic: string | null;
  commitToken: string | null;
  destinationPath: string | null;
  affectedEntryIds: string[];
  affectedPaths: string[];
};

export type FileExplorerCommitReceipt = {
  schemaVersion: typeof FILE_EXPLORER_SCHEMA_VERSION;
  projectRoot: string;
  runtimeSessionId: string;
  mutation: WorkspaceEntryMutationReceipt;
  workbench: WorkbenchCommandReceipt;
  snapshot: FileExplorerSnapshot;
};
