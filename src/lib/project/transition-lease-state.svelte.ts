import { tick } from "svelte";
import { contextMenu } from "$lib/context-menu/store.svelte";
import { drainPreviewStructuralLanes } from "$lib/kernel/preview-structural-lane";
import {
  requireCurrentProjectTransitionFrontendLease,
  runWithProjectTransitionFrontendLease,
  type ProjectTransitionFrontendLease,
  type ProjectTransitionFrontendLeaseRequest,
} from "$lib/state/project-transition-frontend-lease";
import { t } from "$lib/i18n/runtime.svelte";

export type ProjectTransitionLeaseDependencies = {
  guards: () => Readonly<{
    aiEditLocked: boolean;
    aiRecoveryReloadAuthorized: boolean;
    historyLocked: boolean;
  }>;
  cancelEditorDrafts: () => void;
  invalidatePreview: () => void;
  invalidateSourceGraph: () => void;
  quiesceInteractions: () => void;
  drainActiveSave: () => Promise<void>;
  suspendExternalDisk: () => Promise<void>;
  recoverExternalDiskAfterFailure: () => void;
  resumeExternalDisk: () => void;
};

/** Serializes every frontend project transition under one owned lease. */
export class ProjectTransitionLeaseState {
  active = $state.raw<ProjectTransitionFrontendLease | null>(null);
  generation = $state(0);

  get isActive() {
    return this.active !== null;
  }

  private readonly idleWaiters = new Set<() => void>();
  private readonly dependencies: ProjectTransitionLeaseDependencies;

  constructor(dependencies: ProjectTransitionLeaseDependencies) {
    this.dependencies = dependencies;
  }

  async run<T>(
    request: ProjectTransitionFrontendLeaseRequest,
    operation: (lease: ProjectTransitionFrontendLease) => Promise<T>,
  ): Promise<T> {
    const guards = this.dependencies.guards();
    if (guards.aiEditLocked && !guards.aiRecoveryReloadAuthorized) {
      throw new Error(t("workbench-transition-ai-blocked"));
    }
    if (guards.historyLocked) {
      throw new Error(t("workbench-transition-history-blocked"));
    }
    const owner = this;
    return await runWithProjectTransitionFrontendLease(
      {
        get projectTransitionFrontendLease() { return owner.active; },
        set projectTransitionFrontendLease(lease) { owner.active = lease; },
        get projectTransitionFrontendLeaseGeneration() { return owner.generation; },
        set projectTransitionFrontendLeaseGeneration(generation) { owner.generation = generation; },
      },
      request,
      {
        onAcquire: () => {
          this.dependencies.cancelEditorDrafts();
          this.dependencies.invalidatePreview();
          this.dependencies.invalidateSourceGraph();
          contextMenu.close();
          this.dependencies.quiesceInteractions();
        },
        quiesce: async (lease) => {
          await tick();
          this.require(lease);
          await this.dependencies.drainActiveSave();
          this.require(lease);
          await this.dependencies.suspendExternalDisk();
          this.require(lease);
          await drainPreviewStructuralLanes();
          this.require(lease);
        },
        onRelease: () => {
          try {
            this.dependencies.recoverExternalDiskAfterFailure();
          } finally {
            try {
              this.dependencies.resumeExternalDisk();
            } finally {
              const waiters = [...this.idleWaiters];
              this.idleWaiters.clear();
              for (const resolve of waiters) resolve();
            }
          }
        },
      },
      operation,
    );
  }

  require(lease: ProjectTransitionFrontendLease) {
    requireCurrentProjectTransitionFrontendLease({
      projectTransitionFrontendLease: this.active,
    }, lease);
  }

  waitForIdle(): Promise<void> {
    if (!this.active) return Promise.resolve();
    return new Promise((resolve) => {
      this.idleWaiters.add(resolve);
      if (!this.active) {
        this.idleWaiters.delete(resolve);
        resolve();
      }
    });
  }
}
