import {
  startProjectDiskWatch,
  stopProjectDiskWatch,
  type ProjectDiskWatchStopIdentity,
} from "$lib/project/io/external-disk";
import {
  subscribeProjectDiskChanges,
  type ProjectDiskChangeNotice,
} from "$lib/kernel/project-disk-events";
import type {
  ExternalDiskCheckLease,
  ExternalDiskContext,
} from "$lib/session/external-disk/contracts";
import {
  currentExternalDiskCheckLease,
  externalDiskCheckBelongsToCurrentSession,
  externalDiskCheckLeaseMatches,
  runExternalDiskCheck,
} from "$lib/session/external-disk/reconcile";
import { finishExternalDiskCheck } from "$lib/session/external-disk/state";
import { errorMessage } from "$lib/util";
import { t } from "$lib/i18n/runtime.svelte";

export const FULL_MANIFEST_AUDIT_INTERVAL = 5 * 60_000;

type ProjectDiskWatchReceipt = Awaited<ReturnType<typeof startProjectDiskWatch>>;

export type ExternalDiskMonitorPort = Readonly<{
  subscribe: typeof subscribeProjectDiskChanges;
  startWatch: typeof startProjectDiskWatch;
  stopWatch: typeof stopProjectDiskWatch;
  runCheck: (
    context: ExternalDiskContext,
    lease: ExternalDiskCheckLease,
  ) => Promise<void>;
  schedule: (operation: () => void, delayMs: number) => number | null;
  clearSchedule: (timer: number) => void;
}>;

const defaultMonitorPort: ExternalDiskMonitorPort = {
  subscribe: subscribeProjectDiskChanges,
  startWatch: startProjectDiskWatch,
  stopWatch: stopProjectDiskWatch,
  runCheck: runExternalDiskCheck,
  schedule: (operation, delayMs) => (
    typeof window === "undefined" ? null : window.setTimeout(operation, delayMs)
  ),
  clearSchedule: (timer) => {
    if (typeof window !== "undefined") window.clearTimeout(timer);
  },
};

export function startExternalDiskMonitoring(
  context: ExternalDiskContext,
  port: ExternalDiskMonitorPort = defaultMonitorPort,
) {
  clearExternalDiskAudit(context, port);
  if (!externalDiskMonitoringAllowed(context)) return;
  void ensureNativeExternalDiskMonitoring(context, port);
}

export function stopExternalDiskMonitoring(
  context: ExternalDiskContext,
  port: ExternalDiskMonitorPort = defaultMonitorPort,
) {
  clearExternalDiskAudit(context, port);
  void stopNativeExternalDiskMonitoring(context, port).catch((error) => {
    console.error("[Pană Studio] Project disk watcher stop failed", error);
  });
}

export async function suspendAndDrainExternalDiskMonitoring(
  context: ExternalDiskContext,
  port: ExternalDiskMonitorPort = defaultMonitorPort,
) {
  const { runtime } = context;
  runtime.suspended = true;
  clearExternalDiskAudit(context, port);
  await stopNativeExternalDiskMonitoring(context, port);

  runtime.checkGeneration += 1;
  const inFlight = runtime.checkInFlight;
  if (inFlight && externalDiskCheckBelongsToCurrentSession(context, inFlight)) {
    await inFlight.promise;
  } else if (inFlight && runtime.checkInFlight === inFlight) {
    runtime.checkInFlight = null;
  }
  if (runtime.snapshot.checking && !runtime.snapshot.reconciling) {
    finishExternalDiskCheck(context);
  }
  if (
    runtime.checkInFlight
    && externalDiskCheckBelongsToCurrentSession(context, runtime.checkInFlight)
  ) {
    throw new Error(t("external-disk-monitor-restarted"));
  }
  if (runtime.snapshot.checking || runtime.snapshot.reconciling) {
    throw new Error(t("external-disk-monitor-not-terminal"));
  }
}

export function resumeExternalDiskMonitoringAfterSave(
  context: ExternalDiskContext,
  port: ExternalDiskMonitorPort = defaultMonitorPort,
) {
  context.runtime.suspended = false;
  if (
    context.environment.session.transitionLocked
    || context.environment.session.historyLocked
    || !context.environment.session.project
  ) return;
  startExternalDiskMonitoring(context, port);
}

export function resumeExternalDiskMonitoringAfterTransition(
  context: ExternalDiskContext,
  port: ExternalDiskMonitorPort = defaultMonitorPort,
) {
  if (
    context.environment.session.transitionLocked
    || context.environment.session.historyLocked
  ) return;
  context.runtime.suspended = false;
  if (!context.environment.session.project) return;
  startExternalDiskMonitoring(context, port);
}

