import { tick } from "svelte";
import { drainPreviewStructuralLanes } from "$lib/kernel/preview-structural-lane";
import {
  acceptAiEditConflictForReconciliation,
  acknowledgeAiEditQuiescence,
  authorizeAiReconciliationRecoveryReload,
  completeAiReconciliationRecoveryReload,
  completeAiEditReconciliation,
  readAiCoordinationState,
  readProjectWorkspaceState,
} from "$lib/project/io";
import { flushWorkspaceMutationInputs } from "$lib/session/workspace-mutation-coordinator";
import {
  EXTERNAL_CHANGE_NOTIFICATION_ID,
  EXTERNAL_CHANGE_RELOAD_ACTION_ID,
  resumeExternalDiskMonitoringAfterTransitionLease,
  startExternalDiskPolling,
  suspendAndDrainExternalDiskMonitoring,
  type ExternalDiskControllerHost,
} from "$lib/state/external-disk-controller";
import type {
  AiCoordinationSnapshot,
  EditAuthority,
  ProjectWorkspaceSnapshot,
  SaveState,
} from "$lib/types";
import type { ProjectReloadOutcome } from "$lib/state/project-controller";

const AI_COORDINATION_POLL_MS = 500;
export const AI_COORDINATION_NOTIFICATION_ID = "ai.edit-authority";
export const AI_COORDINATION_ACCEPT_DISK_ACTION_ID = "ai.edit-authority.accept-disk";
const aiRecoveryReloadFlights = new WeakMap<object, Promise<ProjectReloadOutcome>>();

export type AiCoordinationControllerHost = {
  aiCoordinationSnapshot: AiCoordinationSnapshot | null;
  aiCoordinationTimer: number | null;
  aiCoordinationOperationInFlight: boolean;
  aiCoordinationHandledRequestId: string | null;
  aiCoordinationReconciliationLeaseId: string | null;
  aiCoordinationAutomaticReloadLeaseId: string | null;
  aiEditLeaseFrontendLockActive: boolean;
  aiReconciliationRecoveryReloadAuthorized: boolean;
  aiContextUiRevision: number;
  activeScannedPath: string | null;
  projectWorkspaceSnapshot: ProjectWorkspaceSnapshot | null;
  externalDiskState: ExternalDiskControllerHost["externalDiskState"];
  quiesceExternalReconcileInteractions: () => void;
  externalDiskControllerHost: () => ExternalDiskControllerHost;
  discardSessionAndReloadFromDisk: (
    preferredRelativePath?: string | null,
  ) => Promise<ProjectReloadOutcome>;
  setGlobalStatus: (text: string, kind: SaveState) => void;
  notify: (notification: {
    id: string;
    level: "info" | "warning" | "error";
    title: string;
    message: string;
    actionLabel?: string | null;
    actionId?: string | null;
  }) => void;
  clearNotification: (id: string) => void;
};

export function startAiCoordinationPolling(host: AiCoordinationControllerHost) {
  stopAiCoordinationPolling(host);
  if (typeof window === "undefined") return;
  void pollAiCoordination(host);
}

export function stopAiCoordinationPolling(host: AiCoordinationControllerHost) {
  if (host.aiCoordinationTimer !== null && typeof window !== "undefined") {
    window.clearTimeout(host.aiCoordinationTimer);
  }
  host.aiCoordinationTimer = null;
}

async function pollAiCoordination(host: AiCoordinationControllerHost) {
  if (host.aiCoordinationOperationInFlight) {
    scheduleNextPoll(host);
    return;
  }
  host.aiCoordinationOperationInFlight = true;
  try {
    const snapshot = await readAiCoordinationState();
    host.aiCoordinationSnapshot = snapshot;
    await applyCoordinationState(host, snapshot.authority);
  } catch (error) {
    host.setGlobalStatus(
      `Coordonarea AI nu poate fi citită: ${errorMessage(error)}. Mutațiile rămân protejate de nucleul Rust.`,
      "error",
    );
  } finally {
    host.aiCoordinationOperationInFlight = false;
    scheduleNextPoll(host);
  }
}

