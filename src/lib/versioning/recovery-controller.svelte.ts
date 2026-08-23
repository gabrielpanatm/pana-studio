import { t } from "$lib/i18n/runtime.svelte";
import type {
  VersionHistoryEntry,
  VersionRestoreRecoveryAction,
  VersionRestoreRecoveryItem,
  VersionRestoreRecoveryScan,
} from "$lib/versioning/contracts";
import {
  readVersionRestoreRecovery,
  resolveVersionRestoreRecovery,
  restoreVersioning,
} from "$lib/versioning/io";
import type { VersioningOperationState } from "$lib/versioning/panel-context.svelte";
import type {
  VersioningSnapshotController,
  VersioningSnapshotParticipant,
} from "$lib/versioning/snapshot-controller.svelte";

/** Owns restore confirmation and durable restore recovery workflows. */
export class VersioningRecoveryController {
  recovery = $state<VersionRestoreRecoveryScan | null>(null);
  restoreEntry = $state<VersionHistoryEntry | null>(null);
  restoreMessage = $state("");
  restoreConfirmation = $state("");

  constructor(
    readonly snapshot: VersioningSnapshotController,
    readonly operations: VersioningOperationState,
  ) {}

  participant(): VersioningSnapshotParticipant {
    return {
      reset: () => this.reset(),
      refresh: (serial) => this.refresh(serial),
    };
  }

  reset() {
    this.recovery = null;
    this.cancelRestore();
  }

  async refresh(parentSerial = this.snapshot.currentSerial()) {
    const identity = this.snapshot.readIdentity();
    if (!identity || this.snapshot.snapshot?.repositoryState !== "ready") {
      this.recovery = null;
      return;
    }
    const next = await readVersionRestoreRecovery(identity);
    if (this.snapshot.isCurrent(parentSerial)) this.recovery = next;
  }

  requestRestore(entry: VersionHistoryEntry) {
    if (entry.oid === this.snapshot.snapshot?.headOid) {
      this.operations.error = t("versions-already-current");
      return;
    }
    this.restoreEntry = entry;
    this.restoreMessage = t("versions-restore-message", {
      oid: entry.shortOid,
      subject: entry.subject,
    });
    this.restoreConfirmation = "";
    this.operations.error = "";
  }

  cancelRestore() {
    this.restoreEntry = null;
    this.restoreMessage = "";
    this.restoreConfirmation = "";
  }

  async restoreCommit() {
    const entry = this.restoreEntry;
    if (!entry) return;
    if (!this.snapshot.snapshot?.clean) {
      this.operations.error = t("versions-restore-clean-required");
      return;
    }
    if (this.operations.host.workspaceDirty()) {
      this.operations.requireMutationAllowed();
      return;
    }
    if (!this.restoreMessage.trim()) {
      this.operations.error = t("versions-restore-message-required");
      return;
    }
    if (this.restoreConfirmation.trim() !== entry.shortOid) {
      this.operations.error = t("versions-confirmation-exact", { value: entry.shortOid });
      return;
    }
    if (!this.operations.requireMutationAllowed()) return;
    this.operations.begin(`restore:${entry.oid}`);
    try {
      if (this.operations.host.activePreviewCommitOid()) {
        await this.operations.host.returnToLivePreview();
      }
      const receipt = await restoreVersioning(
        this.snapshot.mutationIdentity(),
        entry.oid,
        this.restoreMessage,
      );
      if (receipt.snapshot) this.snapshot.publishSnapshot(receipt.snapshot);
      if (!(await this.operations.settlePublishedEffect(
        t("versions-restore-terminal-backend"),
        () => this.operations.host.afterRestore(receipt),
      ))) return;
      if (receipt.status === "recovery_required") {
        await this.snapshot.refresh();
        this.operations.error = t("versions-restore-recovery-required");
        this.operations.host.onStatusUpdate(this.operations.error, "error");
        return;
      }
      await this.snapshot.refresh();
      this.cancelRestore();
      const diagnostic = receipt.diagnostic
        ? ` ${t("versions-technical-details-available")}`
        : "";
      this.operations.host.onStatusUpdate(
        receipt.status === "noop"
          ? t("versions-restore-noop-status", { oid: entry.shortOid, diagnostic })
          : t("versions-restored-status", { oid: entry.shortOid, diagnostic }),
        "restored",
      );
    } catch (reason) {
      const error = this.operations.fail(reason);
      this.operations.host.onStatusUpdate(
        t("versions-restore-blocked", { message: error }),
        "error",
      );
    } finally {
      this.operations.finish();
    }
  }

  recoveryActionLabel(action: VersionRestoreRecoveryAction) {
    if (action === "finalize") return t("versions-recovery-finalize-restore");
    if (action === "rollback") return t("versions-recovery-rollback");
    return t("versions-recovery-clear-marker");
  }

  async resolveRecovery(
    item: VersionRestoreRecoveryItem,
    action: VersionRestoreRecoveryAction,
  ) {
    if (!this.operations.requireMutationAllowed()) return;
    this.operations.begin(`recovery:${item.transactionId}:${action}`);
    try {
      const receipt = await resolveVersionRestoreRecovery(
        this.snapshot.mutationIdentity(),
        item.recoveryRef,
        action,
      );
      if (receipt.snapshot) this.snapshot.publishSnapshot(receipt.snapshot);
      if (!(await this.operations.settlePublishedEffect(
        t("versions-restore-recovery-backend"),
        () => this.operations.host.afterRecovery(receipt),
      ))) return;
      await this.snapshot.refresh();
      if (!receipt.resolved) {
        this.operations.error = t("versions-recovery-not-finished");
        this.operations.host.onStatusUpdate(this.operations.error, "error");
        return;
      }
      this.operations.host.onStatusUpdate(
        t("versions-recovery-resolved", {
          action: this.recoveryActionLabel(action),
        }),
        "restored",
      );
    } catch (reason) {
      const error = this.operations.fail(reason);
      this.operations.host.onStatusUpdate(
        t("versions-recovery-blocked", { message: error }),
        "error",
      );
    } finally {
      this.operations.finish();
    }
  }
}
