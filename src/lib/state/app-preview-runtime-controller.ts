import { serializeOverrides } from "$lib/css/serializer";
import type { EditableStyles } from "$lib/css/contracts";
import {
  applyApplicationAppearanceToPreviewDocument,
  applyStagedOverrideStylesToDocument,
  ensurePreviewInspectorStyles,
} from "$lib/preview/bridge";
import { collectDomTree } from "$lib/preview/selection";
import { isMessageFromExactPreviewFrame } from "$lib/preview/frame-origin";
import { previewUrlForScannedFile as buildPreviewUrlForScannedFile } from "$lib/project/files";
import {
  readSourceGraph,
} from "$lib/source-graph/io";
import type {
  CanvasProjectionIdentity,
  PreviewPhaseReceipt,
} from "$lib/contracts/canvas-projection";
import {
  projectLatestProjectWorkspacePreview,
  scheduleProjectWorkspaceDerivedPreviewProjection,
  type ProjectWorkspacePreviewHost,
} from "$lib/kernel/project-workspace-preview-coordinator";
import { flushFileBufferDraftSync } from "$lib/session/file-buffer-draft-sync";
import {
  handlePreviewProjectionIntent,
  isPreviewProjectionIntentMessage,
  type PreviewProjectionControllerHost,
} from "$lib/state/preview-projection-controller";
import {
  handleCanvasAgentMessage,
  retryCanvasInteractionBinding,
  type CanvasInteractionControllerHost,
} from "$lib/state/canvas-interaction-controller";
import {
  confirmMountedCanvasProjection,
  settleGuardedPreviewNavigation,
  type PreviewControllerHost,
} from "$lib/state/preview-controller";
import { contrastingTextColor } from "$lib/state/app-helpers";
import type { ApplicationPreferencesState } from "$lib/application/preferences.svelte";
import {
  resolveSourceEditLocationForSourceId as resolveSourceEditLocationFromGraph,
  resolveSourceEditTargetForSourceId as resolveSourceEditTargetFromGraph,
} from "$lib/source-graph/location";
import type {
  CanvasElementObservation,
  CoordinatedElementSelection,
  PageSection,
} from "$lib/canvas/contracts";
import type {
  ProjectFile,
  ProjectScan,
} from "$lib/project/lifecycle-contract";
import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";
import type { SourceGraph } from "$lib/source-graph/graph-contract";
import type { CanvasProjectionPlan } from "$lib/contracts/canvas-projection";
import type { PreviewRuntime } from "$lib/editor-runtime/preview-runtime";
import type { WorkspaceDerivedProjectionStatus } from "$lib/session/workspace-mutation-coordinator";
import { errorMessage } from "$lib/util";
import { t } from "$lib/i18n/runtime.svelte";

export type AppPreviewRuntimeControllerHost = PreviewProjectionControllerHost
  & ProjectWorkspacePreviewHost
  & {
    canvasInteraction: CanvasInteractionControllerHost;
    coordinatedElementSelection: CoordinatedElementSelection | null;
    currentSourceRelativePath: string;
    isActiveRenderedPreviewPage: boolean;
    latestPreviewMessageRevision: number;
    overrideRules: Record<string, EditableStyles>;
    pageSections: PageSection[];
    pendingCanvasProjection: CanvasProjectionPlan | null;
    previewCommands: () => PreviewControllerHost;
    previewDocumentMarkup: string | null;
    previewRuntime: PreviewRuntime;
    previewSyncTimer: number | null;
    projectSessionEpoch: number;
    projectWorkspaceSnapshot: ProjectWorkspaceSnapshot | null;
    scannedProject: ProjectScan | null;
    sourceGraph: SourceGraph | null;
    sourceGraphLoadSerial: number;
    sourceGraphProjectionStatus: WorkspaceDerivedProjectionStatus;
    sourceGraphWorkspaceRevision: number | null;
    preferences: Pick<ApplicationPreferencesState, "accent">;
    variableOverrides: Record<string, string>;
    applySelectionState: (selection: CanvasElementObservation) => void;
    applyStagedOverrideStylesToPreview: (css: string) => void;
    cancelPreviewSync: () => void;
    fetchDomTreeFromPreview: () => void;
    getPreviewDocument: () => Document | undefined;
    hydratePageSections: (sections: PageSection[]) => PageSection[];
    restoreLiveCssLayersToPreview: () => void;
    setPageSections: (sections: PageSection[]) => void;
  };