function scheduleNextPoll(host: AiCoordinationControllerHost) {
  if (typeof window === "undefined") return;
  stopAiCoordinationPolling(host);
  host.aiCoordinationTimer = window.setTimeout(() => {
    host.aiCoordinationTimer = null;
    void pollAiCoordination(host);
  }, AI_COORDINATION_POLL_MS);
}

async function applyCoordinationState(
  host: AiCoordinationControllerHost,
  authority: EditAuthority,
) {
  switch (authority.state) {
    case "user_active":
      host.aiCoordinationHandledRequestId = null;
      host.aiCoordinationReconciliationLeaseId = null;
      host.aiCoordinationAutomaticReloadLeaseId = null;
      releaseFrontendLock(host);
      host.clearNotification(AI_COORDINATION_NOTIFICATION_ID);
      return;
    case "ai_requested":
      await acknowledgePendingRequest(host, authority.detail.request);
      return;
    case "ai_active":
      host.aiEditLeaseFrontendLockActive = true;
      host.notify({
        id: AI_COORDINATION_NOTIFICATION_ID,
        level: "info",
        title: "AI editează proiectul",
        message: `Sursa este rezervată sesiunii ${authority.detail.lease.clientSessionId}. Pană Studio rămâne disponibil pentru navigare, preview și terminal.`,
      });
      return;
    case "ai_orphaned":
      host.aiEditLeaseFrontendLockActive = true;
      if (host.aiCoordinationReconciliationLeaseId !== authority.detail.leaseId) {
        host.aiCoordinationReconciliationLeaseId = authority.detail.leaseId;
        resumeExternalDiskMonitoringAfterTransitionLease(host.externalDiskControllerHost());
        startExternalDiskPolling(host.externalDiskControllerHost());
      }
      host.notify({
        id: AI_COORDINATION_NOTIFICATION_ID,
        level: "error",
        title: "Sesiunea AI s-a întrerupt — editarea rămâne blocată",
        message: authority.detail.reason,
        actionLabel: "Adoptă discul și reconstruiește",
        actionId: AI_COORDINATION_ACCEPT_DISK_ACTION_ID,
      });
      return;
    case "reconciling":
      host.aiEditLeaseFrontendLockActive = true;
      await reconcileReleasedLease(host, authority.detail);
      return;
    case "conflict":
      host.aiEditLeaseFrontendLockActive = true;
      host.notify({
        id: AI_COORDINATION_NOTIFICATION_ID,
        level: "error",
        title: "Conflict după editarea AI",
        message: `${authority.detail.reason} ${authority.detail.files.join(", ")}`.trim(),
        actionLabel: "Acceptă discul și reconciliază",
        actionId: AI_COORDINATION_ACCEPT_DISK_ACTION_ID,
      });
  }
}

