import {
  createPreviewRuntime,
  type CanvasPatchPerformanceSnapshot,
  type PreviewRuntime,
} from "$lib/editor-runtime/preview-runtime";
import {
  cancelPreviewSync,
  cancelCanvasProjectionConfirmation,
  fetchDomTreeFromPreview,
  getPreviewDocument,
  hasMountedCanvasProjectionSurface,
  invalidatePreviewDomTreeProjection,
  invalidatePreviewRefreshLease,
  mountCanvasProjectionSurface,
  postPreviewMessage,
  prepareCanvasProjectionNavigation,
  previewReloadUrl,
  reconcileTemplateWorkbenchPreviewDocument,
  refreshRenderedPreviewDocument,
  reloadPreview,
  sendPreviewOperation,
  unmountCanvasProjectionSurface,
  type CanvasProjectionConfirmation,
  type PreviewControllerHost,
  type PreviewRefreshLease,
} from "$lib/state/preview-controller";
import { previewUrlForScannedFile as buildPreviewUrlForScannedFile } from "$lib/project/files";
import { buildInteractivePreviewUrl } from "$lib/preview/interactive";
import {
  recordPreviewRuntimeEvent,
} from "$lib/preview/io";
import type {
  CanvasProjectionIdentity,
  CanvasProjectionPlan,
  PreviewRuntimeEventKind,
  PreviewStylesheetPromotionMetrics,
} from "$lib/contracts/canvas-projection";
import type { CssAuthoringState } from "$lib/css/authoring-state.svelte";
import type { EditorSelectionSessionController } from "$lib/state/editor-selection-session.svelte";
import type { PageSectionsState } from "$lib/preview/page-sections.svelte";
import type { PreviewSurfaceState } from "$lib/preview/surface-state.svelte";
import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { ControlledPreviewWorkspaceState } from "$lib/preview/controlled-state.svelte";
import type { MotionPreviewMode, MotionWorkspaceState } from "$lib/motion/workspace.svelte";
import type {
  ProjectWorkspacePreviewProjectionOptions,
  ProjectWorkspacePreviewProjectionOutcome,
} from "$lib/kernel/project-workspace-preview-coordinator";
import { projectWorkspacePreviewStatusKey } from "$lib/kernel/project-workspace-preview-coordinator";
import type { PreviewRefreshReason } from "$lib/preview/controlled";
import type {
  GlobalStatusKind,
  GlobalStatusPublishOptions,
} from "$lib/status/global-status";
import type { CanvasPatch } from "$lib/preview/contracts";
import type {
  ProjectFile,
  ProjectScan,
} from "$lib/project/lifecycle-contract";
import type { VersionPreviewReceipt } from "$lib/versioning/contracts";
import { errorMessage } from "$lib/util";
import { t } from "$lib/i18n/runtime.svelte";
import { sameCanvasProjectionIdentity } from "$lib/contracts/canvas-identity";

export type PreviewWorkspaceContext = Readonly<{
  activePage: ProjectFile | null;
  isActivePage: boolean;
  templateWorkbenchActive: boolean;
  project: ProjectScan | null;
  activeScannedPath: string | null;
  activeVersionPreview: VersionPreviewReceipt | null;
}>;

export type PreviewWorkspaceDependencies = {
  session: ProjectSessionState;
  surface: PreviewSurfaceState;
  css: CssAuthoringState;
  sections: PageSectionsState;
  selection: Pick<EditorSelectionSessionController, "refreshNavigationSnapshot" | "reset">;
  controlled: () => ControlledPreviewWorkspaceState;
  motion: MotionWorkspaceState;
  context: () => PreviewWorkspaceContext;
  setStatus: (
    text: string,
    kind: GlobalStatusKind,
    options?: GlobalStatusPublishOptions,
  ) => void;
  clearStatus: (id: string) => void;
  reportCanvasDegraded: (
    projectRoot: string,
    runtimeSessionId: string,
    diagnostic: string,
  ) => Promise<void>;
  projectLatest: (
    options: ProjectWorkspacePreviewProjectionOptions,
  ) => Promise<ProjectWorkspacePreviewProjectionOutcome>;
  loadProjectFile: (file: ProjectFile, options?: {
    strict?: boolean;
    skipDraftFlush?: boolean;
    deferPreviewRefresh?: boolean;
    activateTemplateWorkbench?: boolean;
    preferredTemplatePagePath?: string | null;
    preferredTemplateRoute?: string | null;
    syncWorkbench?: boolean;
  }) => Promise<unknown>;
  invalidateSourceGraph: () => void;
};

