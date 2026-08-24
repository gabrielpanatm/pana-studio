import { serializeOverrides } from "$lib/css/serializer";
import type { EditableStyles } from "$lib/css/contracts";
import { buildPreviewStatusDocument } from "$lib/preview/bridge";
import { collectDomTree } from "$lib/preview/selection";
import {
  previewFrameAllowsDocumentAccess,
  rememberPreviewFrameDocumentAccessFailure,
} from "$lib/preview/frame-origin";
import {
  acknowledgeCanvasProjectionPhases,
  readPreviewDocument,
} from "$lib/preview/io";
import type {
  CanvasProjectionIdentity,
  CanvasProjectionPlan,
  PreviewPhaseReceipt,
  PreviewRuntimeEventKind,
  PreviewStylesheetPromotionMetrics,
} from "$lib/contracts/canvas-projection";
import type { PageSection } from "$lib/canvas/contracts";
import type {
  ProjectFile,
  ProjectLifecycleSnapshot,
} from "$lib/project/lifecycle-contract";
import {
  PreviewRuntimeTransportError,
  type PreviewRuntime,
} from "$lib/editor-runtime/preview-runtime";
import { errorMessage } from "$lib/util";
import { t } from "$lib/i18n/runtime.svelte";
import type { EditorSelectionSessionController } from "$lib/state/editor-selection-session.svelte";
import { sameCanvasProjectionIdentity as canvasIdentityMatches } from "$lib/contracts/canvas-identity";

export type PreviewRefreshLeaseHost = {
  sessionProjectRoot: string;
  kernelProjectSessionId: string;
  projectSessionEpoch: number;
  previewRefreshSerial: number;
  previewDomTreeSerial: number;
};

export type PreviewControllerHost = {
  session: PreviewRefreshLeaseHost;
  surface: {
    frame: HTMLIFrameElement | undefined;
    canvasElement: HTMLIFrameElement | null;
    generation: number;
  };
  navigation: {
    src: string;
    reloadSerial: number;
    activeUrl: string;
    guardActive: boolean;
    recoveryUrl: string | null;
  };
  projection: {
    workspaceRevision: string | null;
    pending: CanvasProjectionPlan | null;
    activeIdentity: CanvasProjectionIdentity | null;
    confirmation: CanvasProjectionConfirmation | null;
  };
  timers: {
    previewSync: number | null;
    domTreeFetch: number | null;
  };
  document: {
    markup: string | null;
    activePage: ProjectFile | null;
    isActivePage: boolean;
    projectStatus: string;
  };
  context: {
    lifecycle: ProjectLifecycleSnapshot;
    templateWorkbenchActive: boolean;
  };
  styles: {
    overrideRules: Record<string, EditableStyles>;
    variableOverrides: Record<string, string>;
  };
  sections: {
    items: PageSection[];
    set: (sections: PageSection[]) => void;
  };
  selection: Pick<EditorSelectionSessionController, "refreshNavigationSnapshot">;
  runtime?: PreviewRuntime;
  commands: {
    urlForFile: (file: ProjectFile) => string;
    recordRuntimeEvent?: (
      kind: PreviewRuntimeEventKind,
      identity: CanvasProjectionIdentity,
      durationMs: number,
      diagnostic: string | null,
      stylesheetMetrics?: PreviewStylesheetPromotionMetrics | null,
    ) => Promise<void>;
  };
};

export type CanvasProjectionConfirmation = {
  transactionId: string;
  surfaceGeneration: number;
  startedAt: number;
  lastPhase: "prepared";
  acknowledgement: Promise<void> | null;
  promise: Promise<void>;
  resolve: () => void;
  reject: (error: Error) => void;
  timeout: ReturnType<typeof globalThis.setTimeout>;
};

export class CanvasProjectionSurfaceUnavailableError extends Error {
  readonly code = "canvas_surface_unavailable";
  readonly reason: "surface_not_mounted" | "surface_unmounted";

