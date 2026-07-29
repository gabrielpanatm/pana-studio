import { contextMenu } from "$lib/context-menu/store.svelte";
import {
  htmlElementContextMenuItems,
  teraContextMenuItems,
} from "$lib/editor-runtime/context-menu";
import {
  htmlTargetFromCoordinatedSelection,
  teraTargetFromBoundary,
} from "$lib/editor-runtime/commands";
import {
  createCanvasInteractionIdentity,
  createCanvasInteractionRequest,
  parseCanvasAgentMessage,
  type CanvasAgentActionMessage,
  type CanvasAgentDomInspectionMessage,
  type CanvasAgentGestureMessage,
} from "$lib/preview/canvas-interaction";
import {
  bindCanvasInteractionAgent,
  requestEditorEditScope,
  resolveCanvasInteractionIntent,
} from "$lib/project/io";
import { t } from "$lib/i18n/runtime.svelte";
import type { AppState } from "$lib/state/app.svelte";
import type {
  CanvasInteractionBindingReceipt,
  CanvasInteractionIdentity,
  CanvasInteractionReceipt,
  CanvasInteractionTarget,
  CanvasOverlayProjection,
  EditorNavigationNode,
  CanvasPointerSample,
  EditorMovePlan,
  ProjectMovePosition,
  SelectionSnapshot,
} from "$lib/types";
import { errorMessage } from "$lib/util";

type PendingInspection = {
  target: CanvasInteractionTarget;
  selectionRevision: number;
  pointer: CanvasPointerSample;
  openContextMenu: boolean;
  revealCode: boolean;
};

type CanvasInteractionAuthorityPhase =
  | "dormant"
  | "binding"
  | "activating"
  | "active"
  | "failed";

type CanvasDragMovePreview = {
  sessionId: string;
  sourceNodeId: string;
  targetNodeId: string;
  position: ProjectMovePosition;
  receipt: CanvasInteractionReceipt;
  pending: boolean;
  plan: EditorMovePlan | null;
  error: string;
  promise: Promise<EditorMovePlan | null> | null;
};

type CanvasDragPermission = {
  state: "pending" | "allowed" | "blocked";
};

type CanvasInteractionFrontendRuntime = {
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
  latestPointerMoveSequence: number;
  latestDragOverSequence: number;
  lastObservedAgentSequence: number;
  pendingInspections: Map<string, PendingInspection>;
  nextInspectionSerial: number;
  dragSource: {
    sessionId: string;
    target: CanvasInteractionTarget;
  } | null;
  dragMovePreview: CanvasDragMovePreview | null;
};

const runtimes = new WeakMap<AppState, CanvasInteractionFrontendRuntime>();
const CANVAS_AGENT_ACTIVATION_TIMEOUT_MS = 2_000;

function runtimeFor(app: AppState) {
  let runtime = runtimes.get(app);
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
    latestPointerMoveSequence: 0,
    latestDragOverSequence: 0,
    lastObservedAgentSequence: 0,
    pendingInspections: new Map(),
    nextInspectionSerial: 0,
    dragSource: null,
    dragMovePreview: null,
  };
  runtimes.set(app, runtime);
  return runtime;
}

function canvasBindingKey(
  canvas: CanvasInteractionIdentity["canvas"],
  route: string,
  agentInstanceId: string,
) {
  return JSON.stringify([
    canvas.projectRoot,
    canvas.runtimeSessionId,
    canvas.workspaceRevision,
    canvas.transactionId,
    canvas.previewRevision,
    route,
    agentInstanceId,
  ]);
}

function nextDocumentEpoch(runtime: CanvasInteractionFrontendRuntime) {
  const wallClockEpoch = Math.trunc(Date.now());
  const epoch = Math.max(runtime.nextDocumentEpoch + 1, wallClockEpoch);
  if (!Number.isSafeInteger(epoch) || epoch <= 0) {
    throw new Error("Canvas Interaction nu poate aloca o generație sigură.");
  }
  runtime.nextDocumentEpoch = epoch;
  return epoch;
}

function clearActivationTimer(runtime: CanvasInteractionFrontendRuntime) {
  if (runtime.activationTimer === null) return;
  window.clearTimeout(runtime.activationTimer);
  runtime.activationTimer = null;
}

function clearRuntimeInteractionState(runtime: CanvasInteractionFrontendRuntime) {
  runtime.binding = null;
  runtime.pendingBinding = null;
  runtime.pendingInspections.clear();
  runtime.dragSource = null;
  runtime.dragMovePreview = null;
  runtime.latestPointerMoveSequence = 0;
  runtime.latestDragOverSequence = 0;
  runtime.lastObservedAgentSequence = 0;
}

function deactivateCanvasAgent(
  app: AppState,
  runtime: CanvasInteractionFrontendRuntime,
  agentInstanceId = runtime.agentInstanceId,
) {
  if (!agentInstanceId) return;
  app.postPreviewMessage({
    type: "deactivate-canvas-interaction-agent",
    agentInstanceId,
  });
}

function sameCanvasIdentity(
  left: CanvasInteractionIdentity["canvas"] | null | undefined,
  right: CanvasInteractionIdentity["canvas"] | null | undefined,
) {
  return Boolean(
    left
    && right
    && left.projectRoot === right.projectRoot
    && left.runtimeSessionId === right.runtimeSessionId
    && left.workspaceRevision === right.workspaceRevision
    && left.transactionId === right.transactionId
    && left.previewRevision === right.previewRevision,
  );
}

