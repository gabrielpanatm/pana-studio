import {
  commitEditorMove,
  planEditorMove,
  requestEditorEditScope,
} from "$lib/editor/navigation-io";
import type {
  CanvasProjectionIdentity,
} from "$lib/contracts/canvas-projection";
import {
  blockedAction,
  committedAction,
  failedAction,
  type EditorActionOutcome,
} from "$lib/editor-runtime/action-outcome";
import {
  projectCommittedEditorMoveMutation,
  type CommittedMutationProjectionContext,
} from "$lib/kernel/preview-projection-control";
import { t } from "$lib/i18n/runtime.svelte";
import type { NativeBlockSlotMutationContext } from "$lib/blocks/contracts";
import type {
  EditorMovePlan,
  EditScopeGrant,
} from "$lib/editor/contracts";
import type {
  EditorNavigationNode,
  EditorNavigationSnapshot,
} from "$lib/editor/contracts";
import type {
  SelectionAnchor,
  SelectionSnapshot,
} from "$lib/editor/contracts";
import type { ProjectMovePosition } from "$lib/preview/contracts";
import type { GlobalStatusKind } from "$lib/status/global-status";
import type { PreviewTeraSelectionTarget } from "$lib/state/app-helpers";
import { errorMessage } from "$lib/util";
import type { EditFlushReason } from "$lib/session/edit-flush-registry";
import { sameCanvasProjectionIdentity as sameCanvasIdentity } from "$lib/contracts/canvas-identity";
import type { EditorSelectionSessionController } from "$lib/state/editor-selection-session.svelte";

export type EditorNavigationControllerHost = {
  context: () => Readonly<{
    activeCanvasIdentity: CanvasProjectionIdentity | null;
    projectSessionEpoch: number;
  }>;
  editorSelection: Pick<
    EditorSelectionSessionController,
    "navigationSnapshot" | "editScopeGrant" | "editScopeId"
  >;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
  setPreviewTeraSelection: (
    target: PreviewTeraSelectionTarget,
    options?: { status?: string },
  ) => void;
  flushInteractiveEditorDrafts: (reason: EditFlushReason) => Promise<void>;
  selectCanvasNode: (
    node: EditorNavigationNode,
    options?: { toggle?: boolean; extendRange?: boolean; setPrimary?: boolean },
  ) => Promise<SelectionSnapshot | null>;
  hoverCanvasNode: (node: EditorNavigationNode | null) => void;
  projectCommittedMove: (
    context: CommittedMutationProjectionContext,
    receipt: Parameters<typeof projectCommittedEditorMoveMutation>[2],
  ) => ReturnType<typeof projectCommittedEditorMoveMutation>;
};

export function editorNavigationNodeSelector(node: EditorNavigationNode): string | null {
  const renderInstanceId = node.renderInstanceId
    ?? node.boundary?.rootRenderInstanceIds[0]
    ?? null;
  if (!renderInstanceId) return null;
  return `[data-pana-render-instance-id="${CSS.escape(renderInstanceId)}"]`;
}

export type EditorNavigationDropTarget = {
  targetRenderInstanceId?: string | null;
  targetBoundarySourceId?: string | null;
  targetBoundaryInstanceId?: string | null;
};

export type EditorMoveNodeAnchor = Readonly<Pick<
  SelectionAnchor,
  "editorNodeId" | "sourceNodeId" | "renderInstanceId" | "boundaryInstanceId"
>>;

export function captureEditorMoveNodeAnchor(
  snapshot: EditorNavigationSnapshot,
  nodeId: string,
): EditorMoveNodeAnchor | null {
  const node = snapshot.nodes.find((candidate) => candidate.id === nodeId);
  if (!node) return null;
  return Object.freeze({
    editorNodeId: node.id,
    sourceNodeId: node.sourceNodeId,
    renderInstanceId: node.renderInstanceId,
    boundaryInstanceId: node.boundary?.boundaryInstanceId ?? null,
  });
}

