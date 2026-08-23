import { listen } from "@tauri-apps/api/event";
import type { UnlistenFn } from "@tauri-apps/api/event";
import {
  readApplicationSettings,
  saveApplicationSettings,
} from "$lib/application/io";
import { l10n, t } from "$lib/i18n/runtime.svelte";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import {
  applyApplicationBootProjection,
  storeApplicationBootProjection,
} from "$lib/system-preferences/boot-projection";
import type {
  ApplicationSettingsPatch,
  ApplicationSettingsSnapshot,
  ApplicationTheme,
  ApplicationThemePreference,
  SystemPreferencesSnapshot,
} from "$lib/application/contracts";
import { errorMessage } from "$lib/util";
import { contrastingTextColor } from "$lib/state/app-helpers";

export const SYSTEM_PREFERENCES_CHANGED_EVENT = "system-preferences://changed";

type ApplicationPreferencesStatus = Pick<GlobalStatusState, "clear" | "escalate">;

export type ApplicationPreferencesGateway = {
  read: () => Promise<ApplicationSettingsSnapshot>;
  save: (
    expectedRevision: number,
    patch: ApplicationSettingsPatch,
  ) => Promise<ApplicationSettingsSnapshot>;
  listen: (
    handler: (snapshot: SystemPreferencesSnapshot) => void,
  ) => Promise<UnlistenFn>;
};

const defaultGateway: ApplicationPreferencesGateway = {
  read: readApplicationSettings,
  save: saveApplicationSettings,
  listen: (handler) => listen<SystemPreferencesSnapshot>(
    SYSTEM_PREFERENCES_CHANGED_EVENT,
    ({ payload }) => handler(payload),
  ),
};

function initialTheme(): ApplicationTheme {
  if (typeof document === "undefined") return "dark";
  return document.documentElement.dataset.panaTheme === "light" ? "light" : "dark";
}

/** Owns persisted application preferences and their live system projection. */
export class ApplicationPreferencesState {
  theme = $state<ApplicationTheme>(initialTheme());
  locale = $state("en-US");
  direction = $state<"ltr" | "rtl">("ltr");
  accent = $state("#1d7f6a");
  snapshot = $state<ApplicationSettingsSnapshot | null>(null);
  loading = $state(false);

  private readonly status: ApplicationPreferencesStatus;
  private readonly gateway: ApplicationPreferencesGateway;
  private saveTail: Promise<void> = Promise.resolve();
  private refreshTail: Promise<void> = Promise.resolve();
  private listenerGeneration = 0;
  private listenerStart: Promise<void> | null = null;
  private unlisten: UnlistenFn | null = null;

  constructor(
    status: ApplicationPreferencesStatus,
    gateway: ApplicationPreferencesGateway = defaultGateway,
  ) {
    this.status = status;
    this.gateway = gateway;
  }

  start() {
    if (this.unlisten) return Promise.resolve();
    if (this.listenerStart) return this.listenerStart;
    const generation = ++this.listenerGeneration;
    const operation = this.gateway.listen((system) => {
      if (generation !== this.listenerGeneration) return;
      this.refreshForSystemGeneration(system.generation);
    }).then((unlisten) => {
      if (generation !== this.listenerGeneration) {
        unlisten();
        return;
      }
      this.unlisten?.();
      this.unlisten = unlisten;
    }).catch((error) => {
      if (generation !== this.listenerGeneration) return;
      this.status.escalate({
        id: "application.settings.system-listener",
        level: "warning",
        title: t("diagnostic-system-preferences-live-unavailable"),
        message: errorMessage(error),
      });
    });
    const pending = operation.finally(() => {
      if (this.listenerStart === pending) this.listenerStart = null;
    });
    this.listenerStart = pending;
    return this.listenerStart;
  }

  stop() {
    this.listenerGeneration += 1;
    this.listenerStart = null;
    this.unlisten?.();
    this.unlisten = null;
  }

  destroy() {
    this.stop();
  }

  async initialize() {
    this.syncDocumentTheme();
    this.loading = true;
    try {
      await this.applySnapshot(await this.gateway.read());
    } catch (error) {
      this.status.escalate({
        id: "application.settings.load",
        level: "warning",
        title: t("diagnostic-application-settings-load-failed"),
        message: errorMessage(error),
      });
    } finally {
      this.loading = false;
    }
  }

  toggleTheme() {
    this.theme = this.theme === "dark" ? "light" : "dark";
    this.syncDocumentTheme();
    void this.persistTheme({ mode: "fixed", value: this.theme });
  }

  setTheme(theme: ApplicationTheme) {
    this.setThemePreference({ mode: "fixed", value: theme });
  }

