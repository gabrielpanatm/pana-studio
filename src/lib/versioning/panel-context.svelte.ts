import { t } from "$lib/i18n/runtime.svelte";
import type { GlobalStatusKind } from "$lib/status/global-status";
import type {
  VersionIntegrationReceipt,
  VersionIntegrationRecoveryResolutionReceipt,
  VersionPreviewReceipt,
  VersionRestoreReceipt,
  VersionRestoreRecoveryResolutionReceipt,
} from "$lib/versioning/contracts";
import {
  beginVersioningOperation,
  failVersioningOperation,
  finishVersioningOperation,
} from "$lib/versioning/operation-lifecycle";
import { settleVersioningPublication } from "$lib/versioning/publication-settlement";

export type VersioningPanelHost = Readonly<{
  projectRoot: () => string;
  sessionId: () => string;
  workspaceDirty: () => boolean;
  activePreviewCommitOid: () => string | null;
  onStatusUpdate: (text: string, kind: GlobalStatusKind) => void;
  showPreview: (receipt: VersionPreviewReceipt) => void | Promise<void>;
  returnToLivePreview: () => void | Promise<void>;
  afterRestore: (receipt: VersionRestoreReceipt) => void | Promise<void>;
  afterRecovery: (receipt: VersionRestoreRecoveryResolutionReceipt) => void | Promise<void>;
  afterIntegration: (receipt: VersionIntegrationReceipt) => void | Promise<void>;
  afterIntegrationRecovery: (
    receipt: VersionIntegrationRecoveryResolutionReceipt,
  ) => void | Promise<void>;
}>;

/** Shared UI-only operation state. Rust remains the mutation authority. */
export class VersioningOperationState {
  busyAction = $state("");
  error = $state("");
  private mutationBlocker: () => string = () => "";

  constructor(readonly host: VersioningPanelHost) {}

  setMutationBlocker(blocker: () => string) {
    this.mutationBlocker = blocker;
  }

  requireMutationAllowed() {
    const reason = this.mutationBlocker();
    if (!reason) return true;
    this.error = reason;
    return false;
  }

  begin(action: string) {
    const next = beginVersioningOperation(action);
    this.busyAction = next.busyAction;
    this.error = next.error;
  }

  finish() {
    const next = finishVersioningOperation(this);
    this.busyAction = next.busyAction;
    this.error = next.error;
  }

  errorMessage(value: unknown) {
    return value instanceof Error ? value.message : String(value);
  }

  fail(reason: unknown) {
    const next = failVersioningOperation(this, this.errorMessage(reason));
    this.busyAction = next.busyAction;
    this.error = next.error;
    return this.error;
  }

  async settlePublishedEffect(
    label: string,
    projection: () => void | Promise<void>,
  ) {
    const settlement = await settleVersioningPublication(projection);
    if (settlement.ok) return true;
    this.error = t("versions-projection-failed", {
      label,
      message: this.errorMessage(settlement.error),
    });
    this.host.onStatusUpdate(this.error, "error");
    return false;
  }
}
