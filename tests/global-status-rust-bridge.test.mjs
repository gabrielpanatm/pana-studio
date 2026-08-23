import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";

import { NotificationCenterState } from "$lib/notifications/store.svelte";
import { GlobalStatusState } from "$lib/status/state.svelte";

if (!globalThis.window) globalThis.window = globalThis;

afterEach(() => clearMocks());

function deferred() {
  let resolve;
  const promise = new Promise((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

function host() {
  return new GlobalStatusState(new NotificationCenterState());
}

function eventFromInput(input, sequence) {
  return {
    schemaVersion: 1,
    id: `global-status:${sequence}`,
    code: input.code,
    source: input.source,
    severity: input.severity,
    phase: input.phase ?? "settled",
    priority: input.phase === "active" ? 300 : 100,
    message: input.message,
    detail: input.detail ?? null,
    lifecycle: input.lifecycle ?? "transient",
    escalation: input.escalation ?? "status_only",
    dedupeKey: input.dedupeKey ?? `${input.source}:${input.code}`,
    resolutionKey: input.resolutionKey ?? null,
    resolution: "open",
    sequence,
    createdAt: sequence,
    updatedAt: sequence,
    expiresAt: null,
    resolvedAt: null,
    notification: input.notification ?? null,
  };
}

function snapshot(revision, events) {
  return {
    schemaVersion: 1,
    revision,
    events,
    current: events.at(-1) ?? null,
  };
}

async function nextTurn() {
  await new Promise((resolve) => setImmediate(resolve));
}

test("publicările sunt serializate și devin vizibile numai după receipt-ul Rust", async () => {
  const first = deferred();
  const second = deferred();
  const calls = [];
  mockIPC((command, payload) => {
    assert.equal(command, "publish_global_status");
    calls.push(payload.input);
    return calls.length === 1 ? first.promise : second.promise;
  });
  const app = host();

  app.set("Prima", "saving", {
    source: "test",
    dedupeKey: "test:lane",
    lifecycle: "until_replaced",
    escalation: "status_only",
  });
  app.set("A doua", "saved", { source: "test", dedupeKey: "test:lane" });
  await nextTurn();

  assert.equal(calls.length, 1);
  assert.equal(calls[0].lifecycle, "until_replaced");
  assert.equal(calls[0].escalation, "status_only");
  assert.equal(app.globalStatusEvents.length, 0);

  const firstEvent = eventFromInput(calls[0], 1);
  first.resolve(snapshot(1, [firstEvent]));
  await nextTurn();
  assert.equal(app.globalStatusEvents.at(-1)?.message, "Prima");
  assert.equal(calls.length, 2);

  const secondEvent = eventFromInput(calls[1], 2);
  firstEvent.resolution = "resolved";
  second.resolve(snapshot(2, [firstEvent, secondEvent]));
  await app.settled();
  assert.equal(app.globalStatusEvents.at(-1)?.message, "A doua");
  assert.equal(app.globalStatusRevision, 2);
});

test("escaladarea și rezolvarea folosesc același eveniment Rust", async () => {
  let publishedEvent;
  mockIPC((command, payload) => {
    if (command === "publish_global_status") {
      publishedEvent = eventFromInput(payload.input, 1);
      return snapshot(1, [publishedEvent]);
    }
    assert.equal(command, "resolve_global_status");
    assert.equal(payload.key, "project.problem");
    return snapshot(2, []);
  });
  const app = host();

  app.escalate({
    id: "project.problem",
    level: "warning",
    title: "Titlu persistent",
    message: "Diagnostic complet",
    statusMessage: "Mesaj status",
  });
  await app.settled();

  assert.equal(app.globalStatusEvents[0].message, "Mesaj status");
  assert.equal(app.notificationCenter.notifications[0].title, "Titlu persistent");
  assert.equal(app.notificationCenter.notifications[0].statusEventId, publishedEvent.id);

  app.notificationCenter.dismiss("project.problem");
  assert.equal(app.notificationCenter.notifications.length, 0);
  assert.equal(app.notificationCenter.wasDismissed("project.problem"), true);
  app.clear("project.problem");
  await app.settled();
  assert.equal(app.globalStatusEvents.length, 0);
  assert.equal(app.notificationCenter.notifications.length, 0);
  assert.equal(app.notificationCenter.wasDismissed("project.problem"), false);
});

test("destroy elimină timerul de expirare deținut de Global Status", () => {
  const status = host();
  status.globalStatusExpiryTimer = window.setTimeout(() => {}, 10_000);

  status.destroy();

  assert.equal(status.globalStatusExpiryTimer, null);
});
