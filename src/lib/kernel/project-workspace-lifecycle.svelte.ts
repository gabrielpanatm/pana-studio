import { t } from "$lib/i18n/runtime.svelte";
import { ReactiveEffectsLifecycle } from "$lib/lifecycle/reactive-effects.svelte";
import { subscribeProjectWorkspaceMutations } from "$lib/kernel/project-workspace-events";
import {
  projectWorkspacePreviewRevisionIsPublished,
  scheduleProjectWorkspaceDerivedPreviewProjection,
  type ProjectWorkspacePreviewHost,
} from "$lib/kernel/project-workspace-preview-coordinator";
import {
  readProjectWorkspaceState,
} from "$lib/project/io/workspace";
import type { GlobalStatusEscalationRequest } from "$lib/status/global-status";
import type { AiCoordinationState } from "$lib/ai/coordination-state.svelte";
import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { FileExplorerWorkspaceState } from "$lib/workbench/file-explorer-state.svelte";
import type { WorkbenchWorkspaceState } from "$lib/workbench/workspace-state.svelte";

export type ProjectWorkspaceLifecycleDependencies = {
  project: ProjectSessionState;
  ai: Pick<AiCoordinationState, "snapshot">;
  explorer: FileExplorerWorkspaceState;
  workbench: WorkbenchWorkspaceState;
  preview: ProjectWorkspacePreviewHost;
  escalateStatus: (request: GlobalStatusEscalationRequest) => void;
};

/** Owns read-only frontend projections of the Rust ProjectWorkspace authority. */
export class ProjectWorkspaceLifecycle {
  private readonly effects: ReactiveEffectsLifecycle;

  constructor({
    project,
    ai,
    explorer,
    workbench,
    preview,
    escalateStatus,
  }: ProjectWorkspaceLifecycleDependencies) {
    this.effects = new ReactiveEffectsLifecycle([
      // The structural Explorer namespace changes only with ProjectSession/Workspace.
      // Lightweight document navigation is projected directly by its consumers.
      () => {
        const projectRoot = project.root;
        const sessionId = project.runtimeSessionId;
        const workspaceRevision = project.workspace?.revision ?? null;
        ai.snapshot?.coordinationRevision;
        if (
          !projectRoot
          || !sessionId
          || workspaceRevision === null
        ) {
          explorer.reset();
          return;
        }
        const timer = window.setTimeout(() => {
          void explorer.refresh();
        }, 24);
        return () => window.clearTimeout(timer);
      },

      // Workspace mutation events advance a separate projection epoch.
      () => {
        let disposed = false;
        let unlisten: (() => void) | null = null;
        void subscribeProjectWorkspaceMutations((notice) => {
          if (
            notice.projectRoot === project.root
            && notice.runtimeSessionId === project.runtimeSessionId
          ) {
            const workspaceAlreadyVisible = (
              project.workspace?.projectRoot === notice.projectRoot
              && project.workspace.runtimeSessionId === notice.runtimeSessionId
              && project.workspace.revision >= notice.workspaceRevision
            );
            const previewAlreadyVisible = !notice.previewProjectionRequired
              || projectWorkspacePreviewRevisionIsPublished(
                notice.projectRoot,
                notice.runtimeSessionId,
                notice.workspaceRevision,
              );
            if (!workspaceAlreadyVisible) project.workspaceMutationEpoch += 1;
            if (notice.previewProjectionRequired && !previewAlreadyVisible) {
              scheduleProjectWorkspaceDerivedPreviewProjection(
                preview,
                "workspace-mutation",
                notice.workspaceRevision,
              );
            }
          }
        }).then((cleanup) => {
          if (disposed) cleanup();
          else unlisten = cleanup;
        });
        return () => {
          disposed = true;
          unlisten?.();
        };
      },

      // Mirror Rust-owned Workbench navigation only when bootstrap has not
      // already hydrated the exact live ProjectSession. Document activation
      // belongs to bootstrap and must not be replayed while Canvas confirms
      // its initial projection.
      () => {
        const projectRoot = project.root;
        const sessionId = project.runtimeSessionId;
        if (!projectRoot || !sessionId) {
          workbench.reset();
          return;
        }
        if (workbench.isHydrated(sessionId)) return;
        let cancelled = false;
        const timer = window.setTimeout(() => {
          if (
            cancelled
            || project.root !== projectRoot
            || project.runtimeSessionId !== sessionId
            || workbench.isHydrated(sessionId)
          ) return;
          void workbench.refresh().catch((error) => {
            if (cancelled) return;
            workbench.acceptSnapshot(null);
            escalateStatus({
              id: "workbench.refresh",
              level: "warning",
              title: t("workbench-refresh-failed-title"),
              message: error instanceof Error ? error.message : String(error),
            });
          });
        }, 40);
        return () => {
          cancelled = true;
          window.clearTimeout(timer);
        };
      },

      // Mirror the exact Rust workspace revision without allowing stale reads to win.
      () => {
        const projectRoot = project.root;
        const sessionId = project.runtimeSessionId;
        project.workspaceMutationEpoch;
        project.saveRequest;
        if (!projectRoot || !sessionId) {
          project.workspace = null;
          return;
        }
        let cancelled = false;
        const timer = window.setTimeout(() => {
          void readProjectWorkspaceState()
            .then((snapshot) => {
              if (
                cancelled
                || project.root !== projectRoot
                || project.runtimeSessionId !== sessionId
              ) return;
              if (
                snapshot?.projectRoot === projectRoot
                && snapshot.runtimeSessionId === sessionId
              ) {
                project.workspace = snapshot;
              }
            })
            .catch(() => {
              // A temporary derived-read failure cannot erase a confirmed Rust snapshot.
            });
        }, 40);
        return () => {
          cancelled = true;
          window.clearTimeout(timer);
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
