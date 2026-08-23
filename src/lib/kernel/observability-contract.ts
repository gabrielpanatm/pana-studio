import type { JsonValue } from "$lib/contracts/json-value";

export type KernelLogLevel = "info" | "warn" | "error";

export type KernelObservabilityLogSourceFilter = "all" | "active" | "archives";

type KernelObservabilityHealthStatus = "clean" | "warning" | "error";

type KernelObservabilityLevelCounts = {
  info: number;
  warn: number;
  error: number;
};

type KernelObservabilitySourceCounts = {
  active: number;
  archived: number;
};

type KernelObservabilityHealthProblemSnapshot = {
  eventId: string;
  eventName: string;
  owner: string;
  level: KernelLogLevel;
  severityText: string;
  timestampMs: number;
  message: string;
  sourceLabel: string;
};

type KernelObservabilityModuleHealthSnapshot = {
  owner: string;
  status: KernelObservabilityHealthStatus;
  eventCount: number;
  recoveryCount: number;
  levelCounts: KernelObservabilityLevelCounts;
  latestEventName: string | null;
  latestTimestampMs: number | null;
  latestSeverityText: string | null;
};

export type KernelObservabilityHealthSnapshot = {
  status: KernelObservabilityHealthStatus;
  eventCount: number;
  recoveryCount: number;
  levelCounts: KernelObservabilityLevelCounts;
  sourceCounts: KernelObservabilitySourceCounts;
  moduleCount: number;
  modules: KernelObservabilityModuleHealthSnapshot[];
  latestProblem: KernelObservabilityHealthProblemSnapshot | null;
};

export type KernelObservabilityLogEvent = {
  schemaVersion: number;
  id: string;
  timestampMs: number;
  observedTimestampMs: number;
  level: KernelLogLevel;
  severityText: string;
  severityNumber: number;
  kind: string;
  eventName: string;
  owner: string;
  category: string;
  operation: string;
  target: string | null;
  message: string;
  diagnostic: string | null;
  attributes: Record<string, JsonValue>;
  source: KernelObservabilityLogEventSourceSnapshot;
};

type KernelLogArchiveSnapshot = {
  index: number;
  path: string;
  exists: boolean;
  bytes: number;
};

type KernelLogRetentionSnapshot = {
  maxActiveBytes: number;
  archiveCount: number;
  archivedCount: number;
  archivedBytes: number;
  totalRetainedBytes: number;
  archives: KernelLogArchiveSnapshot[];
};

type KernelObservabilityLogSourceSnapshot = {
  path: string;
  archiveIndex: number | null;
  exists: boolean;
  truncated: boolean;
  scannedBytes: number;
  scannedLineCount: number;
  unreadableCount: number;
};

type KernelObservabilityLogEventSourceSnapshot = {
  path: string;
  archiveIndex: number | null;
  label: string;
  active: boolean;
};

export type KernelObservabilityLogSnapshot = {
  schemaVersion: number;
  logPath: string;
  logExists: boolean;
  truncated: boolean;
  scannedBytes: number;
  scannedLineCount: number;
  returnedCount: number;
  unreadableCount: number;
  recoveryOnly: boolean;
  includeArchives: boolean;
  levels: KernelLogLevel[];
  eventNames: string[];
  sourceFilter: KernelObservabilityLogSourceFilter;
  limit: number;
  retention: KernelLogRetentionSnapshot;
  health: KernelObservabilityHealthSnapshot;
  sources: KernelObservabilityLogSourceSnapshot[];
  events: KernelObservabilityLogEvent[];
  diagnostics: string[];
};
