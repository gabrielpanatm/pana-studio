import { getCurrentWindow } from "@tauri-apps/api/window";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { errorMessage } from "$lib/util";
import { t } from "$lib/i18n/runtime.svelte";
import type { GlobalStatusKind } from "$lib/status/global-status";
import type { ProjectTransitionFrontendLeaseOwner } from "$lib/state/project-transition-frontend-lease";

export const NATIVE_WINDOW_CLOSE_REQUESTED_EVENT = "pana-native-window-close-requested";

type NativeWindowCloseRequestPayload = {
  projectRoot: string;
};

export type NativeWindowCloseControllerHost = {
  nativeWindowClosePending: boolean;
  nativeWindowCloseInProgress: boolean;
  projectTransitionFrontendLeaseActive: boolean;
  projectTransitionFrontendLease?: { kind: string } | null;
  scannedProject: { root: string } | null;
  projectTransitionDecisionRequest: {
    continuation: { kind: string };
  } | null;
  closeCurrentProject: (
    detachedProjectRoot?: string | null,
    leaseOwner?: ProjectTransitionFrontendLeaseOwner,
  ) => Promise<boolean>;
  waitForProjectTransitionFrontendLeaseIdle: () => Promise<void>;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
};

export async function registerNativeWindowCloseGuard(
  app: NativeWindowCloseControllerHost,
): Promise<UnlistenFn> {
  return await getCurrentWindow().listen<NativeWindowCloseRequestPayload>(
    NATIVE_WINDOW_CLOSE_REQUESTED_EVENT,
    (event) => {
      void handleNativeWindowCloseRequest(app, event.payload.projectRoot);
    },
  );
}

export async function handleNativeWindowCloseRequest(
  app: NativeWindowCloseControllerHost,
  requestedProjectRoot: string | null = null,
) {
  if (app.nativeWindowClosePending && isWaitingForProjectCloseDecision(app)) return;
  if (app.nativeWindowCloseInProgress) return;

  app.nativeWindowCloseInProgress = true;
  app.nativeWindowClosePending = true;
  try {
    let waitedForActiveTransition = false;
    const activeTransitionKind = app.projectTransitionFrontendLease?.kind ?? null;
    if (app.projectTransitionFrontendLeaseActive) {
      waitedForActiveTransition = true;
      await app.waitForProjectTransitionFrontendLeaseIdle();
    }
    if (
      waitedForActiveTransition
      && activeTransitionKind === "close"
      && !app.scannedProject
      && !isWaitingForProjectCloseDecision(app)
    ) {
      return;
    }
    if (
      requestedProjectRoot
      && app.scannedProject
      && app.scannedProject.root !== requestedProjectRoot
    ) {
      throw new Error(
        t("native-close-project-mismatch", {
          requested: requestedProjectRoot,
          current: app.scannedProject.root,
        }),
      );
    }
    const closed = await app.closeCurrentProject(
      app.scannedProject ? null : requestedProjectRoot,
      "native-window-close",
    );
    if (!closed && !isWaitingForProjectCloseDecision(app)) {
      app.nativeWindowClosePending = false;
    }
    if (app.scannedProject && !isWaitingForProjectCloseDecision(app)) {
      app.nativeWindowClosePending = false;
    }
  } catch (error) {
    // A failed pre-close drain must keep the native window and the current
    // ProjectSession alive. The originating controller already exposes the
    // detailed surface status; this prevents an unhandled event rejection.
    app.nativeWindowClosePending = false;
    app.setGlobalStatus(t("native-close-stopped", {
      message: errorMessage(error),
    }), "error");
  } finally {
    app.nativeWindowCloseInProgress = false;
  }
}

export async function closeNativeWindowIfProjectClosed(
  app: NativeWindowCloseControllerHost,
): Promise<boolean> {
  if (
    !app.nativeWindowClosePending
    || app.scannedProject
    || isWaitingForProjectCloseDecision(app)
  ) return false;
  app.nativeWindowClosePending = false;
  await getCurrentWindow().close();
  return true;
}

export function cancelPendingNativeWindowClose(app: NativeWindowCloseControllerHost) {
  app.nativeWindowClosePending = false;
  app.nativeWindowCloseInProgress = false;
}

function isWaitingForProjectCloseDecision(app: NativeWindowCloseControllerHost): boolean {
  return app.projectTransitionDecisionRequest?.continuation.kind === "close_project";
}
