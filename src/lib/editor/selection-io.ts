import { selectionResolution } from "$lib/kernel/selection-read-model";
import {
  type HoverSnapshot,
  SELECTION_COORDINATOR_SCHEMA_VERSION,
  type SelectionCoordinatorSnapshot,
  type SelectionIntent,
  type SelectionObservationInput,
  type SelectionObservationReceipt,
} from "$lib/editor/contracts";
import type { CanvasProjectionIdentity } from "$lib/contracts/canvas-projection";
import { sameCanvasProjectionIdentity } from "$lib/contracts/canvas-identity";
import { invoke } from "@tauri-apps/api/core";
import { schemaMismatch } from "$lib/contracts/io-schema";

export async function applySelectionIntent(
  identity: CanvasProjectionIdentity,
  route: string,
  activeDocumentPath: string | null,
  previewContextRenderInstanceId: string | null,
  intent: SelectionIntent,
  editScopeGrant: import("$lib/editor/contracts").EditScopeGrant | null = null,
): Promise<SelectionCoordinatorSnapshot> {
  const receipt = await invoke<SelectionCoordinatorSnapshot>(
    "apply_selection_intent",
    {
      input: {
        schemaVersion: SELECTION_COORDINATOR_SCHEMA_VERSION,
        identity,
        route,
        activeDocumentPath,
        previewContextRenderInstanceId,
        editScopeGrant,
        intent,
      },
    },
  );
  requireSelectionCoordinatorSnapshot(receipt, identity);
  return receipt;
}

export async function readSelectionSnapshot(
  identity: CanvasProjectionIdentity,
  route: string,
  activeDocumentPath: string | null,
  previewContextRenderInstanceId: string | null,
): Promise<SelectionCoordinatorSnapshot> {
  const receipt = await invoke<SelectionCoordinatorSnapshot>(
    "read_selection_snapshot",
    {
      input: {
        schemaVersion: SELECTION_COORDINATOR_SCHEMA_VERSION,
        identity,
        route,
        activeDocumentPath,
        previewContextRenderInstanceId,
      },
    },
  );
  requireSelectionCoordinatorSnapshot(receipt, identity);
  return receipt;
}

export async function acceptSelectionObservation(
  input: SelectionObservationInput,
): Promise<SelectionObservationReceipt> {
  const receipt = await invoke<SelectionObservationReceipt>(
    "accept_selection_observation",
    { input },
  );
  if (
    receipt.schemaVersion !== SELECTION_COORDINATOR_SCHEMA_VERSION
    || receipt.selectionRevision !== input.selectionRevision
    || receipt.documentEpoch !== input.documentEpoch
    || receipt.renderInstanceId !== input.renderInstanceId
    || !sameCanvasProjectionIdentity(receipt.canvasIdentity, input.canvasIdentity)
  ) {
    throw new Error("SelectionObservation nu aparține selecției solicitate.");
  }
  requireInspectorSelectionSummary(
    receipt.inspectorSummary,
    input.canvasIdentity,
    input.selectionRevision,
  );
  if (
    receipt.inspectorSummary.documentEpoch !== input.documentEpoch
    || receipt.inspectorSummary.renderInstanceId !== input.renderInstanceId
    || receipt.inspectorSummary.state !== "resolved"
  ) {
    throw new Error("InspectorSelectionSummary nu confirmă faptele fizice solicitate.");
  }
  return receipt;
}

