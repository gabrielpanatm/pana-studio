import {
  projectCommittedPreviewStructuralMutation,
  previewStructuralBlockingDiagnostic,
  requireCommittedPreviewStructuralPatch,
  type PreviewStructuralCanonicalProjectionHost,
  type PreviewStructuralExecutionReceipt,
} from "$lib/kernel/preview-projection-control";
import {
  blockedAction,
  cancelledAction,
  committedAction,
  failedAction,
  type EditorActionOutcome,
} from "$lib/editor-runtime/action-outcome";
import {
  previewStructuralCommandIdentity,
  runInPreviewStructuralLane,
  type PreviewStructuralSessionLease,
} from "$lib/kernel/preview-structural-lane";
import { projectRelativeZolaPath, scannedCacheKey } from "$lib/project/files";
import {
  executePreviewTeraDeleteIntent,
  executePreviewTeraInsertDropIntent,
  executePreviewTeraMoveDropIntent,
} from "$lib/project/io";
import { resolveTeraDropTarget } from "$lib/tera/drop-targets";
import { resolveTeraMoveTarget } from "$lib/tera/move-targets";
import { deleteTeraNodeCapability } from "$lib/tera/mutations";
import type { TeraDropRequest, TeraMoveRequest } from "$lib/tera/model";
import type { SaveState, SourceGraph, SourceGraphNode } from "$lib/types";
import { errorMessage } from "$lib/util";

export type TeraActionsControllerHost = PreviewStructuralCanonicalProjectionHost & {
  sourceGraph: SourceGraph | null;
  selectedTemplateSourceNode: SourceGraphNode | null;
  selectedTemplateSourceId: string | null;
  templateHtmlEditSourceId: string | null;
  activeScannedPath: string | null;
  activeRenderedTemplatePath: string | null;
  source: string;
  sourceCache: Record<string, string>;
  clearPreviewSelection: (options?: { clearTemplateGate?: boolean; clearHtmlMarker?: boolean }) => void;
  refreshSourceGraph: (options?: { strict?: boolean }) => Promise<void>;
  setGlobalStatus: (text: string, kind: SaveState) => void;
};

export function captureTeraActionTarget(
  node: SourceGraphNode | null | undefined,
): SourceGraphNode | null {
  if (!node) return null;
  return Object.freeze({
    ...node,
    range: node.range ? Object.freeze({ ...node.range }) : null,
    children: Object.freeze([...node.children]) as unknown as string[],
    capabilities: Object.freeze({ ...node.capabilities }),
  });
}

function dropPositionLabel(position: TeraDropRequest["position"]) {
  if (position === "before") return "înainte";
  if (position === "after") return "după";
  return "în interior";
}

function blockedTeraReceiptOutcome(
  receipt: PreviewStructuralExecutionReceipt,
  fallback: string,
): EditorActionOutcome | null {
  if (receipt.status === "committed") return null;
  return blockedAction(
    previewStructuralBlockingDiagnostic(receipt)?.message
      || receipt.message
      || fallback,
  );
}

function projectCommittedTeraSource(
  host: TeraActionsControllerHost,
  patch: { file: string; contents: string },
) {
  host.sourceCache = {
    ...host.sourceCache,
    [scannedCacheKey({ relativePath: patch.file })]: patch.contents,
  };
  if (host.activeScannedPath === patch.file) {
    host.source = patch.contents;
  }
}

export async function insertTeraPaletteItemAtTarget(
  host: TeraActionsControllerHost,
  request: TeraDropRequest,
): Promise<EditorActionOutcome> {
  try {
    const result = await runInPreviewStructuralLane(host, (lease) =>
      insertTeraPaletteItemAtTargetInLane(host, request, lease));
    return result ?? cancelledAction("Inserarea Tera a fost anulată odată cu sesiunea structurală.");
  } catch (error) {
    const reason = errorMessage(error);
    host.setGlobalStatus(`Eroare inserare Tera: ${reason}`, "error");
    return failedAction(reason);
  }
}

