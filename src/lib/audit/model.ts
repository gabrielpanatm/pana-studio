import type {
  AuditCategory,
  AuditImpact,
  AuditOutcome,
  AuditPolicy,
  AuditProviderReceipt,
  AuditProviderStatus,
  AuditRunReceipt,
  AuditSourceOrigin,
} from "$lib/audit/contracts";

export type AuditProviderStatusCounts = Record<AuditProviderStatus, number>;

export type AuditIdentity = {
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number | null;
};

export type AuditFindingFilter = {
  outcome?: AuditOutcome;
  impact?: AuditImpact;
  policy?: AuditPolicy;
  category?: AuditCategory;
  providerId?: string;
  origin?: AuditSourceOrigin;
  query?: string;
  locale?: string;
};

export function auditReceiptIsCurrent(
  receipt: AuditRunReceipt | null,
  identity: AuditIdentity,
) {
  return Boolean(
    receipt
    && identity.workspaceRevision !== null
    && receipt.projectRoot === identity.projectRoot.trim()
    && receipt.runtimeSessionId === identity.runtimeSessionId.trim()
    && receipt.workspaceRevision === identity.workspaceRevision,
  );
}

export function auditProviderStatusCounts(
  providers: readonly AuditProviderReceipt[],
): AuditProviderStatusCounts {
  const counts: AuditProviderStatusCounts = {
    complete: 0,
    partial: 0,
    failed: 0,
    skipped: 0,
  };
  for (const provider of providers) counts[provider.status] += 1;
  return counts;
}
