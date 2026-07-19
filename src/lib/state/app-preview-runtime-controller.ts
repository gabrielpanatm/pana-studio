import { serializeOverrides } from "$lib/css/serializer";
import { htmlVoidTags } from "$lib/html/mutations";
import { applyStagedOverrideStylesToDocument, ensurePreviewInspectorStyles, updatePreviewHtmlSelectionOverlay } from "$lib/preview/bridge";
import { collectDomTree, resolveHtmlSourceSelectionContext } from "$lib/preview/selection";
import { isMessageFromExactPreviewFrame } from "$lib/preview/frame-origin";
import {
  isZolaTemplatePath,
  previewUrlForScannedFile as buildPreviewUrlForScannedFile,
} from "$lib/project/files";
import {
  readSourceGraph,
  type PreviewPhaseReceipt,
} from "$lib/project/io";
import {
  projectLatestProjectWorkspacePreview,
  scheduleProjectWorkspaceDerivedPreviewProjection,
} from "$lib/kernel/project-workspace-preview-coordinator";
import { flushFileBufferDraftSync } from "$lib/session/file-buffer-draft-sync";
import {
  handlePreviewProjectionIntent,
  isPreviewProjectionIntentMessage,
} from "$lib/state/preview-projection-controller";
import { confirmMountedCanvasProjection } from "$lib/state/preview-controller";
import { templateEditGateSelectionStatus } from "$lib/tera/template-edit-gate";
import { normalizedProjectPath, templateOriginKind } from "$lib/state/app-helpers";
import {
  resolveSourceEditLocationForSourceId as resolveSourceEditLocationFromGraph,
  resolveSourceEditTargetForSourceId as resolveSourceEditTargetFromGraph,
} from "$lib/source-graph/location";
import type { AppState } from "$lib/state/app.svelte";
import type { PageSection, PreviewSelectionState, ProjectFile, SelectionInfo } from "$lib/types";
import { errorMessage } from "$lib/util";

const TEMPLATE_GATE_ACTIONS_ID = "pana-studio-template-gate-actions";

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

function markPreviewMessageRevision(app: AppState, data: Record<string, unknown>) {
  const revision = previewMessageRevision(data);
  if (revision === null) return false;
  if (revision < app.latestPreviewMessageRevision) return true;
  app.latestPreviewMessageRevision = revision;
  return false;
}

function activeTemplateContextPath(app: AppState) {
  if (app.activeScannedPath && isZolaTemplatePath(app.activeScannedPath)) {
    return normalizedProjectPath(app.activeScannedPath);
  }
  return normalizedProjectPath(app.activeRenderedTemplatePath ?? app.activeScannedPath);
}

function sourceFileForSection(app: AppState, section: PageSection) {
  const sourceNode = section.sourceId
    ? app.sourceGraph?.nodes.find((node) => node.id === section.sourceId)
    : null;
  if (sourceNode?.file) return normalizedProjectPath(sourceNode.file);
  const templateNode = section.templateSourceId
    ? app.sourceGraph?.nodes.find((node) => node.id === section.templateSourceId)
    : null;
  return normalizedProjectPath(templateNode?.file ?? section.sourceLocation?.file ?? null);
}

function autoSelectionElementForSections(
  app: AppState,
  previewDocument: Document,
  sections: PageSection[],
) {
  const activeTemplatePath = activeTemplateContextPath(app);
  const section =
    (activeTemplatePath
      ? sections.find((candidate) => sourceFileForSection(app, candidate) === activeTemplatePath)
      : null) ??
    sections.find((candidate) => candidate.sourceId || candidate.templateSourceId) ??
    null;
  return section?.selector ? previewDocument.querySelector(section.selector) : null;
}

export async function refreshSourceGraph(
  app: AppState,
  options: { strict?: boolean } = {},
) {
  const serial = ++app.sourceGraphLoadSerial;
  if (!app.scannedProject?.isZola) {
    app.sourceGraph = null;
    app.pageSections = app.hydratePageSections(app.pageSections);
    return true;
  }
  const projectRoot = app.sessionProjectRoot.trim();
  const runtimeSessionId = app.kernelProjectSessionId.trim();
  const projectSessionEpoch = app.projectSessionEpoch;
  const projectionMatches = () => (
    serial === app.sourceGraphLoadSerial
    && app.sessionProjectRoot === projectRoot
    && app.kernelProjectSessionId === runtimeSessionId
    && app.projectSessionEpoch === projectSessionEpoch
  );
  if (!projectRoot || !runtimeSessionId) {
    if (options.strict) {
      throw new Error("Source Graph nu poate fi citit fără identitatea runtime a ProjectSession.");
    }
    return false;
  }
  try {
    const graph = await readSourceGraph(
      {
        expectedProjectRoot: projectRoot,
        expectedSessionId: runtimeSessionId,
      },
    );
    if (!projectionMatches()) {
      if (options.strict) {
        throw new Error("Source Graph refresh a fost înlocuit de altă generație ProjectSession.");
      }
      return false;
    }
    app.sourceGraph = graph;
    app.pageSections = app.hydratePageSections(app.pageSections);
    if (app.selectedElement) {
      app.applySelectionState(app.selectedElement, undefined, false);
    }
    app.syncPreviewTeraGateState();
    return true;
  } catch (error) {
    if (projectionMatches()) app.sourceGraph = null;
    if (options.strict) throw error;
    return false;
  }
}