async function acknowledgePendingRequest(
  host: AiCoordinationControllerHost,
  request: Extract<EditAuthority, { state: "ai_requested" }>["detail"]["request"],
) {
  if (host.aiCoordinationHandledRequestId === request.requestId) return;
  host.aiCoordinationHandledRequestId = request.requestId;
  host.aiEditLeaseFrontendLockActive = true;
  host.quiesceExternalReconcileInteractions();
  host.notify({
    id: AI_COORDINATION_NOTIFICATION_ID,
    level: "info",
    title: "Transfer de autoritate către AI",
    message: "Pană Studio închide editările tranzitorii și verifică dacă sesiunea este clean.",
  });

  let uiQuiescent = true;
  let blockerReason: string | null = null;
  try {
    await tick();
    await flushWorkspaceMutationInputs("snapshot");
    await drainPreviewStructuralLanes();
    await suspendAndDrainExternalDiskMonitoring(host.externalDiskControllerHost());
  } catch (error) {
    uiQuiescent = false;
    blockerReason = `Frontendul nu a putut închide toate editările tranzitorii: ${errorMessage(error)}`;
  }
  if (
    host.externalDiskState.checking
    || host.externalDiskState.reconciling
    || host.externalDiskState.changed
    || host.externalDiskState.blockedByDirtySession
    || host.externalDiskState.workspaceProjectionRecoveryRequired
    || host.externalDiskState.truncated
  ) {
    uiQuiescent = false;
    blockerReason =
      "Interfața nu poate transfera autoritatea cât timp proiecția disc/surse nu este stabilă și curată.";
  }

  try {
    const workspace = await readProjectWorkspaceState();
    host.projectWorkspaceSnapshot = workspace;
    const live = await readAiCoordinationState();
    host.aiCoordinationSnapshot = live;
    if (
      live.authority.state !== "ai_requested"
      || live.authority.detail.request.requestId !== request.requestId
    ) {
      await applyCoordinationState(host, live.authority);
      return;
    }

    const receipt = await acknowledgeAiEditQuiescence(request.clientSessionId, {
      requestId: request.requestId,
      projectSessionId: request.expectedProjectSessionId,
      projectRevision: request.expectedProjectRevision,
      uiRevision: host.aiContextUiRevision,
      uiQuiescent,
      blockerReason,
      dirtyFiles: [],
    });
    const refreshed = await readAiCoordinationState();
    host.aiCoordinationSnapshot = refreshed;
    if (receipt.status === "granted") {
      host.setGlobalStatus("Autoritatea de editare a fost transferată către AI.", "restored");
      return;
    }

    releaseFrontendLock(host);
    const reason = receipt.reason ?? "Lease-ul AI nu a putut fi acordat.";
    const userInstruction = receipt.requiredUserAction === "save_or_discard"
      ? " Salvează sau aruncă modificările, apoi AI poate solicita din nou lease-ul."
      : "";
    host.setGlobalStatus(`${reason}${userInstruction}`, "error");
    host.notify({
      id: AI_COORDINATION_NOTIFICATION_ID,
      level: "warning",
      title: "AI așteaptă utilizatorul",
      message: `${reason}${userInstruction}`,
    });
  } catch (error) {
    releaseFrontendLock(host);
    host.aiCoordinationHandledRequestId = null;
    host.setGlobalStatus(`Transferul autorității AI a eșuat: ${errorMessage(error)}`, "error");
  }
}

async function reconcileReleasedLease(
  host: AiCoordinationControllerHost,
  detail: Extract<EditAuthority, { state: "reconciling" }>["detail"],
) {
  if (host.aiCoordinationReconciliationLeaseId !== detail.leaseId) {
    host.aiCoordinationReconciliationLeaseId = detail.leaseId;
    resumeExternalDiskMonitoringAfterTransitionLease(host.externalDiskControllerHost());
    startExternalDiskPolling(host.externalDiskControllerHost());
    host.notify({
      id: AI_COORDINATION_NOTIFICATION_ID,
      level: "info",
      title: "Se reconciliază modificările AI",
      message: "Pană Studio verifică manifestul de disc și reconstruiește proiecția înainte de a reda controlul utilizatorului.",
    });
    return;
  }

  const external = host.externalDiskState;
  const diskCheckedAfterRelease = (external.lastCheckedAt ?? 0) >= detail.releasedAtMs;
  if (
    diskCheckedAfterRelease
    && shouldAutomaticallyReloadAiReconciliation(external, detail)
  ) {
    await automaticallyReloadAuthorizedAiReconciliation(host, detail.leaseId);
    return;
  }
  if (
    !diskCheckedAfterRelease
    || external.checking
    || external.reconciling
    || external.changed
    || external.blockedByDirtySession
    || external.workspaceProjectionRecoveryRequired
    || external.truncated
  ) return;

  const workspace = await readProjectWorkspaceState();
  host.projectWorkspaceSnapshot = workspace;
  if (
    !workspace
    || workspace.runtimeSessionId !== detail.projectSessionId
    || workspace.dirty
  ) return;

  const receipt = await completeAiEditReconciliation(
    detail.leaseId,
    detail.projectSessionId,
    workspace.revision,
    (external.lastAppliedAt ?? 0) >= detail.releasedAtMs
      ? external.lastAppliedFiles
      : [],
  );
  const refreshed = await readAiCoordinationState();
  host.aiCoordinationSnapshot = refreshed;
  if (receipt.status === "released_to_user") {
    releaseFrontendLock(host);
    host.setGlobalStatus("Modificările AI au fost reconciliate; utilizatorul are din nou autoritatea de editare.", "restored");
  } else if (receipt.status === "conflict") {
    host.notify({
      id: AI_COORDINATION_NOTIFICATION_ID,
      level: "error",
      title: "Conflict la reconcilierea AI",
      message: receipt.reason ?? "Manifestul de disc s-a schimbat din nou în timpul reconcilierii.",
    });
  }
}

