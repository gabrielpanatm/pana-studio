import assert from "node:assert/strict";
import { test } from "node:test";
import {
  isContinuousPixelWheel,
  normalizeWheelDelta,
} from "$lib/ui/smooth-wheel";

test("wheel discret este normalizat identic pe axele verticală și orizontală", () => {
  assert.deepEqual(
    normalizeWheelDelta({
      deltaMode: 1,
      deltaX: 0,
      deltaY: 3,
      pageHeightPx: 600,
      pageWidthPx: 900,
      shiftKey: false,
    }),
    { axis: "y", amount: 48 },
  );
  assert.deepEqual(
    normalizeWheelDelta({
      deltaMode: 1,
      deltaX: 0,
      deltaY: 3,
      pageHeightPx: 600,
      pageWidthPx: 900,
      shiftKey: true,
    }),
    { axis: "x", amount: 48 },
  );
});

test("inputul continuu de trackpad rămâne nativ", () => {
  assert.equal(isContinuousPixelWheel(0, 2.5, 8), true);
  assert.equal(isContinuousPixelWheel(0, 0, 53), false);
  assert.equal(isContinuousPixelWheel(1, 0, 3), false);
});
