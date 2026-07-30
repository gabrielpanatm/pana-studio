import assert from "node:assert/strict";
import { test } from "node:test";
import {
  advanceStableHtmlInspectorProjection,
} from "$lib/inspector/html-projection-stability";

function summary(revision, state = "resolved", renderInstanceId = `render:${revision}`) {
  return {
    projectRoot: "/project",
    runtimeSessionId: "session:runtime",
    selectionRevision: revision,
    state,
    subjectKind: "htmlElement",
    renderInstanceId,
  };
}

function selection(revision, renderInstanceId = `render:${revision}`) {
  return {
    projectRoot: "/project",
    runtimeSessionId: "session:runtime",
    selectionRevision: revision,
    resolution: "resolved",
    anchor: { renderInstanceId },
  };
}

function physicalFacts(revision, renderInstanceId = `render:${revision}`) {
  return {
    selectionRevision: revision,
    renderInstanceId,
    rect: { width: "320px", height: "80px", top: "0px", left: "0px" },
    hasChildElements: false,
    childElementCount: 0,
    zolaImage: null,
  };
}

function input(revision, overrides = {}) {
  return {
    summary: summary(revision),
    selection: selection(revision),
    physicalFacts: physicalFacts(revision),
    attributeValues: { id: `element-${revision}` },
    textContentValue: `Text ${revision}`,
    classEditorValue: `class-${revision}`,
    imageSourceValue: "",
    pendingTag: null,
    attributeStatus: `attributes-${revision}`,
    textStatus: `text-${revision}`,
    classStatus: `class-${revision}`,
    imageStatus: "",
    tagStatus: "",
    canEditHtml: true,
    isActivePreviewHtmlSource: true,
    ...overrides,
  };
}

test("HTML Inspector keeps the last complete projection while Rust resolves a new selection", () => {
  const initial = advanceStableHtmlInspectorProjection(null, input(1));
  assert.equal(initial.pending, false);
  assert.equal(initial.projection?.summary.selectionRevision, 1);

  const resolving = advanceStableHtmlInspectorProjection(initial.projection, input(2, {
    summary: summary(2, "resolving"),
    physicalFacts: null,
    attributeValues: {},
    textContentValue: "",
  }));

  assert.equal(resolving.pending, true);
  assert.equal(resolving.projection, initial.projection);
  assert.deepEqual(resolving.projection?.attributeValues, { id: "element-1" });
  assert.equal(resolving.projection?.textContentValue, "Text 1");
  assert.equal(resolving.projection?.canEditHtml, true);
  assert.equal(resolving.projection?.isActivePreviewHtmlSource, true);
});

test("HTML Inspector swaps semantic and physical data only as one complete revision", () => {
  const firstInput = input(1);
  const initial = advanceStableHtmlInspectorProjection(null, firstInput);
  firstInput.attributeValues.id = "mutated-after-capture";
  assert.deepEqual(initial.projection?.attributeValues, { id: "element-1" });

  const stalePhysical = advanceStableHtmlInspectorProjection(
    initial.projection,
    input(2, { physicalFacts: physicalFacts(1) }),
  );
  assert.equal(stalePhysical.pending, true);
  assert.equal(stalePhysical.projection?.summary.selectionRevision, 1);

  const completed = advanceStableHtmlInspectorProjection(
    stalePhysical.projection,
    input(2),
  );
  assert.equal(completed.pending, false);
  assert.equal(completed.projection?.summary.selectionRevision, 2);
  assert.equal(completed.projection?.selection.selectionRevision, 2);
  assert.equal(completed.projection?.physicalFacts.selectionRevision, 2);
  assert.deepEqual(completed.projection?.attributeValues, { id: "element-2" });
  assert.equal(completed.projection?.textContentValue, "Text 2");
});

test("a different runtime never inherits the previous HTML projection", () => {
  const initial = advanceStableHtmlInspectorProjection(null, input(1));
  const nextSummary = {
    ...summary(2, "resolving"),
    projectRoot: "/other",
    runtimeSessionId: "session:other",
  };
  const transition = advanceStableHtmlInspectorProjection(initial.projection, input(2, {
    summary: nextSummary,
    physicalFacts: null,
  }));

  assert.equal(transition.pending, false);
  assert.equal(transition.projection, null);
});
