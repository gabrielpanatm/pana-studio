import type { MovePosition } from "$lib/html/mutations";
import {
  blockedAction,
  cancelledAction,
  committedAction,
  editorActionSucceeded,
  failedAction,
  type EditorActionOutcome,
} from "$lib/editor-runtime/action-outcome";
import {
  previewStructuralBlockingDiagnostic,
  projectCommittedPreviewStructuralMutation,
  requireCommittedPreviewStructuralPatch,
  type PreviewStructuralCanonicalProjectionHost,
  type PreviewStructuralExecutionReceipt,
} from "$lib/kernel/preview-projection-control";
import {
  previewStructuralCommandIdentity,
  runInPreviewStructuralLane,
  type PreviewStructuralSessionLease,
} from "$lib/kernel/preview-structural-lane";
import { scannedCacheKey } from "$lib/project/files";
import { validateLayerStructureDrop, type LayerMoveRequest } from "$lib/project/layers-drag";
import { executePreviewLayerDropIntent } from "$lib/project/io";
import { collectDomTree, createDomPathSelector } from "$lib/preview/selection";
import { formatSourceEditLocation } from "$lib/source-graph/location";
import type { HtmlPendingArea, PageSection, ProjectSourceEditLocation, SaveState, SelectionInfo, SourceEditLocation } from "$lib/types";
import { errorMessage } from "$lib/util";

export type LayersDragControllerHost = PreviewStructuralCanonicalProjectionHost & {
  structureStatus: string;
  isActivePreviewHtmlSource: boolean;
  htmlSourceMutationBlockedReason: string;
  htmlPending: Record<HtmlPendingArea, boolean>;
  selectedElement: SelectionInfo | null;
  textEditOriginalKey: string | null;
  pageSections: PageSection[];
  activeScannedPath: string | null;
  source: string;
  sourceCache: Record<string, string>;
  setPageSections?: (sections: PageSection[]) => void;
  pendingSelectionSelector: string | null;
  getPreviewDocument: () => Document | undefined;
  applyTextContentToHtml: () => Promise<EditorActionOutcome>;
  setHtmlPending: (area: HtmlPendingArea, pending: boolean) => void;
  setGlobalStatus: (text: string, kind: SaveState) => void;
  previewDropGateStatus?: (target: {
    targetSourceId?: string | null;
    targetTemplateSourceId?: string | null;
  }) => { allowed: boolean; message?: string };
};

function positionLabel(position: MovePosition) {
  if (position === "before") return "înainte";
  if (position === "after") return "după";
  return "în interior";
}

function queryElement(document: Document, selector: string) {
  try {
    return document.querySelector(selector);
  } catch {
    return null;
  }
}

function projectedSourceSelector(sourceId: string | null | undefined) {
  const normalized = sourceId?.trim() ?? "";
  if (!/^sg_[0-9a-f]{16}$/.test(normalized)) {
    throw new Error(
      "Move Engine-ul a comis mutarea fără identitatea Source Graph post-commit necesară proiecției.",
    );
  }
  return `[data-pana-source-id="${normalized}"]`;
}

function blockedMoveReceiptOutcome(
  receipt: PreviewStructuralExecutionReceipt,
): EditorActionOutcome | null {
  if (receipt.status === "committed") return null;
  const reason = previewStructuralBlockingDiagnostic(receipt)?.message
    ?? receipt.message
    ?? "Move Engine-ul a blocat mutarea.";
  return blockedAction(reason);
}

function textEditKey(selection: SelectionInfo | null) {
  if (!selection) return null;
  const sourceLocation = selection.sourceLocation ? formatSourceEditLocation(selection.sourceLocation) : "";
  return `${selection.sourceId ?? selection.sessionId ?? sourceLocation}::${selection.domPath}`;
}

function sourceIdForElement(element: Element | null | undefined) {
  return element?.getAttribute("data-pana-source-id") ?? null;
}

function projectMoveLocation(location: SourceEditLocation | null | undefined): ProjectSourceEditLocation | null {
  if (!location) return null;
  return {
    file: location.file,
    line: location.line,
    column: location.column ?? 1,
  };
}