async function insertTeraPaletteItemAtTargetInLane(
  host: TeraActionsControllerHost,
  request: TeraDropRequest,
  lease: PreviewStructuralSessionLease,
): Promise<EditorActionOutcome> {
  const resolution = resolveTeraDropTarget(host.sourceGraph, request);
  if (!resolution.allowed) {
    host.setGlobalStatus(resolution.reason, "error");
    return blockedAction(resolution.reason);
  }

  const anchor = resolution.anchor;
  const range = anchor.range;
  if (!range) {
    host.setGlobalStatus("Nu pot insera Tera fără o ancoră de sursă stabilă.", "error");
    return blockedAction("Nu pot insera Tera fără o ancoră de sursă stabilă.");
  }

  try {
    const receipt = await executePreviewTeraInsertDropIntent({
      intent: {
        messageType: "preview-tera-drop",
        targetSelector: request.targetSelector,
        targetSourceId: request.targetSourceId,
        targetTemplateSourceId: request.targetTemplateSourceId,
        targetSessionId: request.targetSessionId,
        targetTag: request.targetTag,
        targetKind: anchor.kind,
        position: request.position,
        itemKind: request.item.kind,
      },
      insertIntent: {
        targetSourceId: anchor.id,
        targetLocation: {
          file: anchor.file,
          line: range.line,
          column: range.column,
        },
        targetKind: anchor.kind,
        targetTag: request.targetTag,
        targetSelector: request.targetSelector,
        position: request.position,
        item: {
          kind: request.item.kind,
          label: request.item.label,
          target: request.item.target ?? null,
          name: request.item.name ?? null,
          expression: request.item.expression ?? null,
        },
      },
    }, previewStructuralCommandIdentity(lease));
    const blocked = blockedTeraReceiptOutcome(
      receipt,
      "Tera Insert Engine-ul a blocat inserarea.",
    );
    if (blocked) return blocked;
    const patch = requireCommittedPreviewStructuralPatch(
      receipt,
      "Tera Insert Engine-ul a blocat inserarea.",
    );
    await projectCommittedPreviewStructuralMutation(host, lease, receipt, patch, async () => {
      projectCommittedTeraSource(host, patch);
      await host.refreshSourceGraph({ strict: true });
      host.setGlobalStatus(`${resolution.label} adăugat ${dropPositionLabel(resolution.position)} prin kernel în ${projectRelativeZolaPath(patch.file)}.`, "saved");
    });
    return committedAction();
  } catch (error) {
    const reason = errorMessage(error);
    host.setGlobalStatus(`Eroare inserare Tera: ${reason}`, "error");
    return failedAction(reason);
  }
}

export async function deleteSelectedTeraNode(
  host: TeraActionsControllerHost,
  requestedNode?: SourceGraphNode | null,
): Promise<EditorActionOutcome> {
  const targetNode = captureTeraActionTarget(
    requestedNode === undefined ? host.selectedTemplateSourceNode : requestedNode,
  );
  try {
    const result = await runInPreviewStructuralLane(host, (lease) =>
      deleteSelectedTeraNodeInLane(host, targetNode, lease));
    return result ?? cancelledAction("Ștergerea Tera a fost anulată odată cu sesiunea structurală.");
  } catch (error) {
    const reason = errorMessage(error);
    host.setGlobalStatus(`Eroare ștergere Tera: ${reason}`, "error");
    return failedAction(reason);
  }
}

async function deleteSelectedTeraNodeInLane(
  host: TeraActionsControllerHost,
  node: SourceGraphNode | null,
  lease: PreviewStructuralSessionLease,
): Promise<EditorActionOutcome> {
  const capability = deleteTeraNodeCapability(node);
  if (!node || !capability.canRun || !node.range) {
    host.setGlobalStatus(capability.reason, "error");
    return blockedAction(capability.reason);
  }

  try {
    const receipt = await executePreviewTeraDeleteIntent({
      intent: {
        messageType: "preview-template-delete-selected",
        sourceId: node.id,
      },
      deleteIntent: {
        targetSourceId: node.id,
        targetLocation: {
          file: node.file,
          line: node.range.line,
          column: node.range.column,
        },
        targetKind: node.kind,
        targetLabel: node.label,
      },
    }, previewStructuralCommandIdentity(lease));
    const blocked = blockedTeraReceiptOutcome(
      receipt,
      "Tera Delete Engine-ul a blocat ștergerea.",
    );
    if (blocked) return blocked;
    const patch = requireCommittedPreviewStructuralPatch(
      receipt,
      "Tera Delete Engine-ul a blocat ștergerea.",
    );
    await projectCommittedPreviewStructuralMutation(host, lease, receipt, patch, async () => {
      projectCommittedTeraSource(host, patch);
      host.templateHtmlEditSourceId = null;
      host.clearPreviewSelection({ clearTemplateGate: true, clearHtmlMarker: true });
      await host.refreshSourceGraph({ strict: true });
      host.setGlobalStatus(`${capability.label} executat prin kernel în ${projectRelativeZolaPath(patch.file)}.`, "saved");
    });
    return committedAction();
  } catch (error) {
    const reason = errorMessage(error);
    host.setGlobalStatus(`Eroare ștergere Tera: ${reason}`, "error");
    return failedAction(reason);
  }
}

