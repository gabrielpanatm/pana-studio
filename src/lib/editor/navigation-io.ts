import {
  EDIT_SCOPE_GRANT_SCHEMA_VERSION,
  EDITOR_MOVE_EXECUTION_SCHEMA_VERSION,
  EDITOR_MOVE_LIVE_PROJECTION_SCHEMA_VERSION,
  EDITOR_MOVE_PLAN_SCHEMA_VERSION,
  type EditorMoveCommitInput,
  type EditorMoveExecutionReceipt,
  type EditorMovePlan,
  type EditorMovePlanInput,
  type EditScopeGrant,
} from "$lib/editor/contracts";
import {
  EDITOR_NAVIGATION_SCHEMA_VERSION,
  type EditorNavigationSnapshot,
} from "$lib/editor/contracts";
import type { CanvasProjectionIdentity } from "$lib/contracts/canvas-projection";
import {
  sameCanvasProjectionIdentity,
  sameProjectDocumentPath,
} from "$lib/contracts/canvas-identity";
import { invoke } from "@tauri-apps/api/core";
import { schemaMismatch } from "$lib/contracts/io-schema";

export async function readEditorNavigationSnapshot(
  identity: CanvasProjectionIdentity,
  route: string,
  activeDocumentPath: string | null,
  previewContextRenderInstanceId: string | null = null,
): Promise<EditorNavigationSnapshot> {
  const snapshot = await invoke<EditorNavigationSnapshot>(
    "read_editor_navigation_snapshot",
    {
      input: {
        identity,
        route,
        activeDocumentPath,
        previewContextRenderInstanceId,
      },
    },
  );
  if (snapshot.schemaVersion !== EDITOR_NAVIGATION_SCHEMA_VERSION) {
    throw schemaMismatch(
      "EditorNavigationSnapshot",
      snapshot.schemaVersion,
      EDITOR_NAVIGATION_SCHEMA_VERSION,
    );
  }
  if (!sameCanvasProjectionIdentity(snapshot.identity, identity)) {
    throw new Error("EditorNavigationSnapshot a întors altă identitate Canvas.");
  }
  if (
    activeDocumentPath
    && snapshot.focusedView
    && !sameProjectDocumentPath(
      snapshot.focusedView.activeDocumentPath,
      activeDocumentPath,
    )
  ) {
    throw new Error("EditorNavigationSnapshot a întors alt document activ.");
  }
  return snapshot;
}

export async function requestEditorEditScope(
  identity: CanvasProjectionIdentity,
  route: string,
  activeDocumentPath: string,
  scopeId: string,
  previewContextRenderInstanceId: string | null = null,
): Promise<EditScopeGrant> {
  const grant = await invoke<EditScopeGrant>("request_editor_edit_scope", {
    input: {
      identity,
      route,
      activeDocumentPath,
      previewContextRenderInstanceId,
      scopeId,
    },
  });
  if (grant.schemaVersion !== EDIT_SCOPE_GRANT_SCHEMA_VERSION) {
    throw schemaMismatch(
      "EditScopeGrant",
      grant.schemaVersion,
      EDIT_SCOPE_GRANT_SCHEMA_VERSION,
    );
  }
  if (
    !sameCanvasProjectionIdentity({
      projectRoot: grant.projectRoot,
      runtimeSessionId: grant.runtimeSessionId,
      workspaceRevision: grant.workspaceRevision,
      transactionId: grant.canvasTransactionId,
      previewRevision: grant.previewRevision,
    }, identity)
    || grant.scopeId !== scopeId
    || grant.activeDocumentPath !== activeDocumentPath
  ) {
    throw new Error("EditScopeGrant a întors alt context Canvas.");
  }
  return grant;
}

export async function planEditorMove(
  input: EditorMovePlanInput,
): Promise<EditorMovePlan> {
  const plan = await invoke<EditorMovePlan>("plan_editor_move", { input });
  if (plan.schemaVersion !== EDITOR_MOVE_PLAN_SCHEMA_VERSION) {
    throw schemaMismatch(
      "PlanEditorMove",
      plan.schemaVersion,
      EDITOR_MOVE_PLAN_SCHEMA_VERSION,
    );
  }
  if (
    !sameCanvasProjectionIdentity(plan.identity, input.identity)
    || plan.sourceNodeId !== input.sourceNodeId
    || plan.targetNodeId !== input.targetNodeId
    || plan.position !== input.position
    || plan.activeDocumentPath !== input.activeDocumentPath
  ) {
    throw new Error("PlanEditorMove a întors altă intenție sau identitate Canvas.");
  }
  if (plan.allowed !== Boolean(plan.token && plan.operation)) {
    throw new Error("PlanEditorMove a întors o stare permis/refuz inconsistentă.");
  }
  requireEditorMoveLiveProjection(plan);
  return plan;
}

