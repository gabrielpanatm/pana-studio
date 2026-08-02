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
  resolveCanvasDragOverIntent,
  resolveCanvasHoverIntent,
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
  | "suspended"
  | "waitingNavigation"
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
  projectedGestureSequence: number | null;
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
  };
  runtimes.set(app, runtime);
  return runtime;
}

function canvasBindingKey(
  canvas: CanvasInteractionIdentity["canvas"],
  route: string,
  agentInstanceId: string,
  activeDocumentPath: string | null,
) {
  return JSON.stringify([
    canvas.projectRoot,
    canvas.runtimeSessionId,
    canvas.workspaceRevision,
    canvas.transactionId,
    canvas.previewRevision,
    route,
    agentInstanceId,
    normalizedProjectDocumentPath(activeDocumentPath),
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
  runtime.interactionGeneration += 1;
  runtime.pointerHoverGeneration += 1;
  runtime.pendingPointerMove = null;
  runtime.pendingDragOver = null;
  runtime.dragOverRunningGeneration = null;
  runtime.dragOverTail = Promise.resolve();
  runtime.binding = null;
  runtime.pendingBinding = null;
  runtime.pendingInspections.clear();
  runtime.dragSource = null;
  runtime.dragMovePreview = null;
  runtime.latestPointerMoveSequence = 0;
  runtime.latestDragOverSequence = 0;
  runtime.lastObservedAgentSequence = 0;
}

function suspendRuntimeInteractionState(runtime: CanvasInteractionFrontendRuntime) {
  runtime.interactionGeneration += 1;
  runtime.pointerHoverGeneration += 1;
  runtime.pendingPointerMove = null;
  runtime.pendingInspections.clear();
  runtime.dragSource = null;
  runtime.dragMovePreview = null;
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

function normalizedProjectDocumentPath(path: string | null | undefined) {
  return path
    ?.trim()
    .replaceAll("\\", "/")
    .replace(/^\/+/, "")
    .replace(/^(?:\.\/)+/, "")
    ?? "";
}

function sameProjectDocumentPath(
  left: string | null | undefined,
  right: string | null | undefined,
) {
  return normalizedProjectDocumentPath(left) === normalizedProjectDocumentPath(right);
}

function expectedCanvasDocumentPath(app: AppState) {
  const activePath = normalizedProjectDocumentPath(app.activeScannedPath);
  if (!activePath) return null;
  const activeFile = app.scannedProject?.files.find(
    (file) => normalizedProjectDocumentPath(file.relativePath) === activePath,
  );
  return activeFile?.role === "template"
    ? normalizedProjectDocumentPath(activeFile.relativePath)
    : null;
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

function confirmedCanvasNavigationBinding(
  app: AppState,
  canvas: CanvasInteractionIdentity["canvas"],
  route: string,
) {
  const activeDocumentPath = expectedCanvasDocumentPath(app);
  const snapshot = app.editorNavigationSnapshot;
  const ready = Boolean(
    snapshot
    && sameCanvasIdentity(snapshot.identity, canvas)
    && snapshot.route === route
    && sameProjectDocumentPath(
      snapshot.focusedView?.activeDocumentPath,
      activeDocumentPath,
    ),
  );
  return { activeDocumentPath, ready };
}

function currentBinding(
  app: AppState,
  runtime = runtimeFor(app),
) {
  const binding = runtime.binding;
  return canvasInteractionSurfaceActive(app)
    && runtime.phase === "active"
    && binding
    && runtime.agentInstanceId === binding.identity.agentInstanceId
    && runtime.documentEpoch === binding.identity.documentEpoch
    && sameCanvasIdentity(app.activeCanvasIdentity, binding.identity.canvas)
    && sameProjectDocumentPath(
      binding.activeDocumentPath,
      expectedCanvasDocumentPath(app),
    )
    ? binding
    : null;
}

function canvasInteractionSurfaceActive(app: AppState) {
  return app.applicationSurface === "workbench"
    && (app.workbenchSnapshot?.activeActivity ?? "editor") === "editor"
    && app.centerView !== "kernel";
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
        pendingBinding.activeDocumentPath,
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

  if (!canvasInteractionSurfaceActive(app)) {
    synchronizeCanvasInteractionBinding(app);
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
  if (message.type === "dragPreviewApplied") {
    const preview = runtime.dragMovePreview;
    if (
      preview
      && preview.sessionId === message.dragSessionId
      && preview.plan?.token === message.planToken
      && preview.projectedGestureSequence === message.gestureSequence
    ) {
      void app.recordCanvasProjectionRuntimeEvent(
        "canvas_drag_preview_applied",
        binding.identity.canvas,
        message.dragPreviewAppliedMs,
        null,
      );
    }
    return true;
  }
  if (message.type === "action") {
    if (message.actionSequence <= runtime.lastObservedAgentSequence) return true;
    runtime.lastObservedAgentSequence = message.actionSequence;
    const generation = runtime.interactionGeneration;
    runtime.gestureTail = runtime.gestureTail
      .then(async () => {
        if (generation !== runtime.interactionGeneration) return;
        if (message.action === "enterBoundary") {
          await enterBoundaryFromAgentAction(app, runtime, message, generation);
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
    runtime.pendingPointerMove = message;
    drainLatestPointerHover(app, runtime);
    return true;
  } else if (message.gesture === "dragOver") {
    runtime.latestDragOverSequence = message.gestureSequence;
    runtime.pendingDragOver = message;
    drainLatestCanvasDragOver(app, runtime);
    return true;
  }
  const generation = runtime.interactionGeneration;
  runtime.gestureTail = runtime.gestureTail
    .then(async () => {
      if (generation !== runtime.interactionGeneration) return;
      if (
        message.gesture === "dragOver"
        && message.gestureSequence !== runtime.latestDragOverSequence
      ) return;
      await resolveGesture(app, runtime, message, generation);
    })
    .catch((error) => {
      failCanvasInteractionBinding(app, runtime, error);
    });
  return true;
}

function drainLatestCanvasDragOver(
  app: AppState,
  runtime: CanvasInteractionFrontendRuntime,
) {
  const generation = runtime.interactionGeneration;
  if (runtime.dragOverRunningGeneration === generation) return;
  runtime.dragOverRunningGeneration = generation;

  const dragOverTask = (async () => {
    // DragStart rămâne un fapt ordonat. Primul DragOver îl așteaptă înainte
    // de a citi sursa fixată de recepția Rust.
    await runtime.gestureTail;
    while (generation === runtime.interactionGeneration) {
      const message = runtime.pendingDragOver;
      runtime.pendingDragOver = null;
      if (!message) return;
      if (message.gestureSequence !== runtime.latestDragOverSequence) continue;

      const binding = currentBinding(app, runtime);
      const source = runtime.dragSource;
      const sessionId = message.drag?.sessionId;
      if (
        !binding
        || !source
        || !sessionId
        || source.sessionId !== sessionId
        || message.documentEpoch !== binding.identity.documentEpoch
        || message.agentInstanceId !== binding.identity.agentInstanceId
      ) continue;

      const receipt = await resolveCanvasDragOverIntent({
        request: createCanvasInteractionRequest(binding.identity, message),
        sourceNodeId: source.target.editorNodeId,
        editScopeGrant: app.editorEditScopeGrant,
      });
      if (
        generation !== runtime.interactionGeneration
        || currentBinding(app, runtime) !== binding
        || message.gestureSequence !== runtime.latestDragOverSequence
        || runtime.dragSource !== source
        || runtime.dragSource?.sessionId !== sessionId
      ) continue;
      if (receipt.interaction.status === "stale") continue;
      if (receipt.interaction.status === "rejected") {
        throw new Error(
          receipt.interaction.diagnostics[0]?.message
            ?? "Recepția Canvas DragOver este invalidă.",
        );
      }
      projectResolvedCanvasDragOver(
        app,
        runtime,
        binding,
        source,
        sessionId,
        receipt.interaction,
        receipt.plan,
      );
    }
  })().catch((error) => {
    if (generation === runtime.interactionGeneration) {
      failCanvasInteractionBinding(app, runtime, error);
    }
  }).finally(() => {
    if (runtime.dragOverRunningGeneration === generation) {
      runtime.dragOverRunningGeneration = null;
    }
    if (
      runtime.pendingDragOver
      && runtime.dragOverRunningGeneration !== runtime.interactionGeneration
    ) {
      drainLatestCanvasDragOver(app, runtime);
    }
  });
  runtime.dragOverTail = dragOverTask;
}

function drainLatestPointerHover(
  app: AppState,
  runtime: CanvasInteractionFrontendRuntime,
) {
  const generation = runtime.pointerHoverGeneration;
  if (runtime.pointerHoverRunningGeneration === generation) return;
  runtime.pointerHoverRunningGeneration = generation;

  void (async () => {
    while (generation === runtime.pointerHoverGeneration) {
      const message = runtime.pendingPointerMove;
      runtime.pendingPointerMove = null;
      if (!message) return;
      if (message.gestureSequence !== runtime.latestPointerMoveSequence) continue;

      const binding = currentBinding(app, runtime);
      if (
        !binding
        || message.documentEpoch !== binding.identity.documentEpoch
        || message.agentInstanceId !== binding.identity.agentInstanceId
      ) continue;

      const projectionSerial = app.beginCanvasHoverProjection();
      const request = createCanvasInteractionRequest(binding.identity, message);
      const receipt = await resolveCanvasHoverIntent({
        request,
        editScopeGrant: app.editorEditScopeGrant,
      });
      if (
        generation !== runtime.pointerHoverGeneration
        || currentBinding(app, runtime) !== binding
        || message.gestureSequence !== runtime.latestPointerMoveSequence
      ) continue;
      if (receipt.interaction.status === "stale") continue;
      if (receipt.interaction.status === "rejected" || !receipt.projection) {
        throw new Error(
          receipt.interaction.diagnostics[0]?.message
            ?? "Recepția Canvas hover este invalidă.",
        );
      }
      if (!receipt.projection.changed) continue;
      if (!app.projectCanvasHoverReceipt(
        projectionSerial,
        receipt.interaction.identity.canvas,
        receipt.projection.hover,
      )) {
        continue;
      }
      renderReceiptOverlay(app, binding, "hover", receipt.interaction);
    }
  })().catch((error) => {
    if (generation === runtime.pointerHoverGeneration) {
      failCanvasInteractionBinding(app, runtime, error);
    }
  }).finally(() => {
    if (runtime.pointerHoverRunningGeneration === generation) {
      runtime.pointerHoverRunningGeneration = null;
    }
    if (
      runtime.pendingPointerMove
      && runtime.pointerHoverRunningGeneration !== runtime.pointerHoverGeneration
    ) {
      drainLatestPointerHover(app, runtime);
    }
  });
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
  const navigation = confirmedCanvasNavigationBinding(app, canvas, route);
  const desiredBindingKey = canvasBindingKey(
    canvas,
    route,
    agentInstanceId,
    navigation.activeDocumentPath,
  );
  if (!canvasInteractionSurfaceActive(app)) {
    const retainedBinding = runtime.binding ?? runtime.pendingBinding;
    if (
      runtime.desiredBindingKey === desiredBindingKey
      && retainedBinding
      && retainedBinding.identity.agentInstanceId === agentInstanceId
      && retainedBinding.identity.route === route
      && sameCanvasIdentity(retainedBinding.identity.canvas, canvas)
    ) {
      runtime.bindSerial += 1;
      clearActivationTimer(runtime);
      suspendRuntimeInteractionState(runtime);
      runtime.binding = retainedBinding;
      runtime.pendingBinding = null;
      runtime.phase = "suspended";
      deactivateCanvasAgent(app, runtime);
      return;
    }
    runtime.bindSerial += 1;
    clearActivationTimer(runtime);
    clearRuntimeInteractionState(runtime);
    runtime.desiredBindingKey = null;
    runtime.documentEpoch = 0;
    runtime.phase = "dormant";
    deactivateCanvasAgent(app, runtime);
    return;
  }

  if (
    runtime.phase === "suspended"
    && runtime.desiredBindingKey === desiredBindingKey
    && runtime.binding
    && runtime.binding.identity.agentInstanceId === agentInstanceId
    && runtime.binding.identity.route === route
    && sameCanvasIdentity(runtime.binding.identity.canvas, canvas)
  ) {
    reactivateRetainedCanvasAgent(app, runtime, runtime.binding);
    return;
  }

  if (runtime.desiredBindingKey !== desiredBindingKey) {
    runtime.bindSerial += 1;
    clearActivationTimer(runtime);
    clearRuntimeInteractionState(runtime);
    deactivateCanvasAgent(app, runtime);
    runtime.desiredBindingKey = desiredBindingKey;
    runtime.documentEpoch = nextDocumentEpoch(runtime);
    if (!navigation.ready) {
      runtime.phase = "waitingNavigation";
      return;
    }
    runtime.phase = "binding";
    void bindCurrentCanvasAgent(app);
    return;
  }
  if (currentBinding(app, runtime)) return;
  if (runtime.phase === "binding" || runtime.phase === "activating") return;
  if (!navigation.ready) {
    runtime.phase = "waitingNavigation";
    return;
  }
  runtime.phase = "binding";
  void bindCurrentCanvasAgent(app);
}

function reactivateRetainedCanvasAgent(
  app: AppState,
  runtime: CanvasInteractionFrontendRuntime,
  retainedBinding: CanvasInteractionBindingReceipt,
) {
  const serial = ++runtime.bindSerial;
  const lastAcceptedSequence = Math.max(
    retainedBinding.lastAcceptedSequence,
    runtime.lastObservedAgentSequence,
  );
  const binding = {
    ...retainedBinding,
    lastAcceptedSequence,
  };
  runtime.binding = null;
  runtime.pendingBinding = binding;
  runtime.phase = "activating";
  app.postPreviewMessage({
    type: "activate-canvas-interaction-agent",
    schemaVersion: binding.schemaVersion,
    agentInstanceId: binding.identity.agentInstanceId,
    documentEpoch: binding.identity.documentEpoch,
    lastAcceptedSequence,
    selection: true,
  });
  clearActivationTimer(runtime);
  runtime.activationTimer = window.setTimeout(() => {
    if (
      runtime.phase !== "activating"
      || runtime.pendingBinding !== binding
      || runtime.bindSerial !== serial
    ) return;
    failCanvasInteractionBinding(
      app,
      runtime,
      new Error("CanvasAgent nu a confirmat reactivarea lease-ului păstrat."),
    );
  }, CANVAS_AGENT_ACTIVATION_TIMEOUT_MS);
}

async function bindCurrentCanvasAgent(app: AppState) {
  const runtime = runtimeFor(app);
  const canvas = app.activeCanvasIdentity;
  const agentInstanceId = runtime.agentInstanceId;
  const documentEpoch = runtime.documentEpoch;
  if (!canvas || !agentInstanceId || documentEpoch <= 0) return;

  const route = previewRoute(app);
  const navigation = confirmedCanvasNavigationBinding(app, canvas, route);
  const desiredBindingKey = canvasBindingKey(
    canvas,
    route,
    agentInstanceId,
    navigation.activeDocumentPath,
  );
  if (runtime.desiredBindingKey !== desiredBindingKey) return;
  if (!navigation.ready) {
    runtime.phase = "waitingNavigation";
    return;
  }
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
      navigation.activeDocumentPath,
      app.coordinatedElementSelection?.renderInstanceId ?? null,
    );
    if (!sameProjectDocumentPath(receipt.activeDocumentPath, navigation.activeDocumentPath)) {
      throw new Error("CanvasAgent a întors binding-ul altui document activ.");
    }
    if (
      serial !== runtime.bindSerial
      || runtime.agentInstanceId !== agentInstanceId
      || runtime.documentEpoch !== documentEpoch
      || runtime.desiredBindingKey !== desiredBindingKey
      || !sameCanvasIdentity(app.activeCanvasIdentity, canvas)
      || !confirmedCanvasNavigationBinding(app, canvas, route).ready
      || !sameProjectDocumentPath(
        expectedCanvasDocumentPath(app),
        navigation.activeDocumentPath,
      )
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
  generation: number,
) {
  if (generation !== runtime.interactionGeneration) return;
  if (message.gesture === "pointerMove") return;
  const binding = currentBinding(app, runtime);
  if (!binding) return;
  const request = createCanvasInteractionRequest(binding.identity, message);
  const receipt = await resolveCanvasInteractionIntent({
    request,
    editScopeGrant: app.editorEditScopeGrant,
  });
  if (
    generation !== runtime.interactionGeneration
    || currentBinding(app, runtime) !== binding
  ) return;

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
    await resolveDragGesture(
      app,
      runtime,
      binding,
      message,
      receipt,
      generation,
    );
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
      if (generation !== runtime.interactionGeneration) return;
      app.postPreviewMessage({ type: "clear-canvas-interaction-overlays" });
    }
    return;
  }

  const selectionSnapshot = await app.applySelectionIntent({
    kind: "selectEditorNode",
    editorNodeId: receipt.target.editorNodeId,
  });
  if (
    generation !== runtime.interactionGeneration
    || !selectionSnapshot
    || selectionSnapshot.projections.layers.editorNodeId !== receipt.target.editorNodeId
  ) return;

  renderReceiptOverlay(
    app,
    binding,
    "selection",
    receipt,
    selectionSnapshot.selectionRevision,
  );
  if (
    receipt.target.kind === "teraBoundary"
    || receipt.target.kind === "markdownBoundary"
  ) {
    if (receipt.target.kind === "teraBoundary" && message.gesture === "contextMenu") {
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
  generation: number,
) {
  if (generation !== runtime.interactionGeneration) return;
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

  // Pointer-up can overtake the 6–14 ms Rust DragOver round-trip. Settle the
  // already-issued latest-wins plan first so the authorized DOM projection is
  // retained while the authoritative commit runs.
  let dragOverTail: Promise<void>;
  do {
    dragOverTail = runtime.dragOverTail;
    await dragOverTail;
  } while (runtime.dragOverTail !== dragOverTail);
  if (generation !== runtime.interactionGeneration || runtime.dragSource !== source) return;
  const movePreview = runtime.dragMovePreview;
  runtime.dragSource = null;
  clearReceiptOverlay(app, binding, "drag");
  // Drop confirmă ultimul fapt DragOver ordonat și tokenizat de Rust. Recepția
  // Drop păstrează un fallback pentru cazul fără plan settled, dar nu mută și
  // nu reinterpretează DOM-ul înainte de această confirmare.
  const settledPreview = movePreview
    && movePreview.sessionId === drag.sessionId
    && movePreview.sourceNodeId === source.target.editorNodeId
    ? movePreview
    : null;
  const targetNodeId = settledPreview?.targetNodeId
    ?? receipt.target?.editorNodeId
    ?? null;
  const position = settledPreview?.position ?? drag.position;
  if (!targetNodeId || !position || source.target.editorNodeId === targetNodeId) {
    retireCanvasDragMovePreview(runtime, movePreview);
    cancelCanvasDragDomPreview(app, binding, drag.sessionId);
    return;
  }
  let plan: EditorMovePlan | null = null;
  let planError = "";
  if (
    settledPreview
    && settledPreview.targetNodeId === targetNodeId
    && settledPreview.position === position
  ) {
    plan = settledPreview.promise
      ? await settledPreview.promise
      : settledPreview.plan;
    planError = settledPreview.error;
  } else {
    try {
      plan = await app.previewEditorNavigationMove(
        source.target.editorNodeId,
        targetNodeId,
        position,
      );
    } catch (error) {
      planError = errorMessage(error);
    }
  }
  if (!plan) {
    retireCanvasDragMovePreview(runtime, movePreview);
    cancelCanvasDragDomPreview(app, binding, drag.sessionId);
    if (generation !== runtime.interactionGeneration) return;
    if (planError) app.setGlobalStatus(planError, "error");
    return;
  }
  if (!plan.allowed) {
    retireCanvasDragMovePreview(runtime, movePreview);
    cancelCanvasDragDomPreview(app, binding, drag.sessionId);
    if (generation !== runtime.interactionGeneration) return;
    app.setGlobalStatus(
      plan.reason ?? t("editor-navigation-move-refused"),
      "error",
    );
    return;
  }
  if (generation !== runtime.interactionGeneration) {
    retireCanvasDragMovePreview(runtime, movePreview);
    return;
  }
  projectCanvasDropDomPreview(
    app,
    binding,
    drag.sessionId,
    message.gestureSequence,
    message.emittedAtMs,
    plan,
    settledPreview,
  );
  const outcome = await app.moveEditorNavigationNode(
    source.target.editorNodeId,
    targetNodeId,
    position,
    plan,
    message.emittedAtMs,
  );
  retireCanvasDragMovePreview(runtime, movePreview);
  if (outcome.status !== "committed") {
    cancelCanvasDragDomPreview(app, binding, drag.sessionId);
  }
  await retryCanvasInteractionBinding(app);
}

function projectResolvedCanvasDragOver(
  app: AppState,
  runtime: CanvasInteractionFrontendRuntime,
  binding: CanvasInteractionBindingReceipt,
  source: NonNullable<CanvasInteractionFrontendRuntime["dragSource"]>,
  sessionId: string,
  receipt: CanvasInteractionReceipt,
  plan: EditorMovePlan | null,
) {
  const target = receipt.target;
  const position = receipt.dragPosition;
  if (
    !target
    || !position
    || source.target.editorNodeId === target.editorNodeId
    || !plan
  ) {
    runtime.dragMovePreview = null;
    clearReceiptOverlay(app, binding, "drag");
    cancelCanvasDragDomPreview(app, binding, sessionId);
    return;
  }

  const preview: CanvasDragMovePreview = {
    sessionId,
    sourceNodeId: source.target.editorNodeId,
    targetNodeId: target.editorNodeId,
    position,
    receipt,
    pending: false,
    plan,
    error: "",
    promise: null,
    projectedGestureSequence: null,
  };
  runtime.dragMovePreview = preview;
  renderReceiptOverlay(
    app,
    binding,
    "drag",
    receipt,
    undefined,
    sessionId,
    canvasDragPermission(preview),
  );
}

function projectCanvasDropDomPreview(
  app: AppState,
  binding: CanvasInteractionBindingReceipt,
  sessionId: string,
  gestureSequence: number,
  inputEmittedAtMs: number,
  plan: EditorMovePlan,
  preview: CanvasDragMovePreview | null,
) {
  const liveProjection = plan.liveProjection;
  if (
    plan.allowed
    && liveProjection
    && liveProjection.planToken === plan.token
  ) {
    if (preview) preview.projectedGestureSequence = gestureSequence;
    app.postPreviewMessage({
      type: "project-canvas-drag-preview",
      agentInstanceId: binding.identity.agentInstanceId,
      documentEpoch: binding.identity.documentEpoch,
      dragSessionId: sessionId,
      gestureSequence,
      inputEmittedAtMs,
      projection: liveProjection,
    });
    return;
  }
  if (plan.allowed) {
    void app.recordCanvasProjectionRuntimeEvent(
      "canvas_drag_preview_skipped",
      binding.identity.canvas,
      Math.max(0, Date.now() - inputEmittedAtMs),
      plan.liveProjectionReason,
    );
  }
  cancelCanvasDragDomPreview(app, binding, sessionId);
}

function retireCanvasDragMovePreview(
  runtime: CanvasInteractionFrontendRuntime,
  preview: CanvasDragMovePreview | null,
) {
  if (runtime.dragMovePreview === preview) runtime.dragMovePreview = null;
}

function cancelCanvasDragDomPreview(
  app: AppState,
  binding: CanvasInteractionBindingReceipt,
  sessionId: string,
) {
  app.postPreviewMessage({
    type: "cancel-canvas-drag-preview",
    agentInstanceId: binding.identity.agentInstanceId,
    documentEpoch: binding.identity.documentEpoch,
    dragSessionId: sessionId,
  });
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
    projectedGestureSequence: null,
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
  generation: number,
) {
  if (generation !== runtime.interactionGeneration) return;
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
  if (
    generation !== runtime.interactionGeneration
    || currentBinding(app, runtime) !== binding
  ) return;
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
  if (app.gridOverlayEnabled) {
    app.postPreviewMessage({ type: "set-canvas-grid-overlay", enabled: true });
  }
  if (target.kind !== "teraBoundary" && target.kind !== "markdownBoundary") {
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
  if (target.kind === "teraBoundary" || target.kind === "markdownBoundary") return true;
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
  if (target.kind === "teraBoundary" || target.kind === "markdownBoundary") return;
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
