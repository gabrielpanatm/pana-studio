import test from "node:test";
import assert from "node:assert/strict";

import {
  globalStatusInputFromKind,
  normalizeGlobalStatus,
  notificationFromGlobalStatus,
  pruneGlobalStatusEvents,
  publishGlobalStatusEvent,
  resolveGlobalStatusEvents,
  selectCurrentGlobalStatus,
} from "$lib/status/global-status";
import {
  publishGlobalStatus,
  setGlobalStatus,
} from "$lib/status/controller";
import { NotificationCenterState } from "$lib/notifications/store.svelte";

function event(input, sequence, now = 1_000) {
  return normalizeGlobalStatus(input, sequence, now);
}

function input(overrides = {}) {
  return {
    code: "test.status",
    source: "test",
    message: "Test",
    severity: "info",
    ...overrides,
  };
}

function host() {
  return {
    globalStatusEvents: [],
    globalStatusSequence: 0,
    globalStatusExpiryTimer: null,
    notificationCenter: new NotificationCenterState(),
  };
}

test("GlobalStatus respectă ordinea blocaj, eroare, avertisment, activ, succes, info", () => {
  const events = [
    event(input({ code: "info", severity: "info" }), 1),
    event(input({ code: "success", severity: "success" }), 2),
    event(input({ code: "active", severity: "info", phase: "active" }), 3),
    event(input({ code: "warning", severity: "warning" }), 4),
    event(input({ code: "error", severity: "error" }), 5),
    event(input({ code: "blocking", severity: "blocking" }), 6),
  ];
  assert.equal(selectCurrentGlobalStatus(events, 1_001)?.code, "blocking");
  const withoutBlocking = resolveGlobalStatusEvents(events, events[5].id, 1_002);
  assert.equal(selectCurrentGlobalStatus(withoutBlocking, 1_003)?.code, "error");
  const withoutError = resolveGlobalStatusEvents(withoutBlocking, events[4].id, 1_004);
  assert.equal(selectCurrentGlobalStatus(withoutError, 1_005)?.code, "warning");
});

test("publicarea este latest-wins pe dedupeKey și păstrează dovada rezolvării", () => {
  const first = event(input({ message: "prima", dedupeKey: "lane" }), 1);
  const second = event(input({ message: "a doua", dedupeKey: "lane" }), 2, 1_100);
  const events = publishGlobalStatusEvent(
    publishGlobalStatusEvent([], first),
    second,
  );
  assert.equal(events[0].resolution, "resolved");
  assert.equal(events[0].resolvedAt, 1_100);
  assert.equal(selectCurrentGlobalStatus(events, 1_101)?.message, "a doua");
});

test("statusurile tranzitorii expiră, iar cele active rămân până la înlocuire", () => {
  const transient = event(input({ timeoutMs: 100 }), 1, 1_000);
  const active = event(input({
    code: "active",
    phase: "active",
    timeoutMs: 1,
  }), 2, 1_000);
  const pruned = pruneGlobalStatusEvents([transient, active], 1_101);
  assert.equal(pruned.find((candidate) => candidate.id === transient.id)?.resolution, "resolved");
  assert.equal(pruned.find((candidate) => candidate.id === active.id)?.resolution, "open");
  assert.equal(selectCurrentGlobalStatus(pruned, 1_101)?.id, active.id);
});

test("erorile se escaladează implicit, informațiile numai explicit", () => {
  const error = event(input({ severity: "error" }), 1);
  const info = event(input({ code: "info" }), 2);
  const explicit = event(input({
    code: "explicit",
    escalation: "notification",
    dedupeKey: "notice",
    notification: { title: "Titlu" },
  }), 3);
  assert.equal(notificationFromGlobalStatus(error)?.level, "error");
  assert.equal(notificationFromGlobalStatus(info), null);
  assert.equal(notificationFromGlobalStatus(explicit)?.id, "notice");
});

test("coordonatorul proiectează aceeași notificare persistentă", () => {
  const state = host();
  const published = publishGlobalStatus(state, input({
    severity: "error",
    dedupeKey: "problem",
    resolutionKey: "problem",
    notification: { title: "Problemă", message: "Diagnostic" },
  }));
  assert.equal(state.notificationCenter.notifications.length, 1);
  assert.equal(state.notificationCenter.notifications[0].statusEventId, published.id);
});

test("adaptorul de producător afișează inclusiv mesajele idle utile", () => {
  const state = host();
  setGlobalStatus(state, "Ținta a fost selectată.", "idle");
  assert.equal(selectCurrentGlobalStatus(state.globalStatusEvents)?.message, "Ținta a fost selectată.");
  assert.equal(
    globalStatusInputFromKind("Se salvează", "saving").phase,
    "active",
  );
  assert.equal(
    globalStatusInputFromKind("Eroare pasivă", "error").escalation,
    "status_only",
  );
});
