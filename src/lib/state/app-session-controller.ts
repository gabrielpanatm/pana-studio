import { untrack } from "svelte";
import {
  dismissNotification as dismissNotificationFromCenter,
  type AppNotification,
} from "$lib/notifications/center";
import {
  EXTERNAL_CHANGE_KEEP_SESSION_ACTION_ID,
  EXTERNAL_CHANGE_NOTIFICATION_ID,
  EXTERNAL_CHANGE_RELOAD_ACTION_ID,
} from "$lib/state/external-disk-controller";
import {
  AI_COORDINATION_ACCEPT_DISK_ACTION_ID,
  AI_COORDINATION_NOTIFICATION_ID,
  reloadAuthorizedAiReconciliationFromDisk,
} from "$lib/state/ai-coordination-controller";
import {
  applyGlobalStatusSnapshot,
  publishGlobalStatus as publishGlobalStatusFromController,
} from "$lib/state/status-controller";
import {
  globalStatusInputFromKind,
  type GlobalStatusEscalationRequest,
  type GlobalStatusInput,
  type GlobalStatusKind,
  type GlobalStatusPublishOptions,
  type GlobalStatusSnapshot,
} from "$lib/status/global-status";
import {
  publishKernelGlobalStatus,
  readKernelGlobalStatus,
  readProjectFile,
  resolveKernelGlobalStatus,
} from "$lib/project/io";
import { scannedCacheKey } from "$lib/project/files";
import { createEmptyHtmlPending } from "$lib/state/app-helpers";
import {
  updateInspectorPendingSource,
  type InspectorPendingSource,
} from "$lib/state/inspector-pending";
import type { AppState } from "$lib/state/app.svelte";
import type {
  HtmlPendingArea,
  InspectorPendingArea,
} from "$lib/types";
import { t } from "$lib/i18n/runtime.svelte";

function projectKernelFailure(
  app: AppState,
  error: unknown,
) {
  const detail = error instanceof Error ? error.message : String(error);
  publishGlobalStatusFromController(app.statusControllerHost(), {
    code: "global-status.kernel-command-failed",
    source: "global-status",
    severity: "error",
    message: t("app-session-global-status-kernel-failed"),
    detail,
    lifecycle: "until_resolved",
    escalation: "notification",
    dedupeKey: "global-status.kernel",
    resolutionKey: "global-status.kernel",
    notification: {
      title: t("app-session-global-status-unavailable"),
      message: detail,
      level: "error",
    },
  });
}

function queueKernelStatusCommand(
  app: AppState,
  command: () => Promise<GlobalStatusSnapshot>,
) {
  const operation = app.globalStatusKernelTail
    .catch(() => undefined)
    .then(async () => {
      try {
        const snapshot = await command();
        applyGlobalStatusSnapshot(app.statusControllerHost(), snapshot);
      } catch (error) {
        projectKernelFailure(app, error);
      }
    });
  app.globalStatusKernelTail = operation;
  return operation;
}

export function setGlobalStatus(
  app: AppState,
  text: string,
  kind: GlobalStatusKind,
  options: GlobalStatusPublishOptions = {},
) {
  const input = globalStatusInputFromKind(text, kind, options);
  void queueKernelStatusCommand(app, () => publishKernelGlobalStatus(input));
}

export function escalateGlobalStatus(
  app: AppState,
  notification: GlobalStatusEscalationRequest,
) {
  const input: GlobalStatusInput = {
    code: notification.id,
    source: notification.id.split(".")[0] || "application",
    severity: notification.level === "error"
      ? "error"
      : notification.level === "warning"
        ? "warning"
        : "info",
    phase: "settled",
    message: notification.statusMessage ?? notification.title,
    detail: notification.message,
    lifecycle: "until_resolved",
    escalation: "notification",
    dedupeKey: notification.id,
    resolutionKey: notification.id,
    notification: {
      title: notification.title,
      message: notification.message,
      level: notification.level,
      actionLabel: notification.actionLabel,
      actionId: notification.actionId,
      secondaryActionLabel: notification.secondaryActionLabel,
      secondaryActionId: notification.secondaryActionId,
    },
  };
  void queueKernelStatusCommand(app, () => publishKernelGlobalStatus(input));
}

export function clearNotification(app: AppState, id: string) {
  const hasProjection = app.notifications.some((notification) => notification.id === id);
  const hasOpenStatus = app.globalStatusEvents.some((event) => (
    event.resolution === "open"
    && (
      event.id === id
      || event.dedupeKey === id
      || event.resolutionKey === id
    )
  ));
  if (!hasProjection && !hasOpenStatus && !app.dismissedNotificationIds.has(id)) return;
  void queueKernelStatusCommand(app, () => resolveKernelGlobalStatus(id));
}

export function refreshGlobalStatusFromKernel(app: AppState) {
  return queueKernelStatusCommand(app, readKernelGlobalStatus);
}

export function dismissNotification(app: AppState, id: string) {
  app.notifications = dismissNotificationFromCenter(app.notifications, id);
  app.dismissedNotificationIds = new Set([...app.dismissedNotificationIds, id]);
}

export async function handleNotificationAction(app: AppState, notification: AppNotification, actionId: string) {
  if (
    notification.id === AI_COORDINATION_NOTIFICATION_ID
    && actionId === AI_COORDINATION_ACCEPT_DISK_ACTION_ID
  ) {
    await reloadAuthorizedAiReconciliationFromDisk(app);
    app.setGlobalStatus(
      t("app-session-disk-state-adopted"),
      "restored",
    );
    return;
  }
  if (notification.id === EXTERNAL_CHANGE_NOTIFICATION_ID) {
    if (actionId === EXTERNAL_CHANGE_RELOAD_ACTION_ID) {
      await reloadAuthorizedAiReconciliationFromDisk(app);
      return;
    }
    if (actionId === EXTERNAL_CHANGE_KEEP_SESSION_ACTION_ID) {
      app.setGlobalStatus(
        t("app-session-external-conflict-kept"),
        "idle",
      );
      return;
    }
  }
  await app.saveActiveFile();
}

export function setInspectorPending(
  app: AppState,
  area: InspectorPendingArea,
  pending: boolean,
  source: InspectorPendingSource = "session",
) {
  const aggregatePending = updateInspectorPendingSource(app.inspectorPendingSources, area, source, pending);
  const current = untrack(() => app.inspectorPending);
  if (current[area] === aggregatePending) return;
  app.markEditorMutation();
  app.inspectorPending = { ...current, [area]: aggregatePending };
}

export function setHtmlPending(app: AppState, area: HtmlPendingArea, pending: boolean) {
  if (app.htmlPending[area] === pending) return;
  const next = { ...app.htmlPending, [area]: pending };
  app.htmlPending = next;
  app.setInspectorPending("html", Object.values(next).some(Boolean));
}

export function clearHtmlPending(app: AppState) {
  if (Object.values(app.htmlPending).some(Boolean)) app.markEditorMutation();
  app.htmlPending = createEmptyHtmlPending();
  app.setInspectorPending("html", false);
}

export async function refreshCurrentSession(app: AppState) {
  if (!app.scannedProject) return;
  await app.flushInteractiveEditorDrafts("manual");
  if (app.activeScannedPath) {
    const source = await readProjectFile(app.activeScannedPath);
    app.source = source;
    app.sourceCache = {
      ...app.sourceCache,
      [scannedCacheKey({ relativePath: app.activeScannedPath })]: source,
    };
  }
  app.refreshToken += 1;
  await app.requestPreviewRefresh("session-refresh");
  app.setGlobalStatus(t("app-session-projection-reloaded"), "restored");
}
