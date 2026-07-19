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

const sourceFilterLabels: Record<KernelObservabilityLogSourceFilter, string> = {
  all: "toate sursele",
  active: "log activ",
  archives: "arhive",
};

const healthLabels: Record<KernelObservabilityHealthStatus, string> = {
  clean: "Operațional curat",
  warning: "Necesită atenție",
  error: "Problemă operațională",
};

export function observabilitySummary(snapshot: KernelObservabilityLogSnapshot | null): ObservabilitySummary {
  if (!snapshot) {
    return {
      tone: "idle",
      label: "Observability Log indisponibil",
      detail: "Kernel-ul nu a încărcat încă logul operațional.",
    };
  }

  if (!snapshot.logExists) {
    return {
      tone: "warning",
      label: "Log operațional lipsă",
      detail: "Application Home nu conține încă kernel.jsonl.",
    };
  }

  if (snapshot.unreadableCount > 0) {
    return {
      tone: "error",
      label: "Log citit cu diagnostics",
      detail: `${snapshot.returnedCount} evenimente afișate, ${snapshot.unreadableCount} linii ignorate.`,
    };
  }

  return {
    tone: snapshot.truncated ? "warning" : "clean",
    label: snapshot.recoveryOnly ? "Recovery events" : "Kernel events",
    detail: `${snapshot.returnedCount} evenimente afișate din ${snapshot.scannedLineCount} linii scanate · ${kernelLogLevelFilterLabel(snapshot.levels)} · ${kernelLogSourceFilterLabel(snapshot.sourceFilter)}.`,
  };
}

export function kernelLogLevelLabel(level: KernelLogLevel): string {
  return levelLabels[level] ?? level.toUpperCase();
}

export function kernelLogLevelFilterLabel(levels: KernelLogLevel[]): string {
  if (!levels.length) return "nicio severitate";
  const unique = ["info", "warn", "error"].filter((level) =>
    levels.includes(level as KernelLogLevel),
  ) as KernelLogLevel[];
  if (unique.length === 3) return "toate severitățile";
  return unique.map(kernelLogLevelLabel).join(", ");
}

export function kernelLogLevelTone(level: KernelLogLevel): "info" | "warn" | "error" {
  return level === "error" ? "error" : level === "warn" ? "warn" : "info";
}

export function kernelObservabilityEventLimitLabel(limit: number): string {
  return `${limit} evenimente`;
}

export function formatKernelLogTime(timestampMs: number): string {
  if (!timestampMs) return "timp necunoscut";
  return new Intl.DateTimeFormat("ro-RO", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(new Date(timestampMs));
}

export function kernelLogTargetLabel(event: KernelObservabilityLogEvent): string {
  return event.target ? compactKernelPath(event.target, 72) : "fără target";
}

export function kernelLogSourceLabel(event: KernelObservabilityLogEvent): string {
  return event.source?.label ?? "sursă necunoscută";
}

export function kernelLogSourceFilterLabel(sourceFilter: KernelObservabilityLogSourceFilter): string {
  return sourceFilterLabels[sourceFilter] ?? sourceFilter;
}

export function observabilityHealthTone(
  status: KernelObservabilityHealthStatus,
): "clean" | "warning" | "error" {
  return status === "error" ? "error" : status === "warning" ? "warning" : "clean";
}

export function observabilityHealthLabel(health: KernelObservabilityHealthSnapshot): string {
  return healthLabels[health.status] ?? health.status;
}

export function observabilityHealthDetail(health: KernelObservabilityHealthSnapshot): string {
  const moduleDetail = health.moduleCount === 1 ? "1 modul" : `${health.moduleCount} module`;
  return `${health.eventCount} evenimente analizate · ${health.recoveryCount} recovery · ${moduleDetail}`;
}

export function observabilityHealthProblemLabel(health: KernelObservabilityHealthSnapshot): string {
  if (!health.latestProblem) return "Fără eveniment critic recent";
  return `${kernelLogLevelLabel(health.latestProblem.level)} · ${health.latestProblem.owner} · ${health.latestProblem.eventName}`;
}

export function observabilityModuleHealthLabel(module: KernelObservabilityModuleHealthSnapshot): string {
  const problemCount = module.levelCounts.error || module.levelCounts.warn;
  const problemDetail = problemCount ? `${problemCount} probleme` : "curat";
  return `${module.eventCount} evenimente · ${problemDetail}`;
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
    ? `scan limitat la ${formatBytes(snapshot.scannedBytes)}`
    : `${formatBytes(snapshot.scannedBytes)} scanați`;
  const archives = snapshot.retention.archivedCount
    ? ` · ${snapshot.retention.archivedCount}/${snapshot.retention.archiveCount} arhive, ${formatBytes(snapshot.retention.totalRetainedBytes)} retenție`
    : ` · fără arhive, limită activă ${formatBytes(snapshot.retention.maxActiveBytes)}`;
  const sourceCount = snapshot.includeArchives
    ? ` · ${snapshot.sources.filter((source) => source.exists).length} surse citite`
    : "";
  return `${compactKernelPath(snapshot.logPath, 92)} · ${scan}${archives}${sourceCount}`;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
}