const EMPTY_CANVAS_PERFORMANCE: CanvasPatchPerformanceSnapshot = {
  sampleCount: 0,
  receiptToCommitP50Ms: null,
  receiptToCommitP95Ms: null,
  receiptToCommitMaxMs: null,
  bridgeCommitP95Ms: null,
  budgetMs: 50,
  budgetMet: null,
};

/** Owns the embedded Preview document, Canvas identity and bridge lifecycle. */
export class PreviewWorkspaceState {
  src = $state("about:blank");
  workspaceRevision = $state<string | null>(null);
  pendingProjection = $state<CanvasProjectionPlan | null>(null);
  activeIdentity = $state<CanvasProjectionIdentity | null>(null);
  activeUrl = $state("about:blank");
  navigationGuardActive = $state(false);
  navigationRecoveryUrl = $state<string | null>(null);
  documentMarkup = $state<string | null>(null);
  latestMessageRevision = $state(0);
  gridOverlayEnabled = $state(false);
  interactiveEnabled = $state(false);
  interactiveDomNodes = $state<import("$lib/preview/interactive").InteractivePreviewDomNode[]>([]);
  canvasPatchPerformance = $state<CanvasPatchPerformanceSnapshot>({ ...EMPTY_CANVAS_PERFORMANCE });

  refreshSerial = 0;
  domTreeSerial = 0;
  reloadSerial = 0;
  confirmation: CanvasProjectionConfirmation | null = null;
  deferredProjection: CanvasProjectionPlan | null = null;
  syncTimer: number | null = null;
  domTreeFetchTimer: number | null = null;
  structuralWriteBoundaryActive = false;
  structuralWriteBoundaryResumesMonitoring = false;
  readonly runtime: PreviewRuntime;
  private readonly resumeDiagnostics: string[] = [];

  private readonly dependencies: PreviewWorkspaceDependencies;

  constructor(dependencies: PreviewWorkspaceDependencies) {
    this.dependencies = dependencies;
    this.runtime = createPreviewRuntime({
      postPreviewMessage: (payload) => this.postMessage(payload),
      setGlobalStatus: (text, kind) => dependencies.setStatus(text, kind),
    });
  }

  urlForFile(file: ProjectFile) {
    const url = buildPreviewUrlForScannedFile(file, {
      previewBaseUrl: this.dependencies.session.project?.previewBaseUrl,
    });
    const revision = this.pendingProjection?.identity.previewRevision;
    if (url === "about:blank" || !revision) return url;
    const stagedUrl = new URL(url);
    stagedUrl.searchParams.set("__pana_preview_revision", revision);
    return stagedUrl.toString();
  }

  get interactiveUrl() {
    if (!this.activeIdentity) return "";
    const sourceUrl = this.pendingProjection ? this.activeUrl : this.src;
    return buildInteractivePreviewUrl(sourceUrl, this.activeIdentity);
  }

  async applyCanvasPatch(patch: CanvasPatch) {
    const identity = this.activeIdentity;
    const startedAt = performance.now();
    try {
      const receipt = await this.runtime.applyCanvasPatch(patch);
      this.canvasPatchPerformance = this.runtime.canvasPatchPerformance();
      if (
        identity
        && identity.projectRoot === patch.projectRoot
        && identity.runtimeSessionId === patch.runtimeSessionId
        && identity.workspaceRevision === patch.baseWorkspaceRevision
      ) {
        void this.recordRuntimeEvent(
          "canvas_patch_applied",
          identity,
          Math.max(0, performance.now() - startedAt),
          patch.patchId,
        );
      }
      return receipt;
    } catch (error) {
      if (
        identity
        && identity.projectRoot === patch.projectRoot
        && identity.runtimeSessionId === patch.runtimeSessionId
        && identity.workspaceRevision === patch.baseWorkspaceRevision
      ) {
        void this.recordRuntimeEvent(
          "canvas_patch_refused",
          identity,
          Math.max(0, performance.now() - startedAt),
          errorMessage(error),
        );
      }
      throw error;
    }
  }

