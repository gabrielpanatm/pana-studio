import { t } from "$lib/i18n/runtime.svelte";
import {
  createCanvasInteractionRequest,
  type CanvasAgentGestureMessage,
} from "$lib/preview/canvas-interaction";
import {
  resolveCanvasDragOverIntent,
} from "$lib/canvas/interaction-io";
import type { CanvasInteractionControllerHost } from "$lib/state/canvas-interaction-host";
import {
  clearCanvasInteractionOverlay,
  renderCanvasInteractionOverlay,
} from "$lib/state/canvas-interaction-overlay";
import {
  type CanvasDragMovePreview,
  type CanvasInteractionFrontendRuntime,
} from "$lib/state/canvas-interaction-runtime";
import {
  currentCanvasInteractionBinding,
  failCanvasInteractionBinding,
  retryCanvasInteractionBinding,
} from "$lib/state/canvas-interaction-session";
import type {
  CanvasInteractionBindingReceipt,
  CanvasInteractionReceipt,
} from "$lib/canvas/contracts";
import type { EditorMovePlan } from "$lib/editor/contracts";
import { errorMessage } from "$lib/util";

export function drainLatestCanvasDragOver(
  app: CanvasInteractionControllerHost,
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

      const binding = currentCanvasInteractionBinding(app, runtime);
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
        editScopeGrant: app.selection.editorSelection.editScopeGrant,
      });
      if (
        generation !== runtime.interactionGeneration
        || currentCanvasInteractionBinding(app, runtime) !== binding
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

export async function resolveCanvasDragGesture(
  app: CanvasInteractionControllerHost,
  runtime: CanvasInteractionFrontendRuntime,
  binding: CanvasInteractionBindingReceipt,
  message: CanvasAgentGestureMessage,
  receipt: CanvasInteractionReceipt,
  generation: number,
) {
  if (generation !== runtime.interactionGeneration) return;
  const drag = message.drag;
  if (
    !drag
    || (message.gesture !== "dragStart" && message.gesture !== "drop")
  ) return;
  if (message.gesture === "dragStart") {
    clearCanvasInteractionOverlay(app, binding, "drag");
    clearCanvasInteractionOverlay(app, binding, "hover");
    runtime.dragMovePreview = null;
    const target = receipt.target;
    runtime.dragSource = target
      && (target.actions.canMove || target.actions.canMoveAtomic)
      ? { sessionId: drag.sessionId, target }
      : null;
    if (runtime.dragSource) {
      renderCanvasInteractionOverlay(app, binding, "selection", receipt);
    }
    return;
  }

  const source = runtime.dragSource;
  if (!source || source.sessionId !== drag.sessionId) {
    clearCanvasInteractionOverlay(app, binding, "drag");
    return;
  }
  let dragOverTail: Promise<void>;
  do {
    dragOverTail = runtime.dragOverTail;
    await dragOverTail;
  } while (runtime.dragOverTail !== dragOverTail);
  if (
    generation !== runtime.interactionGeneration
    || currentCanvasInteractionBinding(app, runtime) !== binding
    || runtime.dragSource !== source
  ) return;
  const movePreview = runtime.dragMovePreview;
  runtime.dragSource = null;
  clearCanvasInteractionOverlay(app, binding, "drag");
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
    plan = settledPreview.plan;
  } else {
    try {
      plan = await app.commands.previewEditorNavigationMove(
        source.target.editorNodeId,
        targetNodeId,
        position,
      );
    } catch (error) {
      planError = errorMessage(error);
    }
  }
  if (
    generation !== runtime.interactionGeneration
    || currentCanvasInteractionBinding(app, runtime) !== binding
  ) {
    retireCanvasDragMovePreview(runtime, movePreview);
    return;
  }
  if (!plan) {
    retireCanvasDragMovePreview(runtime, movePreview);
    cancelCanvasDragDomPreview(app, binding, drag.sessionId);
    if (generation !== runtime.interactionGeneration) return;
    if (planError) app.commands.setGlobalStatus(planError, "error");
    return;
  }
  if (!plan.allowed) {
    retireCanvasDragMovePreview(runtime, movePreview);
    cancelCanvasDragDomPreview(app, binding, drag.sessionId);
    if (generation !== runtime.interactionGeneration) return;
    app.commands.setGlobalStatus(
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
  const outcome = await app.commands.moveEditorNavigationNode(
    source.target.editorNodeId,
    targetNodeId,
    position,
    plan,
    message.emittedAtMs,
  );
  if (
    generation !== runtime.interactionGeneration
    || currentCanvasInteractionBinding(app, runtime) !== binding
  ) {
    retireCanvasDragMovePreview(runtime, movePreview);
    return;
  }
  retireCanvasDragMovePreview(runtime, movePreview);
  if (outcome.status !== "committed") {
    cancelCanvasDragDomPreview(app, binding, drag.sessionId);
  }
  await retryCanvasInteractionBinding(app);
}

function projectResolvedCanvasDragOver(
  app: CanvasInteractionControllerHost,
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
    clearCanvasInteractionOverlay(app, binding, "drag");
    cancelCanvasDragDomPreview(app, binding, sessionId);
    return;
  }

  const preview: CanvasDragMovePreview = {
    sessionId,
    sourceNodeId: source.target.editorNodeId,
    targetNodeId: target.editorNodeId,
    position,
    plan,
    projectedGestureSequence: null,
  };
  runtime.dragMovePreview = preview;
  renderCanvasInteractionOverlay(
    app,
    binding,
    "drag",
    receipt,
    undefined,
    sessionId,
    { state: plan.allowed ? "allowed" : "blocked" },
  );
}

function projectCanvasDropDomPreview(
  app: CanvasInteractionControllerHost,
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
    app.commands.postPreviewMessage({
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
    void app.commands.recordCanvasProjectionRuntimeEvent(
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
  app: CanvasInteractionControllerHost,
  binding: CanvasInteractionBindingReceipt,
  sessionId: string,
) {
  app.commands.postPreviewMessage({
    type: "cancel-canvas-drag-preview",
    agentInstanceId: binding.identity.agentInstanceId,
    documentEpoch: binding.identity.documentEpoch,
    dragSessionId: sessionId,
  });
}
