import {
  createCssRequestIdentity,
  getScssVariables,
  readCurrentProjectDiskManifest,
  readProjectWorkspaceState,
  reconcileCleanExternalProjectFiles,
  scanProject,
  type CanvasProjectionPlan,
} from "$lib/project/io";
import { projectLatestProjectWorkspacePreview } from "$lib/kernel/project-workspace-preview-coordinator";
import { diffDiskManifests } from "$lib/project/disk-manifest";
import { preservePreviewBaseUrl } from "$lib/project/session";
import {
  acceptedExternalReconcileManifest,
  externalReconcileUiLeaseMatches,
  projectExternalReconcileSources,
  type ExternalReconcileUiLease,
} from "$lib/project/external-reconcile-projection";
import {
  invalidateFileBufferDraftSyncCursor,
} from "$lib/session/file-buffer-draft-sync";
import { flushWorkspaceMutationInputs } from "$lib/session/workspace-mutation-coordinator";
import type {
  ExternalDiskState,
  KernelExternalDiskReconcileReceipt,
  ProjectDiskManifest,
  ProjectDiskManifestEntry,
  ProjectScan,
  ProjectWorkspaceSnapshot,
  SaveState,
  ScssVariable,
} from "$lib/types";
import { errorMessage } from "$lib/util";

const ACTIVE_CHECK_INTERVAL = 2000;
const BACKGROUND_CHECK_INTERVAL = 8000;
const EXTERNAL_PROJECTION_DEADLINE_MS = 30_000;
export const EXTERNAL_CHANGE_NOTIFICATION_ID = "project.external-disk-change";
export const EXTERNAL_CHANGE_RELOAD_ACTION_ID = "external-disk.reload";
export const EXTERNAL_CHANGE_KEEP_SESSION_ACTION_ID = "external-disk.keep-session";
let externalReconcileGeneration = 0;

export function createExternalDiskState(): ExternalDiskState {
  return {
    baseline: null,
    reconciling: false,
    changed: false,
    changedFiles: [],
    activeFileChanged: false,
    previewRelevantChanged: false,
    blockedByDirtySession: false,
    lastDetectedAt: null,
    lastDetectedFiles: [],
    lastDetectedActiveFileChanged: false,
    lastDetectedPreviewRelevantChanged: false,
    lastAppliedAt: null,
    lastAppliedFiles: [],
    lastCheckedAt: null,
    checking: false,
    workspaceProjectionRecoveryRequired: false,
    truncated: false,
  };
}

export type ExternalDiskControllerHost = {
  sessionProjectRoot: string;
  externalDiskState: ExternalDiskState;
  externalDiskTimer: number | null;
  externalDiskSuspended: boolean;
  externalDiskCheckInFlight: ExternalDiskCheckInFlight | null;
  externalDiskCheckGeneration: number;
  projectTransitionFrontendLeaseActive: boolean;
  kernelUndoRedoFrontendLeaseActive?: boolean;
  aiEditLeaseFrontendLockActive: boolean;
  scannedProject: ProjectScan | null;
  activeScannedPath: string | null;
  source: string;
  sourceCache: Record<string, string>;
  projectSessionEpoch: number;
  kernelProjectSessionId: string;
  projectWorkspaceSnapshot: ProjectWorkspaceSnapshot | null;
  editorMutationEpoch: number;
  selectionEpoch: number;
  refreshToken: number;
  jsRefreshToken: number;
  previewWorkspaceRevision: string | null;
  pendingCanvasProjection: CanvasProjectionPlan | null;
  scssVariables: ScssVariable[];
  globalDirtyState: {
    dirty: boolean;
  };
  projectStatus: string;
  setGlobalStatus: (text: string, kind: SaveState) => void;
  notify: (notification: {
    id: string;
    level: "info" | "warning" | "error";
    title: string;
    message: string;
    actionLabel?: string | null;
    actionId?: string | null;
    secondaryActionLabel?: string | null;
    secondaryActionId?: string | null;
  }) => void;
  clearNotification: (id: string) => void;
  refreshSourceGraph?: (options?: { strict?: boolean }) => Promise<void>;
  quiesceExternalReconcileInteractions: () => void;
  waitForExternalReconcileInteractionLock: () => Promise<void>;
  resetHistoryAfterExternalReconcile: () => Promise<void>;
  requestPreviewRefresh: (reason: "external-change") => Promise<boolean>;
};

type ExternalDiskCheckLease = {
  projectRoot: string;
  runtimeSessionId: string;
  projectSessionEpoch: number;
  generation: number;
};

type ExternalDiskCheckInFlight = ExternalDiskCheckLease & {
  promise: Promise<void>;
};

export function startExternalDiskPolling(host: ExternalDiskControllerHost) {
  stopExternalDiskPolling(host);
  if (
    host.externalDiskSuspended ||
    host.projectTransitionFrontendLeaseActive ||
    host.kernelUndoRedoFrontendLeaseActive ||
    host.externalDiskState.workspaceProjectionRecoveryRequired ||
    !host.externalDiskState.baseline ||
    host.externalDiskState.baseline.truncated
  ) return;
  scheduleNextExternalDiskCheck(host, 300);
}