function previewRoute(app: AppState) {
  const source = app.activeCanvasUrl && app.activeCanvasUrl !== "about:blank"
    ? app.activeCanvasUrl
    : app.previewSrc;
  if (source && source !== "about:blank") {
    try {
      return new URL(source, "http://pana.local/").pathname || "/";
    } catch {
      // Fall through to the browser-owned route.
    }
  }
  const fallback = app.browserPreviewRoute.trim() || "/";
  return fallback.startsWith("/") ? fallback : `/${fallback}`;
}

function currentBinding(
  app: AppState,
  runtime = runtimeFor(app),
) {
  const binding = runtime.binding;
  return runtime.phase === "active"
    && binding
    && runtime.agentInstanceId === binding.identity.agentInstanceId
    && runtime.documentEpoch === binding.identity.documentEpoch
    && sameCanvasIdentity(app.activeCanvasIdentity, binding.identity.canvas)
    ? binding
    : null;
}

export function handleCanvasAgentMessage(app: AppState, event: MessageEvent) {
  const raw = event.data as Record<string, unknown> | null;
  if (raw?.source !== "pana-studio-canvas-agent") return false;

  const runtime = runtimeFor(app);
  const ready = raw.type === "agentReady";
  const message = parseCanvasAgentMessage(
    app.previewFrame,
    event,
    ready ? null : runtime.agentInstanceId,
  );
  if (!message) return true;

  if (message.type === "agentReady") {
    if (runtime.agentInstanceId !== message.agentInstanceId) {
      const previousAgentInstanceId = runtime.agentInstanceId;
      runtime.bindSerial += 1;
      clearActivationTimer(runtime);
      deactivateCanvasAgent(app, runtime, previousAgentInstanceId);
      runtime.agentInstanceId = message.agentInstanceId;
      runtime.documentEpoch = 0;
      runtime.desiredBindingKey = null;
      runtime.phase = "dormant";
      clearRuntimeInteractionState(runtime);
    }
    synchronizeCanvasInteractionBinding(app);
    return true;
  }

  if (message.type === "agentActivated") {
    const pendingBinding = runtime.pendingBinding;
    if (
      runtime.phase !== "activating"
      || !pendingBinding
      || message.agentInstanceId !== pendingBinding.identity.agentInstanceId
      || message.documentEpoch !== pendingBinding.identity.documentEpoch
      || runtime.desiredBindingKey !== canvasBindingKey(
        pendingBinding.identity.canvas,
        pendingBinding.identity.route,
        pendingBinding.identity.agentInstanceId,
      )
      || !sameCanvasIdentity(app.activeCanvasIdentity, pendingBinding.identity.canvas)
    ) return true;
    clearActivationTimer(runtime);
    runtime.pendingBinding = null;
    runtime.binding = pendingBinding;
    runtime.phase = "active";
    runtime.latestPointerMoveSequence = pendingBinding.lastAcceptedSequence;
    runtime.latestDragOverSequence = pendingBinding.lastAcceptedSequence;
    runtime.lastObservedAgentSequence = pendingBinding.lastAcceptedSequence;
    projectCurrentSelectionOverlay(app, pendingBinding);
    return true;
  }

  const binding = currentBinding(app, runtime);
  if (
    !binding
    || message.documentEpoch !== binding.identity.documentEpoch
    || message.agentInstanceId !== binding.identity.agentInstanceId
  ) return true;

  if (message.type === "domInspection") {
    void applyDomInspection(app, runtime, message);
    return true;
  }
  if (message.type === "action") {
    if (message.actionSequence <= runtime.lastObservedAgentSequence) return true;
    runtime.lastObservedAgentSequence = message.actionSequence;
    runtime.gestureTail = runtime.gestureTail
      .then(async () => {
        if (message.action === "enterBoundary") {
          await enterBoundaryFromAgentAction(app, runtime, message);
        } else {
          await deleteSelectionFromAgentAction(app, runtime, message);
        }
      })
      .catch((error) => {
        failCanvasInteractionBinding(app, runtime, error);
      });
    return true;
  }

  if (message.gestureSequence <= runtime.lastObservedAgentSequence) return true;
  runtime.lastObservedAgentSequence = message.gestureSequence;
  if (message.gesture === "pointerMove") {
    runtime.latestPointerMoveSequence = message.gestureSequence;
  } else if (message.gesture === "dragOver") {
    runtime.latestDragOverSequence = message.gestureSequence;
  }
  runtime.gestureTail = runtime.gestureTail
    .then(async () => {
      if (
        message.gesture === "pointerMove"
        && message.gestureSequence !== runtime.latestPointerMoveSequence
      ) return;
      if (
        message.gesture === "dragOver"
        && message.gestureSequence !== runtime.latestDragOverSequence
      ) return;
      await resolveGesture(app, runtime, message);
    })
    .catch((error) => {
      failCanvasInteractionBinding(app, runtime, error);
    });
  return true;
}

export async function retryCanvasInteractionBinding(app: AppState) {
  const runtime = runtimeFor(app);
  if (!runtime.agentInstanceId || currentBinding(app, runtime)) return;
  synchronizeCanvasInteractionBinding(app);
  if (runtime.phase === "binding" || runtime.phase === "activating") return;
  await bindCurrentCanvasAgent(app);
}

