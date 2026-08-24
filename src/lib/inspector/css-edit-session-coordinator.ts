import type { CssViewport } from "$lib/css/contracts";
import type { CssMutationAuthorityReceipt } from "$lib/css/mutation-contract";
import { cssSemanticSelectionKey } from "$lib/inspector/css-selection-stability";
import type { SelectionMutationIdentity } from "$lib/preview/contracts";
import {
  projectWorkspaceDirtyStatusKey,
  type GlobalStatusPublishOptions,
} from "$lib/status/global-status";

export type CssEditSessionTarget = Readonly<{
  projectRoot: string;
  runtimeSessionId: string;
  selector: string;
  file: string;
  viewport: CssViewport;
  expectedSelection: SelectionMutationIdentity;
}>;

export type CssEditGesture = Readonly<{
  interactionId: string;
  started: boolean;
}>;

export type CssEditAuthorityOutcome = Readonly<{
  kind: "applied" | "noop" | "superseded";
  interactionId: string;
  workspaceRevision: number | null;
  transactionId: string | null;
}>;

export type CssEditReadDecision =
  | Readonly<{ kind: "read"; canonicalWorkspaceRevision: number }>
  | Readonly<{
      kind: "retain";
      reason: "awaitingCanonicalPreview";
      expectedWorkspaceRevision: number;
    }>;

export type CssEditSessionSnapshot = Readonly<{
  sessionKey: string;
  targetKey: string;
  pendingWorkspaceRevision: number | null;
  pendingTransactionId: string | null;
  canonicalWorkspaceRevision: number | null;
  latestInteractionId: string | null;
  unsettledInteractionCount: number;
}>;

type MutableCssEditSession = {
  sessionKey: string;
  targetKey: string;
  pendingWorkspaceRevision: number | null;
  pendingTransactionId: string | null;
  canonicalWorkspaceRevision: number | null;
  latestInteractionId: string | null;
  knownInteractions: Set<string>;
  unsettledInteractions: Set<string>;
};

let nextCssInteractionSequence = 0;

function runtimeKey(projectRoot: string, runtimeSessionId: string) {
  return `${projectRoot.trim()}\u0000${runtimeSessionId.trim()}`;
}

function targetKey(target: CssEditSessionTarget) {
  return [
    runtimeKey(target.projectRoot, target.runtimeSessionId),
    cssSemanticSelectionKey(target.expectedSelection),
    target.file.trim(),
    target.selector.trim(),
    target.viewport,
  ].join("\u0000");
}

function validRevision(value: number) {
  return Number.isSafeInteger(value) && value >= 0;
}

/**
 * Owns the semantic lifetime of one CSS Inspector target.
 *
 * Durable writes may advance ProjectWorkspace before Canvas and Selection are
 * canonical. This coordinator keeps the optimistic projection alive during
 * that interval and authorizes one exact read only when both authorities have
 * reached the same latest revision.
 */
export class CssEditSessionCoordinator {
  private runtimeKey = "";
  private dirtyStatusResolutionKey = "";
  private current: MutableCssEditSession | null = null;
  private activeGesture: { key: string; interactionId: string } | null = null;

  get snapshot(): CssEditSessionSnapshot | null {
    const session = this.current;
    if (!session) return null;
    return Object.freeze({
      sessionKey: session.sessionKey,
      targetKey: session.targetKey,
      pendingWorkspaceRevision: session.pendingWorkspaceRevision,
      pendingTransactionId: session.pendingTransactionId,
      canonicalWorkspaceRevision: session.canonicalWorkspaceRevision,
      latestInteractionId: session.latestInteractionId,
      unsettledInteractionCount: session.unsettledInteractions.size,
    });
  }

  syncRuntime(projectRoot: string, runtimeSessionId: string) {
    const nextRuntimeKey = runtimeKey(projectRoot, runtimeSessionId);
    if (nextRuntimeKey === this.runtimeKey) return;
    this.runtimeKey = nextRuntimeKey;
    this.dirtyStatusResolutionKey = projectWorkspaceDirtyStatusKey(
      projectRoot,
      runtimeSessionId,
    );
    this.current = null;
    this.activeGesture = null;
  }

  beginGesture(target: CssEditSessionTarget, property: string): CssEditGesture {
    const session = this.ensureTarget(target);
    const gestureKey = `${session.targetKey}\u0000${property.trim()}`;
    if (this.activeGesture?.key === gestureKey) {
      return Object.freeze({
        interactionId: this.activeGesture.interactionId,
        started: false,
      });
    }
    const interactionId = `css-edit:${++nextCssInteractionSequence}`;
    this.activeGesture = { key: gestureKey, interactionId };
    session.latestInteractionId = interactionId;
    session.knownInteractions.add(interactionId);
    session.unsettledInteractions.add(interactionId);
    return Object.freeze({ interactionId, started: true });
  }

