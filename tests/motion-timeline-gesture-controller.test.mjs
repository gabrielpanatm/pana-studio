import assert from "node:assert/strict";
import { test } from "node:test";
import { MotionTimelineGestureController } from "$lib/motion/timeline-gesture-controller";
import {
  createPointerHarness,
  pointerEvent,
} from "./pointer-session-test-helpers.mjs";

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((onResolve, onReject) => {
    resolve = onResolve;
    reject = onReject;
  });
  return { promise, resolve, reject };
}

function createControllerHarness() {
  const pointer = createPointerHarness();
  const drafts = new Map();
  const draftWrites = [];
  const playheads = [];
  const seeks = [];
  const commits = [];
  const commitResults = [];
  const active = [];
  let selectedInteractionId = "interaction-a";
  const controller = new MotionTimelineGestureController({
    selectedInteractionId: () => selectedInteractionId,
    selectAction: (interactionId, actionId) => {
      selectedInteractionId = interactionId;
      active.push(["select", actionId]);
    },
    setTimingDraft: (actionId, draft) => {
      draftWrites.push([actionId, draft]);
      if (draft) drafts.set(actionId, draft);
      else drafts.delete(actionId);
    },
    setPlayhead: (value) => playheads.push(value),
    requestSeek: (interactionId, value) => seeks.push([interactionId, value]),
    commitTiming: (commit) => {
      commits.push(commit);
      const result = deferred();
      commitResults.push(result);
      return result.promise;
    },
    setGestureActive: (gesture, value) => active.push([gesture, value]),
  }, pointer.environment);
  return {
    ...pointer,
    controller,
    drafts,
    draftWrites,
    playheads,
    seeks,
    commits,
    commitResults,
    active,
    setSelectedInteractionId(value) {
      selectedInteractionId = value;
    },
  };
}

const action = {
  id: "action-a",
  type: "animate",
};

function beginActionDrag(harness, overrides = {}) {
  return harness.controller.beginActionDrag({
    event: pointerEvent("pointerdown", { pointerId: 4, clientX: 0 }),
    captureTarget: harness.capture,
    action,
    interactionId: "interaction-a",
    domain: "time",
    mode: "move",
    canvasWidth: 100,
    timelineDuration: 1_000,
    snap: false,
    original: { start: 0, duration: 100 },
    safetyTimeoutMs: 100,
    ...overrides,
  });
}

test("controllerul Motion coalescă drafturile și emite o singură mutație la final exact", () => {
  const harness = createControllerHarness();
  assert.equal(beginActionDrag(harness), true);

  harness.windowTarget.dispatchEvent(pointerEvent("pointermove", { pointerId: 4, clientX: 10 }));
  harness.windowTarget.dispatchEvent(pointerEvent("pointermove", { pointerId: 4, clientX: 20 }));
  assert.equal(harness.commits.length, 0);
  assert.equal(harness.frames.size, 1);
  harness.runFrames();
  assert.deepEqual(harness.drafts.get(action.id), { start: 200, duration: 100 });

  harness.windowTarget.dispatchEvent(pointerEvent("pointerup", { pointerId: 4, clientX: 25 }));
  assert.deepEqual(harness.commits, [{
    interactionId: "interaction-a",
    actionId: "action-a",
    start: 250,
    duration: 100,
  }]);
  assert.deepEqual(harness.drafts.get(action.id), { start: 250, duration: 100 });
  assert.equal(harness.controller.dragging, false);
  assert.equal(harness.windowTarget.totalListenerCount(), 0);
  assert.deepEqual(harness.capture.released, [4]);
});

test("controllerul Motion păstrează draftul celui mai nou commit până la receipt", async () => {
  const harness = createControllerHarness();
  beginActionDrag(harness);
  harness.windowTarget.dispatchEvent(pointerEvent("pointerup", { pointerId: 4, clientX: 10 }));
  beginActionDrag(harness, {
    event: pointerEvent("pointerdown", { pointerId: 5, clientX: 10 }),
    original: harness.drafts.get(action.id),
  });
  harness.windowTarget.dispatchEvent(pointerEvent("pointerup", { pointerId: 5, clientX: 30 }));
  assert.equal(harness.commits.length, 2);
  assert.deepEqual(harness.drafts.get(action.id), { start: 300, duration: 100 });

  harness.commitResults[0].resolve();
  await Promise.resolve();
  assert.deepEqual(harness.drafts.get(action.id), { start: 300, duration: 100 });
  harness.commitResults[1].resolve();
  await Promise.resolve();
  assert.equal(harness.drafts.has(action.id), false);
});

test("schimbarea identității anulează drag-ul fără mutație și restaurează draftul", () => {
  const harness = createControllerHarness();
  beginActionDrag(harness);
  harness.windowTarget.dispatchEvent(pointerEvent("pointermove", { pointerId: 4, clientX: 20 }));
  harness.runFrames();
  assert.equal(harness.drafts.has(action.id), true);

  harness.setSelectedInteractionId("interaction-b");
  harness.controller.reconcileInteraction("interaction-b");
  assert.equal(harness.controller.dragging, false);
  assert.equal(harness.drafts.has(action.id), false);
  assert.equal(harness.commits.length, 0);
  assert.equal(harness.windowTarget.totalListenerCount(), 0);
});

test("seek-ul Motion publică cel mult o dată pe frame, face final flush și nu mută Rust", () => {
  const harness = createControllerHarness();
  const started = harness.controller.beginSeek({
    event: pointerEvent("pointerdown", { pointerId: 9, clientX: 10 }),
    captureTarget: harness.capture,
    interactionId: "interaction-a",
    domain: "time",
    canvasLeft: 0,
    canvasWidth: 100,
    timelineDuration: 1_000,
    snap: false,
    safetyTimeoutMs: 100,
  });
  assert.equal(started, true);
  assert.deepEqual(harness.seeks, [["interaction-a", 100]]);

  harness.windowTarget.dispatchEvent(pointerEvent("pointermove", { pointerId: 9, clientX: 20 }));
  harness.windowTarget.dispatchEvent(pointerEvent("pointermove", { pointerId: 9, clientX: 30 }));
  assert.deepEqual(harness.seeks, [["interaction-a", 100]]);
  harness.runFrames();
  assert.deepEqual(harness.seeks.at(-1), ["interaction-a", 300]);

  harness.windowTarget.dispatchEvent(pointerEvent("pointermove", { pointerId: 9, clientX: 40 }));
  harness.windowTarget.dispatchEvent(pointerEvent("pointerup", { pointerId: 9, clientX: 50 }));
  assert.deepEqual(harness.seeks.at(-1), ["interaction-a", 500]);
  assert.equal(harness.commits.length, 0);
  assert.equal(harness.controller.seeking, false);
  assert.equal(harness.windowTarget.totalListenerCount(), 0);
  assert.equal(harness.frames.size, 0);
});