export function synchronizeCanvasInteractionBinding(app: AppState) {
  const runtime = runtimeFor(app);
  const canvas = app.activeCanvasIdentity;
  const agentInstanceId = runtime.agentInstanceId;
  if (!canvas || !agentInstanceId) {
    runtime.bindSerial += 1;
    clearActivationTimer(runtime);
    clearRuntimeInteractionState(runtime);
    runtime.desiredBindingKey = null;
    runtime.documentEpoch = 0;
    runtime.phase = "dormant";
    deactivateCanvasAgent(app, runtime);
    return;
  }

  const route = previewRoute(app);
  const desiredBindingKey = canvasBindingKey(canvas, route, agentInstanceId);
  if (runtime.desiredBindingKey !== desiredBindingKey) {
    runtime.bindSerial += 1;
    clearActivationTimer(runtime);
    clearRuntimeInteractionState(runtime);
    deactivateCanvasAgent(app, runtime);
    runtime.desiredBindingKey = desiredBindingKey;
    runtime.documentEpoch = nextDocumentEpoch(runtime);
    runtime.phase = "binding";
    void bindCurrentCanvasAgent(app);
    return;
  }
  if (currentBinding(app, runtime)) return;
  if (runtime.phase === "binding" || runtime.phase === "activating") return;
  runtime.phase = "binding";
  void bindCurrentCanvasAgent(app);
}

async function bindCurrentCanvasAgent(app: AppState) {
  const runtime = runtimeFor(app);
  const canvas = app.activeCanvasIdentity;
  const agentInstanceId = runtime.agentInstanceId;
  const documentEpoch = runtime.documentEpoch;
  if (!canvas || !agentInstanceId || documentEpoch <= 0) return;

  const route = previewRoute(app);
  const desiredBindingKey = canvasBindingKey(canvas, route, agentInstanceId);
  if (runtime.desiredBindingKey !== desiredBindingKey) return;
  runtime.phase = "binding";
  const serial = ++runtime.bindSerial;
  const identity = createCanvasInteractionIdentity(
    canvas,
    route,
    documentEpoch,
    agentInstanceId,
  );
  try {
    const receipt = await bindCanvasInteractionAgent(
      identity,
      app.editorNavigationSnapshot?.focusedView?.activeDocumentPath
        ?? null,
      app.coordinatedElementSelection?.renderInstanceId ?? null,
    );
    if (
      serial !== runtime.bindSerial
      || runtime.agentInstanceId !== agentInstanceId
      || runtime.documentEpoch !== documentEpoch
      || runtime.desiredBindingKey !== desiredBindingKey
      || !sameCanvasIdentity(app.activeCanvasIdentity, canvas)
    ) return;
    runtime.pendingBinding = receipt;
    runtime.phase = "activating";
    runtime.pendingInspections.clear();
    runtime.dragSource = null;
    runtime.dragMovePreview = null;
    runtime.latestPointerMoveSequence = receipt.lastAcceptedSequence;
    runtime.latestDragOverSequence = receipt.lastAcceptedSequence;
    runtime.lastObservedAgentSequence = receipt.lastAcceptedSequence;
    app.postPreviewMessage({
      type: "activate-canvas-interaction-agent",
      schemaVersion: receipt.schemaVersion,
      agentInstanceId,
      documentEpoch,
      lastAcceptedSequence: receipt.lastAcceptedSequence,
      selection: true,
    });
    clearActivationTimer(runtime);
    runtime.activationTimer = window.setTimeout(() => {
      if (
        runtime.phase !== "activating"
        || runtime.pendingBinding !== receipt
        || runtime.bindSerial !== serial
      ) return;
      failCanvasInteractionBinding(
        app,
        runtime,
        new Error("CanvasAgent nu a confirmat activarea."),
      );
    }, CANVAS_AGENT_ACTIVATION_TIMEOUT_MS);
  } catch (error) {
    if (serial !== runtime.bindSerial) return;
    clearActivationTimer(runtime);
    clearRuntimeInteractionState(runtime);
    runtime.phase = "failed";
    deactivateCanvasAgent(app, runtime, agentInstanceId);
    app.setGlobalStatus(
      `Canvas Interaction Rust nu a putut fi activat: ${errorMessage(error)}`,
      "error",
    );
  }
}

