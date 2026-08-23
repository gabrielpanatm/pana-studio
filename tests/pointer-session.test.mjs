import assert from "node:assert/strict";
import { test } from "node:test";
import { startPointerSession } from "$lib/ui/pointer-session";
import {
  createPointerHarness,
  keyboardEvent,
  pointerEvent,
} from "./pointer-session-test-helpers.mjs";

test("PointerSession filtrează pointerul, coalescă RAF și face final flush o singură dată", () => {
  const harness = createPointerHarness();
  const published = [];
  let latestX = 0;
  let commits = 0;
  const session = startPointerSession({
    pointerId: 7,
    captureTarget: harness.capture,
    environment: harness.environment,
    safetyTimeoutMs: 100,
    onMove(event, current) {
      latestX = event.clientX;
      current.requestFrame(() => published.push(latestX));
    },
    onCommit(event, current) {
      latestX = event.clientX;
      current.flushFrame();
      commits += 1;
    },
  });

  assert.deepEqual([...harness.capture.captured], [7]);
  harness.windowTarget.dispatchEvent(pointerEvent("pointermove", { pointerId: 8, clientX: 5 }));
  harness.windowTarget.dispatchEvent(pointerEvent("pointermove", { pointerId: 7, clientX: 10 }));
  harness.windowTarget.dispatchEvent(pointerEvent("pointermove", { pointerId: 7, clientX: 20 }));
  assert.equal(harness.frames.size, 1);
  harness.runFrames();
  assert.deepEqual(published, [20]);

  harness.windowTarget.dispatchEvent(pointerEvent("pointermove", { pointerId: 7, clientX: 30 }));
  harness.windowTarget.dispatchEvent(pointerEvent("pointerup", { pointerId: 8, clientX: 40 }));
  assert.equal(session.active, true);
  harness.windowTarget.dispatchEvent(pointerEvent("pointerup", { pointerId: 7, clientX: 40 }));

  assert.deepEqual(published, [20, 40]);
  assert.equal(commits, 1);
  assert.equal(session.commit(), false);
  assert.deepEqual(harness.capture.released, [7]);
  assert.equal(harness.windowTarget.totalListenerCount(), 0);
  assert.equal(harness.documentTarget.totalListenerCount(), 0);
  assert.equal(harness.frames.size, 0);
  assert.equal(harness.timers.size, 0);
});

test("PointerSession anulează și curăță complet pentru fiecare cauză browser", async (t) => {
  const cases = [
    ["pointercancel", (harness) => {
      harness.windowTarget.dispatchEvent(pointerEvent("pointercancel", { pointerId: 3 }));
    }],
    ["blur", (harness) => harness.windowTarget.dispatchEvent(new Event("blur"))],
    ["escape", (harness) => harness.windowTarget.dispatchEvent(keyboardEvent("Escape"))],
    ["hidden", (harness) => {
      harness.documentTarget.visibilityState = "hidden";
      harness.documentTarget.dispatchEvent(new Event("visibilitychange"));
    }],
    ["timeout", (harness) => harness.runTimers()],
  ];

  for (const [expectedReason, trigger] of cases) {
    await t.test(expectedReason, () => {
      const harness = createPointerHarness();
      const reasons = [];
      const session = startPointerSession({
        pointerId: 3,
        captureTarget: harness.capture,
        environment: harness.environment,
        safetyTimeoutMs: 100,
        onMove(_event, current) {
          current.requestFrame(() => assert.fail("frame stale executat după cancel"));
        },
        onCancel(reason) {
          reasons.push(reason);
        },
      });
      harness.windowTarget.dispatchEvent(pointerEvent("pointermove", { pointerId: 3 }));
      trigger(harness);

      assert.deepEqual(reasons, [expectedReason]);
      assert.equal(session.active, false);
      assert.equal(session.cancel(), false);
      assert.deepEqual(harness.capture.released, [3]);
      assert.equal(harness.windowTarget.totalListenerCount(), 0);
      assert.equal(harness.documentTarget.totalListenerCount(), 0);
      assert.equal(harness.frames.size, 0);
      assert.equal(harness.timers.size, 0);
    });
  }
});

test("PointerSession destroy este idempotent și raportează motivul", () => {
  const harness = createPointerHarness();
  const reasons = [];
  const session = startPointerSession({
    pointerId: 11,
    captureTarget: harness.capture,
    environment: harness.environment,
    onCancel: (reason) => reasons.push(reason),
  });

  assert.equal(session.destroy(), true);
  assert.equal(session.destroy(), false);
  assert.deepEqual(reasons, ["destroy"]);
  assert.equal(harness.windowTarget.totalListenerCount(), 0);
  assert.equal(harness.documentTarget.totalListenerCount(), 0);
});