export function stopExternalDiskPolling(host: ExternalDiskControllerHost) {
  if (host.externalDiskTimer !== null && typeof window !== "undefined") {
    window.clearTimeout(host.externalDiskTimer);
  }
  host.externalDiskTimer = null;
}

/**
 * Suspends future monitor ticks and drains the exact check which may already
 * be waiting on disk or Rust. Every project writer (Save, Undo/Redo or Project
 * Transition) must await this barrier before its first persistent effect;
 * stopping the timer alone cannot cancel an async callback which has already
 * started.
 */
export async function suspendAndDrainExternalDiskMonitoring(
  host: ExternalDiskControllerHost,
) {
  host.externalDiskSuspended = true;
  stopExternalDiskPolling(host);

  // Invalidate a check still in its read-only manifest phase. A check which
  // has already entered Rust reconcile remains owned by its tracked Promise
  // and is awaited to its normal terminal state below.
  host.externalDiskCheckGeneration += 1;
  const inFlight = host.externalDiskCheckInFlight;
  if (inFlight && externalDiskCheckBelongsToCurrentSession(host, inFlight)) {
    await inFlight.promise;
  } else if (inFlight && host.externalDiskCheckInFlight === inFlight) {
    host.externalDiskCheckInFlight = null;
  }

  // A pre-existing implementation could have left `checking` set after a
  // generation invalidation. Once the tracked Promise is drained and no
  // reconcile owns the state, clearing it is safe and deterministic.
  if (host.externalDiskState.checking && !host.externalDiskState.reconciling) {
    finishSuspendedCheck(host);
  }
  if (
    host.externalDiskCheckInFlight
    && externalDiskCheckBelongsToCurrentSession(
      host,
      host.externalDiskCheckInFlight,
    )
  ) {
    throw new Error(
      "Monitorul extern a pornit o verificare nouă după rezervarea scrierii proiectului.",
    );
  }
  if (host.externalDiskState.checking || host.externalDiskState.reconciling) {
    throw new Error(
      "Monitorul extern nu a ajuns într-o stare terminală înaintea scrierii proiectului.",
    );
  }
}

export function resumeExternalDiskMonitoringAfterSave(
  host: ExternalDiskControllerHost,
) {
  host.externalDiskSuspended = false;
  if (
    host.projectTransitionFrontendLeaseActive
    || host.kernelUndoRedoFrontendLeaseActive
    || !host.scannedProject?.isZola
  ) return;
  startExternalDiskPolling(host);
}

export function resumeExternalDiskMonitoringAfterTransitionLease(
  host: ExternalDiskControllerHost,
) {
  if (
    host.projectTransitionFrontendLeaseActive
    || host.kernelUndoRedoFrontendLeaseActive
  ) return;
  host.externalDiskSuspended = false;
  if (!host.scannedProject?.isZola) return;
  startExternalDiskPolling(host);
}

export function resetExternalDiskState(host: ExternalDiskControllerHost) {
  externalReconcileGeneration += 1;
  host.projectSessionEpoch += 1;
  detachExternalDiskCheck(host);
  stopExternalDiskPolling(host);
  host.externalDiskState = createExternalDiskState();
  host.clearNotification(EXTERNAL_CHANGE_NOTIFICATION_ID);
}

export function invalidateExternalReconcileForProjectTransition(
  host: ExternalDiskControllerHost,
) {
  externalReconcileGeneration += 1;
  host.projectSessionEpoch += 1;
  detachExternalDiskCheck(host);
  stopExternalDiskPolling(host);
  const reconcileMayHaveCommitted = host.externalDiskState.reconciling;
  host.externalDiskState = {
    ...host.externalDiskState,
    reconciling: true,
    checking: false,
    changed: reconcileMayHaveCommitted || host.externalDiskState.changed,
    blockedByDirtySession:
      reconcileMayHaveCommitted || host.externalDiskState.blockedByDirtySession,
    workspaceProjectionRecoveryRequired:
      reconcileMayHaveCommitted || host.externalDiskState.workspaceProjectionRecoveryRequired,
  };
  host.quiesceExternalReconcileInteractions();
}

export function resumeExternalMonitoringAfterFailedTransition(
  host: ExternalDiskControllerHost,
) {
  externalReconcileGeneration += 1;
  host.projectSessionEpoch += 1;
  detachExternalDiskCheck(host);
  host.externalDiskState = {
    ...host.externalDiskState,
    reconciling: false,
    checking: false,
  };
  if (
    host.scannedProject?.isZola &&
    !host.externalDiskState.workspaceProjectionRecoveryRequired
  ) {
    startExternalDiskPolling(host);
  }
}

export function markWorkspaceProjectionRecoveryRequired(
  host: ExternalDiskControllerHost,
  message: string,
) {
  externalReconcileGeneration += 1;
  host.projectSessionEpoch += 1;
  detachExternalDiskCheck(host);
  stopExternalDiskPolling(host);
  host.externalDiskState = {
    ...host.externalDiskState,
    reconciling: false,
    checking: false,
    changed: true,
    blockedByDirtySession: true,
    workspaceProjectionRecoveryRequired: true,
  };
  host.notify({
    id: EXTERNAL_CHANGE_NOTIFICATION_ID,
    level: "error",
    title: "Interfața necesită reproiectare",
    message,
    actionLabel: "Reîncarcă de pe disk",
    actionId: EXTERNAL_CHANGE_RELOAD_ACTION_ID,
  });
}

