import type { ScssVariable } from "$lib/types";

export type InspectorCssDraft = {
  selector: string;
  properties: Record<string, string>;
  viewport: "desktop" | "tablet" | "mobile";
};

export type InspectorLiveCssIdentity = Readonly<{
  projectRoot: string;
  runtimeSessionId: string;
  epoch: number;
  workspaceRevision: number | null;
  workspaceTransactionId: string | null;
  canvasTransactionId: string | null;
  previewRevision: string | null;
}>;

export type PreviewLiveControllerHost = {
  scssVariables: ScssVariable[];
  previewDevice: "desktop" | "tablet" | "mobile";
  liveCssById: Record<string, string>;
  inspectorLiveCssEpoch: number;
  inspectorLiveCssIdentity: InspectorLiveCssIdentity | null;
  sessionProjectRoot: string;
  kernelProjectSessionId: string;
  getPreviewDocument: () => Document | undefined;
  postPreviewMessage: (payload: Record<string, unknown>) => void;
  markPreviewLive?: (message?: string) => void;
};

export const INSPECTOR_LIVE_STYLE_ID = "pana-inspector-live-overrides";

function inspectorDraftIdentity(
  host: PreviewLiveControllerHost,
  epoch: number,
): InspectorLiveCssIdentity {
  return {
    projectRoot: host.sessionProjectRoot,
    runtimeSessionId: host.kernelProjectSessionId,
    epoch,
    workspaceRevision: null,
    workspaceTransactionId: null,
    canvasTransactionId: null,
    previewRevision: null,
  };
}

function sameInspectorLiveCssIdentity(
  left: InspectorLiveCssIdentity | null,
  right: InspectorLiveCssIdentity | null,
) {
  return Boolean(
    left
    && right
    && left.projectRoot === right.projectRoot
    && left.runtimeSessionId === right.runtimeSessionId
    && left.epoch === right.epoch
    && left.workspaceRevision === right.workspaceRevision
    && left.workspaceTransactionId === right.workspaceTransactionId
    && left.canvasTransactionId === right.canvasTransactionId
    && left.previewRevision === right.previewRevision,
  );
}

export function breakpointValue(host: PreviewLiveControllerHost, name: string, fallback: string) {
  return host.scssVariables.find((variable) => variable.name === name)?.value || fallback;
}

function resolveScssTokenForLiveCss(host: PreviewLiveControllerHost, value: string) {
  const trimmed = value.trim();
  if (!trimmed) return trimmed;
  if (trimmed.startsWith("var(") || trimmed.startsWith("--")) return trimmed;
  const variableValue = (name: string) => host.scssVariables.find((entry) => entry.name === name)?.value ?? null;
  const replaced = trimmed.replace(/\$([A-Za-z_][A-Za-z0-9_-]*)/g, (match, name: string) =>
    variableValue(name) ?? match,
  );
  if (replaced !== trimmed) return replaced;
  if (!/^[A-Za-z_][A-Za-z0-9_-]*$/.test(trimmed)) return trimmed;
  return variableValue(trimmed) ?? trimmed;
}

function cssDeclarationForLive(host: PreviewLiveControllerHost, property: string, value: string) {
  // This style is injected into an arbitrary project document and must temporarily
  // outrank the project's author CSS until the Rust-backed commit is confirmed.
  return `${property}: ${resolveScssTokenForLiveCss(host, value)} !important;`;
}

export function applyInspectorLiveProperties(
  host: PreviewLiveControllerHost,
  selector: string | null,
  properties: Record<string, string>,
  viewport: "desktop" | "tablet" | "mobile" = host.previewDevice,
) {
  const activeProperties = Object.entries(properties).filter(([, value]) => value.trim() !== "");
  let css = selector && activeProperties.length > 0
    ? `${selector} { ${activeProperties.map(([property, value]) => cssDeclarationForLive(host, property, value)).join(" ")} }`
    : "";

  if (css && viewport === "tablet") {
    css = `@media (max-width: ${breakpointValue(host, "bp-tableta", "1024px")}) { ${css} }`;
  } else if (css && viewport === "mobile") {
    css = `@media (max-width: ${breakpointValue(host, "bp-mobil", "768px")}) { ${css} }`;
  }

  host.inspectorLiveCssEpoch = nextInspectorLiveCssEpoch(host.inspectorLiveCssEpoch);
  host.inspectorLiveCssIdentity = inspectorDraftIdentity(host, host.inspectorLiveCssEpoch);
  injectRawCss(host, INSPECTOR_LIVE_STYLE_ID, css);
  return host.inspectorLiveCssEpoch;
}

export function applyInspectorLivePropertyDrafts(
  host: PreviewLiveControllerHost,
  entries: InspectorCssDraft[],
) {
  const blocks = entries.flatMap((entry) => {
    const activeProperties = Object.entries(entry.properties).filter(([, value]) => value.trim() !== "");
    if (!entry.selector || activeProperties.length === 0) return [];

    const rule = `${entry.selector} { ${activeProperties.map(([property, value]) => cssDeclarationForLive(host, property, value)).join(" ")} }`;
    if (entry.viewport === "tablet") {
      return [`@media (max-width: ${breakpointValue(host, "bp-tableta", "1024px")}) { ${rule} }`];
    }
    if (entry.viewport === "mobile") {
      return [`@media (max-width: ${breakpointValue(host, "bp-mobil", "768px")}) { ${rule} }`];
    }
    return [rule];
  });

  host.inspectorLiveCssEpoch = nextInspectorLiveCssEpoch(host.inspectorLiveCssEpoch);
  host.inspectorLiveCssIdentity = inspectorDraftIdentity(host, host.inspectorLiveCssEpoch);
  injectRawCss(host, INSPECTOR_LIVE_STYLE_ID, blocks.join("\n"));
  return host.inspectorLiveCssEpoch;
}

