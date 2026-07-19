import {
  flushRegisteredEditDrafts,
  type EditFlushReason,
} from "$lib/session/edit-flush-registry";
import { flushFileBufferDraftSync } from "$lib/session/file-buffer-draft-sync";
import { flushPageJsDraftSync } from "$lib/session/page-js-draft-sync";

export type WorkspaceMutationFlushPhase = "editors" | "page-js" | "file-buffer";

/**
 * Establishes the single frontend mutation boundary used before Save,
 * history, project transitions and external-disk reconciliation.
 */
export async function flushWorkspaceMutationInputs(
  reason: EditFlushReason,
  options: {
    checkpoint?: (phase: WorkspaceMutationFlushPhase) => void;
  } = {},
) {
  await flushRegisteredEditDrafts(reason);
  options.checkpoint?.("editors");
  await flushPageJsDraftSync({ throwOnFailure: true });
  options.checkpoint?.("page-js");
  await flushFileBufferDraftSync({ throwOnFailure: true });
  options.checkpoint?.("file-buffer");
}