export async function establishExternalDiskBaseline(host: ExternalDiskControllerHost) {
  if (!host.scannedProject?.isZola) return;
  if (
    host.externalDiskState.checking ||
    host.externalDiskState.reconciling ||
    host.externalDiskState.workspaceProjectionRecoveryRequired
  ) return;
  const expectedRoot = host.scannedProject.root;
  const manifest = host.scannedProject.acceptedDiskManifest;
  const acceptedDiskGeneration = host.scannedProject.acceptedDiskGeneration;
  if (
    !manifest ||
    manifest.root !== expectedRoot ||
    manifest.truncated ||
    !Number.isSafeInteger(acceptedDiskGeneration) ||
    (acceptedDiskGeneration ?? 0) < 1 ||
    !host.scannedProject.kernelSessionId ||
    host.scannedProject.kernelSessionId !== host.kernelProjectSessionId
  ) {
    preserveUninitializedExternalMonitor(host, expectedRoot);
    return;
  }
  host.externalDiskState = {
    ...createExternalDiskState(),
    baseline: manifest,
    lastCheckedAt: Date.now(),
    truncated: manifest.truncated,
  };
  host.clearNotification(EXTERNAL_CHANGE_NOTIFICATION_ID);
}

/**
 * Publishes the exact disk baseline already accepted by the Rust Save
 * transaction. The external monitor is only a read projection of that
 * authority and must observe this acknowledgement before polling resumes;
 * otherwise it can misclassify the application's own Save as external.
 */
export function acceptProjectWorkspaceSaveBaseline(
  host: ExternalDiskControllerHost,
  acceptedManifest: ProjectDiskManifest,
  acceptedDiskGeneration: number,
) {
  const project = host.scannedProject;
  if (
    !project
    || project.root !== acceptedManifest.root
    || project.kernelSessionId !== host.kernelProjectSessionId
    || !Number.isSafeInteger(acceptedDiskGeneration)
    || acceptedDiskGeneration < 1
    || acceptedManifest.truncated
  ) {
    throw new Error(
      "Baseline-ul Save nu poate fi publicat în monitorul extern pentru alt proiect, altă sesiune sau un manifest invalid.",
    );
  }

  // Save owns the suspended monitor boundary. Invalidate any scheduled lease
  // before replacing its baseline, then publish the Rust receipt atomically
  // to both frontend projections.
  host.externalDiskCheckGeneration += 1;
  stopExternalDiskPolling(host);
  host.scannedProject = {
    ...project,
    acceptedDiskGeneration,
    acceptedDiskManifest: acceptedManifest,
  };
  host.externalDiskState = {
    ...createExternalDiskState(),
    baseline: acceptedManifest,
    lastCheckedAt: Date.now(),
    truncated: false,
  };
  host.clearNotification(EXTERNAL_CHANGE_NOTIFICATION_ID);
}

async function checkExternalDisk(
  host: ExternalDiskControllerHost,
  checkLease: ExternalDiskCheckLease,
) {
  if (
    !host.scannedProject?.isZola ||
    host.externalDiskSuspended ||
    host.projectTransitionFrontendLeaseActive ||
    host.kernelUndoRedoFrontendLeaseActive ||
    host.externalDiskState.checking ||
    host.externalDiskState.reconciling ||
    host.externalDiskState.workspaceProjectionRecoveryRequired
  ) return;
  if (!externalDiskCheckLeaseMatches(host, checkLease)) return;
  const expectedRoot = checkLease.projectRoot;
  const expectedSessionEpoch = checkLease.projectSessionEpoch;
  const reconcileGenerationAtStart = externalReconcileGeneration;
  host.externalDiskState = { ...host.externalDiskState, checking: true };

  try {
    if (host.externalDiskSuspended) {
      finishSuspendedCheck(host);
      return;
    }

    const current = await readCurrentProjectDiskManifest();
    if (
      !externalDiskCheckLeaseMatches(host, checkLease) ||
      reconcileGenerationAtStart !== externalReconcileGeneration ||
      host.externalDiskState.reconciling ||
      host.externalDiskState.workspaceProjectionRecoveryRequired ||
      host.projectSessionEpoch !== expectedSessionEpoch ||
      host.scannedProject?.root !== expectedRoot ||
      current.root !== expectedRoot
    ) return;
    if (current.truncated || host.externalDiskState.baseline?.truncated) {
      preserveUninitializedExternalMonitor(host, current.root);
      return;
    }
    if (host.externalDiskSuspended) {
      finishSuspendedCheck(host);
      return;
    }

    if (
      !host.externalDiskState.baseline ||
      host.externalDiskState.baseline.root !== current.root
    ) {
      preserveUninitializedExternalMonitor(host, current.root);
      return;
    }

    const diff = diffDiskManifests(host.externalDiskState.baseline, current);
    const changed = diff.changedFiles.length > 0;
    const activeFileChanged = Boolean(
      host.activeScannedPath && diff.changedFiles.includes(host.activeScannedPath),
    );
    const blockedByDirtySession = changed && host.globalDirtyState.dirty;

    host.externalDiskState = {
      // A changed manifest is only accepted after the Rust reconcile receipt.
      baseline: changed ? host.externalDiskState.baseline : current,
      reconciling: false,
      changed,
      changedFiles: diff.changedFiles,
      activeFileChanged,
      previewRelevantChanged: diff.previewRelevantChanged,
      blockedByDirtySession,
      lastDetectedAt: changed ? Date.now() : host.externalDiskState.lastDetectedAt,
      lastDetectedFiles: changed ? diff.changedFiles : host.externalDiskState.lastDetectedFiles,
      lastDetectedActiveFileChanged: changed ? activeFileChanged : host.externalDiskState.lastDetectedActiveFileChanged,
      lastDetectedPreviewRelevantChanged: changed
        ? diff.previewRelevantChanged
        : host.externalDiskState.lastDetectedPreviewRelevantChanged,
      lastAppliedAt: host.externalDiskState.lastAppliedAt,
      lastAppliedFiles: host.externalDiskState.lastAppliedFiles,
      lastCheckedAt: Date.now(),
      checking: false,
      workspaceProjectionRecoveryRequired: changed
        ? host.externalDiskState.workspaceProjectionRecoveryRequired
        : false,
      truncated: current.truncated,
    };

    if (!changed) {
      host.clearNotification(EXTERNAL_CHANGE_NOTIFICATION_ID);
      return;
    }

    if (blockedByDirtySession) {
      notifyBlockedExternalChange(host, diff.changedFiles);
      return;
    }

    await applyCleanExternalChanges(host, current, diff.changedFiles, {
      activeFileChanged,
      previewRelevantChanged: diff.previewRelevantChanged,
    });
  } catch (error) {
    if (
      !externalDiskCheckLeaseMatches(host, checkLease) ||
      reconcileGenerationAtStart !== externalReconcileGeneration ||
      host.projectSessionEpoch !== expectedSessionEpoch ||
      host.scannedProject?.root !== expectedRoot
    ) return;
    host.externalDiskState = {
      ...host.externalDiskState,
      checking: false,
      lastCheckedAt: Date.now(),
    };
    host.projectStatus = `Monitorizarea fișierelor a eșuat: ${errorMessage(error)}`;
  }
}

