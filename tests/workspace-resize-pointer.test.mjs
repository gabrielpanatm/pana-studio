import assert from "node:assert/strict";
import { afterEach, beforeEach, test } from "node:test";
import {
  beginResizeDrag,
  clearResizeBodyClasses,
} from "$lib/ui/resize";
import {
  createPointerHarness,
  keyboardEvent,
  pointerEvent,
} from "./pointer-session-test-helpers.mjs";

let originalDocument;
let bodyClasses;

beforeEach(() => {
  originalDocument = globalThis.document;
  bodyClasses = new Set();
  globalThis.document = {
    body: {
      classList: {
        add: (...names) => names.forEach((name) => bodyClasses.add(name)),
        remove: (...names) => names.forEach((name) => bodyClasses.delete(name)),
        toggle(name, force) {
          if (force) bodyClasses.add(name);
          else bodyClasses.delete(name);
        },
      },
    },
  };
});

afterEach(() => {
  globalThis.document = originalDocument;
});

test("resize rollback restaurează snapshotul și curăță sesiunea la Escape", () => {
  const harness = createPointerHarness();
  let liveState = { leftPaneWidth: 260, rightPaneWidth: 320, terminalPaneHeight: 240 };
  const committed = [];
  let stops = 0;
  beginResizeDrag({
    kind: "left",
    event: pointerEvent("pointerdown", { pointerId: 21, clientX: 100 }),
    state: liveState,
    captureTarget: harness.capture,
    environment: harness.environment,
    applyLiveState: (next) => {
      liveState = next;
    },
    onUpdate: (next) => {
      liveState = next;
      committed.push(next);
    },
    onStop: () => {
      stops += 1;
      clearResizeBodyClasses();
    },
  });

  harness.windowTarget.dispatchEvent(pointerEvent("pointermove", {
    pointerId: 21,
    clientX: 180,
  }));
  harness.runFrames();
  assert.equal(liveState.leftPaneWidth, 340);
  assert.equal(bodyClasses.has("is-resizing"), true);

  harness.windowTarget.dispatchEvent(keyboardEvent("Escape"));
  assert.deepEqual(liveState, {
    leftPaneWidth: 260,
    rightPaneWidth: 320,
    terminalPaneHeight: 240,
  });
  assert.equal(committed.length, 1);
  assert.equal(stops, 1);
  assert.equal(bodyClasses.size, 0);
  assert.equal(harness.windowTarget.totalListenerCount(), 0);
  assert.deepEqual(harness.capture.released, [21]);
});

test("resize pointerup publică poziția finală chiar dacă RAF nu a rulat", () => {
  const harness = createPointerHarness();
  const committed = [];
  beginResizeDrag({
    kind: "terminal",
    event: pointerEvent("pointerdown", { pointerId: 22, clientY: 400 }),
    state: { leftPaneWidth: 260, rightPaneWidth: 320, terminalPaneHeight: 240 },
    captureTarget: harness.capture,
    environment: harness.environment,
    applyLiveState: () => {},
    onUpdate: (next) => committed.push(next),
    onStop: clearResizeBodyClasses,
  });
  harness.windowTarget.dispatchEvent(pointerEvent("pointermove", {
    pointerId: 22,
    clientY: 350,
  }));
  harness.windowTarget.dispatchEvent(pointerEvent("pointerup", {
    pointerId: 22,
    clientY: 300,
  }));

  assert.equal(committed.length, 1);
  assert.equal(committed[0].terminalPaneHeight, 340);
  assert.equal(harness.frames.size, 0);
  assert.equal(harness.windowTarget.totalListenerCount(), 0);
  assert.equal(bodyClasses.size, 0);
});
