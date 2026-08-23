import { auditReceiptIsCurrent } from "$lib/audit/model";
import { t } from "$lib/i18n/runtime.svelte";
import type { PreviewStructuralSessionLease } from "$lib/kernel/preview-structural-lane";
import {
  applyAuditFix as applyAuditFixInRust,
  readProjectAudit,
} from "$lib/audit/io";
import type {
  AuditFinding,
  AuditRunMode,
  AuditRunReceipt,
} from "$lib/audit/contracts";
import type { AuditRefreshResult } from "$lib/deploy/contracts";
import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";
import {
  AUDIT_FIX_APPLY_SCHEMA_VERSION,
  AUDIT_RULESET_VERSION,
  AUDIT_RUN_SCHEMA_VERSION,
} from "$lib/audit/contracts";
import { errorMessage } from "$lib/util";

export type ProjectAuditAuthority = Readonly<{
  projectRoot: string;
  runtimeSessionId: string;
  workspace: ProjectWorkspaceSnapshot | null;
  activeRelativePath: string | null;
}>;

export type ProjectAuditWorkspaceCommands = {
  authority: () => ProjectAuditAuthority;
  runStructural: <T>(
    operation: (lease: PreviewStructuralSessionLease) => Promise<T>,
  ) => Promise<T | null>;
  requireStructuralLease: (lease: PreviewStructuralSessionLease) => void;
  settleMutation: (
    receipt: Parameters<typeof import("$lib/session/workspace-mutation-coordinator").settleProjectWorkspaceMutation>[1],
    options: Parameters<typeof import("$lib/session/workspace-mutation-coordinator").settleProjectWorkspaceMutation>[2],
  ) => Promise<unknown>;
  invalidatePublish: () => void;
  setStatus: (text: string, kind: "unsaved") => void;
};

export type ProjectAuditGateway = {
  read: typeof readProjectAudit;
  applyFix: typeof applyAuditFixInRust;
};

const rustGateway: ProjectAuditGateway = {
  read: readProjectAudit,
  applyFix: applyAuditFixInRust,
};

/** Owns audit receipts, request coalescing, view state and safe-fix authorization. */
export class ProjectAuditWorkspaceState {
  snapshot = $state<AuditRunReceipt | null>(null);
  loading = $state(false);
  error = $state("");
  view = $state<"overview" | "runtime">("overview");
  observabilityFocusSerial = $state(0);

  private requestSerial = 0;
  private requestKey = "";
  private request: Promise<AuditRefreshResult> | null = null;

  constructor(
    private readonly commands: ProjectAuditWorkspaceCommands,
    private readonly gateway: ProjectAuditGateway = rustGateway,
  ) {}

  current(receipt: AuditRunReceipt | null = this.snapshot) {
    const authority = this.commands.authority();
    return auditReceiptIsCurrent(receipt, {
      projectRoot: authority.projectRoot,
      runtimeSessionId: authority.runtimeSessionId,
      workspaceRevision: authority.workspace?.revision ?? null,
    }) ? receipt : null;
  }

  accept(receipt: AuditRunReceipt, clearError = false) {
    this.snapshot = receipt;
    if (clearError) this.error = "";
  }

  open(view: "overview" | "runtime", focusObservability = false) {
    this.view = view;
    if (focusObservability) this.observabilityFocusSerial += 1;
  }

  reset(options: { resetView?: boolean } = {}) {
    this.requestSerial += 1;
    this.requestKey = "";
    this.request = null;
    this.snapshot = null;
    this.loading = false;
    this.error = "";
    if (options.resetView) {
      this.view = "overview";
      this.observabilityFocusSerial = 0;
    }
  }

