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
  primarySelectionEditorNodeId,
  primarySelectionRenderInstanceId,
  selectionResolution,
} from "$lib/kernel/selection-read-model";
import {
  CANVAS_AGENT_MESSAGE_SCHEMA_VERSION,
  type CanvasAgentActionMessage,
  type CanvasAgentDomInspectionMessage,
} from "$lib/preview/canvas-interaction";
import {
  requestEditorEditScope,
} from "$lib/editor/navigation-io";
import { sameCanvasProjectionIdentity } from "$lib/contracts/canvas-identity";
import type { CanvasInteractionControllerHost } from "$lib/state/canvas-interaction-host";
import {
  canvasInteractionRuntimeFor,
  canvasNavigationNodeIndex,
  type CanvasInteractionFrontendRuntime,
  type PendingCanvasInspection,
} from "$lib/state/canvas-interaction-runtime";
import { currentCanvasInteractionBinding } from "$lib/state/canvas-interaction-session";
import type {
  CanvasInteractionBindingReceipt,
  CanvasInteractionTarget,
  CanvasOverlayProjection,
  CanvasPointerSample,
} from "$lib/canvas/contracts";
import type { EditorNavigationNode } from "$lib/editor/contracts";
import {
  SELECTION_COORDINATOR_SCHEMA_VERSION,
  type SelectionSnapshot,
} from "$lib/editor/contracts";

function requestDomInspection(
  app: CanvasInteractionControllerHost,
  runtime: CanvasInteractionFrontendRuntime,
  target: CanvasInteractionTarget,
  pending: Omit<PendingCanvasInspection, "target">,
) {
  const binding = currentCanvasInteractionBinding(app, runtime);
  if (!binding || !target.renderInstanceId || !target.actions.canInspect) return;
  runtime.nextInspectionSerial += 1;
  const inspectionRequestId = `inspection:${runtime.documentEpoch}:${runtime.nextInspectionSerial}`;
  runtime.pendingInspections.set(inspectionRequestId, { ...pending, target });
  while (runtime.pendingInspections.size > 32) {
    const oldest = runtime.pendingInspections.keys().next().value;
    if (typeof oldest !== "string") break;
    runtime.pendingInspections.delete(oldest);
  }
  app.commands.postPreviewMessage({
    type: "inspect-canvas-interaction-target",
    schemaVersion: CANVAS_AGENT_MESSAGE_SCHEMA_VERSION,
    agentInstanceId: binding.identity.agentInstanceId,
    documentEpoch: binding.identity.documentEpoch,
    inspectionRequestId,
    renderInstanceId: target.renderInstanceId,
  });
}

export async function enterBoundaryFromAgentAction(
  app: CanvasInteractionControllerHost,
  runtime: CanvasInteractionFrontendRuntime,
  message: CanvasAgentActionMessage,
  generation: number,
) {
  if (generation !== runtime.interactionGeneration) return;
  const binding = currentCanvasInteractionBinding(app, runtime);
  const coordinated = coordinatedActionTarget(app, binding, message);
  const target = coordinated?.target;
  const activeDocumentPath = binding?.activeDocumentPath ?? null;
  if (
    !binding
    || !target
    || message.action !== "enterBoundary"
    || target.kind !== "boundary"
    || target.boundaryKind === "markdown"
    || !target.actions.canEnterBoundary
    || !target.requiredEditScopeId
    || !activeDocumentPath
  ) return;

  const grant = await requestEditorEditScope(
    binding.identity.canvas,
    binding.identity.route,
    activeDocumentPath,
    target.requiredEditScopeId,
    primarySelectionRenderInstanceId(coordinated.selection),
  );
  if (
    generation !== runtime.interactionGeneration
    || currentCanvasInteractionBinding(app, runtime) !== binding
  ) return;
  app.selection.editorSelection.editScopeGrant = grant;
  app.selection.editorSelection.editScopeId = grant.scopeId;
  app.commands.setGlobalStatus(
    target.effectScope === "sharedDefinition"
      ? "Scope-ul partajat a fost autorizat de kernel."
      : "Conținutul boundary-ului poate fi selectat și editat.",
    "idle",
  );
}

