import type {
  DataMutationApplyReceipt,
  DataMutationInput,
  DataNodeEditorSnapshot,
} from "$lib/data/contracts";
import type { FileBufferRequestIdentity } from "$lib/project/workspace-contract";
import { invoke } from "@tauri-apps/api/core";
import { t } from "$lib/i18n/runtime.svelte";
import { schemaMismatch } from "$lib/contracts/io-schema";
import {
  requireProjectFileRequestIdentity,
  requireProjectFileReceiptIdentity,
} from "$lib/session/workspace-entry-io";

export async function applyDataMutation(
  input: DataMutationInput,
  identity: FileBufferRequestIdentity,
): Promise<DataMutationApplyReceipt> {
  requireProjectFileRequestIdentity(identity);
  const receipt = await invoke<DataMutationApplyReceipt>("apply_data_mutation", {
    input,
    identity,
  });
  requireProjectFileReceiptIdentity(receipt.workspace, identity, "apply_data_mutation");
  if (receipt.plan.schemaVersion !== 1) {
    throw schemaMismatch(
      t("io-resource-data-plan"),
      receipt.plan.schemaVersion,
      1,
    );
  }
  return receipt;
}

export async function readDataNodeEditor(
  file: string,
  nodeId: string,
  identity: FileBufferRequestIdentity,
): Promise<DataNodeEditorSnapshot> {
  requireProjectFileRequestIdentity(identity);
  const snapshot = await invoke<DataNodeEditorSnapshot>("read_data_node_editor", {
    file,
    nodeId,
    identity,
  });
  if (snapshot.schemaVersion !== 1 || snapshot.file !== file || snapshot.nodeId !== nodeId) {
    throw new Error(t("io-data-node-selection-mismatch"));
  }
  return snapshot;
}
