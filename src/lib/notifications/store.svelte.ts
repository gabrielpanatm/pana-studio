import {
  dismissNotification as withoutNotification,
  upsertNotification,
  type AppNotification,
} from "$lib/notifications/center";

export class NotificationCenterState {
  notifications = $state<AppNotification[]>([]);
  dismissedIds = $state<Set<string>>(new Set());

  has(id: string) {
    return this.notifications.some((notification) => notification.id === id);
  }

  wasDismissed(id: string) {
    return this.dismissedIds.has(id);
  }

  upsert(notification: Omit<AppNotification, "createdAt">) {
    if (this.wasDismissed(notification.id)) return false;
    this.notifications = upsertNotification(this.notifications, notification);
    return true;
  }

  dismiss(id: string) {
    this.notifications = withoutNotification(this.notifications, id);
    this.dismissedIds = new Set([...this.dismissedIds, id]);
  }

  removeWhere(predicate: (notification: AppNotification) => boolean) {
    this.notifications = this.notifications.filter((notification) => !predicate(notification));
  }

  retainDismissedWhere(predicate: (id: string) => boolean) {
    this.dismissedIds = new Set([...this.dismissedIds].filter(predicate));
  }

  forgetDismissal(id: string) {
    if (!this.dismissedIds.has(id)) return;
    this.dismissedIds = new Set(
      [...this.dismissedIds].filter((candidate) => candidate !== id),
    );
  }
}