  constructor(
    reason: "surface_not_mounted" | "surface_unmounted",
    message = reason === "surface_unmounted"
      ? t("preview-controller-surface-unmounted")
      : t("preview-controller-surface-missing"),
  ) {
    super(message);
    this.name = "CanvasProjectionSurfaceUnavailableError";
    this.reason = reason;
  }
}

export function isCanvasProjectionSurfaceUnavailableError(
  error: unknown,
): error is CanvasProjectionSurfaceUnavailableError {
  return error instanceof CanvasProjectionSurfaceUnavailableError
    || (
      typeof error === "object"
      && error !== null
      && "code" in error
      && error.code === "canvas_surface_unavailable"
    );
}

export function hasMountedCanvasProjectionSurface(host: PreviewControllerHost) {
  return Boolean(
    host.surface.canvasElement
    && host.surface.canvasElement === host.surface.frame
    && host.surface.canvasElement.contentWindow,
  );
}

function beginGuardedPreviewNavigation(
  host: PreviewControllerHost,
  candidateUrl: string,
) {
  const currentUrl = host.navigation.activeUrl && host.navigation.activeUrl !== "about:blank"
    ? host.navigation.activeUrl
    : host.navigation.src;
  host.navigation.recoveryUrl = host.projection.activeIdentity
    && currentUrl
    && currentUrl !== "about:blank"
    ? currentUrl
    : null;
  host.navigation.guardActive = true;
  host.navigation.src = candidateUrl;
}

function restoreLastStyledPreviewAfterNavigationFailure(
  host: PreviewControllerHost,
) {
  const recoveryUrl = host.navigation.recoveryUrl;
  if (!host.navigation.guardActive || !recoveryUrl) return false;
  host.navigation.recoveryUrl = null;
  host.navigation.src = recoveryUrl;
  return true;
}

export function settleGuardedPreviewNavigation(
  host: PreviewControllerHost,
  identity: CanvasProjectionIdentity | null,
) {
  if (!host.navigation.guardActive || !identity) return false;
  if (
    !canvasIdentityMatches(identity, host.projection.activeIdentity)
    && !canvasIdentityMatches(identity, host.projection.pending?.identity)
  ) return false;
  host.navigation.guardActive = false;
  host.navigation.recoveryUrl = null;
  return true;
}

export function mountCanvasProjectionSurface(
  host: PreviewControllerHost,
  frame: HTMLIFrameElement,
) {
  if (host.surface.canvasElement === frame) return host.surface.generation;
  if (host.surface.canvasElement) {
    invalidatePreviewRefreshLease(host.session);
    cancelCanvasProjectionConfirmation(
      host,
      new CanvasProjectionSurfaceUnavailableError(
        "surface_unmounted",
        t("preview-controller-surface-replaced"),
      ),
    );
    host.projection.pending = null;
    host.projection.workspaceRevision = null;
  }
  host.surface.generation += 1;
  host.surface.canvasElement = frame;
  host.surface.frame = frame;
  return host.surface.generation;
}

