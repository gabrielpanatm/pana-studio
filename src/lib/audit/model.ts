import type {
  AuditCategory,
  AuditFinding,
  AuditImpact,
  AuditOutcome,
  AuditPolicy,
  AuditProviderReceipt,
  AuditProviderStatus,
  AuditRunReceipt,
  AuditSourceOrigin,
} from "$lib/types";

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

export function filterAuditFindings(
  findings: readonly AuditFinding[],
  filter: AuditFindingFilter,
) {
  const locale = filter.locale || undefined;
  const query = filter.query?.trim().toLocaleLowerCase(locale) ?? "";
  return findings.filter((finding) => {
    if (filter.outcome && finding.outcome !== filter.outcome) return false;
    if (filter.impact && finding.impact !== filter.impact) return false;
    if (filter.policy && finding.policy !== filter.policy) return false;
    if (filter.category && finding.category !== filter.category) return false;
    if (filter.providerId && finding.providerId !== filter.providerId) return false;
    if (filter.origin && finding.primaryLocation?.origin !== filter.origin) return false;
    if (!query) return true;
    const searchable = [
      finding.ruleCode,
      finding.providerId,
      finding.titleDiagnostic.code,
      finding.messageDiagnostic.code,
      finding.primaryLocation?.file ?? "",
    ];
    return searchable.some((value) => value.toLocaleLowerCase(locale).includes(query));
  });
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
