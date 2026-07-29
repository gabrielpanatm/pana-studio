import assert from "node:assert/strict";
import { test } from "node:test";
import {
  deriveTerminalScrollProxyGeometry,
  terminalLineFromProxyScroll,
} from "$lib/terminal/scroll-proxy";
import { createTerminalTheme } from "$lib/terminal/runtime";

test("scrollbarul terminalului folosește toată înălțimea hostului", () => {
  const geometry = deriveTerminalScrollProxyGeometry({
    viewportHeightPx: 205,
    rows: 10,
    baseY: 30,
    viewportY: 30,
  });

  assert.equal(geometry.contentHeightPx, 820);
  assert.equal(geometry.scrollTopPx, 615);
  assert.equal(
    terminalLineFromProxyScroll(615, 615, geometry.maxLine),
    30,
  );
});

test("sincronizarea scrollbarului rămâne stabilă la început și fără scrollback", () => {
  const geometry = deriveTerminalScrollProxyGeometry({
    viewportHeightPx: 205,
    rows: 10,
    baseY: 0,
    viewportY: 0,
  });

  assert.deepEqual(geometry, {
    contentHeightPx: 205,
    maxLine: 0,
    scrollTopPx: 0,
  });
  assert.equal(terminalLineFromProxyScroll(100, 0, 0), 0);
});

test("tema terminalului folosește accentul efectiv al aplicației", () => {
  assert.deepEqual(
    {
      cursor: createTerminalTheme("light", "#c2410c").cursor,
      selection: createTerminalTheme("dark", "#c2410c").selectionBackground,
    },
    {
      cursor: "#c2410c",
      selection: "rgba(194, 65, 12, 0.22)",
    },
  );
  assert.equal(createTerminalTheme("light", "invalid").cursor, "#1d7f6a");
});