function finishSuspendedCheck(host: ExternalDiskControllerHost) {
  host.externalDiskState = {
    ...host.externalDiskState,
    checking: false,
    lastCheckedAt: Date.now(),
  };
}

function scheduleNextExternalDiskCheck(host: ExternalDiskControllerHost, delay?: number) {
  if (typeof window === "undefined") return;
  stopExternalDiskPolling(host);
  const scheduledLease = currentExternalDiskCheckLease(host);
  if (!scheduledLease) return;
  const focused = typeof document === "undefined" ? true : document.hasFocus();
  const nextDelay = delay ?? (focused ? ACTIVE_CHECK_INTERVAL : BACKGROUND_CHECK_INTERVAL);
  let timerId: number | null = null;
  timerId = window.setTimeout(async () => {
    if (
      host.externalDiskTimer !== timerId
      || host.externalDiskSuspended
      || host.projectTransitionFrontendLeaseActive
      || host.kernelUndoRedoFrontendLeaseActive
      || !externalDiskCheckLeaseMatches(host, scheduledLease)
    ) return;
    const completedLease = await runTrackedExternalDiskCheck(host, scheduledLease);
    if (
      !completedLease
      || !externalDiskCheckLeaseMatches(host, completedLease)
    ) return;
    if (
      host.scannedProject?.isZola &&
      !host.externalDiskSuspended &&
      !host.projectTransitionFrontendLeaseActive &&
      !host.kernelUndoRedoFrontendLeaseActive &&
      !host.externalDiskState.workspaceProjectionRecoveryRequired
    ) {
      scheduleNextExternalDiskCheck(host);
    } else {
      host.externalDiskTimer = null;
    }
  }, nextDelay);
  host.externalDiskTimer = timerId;
}

async function runTrackedExternalDiskCheck(
  host: ExternalDiskControllerHost,
  scheduledLease: ExternalDiskCheckLease,
): Promise<ExternalDiskCheckLease | null> {
  if (!externalDiskCheckLeaseMatches(host, scheduledLease)) return null;
  const existing = host.externalDiskCheckInFlight;
  if (existing) {
    if (!externalDiskCheckBelongsToCurrentSession(host, existing)) {
      if (host.externalDiskCheckInFlight === existing) {
        host.externalDiskCheckInFlight = null;
      }
      return null;
    }
    await existing.promise;
    return externalDiskCheckLeaseMatches(host, existing) ? existing : null;
  }

  const checkGeneration = host.externalDiskCheckGeneration + 1;
  host.externalDiskCheckGeneration = checkGeneration;
  const checkLease: ExternalDiskCheckLease = {
    projectRoot: scheduledLease.projectRoot,
    runtimeSessionId: scheduledLease.runtimeSessionId,
    projectSessionEpoch: scheduledLease.projectSessionEpoch,
    generation: checkGeneration,
  };
  const operation = checkExternalDisk(host, checkLease);
  const tracked: ExternalDiskCheckInFlight = { ...checkLease, promise: operation };
  host.externalDiskCheckInFlight = tracked;
  try {
    await operation;
  } finally {
    if (host.externalDiskCheckInFlight === tracked) {
      host.externalDiskCheckInFlight = null;
    }
    if (
      host.externalDiskSuspended
      && externalDiskCheckBelongsToCurrentSession(host, tracked)
      && host.externalDiskState.checking
      && !host.externalDiskState.reconciling
    ) {
      finishSuspendedCheck(host);
    }
  }
  return externalDiskCheckLeaseMatches(host, checkLease) ? checkLease : null;
}