async function resolveGesture(
  app: AppState,
  runtime: CanvasInteractionFrontendRuntime,
  message: CanvasAgentGestureMessage,
) {
  const binding = currentBinding(app, runtime);
  if (!binding) return;
  const request = createCanvasInteractionRequest(binding.identity, message);
  const receipt = await resolveCanvasInteractionIntent({
    request,
    editScopeGrant: app.editorEditScopeGrant,
  });
  if (currentBinding(app, runtime) !== binding) return;

  if (receipt.status === "stale" || receipt.status === "rejected") {
    failCanvasInteractionBinding(
      app,
      runtime,
      new Error(receipt.diagnostics[0]?.message ?? "Recepție Canvas invalidă."),
    );
    return;
  }

  if (
    message.gesture === "dragStart"
    || message.gesture === "dragOver"
    || message.gesture === "drop"
  ) {
    await resolveDragGesture(app, runtime, binding, message, receipt);
    return;
  }
  if (message.gesture === "pointerMove") {
    if (receipt.target) {
      await app.applyHoverIntent({
        kind: "setHover",
        editorNodeId: receipt.target.editorNodeId,
        documentEpoch: message.documentEpoch,
      });
    } else {
      await app.applyHoverIntent({
        kind: "clearHover",
        documentEpoch: message.documentEpoch,
      });
    }
    renderReceiptOverlay(app, binding, "hover", receipt);
    return;
  }
  if (message.gesture === "pointerDown") {
    app.closeContextMenu();
    return;
  }
  if (message.gesture !== "click" && message.gesture !== "contextMenu") return;

  if (!receipt.target || !receipt.overlay) {
    if (message.gesture === "click") {
      runtime.pendingInspections.clear();
      await app.applySelectionIntent({ kind: "clearSelection" });
      app.postPreviewMessage({ type: "clear-canvas-interaction-overlays" });
    }
    return;
  }

  const selectionSnapshot = await app.applySelectionIntent({
    kind: "selectEditorNode",
    editorNodeId: receipt.target.editorNodeId,
  });
  if (
    !selectionSnapshot
    || selectionSnapshot.projections.layers.editorNodeId !== receipt.target.editorNodeId
  ) return;

  renderReceiptOverlay(
    app,
    binding,
    "selection",
    receipt,
    selectionSnapshot.selectionRevision,
  );
  if (receipt.target.kind === "teraBoundary") {
    if (message.gesture === "contextMenu") {
      openTeraContextMenu(app, receipt.target, message.pointer);
    }
    return;
  }

  if (!receipt.target.renderInstanceId || !receipt.target.actions.canInspect) return;
  requestDomInspection(app, runtime, receipt.target, {
    selectionRevision: selectionSnapshot.selectionRevision,
    pointer: message.pointer,
    openContextMenu: message.gesture === "contextMenu",
    revealCode: false,
  });
}

function requestDomInspection(
  app: AppState,
  runtime: CanvasInteractionFrontendRuntime,
  target: CanvasInteractionTarget,
  pending: Omit<PendingInspection, "target">,
) {
  const binding = currentBinding(app, runtime);
  if (!binding || !target.renderInstanceId || !target.actions.canInspect) return;
  runtime.nextInspectionSerial += 1;
  const inspectionRequestId = `inspection:${runtime.documentEpoch}:${runtime.nextInspectionSerial}`;
  runtime.pendingInspections.set(inspectionRequestId, { ...pending, target });
  while (runtime.pendingInspections.size > 32) {
    const oldest = runtime.pendingInspections.keys().next().value;
    if (typeof oldest !== "string") break;
    runtime.pendingInspections.delete(oldest);
  }
  app.postPreviewMessage({
    type: "inspect-canvas-interaction-target",
    schemaVersion: binding.schemaVersion,
    agentInstanceId: binding.identity.agentInstanceId,
    documentEpoch: binding.identity.documentEpoch,
    inspectionRequestId,
    renderInstanceId: target.renderInstanceId,
  });
}

async function resolveDragGesture(
  app: AppState,
  runtime: CanvasInteractionFrontendRuntime,
  binding: CanvasInteractionBindingReceipt,
  message: CanvasAgentGestureMessage,
  receipt: CanvasInteractionReceipt,
) {
  const drag = message.drag;
  if (!drag) return;
  if (message.gesture === "dragStart") {
    clearReceiptOverlay(app, binding, "drag");
    clearReceiptOverlay(app, binding, "hover");
    runtime.dragMovePreview = null;
    const target = receipt.target;
    runtime.dragSource = target
      && (target.actions.canMove || target.actions.canMoveAtomic)
      ? { sessionId: drag.sessionId, target }
      : null;
    if (runtime.dragSource) {
      renderReceiptOverlay(app, binding, "selection", receipt);
    }
    return;
  }

  const source = runtime.dragSource;
  if (!source || source.sessionId !== drag.sessionId) {
    clearReceiptOverlay(app, binding, "drag");
    return;
  }
  if (message.gesture === "dragOver") {
    if (
      !receipt.target
      || !receipt.dragPosition
      || source.target.editorNodeId === receipt.target.editorNodeId
    ) {
      runtime.dragMovePreview = null;
      clearReceiptOverlay(app, binding, "drag");
      return;
    }
    projectCanvasDragPermission(
      app,
      runtime,
      binding,
      source,
      drag.sessionId,
      receipt,
    );
    return;
  }

  const movePreview = runtime.dragMovePreview;
  runtime.dragSource = null;
  runtime.dragMovePreview = null;
  clearReceiptOverlay(app, binding, "drag");
  const target = receipt.target;
  if (!target || !drag.position || source.target.editorNodeId === target.editorNodeId) {
    return;
  }
  let plan: EditorMovePlan | null = null;
  let planError = "";
  if (
    movePreview
    && movePreview.sessionId === drag.sessionId
    && movePreview.sourceNodeId === source.target.editorNodeId
    && movePreview.targetNodeId === target.editorNodeId
    && movePreview.position === drag.position
  ) {
    plan = movePreview.promise
      ? await movePreview.promise
      : movePreview.plan;
    planError = movePreview.error;
  } else {
    try {
      plan = await app.previewEditorNavigationMove(
        source.target.editorNodeId,
        target.editorNodeId,
        drag.position,
      );
    } catch (error) {
      planError = errorMessage(error);
    }
  }
  if (!plan) {
    if (planError) app.setGlobalStatus(planError, "error");
    return;
  }
  if (!plan.allowed) {
    app.setGlobalStatus(
      plan.reason ?? t("editor-navigation-move-refused"),
      "error",
    );
    return;
  }
  await app.moveEditorNavigationNode(
    source.target.editorNodeId,
    target.editorNodeId,
    drag.position,
  );
  await retryCanvasInteractionBinding(app);
}

