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
  ScssVariable,
} from "$lib/types";
import type {
  GlobalStatusEscalationRequest,
  GlobalStatusKind,
} from "$lib/status/global-status";
import { t } from "$lib/i18n/runtime.svelte";
import { errorMessage } from "$lib/util";

const ACTIVE_CHECK_INTERVAL = 5000;
const BACKGROUND_CHECK_INTERVAL = 15000;
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
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
  escalateGlobalStatus: (notification: GlobalStatusEscalationRequest) => void;
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
      t("external-disk-monitor-restarted"),
    );
  }
  if (host.externalDiskState.checking || host.externalDiskState.reconciling) {
    throw new Error(
      t("external-disk-monitor-not-terminal"),
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
    || !host.scannedProject
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
  if (!host.scannedProject) return;
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
    host.scannedProject &&
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
  host.escalateGlobalStatus({
    id: EXTERNAL_CHANGE_NOTIFICATION_ID,
    level: "error",
    title: t("external-disk-reprojection-title"),
    message,
    actionLabel: t("external-disk-reload"),
    actionId: EXTERNAL_CHANGE_RELOAD_ACTION_ID,
  });
}

export async function establishExternalDiskBaseline(host: ExternalDiskControllerHost) {
  if (!host.scannedProject) return;
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
      t("external-disk-save-baseline-invalid"),
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
    !host.scannedProject ||
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
  host.externalDiskState.checking = true;

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

    if (!changed) {
      host.externalDiskState.baseline = current;
      if (host.externalDiskState.reconciling) {
        host.externalDiskState.reconciling = false;
      }
      if (host.externalDiskState.changed) {
        host.externalDiskState.changed = false;
      }
      if (host.externalDiskState.changedFiles.length > 0) {
        host.externalDiskState.changedFiles = [];
      }
      if (host.externalDiskState.activeFileChanged) {
        host.externalDiskState.activeFileChanged = false;
      }
      if (host.externalDiskState.previewRelevantChanged) {
        host.externalDiskState.previewRelevantChanged = false;
      }
      if (host.externalDiskState.blockedByDirtySession) {
        host.externalDiskState.blockedByDirtySession = false;
      }
      host.externalDiskState.lastCheckedAt = Date.now();
      host.externalDiskState.checking = false;
      if (host.externalDiskState.workspaceProjectionRecoveryRequired) {
        host.externalDiskState.workspaceProjectionRecoveryRequired = false;
      }
      if (host.externalDiskState.truncated !== current.truncated) {
        host.externalDiskState.truncated = current.truncated;
      }
      host.clearNotification(EXTERNAL_CHANGE_NOTIFICATION_ID);
      return;
    }

    host.externalDiskState = {
      // A changed manifest is only accepted after the Rust reconcile receipt.
      baseline: host.externalDiskState.baseline,
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

    if (blockedByDirtySession) {
      escalateBlockedExternalChange(host, diff.changedFiles);
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
    host.externalDiskState.checking = false;
    host.externalDiskState.lastCheckedAt = Date.now();
    host.projectStatus = t("external-disk-monitor-failed", {
      message: errorMessage(error),
    });
  }
}

function finishSuspendedCheck(host: ExternalDiskControllerHost) {
  host.externalDiskState.checking = false;
  host.externalDiskState.lastCheckedAt = Date.now();
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
      host.scannedProject &&
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
    !project
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
    host.scannedProject
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
    escalateBlockedExternalChange(host, changedFiles);
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
    throw new Error(t("external-disk-receipt-session-mismatch"));
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
    throw new Error(t("external-disk-revision-missing"));
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
      t("external-disk-snapshot-mismatch"),
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
    const scanned = await scanProject(expectedRoot);
    if (
      scanned.root !== receipt.projectRoot
      || scanned.kernelSessionId !== receipt.sessionId
      || scanned.workspaceRevision !== receipt.workspaceRevision
    ) {
      throw new Error(
        t("external-disk-scan-mismatch"),
      );
    }
    const project = preservePreviewBaseUrl(
      scanned,
      projectBeforeReconcile,
    );
    if (!isCurrentReconcile(host, expectedRoot, reconcileGeneration)) return;
    host.scannedProject = project;
  }
  if (receipt.projectionHints.sourceGraph) {
    if (!receipt.sourceGraphInvalidated) {
      throw new Error(t("external-disk-source-graph-not-invalidated"));
    }
    await host.refreshSourceGraph?.({ strict: true });
    if (!isCurrentReconcile(host, expectedRoot, reconcileGeneration)) return;
  }
  if (receipt.projectionHints.scss) {
    const cssIdentity = createCssRequestIdentity(receipt.projectRoot, receipt.sessionId);
    const nextScssVariables = await getScssVariables(
      cssIdentity,
      receipt.workspaceRevision ?? undefined,
    );
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
    t("external-disk-reloaded", { files: formatChangedFiles(changedFiles) }),
    "restored",
  );
  } catch (error) {
    if (
      rustReceiptAccepted
      && isCurrentReconcile(host, expectedRoot, reconcileGeneration)
    ) {
      preserveProjectionFailureAfterCommit(host, changedFiles, flags, error);
      return;
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

async function withExternalProjectionDeadline<T>(operation: Promise<T>): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | null = null;
  const deadline = new Promise<never>((_resolve, reject) => {
    timer = setTimeout(() => {
      reject(new Error(
        t("external-disk-projection-timeout", {
          seconds: EXTERNAL_PROJECTION_DEADLINE_MS / 1000,
        }),
      ));
    }, EXTERNAL_PROJECTION_DEADLINE_MS);
  });
  try {
    return await Promise.race([operation, deadline]);
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
  const message = t("external-disk-concurrent-ui-message");
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
  host.escalateGlobalStatus({
    id: EXTERNAL_CHANGE_NOTIFICATION_ID,
    level: "error",
    title: t("external-disk-concurrent-ui-title"),
    message,
    statusMessage: message,
    actionLabel: t("external-disk-reload"),
    actionId: EXTERNAL_CHANGE_RELOAD_ACTION_ID,
  });
}

function preserveUninitializedExternalMonitor(
  host: ExternalDiskControllerHost,
  observedRoot: string,
) {
  const message = t("external-disk-baseline-unverified-message", {
    root: observedRoot,
  });
  host.externalDiskState = {
    ...host.externalDiskState,
    changed: true,
    blockedByDirtySession: true,
    checking: false,
    workspaceProjectionRecoveryRequired: true,
    lastCheckedAt: Date.now(),
  };
  host.escalateGlobalStatus({
    id: EXTERNAL_CHANGE_NOTIFICATION_ID,
    level: "error",
    title: t("external-disk-baseline-unverified-title"),
    message,
    statusMessage: message,
    actionLabel: t("external-disk-reload"),
    actionId: EXTERNAL_CHANGE_RELOAD_ACTION_ID,
  });
}

function preserveProjectionFailureAfterCommit(
  host: ExternalDiskControllerHost,
  changedFiles: string[],
  flags: { activeFileChanged: boolean; previewRelevantChanged: boolean },
  error: unknown,
) {
  const message = t("external-disk-projection-failed-message", {
    message: errorMessage(error),
  });
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
  host.escalateGlobalStatus({
    id: EXTERNAL_CHANGE_NOTIFICATION_ID,
    level: "error",
    title: t("external-disk-projection-recovery-title"),
    message,
    statusMessage: message,
    actionLabel: t("external-disk-reload"),
    actionId: EXTERNAL_CHANGE_RELOAD_ACTION_ID,
  });
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
  host.escalateGlobalStatus({
    id: EXTERNAL_CHANGE_NOTIFICATION_ID,
    level: "warning",
    title: t("external-disk-reconcile-blocked-title"),
    message: localizedExternalReconcileVerdict(receipt),
    statusMessage: localizedExternalReconcileVerdict(receipt),
    actionLabel: t("external-disk-reload"),
    actionId: EXTERNAL_CHANGE_RELOAD_ACTION_ID,
    secondaryActionLabel: t("external-disk-keep-session"),
    secondaryActionId: EXTERNAL_CHANGE_KEEP_SESSION_ACTION_ID,
  });
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
    host.setGlobalStatus(
      t("external-disk-ai-structure-detected"),
      "saving",
    );
    return;
  }
  host.escalateGlobalStatus({
    id: EXTERNAL_CHANGE_NOTIFICATION_ID,
    level: "warning",
    title: t("external-disk-structure-changed-title"),
    message: localizedExternalReconcileVerdict(receipt),
    statusMessage: localizedExternalReconcileVerdict(receipt),
    actionLabel: t("external-disk-reload"),
    actionId: EXTERNAL_CHANGE_RELOAD_ACTION_ID,
    secondaryActionLabel: t("external-disk-keep-session"),
    secondaryActionId: EXTERNAL_CHANGE_KEEP_SESSION_ACTION_ID,
  });
}

function escalateBlockedExternalChange(host: ExternalDiskControllerHost, changedFiles: string[]) {
  host.escalateGlobalStatus({
    id: EXTERNAL_CHANGE_NOTIFICATION_ID,
    level: "warning",
    title: t("external-disk-files-changed-title"),
    message: t("external-disk-files-changed-message", {
      files: formatChangedFiles(changedFiles),
    }),
    statusMessage: t("external-disk-files-changed-status"),
    actionLabel: t("external-disk-reload"),
    actionId: EXTERNAL_CHANGE_RELOAD_ACTION_ID,
    secondaryActionLabel: t("external-disk-keep-session"),
    secondaryActionId: EXTERNAL_CHANGE_KEEP_SESSION_ACTION_ID,
  });
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
      t("external-disk-generation-invalid"),
    );
  }
  const changedFiles = diffDiskManifests(beforeManifest, acceptedManifest).changedFiles;
  const expectedGeneration = currentGeneration! + (changedFiles.length > 0 ? 1 : 0);
  if (acceptedDiskGeneration !== expectedGeneration) {
    throw new Error(
      t("external-disk-generation-stale", {
        expected: expectedGeneration,
        actual: acceptedDiskGeneration,
      }),
    );
  }
  return acceptedDiskGeneration;
}

function localizedExternalReconcileVerdict(
  receipt: KernelExternalDiskReconcileReceipt,
): string {
  const diagnostic = receipt.diagnostics[0]?.messageDiagnostic;
  if (diagnostic) return errorMessage(diagnostic);
  if (receipt.status === "reload_required") return t("external-disk-verdict-reload-required");
  if (receipt.status === "stale_evidence") return t("external-disk-verdict-stale");
  if (receipt.status === "blocked") return t("external-disk-verdict-blocked");
  if (receipt.status === "applied") {
    return t("external-disk-verdict-applied", {
      content: receipt.reconciledCount,
      metadata: receipt.metadataRefreshedCount,
    });
  }
  return t("external-disk-verdict-noop");
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