function currentExternalDiskCheckLease(
  host: ExternalDiskControllerHost,
): ExternalDiskCheckLease | null {
  const project = host.scannedProject;
  if (
    !project?.isZola
    || !project.root
    || !host.kernelProjectSessionId
  ) return null;
  return {
    projectRoot: project.root,
    runtimeSessionId: host.kernelProjectSessionId,
    projectSessionEpoch: host.projectSessionEpoch,
    generation: host.externalDiskCheckGeneration,
  };
}

function externalDiskCheckBelongsToCurrentSession(
  host: ExternalDiskControllerHost,
  lease: ExternalDiskCheckLease,
) {
  return Boolean(
    host.scannedProject?.isZola
    && host.scannedProject.root === lease.projectRoot
    && host.kernelProjectSessionId === lease.runtimeSessionId
    && host.projectSessionEpoch === lease.projectSessionEpoch
  );
}

function externalDiskCheckLeaseMatches(
  host: ExternalDiskControllerHost,
  lease: ExternalDiskCheckLease,
) {
  return externalDiskCheckBelongsToCurrentSession(host, lease)
    && host.externalDiskCheckGeneration === lease.generation;
}

function detachExternalDiskCheck(host: ExternalDiskControllerHost) {
  host.externalDiskCheckGeneration += 1;
  host.externalDiskCheckInFlight = null;
}

