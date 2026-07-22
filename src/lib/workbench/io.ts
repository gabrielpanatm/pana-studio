import { invoke } from "@tauri-apps/api/core";
import {
  WORKBENCH_COMMAND_SCHEMA_VERSION,
  WORKBENCH_SCHEMA_VERSION,
  type WorkbenchCommandReceipt,
  type WorkbenchIdentity,
  type WorkbenchIntent,
  type WorkbenchSnapshot,
} from "$lib/types";

export async function readWorkbenchState(): Promise<WorkbenchSnapshot | null> {
  const snapshot = await invoke<WorkbenchSnapshot | null>("read_workbench_state");
  if (snapshot) requireWorkbenchSnapshot(snapshot);
  return snapshot;
}

export async function applyWorkbenchIntent(
  identity: WorkbenchIdentity,
  intent: WorkbenchIntent,
): Promise<WorkbenchCommandReceipt> {
  const receipt = await invoke<WorkbenchCommandReceipt>("apply_workbench_intent", {
    identity,
    intent,
  });
  requireWorkbenchReceipt(receipt, identity);
  return receipt;
}

export function workbenchIdentity(snapshot: WorkbenchSnapshot): WorkbenchIdentity {
  return {
    expectedProjectRoot: snapshot.projectRoot,
    expectedRuntimeSessionId: snapshot.runtimeSessionId,
    expectedRevision: snapshot.revision,
  };
}

export function requireWorkbenchSnapshot(snapshot: WorkbenchSnapshot): void {
  if (
    snapshot.schemaVersion !== WORKBENCH_SCHEMA_VERSION
    || !snapshot.projectRoot.trim()
    || !snapshot.projectSessionId.trim()
    || !snapshot.runtimeSessionId.trim()
    || !Number.isSafeInteger(snapshot.revision)
    || snapshot.revision < 0
    || !Number.isSafeInteger(snapshot.splitRatioBasisPoints)
    || snapshot.splitRatioBasisPoints < 2_000
    || snapshot.splitRatioBasisPoints > 8_000
    || !Number.isSafeInteger(snapshot.canvasViewport?.widthPx)
    || snapshot.canvasViewport.widthPx < 320
    || snapshot.canvasViewport.widthPx > 3_840
    || !Number.isSafeInteger(snapshot.canvasViewport.zoomPercent)
    || snapshot.canvasViewport.zoomPercent < 25
    || snapshot.canvasViewport.zoomPercent > 200
    || !Array.isArray(snapshot.groups)
    || snapshot.groups.length < 1
    || snapshot.groups.length > 2
  ) {
    throw new Error("[workbench_invalid_snapshot] Rust a returnat un snapshot Workbench invalid.");
  }
}

function requireWorkbenchReceipt(
  receipt: WorkbenchCommandReceipt,
  identity: WorkbenchIdentity,
): void {
  requireWorkbenchSnapshot(receipt.snapshot);
  const expectedRevisionAfter = receipt.changed
    ? identity.expectedRevision + 1
    : identity.expectedRevision;
  if (
    receipt.schemaVersion !== WORKBENCH_COMMAND_SCHEMA_VERSION
    || receipt.projectRoot !== identity.expectedProjectRoot
    || receipt.runtimeSessionId !== identity.expectedRuntimeSessionId
    || receipt.revisionBefore !== identity.expectedRevision
    || receipt.revisionAfter !== expectedRevisionAfter
    || receipt.snapshot.projectRoot !== receipt.projectRoot
    || receipt.snapshot.runtimeSessionId !== receipt.runtimeSessionId
    || receipt.snapshot.revision !== receipt.revisionAfter
  ) {
    throw new Error("[workbench_invalid_receipt] Rust a returnat un receipt Workbench invalid sau stale.");
  }
}
