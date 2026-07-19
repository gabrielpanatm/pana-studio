import { createDomPathSelector } from "$lib/preview/selection";
import { hidePreviewHtmlSelectionOverlay } from "$lib/preview/bridge";
import { executePreviewTemplateEditIntent } from "$lib/project/io";
import {
  previewStructuralCommandIdentity,
  requireCurrentPreviewStructuralSession,
  requirePreviewStructuralReceiptIdentity,
  runInPreviewStructuralLane,
} from "$lib/kernel/preview-structural-lane";
import { isZolaTemplatePath } from "$lib/project/files";
import { hydrateSelectionSource } from "$lib/source-graph/location";
import {
  activeTemplateFilesForContext,
  openTeraGateSourceIds,
  teraGateDropStatus,
  type TeraGateDropTarget,
  type TeraGateDropVerdict,
} from "$lib/source-graph/interaction";
import { applySelectionState as applySelectionStateFromController } from "$lib/state/selection-controller";
import { templateEditGateSelectionStatus } from "$lib/tera/template-edit-gate";
import {
  templateGateForPageSection as buildTemplateGateForPageSection,
  templateGateForPreviewClick as buildTemplateGateForPreviewClick,
  templateGateForSelection as buildTemplateGateForSelection,
  templateGateForTeraSource as buildTemplateGateForTeraSource,
  templateGateFromBridgeData,
  templateGateSourceIdForSelection as buildTemplateGateSourceIdForSelection,
  type TemplateGateContext,
} from "$lib/state/template-gate";
import type { PreviewTemplateGate } from "$lib/state/app-helpers";
import type { AppState } from "$lib/state/app.svelte";
import type { EditableStyles, PageSection, PreviewSelectionState, SelectionInfo } from "$lib/types";
import { errorMessage } from "$lib/util";

export type PreviewDropGateTarget = TeraGateDropTarget;
export type PreviewDropGateVerdict = TeraGateDropVerdict;

export function clearPreviewHtmlSelectionMarker(app: AppState) {
  const selectedPreviewDocument = app.selectedPreviewElement?.ownerDocument;
  app.selectedPreviewElement?.classList.remove(app.selectedClass);
  app.selectedPreviewElement = null;
  const previewDocument = app.getPreviewDocument();
  previewDocument
    ?.querySelectorAll(`.${app.selectedClass}`)
    .forEach((node) => node.classList.remove(app.selectedClass));
  hidePreviewHtmlSelectionOverlay(previewDocument ?? selectedPreviewDocument);
}

export function renderPreviewSelectionToBridge(app: AppState, selection: PreviewSelectionState = app.previewSelection) {
  syncPreviewTeraGateState(app);

  if (selection.kind === "html") {
    app.postPreviewMessage({
      type: "render-preview-selection",
      selection: {
        kind: "html",
        selector: selection.selector,
        sourceId: selection.sourceId,
        templateSourceId: selection.templateSourceId,
        sessionId: selection.sessionId,
      },
    });
    return;
  }

  if (selection.kind === "tera") {
    app.postPreviewMessage({
      type: "render-preview-selection",
      selection: {
        kind: "tera",
        selector: selection.selector,
        sourceId: selection.sourceId,
        templateSourceId: selection.templateSourceId,
        origin: selection.origin ?? "local",
        themeName: selection.themeName,
        canSelectHtml: selection.canSelectHtml ?? true,
      },
    });
    return;
  }

  app.postPreviewMessage({
    type: "render-preview-selection",
    selection: { kind: "none" },
  });
}

export function clearPreviewTeraSelection(app: AppState) {
  app.selectedTemplateSourceId = null;
  app.selectedTemplatePreviewSelector = null;
}

export function clearPreviewSelection(
  app: AppState,
  options: { clearTemplateGate?: boolean; clearHtmlMarker?: boolean } = {},
) {
  if (options.clearHtmlMarker) clearPreviewHtmlSelectionMarker(app);
  app.selectedElement = null;
  clearPreviewTeraSelection(app);
  if (options.clearTemplateGate) renderPreviewSelectionToBridge(app, { kind: "none" });
}

