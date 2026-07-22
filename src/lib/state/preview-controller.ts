import { serializeOverrides } from "$lib/css/serializer";
import {
  buildPreviewStatusDocument,
  hidePreviewHtmlSelectionOverlay,
} from "$lib/preview/bridge";
import { collectDomTree } from "$lib/preview/selection";
import {
  previewFrameAllowsDocumentAccess,
  rememberPreviewFrameDocumentAccessFailure,
} from "$lib/preview/frame-origin";
import {
  acknowledgeCanvasProjectionPhase,
  readPreviewDocument,
  type CanvasProjectionIdentity,
  type CanvasProjectionPlan,
  type PreviewPhaseReceipt,
  type PreviewRuntimeEventKind,
} from "$lib/project/io";
import type {
  EditableStyles,
  PageSection,
  ProjectFile,
  SelectionInfo,
} from "$lib/types";
import {
  PreviewRuntimeTransportError,
  type PreviewRuntime,
} from "$lib/editor-runtime/preview-runtime";
import { errorMessage } from "$lib/util";

export type PreviewRefreshLeaseHost = {
  sessionProjectRoot: string;
  kernelProjectSessionId: string;
  projectSessionEpoch: number;
  previewRefreshSerial: number;
  previewDomTreeSerial: number;
};

export type PreviewControllerHost = PreviewRefreshLeaseHost & {
  previewFrame: HTMLIFrameElement | undefined;
  previewSrc: string;
  previewReloadSerial: number;
  previewWorkspaceRevision: string | null;
  pendingCanvasProjection: CanvasProjectionPlan | null;
  activeCanvasIdentity: CanvasProjectionIdentity | null;
  activeCanvasUrl: string;
  canvasProjectionConfirmation: CanvasProjectionConfirmation | null;
  previewSyncTimer: number | null;
  domTreeFetchTimer: number | null;
  previewDocumentMarkup: string | null;
  activeRenderedPreviewPageFile: ProjectFile | null;
  isActiveRenderedPreviewPage: boolean;
  selectedPreviewElement: Element | null;
  selectedElement: SelectionInfo | null;
  projectStatus: string;
  overrideRules: Record<string, EditableStyles>;
  variableOverrides: Record<string, string>;
  pageSections: PageSection[];
  previewRuntime?: PreviewRuntime;
  previewUrlForScannedFile: (file: ProjectFile) => string;
  recordCanvasProjectionRuntimeEvent?: (
    kind: PreviewRuntimeEventKind,
    identity: CanvasProjectionIdentity,
    durationMs: number,
    diagnostic: string | null,
  ) => Promise<void>;
  setPageSections?: (sections: PageSection[]) => void;
};

export type CanvasProjectionConfirmation = {
  transactionId: string;
  promise: Promise<void>;
  resolve: () => void;
  reject: (error: Error) => void;
  timeout: ReturnType<typeof globalThis.setTimeout>;
};

export type PreviewRefreshLease = Readonly<{
  projectRoot: string;
  runtimeSessionId: string;
  projectSessionEpoch: number;
  serial: number;
}>;

const PREVIEW_REVISION_ATTR = "data-pana-preview-revision";
const PREVIEW_REVISION_ATTEMPTS = 8;
const PREVIEW_REVISION_DELAYS_MS = [120, 180, 260, 360, 520, 700, 900, 1200];

export function previewReloadUrl(host: PreviewControllerHost, url: string) {
  if (url === "about:blank") return url;
  const next = new URL(url);
  next.searchParams.set("__pana_reload", String(++host.previewReloadSerial));
  return next.toString();
}

