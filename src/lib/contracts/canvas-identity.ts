import type { CanvasInteractionIdentity } from "$lib/canvas/contracts";
import type { CanvasProjectionIdentity } from "$lib/contracts/canvas-projection";

export function sameCanvasProjectionIdentity(
  left: CanvasProjectionIdentity | null | undefined,
  right: CanvasProjectionIdentity | null | undefined,
) {
  return Boolean(
    left
    && right
    && left.projectRoot === right.projectRoot
    && left.runtimeSessionId === right.runtimeSessionId
    && left.workspaceRevision === right.workspaceRevision
    && left.transactionId === right.transactionId
    && left.previewRevision === right.previewRevision,
  );
}

export function sameCanvasInteractionIdentity(
  left: CanvasInteractionIdentity | null | undefined,
  right: CanvasInteractionIdentity | null | undefined,
) {
  return Boolean(
    left
    && right
    && left.route === right.route
    && left.documentEpoch === right.documentEpoch
    && left.agentInstanceId === right.agentInstanceId
    && sameCanvasProjectionIdentity(left.canvas, right.canvas),
  );
}

export function normalizeProjectDocumentPath(path: string | null | undefined) {
  return path
    ?.trim()
    .replaceAll("\\", "/")
    .replace(/\/+/g, "/")
    .replace(/^\/+/, "")
    .replace(/^(?:\.\/)+/, "")
    ?? "";
}

export function sameProjectDocumentPath(
  left: string | null | undefined,
  right: string | null | undefined,
) {
  return normalizeProjectDocumentPath(left) === normalizeProjectDocumentPath(right);
}

export function canvasRouteFromPreviewUrl(
  previewUrl: string | null | undefined,
  fallbackRoute: string | null | undefined,
) {
  if (previewUrl && previewUrl !== "about:blank") {
    try {
      return new URL(previewUrl, "http://pana.local/").pathname || "/";
    } catch {
      // Fall through to the browser-owned route.
    }
  }
  const fallback = fallbackRoute?.trim() || "/";
  return fallback.startsWith("/") ? fallback : `/${fallback}`;
}
