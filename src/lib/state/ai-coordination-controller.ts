import { tick } from "svelte";
import { subscribeAiCoordinationChanges } from "$lib/kernel/ai-coordination-events";
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
  startExternalDiskMonitoring,
  suspendAndDrainExternalDiskMonitoring,
  type ExternalDiskControllerHost,
} from "$lib/state/external-disk-controller";
import type {
  AiCoordinationSnapshot,
  EditAuthority,
  ProjectWorkspaceSnapshot,
} from "$lib/types";
import type {
  GlobalStatusEscalationRequest,
  GlobalStatusKind,
  GlobalStatusPublishOptions,
} from "$lib/status/global-status";
import type { ProjectReloadOutcome } from "$lib/state/project-controller";
import { t } from "$lib/i18n/runtime.svelte";

export const AI_COORDINATION_NOTIFICATION_ID = "ai.edit-authority";
export const AI_COORDINATION_ACCEPT_DISK_ACTION_ID = "ai.edit-authority.accept-disk";
const aiRecoveryReloadFlights = new WeakMap<object, Promise<ProjectReloadOutcome>>();

export type AiCoordinationControllerHost = {
  aiCoordinationSnapshot: AiCoordinationSnapshot | null;
  aiCoordinationUnlisten: (() => void) | null;
  aiCoordinationSubscriptionGeneration: number;
  aiCoordinationPendingSnapshot: AiCoordinationSnapshot | null;
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
  setGlobalStatus: (
    text: string,
    kind: GlobalStatusKind,
    options?: GlobalStatusPublishOptions,
  ) => void;
  escalateGlobalStatus: (notification: GlobalStatusEscalationRequest) => void;
  clearNotification: (id: string) => void;
};

export function startAiCoordinationEvents(host: AiCoordinationControllerHost) {
  stopAiCoordinationEvents(host);
  const generation = host.aiCoordinationSubscriptionGeneration;
  void subscribeAiCoordinationChanges((snapshot) => {
    if (host.aiCoordinationSubscriptionGeneration !== generation) return;
    enqueueAiCoordinationSnapshot(host, snapshot);
  }).then((unlisten) => {
    if (host.aiCoordinationSubscriptionGeneration !== generation) {
      unlisten();
      return;
    }
    host.aiCoordinationUnlisten = unlisten;
    void readAiCoordinationState()
      .then((snapshot) => enqueueAiCoordinationSnapshot(host, snapshot))
      .catch((error) => reportAiCoordinationReadFailure(host, error));
  }).catch((error) => reportAiCoordinationReadFailure(host, error));
}

export function stopAiCoordinationEvents(host: AiCoordinationControllerHost) {
  host.aiCoordinationSubscriptionGeneration += 1;
  host.aiCoordinationUnlisten?.();
  host.aiCoordinationUnlisten = null;
  host.aiCoordinationPendingSnapshot = null;
}

function enqueueAiCoordinationSnapshot(
  host: AiCoordinationControllerHost,
  snapshot: AiCoordinationSnapshot,
) {
  if (
    host.aiCoordinationSnapshot
    && snapshot.coordinationRevision < host.aiCoordinationSnapshot.coordinationRevision
  ) return;
  if (
    !host.aiCoordinationPendingSnapshot
    || snapshot.coordinationRevision >= host.aiCoordinationPendingSnapshot.coordinationRevision
  ) {
    host.aiCoordinationPendingSnapshot = snapshot;
  }
  void drainAiCoordinationSnapshots(host);
}

async function drainAiCoordinationSnapshots(host: AiCoordinationControllerHost) {
  if (host.aiCoordinationOperationInFlight) {
    return;
  }
  host.aiCoordinationOperationInFlight = true;
  try {
    while (host.aiCoordinationPendingSnapshot) {
      const snapshot = host.aiCoordinationPendingSnapshot;
      host.aiCoordinationPendingSnapshot = null;
      if (
        host.aiCoordinationSnapshot
        && snapshot.coordinationRevision < host.aiCoordinationSnapshot.coordinationRevision
      ) continue;
      host.aiCoordinationSnapshot = snapshot;
      await applyCoordinationState(host, snapshot.authority);
    }
  } finally {
    host.aiCoordinationOperationInFlight = false;
    if (host.aiCoordinationPendingSnapshot) {
      void drainAiCoordinationSnapshots(host);
    }
  }
}

