import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("Rust owns the inspector summary state machine and exact physical identity", () => {
  const rust = source("../src-tauri/src/kernel/selection_coordinator.rs");
  const commands = source("../src-tauri/src/commands/editor_navigation.rs");
  const io = source("../src/lib/editor/selection-io.ts");
  const canvas = source("../src/lib/state/canvas-interaction-selection.ts");

  for (const contract of [
    "InspectorSelectionSummaryState",
    "InspectorSelectionSummaryReason",
    "InspectorSelectionPhysicalFacts",
    "InspectorSelectionSummarySnapshot",
    "SelectionObservationReceipt",
  ]) {
    assert.match(rust, new RegExp(`(?:struct|enum) ${contract}\\b`));
  }
  for (const state of [
    "Empty",
    "Resolving",
    "Resolved",
    "NotRendered",
    "Ambiguous",
    "Uninspectable",
  ]) {
    assert.match(rust, new RegExp(`InspectorSelectionSummaryState::${state}\\b`));
  }

  const summaryStruct = rust.slice(
    rust.indexOf("pub struct InspectorSelectionSummarySnapshot"),
    rust.indexOf("pub struct SelectionObservationReceipt"),
  );
  assert.doesNotMatch(summaryStruct, /\bValue\b|serde_json/);
  assert.match(rust, /active_inspector_document:\s*Option<CanvasInteractionIdentity>/);
  assert.match(rust, /active_document\.document_epoch != input\.document_epoch/);
  assert.match(rust, /validate_inspector_facts\(selection, input\.inspector_facts\)/);
  assert.match(rust, /expected_selection_revision:\s*Option<u64>/);
  assert.match(rust, /current_revision != expected_revision/);
  assert.match(commands, /\.bind_inspector_document\(receipt\.identity\.clone\(\)\)/);
  assert.match(canvas, /inspectorFacts:\s*\{\s*observedTag:/);
  assert.match(io, /expectedInspectorStates/);
  assert.match(io, /InspectorSelectionSummary nu confirmă faptele fizice solicitate/);
});

test("the selection card is a pure accessible renderer of the Rust snapshot", () => {
  const card = source("../src/lib/components/inspector/SelectionSummaryCard.svelte");
  const inspector = source("../src/lib/components/InspectorPane.svelte");
  const htmlCoordinator = source("../src/lib/components/inspector/HtmlInspectorCoordinator.svelte");
  const cssCoordinator = source("../src/lib/components/inspector/CssInspectorCoordinator.svelte");
  const cssReader = source("../src/lib/inspector/css-inspector-reader.ts");
  const workspace = source("../src/lib/components/workspace/WorkspaceInspectorArea.svelte");
  const application = source("../src/lib/components/application/ApplicationWorkspace.svelte");

  assert.match(card, /InspectorSelectionSummarySnapshot/);
  assert.doesNotMatch(
    card,
    /selectionPresentation|CanvasElementObservation|resolvePageCssTarget|invoke\(/,
  );
  assert.match(card, /selection\?\.aggregateCapabilities\.memberCount/);
  assert.match(card, /selection\?\.aggregateHtmlFacts/);
  assert.match(card, /data-selection-aggregate="common"/);
  assert.match(card, /data-selection-aggregate="mixed"/);
  assert.match(card, /aggregateHtmlFacts\.commonClasses/);
  assert.match(card, /aggregateHtmlFacts\.mixedAttributeNames/);
  for (const state of [
    "empty",
    "resolving",
    "resolved",
    "notRendered",
    "ambiguous",
    "uninspectable",
  ]) {
    assert.match(card, new RegExp(`(?:case |=== )"${state}"`));
  }
  assert.match(card, /aria-live="polite"/);
  assert.match(card, /aria-pressed=\{summary\.activeCssClass === className\}/);
  assert.match(card, /ArrowLeft/);
  assert.match(card, /ArrowRight/);
  assert.match(card, /data-summary-state=/);
  assert.match(card, /summary && summary\.state !== "empty"/);
  assert.match(card, /summary\.classes\.length/);
  assert.match(card, /value\.subjectKind === "boundary"/);
  assert.match(card, /value\.boundaryKind === "component"/);
  assert.match(card, /data-entity-kind/);
  assert.match(card, /summary\.subjectKind === "runtimeElement"/);

  assert.match(inspector, /import SelectionSummaryCard/);
  assert.match(inspector, /summary=\{presentedInspectorSelectionSummary\}/);
  assert.match(htmlCoordinator, /advanceStableHtmlInspectorProjection/);
  assert.match(inspector, /aria-busy=\{htmlProjectionPending\}/);
  assert.match(inspector, /selectClass=\{selectClassForCss\}/);
  assert.doesNotMatch(inspector, /<section class="selection-card">/);
  assert.doesNotMatch(inspector, /^\s*\.selection-card\s*\{/m);
  const classIntent = inspector.slice(
    inspector.indexOf("async function selectClassForCss"),
    inspector.indexOf("$effect", inspector.indexOf("async function selectClassForCss")),
  );
  assert.match(classIntent, /changeInspectorTab\("css"\)/);
  assert.match(classIntent, /return "blocked"/);
  assert.match(classIntent, /cssCoordinator\?\.selectClass\(className\)/);
  const cssClassIntent = cssReader.slice(
    cssReader.indexOf("async selectClass"),
    cssReader.indexOf("\n  captureCurrentSelection()"),
  );
  assert.match(cssClassIntent, /expectedSelectionRevision/);
  assert.match(cssClassIntent, /const resolution = await this\.resolve/);
  assert.match(cssReader, /expectedWorkspaceRevision: input\.workspaceRevision/);
  assert.match(cssClassIntent, /expectedSelection/);
  assert.match(cssClassIntent, /resolution\.state === "ambiguous"/);
  assert.match(cssClassIntent, /kind: "targetFailed"/);
  assert.match(cssClassIntent, /this\.dependencies\.changeCodeTarget\?\./);
  assert.match(cssClassIntent, /return "blocked"/);
  assert.match(
    cssClassIntent,
    /this\.applyResolution\(resolution, currentSelection\);\s*return "allowed"/,
  );
  const session = source("../src/lib/state/editor-selection-session.svelte.ts");
  assert.match(application, /inspectorSelectionSummary:\s*selectionWorkspace\.session\.inspectorSummary/);
  assert.match(session, /inspectorSummary = \$state<InspectorSelectionSummarySnapshot \| null>/);
  assert.match(session, /this\.inspectorSummary = receipt\.inspectorSummary/);
  assert.doesNotMatch(application, /^\s*inspectorSelectionSummary = \$state/m);
});