/**
 * Messages which complete an application-owned Preview transaction must keep
 * flowing while user intents are locked. Treating these ACKs like clicks or
 * shortcuts makes every guarded Project Transition time out by construction.
 */
export function isPreviewControlPlaneMessage(data: unknown) {
  if (!data || typeof data !== "object") return false;
  const message = data as Record<string, unknown>;
  return message.source === "pana-studio-preview"
    && (message.type === "ready" || message.type === "preview-operation-complete");
}

function previewMessageRevision(data: Record<string, unknown>) {
  return typeof data.previewRevision === "number" && data.previewRevision > 0
    ? data.previewRevision
    : null;
}

function markPreviewMessageRevision(
  app: AppPreviewRuntimeControllerHost,
  data: Record<string, unknown>,
) {
  const revision = previewMessageRevision(data);
  if (revision === null) return false;
  if (revision < app.latestPreviewMessageRevision) return true;
  app.latestPreviewMessageRevision = revision;
  return false;
}

export async function refreshSourceGraph(
  app: AppPreviewRuntimeControllerHost,
  options: { strict?: boolean } = {},
) {
  const serial = ++app.sourceGraphLoadSerial;
  if (!app.scannedProject) {
    app.sourceGraph = null;
    app.sourceGraphProjectionStatus = "deferred";
    app.sourceGraphWorkspaceRevision = null;
    app.pageSections = app.hydratePageSections(app.pageSections);
    return true;
  }
  const projectRoot = app.sessionProjectRoot.trim();
  const runtimeSessionId = app.kernelProjectSessionId.trim();
  const projectSessionEpoch = app.projectSessionEpoch;
  const expectedWorkspaceRevision = app.projectWorkspaceSnapshot?.revision ?? null;
  const projectionMatches = () => (
    serial === app.sourceGraphLoadSerial
    && app.sessionProjectRoot === projectRoot
    && app.kernelProjectSessionId === runtimeSessionId
    && app.projectSessionEpoch === projectSessionEpoch
  );
  if (!projectRoot || !runtimeSessionId) {
    if (options.strict) {
      throw new Error(t("source-graph-runtime-identity-missing"));
    }
    return false;
  }
  try {
    const receipt = await readSourceGraph(
      {
        expectedProjectRoot: projectRoot,
        expectedSessionId: runtimeSessionId,
      },
    );
    if (!projectionMatches()) {
      if (options.strict) {
        throw new Error(t("source-graph-refresh-superseded"));
      }
      return false;
    }
    if (
      receipt.projectRoot !== projectRoot
      || receipt.runtimeSessionId !== runtimeSessionId
      || !Number.isSafeInteger(receipt.workspaceRevision)
      || receipt.workspaceRevision < 0
    ) {
      throw new Error(t("source-graph-workspace-identity-invalid"));
    }
    const currentRevision = app.projectWorkspaceSnapshot?.revision ?? null;
    if (
      expectedWorkspaceRevision !== null
      && receipt.workspaceRevision !== expectedWorkspaceRevision
    ) {
      if (currentRevision !== null && currentRevision > expectedWorkspaceRevision) {
        return false;
      }
      throw new Error(
        t("source-graph-revision-mismatch", {
          actual: receipt.workspaceRevision,
          expected: expectedWorkspaceRevision,
        }),
      );
    }
    if (currentRevision !== null && receipt.workspaceRevision !== currentRevision) return false;
    app.sourceGraph = receipt.graph;
    app.sourceGraphProjectionStatus = "current";
    app.sourceGraphWorkspaceRevision = receipt.workspaceRevision;
    app.pageSections = app.hydratePageSections(app.pageSections);
    if (app.coordinatedElementSelection) {
      app.applySelectionState(app.coordinatedElementSelection.observation);
    }
    return true;
  } catch (error) {
    if (projectionMatches()) app.sourceGraphProjectionStatus = "degraded";
    if (options.strict) throw error;
    return false;
  }
}