  async rollbackCanvasPatch(patch: CanvasPatch) {
    const identity = this.pendingProjection?.identity ?? null;
    const startedAt = performance.now();
    const receipt = await this.runtime.rollbackCanvasPatch(patch);
    if (identity?.workspaceRevision === patch.workspaceRevision) {
      void this.recordRuntimeEvent(
        "canvas_patch_rolled_back",
        identity,
        Math.max(0, performance.now() - startedAt),
        t("workbench-canvas-patch-withdrawn", { patch: patch.patchId }),
      );
    }
    return receipt;
  }

  acceptInteractiveDomSnapshot(
    nodes: import("$lib/preview/interactive").InteractivePreviewDomNode[],
  ) {
    if (!this.interactiveEnabled || !this.activeIdentity) return;
    this.interactiveDomNodes = nodes.slice(0, 5_000);
  }

  async recordInteractiveRealmEvent(
    kind: PreviewRuntimeEventKind,
    previewRevision: string,
    durationMs: number,
    diagnostic: string | null = null,
  ) {
    const identity = this.activeIdentity;
    if (
      !identity
      || identity.previewRevision !== previewRevision
      || !Number.isFinite(durationMs)
      || durationMs < 0
    ) return;
    await this.recordRuntimeEvent(kind, identity, durationMs, diagnostic);
  }

  commands(): PreviewControllerHost {
    const owner = this;
    const { session, surface, css, sections, selection } = this.dependencies;
    return {
      session: {
        get sessionProjectRoot() { return session.root; },
        get kernelProjectSessionId() { return session.runtimeSessionId; },
        get projectSessionEpoch() { return session.epoch; },
        get previewRefreshSerial() { return owner.refreshSerial; },
        set previewRefreshSerial(serial) { owner.refreshSerial = serial; },
        get previewDomTreeSerial() { return owner.domTreeSerial; },
        set previewDomTreeSerial(serial) { owner.domTreeSerial = serial; },
      },
      surface: {
        get frame() { return surface.frame; },
        set frame(frame) { surface.frame = frame; },
        get canvasElement() { return surface.canvasElement; },
        set canvasElement(element) { surface.canvasElement = element; },
        get generation() { return surface.generation; },
        set generation(generation) { surface.generation = generation; },
      },
      navigation: {
        get src() { return owner.src; },
        set src(src) { owner.src = src; },
        get reloadSerial() { return owner.reloadSerial; },
        set reloadSerial(serial) { owner.reloadSerial = serial; },
        get activeUrl() { return owner.activeUrl; },
        set activeUrl(url) { owner.activeUrl = url; },
        get guardActive() { return owner.navigationGuardActive; },
        set guardActive(active) { owner.navigationGuardActive = active; },
        get recoveryUrl() { return owner.navigationRecoveryUrl; },
        set recoveryUrl(url) { owner.navigationRecoveryUrl = url; },
      },
      projection: {
        get workspaceRevision() { return owner.workspaceRevision; },
        set workspaceRevision(revision) { owner.workspaceRevision = revision; },
        get pending() { return owner.pendingProjection; },
        set pending(plan) { owner.setPendingProjection(plan); },
        get activeIdentity() { return owner.activeIdentity; },
        set activeIdentity(identity) { owner.activeIdentity = identity; },
        get confirmation() { return owner.confirmation; },
        set confirmation(confirmation) { owner.setCanvasConfirmation(confirmation); },
      },
      timers: {
        get previewSync() { return owner.syncTimer; },
        set previewSync(timer) { owner.syncTimer = timer; },
        get domTreeFetch() { return owner.domTreeFetchTimer; },
        set domTreeFetch(timer) { owner.domTreeFetchTimer = timer; },
      },
      document: {
        get markup() { return owner.documentMarkup; },
        set markup(markup) { owner.documentMarkup = markup; },
        get activePage() { return owner.dependencies.context().activePage; },
        get isActivePage() { return owner.dependencies.context().isActivePage; },
        get projectStatus() { return session.status; },
        set projectStatus(status) { session.status = status; },
      },
      context: {
        get lifecycle() { return session.lifecycle; },
        get templateWorkbenchActive() {
          return owner.dependencies.context().templateWorkbenchActive;
        },
      },
      styles: {
        get overrideRules() { return css.overrideRules; },
        get variableOverrides() { return css.variableOverrides; },
      },
      sections: {
        get items() { return sections.sections; },
        set: (nextSections) => sections.set(nextSections),
      },
      selection,
      runtime: this.runtime,
      commands: {
        urlForFile: (file) => this.urlForFile(file),
        recordRuntimeEvent: (kind, identity, durationMs, diagnostic, stylesheetMetrics) => (
          this.recordRuntimeEvent(kind, identity, durationMs, diagnostic, stylesheetMetrics)
        ),
      },
    };
  }