export function unmountCanvasProjectionSurface(
  host: PreviewControllerHost,
  frame: HTMLIFrameElement,
) {
  if (host.surface.canvasElement !== frame) return false;
  host.surface.canvasElement = null;
  if (host.surface.frame === frame) host.surface.frame = undefined;
  host.surface.generation += 1;
  invalidatePreviewRefreshLease(host.session);
  cancelCanvasProjectionConfirmation(
    host,
    new CanvasProjectionSurfaceUnavailableError("surface_unmounted"),
  );
  host.projection.pending = null;
  host.projection.workspaceRevision = null;
  return true;
}

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
  next.searchParams.set("__pana_reload", String(++host.navigation.reloadSerial));
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
  const requiredRevision = host.projection.workspaceRevision;
  let lastUrl = "";
  let lastError: unknown = null;

  for (let attempt = 0; attempt < PREVIEW_REVISION_ATTEMPTS; attempt += 1) {
    if (attempt > 0) {
      await wait(PREVIEW_REVISION_DELAYS_MS[Math.min(attempt, PREVIEW_REVISION_DELAYS_MS.length - 1)]);
      if (!previewRefreshLeaseMatches(host.session, lease)) return null;
    }

    lastUrl = previewReloadUrl(host, host.commands.urlForFile(previewPage));
    if (requiredRevision) {
      const stagedUrl = new URL(lastUrl);
      stagedUrl.searchParams.set("__pana_preview_revision", requiredRevision);
      lastUrl = stagedUrl.toString();
    }

    try {
      const html = await readPreviewDocument(lastUrl);
      if (!previewRefreshLeaseMatches(host.session, lease)) return null;
      if (!requiredRevision || previewDocumentHasRevision(html, requiredRevision)) {
        return { url: lastUrl, revision: requiredRevision, html };
      }
      lastError = new Error(t("preview-controller-render-generation-pending"));
    } catch (error) {
      if (!previewRefreshLeaseMatches(host.session, lease)) return null;
      lastError = error;
    }
  }

  throw lastError ?? new Error(t("preview-controller-render-generation-missing"));
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
      if (!previewRefreshLeaseMatches(host.session, lease)) return null;
    }
    lastUrl = previewReloadUrl(host, previewUrl);
    try {
      const html = await readPreviewDocument(lastUrl);
      if (!previewRefreshLeaseMatches(host.session, lease)) return null;
      if (previewDocumentHasRevision(html, requiredRevision)) {
        return { url: lastUrl, revision: requiredRevision, html };
      }
      lastError = new Error(
        t("preview-controller-template-generation-pending"),
      );
    } catch (error) {
      if (!previewRefreshLeaseMatches(host.session, lease)) return null;
      lastError = error;
    }
  }

  throw lastError ?? new Error(t("preview-controller-template-generation-missing"));
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

function validateCanvasPhaseReceipts(
  plan: CanvasProjectionPlan,
  receipts: PreviewPhaseReceipt[],
) {
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
    throw new Error(t("preview-controller-ack-sequence-invalid"));
  }

  for (const receipt of receipts) {
    if (
      receipt.schemaVersion !== plan.schemaVersion
      || !canvasIdentityMatches(receipt.identity, plan.identity)
      || !receipt.phaseTimingsMs
      || typeof receipt.phaseTimingsMs !== "object"
    ) {
      throw new Error(t("preview-controller-ack-transaction-mismatch"));
    }
  }
  return failed;
}

async function completePendingCanvasProjection(
  host: PreviewControllerHost,
  plan: CanvasProjectionPlan,
  receipts: PreviewPhaseReceipt[],
  failed: boolean,
  confirmation: CanvasProjectionConfirmation,
) {
  const confirmed = await acknowledgeCanvasProjectionPhases(receipts);
  if (
    host.projection.confirmation !== confirmation
    || host.surface.generation !== confirmation.surfaceGeneration
    || !canvasIdentityMatches(host.projection.pending?.identity, plan.identity)
  ) throw new Error(t("preview-controller-ack-stale"));
  const expectedPhase = failed ? "failed" : "canonicalVerified";
  if (
    confirmed.phase !== expectedPhase
    || !canvasIdentityMatches(confirmed.identity, plan.identity)
  ) {
    throw new Error(t("preview-controller-phase-unconfirmed", {
      phase: failed ? "failed" : "styledReady",
    }));
  }

  if (failed) {
    if (canvasIdentityMatches(host.projection.pending?.identity, plan.identity)) {
      host.projection.pending = null;
    }
    restoreLastStyledPreviewAfterNavigationFailure(host);
    throw new PreviewProjectionDiagnosticError(
      "preview_reconcile_failed",
      receipts[0]?.diagnostic || t("preview-controller-browser-transaction-failed"),
    );
  }
  if (!confirmed || confirmed.phase !== "canonicalVerified") {
    throw new Error(t("preview-controller-rust-transaction-unconfirmed"));
  }
  host.projection.activeIdentity = { ...plan.identity };
  host.navigation.activeUrl = host.navigation.src;
  host.navigation.guardActive = false;
  host.navigation.recoveryUrl = null;
  const readiness = host.context.lifecycle?.activeSession?.readiness.state ?? null;
  if (!readiness || readiness === "ready" || readiness === "degraded") {
    // Canvas-ul canonic nu este utilizabil de Inspector până când selecția
    // semantică nu a fost rebazată pe aceeași identitate. Dacă rezolvăm
    // confirmarea înaintea acestei bariere, un focus CSS poate concura cu
    // rebazarea Rust și poate captura o revizie de selecție deja expirată.
    await host.selection.refreshNavigationSnapshot(plan.identity, host.navigation.src);
  }
  if (canvasIdentityMatches(host.projection.pending?.identity, plan.identity)) {
    host.projection.pending = null;
  }
  if (host.projection.confirmation === confirmation) {
    globalThis.clearTimeout(confirmation.timeout);
    confirmation.resolve();
    host.projection.confirmation = null;
  }
}

