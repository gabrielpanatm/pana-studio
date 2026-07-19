import type { CanvasProjectionIdentity } from "$lib/project/io";

export const INTERACTIVE_PREVIEW_MESSAGE_SOURCE = "pana-studio-interactive";
export const INTERACTIVE_PREVIEW_SCHEMA_VERSION = 1;
const MAX_INTERACTIVE_DOM_NODES = 5000;

export type InteractivePreviewDomNode = {
  tag: string;
  id: string | null;
  classes: string[];
  sourceId: string | null;
  renderInstanceId: string | null;
  depth: number;
  text: string;
};

export type InteractivePreviewMessage =
  | {
      type: "ready";
      previewRevision: string;
      nodeCount: number;
    }
  | {
      type: "dom-snapshot";
      previewRevision: string;
      reason: string;
      truncated: boolean;
      nodes: InteractivePreviewDomNode[];
    }
  | {
      type: "lifecycle-error";
      previewRevision: string;
      componentId: string;
      phase: string;
      message: string;
    };

export function buildInteractivePreviewUrl(
  previewSrc: string,
  identity: CanvasProjectionIdentity,
) {
  if (
    !previewSrc
    || previewSrc === "about:blank"
    || !identity.previewRevision.trim()
    || !identity.transactionId.trim()
  ) return "";
  try {
    const url = new URL(previewSrc);
    if (url.protocol !== "http:" || !["127.0.0.1", "localhost"].includes(url.hostname)) {
      return "";
    }
    for (const key of [
      "__pana_plain",
      "__pana_view",
      "__pana_reload",
      "__pana_interactive",
      "__pana_interactive_restart",
    ]) {
      url.searchParams.delete(key);
    }
    url.searchParams.set("__pana_view", "interactive");
    url.searchParams.set("__pana_preview_revision", identity.previewRevision);
    url.searchParams.set("__pana_canvas_transaction", identity.transactionId);
    return url.toString();
  } catch {
    return "";
  }
}

export function parseInteractivePreviewMessage(
  frame: HTMLIFrameElement | null | undefined,
  event: MessageEvent,
  expectedPreviewRevision: string,
): InteractivePreviewMessage | null {
  if (!frame?.contentWindow || event.source !== frame.contentWindow) return null;
  const data = event.data;
  if (
    !data
    || typeof data !== "object"
    || data.source !== INTERACTIVE_PREVIEW_MESSAGE_SOURCE
    || data.schemaVersion !== INTERACTIVE_PREVIEW_SCHEMA_VERSION
    || data.previewRevision !== expectedPreviewRevision
  ) return null;

  if (data.type === "ready") {
    return {
      type: "ready",
      previewRevision: expectedPreviewRevision,
      nodeCount: boundedInteger(data.nodeCount, 0, MAX_INTERACTIVE_DOM_NODES * 20),
    };
  }
  if (data.type === "dom-snapshot" && Array.isArray(data.nodes)) {
    const nodes = data.nodes
      .slice(0, MAX_INTERACTIVE_DOM_NODES)
      .map((node: unknown) => parseDomNode(node))
      .filter((node: InteractivePreviewDomNode | null): node is InteractivePreviewDomNode => node !== null);
    return {
      type: "dom-snapshot",
      previewRevision: expectedPreviewRevision,
      reason: boundedString(data.reason, 64),
      truncated: data.truncated === true || data.nodes.length > MAX_INTERACTIVE_DOM_NODES,
      nodes,
    };
  }
  if (data.type === "lifecycle-error") {
    return {
      type: "lifecycle-error",
      previewRevision: expectedPreviewRevision,
      componentId: boundedString(data.componentId, 128),
      phase: boundedString(data.phase, 32),
      message: boundedString(data.message, 1024),
    };
  }
  return null;
}

function parseDomNode(value: unknown): InteractivePreviewDomNode | null {
  if (!value || typeof value !== "object") return null;
  const node = value as Record<string, unknown>;
  const tag = boundedString(node.tag, 64).toLowerCase();
  if (!tag) return null;
  return {
    tag,
    id: nullableBoundedString(node.id, 256),
    classes: Array.isArray(node.classes)
      ? node.classes.slice(0, 32).map((entry) => boundedString(entry, 128)).filter(Boolean)
      : [],
    sourceId: nullableBoundedString(node.sourceId, 256),
    renderInstanceId: nullableBoundedString(node.renderInstanceId, 256),
    depth: boundedInteger(node.depth, 0, 64),
    text: boundedString(node.text, 160),
  };
}

function boundedString(value: unknown, maxLength: number) {
  return typeof value === "string" ? value.slice(0, maxLength) : "";
}

function nullableBoundedString(value: unknown, maxLength: number) {
  const bounded = boundedString(value, maxLength);
  return bounded || null;
}

function boundedInteger(value: unknown, minimum: number, maximum: number) {
  return typeof value === "number" && Number.isSafeInteger(value)
    ? Math.min(maximum, Math.max(minimum, value))
    : minimum;
}
