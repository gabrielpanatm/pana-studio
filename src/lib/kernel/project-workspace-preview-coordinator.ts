import type { PreviewRefreshReason } from "$lib/preview/controlled";
import {
  projectProjectWorkspacePreview,
  readProjectWorkspaceState,
  requireProjectPreviewMutationReceipt,
  type CanvasProjectionPlan,
  type ProjectWorkspacePreviewRequest,
} from "$lib/project/io";
import type { ProjectWorkspaceSnapshot, SaveState } from "$lib/types";
import { errorMessage } from "$lib/util";

const MUTATION_DEBOUNCE_MS = 120;

type ProjectWorkspacePreviewIdentityHost = {
  sessionProjectRoot: string;
  kernelProjectSessionId: string;
  scannedProject: { isZola: boolean } | null;
  previewWorkspaceRevision: string | null;
  pendingCanvasProjection: CanvasProjectionPlan | null;
  /**
   * A ProjectWorkspace revision remains authoritative even while the Design
   * Safe iframe is unmounted. In that state projection is deferred until a
   * mounted surface can emit the Canvas phase receipts.
   */
  canProjectWorkspacePreview?: () => boolean;
  templateWorkbenchActive?: boolean;
  reprojectActiveTemplateWorkbench?: (minimumWorkspaceRevision: number) => Promise<boolean>;
  setGlobalStatus?: (text: string, kind: SaveState) => void;
};

export type ProjectWorkspacePreviewHost<
  TReason extends PreviewRefreshReason = PreviewRefreshReason,
> = ProjectWorkspacePreviewIdentityHost & {
  requestPreviewRefresh: (reason: TReason) => Promise<boolean>;
  requestWorkspaceProjectionPreviewRefresh?: (reason: TReason) => Promise<boolean>;
};

export type ProjectWorkspacePreviewProjectionOptions<
  TReason extends PreviewRefreshReason = PreviewRefreshReason,
> = {
  reason: TReason;
  minimumWorkspaceRevision?: number;
  requestedPaths?: string[];
  force?: boolean;
  expectedWorkspaceRevision?: number;
  expectedWorkspaceTransactionId?: string;
  onCanvasPlanPrepared?: (plan: CanvasProjectionPlan) => void;
};

type ProjectionIdentity = {
  projectRoot: string;
  sessionId: string;
  generation: number;
};

let activeKey = "";
let activeGeneration = 0;
let projectedEvidence: {
  workspaceRevision: number;
  canvasPlan: CanvasProjectionPlan | null;
} | null = null;
let projectionTail: Promise<void> = Promise.resolve();
let templateWorkbenchProjectionTail: Promise<void> = Promise.resolve();
let scheduledTimer: ReturnType<typeof setTimeout> | null = null;
let scheduledHost: ProjectWorkspacePreviewHost | null = null;
let scheduledReason: PreviewRefreshReason = "workspace-mutation";
let scheduledMinimumWorkspaceRevision: number | undefined;

function identityKey(projectRoot: string, sessionId: string) {
  return `${projectRoot}\u0000${sessionId}`;
}

function captureIdentity(host: ProjectWorkspacePreviewIdentityHost): ProjectionIdentity | null {
  const projectRoot = host.sessionProjectRoot.trim();
  const sessionId = host.kernelProjectSessionId.trim();
  if (!projectRoot || !sessionId || !host.scannedProject?.isZola) return null;
  const key = identityKey(projectRoot, sessionId);
  if (activeKey !== key) {
    activeKey = key;
    activeGeneration += 1;
    projectedEvidence = null;
  }
  return { projectRoot, sessionId, generation: activeGeneration };
}

function identityIsCurrent(
  host: ProjectWorkspacePreviewIdentityHost,
  identity: ProjectionIdentity,
) {
  return identity.generation === activeGeneration
    && identityKey(host.sessionProjectRoot, host.kernelProjectSessionId) === activeKey
    && host.scannedProject?.isZola === true;
}

function requireSnapshotIdentity(
  snapshot: ProjectWorkspaceSnapshot | null,
  identity: ProjectionIdentity,
): ProjectWorkspaceSnapshot {
  if (
    !snapshot
    || snapshot.projectRoot !== identity.projectRoot
    || snapshot.runtimeSessionId !== identity.sessionId
  ) {
    throw new Error(
      "Coordonatorul Preview a refuzat un snapshot ProjectWorkspace din altă sesiune.",
    );
  }
  return snapshot;
}

function normalizePaths(paths: string[] | undefined) {
  return [...new Set(
    (paths ?? []).map((path) => path.trim()).filter(Boolean),
  )].sort();
}

function canvasProjectionSurfaceAvailable(host: ProjectWorkspacePreviewIdentityHost) {
  return host.canProjectWorkspacePreview?.() !== false;
}

