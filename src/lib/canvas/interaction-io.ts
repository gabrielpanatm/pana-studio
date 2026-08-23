import {
  CANVAS_INTERACTION_SCHEMA_VERSION,
  type CanvasDragOverReceipt,
  type CanvasDragOverResolveInput,
  type CanvasHoverReceipt,
  type CanvasInteractionBindingReceipt,
  type CanvasInteractionIdentity,
  type CanvasInteractionReceipt,
  type CanvasInteractionResolveInput,
} from "$lib/canvas/contracts";
import { EDITOR_MOVE_PLAN_SCHEMA_VERSION } from "$lib/editor/contracts";
import {
  sameCanvasInteractionIdentity,
  sameCanvasProjectionIdentity,
} from "$lib/contracts/canvas-identity";
import { invoke } from "@tauri-apps/api/core";
import { schemaMismatch } from "$lib/contracts/io-schema";
import { requireEditorMoveLiveProjection } from "$lib/editor/navigation-io";
import { requireHoverSnapshot } from "$lib/editor/selection-io";

export async function bindCanvasInteractionAgent(
  identity: CanvasInteractionIdentity,
  activeDocumentPath: string | null,
  previewContextRenderInstanceId: string | null = null,
): Promise<CanvasInteractionBindingReceipt> {
  const receipt = await invoke<CanvasInteractionBindingReceipt>(
    "bind_canvas_interaction_agent",
    {
      input: {
        schemaVersion: CANVAS_INTERACTION_SCHEMA_VERSION,
        identity,
        activeDocumentPath,
        previewContextRenderInstanceId,
      },
    },
  );
  if (receipt.schemaVersion !== CANVAS_INTERACTION_SCHEMA_VERSION) {
    throw schemaMismatch(
      "CanvasInteractionBindingReceipt",
      receipt.schemaVersion,
      CANVAS_INTERACTION_SCHEMA_VERSION,
    );
  }
  if (
    !sameCanvasInteractionIdentity(receipt.identity, identity)
    || !Number.isSafeInteger(receipt.lastAcceptedSequence)
    || receipt.lastAcceptedSequence < 0
    || (
      receipt.activeDocumentPath !== null
      && typeof receipt.activeDocumentPath !== "string"
    )
    || !Array.isArray(receipt.authoringSurfaces)
    || receipt.authoringSurfaces.some((surface) => (
      !surface
      || typeof surface.sourceNodeId !== "string"
      || surface.sourceNodeId.length === 0
      || typeof surface.boundaryInstanceId !== "string"
      || surface.boundaryInstanceId.length === 0
      || (
        surface.renderInstanceId !== null
        && (
          typeof surface.renderInstanceId !== "string"
          || surface.renderInstanceId.length === 0
        )
      )
    ))
  ) {
    throw new Error("CanvasAgent a întors alt binding sau o secvență invalidă.");
  }
  return receipt;
}

export async function resolveCanvasInteractionIntent(
  input: CanvasInteractionResolveInput,
): Promise<CanvasInteractionReceipt> {
  const receipt = await invoke<CanvasInteractionReceipt>(
    "resolve_canvas_interaction_intent",
    { input },
  );
  requireCanvasInteractionReceipt(receipt, input);
  return receipt;
}

export async function resolveCanvasDragOverIntent(
  input: CanvasDragOverResolveInput,
): Promise<CanvasDragOverReceipt> {
  if (input.request.gesture !== "dragOver") {
    throw new Error("Canvas DragOver acceptă numai gesturi dragOver.");
  }
  const receipt = await invoke<CanvasDragOverReceipt>(
    "resolve_canvas_drag_over_intent",
    { input },
  );
  if (receipt.schemaVersion !== CANVAS_INTERACTION_SCHEMA_VERSION) {
    throw schemaMismatch(
      "CanvasDragOverReceipt",
      receipt.schemaVersion,
      CANVAS_INTERACTION_SCHEMA_VERSION,
    );
  }
  requireCanvasInteractionReceipt(receipt.interaction, {
    request: input.request,
    editScopeGrant: input.editScopeGrant,
  });
  const plan = receipt.plan;
  const target = receipt.interaction.target;
  const position = receipt.interaction.dragPosition;
  const accepted = receipt.interaction.status === "resolved" && target && position;
  if (Boolean(plan) !== Boolean(accepted)) {
    throw new Error("CanvasDragOverReceipt are un plan inconsistent cu ținta Rust.");
  }
  if (plan && target && position) {
    if (
      plan.schemaVersion !== EDITOR_MOVE_PLAN_SCHEMA_VERSION
      || plan.sourceNodeId !== input.sourceNodeId
      || plan.targetNodeId !== target.editorNodeId
      || plan.position !== position
      || !sameCanvasProjectionIdentity(plan.identity, input.request.identity.canvas)
      || plan.allowed !== Boolean(plan.token && plan.operation)
    ) {
      throw new Error("CanvasDragOverReceipt a întors alt plan semantic.");
    }
    requireEditorMoveLiveProjection(plan);
  }
  const timings = receipt.timings;
  if (
    !timings
    || !Number.isSafeInteger(timings.emittedAtMs)
    || timings.emittedAtMs < 0
    || !Number.isSafeInteger(timings.rustReceivedAtMs)
    || timings.rustReceivedAtMs < 0
    || !Number.isSafeInteger(timings.rustCompletedAtMs)
    || timings.rustCompletedAtMs < timings.rustReceivedAtMs
    || !Number.isSafeInteger(timings.inputToPlanDurationMs)
    || timings.inputToPlanDurationMs < 0
    || (
      plan?.allowed
        ? !Number.isSafeInteger(timings.inputToFirstAllowedPlanMs)
          || (timings.inputToFirstAllowedPlanMs ?? -1) < 0
          || timings.inputToFirstAllowedPlanMs !== timings.inputToPlanDurationMs
        : timings.inputToFirstAllowedPlanMs !== null
    )
    || !Number.isSafeInteger(timings.rustDurationMs)
    || timings.rustDurationMs < 0
  ) {
    throw new Error("CanvasDragOverReceipt are telemetrie Rust invalidă.");
  }
  return receipt;
}