  reloadUrl(url: string) {
    return previewReloadUrl(this.commands(), url);
  }

  cancelSync() {
    cancelPreviewSync(this.commands());
  }

  fetchDomTree() {
    fetchDomTreeFromPreview(this.commands());
  }

  getDocument() {
    return getPreviewDocument(this.commands());
  }

  postMessage(payload: Record<string, unknown>) {
    postPreviewMessage(this.commands(), payload);
  }

  setGridOverlay(enabled: boolean) {
    this.gridOverlayEnabled = enabled;
    this.postMessage({ type: "set-canvas-grid-overlay", enabled });
  }

  send(payload: Record<string, unknown> & { type: string }) {
    return sendPreviewOperation(this.commands(), payload);
  }

  refreshDocument(lease?: PreviewRefreshLease) {
    return refreshRenderedPreviewDocument(this.commands(), lease);
  }

  prepareNavigation(plan: CanvasProjectionPlan) {
    return prepareCanvasProjectionNavigation(this.commands(), plan);
  }

  hasMountedSurface() {
    return hasMountedCanvasProjectionSurface(this.commands());
  }

  canReuseCanonicalWorkbenchSurface(
    identity: CanvasProjectionIdentity,
    previewUrl: string,
  ) {
    const surface = this.dependencies.surface;
    return Boolean(
      sameCanvasProjectionIdentity(this.activeIdentity, identity)
      && sameCanonicalPreviewRoute(this.src, previewUrl)
      && sameCanonicalPreviewRoute(this.activeUrl, previewUrl)
      && this.documentMarkup === null
      && this.hasMountedSurface()
      && surface.frame === surface.canvasElement
      && surface.loadedGeneration === surface.generation
      && !surface.resumeRequired
      && !surface.resumeScheduled
      && !surface.resumePromise
      && !this.pendingProjection
      && !this.confirmation
      && !this.navigationGuardActive
    );
  }

  mountSurface(frame: HTMLIFrameElement) {
    return mountCanvasProjectionSurface(this.commands(), frame);
  }

  mountAndTrackSurface(frame: HTMLIFrameElement) {
    const replaced = Boolean(
      this.dependencies.surface.canvasElement
      && this.dependencies.surface.canvasElement !== frame,
    );
    const generation = this.mountSurface(frame);
    if (replaced) this.deferSurfaceProjection();
    return generation;
  }

  unmountSurface(frame: HTMLIFrameElement) {
    return unmountCanvasProjectionSurface(this.commands(), frame);
  }

  unmountAndTrackSurface(frame: HTMLIFrameElement) {
    if (!this.unmountSurface(frame)) return false;
    this.deferSurfaceProjection();
    return true;
  }

  deferSurfaceProjection() {
    if (!this.dependencies.context().project) return;
    if (this.pendingProjection?.phase === "prepared") {
      this.deferredProjection = this.pendingProjection;
    }
    this.dependencies.surface.resumeRequired = true;
    this.recordResumeDiagnostic("resume_required");
    this.scheduleSurfaceProjectionResume();
  }

  markSurfaceCurrent() {
    this.dependencies.surface.resumeRequired = false;
    this.deferredProjection = null;
    this.recordResumeDiagnostic("surface_current");
  }

  setPendingProjection(plan: CanvasProjectionPlan | null) {
    const previous = this.pendingProjection;
    if (
      !plan
      && previous?.phase === "prepared"
      && (
        this.dependencies.surface.resumeRequired
        || !this.hasMountedSurface()
      )
    ) this.deferredProjection = previous;
    this.pendingProjection = plan;
    this.recordResumeDiagnostic(plan ? "pending_set" : "pending_cleared");
    if (!plan) this.scheduleSurfaceProjectionResume();
  }