async function applyCleanExternalChanges(
  host: ExternalDiskControllerHost,
  current: ProjectDiskManifest,
  changedFiles: string[],
  flags: { activeFileChanged: boolean; previewRelevantChanged: boolean },
) {
  if (!host.scannedProject) return;
  if (
    host.externalDiskState.reconciling ||
    host.externalDiskState.workspaceProjectionRecoveryRequired
  ) return;
  const projectBeforeReconcile = host.scannedProject;
  const expectedRoot = projectBeforeReconcile.root;
  const reconcileGeneration = ++externalReconcileGeneration;
  let rustReceiptAccepted = false;

  host.externalDiskState = {
    ...host.externalDiskState,
    reconciling: true,
    checking: true,
    workspaceProjectionRecoveryRequired: false,
  };
  host.quiesceExternalReconcileInteractions();
  await host.waitForExternalReconcileInteractionLock();

  try {
  await flushWorkspaceMutationInputs("manual");
  if (!isCurrentReconcile(host, expectedRoot, reconcileGeneration)) return;

  const uiLease = currentExternalReconcileUiLease(host, expectedRoot);

  if (host.globalDirtyState.dirty) {
    host.externalDiskState = {
      ...host.externalDiskState,
      baseline: host.externalDiskState.baseline,
      changed: true,
      changedFiles,
      activeFileChanged: flags.activeFileChanged,
      previewRelevantChanged: flags.previewRelevantChanged,
      blockedByDirtySession: true,
      checking: false,
      lastCheckedAt: Date.now(),
    };
    notifyBlockedExternalChange(host, changedFiles);
    return;
  }

  const receipt = await reconcileCleanExternalProjectFiles({
    expectedProjectRoot: expectedRoot,
    expectedSessionId: host.kernelProjectSessionId,
    observedManifest: current,
    relativePaths: changedFiles,
    activeRelativePath: host.activeScannedPath,
  });
  if (!isCurrentReconcile(host, expectedRoot, reconcileGeneration)) return;
  if (
    receipt.projectRoot !== expectedRoot ||
    receipt.sessionId !== host.kernelProjectSessionId
  ) {
    throw new Error("Receipt-ul external reconcile aparține altei sesiuni de proiect.");
  }

  if (receipt.status === "blocked" || receipt.status === "stale_evidence") {
    preserveBlockedReceipt(host, changedFiles, flags, receipt);
    return;
  }
  if (receipt.status === "reload_required") {
    preserveReloadRequiredReceipt(host, changedFiles, flags, receipt);
    return;
  }
  rustReceiptAccepted = true;
  if (receipt.workspaceRevision === null) {
    throw new Error("External reconcile nu a publicat revizia ProjectWorkspace rezultată.");
  }
  const workspaceAfterCommit = await readProjectWorkspaceState();
  if (!isCurrentReconcile(host, expectedRoot, reconcileGeneration)) return;
  if (
    !workspaceAfterCommit
    || workspaceAfterCommit.projectRoot !== expectedRoot
    || workspaceAfterCommit.runtimeSessionId !== host.kernelProjectSessionId
    || workspaceAfterCommit.revision !== receipt.workspaceRevision
    || workspaceAfterCommit.dirty
  ) {
    throw new Error(
      "Snapshotul ProjectWorkspace nu confirmă exact commit-ul external reconcile.",
    );
  }
  host.projectWorkspaceSnapshot = workspaceAfterCommit;

  if (!externalReconcileUiLeaseMatches(
    uiLease,
    currentExternalReconcileUiLease(host, expectedRoot),
  )) {
    preserveConcurrentUiMutationAfterCommit(host, changedFiles, flags);
    return;
  }

  const acceptedManifest = acceptedExternalReconcileManifest(receipt, expectedRoot);
  const acceptedDiskGeneration = requireAcceptedExternalDiskGeneration(
    receipt,
    projectBeforeReconcile.acceptedDiskGeneration,
    host.externalDiskState.baseline,
    acceptedManifest,
  );

  for (const relativePath of receipt.invalidatedPaths) {
    invalidateFileBufferDraftSyncCursor(relativePath);
  }
  const sourceProjection = projectExternalReconcileSources(
    host.sourceCache,
    receipt,
    host.activeScannedPath,
    flags.activeFileChanged,
  );
  host.sourceCache = sourceProjection.sourceCache;
  if (sourceProjection.activeSource !== null) host.source = sourceProjection.activeSource;

  // Build the frontend history baseline only after source/sourceCache contain
  // the exact NEW buffer acknowledged by Rust.
  if (receipt.historyInvalidated) {
    await host.resetHistoryAfterExternalReconcile();
    if (!isCurrentReconcile(host, expectedRoot, reconcileGeneration)) return;
  }

  if (receipt.projectionHints.projectRescan) {
    const project = preservePreviewBaseUrl(
      await scanProject(expectedRoot),
      projectBeforeReconcile,
    );
    if (!isCurrentReconcile(host, expectedRoot, reconcileGeneration)) return;
    host.scannedProject = project;
  }
  if (receipt.projectionHints.sourceGraph) {
    if (!receipt.sourceGraphInvalidated) {
      throw new Error("Nucleul nu a confirmat invalidarea cache-ului Source Graph.");
    }
    await host.refreshSourceGraph?.({ strict: true });
    if (!isCurrentReconcile(host, expectedRoot, reconcileGeneration)) return;
  }
  if (receipt.projectionHints.scss) {
    const cssIdentity = createCssRequestIdentity(receipt.projectRoot, receipt.sessionId);
    const nextScssVariables = await getScssVariables(cssIdentity);
    if (
      !isCurrentReconcile(host, expectedRoot, reconcileGeneration)
      || host.scannedProject?.root !== cssIdentity.expectedProjectRoot
      || host.kernelProjectSessionId !== cssIdentity.expectedSessionId
    ) return;
    host.scssVariables = nextScssVariables;
  }
  host.refreshToken += 1;
  if (receipt.projectionHints.pageJs) host.jsRefreshToken += 1;

  if (receipt.projectionHints.preview) {
    await withExternalProjectionDeadline(
      projectLatestProjectWorkspacePreview(host, {
        reason: "external-change",
        minimumWorkspaceRevision: receipt.workspaceRevision,
        requestedPaths: receipt.requestedPaths,
      }),
    );
    if (!isCurrentReconcile(host, expectedRoot, reconcileGeneration)) return;
  }
  if (!isCurrentReconcile(host, expectedRoot, reconcileGeneration)) return;
  if (!externalReconcileUiLeaseMatches(
    uiLease,
    currentExternalReconcileUiLease(host, expectedRoot),
  )) {
    preserveConcurrentUiMutationAfterCommit(host, changedFiles, flags);
    return;
  }

  host.scannedProject = {
    ...host.scannedProject,
    acceptedDiskGeneration,
    acceptedDiskManifest: acceptedManifest,
  };

  host.externalDiskState = {
    baseline: acceptedManifest,
    reconciling: true,
    changed: false,
    changedFiles: [],
    activeFileChanged: false,
    previewRelevantChanged: false,
    blockedByDirtySession: false,
    lastDetectedAt: host.externalDiskState.lastDetectedAt,
    lastDetectedFiles: host.externalDiskState.lastDetectedFiles,
    lastDetectedActiveFileChanged: host.externalDiskState.lastDetectedActiveFileChanged,
    lastDetectedPreviewRelevantChanged: host.externalDiskState.lastDetectedPreviewRelevantChanged,
    lastAppliedAt: Date.now(),
    lastAppliedFiles: changedFiles,
    lastCheckedAt: Date.now(),
    checking: false,
    workspaceProjectionRecoveryRequired: false,
    truncated: acceptedManifest.truncated,
  };
  host.clearNotification(EXTERNAL_CHANGE_NOTIFICATION_ID);
  host.setGlobalStatus(
    `Schimbări externe detectate și reîncărcate: ${formatChangedFiles(changedFiles)}.`,
    "restored",
  );
  } catch (error) {
    if (
      rustReceiptAccepted
      && isCurrentReconcile(host, expectedRoot, reconcileGeneration)
    ) {
      preserveProjectionFailureAfterCommit(host, changedFiles, flags, error);
    }
    throw error;
  } finally {
    if (isCurrentReconcile(host, expectedRoot, reconcileGeneration)) {
      host.externalDiskState = {
        ...host.externalDiskState,
        reconciling: false,
        checking: false,
      };
    }
  }
}

