import { listen } from "@tauri-apps/api/event";
import type { AppState } from "$lib/state/app.svelte";
import type { SystemPreferencesSnapshot } from "$lib/types";

export const SYSTEM_PREFERENCES_CHANGED_EVENT = "system-preferences://changed";

export async function listenForSystemPreferences(app: AppState) {
  app.systemPreferencesUnlisten?.();
  app.systemPreferencesUnlisten = await listen<SystemPreferencesSnapshot>(
    SYSTEM_PREFERENCES_CHANGED_EVENT,
    ({ payload }) => {
      app.refreshApplicationSettingsForSystemGeneration(payload.generation);
    },
  );
}