export async function resolveCanvasHoverIntent(
  input: CanvasInteractionResolveInput,
): Promise<CanvasHoverReceipt> {
  if (input.request.gesture !== "pointerMove") {
    throw new Error("CanvasHover acceptă numai gesturi pointerMove.");
  }
  const receipt = await invoke<CanvasHoverReceipt>(
    "resolve_canvas_hover_intent",
    { input },
  );
  if (receipt.schemaVersion !== CANVAS_INTERACTION_SCHEMA_VERSION) {
    throw schemaMismatch(
      "CanvasHoverReceipt",
      receipt.schemaVersion,
      CANVAS_INTERACTION_SCHEMA_VERSION,
    );
  }
  requireCanvasInteractionReceipt(receipt.interaction, input);
  const accepted = receipt.interaction.status === "resolved"
    || receipt.interaction.status === "noTarget";
  if (accepted !== Boolean(receipt.projection)) {
    throw new Error("CanvasHoverReceipt are o proiecție semantică inconsistentă.");
  }
  if (receipt.projection) {
    if (typeof receipt.projection.changed !== "boolean") {
      throw new Error("CanvasHoverReceipt nu declară starea proiecției.");
    }
    const hover = receipt.projection.hover;
    if (hover) {
      requireHoverSnapshot(hover, input.request.identity.canvas);
    }
    const target = receipt.interaction.target;
    if (
      (target && (
        !hover
        || hover.editorNodeId !== target.editorNodeId
        || hover.documentEpoch !== input.request.identity.documentEpoch
      ))
      || (!target && hover)
    ) {
      throw new Error("CanvasHoverReceipt nu proiectează ținta Rust rezolvată.");
    }
  }
  const timings = receipt.timings;
  if (
    !timings
    || !Number.isSafeInteger(timings.emittedAtMs)
    || timings.emittedAtMs < 0
    || !Number.isSafeInteger(timings.rustReceivedAtMs)
    || timings.rustReceivedAtMs < 0
    || !Number.isSafeInteger(timings.rustCompletedAtMs)
    || timings.rustCompletedAtMs < timings.rustReceivedAtMs
    || !Number.isSafeInteger(timings.inputToProjectionDurationMs)
    || timings.inputToProjectionDurationMs < 0
    || !Number.isSafeInteger(timings.rustDurationMs)
    || timings.rustDurationMs < 0
  ) {
    throw new Error("CanvasHoverReceipt are telemetrie Rust invalidă.");
  }
  return receipt;
}

function requireCanvasInteractionReceipt(
  receipt: CanvasInteractionReceipt,
  input: CanvasInteractionResolveInput,
) {
  if (receipt.schemaVersion !== CANVAS_INTERACTION_SCHEMA_VERSION) {
    throw schemaMismatch(
      "CanvasInteractionReceipt",
      receipt.schemaVersion,
      CANVAS_INTERACTION_SCHEMA_VERSION,
    );
  }
  if (
    !sameCanvasInteractionIdentity(receipt.identity, input.request.identity)
    || receipt.gestureSequence !== input.request.gestureSequence
    || receipt.gesture !== input.request.gesture
  ) {
    throw new Error("CanvasInteractionReceipt nu aparține gestului solicitat.");
  }
  const expectedDragPosition = receipt.status === "resolved"
    && input.request.gesture === "dragOver"
    ? input.request.drag?.position ?? null
    : null;
  if (receipt.dragPosition !== expectedDragPosition) {
    throw new Error(
      "CanvasInteractionReceipt a întors o proiecție drag incompatibilă cu gestul.",
    );
  }
}
