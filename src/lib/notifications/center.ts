export type AppNotificationLevel = "info" | "warning" | "error";

export type AppNotification = {
  id: string;
  level: AppNotificationLevel;
  title: string;
  message: string;
  actionLabel?: string | null;
  actionId?: string | null;
  secondaryActionLabel?: string | null;
  secondaryActionId?: string | null;
  createdAt: number;
};

export function upsertNotification(
  notifications: AppNotification[],
  notification: Omit<AppNotification, "createdAt">,
) {
  const existing = notifications.find((item) => item.id === notification.id);
  const next: AppNotification = {
    ...notification,
    createdAt: existing?.createdAt ?? Date.now(),
  };
  if (!existing) return [...notifications, next];
  return notifications.map((item) => (item.id === notification.id ? next : item));
}

export function dismissNotification(notifications: AppNotification[], id: string) {
  return notifications.filter((item) => item.id !== id);
}