export function previewUrlForScannedFile(
  app: AppPreviewRuntimeControllerHost,
  file: ProjectFile,
) {
  const url = buildPreviewUrlForScannedFile(file, {
    previewBaseUrl: app.scannedProject?.previewBaseUrl,
  });
  const revision = app.pendingCanvasProjection?.identity.previewRevision;
  if (url === "about:blank" || !revision) return url;
  const stagedUrl = new URL(url);
  stagedUrl.searchParams.set("__pana_preview_revision", revision);
  return stagedUrl.toString();
}

export function resolveSourceEditTargetForSourceId(
  app: AppPreviewRuntimeControllerHost,
  sourceId: string | null | undefined,
) {
  return resolveSourceEditTargetFromGraph(app.sourceGraph, sourceId);
}

export function resolveSourceEditLocationForSourceId(
  app: AppPreviewRuntimeControllerHost,
  sourceId: string | null | undefined,
) {
  return resolveSourceEditLocationFromGraph(app.sourceGraph, sourceId);
}

export function syncHtmlCodeToPreview(
  app: AppPreviewRuntimeControllerHost,
  sourceText: string,
  _cursorPosition: number,
) {
  app.cancelPreviewSync();
  const parsedDocument = new DOMParser().parseFromString(sourceText, "text/html");
  app.setPageSections(collectDomTree(parsedDocument));

  const projectRoot = app.sessionProjectRoot;
  const runtimeSessionId = app.kernelProjectSessionId;
  const projectSessionEpoch = app.projectSessionEpoch;
  const sourcePath = app.currentSourceRelativePath;

  app.previewSyncTimer = window.setTimeout(() => {
    app.previewSyncTimer = null;
    void (async () => {
      try {
        await flushFileBufferDraftSync();
        if (
          !app.isActiveRenderedPreviewPage
          || app.sessionProjectRoot !== projectRoot
          || app.kernelProjectSessionId !== runtimeSessionId
          || app.projectSessionEpoch !== projectSessionEpoch
          || app.currentSourceRelativePath !== sourcePath
        ) return;
        await projectLatestProjectWorkspacePreview(app, {
          reason: "workspace-mutation",
          requestedPaths: sourcePath ? [sourcePath] : [],
        });
      } catch (error) {
        if (
          app.sessionProjectRoot !== projectRoot
          || app.kernelProjectSessionId !== runtimeSessionId
          || app.projectSessionEpoch !== projectSessionEpoch
        ) return;
        app.setGlobalStatus(
          t("preview-runtime-draft-projection-failed", {
            message: errorMessage(error),
          }),
          "error",
        );
      }
    })();
  }, 220);
}

export function applyStagedOverrideStylesToPreview(
  app: AppPreviewRuntimeControllerHost,
  css: string,
) {
  const previewDocument = app.getPreviewDocument();
  if (!previewDocument) {
    app.canvasInteraction.commands.postPreviewMessage({ type: "set-live-overrides-css", css });
    return;
  }
  applyStagedOverrideStylesToDocument(previewDocument, css);
}

function syncApplicationAppearanceToPreview(app: AppPreviewRuntimeControllerHost) {
  const textOnAccent = contrastingTextColor(app.preferences.accent);
  const previewDocument = app.getPreviewDocument();
  if (previewDocument) {
    applyApplicationAppearanceToPreviewDocument(previewDocument, app.preferences.accent, textOnAccent);
  }
  app.canvasInteraction.commands.postPreviewMessage({
    type: "set-application-appearance",
    accent: app.preferences.accent,
    textOnAccent,
  });
}

