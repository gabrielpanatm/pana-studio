import { normalizeProjectDocumentPath } from "$lib/contracts/canvas-identity";
import type { CanvasAgentGestureMessage } from "$lib/preview/canvas-interaction";
import type {
  CanvasInteractionBindingReceipt,
  CanvasInteractionTarget,
  CanvasPointerSample,
} from "$lib/canvas/contracts";
import type { EditorMovePlan } from "$lib/editor/contracts";
import type {
  EditorNavigationNode,
  EditorNavigationSnapshot,
} from "$lib/editor/contracts";
import type { ProjectMovePosition } from "$lib/preview/contracts";
import type { ProjectScan } from "$lib/project/lifecycle-contract";

export type PendingCanvasInspection = {
  target: CanvasInteractionTarget;
  selectionRevision: number;
  pointer: CanvasPointerSample;
  openContextMenu: boolean;
  revealCode: boolean;
};

export type CanvasDragMovePreview = {
  sessionId: string;
  sourceNodeId: string;
  targetNodeId: string;
  position: ProjectMovePosition;
  plan: EditorMovePlan;
  projectedGestureSequence: number | null;
};

export type CanvasInteractionAuthorityPhase =
  | "dormant"
  | "suspended"
  | "waitingNavigation"
  | "binding"
  | "activating"
  | "active"
  | "failed";

export type CanvasInteractionFrontendRuntime = {
  agentInstanceId: string | null;
  documentEpoch: number;
  nextDocumentEpoch: number;
  bindSerial: number;
  desiredBindingKey: string | null;
  phase: CanvasInteractionAuthorityPhase;
  binding: CanvasInteractionBindingReceipt | null;
  pendingBinding: CanvasInteractionBindingReceipt | null;
  activationTimer: number | null;
  gestureTail: Promise<void>;
  interactionGeneration: number;
  pointerHoverGeneration: number;
  pointerHoverRunningGeneration: number | null;
  pendingPointerMove: CanvasAgentGestureMessage | null;
  pendingDragOver: CanvasAgentGestureMessage | null;
  dragOverRunningGeneration: number | null;
  dragOverTail: Promise<void>;
  latestPointerMoveSequence: number;
  latestDragOverSequence: number;
  lastObservedAgentSequence: number;
  pendingInspections: Map<string, PendingCanvasInspection>;
  nextInspectionSerial: number;
  dragSource: {
    sessionId: string;
    target: CanvasInteractionTarget;
  } | null;
  dragMovePreview: CanvasDragMovePreview | null;
  documentPathSource: string | null;
  documentProject: ProjectScan | null;
  expectedDocumentPath: string | null;
  indexedNavigationSnapshot: EditorNavigationSnapshot | null;
  navigationNodesById: Map<string, EditorNavigationNode>;
  navigationNodesByRenderInstanceId: Map<string, EditorNavigationNode>;
};

type CanvasInteractionRuntimeHost = {
  session: {
    activeScannedPath: string | null;
    scannedProject: ProjectScan | null;
  };
  selection: {
    editorSelection: {
      navigationSnapshot: EditorNavigationSnapshot | null;
    };
  };
};

const runtimes = new WeakMap<object, CanvasInteractionFrontendRuntime>();

export function canvasInteractionRuntimeFor(host: CanvasInteractionRuntimeHost) {
  let runtime = runtimes.get(host);
  if (runtime) return runtime;
  runtime = {
    agentInstanceId: null,
    documentEpoch: 0,
    nextDocumentEpoch: 0,
    bindSerial: 0,
    desiredBindingKey: null,
    phase: "dormant",
    binding: null,
    pendingBinding: null,
    activationTimer: null,
    gestureTail: Promise.resolve(),
    interactionGeneration: 0,
    pointerHoverGeneration: 0,
    pointerHoverRunningGeneration: null,
    pendingPointerMove: null,
    pendingDragOver: null,
    dragOverRunningGeneration: null,
    dragOverTail: Promise.resolve(),
    latestPointerMoveSequence: 0,
    latestDragOverSequence: 0,
    lastObservedAgentSequence: 0,
    pendingInspections: new Map(),
    nextInspectionSerial: 0,
    dragSource: null,
    dragMovePreview: null,
    documentPathSource: null,
    documentProject: null,
    expectedDocumentPath: null,
    indexedNavigationSnapshot: null,
    navigationNodesById: new Map(),
    navigationNodesByRenderInstanceId: new Map(),
  };
  runtimes.set(host, runtime);
  return runtime;
}

