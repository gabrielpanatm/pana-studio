import assert from "node:assert/strict";
import { test } from "node:test";
import { presentedWorkbenchDocumentId } from "$lib/workbench/document-tab-projection";

function activation(phase, documentId = "document:next") {
  return {
    serial: 2,
    phase,
    documentId,
    relativePath: "templates/next.html",
    surface: "code",
    cacheOutcome: "unknown",
    diagnostic: null,
    metrics: {
      intentMs: null,
      resolveMs: null,
      loadMs: null,
      surfaceMs: null,
      totalMs: null,
    },
  };
}

test("tabul prezintă imediat intenția latest-wins cât timp Rust o confirmă", () => {
  assert.equal(
    presentedWorkbenchDocumentId("document:current", activation("applying")),
    "document:next",
  );
  assert.equal(
    presentedWorkbenchDocumentId("document:current", activation("loading")),
    "document:next",
  );
});

test("intenția locală a clickului precede inclusiv settlement-ul vechi propagat de owner", () => {
  assert.equal(
    presentedWorkbenchDocumentId(
      "document:current",
      activation("ready", "document:previous"),
      "document:latest-click",
    ),
    "document:latest-click",
  );
});

test("ready, failed și o intenție incompletă folosesc numai selecția autoritativă", () => {
  assert.equal(
    presentedWorkbenchDocumentId("document:confirmed", activation("ready")),
    "document:confirmed",
  );
  assert.equal(
    presentedWorkbenchDocumentId("document:current", activation("failed")),
    "document:current",
  );
  assert.equal(
    presentedWorkbenchDocumentId("document:current", activation("applying", null)),
    "document:current",
  );
});