async function confirmPendingCanvasProjection(
  host: PreviewControllerHost,
  plan: CanvasProjectionPlan,
  receipts: PreviewPhaseReceipt[],
) {
  const failed = validateCanvasPhaseReceipts(plan, receipts);
  if (
    !failed
    && !host.projection.pending
    && canvasIdentityMatches(host.projection.activeIdentity, plan.identity)
  ) return;
  if (!canvasIdentityMatches(host.projection.pending?.identity, plan.identity)) {
    throw new Error(t("preview-controller-ack-stale"));
  }
  const confirmation = host.projection.confirmation;
  if (
    !confirmation
    || confirmation.transactionId !== plan.identity.transactionId
    || confirmation.surfaceGeneration !== host.surface.generation
  ) throw new Error(t("preview-controller-ack-stale"));
  if (confirmation.acknowledgement) {
    await confirmation.acknowledgement;
    return;
  }
  const acknowledgement = completePendingCanvasProjection(
    host,
    plan,
    receipts,
    failed,
    confirmation,
  );
  confirmation.acknowledgement = acknowledgement;
  await acknowledgement;
}

function beginCanvasProjectionConfirmation(
  host: PreviewControllerHost,
  plan: CanvasProjectionPlan,
) {
  if (!hasMountedCanvasProjectionSurface(host)) {
    throw new CanvasProjectionSurfaceUnavailableError("surface_not_mounted");
  }
  cancelCanvasProjectionConfirmation(
    host,
    t("preview-controller-confirmation-superseded"),
  );
  const surfaceGeneration = host.surface.generation;
  const startedAt = performance.now();
  let resolve!: () => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<void>((accept, deny) => {
    resolve = accept;
    reject = deny;
  });
  // Some confirmation owners fail before they can await the public barrier.
  // Keep that rejection observable to explicit awaiters without leaking an
  // unhandled promise from the internal cancellation path.
  void promise.catch(() => undefined);
  const timeout = globalThis.setTimeout(() => {
    const confirmation = host.projection.confirmation;
    if (
      confirmation?.transactionId !== plan.identity.transactionId
      || confirmation.surfaceGeneration !== surfaceGeneration
      || host.surface.generation !== surfaceGeneration
      || !hasMountedCanvasProjectionSurface(host)
    ) return;
    host.projection.confirmation = null;
    const durationMs = Math.max(0, performance.now() - startedAt);
    void host.commands.recordRuntimeEvent?.(
      "canvas_ack_timeout",
      plan.identity,
      durationMs,
      `surfaceGeneration=${surfaceGeneration};lastPhase=prepared`,
    );
    restoreLastStyledPreviewAfterNavigationFailure(host);
    reject(new Error(t("preview-controller-styled-ready-timeout")));
  }, 15_000);
  host.projection.confirmation = {
    transactionId: plan.identity.transactionId,
    surfaceGeneration,
    startedAt,
    lastPhase: "prepared",
    acknowledgement: null,
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
    throw new Error(t("preview-controller-navigation-phase-invalid", {
      phase: plan.phase,
    }));
  }
  if (!hasMountedCanvasProjectionSurface(host)) {
    throw new CanvasProjectionSurfaceUnavailableError("surface_not_mounted");
  }
  host.projection.pending = plan;
  host.projection.workspaceRevision = plan.identity.previewRevision;
  return beginCanvasProjectionConfirmation(host, plan);
}

