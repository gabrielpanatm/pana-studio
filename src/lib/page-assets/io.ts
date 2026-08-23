import type {
  PageAssetContractApplyInput,
  PageAssetContractApplyReceipt,
} from "$lib/page-contracts/contracts";
import type {
  FileBufferRequestIdentity,
  WorkspaceEntryMutationReceipt,
} from "$lib/project/workspace-contract";
import { invoke } from "@tauri-apps/api/core";
import {
  open as openDialog,
} from "@tauri-apps/plugin-dialog";
import { t } from "$lib/i18n/runtime.svelte";
import { invokeWorkspaceEntryMutation } from "$lib/session/workspace-entry-io";

export function applyPageAssetContract(
  input: PageAssetContractApplyInput,
): Promise<PageAssetContractApplyReceipt> {
  return invoke<PageAssetContractApplyReceipt>("apply_page_asset_contract", { input });
}

export function importProjectAsset(
  sourcePath: string,
  destinationDirectory: string,
  fileName: string,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "import_project_asset",
    { sourcePath, destinationDirectory, fileName, identity },
    identity,
  );
}

export async function chooseAssetFile(): Promise<string | null> {
  const selected = await openDialog({
    directory: false,
    multiple: false,
    title: t("io-dialog-import-asset"),
  });
  if (!selected || Array.isArray(selected)) return null;
  return selected;
}