async function projectLatestWorkspaceRevision<TReason extends PreviewRefreshReason>(
  host: ProjectWorkspacePreviewHost<TReason>,
  identity: ProjectionIdentity,
  options: ProjectWorkspacePreviewProjectionOptions<TReason>,
): Promise<number | undefined> {
  const minimumRevision = options.minimumWorkspaceRevision;
  if (
    minimumRevision !== undefined
    && (!Number.isSafeInteger(minimumRevision) || minimumRevision < 0)
  ) {
    throw new Error("Coordonatorul Preview a primit o revizie minimă invalidă.");
  }

  let attempts = 0;
  while (identityIsCurrent(host, identity)) {
    // Do not materialize a Rust candidate that no mounted Canvas can confirm.
    // projectedEvidence intentionally stays behind, so the next request made
    // with a mounted surface will project the latest workspace revision.
    if (!canvasProjectionSurfaceAvailable(host)) return;
    attempts += 1;
    if (attempts > 32) {
      throw new Error(
        "Coordonatorul Preview nu a putut stabiliza ProjectWorkspace după 32 de revizii consecutive.",
      );
    }

    const snapshot = requireSnapshotIdentity(
      await readProjectWorkspaceState(),
      identity,
    );
    if (!identityIsCurrent(host, identity)) return;
    if (minimumRevision !== undefined && snapshot.revision < minimumRevision) {
      throw new Error(
        `ProjectWorkspace este la revizia ${snapshot.revision}, sub revizia minimă cerută ${minimumRevision}.`,
      );
    }
    if (!options.force && projectedEvidence?.workspaceRevision === snapshot.revision) {
      const cachedPlan = projectedEvidence.canvasPlan;
      if (
        options.expectedWorkspaceRevision !== undefined
        && options.expectedWorkspaceRevision !== snapshot.revision
      ) {
        throw new Error("Cache-ul Preview nu corespunde reviziei ProjectWorkspace așteptate.");
      }
      if (
        options.expectedWorkspaceTransactionId !== undefined
        && cachedPlan?.workspaceTransactionId !== options.expectedWorkspaceTransactionId
      ) {
        throw new Error(
          "Cache-ul Preview nu dovedește tranzacția ProjectWorkspace așteptată.",
        );
      }
      if (cachedPlan) options.onCanvasPlanPrepared?.(cachedPlan);
      return snapshot.revision;
    }

    const input: ProjectWorkspacePreviewRequest = {
      expectedProjectRoot: identity.projectRoot,
      expectedSessionId: identity.sessionId,
      expectedWorkspaceRevision: snapshot.revision,
      requestedPaths: normalizePaths(options.requestedPaths),
    };

    try {
      const receipt = await projectProjectWorkspacePreview(input);
      requireProjectPreviewMutationReceipt(input, receipt);
      if (
        options.expectedWorkspaceRevision !== undefined
        && (
          receipt.workspaceRevision !== options.expectedWorkspaceRevision
          || receipt.canvasProjection?.identity.workspaceRevision !== options.expectedWorkspaceRevision
        )
      ) {
        throw new Error(
          "Proiecția Canvas nu corespunde reviziei fast patch-ului confirmat.",
        );
      }
      if (
        options.expectedWorkspaceTransactionId !== undefined
        && receipt.canvasProjection?.workspaceTransactionId
          !== options.expectedWorkspaceTransactionId
      ) {
        throw new Error(
          "Proiecția Canvas nu aparține tranzacției ProjectWorkspace a fast patch-ului.",
        );
      }
      const confirmedPaths = new Set(receipt.requestedPaths);
      if (!input.requestedPaths.every((path) => confirmedPaths.has(path))) {
        throw new Error(
          "Preview workspace nu a confirmat toate resursele cerute pentru proiecție.",
        );
      }
      if (!identityIsCurrent(host, identity)) return;
      if (!receipt.previewRevision) {
        projectedEvidence = {
          workspaceRevision: receipt.workspaceRevision,
          canvasPlan: null,
        };
        return receipt.workspaceRevision;
      }
      if (!receipt.canvasProjection) {
        throw new Error("Preview workspace a publicat o revizie fără plan Canvas.");
      }
      // The surface may have been unmounted while Rust built the candidate.
      // Leave the revision unpublished in the UI and retry it on the next
      // mounted-surface request instead of starting a guaranteed timeout.
      if (!canvasProjectionSurfaceAvailable(host)) return;
      options.onCanvasPlanPrepared?.(receipt.canvasProjection);
      host.previewWorkspaceRevision = receipt.previewRevision;
      host.pendingCanvasProjection = receipt.canvasProjection;
      const refreshed = host.templateWorkbenchActive && host.reprojectActiveTemplateWorkbench
        ? await projectLatestActiveTemplateWorkbench(host, identity, receipt.workspaceRevision)
        : await (host.requestWorkspaceProjectionPreviewRefresh
            ? host.requestWorkspaceProjectionPreviewRefresh(options.reason)
            : host.requestPreviewRefresh(options.reason));
      if (!identityIsCurrent(host, identity)) return;
      if (!refreshed) {
        host.pendingCanvasProjection = null;
        throw new Error(
          "Preview-ul nu a confirmat generația ProjectWorkspace publicată de Rust.",
        );
      }
      projectedEvidence = {
        workspaceRevision: receipt.workspaceRevision,
        canvasPlan: receipt.canvasProjection,
      };
      return receipt.workspaceRevision;
    } catch (error) {
      if (!identityIsCurrent(host, identity)) return;
      const latest = requireSnapshotIdentity(
        await readProjectWorkspaceState(),
        identity,
      );
      if (latest.revision !== snapshot.revision) {
        // Mutația a fost depășită în timpul materializării. Proiectăm direct
        // ultima revizie; generația candidată veche este eliminată în Rust.
        continue;
      }
      throw error;
    }
  }
}

