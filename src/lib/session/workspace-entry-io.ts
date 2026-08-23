import type {
  FileBufferRequestIdentity,
  WorkspaceEntryMutationReceipt,
} from "$lib/project/workspace-contract";
import { invoke } from "@tauri-apps/api/core";
import { t } from "$lib/i18n/runtime.svelte";

/** @internal Shared identity validation for session-bound domain IO. */
export function requireProjectFileRequestIdentity(identity: FileBufferRequestIdentity) {
  if (!identity.expectedProjectRoot.trim() || !identity.expectedSessionId.trim()) {
    throw new Error(
      t("io-project-file-identity-invalid"),
    );
  }
}

/** @internal Shared receipt validation for session-bound domain IO. */
export function requireProjectFileReceiptIdentity(
  receipt: { projectRoot: string; runtimeSessionId: string },
  identity: FileBufferRequestIdentity,
  operation: string,
) {
  if (
    receipt.projectRoot !== identity.expectedProjectRoot
    || receipt.runtimeSessionId !== identity.expectedSessionId
  ) {
    throw new Error(
      t("io-project-file-stale-receipt", {
        operation,
        expectedRoot: identity.expectedProjectRoot,
        expectedSession: identity.expectedSessionId,
        actualRoot: receipt.projectRoot,
        actualSession: receipt.runtimeSessionId,
      }),
    );
  }
}

/** @internal Shared Tauri boundary for workspace-entry commands. */
export async function invokeWorkspaceEntryMutation(
  command: string,
  args: Record<string, unknown>,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  requireProjectFileRequestIdentity(identity);
  const receipt = await invoke<WorkspaceEntryMutationReceipt>(command, args);
  requireProjectFileReceiptIdentity(receipt, identity, command);
  return receipt;
}
