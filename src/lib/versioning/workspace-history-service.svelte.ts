import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { ProjectDocumentWorkspaceState } from "$lib/project/document-workspace.svelte";
import type { SourceWorkspaceState } from "$lib/editor/source-workspace.svelte";
import type { CssWorkspaceService } from "$lib/css/workspace-service";
import type { WorkbenchWorkspaceState } from "$lib/workbench/workspace-state.svelte";
import type { PreviewWorkspaceState } from "$lib/preview/workspace-state.svelte";
import type { ProjectDerivedStateService } from "$lib/project/derived-state-service";
import type { WorkspaceAuthorityService } from "$lib/session/workspace-authority-service";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import { scannedCacheKey } from "$lib/project/files";
import { requireProjectWorkspaceUndoRedoCommandReceipt } from "$lib/kernel/project-workspace-undo-redo-receipt";
import { reconcileProjectWorkspaceTopologyAfterHistory } from "$lib/kernel/project-workspace-history-topology";
import {
  readProjectWorkspaceState,
  redoProjectWorkspace,
  undoProjectWorkspace,
} from "$lib/project/io/workspace";
import { rebaseFileBufferDraftSyncProjection } from "$lib/session/file-buffer-draft-sync";
import {
  selectTopbarUndoRedoRoute,
  topbarUndoRedoState,
  type TopbarUndoRedoDirection,
} from "$lib/ui/undo-redo-router";
import type {
  ProjectWorkspaceSnapshot,
  ProjectWorkspaceUndoRedoCommandReceipt,
} from "$lib/project/workspace-contract";
import { errorMessage } from "$lib/util";
import { t } from "$lib/i18n/runtime.svelte";

type KernelUndoRedoContext = Readonly<{
  projectRoot: string;
  runtimeSessionId: string;
  projectSessionEpoch: number;
}>;

export type WorkspaceHistoryServiceDependencies = Readonly<{
  project: ProjectSessionState;
  documents: ProjectDocumentWorkspaceState;
  source: SourceWorkspaceState;
  css: CssWorkspaceService;
  workbench: WorkbenchWorkspaceState;
  preview: PreviewWorkspaceState;
  derived: ProjectDerivedStateService;
  authority: WorkspaceAuthorityService;
  status: GlobalStatusState;
}>;

/** Owns Rust ProjectWorkspace undo/redo and its asynchronous projections. */
export class WorkspaceHistoryService {
  snapshot = $state<ProjectWorkspaceSnapshot | null>(null);
  loading = $state(false);
  private key = "";
  private inFlight = false;
  private readonly dependencies: WorkspaceHistoryServiceDependencies;

  constructor(dependencies: WorkspaceHistoryServiceDependencies) {
    this.dependencies = dependencies;
  }

  get state() {
    return topbarUndoRedoState({
      kernelCanUndo: Boolean(this.snapshot?.history.canUndo),
      kernelCanRedo: Boolean(this.snapshot?.history.canRedo),
    });
  }

  synchronize() {
    const project = this.dependencies.project;
    if (!project.project || !project.root || !project.runtimeSessionId) {
      this.key = "";
      this.snapshot = null;
      return;
    }
    const nextKey = `${project.root}\u0000${project.runtimeSessionId}\u0000${project.workspace?.revision ?? -1}`;
    if (nextKey === this.key || this.loading) return;
    this.key = nextKey;
    void this.refresh();
  }

  async refresh() {
    if (!this.dependencies.project.project) {
      this.snapshot = null;
      return null;
    }
    this.loading = true;
    try {
      this.snapshot = await readProjectWorkspaceState();
      return this.snapshot;
    } catch (error) {
      this.snapshot = null;
      this.dependencies.status.set(
        t("workbench-history-read-failed", { error: errorMessage(error) }),
        "error",
      );
      return null;
    } finally {
      this.loading = false;
    }
  }

  async run(direction: TopbarUndoRedoDirection) {
    if (this.dependencies.project.project) await this.refresh();
    const route = selectTopbarUndoRedoRoute(direction, {
      kernelCanUndo: Boolean(this.snapshot?.history.canUndo),
      kernelCanRedo: Boolean(this.snapshot?.history.canRedo),
    });
    if (route === "workspace") await this.runKernel(direction);
  }

