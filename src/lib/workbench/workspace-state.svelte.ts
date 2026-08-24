import { untrack } from "svelte";
import type { EditFlushReason } from "$lib/session/edit-flush-registry";
import type { GlobalStatusEscalationRequest } from "$lib/status/global-status";
import type { CenterView } from "$lib/application/contracts";
import type {
  ProjectFile,
  ProjectScan,
} from "$lib/project/lifecycle-contract";
import type {
  WorkbenchActivity,
  WorkbenchBottomPanelView,
  WorkbenchCanvasMode,
  WorkbenchCanvasPreset,
  WorkbenchCanvasViewportSnapshot,
  WorkbenchDocumentActivationCacheOutcome,
  WorkbenchDocumentActivationPhase,
  WorkbenchDocumentActivationSnapshot,
  WorkbenchDocumentPresentation,
  WorkbenchDocumentSnapshot,
  WorkbenchIntent,
  WorkbenchSnapshot,
  WorkbenchSplit,
  WorkbenchSurface,
} from "$lib/workbench/contracts";
import { t } from "$lib/i18n/runtime.svelte";
import { errorMessage } from "$lib/util";
import { WorkbenchProjectionController } from "$lib/workbench/controller";
import { activeWorkbenchDocument } from "$lib/workbench/document-presentation";

const MIN_PREVIEW_ZOOM = 25;
const MAX_PREVIEW_ZOOM = 200;

export type WorkbenchAuthority = Readonly<{
  projectRoot: string;
  runtimeSessionId: string;
  project: ProjectScan | null;
  activeRelativePath: string | null;
  centerView: CenterView;
}>;

export type WorkbenchWorkspaceCommands = {
  authority: () => WorkbenchAuthority;
  flushDrafts: (reason: EditFlushReason) => Promise<void>;
  loadProjectFile: (
    file: ProjectFile,
    options: {
      strict?: boolean;
      skipDraftFlush?: boolean;
      activateTemplateWorkbench?: boolean;
      syncWorkbench?: boolean;
    },
  ) => Promise<unknown>;
  setCenterView: (view: CenterView) => void;
  projectActiveDocument: (
    document: WorkbenchDocumentSnapshot | null,
    previous: WorkbenchDocumentSnapshot | null,
  ) => void;
  synchronizeTerminalPane: (open: boolean) => void;
  clearStatus: (id: string) => void;
  escalateStatus: (request: GlobalStatusEscalationRequest) => void;
};

/** Owns the durable Workbench projection and its canvas/pane presentation. */
export class WorkbenchWorkspaceState {
  private snapshotState = $state<WorkbenchSnapshot | null>(null);
  documentActivation = $state<WorkbenchDocumentActivationSnapshot>(
    emptyDocumentActivation(),
  );
  previewDevice = $state<"desktop" | "tablet" | "mobile">("desktop");
  previewZoom = $state(100);
  canvasMode = $state<WorkbenchCanvasMode>("fit");
  canvasPreset = $state<WorkbenchCanvasPreset>("desktop");
  previewWidthPx = $state(1_440);
  rulers = $state(true);

  private hydratedRuntimeSessionId = "";
  private readonly projection: WorkbenchProjectionController;
  private readonly commands: WorkbenchWorkspaceCommands;

  get snapshot(): WorkbenchSnapshot | null {
    return this.snapshotState;
  }

  constructor(commands: WorkbenchWorkspaceCommands) {
    this.commands = commands;
    const state = this;
    this.projection = new WorkbenchProjectionController(() => {
      const authority = this.commands.authority();
      return {
        sessionProjectRoot: authority.projectRoot,
        kernelProjectSessionId: authority.runtimeSessionId,
        get workbenchSnapshot() {
          return state.snapshot;
        },
        set workbenchSnapshot(snapshot) {
          state.publishSnapshot(snapshot);
        },
        projectActiveDocument(previous) {
          state.projectActiveDocument(previous);
        },
      };
    });
  }

  reset() {
    // Reset can be called from project lifecycle effects. Keep every nested
    // state read out of the caller's dependency graph so the writes below do
    // not make that effect subscribe to and reactivate itself.
    untrack(() => {
      this.projection.reset();
      this.hydratedRuntimeSessionId = "";
      this.documentActivation = {
        ...emptyDocumentActivation(),
        serial: this.documentActivation.serial + 1,
      };
    });
  }

  get activeDocument() {
    return activeWorkbenchDocument(this.snapshot);
  }

  get activeDocumentPresentation(): WorkbenchDocumentPresentation | null {
    return this.activeDocument?.presentation ?? null;
  }

  private publishSnapshot(snapshot: WorkbenchSnapshot | null) {
    const previous = activeWorkbenchDocument(this.snapshot);
    this.snapshotState = snapshot;
    this.projectActiveDocument(previous);
  }