function projectCanvasDragPermission(
  app: AppState,
  runtime: CanvasInteractionFrontendRuntime,
  binding: CanvasInteractionBindingReceipt,
  source: NonNullable<CanvasInteractionFrontendRuntime["dragSource"]>,
  sessionId: string,
  receipt: CanvasInteractionReceipt,
) {
  const target = receipt.target;
  const position = receipt.dragPosition;
  if (!target || !position) return;

  const current = runtime.dragMovePreview;
  if (
    current
    && current.sessionId === sessionId
    && current.sourceNodeId === source.target.editorNodeId
    && current.targetNodeId === target.editorNodeId
    && current.position === position
  ) {
    current.receipt = receipt;
    renderReceiptOverlay(
      app,
      binding,
      "drag",
      receipt,
      undefined,
      sessionId,
      canvasDragPermission(current),
    );
    return;
  }

  const preview: CanvasDragMovePreview = {
    sessionId,
    sourceNodeId: source.target.editorNodeId,
    targetNodeId: target.editorNodeId,
    position,
    receipt,
    pending: true,
    plan: null,
    error: "",
    promise: null,
  };
  runtime.dragMovePreview = preview;
  renderReceiptOverlay(
    app,
    binding,
    "drag",
    receipt,
    undefined,
    sessionId,
    { state: "pending" },
  );
  preview.promise = app.previewEditorNavigationMove(
    preview.sourceNodeId,
    preview.targetNodeId,
    preview.position,
  )
    .then((plan) => {
      preview.plan = plan;
      return plan;
    })
    .catch((error) => {
      preview.error = errorMessage(error);
      return null;
    })
    .finally(() => {
      preview.pending = false;
      const activeSource = runtime.dragSource;
      if (
        runtime.dragMovePreview !== preview
        || currentBinding(app, runtime) !== binding
        || !activeSource
        || activeSource.sessionId !== sessionId
        || activeSource.target.editorNodeId !== preview.sourceNodeId
      ) return;
      renderReceiptOverlay(
        app,
        binding,
        "drag",
        preview.receipt,
        undefined,
        sessionId,
        canvasDragPermission(preview),
      );
    });
}

function canvasDragPermission(
  preview: CanvasDragMovePreview,
): CanvasDragPermission {
  if (preview.pending) return { state: "pending" };
  return { state: preview.plan?.allowed === true ? "allowed" : "blocked" };
}

function renderReceiptOverlay(
  app: AppState,
  binding: CanvasInteractionBindingReceipt,
  channel: "hover" | "selection" | "drag",
  receipt: CanvasInteractionReceipt,
  selectionRevision?: number,
  dragSessionId?: string,
  dragPermission?: CanvasDragPermission,
) {
  if (
    !receipt.target
    || !receipt.overlay
    || (channel === "drag" && (!receipt.dragPosition || !dragSessionId))
  ) {
    if (channel !== "selection") clearReceiptOverlay(app, binding, channel);
    return;
  }
  app.postPreviewMessage({
    type: "render-canvas-interaction-overlay",
    agentInstanceId: binding.identity.agentInstanceId,
    documentEpoch: binding.identity.documentEpoch,
    channel,
    targetKind: receipt.target.kind,
    editorNodeId: receipt.target.editorNodeId,
    actions: receipt.target.actions,
    gestureSequence: receipt.gestureSequence,
    ...(channel === "selection" && selectionRevision
      ? { selectionRevision }
      : {}),
    ...(channel === "drag"
      ? {
          dragPosition: receipt.dragPosition,
          dragSessionId,
          dragPermission: dragPermission ?? { state: "pending" },
        }
      : {}),
    projection: receipt.overlay,
  });
}

function clearReceiptOverlay(
  app: AppState,
  binding: CanvasInteractionBindingReceipt,
  channel: "hover" | "drag",
) {
  app.postPreviewMessage({
    type: "render-canvas-interaction-overlay",
    agentInstanceId: binding.identity.agentInstanceId,
    documentEpoch: binding.identity.documentEpoch,
    channel,
    targetKind: null,
    projection: { renderInstanceIds: [] },
  });
}

async function enterBoundaryFromAgentAction(
  app: AppState,
  runtime: CanvasInteractionFrontendRuntime,
  message: CanvasAgentActionMessage,
) {
  const binding = currentBinding(app, runtime);
  const coordinated = coordinatedActionTarget(app, binding, message);
  const target = coordinated?.target;
  const activeDocumentPath = binding?.activeDocumentPath ?? null;
  if (
    !binding
    || !target
    || message.action !== "enterBoundary"
    || target.kind !== "teraBoundary"
    || !target.actions.canEnterBoundary
    || !target.requiredEditScopeId
    || !activeDocumentPath
  ) return;

  const grant = await requestEditorEditScope(
    binding.identity.canvas,
    binding.identity.route,
    activeDocumentPath,
    target.requiredEditScopeId,
    coordinated.selection.projections.preview.primaryRenderInstanceId,
  );
  if (currentBinding(app, runtime) !== binding) return;
  app.editorEditScopeGrant = grant;
  app.editorEditScopeId = grant.scopeId;
  app.setGlobalStatus(
    target.effectScope === "sharedDefinition"
      ? "Scope-ul partajat a fost autorizat de kernel."
      : "Conținutul boundary-ului poate fi selectat și editat.",
    "idle",
  );
}