export function derivePreviewSelectionState(app: AppState): PreviewSelectionState {
  if (app.selectedTemplateSourceId) {
    const node = app.selectedTemplateSourceNode;
    return {
      kind: "tera",
      selector: app.selectedTemplatePreviewSelector,
      sourceId: app.selectedTemplateSourceId,
      templateSourceId: node?.kind === "html" ? null : app.selectedTemplateSourceId,
      origin: node ? templateOriginKind(node, app.activeScannedPath) : null,
      themeName: node?.themeName ?? null,
      canSelectHtml: true,
      editable: false,
    };
  }

  if (app.selectedElement) {
    return {
      kind: "html",
      selector: app.selectedElement.domPath ?? app.selectedElement.cssSelector ?? null,
      sourceId: app.selectedElement.sourceId,
      templateSourceId: app.selectedElement.templateSourceId,
      sessionId: app.selectedElement.sessionId,
      selection: app.selectedElement,
      editable: app.canEditHtml,
    };
  }

  return { kind: "none" };
}

export function previewUrlForScannedFile(app: AppState, file: ProjectFile) {
  const url = buildPreviewUrlForScannedFile(file, {
    previewBaseUrl: app.scannedProject?.previewBaseUrl,
  });
  const revision = app.pendingCanvasProjection?.identity.previewRevision;
  if (url === "about:blank" || !revision) return url;
  const stagedUrl = new URL(url);
  stagedUrl.searchParams.set("__pana_preview_revision", revision);
  return stagedUrl.toString();
}

export function resolveSourceEditTargetForSourceId(app: AppState, sourceId: string | null | undefined) {
  return resolveSourceEditTargetFromGraph(app.sourceGraph, sourceId);
}

export function resolveSourceEditLocationForSourceId(app: AppState, sourceId: string | null | undefined) {
  return resolveSourceEditLocationFromGraph(app.sourceGraph, sourceId);
}

export function syncHtmlCodeToPreview(app: AppState, sourceText: string, cursorPosition: number) {
  app.cancelPreviewSync();
  const context = resolveHtmlSourceSelectionContext({
    sourceText,
    cursorPosition,
    selectedElement: app.selectedElement,
    htmlVoidTags,
  });
  app.setPageSections(context.pageSections);
  app.reconcileSelectionWithSourceDocument(context.parsedDocument, context.activeSelector);
  app.pendingSelectionSelector = context.pendingSelector;

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
          `Preview-ul documentului nu a putut proiecta draftul ProjectWorkspace: ${errorMessage(error)}`,
          "error",
        );
      }
    })();
  }, 220);
}

export function applyStagedOverrideStylesToPreview(app: AppState, css: string) {
  const previewDocument = app.getPreviewDocument();
  if (!previewDocument) {
    app.postPreviewMessage({ type: "set-live-overrides-css", css });
    return;
  }
  applyStagedOverrideStylesToDocument(previewDocument, css);
  updatePreviewHtmlSelectionOverlay(app.selectedPreviewElement);
}