export function requireEditorMoveLiveProjection(plan: EditorMovePlan) {
  const projection = plan.liveProjection;
  if (!projection) {
    if (plan.liveProjectionReason === "ready") {
      throw new Error("PlanEditorMove a omis proiecția live marcată ready.");
    }
    return;
  }
  if (
    !plan.allowed
    || !plan.token
    || plan.liveProjectionReason !== "ready"
    || projection.schemaVersion !== EDITOR_MOVE_LIVE_PROJECTION_SCHEMA_VERSION
    || projection.operation !== "move"
    || projection.scope !== "selectedInstance"
    || projection.planToken !== plan.token
    || projection.position !== plan.position
    || projection.sourceRenderInstanceId.length === 0
    || projection.targetRenderInstanceId.length === 0
    || projection.sourceRenderInstanceId === projection.targetRenderInstanceId
    || !sameCanvasProjectionIdentity(projection.identity, plan.identity)
  ) {
    throw new Error("PlanEditorMove a întors o proiecție live inconsistentă.");
  }
}

export async function commitEditorMove(
  input: EditorMoveCommitInput,
): Promise<EditorMoveExecutionReceipt> {
  const receipt = await invoke<EditorMoveExecutionReceipt>(
    "commit_editor_move",
    { input },
  );
  if (receipt.schemaVersion !== EDITOR_MOVE_EXECUTION_SCHEMA_VERSION) {
    throw schemaMismatch(
      "EditorMoveExecutionReceipt",
      receipt.schemaVersion,
      EDITOR_MOVE_EXECUTION_SCHEMA_VERSION,
    );
  }
  if (receipt.planToken !== input.planToken) {
    throw new Error("EditorMoveExecutionReceipt aparține altui plan.");
  }
  if (
    receipt.projectRoot !== input.identity.projectRoot
    || receipt.runtimeSessionId !== input.identity.runtimeSessionId
  ) {
    throw new Error("EditorMoveExecutionReceipt aparține altei sesiuni.");
  }
  const timings = receipt.timings;
  if (
    !timings
    || !Number.isSafeInteger(timings.inputEmittedAtMs)
    || timings.inputEmittedAtMs < 0
    || timings.inputEmittedAtMs !== (input.inputEmittedAtMs ?? 0)
    || !Number.isSafeInteger(timings.planIssuedAtMs)
    || timings.planIssuedAtMs <= 0
    || !Number.isSafeInteger(timings.rustReceivedAtMs)
    || timings.rustReceivedAtMs <= 0
    || !Number.isSafeInteger(timings.rustCompletedAtMs)
    || timings.rustCompletedAtMs < timings.rustReceivedAtMs
    || !Number.isSafeInteger(timings.inputToReceiptMs)
    || timings.inputToReceiptMs < 0
    || !Number.isSafeInteger(timings.pointerUpToCommitReceiptMs)
    || timings.pointerUpToCommitReceiptMs < 0
    || timings.pointerUpToCommitReceiptMs !== timings.inputToReceiptMs
    || !Number.isSafeInteger(timings.planToReceiptMs)
    || timings.planToReceiptMs < 0
    || !Number.isSafeInteger(timings.rustCommandMs)
    || timings.rustCommandMs < 0
    || !Number.isSafeInteger(timings.candidateCloneMs)
    || timings.candidateCloneMs < 0
    || !Number.isSafeInteger(timings.mutationMs)
    || timings.mutationMs < 0
    || !Number.isSafeInteger(timings.recoveryPersistMs)
    || timings.recoveryPersistMs < 0
    || !Number.isSafeInteger(timings.authorityPublishMs)
    || timings.authorityPublishMs < 0
    || !Number.isSafeInteger(timings.authorityTransactionMs)
    || timings.authorityTransactionMs < 0
    || !Number.isSafeInteger(timings.planRevalidationMs)
    || timings.planRevalidationMs < 0
    || !Number.isSafeInteger(timings.nativeBlockContractMs)
    || timings.nativeBlockContractMs < 0
    || !Number.isSafeInteger(timings.workspaceStageMs)
    || timings.workspaceStageMs < 0
    || !Number.isSafeInteger(timings.afterProjectModelBuildMs)
    || timings.afterProjectModelBuildMs < 0
    || (
      timings.patchIssuedToReceiptMs !== null
      && (
        !Number.isSafeInteger(timings.patchIssuedToReceiptMs)
        || timings.patchIssuedToReceiptMs < 0
      )
    )
  ) {
    throw new Error("EditorMoveExecutionReceipt are telemetrie Rust invalidă.");
  }
  return receipt;
}