export async function deleteSelectionFromAgentAction(
  app: CanvasInteractionControllerHost,
  runtime: CanvasInteractionFrontendRuntime,
  message: CanvasAgentActionMessage,
) {
  const binding = currentCanvasInteractionBinding(app, runtime);
  const coordinated = coordinatedActionTarget(app, binding, message);
  const target = coordinated?.target;
  if (
    !binding
    || !target
    || message.action !== "deleteSelection"
    || target.actions.readOnly
  ) return;

  if (target.kind === "htmlElement") {
    const selection = app.selection.coordinatedElementSelection;
    if (
      !selection
      || !target.renderInstanceId
      || selection.renderInstanceId !== target.renderInstanceId
    ) return;
    await app.runtime.editorRuntime.dispatch({
      type: "delete-html",
      surface: "shortcut",
      target: htmlTargetFromCoordinatedSelection(selection),
    });
    return;
  }

  if (
    target.kind !== "boundary"
    || target.boundaryKind === "markdown"
    || !target.sourceNodeId
  ) return;
  const sourceNode = app.selection.sourceGraph?.nodes.find(
    (node) => node.id === target.sourceNodeId,
  ) ?? null;
  await app.runtime.editorRuntime.dispatch({
    type: "delete-tera",
    surface: "shortcut",
    target: teraTargetFromBoundary({
      sourceId: target.sourceNodeId,
      renderInstanceId: primarySelectionRenderInstanceId(coordinated.selection),
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
  app: CanvasInteractionControllerHost,
  binding: CanvasInteractionBindingReceipt | null,
  message: CanvasAgentActionMessage,
) {
  const selection = app.selection.editorSelection.selectionSnapshot;
  if (
    !binding
    || !selection
    || selectionResolution(selection) !== "resolved"
    || selection.selectionRevision !== message.selectionRevision
    || primarySelectionEditorNodeId(selection) !== message.editorNodeId
    || !sameCanvasProjectionIdentity(selection.canvasIdentity, binding.identity.canvas)
  ) return null;
  const node = canvasNavigationNodeIndex(app).byId.get(message.editorNodeId) ?? null;
  if (!node) return null;
  return {
    selection,
    target: canvasTargetFromNavigationNode(app, node),
  };
}

export async function applyDomInspection(
  app: CanvasInteractionControllerHost,
  runtime: CanvasInteractionFrontendRuntime,
  binding: CanvasInteractionBindingReceipt,
  message: CanvasAgentDomInspectionMessage,
  generation: number,
) {
  if (
    generation !== runtime.interactionGeneration
    || currentCanvasInteractionBinding(app, runtime) !== binding
  ) return;
  const pending = runtime.pendingInspections.get(message.inspectionRequestId);
  runtime.pendingInspections.delete(message.inspectionRequestId);
  const target = pending?.target;
  const selection = app.selection.editorSelection.selectionSnapshot;
  if (
    !pending
    || !target
    || !selection
    || target.renderInstanceId !== message.renderInstanceId
    || (target.tag && target.tag.toLowerCase() !== message.observation.tag)
    || selection.selectionRevision !== pending.selectionRevision
    || !sameCanvasProjectionIdentity(selection.canvasIdentity, binding.identity.canvas)
  ) return;

  const observation = message.observation;
  const accepted = await app.selection.editorSelection.acceptObservation({
    schemaVersion: SELECTION_COORDINATOR_SCHEMA_VERSION,
    selectionRevision: pending.selectionRevision,
    canvasIdentity: selection.canvasIdentity,
    documentEpoch: message.documentEpoch,
    renderInstanceId: message.renderInstanceId,
    inspectorFacts: {
      observedTag: observation.tag,
      elementId: observation.id,
      classes: observation.classes,
      blockContext: observation.blockContext
        ? {
            providerId: observation.blockContext.providerId,
            rootTag: observation.blockContext.rootTag,
          }
        : null,
    },
  }, observation);
  if (
    !accepted
    || generation !== runtime.interactionGeneration
    || currentCanvasInteractionBinding(app, runtime) !== binding
    || app.selection.editorSelection.selectionSnapshot?.selectionRevision !== pending.selectionRevision
  ) return;
  app.commands.syncCodeSelectionHighlight(pending.revealCode);
  if (pending.openContextMenu) {
    openHtmlContextMenu(app, pending.pointer);
  }
}

function canvasTargetFromNavigationNode(
  app: CanvasInteractionControllerHost,
  node: EditorNavigationNode,
): CanvasInteractionTarget {
  const requiredEditScopeId = node.capabilities.requiresEditScopeId;
  return {
    editorNodeId: node.id,
    kind: node.kind,
    boundaryKind: node.boundary?.kind ?? null,
    componentKind: node.boundary?.componentKind ?? null,
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
      : app.selection.editorSelection.editScopeGrant?.scopeId === requiredEditScopeId
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

export function projectCurrentSelectionOverlay(
  app: CanvasInteractionControllerHost,
  binding: CanvasInteractionBindingReceipt,
) {
  const selection = app.selection.editorSelection.selectionSnapshot;
  if (
    !selection
    || !sameCanvasProjectionIdentity(selection.canvasIdentity, binding.identity.canvas)
  ) return;
  projectSelectionSnapshotOnCanvas(app, selection);
  if (app.runtime.gridOverlayEnabled) {
    app.commands.postPreviewMessage({ type: "set-canvas-grid-overlay", enabled: true });
  }
}

function currentNavigationNode(
  app: CanvasInteractionControllerHost,
  requested: EditorNavigationNode,
) {
  const runtime = canvasInteractionRuntimeFor(app);
  const binding = currentCanvasInteractionBinding(app, runtime);
  const index = canvasNavigationNodeIndex(app, runtime);
  const snapshot = index.snapshot;
  if (
    !binding
    || !snapshot
    || !sameCanvasProjectionIdentity(snapshot.identity, binding.identity.canvas)
  ) return null;
  return index.byId.get(requested.id) ?? null;
}

export function hoverCanvasNavigationNode(
  app: CanvasInteractionControllerHost,
  requested: EditorNavigationNode | null,
) {
  const runtime = canvasInteractionRuntimeFor(app);
  const binding = currentCanvasInteractionBinding(app, runtime);
  if (!binding) return;
  if (!requested) {
    void app.selection.editorSelection.applyHoverIntent({
      kind: "clearHover",
      documentEpoch: binding.identity.documentEpoch,
    });
    app.commands.postPreviewMessage({
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
  void app.selection.editorSelection.applyHoverIntent({
    kind: "setHover",
    editorNodeId: node.id,
    documentEpoch: binding.identity.documentEpoch,
  }).then((hover) => {
    if (
      !hover
      || currentCanvasInteractionBinding(app, runtime) !== binding
      || hover.documentEpoch !== binding.identity.documentEpoch
    ) return;
    const effectiveNode = canvasNavigationNodeIndex(app, runtime).byId.get(hover.editorNodeId)
      ?? null;
    if (!effectiveNode) return;
    const target = canvasTargetFromNavigationNode(app, effectiveNode);
    app.commands.postPreviewMessage({
      type: "render-canvas-interaction-overlay",
      agentInstanceId: binding.identity.agentInstanceId,
      documentEpoch: binding.identity.documentEpoch,
      channel: "hover",
      targetKind: target.kind,
      boundaryKind: target.boundaryKind,
      componentKind: target.componentKind,
      editorNodeId: target.editorNodeId,
      actions: target.actions,
      projection: canvasOverlayFromNavigationNode(effectiveNode),
    });
  });
}

export async function selectCanvasNavigationNode(
  app: CanvasInteractionControllerHost,
  requested: EditorNavigationNode,
  options: {
    toggle?: boolean;
    extendRange?: boolean;
    setPrimary?: boolean;
    revealCode?: boolean;
  } = {},
): Promise<SelectionSnapshot | null> {
  const runtime = canvasInteractionRuntimeFor(app);
  const binding = currentCanvasInteractionBinding(app, runtime);
  const node = currentNavigationNode(app, requested);
  if (!binding || !node || !node.capabilities.canSelect) return null;
  return await commitNavigationSelection(app, runtime, binding, node, options);
}

export function selectCanvasPreviewElement(
  app: CanvasInteractionControllerHost,
  element: Element,
  options: { revealCode?: boolean } = {},
) {
  const renderInstanceId = element.getAttribute("data-pana-render-instance-id");
  if (!renderInstanceId) return false;
  const runtime = canvasInteractionRuntimeFor(app);
  const node = canvasNavigationNodeIndex(app, runtime).byRenderInstanceId.get(renderInstanceId)
    ?? null;
  if (!node) return false;

  const binding = currentCanvasInteractionBinding(app, runtime);
  const currentNode = currentNavigationNode(app, node);
  if (!binding || !currentNode || !currentNode.capabilities.canSelect) return false;
  void commitNavigationSelection(
    app,
    runtime,
    binding,
    currentNode,
    { revealCode: options.revealCode === true },
  );
  return true;
}

export function projectSelectionSnapshotOnCanvas(
  app: CanvasInteractionControllerHost,
  selection: SelectionSnapshot,
  options: {
    revealCode?: boolean;
    pointer?: CanvasPointerSample;
    openContextMenu?: boolean;
  } = {},
) {
  if (selection.members.length === 0) return false;
  const runtime = canvasInteractionRuntimeFor(app);
  const binding = currentCanvasInteractionBinding(app, runtime);
  if (
    !binding
    || !sameCanvasProjectionIdentity(binding.identity.canvas, selection.canvasIdentity)
  ) return false;
  const navigationNodes = canvasNavigationNodeIndex(app, runtime).byId;
  const members = selection.members.flatMap((member) => {
    if (member.resolution !== "resolved" || !member.anchor.editorNodeId) return [];
    const node = navigationNodes.get(member.anchor.editorNodeId);
    if (!node) return [];
    const target = canvasTargetFromNavigationNode(app, node);
    return [{
      memberId: member.memberId,
      targetKind: target.kind,
      boundaryKind: target.boundaryKind,
      componentKind: target.componentKind,
      editorNodeId: target.editorNodeId,
      actions: target.actions,
      selectionRevision: selection.selectionRevision,
      projection: canvasOverlayFromNavigationNode(node),
    }];
  });
  if (members.length === 0) return false;
  runtime.pendingInspections.clear();
  app.commands.postPreviewMessage({
    type: "render-canvas-interaction-overlay",
    agentInstanceId: binding.identity.agentInstanceId,
    documentEpoch: binding.identity.documentEpoch,
    channel: "selection",
    selectionRevision: selection.selectionRevision,
    primaryMemberId: selection.primaryMemberId,
    members,
  });
  const primaryMember = members.find(
    (member) => member.memberId === selection.primaryMemberId,
  );
  if (
    !primaryMember
    || primaryMember.targetKind === "boundary"
  ) return true;
  const primaryNode = navigationNodes.get(primaryMember.editorNodeId);
  if (!primaryNode) return true;
  const primaryTarget = canvasTargetFromNavigationNode(app, primaryNode);
  requestDomInspection(app, runtime, primaryTarget, {
    selectionRevision: selection.selectionRevision,
    pointer: options.pointer ?? {
      clientX: 0,
      clientY: 0,
      button: "none",
      buttons: 0,
      modifiers: { alt: false, control: false, meta: false, shift: false },
    },
    openContextMenu: options.openContextMenu === true,
    revealCode: options.revealCode === true,
  });
  return true;
}

async function commitNavigationSelection(
  app: CanvasInteractionControllerHost,
  runtime: CanvasInteractionFrontendRuntime,
  binding: CanvasInteractionBindingReceipt,
  node: EditorNavigationNode,
  options: {
    toggle?: boolean;
    extendRange?: boolean;
    setPrimary?: boolean;
    revealCode?: boolean;
  },
) {
  const kind = options.setPrimary
    ? "setPrimaryEditorNode"
    : options.extendRange
      ? "extendRangeToEditorNode"
      : options.toggle
        ? "toggleEditorNode"
        : "selectEditorNode";
  const selectionSnapshot = await app.selection.editorSelection.applySelectionIntent({
    kind,
    editorNodeId: node.id,
  });
  if (
    !selectionSnapshot
    || currentCanvasInteractionBinding(app, runtime) !== binding
  ) return null;
  projectSelectionSnapshotOnCanvas(app, selectionSnapshot, {
    revealCode: options.revealCode === true,
  });
  return selectionSnapshot;
}

function contextMenuPosition(
  app: CanvasInteractionControllerHost,
  pointer: CanvasPointerSample,
) {
  const frameRect = app.session.previewFrame?.getBoundingClientRect();
  return {
    x: (frameRect?.left ?? 0) + pointer.clientX,
    y: (frameRect?.top ?? 0) + pointer.clientY,
  };
}

function openHtmlContextMenu(
  app: CanvasInteractionControllerHost,
  pointer: CanvasPointerSample,
) {
  const selection = app.selection.coordinatedElementSelection;
  if (!selection) return;
  const observation = selection.observation;
  const position = contextMenuPosition(app, pointer);
  contextMenu.open({
    source: "preview",
    ...position,
    title: observation.selector || `<${observation.tag}>`,
    subtitle: observation.text,
    items: htmlElementContextMenuItems(
      app.runtime.editorRuntime,
      htmlTargetFromCoordinatedSelection(selection),
      "preview",
    ),
  });
}

export function openTeraContextMenu(
  app: CanvasInteractionControllerHost,
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
      app.runtime.editorRuntime,
      teraTargetFromBoundary({
        sourceId: target.sourceNodeId,
        renderInstanceId: target.renderInstanceId,
        origin: target.origin === "theme" ? "theme" : "current",
        themeName: target.themeName,
        editorNodeId: target.editorNodeId,
        canEnterBoundary: target.actions.canEnterBoundary,
      }),
      "preview",
    ),
  });
}
