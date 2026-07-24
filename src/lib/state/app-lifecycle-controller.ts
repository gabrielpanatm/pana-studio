import { clearAiContextTimer as clearAiContextTimerFromController } from "$lib/state/ai-context-controller";
import {
  startAiCoordinationPolling,
  stopAiCoordinationPolling,
} from "$lib/state/ai-coordination-controller";
import { stopExternalDiskPolling as stopExternalDiskPollingFromController } from "$lib/state/external-disk-controller";
import { clearPreviewTimers as clearPreviewTimersFromController } from "$lib/state/preview-controller";
import { clearStatusDismissTimer as clearStatusDismissTimerFromController } from "$lib/state/status-controller";
import { initUiFromStorage as initUiFromStorageFromController } from "$lib/state/ui-controller";
import type { AppState } from "$lib/state/app.svelte";

export async function initFromStorage(app: AppState, storage: Storage) {
  startAiCoordinationPolling(app);
  initUiFromStorageFromController(app.uiControllerHost(), storage);
  await app.initApplicationSettings();
  try {
    await app.reattachCurrentProjectSession();
  } catch {
    // The reattachment controller already exposes a persistent diagnostic.
    // Startup remains interactive so the operator can inspect/recover it.
  }
}

export function destroyApp(app: AppState) {
  app.previewRuntime.reset();
  app.stopResizeDrag();
  app.terminalController.destroyAll();
  app.codeEditorController?.destroy();
  app.codeEditorController = null;
  clearStatusDismissTimerFromController(app.statusControllerHost());
  if (app.pendingRestoredSelectionTimer !== null) window.clearTimeout(app.pendingRestoredSelectionTimer);
  clearPreviewTimersFromController(app.previewControllerHost());
  clearAiContextTimerFromController(app.aiContextControllerHost());
  stopAiCoordinationPolling(app);
  stopExternalDiskPollingFromController(app.externalDiskControllerHost());
}
