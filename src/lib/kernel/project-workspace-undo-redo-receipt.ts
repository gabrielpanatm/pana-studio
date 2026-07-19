import {
  PROJECT_WORKSPACE_SCHEMA_VERSION,
  PROJECT_WORKSPACE_UNDO_REDO_COMMAND_SCHEMA_VERSION,
  type ProjectWorkspaceUndoRedoCommandReceipt,
  type WorkspaceHistoryDirection,
} from "$lib/types";

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
      `Receipt-ul Undo/Redo are schema comenzii ${receipt.schemaVersion}; era necesară ${PROJECT_WORKSPACE_UNDO_REDO_COMMAND_SCHEMA_VERSION}.`,
    );
  }
  if (receipt.workspace.schemaVersion !== PROJECT_WORKSPACE_SCHEMA_VERSION) {
    throw new Error(
      `Snapshot-ul ProjectWorkspace din receipt-ul Undo/Redo are schema ${receipt.workspace.schemaVersion}; era necesară ${PROJECT_WORKSPACE_SCHEMA_VERSION}.`,
    );
  }
  if (receipt.result.schemaVersion !== PROJECT_WORKSPACE_SCHEMA_VERSION) {
    throw new Error(
      `Rezultatul Undo/Redo are schema ProjectWorkspace ${receipt.result.schemaVersion}; era necesară ${PROJECT_WORKSPACE_SCHEMA_VERSION}.`,
    );
  }
  if (
    receipt.projectRoot !== expected.projectRoot
    || receipt.workspace.projectRoot !== expected.projectRoot
  ) {
    throw new Error("Receipt-ul Undo/Redo aparține altui proiect decât cel rezervat.");
  }
  if (
    receipt.runtimeSessionId !== expected.runtimeSessionId
    || receipt.workspace.runtimeSessionId !== expected.runtimeSessionId
  ) {
    throw new Error("Receipt-ul Undo/Redo aparține altei instanțe ProjectSession.");
  }
  if (receipt.result.direction !== expected.direction) {
    throw new Error(
      `Receipt-ul Undo/Redo confirmă direcția ${receipt.result.direction}, nu ${expected.direction}.`,
    );
  }
  if (receipt.result.revisionBefore !== expected.revisionBefore) {
    throw new Error(
      `Receipt-ul Undo/Redo pornește de la revizia ${receipt.result.revisionBefore}, nu de la revizia rezervată ${expected.revisionBefore}.`,
    );
  }
  if (receipt.result.revisionAfter !== expected.revisionBefore + 1) {
    throw new Error(
      `Receipt-ul Undo/Redo a publicat revizia ${receipt.result.revisionAfter}; următoarea revizie trebuia să fie ${expected.revisionBefore + 1}.`,
    );
  }
  if (receipt.workspace.revision !== receipt.result.revisionAfter) {
    throw new Error(
      `Snapshot-ul Undo/Redo este la revizia ${receipt.workspace.revision}, dar rezultatul confirmă revizia ${receipt.result.revisionAfter}.`,
    );
  }
  const entry = receipt.result.entry;
  if (entry.transactionId !== expected.transactionId) {
    throw new Error(
      `Receipt-ul Undo/Redo confirmă tranzacția ${entry.transactionId}, `
        + `nu ținta rezervată ${expected.transactionId}.`,
    );
  }
  if (
    !Array.isArray(entry.documentPaths)
    || !entry.documentPaths.every((path) => typeof path === "string" && path.length > 0)
    || !Array.isArray(entry.topologyPaths)
    || !entry.topologyPaths.every((path) => typeof path === "string" && path.length > 0)
  ) {
    throw new Error("Receipt-ul Undo/Redo nu conține un manifest valid al topologiei tranzacției.");
  }
  const documentPaths = new Set(entry.documentPaths);
  if (!entry.topologyPaths.every((path) => documentPaths.has(path))) {
    throw new Error(
      "Receipt-ul Undo/Redo declară o schimbare de topologie în afara resurselor tranzacției.",
    );
  }
  if (!Array.isArray(receipt.result.documents)) {
    throw new Error("Receipt-ul Undo/Redo nu conține proiecția documentelor tranzacției.");
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
      throw new Error("Receipt-ul Undo/Redo conține o proiecție de document invalidă sau duplicată.");
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
        `Receipt-ul Undo/Redo conține un snapshot FileBuffer invalid pentru ${projection.relativePath}.`,
      );
    }
  }
  return receipt;
}
