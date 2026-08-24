import type {
  AppNotification,
  AppNotificationLevel,
} from "$lib/notifications/center";

export const GLOBAL_STATUS_SCHEMA_VERSION = 1 as const;

export type GlobalStatusSeverity =
  | "info"
  | "success"
  | "warning"
  | "error"
  | "blocking";

export type GlobalStatusPhase = "idle" | "active" | "settled";
export type GlobalStatusLifecycle = "transient" | "until_replaced" | "until_resolved";
export type GlobalStatusEscalation = "status_only" | "notification";
export type GlobalStatusResolution = "open" | "resolved";

/** Compact producer vocabulary projected into the canonical event contract. */
export type GlobalStatusKind =
  | "idle"
  | "unsaved"
  | "saving"
  | "saved"
  | "restored"
  | "error";

export type GlobalStatusNotification = {
  title: string;
  message?: string;
  level?: AppNotificationLevel;
  actionLabel?: string | null;
  actionId?: string | null;
  secondaryActionLabel?: string | null;
  secondaryActionId?: string | null;
};

export type GlobalStatusEscalationRequest = {
  id: string;
  level: AppNotificationLevel;
  title: string;
  message: string;
  statusMessage?: string;
  actionLabel?: string | null;
  actionId?: string | null;
  secondaryActionLabel?: string | null;
  secondaryActionId?: string | null;
};

export type GlobalStatusEvent = {
  schemaVersion: typeof GLOBAL_STATUS_SCHEMA_VERSION;
  id: string;
  code: string;
  source: string;
  severity: GlobalStatusSeverity;
  phase: GlobalStatusPhase;
  priority: number;
  message: string;
  detail: string | null;
  lifecycle: GlobalStatusLifecycle;
  escalation: GlobalStatusEscalation;
  dedupeKey: string;
  resolutionKey: string | null;
  resolution: GlobalStatusResolution;
  sequence: number;
  createdAt: number;
  updatedAt: number;
  expiresAt: number | null;
  resolvedAt: number | null;
  notification: GlobalStatusNotification | null;
};

export type GlobalStatusInput = {
  code: string;
  source: string;
  message: string;
  severity: GlobalStatusSeverity;
  phase?: GlobalStatusPhase;
  priority?: number;
  detail?: string | null;
  lifecycle?: GlobalStatusLifecycle;
  escalation?: GlobalStatusEscalation;
  dedupeKey?: string;
  resolutionKey?: string | null;
  timeoutMs?: number | null;
  notification?: GlobalStatusNotification | null;
};

export type KernelGlobalStatusInput = GlobalStatusInput & {
  schemaVersion: typeof GLOBAL_STATUS_SCHEMA_VERSION;
};

export type GlobalStatusSnapshot = {
  schemaVersion: typeof GLOBAL_STATUS_SCHEMA_VERSION;
  revision: number;
  events: GlobalStatusEvent[];
  current: GlobalStatusEvent | null;
};

export type GlobalStatusPublishOptions = Partial<
  Omit<GlobalStatusInput, "message" | "severity">
> & {
  severity?: GlobalStatusSeverity;
};

const DEFAULT_TRANSIENT_MS = 4_000;

function stableSegment(value: string) {
  const normalized = value
    .trim()
    .toLocaleLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, ".")
    .replace(/^\.+|\.+$/g, "");
  return normalized.slice(0, 72) || "status";
}

function stableIdentityHash(value: string) {
  let hash = 0xcbf29ce484222325n;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= BigInt(value.charCodeAt(index));
    hash = BigInt.asUintN(64, hash * 0x100000001b3n);
  }
  return hash.toString(16).padStart(16, "0");
}

/** Resolves every session-only status made durable by one ProjectWorkspace Save. */
export function projectWorkspaceDirtyStatusKey(
  projectRoot: string,
  runtimeSessionId: string,
) {
  const identity = `${projectRoot.trim()}\u0000${runtimeSessionId.trim()}`;
  return [
    "project-workspace:dirty",
    stableIdentityHash(identity),
    stableSegment(runtimeSessionId),
  ].join(":");
}

function severityPriority(severity: GlobalStatusSeverity, phase: GlobalStatusPhase) {
  if (severity === "blocking") return 500;
  if (severity === "error") return 450;
  if (severity === "warning") return 350;
  if (phase === "active") return 300;
  if (severity === "success") return 200;
  return 100;
}

function defaultLifecycle(
  severity: GlobalStatusSeverity,
  phase: GlobalStatusPhase,
): GlobalStatusLifecycle {
  if (severity === "blocking" || severity === "error") return "until_resolved";
  if (severity === "warning" || phase === "active") return "until_replaced";
  return "transient";
}

function defaultEscalation(severity: GlobalStatusSeverity): GlobalStatusEscalation {
  return severity === "blocking" || severity === "error"
    ? "notification"
    : "status_only";
}

function defaultTimeout(
  lifecycle: GlobalStatusLifecycle,
  timeoutMs: number | null | undefined,
) {
  if (lifecycle !== "transient") return null;
  if (typeof timeoutMs === "number" && Number.isFinite(timeoutMs)) {
    return Math.max(0, Math.round(timeoutMs));
  }
  return DEFAULT_TRANSIENT_MS;
}

function normalizedPriority(
  priority: number | undefined,
  severity: GlobalStatusSeverity,
  phase: GlobalStatusPhase,
) {
  if (typeof priority !== "number" || !Number.isFinite(priority)) {
    return severityPriority(severity, phase);
  }
  return Math.min(65_535, Math.max(0, Math.round(priority)));
}