export function resolveEditorMoveNodeAnchor(
  snapshot: EditorNavigationSnapshot,
  anchor: EditorMoveNodeAnchor,
): EditorNavigationNode | null {
  const exact = snapshot.nodes.find((node) => node.id === anchor.editorNodeId);
  if (exact && editorMoveNodeMatchesAnchor(exact, anchor)) return exact;
  const candidates = anchor.renderInstanceId
    ? snapshot.nodes.filter((node) => node.renderInstanceId === anchor.renderInstanceId)
    : anchor.boundaryInstanceId
      ? snapshot.nodes.filter(
          (node) => node.boundary?.boundaryInstanceId === anchor.boundaryInstanceId,
        )
      : [];
  const matched = candidates.filter((node) => editorMoveNodeMatchesAnchor(node, anchor));
  return matched.length === 1 ? matched[0] : null;
}

function editorMoveNodeMatchesAnchor(
  node: EditorNavigationNode,
  anchor: EditorMoveNodeAnchor,
) {
  return (!anchor.sourceNodeId || node.sourceNodeId === anchor.sourceNodeId)
    && (!anchor.renderInstanceId || node.renderInstanceId === anchor.renderInstanceId)
    && (
      !anchor.boundaryInstanceId
      || node.boundary?.boundaryInstanceId === anchor.boundaryInstanceId
    );
}

export function editorNavigationDropTargetStatus(
  host: {
    editorNavigationSnapshot: EditorNavigationSnapshot | null;
    editorEditScopeGrant: EditScopeGrant | null;
  },
  target: EditorNavigationDropTarget,
) {
  const snapshot = host.editorNavigationSnapshot;
  if (!snapshot) {
    return {
      allowed: false,
      message: "EditorNavigationSnapshot nu este disponibil pentru drop.",
    };
  }
  const renderInstanceId = target.targetRenderInstanceId?.trim() || null;
  const boundarySourceId = target.targetBoundarySourceId?.trim() || null;
  const boundaryInstanceId = target.targetBoundaryInstanceId?.trim() || null;
  // An active empty document can also contain a synthetic render node used
  // only to give the boundary geometry. Its semantic target remains the Rust
  // boundary instance, so never let that helper render identity shadow it.
  const node = boundaryInstanceId
    ? snapshot.nodes.find(
      (candidate) => candidate.kind === "boundary"
        && candidate.boundary?.boundaryInstanceId === boundaryInstanceId
        && (!boundarySourceId || candidate.boundary.sourceNodeId === boundarySourceId),
    ) ?? null
    : renderInstanceId
      ? snapshot.nodes.find(
        (candidate) => candidate.renderInstanceId === renderInstanceId,
      ) ?? null
      : null;
  if (!node) {
    return {
      allowed: false,
      message: "Ținta fizică nu există în EditorNavigationSnapshot-ul Rust curent.",
    };
  }
  const requiredScopeId = node.capabilities.requiresEditScopeId;
  if (
    requiredScopeId
    && host.editorEditScopeGrant?.scopeId !== requiredScopeId
  ) {
    return {
      allowed: false,
      message: "Deschide boundary-ul semantic înainte de drop.",
    };
  }
  if (
    (node.capabilities.readOnly && !requiredScopeId)
    || node.origin === "theme"
  ) {
    return {
      allowed: false,
      message: "Ținta Rust este read-only.",
    };
  }
  return { allowed: true, editorNodeId: node.id };
}

function editorNavigationViewNodeRenderInstanceId(
  renderInstanceIds: string[],
  boundaryRootIds: string[] = [],
) {
  return renderInstanceIds[0] ?? boundaryRootIds[0] ?? null;
}

export function selectEditorNavigationNode(
  host: EditorNavigationControllerHost,
  node: EditorNavigationNode,
  options: {
    toggle?: boolean;
    extendRange?: boolean;
    setPrimary?: boolean;
  } = {},
) {
  return host.selectCanvasNode(node, options);
}

export function hoverEditorNavigationNode(
  host: EditorNavigationControllerHost,
  node: EditorNavigationNode | null,
) {
  host.hoverCanvasNode(node);
}