function sessionIdForElement(element: Element | null | undefined) {
  return element?.getAttribute("data-pana-session-id") ?? null;
}

function syncLayerSectionsFromPreview(host: LayersDragControllerHost, previewDocument: Document | undefined) {
  if (!previewDocument?.body) return host.pageSections;
  const sections = collectDomTree(previewDocument);
  host.setPageSections?.(sections);
  return host.setPageSections ? host.pageSections : sections;
}

function findSectionByLiveIdentity(
  sections: PageSection[],
  selector: string,
  previous: PageSection | null,
  options: { preferPreviousIdentity?: boolean } = {},
) {
  const selectorMatch = sections.find((node) => node.selector === selector) ?? null;
  const sessionMatch = previous?.sessionId
    ? sections.find((node) => node.sessionId === previous.sessionId && node.tag === previous.tag) ?? null
    : null;
  const sourceMatch = previous?.sourceId
    ? sections.find((node) => node.sourceId === previous.sourceId && node.tag === previous.tag) ?? null
    : null;
  const match = options.preferPreviousIdentity
    ? sessionMatch ?? sourceMatch ?? selectorMatch
    : selectorMatch ?? sessionMatch ?? sourceMatch;
  if (!match || !previous) return match;
  return {
    ...match,
    sourceId: match.sourceId ?? previous.sourceId,
    templateSourceId: match.templateSourceId ?? previous.templateSourceId,
    sessionId: match.sessionId ?? previous.sessionId,
    sourceLocation: match.sourceLocation ?? previous.sourceLocation,
  };
}

function sectionWithRequestAnchors(
  node: PageSection,
  request: LayerMoveRequest,
  side: "source" | "target",
): PageSection {
  const sessionId = side === "source" ? request.sourceSessionId : request.targetSessionId;
  const sourceId = side === "source" ? request.sourceSourceId : request.targetSourceId;
  const templateSourceId = side === "source" ? request.sourceTemplateSourceId : request.targetTemplateSourceId;
  return {
    ...node,
    sessionId: node.sessionId ?? sessionId ?? null,
    sourceId: node.sourceId ?? sourceId ?? null,
    templateSourceId: node.templateSourceId ?? templateSourceId ?? null,
  };
}

