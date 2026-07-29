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
import { runInPreviewStructuralLane } from "$lib/kernel/preview-structural-lane";
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
} & Parameters<typeof runInPreviewStructuralLane>[0]
  & Parameters<typeof projectCommittedEditorMoveMutation>[0];

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
};

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
  const node = renderInstanceId
    ? snapshot.nodes.find(
      (candidate) => candidate.renderInstanceId === renderInstanceId,
    ) ?? null
    : boundarySourceId
      ? snapshot.nodes.find(
        (candidate) => candidate.kind === "teraBoundary"
          && candidate.boundary?.sourceNodeId === boundarySourceId
          && candidate.boundary.empty,
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
): Promise<EditorActionOutcome> {
  try {
    const outcome = await runInPreviewStructuralLane(host, async () => {
      const plan = await previewEditorNavigationMove(
        host,
        sourceNodeId,
        targetNodeId,
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
        selection: null,
      }, receipt);
      exitEditorNavigationScope(host);
      host.setGlobalStatus(t("editor-navigation-move-confirmed"), "idle");
      return committedAction();
    });
    return outcome ?? blockedAction(t("editor-navigation-session-inactive"));
  } catch (error) {
    const reason = errorMessage(error);
    host.setGlobalStatus(reason, "error");
    return failedAction(reason);
  }
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