export function normalizeGlobalStatus(
  input: GlobalStatusInput,
  sequence: number,
  now = Date.now(),
): GlobalStatusEvent {
  const phase = input.phase ?? "settled";
  const lifecycle = input.lifecycle ?? defaultLifecycle(input.severity, phase);
  const escalation = input.escalation ?? defaultEscalation(input.severity);
  const source = input.source.trim() || "application";
  const code = input.code.trim() || `${source}.status`;
  const dedupeKey = input.dedupeKey?.trim() || `${source}:${code}`;
  const timeoutMs = defaultTimeout(lifecycle, input.timeoutMs);
  return {
    schemaVersion: GLOBAL_STATUS_SCHEMA_VERSION,
    id: `global-status:${sequence}`,
    code,
    source,
    severity: input.severity,
    phase,
    priority: normalizedPriority(input.priority, input.severity, phase),
    message: input.message.trim(),
    detail: input.detail ?? null,
    lifecycle,
    escalation,
    dedupeKey,
    resolutionKey: input.resolutionKey?.trim() || null,
    resolution: "open",
    sequence,
    createdAt: now,
    updatedAt: now,
    expiresAt: timeoutMs === null ? null : now + timeoutMs,
    resolvedAt: null,
    notification: input.notification ?? null,
  };
}

export function globalStatusInputFromKind(
  message: string,
  kind: GlobalStatusKind,
  options: GlobalStatusPublishOptions = {},
): GlobalStatusInput {
  const phase = options.phase ?? (kind === "saving" ? "active" : kind === "idle" ? "idle" : "settled");
  const severity = options.severity ?? (
    kind === "error"
      ? "error"
      : kind === "unsaved"
        ? "warning"
        : kind === "saved"
          ? "success"
          : kind === "restored"
            ? "success"
            : "info"
  );
  const source = options.source ?? "application";
  const code = options.code ?? `${stableSegment(source)}.${stableSegment(kind)}.${stableSegment(message)}`;
  return {
    code,
    source,
    message,
    severity,
    phase,
    priority: options.priority,
    detail: options.detail,
    lifecycle: options.lifecycle,
    escalation: options.escalation ?? "status_only",
    dedupeKey: options.dedupeKey ?? `${stableSegment(source)}:current`,
    resolutionKey: options.resolutionKey,
    timeoutMs: options.timeoutMs,
    notification: options.notification,
  };
}

export function publishGlobalStatusEvent(
  events: GlobalStatusEvent[],
  event: GlobalStatusEvent,
) {
  const superseded = events.map((candidate) => {
    if (
      candidate.resolution === "open"
      && candidate.dedupeKey === event.dedupeKey
    ) {
      return {
        ...candidate,
        resolution: "resolved" as const,
        resolvedAt: event.updatedAt,
      };
    }
    return candidate;
  });
  return [...superseded, event];
}

export function resolveGlobalStatusEvents(
  events: GlobalStatusEvent[],
  key: string,
  now = Date.now(),
) {
  return events.map((event) => {
    if (
      event.resolution === "open"
      && (
        event.id === key
        || event.dedupeKey === key
        || event.resolutionKey === key
      )
    ) {
      return {
        ...event,
        resolution: "resolved" as const,
        resolvedAt: now,
        updatedAt: now,
      };
    }
    return event;
  });
}

export function pruneGlobalStatusEvents(
  events: GlobalStatusEvent[],
  now = Date.now(),
  retainedResolved = 24,
) {
  const live = events.filter((event) => (
    event.resolution === "open"
    && (event.expiresAt === null || event.expiresAt > now)
  ));
  const resolved = events
    .filter((event) => (
      event.resolution === "resolved"
      || (event.expiresAt !== null && event.expiresAt <= now)
    ))
    .map((event) => (
      event.resolution === "resolved"
        ? event
        : {
            ...event,
            resolution: "resolved" as const,
            resolvedAt: event.expiresAt,
            updatedAt: event.expiresAt ?? event.updatedAt,
          }
    ))
    .sort((left, right) => right.sequence - left.sequence)
    .slice(0, retainedResolved);
  return [...resolved.reverse(), ...live];
}

export function selectCurrentGlobalStatus(
  events: GlobalStatusEvent[],
  now = Date.now(),
) {
  return events
    .filter((event) => (
      event.resolution === "open"
      && (event.expiresAt === null || event.expiresAt > now)
    ))
    .sort((left, right) => (
      right.priority - left.priority
      || right.sequence - left.sequence
    ))[0] ?? null;
}

export function nextGlobalStatusExpiry(events: GlobalStatusEvent[]) {
  const expiries = events
    .filter((event) => event.resolution === "open" && event.expiresAt !== null)
    .map((event) => event.expiresAt as number);
  return expiries.length > 0 ? Math.min(...expiries) : null;
}

function notificationLevelForStatus(event: GlobalStatusEvent): AppNotificationLevel {
  if (event.notification?.level) return event.notification.level;
  if (event.severity === "blocking" || event.severity === "error") return "error";
  if (event.severity === "warning") return "warning";
  return "info";
}

export function notificationFromGlobalStatus(
  event: GlobalStatusEvent,
): Omit<AppNotification, "createdAt"> | null {
  if (event.escalation !== "notification") return null;
  return {
    id: event.dedupeKey,
    statusEventId: event.id,
    statusResolutionKey: event.resolutionKey ?? event.dedupeKey,
    level: notificationLevelForStatus(event),
    title: event.notification?.title ?? event.message,
    message: event.notification?.message ?? event.detail ?? event.message,
    actionLabel: event.notification?.actionLabel,
    actionId: event.notification?.actionId,
    secondaryActionLabel: event.notification?.secondaryActionLabel,
    secondaryActionId: event.notification?.secondaryActionId,
  };
}