export function setPreviewTeraSelection(
  app: AppState,
  gate: PreviewTemplateGate,
  options: { status?: string; showGate?: boolean; clearHtmlMarker?: boolean } = {},
) {
  if (options.clearHtmlMarker ?? true) {
    clearPreviewHtmlSelectionMarker(app);
  } else {
    app.selectedPreviewElement = null;
  }
  app.selectedElement = null;
  app.selectedTemplateSourceId = gate.sourceId;
  app.selectedTemplatePreviewSelector = gate.selector;
  if (options.showGate ?? true) {
    renderPreviewSelectionToBridge(app, {
      kind: "tera",
      selector: gate.selector,
      sourceId: gate.sourceId,
      templateSourceId: gate.sourceId,
      origin: gate.origin,
      themeName: gate.themeName,
      canSelectHtml: gate.canSelectHtml ?? true,
      editable: false,
    });
  }
  if (options.status) {
    app.setGlobalStatus(options.status, "idle");
  }
  app.syncCodeSelectionHighlight(false);
}

export function applySelectionState(
  app: AppState,
  selection: SelectionInfo,
  resolvedStyles?: EditableStyles,
) {
  const resolvedSelection = hydrateSelectionSource(selection, app.sourceGraph);
  applySelectionStateFromController(app.selectionControllerHost(), resolvedSelection, resolvedStyles);
  renderPreviewSelectionToBridge(app, {
    kind: "html",
    selector: resolvedSelection.domPath ?? resolvedSelection.cssSelector ?? null,
    sourceId: resolvedSelection.sourceId,
    templateSourceId: resolvedSelection.templateSourceId,
    sessionId: resolvedSelection.sessionId,
    selection: resolvedSelection,
    editable: app.canEditHtml,
  });
  syncTemplateHtmlEditLock(app, resolvedSelection);
  rememberSelectedElement(app, resolvedSelection);
}

export function rememberSelectedElement(app: AppState, selection = app.selectedElement) {
  if (!selection || selection.tag === "body" || selection.tag === "html") return;
  app.lastMeaningfulSelectedElement = selection;
  if (selection.tag === "img") app.lastSelectedImageElement = selection;
}

export function templateGateContext(app: AppState): TemplateGateContext {
  const activeTemplatePath =
    app.activeScannedPath && isZolaTemplatePath(app.activeScannedPath)
      ? app.activeScannedPath
      : app.activeRenderedTemplatePath ?? app.activeScannedPath;
  // În Workbench, planul kernel este autoritatea pentru lanțul vizibil
  // părinte-direct + activ. Traversarea frontend rămâne doar pentru canvasul
  // compus obișnuit, unde nu există un plan Workbench capturat pe revizie.
  const activeTemplateFiles = app.templateWorkbenchPlan
    ? app.templateWorkbenchPlan.navigator.map((entry) => entry.template.file)
    : activeTemplateFilesForContext(
      app.sourceGraph,
      activeTemplatePath,
      app.selectedElement?.sourceLocation?.file ?? null,
    );
  return {
    sourceGraph: app.sourceGraph,
    activeScannedPath: activeTemplatePath,
    templateHtmlEditSourceId: app.templateHtmlEditSourceId,
    activeTemplateFiles,
    previewDocument: app.getPreviewDocument() ?? null,
  };
}

export function syncPreviewTeraGateState(app: AppState) {
  const context = templateGateContext(app);
  app.postPreviewMessage({
    type: "set-tera-gate-state",
    openGateSourceIds: openTeraGateSourceIds(context.sourceGraph, {
      openedGateSourceId: context.templateHtmlEditSourceId,
      activeScannedPath: context.activeScannedPath,
      activeTemplateFiles: context.activeTemplateFiles,
    }),
  });
}

