import type { CanvasInteractionControllerHost } from "$lib/state/canvas-interaction-host";
import type {
  CanvasInteractionBindingReceipt,
  CanvasInteractionReceipt,
} from "$lib/canvas/contracts";

export type CanvasDragPermission = {
  state: "pending" | "allowed" | "blocked";
};

export function renderCanvasInteractionOverlay(
  app: CanvasInteractionControllerHost,
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
    if (channel !== "selection") clearCanvasInteractionOverlay(app, binding, channel);
    return;
  }
  app.commands.postPreviewMessage({
    type: "render-canvas-interaction-overlay",
    agentInstanceId: binding.identity.agentInstanceId,
    documentEpoch: binding.identity.documentEpoch,
    channel,
    targetKind: receipt.target.kind,
    boundaryKind: receipt.target.boundaryKind,
    componentKind: receipt.target.componentKind,
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

export function clearCanvasInteractionOverlay(
  app: CanvasInteractionControllerHost,
  binding: CanvasInteractionBindingReceipt,
  channel: "hover" | "drag",
) {
  app.commands.postPreviewMessage({
    type: "render-canvas-interaction-overlay",
    agentInstanceId: binding.identity.agentInstanceId,
    documentEpoch: binding.identity.documentEpoch,
    channel,
    targetKind: null,
    projection: { renderInstanceIds: [] },
  });
}
