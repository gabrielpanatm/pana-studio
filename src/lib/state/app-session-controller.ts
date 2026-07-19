import { untrack } from "svelte";
import {
  dismissNotification as dismissNotificationFromCenter,
  upsertNotification,
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
import { setGlobalStatus as setGlobalStatusFromController } from "$lib/state/status-controller";
import {
  createCssRequestIdentity,
  createProjectTextFile,
  getScssVariables,
  readProjectFile,
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
  SaveState,
} from "$lib/types";
import {
  previewStructuralCommandIdentity,
  previewStructuralSessionLeaseMatches,
  requireCurrentPreviewStructuralSession,
  runInPreviewStructuralLane,
} from "$lib/kernel/preview-structural-lane";

export function setGlobalStatus(app: AppState, text: string, kind: SaveState) {
  setGlobalStatusFromController(app.statusControllerHost(), text, kind);
  if (kind === "error") {
    notify(app, {
      id: `status.error.${text}`,
      level: "error",
      title: "Eroare",
      message: text,
    });
  }
}

export function notify(app: AppState, notification: Omit<AppNotification, "createdAt">) {
  app.notifications = upsertNotification(app.notifications, notification);
}

export function clearNotification(app: AppState, id: string) {
  app.notifications = app.notifications.filter((item) => item.id !== id);
  if (app.dismissedNotificationIds.has(id)) {
    app.dismissedNotificationIds = new Set([...app.dismissedNotificationIds].filter((item) => item !== id));
  }
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
      "Starea stabilă de pe disc a fost adoptată și reproiectată; autoritatea utilizatorului este restaurată.",
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
        "Sesiunea rămâne deschisă, dar conflictul extern nu a fost acceptat; Save rămâne protejat de Disk Conflict Gate.",
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
  app.setGlobalStatus("Proiecția ProjectWorkspace a fost reîncărcată în editor și preview.", "restored");
}

export async function createProjectFile(app: AppState, relativePath: string, content: string) {
  await runInPreviewStructuralLane(app, async (lease) => {
    try {
      const identity = previewStructuralCommandIdentity(lease);
      const receipt = await createProjectTextFile(relativePath, content, identity);
      requireCurrentPreviewStructuralSession(app, lease);
      const createdPath = receipt.relativePath;
      if (!createdPath) {
        throw new Error("Receipt-ul creării nu conține path-ul fișierului.");
      }
      requireCurrentPreviewStructuralSession(app, lease);
      await app.rescanCurrentProjectWithinStructuralLane(lease, createdPath, { strict: true });
      requireCurrentPreviewStructuralSession(app, lease);
    } catch (error) {
      if (!previewStructuralSessionLeaseMatches(app, lease)) return;
      throw error;
    }
  });
}

export async function afterSave(app: AppState) {
  const identity = createCssRequestIdentity(app.sessionProjectRoot, app.kernelProjectSessionId);
  await app.requestPreviewRefresh("after-save");
  app.scssVariables = await getScssVariables(identity).catch(() => app.scssVariables);
}
