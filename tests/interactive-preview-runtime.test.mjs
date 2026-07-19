import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import {
  buildInteractivePreviewUrl,
  parseInteractivePreviewMessage,
} from "$lib/preview/interactive";

const identity = {
  projectRoot: "/project",
  runtimeSessionId: "session:runtime",
  workspaceRevision: 12,
  transactionId: "canvas-12",
  previewRevision: "preview-12",
};

test("Interactive Preview URL is bound to one loopback Canvas revision", () => {
  const result = new URL(buildInteractivePreviewUrl(
    "http://127.0.0.1:41234/despre/?__pana_reload=9&x=1",
    identity,
  ));
  assert.equal(result.origin, "http://127.0.0.1:41234");
  assert.equal(result.pathname, "/despre/");
  assert.equal(result.searchParams.get("x"), "1");
  assert.equal(result.searchParams.get("__pana_view"), "interactive");
  assert.equal(result.searchParams.get("__pana_preview_revision"), "preview-12");
  assert.equal(result.searchParams.get("__pana_canvas_transaction"), "canvas-12");
  assert.equal(result.searchParams.has("__pana_reload"), false);
  assert.equal(buildInteractivePreviewUrl("https://example.com/", identity), "");
});

test("Interactive DOM inspection is exact-frame, exact-revision and bounded", () => {
  const contentWindow = {};
  const frame = { contentWindow };
  const event = {
    source: contentWindow,
    data: {
      source: "pana-studio-interactive",
      schemaVersion: 1,
      type: "dom-snapshot",
      previewRevision: "preview-12",
      reason: "mutation",
      truncated: false,
      nodes: Array.from({ length: 5002 }, (_, index) => ({
        tag: "DIV",
        id: `node-${index}`,
        classes: ["card"],
        sourceId: index === 0 ? "source-1" : null,
        renderInstanceId: null,
        depth: 999,
        text: "x".repeat(300),
      })),
    },
  };
  const parsed = parseInteractivePreviewMessage(frame, event, "preview-12");
  assert.equal(parsed.type, "dom-snapshot");
  assert.equal(parsed.nodes.length, 5000);
  assert.equal(parsed.truncated, true);
  assert.equal(parsed.nodes[0].tag, "div");
  assert.equal(parsed.nodes[0].depth, 64);
  assert.equal(parsed.nodes[0].text.length, 160);
  assert.equal(parseInteractivePreviewMessage(frame, event, "preview-străin"), null);
  assert.equal(parseInteractivePreviewMessage({ contentWindow: {} }, event, "preview-12"), null);
});

test("Interactive realm has no Tauri/file bridge and never uses allow-same-origin", () => {
  const runtime = readFileSync(
    new URL("../src-tauri/src/preview/interactive_runtime.js", import.meta.url),
    "utf8",
  );
  const surface = readFileSync(
    new URL("../src/lib/components/preview/InteractivePreviewSurface.svelte", import.meta.url),
    "utf8",
  );
  assert.doesNotMatch(runtime, /__TAURI__|invoke\(|@tauri-apps|readFile|writeFile/);
  assert.match(runtime, /PanaInteractiveRuntime/);
  assert.match(runtime, /mount/);
  assert.match(runtime, /dispose/);
  assert.match(surface, /sandbox="allow-scripts"/);
  assert.doesNotMatch(surface, /allow-same-origin|allow-forms|allow-popups|allow-top-navigation/);
  assert.match(surface, /ultima revizie interactivă validă rămâne activă/);
});