export function shouldAutomaticallyReloadAiReconciliation(
  external: ExternalDiskControllerHost["externalDiskState"],
  detail: Extract<EditAuthority, { state: "reconciling" }>["detail"],
) {
  if (
    !external.changed
    || external.checking
    || external.reconciling
    || external.blockedByDirtySession
    || external.workspaceProjectionRecoveryRequired
    || external.truncated
  ) return false;
  return sameFileSet(external.changedFiles, detail.expectedChangedFiles)
    && sameFileSet(external.changedFiles, detail.observedChangedFiles);
}

export type AiRecoveryAuthorityDisposition =
  | "reload"
  | "accept_conflict"
  | "authorize_recovery"
  | "reject_active_lease";

export function aiRecoveryAuthorityDisposition(
  authority: EditAuthority,
): AiRecoveryAuthorityDisposition {
  switch (authority.state) {
    case "ai_requested":
    case "ai_active":
      return "reject_active_lease";
    case "conflict":
      return "accept_conflict";
    case "ai_orphaned":
      return "authorize_recovery";
    case "reconciling":
      return authority.detail.recoveryReloadAuthorized
        ? "reload"
        : "authorize_recovery";
    case "user_active":
      return "reload";
  }
}

async function automaticallyReloadAuthorizedAiReconciliation(
  host: AiCoordinationControllerHost,
  leaseId: string,
) {
  if (host.aiCoordinationAutomaticReloadLeaseId === leaseId) return;
  host.aiCoordinationAutomaticReloadLeaseId = leaseId;
  host.notify({
    id: AI_COORDINATION_NOTIFICATION_ID,
    level: "info",
    title: "Se aplică modificările AI",
    message: "Manifestul declarat corespunde discului. Pană Studio reconstruiește automat proiecția proiectului.",
  });
  try {
    await reloadAuthorizedAiReconciliationFromDisk(host);
    const refreshed = await readAiCoordinationState();
    host.aiCoordinationSnapshot = refreshed;
    if (refreshed.authority.state !== "user_active") {
      throw new Error(
        "Reconstrucția de pe disc nu a readus coordonarea în starea activă a utilizatorului.",
      );
    }
  } catch (error) {
    host.notify({
      id: EXTERNAL_CHANGE_NOTIFICATION_ID,
      level: "error",
      title: "Aplicarea modificărilor AI a fost oprită",
      message: errorMessage(error),
      actionLabel: "Reîncearcă reconstruirea",
      actionId: EXTERNAL_CHANGE_RELOAD_ACTION_ID,
    });
    host.setGlobalStatus(
      `Reconstrucția automată după editarea AI a eșuat: ${errorMessage(error)}`,
      "error",
    );
  }
}

export function reloadAuthorizedAiReconciliationFromDisk(
  host: Pick<
    AiCoordinationControllerHost,
    | "aiCoordinationSnapshot"
    | "aiReconciliationRecoveryReloadAuthorized"
    | "activeScannedPath"
    | "discardSessionAndReloadFromDisk"
  >,
): Promise<ProjectReloadOutcome> {
  const flightKey = host as object;
  const existing = aiRecoveryReloadFlights.get(flightKey);
  if (existing) return existing;

  const operation = performAuthorizedAiReconciliationReload(host);
  aiRecoveryReloadFlights.set(flightKey, operation);
  const clearFlight = () => {
    if (aiRecoveryReloadFlights.get(flightKey) === operation) {
      aiRecoveryReloadFlights.delete(flightKey);
    }
  };
  void operation.then(clearFlight, clearFlight);
  return operation;
}

