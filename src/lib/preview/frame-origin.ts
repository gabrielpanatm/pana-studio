const failedDocumentAccess = new WeakMap<object, string>();

function previewFrameSourceSignature(
  frame: Pick<HTMLIFrameElement, "getAttribute" | "hasAttribute">,
): string {
  return [
    frame.hasAttribute("srcdoc") ? "srcdoc" : "src",
    frame.getAttribute("src")?.trim() ?? "",
    frame.getAttribute("sandbox")?.trim() ?? "",
  ].join("\u0000");
}

export function previewFrameAllowsDocumentAccess(
  frame: Pick<HTMLIFrameElement, "getAttribute" | "hasAttribute">,
  baseUrl: string,
): boolean {
  if (failedDocumentAccess.get(frame) === previewFrameSourceSignature(frame)) return false;
  const sandbox = frame.getAttribute("sandbox");
  if (sandbox !== null) {
    const tokens = new Set(sandbox.toLowerCase().split(/\s+/).filter(Boolean));
    if (!tokens.has("allow-same-origin")) return false;
  }
  if (frame.hasAttribute("srcdoc")) return true;
  const source = frame.getAttribute("src")?.trim() ?? "";
  if (!source || source === "about:blank" || source.startsWith("about:blank#")) return true;

  try {
    return new URL(source, baseUrl).origin === new URL(baseUrl).origin;
  } catch {
    return false;
  }
}

export function rememberPreviewFrameDocumentAccessFailure(
  frame: Pick<HTMLIFrameElement, "getAttribute" | "hasAttribute">,
) {
  failedDocumentAccess.set(frame, previewFrameSourceSignature(frame));
}

export function resetPreviewFrameDocumentAccess(
  frame: Pick<HTMLIFrameElement, "getAttribute" | "hasAttribute">,
) {
  failedDocumentAccess.delete(frame);
}

/**
 * Shell-side half of the Design Safe message boundary.
 *
 * This proves that the browser delivered the message from the currently
 * mounted iframe WindowProxy. Document trust is supplied separately by the
 * parser-first, zero-project-script response and its exact-hash CSP; privileged
 * gesture paths are additionally gated inside that sole internal bridge.
 */
export function isMessageFromExactPreviewFrame(
  frame: Pick<HTMLIFrameElement, "contentWindow"> | null | undefined,
  event: Pick<MessageEvent, "source">,
): boolean {
  const previewWindow = frame?.contentWindow ?? null;
  return previewWindow !== null && event.source !== null && event.source === previewWindow;
}