  setCanvasConfirmation(confirmation: CanvasProjectionConfirmation | null) {
    this.confirmation = confirmation;
    this.recordResumeDiagnostic(confirmation ? "confirmation_set" : "confirmation_cleared");
    if (!confirmation) this.scheduleSurfaceProjectionResume();
  }

  onSurfaceLoaded(frame: HTMLIFrameElement) {
    const surface = this.dependencies.surface;
    if (surface.canvasElement !== frame) return;
    surface.loadedGeneration = surface.generation;
    this.recordResumeDiagnostic("surface_loaded");
    this.scheduleSurfaceProjectionResume();
  }

  resumeDiagnosticSnapshot() {
    return [...this.resumeDiagnostics];
  }

  private recordResumeDiagnostic(event: string) {
    if (!import.meta.env?.DEV) return;
    const surface = this.dependencies.surface;
    this.resumeDiagnostics.push([
      Math.round(performance.now()),
      event,
      `generation=${surface.generation}`,
      `loaded=${surface.loadedGeneration}`,
      `required=${surface.resumeRequired}`,
      `pending=${Boolean(this.pendingProjection)}`,
      `confirmation=${Boolean(this.confirmation)}`,
      `inFlight=${Boolean(surface.resumePromise)}`,
    ].join(";"));
    if (this.resumeDiagnostics.length > 64) this.resumeDiagnostics.shift();
  }

  private scheduleSurfaceProjectionResume() {
    const surface = this.dependencies.surface;
    if (surface.resumeScheduled) return;
    surface.resumeScheduled = true;
    queueMicrotask(() => {
      surface.resumeScheduled = false;
      this.resumeSurfaceProjection();
    });
  }

  private resumeSurfaceProjection() {
    const surface = this.dependencies.surface;
    const frame = surface.canvasElement;
    const context = this.dependencies.context();
    if (
      !frame
      || surface.frame !== frame
      || surface.loadedGeneration !== surface.generation
      || !surface.resumeRequired
      || this.confirmation
      || this.pendingProjection
      || !this.canProjectWorkspacePreview()
      || context.activeVersionPreview
      || surface.resumePromise
    ) {
      this.recordResumeDiagnostic("resume_blocked");
      return;
    }

    const surfaceGeneration = surface.generation;
    const projectRoot = this.dependencies.session.root;
    const runtimeSessionId = this.dependencies.session.runtimeSessionId;
    const workspaceRevision = this.dependencies.session.workspace?.revision ?? 0;
    const statusKey = projectWorkspacePreviewStatusKey(
      projectRoot,
      runtimeSessionId,
      workspaceRevision,
    );
    this.recordResumeDiagnostic("resume_started");
    const resume = (async () => {
      const deferredPlan = this.currentDeferredProjection(projectRoot, runtimeSessionId);
      const outcome = deferredPlan
        ? await this.resumePreparedProjection(deferredPlan)
        : await this.dependencies.projectLatest({ reason: "session-refresh" });
      this.recordResumeDiagnostic(`resume_outcome_${outcome.status}`);
      if (
        surface.generation !== surfaceGeneration
        || surface.canvasElement !== frame
        || this.dependencies.session.root !== projectRoot
        || this.dependencies.session.runtimeSessionId !== runtimeSessionId
      ) return;
      if (outcome.status !== "published" && outcome.status !== "already_current") return;
      const activeIdentity = this.activeIdentity;
      if (
        !activeIdentity
        || activeIdentity.projectRoot !== projectRoot
        || activeIdentity.runtimeSessionId !== runtimeSessionId
        || activeIdentity.workspaceRevision !== this.dependencies.session.workspace?.revision
      ) throw new Error(t("workbench-preview-generation-unconfirmed"));
      await this.dependencies.selection.refreshNavigationSnapshot(
        activeIdentity,
        this.activeUrl || this.src,
        { strict: true },
      );
      this.markSurfaceCurrent();
      this.dependencies.clearStatus(statusKey);
      const current = this.dependencies.context();
      const activeFile = current.project?.files.find(
        (file) => file.relativePath === current.activeScannedPath,
      );
      if (activeFile?.role === "template" && !current.templateWorkbenchActive) {
        await this.dependencies.loadProjectFile(activeFile, {
          strict: true,
          skipDraftFlush: true,
          activateTemplateWorkbench: true,
          syncWorkbench: false,
        });
      }
    })()
      .catch(async (error) => {
        this.recordResumeDiagnostic("resume_failed");
        if (
          surface.generation === surfaceGeneration
          && surface.canvasElement === frame
          && this.dependencies.session.root === projectRoot
          && this.dependencies.session.runtimeSessionId === runtimeSessionId
        ) {
          this.dependencies.setStatus(
            t("workbench-preview-resume-failed", { message: errorMessage(error) }),
            "error",
            {
              source: "preview",
              code: "preview.resume.failed",
              dedupeKey: statusKey,
              resolutionKey: statusKey,
              lifecycle: "until_resolved",
            },
          );
          await this.dependencies.reportCanvasDegraded(
            projectRoot,
            runtimeSessionId,
            errorMessage(error),
          ).catch(() => undefined);
        }
      })
      .finally(() => {
        this.recordResumeDiagnostic("resume_settled");
        if (surface.resumePromise === resume) {
          surface.resumePromise = null;
          const currentSurface = surface.canvasElement;
          if (
            currentSurface
            && surface.resumeRequired
            && surface.generation !== surfaceGeneration
          ) this.scheduleSurfaceProjectionResume();
        }
      });
    surface.resumePromise = resume;
  }

