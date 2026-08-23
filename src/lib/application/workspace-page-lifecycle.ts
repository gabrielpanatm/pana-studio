import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import {
  destroyApplicationRuntime,
  initializeApplicationRuntime,
  type ApplicationRuntimeLifecycleDependencies,
} from "$lib/application/runtime-lifecycle";
import type { ApplicationPreferencesState } from "$lib/application/preferences.svelte";
import type { LifecycleGroup } from "$lib/lifecycle/group";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import type { WorkspaceLayoutState } from "$lib/ui/workspace-layout.svelte";
import { installSmoothWheelScrolling } from "$lib/ui/smooth-wheel";
import {
  nativeZoomListenerOptions,
  preventNativeGestureZoom,
  preventNativeZoomWheel,
  resetNativeWebviewZoom,
  resetNativeZoomIfVisualViewportChanged,
} from "$lib/ui/native-zoom";
import type { ProjectLifecycleSnapshot } from "$lib/project/lifecycle-contract";
import { t } from "$lib/i18n/runtime.svelte";

export type WorkspacePageLifecycleResources = Readonly<{
  domains: LifecycleGroup;
  layout: Pick<WorkspaceLayoutState, "initialize" | "destroy">;
  preferences: Pick<ApplicationPreferencesState, "start" | "initialize" | "destroy">;
  status: Pick<GlobalStatusState, "destroy">;
  unregisterRuntimeProbe: () => void;
}>;

export type WorkspacePageLifecycleEvents = Readonly<{
  message: (event: MessageEvent) => void;
  shortcut: (event: KeyboardEvent) => void;
  deleteShortcut: (event: KeyboardEvent) => void;
  projectLifecycle: (snapshot: ProjectLifecycleSnapshot) => void;
}>;

export type WorkspacePageLifecyclePlatform = Readonly<{
  listenProjectLifecycle: (
    handler: (snapshot: ProjectLifecycleSnapshot) => void,
  ) => Promise<() => void>;
  installSmoothScrolling: (target: Window) => () => void;
  showWindow: () => Promise<void>;
  resetNativeZoom: () => void;
}>;

export type WorkspacePageLifecycleDependencies = Readonly<{
  resources: WorkspacePageLifecycleResources;
  runtime: ApplicationRuntimeLifecycleDependencies;
  events: WorkspacePageLifecycleEvents;
  platform?: WorkspacePageLifecyclePlatform;
}>;

const defaultPlatform: WorkspacePageLifecyclePlatform = {
  listenProjectLifecycle: (handler) => listen<ProjectLifecycleSnapshot>(
    "project-lifecycle-changed",
    ({ payload }) => handler(payload),
  ),
  installSmoothScrolling: installSmoothWheelScrolling,
  showWindow: async () => {
    try {
      await getCurrentWindow().show();
    } catch {
      // Browser-only development does not expose a native Tauri window.
    }
  },
  resetNativeZoom: resetNativeWebviewZoom,
};

/** Owns every window resource and asynchronous bootstrap step for the application page. */
export class WorkspacePageLifecycle {
  private active = false;
  private generation = 0;
  private bootstrapFrame: number | null = null;
  private bootstrapTimer: number | null = null;
  private revealFrame: number | null = null;
  private bootRemovalTimer: number | null = null;
  private stopProjectLifecycle: (() => void) | null = null;
  private stopSmoothScrolling: (() => void) | null = null;
  private readonly dependencies: WorkspacePageLifecycleDependencies;
  private readonly platform: WorkspacePageLifecyclePlatform;

  constructor(dependencies: WorkspacePageLifecycleDependencies) {
    this.dependencies = dependencies;
    this.platform = dependencies.platform ?? defaultPlatform;
  }

  get started() {
    return this.active;
  }

  start() {
    if (this.active) return false;
    this.active = true;
    const generation = ++this.generation;
    const { events, resources } = this.dependencies;
    try {
      resources.domains.start();
      this.registerProjectLifecycleListener(generation);
      this.stopSmoothScrolling = this.platform.installSmoothScrolling(window);
      window.addEventListener("message", events.message);
      window.addEventListener("keydown", events.shortcut, { capture: true });
      window.addEventListener("keydown", events.deleteShortcut, { capture: true });
      window.addEventListener("wheel", preventNativeZoomWheel, nativeZoomListenerOptions);
      window.addEventListener("gesturestart", preventNativeGestureZoom, nativeZoomListenerOptions);
      window.addEventListener("gesturechange", preventNativeGestureZoom, nativeZoomListenerOptions);
      window.addEventListener("gestureend", preventNativeGestureZoom, nativeZoomListenerOptions);
      window.visualViewport?.addEventListener("resize", this.handleVisualViewportChange);
      window.visualViewport?.addEventListener("scroll", this.handleVisualViewportChange);
      this.platform.resetNativeZoom();
      this.scheduleBootstrap(generation);
      return true;
    } catch (error) {
      this.stop();
      throw error;
    }
  }

