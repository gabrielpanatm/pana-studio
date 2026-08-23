import { sameCanvasProjectionIdentity } from "$lib/contracts/canvas-identity";
import { parseCanvasAgentMessage } from "$lib/preview/canvas-interaction";
import {
  acceptCanvasDomInspection,
  queueCanvasAgentAction,
  queueCanvasAgentGesture,
} from "$lib/state/canvas-interaction-gestures";
import type { CanvasInteractionControllerHost } from "$lib/state/canvas-interaction-host";
import {
  canvasInteractionRuntimeFor,
  clearCanvasActivationTimer,
  resetCanvasInteractionRuntime,
} from "$lib/state/canvas-interaction-runtime";
import { projectCurrentSelectionOverlay } from "$lib/state/canvas-interaction-selection";
import {
  canvasInteractionBindingKey,
  canvasInteractionSurfaceActive,
  currentCanvasInteractionBinding,
  deactivateCanvasAgent,
  synchronizeCanvasInteractionBinding,
} from "$lib/state/canvas-interaction-session";

export type { CanvasInteractionControllerHost } from "$lib/state/canvas-interaction-host";
export {
  hoverCanvasNavigationNode,
  projectSelectionSnapshotOnCanvas,
  selectCanvasNavigationNode,
  selectCanvasPreviewElement,
} from "$lib/state/canvas-interaction-selection";
export {
  retryCanvasInteractionBinding,
  synchronizeCanvasInteractionBinding,
} from "$lib/state/canvas-interaction-session";

export function handleCanvasAgentMessage(
  app: CanvasInteractionControllerHost,
  event: MessageEvent,
) {
  const raw = event.data as Record<string, unknown> | null;
  if (raw?.source !== "pana-studio-canvas-agent") return false;

  const runtime = canvasInteractionRuntimeFor(app);
  const ready = raw.type === "agentReady";
  const message = parseCanvasAgentMessage(
    app.session.previewFrame,
    event,
    ready ? null : runtime.agentInstanceId,
  );
  if (!message) return true;

  if (message.type === "agentReady") {
    if (runtime.agentInstanceId !== message.agentInstanceId) {
      const previousAgentInstanceId = runtime.agentInstanceId;
      runtime.bindSerial += 1;
      clearCanvasActivationTimer(runtime);
      deactivateCanvasAgent(app, runtime, previousAgentInstanceId);
      runtime.agentInstanceId = message.agentInstanceId;
      runtime.documentEpoch = 0;
      runtime.desiredBindingKey = null;
      runtime.phase = "dormant";
      resetCanvasInteractionRuntime(runtime);
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
      || runtime.desiredBindingKey !== canvasInteractionBindingKey(
        pendingBinding.identity.canvas,
        pendingBinding.identity.route,
        pendingBinding.identity.agentInstanceId,
        pendingBinding.activeDocumentPath,
      )
      || !sameCanvasProjectionIdentity(
        app.session.activeCanvasIdentity,
        pendingBinding.identity.canvas,
      )
    ) return true;
    clearCanvasActivationTimer(runtime);
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

  const binding = currentCanvasInteractionBinding(app, runtime);
  if (
    !binding
    || message.documentEpoch !== binding.identity.documentEpoch
    || message.agentInstanceId !== binding.identity.agentInstanceId
  ) return true;

  if (message.type === "domInspection") {
    acceptCanvasDomInspection(app, runtime, message);
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
      void app.commands.recordCanvasProjectionRuntimeEvent(
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
    queueCanvasAgentAction(app, runtime, message);
    return true;
  }

  if (message.gestureSequence <= runtime.lastObservedAgentSequence) return true;
  runtime.lastObservedAgentSequence = message.gestureSequence;
  queueCanvasAgentGesture(app, runtime, message);
  return true;
}