export async function ensureNativeExternalDiskMonitoring(
  context: ExternalDiskContext,
  port: ExternalDiskMonitorPort = defaultMonitorPort,
) {
  try {
    await stopNativeExternalDiskMonitoring(context, port);
  } catch (error) {
    publishMonitorFailure(context, error);
    scheduleExternalDiskAudit(context, port);
    return;
  }
  if (!externalDiskMonitoringAllowed(context)) return;
  const identity = currentProjectDiskWatchIdentity(context);
  if (!identity) return;
  const subscriptionGeneration = ++context.runtime.watchSubscriptionGeneration;
  let releaseOwnListener: (() => void) | null = null;
  let receipt: ProjectDiskWatchReceipt | null = null;

  try {
    const unlisten = await port.subscribe((notice) => {
      if (
        context.runtime.watchSubscriptionGeneration !== subscriptionGeneration
        || notice.projectRoot !== identity.expectedProjectRoot
        || notice.runtimeSessionId !== identity.expectedSessionId
      ) return;
      if (context.runtime.watchGeneration === null) {
        context.runtime.pendingWatchNotice = notice;
        return;
      }
      acceptProjectDiskWatchNotice(context, notice, port);
    });
    releaseOwnListener = once(() => {
      if (context.runtime.watchUnlisten === releaseOwnListener) {
        context.runtime.watchUnlisten = null;
      }
      unlisten();
    });
    if (context.runtime.watchSubscriptionGeneration !== subscriptionGeneration) {
      releaseOwnListener();
      return;
    }
    context.runtime.watchUnlisten = releaseOwnListener;
    receipt = await port.startWatch(identity);
    if (
      context.runtime.watchSubscriptionGeneration !== subscriptionGeneration
      || context.runtime.suspended
      || receipt.projectRoot !== identity.expectedProjectRoot
      || receipt.runtimeSessionId !== identity.expectedSessionId
    ) {
      releaseOwnListener();
      await stopOwnedWatch(port, receipt);
      if (context.runtime.watchSubscriptionGeneration === subscriptionGeneration) {
        scheduleExternalDiskAudit(context, port);
      }
      return;
    }

    context.runtime.watchGeneration = receipt.watchGeneration;
    context.runtime.watchStopIdentity = {
      expectedProjectRoot: receipt.projectRoot,
      expectedSessionId: receipt.runtimeSessionId,
      expectedWatchGeneration: receipt.watchGeneration,
    };
    context.runtime.watchRevision = 0;
    const pending = context.runtime.pendingWatchNotice;
    context.runtime.pendingWatchNotice = null;
    if (pending) acceptProjectDiskWatchNotice(context, pending, port);
    scheduleExternalDiskAudit(context, port);
  } catch (error) {
    releaseOwnListener?.();
    if (receipt) await stopOwnedWatch(port, receipt);
    if (context.runtime.watchSubscriptionGeneration !== subscriptionGeneration) return;
    context.runtime.watchGeneration = null;
    context.runtime.watchStopIdentity = null;
    context.runtime.pendingWatchNotice = null;
    publishMonitorFailure(context, error);
    scheduleExternalDiskAudit(context, port);
  }
}

async function stopNativeExternalDiskMonitoring(
  context: ExternalDiskContext,
  port: ExternalDiskMonitorPort,
) {
  const { runtime } = context;
  runtime.watchSubscriptionGeneration += 1;
  const unlisten = runtime.watchUnlisten;
  runtime.watchUnlisten = null;
  unlisten?.();
  runtime.pendingWatchNotice = null;
  runtime.watchEventPending = false;
  const stopIdentity = runtime.watchStopIdentity;
  runtime.watchStopIdentity = null;
  runtime.watchGeneration = null;
  runtime.watchRevision = 0;
  if (!stopIdentity) return;
  const stopGeneration = runtime.watchSubscriptionGeneration;
  try {
    await port.stopWatch(stopIdentity);
  } catch (error) {
    if (
      runtime.watchSubscriptionGeneration === stopGeneration
      && runtime.watchStopIdentity === null
    ) {
      runtime.watchStopIdentity = stopIdentity;
      runtime.watchGeneration = stopIdentity.expectedWatchGeneration;
    }
    throw error;
  }
}

function currentProjectDiskWatchIdentity(context: ExternalDiskContext) {
  const project = context.environment.session.project;
  if (!project?.root || !context.environment.session.runtimeSessionId) return null;
  return {
    expectedProjectRoot: project.root,
    expectedSessionId: context.environment.session.runtimeSessionId,
  };
}

function acceptProjectDiskWatchNotice(
  context: ExternalDiskContext,
  notice: ProjectDiskChangeNotice,
  port: ExternalDiskMonitorPort,
) {
  if (
    notice.watchGeneration !== context.runtime.watchGeneration
    || notice.watchRevision <= context.runtime.watchRevision
  ) return;
  context.runtime.watchRevision = notice.watchRevision;
  context.runtime.watchEventPending = true;
  void drainProjectDiskWatchEvents(context, port);
}