  stop() {
    if (!this.active) return false;
    this.active = false;
    this.generation += 1;
    this.cancelScheduledWork();
    this.stopProjectLifecycle?.();
    this.stopProjectLifecycle = null;
    this.stopSmoothScrolling?.();
    this.stopSmoothScrolling = null;

    const { events, resources, runtime } = this.dependencies;
    window.removeEventListener("message", events.message);
    window.removeEventListener("keydown", events.shortcut, { capture: true });
    window.removeEventListener("keydown", events.deleteShortcut, { capture: true });
    window.removeEventListener("wheel", preventNativeZoomWheel, nativeZoomListenerOptions);
    window.removeEventListener("gesturestart", preventNativeGestureZoom, nativeZoomListenerOptions);
    window.removeEventListener("gesturechange", preventNativeGestureZoom, nativeZoomListenerOptions);
    window.removeEventListener("gestureend", preventNativeGestureZoom, nativeZoomListenerOptions);
    window.visualViewport?.removeEventListener("resize", this.handleVisualViewportChange);
    window.visualViewport?.removeEventListener("scroll", this.handleVisualViewportChange);

    resources.domains.stop();
    resources.layout.destroy();
    resources.preferences.destroy();
    resources.status.destroy();
    destroyApplicationRuntime(runtime);
    resources.unregisterRuntimeProbe();
    return true;
  }

  private registerProjectLifecycleListener(generation: number) {
    void this.platform.listenProjectLifecycle(this.dependencies.events.projectLifecycle).then((stop) => {
      if (!this.isCurrent(generation)) stop();
      else this.stopProjectLifecycle = stop;
    });
  }

  private scheduleBootstrap(generation: number) {
    this.bootstrapFrame = window.requestAnimationFrame(() => {
      this.bootstrapFrame = null;
      if (!this.isCurrent(generation)) return;
      this.bootstrapTimer = window.setTimeout(() => {
        this.bootstrapTimer = null;
        if (!this.isCurrent(generation)) return;
        void this.initialize(generation);
      }, 0);
    });
  }

  private async initialize(generation: number) {
    const { resources, runtime } = this.dependencies;
    resources.layout.initialize(window.localStorage);
    await resources.preferences.start();
    if (!this.isCurrent(generation)) return;
    await resources.preferences.initialize();
    if (!this.isCurrent(generation)) return;
    try {
      await initializeApplicationRuntime(runtime);
    } finally {
      if (this.isCurrent(generation)) this.scheduleReveal(generation);
      else destroyApplicationRuntime(runtime);
    }
  }

  private scheduleReveal(generation: number) {
    const bootScreen = document.getElementById("pana-boot-screen");
    if (bootScreen) {
      bootScreen.setAttribute("aria-label", t("application-loading-label"));
      const subtitle = bootScreen.querySelector<HTMLElement>(".boot-subtitle");
      if (subtitle) subtitle.textContent = t("application-loading-subtitle");
    }
    this.revealFrame = window.requestAnimationFrame(() => {
      this.revealFrame = null;
      if (!this.isCurrent(generation)) return;
      this.hideBootScreen();
      void this.platform.showWindow();
    });
  }

  private hideBootScreen() {
    const bootScreen = document.getElementById("pana-boot-screen");
    if (!bootScreen) return;
    bootScreen.classList.add("is-hidden");
    this.bootRemovalTimer = window.setTimeout(() => {
      this.bootRemovalTimer = null;
      bootScreen.remove();
    }, 120);
  }

  private readonly handleVisualViewportChange = () => {
    resetNativeZoomIfVisualViewportChanged();
  };

  private isCurrent(generation: number) {
    return this.active && this.generation === generation;
  }

  private cancelScheduledWork() {
    if (this.bootstrapFrame !== null) window.cancelAnimationFrame(this.bootstrapFrame);
    if (this.revealFrame !== null) window.cancelAnimationFrame(this.revealFrame);
    if (this.bootstrapTimer !== null) window.clearTimeout(this.bootstrapTimer);
    if (this.bootRemovalTimer !== null) window.clearTimeout(this.bootRemovalTimer);
    this.bootstrapFrame = null;
    this.revealFrame = null;
    this.bootstrapTimer = null;
    this.bootRemovalTimer = null;
  }
}
