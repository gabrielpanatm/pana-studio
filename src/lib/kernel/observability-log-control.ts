import type {
  JsonValue,
  KernelLogLevel,
  KernelObservabilityHealthSnapshot,
  KernelObservabilityHealthStatus,
  KernelObservabilityModuleHealthSnapshot,
  KernelObservabilityLogEvent,
  KernelObservabilityLogSnapshot,
  KernelObservabilityLogSourceFilter,
} from "$lib/types";
import { compactKernelPath } from "$lib/kernel/recovery-control";
import { l10n, t } from "$lib/i18n/runtime.svelte";

export type ObservabilitySummaryTone = "idle" | "clean" | "warning" | "error";

export type ObservabilitySummary = {
  tone: ObservabilitySummaryTone;
  label: string;
  detail: string;
};

export const kernelObservabilityEventLimitOptions = [40, 80, 120, 200] as const;

export type KernelObservabilityEventLimit = (typeof kernelObservabilityEventLimitOptions)[number];

const levelLabels: Record<KernelLogLevel, string> = {
  info: "INFO",
  warn: "WARN",
  error: "ERROR",
};

export function observabilitySummary(snapshot: KernelObservabilityLogSnapshot | null): ObservabilitySummary {
  if (!snapshot) {
    return {
      tone: "idle",
      label: t("observability-unavailable-title"),
      detail: t("observability-unavailable-detail"),
    };
  }

  if (!snapshot.logExists) {
    return {
      tone: "warning",
      label: t("observability-log-missing-title"),
      detail: t("observability-log-missing-detail"),
    };
  }

  if (snapshot.unreadableCount > 0) {
    return {
      tone: "error",
      label: t("observability-diagnostics-title"),
      detail: t("observability-diagnostics-detail", {
        returned: snapshot.returnedCount,
        unreadable: snapshot.unreadableCount,
      }),
    };
  }

  return {
    tone: snapshot.truncated ? "warning" : "clean",
    label: snapshot.recoveryOnly
      ? t("observability-recovery-events")
      : t("observability-kernel-events"),
    detail: t("observability-summary-detail", {
      returned: snapshot.returnedCount,
      scanned: snapshot.scannedLineCount,
      levels: kernelLogLevelFilterLabel(snapshot.levels),
      source: kernelLogSourceFilterLabel(snapshot.sourceFilter),
    }),
  };
}

export function kernelLogLevelLabel(level: KernelLogLevel): string {
  return levelLabels[level] ?? level.toUpperCase();
}

export function kernelLogLevelFilterLabel(levels: KernelLogLevel[]): string {
  if (!levels.length) return t("observability-no-level");
  const unique = ["info", "warn", "error"].filter((level) =>
    levels.includes(level as KernelLogLevel),
  ) as KernelLogLevel[];
  if (unique.length === 3) return t("observability-all-levels");
  return unique.map(kernelLogLevelLabel).join(", ");
}

export function kernelLogLevelTone(level: KernelLogLevel): "info" | "warn" | "error" {
  return level === "error" ? "error" : level === "warn" ? "warn" : "info";
}

export function kernelObservabilityEventLimitLabel(limit: number): string {
  return t("observability-event-limit", { count: limit });
}

export function formatKernelLogTime(timestampMs: number): string {
  if (!timestampMs) return t("observability-time-unknown");
  return l10n.formatDate(timestampMs, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

export function kernelLogTargetLabel(event: KernelObservabilityLogEvent): string {
  return event.target ? compactKernelPath(event.target, 72) : t("observability-no-target");
}

export function kernelLogSourceLabel(event: KernelObservabilityLogEvent): string {
  return event.source?.label ?? t("observability-source-unknown");
}

export function kernelLogSourceFilterLabel(sourceFilter: KernelObservabilityLogSourceFilter): string {
  if (sourceFilter === "active") return t("observability-source-active");
  if (sourceFilter === "archives") return t("observability-source-archives");
  return t("observability-source-all");
}

export function observabilityHealthTone(
  status: KernelObservabilityHealthStatus,
): "clean" | "warning" | "error" {
  return status === "error" ? "error" : status === "warning" ? "warning" : "clean";
}

export function observabilityHealthLabel(health: KernelObservabilityHealthSnapshot): string {
  if (health.status === "warning") return t("observability-health-warning");
  if (health.status === "error") return t("observability-health-error");
  return t("observability-health-clean");
}

export function observabilityHealthDetail(health: KernelObservabilityHealthSnapshot): string {
  return t("observability-health-detail", {
    events: health.eventCount,
    recovery: health.recoveryCount,
    modules: health.moduleCount,
  });
}

export function observabilityHealthProblemLabel(health: KernelObservabilityHealthSnapshot): string {
  if (!health.latestProblem) return t("observability-no-recent-problem");
  return `${kernelLogLevelLabel(health.latestProblem.level)} · ${health.latestProblem.owner} · ${health.latestProblem.eventName}`;
}

export function observabilityModuleHealthLabel(module: KernelObservabilityModuleHealthSnapshot): string {
  const problemCount = module.levelCounts.error || module.levelCounts.warn;
  return t("observability-module-detail", {
    events: module.eventCount,
    problems: problemCount,
  });
}

export function kernelLogAttributeEntries(
  event: KernelObservabilityLogEvent,
  maxEntries = 8,
): Array<[string, string]> {
  return Object.entries(event.attributes ?? {})
    .slice(0, maxEntries)
    .map(([key, value]) => [key, formatKernelAttributeValue(value)]);
}

export function formatKernelAttributeValue(value: JsonValue): string {
  if (value === null) return "null";
  if (typeof value === "string") return compactKernelPath(value, 80);
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  if (Array.isArray(value)) {
    if (!value.length) return "[]";
    const preview = value.slice(0, 3).map(formatKernelAttributeValue).join(", ");
    return value.length > 3 ? `[${preview}, +${value.length - 3}]` : `[${preview}]`;
  }
  const keys = Object.keys(value);
  if (!keys.length) return "{}";
  const preview = keys.slice(0, 3).join(", ");
  return keys.length > 3 ? `{${preview}, +${keys.length - 3}}` : `{${preview}}`;
}

export function kernelLogPathSummary(snapshot: KernelObservabilityLogSnapshot): string {
  const scan = snapshot.truncated
    ? t("observability-path-scan-limited", { size: formatBytes(snapshot.scannedBytes) })
    : t("observability-path-scanned", { size: formatBytes(snapshot.scannedBytes) });
  const archives = snapshot.retention.archivedCount
    ? t("observability-path-archives", {
        retained: snapshot.retention.archivedCount,
        limit: snapshot.retention.archiveCount,
        size: formatBytes(snapshot.retention.totalRetainedBytes),
      })
    : t("observability-path-no-archives", {
        size: formatBytes(snapshot.retention.maxActiveBytes),
      });
  const sourceCount = snapshot.includeArchives
    ? ` · ${t("observability-path-sources", {
        count: snapshot.sources.filter((source) => source.exists).length,
      })}`
    : "";
  return `${compactKernelPath(snapshot.logPath, 92)} · ${scan} · ${archives}${sourceCount}`;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${l10n.formatNumber(bytes)} B`;
  if (bytes < 1024 * 1024) {
    return `${l10n.formatNumber(bytes / 1024, { maximumFractionDigits: 1 })} KiB`;
  }
  return `${l10n.formatNumber(bytes / (1024 * 1024), { maximumFractionDigits: 1 })} MiB`;
}
