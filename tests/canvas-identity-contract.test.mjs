import assert from "node:assert/strict";
import { test } from "node:test";
import {
  canvasRouteFromPreviewUrl,
  normalizeProjectDocumentPath,
  sameCanvasInteractionIdentity,
  sameCanvasProjectionIdentity,
  sameProjectDocumentPath,
} from "$lib/contracts/canvas-identity";

const canvas = {
  projectRoot: "/project",
  runtimeSessionId: "runtime-1",
  workspaceRevision: 7,
  transactionId: "canvas-7",
  previewRevision: "preview-7",
};

test("Canvas projection identity requires every causal field", () => {
  assert.equal(sameCanvasProjectionIdentity(canvas, { ...canvas }), true);
  assert.equal(sameCanvasProjectionIdentity(canvas, null), false);
  for (const [field, value] of [
    ["projectRoot", "/other"],
    ["runtimeSessionId", "runtime-2"],
    ["workspaceRevision", 8],
    ["transactionId", "canvas-8"],
    ["previewRevision", "preview-8"],
  ]) {
    assert.equal(
      sameCanvasProjectionIdentity(canvas, { ...canvas, [field]: value }),
      false,
      field,
    );
  }
});

test("Canvas interaction identity also binds route, epoch and agent", () => {
  const identity = {
    canvas,
    route: "/page/",
    documentEpoch: 11,
    agentInstanceId: "agent-1",
  };
  assert.equal(sameCanvasInteractionIdentity(identity, structuredClone(identity)), true);
  assert.equal(sameCanvasInteractionIdentity(identity, { ...identity, route: "/other/" }), false);
  assert.equal(sameCanvasInteractionIdentity(identity, { ...identity, documentEpoch: 12 }), false);
  assert.equal(sameCanvasInteractionIdentity(identity, { ...identity, agentInstanceId: "agent-2" }), false);
  assert.equal(sameCanvasInteractionIdentity(identity, {
    ...identity,
    canvas: { ...canvas, transactionId: "canvas-other" },
  }), false);
});

test("Canvas document paths have one project-relative canonical form", () => {
  assert.equal(normalizeProjectDocumentPath(" /./templates\\page.html "), "templates/page.html");
  assert.equal(normalizeProjectDocumentPath(".//templates///page.html"), "templates/page.html");
  assert.equal(normalizeProjectDocumentPath(null), "");
  assert.equal(
    sameProjectDocumentPath("/templates\\page.html", "./templates/page.html"),
    true,
  );
});

test("Canvas route comes from the preview URL with a normalized browser fallback", () => {
  assert.equal(canvasRouteFromPreviewUrl("http://127.0.0.1:41000/page/?draft=1", "/"), "/page/");
  assert.equal(canvasRouteFromPreviewUrl("about:blank", "fallback/"), "/fallback/");
  assert.equal(canvasRouteFromPreviewUrl("not a valid absolute url", "fallback"), "/not%20a%20valid%20absolute%20url");
  assert.equal(canvasRouteFromPreviewUrl(null, ""), "/");
});
