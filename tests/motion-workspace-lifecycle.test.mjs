import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { flushRegisteredEditDrafts } from "$lib/session/edit-flush-registry";
import { MotionWorkspaceLifecycle } from "$lib/motion/workspace-lifecycle";

const lifecycles = [];

afterEach(() => {
  while (lifecycles.length > 0) lifecycles.pop()?.stop();
});

test("lifecycle-ul Motion deține un singur handler și îl elimină la stop", async () => {
  const reasons = [];
  const lifecycle = new MotionWorkspaceLifecycle(async () => {
    reasons.push("flush");
  });
  lifecycles.push(lifecycle);

  assert.equal(lifecycle.active, false);
  assert.equal(lifecycle.start(), true);
  assert.equal(lifecycle.start(), false);
  assert.equal(lifecycle.active, true);

  await flushRegisteredEditDrafts("manual");
  assert.deepEqual(reasons, ["flush"]);

  assert.equal(lifecycle.stop(), true);
  assert.equal(lifecycle.stop(), false);
  assert.equal(lifecycle.active, false);
  await flushRegisteredEditDrafts("unmount");
  assert.deepEqual(reasons, ["flush"]);
});

test("lifecycle-ul Motion poate fi repornit fără handler stale", async () => {
  let flushCount = 0;
  const lifecycle = new MotionWorkspaceLifecycle(() => {
    flushCount += 1;
  });
  lifecycles.push(lifecycle);

  lifecycle.start();
  lifecycle.stop();
  lifecycle.start();
  await flushRegisteredEditDrafts("save");

  assert.equal(flushCount, 1);
});