export function projectLatestProjectWorkspacePreview<TReason extends PreviewRefreshReason>(
  host: ProjectWorkspacePreviewHost<TReason>,
  options: ProjectWorkspacePreviewProjectionOptions<TReason>,
): Promise<void> {
  const identity = captureIdentity(host);
  if (!identity) return Promise.resolve();
  if (
    scheduledHost === host
    && (
      scheduledMinimumWorkspaceRevision === undefined
      || (options.minimumWorkspaceRevision ?? -1) >= scheduledMinimumWorkspaceRevision
    )
  ) {
    if (scheduledTimer !== null) clearTimeout(scheduledTimer);
    scheduledTimer = null;
    scheduledHost = null;
    scheduledMinimumWorkspaceRevision = undefined;
  }
  const canonicalTask = projectionTail
    .catch(() => undefined)
    .then(() => projectLatestWorkspaceRevision(host, identity, options));
  projectionTail = canonicalTask.then(() => undefined).catch(() => undefined);
  return canonicalTask.then(() => undefined);
}

function projectLatestActiveTemplateWorkbench(
  host: ProjectWorkspacePreviewIdentityHost,
  identity: ProjectionIdentity,
  minimumWorkspaceRevision: number,
): Promise<boolean> {
  if (!host.templateWorkbenchActive || !host.reprojectActiveTemplateWorkbench) {
    return Promise.resolve(false);
  }
  const task = templateWorkbenchProjectionTail
    .catch(() => undefined)
    .then(async () => {
      if (!identityIsCurrent(host, identity) || !host.templateWorkbenchActive) return false;
      return await host.reprojectActiveTemplateWorkbench?.(minimumWorkspaceRevision) === true;
    });
  templateWorkbenchProjectionTail = task.then(() => undefined).catch(() => undefined);
  return task;
}

export function scheduleProjectWorkspaceDerivedPreviewProjection(
  host: ProjectWorkspacePreviewHost,
  reason: PreviewRefreshReason = "workspace-mutation",
  minimumWorkspaceRevision?: number,
) {
  const identity = captureIdentity(host);
  if (!identity) return;
  if (
    minimumWorkspaceRevision !== undefined
    && (!Number.isSafeInteger(minimumWorkspaceRevision) || minimumWorkspaceRevision < 0)
  ) {
    throw new Error("Coordonatorul Preview a primit o revizie minimă invalidă.");
  }
  scheduledHost = host;
  scheduledReason = reason;
  if (minimumWorkspaceRevision !== undefined) {
    scheduledMinimumWorkspaceRevision = Math.max(
      scheduledMinimumWorkspaceRevision ?? 0,
      minimumWorkspaceRevision,
    );
  }
  if (scheduledTimer !== null) clearTimeout(scheduledTimer);
  scheduledTimer = setTimeout(() => {
    scheduledTimer = null;
    const target = scheduledHost;
    const minimumRevision = scheduledMinimumWorkspaceRevision;
    scheduledHost = null;
    scheduledMinimumWorkspaceRevision = undefined;
    if (!target || !identityIsCurrent(target, identity)) return;
    void projectLatestProjectWorkspacePreview(target, {
      reason: scheduledReason,
      minimumWorkspaceRevision: minimumRevision,
    })
      .catch((error) => {
        if (!identityIsCurrent(target, identity)) return;
        target.setGlobalStatus?.(
          `Proiecția Preview/Template Workbench a ProjectWorkspace a eșuat: ${errorMessage(error)}`,
          "error",
        );
      });
  }, MUTATION_DEBOUNCE_MS);
}

export function markProjectWorkspacePreviewPublished(
  projectRoot: string,
  sessionId: string,
  workspaceRevision: number,
  canvasPlan: CanvasProjectionPlan | null = null,
) {
  if (!projectRoot.trim() || !sessionId.trim() || !Number.isSafeInteger(workspaceRevision)) return;
  const key = identityKey(projectRoot, sessionId);
  if (activeKey !== key) {
    activeKey = key;
    activeGeneration += 1;
  }
  projectedEvidence = { workspaceRevision, canvasPlan };
}

export function resetProjectWorkspacePreviewCoordinator() {
  activeKey = "";
  activeGeneration += 1;
  projectedEvidence = null;
  scheduledHost = null;
  scheduledMinimumWorkspaceRevision = undefined;
  if (scheduledTimer !== null) clearTimeout(scheduledTimer);
  scheduledTimer = null;
}
