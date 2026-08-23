import {
  AUDIT_FIX_APPLY_SCHEMA_VERSION,
  AUDIT_RULESET_VERSION,
  AUDIT_RUN_SCHEMA_VERSION,
  type AuditFixApplyInput,
  type AuditFixApplyReceipt,
  type AuditRequest,
  type AuditRunReceipt,
} from "$lib/audit/contracts";
import { invoke } from "@tauri-apps/api/core";
import { t } from "$lib/i18n/runtime.svelte";
import { schemaMismatch } from "$lib/contracts/io-schema";

export async function readProjectAudit(request: AuditRequest): Promise<AuditRunReceipt> {
  const receipt = await invoke<AuditRunReceipt>("read_project_audit", { request });
  if (receipt.schemaVersion !== AUDIT_RUN_SCHEMA_VERSION) {
    throw schemaMismatch(
      t("io-resource-project-audit"),
      receipt.schemaVersion,
      AUDIT_RUN_SCHEMA_VERSION,
    );
  }
  if (
    receipt.rulesetVersion !== AUDIT_RULESET_VERSION
    || !Array.isArray(receipt.findings)
    || !Array.isArray(receipt.providers)
    || !receipt.findings.every((finding) => (
      typeof finding.id === "string"
      && finding.fingerprint.startsWith("sha256:")
      && typeof finding.providerId === "string"
      && typeof finding.ruleCode === "string"
    ))
    || !receipt.providers.every((provider) => (
      typeof provider.id === "string"
      && ["complete", "partial", "failed", "skipped"].includes(provider.status)
      && ["required", "advisory"].includes(provider.publishCoverageRequirement)
      && Number.isSafeInteger(provider.coverage.eligible)
      && Number.isSafeInteger(provider.coverage.analyzed)
    ))
  ) {
    throw new Error(t("io-audit-receipt-invalid"));
  }
  return receipt;
}

export async function applyAuditFix(
  input: AuditFixApplyInput,
): Promise<AuditFixApplyReceipt> {
  const receipt = await invoke<AuditFixApplyReceipt>("apply_audit_fix", { input });
  if (
    receipt.schemaVersion !== AUDIT_FIX_APPLY_SCHEMA_VERSION
    || receipt.findingFingerprint !== input.findingFingerprint
    || receipt.fixId !== input.fixId
    || receipt.audit.schemaVersion !== AUDIT_RUN_SCHEMA_VERSION
    || receipt.audit.rulesetVersion !== AUDIT_RULESET_VERSION
    || receipt.audit.projectRoot !== input.expectedProjectRoot
    || receipt.audit.runtimeSessionId !== input.expectedSessionId
    || receipt.audit.workspaceRevision !== receipt.mutation.revisionAfter
    || receipt.workspace.projectRoot !== input.expectedProjectRoot
    || receipt.workspace.runtimeSessionId !== input.expectedSessionId
    || receipt.workspace.revision !== receipt.mutation.revisionAfter
  ) {
    throw new Error(t("io-audit-fix-receipt-invalid"));
  }
  return receipt;
}