  finishGesture() {
    this.activeGesture = null;
  }

  abandonGesture() {
    const interactionId = this.activeGesture?.interactionId ?? null;
    this.activeGesture = null;
    if (!interactionId || !this.current) return;
    this.current.unsettledInteractions.delete(interactionId);
  }

  acceptAuthority(
    interactionId: string,
    authority: Pick<
      CssMutationAuthorityReceipt,
      | "operationId"
      | "projectRoot"
      | "sessionId"
      | "revisionBefore"
      | "revisionAfter"
    >,
  ): CssEditAuthorityOutcome {
    const session = this.current;
    const foreign = !session
      || !session.knownInteractions.has(interactionId)
      || session.sessionKey !== runtimeKey(authority.projectRoot, authority.sessionId);
    if (foreign) {
      return Object.freeze({
        kind: "superseded",
        interactionId,
        workspaceRevision: null,
        transactionId: null,
      });
    }
    session.unsettledInteractions.delete(interactionId);
    if (!validRevision(authority.revisionAfter) || authority.revisionAfter < authority.revisionBefore) {
      throw new Error("CssEditSession a primit o revizie autoritară invalidă.");
    }
    if (
      session.pendingWorkspaceRevision !== null
      && authority.revisionAfter < session.pendingWorkspaceRevision
    ) {
      return Object.freeze({
        kind: "superseded",
        interactionId,
        workspaceRevision: authority.revisionAfter,
        transactionId: authority.operationId,
      });
    }
    session.pendingWorkspaceRevision = authority.revisionAfter;
    session.pendingTransactionId = authority.operationId;
    return Object.freeze({
      kind: authority.revisionAfter === authority.revisionBefore ? "noop" : "applied",
      interactionId,
      workspaceRevision: authority.revisionAfter,
      transactionId: authority.operationId,
    });
  }

  rejectAuthority(interactionId: string) {
    this.current?.unsettledInteractions.delete(interactionId);
  }

  readDecision(input: CssEditSessionTarget & Readonly<{
    workspaceRevision: number;
    selectionWorkspaceRevision: number;
  }>): CssEditReadDecision {
    const session = this.ensureTarget(input);
    const latestWorkspaceRevision = Math.max(
      input.workspaceRevision,
      session.pendingWorkspaceRevision ?? input.workspaceRevision,
    );
    if (
      input.workspaceRevision !== input.selectionWorkspaceRevision
      || input.workspaceRevision < latestWorkspaceRevision
      || session.unsettledInteractions.size > 0
    ) {
      return Object.freeze({
        kind: "retain",
        reason: "awaitingCanonicalPreview",
        expectedWorkspaceRevision: latestWorkspaceRevision,
      });
    }
    session.canonicalWorkspaceRevision = input.workspaceRevision;
    if (
      session.pendingWorkspaceRevision !== null
      && input.workspaceRevision >= session.pendingWorkspaceRevision
    ) {
      session.pendingWorkspaceRevision = null;
      session.pendingTransactionId = null;
    }
    return Object.freeze({
      kind: "read",
      canonicalWorkspaceRevision: input.workspaceRevision,
    });
  }

  statusOptions(
    interactionId: string,
    phase: "preview" | "saved" | "error",
  ): GlobalStatusPublishOptions {
    const lane = `css-inspector:${interactionId}`;
    return {
      code: `css-inspector.${phase}`,
      source: "css-inspector",
      dedupeKey: lane,
      resolutionKey: phase === "error"
        ? lane
        : this.dirtyStatusResolutionKey || lane,
      escalation: phase === "error" ? "notification" : "status_only",
    };
  }

  reset() {
    this.runtimeKey = "";
    this.dirtyStatusResolutionKey = "";
    this.current = null;
    this.activeGesture = null;
  }

  private ensureTarget(target: CssEditSessionTarget) {
    this.syncRuntime(target.projectRoot, target.runtimeSessionId);
    const nextTargetKey = targetKey(target);
    if (!this.current || this.current.targetKey !== nextTargetKey) {
      this.current = {
        sessionKey: this.runtimeKey,
        targetKey: nextTargetKey,
        pendingWorkspaceRevision: null,
        pendingTransactionId: null,
        canonicalWorkspaceRevision: null,
        latestInteractionId: null,
        knownInteractions: new Set(),
        unsettledInteractions: new Set(),
      };
      this.activeGesture = null;
    }
    return this.current;
  }
}