async function moveTemplateLayer(
  host: LayersDragControllerHost,
  request: LayerMoveRequest,
  sourceNode: PageSection,
  targetNode: PageSection,
  previewDocument: Document | undefined,
  lease: PreviewStructuralSessionLease,
): Promise<EditorActionOutcome> {
  if (request.targetKind === "empty-tera-slot") {
    host.structureStatus = "Move Engine-ul canonic nu mută încă HTML direct într-un slot Tera gol.";
    host.setGlobalStatus(host.structureStatus, "error");
    return blockedAction(host.structureStatus);
  }

  const label = positionLabel(request.position);
  const sourceElementBeforeMove = previewDocument ? queryElement(previewDocument, request.sourceSelector) : null;
  const targetElementBeforeMove = previewDocument ? queryElement(previewDocument, request.targetSelector) : null;
  if (previewDocument && (!sourceElementBeforeMove || !targetElementBeforeMove)) {
    host.structureStatus = "Arborele Straturi era nesincronizat cu preview-ul. Încearcă din nou după reîmprospătarea listei.";
    host.setGlobalStatus(host.structureStatus, "error");
    if (previewDocument.body) host.setPageSections?.(collectDomTree(previewDocument));
    return blockedAction(host.structureStatus);
  }
  if (
    sourceElementBeforeMove &&
    targetElementBeforeMove &&
    (targetElementBeforeMove === sourceElementBeforeMove || sourceElementBeforeMove.contains(targetElementBeforeMove))
  ) {
    host.structureStatus = "Destinația mutării nu mai este validă după ultima restaurare History.";
    host.setGlobalStatus(host.structureStatus, "error");
    if (previewDocument?.body) host.setPageSections?.(collectDomTree(previewDocument));
    return blockedAction(host.structureStatus);
  }
  const sourceSessionId = sessionIdForElement(sourceElementBeforeMove) ?? sourceNode.sessionId;
  const sourceId =
    sourceIdForElement(sourceElementBeforeMove) ??
    request.sourceSourceId ??
    sourceNode.sourceId;
  const sourceSelector = sourceElementBeforeMove ? createDomPathSelector(sourceElementBeforeMove) : sourceNode.selector;
  const targetSessionId = sessionIdForElement(targetElementBeforeMove) ?? targetNode.sessionId;
  const targetSourceId =
    sourceIdForElement(targetElementBeforeMove) ??
    request.targetSourceId ??
    targetNode.sourceId;
  const targetSelector = targetElementBeforeMove ? createDomPathSelector(targetElementBeforeMove) : targetNode.selector;
  const moveIntent = {
    sourceSourceId: sourceId ?? null,
    targetSourceId: targetSourceId ?? null,
    sourceLocation: projectMoveLocation(sourceNode.sourceLocation),
    targetLocation: projectMoveLocation(targetNode.sourceLocation),
    sourceTag: sourceNode.tag,
    targetTag: targetNode.tag,
    sourceSelector,
    targetSelector,
    position: request.position,
  };
  const receipt = await executePreviewLayerDropIntent({
    intent: {
      messageType: "preview-layer-drop",
      sourceSelector,
      targetSelector,
      sourceId: sourceId ?? null,
      targetSourceId: targetSourceId ?? null,
      sourceTemplateSourceId: request.sourceTemplateSourceId ?? sourceNode.templateSourceId ?? null,
      targetTemplateSourceId: request.targetTemplateSourceId ?? targetNode.templateSourceId ?? null,
      sourceSessionId: sourceSessionId ?? null,
      targetSessionId: targetSessionId ?? null,
      sourceTag: sourceNode.tag,
      targetTag: targetNode.tag,
      targetKind: request.targetKind ?? "html",
      position: request.position,
    },
    moveIntent,
  }, previewStructuralCommandIdentity(lease));

  const blocked = blockedMoveReceiptOutcome(receipt);
  if (blocked) {
    host.structureStatus = blocked.reason ?? "Move Engine-ul a blocat mutarea.";
    host.setGlobalStatus(host.structureStatus, "error");
    return blocked;
  }

  const patch = requireCommittedPreviewStructuralPatch(
    receipt,
    "Move Engine-ul a blocat mutarea.",
  );
  await projectCommittedPreviewStructuralMutation(host, lease, receipt, patch, async () => {
    const postCommitSelector = projectedSourceSelector(receipt.projectedSourceId);
    host.sourceCache = {
      ...host.sourceCache,
      [scannedCacheKey({ relativePath: patch.file })]: patch.contents,
    };
    if (host.activeScannedPath === patch.file) {
      host.source = patch.contents;
    }
    host.pendingSelectionSelector = postCommitSelector;
    host.structureStatus = `Element mutat ${label} și salvat prin kernel.`;
  });
  return committedAction();
}

export async function moveLayerElement(
  host: LayersDragControllerHost,
  request: LayerMoveRequest,
): Promise<EditorActionOutcome> {
  if (host.htmlPending.text) {
    const currentTextEditKey = textEditKey(host.selectedElement);
    if (host.textEditOriginalKey && host.textEditOriginalKey !== currentTextEditKey) {
      host.structureStatus = "Mutarea a fost oprită: există o editare de text neaplicată pe alt element.";
      host.setGlobalStatus("Aplică sau anulează editarea textului înainte să muți alt element.", "error");
      return blockedAction(host.structureStatus);
    }
    const textResult = await host.applyTextContentToHtml();
    if (!editorActionSucceeded(textResult) || host.htmlPending.text) {
      const reason = textResult.reason
        ?? (host.htmlPending.text
          ? "Mutarea a fost oprită: editarea de text a rămas pending după încercarea de aplicare."
          : `Mutarea a fost oprită de rezultatul editării textului (${textResult.status}).`);
      host.structureStatus = reason;
      host.setGlobalStatus(reason, "error");
      if (editorActionSucceeded(textResult)) return blockedAction(reason);
      return textResult;
    }
  }

  try {
    const result = await runInPreviewStructuralLane(host, (lease) =>
      moveLayerElementInLane(host, request, lease));
    return result ?? cancelledAction("Mutarea a fost anulată odată cu sesiunea structurală.");
  } catch (error) {
    const reason = errorMessage(error);
    host.structureStatus = `Nu am putut muta elementul: ${reason}`;
    host.setGlobalStatus(`Eroare mutare: ${reason}`, "error");
    return failedAction(reason);
  }
}