  private async runKernel(direction: TopbarUndoRedoDirection) {
    if (this.inFlight) return { ok: false, message: t("workbench-history-in-flight") } as const;
    const project = this.dependencies.project;
    const context: KernelUndoRedoContext = {
      projectRoot: project.root,
      runtimeSessionId: project.runtimeSessionId,
      projectSessionEpoch: project.epoch,
    };
    if (!context.projectRoot || !context.runtimeSessionId) {
      return { ok: false, message: t("workbench-history-session-required") } as const;
    }

    this.inFlight = true;
    let operationReceipt: ProjectWorkspaceUndoRedoCommandReceipt | null = null;
    try {
      const before = this.snapshot ?? await this.refresh();
      this.requireCurrent(context);
      const target = direction === "undo" ? before?.history.nextUndo : before?.history.nextRedo;
      if (!before || !target) {
        return {
          ok: false,
          message: direction === "undo"
            ? t("workbench-history-no-undo")
            : t("workbench-history-no-redo"),
        } as const;
      }
      this.dependencies.status.set(
        direction === "undo"
          ? t("workbench-history-applying-undo")
          : t("workbench-history-applying-redo"),
        "saving",
      );
      const identity = {
        expectedProjectRoot: before.projectRoot,
        expectedSessionId: before.runtimeSessionId,
        expectedRevision: before.revision,
        expectedTransactionId: target.transactionId,
      };
      const receipt = direction === "undo"
        ? await undoProjectWorkspace(identity)
        : await redoProjectWorkspace(identity);
      operationReceipt = receipt;
      this.requireCurrent(context);
      requireProjectWorkspaceUndoRedoCommandReceipt(receipt, {
        projectRoot: context.projectRoot,
        runtimeSessionId: context.runtimeSessionId,
        direction,
        revisionBefore: before.revision,
        transactionId: target.transactionId,
      });
      this.dependencies.css.clearLiveProperties();
      if (receipt.workbench) this.dependencies.workbench.snapshot = receipt.workbench.snapshot;
      project.workspace = receipt.workspace;
      this.snapshot = receipt.workspace;
      let previewWarning: string | null = null;
      let canvasPatchApplied = false;
      if (receipt.canvasPatch) {
        try {
          await this.dependencies.preview.applyCanvasPatch(receipt.canvasPatch);
          canvasPatchApplied = true;
        } catch (error) {
          previewWarning = errorMessage(error);
        }
      }
      this.applyLocalProjection(receipt, context);
      const label = direction === "undo"
        ? t("workbench-history-undo-label")
        : t("workbench-history-redo-label");
      this.dependencies.status.set(
        previewWarning
          ? t("workbench-history-applied-preview-warning", { operation: label, warning: previewWarning })
          : t("workbench-history-applied", { operation: label }),
        previewWarning ? "unsaved" : "restored",
      );
      void this.settleCanonicalProjection(receipt, context, canvasPatchApplied).then((warning) => {
        if (!warning || !this.contextIsCurrent(context)) return;
        this.dependencies.status.set(
          t("workbench-history-applied-preview-warning", { operation: label, warning }),
          "unsaved",
        );
      });
      return { ok: true, snapshot: receipt.workspace.history, receipt } as const;
    } catch (error) {
      const label = direction === "undo"
        ? t("workbench-history-undo-label")
        : t("workbench-history-redo-label");
      const detail = errorMessage(error);
      const message = operationReceipt
        ? t("workbench-history-projection-failed", { operation: label, error: detail })
        : t("workbench-history-not-applied", { operation: label, error: detail });
      this.dependencies.status.set(message, "error");
      await this.refresh();
      return { ok: false, message } as const;
    } finally {
      this.inFlight = false;
    }
  }

  private contextIsCurrent(context: KernelUndoRedoContext) {
    const project = this.dependencies.project;
    return project.root === context.projectRoot
      && project.runtimeSessionId === context.runtimeSessionId
      && project.epoch === context.projectSessionEpoch;
  }

  private requireCurrent(context: KernelUndoRedoContext) {
    if (!this.contextIsCurrent(context)) {
      throw new Error(t("workbench-history-session-changed", { operation: "Undo/Redo" }));
    }
  }

  private applyLocalProjection(
    receipt: ProjectWorkspaceUndoRedoCommandReceipt,
    context: KernelUndoRedoContext,
  ) {
    this.requireCurrent(context);
    const { project, source, documents } = this.dependencies;
    const entry = receipt.result.entry;
    for (const projection of receipt.result.documents) {
      rebaseFileBufferDraftSyncProjection(projection.relativePath, projection.snapshot);
      const cacheKey = scannedCacheKey({ relativePath: projection.relativePath });
      if (projection.snapshot) {
        source.sourceCache = { ...source.sourceCache, [cacheKey]: projection.snapshot.text };
        if (documents.activeScannedPath === projection.relativePath) {
          source.source = projection.snapshot.text;
        }
      } else {
        const nextCache = { ...source.sourceCache };
        delete nextCache[cacheKey];
        source.sourceCache = nextCache;
        if (documents.activeScannedPath === projection.relativePath) source.source = "";
      }
    }
    if (entry.pageJsPaths.length > 0) project.jsRefreshToken += 1;
    if (entry.documentPaths.some((path) => /\.(?:css|scss)$/i.test(path))) {
      source.notifyCssSourceChanged();
    }
    project.refreshToken += 1;
  }

  private async settleCanonicalProjection(
    receipt: ProjectWorkspaceUndoRedoCommandReceipt,
    context: KernelUndoRedoContext,
    canvasPatchApplied: boolean,
  ) {
    const entry = receipt.result.entry;
    const service = this;
    try {
      await reconcileProjectWorkspaceTopologyAfterHistory({
        get activeScannedPath() { return service.dependencies.documents.activeScannedPath; },
        rescanCurrentProjectForCommittedHistory: (historyContext, path, options) => (
          this.dependencies.derived.rescanCommittedHistory(historyContext, path, options)
        ),
      }, receipt, {
        projectRoot: context.projectRoot,
        runtimeSessionId: context.runtimeSessionId,
        projectSessionEpoch: context.projectSessionEpoch,
        workspaceRevision: receipt.workspace.revision,
      });
      if (!this.contextIsCurrent(context)) return null;
      await this.dependencies.authority.projectLatest({
        reason: "history-restore",
        minimumWorkspaceRevision: receipt.workspace.revision,
        requestedPaths: [...new Set([...entry.documentPaths, ...entry.pageJsPaths])].sort(),
      });
      return null;
    } catch (error) {
      if (
        !this.contextIsCurrent(context)
        || (this.dependencies.project.workspace?.revision ?? 0) > receipt.workspace.revision
      ) return null;
      let warning = errorMessage(error);
      if (canvasPatchApplied && receipt.canvasPatch) {
        try {
          await this.dependencies.preview.rollbackCanvasPatch(receipt.canvasPatch);
        } catch (rollbackError) {
          warning = `${warning} ${t("structural-projection-canvas-rollback-refused", {
            message: errorMessage(rollbackError),
          })}`;
        }
      }
      return warning;
    }
  }
}
