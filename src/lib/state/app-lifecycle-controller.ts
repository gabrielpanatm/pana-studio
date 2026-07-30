import { clearAiContextTimer as clearAiContextTimerFromController } from "$lib/state/ai-context-controller";
import { listenForSystemPreferences } from "$lib/application/system-preferences";
import { t } from "$lib/i18n/runtime.svelte";
import {
  startAiCoordinationEvents,
  stopAiCoordinationEvents,
} from "$lib/state/ai-coordination-controller";
import { stopExternalDiskMonitoring } from "$lib/state/external-disk-controller";
import { clearPreviewTimers as clearPreviewTimersFromController } from "$lib/state/preview-controller";
import { clearGlobalStatusExpiryTimer } from "$lib/state/status-controller";
import { initUiFromStorage as initUiFromStorageFromController } from "$lib/state/ui-controller";
import type { AppState } from "$lib/state/app.svelte";

export async function initFromStorage(app: AppState, storage: Storage) {
  await app.refreshGlobalStatusFromKernel();
  startAiCoordinationEvents(app);
  initUiFromStorageFromController(app.uiControllerHost(), storage);
  try {
    await listenForSystemPreferences(app);
  } catch (error) {
    app.escalateGlobalStatus({
      id: "application.settings.system-listener",
      level: "warning",
      title: t("diagnostic-system-preferences-live-unavailable"),
      message: error instanceof Error ? error.message : String(error),
    });
  }
  await app.initApplicationSettings();
  try {
    const reattached = await app.reattachCurrentProjectSession();
    if (!reattached) await app.refreshStartupFlow();
  } catch {
    // The reattachment controller already exposes a persistent diagnostic.
    // Startup remains interactive so the operator can inspect/recover it.
    await app.refreshStartupFlow().catch(() => undefined);
  }
}

export function destroyApp(app: AppState) {
  app.systemPreferencesUnlisten?.();
  app.systemPreferencesUnlisten = null;
  app.previewRuntime.reset();
  app.stopResizeDrag();
  app.terminalController.destroyAll();
  app.codeEditorController?.destroy();
  app.codeEditorController = null;
  clearGlobalStatusExpiryTimer(app.statusControllerHost());
  if (app.pendingRestoredSelectionTimer !== null) window.clearTimeout(app.pendingRestoredSelectionTimer);
  clearPreviewTimersFromController(app.previewControllerHost());
  clearAiContextTimerFromController(app.aiContextControllerHost());
  stopAiCoordinationEvents(app);
  stopExternalDiskMonitoring(app.externalDiskControllerHost());
}
