import { t } from "$lib/i18n/runtime.svelte";
import type {
  VersionIntegrationMode,
  VersionIntegrationPlan,
  VersionIntegrationRecoveryAction,
  VersionIntegrationRecoveryItem,
  VersionIntegrationRecoveryScan,
  VersionDiffReceipt,
} from "$lib/versioning/contracts";
import {
  clearVersionUpstream,
  configureVersionUpstream,
  createVersionBranch,
  deleteVersionBranch,
  integrateVersionTarget,
  readVersionDiff,
  readVersionIntegrationPlan,
  readVersionIntegrationRecovery,
  resolveVersionIntegrationRecovery,
  switchVersionBranch,
} from "$lib/versioning/io";
import type { VersioningNetworkController } from "$lib/versioning/network-controller.svelte";
import type { VersioningOperationState } from "$lib/versioning/panel-context.svelte";
import type {
  VersioningSnapshotController,
  VersioningSnapshotParticipant,
} from "$lib/versioning/snapshot-controller.svelte";

/** Owns branches, upstream selection, integration planning and integration recovery. */
export class VersioningIntegrationController {
  recovery = $state<VersionIntegrationRecoveryScan | null>(null);
  plan = $state<VersionIntegrationPlan | null>(null);
  diff = $state<VersionDiffReceipt | null>(null);
  message = $state("");
  newBranchName = $state("");
  pendingBranchRemoval = $state("");
  branchRemovalConfirmation = $state("");

  private network: VersioningNetworkController | null = null;

  constructor(
    readonly snapshot: VersioningSnapshotController,
    readonly operations: VersioningOperationState,
  ) {}

  bindNetwork(network: VersioningNetworkController) {
    this.network = network;
  }

  participant(): VersioningSnapshotParticipant {
    return {
      reset: () => this.reset(),
      beforeRefresh: (keepDiff) => {
        if (!keepDiff) this.clearPlan();
      },
      refresh: (serial) => this.refreshRecovery(serial),
      refreshAfterRepositoryMutation: (serial) => this.refreshRecovery(serial),
    };
  }

  reset() {
    this.recovery = null;
    this.clearPlan();
    this.message = "";
    this.newBranchName = "";
    this.pendingBranchRemoval = "";
    this.branchRemovalConfirmation = "";
  }

  clearPlan() {
    this.plan = null;
    this.diff = null;
  }

  selectionChanged(remote: string, remoteBranch: string) {
    const defaultMessage = t("versions-default-integration-message");
    if (this.message.trim() && this.message !== defaultMessage) return;
    this.message = remoteBranch
      ? t("versions-integration-message", { remote, branch: remoteBranch })
      : defaultMessage;
  }

  async refreshRecovery(parentSerial = this.snapshot.currentSerial()) {
    const identity = this.snapshot.readIdentity();
    if (!identity || this.snapshot.snapshot?.repositoryState !== "ready") {
      this.recovery = null;
      return;
    }
    const next = await readVersionIntegrationRecovery(identity);
    if (this.snapshot.isCurrent(parentSerial)) this.recovery = next;
  }

  async saveUpstream() {
    const snapshot = this.snapshot.snapshot;
    const network = this.requireNetwork();
    if (!snapshot?.branch || !network.selectedRemote || !network.selectedRemoteBranch) {
      this.operations.error = t("versions-upstream-required");
      return;
    }
    await this.snapshot.runSnapshotMutation(
      t("versions-upstream-saved"),
      () => configureVersionUpstream(this.snapshot.mutationIdentity(), {
        localBranch: snapshot.branch!,
        remote: network.selectedRemote,
        remoteBranch: network.selectedRemoteBranch,
      }),
    );
  }

  async removeUpstream() {
    const branch = this.snapshot.snapshot?.branch;
    if (!branch) return;
    await this.snapshot.runSnapshotMutation(
      t("versions-upstream-removed"),
      () => clearVersionUpstream(this.snapshot.mutationIdentity(), branch),
    );
  }

  async createBranch() {
    const name = this.newBranchName.trim();
    if (!name) {
      this.operations.error = t("versions-branch-name-required");
      return;
    }
    await this.snapshot.runSnapshotMutation(
      t("versions-branch-created", { name }),
      () => createVersionBranch(this.snapshot.mutationIdentity(), name),
    );
    this.newBranchName = "";
  }

  requestBranchRemoval(branch: string) {
    this.pendingBranchRemoval = branch;
    this.branchRemovalConfirmation = "";
  }

  cancelBranchRemoval() {
    this.pendingBranchRemoval = "";
    this.branchRemovalConfirmation = "";
  }

  async switchBranch(branch: string, oid: string | null) {
    if (!oid || !this.snapshot.snapshot?.clean) {
      this.operations.error = t("versions-switch-clean-required");
      return;
    }
    if (!this.operations.requireMutationAllowed()) return;
    this.operations.begin(`switch:${branch}`);
    try {
      const receipt = await switchVersionBranch(this.snapshot.mutationIdentity(), branch, oid);
      if (receipt.snapshot) this.snapshot.publishSnapshot(receipt.snapshot);
      if (!(await this.operations.settlePublishedEffect(
        t("versions-branch-switched-backend", { branch }),
        () => this.operations.host.afterIntegration(receipt),
      ))) return;
      await this.snapshot.refresh();
      this.operations.host.onStatusUpdate(
        t("versions-active-branch", { branch }),
        "restored",
      );
    } catch (reason) {
      const error = this.operations.fail(reason);
      this.operations.host.onStatusUpdate(
        t("versions-switch-blocked", { message: error }),
        "error",
      );
    } finally {
      this.operations.finish();
    }
  }