function wait(ms: number) {
  return new Promise<void>((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

function previewDocumentHasRevision(html: string, revision: string) {
  return html.includes(`${PREVIEW_REVISION_ATTR}="${revision}"`);
}

export function beginPreviewRefreshLease(host: PreviewRefreshLeaseHost): PreviewRefreshLease | null {
  const serial = ++host.previewRefreshSerial;
  const projectRoot = host.sessionProjectRoot.trim();
  const runtimeSessionId = host.kernelProjectSessionId.trim();
  if (!projectRoot || !runtimeSessionId) return null;
  return {
    projectRoot,
    runtimeSessionId,
    projectSessionEpoch: host.projectSessionEpoch,
    serial,
  };
}

export function previewRefreshLeaseMatches(
  host: PreviewRefreshLeaseHost,
  lease: PreviewRefreshLease,
) {
  return host.previewRefreshSerial === lease.serial
    && host.sessionProjectRoot === lease.projectRoot
    && host.kernelProjectSessionId === lease.runtimeSessionId
    && host.projectSessionEpoch === lease.projectSessionEpoch;
}

export function invalidatePreviewRefreshLease(host: PreviewRefreshLeaseHost) {
  host.previewRefreshSerial += 1;
}

type RenderedPreviewReady = {
  url: string;
  revision: string | null;
  html: string;
};

type InPlaceProjectionOutcome =
  | { kind: "committed" }
  | { kind: "stale" }
  | {
      kind: "unsupported";
      reason:
        | "runtime_unavailable"
        | "frame_unavailable"
        | "initial_navigation"
        | "markup_preview"
        | "route_changed"
        | "plan_missing"
        | "runtime_unresponsive";
    };

class PreviewProjectionDiagnosticError extends Error {
  readonly code: "preview_reconcile_failed" | "preview_canvas_identity_mismatch";

  constructor(
    code: "preview_reconcile_failed" | "preview_canvas_identity_mismatch",
    message: string,
  ) {
    super(`[${code}] ${message}`);
    this.name = "PreviewProjectionDiagnosticError";
    this.code = code;
  }
}

async function waitForRenderedPreviewUrl(
  host: PreviewControllerHost,
  previewPage: ProjectFile,
  lease: PreviewRefreshLease,
): Promise<RenderedPreviewReady | null> {
  const requiredRevision = host.previewWorkspaceRevision;
  let lastUrl = "";
  let lastError: unknown = null;

  for (let attempt = 0; attempt < PREVIEW_REVISION_ATTEMPTS; attempt += 1) {
    if (attempt > 0) {
      await wait(PREVIEW_REVISION_DELAYS_MS[Math.min(attempt, PREVIEW_REVISION_DELAYS_MS.length - 1)]);
      if (!previewRefreshLeaseMatches(host, lease)) return null;
    }

    lastUrl = previewReloadUrl(host, host.previewUrlForScannedFile(previewPage));
    if (requiredRevision) {
      const stagedUrl = new URL(lastUrl);
      stagedUrl.searchParams.set("__pana_preview_revision", requiredRevision);
      lastUrl = stagedUrl.toString();
    }

    try {
      const html = await readPreviewDocument(lastUrl);
      if (!previewRefreshLeaseMatches(host, lease)) return null;
      if (!requiredRevision || previewDocumentHasRevision(html, requiredRevision)) {
        return { url: lastUrl, revision: requiredRevision, html };
      }
      lastError = new Error("Randarea Zola nu a ajuns încă la ultima generație de preview.");
    } catch (error) {
      if (!previewRefreshLeaseMatches(host, lease)) return null;
      lastError = error;
    }
  }

  throw lastError ?? new Error("Randarea Zola nu a răspuns cu generația cerută de preview.");
}

async function waitForPreviewDocumentUrl(
  host: PreviewControllerHost,
  previewUrl: string,
  requiredRevision: string,
  lease: PreviewRefreshLease,
): Promise<RenderedPreviewReady | null> {
  let lastUrl = previewUrl;
  let lastError: unknown = null;

  for (let attempt = 0; attempt < PREVIEW_REVISION_ATTEMPTS; attempt += 1) {
    if (attempt > 0) {
      await wait(PREVIEW_REVISION_DELAYS_MS[Math.min(attempt, PREVIEW_REVISION_DELAYS_MS.length - 1)]);
      if (!previewRefreshLeaseMatches(host, lease)) return null;
    }
    lastUrl = previewReloadUrl(host, previewUrl);
    try {
      const html = await readPreviewDocument(lastUrl);
      if (!previewRefreshLeaseMatches(host, lease)) return null;
      if (previewDocumentHasRevision(html, requiredRevision)) {
        return { url: lastUrl, revision: requiredRevision, html };
      }
      lastError = new Error(
        "Context de template nu a ajuns încă la generația Canvas cerută.",
      );
    } catch (error) {
      if (!previewRefreshLeaseMatches(host, lease)) return null;
      lastError = error;
    }
  }

  throw lastError ?? new Error("Context de template nu a răspuns cu generația cerută.");
}

function samePreviewRoute(currentUrl: string, nextUrl: string) {
  try {
    const current = new URL(currentUrl);
    const next = new URL(nextUrl);
    return current.origin === next.origin && current.pathname === next.pathname;
  } catch {
    return false;
  }
}

function canvasIdentityMatches(
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

async function confirmPendingCanvasProjection(
  host: PreviewControllerHost,
  plan: CanvasProjectionPlan,
  receipts: PreviewPhaseReceipt[],
) {
  if (!canvasIdentityMatches(host.pendingCanvasProjection?.identity, plan.identity)) {
    throw new Error("Canvas ACK a devenit stale înainte de confirmarea Rust.");
  }

  const phases = receipts.map((receipt) => receipt.phase);
  const failed = phases.length === 1 && phases[0] === "failed";
  if (
    !failed
    && (
      phases.length !== 3
      || phases[0] !== "resourcesReady"
      || phases[1] !== "committed"
      || phases[2] !== "styledReady"
    )
  ) {
    throw new Error("Browserul nu a furnizat secvența ACK Canvas completă și ordonată.");
  }

  let confirmed: CanvasProjectionPlan | null = null;
  for (const receipt of receipts) {
    if (
      receipt.schemaVersion !== plan.schemaVersion
      || !canvasIdentityMatches(receipt.identity, plan.identity)
      || !receipt.phaseTimingsMs
      || typeof receipt.phaseTimingsMs !== "object"
    ) {
      throw new Error("Browserul a furnizat un ACK Canvas din altă tranzacție.");
    }
    confirmed = await acknowledgeCanvasProjectionPhase(receipt);
    const expectedPhase = receipt.phase === "styledReady"
      ? "canonicalVerified"
      : receipt.phase;
    if (
      confirmed.phase !== expectedPhase
      || !canvasIdentityMatches(confirmed.identity, plan.identity)
    ) {
      throw new Error(`Rust nu a confirmat exact faza Canvas ${receipt.phase}.`);
    }
  }

  if (failed) {
    if (canvasIdentityMatches(host.pendingCanvasProjection?.identity, plan.identity)) {
      host.pendingCanvasProjection = null;
    }
    throw new PreviewProjectionDiagnosticError(
      "preview_reconcile_failed",
      receipts[0]?.diagnostic || "Browserul a raportat eșecul tranzacției Canvas.",
    );
  }
  if (!confirmed || confirmed.phase !== "canonicalVerified") {
    throw new Error("Rust nu a confirmat canonic exact tranzacția Canvas stilizată.");
  }
  host.activeCanvasIdentity = { ...plan.identity };
  host.activeCanvasUrl = host.previewSrc;
  if (canvasIdentityMatches(host.pendingCanvasProjection?.identity, plan.identity)) {
    host.pendingCanvasProjection = null;
  }
  if (host.canvasProjectionConfirmation?.transactionId === plan.identity.transactionId) {
    globalThis.clearTimeout(host.canvasProjectionConfirmation.timeout);
    host.canvasProjectionConfirmation.resolve();
    host.canvasProjectionConfirmation = null;
  }
}

function beginCanvasProjectionConfirmation(
  host: PreviewControllerHost,
  plan: CanvasProjectionPlan,
) {
  cancelCanvasProjectionConfirmation(
    host,
    "Confirmarea Canvas a fost înlocuită de altă tranzacție.",
  );
  let resolve!: () => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<void>((accept, deny) => {
    resolve = accept;
    reject = deny;
  });
  const timeout = globalThis.setTimeout(() => {
    if (host.canvasProjectionConfirmation?.transactionId !== plan.identity.transactionId) return;
    host.canvasProjectionConfirmation = null;
    reject(new Error("Canvas-ul navigat nu a confirmat styledReady în 15 secunde."));
  }, 15_000);
  host.canvasProjectionConfirmation = {
    transactionId: plan.identity.transactionId,
    promise,
    resolve,
    reject,
    timeout,
  };
  return promise;
}

export function prepareCanvasProjectionNavigation(
  host: PreviewControllerHost,
  plan: CanvasProjectionPlan,
) {
  if (plan.phase !== "prepared") {
    throw new Error(`Canvas navigation cere faza prepared, nu ${plan.phase}.`);
  }
  host.pendingCanvasProjection = plan;
  host.previewWorkspaceRevision = plan.identity.previewRevision;
  return beginCanvasProjectionConfirmation(host, plan);
}

export function cancelCanvasProjectionConfirmation(
  host: PreviewControllerHost,
  reason = "Confirmarea Canvas a fost anulată.",
) {
  const confirmation = host.canvasProjectionConfirmation;
  if (!confirmation) return;
  globalThis.clearTimeout(confirmation.timeout);
  host.canvasProjectionConfirmation = null;
  confirmation.reject(new Error(reason));
}

export async function confirmMountedCanvasProjection(
  host: PreviewControllerHost,
  documentCanvasIdentity: CanvasProjectionIdentity | null,
  phaseReceipts: PreviewPhaseReceipt[],
) {
  const plan = host.pendingCanvasProjection;
  if (!plan || !canvasIdentityMatches(documentCanvasIdentity, plan.identity)) return false;
  try {
    await confirmPendingCanvasProjection(host, plan, phaseReceipts);
    return true;
  } catch (error) {
    if (host.canvasProjectionConfirmation?.transactionId === plan.identity.transactionId) {
      globalThis.clearTimeout(host.canvasProjectionConfirmation.timeout);
      host.canvasProjectionConfirmation.reject(
        error instanceof Error ? error : new Error(String(error)),
      );
      host.canvasProjectionConfirmation = null;
    }
    throw error;
  }
}

async function replaceMountedPreviewWithCanonicalDocument(
  host: PreviewControllerHost,
  ready: RenderedPreviewReady,
  lease: PreviewRefreshLease,
): Promise<InPlaceProjectionOutcome> {
  if (!host.previewRuntime) return { kind: "unsupported", reason: "runtime_unavailable" };
  if (!host.previewFrame?.contentWindow) return { kind: "unsupported", reason: "frame_unavailable" };
  if (host.previewSrc === "about:blank") return { kind: "unsupported", reason: "initial_navigation" };
  if (host.previewDocumentMarkup !== null) return { kind: "unsupported", reason: "markup_preview" };
  if (!samePreviewRoute(host.previewSrc, ready.url)) {
    return { kind: "unsupported", reason: "route_changed" };
  }
  const selector = host.selectedElement?.domPath
    ?? host.selectedElement?.cssSelector
    ?? null;
  const plan = host.pendingCanvasProjection;
  if (!plan || plan.identity.previewRevision !== ready.revision) {
    return { kind: "unsupported", reason: "plan_missing" };
  }
  hidePreviewHtmlSelectionOverlay(getPreviewDocument(host));
  host.selectedPreviewElement = null;
  let ack;
  try {
    ack = await host.previewRuntime.sendAndWait({
      type: "replace-document",
      html: ready.html,
      selector,
      liveCss: serializeOverrides(host.overrideRules, host.variableOverrides),
      canvasIdentity: plan.identity,
    });
  } catch (error) {
    if (error instanceof PreviewRuntimeTransportError) {
      return { kind: "unsupported", reason: "runtime_unresponsive" };
    }
    throw error;
  }
  if (!previewRefreshLeaseMatches(host, lease)) return { kind: "stale" };
  if (!canvasIdentityMatches(ack.canvasIdentity, plan.identity)) {
    throw new PreviewProjectionDiagnosticError(
      "preview_canvas_identity_mismatch",
      "Legătura previzualizării a confirmat altă tranzacție Canvas.",
    );
  }
  await confirmPendingCanvasProjection(host, plan, ack.canvasPhaseReceipts ?? []);
  if (!ack.ok) {
    throw new PreviewProjectionDiagnosticError(
      "preview_reconcile_failed",
      ack.error || "Legătura previzualizării a refuzat documentul Zola canonic.",
    );
  }
  if (!previewRefreshLeaseMatches(host, lease)) return { kind: "stale" };
  return { kind: "committed" };
}

export function cancelPreviewSync(host: PreviewControllerHost) {
  if (host.previewSyncTimer !== null) {
    window.clearTimeout(host.previewSyncTimer);
    host.previewSyncTimer = null;
  }
}

export function invalidatePreviewDomTreeProjection(host: PreviewControllerHost) {
  host.previewDomTreeSerial += 1;
  if (host.domTreeFetchTimer !== null) {
    window.clearTimeout(host.domTreeFetchTimer);
    host.domTreeFetchTimer = null;
  }
}

export function clearPreviewTimers(host: PreviewControllerHost) {
  invalidatePreviewRefreshLease(host);
  invalidatePreviewDomTreeProjection(host);
  cancelPreviewSync(host);
  cancelCanvasProjectionConfirmation(host);
}

export function getPreviewDocument(host: PreviewControllerHost): Document | undefined {
  const frame = host.previewFrame;
  if (!frame || !previewFrameAllowsDocumentAccess(frame, window.location.href)) return undefined;
  try {
    const previewDocument = frame.contentDocument ?? undefined;
    if (!previewDocument) rememberPreviewFrameDocumentAccessFailure(frame);
    return previewDocument;
  } catch {
    rememberPreviewFrameDocumentAccessFailure(frame);
    return undefined;
  }
}

export function postPreviewMessage(host: PreviewControllerHost, payload: Record<string, unknown>) {
  const message = JSON.parse(JSON.stringify({ source: "pana-studio-app", ...payload }));
  host.previewFrame?.contentWindow?.postMessage(message, "*");
}

export function sendPreviewOperation(host: PreviewControllerHost, payload: Record<string, unknown> & { type: string }) {
  if (host.previewRuntime) return host.previewRuntime.send(payload);
  postPreviewMessage(host, payload);
  return null;
}

export async function refreshRenderedPreviewDocument(
  host: PreviewControllerHost,
  providedLease?: PreviewRefreshLease,
) {
  const lease = providedLease ?? beginPreviewRefreshLease(host);
  if (!lease || !previewRefreshLeaseMatches(host, lease)) return false;
  const previewPage = host.activeRenderedPreviewPageFile;
  if (!previewPage) return false;
  try {
    const ready = await waitForRenderedPreviewUrl(host, previewPage, lease);
    if (!ready || !previewRefreshLeaseMatches(host, lease)) return false;
    const inPlace = await replaceMountedPreviewWithCanonicalDocument(host, ready, lease);
    if (!previewRefreshLeaseMatches(host, lease)) return false;
    if (inPlace.kind === "stale") return false;
    if (inPlace.kind === "unsupported") {
      const plan = host.pendingCanvasProjection;
      if (plan) {
        void host.recordCanvasProjectionRuntimeEvent?.(
          "canvas_fallback",
          plan.identity,
          0,
          inPlace.reason,
        );
      }
      const confirmation = plan && plan.identity.previewRevision === ready.revision
        ? beginCanvasProjectionConfirmation(host, plan)
        : null;
      host.previewSrc = ready.url;
      if (confirmation) await confirmation;
      if (!previewRefreshLeaseMatches(host, lease)) return false;
    }
    host.previewDocumentMarkup = null;
    if (ready.revision && host.previewWorkspaceRevision === ready.revision) {
      host.previewWorkspaceRevision = null;
    }
    return true;
  } catch (error) {
    if (!previewRefreshLeaseMatches(host, lease)) return false;
    const message = errorMessage(error);
    host.projectStatus = `Randarea previzualizării a eșuat: ${message}`;
    if (!host.previewSrc || host.previewSrc === "about:blank" || !host.previewFrame) {
      host.previewSrc = "about:blank";
      host.previewDocumentMarkup = buildPreviewStatusDocument(
        "Previzualizare indisponibilă",
        `Previzualizarea Zola nu răspunde momentan.\n\n${message}`,
      );
    }
    return false;
  }
}

/**
 * Confirmă un candidat Canvas prin documentul Workbench deja montat. Când
 * ruta rămâne aceeași, bridge-ul reconciliază DOM-ul în loc și păstrează
 * selecția/gate-ul; navigarea iframe este doar fallback pentru prima montare.
 */
export async function reconcileTemplateWorkbenchPreviewDocument(
  host: PreviewControllerHost,
  previewUrl: string,
  plan: CanvasProjectionPlan,
) {
  if (plan.phase !== "prepared") {
    throw new Error(`Reconcilierea Workbench cere faza prepared, nu ${plan.phase}.`);
  }
  const requestedUrl = new URL(previewUrl);
  if (
    !requestedUrl.pathname.startsWith("/__pana_workbench/")
    || requestedUrl.searchParams.get("__pana_preview_revision") !== plan.identity.previewRevision
    || requestedUrl.searchParams.get("__pana_canvas_transaction") !== plan.identity.transactionId
  ) {
    throw new Error("URL-ul Context de template nu aparține candidatului Canvas primit.");
  }

  const lease = beginPreviewRefreshLease(host);
  if (!lease || !previewRefreshLeaseMatches(host, lease)) return false;
  const confirmation = prepareCanvasProjectionNavigation(host, plan);
  try {
    const ready = await waitForPreviewDocumentUrl(
      host,
      previewUrl,
      plan.identity.previewRevision,
      lease,
    );
    if (!ready || !previewRefreshLeaseMatches(host, lease)) return false;
    const inPlace = await replaceMountedPreviewWithCanonicalDocument(host, ready, lease);
    if (!previewRefreshLeaseMatches(host, lease) || inPlace.kind === "stale") return false;
    if (inPlace.kind === "unsupported") {
      void host.recordCanvasProjectionRuntimeEvent?.(
        "canvas_fallback",
        plan.identity,
        0,
        `template_workbench_${inPlace.reason}`,
      );
      host.previewSrc = ready.url;
    }
    await confirmation;
    if (!previewRefreshLeaseMatches(host, lease)) return false;
    if (host.previewWorkspaceRevision === plan.identity.previewRevision) {
      host.previewWorkspaceRevision = null;
    }
    return true;
  } catch (error) {
    if (host.canvasProjectionConfirmation?.transactionId === plan.identity.transactionId) {
      cancelCanvasProjectionConfirmation(
        host,
        error instanceof Error ? error.message : String(error),
      );
    }
    if (canvasIdentityMatches(host.pendingCanvasProjection?.identity, plan.identity)) {
      host.pendingCanvasProjection = null;
    }
    throw error;
  }
}

export async function reloadPreview(
  host: PreviewControllerHost,
  providedLease?: PreviewRefreshLease,
) {
  const lease = providedLease ?? beginPreviewRefreshLease(host);
  if (!lease || !previewRefreshLeaseMatches(host, lease)) return false;
  hidePreviewHtmlSelectionOverlay(getPreviewDocument(host));
  host.selectedPreviewElement = null;
  const rendered = await refreshRenderedPreviewDocument(host, lease);
  if (!previewRefreshLeaseMatches(host, lease)) return false;
  if (rendered) return true;
  if (host.isActiveRenderedPreviewPage) return false;
  const frame = host.previewFrame;
  if (!frame) return false;
  if (previewFrameAllowsDocumentAccess(frame, window.location.href)) {
    try {
      frame.contentWindow?.location.reload();
      return true;
    } catch {
      // The iframe may have redirected after the source-origin check.
      rememberPreviewFrameDocumentAccessFailure(frame);
    }
  }
  if (host.previewSrc && host.previewSrc !== "about:blank") {
    host.previewSrc = previewReloadUrl(host, host.previewSrc);
    return true;
  }
  return false;
}

export function fetchDomTreeFromPreview(host: PreviewControllerHost) {
  const url = host.previewSrc;
  if (!url || url === "about:blank") return;
  const projectRoot = host.sessionProjectRoot.trim();
  const runtimeSessionId = host.kernelProjectSessionId.trim();
  if (!projectRoot || !runtimeSessionId) return;
  const lease = {
    projectRoot,
    runtimeSessionId,
    projectSessionEpoch: host.projectSessionEpoch,
    serial: ++host.previewDomTreeSerial,
  };
  const leaseMatches = () => (
    host.previewDomTreeSerial === lease.serial
    && host.sessionProjectRoot === lease.projectRoot
    && host.kernelProjectSessionId === lease.runtimeSessionId
    && host.projectSessionEpoch === lease.projectSessionEpoch
    && host.previewSrc === url
  );
  if (host.domTreeFetchTimer !== null) {
    window.clearTimeout(host.domTreeFetchTimer);
  }
  host.domTreeFetchTimer = window.setTimeout(() => {
    host.domTreeFetchTimer = null;
    if (!leaseMatches()) return;
    readPreviewDocument(url)
      .then((html) => {
        if (!leaseMatches()) return;
        const parser = new DOMParser();
        const doc = parser.parseFromString(html, "text/html");
        const sections = collectDomTree(doc);
        if (!leaseMatches()) return;
        if (host.setPageSections) host.setPageSections(sections);
        else host.pageSections = sections;
      })
      .catch(() => {});
  }, 150);
}
