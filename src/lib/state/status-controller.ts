import {
  globalStatusInputFromKind,
  nextGlobalStatusExpiry,
  normalizeGlobalStatus,
  notificationFromGlobalStatus,
  pruneGlobalStatusEvents,
  publishGlobalStatusEvent,
  resolveGlobalStatusEvents,
  selectCurrentGlobalStatus,
  type GlobalStatusEvent,
  type GlobalStatusInput,
  type GlobalStatusKind,
  type GlobalStatusPublishOptions,
  type GlobalStatusSnapshot,
} from "$lib/status/global-status";
import {
  upsertNotification,
  type AppNotification,
} from "$lib/notifications/center";

export type StatusControllerHost = {
  globalStatusEvents: GlobalStatusEvent[];
  globalStatusRevision: number;
  globalStatusSequence: number;
  globalStatusExpiryTimer: number | null;
  notifications: AppNotification[];
  dismissedNotificationIds: Set<string>;
  refreshGlobalStatusFromKernel?: () => void | Promise<void>;
};

export function clearGlobalStatusExpiryTimer(host: StatusControllerHost) {
  if (host.globalStatusExpiryTimer === null || typeof window === "undefined") return;
  window.clearTimeout(host.globalStatusExpiryTimer);
  host.globalStatusExpiryTimer = null;
}

function clearResolvedNotificationProjections(
  host: StatusControllerHost,
  previous: GlobalStatusEvent[],
) {
  const liveIds = new Set(
    host.globalStatusEvents
      .filter((event) => event.resolution === "open")
      .map((event) => event.id),
  );
  const resolvedIds = new Set(
    previous
      .filter((event) => !liveIds.has(event.id))
      .map((event) => event.id),
  );
  if (resolvedIds.size === 0) return;
  host.notifications = host.notifications.filter((notification) => (
    !notification.statusEventId || !resolvedIds.has(notification.statusEventId)
  ));
}

function pruneExpiredGlobalStatuses(host: StatusControllerHost, now = Date.now()) {
  const previous = host.globalStatusEvents;
  host.globalStatusEvents = pruneGlobalStatusEvents(previous, now);
  clearResolvedNotificationProjections(host, previous);
}

function scheduleNextExpiry(host: StatusControllerHost) {
  clearGlobalStatusExpiryTimer(host);
  if (typeof window === "undefined") return;
  const nextExpiry = nextGlobalStatusExpiry(host.globalStatusEvents);
  if (nextExpiry === null) return;
  const delay = Math.max(0, nextExpiry - Date.now());
  host.globalStatusExpiryTimer = window.setTimeout(() => {
    host.globalStatusExpiryTimer = null;
    pruneExpiredGlobalStatuses(host);
    scheduleNextExpiry(host);
    void host.refreshGlobalStatusFromKernel?.();
  }, delay);
}

function projectNotification(host: StatusControllerHost, event: GlobalStatusEvent) {
  const notification = notificationFromGlobalStatus(event);
  if (!notification || host.dismissedNotificationIds.has(notification.id)) return;
  host.notifications = upsertNotification(host.notifications, notification);
}

export function publishGlobalStatus(
  host: StatusControllerHost,
  input: GlobalStatusInput,
) {
  pruneExpiredGlobalStatuses(host);
  const event = normalizeGlobalStatus(input, ++host.globalStatusSequence);
  const previous = host.globalStatusEvents;
  host.globalStatusEvents = publishGlobalStatusEvent(previous, event);
  clearResolvedNotificationProjections(host, previous);
  projectNotification(host, event);
  scheduleNextExpiry(host);
  return event;
}

export function applyGlobalStatusSnapshot(
  host: StatusControllerHost,
  snapshot: GlobalStatusSnapshot,
) {
  if (snapshot.schemaVersion !== 1) {
    throw new Error(
      `GlobalStatus a returnat schema ${snapshot.schemaVersion}; schema 1 este obligatorie.`,
    );
  }
  if (!Number.isSafeInteger(snapshot.revision) || snapshot.revision < 0) {
    throw new Error("GlobalStatus a returnat o revizie invalidă.");
  }
  if (snapshot.revision < host.globalStatusRevision) return false;
  if (snapshot.events.some((event) => (
    event.schemaVersion !== 1
    || !Number.isSafeInteger(event.sequence)
    || event.sequence < 0
  ))) {
    throw new Error("GlobalStatus a returnat un eveniment incompatibil.");
  }
  host.globalStatusRevision = snapshot.revision;
  host.globalStatusEvents = snapshot.events;
  host.globalStatusSequence = Math.max(
    host.globalStatusSequence,
    ...snapshot.events.map((event) => event.sequence),
    0,
  );
  const openEventIds = new Set(
    snapshot.events
      .filter((event) => event.resolution === "open")
      .map((event) => event.id),
  );
  const openProjectionKeys = new Set(
    snapshot.events
      .filter((event) => event.resolution === "open")
      .flatMap((event) => [
        event.id,
        event.dedupeKey,
        event.resolutionKey,
      ])
      .filter((key): key is string => Boolean(key)),
  );
  host.dismissedNotificationIds = new Set(
    [...host.dismissedNotificationIds].filter((id) => openProjectionKeys.has(id)),
  );
  host.notifications = host.notifications.filter((notification) => (
    !notification.statusEventId || openEventIds.has(notification.statusEventId)
  ));
  for (const event of snapshot.events) {
    if (event.resolution === "open") projectNotification(host, event);
  }
  scheduleNextExpiry(host);
  return true;
}

export function setGlobalStatus(
  host: StatusControllerHost,
  text: string,
  kind: GlobalStatusKind,
  options: GlobalStatusPublishOptions = {},
) {
  return publishGlobalStatus(
    host,
    globalStatusInputFromKind(text, kind, options),
  );
}

export function resolveGlobalStatus(
  host: StatusControllerHost,
  key: string,
) {
  const previous = host.globalStatusEvents;
  host.globalStatusEvents = resolveGlobalStatusEvents(previous, key);
  clearResolvedNotificationProjections(host, previous);
  host.notifications = host.notifications.filter((notification) => (
    notification.id !== key
    && notification.statusEventId !== key
    && notification.statusResolutionKey !== key
  ));
  if (host.dismissedNotificationIds.has(key)) {
    host.dismissedNotificationIds = new Set(
      [...host.dismissedNotificationIds].filter((candidate) => candidate !== key),
    );
  }
  scheduleNextExpiry(host);
}

export function currentGlobalStatus(host: StatusControllerHost) {
  return selectCurrentGlobalStatus(host.globalStatusEvents);
}