async function deleteSelectionFromAgentAction(
  app: AppState,
  runtime: CanvasInteractionFrontendRuntime,
  message: CanvasAgentActionMessage,
) {
  const binding = currentBinding(app, runtime);
  const coordinated = coordinatedActionTarget(app, binding, message);
  const target = coordinated?.target;
  if (
    !binding
    || !target
    || message.action !== "deleteSelection"
    || target.actions.readOnly
  ) return;

  if (target.kind === "htmlElement") {
    const selection = app.coordinatedElementSelection;
    if (
      !selection
      || !target.renderInstanceId
      || selection.renderInstanceId !== target.renderInstanceId
    ) return;
    await app.editorRuntime.dispatch({
      type: "delete-html",
      surface: "shortcut",
      target: htmlTargetFromCoordinatedSelection(selection),
    });
    return;
  }

  if (target.kind !== "teraBoundary" || !target.sourceNodeId) return;
  const sourceNode = app.sourceGraph?.nodes.find(
    (node) => node.id === target.sourceNodeId,
  ) ?? null;
  await app.editorRuntime.dispatch({
    type: "delete-tera",
    surface: "shortcut",
    target: teraTargetFromBoundary({
      selector: coordinated.selection.projections.preview.primaryRenderInstanceId
        ? `[data-pana-render-instance-id="${CSS.escape(coordinated.selection.projections.preview.primaryRenderInstanceId)}"]`
        : null,
      sourceId: target.sourceNodeId,
      origin: target.origin === "theme" ? "theme" : "current",
      themeName: target.themeName,
    }, {
      label: target.label,
      kindLabel: sourceNode?.kind ?? "Tera",
      file: target.file,
      sourceNode,
    }),
  });
}

function coordinatedActionTarget(
  app: AppState,
  binding: CanvasInteractionBindingReceipt | null,
  message: CanvasAgentActionMessage,
) {
  const selection = app.selectionSnapshot;
  if (
    !binding
    || !selection
    || selection.resolution !== "resolved"
    || selection.selectionRevision !== message.selectionRevision
    || selection.projections.layers.editorNodeId !== message.editorNodeId
    || !sameCanvasIdentity(selection.canvasIdentity, binding.identity.canvas)
  ) return null;
  const node = app.editorNavigationSnapshot?.nodes.find(
    (candidate) => candidate.id === message.editorNodeId,
  ) ?? null;
  if (!node) return null;
  return {
    selection,
    target: canvasTargetFromNavigationNode(app, node),
  };
}

async function applyDomInspection(
  app: AppState,
  runtime: CanvasInteractionFrontendRuntime,
  message: CanvasAgentDomInspectionMessage,
) {
  const pending = runtime.pendingInspections.get(message.inspectionRequestId);
  runtime.pendingInspections.delete(message.inspectionRequestId);
  const target = pending?.target;
  if (
    !pending
    || !target
    || target.renderInstanceId !== message.renderInstanceId
    || (target.tag && target.tag.toLowerCase() !== message.observation.tag)
    || app.selectionSnapshot?.selectionRevision !== pending.selectionRevision
  ) return;

  const observation = message.observation;
  const accepted = await app.acceptSelectionObservation({
    schemaVersion: 1,
    selectionRevision: pending.selectionRevision,
    canvasIdentity: app.selectionSnapshot.canvasIdentity,
    documentEpoch: message.documentEpoch,
    renderInstanceId: message.renderInstanceId,
    inspectorFacts: {
      observedTag: observation.tag,
      elementId: observation.id,
      classes: observation.classes,
      blockContext: observation.blockContext
        ? {
            providerId: observation.blockContext.providerId,
            markerKind: observation.blockContext.markerKind,
            rootTag: observation.blockContext.rootTag,
          }
        : null,
    },
  }, observation);
  if (!accepted || app.selectionSnapshot?.selectionRevision !== pending.selectionRevision) return;
  app.applySelectionState(observation);
  app.syncCodeSelectionHighlight(pending.revealCode);
  if (pending.openContextMenu) {
    openHtmlContextMenu(app, pending.pointer);
  }
}

function canvasTargetFromNavigationNode(
  app: AppState,
  node: EditorNavigationNode,
): CanvasInteractionTarget {
  const requiredEditScopeId = node.capabilities.requiresEditScopeId;
  return {
    editorNodeId: node.id,
    kind: node.kind,
    label: node.label,
    tag: node.tag,
    sourceNodeId: node.sourceNodeId,
    file: node.file,
    range: node.range,
    renderInstanceId: node.renderInstanceId,
    boundaryInstanceId: node.boundary?.boundaryInstanceId ?? null,
    origin: node.origin,
    themeName: node.themeName,
    sourceProvenance: node.sourceProvenance,
    requiredEditScopeId,
    scopeState: requiredEditScopeId === null
      ? "unscoped"
      : app.editorEditScopeGrant?.scopeId === requiredEditScopeId
        ? "authorized"
        : "locked",
    effectScope: node.boundary?.effectScope ?? "singleSource",
    renderedInstanceCount: node.boundary?.renderedInstanceCount ?? 1,
    actions: {
      canSelect: node.capabilities.canSelect,
      canInspect: node.capabilities.canInspect,
      canOpenInCode: node.capabilities.canOpenInCode,
      canEnterBoundary: node.capabilities.canEnterBoundary,
      canMoveAtomic: node.capabilities.canMoveAtomic,
      canMove: node.capabilities.canMove,
      canEditText: node.capabilities.canEditText,
      canEditAttributes: node.capabilities.canEditAttributes,
      readOnly: node.capabilities.readOnly,
      reasonCode: node.capabilities.reasonCode,
    },
  };
}