async function performAuthorizedAiReconciliationReload(
  host: Pick<
    AiCoordinationControllerHost,
    | "aiCoordinationSnapshot"
    | "aiReconciliationRecoveryReloadAuthorized"
    | "activeScannedPath"
    | "discardSessionAndReloadFromDisk"
  >,
): Promise<ProjectReloadOutcome> {
  let live = await readAiCoordinationState();
  host.aiCoordinationSnapshot = live;
  let reconciliationLeaseId: string | null = null;
  let readyToReload = false;
  for (let transitionCount = 0; transitionCount < 4; transitionCount += 1) {
    const disposition = aiRecoveryAuthorityDisposition(live.authority);
    if (disposition === "reject_active_lease") {
      throw new Error(
        "Recovery reload este refuzat cât timp o sesiune AI solicită sau deține încă un lease valid.",
      );
    }
    if (disposition === "accept_conflict") {
      const accepted = await acceptAiEditConflictForReconciliation();
      if (accepted.status !== "reconciling" || accepted.authority.state !== "reconciling") {
        throw new Error(accepted.reason ?? "Conflictul AI nu a putut intra în reconciliere.");
      }
      live = await readAiCoordinationState();
      host.aiCoordinationSnapshot = live;
      continue;
    }
    if (disposition === "authorize_recovery") {
      const authorization = await authorizeAiReconciliationRecoveryReload();
      if (
        authorization.status !== "reconciling"
        || authorization.authority.state !== "reconciling"
        || !authorization.authority.detail.recoveryReloadAuthorized
      ) {
        throw new Error(
          authorization.reason ?? "Nucleul nu a autorizat reconstruirea sesiunii proiectului de pe disc.",
        );
      }
      live = await readAiCoordinationState();
      host.aiCoordinationSnapshot = live;
      continue;
    }

    reconciliationLeaseId =
      live.authority.state === "reconciling"
        ? live.authority.detail.leaseId
        : null;
    readyToReload = true;
    break;
  }
  if (!readyToReload) {
    throw new Error(
      "Coordonarea AI nu a ajuns la o stare stabilă pentru reconstruirea de pe disc.",
    );
  }

  host.aiReconciliationRecoveryReloadAuthorized = true;
  try {
    const outcome = await host.discardSessionAndReloadFromDisk(host.activeScannedPath);
    if (outcome.status !== "completed") {
      throw new Error(outcome.message);
    }
    if (reconciliationLeaseId !== null) {
      const completion = await completeAiReconciliationRecoveryReload(
        reconciliationLeaseId,
        outcome.projectSessionId,
      );
      if (
        completion.status !== "released_to_user"
        || completion.authority.state !== "user_active"
      ) {
        throw new Error(
          completion.reason
            ?? "Nucleul nu a confirmat proiecția frontend a noii ProjectSession.",
        );
      }
    }
    const terminal = await readAiCoordinationState();
    host.aiCoordinationSnapshot = terminal;
    if (terminal.authority.state !== "user_active") {
      throw new Error(
        "Recovery reload a reconstruit proiectul, dar autoritatea Rust nu a ajuns în starea user_active.",
      );
    }
    return outcome;
  } finally {
    host.aiReconciliationRecoveryReloadAuthorized = false;
  }
}

function sameFileSet(left: string[], right: string[]) {
  if (left.length !== right.length) return false;
  const rightSet = new Set(right);
  return rightSet.size === right.length && left.every((path) => rightSet.has(path));
}

function releaseFrontendLock(host: AiCoordinationControllerHost) {
  const wasLocked = host.aiEditLeaseFrontendLockActive;
  host.aiEditLeaseFrontendLockActive = false;
  if (wasLocked) {
    resumeExternalDiskMonitoringAfterTransitionLease(host.externalDiskControllerHost());
  }
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
