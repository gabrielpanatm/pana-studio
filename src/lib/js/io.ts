import { invoke } from "@tauri-apps/api/core";
import { t } from "$lib/i18n/runtime.svelte";
import type {
  PageJsCommandReceipt,
  PageJsDraftSessionIdentity,
  PageJsDraftStageInput,
  PageJsDraftStageReceipt,
  PageJsRequestIdentity,
  PageJsWorkspaceState,
} from "$lib/js/contracts";
import type {
  MotionPageMutationInput,
  MotionPageMutationReceipt,
} from "$lib/js/contracts";

export function getPageJsWorkspaceState(
  templatePath: string,
  identity: PageJsRequestIdentity,
): Promise<PageJsCommandReceipt<PageJsWorkspaceState>> {
  return invoke<PageJsCommandReceipt<PageJsWorkspaceState>>(
    "get_page_js_workspace_state",
    { templatePath, identity },
  );
}

export function stagePageJsDraft(
  input: PageJsDraftStageInput,
  identity: PageJsDraftSessionIdentity,
): Promise<PageJsDraftStageReceipt> {
  return invoke<PageJsDraftStageReceipt>("stage_page_js_draft", {
    input: { ...input, ...identity },
  });
}

export function clearPageJsDraft(
  templatePath: string,
  expectedRevision: number | null,
  identity: PageJsDraftSessionIdentity,
): Promise<PageJsDraftStageReceipt> {
  return invoke<PageJsDraftStageReceipt>("clear_page_js_draft", {
    templatePath,
    expectedRevision,
    ...identity,
  });
}

export async function applyMotionMutation(
  input: MotionPageMutationInput,
): Promise<MotionPageMutationReceipt> {
  const receipt = await invoke<MotionPageMutationReceipt>("apply_motion_mutation", { input });
  if (
    receipt.mutation.schemaVersion !== 3
    || (receipt.mutation.transaction && receipt.mutation.transaction.schemaVersion !== 3)
  ) {
    throw new Error(t("io-schema-mismatch", {
      resource: "Motion mutation",
      actual: receipt.mutation.schemaVersion,
      expected: 3,
    }));
  }
  return receipt;
}