export async function enterEditorNavigationScope(
  host: EditorNavigationControllerHost,
  scopeId: string,
) {
  const snapshot = host.editorSelection.navigationSnapshot;
  const identity = host.context().activeCanvasIdentity;
  if (!snapshot || !identity) {
    throw new Error(t("editor-navigation-snapshot-unavailable"));
  }
  const scope = snapshot.focusedView?.nodes.find(
    (node) => node.editorNodeId === scopeId && node.kind === "boundary",
  );
  if (!scope?.boundary) {
    throw new Error(t("editor-navigation-boundary-missing"));
  }
  const activeDocumentPath = requireFocusedActiveDocument(snapshot);
  const grant = await requestEditorEditScope(
    identity,
    snapshot.route,
    activeDocumentPath,
    scopeId,
    snapshot.focusedView?.previewContextRenderInstanceId ?? null,
  );
  if (!sameCanvasIdentity(host.context().activeCanvasIdentity, identity)) {
    throw new Error(t("editor-navigation-preview-changed"));
  }
  host.editorSelection.editScopeGrant = grant;
  host.editorSelection.editScopeId = scopeId;
  const renderInstanceId = editorNavigationViewNodeRenderInstanceId(
    scope.renderInstanceIds,
    scope.boundary.rootRenderInstanceIds,
  );
  host.setPreviewTeraSelection({
    sourceId: scope.boundary.sourceNodeId,
    renderInstanceId,
    origin: scope.origin === "theme"
      ? "theme"
      : scope.origin === "project"
        ? "current"
        : "unknown",
    themeName: scope.themeName,
  });
  host.setGlobalStatus(
    scope.boundary.effectScope === "sharedDefinition"
      ? t("project-navigation-shared-definition")
      : t("editor-navigation-scope-opened"),
    "idle",
  );
  return grant;
}

export function exitEditorNavigationScope(host: EditorNavigationControllerHost) {
  const scope = host.editorSelection.navigationSnapshot?.focusedView?.nodes.find(
    (node) => node.editorNodeId === host.editorSelection.editScopeId
      && node.kind === "boundary",
  ) ?? null;
  host.editorSelection.editScopeGrant = null;
  host.editorSelection.editScopeId = null;
  if (scope?.boundary) {
    const renderInstanceId = editorNavigationViewNodeRenderInstanceId(
      scope.renderInstanceIds,
      scope.boundary.rootRenderInstanceIds,
    );
    host.setPreviewTeraSelection({
      sourceId: scope.boundary.sourceNodeId,
      renderInstanceId,
      origin: scope.origin === "theme"
        ? "theme"
        : scope.origin === "project"
          ? "current"
          : "unknown",
      themeName: scope.themeName,
    });
  }
}

export async function previewEditorNavigationMove(
  host: EditorNavigationControllerHost,
  sourceNodeId: string,
  targetNodeId: string,
  position: ProjectMovePosition,
  nativeBlockSlot: NativeBlockSlotMutationContext | null = null,
): Promise<EditorMovePlan> {
  const snapshot = host.editorSelection.navigationSnapshot;
  const identity = host.context().activeCanvasIdentity;
  if (!snapshot || !identity || !sameCanvasIdentity(snapshot.identity, identity)) {
    throw new Error(t("editor-navigation-snapshot-stale"));
  }
  return await planEditorMove({
    identity,
    route: snapshot.route,
    activeDocumentPath: requireFocusedActiveDocument(snapshot),
    previewContextRenderInstanceId:
      snapshot.focusedView?.previewContextRenderInstanceId ?? null,
    sourceNodeId,
    targetNodeId,
    position,
    editScopeGrant: host.editorSelection.editScopeGrant,
    nativeBlockSlot,
  });
}