export function nextCanvasDocumentEpoch(runtime: CanvasInteractionFrontendRuntime) {
  const wallClockEpoch = Math.trunc(Date.now());
  const epoch = Math.max(runtime.nextDocumentEpoch + 1, wallClockEpoch);
  if (!Number.isSafeInteger(epoch) || epoch <= 0) {
    throw new Error("Canvas Interaction nu poate aloca o generație sigură.");
  }
  runtime.nextDocumentEpoch = epoch;
  return epoch;
}

export function clearCanvasActivationTimer(runtime: CanvasInteractionFrontendRuntime) {
  if (runtime.activationTimer === null) return;
  window.clearTimeout(runtime.activationTimer);
  runtime.activationTimer = null;
}

export function resetCanvasInteractionRuntime(runtime: CanvasInteractionFrontendRuntime) {
  runtime.interactionGeneration += 1;
  runtime.pointerHoverGeneration += 1;
  runtime.gestureTail = Promise.resolve();
  runtime.pendingPointerMove = null;
  runtime.pendingDragOver = null;
  runtime.dragOverRunningGeneration = null;
  runtime.dragOverTail = Promise.resolve();
  runtime.binding = null;
  runtime.pendingBinding = null;
  runtime.pendingInspections.clear();
  runtime.dragSource = null;
  runtime.dragMovePreview = null;
  runtime.indexedNavigationSnapshot = null;
  runtime.navigationNodesById.clear();
  runtime.navigationNodesByRenderInstanceId.clear();
  runtime.latestPointerMoveSequence = 0;
  runtime.latestDragOverSequence = 0;
  runtime.lastObservedAgentSequence = 0;
}

export function suspendCanvasInteractionRuntime(runtime: CanvasInteractionFrontendRuntime) {
  runtime.interactionGeneration += 1;
  runtime.pointerHoverGeneration += 1;
  runtime.gestureTail = Promise.resolve();
  runtime.pendingPointerMove = null;
  runtime.pendingDragOver = null;
  runtime.dragOverRunningGeneration = null;
  runtime.dragOverTail = Promise.resolve();
  runtime.pendingInspections.clear();
  runtime.dragSource = null;
  runtime.dragMovePreview = null;
}

function computeExpectedCanvasDocumentPath(host: CanvasInteractionRuntimeHost) {
  const activePath = normalizeProjectDocumentPath(host.session.activeScannedPath);
  if (!activePath) return null;
  const activeFile = host.session.scannedProject?.files.find(
    (file) => normalizeProjectDocumentPath(file.relativePath) === activePath,
  );
  return activeFile?.role === "template"
    ? normalizeProjectDocumentPath(activeFile.relativePath)
    : null;
}

export function refreshExpectedCanvasDocumentPath(
  host: CanvasInteractionRuntimeHost,
  runtime: CanvasInteractionFrontendRuntime,
) {
  if (
    runtime.documentPathSource === host.session.activeScannedPath
    && runtime.documentProject === host.session.scannedProject
  ) return runtime.expectedDocumentPath;
  runtime.documentPathSource = host.session.activeScannedPath;
  runtime.documentProject = host.session.scannedProject;
  runtime.expectedDocumentPath = computeExpectedCanvasDocumentPath(host);
  return runtime.expectedDocumentPath;
}

export function canvasRuntimeDocumentPathIsCurrent(
  host: CanvasInteractionRuntimeHost,
  runtime: CanvasInteractionFrontendRuntime,
) {
  return runtime.documentPathSource === host.session.activeScannedPath
    && runtime.documentProject === host.session.scannedProject;
}

export function canvasNavigationNodeIndex(
  host: CanvasInteractionRuntimeHost,
  runtime = canvasInteractionRuntimeFor(host),
) {
  const snapshot = host.selection.editorSelection.navigationSnapshot;
  if (runtime.indexedNavigationSnapshot !== snapshot) {
    runtime.indexedNavigationSnapshot = snapshot;
    runtime.navigationNodesById = new Map();
    runtime.navigationNodesByRenderInstanceId = new Map();
    for (const node of snapshot?.nodes ?? []) {
      runtime.navigationNodesById.set(node.id, node);
      if (node.renderInstanceId) {
        runtime.navigationNodesByRenderInstanceId.set(node.renderInstanceId, node);
      }
    }
  }
  return {
    snapshot,
    byId: runtime.navigationNodesById,
    byRenderInstanceId: runtime.navigationNodesByRenderInstanceId,
  };
}
