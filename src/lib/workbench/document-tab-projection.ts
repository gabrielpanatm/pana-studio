import type {
  WorkbenchDocumentActivationSnapshot,
} from "$lib/workbench/contracts";

/**
 * Presents the latest user intent immediately while Rust confirms it.
 * Ready and failed activations always fall back to the authoritative snapshot.
 */
export function presentedWorkbenchDocumentId(
  authoritativeDocumentId: string | null | undefined,
  activation: WorkbenchDocumentActivationSnapshot | null | undefined,
  locallyRequestedDocumentId: string | null = null,
) {
  if (locallyRequestedDocumentId) return locallyRequestedDocumentId;
  if (
    (activation?.phase === "applying" || activation?.phase === "loading")
    && activation.documentId
  ) return activation.documentId;
  return authoritativeDocumentId ?? null;
}
