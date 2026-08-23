import assert from "node:assert/strict";
import { test } from "node:test";
import { NotificationCenterState } from "$lib/notifications/store.svelte";

function notification(id, message = id) {
  return {
    id,
    level: "warning",
    title: id,
    message,
  };
}

test("centrul de notificări deține upsert, dismiss și deduplicarea", () => {
  const center = new NotificationCenterState();

  assert.equal(center.upsert(notification("disk", "first")), true);
  const createdAt = center.notifications[0].createdAt;
  assert.equal(center.upsert(notification("disk", "updated")), true);
  assert.equal(center.notifications.length, 1);
  assert.equal(center.notifications[0].message, "updated");
  assert.equal(center.notifications[0].createdAt, createdAt);

  center.dismiss("disk");
  assert.deepEqual(center.notifications, []);
  assert.equal(center.wasDismissed("disk"), true);
  assert.equal(center.upsert(notification("disk", "stale")), false);
  assert.deepEqual(center.notifications, []);

  center.forgetDismissal("disk");
  assert.equal(center.upsert(notification("disk", "fresh")), true);
  assert.equal(center.notifications[0].message, "fresh");
});

test("retenția proiecțiilor dismiss păstrează numai cheile încă deschise", () => {
  const center = new NotificationCenterState();
  center.dismiss("closed");
  center.dismiss("open");

  center.retainDismissedWhere((id) => id === "open");

  assert.equal(center.wasDismissed("closed"), false);
  assert.equal(center.wasDismissed("open"), true);
});