export function cancelCanvasProjectionConfirmation(
  host: PreviewControllerHost,
  reason: string | Error = t("preview-controller-confirmation-cancelled"),
) {
  const confirmation = host.projection.confirmation;
  if (!confirmation) return;
  globalThis.clearTimeout(confirmation.timeout);
  host.projection.confirmation = null;
  confirmation.reject(reason instanceof Error ? reason : new Error(reason));
}

export async function confirmMountedCanvasProjection(
  host: PreviewControllerHost,
  documentCanvasIdentity: CanvasProjectionIdentity | null,
  phaseReceipts: PreviewPhaseReceipt[],
) {
  const plan = host.projection.pending;
  const confirmation = host.projection.confirmation;
  if (
    !plan
    || !confirmation
    || confirmation.surfaceGeneration !== host.surface.generation
    || !hasMountedCanvasProjectionSurface(host)
    || !canvasIdentityMatches(documentCanvasIdentity, plan.identity)
  ) return false;
  try {
    await confirmPendingCanvasProjection(host, plan, phaseReceipts);
    return true;
  } catch (error) {
    if (host.projection.confirmation === confirmation) {
      globalThis.clearTimeout(confirmation.timeout);
      confirmation.reject(
        error instanceof Error ? error : new Error(String(error)),
      );
      host.projection.confirmation = null;
    }
    throw error;
  }
}

