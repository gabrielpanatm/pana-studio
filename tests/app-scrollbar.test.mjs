import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import {
  appScrollbarGeometry,
  appScrollbarOffsetFromThumbDelta,
  appScrollbarOffsetFromTrackPoint,
} from "../src/lib/ui/app-scrollbar.ts";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("geometria scrollbarului este proporțională și respectă dimensiunea minimă", () => {
  const geometry = appScrollbarGeometry(100, 400, 150, 100, 28);

  assert.equal(geometry.overflow, true);
  assert.equal(geometry.maxScrollOffset, 300);
  assert.equal(geometry.thumbSize, 28);
  assert.equal(geometry.thumbTravel, 72);
  assert.equal(geometry.thumbOffset, 36);
});

test("geometria neutralizează valori invalide și limitează poziția", () => {
  assert.deepEqual(
    appScrollbarGeometry(100, 80, 30, 96, 28),
    {
      overflow: false,
      viewportSize: 100,
      contentSize: 80,
      trackSize: 96,
      maxScrollOffset: 0,
      thumbSize: 96,
      thumbTravel: 0,
      thumbOffset: 0,
    },
  );

  const clamped = appScrollbarGeometry(100, 500, Number.POSITIVE_INFINITY, 100);
  assert.equal(clamped.thumbOffset, 0);
  assert.equal(appScrollbarGeometry(100, 500, 9999, 100).thumbOffset, 72);
});

test("drag-ul și click-ul pe pistă sunt mapate în offsetul nativ", () => {
  const geometry = appScrollbarGeometry(100, 400, 150, 100, 28);

  assert.equal(appScrollbarOffsetFromThumbDelta(geometry, 150, 18), 225);
  assert.equal(appScrollbarOffsetFromThumbDelta(geometry, 290, 18), 300);
  assert.equal(appScrollbarOffsetFromThumbDelta(geometry, 10, -18), 0);
  assert.equal(appScrollbarOffsetFromTrackPoint(geometry, 50), 150);
  assert.equal(appScrollbarOffsetFromTrackPoint(geometry, -20), 0);
  assert.equal(appScrollbarOffsetFromTrackPoint(geometry, 200), 300);
});

test("AppScrollArea deține aspectul, interacțiunea și ascunderea ornamentului nativ", () => {
  const component = source("../src/lib/components/ui/AppScrollArea.svelte");
  const designSystem = source("../src/routes/design-system.css");

  assert.match(component, /data-app-scroll-area/);
  assert.match(component, /role="scrollbar"/);
  assert.match(component, /aria-controls=\{viewportId\}/);
  assert.match(component, /setPointerCapture/);
  assert.match(component, /ResizeObserver/);
  assert.match(component, /MutationObserver/);
  assert.match(component, /scrollbar-width:\s*none/);
  assert.match(component, /app-scroll-viewport::\-webkit-scrollbar[\s\S]*display:\s*none/);
  assert.match(component, /axis === "vertical" \|\| axis === "both"/);
  assert.match(component, /axis === "horizontal" \|\| axis === "both"/);
  assert.match(designSystem, /--app-scrollbar-indicator-size:\s*3px/);
  assert.match(designSystem, /--app-scrollbar-slider-size:\s*7px/);
});
