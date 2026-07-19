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

export type RecoveryCoordinatorTone = "idle" | "clean" | "blocked" | "error";

export type RecoveryCoordinatorSummary = {
  tone: RecoveryCoordinatorTone;
  label: string;
  detail: string;
  blocked: boolean;
};

const statusLabels: Record<RecoveryCoordinatorStatus, string> = {
  clean: "curat",
  needs_attention: "necesită atenție",
  unreadable: "necitibil",
};

const familyLabels: Record<RecoveryJournalFamily, string> = {
  project_workspace_save: "ProjectWorkspace Save",
  project_transition_decision_retention: "ProjectTransition Decision Retention",
};

const familyStatusLabels: Record<RecoveryJournalFamilyStatus, string> = {
  needs_attention: "necesită atenție",
  manual_review_required: "review manual obligatoriu",
};

const severityLabels: Record<RecoveryCoordinatorDiagnostic["severity"], string> = {
  warning: "avertisment",
  error: "eroare",
};

export function recoveryCoordinatorSummary(
  scan: RecoveryCoordinatorScan | null,
): RecoveryCoordinatorSummary {
  if (!scan) {
    return {
      tone: "idle",
      label: "Recovery Coordinator indisponibil",
      detail: "Deschide un proiect valid pentru scanarea jurnalelor active.",
      blocked: false,
    };
  }
  if (scan.status === "clean") {
    return {
      tone: "clean",
      label: "Recovery Coordinator curat",
      detail: "Nu există operații pe disk întrerupte care necesită intervenție.",
      blocked: false,
    };
  }
  const journalCount = scan.hotJournalFamilies.reduce((total, family) => total + family.count, 0);
  if (scan.status === "unreadable") {
    return {
      tone: "error",
      label: "Recovery Coordinator necitibil",
      detail: `${scan.diagnostics.length} diagnostice; mutațiile pe disk rămân blocate până la clarificare.`,
      blocked: true,
    };
  }
  return {
    tone: "blocked",
    label: "Recovery necesar",
    detail: `${journalCount} jurnale active în ${scan.hotJournalFamilies.length} familii.`,
    blocked: true,
  };
}

export function recoveryCoordinatorStatusLabel(status: RecoveryCoordinatorStatus): string {
  return statusLabels[status];
}

export function recoveryJournalFamilyLabel(family: RecoveryJournalFamily): string {
  return familyLabels[family];
}

export function recoveryJournalFamilyStatusLabel(status: RecoveryJournalFamilyStatus): string {
  return familyStatusLabels[status];
}

export function recoveryJournalFamilyActionLabel(summary: RecoveryJournalFamilySummary): string {
  const parts = [
    summary.clearableCount ? `${summary.clearableCount} curățare` : "",
    summary.rollbackCount ? `${summary.rollbackCount} rollback` : "",
    summary.restoreCount ? `${summary.restoreCount} restaurare` : "",
    summary.manualReviewCount ? `${summary.manualReviewCount} manual` : "",
  ].filter(Boolean);
  return parts.length ? parts.join(" · ") : "fără acțiune automată";
}

export function recoveryJournalFamilyStateLabel(summary: RecoveryJournalFamilySummary): string {
  return summary.stateCounts.length
    ? summary.stateCounts.map((item) => `${item.value}: ${item.count}`).join(" · ")
    : "fără stări";
}

export function recoverySeverityLabel(
  severity: RecoveryCoordinatorDiagnostic["severity"],
): string {
  return severityLabels[severity];
}

export function normalizeRecoveryDiagnostic(value: string): string {
  return value.trim().replace(/\s+/g, " ");
}

export function recoveryDiagnosticIsActionable(value: string): boolean {
  return normalizeRecoveryDiagnostic(value).length >= 12;
}

export function formatRecoveryTime(timestampMs: number | null | undefined): string {
  if (!timestampMs) return "timp necunoscut";
  return new Intl.DateTimeFormat("ro-RO", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(new Date(timestampMs));
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
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
}

export function projectTransitionDecisionRetentionCandidateIdsLabel(
  journal: KernelProjectTransitionDecisionRetentionHotJournal,
): string {
  if (!journal.candidateRecordIds.length) return "fără candidați declarați";
  if (journal.candidateRecordIds.length === 1) return journal.candidateRecordIds[0];
  const visible = journal.candidateRecordIds.slice(0, 3).join(", ");
  const hidden = journal.candidateRecordIds.length - 3;
  return hidden > 0 ? `${visible} și încă ${hidden}` : visible;
}

export function projectTransitionRetentionStateLabel(
  state: KernelProjectTransitionDecisionRetentionHotJournalDiskState,
): string {
  const labels: Record<KernelProjectTransitionDecisionRetentionHotJournalDiskState, string> = {
    no_effect: "fără efect",
    completed_retention: "retention finalizat",
    partial_retention: "retention parțial",
    conflict_state: "conflict",
  };
  return labels[state];
}

export function projectTransitionRetentionActionLabel(
  action: KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
): string {
  const labels: Record<KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction, string> = {
    clear_no_effect_journal: "curăță jurnalul fără efect",
    clear_completed_journal: "curăță jurnalul finalizat",
    restore_before_journal: "restaurează jurnalul anterior",
    manual_review_conflict: "review manual al conflictului",
  };
  return labels[action];
}