export async function moveEditorNavigationNode(
  host: EditorNavigationControllerHost,
  sourceNodeId: string,
  targetNodeId: string,
  position: ProjectMovePosition,
  preplanned: EditorMovePlan | null = null,
  inputEmittedAtMs = 0,
  nativeBlockSlot: NativeBlockSlotMutationContext | null = null,
): Promise<EditorActionOutcome> {
  try {
    const capturedSnapshot = host.editorSelection.navigationSnapshot;
    if (!capturedSnapshot) {
      throw new Error(t("editor-navigation-snapshot-unavailable"));
    }
    const sourceAnchor = captureEditorMoveNodeAnchor(capturedSnapshot, sourceNodeId);
    const targetAnchor = captureEditorMoveNodeAnchor(capturedSnapshot, targetNodeId);
    if (!sourceAnchor || !targetAnchor) {
      throw new Error(t("editor-navigation-snapshot-stale"));
    }
    // Text/attribute editing and structural moves must become distinct,
    // ordered ProjectWorkspace history entries. Closing the captured draft
    // before planning the move also prevents Undo from inheriting a stale
    // interactive edit session.
    await host.flushInteractiveEditorDrafts("snapshot");
    const settledSnapshot = host.editorSelection.navigationSnapshot;
    if (!settledSnapshot) {
      throw new Error(t("editor-navigation-snapshot-unavailable"));
    }
    const settledSource = resolveEditorMoveNodeAnchor(settledSnapshot, sourceAnchor);
    const settledTarget = resolveEditorMoveNodeAnchor(settledSnapshot, targetAnchor);
    if (!settledSource || !settledTarget || settledSource.id === settledTarget.id) {
      throw new Error(t("editor-navigation-snapshot-stale"));
    }
    const plan = editorMovePlanMatchesCurrentSnapshot(
      preplanned,
      settledSnapshot,
      settledSource.id,
      settledTarget.id,
      position,
    )
      ? preplanned
      : await previewEditorNavigationMove(
          host,
          settledSource.id,
          settledTarget.id,
          position,
          nativeBlockSlot,
        );
    if (!plan.allowed || !plan.token) {
      const reason = plan.reason ?? t("editor-navigation-move-refused");
      host.setGlobalStatus(reason, "error");
      return blockedAction(reason);
    }
    const receipt = await commitEditorMove({
      identity: plan.identity,
      route: plan.route,
      activeDocumentPath: plan.activeDocumentPath,
      previewContextRenderInstanceId:
        host.editorSelection.navigationSnapshot?.focusedView
          ?.previewContextRenderInstanceId ?? null,
      planToken: plan.token,
      inputEmittedAtMs,
      editScopeGrant: host.editorSelection.editScopeGrant,
    });
    if (receipt.status !== "committed" || !receipt.workspaceMutation) {
      const reason = receipt.diagnostic ?? t("editor-navigation-commit-refused");
      host.setGlobalStatus(reason, "error");
      return blockedAction(reason);
    }
    await host.projectCommittedMove({
      projectRoot: plan.identity.projectRoot,
      sessionId: plan.identity.runtimeSessionId,
      projectSessionEpoch: host.context().projectSessionEpoch,
      expectedWorkspaceRevision: receipt.workspaceMutation.revisionAfter,
    }, receipt);
    exitEditorNavigationScope(host);
    host.setGlobalStatus(t("editor-navigation-move-confirmed"), "idle");
    return committedAction();
  } catch (error) {
    const reason = errorMessage(error);
    host.setGlobalStatus(reason, "error");
    return failedAction(reason);
  }
}

function editorMovePlanMatchesCurrentSnapshot(
  plan: EditorMovePlan | null,
  snapshot: EditorNavigationSnapshot,
  sourceNodeId: string,
  targetNodeId: string,
  position: ProjectMovePosition,
): plan is EditorMovePlan {
  return Boolean(
    plan
    && plan.allowed
    && plan.token
    && plan.operation
    && sameCanvasIdentity(plan.identity, snapshot.identity)
    && plan.modelRevision === snapshot.modelRevision
    && plan.route === snapshot.route
    && plan.activeDocumentPath === snapshot.focusedView?.activeDocumentPath
    && plan.sourceNodeId === sourceNodeId
    && plan.targetNodeId === targetNodeId
    && plan.position === position,
  );
}

function requireFocusedActiveDocument(snapshot: EditorNavigationSnapshot) {
  const activeDocumentPath = snapshot.focusedView?.activeDocumentPath?.trim();
  if (!activeDocumentPath) {
    throw new Error(t("editor-navigation-snapshot-unavailable"));
  }
  return activeDocumentPath;
}