function canvasOverlayFromNavigationNode(
  node: EditorNavigationNode,
): CanvasOverlayProjection {
  if (node.boundary) {
    return {
      primaryRenderInstanceId: node.boundary.rootRenderInstanceIds[0] ?? null,
      renderInstanceIds: [...node.boundary.rootRenderInstanceIds],
      boundaryInstanceId: node.boundary.boundaryInstanceId,
    };
  }
  return {
    primaryRenderInstanceId: node.renderInstanceId,
    renderInstanceIds: node.renderInstanceId ? [node.renderInstanceId] : [],
    boundaryInstanceId: null,
  };
}

function projectCurrentSelectionOverlay(
  app: AppState,
  binding: CanvasInteractionBindingReceipt,
) {
  const selection = app.selectionSnapshot;
  const editorNodeId = selection?.projections.preview.editorNodeId;
  if (
    !selection
    || selection.resolution !== "resolved"
    || !editorNodeId
    || !sameCanvasIdentity(selection.canvasIdentity, binding.identity.canvas)
  ) return;
  const node = app.editorNavigationSnapshot?.nodes.find(
    (candidate) => candidate.id === editorNodeId,
  ) ?? null;
  if (!node) return;
  const target = canvasTargetFromNavigationNode(app, node);
  app.postPreviewMessage({
    type: "render-canvas-interaction-overlay",
    agentInstanceId: binding.identity.agentInstanceId,
    documentEpoch: binding.identity.documentEpoch,
    channel: "selection",
    targetKind: target.kind,
    editorNodeId: target.editorNodeId,
    actions: target.actions,
    selectionRevision: selection.selectionRevision,
    projection: canvasOverlayFromNavigationNode(node),
  });
  if (target.kind !== "teraBoundary") {
    requestDomInspection(app, runtimeFor(app), target, {
      selectionRevision: selection.selectionRevision,
      pointer: {
        clientX: 0,
        clientY: 0,
        button: "none",
        buttons: 0,
        modifiers: { alt: false, control: false, meta: false, shift: false },
      },
      openContextMenu: false,
      revealCode: false,
    });
  }
}

function currentNavigationNode(app: AppState, requested: EditorNavigationNode) {
  const binding = currentBinding(app);
  const snapshot = app.editorNavigationSnapshot;
  if (
    !binding
    || !snapshot
    || !sameCanvasIdentity(snapshot.identity, binding.identity.canvas)
  ) return null;
  return snapshot.nodes.find((node) => node.id === requested.id) ?? null;
}

export function hoverCanvasNavigationNode(
  app: AppState,
  requested: EditorNavigationNode | null,
) {
  const runtime = runtimeFor(app);
  const binding = currentBinding(app, runtime);
  if (!binding) return;
  if (!requested) {
    void app.applyHoverIntent({
      kind: "clearHover",
      documentEpoch: binding.identity.documentEpoch,
    });
    app.postPreviewMessage({
      type: "render-canvas-interaction-overlay",
      agentInstanceId: binding.identity.agentInstanceId,
      documentEpoch: binding.identity.documentEpoch,
      channel: "hover",
      targetKind: null,
      projection: { renderInstanceIds: [] },
    });
    return;
  }
  const node = currentNavigationNode(app, requested);
  if (!node) return;
  void app.applyHoverIntent({
    kind: "setHover",
    editorNodeId: node.id,
    documentEpoch: binding.identity.documentEpoch,
  });
  const target = canvasTargetFromNavigationNode(app, node);
  app.postPreviewMessage({
    type: "render-canvas-interaction-overlay",
    agentInstanceId: binding.identity.agentInstanceId,
    documentEpoch: binding.identity.documentEpoch,
    channel: "hover",
    targetKind: target.kind,
    editorNodeId: target.editorNodeId,
    actions: target.actions,
    projection: canvasOverlayFromNavigationNode(node),
  });
}

export function selectCanvasNavigationNode(
  app: AppState,
  requested: EditorNavigationNode,
) {
  const runtime = runtimeFor(app);
  const binding = currentBinding(app, runtime);
  const node = currentNavigationNode(app, requested);
  if (!binding || !node || !node.capabilities.canSelect) return;
  const target = canvasTargetFromNavigationNode(app, node);
  void commitNavigationSelection(app, runtime, binding, node, target, false);
}

export function selectCanvasPreviewElement(
  app: AppState,
  element: Element,
  options: { revealCode?: boolean } = {},
) {
  const renderInstanceId = element.getAttribute("data-pana-render-instance-id");
  if (!renderInstanceId) return false;
  const node = app.editorNavigationSnapshot?.nodes.find(
    (candidate) => candidate.renderInstanceId === renderInstanceId,
  ) ?? null;
  if (!node) return false;

  const runtime = runtimeFor(app);
  const binding = currentBinding(app, runtime);
  const currentNode = currentNavigationNode(app, node);
  if (!binding || !currentNode || !currentNode.capabilities.canSelect) return false;
  const target = canvasTargetFromNavigationNode(app, currentNode);
  void commitNavigationSelection(
    app,
    runtime,
    binding,
    currentNode,
    target,
    options.revealCode === true,
  );
  return true;
}

