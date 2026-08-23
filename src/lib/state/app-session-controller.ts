import type { AppNotification } from "$lib/notifications/center";
import {
  EXTERNAL_CHANGE_KEEP_SESSION_ACTION_ID,
  EXTERNAL_CHANGE_NOTIFICATION_ID,
  EXTERNAL_CHANGE_RELOAD_ACTION_ID,
} from "$lib/session/external-disk/contracts";
import {
  AI_COORDINATION_ACCEPT_DISK_ACTION_ID,
  AI_COORDINATION_NOTIFICATION_ID,
  reloadAuthorizedAiReconciliationFromDisk,
} from "$lib/state/ai-coordination-controller";
import type { GlobalStatusKind } from "$lib/status/global-status";
import {
  readProjectFile,
} from "$lib/project/io/workspace";
import { scannedCacheKey } from "$lib/project/files";
import type { ProjectScan } from "$lib/project/lifecycle-contract";
import type { EditFlushReason } from "$lib/session/edit-flush-registry";
import type { PreviewRefreshReason } from "$lib/preview/controlled";
import type { AiCoordinationState } from "$lib/ai/coordination-state.svelte";
import type { ProjectDocumentWorkspaceState } from "$lib/project/document-workspace.svelte";
import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { SourceWorkspaceState } from "$lib/editor/source-workspace.svelte";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import { t } from "$lib/i18n/runtime.svelte";
import { errorMessage } from "$lib/util";

export type AppSessionControllerHost = {
  aiCoordination: Pick<AiCoordinationState, "controllerHost">;
  activeScannedPath: string | null;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
  saveActiveFile: () => Promise<unknown>;
  scannedProject: ProjectScan | null;
  flushInteractiveEditorDrafts: (reason: EditFlushReason) => Promise<void>;
  source: string;
  sourceCache: Record<string, string>;
  refreshToken: number;
  requestPreviewRefresh: (reason: PreviewRefreshReason) => Promise<boolean>;
};

export type AppSessionServiceDependencies = {
  ai: AiCoordinationState;
  status: GlobalStatusState;
  project: ProjectSessionState;
  documents: ProjectDocumentWorkspaceState;
  source: SourceWorkspaceState;
  save: () => Promise<unknown>;
  flushDrafts: (reason: EditFlushReason) => Promise<void>;
  requestPreviewRefresh: (reason: PreviewRefreshReason) => Promise<boolean>;
};

/** User-facing session refresh and notification actions over domain owners. */
export class AppSessionService {
  private readonly host: AppSessionControllerHost;

  constructor(dependencies: AppSessionServiceDependencies) {
    const { ai, status, project, documents, source } = dependencies;
    this.host = {
      aiCoordination: ai,
      get activeScannedPath() { return documents.activeScannedPath; },
      setGlobalStatus: (text, kind) => status.set(text, kind),
      saveActiveFile: dependencies.save,
      get scannedProject() { return project.project; },
      flushInteractiveEditorDrafts: dependencies.flushDrafts,
      get source() { return source.source; },
      set source(nextSource) { source.source = nextSource; },
      get sourceCache() { return source.sourceCache; },
      set sourceCache(cache) { source.sourceCache = cache; },
      get refreshToken() { return project.refreshToken; },
      set refreshToken(token) { project.refreshToken = token; },
      requestPreviewRefresh: dependencies.requestPreviewRefresh,
    };
  }

  async handleNotification(notification: AppNotification, actionId: string) {
    try {
      await handleNotificationAction(this.host, notification, actionId);
    } catch (error) {
      this.host.setGlobalStatus(t("workbench-notification-action-failed", {
        action: notification.actionLabel ?? actionId,
        message: errorMessage(error),
      }), "error");
    }
  }

  refresh() {
    return refreshCurrentSession(this.host);
  }
}

export async function handleNotificationAction(
  app: AppSessionControllerHost,
  notification: AppNotification,
  actionId: string,
) {
  if (
    notification.id === AI_COORDINATION_NOTIFICATION_ID
    && actionId === AI_COORDINATION_ACCEPT_DISK_ACTION_ID
  ) {
    await reloadAuthorizedAiReconciliationFromDisk(app.aiCoordination.controllerHost());
    app.setGlobalStatus(
      t("app-session-disk-state-adopted"),
      "restored",
    );
    return;
  }
  if (notification.id === EXTERNAL_CHANGE_NOTIFICATION_ID) {
    if (actionId === EXTERNAL_CHANGE_RELOAD_ACTION_ID) {
      await reloadAuthorizedAiReconciliationFromDisk(app.aiCoordination.controllerHost());
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

export async function refreshCurrentSession(app: AppSessionControllerHost) {
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