/**
 * Binds an optimistic Inspector layer to the exact Canvas plan emitted by
 * Rust. The binding happens before the browser commit, so every visible live
 * layer is either an uncommitted draft or belongs to one exact transaction.
 */
export function bindInspectorLiveCssTransaction(
  host: PreviewLiveControllerHost,
  expectedDraft: InspectorLiveCssIdentity,
  transaction: {
    workspaceRevision: number;
    workspaceTransactionId: string;
    canvasTransactionId: string;
    previewRevision: string;
  },
): InspectorLiveCssIdentity | null {
  if (
    !sameInspectorLiveCssIdentity(host.inspectorLiveCssIdentity, expectedDraft)
    || expectedDraft.workspaceRevision !== null
    || expectedDraft.workspaceTransactionId !== null
    || expectedDraft.canvasTransactionId !== null
    || expectedDraft.previewRevision !== null
    || !Number.isSafeInteger(transaction.workspaceRevision)
    || transaction.workspaceRevision < 0
    || !transaction.workspaceTransactionId.trim()
    || !transaction.canvasTransactionId.trim()
    || !transaction.previewRevision.trim()
  ) {
    return null;
  }

  const bound: InspectorLiveCssIdentity = {
    ...expectedDraft,
    workspaceRevision: transaction.workspaceRevision,
    workspaceTransactionId: transaction.workspaceTransactionId,
    canvasTransactionId: transaction.canvasTransactionId,
    previewRevision: transaction.previewRevision,
  };
  host.inspectorLiveCssIdentity = bound;
  return bound;
}

/**
 * Removes only the Inspector's optimistic CSS layer. When an expected
 * identity is supplied, all root/session/revision/transaction fields must
 * match; an ACK from an older or foreign Canvas can never erase a newer edit.
 */
export function clearInspectorLiveProperties(
  host: PreviewLiveControllerHost,
  expectedIdentity?: InspectorLiveCssIdentity,
) {
  if (
    expectedIdentity !== undefined
    && !sameInspectorLiveCssIdentity(host.inspectorLiveCssIdentity, expectedIdentity)
  ) return false;

  host.inspectorLiveCssEpoch = nextInspectorLiveCssEpoch(host.inspectorLiveCssEpoch);
  host.inspectorLiveCssIdentity = null;
  const nextLiveCss = { ...host.liveCssById };
  delete nextLiveCss[INSPECTOR_LIVE_STYLE_ID];
  host.liveCssById = nextLiveCss;

  host.getPreviewDocument()?.getElementById(INSPECTOR_LIVE_STYLE_ID)?.remove();
  host.postPreviewMessage({
    type: "set-live-style-css",
    id: INSPECTOR_LIVE_STYLE_ID,
    css: "",
    refreshSelection: false,
  });
  return true;
}

export function captureInspectorLiveCssIdentity(
  host: PreviewLiveControllerHost,
  expectedEpoch?: number,
): InspectorLiveCssIdentity | null {
  const identity = host.inspectorLiveCssIdentity;
  if (!identity) return null;
  if (expectedEpoch !== undefined && identity.epoch !== expectedEpoch) return null;
  if (
    identity.projectRoot !== host.sessionProjectRoot
    || identity.runtimeSessionId !== host.kernelProjectSessionId
  ) return null;
  return identity;
}

export function injectRawCss(host: PreviewLiveControllerHost, id: string, css: string) {
  host.liveCssById = { ...host.liveCssById, [id]: css };
  const previewDoc = host.getPreviewDocument();
  if (previewDoc) {
    ensureStyleElement(previewDoc, id).textContent = css;
  }

  host.postPreviewMessage({
    type: "set-live-style-css",
    id,
    css,
    refreshSelection: false,
  });
  if (css.trim()) host.markPreviewLive?.("Previzualizare live CSS actualizată.");
}

export function restoreLiveCssLayersToPreview(host: PreviewLiveControllerHost) {
  const previewDoc = host.getPreviewDocument();
  for (const [id, css] of Object.entries(host.liveCssById)) {
    if (previewDoc) {
      ensureStyleElement(previewDoc, id).textContent = css;
    }

    host.postPreviewMessage({
      type: "set-live-style-css",
      id,
      css,
      refreshSelection: false,
    });
  }
}

function ensureStyleElement(document: Document, id: string): HTMLStyleElement {
  let element = document.getElementById(id) as HTMLStyleElement | null;
  if (!element) {
    element = document.createElement("style") as HTMLStyleElement;
    element.id = id;
    element.setAttribute("data-pana-internal-style", "");
    document.head.appendChild(element);
  }
  return element;
}

function nextInspectorLiveCssEpoch(current: number) {
  return Number.isSafeInteger(current) && current >= 0 && current < Number.MAX_SAFE_INTEGER
    ? current + 1
    : 1;
}
