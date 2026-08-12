import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("SelectionCoordinator is the sole semantic selection authority", () => {
  const rust = source("../src-tauri/src/kernel/selection_coordinator.rs");
  const app = source("../src/lib/state/app.svelte.ts");
  const canvas = source("../src/lib/state/canvas-interaction-controller.ts");
  const inspector = source("../src/lib/components/InspectorPane.svelte");

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
  assert.match(app, /selectionSnapshot = \$state<SelectionSnapshot \| null>/);
  assert.match(canvas, /message\.pointer\.modifiers\.shift[\s\S]*kind: "extendRangeToEditorNode"/);
  assert.match(canvas, /message\.pointer\.modifiers\.control \|\| message\.pointer\.modifiers\.meta[\s\S]*kind: "toggleEditorNode"/);
  assert.match(canvas, /app\.applySelectionIntent\(intent\)/);
  assert.match(canvas, /app\.applyHoverIntent\(/);
  assert.match(inspector, /const selectedClass = \$derived\(coordinatedCssState/);

  const combined = `${app}\n${canvas}\n${inspector}`;
  assert.doesNotMatch(
    combined,
    /\bselectedElement\b|\bselectedTemplateSourceId\b|\bselectedEditorNavigationNodeId\b|\bselectedReceipt\b|\bselectedKey\b/,
  );
});

test("DOM observation and source cursor remain subordinate to Rust identity", () => {
  const canvas = source("../src/lib/state/canvas-interaction-controller.ts");
  const sourceEditor = source("../src/lib/state/source-editor-controller.ts");
  const bridge = source("../src-tauri/src/preview/bridge/03_canvas_agent.js");
  const rust = source("../src-tauri/src/kernel/selection_coordinator.rs");

  assert.match(canvas, /selectionRevision: pending\.selectionRevision/);
  assert.match(canvas, /app\.acceptSelectionObservation/);
  assert.match(canvas, /selection\.selectionRevision !== message\.selectionRevision/);
  assert.match(rust, /Observația DOM aparține unei revizii de selecție vechi/);
  assert.match(sourceEditor, /new TextEncoder\(\)\.encode/);
  assert.match(sourceEditor, /selectSourcePositionFromCode/);
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
  const inspector = source("../src/lib/components/InspectorPane.svelte");

  assert.match(model, /PreviewStructuralSelectionIdentity/);
  assert.match(coordinator, /struct SelectionMutationIdentity/);
  assert.match(coordinator, /selection_revision: u64/);
  assert.match(pipeline, /selection_coordinator\.with_mutation_target/);
  assert.match(css, /execute_selection_bound_css_workspace_mutation/);
  assert.match(css, /with_stable_semantic_mutation_target/);
  assert.match(lane, /expectedSelection: lease\.selection/);
  assert.match(lane, /requireCapturedSelection/);
  assert.match(inspector, /expectedSelection/);
  assert.match(inspector, /sameCssSemanticSelection/);
  assert.match(inspector, /cssSemanticSelectionKey\(expectedSelection\)/);
});