export function attachPreviewInspector(app: AppState) {
  app.previewRuntime.reset();
  // Skip when showing a status/placeholder document (not a real page).
  if (app.previewDocumentMarkup !== null) return;

  const previewDocument = app.getPreviewDocument();
  const overrideCss = serializeOverrides(app.overrideRules, app.variableOverrides);

  if (previewDocument?.body) {
    ensurePreviewInspectorStyles(previewDocument);
    previewDocument.defaultView?.addEventListener("scroll", () => {
      updatePreviewHtmlSelectionOverlay(app.selectedPreviewElement);
    }, true);
    previewDocument.defaultView?.addEventListener("resize", () => {
      updatePreviewHtmlSelectionOverlay(app.selectedPreviewElement);
    });
    app.applyStagedOverrideStylesToPreview(overrideCss);
    app.restoreLiveCssLayersToPreview();
    const sections = collectDomTree(previewDocument);
    app.setPageSections(sections);
    app.syncPreviewTeraGateState();

    previewDocument.addEventListener("click", (event) => {
      const target = event.target;
      if (!(target instanceof previewDocument.defaultView!.Element)) return;
      if (target.closest(`#${TEMPLATE_GATE_ACTIONS_ID}`)) return;
      event.preventDefault();
      event.stopPropagation();
      const templateGate = app.templateGateForPreviewClick(target);
      if (templateGate && templateGate.sourceId !== app.templateHtmlEditSourceId) {
        app.selectPreviewTemplateElement(templateGate.element, templateGate);
        return;
      }
      app.selectPreviewElement(target, { revealCode: false });
    }, true);

    if (app.previewSelection.kind === "tera" && app.previewSelection.selector) {
      app.renderPreviewSelectionToBridge();
      return;
    }

    const preservedSelection = app.selectedElement?.tag === "body" || app.selectedElement?.tag === "html"
      ? app.lastMeaningfulSelectedElement ?? app.lastSelectedImageElement
      : app.selectedElement;
    const preservedSelectionSelector = preservedSelection?.domPath ?? preservedSelection?.cssSelector ?? null;
    const preservedSessionElement = preservedSelection?.sessionId
      ? previewDocument.querySelector(`[data-pana-session-id="${preservedSelection.sessionId}"]`)
      : null;
    const initialElement =
      (app.pendingSelectionSelector ? previewDocument.querySelector(app.pendingSelectionSelector) : null) ??
      preservedSessionElement ??
      (preservedSelectionSelector ? previewDocument.querySelector(preservedSelectionSelector) : null) ??
      autoSelectionElementForSections(app, previewDocument, sections);
    app.pendingSelectionSelector = null;
    if (!initialElement) {
      app.renderPreviewSelectionToBridge({ kind: "none" });
      return;
    }
    const initialTemplateGate = app.templateGateForPreviewClick(initialElement);
    if (initialTemplateGate && initialTemplateGate.sourceId !== app.templateHtmlEditSourceId) {
      app.selectPreviewTemplateElement(initialTemplateGate.element, initialTemplateGate);
    } else {
      app.selectPreviewElement(initialElement);
    }
    return;
  }

  app.applyStagedOverrideStylesToPreview(overrideCss);
  app.restoreLiveCssLayersToPreview();
  app.syncPreviewTeraGateState();
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
  if (app.pendingSelectionSelector) {
    app.postPreviewMessage({
      type: "select-by-selector",
      selector: app.pendingSelectionSelector,
    });
  } else {
    app.renderPreviewSelectionToBridge({ kind: "none" });
  }
  app.pendingSelectionSelector = null;

  // Cross-origin iframe: fetch the rendered HTML and build full DOM tree.
  app.fetchDomTreeFromPreview();
}

export function handlePreviewMessage(app: AppState, event: MessageEvent) {
  const data = event.data;
  if (!data || data.source !== "pana-studio-preview") return;
  if (!isMessageFromExactPreviewFrame(app.previewFrame, event)) return;
  if (!app.previewRuntime.acceptIncomingMessage()) return;
  const ack = app.previewRuntime.handleAck(data);
  if (ack) {
    if (ack.revision > app.latestPreviewMessageRevision) {
      app.latestPreviewMessageRevision = ack.revision;
    }
    return;
  }
  if (data.type === "ready") {
    void confirmMountedCanvasProjection(
      app.previewControllerHost(),
      data.canvasIdentity && typeof data.canvasIdentity === "object"
        ? data.canvasIdentity
        : null,
      Array.isArray(data.canvasPhaseReceipts)
        ? data.canvasPhaseReceipts as PreviewPhaseReceipt[]
        : [],
    ).catch((error) => {
      app.setGlobalStatus(
        `Confirmarea Canvas a eșuat: ${error instanceof Error ? error.message : String(error)}`,
        "error",
      );
    });
    app.restoreLiveCssLayersToPreview();
    app.syncPreviewTeraGateState();
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
  if (data.type === "preview-hover") {
    app.hoverPreviewSelection((data.selection ?? null) as SelectionInfo | null);
    return;
  }
  if (data.type === "preview-hover-clear") {
    app.hoverPreviewSelection(null);
    return;
  }
  if (data.type === "preview-context-menu") {
    app.openPreviewContextMenu(data);
    return;
  }
  if (data.type === "preview-pointerdown") {
    app.closeContextMenu();
    return;
  }
  if (data.type !== "selection") return;
  if (markPreviewMessageRevision(app, data)) return;
  const selection = data.selection as SelectionInfo;
  if (app.pendingRestoredSelectionTag) {
    if (selection.tag !== app.pendingRestoredSelectionTag) return;
    app.pendingRestoredSelectionTag = null;
    if (app.pendingRestoredSelectionTimer !== null) {
      window.clearTimeout(app.pendingRestoredSelectionTimer);
      app.pendingRestoredSelectionTimer = null;
    }
  }
  const templateGate = app.templateGateForSelection(selection);
  if (templateGate && templateGate.sourceId !== app.templateHtmlEditSourceId) {
    app.setPreviewTeraSelection(templateGate, {
      clearHtmlMarker: false,
      status: templateEditGateSelectionStatus(templateGate.canSelectHtml, "element"),
    });
    return;
  }

  const previewDocument = app.getPreviewDocument();
  app.selectedPreviewElement =
    previewDocument?.querySelector(selection.domPath) ??
    previewDocument?.querySelector(selection.cssSelector) ??
    null;
  app.applySelectionState(selection);
}