export async function moveTeraNodeAtTarget(
  host: TeraActionsControllerHost,
  request: TeraMoveRequest,
): Promise<EditorActionOutcome> {
  try {
    const result = await runInPreviewStructuralLane(host, (lease) =>
      moveTeraNodeAtTargetInLane(host, request, lease));
    return result ?? cancelledAction("Mutarea Tera a fost anulată odată cu sesiunea structurală.");
  } catch (error) {
    const reason = errorMessage(error);
    host.setGlobalStatus(`Eroare mutare Tera: ${reason}`, "error");
    return failedAction(reason);
  }
}

async function moveTeraNodeAtTargetInLane(
  host: TeraActionsControllerHost,
  request: TeraMoveRequest,
  lease: PreviewStructuralSessionLease,
): Promise<EditorActionOutcome> {
  const resolution = resolveTeraMoveTarget(host.sourceGraph, request, {
    activeScannedPath: host.activeScannedPath,
    activeTemplatePath: host.activeRenderedTemplatePath,
  });
  if (!resolution.allowed) {
    host.setGlobalStatus(resolution.reason, "error");
    return blockedAction(resolution.reason);
  }

  const label = dropPositionLabel(resolution.position);
  const sourceRange = resolution.source.range;
  const targetRange = resolution.anchor.range;
  if (!sourceRange || !targetRange) {
    host.setGlobalStatus("Mutarea Tera cere ancore Source Graph stabile pentru sursă și destinație.", "error");
    return blockedAction("Mutarea Tera cere ancore Source Graph stabile pentru sursă și destinație.");
  }

  try {
    const receipt = await executePreviewTeraMoveDropIntent({
      intent: {
        messageType: "preview-tera-move-drop",
        sourceId: resolution.source.id,
        targetSelector: request.targetSelector,
        targetSourceId: resolution.anchor.id,
        targetTemplateSourceId: request.targetTemplateSourceId,
        targetTag: request.targetTag,
        targetKind: resolution.anchor.kind,
        position: request.position,
      },
      moveIntent: {
        sourceSourceId: resolution.source.id,
        targetSourceId: resolution.anchor.id,
        sourceLocation: {
          file: resolution.source.file,
          line: sourceRange.line,
          column: sourceRange.column,
        },
        targetLocation: {
          file: resolution.anchor.file,
          line: targetRange.line,
          column: targetRange.column,
        },
        sourceKind: resolution.source.kind,
        targetKind: resolution.anchor.kind,
        sourceLabel: resolution.source.label,
        targetTag: request.targetTag,
        targetSelector: request.targetSelector,
        position: resolution.position,
      },
    }, previewStructuralCommandIdentity(lease));
    const blocked = blockedTeraReceiptOutcome(
      receipt,
      "Tera Move Engine-ul a blocat mutarea.",
    );
    if (blocked) return blocked;
    const patch = requireCommittedPreviewStructuralPatch(
      receipt,
      "Tera Move Engine-ul a blocat mutarea.",
    );
    await projectCommittedPreviewStructuralMutation(host, lease, receipt, patch, async () => {
      projectCommittedTeraSource(host, patch);
      host.templateHtmlEditSourceId = null;
      host.clearPreviewSelection({ clearTemplateGate: true, clearHtmlMarker: true });
      await host.refreshSourceGraph({ strict: true });
      host.setGlobalStatus(`${resolution.label} mutat ${label} prin kernel în ${projectRelativeZolaPath(patch.file)}.`, "saved");
    });
    return committedAction();
  } catch (error) {
    const reason = errorMessage(error);
    host.setGlobalStatus(`Eroare mutare Tera: ${reason}`, "error");
    return failedAction(reason);
  }
}
