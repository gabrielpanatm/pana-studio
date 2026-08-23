import { invoke } from "@tauri-apps/api/core";
import {
  FILE_EXPLORER_SCHEMA_VERSION,
  type FileExplorerCommitReceipt,
  type FileExplorerOperationPlan,
  type FileExplorerOperationRequest,
  type FileExplorerSelectionReceipt,
  type FileExplorerSnapshot,
} from "$lib/project/file-explorer-contract";
import type { ProjectWorkspaceIdentity } from "$lib/project/workspace-contract";

export async function readFileExplorerSnapshot(
  identity: ProjectWorkspaceIdentity,
): Promise<FileExplorerSnapshot> {
  const snapshot = await invoke<FileExplorerSnapshot>("read_file_explorer_snapshot", {
    identity,
  });
  requireFileExplorerSnapshot(snapshot, identity);
  return snapshot;
}

export async function selectFileExplorerEntry(input: {
  identity: ProjectWorkspaceIdentity;
  expectedWorkbenchRevision: number;
  entryId: string;
}): Promise<FileExplorerSelectionReceipt> {
  const receipt = await invoke<FileExplorerSelectionReceipt>("select_file_explorer_entry", {
    input: {
      schemaVersion: FILE_EXPLORER_SCHEMA_VERSION,
      ...input,
    },
  });
  requireFileExplorerSnapshot(receipt.snapshot, input.identity);
  if (
    receipt.schemaVersion !== FILE_EXPLORER_SCHEMA_VERSION
    || receipt.projectRoot !== input.identity.expectedProjectRoot
    || receipt.runtimeSessionId !== input.identity.expectedSessionId
    || receipt.workspaceRevision !== input.identity.expectedRevision
    || receipt.snapshot.workbenchRevision !== receipt.workbench.revisionAfter
  ) {
    throw new Error("FileExplorer a primit un receipt de selecție din altă revizie.");
  }
  return receipt;
}

export async function planFileExplorerOperation(input: {
  identity: ProjectWorkspaceIdentity;
  expectedWorkbenchRevision: number;
  operation: FileExplorerOperationRequest;
}): Promise<FileExplorerOperationPlan> {
  const plan = await invoke<FileExplorerOperationPlan>("plan_file_explorer_operation", {
    input: {
      schemaVersion: FILE_EXPLORER_SCHEMA_VERSION,
      ...input,
    },
  });
  if (
    plan.schemaVersion !== FILE_EXPLORER_SCHEMA_VERSION
    || plan.projectRoot !== input.identity.expectedProjectRoot
    || plan.runtimeSessionId !== input.identity.expectedSessionId
    || plan.workspaceRevision !== input.identity.expectedRevision
    || !Array.isArray(plan.affectedEntryIds)
    || !Array.isArray(plan.affectedPaths)
    || (plan.allowed && !plan.commitToken)
    || (!plan.allowed && plan.commitToken !== null)
  ) {
    throw new Error("FileExplorer a primit un plan din altă identitate sau cu formă invalidă.");
  }
  return plan;
}

export async function commitFileExplorerOperation(input: {
  identity: ProjectWorkspaceIdentity;
  expectedAcceptedDiskGeneration: number;
  commitToken: string;
}): Promise<FileExplorerCommitReceipt> {
  const receipt = await invoke<FileExplorerCommitReceipt>("commit_file_explorer_operation", {
    input: {
      schemaVersion: FILE_EXPLORER_SCHEMA_VERSION,
      ...input,
    },
  });
  if (
    receipt.schemaVersion !== FILE_EXPLORER_SCHEMA_VERSION
    || receipt.projectRoot !== input.identity.expectedProjectRoot
    || receipt.runtimeSessionId !== input.identity.expectedSessionId
    || receipt.mutation.projectRoot !== receipt.projectRoot
    || receipt.mutation.runtimeSessionId !== receipt.runtimeSessionId
  ) {
    throw new Error("FileExplorer a primit un receipt de commit din altă sesiune.");
  }
  requireFileExplorerSnapshot(receipt.snapshot, {
    expectedProjectRoot: receipt.projectRoot,
    expectedSessionId: receipt.runtimeSessionId,
    expectedRevision: receipt.mutation.workspace.revision,
  });
  if (
    receipt.snapshot.workbenchRevision !== receipt.workbench.revisionAfter
    || receipt.snapshot.workspaceRevision !== receipt.mutation.workspace.revision
  ) {
    throw new Error("FileExplorer Commit nu a publicat proiecțiile aceleiași revizii.");
  }
  return receipt;
}

export function requireFileExplorerSnapshot(
  snapshot: FileExplorerSnapshot,
  identity: ProjectWorkspaceIdentity,
) {
  if (
    snapshot.schemaVersion !== FILE_EXPLORER_SCHEMA_VERSION
    || snapshot.projectRoot !== identity.expectedProjectRoot
    || snapshot.runtimeSessionId !== identity.expectedSessionId
    || snapshot.workspaceRevision !== identity.expectedRevision
    || !Number.isSafeInteger(snapshot.workbenchRevision)
    || snapshot.workbenchRevision < 0
    || !Number.isSafeInteger(snapshot.selectionRevision)
    || snapshot.selectionRevision < 0
    || !Array.isArray(snapshot.entries)
    || !Array.isArray(snapshot.diagnostics)
  ) {
    throw new Error("Snapshotul FileExplorer nu corespunde identității Rust solicitate.");
  }
  const ids = new Set<string>();
  const paths = new Set<string>();
  for (const entry of snapshot.entries) {
    if (
      !entry.id.trim()
      || !entry.relativePath.trim()
      || ids.has(entry.id)
      || paths.has(entry.relativePath)
      || !Number.isSafeInteger(entry.depth)
      || entry.depth < 0
    ) {
      throw new Error("Snapshotul FileExplorer conține o intrare invalidă sau duplicată.");
    }
    ids.add(entry.id);
    paths.add(entry.relativePath);
  }
  for (const entry of snapshot.entries) {
    if (entry.parentId !== null && !ids.has(entry.parentId)) {
      throw new Error("Snapshotul FileExplorer conține un parentId necunoscut.");
    }
  }
  if (
    snapshot.selectedEntry
    && !ids.has(snapshot.selectedEntry.entryId)
  ) {
    throw new Error("Snapshotul FileExplorer indică o selecție inexistentă.");
  }
}
