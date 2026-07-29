import type {
  KernelProjectTransitionDecisionRetentionHotJournal,
  KernelProjectTransitionDecisionRetentionHotJournalDiskState,
  KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
  RecoveryCoordinatorDiagnostic,
  RecoveryCoordinatorScan,
  RecoveryCoordinatorStatus,
  RecoveryJournalFamily,
  RecoveryJournalFamilyStatus,
  RecoveryJournalFamilySummary,
} from "$lib/types";
import { l10n, t } from "$lib/i18n/runtime.svelte";

export type RecoveryCoordinatorTone = "idle" | "clean" | "blocked" | "error";

export type RecoveryCoordinatorSummary = {
  tone: RecoveryCoordinatorTone;
  label: string;
  detail: string;
  blocked: boolean;
};

export function recoveryCoordinatorSummary(
  scan: RecoveryCoordinatorScan | null,
): RecoveryCoordinatorSummary {
  if (!scan) {
    return {
      tone: "idle",
      label: t("project-recovery-unavailable"),
      detail: t("project-recovery-unavailable-detail"),
      blocked: false,
    };
  }
  if (scan.status === "clean") {
    return {
      tone: "clean",
      label: t("project-recovery-clean"),
      detail: t("project-recovery-clean-detail"),
      blocked: false,
    };
  }
  const journalCount = scan.hotJournalFamilies.reduce((total, family) => total + family.count, 0);
  if (scan.status === "unreadable") {
    return {
      tone: "error",
      label: t("project-recovery-unreadable"),
      detail: t("project-recovery-unreadable-detail", {
        count: scan.diagnostics.length,
      }),
      blocked: true,
    };
  }
  return {
    tone: "blocked",
    label: t("project-recovery-required"),
    detail: t("project-recovery-required-detail", {
      journals: journalCount,
      families: scan.hotJournalFamilies.length,
    }),
    blocked: true,
  };
}

export function recoveryCoordinatorStatusLabel(status: RecoveryCoordinatorStatus): string {
  if (status === "clean") return t("project-recovery-status-clean");
  if (status === "needs_attention") return t("project-recovery-status-attention");
  return t("project-recovery-status-unreadable");
}

export function recoveryJournalFamilyLabel(family: RecoveryJournalFamily): string {
  return family === "project_workspace_save"
    ? t("project-recovery-family-workspace-save")
    : t("project-recovery-family-transition-retention");
}

export function recoveryJournalFamilyStatusLabel(status: RecoveryJournalFamilyStatus): string {
  return status === "needs_attention"
    ? t("project-recovery-status-attention")
    : t("project-recovery-status-manual");
}

export function recoveryJournalFamilyActionLabel(summary: RecoveryJournalFamilySummary): string {
  const parts = [
    summary.clearableCount
      ? t("project-recovery-action-clear", { count: summary.clearableCount })
      : "",
    summary.rollbackCount
      ? t("project-recovery-action-rollback", { count: summary.rollbackCount })
      : "",
    summary.restoreCount
      ? t("project-recovery-action-restore", { count: summary.restoreCount })
      : "",
    summary.manualReviewCount
      ? t("project-recovery-action-manual", { count: summary.manualReviewCount })
      : "",
  ].filter(Boolean);
  return parts.length ? parts.join(" · ") : t("project-recovery-no-automatic-action");
}

export function recoveryJournalFamilyStateLabel(summary: RecoveryJournalFamilySummary): string {
  return summary.stateCounts.length
    ? summary.stateCounts.map((item) => `${item.value}: ${item.count}`).join(" · ")
    : t("project-recovery-no-states");
}

export function recoverySeverityLabel(
  severity: RecoveryCoordinatorDiagnostic["severity"],
): string {
  return severity === "warning"
    ? t("project-recovery-severity-warning")
    : t("project-recovery-severity-error");
}

export function normalizeRecoveryDiagnostic(value: string): string {
  return value.trim().replace(/\s+/g, " ");
}

export function recoveryDiagnosticIsActionable(value: string): boolean {
  return normalizeRecoveryDiagnostic(value).length >= 12;
}

export function formatRecoveryTime(timestampMs: number | null | undefined): string {
  if (!timestampMs) return t("project-recovery-time-unknown");
  return l10n.formatDate(new Date(timestampMs), {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

export function compactKernelPath(path: string, maxLength = 72): string {
  if (path.length <= maxLength) return path;
  const separator = path.includes("\\") ? "\\" : "/";
  const parts = path.split(/[\\/]/).filter(Boolean);
  if (parts.length <= 2) return `...${path.slice(-(maxLength - 3))}`;
  const tail: string[] = [];
  let length = 3;
  for (let index = parts.length - 1; index >= 0; index -= 1) {
    const nextLength = length + parts[index].length + separator.length;
    if (nextLength > maxLength) break;
    tail.unshift(parts[index]);
    length = nextLength;
  }
  return `...${separator}${tail.join(separator)}`;
}

export function shortHash(value: string | null | undefined): string {
  if (!value) return "—";
  return value.length > 12 ? `${value.slice(0, 12)}…` : value;
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${l10n.formatNumber(bytes)} B`;
  if (bytes < 1024 * 1024) {
    return `${l10n.formatNumber(bytes / 1024, { maximumFractionDigits: 1 })} KiB`;
  }
  return `${l10n.formatNumber(bytes / (1024 * 1024), {
    maximumFractionDigits: 1,
  })} MiB`;
}

export function projectTransitionDecisionRetentionCandidateIdsLabel(
  journal: KernelProjectTransitionDecisionRetentionHotJournal,
): string {
  if (!journal.candidateRecordIds.length) return t("project-recovery-no-candidates");
  if (journal.candidateRecordIds.length === 1) return journal.candidateRecordIds[0];
  const visible = journal.candidateRecordIds.slice(0, 3).join(", ");
  const hidden = journal.candidateRecordIds.length - 3;
  return hidden > 0
    ? t("project-recovery-more-candidates", { visible, count: hidden })
    : visible;
}

export function projectTransitionRetentionStateLabel(
  state: KernelProjectTransitionDecisionRetentionHotJournalDiskState,
): string {
  switch (state) {
    case "no_effect": return t("project-recovery-retention-no-effect");
    case "completed_retention": return t("project-recovery-retention-completed");
    case "partial_retention": return t("project-recovery-retention-partial");
    case "conflict_state": return t("project-recovery-retention-conflict");
  }
}

export function projectTransitionRetentionActionLabel(
  action: KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
): string {
  switch (action) {
    case "clear_no_effect_journal":
      return t("project-recovery-retention-clear-no-effect");
    case "clear_completed_journal":
      return t("project-recovery-retention-clear-completed");
    case "restore_before_journal":
      return t("project-recovery-retention-restore-before");
    case "manual_review_conflict":
      return t("project-recovery-retention-manual-conflict");
  }
}