  async deleteBranch(branch: string) {
    if (this.branchRemovalConfirmation !== branch) {
      this.operations.error = t("versions-delete-branch-confirmation", { branch });
      return;
    }
    await this.snapshot.runSnapshotMutation(
      t("versions-branch-removed", { branch }),
      () => deleteVersionBranch(this.snapshot.mutationIdentity(), branch),
    );
    this.pendingBranchRemoval = "";
    this.branchRemovalConfirmation = "";
  }

  selectedTarget() {
    const network = this.requireNetwork();
    return this.snapshot.snapshot?.remoteBranches.find(
      (branch) => branch.remote === network.selectedRemote
        && branch.name === network.selectedRemoteBranch,
    ) ?? null;
  }

  async analyzeIntegration() {
    const identity = this.snapshot.readIdentity();
    const target = this.selectedTarget();
    if (!identity || !target) {
      this.operations.error = t("versions-integration-target-required");
      return;
    }
    this.operations.begin(`plan:${target.refName}`);
    try {
      const [plan, previewDiff] = await Promise.all([
        readVersionIntegrationPlan(identity, target.refName, target.oid),
        readVersionDiff(identity, {
          kind: "integration",
          targetRef: target.refName,
          expectedTargetOid: target.oid,
        }),
      ]);
      this.plan = plan;
      this.diff = previewDiff;
      this.message = t("versions-integration-message", {
        remote: target.remote,
        branch: target.name,
      });
    } catch (reason) {
      this.operations.fail(reason);
      this.clearPlan();
    } finally {
      this.operations.finish();
    }
  }

  async applyIntegration(mode: VersionIntegrationMode) {
    const plan = this.plan;
    if (!plan || !this.message.trim()) return;
    if (!this.operations.requireMutationAllowed()) return;
    this.operations.begin(`integrate:${mode}`);
    try {
      if (this.operations.host.activePreviewCommitOid()) {
        await this.operations.host.returnToLivePreview();
      }
      const receipt = await integrateVersionTarget(this.snapshot.mutationIdentity(), {
        targetRef: plan.targetRef,
        expectedTargetOid: plan.targetOid,
        mode,
        message: this.message.trim(),
      });
      if (receipt.snapshot) this.snapshot.publishSnapshot(receipt.snapshot);
      this.clearPlan();
      if (!(await this.operations.settlePublishedEffect(
        t("versions-integration-backend"),
        () => this.operations.host.afterIntegration(receipt),
      ))) return;
      await this.snapshot.refresh();
      if (receipt.status === "conflict_resolution_required") {
        this.operations.host.onStatusUpdate(
          t("versions-conflicts-count", { count: receipt.conflictPaths.length }),
          "error",
        );
      } else if (receipt.status === "recovery_required") {
        this.operations.error = t("versions-integration-recovery-required");
        this.operations.host.onStatusUpdate(this.operations.error, "error");
      } else {
        this.operations.host.onStatusUpdate(
          receipt.status === "noop"
            ? t("versions-target-already-integrated")
            : t("versions-integration-published"),
          "restored",
        );
      }
    } catch (reason) {
      const error = this.operations.fail(reason);
      this.operations.host.onStatusUpdate(
        t("versions-integration-blocked", { message: error }),
        "error",
      );
    } finally {
      this.operations.finish();
    }
  }

  recoveryActionLabel(action: VersionIntegrationRecoveryAction) {
    if (action === "finalize") return t("versions-recovery-finalize-integration");
    if (action === "continue") return t("versions-recovery-continue-merge");
    if (action === "rollback") return t("versions-recovery-cancel-return");
    return t("versions-recovery-clear-marker");
  }

  async resolveRecovery(
    item: VersionIntegrationRecoveryItem,
    action: VersionIntegrationRecoveryAction,
  ) {
    if (!this.operations.requireMutationAllowed()) return;
    this.operations.begin(`integration-recovery:${item.transactionId}:${action}`);
    try {
      const receipt = await resolveVersionIntegrationRecovery(
        this.snapshot.mutationIdentity(),
        item.recoveryRef,
        action,
      );
      if (receipt.snapshot) this.snapshot.publishSnapshot(receipt.snapshot);
      if (!(await this.operations.settlePublishedEffect(
        t("versions-integration-recovery-backend"),
        () => this.operations.host.afterIntegrationRecovery(receipt),
      ))) return;
      await this.snapshot.refresh();
      if (!receipt.resolved) {
        this.operations.error = t("versions-integration-still-recovery");
        this.operations.host.onStatusUpdate(this.operations.error, "error");
      } else {
        this.operations.host.onStatusUpdate(
          t("versions-integration-recovery-status", {
            action: this.recoveryActionLabel(action),
          }),
          "restored",
        );
      }
    } catch (reason) {
      const error = this.operations.fail(reason);
      this.operations.host.onStatusUpdate(
        t("versions-integration-recovery-blocked", { message: error }),
        "error",
      );
    } finally {
      this.operations.finish();
    }
  }

  private requireNetwork() {
    if (!this.network) throw new Error("VersioningNetworkController nu este conectat.");
    return this.network;
  }
}
