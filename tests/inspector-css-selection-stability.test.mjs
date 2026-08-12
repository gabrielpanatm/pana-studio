import assert from "node:assert/strict";
import { test } from "node:test";
import {
  cssInspectorSubjectKey,
  cssSemanticSelectionKey,
  sameCssSemanticSelection,
} from "$lib/inspector/css-selection-stability";

function identity(selectionRevision, editorNodeId, sourceNodeId, renderInstanceId) {
  return Object.freeze({
    selectionRevision,
    workspaceRevision: 7,
    primaryMemberId: editorNodeId,
    members: [{ memberId: editorNodeId, editorNodeId, sourceNodeId, renderInstanceId }],
  });
}

const captured = identity(
  16,
  "editor:hero-title",
  "source:hero-title",
  "render:hero-title",
);

test("CSS editing keeps one semantic session across focus-only revisions", () => {
  const focused = identity(
    17,
    "editor:hero-title",
    "source:hero-title",
    "render:hero-title",
  );

  assert.equal(sameCssSemanticSelection(captured, focused), true);
  assert.equal(
    cssSemanticSelectionKey(captured),
    cssSemanticSelectionKey(focused),
  );
});

test("CSS editing rejects a newer revision that belongs to another element", () => {
  const otherElement = identity(
    17,
    "editor:subtitle",
    "source:subtitle",
    "render:subtitle",
  );

  assert.equal(sameCssSemanticSelection(captured, otherElement), false);
  assert.notEqual(
    cssSemanticSelectionKey(captured),
    cssSemanticSelectionKey(otherElement),
  );
});

test("CSS semantic identity requires at least one Rust-owned anchor", () => {
  const emptyAt16 = { selectionRevision: 16, workspaceRevision: 7, primaryMemberId: null, members: [] };
  const emptyAt17 = { selectionRevision: 17, workspaceRevision: 7, primaryMemberId: null, members: [] };

  assert.equal(sameCssSemanticSelection(emptyAt16, emptyAt17), false);
  assert.equal(cssSemanticSelectionKey(emptyAt16), "");
});

test("CSS Inspector keeps its subject across a preview render replacement", () => {
  const rebased = identity(
    18,
    "editor:hero-title:next-render",
    "source:hero-title",
    "render:hero-title:next-render",
  );

  assert.notEqual(cssSemanticSelectionKey(captured), cssSemanticSelectionKey(rebased));
  assert.equal(cssInspectorSubjectKey(captured), cssInspectorSubjectKey(rebased));
});