  setThemePreference(preference: ApplicationThemePreference) {
    if (preference.mode === "fixed") {
      this.theme = preference.value;
      this.syncDocumentTheme();
    }
    void this.persistTheme(preference);
  }

  persistPatch(
    patch: ApplicationSettingsPatch,
    failureTitle = t("diagnostic-application-settings-save-failed"),
  ) {
    const operation = this.saveTail.then(async () => {
      const current = this.snapshot ?? await this.gateway.read();
      await this.applySnapshot(await this.gateway.save(current.revision, patch));
      this.status.clear("application.settings.load");
      this.status.clear("application.settings.save");
    });
    this.saveTail = operation.then(
      () => undefined,
      (error) => {
        this.status.escalate({
          id: "application.settings.save",
          level: "warning",
          title: failureTitle,
          message: errorMessage(error),
        });
      },
    );
    return this.saveTail;
  }

  refreshForSystemGeneration(generation: number) {
    if ((this.snapshot?.system.generation ?? 0) >= generation) return;
    const operation = this.refreshTail.then(async () => {
      if ((this.snapshot?.system.generation ?? 0) >= generation) return;
      await this.applySnapshot(await this.gateway.read());
    });
    this.refreshTail = operation.catch((error) => {
      this.status.escalate({
        id: "application.settings.system-refresh",
        level: "warning",
        title: t("diagnostic-application-settings-system-refresh-failed"),
        message: errorMessage(error),
      });
    });
  }

  persistBlockPropertiesLayout(height: number, collapsed: boolean) {
    const normalizedHeight = Math.max(140, Math.min(520, Math.round(height)));
    const operation = this.saveTail.then(async () => {
      const current = this.snapshot ?? await this.gateway.read();
      if (
        current.blockPropertiesHeight === normalizedHeight
        && current.blockPropertiesCollapsed === collapsed
      ) return;
      await this.applySnapshot(await this.gateway.save(current.revision, {
        blockPropertiesHeight: normalizedHeight,
        blockPropertiesCollapsed: collapsed,
      }));
      this.status.clear("application.settings.save");
    });
    this.saveTail = operation.then(
      () => undefined,
      (error) => {
        this.status.escalate({
          id: "application.settings.save",
          level: "warning",
          title: t("diagnostic-application-settings-layout-save-failed"),
          message: errorMessage(error),
        });
      },
    );
    return this.saveTail;
  }

  async settled() {
    await Promise.all([this.saveTail, this.refreshTail]);
  }

  private persistTheme(theme: ApplicationThemePreference) {
    return this.persistPatch({ theme });
  }

  private async applySnapshot(snapshot: ApplicationSettingsSnapshot) {
    await l10n.setLocale(snapshot.effective.locale);
    this.snapshot = snapshot;
    this.locale = snapshot.effective.locale;
    this.direction = snapshot.effective.direction;
    this.accent = snapshot.effective.accent;
    this.theme = snapshot.effective.theme;
    this.syncDocumentProjection(snapshot);
  }

  private syncDocumentTheme() {
    if (typeof document === "undefined") return;
    document.documentElement.dataset.panaTheme = this.theme;
    document.documentElement.style.colorScheme = this.theme;
    document.querySelector('meta[name="theme-color"]')?.setAttribute(
      "content",
      this.theme === "light" ? "#edf1ee" : "#111315",
    );
  }

  private syncDocumentProjection(snapshot: ApplicationSettingsSnapshot) {
    if (typeof document === "undefined") return;
    this.syncDocumentTheme();
    document.documentElement.lang = this.locale;
    document.documentElement.dir = this.direction;
    document.documentElement.dataset.panaLocale = this.locale;
    document.documentElement.dataset.panaContrast = snapshot.system.contrast ?? "normal";
    document.documentElement.dataset.panaReducedMotion =
      snapshot.system.reducedMotion === true ? "true" : "false";
    document.documentElement.style.setProperty("--brand", this.accent);
    document.documentElement.style.setProperty(
      "--brand-strong",
      `color-mix(in srgb, ${this.accent} 70%, ${this.theme === "dark" ? "white" : "black"})`,
    );
    document.documentElement.style.setProperty(
      "--brand-soft",
      `color-mix(in srgb, ${this.accent} ${this.theme === "dark" ? "19%" : "11%"}, transparent)`,
    );
    document.documentElement.style.setProperty(
      "--focus-ring",
      `color-mix(in srgb, ${this.accent} 72%, ${this.theme === "dark" ? "white" : "black"})`,
    );
    document.documentElement.style.setProperty(
      "--text-on-accent",
      contrastingTextColor(this.accent),
    );
    applyApplicationBootProjection(document, snapshot.boot);
    if (typeof window !== "undefined") {
      storeApplicationBootProjection(window.localStorage, snapshot.boot);
    }
  }
}