async function replaceMountedPreviewWithCanonicalDocument(
  host: PreviewControllerHost,
  ready: RenderedPreviewReady,
  lease: PreviewRefreshLease,
): Promise<InPlaceProjectionOutcome> {
  if (!host.runtime) return { kind: "unsupported", reason: "runtime_unavailable" };
  if (!host.surface.frame?.contentWindow) return { kind: "unsupported", reason: "frame_unavailable" };
  if (host.navigation.src === "about:blank") return { kind: "unsupported", reason: "initial_navigation" };
  if (host.document.markup !== null) return { kind: "unsupported", reason: "markup_preview" };
  if (!samePreviewRoute(host.navigation.src, ready.url)) {
    return { kind: "unsupported", reason: "route_changed" };
  }
  const plan = host.projection.pending;
  if (!plan || plan.identity.previewRevision !== ready.revision) {
    return { kind: "unsupported", reason: "plan_missing" };
  }
  let ack;
  try {
    ack = await host.runtime.sendAndWait(
      {
        type: "replace-document",
        html: ready.html,
        liveCss: serializeOverrides(host.styles.overrideRules, host.styles.variableOverrides),
        canvasIdentity: plan.identity,
      },
      host.context.templateWorkbenchActive ? { ackTimeoutMs: 1_500 } : {},
    );
  } catch (error) {
    if (error instanceof PreviewRuntimeTransportError) {
      return { kind: "unsupported", reason: "runtime_unresponsive" };
    }
    throw error;
  }
  if (!previewRefreshLeaseMatches(host.session, lease)) return { kind: "stale" };
  if (!canvasIdentityMatches(ack.canvasIdentity, plan.identity)) {
    throw new PreviewProjectionDiagnosticError(
      "preview_canvas_identity_mismatch",
      t("preview-controller-bridge-transaction-mismatch"),
    );
  }
  await confirmPendingCanvasProjection(host, plan, ack.canvasPhaseReceipts ?? []);
  if (!ack.ok) {
    throw new PreviewProjectionDiagnosticError(
      "preview_reconcile_failed",
      ack.error || t("preview-controller-bridge-document-rejected"),
    );
  }
  const promotion = ack.stylesheetPromotion;
  const promotionIntegerMetrics = promotion
    ? [
        promotion.reused,
        promotion.staged,
        promotion.retired,
        promotion.preloadsReused ?? 0,
        promotion.preloadsStaged ?? 0,
        promotion.preloadsRetired ?? 0,
        promotion.headNodesReused ?? 0,
        promotion.headNodesCreated ?? 0,
        promotion.headNodesRetired ?? 0,
        promotion.headNodesReordered ?? 0,
        promotion.stylesheetAttributeMutations ?? 0,
        promotion.preloadAttributeMutations ?? 0,
        promotion.fontInvalidationCount ?? 0,
        promotion.fontFallbackFrames ?? 0,
        promotion.fontActivationErrorCount ?? 0,
        promotion.fontsReadyMs ?? 0,
        promotion.activationToStyledMs,
      ]
    : [];
  if (
    promotion
    && promotion.schemaVersion === 1
    && promotion.mode === "in_place"
    && promotionIntegerMetrics
      .every((value) => Number.isSafeInteger(value) && value >= 0 && value <= 600_000)
    && Number.isFinite(promotion.maxTextMetricDelta ?? 0)
    && (promotion.maxTextMetricDelta ?? 0) >= 0
    && (promotion.maxTextMetricDelta ?? 0) <= 1_000_000
    && (promotion.fontActivationErrorCount ?? 0) <= 4_096
    && (
      promotion.fontActivationDiagnostic == null
      || (
        typeof promotion.fontActivationDiagnostic === "string"
        && promotion.fontActivationDiagnostic.length > 0
        && promotion.fontActivationDiagnostic.length <= 4_000
      )
    )
    && (((promotion.fontActivationErrorCount ?? 0) === 0)
      === (promotion.fontActivationDiagnostic == null))
  ) {
    void host.commands.recordRuntimeEvent?.(
      "canvas_stylesheets_promoted",
      plan.identity,
      promotion.activationToStyledMs,
      promotion.fontActivationDiagnostic ?? null,
      {
        reused: promotion.reused,
        staged: promotion.staged,
        retired: promotion.retired,
        preloadsReused: promotion.preloadsReused ?? 0,
        preloadsStaged: promotion.preloadsStaged ?? 0,
        preloadsRetired: promotion.preloadsRetired ?? 0,
        headNodesReused: promotion.headNodesReused ?? 0,
        headNodesCreated: promotion.headNodesCreated ?? 0,
        headNodesRetired: promotion.headNodesRetired ?? 0,
        headNodesReordered: promotion.headNodesReordered ?? 0,
        stylesheetAttributeMutations: promotion.stylesheetAttributeMutations ?? 0,
        preloadAttributeMutations: promotion.preloadAttributeMutations ?? 0,
        fontInvalidationCount: promotion.fontInvalidationCount ?? 0,
        fontFallbackFrames: promotion.fontFallbackFrames ?? 0,
        maxTextMetricDelta: promotion.maxTextMetricDelta ?? 0,
        fontActivationErrorCount: promotion.fontActivationErrorCount ?? 0,
        fontActivationDiagnostic: promotion.fontActivationDiagnostic ?? null,
        fontsReadyMs: promotion.fontsReadyMs ?? 0,
        activationToStyledMs: promotion.activationToStyledMs,
      },
    );
  }
  if (!previewRefreshLeaseMatches(host.session, lease)) return { kind: "stale" };
  return { kind: "committed" };
}

export function cancelPreviewSync(host: PreviewControllerHost) {
  if (host.timers.previewSync !== null) {
    window.clearTimeout(host.timers.previewSync);
    host.timers.previewSync = null;
  }
}

export function invalidatePreviewDomTreeProjection(host: PreviewControllerHost) {
  host.session.previewDomTreeSerial += 1;
  if (host.timers.domTreeFetch !== null) {
    window.clearTimeout(host.timers.domTreeFetch);
    host.timers.domTreeFetch = null;
  }
}

export function clearPreviewTimers(host: PreviewControllerHost) {
  invalidatePreviewRefreshLease(host.session);
  invalidatePreviewDomTreeProjection(host);
  cancelPreviewSync(host);
  cancelCanvasProjectionConfirmation(host);
}

