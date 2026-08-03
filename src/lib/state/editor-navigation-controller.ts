import {
  commitEditorMove,
  planEditorMove,
  requestEditorEditScope,
  type CanvasProjectionIdentity,
} from "$lib/project/io";
import {
  blockedAction,
  committedAction,
  failedAction,
  type EditorActionOutcome,
} from "$lib/editor-runtime/action-outcome";
import { projectCommittedEditorMoveMutation } from "$lib/kernel/preview-projection-control";
import { t } from "$lib/i18n/runtime.svelte";
import type {
  EditScopeGrant,
  EditorMovePlan,
  EditorNavigationNode,
  EditorNavigationSnapshot,
  ProjectMovePosition,
} from "$lib/types";
import type { GlobalStatusKind } from "$lib/status/global-status";
import type { PreviewTeraSelectionTarget } from "$lib/state/app-helpers";
import type { AppState } from "$lib/state/app.svelte";
import { errorMessage } from "$lib/util";
import type { EditFlushReason } from "$lib/session/edit-flush-registry";
import {
  hoverCanvasNavigationNode,
  selectCanvasNavigationNode,
} from "$lib/state/canvas-interaction-controller";

export type EditorNavigationControllerHost = {
  activeCanvasIdentity: CanvasProjectionIdentity | null;
  editorNavigationSnapshot: EditorNavigationSnapshot | null;
  editorEditScopeGrant: EditScopeGrant | null;
  editorEditScopeId: string | null;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
  setPreviewTeraSelection: (
    target: PreviewTeraSelectionTarget,
    options?: { status?: string },
  ) => void;
  refreshEditorNavigationSnapshot: (
    identity?: CanvasProjectionIdentity,
    previewUrl?: string,
  ) => Promise<void>;
  flushInteractiveEditorDrafts: (reason: EditFlushReason) => Promise<void>;
} & Parameters<typeof projectCommittedEditorMoveMutation>[0];

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

export type EditorMoveNodeAnchor = Readonly<{
  originalId: string;
  structuralPath: readonly number[];
  kind: EditorNavigationNode["kind"];
  tag: string | null;
  file: string | null;
  origin: EditorNavigationNode["origin"];
}>;

export function captureEditorMoveNodeAnchor(
  snapshot: EditorNavigationSnapshot,
  nodeId: string,
): EditorMoveNodeAnchor | null {
  const node = snapshot.nodes.find((candidate) => candidate.id === nodeId);
  if (!node) return null;
  const nodes = new Map(snapshot.nodes.map((candidate) => [candidate.id, candidate]));
  const path: number[] = [];
  const visited = new Set<string>();
  let current: EditorNavigationNode | undefined = node;
  while (current) {
    if (!visited.add(current.id)) return null;
    const siblings = current.parentId
      ? nodes.get(current.parentId)?.children
      : snapshot.rootNodeIds;
    if (!siblings) return null;
    const index = siblings.indexOf(current.id);
    if (index < 0) return null;
    path.unshift(index);
    current = current.parentId ? nodes.get(current.parentId) : undefined;
  }
  return Object.freeze({
    originalId: node.id,
    structuralPath: Object.freeze(path),
    kind: node.kind,
    tag: node.tag,
    file: node.file,
    origin: node.origin,
  });
}

export function resolveEditorMoveNodeAnchor(
  snapshot: EditorNavigationSnapshot,
  anchor: EditorMoveNodeAnchor,
): EditorNavigationNode | null {
  const exact = snapshot.nodes.find((node) => node.id === anchor.originalId);
  if (exact && editorMoveNodeMatchesAnchor(exact, anchor)) return exact;

  const nodes = new Map(snapshot.nodes.map((node) => [node.id, node]));
  let siblings = snapshot.rootNodeIds;
  let candidate: EditorNavigationNode | undefined;
  for (const index of anchor.structuralPath) {
    const id = siblings[index];
    if (!id) return null;
    candidate = nodes.get(id);
    if (!candidate) return null;
    siblings = candidate.children;
  }
  return candidate && editorMoveNodeMatchesAnchor(candidate, anchor)
    ? candidate
    : null;
}