export function projectSelectionSnapshotOnCanvas(
  app: AppState,
  selection: SelectionSnapshot,
  options: { revealCode?: boolean } = {},
) {
  if (
    selection.resolution !== "resolved"
    || !selection.projections.layers.editorNodeId
  ) return false;
  const runtime = runtimeFor(app);
  const binding = currentBinding(app, runtime);
  if (
    !binding
    || !sameCanvasIdentity(binding.identity.canvas, selection.canvasIdentity)
  ) return false;
  const node = app.editorNavigationSnapshot?.nodes.find(
    (candidate) => candidate.id === selection.projections.layers.editorNodeId,
  ) ?? null;
  if (!node) return false;
  const target = canvasTargetFromNavigationNode(app, node);
  runtime.pendingInspections.clear();
  app.postPreviewMessage({
    type: "render-canvas-interaction-overlay",
    agentInstanceId: binding.identity.agentInstanceId,
    documentEpoch: binding.identity.documentEpoch,
    channel: "selection",
    targetKind: target.kind,
    editorNodeId: target.editorNodeId,
    actions: target.actions,
    selectionRevision: selection.selectionRevision,
    projection: canvasOverlayFromNavigationNode(node),
  });
  if (target.kind === "teraBoundary") return true;
  requestDomInspection(app, runtime, target, {
    selectionRevision: selection.selectionRevision,
    pointer: {
      clientX: 0,
      clientY: 0,
      button: "none",
      buttons: 0,
      modifiers: { alt: false, control: false, meta: false, shift: false },
    },
    openContextMenu: false,
    revealCode: options.revealCode === true,
  });
  return true;
}

async function commitNavigationSelection(
  app: AppState,
  runtime: CanvasInteractionFrontendRuntime,
  binding: CanvasInteractionBindingReceipt,
  node: EditorNavigationNode,
  target: CanvasInteractionTarget,
  revealCode: boolean,
) {
  const selectionSnapshot = await app.applySelectionIntent({
    kind: "selectEditorNode",
    editorNodeId: node.id,
  });
  if (
    !selectionSnapshot
    || selectionSnapshot.projections.layers.editorNodeId !== node.id
    || currentBinding(app, runtime) !== binding
  ) return;

  runtime.pendingInspections.clear();
  app.postPreviewMessage({
    type: "render-canvas-interaction-overlay",
    agentInstanceId: binding.identity.agentInstanceId,
    documentEpoch: binding.identity.documentEpoch,
    channel: "selection",
    targetKind: target.kind,
    editorNodeId: target.editorNodeId,
    actions: target.actions,
    selectionRevision: selectionSnapshot.selectionRevision,
    projection: canvasOverlayFromNavigationNode(node),
  });
  if (target.kind === "teraBoundary") return;
  requestDomInspection(app, runtime, target, {
    selectionRevision: selectionSnapshot.selectionRevision,
    pointer: {
      clientX: 0,
      clientY: 0,
      button: "none",
      buttons: 0,
      modifiers: { alt: false, control: false, meta: false, shift: false },
    },
    openContextMenu: false,
    revealCode,
  });
}

function contextMenuPosition(app: AppState, pointer: CanvasPointerSample) {
  const frameRect = app.previewFrame?.getBoundingClientRect();
  return {
    x: (frameRect?.left ?? 0) + pointer.clientX,
    y: (frameRect?.top ?? 0) + pointer.clientY,
  };
}

function openHtmlContextMenu(
  app: AppState,
  pointer: CanvasPointerSample,
) {
  const selection = app.coordinatedElementSelection;
  if (!selection) return;
  const observation = selection.observation;
  const position = contextMenuPosition(app, pointer);
  contextMenu.open({
    source: "preview",
    ...position,
    title: observation.selector || `<${observation.tag}>`,
    subtitle: observation.text,
    items: htmlElementContextMenuItems(
      app.editorRuntime,
      htmlTargetFromCoordinatedSelection(selection),
      "preview",
    ),
  });
}

function openTeraContextMenu(
  app: AppState,
  target: CanvasInteractionTarget,
  pointer: CanvasPointerSample,
) {
  if (!target.sourceNodeId) return;
  const position = contextMenuPosition(app, pointer);
  contextMenu.open({
    source: "preview",
    ...position,
    title: target.label,
    subtitle: target.file ?? target.sourceNodeId,
    items: teraContextMenuItems(
      app.editorRuntime,
      teraTargetFromBoundary({
        selector: null,
        sourceId: target.sourceNodeId,
        origin: target.origin === "theme" ? "theme" : "current",
        themeName: target.themeName,
        editorNodeId: target.editorNodeId,
        canEnterBoundary: target.actions.canEnterBoundary,
      }),
      "preview",
    ),
  });
}

function failCanvasInteractionBinding(
  app: AppState,
  runtime: CanvasInteractionFrontendRuntime,
  error: unknown,
) {
  const agentInstanceId = runtime.agentInstanceId;
  runtime.bindSerial += 1;
  clearActivationTimer(runtime);
  clearRuntimeInteractionState(runtime);
  runtime.phase = "failed";
  deactivateCanvasAgent(app, runtime, agentInstanceId);
  app.setGlobalStatus(
    `Canvas Interaction Rust a fost oprit după o eroare: ${errorMessage(error)}`,
    "error",
  );
}