export function attachPreviewInspector(app: AppPreviewRuntimeControllerHost) {
  app.previewRuntime.reset();
  // Skip when showing a status/placeholder document (not a real page).
  if (app.previewDocumentMarkup !== null) return;

  const previewDocument = app.getPreviewDocument();
  const overrideCss = serializeOverrides(app.overrideRules, app.variableOverrides);
  syncApplicationAppearanceToPreview(app);

  if (previewDocument?.body) {
    ensurePreviewInspectorStyles(previewDocument);
    app.applyStagedOverrideStylesToPreview(overrideCss);
    app.restoreLiveCssLayersToPreview();
    const sections = collectDomTree(previewDocument);
    app.setPageSections(sections);
    return;
  }

  app.applyStagedOverrideStylesToPreview(overrideCss);
  app.restoreLiveCssLayersToPreview();
  // ACK-ul de structură este urmărit: dacă iframe-ul este înlocuit sau bridge-ul
  // lipsește, eroarea rămâne în controlerul care poate decide recovery-ul și nu
  // produce un toast concurent cu proiecția canonică.
  const projectRoot = app.sessionProjectRoot;
  const runtimeSessionId = app.kernelProjectSessionId;
  const projectSessionEpoch = app.projectSessionEpoch;
  void app.previewRuntime.sendAndWait({ type: "sync-structure" })
    .then(() => {
      if (
        app.sessionProjectRoot !== projectRoot
        || app.kernelProjectSessionId !== runtimeSessionId
        || app.projectSessionEpoch !== projectSessionEpoch
        || app.pendingCanvasProjection
      ) return;
      // O revizie ProjectWorkspace poate fi amânată cât iframe-ul este
      // nemontat. Primul ACK al bridge-ului reproiectează automat ultima stare.
      scheduleProjectWorkspaceDerivedPreviewProjection(app, "session-refresh");
    })
    .catch(() => undefined);

  // Cross-origin iframe: fetch the rendered HTML and build full DOM tree.
  app.fetchDomTreeFromPreview();
}

export function handlePreviewMessage(
  app: AppPreviewRuntimeControllerHost,
  event: MessageEvent,
) {
  const data = event.data;
  if (data?.source === "pana-studio-canvas-agent") {
    const exactFrame = isMessageFromExactPreviewFrame(app.canvasInteraction.session.previewFrame, event);
    if (!exactFrame) return;
    if (!app.previewRuntime.acceptIncomingMessage()) return;
    handleCanvasAgentMessage(app.canvasInteraction, event);
    return;
  }
  if (!data || data.source !== "pana-studio-preview") return;
  const exactFrame = isMessageFromExactPreviewFrame(app.canvasInteraction.session.previewFrame, event);
  if (!exactFrame) return;
  if (!app.previewRuntime.acceptIncomingMessage()) return;
  const ack = app.previewRuntime.handleAck(data);
  if (ack) {
    if (ack.revision > app.latestPreviewMessageRevision) {
      app.latestPreviewMessageRevision = ack.revision;
    }
    return;
  }
  if (data.type === "ready") {
    const readyIdentity = data.canvasIdentity && typeof data.canvasIdentity === "object"
      ? data.canvasIdentity as CanvasProjectionIdentity
      : null;
    const readyReceipts = Array.isArray(data.canvasPhaseReceipts)
      ? data.canvasPhaseReceipts as PreviewPhaseReceipt[]
      : [];
    if (
      readyReceipts.length === 3
      && readyReceipts[2]?.phase === "styledReady"
    ) {
      settleGuardedPreviewNavigation(app.previewCommands(), readyIdentity);
    }
    syncApplicationAppearanceToPreview(app);
    void confirmMountedCanvasProjection(
      app.previewCommands(),
      readyIdentity,
      readyReceipts,
    ).catch((error) => {
      app.setGlobalStatus(
        t("preview-runtime-canvas-confirmation-failed", {
          message: error instanceof Error ? error.message : String(error),
        }),
        "error",
      );
    });
    app.restoreLiveCssLayersToPreview();
    void retryCanvasInteractionBinding(app.canvasInteraction);
    return;
  }
  if (data.type === "structure") {
    if (markPreviewMessageRevision(app, data)) return;
    const previewDocument = app.getPreviewDocument();
    if (previewDocument?.body) {
      app.setPageSections(collectDomTree(previewDocument));
      return;
    }
    if (Array.isArray(data.sections)) {
      app.setPageSections(data.sections as PageSection[]);
      return;
    }
    app.fetchDomTreeFromPreview();
    return;
  }
  if (isPreviewProjectionIntentMessage(data.type)) {
    void handlePreviewProjectionIntent(app, data);
    return;
  }
}