export function getPreviewDocument(host: PreviewControllerHost): Document | undefined {
  const frame = host.surface.frame;
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
  host.surface.frame?.contentWindow?.postMessage(message, "*");
}

export function sendPreviewOperation(host: PreviewControllerHost, payload: Record<string, unknown> & { type: string }) {
  if (host.runtime) return host.runtime.send(payload);
  postPreviewMessage(host, payload);
  return null;
}

export async function refreshRenderedPreviewDocument(
  host: PreviewControllerHost,
  providedLease?: PreviewRefreshLease,
) {
  const lease = providedLease ?? beginPreviewRefreshLease(host.session);
  if (!lease || !previewRefreshLeaseMatches(host.session, lease)) return false;
  const previewPage = host.document.activePage;
  if (!previewPage) return false;
  let confirmationPlan: CanvasProjectionPlan | null = null;
  let confirmation: Promise<void> | null = null;
  let confirmationOwner: CanvasProjectionConfirmation | null = null;
  try {
    const ready = await waitForRenderedPreviewUrl(host, previewPage, lease);
    if (!ready || !previewRefreshLeaseMatches(host.session, lease)) return false;
    confirmationPlan = host.projection.pending;
    if (
      confirmationPlan
      && confirmationPlan.identity.previewRevision === ready.revision
    ) {
      const activeConfirmation = host.projection.confirmation;
      confirmation = activeConfirmation?.transactionId === confirmationPlan.identity.transactionId
        && activeConfirmation.surfaceGeneration === host.surface.generation
        ? activeConfirmation.promise
        : beginCanvasProjectionConfirmation(host, confirmationPlan);
      confirmationOwner = host.projection.confirmation;
    }
    const inPlace = await replaceMountedPreviewWithCanonicalDocument(host, ready, lease);
    if (!previewRefreshLeaseMatches(host.session, lease)) return false;
    if (inPlace.kind === "stale") return false;
    if (inPlace.kind === "unsupported") {
      if (confirmationPlan) {
        void host.commands.recordRuntimeEvent?.(
          "canvas_fallback",
          confirmationPlan.identity,
          0,
          inPlace.reason,
        );
      }
      beginGuardedPreviewNavigation(host, ready.url);
    }
    if (confirmation) await confirmation;
    if (!previewRefreshLeaseMatches(host.session, lease)) return false;
    host.document.markup = null;
    if (ready.revision && host.projection.workspaceRevision === ready.revision) {
      host.projection.workspaceRevision = null;
    }
    return true;
  } catch (error) {
    if (
      confirmationOwner
      && host.projection.confirmation === confirmationOwner
    ) {
      cancelCanvasProjectionConfirmation(
        host,
        error instanceof Error ? error : new Error(String(error)),
      );
    }
    if (!previewRefreshLeaseMatches(host.session, lease)) return false;
    const message = errorMessage(error);
    host.document.projectStatus = t("preview-controller-render-failed", { message });
    if (!host.navigation.src || host.navigation.src === "about:blank" || !host.surface.frame) {
      host.navigation.src = "about:blank";
      host.document.markup = buildPreviewStatusDocument(
        t("preview-controller-unavailable-title"),
        t("preview-controller-unavailable-message", { message }),
      );
    }
    return false;
  }
}

/**
 * Confirmă un candidat Canvas prin documentul Workbench deja montat. Când
 * ruta rămâne aceeași, bridge-ul reconciliază DOM-ul în loc și păstrează
 * selecția semantică; navigarea iframe este doar fallback pentru prima montare.
 */
