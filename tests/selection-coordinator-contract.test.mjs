import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("SelectionCoordinator is the sole semantic selection authority", () => {
  const rust = source("../src-tauri/src/kernel/selection_coordinator.rs");
  const selectionWorkspace = source("../src/lib/editor/selection-workspace.svelte.ts");
  const session = source("../src/lib/state/editor-selection-session.svelte.ts");
  const canvas = `${source("../src/lib/state/canvas-interaction-gestures.ts")}\n${source("../src/lib/state/canvas-interaction-selection.ts")}`;
  const inspector = source("../src/lib/components/inspector/CssInspectorCoordinator.svelte");
  const inspectorState = source("../src/lib/inspector/css-inspector-state.svelte.ts");

  for (const contract of [
    "SelectionIntent",
    "SelectionSnapshot",
    "SelectionSubject",
    "SelectionFocus",
    "SelectionAnchor",
    "SelectionResolution",
    "HoverSnapshot",
  ]) {
    assert.match(rust, new RegExp(`(?:struct|enum) ${contract}\\b`));
  }
  assert.match(session, /selectionSnapshot = \$state<SelectionSnapshot \| null>/);
  assert.match(session, /readonly diagnostics = emptyDiagnostics\(\)/);
  assert.doesNotMatch(session, /diagnostics = \$state/);
  assert.match(selectionWorkspace, /this\.session = new EditorSelectionSessionController/);
  assert.doesNotMatch(selectionWorkspace, /^\s*selectionSnapshot = \$state/m);
  assert.match(canvas, /message\.pointer\.modifiers\.shift[\s\S]*kind: "extendRangeToEditorNode"/);
  assert.match(canvas, /message\.pointer\.modifiers\.control \|\| message\.pointer\.modifiers\.meta[\s\S]*kind: "toggleEditorNode"/);
  assert.match(canvas, /app\.selection\.editorSelection\.applySelectionIntent\(intent\)/);
  assert.match(canvas, /app\.selection\.editorSelection\.applyHoverIntent\(/);
  assert.match(inspectorState, /get selectedClass\(\)/);
  assert.match(inspectorState, /this\.coordinatedSelector\?\.selectedClass/);

  const combined = `${selectionWorkspace}\n${session}\n${canvas}\n${inspector}\n${inspectorState}`;
  assert.doesNotMatch(
    combined,
    /\bselectedElement\b|\bselectedTemplateSourceId\b|\bselectedEditorNavigationNodeId\b|\bselectedReceipt\b|\bselectedKey\b/,
  );
});
test("DOM observation and source cursor remain subordinate to Rust identity", () => {
  const canvas = source("../src/lib/state/canvas-interaction-selection.ts");
  const sourceEditor = source("../src/lib/editor/source-workspace.svelte.ts");
  const bridge = source("../src-tauri/src/preview/bridge/03_canvas_agent.js");
  const rust = source("../src-tauri/src/kernel/selection_coordinator.rs");

  assert.match(canvas, /selectionRevision: pending\.selectionRevision/);
  assert.match(canvas, /app\.selection\.editorSelection\.acceptObservation/);
  assert.match(canvas, /selection\.selectionRevision !== message\.selectionRevision/);
  assert.match(rust, /Observația DOM aparține unei revizii de selecție vechi/);
  assert.match(sourceEditor, /new TextEncoder\(\)\.encode/);
  assert.match(sourceEditor, /selectSourcePosition/);
  assert.doesNotMatch(
    sourceEditor,
    /querySelector|findHtmlNodeAtPosition|codeSelectionRangeForSelection|cssSelectorAtPosition|codeSelectionRangeForCssSelector/,
  );
  assert.match(rust, /range:\s*Option<SourceRange>/);
  assert.match(bridge, /selectionRevision/);
  assert.doesNotMatch(bridge, /selectedReceipt|selectedKey/);
  assert.doesNotMatch(rust, /serde_json::Value|\bpayload:\s*Value\b/);
});

test("rebase has no body or first-instance fallback", () => {
  const rust = source("../src-tauri/src/kernel/selection_coordinator.rs");
  const rebase = rust.slice(
    rust.indexOf("fn rebase_candidate"),
    rust.indexOf("fn selection_from_node"),
  );

  assert.match(rebase, /editor_node_id/);
  assert.match(rebase, /render_instance_id/);
  assert.match(rebase, /source_node_id/);
  assert.match(rebase, /anchor\.editor_node_id\.is_some\(\) \|\| !anchor\.render_instance_ids\.is_empty\(\)/);
  assert.match(rebase, /RebaseCandidate::Ambiguous/);
  assert.doesNotMatch(rebase, /component_invocation_ids|binding_path|binding_key|body|first\(\)/);
});

test("selection-driven mutations carry a Rust-validated revision", () => {
  const model = source("../src-tauri/src/kernel/preview_projection/model.rs");
  const coordinator = source("../src-tauri/src/kernel/selection_coordinator.rs");
  const pipeline = source("../src-tauri/src/commands/kernel_preview_pipeline.rs");
  const css = source("../src-tauri/src/commands/css.rs");
  const lane = source("../src/lib/kernel/preview-structural-lane.ts");
  const inspector = source("../src/lib/components/inspector/CssInspectorCoordinator.svelte");
  const queue = source("../src/lib/inspector/css-inspector-mutation-queue.ts");

  assert.match(model, /PreviewStructuralSelectionIdentity/);
  assert.match(coordinator, /struct SelectionMutationIdentity/);
  assert.match(coordinator, /selection_revision: u64/);
  assert.match(pipeline, /selection_coordinator\.with_mutation_target/);
  assert.match(css, /execute_selection_bound_css_workspace_mutation/);
  assert.match(css, /with_stable_semantic_mutation_target/);
  assert.match(lane, /expectedSelection: lease\.selection/);
  assert.match(lane, /requireCapturedSelection/);
  assert.match(queue, /expectedSelection/);
  assert.match(queue, /sameCssSemanticSelection/);
  assert.match(queue, /cssSemanticSelectionKey\(expectedSelection\)/);
});