function reportAiCoordinationReadFailure(
  host: AiCoordinationControllerHost,
  error: unknown,
) {
  host.setGlobalStatus(
    t("ai-coordination-read-failed", { message: errorMessage(error) }),
    "error",
    {
      code: "ai.coordination.read-failed",
      source: "ai",
      dedupeKey: "ai.coordination.events",
      resolutionKey: "ai.coordination.events",
    },
  );
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
      host.setGlobalStatus(
        t("ai-coordination-editing-message", {
          session: authority.detail.lease.clientSessionId,
        }),
        "saving",
        {
          code: "ai.coordination.active",
          source: "ai",
          lifecycle: "until_replaced",
          escalation: "status_only",
          dedupeKey: AI_COORDINATION_NOTIFICATION_ID,
          resolutionKey: AI_COORDINATION_NOTIFICATION_ID,
        },
      );
      return;
    case "ai_orphaned":
      host.aiEditLeaseFrontendLockActive = true;
      if (host.aiCoordinationReconciliationLeaseId !== authority.detail.leaseId) {
        host.aiCoordinationReconciliationLeaseId = authority.detail.leaseId;
        resumeExternalDiskMonitoringAfterTransitionLease(host.externalDiskControllerHost());
        startExternalDiskMonitoring(host.externalDiskControllerHost());
      }
      host.escalateGlobalStatus({
        id: AI_COORDINATION_NOTIFICATION_ID,
        level: "error",
        title: t("ai-coordination-orphaned-title"),
        message: t("ai-coordination-orphaned-message"),
        actionLabel: t("ai-coordination-adopt-disk"),
        actionId: AI_COORDINATION_ACCEPT_DISK_ACTION_ID,
      });
      return;
    case "reconciling":
      host.aiEditLeaseFrontendLockActive = true;
      await reconcileReleasedLease(host, authority.detail);
      return;
    case "conflict":
      host.aiEditLeaseFrontendLockActive = true;
      host.escalateGlobalStatus({
        id: AI_COORDINATION_NOTIFICATION_ID,
        level: "error",
        title: t("ai-coordination-conflict-title"),
        message: t("ai-coordination-conflict-message", {
          files: authority.detail.files.join(", "),
        }),
        actionLabel: t("ai-coordination-accept-disk"),
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
  host.setGlobalStatus(
    t("ai-coordination-transfer-message"),
    "saving",
    {
      code: "ai.coordination.transfer",
      source: "ai",
      lifecycle: "until_replaced",
      escalation: "status_only",
      dedupeKey: AI_COORDINATION_NOTIFICATION_ID,
      resolutionKey: AI_COORDINATION_NOTIFICATION_ID,
    },
  );

  let uiQuiescent = true;
  let blockerReason: string | null = null;
  try {
    await tick();
    await flushWorkspaceMutationInputs("snapshot");
    await drainPreviewStructuralLanes();
    await suspendAndDrainExternalDiskMonitoring(host.externalDiskControllerHost());
  } catch (error) {
    uiQuiescent = false;
    blockerReason = t("ai-coordination-quiesce-failed", {
      message: errorMessage(error),
    });
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
    blockerReason = t("ai-coordination-projection-unstable");
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
      host.setGlobalStatus(t("ai-coordination-transfer-complete"), "restored", {
        code: "ai.coordination.transfer-complete",
        source: "ai",
        dedupeKey: AI_COORDINATION_NOTIFICATION_ID,
        resolutionKey: AI_COORDINATION_NOTIFICATION_ID,
      });
      return;
    }

    releaseFrontendLock(host);
    const reason = t("ai-coordination-lease-denied");
    const userInstruction = receipt.requiredUserAction === "save_or_discard"
      ? ` ${t("ai-coordination-save-or-discard")}`
      : "";
    host.escalateGlobalStatus({
      id: AI_COORDINATION_NOTIFICATION_ID,
      level: "warning",
      title: t("ai-coordination-awaiting-user"),
      message: `${reason}${userInstruction}`,
    });
  } catch (error) {
    releaseFrontendLock(host);
    host.aiCoordinationHandledRequestId = null;
    host.setGlobalStatus(t("ai-coordination-transfer-failed", {
      message: errorMessage(error),
    }), "error");
  }
}

async function reconcileReleasedLease(
  host: AiCoordinationControllerHost,
  detail: Extract<EditAuthority, { state: "reconciling" }>["detail"],
) {
  if (host.aiCoordinationReconciliationLeaseId !== detail.leaseId) {
    host.aiCoordinationReconciliationLeaseId = detail.leaseId;
    resumeExternalDiskMonitoringAfterTransitionLease(host.externalDiskControllerHost());
    startExternalDiskMonitoring(host.externalDiskControllerHost());
    host.setGlobalStatus(
      t("ai-coordination-reconciling-message"),
      "saving",
      {
        code: "ai.coordination.reconciling",
        source: "ai",
        lifecycle: "until_replaced",
        escalation: "status_only",
        dedupeKey: AI_COORDINATION_NOTIFICATION_ID,
        resolutionKey: AI_COORDINATION_NOTIFICATION_ID,
      },
    );
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
    host.setGlobalStatus(t("ai-coordination-reconciled"), "restored", {
      code: "ai.coordination.reconciled",
      source: "ai",
      dedupeKey: AI_COORDINATION_NOTIFICATION_ID,
      resolutionKey: AI_COORDINATION_NOTIFICATION_ID,
    });
  } else if (receipt.status === "conflict") {
    host.escalateGlobalStatus({
      id: AI_COORDINATION_NOTIFICATION_ID,
      level: "error",
      title: t("ai-coordination-reconcile-conflict-title"),
      message: t("ai-coordination-reconcile-conflict-message"),
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
  host.setGlobalStatus(
    t("ai-coordination-applying-message"),
    "saving",
    {
      code: "ai.coordination.applying",
      source: "ai",
      lifecycle: "until_replaced",
      escalation: "status_only",
      dedupeKey: AI_COORDINATION_NOTIFICATION_ID,
      resolutionKey: AI_COORDINATION_NOTIFICATION_ID,
    },
  );
  try {
    await reloadAuthorizedAiReconciliationFromDisk(host);
    const refreshed = await readAiCoordinationState();
    host.aiCoordinationSnapshot = refreshed;
    if (refreshed.authority.state !== "user_active") {
      throw new Error(
        t("ai-coordination-rebuild-not-user-active"),
      );
    }
  } catch (error) {
    host.escalateGlobalStatus({
      id: EXTERNAL_CHANGE_NOTIFICATION_ID,
      level: "error",
      title: t("ai-coordination-apply-stopped"),
      message: errorMessage(error),
      actionLabel: t("ai-coordination-retry-rebuild"),
      actionId: EXTERNAL_CHANGE_RELOAD_ACTION_ID,
    });
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
        t("ai-coordination-reload-active-lease"),
      );
    }
    if (disposition === "accept_conflict") {
      const accepted = await acceptAiEditConflictForReconciliation();
      if (accepted.status !== "reconciling" || accepted.authority.state !== "reconciling") {
        throw new Error(t("ai-coordination-conflict-not-reconciling"));
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
          t("ai-coordination-rebuild-not-authorized"),
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
      t("ai-coordination-not-stable"),
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
          t("ai-coordination-session-not-confirmed"),
        );
      }
    }
    const terminal = await readAiCoordinationState();
    host.aiCoordinationSnapshot = terminal;
    if (terminal.authority.state !== "user_active") {
      throw new Error(
        t("ai-coordination-reload-not-user-active"),
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