  private currentDeferredProjection(
    projectRoot: string,
    runtimeSessionId: string,
  ) {
    const plan = this.deferredProjection;
    if (
      plan?.phase !== "prepared"
      || plan.identity.projectRoot !== projectRoot
      || plan.identity.runtimeSessionId !== runtimeSessionId
      || plan.identity.workspaceRevision !== this.dependencies.session.workspace?.revision
    ) return null;
    return plan;
  }

  private async resumePreparedProjection(plan: CanvasProjectionPlan) {
    const context = this.dependencies.context();
    let refreshed = false;
    if (context.templateWorkbenchActive) {
      refreshed = await this.reconcileWorkbenchDocument(this.src, plan);
    } else {
      this.workspaceRevision = plan.identity.previewRevision;
      this.setPendingProjection(plan);
      refreshed = await this.requestWorkspaceProjectionRefresh("session-refresh");
    }
    if (!refreshed || !sameCanvasProjectionIdentity(this.activeIdentity, plan.identity)) {
      throw new Error(t("workbench-preview-generation-unconfirmed"));
    }
    return {
      status: "published" as const,
      workspaceRevision: plan.identity.workspaceRevision,
    };
  }

  async reconcileWorkbenchDocument(previewUrl: string, plan: CanvasProjectionPlan) {
    const reconciled = await reconcileTemplateWorkbenchPreviewDocument(
      this.commands(),
      previewUrl,
      plan,
    );
    if (reconciled) {
      this.markSurfaceCurrent();
      this.dependencies.clearStatus(projectWorkspacePreviewStatusKey(
        plan.identity.projectRoot,
        plan.identity.runtimeSessionId,
        plan.identity.workspaceRevision,
      ));
    }
    return reconciled;
  }

  reload(lease?: PreviewRefreshLease) {
    return reloadPreview(this.commands(), lease);
  }

  async requestRefresh(reason: PreviewRefreshReason = "manual") {
    const refreshed = await this.dependencies.controlled().requestRefresh(reason);
    const project = this.dependencies.context().project;
    if (refreshed && project?.previewWarning) {
      this.dependencies.session.project = { ...project, previewWarning: null };
      this.dependencies.clearStatus("project.preview.warning");
    }
    return refreshed;
  }

  async requestWorkspaceProjectionRefresh(reason: PreviewRefreshReason) {
    const refreshed = await this.dependencies.controlled().requestRefresh(
      reason,
      { publishFailure: false },
    );
    if (refreshed) {
      const project = this.dependencies.context().project;
      if (project?.previewWarning) {
        this.dependencies.session.project = { ...project, previewWarning: null };
        this.dependencies.clearStatus("project.preview.warning");
      }
      return true;
    }
    throw new Error(
      this.dependencies.session.status || t("workbench-preview-generation-unconfirmed"),
    );
  }

