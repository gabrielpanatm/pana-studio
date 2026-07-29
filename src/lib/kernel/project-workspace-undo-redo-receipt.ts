import {
  PROJECT_WORKSPACE_SCHEMA_VERSION,
  PROJECT_WORKSPACE_UNDO_REDO_COMMAND_SCHEMA_VERSION,
  type ProjectWorkspaceUndoRedoCommandReceipt,
  type WorkspaceHistoryDirection,
} from "$lib/types";
import { t } from "$lib/i18n/runtime.svelte";
import { requireWorkbenchReceipt } from "$lib/workbench/io";

export type ProjectWorkspaceUndoRedoReceiptExpectation = {
  projectRoot: string;
  runtimeSessionId: string;
  direction: WorkspaceHistoryDirection;
  revisionBefore: number;
  transactionId: string;
};

export function requireProjectWorkspaceUndoRedoCommandReceipt(
  receipt: ProjectWorkspaceUndoRedoCommandReceipt,
  expected: ProjectWorkspaceUndoRedoReceiptExpectation,
): ProjectWorkspaceUndoRedoCommandReceipt {
  if (receipt.schemaVersion !== PROJECT_WORKSPACE_UNDO_REDO_COMMAND_SCHEMA_VERSION) {
    throw new Error(
      t("history-receipt-command-schema", {
        actual: receipt.schemaVersion,
        expected: PROJECT_WORKSPACE_UNDO_REDO_COMMAND_SCHEMA_VERSION,
      }),
    );
  }
  if (receipt.workspace.schemaVersion !== PROJECT_WORKSPACE_SCHEMA_VERSION) {
    throw new Error(
      t("history-receipt-workspace-schema", {
        actual: receipt.workspace.schemaVersion,
        expected: PROJECT_WORKSPACE_SCHEMA_VERSION,
      }),
    );
  }
  if (receipt.result.schemaVersion !== PROJECT_WORKSPACE_SCHEMA_VERSION) {
    throw new Error(
      t("history-receipt-result-schema", {
        actual: receipt.result.schemaVersion,
        expected: PROJECT_WORKSPACE_SCHEMA_VERSION,
      }),
    );
  }
  if (
    receipt.projectRoot !== expected.projectRoot
    || receipt.workspace.projectRoot !== expected.projectRoot
  ) {
    throw new Error(t("history-receipt-project-mismatch"));
  }
  if (
    receipt.runtimeSessionId !== expected.runtimeSessionId
    || receipt.workspace.runtimeSessionId !== expected.runtimeSessionId
  ) {
    throw new Error(t("history-receipt-session-mismatch"));
  }
  if (receipt.result.direction !== expected.direction) {
    throw new Error(
      t("history-receipt-direction-mismatch", {
        actual: receipt.result.direction,
        expected: expected.direction,
      }),
    );
  }
  if (receipt.result.revisionBefore !== expected.revisionBefore) {
    throw new Error(
      t("history-receipt-start-revision-mismatch", {
        actual: receipt.result.revisionBefore,
        expected: expected.revisionBefore,
      }),
    );
  }
  if (receipt.result.revisionAfter !== expected.revisionBefore + 1) {
    throw new Error(
      t("history-receipt-next-revision-mismatch", {
        actual: receipt.result.revisionAfter,
        expected: expected.revisionBefore + 1,
      }),
    );
  }
  if (receipt.workspace.revision !== receipt.result.revisionAfter) {
    throw new Error(
      t("history-receipt-snapshot-revision-mismatch", {
        actual: receipt.workspace.revision,
        expected: receipt.result.revisionAfter,
      }),
    );
  }
  const entry = receipt.result.entry;
  if (entry.transactionId !== expected.transactionId) {
    throw new Error(
      t("history-receipt-transaction-mismatch", {
        actual: entry.transactionId,
        expected: expected.transactionId,
      }),
    );
  }
  if (
    !Array.isArray(entry.documentPaths)
    || !entry.documentPaths.every((path) => typeof path === "string" && path.length > 0)
    || !Array.isArray(entry.topologyPaths)
    || !entry.topologyPaths.every((path) => typeof path === "string" && path.length > 0)
  ) {
    throw new Error(t("history-receipt-topology-manifest-invalid"));
  }
  const documentPaths = new Set(entry.documentPaths);
  if (!entry.topologyPaths.every((path) => documentPaths.has(path))) {
    throw new Error(
      t("history-receipt-topology-outside-transaction"),
    );
  }
  if (!Array.isArray(receipt.result.documents)) {
    throw new Error(t("history-receipt-documents-missing"));
  }
  if (receipt.workbench) {
    if (
      receipt.workbench.projectRoot !== expected.projectRoot
      || receipt.workbench.runtimeSessionId !== expected.runtimeSessionId
    ) {
      throw new Error(t("workbench-invalid-receipt"));
    }
    requireWorkbenchReceipt(receipt.workbench, {
      expectedProjectRoot: receipt.workbench.projectRoot,
      expectedRuntimeSessionId: receipt.workbench.runtimeSessionId,
      expectedRevision: receipt.workbench.revisionBefore,
    });
  }
  const projectedPaths = new Set<string>();
  for (const projection of receipt.result.documents) {
    if (
      !projection
      || typeof projection.relativePath !== "string"
      || projection.relativePath.length === 0
      || projectedPaths.has(projection.relativePath)
      || !documentPaths.has(projection.relativePath)
    ) {
      throw new Error(t("history-receipt-document-invalid"));
    }
    projectedPaths.add(projection.relativePath);
    const snapshot = projection.snapshot;
    if (snapshot === null) continue;
    if (
      !snapshot
      || snapshot.relativePath !== projection.relativePath
      || typeof snapshot.text !== "string"
      || typeof snapshot.dirty !== "boolean"
      || typeof snapshot.hash !== "string"
      || typeof snapshot.bytes !== "number"
      || !Number.isSafeInteger(snapshot.revision)
      || snapshot.revision < 0
    ) {
      throw new Error(
        t("history-receipt-file-buffer-invalid", {
          path: projection.relativePath,
        }),
      );
    }
  }
  return receipt;
}
