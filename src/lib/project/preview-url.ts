import type {
  CanvasProjectionIdentity,
} from "$lib/contracts/canvas-projection";

export function bindCanvasCandidateIdentityToPreviewUrl(
  url: string,
  identity: CanvasProjectionIdentity,
) {
  const candidateUrl = new URL(url);
  candidateUrl.searchParams.set("__pana_preview_revision", identity.previewRevision);
  candidateUrl.searchParams.set("__pana_canvas_transaction", identity.transactionId);
  return candidateUrl.toString();
}
