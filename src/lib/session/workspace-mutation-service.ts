import {
  projectLatestProjectWorkspacePreview,
  type ProjectWorkspacePreviewProjectionOptions,
} from "$lib/kernel/project-workspace-preview-coordinator";
import {
  settleProjectWorkspaceMutation,
  type WorkspaceDerivedReconciliationOutcome,
  type WorkspaceMutationAuthorityReceipt,
  type WorkspaceMutationSettlementHost,
  type WorkspaceMutationSettlementOptions,
} from "$lib/session/workspace-mutation-coordinator";
import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";

/** Narrow command surface for UI mutations that must settle through Rust authority. */
export class ProjectWorkspaceMutationService {
  private readonly host: WorkspaceMutationSettlementHost;

  constructor(host: WorkspaceMutationSettlementHost) {
    this.host = host;
  }

  get snapshot() {
    return this.host.projectWorkspaceSnapshot;
  }

  get identity() {
    const snapshot = this.host.projectWorkspaceSnapshot;
    if (!snapshot) return null;
    return {
      expectedProjectRoot: snapshot.projectRoot,
      expectedSessionId: snapshot.runtimeSessionId,
      expectedRevision: snapshot.revision,
    } as const;
  }

  settle(
    receipt: WorkspaceMutationAuthorityReceipt,
    options: WorkspaceMutationSettlementOptions = {},
  ) {
    return settleProjectWorkspaceMutation(this.host, receipt, options);
  }

  publishSnapshot(snapshot: ProjectWorkspaceSnapshot) {
    if (
      snapshot.projectRoot !== this.host.sessionProjectRoot
      || snapshot.runtimeSessionId !== this.host.kernelProjectSessionId
    ) return false;
    const current = this.host.projectWorkspaceSnapshot;
    if (
      current?.projectRoot === snapshot.projectRoot
      && current.runtimeSessionId === snapshot.runtimeSessionId
      && current.revision > snapshot.revision
    ) return false;
    this.host.projectWorkspaceSnapshot = snapshot;
    return true;
  }

  reconcile(options: Parameters<WorkspaceMutationSettlementHost["reconcileWorkspaceDerivedState"]>[0]): Promise<WorkspaceDerivedReconciliationOutcome> {
    return this.host.reconcileWorkspaceDerivedState(options);
  }

  projectPreview(options: ProjectWorkspacePreviewProjectionOptions) {
    return projectLatestProjectWorkspacePreview(this.host, options);
  }
}
