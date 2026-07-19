import { getCurrentWindow } from "@tauri-apps/api/window";
import type { UnlistenFn } from "@tauri-apps/api/event";
import type { AppState } from "$lib/state/app.svelte";
import { errorMessage } from "$lib/util";

export const NATIVE_WINDOW_CLOSE_REQUESTED_EVENT = "pana-native-window-close-requested";

type NativeWindowCloseRequestPayload = {
  projectRoot: string;
};

export async function registerNativeWindowCloseGuard(app: AppState): Promise<UnlistenFn> {
  return await getCurrentWindow().listen<NativeWindowCloseRequestPayload>(
    NATIVE_WINDOW_CLOSE_REQUESTED_EVENT,
    (event) => {
      void handleNativeWindowCloseRequest(app, event.payload.projectRoot);
    },
  );
}

export async function handleNativeWindowCloseRequest(
  app: AppState,
  requestedProjectRoot: string | null = null,
) {
  if (app.nativeWindowClosePending && isWaitingForProjectCloseDecision(app)) return;
  if (app.nativeWindowCloseInProgress) return;

  app.nativeWindowCloseInProgress = true;
  app.nativeWindowClosePending = true;
  try {
    if (
      requestedProjectRoot
      && app.scannedProject
      && app.scannedProject.root !== requestedProjectRoot
    ) {
      throw new Error(
        `Cererea nativă de închidere aparține proiectului ${requestedProjectRoot}, nu ${app.scannedProject.root}.`,
      );
    }
    const closed = await app.closeCurrentProject(app.scannedProject ? null : requestedProjectRoot);
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
    app.setGlobalStatus(`Închiderea aplicației a fost oprită: ${errorMessage(error)}`, "error");
  } finally {
    app.nativeWindowCloseInProgress = false;
  }
}

export async function closeNativeWindowIfProjectClosed(app: AppState): Promise<boolean> {
  if (
    !app.nativeWindowClosePending
    || app.scannedProject
    || isWaitingForProjectCloseDecision(app)
  ) return false;
  app.nativeWindowClosePending = false;
  await getCurrentWindow().close();
  return true;
}

export function cancelPendingNativeWindowClose(app: AppState) {
  app.nativeWindowClosePending = false;
  app.nativeWindowCloseInProgress = false;
}

function isWaitingForProjectCloseDecision(app: AppState): boolean {
  return app.projectTransitionDecisionRequest?.continuation.kind === "close_project";
}