async function withExternalProjectionDeadline(operation: Promise<void>) {
  let timer: ReturnType<typeof setTimeout> | null = null;
  const deadline = new Promise<never>((_resolve, reject) => {
    timer = setTimeout(() => {
      reject(new Error(
        `Proiecția UI nu a ajuns într-o stare terminală în ${EXTERNAL_PROJECTION_DEADLINE_MS / 1000} de secunde.`,
      ));
    }, EXTERNAL_PROJECTION_DEADLINE_MS);
  });
  try {
    await Promise.race([operation, deadline]);
  } finally {
    if (timer !== null) clearTimeout(timer);
  }
}

function currentExternalReconcileUiLease(
  host: ExternalDiskControllerHost,
  projectRoot: string,
): ExternalReconcileUiLease {
  return {
    projectRoot,
    kernelSessionId: host.kernelProjectSessionId,
    projectSessionEpoch: host.projectSessionEpoch,
    activeRelativePath: host.activeScannedPath,
    editorMutationEpoch: host.editorMutationEpoch,
    selectionEpoch: host.selectionEpoch,
  };
}

function isCurrentReconcile(
  host: ExternalDiskControllerHost,
  expectedRoot: string,
  generation: number,
) {
  return generation === externalReconcileGeneration && host.scannedProject?.root === expectedRoot;
}

function preserveConcurrentUiMutationAfterCommit(
  host: ExternalDiskControllerHost,
  changedFiles: string[],
  flags: { activeFileChanged: boolean; previewRelevantChanged: boolean },
) {
  const message =
    "Nucleul a reconciliat disk-ul, dar o intenție de editare sau selecție a apărut în timpul operației. Proiecția a fost oprită înainte să suprascrie UI-ul; reîncărcarea explicită este necesară.";
  host.externalDiskState = {
    ...host.externalDiskState,
    changed: true,
    changedFiles,
    activeFileChanged: flags.activeFileChanged,
    previewRelevantChanged: flags.previewRelevantChanged,
    blockedByDirtySession: true,
    workspaceProjectionRecoveryRequired: true,
    checking: false,
    lastCheckedAt: Date.now(),
  };
  host.notify({
    id: EXTERNAL_CHANGE_NOTIFICATION_ID,
    level: "error",
    title: "Proiecție externă oprită în siguranță",
    message,
    actionLabel: "Reîncarcă de pe disk",
    actionId: EXTERNAL_CHANGE_RELOAD_ACTION_ID,
  });
  host.setGlobalStatus(message, "error");
}

function preserveUninitializedExternalMonitor(
  host: ExternalDiskControllerHost,
  observedRoot: string,
) {
  const message =
    `Monitorul extern nu are un baseline Rust verificat pentru ${observedRoot}. ` +
    "Manifestul observat nu a fost acceptat automat.";
  host.externalDiskState = {
    ...host.externalDiskState,
    changed: true,
    blockedByDirtySession: true,
    checking: false,
    workspaceProjectionRecoveryRequired: true,
    lastCheckedAt: Date.now(),
  };
  host.notify({
    id: EXTERNAL_CHANGE_NOTIFICATION_ID,
    level: "error",
    title: "Baseline extern neverificat",
    message,
    actionLabel: "Reîncarcă de pe disk",
    actionId: EXTERNAL_CHANGE_RELOAD_ACTION_ID,
  });
  host.setGlobalStatus(message, "error");
}

function preserveProjectionFailureAfterCommit(
  host: ExternalDiskControllerHost,
  changedFiles: string[],
  flags: { activeFileChanged: boolean; previewRelevantChanged: boolean },
  error: unknown,
) {
  const message =
    `Nucleul a reconciliat disk-ul, dar proiecția UI nu s-a încheiat: ${errorMessage(error)}. ` +
    "Workspace-ul rămâne blocat până la reîncărcarea explicită de pe disk.";
  host.externalDiskState = {
    ...host.externalDiskState,
    changed: true,
    changedFiles,
    activeFileChanged: flags.activeFileChanged,
    previewRelevantChanged: flags.previewRelevantChanged,
    blockedByDirtySession: true,
    workspaceProjectionRecoveryRequired: true,
    checking: false,
    lastCheckedAt: Date.now(),
  };
  host.notify({
    id: EXTERNAL_CHANGE_NOTIFICATION_ID,
    level: "error",
    title: "Proiecția externă necesită recovery",
    message,
    actionLabel: "Reîncarcă de pe disk",
    actionId: EXTERNAL_CHANGE_RELOAD_ACTION_ID,
  });
  host.setGlobalStatus(message, "error");
}

function preserveBlockedReceipt(
  host: ExternalDiskControllerHost,
  changedFiles: string[],
  flags: { activeFileChanged: boolean; previewRelevantChanged: boolean },
  receipt: KernelExternalDiskReconcileReceipt,
) {
  host.externalDiskState = {
    ...host.externalDiskState,
    changed: true,
    changedFiles,
    activeFileChanged: flags.activeFileChanged,
    previewRelevantChanged: flags.previewRelevantChanged,
    blockedByDirtySession: true,
    checking: false,
    lastCheckedAt: Date.now(),
  };
  host.notify({
    id: EXTERNAL_CHANGE_NOTIFICATION_ID,
    level: "warning",
    title: "Reconciliere externă blocată",
    message: receipt.verdictReason,
    actionLabel: "Reîncarcă de pe disk",
    actionId: EXTERNAL_CHANGE_RELOAD_ACTION_ID,
    secondaryActionLabel: "Păstrează sesiunea",
    secondaryActionId: EXTERNAL_CHANGE_KEEP_SESSION_ACTION_ID,
  });
  host.setGlobalStatus(receipt.verdictReason, "error");
}

