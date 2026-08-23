import assert from "node:assert/strict";
import { test } from "node:test";
import { WorkspaceLayoutPersistenceLifecycle } from "$lib/ui/workspace-layout-lifecycle.svelte";
import { WorkspaceLayoutState } from "$lib/ui/workspace-layout.svelte";

test("lifecycle-ul Workspace Layout deține un singur efect și persistă snapshotul curent", () => {
  const layout = new WorkspaceLayoutState();
  const storage = {};
  const writes = [];
  const effects = [];
  let cleanupCount = 0;
  const lifecycle = new WorkspaceLayoutPersistenceLifecycle(
    layout,
    () => storage,
    (receivedStorage, dimensions) => {
      writes.push({ receivedStorage, dimensions });
    },
    (effect) => {
      effects.push(effect);
      return () => {
        cleanupCount += 1;
      };
    },
  );

  assert.equal(lifecycle.active, false);
  assert.equal(lifecycle.start(), true);
  assert.equal(lifecycle.start(), false);
  assert.equal(lifecycle.active, true);
  assert.equal(effects.length, 1);

  layout.leftPaneWidth = 301;
  layout.rightPaneWidth = 377;
  layout.terminalPaneHeight = 266;
  effects[0]();
  assert.deepEqual(writes, [{
    receivedStorage: storage,
    dimensions: {
      leftPaneWidth: 301,
      rightPaneWidth: 377,
      terminalPaneHeight: 266,
    },
  }]);

  assert.equal(lifecycle.stop(), true);
  assert.equal(lifecycle.stop(), false);
  assert.equal(lifecycle.active, false);
  assert.equal(cleanupCount, 1);
});

test("lifecycle-ul Workspace Layout poate fi repornit și ignoră persistența fără Storage", () => {
  const layout = new WorkspaceLayoutState();
  const writes = [];
  const effects = [];
  let cleanupCount = 0;
  const lifecycle = new WorkspaceLayoutPersistenceLifecycle(
    layout,
    () => null,
    (_storage, dimensions) => writes.push(dimensions),
    (effect) => {
      effects.push(effect);
      return () => {
        cleanupCount += 1;
      };
    },
  );

  lifecycle.start();
  effects[0]();
  lifecycle.stop();
  assert.equal(lifecycle.start(), true);
  effects[1]();

  assert.deepEqual(writes, []);
  assert.equal(effects.length, 2);
  assert.equal(cleanupCount, 1);
  lifecycle.stop();
  assert.equal(cleanupCount, 2);
});