  canProjectWorkspacePreview() {
    const project = this.dependencies.context().project;
    return Boolean(
      this.hasMountedSurface()
      && project?.previewBaseUrl
      && this.src
      && this.src !== "about:blank"
      && this.documentMarkup === null,
    );
  }

  markLive(message?: string) {
    this.dependencies.controlled().markLive(message);
  }

  markSavedToDisk(message?: string) {
    this.dependencies.controlled().markSaved(message);
  }

  resetControlled() {
    cancelCanvasProjectionConfirmation(this.commands());
    this.dependencies.surface.resumeRequired = false;
    this.dependencies.surface.resumeScheduled = false;
    this.dependencies.surface.resumePromise = null;
    this.dependencies.selection.reset();
    this.dependencies.motion.previewMode = "design";
    this.dependencies.motion.previewStatus = null;
    invalidatePreviewRefreshLease(this.commands().session);
    invalidatePreviewDomTreeProjection(this.commands());
    this.dependencies.invalidateSourceGraph();
    this.dependencies.controlled().reset();
    this.reset();
  }

  setInteractiveEnabled(enabled: boolean) {
    this.interactiveEnabled = Boolean(
      enabled
      && this.activeIdentity
      && this.src
      && this.src !== "about:blank",
    );
    if (!this.interactiveEnabled) this.interactiveDomNodes = [];
  }

  setExecutionMode(mode: MotionPreviewMode) {
    this.dependencies.motion.previewMode = mode;
    if (mode !== "motion") this.dependencies.motion.previewStatus = null;
    this.setInteractiveEnabled(mode !== "design");
    if (mode !== "design" && !this.interactiveEnabled) {
      this.dependencies.motion.previewMode = "design";
      this.dependencies.motion.previewStatus = null;
    }
  }

  async recordRuntimeEvent(
    kind: PreviewRuntimeEventKind,
    identity: CanvasProjectionIdentity,
    durationMs: number,
    diagnostic: string | null,
    stylesheetMetrics: PreviewStylesheetPromotionMetrics | null = null,
  ) {
    if (!Number.isFinite(durationMs) || durationMs < 0) return;
    try {
      const receipt = await recordPreviewRuntimeEvent({
        schemaVersion: 1,
        identity,
        kind,
        durationMs: Math.min(600_000, Math.round(durationMs)),
        diagnostic,
        stylesheetMetrics,
      });
      if (
        !receipt.accepted
        || receipt.identity.projectRoot !== identity.projectRoot
        || receipt.identity.runtimeSessionId !== identity.runtimeSessionId
        || receipt.identity.workspaceRevision !== identity.workspaceRevision
        || receipt.identity.transactionId !== identity.transactionId
        || receipt.identity.previewRevision !== identity.previewRevision
        || receipt.kind !== kind
      ) throw new Error(t("workbench-canvas-event-mismatch"));
    } catch (error) {
      if (this.activeIdentity?.transactionId !== identity.transactionId) return;
      this.dependencies.setStatus(
        t("workbench-canvas-observability-failed", { message: errorMessage(error) }),
        "error",
      );
    }
  }

  reset() {
    this.runtime.reset();
    this.canvasPatchPerformance = this.runtime.canvasPatchPerformance();
    this.src = "about:blank";
    this.workspaceRevision = null;
    this.pendingProjection = null;
    this.deferredProjection = null;
    this.activeIdentity = null;
    this.activeUrl = "about:blank";
    this.navigationGuardActive = false;
    this.navigationRecoveryUrl = null;
    this.documentMarkup = null;
    this.latestMessageRevision = 0;
    this.gridOverlayEnabled = false;
    this.interactiveEnabled = false;
    this.interactiveDomNodes = [];
    this.confirmation = null;
    this.refreshSerial += 1;
    this.domTreeSerial += 1;
  }
}

function sameCanonicalPreviewRoute(currentUrl: string, expectedUrl: string) {
  try {
    const current = new URL(currentUrl);
    const expected = new URL(expectedUrl);
    return current.origin === expected.origin && current.pathname === expected.pathname;
  } catch {
    return false;
  }
}
