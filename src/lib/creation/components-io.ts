import type {
  ComponentMutationApplyReceipt,
  ComponentMutationInput,
} from "$lib/creation/contracts";
import type { FileBufferRequestIdentity } from "$lib/project/workspace-contract";
import { invoke } from "@tauri-apps/api/core";
import { t } from "$lib/i18n/runtime.svelte";
import { schemaMismatch } from "$lib/contracts/io-schema";
import {
  requireProjectFileRequestIdentity,
  requireProjectFileReceiptIdentity,
} from "$lib/session/workspace-entry-io";

export async function applyComponentMutation(
  input: ComponentMutationInput,
  identity: FileBufferRequestIdentity,
): Promise<ComponentMutationApplyReceipt> {
  requireProjectFileRequestIdentity(identity);
  const receipt = await invoke<ComponentMutationApplyReceipt>("apply_component_mutation", {
    input,
    identity,
  });
  requireProjectFileReceiptIdentity(receipt.workspace, identity, "apply_component_mutation");
  if (receipt.plan.schemaVersion !== 2) {
    throw schemaMismatch(
      t("io-resource-component-plan"),
      receipt.plan.schemaVersion,
      2,
    );
  }
  return receipt;
}
