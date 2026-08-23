import {
  createCanvasInteractionRequest,
  type CanvasAgentActionMessage,
  type CanvasAgentGestureMessage,
} from "$lib/preview/canvas-interaction";
import {
  resolveCanvasHoverIntent,
  resolveCanvasInteractionIntent,
} from "$lib/canvas/interaction-io";
import {
  drainLatestCanvasDragOver,
  resolveCanvasDragGesture,
} from "$lib/state/canvas-interaction-drag";
import type { CanvasInteractionControllerHost } from "$lib/state/canvas-interaction-host";
import { renderCanvasInteractionOverlay } from "$lib/state/canvas-interaction-overlay";
import type {
  CanvasInteractionFrontendRuntime,
} from "$lib/state/canvas-interaction-runtime";
import {
  applyDomInspection,
  deleteSelectionFromAgentAction,
  enterBoundaryFromAgentAction,
  openTeraContextMenu,
  projectSelectionSnapshotOnCanvas,
} from "$lib/state/canvas-interaction-selection";
import {
  currentCanvasInteractionBinding,
  failCanvasInteractionBinding,
} from "$lib/state/canvas-interaction-session";

export function queueCanvasAgentAction(
  app: CanvasInteractionControllerHost,
  runtime: CanvasInteractionFrontendRuntime,
  message: CanvasAgentActionMessage,
) {
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
      if (generation === runtime.interactionGeneration) {
        failCanvasInteractionBinding(app, runtime, error);
      }
    });
}

export function queueCanvasAgentGesture(
  app: CanvasInteractionControllerHost,
  runtime: CanvasInteractionFrontendRuntime,
  message: CanvasAgentGestureMessage,
) {
  if (message.gesture === "pointerMove") {
    runtime.latestPointerMoveSequence = message.gestureSequence;
    runtime.pendingPointerMove = message;
    drainLatestCanvasPointerHover(app, runtime);
    return;
  }
  if (message.gesture === "dragOver") {
    runtime.latestDragOverSequence = message.gestureSequence;
    runtime.pendingDragOver = message;
    drainLatestCanvasDragOver(app, runtime);
    return;
  }

  const generation = runtime.interactionGeneration;
  runtime.gestureTail = runtime.gestureTail
    .then(async () => {
      if (generation !== runtime.interactionGeneration) return;
      await resolveCanvasGesture(app, runtime, message, generation);
    })
    .catch((error) => {
      if (generation === runtime.interactionGeneration) {
        failCanvasInteractionBinding(app, runtime, error);
      }
    });
}

export function acceptCanvasDomInspection(
  app: CanvasInteractionControllerHost,
  runtime: CanvasInteractionFrontendRuntime,
  message: Parameters<typeof applyDomInspection>[3],
) {
  const binding = currentCanvasInteractionBinding(app, runtime);
  if (!binding) return;
  void applyDomInspection(
    app,
    runtime,
    binding,
    message,
    runtime.interactionGeneration,
  );
}

function drainLatestCanvasPointerHover(
  app: CanvasInteractionControllerHost,
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

      const binding = currentCanvasInteractionBinding(app, runtime);
      if (
        !binding
        || message.documentEpoch !== binding.identity.documentEpoch
        || message.agentInstanceId !== binding.identity.agentInstanceId
      ) continue;

      const projectionSerial = app.selection.editorSelection.beginCanvasHoverProjection();
      const request = createCanvasInteractionRequest(binding.identity, message);
      const receipt = await resolveCanvasHoverIntent({
        request,
        editScopeGrant: app.selection.editorSelection.editScopeGrant,
      });
      if (
        generation !== runtime.pointerHoverGeneration
        || currentCanvasInteractionBinding(app, runtime) !== binding
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
      if (!app.selection.editorSelection.projectCanvasHoverReceipt(
        projectionSerial,
        receipt.interaction.identity.canvas,
        receipt.projection.hover,
      )) continue;
      renderCanvasInteractionOverlay(app, binding, "hover", receipt.interaction);
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
      drainLatestCanvasPointerHover(app, runtime);
    }
  });
}

async function resolveCanvasGesture(
  app: CanvasInteractionControllerHost,
  runtime: CanvasInteractionFrontendRuntime,
  message: CanvasAgentGestureMessage,
  generation: number,
) {
  if (generation !== runtime.interactionGeneration) return;
  const binding = currentCanvasInteractionBinding(app, runtime);
  if (!binding) return;
  const request = createCanvasInteractionRequest(binding.identity, message);
  const receipt = await resolveCanvasInteractionIntent({
    request,
    editScopeGrant: app.selection.editorSelection.editScopeGrant,
  });
  if (
    generation !== runtime.interactionGeneration
    || currentCanvasInteractionBinding(app, runtime) !== binding
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
    || message.gesture === "drop"
  ) {
    await resolveCanvasDragGesture(app, runtime, binding, message, receipt, generation);
    return;
  }
  if (message.gesture === "pointerDown") {
    app.commands.closeContextMenu();
    return;
  }
  if (message.gesture !== "click" && message.gesture !== "contextMenu") return;

  if (!receipt.target || !receipt.overlay) {
    if (message.gesture === "click") {
      runtime.pendingInspections.clear();
      await app.selection.editorSelection.applySelectionIntent({ kind: "clearSelection" });
      if (generation !== runtime.interactionGeneration) return;
      app.commands.postPreviewMessage({ type: "clear-canvas-interaction-overlays" });
    }
    return;
  }

  const targetAlreadySelected = app.selection.editorSelection.selectionSnapshot?.members.some(
    (member) => member.anchor.editorNodeId === receipt.target?.editorNodeId,
  ) === true;
  const intent = message.gesture === "contextMenu" && targetAlreadySelected
    ? {
        kind: "setPrimaryEditorNode" as const,
        editorNodeId: receipt.target.editorNodeId,
      }
    : message.pointer.modifiers.shift
      ? {
          kind: "extendRangeToEditorNode" as const,
          editorNodeId: receipt.target.editorNodeId,
        }
      : message.pointer.modifiers.control || message.pointer.modifiers.meta
        ? {
            kind: "toggleEditorNode" as const,
            editorNodeId: receipt.target.editorNodeId,
          }
        : {
            kind: "selectEditorNode" as const,
            editorNodeId: receipt.target.editorNodeId,
          };
  const selectionSnapshot = await app.selection.editorSelection.applySelectionIntent(intent);
  if (
    generation !== runtime.interactionGeneration
    || currentCanvasInteractionBinding(app, runtime) !== binding
    || !selectionSnapshot
  ) return;

  projectSelectionSnapshotOnCanvas(app, selectionSnapshot, {
    pointer: message.pointer,
    openContextMenu: message.gesture === "contextMenu",
  });
  if (selectionSnapshot.primaryMemberId !== receipt.target.editorNodeId) return;
  if (receipt.target.kind === "boundary") {
    if (receipt.target.boundaryKind !== "markdown" && message.gesture === "contextMenu") {
      openTeraContextMenu(app, receipt.target, message.pointer);
    }
  }
}