function requireSelectionCoordinatorSnapshot(
  receipt: SelectionCoordinatorSnapshot,
  identity: CanvasProjectionIdentity,
) {
  if (
    receipt.schemaVersion !== SELECTION_COORDINATOR_SCHEMA_VERSION
    || receipt.selection.schemaVersion !== SELECTION_COORDINATOR_SCHEMA_VERSION
  ) {
    throw schemaMismatch(
      "SelectionCoordinator",
      receipt.schemaVersion,
      SELECTION_COORDINATOR_SCHEMA_VERSION,
    );
  }
  if (
    receipt.selection.projectRoot !== identity.projectRoot
    || receipt.selection.runtimeSessionId !== identity.runtimeSessionId
    || !sameCanvasProjectionIdentity(receipt.selection.canvasIdentity, identity)
    || !Number.isSafeInteger(receipt.selection.selectionRevision)
    || receipt.selection.selectionRevision <= 0
  ) {
    throw new Error("SelectionCoordinator a întors altă sesiune sau o revizie invalidă.");
  }
  const selection = receipt.selection;
  if (!Array.isArray(selection.members)) {
    throw new Error("SelectionCoordinator a întors un set de membri invalid.");
  }
  const memberIds = selection.members.map((member) => member.memberId);
  const memberIdSet = new Set(memberIds);
  const primary = selection.primaryMemberId
    ? selection.members.find((member) => member.memberId === selection.primaryMemberId) ?? null
    : null;
  const resolution = selectionResolution(selection);
  requireInspectorSelectionSummary(
    receipt.inspectorSummary,
    identity,
    selection.selectionRevision,
  );
  const expectedInspectorStates = {
    cleared: new Set(["empty"]),
    resolved: new Set(["resolving", "resolved", "uninspectable"]),
    notRendered: new Set(["notRendered"]),
    ambiguous: new Set(["ambiguous"]),
  } satisfies Record<
    ReturnType<typeof selectionResolution>,
    Set<string>
  >;
  if (
    selection.members.length > 256
    || memberIdSet.size !== memberIds.length
    || memberIds.some((memberId) => typeof memberId !== "string" || !memberId)
    || selection.aggregateCapabilities.memberCount !== selection.members.length
    || (selection.members.length > 0 && !primary)
    || (selection.members.length === 0 && selection.primaryMemberId !== null)
    || (
      selection.rangeOriginMemberId !== null
      && !memberIdSet.has(selection.rangeOriginMemberId)
    )
    || selection.members.some((member) => (
      !member.anchor
      || !member.subject
      || !member.capabilities
      || !Array.isArray(member.diagnostics)
    ))
    || (
      resolution === "resolved"
      && selection.members.some((member) => member.resolution !== "resolved")
    )
    || (
      resolution === "cleared"
      && selection.members.length > 0
    )
    || !expectedInspectorStates[resolution].has(receipt.inspectorSummary.state)
  ) {
    throw new Error("SelectionCoordinator a întors o proiecție semantică inconsistentă.");
  }
  if (receipt.hover) requireHoverSnapshot(receipt.hover, identity);
}

export function requireHoverSnapshot(
  hover: HoverSnapshot,
  identity: CanvasProjectionIdentity,
) {
  if (
    hover.schemaVersion !== SELECTION_COORDINATOR_SCHEMA_VERSION
    || !sameCanvasProjectionIdentity(hover.canvasIdentity, identity)
    || !Number.isSafeInteger(hover.hoverRevision)
    || hover.hoverRevision <= 0
    || !Number.isSafeInteger(hover.documentEpoch)
    || hover.documentEpoch <= 0
  ) {
    throw new Error("SelectionCoordinator a întors un HoverSnapshot invalid.");
  }
}

function requireInspectorSelectionSummary(
  summary: SelectionCoordinatorSnapshot["inspectorSummary"],
  identity: CanvasProjectionIdentity,
  selectionRevision: number,
) {
  const states = new Set([
    "empty",
    "resolving",
    "resolved",
    "notRendered",
    "ambiguous",
    "uninspectable",
  ]);
  const reasons = new Set([
    "noSelection",
    "awaitingPhysicalFacts",
    "selectionNotRendered",
    "selectionAmbiguous",
    "inspectionDisabled",
    "missingRenderInstance",
  ]);
  if (
    !summary
    || summary.schemaVersion !== SELECTION_COORDINATOR_SCHEMA_VERSION
    || summary.projectRoot !== identity.projectRoot
    || summary.runtimeSessionId !== identity.runtimeSessionId
    || summary.selectionRevision !== selectionRevision
    || !sameCanvasProjectionIdentity(summary.canvasIdentity, identity)
    || !states.has(summary.state)
    || (
      summary.documentEpoch !== null
      && (!Number.isSafeInteger(summary.documentEpoch) || summary.documentEpoch <= 0)
    )
    || !Array.isArray(summary.classes)
    || !Array.isArray(summary.diagnostics)
    || summary.classes.some((className) => (
      typeof className !== "string"
      || className.length === 0
      || /\s|[\u0000-\u001f\u007f]/u.test(className)
    ))
    || (
      summary.reason !== null
      && !reasons.has(summary.reason)
    )
    || summary.diagnostics.some((diagnostic) => (
      !diagnostic
      || !reasons.has(diagnostic.code)
      || typeof diagnostic.message !== "string"
      || diagnostic.message.length === 0
    ))
  ) {
    throw new Error("InspectorSelectionSummary a întors altă selecție sau o stare invalidă.");
  }
  if (
    (summary.state === "empty" && (
      summary.subjectKind !== null
      || summary.selector !== null
      || summary.classes.length > 0
    ))
    || (summary.state === "resolved" && summary.subjectKind === null)
    || (summary.state === "resolved" && summary.reason !== null)
    || (summary.state !== "resolved" && summary.reason === null)
    || (summary.reason === null && summary.diagnostics.length > 0)
    || (summary.reason !== null && summary.diagnostics[0]?.code !== summary.reason)
  ) {
    throw new Error("InspectorSelectionSummary conține o proiecție inconsistentă.");
  }
}