  /** Accepts a Rust-confirmed snapshot through the single presentation boundary. */
  acceptSnapshot(snapshot: WorkbenchSnapshot | null) {
    this.publishSnapshot(snapshot);
  }

  private projectActiveDocument(previous: WorkbenchDocumentSnapshot | null) {
    this.commands.projectActiveDocument(activeWorkbenchDocument(this.snapshot), previous);
  }

  beginDocumentActivation(serial: number, document: WorkbenchDocumentSnapshot) {
    this.documentActivation = {
      serial,
      phase: "applying",
      documentId: document.documentId,
      relativePath: document.relativePath,
      surface: document.surface,
      cacheOutcome: "unknown",
      diagnostic: null,
      metrics: emptyDocumentActivationMetrics(),
    };
  }

  updateDocumentActivation(
    serial: number,
    patch: {
      phase?: WorkbenchDocumentActivationPhase;
      cacheOutcome?: WorkbenchDocumentActivationCacheOutcome;
      diagnostic?: string | null;
      metrics?: Partial<WorkbenchDocumentActivationSnapshot["metrics"]>;
    },
  ) {
    if (this.documentActivation.serial !== serial) return false;
    this.documentActivation = {
      ...this.documentActivation,
      ...patch,
      metrics: {
        ...this.documentActivation.metrics,
        ...patch.metrics,
      },
    };
    return true;
  }

  isHydrated(runtimeSessionId: string) {
    return this.hydratedRuntimeSessionId === runtimeSessionId;
  }

  refresh() {
    return this.projection.refresh();
  }

  apply(intent: WorkbenchIntent) {
    return this.projection.apply(intent);
  }

  async openDocument(file: ProjectFile, centerView: CenterView) {
    return await this.projection.openDocument(file, centerView);
  }

  async setActiveDocumentSurface(relativePath: string, centerView: CenterView) {
    return await this.projection.setActiveDocumentSurface(relativePath, centerView);
  }

  hydrateBootstrap(snapshot: WorkbenchSnapshot) {
    const authority = this.commands.authority();
    if (
      snapshot.projectRoot !== authority.projectRoot
      || snapshot.runtimeSessionId !== authority.runtimeSessionId
    ) throw new Error("Snapshot-ul Workbench din bootstrap aparține altei sesiuni.");
    this.publishSnapshot(snapshot);
    this.projectCanvas(snapshot.canvasViewport);
    const group = snapshot.groups.find(
      (candidate) => candidate.groupId === snapshot.activeGroupId,
    );
    const document = group?.documents.find(
      (candidate) => candidate.documentId === group.activeDocumentId,
    );
    this.hydratedRuntimeSessionId = snapshot.runtimeSessionId;
    this.commands.synchronizeTerminalPane(
      snapshot.bottomPanel.open && snapshot.bottomPanel.activeView === "terminal",
    );
    this.projectActivity(snapshot.activeActivity, document?.surface ?? "visual");
  }

  private projectCanvas(viewport: WorkbenchCanvasViewportSnapshot) {
    this.canvasMode = viewport.mode;
    this.canvasPreset = viewport.preset;
    this.previewWidthPx = viewport.widthPx;
    this.previewZoom = viewport.zoomPercent;
    this.rulers = viewport.showRulers;
    this.previewDevice = viewport.mode === "fit"
      ? "desktop"
      : viewport.preset === "mobile"
      ? "mobile"
      : viewport.preset === "tablet"
        ? "tablet"
        : viewport.preset === "custom" && viewport.widthPx <= 600
          ? "mobile"
          : viewport.preset === "custom" && viewport.widthPx <= 1_100
            ? "tablet"
            : "desktop";
  }

  async setCanvasViewport(viewport: Partial<WorkbenchCanvasViewportSnapshot>) {
    const current = this.snapshot?.canvasViewport ?? {
      mode: this.canvasMode,
      preset: this.canvasPreset,
      widthPx: this.previewWidthPx,
      zoomPercent: this.previewZoom,
      showRulers: this.rulers,
    } satisfies WorkbenchCanvasViewportSnapshot;
    const next: WorkbenchCanvasViewportSnapshot = {
      ...current,
      ...viewport,
      widthPx: Math.round(viewport.widthPx ?? current.widthPx),
      zoomPercent: Math.round(viewport.zoomPercent ?? current.zoomPercent),
    };
    try {
      const receipt = await this.projection.apply({ kind: "set_canvas_viewport", viewport: next });
      this.projectCanvas(receipt.snapshot.canvasViewport);
      this.commands.clearStatus("workbench.canvas-viewport");
      return receipt;
    } catch (error) {
      this.commands.escalateStatus({
        id: "workbench.canvas-viewport",
        level: "warning",
        title: t("workbench-canvas-viewport-failed"),
        message: errorMessage(error),
      });
      return null;
    }
  }