export function previewDropGateStatus(app: AppState, target: PreviewDropGateTarget): PreviewDropGateVerdict {
  const context = templateGateContext(app);
  return teraGateDropStatus(context.sourceGraph, {
    openedGateSourceId: context.templateHtmlEditSourceId,
    activeScannedPath: context.activeScannedPath,
    activeTemplateFiles: context.activeTemplateFiles,
  }, target);
}

export function templateGateForSelection(app: AppState, selection: SelectionInfo): PreviewTemplateGate | null {
  return buildTemplateGateForSelection(selection, templateGateContext(app));
}

export function templateGateForPageSection(app: AppState, section: PageSection): PreviewTemplateGate | null {
  return buildTemplateGateForPageSection(section, templateGateContext(app));
}

export function templateGateForPreviewClick(app: AppState, element: Element): (PreviewTemplateGate & { element: Element }) | null {
  return buildTemplateGateForPreviewClick(element, templateGateContext(app));
}

export function templateGateForTeraSource(app: AppState, sourceId: string | null | undefined, selector: string | null | undefined): PreviewTemplateGate | null {
  return buildTemplateGateForTeraSource(sourceId, selector, templateGateContext(app));
}

export function templateGateSourceIdForSelection(app: AppState, selection: SelectionInfo) {
  return buildTemplateGateSourceIdForSelection(selection, templateGateContext(app));
}

export function syncTemplateHtmlEditLock(app: AppState, selection: SelectionInfo | null) {
  if (!app.templateHtmlEditSourceId) return;
  const selectionSourceId = selection ? templateGateSourceIdForSelection(app, selection) : null;
  if (selectionSourceId !== app.templateHtmlEditSourceId) {
    app.templateHtmlEditSourceId = null;
    syncPreviewTeraGateState(app);
  }
}

export function selectTemplateGateFromBridge(app: AppState, data: Record<string, unknown>) {
  const gate = templateGateFromBridgeData(data, templateGateContext(app));
  if (!gate) return;
  setPreviewTeraSelection(
    app,
    gate,
    { showGate: false, clearHtmlMarker: false },
  );
}

export async function allowTemplateHtmlEditFromBridge(app: AppState, data: Record<string, unknown>) {
  const sourceId = typeof data.sourceId === "string" ? data.sourceId : null;
  const selector = typeof data.selector === "string" ? data.selector : null;
  await requestTemplateHtmlEditPermission(app, sourceId, selector);
}

function allowTemplateHtmlEdit(app: AppState, sourceId: string | null, selector: string | null) {
  if (!sourceId || !selector) return;
  app.templateHtmlEditSourceId = sourceId;
  clearPreviewTeraSelection(app);
  clearTemplateGateInPreview(app);
  app.postPreviewMessage({ type: "select-by-selector", selector });
  app.setGlobalStatus("Elementul din template este deblocat pentru editare HTML vizuală.", "idle");
}

export async function editSelectedTeraLayer(app: AppState) {
  if (app.previewSelection.kind !== "tera") return;
  await requestTemplateHtmlEditPermission(app, app.previewSelection.sourceId, app.previewSelection.selector);
}

export async function requestTemplateHtmlEditPermission(
  app: AppState,
  sourceId: string | null,
  selector: string | null,
) {
  if (!sourceId || !selector) return;
  try {
    await runInPreviewStructuralLane(app, async (lease) => {
      const receipt = await executePreviewTemplateEditIntent({
        intent: {
          messageType: "preview-template-edit-selected",
          sourceId,
          selector,
        },
        editIntent: {
          targetSourceId: sourceId,
          targetSelector: selector,
        },
      }, previewStructuralCommandIdentity(lease));
      requirePreviewStructuralReceiptIdentity(receipt.intent, lease);
      requireCurrentPreviewStructuralSession(app, lease);
      if (receipt.status !== "granted" || !receipt.grant) {
        const blocking = receipt.diagnostics.find((diagnostic) => diagnostic.blocking);
        throw new Error(blocking?.message ?? receipt.message ?? "Template Edit Gate-ul a blocat deblocarea.");
      }
      allowTemplateHtmlEdit(app, receipt.grant.resolvedTargetId, receipt.grant.selector);
    });
  } catch (error) {
    app.setGlobalStatus(`Eroare deblocare template: ${errorMessage(error)}`, "error");
  }
}