export async function reconcileTemplateWorkbenchPreviewDocument(
  host: PreviewControllerHost,
  previewUrl: string,
  plan: CanvasProjectionPlan,
) {
  if (plan.phase !== "prepared") {
    throw new Error(t("preview-controller-workbench-phase-invalid", {
      phase: plan.phase,
    }));
  }
  const requestedUrl = new URL(previewUrl);
  if (
    !requestedUrl.pathname.startsWith("/__pana_workbench/")
    || requestedUrl.searchParams.get("__pana_preview_revision") !== plan.identity.previewRevision
    || requestedUrl.searchParams.get("__pana_canvas_transaction") !== plan.identity.transactionId
  ) {
    throw new Error(t("preview-controller-template-url-mismatch"));
  }

  const lease = beginPreviewRefreshLease(host.session);
  if (!lease || !previewRefreshLeaseMatches(host.session, lease)) return false;
  const confirmation = prepareCanvasProjectionNavigation(host, plan);
  const confirmationOwner = host.projection.confirmation;
  try {
    const ready = await waitForPreviewDocumentUrl(
      host,
      previewUrl,
      plan.identity.previewRevision,
      lease,
    );
    if (!ready || !previewRefreshLeaseMatches(host.session, lease)) return false;
    const inPlace = await replaceMountedPreviewWithCanonicalDocument(host, ready, lease);
    if (!previewRefreshLeaseMatches(host.session, lease) || inPlace.kind === "stale") return false;
    if (inPlace.kind === "unsupported") {
      void host.commands.recordRuntimeEvent?.(
        "canvas_fallback",
        plan.identity,
        0,
        `template_workbench_${inPlace.reason}`,
      );
      beginGuardedPreviewNavigation(host, ready.url);
    }
    await confirmation;
    if (!previewRefreshLeaseMatches(host.session, lease)) return false;
    if (host.projection.workspaceRevision === plan.identity.previewRevision) {
      host.projection.workspaceRevision = null;
    }
    return true;
  } catch (error) {
    if (confirmationOwner && host.projection.confirmation === confirmationOwner) {
      cancelCanvasProjectionConfirmation(
        host,
        error instanceof Error ? error.message : String(error),
      );
    }
    if (canvasIdentityMatches(host.projection.pending?.identity, plan.identity)) {
      host.projection.pending = null;
    }
    throw error;
  }
}

export async function reloadPreview(
  host: PreviewControllerHost,
  providedLease?: PreviewRefreshLease,
) {
  const lease = providedLease ?? beginPreviewRefreshLease(host.session);
  if (!lease || !previewRefreshLeaseMatches(host.session, lease)) return false;
  const rendered = await refreshRenderedPreviewDocument(host, lease);
  if (!previewRefreshLeaseMatches(host.session, lease)) return false;
  if (rendered) return true;
  if (host.document.isActivePage) return false;
  const frame = host.surface.frame;
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
  if (host.navigation.src && host.navigation.src !== "about:blank") {
    host.navigation.src = previewReloadUrl(host, host.navigation.src);
    return true;
  }
  return false;
}

export function fetchDomTreeFromPreview(host: PreviewControllerHost) {
  const url = host.navigation.src;
  if (!url || url === "about:blank") return;
  const projectRoot = host.session.sessionProjectRoot.trim();
  const runtimeSessionId = host.session.kernelProjectSessionId.trim();
  if (!projectRoot || !runtimeSessionId) return;
  const lease = {
    projectRoot,
    runtimeSessionId,
    projectSessionEpoch: host.session.projectSessionEpoch,
    serial: ++host.session.previewDomTreeSerial,
  };
  const leaseMatches = () => (
    host.session.previewDomTreeSerial === lease.serial
    && host.session.sessionProjectRoot === lease.projectRoot
    && host.session.kernelProjectSessionId === lease.runtimeSessionId
    && host.session.projectSessionEpoch === lease.projectSessionEpoch
    && host.navigation.src === url
  );
  if (host.timers.domTreeFetch !== null) {
    window.clearTimeout(host.timers.domTreeFetch);
  }
  host.timers.domTreeFetch = window.setTimeout(() => {
    host.timers.domTreeFetch = null;
    if (!leaseMatches()) return;
    readPreviewDocument(url)
      .then((html) => {
        if (!leaseMatches()) return;
        const parser = new DOMParser();
        const doc = parser.parseFromString(html, "text/html");
        const sections = collectDomTree(doc);
        if (!leaseMatches()) return;
        host.sections.set(sections);
      })
      .catch(() => {});
  }, 150);
}