  setPreviewZoom(value: number) {
    const rounded = Math.round(value);
    this.previewZoom = Math.min(MAX_PREVIEW_ZOOM, Math.max(MIN_PREVIEW_ZOOM, rounded));
  }

  resetPreviewZoom() {
    this.previewZoom = 100;
  }

  async setSplit(split: WorkbenchSplit) {
    try {
      const authority = this.commands.authority();
      if (split === "none") {
        const receipt = await this.projection.apply({ kind: "set_split", split });
        if (authority.activeRelativePath) {
          await this.projection.setActiveDocumentSurface(
            authority.activeRelativePath,
            authority.centerView,
          );
        }
        this.commands.clearStatus("workbench.split");
        return receipt;
      }
      if (!authority.activeRelativePath) throw new Error(t("workbench-split-document-required"));
      const document = this.activeDocument;
      if (!document || document.presentation !== "html") {
        throw new Error("Modul split Vizual | Cod este disponibil numai pentru HTML.");
      }
      const secondarySurface: WorkbenchSurface = "code";
      const receipt = await this.projection.apply({
        kind: "configure_synchronized_split",
        split,
        relativePath: authority.activeRelativePath,
        secondarySurface,
        presentation: document.presentation,
      });
      this.commands.clearStatus("workbench.split");
      return receipt;
    } catch (error) {
      this.commands.escalateStatus({
        id: "workbench.split",
        level: "warning",
        title: t("workbench-split-update-failed"),
        message: errorMessage(error),
      });
      return null;
    }
  }

  async setSplitRatio(ratioBasisPoints: number) {
    try {
      const receipt = await this.projection.apply({
        kind: "set_split_ratio",
        ratioBasisPoints: Math.round(ratioBasisPoints),
      });
      this.commands.clearStatus("workbench.split-ratio");
      return receipt;
    } catch (error) {
      this.commands.escalateStatus({
        id: "workbench.split-ratio",
        level: "warning",
        title: t("workbench-split-ratio-save-failed"),
        message: errorMessage(error),
      });
      return null;
    }
  }

  async setBottomPanel(
    open: boolean,
    activeView: WorkbenchBottomPanelView = "terminal",
  ) {
    try {
      const receipt = await this.projection.apply({ kind: "set_bottom_panel", open, activeView });
      this.commands.synchronizeTerminalPane(
        receipt.snapshot.bottomPanel.open
          && receipt.snapshot.bottomPanel.activeView === "terminal",
      );
      this.commands.clearStatus("workbench.bottom-panel");
      return true;
    } catch (error) {
      this.commands.escalateStatus({
        id: "workbench.bottom-panel",
        level: "warning",
        title: t("workbench-bottom-panel-update-failed"),
        message: errorMessage(error),
      });
      return false;
    }
  }

  async setActivity(activity: WorkbenchActivity) {
    await this.commands.flushDrafts("template-switch");
    const receipt = await this.projection.apply({ kind: "set_activity", activity });
    const group = receipt.snapshot.groups.find(
      (candidate) => candidate.groupId === receipt.snapshot.activeGroupId,
    );
    const document = group?.documents.find(
      (candidate) => candidate.documentId === group.activeDocumentId,
    );
    const authority = this.commands.authority();
    if (activity === "editor" && document && authority.project) {
      const file = authority.project.files.find(
        (candidate) => candidate.relativePath === document.relativePath,
      );
      if (file && authority.activeRelativePath !== file.relativePath) {
        await this.commands.loadProjectFile(file, { strict: true, syncWorkbench: false });
      }
    }
    this.projectActivity(activity, document?.surface ?? "visual");
    return receipt;
  }

  async openContentPage(relativePath: string) {
    await this.commands.flushDrafts("template-switch");
    const receipt = await this.projection.apply({ kind: "open_content_page", relativePath });
    this.projectActivity("content", "code");
    return receipt;
  }

  private projectActivity(activity: WorkbenchActivity, surface: "visual" | "code") {
    if (activity === "editor") {
      this.commands.setCenterView(surface === "code" ? "code" : "preview");
    } else if (activity === "audit") {
      this.commands.setCenterView("kernel");
    }
  }
}

function emptyDocumentActivationMetrics() {
  return {
    intentMs: null,
    resolveMs: null,
    loadMs: null,
    surfaceMs: null,
    totalMs: null,
  };
}

function emptyDocumentActivation(): WorkbenchDocumentActivationSnapshot {
  return {
    serial: 0,
    phase: "idle",
    documentId: null,
    relativePath: null,
    surface: null,
    cacheOutcome: "unknown",
    diagnostic: null,
    metrics: emptyDocumentActivationMetrics(),
  };
}