async function drainProjectDiskWatchEvents(
  context: ExternalDiskContext,
  port: ExternalDiskMonitorPort,
) {
  const { environment, runtime } = context;
  if (runtime.watchEventDrainInFlight) return;
  runtime.watchEventDrainInFlight = true;
  try {
    while (runtime.watchEventPending) {
      runtime.watchEventPending = false;
      if (
        runtime.suspended
        || environment.session.transitionLocked
        || environment.session.historyLocked
        || runtime.snapshot.workspaceProjectionRecoveryRequired
      ) continue;
      const lease = currentExternalDiskCheckLease(context);
      if (!lease) continue;
      await runTrackedExternalDiskCheck(context, lease, port);
    }
  } finally {
    runtime.watchEventDrainInFlight = false;
    if (runtime.watchEventPending) void drainProjectDiskWatchEvents(context, port);
  }
}

function scheduleExternalDiskAudit(
  context: ExternalDiskContext,
  port: ExternalDiskMonitorPort,
) {
  clearExternalDiskAudit(context, port);
  if (!externalDiskMonitoringAllowed(context)) return;
  let timerId: number | null = null;
  timerId = port.schedule(() => {
    if (context.runtime.auditTimer !== timerId || context.runtime.suspended) return;
    context.runtime.auditTimer = null;
    if (context.runtime.watchGeneration === null) {
      void ensureNativeExternalDiskMonitoring(context, port);
      return;
    }
    context.runtime.watchEventPending = true;
    void drainProjectDiskWatchEvents(context, port).finally(() => {
      if (externalDiskMonitoringAllowed(context)) scheduleExternalDiskAudit(context, port);
    });
  }, FULL_MANIFEST_AUDIT_INTERVAL);
  context.runtime.auditTimer = timerId;
}

function clearExternalDiskAudit(
  context: ExternalDiskContext,
  port: ExternalDiskMonitorPort,
) {
  if (context.runtime.auditTimer !== null) {
    port.clearSchedule(context.runtime.auditTimer);
  }
  context.runtime.auditTimer = null;
}

async function runTrackedExternalDiskCheck(
  context: ExternalDiskContext,
  scheduledLease: ExternalDiskCheckLease,
  port: ExternalDiskMonitorPort,
): Promise<ExternalDiskCheckLease | null> {
  const { runtime } = context;
  if (!externalDiskCheckLeaseMatches(context, scheduledLease)) return null;
  const existing = runtime.checkInFlight;
  if (existing) {
    if (!externalDiskCheckBelongsToCurrentSession(context, existing)) {
      if (runtime.checkInFlight === existing) runtime.checkInFlight = null;
      return null;
    }
    await existing.promise;
    return externalDiskCheckLeaseMatches(context, existing) ? existing : null;
  }

  const checkGeneration = runtime.checkGeneration + 1;
  runtime.checkGeneration = checkGeneration;
  const checkLease: ExternalDiskCheckLease = {
    projectRoot: scheduledLease.projectRoot,
    runtimeSessionId: scheduledLease.runtimeSessionId,
    projectSessionEpoch: scheduledLease.projectSessionEpoch,
    generation: checkGeneration,
  };
  const operation = port.runCheck(context, checkLease);
  const tracked = { ...checkLease, promise: operation };
  runtime.checkInFlight = tracked;
  try {
    await operation;
  } finally {
    if (runtime.checkInFlight === tracked) runtime.checkInFlight = null;
    if (
      runtime.suspended
      && externalDiskCheckBelongsToCurrentSession(context, tracked)
      && runtime.snapshot.checking
      && !runtime.snapshot.reconciling
    ) {
      finishExternalDiskCheck(context);
    }
  }
  return externalDiskCheckLeaseMatches(context, checkLease) ? checkLease : null;
}

function externalDiskMonitoringAllowed(context: ExternalDiskContext) {
  const { environment, runtime } = context;
  return !runtime.suspended
    && !environment.session.transitionLocked
    && !environment.session.historyLocked
    && !runtime.snapshot.workspaceProjectionRecoveryRequired
    && Boolean(runtime.snapshot.baseline)
    && !runtime.snapshot.baseline?.truncated;
}

function once(operation: () => void) {
  let active = true;
  return () => {
    if (!active) return;
    active = false;
    operation();
  };
}

function publishMonitorFailure(context: ExternalDiskContext, error: unknown) {
  context.environment.projections.setProjectStatus(t("external-disk-monitor-failed", {
    message: errorMessage(error),
  }));
}

async function stopOwnedWatch(
  port: ExternalDiskMonitorPort,
  receipt: ProjectDiskWatchReceipt,
) {
  const identity: ProjectDiskWatchStopIdentity = {
    expectedProjectRoot: receipt.projectRoot,
    expectedSessionId: receipt.runtimeSessionId,
    expectedWatchGeneration: receipt.watchGeneration,
  };
  await port.stopWatch(identity).catch(() => undefined);
}
