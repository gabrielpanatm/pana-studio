import {
  registerNativeWindowCloseGuard,
  type NativeWindowCloseControllerHost,
} from "$lib/state/native-window-close-controller";
import { ReactiveEffectsLifecycle } from "$lib/lifecycle/reactive-effects.svelte";
import type { ApplicationShellState } from "$lib/application/shell-state.svelte";
import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { ProjectStartupState } from "$lib/project/startup-state.svelte";
import type { ProjectTransitionLeaseState } from "$lib/project/transition-lease-state.svelte";
import type { GlobalStatusKind } from "$lib/status/global-status";
import type { ProjectTransitionFrontendLeaseOwner } from "$lib/state/project-transition-frontend-lease";

export type NativeWindowCloseLifecycleDependencies = {
  shell: ApplicationShellState;
  project: Pick<ProjectSessionState, "project">;
  startup: Pick<ProjectStartupState, "transitionDecision">;
  transition: ProjectTransitionLeaseState;
  closeProject: (
    detachedProjectRoot?: string | null,
    leaseOwner?: ProjectTransitionFrontendLeaseOwner,
  ) => Promise<boolean>;
  setStatus: (text: string, kind: GlobalStatusKind) => void;
};

export function nativeWindowCloseControllerHost(
  dependencies: NativeWindowCloseLifecycleDependencies,
): NativeWindowCloseControllerHost {
  const { shell, project, startup, transition } = dependencies;
  return {
    get nativeWindowClosePending() { return shell.nativeWindowClosePending; },
    set nativeWindowClosePending(pending) { shell.nativeWindowClosePending = pending; },
    get nativeWindowCloseInProgress() { return shell.nativeWindowCloseInProgress; },
    set nativeWindowCloseInProgress(inProgress) { shell.nativeWindowCloseInProgress = inProgress; },
    get projectTransitionFrontendLeaseActive() { return transition.isActive; },
    get projectTransitionFrontendLease() { return transition.active; },
    get scannedProject() { return project.project; },
    get projectTransitionDecisionRequest() { return startup.transitionDecision; },
    closeCurrentProject: dependencies.closeProject,
    waitForProjectTransitionFrontendLeaseIdle: () => transition.waitForIdle(),
    setGlobalStatus: dependencies.setStatus,
  };
}

/** Owns the native close listener for one mounted application composition. */
export class NativeWindowCloseLifecycle {
  private readonly effects: ReactiveEffectsLifecycle;

  constructor(dependencies: NativeWindowCloseLifecycleDependencies) {
    const host = nativeWindowCloseControllerHost(dependencies);
    this.effects = new ReactiveEffectsLifecycle([
      () => {
        let disposed = false;
        let unlisten: (() => void) | null = null;
        void registerNativeWindowCloseGuard(host).then((cleanup) => {
          if (disposed) cleanup();
          else unlisten = cleanup;
        });
        return () => {
          disposed = true;
          unlisten?.();
        };
      },
    ]);
  }

  start() {
    return this.effects.start();
  }

  stop() {
    return this.effects.stop();
  }
}
