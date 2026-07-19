import assert from "node:assert/strict";
import { test } from "node:test";

import { createMoodBoardHeavyAssetGate } from "$lib/mood-board/heavy-asset-gate";

function deferred() {
  let resolve;
  const promise = new Promise((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

test("two rapid heavy asset intents start only one operation", async () => {
  const gate = createMoodBoardHeavyAssetGate();
  const inFlight = deferred();
  let ipcCalls = 0;

  async function runIntent() {
    const permit = gate.tryAcquire();
    if (!permit) return false;
    try {
      ipcCalls += 1;
      await inFlight.promise;
      return true;
    } finally {
      permit.release();
    }
  }

  const first = runIntent();
  const second = runIntent();
  assert.equal(await second, false);
  assert.equal(ipcCalls, 1);
  assert.equal(gate.isBusy(), true);

  inFlight.resolve();
  assert.equal(await first, true);
  assert.equal(gate.isBusy(), false);
});

test("heavy asset permits release idempotently", () => {
  const gate = createMoodBoardHeavyAssetGate();
  const permit = gate.tryAcquire();
  assert.ok(permit);
  permit.release();
  permit.release();
  assert.equal(gate.isBusy(), false);
  assert.ok(gate.tryAcquire());
});
