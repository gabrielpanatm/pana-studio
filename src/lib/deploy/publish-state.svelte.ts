import {
  buildForPublish as buildForPublishInRust,
  currentPublishBuildReceipt as currentPublishBuildReceiptInRust,
  currentPublishPreflightReceipt as currentPublishPreflightReceiptInRust,
  runPublishPreflight as runPublishPreflightInRust,
} from "$lib/deploy/io";
import { t } from "$lib/i18n/runtime.svelte";
import type { AuditRunReceipt } from "$lib/audit/contracts";
import type {
  PublishBuildReceipt,
  PublishPreflightReceipt,
} from "$lib/deploy/contracts";
import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";

export type PublishAuthoritySnapshot = Readonly<{
  projectRoot: string;
  runtimeSessionId: string;
  workspace: ProjectWorkspaceSnapshot | null;
  workspaceDirty: boolean;
}>;

export type PublishWorkspaceCommands = {
  authority: () => PublishAuthoritySnapshot;
  acceptAudit: (receipt: AuditRunReceipt, clearError: boolean) => void;
};

/** Owns the short-lived authorization chain preflight -> build. */
export class PublishWorkspaceState {
  cachebustAssets = $state(false);
  preflight = $state<PublishPreflightReceipt | null>(null);
  build = $state<PublishBuildReceipt | null>(null);

  constructor(private readonly commands: PublishWorkspaceCommands) {}

  isPreflightCurrent(receipt: PublishPreflightReceipt | null = this.preflight) {
    const authority = this.commands.authority();
    return Boolean(
      receipt
      && authority.workspace
      && receipt.projectRoot === authority.projectRoot.trim()
      && receipt.runtimeSessionId === authority.runtimeSessionId.trim()
      && receipt.workspaceRevision === authority.workspace.revision
      && receipt.diskGeneration === authority.workspace.diskGeneration
      && receipt.workspaceDirty === authority.workspaceDirty,
    );
  }

  currentPreflight() {
    return this.isPreflightCurrent() ? this.preflight : null;
  }

  currentBuild() {
    const preflight = this.currentPreflight();
    const build = this.build;
    return Boolean(
      preflight
      && preflight.status === "ready"
      && build
      && build.projectRoot === preflight.projectRoot
      && build.runtimeSessionId === preflight.runtimeSessionId
      && build.workspaceRevision === preflight.workspaceRevision
      && build.diskGeneration === preflight.diskGeneration
      && build.projectModelRevision === preflight.projectModelRevision
      && build.deploySettingsRevision === preflight.deploySettingsRevision
      && build.deploySettingsFingerprint === preflight.deploySettingsFingerprint
      && build.preflightToken === preflight.preflightToken
      && build.targetId === preflight.activeTarget?.targetId
    ) ? build : null;
  }

  invalidate() {
    this.preflight = null;
    this.build = null;
  }

  async refreshAuthorization() {
    const preflight = await currentPublishPreflightReceiptInRust();
    this.preflight = preflight;
    if (preflight) this.commands.acceptAudit(preflight.auditReceipt, false);
    this.build = preflight?.status === "ready"
      ? await currentPublishBuildReceiptInRust()
      : null;
    return preflight;
  }

  async runPreflight() {
    const receipt = await runPublishPreflightInRust();
    if (!this.isPreflightCurrent(receipt)) {
      this.invalidate();
      throw new Error(t("publish-preflight-stale-result"));
    }
    this.preflight = receipt;
    this.build = null;
    this.commands.acceptAudit(receipt.auditReceipt, true);
    return receipt;
  }

  async buildForPublish() {
    const preflight = this.currentPreflight();
    if (!preflight || preflight.status !== "ready") {
      throw new Error(t("publish-build-requires-preflight"));
    }
    const receipt = await buildForPublishInRust(preflight.preflightToken);
    this.build = receipt;
    if (!this.currentBuild()) {
      this.build = null;
      throw new Error(t("publish-build-stale-result"));
    }
    return receipt;
  }
}
