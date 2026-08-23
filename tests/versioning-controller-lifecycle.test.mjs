import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { VersionNetworkProgressLifetime } from "../src/lib/versioning/network-progress-lifetime.ts";
import {
  beginVersioningOperation,
  failVersioningOperation,
  finishVersioningOperation,
} from "../src/lib/versioning/operation-lifecycle.ts";
import { settleVersioningPublication } from "../src/lib/versioning/publication-settlement.ts";
import { VersioningSessionEpoch } from "../src/lib/versioning/session-epoch.ts";

function source(relativePath) {
  return readFileSync(new URL(`../${relativePath}`, import.meta.url), "utf8");
}

test("session epoch invalidează răspunsurile proiectului înlocuit", () => {
  const epoch = new VersioningSessionEpoch();
  assert.deepEqual(epoch.synchronize("/project", "session-a"), {
    changed: true,
    serial: 1,
  });
  const request = epoch.nextRequest();
  assert.equal(epoch.isCurrent(request), true);
  assert.deepEqual(epoch.synchronize("/project", "session-b"), {
    changed: true,
    serial: 3,
  });
  assert.equal(epoch.isCurrent(request), false);
  assert.equal(epoch.synchronize("/project", "session-b").changed, false);
});

test("network terminal lifetime anulează timerul stale și se golește la dispose", () => {
  let nextHandle = 0;
  const scheduled = new Map();
  const cancelled = [];
  const published = [];
  const lifetime = new VersionNetworkProgressLifetime({
    schedule(callback, delayMs) {
      const handle = ++nextHandle;
      scheduled.set(handle, { callback, delayMs });
      return handle;
    },
    cancel(handle) {
      cancelled.push(handle);
      scheduled.delete(handle);
    },
  }, 25);

  lifetime.receive(
    { operationId: "fetch-a", status: "completed" },
    (value) => published.push(value),
  );
  assert.equal(scheduled.get(1).delayMs, 25);
  lifetime.receive(
    { operationId: "fetch-b", status: "progress" },
    (value) => published.push(value),
  );
  assert.deepEqual(cancelled, [1]);
  assert.equal(published.at(-1).operationId, "fetch-b");

  lifetime.receive(
    { operationId: "fetch-b", status: "cancelled" },
    (value) => published.push(value),
  );
  scheduled.get(2).callback();
  assert.equal(published.at(-1), null);

  lifetime.receive(
    { operationId: "push-c", status: "failed" },
    (value) => published.push(value),
  );
  lifetime.clear((value) => published.push(value));
  assert.equal(published.at(-1), null);
  assert.deepEqual(cancelled, [1, 3]);
});

test("publicarea frontend rulează efectul o singură dată și păstrează eroarea", async () => {
  let calls = 0;
  const success = await settleVersioningPublication(() => { calls += 1; });
  assert.deepEqual(success, { ok: true });
  assert.equal(calls, 1);

  const expected = new Error("projection failed");
  const failure = await settleVersioningPublication(() => {
    calls += 1;
    throw expected;
  });
  assert.equal(failure.ok, false);
  assert.equal(failure.error, expected);
  assert.equal(calls, 2);
});

test("operation lifecycle curăță busy și păstrează eroarea până la următorul begin", () => {
  const started = beginVersioningOperation("restore:abc");
  assert.deepEqual(started, { busyAction: "restore:abc", error: "" });

  const failed = failVersioningOperation(started, "projection failed");
  assert.deepEqual(failed, {
    busyAction: "restore:abc",
    error: "projection failed",
  });
  assert.deepEqual(finishVersioningOperation(failed), {
    busyAction: "",
    error: "projection failed",
  });
  assert.deepEqual(beginVersioningOperation("fetch:origin"), {
    busyAction: "fetch:origin",
    error: "",
  });
});

test("panelul este prezentare, iar controllerele dețin lifecycle-ul async", () => {
  const panel = source("src/lib/components/VersionsPanel.svelte");
  assert.doesNotMatch(panel, /\$lib\/versioning\/io/);
  assert.doesNotMatch(panel, /listen<|async function/);
  for (const controller of ["snapshot", "network", "integration", "recovery"]) {
    assert.match(
      panel,
      new RegExp(`Versioning${controller[0].toUpperCase()}${controller.slice(1)}Controller`),
    );
  }

  for (const file of [
    "snapshot-controller.svelte.ts",
    "network-controller.svelte.ts",
    "integration-controller.svelte.ts",
    "recovery-controller.svelte.ts",
  ]) {
    const controller = source(`src/lib/versioning/${file}`);
    const begins = controller.match(/operations\.begin\(/g)?.length ?? 0;
    const finishes = controller.match(/operations\.finish\(\)/g)?.length ?? 0;
    assert.ok(finishes >= begins, `${file} trebuie să curețe toate busy leases`);
  }

  const recovery = source("src/lib/versioning/recovery-controller.svelte.ts");
  assert.equal(recovery.match(/afterRecovery\(receipt\)/g)?.length, 1);
  const integration = source("src/lib/versioning/integration-controller.svelte.ts");
  assert.equal(integration.match(/afterIntegrationRecovery\(receipt\)/g)?.length, 1);
});

test("facade-ul Rust re-exportă toate cele 29 de comenzi din module distincte", () => {
  const facade = source("src-tauri/src/commands/versioning/mod.rs");
  assert.match(facade, /mod session;/);
  assert.match(facade, /mod local;/);
  assert.match(facade, /mod network;/);
  assert.match(facade, /mod integration;/);
  assert.match(facade, /mod restore;/);
  assert.match(facade, /mod publication;/);
  assert.doesNotMatch(facade, /#\[tauri::command\]/);

  for (const [name, expected] of [
    ["local", 18],
    ["network", 3],
    ["integration", 5],
    ["restore", 3],
  ]) {
    const module = source(`src-tauri/src/commands/versioning/${name}.rs`);
    assert.equal(module.match(/#\[tauri::command\]/g)?.length, expected, name);
  }
});