async function moveLayerElementInLane(
  host: LayersDragControllerHost,
  request: LayerMoveRequest,
  lease: PreviewStructuralSessionLease,
): Promise<EditorActionOutcome> {

  const previewDocument = host.getPreviewDocument();
  const requestHasLiveAnchors = Boolean(
    request.sourceSessionId ||
    request.sourceSourceId ||
    request.sourceTemplateSourceId ||
    request.targetSessionId ||
    request.targetSourceId ||
    request.targetTemplateSourceId,
  );
  const previousSourceNode = host.pageSections.find((node) => node.selector === request.sourceSelector) ?? null;
  const previousTargetNode = host.pageSections.find((node) => node.selector === request.targetSelector) ?? null;
  const layerSections = syncLayerSectionsFromPreview(host, previewDocument);
  const rawSourceNode = findSectionByLiveIdentity(
    layerSections,
    request.sourceSelector,
    previousSourceNode,
    { preferPreviousIdentity: !requestHasLiveAnchors },
  );
  const rawTargetNode = findSectionByLiveIdentity(
    layerSections,
    request.targetSelector,
    previousTargetNode,
    { preferPreviousIdentity: !requestHasLiveAnchors },
  );
  if (!rawSourceNode || !rawTargetNode) {
    host.structureStatus = "Elementul sursă sau destinația nu mai există în arborele live.";
    host.setGlobalStatus(host.structureStatus, "error");
    return blockedAction(host.structureStatus);
  }
  const sourceNode = sectionWithRequestAnchors(rawSourceNode, request, "source");
  const targetNode = sectionWithRequestAnchors(rawTargetNode, request, "target");
  const liveRequest = {
    ...request,
    sourceSelector: sourceNode.selector,
    targetSelector: targetNode.selector,
  };
  const gateValidation = host.previewDropGateStatus?.({
    targetSourceId: targetNode.sourceId,
    targetTemplateSourceId: targetNode.templateSourceId,
  });
  if (gateValidation && !gateValidation.allowed) {
    host.structureStatus = gateValidation.message ?? "Drop blocat de gate-ul Tera.";
    host.setGlobalStatus(host.structureStatus, "error");
    return blockedAction(host.structureStatus);
  }
  const validation = validateLayerStructureDrop(sourceNode, targetNode, request.position);
  if (!validation.allowed) {
    host.structureStatus = validation.reason ?? "Drop invalid.";
    host.setGlobalStatus(validation.reason ?? "Drop invalid.", "error");
    return blockedAction(host.structureStatus);
  }

  const hasProjectAnchor = Boolean(
    (sourceNode.sourceId || request.sourceSourceId || sourceNode.sourceLocation) &&
    (targetNode.sourceId || request.targetSourceId || targetNode.sourceLocation),
  );
  if (hasProjectAnchor) {
    return await moveTemplateLayer(host, liveRequest, sourceNode, targetNode, previewDocument, lease);
  }

  if (host.isActivePreviewHtmlSource) {
    host.structureStatus = "Nu pot muta elementul în HTML activ: sursa și destinația nu au locații sursă unice pentru kernel.";
    host.setGlobalStatus(host.structureStatus, "error");
    return blockedAction(host.structureStatus);
  }

  host.structureStatus = "Move Engine-ul nu a primit identități Source Graph pentru sursă și destinație.";
  host.setGlobalStatus(host.structureStatus, "error");
  return blockedAction(host.structureStatus);
}