  async refresh(
    force = false,
    mode: AuditRunMode = "quick",
  ): Promise<AuditRefreshResult> {
    const authority = this.commands.authority();
    const projectRoot = authority.projectRoot.trim();
    const runtimeSessionId = authority.runtimeSessionId.trim();
    const workspaceRevision = authority.workspace?.revision ?? null;
    if (!projectRoot || !runtimeSessionId || workspaceRevision === null) {
      this.snapshot = null;
      this.error = "";
      return { ok: false, error: t("workbench-audit-session-mismatch"), stale: true };
    }

    const requestKey = `${projectRoot}\u0000${runtimeSessionId}\u0000${workspaceRevision}\u0000${mode}`;
    const current = this.snapshot;
    if (
      !force
      && current?.projectRoot === projectRoot
      && current.runtimeSessionId === runtimeSessionId
      && current.workspaceRevision === workspaceRevision
      && (current.mode === mode || current.mode === "full")
    ) return { ok: true, receipt: current };
    if (!force && this.request && this.requestKey === requestKey) return await this.request;

    const serial = ++this.requestSerial;
    this.requestKey = requestKey;
    this.loading = true;
    this.error = "";
    const request = (async () => {
      try {
        const snapshot = await this.gateway.read({
          mode,
          scope: { kind: "project" },
          policyOverrides: [],
          suppressions: [],
        });
        const latest = this.commands.authority();
        if (
          serial !== this.requestSerial
          || latest.projectRoot !== projectRoot
          || latest.runtimeSessionId !== runtimeSessionId
          || latest.workspace?.revision !== workspaceRevision
        ) return { ok: false, error: "", stale: true } as const;
        if (
          snapshot.projectRoot !== projectRoot
          || snapshot.runtimeSessionId !== runtimeSessionId
          || snapshot.workspaceRevision !== workspaceRevision
        ) throw new Error(t("workbench-audit-session-mismatch"));
        this.snapshot = snapshot;
        return { ok: true, receipt: snapshot } as const;
      } catch (error) {
        const message = errorMessage(error);
        if (serial !== this.requestSerial) {
          return { ok: false, error: message, stale: true } as const;
        }
        this.error = message;
        return { ok: false, error: message, stale: false } as const;
      } finally {
        if (serial === this.requestSerial) {
          this.loading = false;
          this.request = null;
          this.requestKey = "";
        }
      }
    })();
    this.request = request;
    return await request;
  }

  async applySafeFix(finding: AuditFinding, fixId: string): Promise<boolean> {
    const snapshot = this.current();
    if (
      !snapshot
      || !finding.fixes.some((fix) => fix.id === fixId && fix.applicability === "safe")
    ) throw new Error(t("audit-fix-stale"));

    const outcome = await this.commands.runStructural(async (lease) => {
      const authority = this.commands.authority();
      if (
        lease.projectRoot !== snapshot.projectRoot
        || lease.sessionId !== snapshot.runtimeSessionId
        || authority.workspace?.revision !== snapshot.workspaceRevision
      ) throw new Error(t("audit-fix-stale"));
      const receipt = await this.gateway.applyFix({
        schemaVersion: AUDIT_FIX_APPLY_SCHEMA_VERSION,
        expectedAuditSchemaVersion: AUDIT_RUN_SCHEMA_VERSION,
        expectedRulesetVersion: AUDIT_RULESET_VERSION,
        expectedProjectRoot: snapshot.projectRoot,
        expectedSessionId: snapshot.runtimeSessionId,
        expectedWorkspaceRevision: snapshot.workspaceRevision,
        expectedProjectModelRevision: snapshot.projectModelRevision,
        findingFingerprint: finding.fingerprint,
        fixId,
      });
      this.commands.requireStructuralLease(lease);
      await this.commands.settleMutation({
        projectRoot: receipt.workspace.projectRoot,
        runtimeSessionId: receipt.workspace.runtimeSessionId,
        mutation: receipt.mutation,
        workspace: receipt.workspace,
      }, {
        preferredRelativePath: finding.primaryLocation?.file
          ?? this.commands.authority().activeRelativePath,
        warningLabel: t("audit-fix-operation"),
      });
      this.commands.requireStructuralLease(lease);
      this.requestSerial += 1;
      this.request = null;
      this.requestKey = "";
      this.snapshot = receipt.audit;
      this.error = "";
      this.commands.invalidatePublish();
      this.commands.setStatus(t("audit-fix-applied"), "unsaved");
      return true;
    });
    return outcome ?? false;
  }
}