function preserveReloadRequiredReceipt(
  host: ExternalDiskControllerHost,
  changedFiles: string[],
  flags: { activeFileChanged: boolean; previewRelevantChanged: boolean },
  receipt: KernelExternalDiskReconcileReceipt,
) {
  host.externalDiskState = {
    ...host.externalDiskState,
    changed: true,
    changedFiles,
    activeFileChanged: flags.activeFileChanged,
    previewRelevantChanged: flags.previewRelevantChanged,
    blockedByDirtySession: false,
    checking: false,
    lastCheckedAt: Date.now(),
  };
  if (host.aiEditLeaseFrontendLockActive) {
    host.notify({
      id: EXTERNAL_CHANGE_NOTIFICATION_ID,
      level: "info",
      title: "Se aplică modificările AI",
      message:
        "Manifestul autorizat schimbă structura proiectului. Pană Studio reconstruiește automat proiecția din disk.",
    });
    host.setGlobalStatus(
      "Structura declarată de AI a fost detectată; se reconstruiește automat ProjectSession.",
      "idle",
    );
    return;
  }
  host.notify({
    id: EXTERNAL_CHANGE_NOTIFICATION_ID,
    level: "warning",
    title: "Structura proiectului s-a schimbat",
    message: receipt.verdictReason,
    actionLabel: "Reîncarcă de pe disk",
    actionId: EXTERNAL_CHANGE_RELOAD_ACTION_ID,
    secondaryActionLabel: "Păstrează sesiunea",
    secondaryActionId: EXTERNAL_CHANGE_KEEP_SESSION_ACTION_ID,
  });
  host.setGlobalStatus(receipt.verdictReason, "error");
}

function notifyBlockedExternalChange(host: ExternalDiskControllerHost, changedFiles: string[]) {
  host.notify({
    id: EXTERNAL_CHANGE_NOTIFICATION_ID,
    level: "warning",
    title: "Fișiere modificate din exterior",
    message:
      `Am detectat schimbări pe disk (${formatChangedFiles(changedFiles)}), dar sesiunea Pană Studio are modificări nesalvate. Salvează sau reîncarcă manual înainte de a continua.`,
    actionLabel: "Reîncarcă de pe disk",
    actionId: EXTERNAL_CHANGE_RELOAD_ACTION_ID,
    secondaryActionLabel: "Păstrează sesiunea",
    secondaryActionId: EXTERNAL_CHANGE_KEEP_SESSION_ACTION_ID,
  });
  host.setGlobalStatus("Fișiere modificate din exterior. Sesiunea curentă are modificări nesalvate.", "error");
}

function formatChangedFiles(files: string[]) {
  if (files.length <= 3) return files.join(", ");
  return `${files.slice(0, 3).join(", ")} +${files.length - 3}`;
}

function requireAcceptedExternalDiskGeneration(
  receipt: KernelExternalDiskReconcileReceipt,
  currentGeneration: number | undefined,
  beforeManifest: ProjectDiskManifest | null,
  acceptedManifest: ProjectDiskManifest,
): number {
  const acceptedDiskGeneration = receipt.acceptedDiskGeneration;
  if (
    !Number.isSafeInteger(currentGeneration)
    || (currentGeneration ?? 0) < 1
    || acceptedDiskGeneration === null
    || !Number.isSafeInteger(acceptedDiskGeneration)
    || acceptedDiskGeneration < 1
    || !beforeManifest
    || beforeManifest.root !== acceptedManifest.root
    || beforeManifest.truncated
  ) {
    throw new Error(
      "Receipt-ul reconcile nu poate avansa o generație AcceptedDisk neinițializată sau invalidă.",
    );
  }
  const changedFiles = diffDiskManifests(beforeManifest, acceptedManifest).changedFiles;
  const expectedGeneration = currentGeneration! + (changedFiles.length > 0 ? 1 : 0);
  if (acceptedDiskGeneration !== expectedGeneration) {
    throw new Error(
      `Receipt-ul reconcile are generație AcceptedDisk stale (așteptat=${expectedGeneration}; primit=${acceptedDiskGeneration}).`,
    );
  }
  return acceptedDiskGeneration;
}

function acknowledgedInternalWriteBaseline(
  previous: ProjectDiskManifest,
  current: ProjectDiskManifest,
  acknowledgedFiles: string[],
): ProjectDiskManifest {
  const nextEntries = new Map<string, ProjectDiskManifestEntry>(
    previous.files.map((entry) => [entry.relativePath, entry]),
  );
  const currentEntries = new Map<string, ProjectDiskManifestEntry>(
    current.files.map((entry) => [entry.relativePath, entry]),
  );

  for (const file of acknowledgedFiles) {
    const currentEntry = currentEntries.get(file);
    if (currentEntry) {
      nextEntries.set(file, currentEntry);
    } else {
      nextEntries.delete(file);
    }
  }

  return {
    root: current.root,
    files: [...nextEntries.values()].sort((left, right) =>
      left.relativePath.localeCompare(right.relativePath),
    ),
    truncated: current.truncated,
    maxFiles: current.maxFiles,
  };
}