export async function openSelectedTeraSource(app: AppState) {
  const node = app.selectedTemplateSourceNode;
  if (!node) {
    app.setGlobalStatus("Nu există un nod Tera selectat.", "error");
    return;
  }
  const source = node.range
    ? `${node.file}:${node.range.line}:${node.range.column}`
    : node.file;
  await app.openSourceLocation(source);
  await app.setCenterView("code");
}

export function selectLayerSection(app: AppState, section: PageSection) {
  const gate = templateGateForPageSection(app, section);
  if (gate && gate.sourceId !== app.templateHtmlEditSourceId) {
    setPreviewTeraSelection(app, gate, {
      status: templateEditGateSelectionStatus(gate.canSelectHtml, "zone"),
    });
    return;
  }

  app.selectDomNode(section.selector);
}

export function selectTeraLayerSource(app: AppState, section: PageSection, sourceId: string) {
  const gate = templateGateForTeraSource(app, sourceId, section.selector);
  if (!gate) {
    app.selectDomNode(section.selector);
    return;
  }
  setPreviewTeraSelection(app, gate, {
    status: templateEditGateSelectionStatus(gate.canSelectHtml, "node"),
  });
}

export function hoverLayerSection(app: AppState, section: PageSection | null) {
  if (!section) {
    app.postPreviewMessage({ type: "clear-preview-hover" });
    return;
  }

  const gate = templateGateForPageSection(app, section);
  if (gate && gate.sourceId !== app.templateHtmlEditSourceId) {
    app.postPreviewMessage({
      type: "show-preview-hover",
      selector: gate.selector,
      sourceId: gate.sourceId,
      variant: "tera",
      origin: gate.origin,
    });
    return;
  }

  app.postPreviewMessage({
    type: "show-preview-hover",
    selector: section.selector,
    sourceId: section.sourceId ?? null,
    variant: "html",
    origin: null,
  });
}

export function hoverTeraLayerSource(app: AppState, section: PageSection, sourceId: string) {
  const gate = templateGateForTeraSource(app, sourceId, section.selector);
  if (!gate) {
    hoverLayerSection(app, section);
    return;
  }
  app.postPreviewMessage({
    type: "show-preview-hover",
    selector: gate.selector,
    sourceId: gate.sourceId,
    variant: "tera",
    origin: gate.origin,
  });
}

export function hoverPreviewSelection(app: AppState, selection: SelectionInfo | null) {
  if (!selection) {
    app.postPreviewMessage({ type: "clear-preview-hover" });
    return;
  }

  const gate = templateGateForSelection(app, selection);
  if (gate && gate.sourceId !== app.templateHtmlEditSourceId) {
    app.postPreviewMessage({
      type: "show-preview-hover",
      selector: gate.selector,
      sourceId: gate.sourceId,
      variant: "tera",
      origin: gate.origin,
    });
    return;
  }

  app.postPreviewMessage({
    type: "show-preview-hover",
    selector: selection.domPath ?? selection.cssSelector ?? null,
    sourceId: selection.sourceId ?? null,
    variant: "html",
    origin: null,
  });
}

export function clearTemplateGateInPreview(app: AppState) {
  renderPreviewSelectionToBridge(app, { kind: "none" });
}

export function selectPreviewTemplateElement(
  app: AppState,
  element: Element,
  gate: PreviewTemplateGate,
) {
  setPreviewTeraSelection(app, {
    ...gate,
    selector: createDomPathSelector(element),
  }, {
    status: templateEditGateSelectionStatus(gate.canSelectHtml, "element"),
  });
}