function editorMoveNodeMatchesAnchor(
  node: EditorNavigationNode,
  anchor: EditorMoveNodeAnchor,
) {
  return node.kind === anchor.kind
    && node.tag === anchor.tag
    && node.file === anchor.file
    && node.origin === anchor.origin;
}

export function editorNavigationDropTargetStatus(
  host: Pick<
    EditorNavigationControllerHost,
    "editorNavigationSnapshot" | "editorEditScopeGrant"
  >,
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
      (candidate) => candidate.kind === "teraBoundary"
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
      message: "Deschide boundary-ul Tera înainte de drop.",
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

function editorNavigationViewNodeSelector(
  renderInstanceIds: string[],
  boundaryRootIds: string[] = [],
) {
  const renderInstanceId = renderInstanceIds[0] ?? boundaryRootIds[0] ?? null;
  return renderInstanceId
    ? `[data-pana-render-instance-id="${CSS.escape(renderInstanceId)}"]`
    : null;
}

export function selectEditorNavigationNode(
  host: EditorNavigationControllerHost,
  node: EditorNavigationNode,
) {
  selectCanvasNavigationNode(host as AppState, node);
}

export function hoverEditorNavigationNode(
  host: EditorNavigationControllerHost,
  node: EditorNavigationNode | null,
) {
  hoverCanvasNavigationNode(host as AppState, node);
}

export async function enterEditorNavigationScope(
  host: EditorNavigationControllerHost,
  scopeId: string,
) {
  const snapshot = host.editorNavigationSnapshot;
  const identity = host.activeCanvasIdentity;
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
  if (!sameCanvasIdentity(host.activeCanvasIdentity, identity)) {
    throw new Error(t("editor-navigation-preview-changed"));
  }
  host.editorEditScopeGrant = grant;
  host.editorEditScopeId = scopeId;
  host.setPreviewTeraSelection({
    selector: editorNavigationViewNodeSelector(
      scope.renderInstanceIds,
      scope.boundary.rootRenderInstanceIds,
    ) ?? "",
    sourceId: scope.boundary.sourceNodeId,
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
  const scope = host.editorNavigationSnapshot?.focusedView?.nodes.find(
    (node) => node.editorNodeId === host.editorEditScopeId
      && node.kind === "boundary",
  ) ?? null;
  host.editorEditScopeGrant = null;
  host.editorEditScopeId = null;
  if (scope?.boundary) {
    host.setPreviewTeraSelection({
      selector: editorNavigationViewNodeSelector(
        scope.renderInstanceIds,
        scope.boundary.rootRenderInstanceIds,
      ) ?? "",
      sourceId: scope.boundary.sourceNodeId,
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
): Promise<EditorMovePlan> {
  const snapshot = host.editorNavigationSnapshot;
  const identity = host.activeCanvasIdentity;
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
    editScopeGrant: host.editorEditScopeGrant,
  });
}

export async function moveEditorNavigationNode(
  host: EditorNavigationControllerHost,
  sourceNodeId: string,
  targetNodeId: string,
  position: ProjectMovePosition,
  preplanned: EditorMovePlan | null = null,
  inputEmittedAtMs = 0,
): Promise<EditorActionOutcome> {
  try {
    const capturedSnapshot = host.editorNavigationSnapshot;
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
    const settledSnapshot = host.editorNavigationSnapshot;
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
        host.editorNavigationSnapshot?.focusedView
          ?.previewContextRenderInstanceId ?? null,
      planToken: plan.token,
      inputEmittedAtMs,
      editScopeGrant: host.editorEditScopeGrant,
    });
    if (receipt.status !== "committed" || !receipt.workspaceMutation) {
      const reason = receipt.diagnostic ?? t("editor-navigation-commit-refused");
      host.setGlobalStatus(reason, "error");
      return blockedAction(reason);
    }
    await projectCommittedEditorMoveMutation(host, {
      projectRoot: plan.identity.projectRoot,
      sessionId: plan.identity.runtimeSessionId,
      projectSessionEpoch: host.projectSessionEpoch,
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

function sameCanvasIdentity(
  left: CanvasProjectionIdentity | EditorNavigationSnapshot["identity"] | null,
  right: CanvasProjectionIdentity | EditorNavigationSnapshot["identity"] | null,
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
