import {
  startAiCoordinationEvents,
  stopAiCoordinationEvents,
  type AiCoordinationControllerHost,
} from "$lib/state/ai-coordination-controller";
import type { AiContextState } from "$lib/ai/context-state.svelte";
import type { ExternalDiskState } from "$lib/session/external-disk-state.svelte";
import type {
  GlobalStatusEscalationRequest,
  GlobalStatusKind,
  GlobalStatusPublishOptions,
} from "$lib/status/global-status";
import type { ProjectReloadOutcome } from "$lib/project/controller-contracts";
import type { AiCoordinationSnapshot } from "$lib/ai/contracts";
import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";

export type AiCoordinationStateDependencies = {
  context: AiContextState;
  activeScannedPath: () => string | null;
  workspace: () => ProjectWorkspaceSnapshot | null;
  externalDisk: ExternalDiskState;
  quiesceInteractions: () => void;
  discardAndReload: (preferredRelativePath?: string | null) => Promise<ProjectReloadOutcome>;
  setStatus: (
    text: string,
    kind: GlobalStatusKind,
    options?: GlobalStatusPublishOptions,
  ) => void;
  escalateStatus: (notification: GlobalStatusEscalationRequest) => void;
  clearStatus: (id: string) => void;
};

/** Owns the frontend projection and leases of Rust AI edit coordination. */
export class AiCoordinationState {
  snapshot = $state<AiCoordinationSnapshot | null>(null);
  frontendLockActive = $state(false);
  unlisten: (() => void) | null = null;
  subscriptionGeneration = 0;
  pendingSnapshot: AiCoordinationSnapshot | null = null;
  operationInFlight = false;
  handledRequestId: string | null = null;
  reconciliationLeaseId: string | null = null;
  automaticReloadLeaseId: string | null = null;
  recoveryReloadAuthorized = false;

  private readonly host: AiCoordinationControllerHost;

  constructor(dependencies: AiCoordinationStateDependencies) {
    const owner = this;
    this.host = {
      state: {
        get snapshot() { return owner.snapshot; },
        set snapshot(snapshot) { owner.snapshot = snapshot; },
        get unlisten() { return owner.unlisten; },
        set unlisten(unlisten) { owner.unlisten = unlisten; },
        get subscriptionGeneration() { return owner.subscriptionGeneration; },
        set subscriptionGeneration(generation) { owner.subscriptionGeneration = generation; },
        get pendingSnapshot() { return owner.pendingSnapshot; },
        set pendingSnapshot(snapshot) { owner.pendingSnapshot = snapshot; },
        get operationInFlight() { return owner.operationInFlight; },
        set operationInFlight(inFlight) { owner.operationInFlight = inFlight; },
        get handledRequestId() { return owner.handledRequestId; },
        set handledRequestId(requestId) { owner.handledRequestId = requestId; },
        get reconciliationLeaseId() { return owner.reconciliationLeaseId; },
        set reconciliationLeaseId(leaseId) { owner.reconciliationLeaseId = leaseId; },
        get automaticReloadLeaseId() { return owner.automaticReloadLeaseId; },
        set automaticReloadLeaseId(leaseId) { owner.automaticReloadLeaseId = leaseId; },
        get frontendLockActive() { return owner.frontendLockActive; },
        set frontendLockActive(active) { owner.frontendLockActive = active; },
        get recoveryReloadAuthorized() { return owner.recoveryReloadAuthorized; },
        set recoveryReloadAuthorized(authorized) { owner.recoveryReloadAuthorized = authorized; },
      },
      session: {
        get activeScannedPath() { return dependencies.activeScannedPath(); },
        get workspace() { return dependencies.workspace(); },
        get externalDisk() { return dependencies.externalDisk.snapshot; },
      },
      context: dependencies.context,
      commands: {
        quiesceInteractions: dependencies.quiesceInteractions,
        externalDisk: dependencies.externalDisk,
        discardAndReload: dependencies.discardAndReload,
        setStatus: dependencies.setStatus,
        escalateStatus: dependencies.escalateStatus,
        clearStatus: dependencies.clearStatus,
      },
    };
  }

  start() {
    startAiCoordinationEvents(this.host);
  }

  stop() {
    stopAiCoordinationEvents(this.host);
  }

  controllerHost() {
    return this.host;
  }

  reset() {
    this.stop();
    this.snapshot = null;
    this.pendingSnapshot = null;
    this.operationInFlight = false;
    this.handledRequestId = null;
    this.reconciliationLeaseId = null;
    this.automaticReloadLeaseId = null;
    this.frontendLockActive = false;
    this.recoveryReloadAuthorized = false;
  }
}
