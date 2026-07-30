import assert from "node:assert/strict";
import { test } from "node:test";
import {
  cssInspectorSubjectKey,
  cssSemanticSelectionKey,
  sameCssSemanticSelection,
} from "$lib/inspector/css-selection-stability";

const captured = Object.freeze({
  selectionRevision: 16,
  editorNodeId: "editor:hero-title",
  sourceNodeId: "source:hero-title",
  renderInstanceId: "render:hero-title",
});

test("CSS editing keeps one semantic session across focus-only revisions", () => {
  const focused = Object.freeze({
    ...captured,
    selectionRevision: 17,
  });

  assert.equal(sameCssSemanticSelection(captured, focused), true);
  assert.equal(
    cssSemanticSelectionKey(captured),
    cssSemanticSelectionKey(focused),
  );
});

test("CSS editing rejects a newer revision that belongs to another element", () => {
  const otherElement = Object.freeze({
    selectionRevision: 17,
    editorNodeId: "editor:subtitle",
    sourceNodeId: "source:subtitle",
    renderInstanceId: "render:subtitle",
  });

  assert.equal(sameCssSemanticSelection(captured, otherElement), false);
  assert.notEqual(
    cssSemanticSelectionKey(captured),
    cssSemanticSelectionKey(otherElement),
  );
});

test("CSS semantic identity requires at least one Rust-owned anchor", () => {
  const emptyAt16 = { selectionRevision: 16 };
  const emptyAt17 = { selectionRevision: 17 };

  assert.equal(sameCssSemanticSelection(emptyAt16, emptyAt17), false);
  assert.equal(cssSemanticSelectionKey(emptyAt16), "");
});

test("CSS Inspector keeps its subject across a preview render replacement", () => {
  const rebased = {
    selectionRevision: 18,
    editorNodeId: "editor:hero-title:next-render",
    sourceNodeId: captured.sourceNodeId,
    renderInstanceId: "render:hero-title:next-render",
  };

  assert.notEqual(cssSemanticSelectionKey(captured), cssSemanticSelectionKey(rebased));
  assert.equal(cssInspectorSubjectKey(captured), cssInspectorSubjectKey(rebased));
});
