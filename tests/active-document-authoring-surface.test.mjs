import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("Rust proiectează explicit rădăcina persistentă a documentului activ", () => {
  const kernel = source("../src-tauri/src/kernel/canvas_interaction.rs");
  const navigation = source("../src-tauri/src/kernel/editor_navigation.rs");
  const types = source("../src/lib/types.ts");
  const io = source("../src/lib/project/io.ts");

  assert.match(kernel, /struct CanvasInteractionAuthoringSurface[\s\S]*source_node_id[\s\S]*boundary_instance_id[\s\S]*render_instance_id: Option<String>/);
  assert.match(
    kernel,
    /active_document_authoring_surfaces[\s\S]*active_document_path: Option<&str>[\s\S]*EditorNavigationNodeKind::TeraBoundary[\s\S]*active_document_authoring_source_kind[\s\S]*requires_edit_scope_id\.is_none\(\)[\s\S]*EditorNavigationOrigin::Project[\s\S]*normalized_canvas_document_path\(file\) == active_document_path/,
  );
  assert.match(kernel, /SourceNodeKind::Block \| SourceNodeKind::Template \| SourceNodeKind::Partial/);
  assert.match(kernel, /focused_authoring_context[\s\S]*root_render_instance_ids[\s\S]*render_instance_id: None/);
  assert.match(navigation, /add_empty_document_authoring_root/);
  assert.match(navigation, /EditorNavigationViewNodeKind::Slot/);
  assert.match(navigation, /editor_view_authoring_root/);
  assert.doesNotMatch(kernel, /synthetic_empty_slot/);
  assert.match(types, /CanvasInteractionBindingReceipt[\s\S]*authoringSurfaces: CanvasInteractionAuthoringSurface\[\]/);
  assert.match(io, /Array\.isArray\(receipt\.authoringSurfaces\)/);
});

test("Insert Engine scrie repetat numai în rădăcina deținută de documentul activ", () => {
  const commands = source("../src-tauri/src/commands/kernel_preview_context.rs");
  const executor = source("../src-tauri/src/kernel/preview_projection/executor/html.rs");
  const teraExecutor = source("../src-tauri/src/kernel/preview_projection/executor/tera.rs");
  const insertEngine = source("../src-tauri/src/project_model/insert_engine.rs");
  const teraInsertEngine = source("../src-tauri/src/project_model/tera_insert_engine.rs");
  const previewEngine = source("../src-tauri/src/preview/engine.rs");
  const emptyZones = source("../src-tauri/src/preview/bridge/06_empty_zones.js");
  const inject = source("../src-tauri/src/preview/preprocess/annotate/inject.rs");

  assert.match(commands, /active_document_path[\s\S]*workbench\.active_group_id[\s\S]*document\.relative_path\.clone\(\)/);
  assert.match(executor, /plan_html_insert\([\s\S]*active_document_path/);
  assert.match(executor, /target_kind[\s\S]*active-document-root[\s\S]*ProjectMovePosition::Before/);
  assert.match(insertEngine, /plan_html_insert_into_active_document_root/);
  assert.match(insertEngine, /target_node\.origin != SourceOrigin::Local/);
  assert.match(insertEngine, /same_model_path\(&target_node\.file, active_document_path\)/);
  assert.match(insertEngine, /sursei externe/);
  assert.doesNotMatch(insertEngine, /Inserarea în slot Tera cere planner Tera dedicat/);
  assert.match(teraExecutor, /plan_tera_insert_for_active_document[\s\S]*active_document_path/);
  assert.match(teraInsertEngine, /plan_tera_insert_for_active_document[\s\S]*same_model_path\(&target_node\.file, active_document_path\)[\s\S]*sursa externă/);
  assert.match(previewEngine, /pana-template-source-start:\{source_id\}[\s\S]*\{fragment\}[\s\S]*pana-template-source-end:\{source_id\}/);
  assert.match(insertEngine, /append_document_fragment\(&file\.contents, &inserted\)/);
  assert.match(teraInsertEngine, /apply_tera_insert_into_document_fragment_root/);
  assert.match(emptyZones, /meaningfulContentBetween\(pair\.start, pair\.end\)[\s\S]*activeDocumentRootBetween[\s\S]*data-pana-empty-label/);
  assert.match(emptyZones, /authoringSurfaceForSourceId\(pair\.id\)/);
  assert.match(emptyZones, /data-pana-empty-label", "Document gol"/);
  assert.match(emptyZones, /ACTIVE_AUTHORING_ATTR, authoringSurface\.boundaryInstanceId/);
  assert.doesNotMatch(emptyZones, /data-pana-empty-label", "Block Tera gol"/);
  assert.match(inject, /<div hidden class="pana-studio-empty-editable pana-studio-empty-tera-slot"/);
  assert.doesNotMatch(inject, /data-pana-empty-label="Block Tera gol"/);
});

test("Straturi și Inspector prezintă rădăcina locală drept document editabil", () => {
  const tree = source("../src/lib/components/project/EditorNavigationTree.svelte");
  const summary = source("../src/lib/components/inspector/SelectionSummaryCard.svelte");
  const inspector = source("../src/lib/components/InspectorPane.svelte");
  const teraCard = source("../src/lib/components/inspector/TeraSourceCard.svelte");

  assert.match(tree, /case "slot": return IconFileCode/);
  assert.match(inspector, /directAuthoringDocumentPath/);
  assert.match(summary, /authoringDocumentPath[\s\S]*inspector-summary-kind-document/);
  assert.match(teraCard, /directAuthoringBoundary[\s\S]*inspector-summary-kind-document/);
});

test("CanvasAgent prioritizează boundary-ul activ în zona vizuală extinsă", () => {
  const controller = source("../src/lib/state/canvas-interaction-controller.ts");
  const agent = source("../src-tauri/src/preview/bridge/03_canvas_agent.js");
  const emptyZones = source("../src-tauri/src/preview/bridge/06_empty_zones.js");
  const dragDrop = source("../src-tauri/src/preview/bridge/07_drag_drop.js");
  const geometry = source("../src-tauri/src/preview/bridge/03_overlay_geometry.js");

  assert.equal(controller.match(/authoringSurfaces: (?:binding|receipt)\.authoringSurfaces/g)?.length, 2);
  assert.match(agent, /configureActiveDocumentAuthoringSurfaces\(data\.authoringSurfaces\)/);
  assert.match(
    agent,
    /activeDocumentAuthoringTargetAtPoint[\s\S]*kind: "boundaryInstance"[\s\S]*boundaryInstanceId/,
  );
  assert.match(emptyZones, /activeDocumentAuthoringRectForElement[\s\S]*parent\.getBoundingClientRect\(\)[\s\S]*contentBottom/);
  assert.match(emptyZones, /fitActiveDocumentAuthoringFlow[\s\S]*authoredFlowBottom\(\)[\s\S]*residualViewportHeight[\s\S]*ACTIVE_AUTHORING_MIN_HEIGHT_PROPERTY/);
  assert.match(emptyZones, /ACTIVE_AUTHORING_POPULATED_ATTR[\s\S]*ACTIVE_AUTHORING_APPEND_HIT_HEIGHT/);
  assert.match(emptyZones, /hasAttribute\(ACTIVE_AUTHORING_POPULATED_ATTR\)[\s\S]*appendBottom[\s\S]*appendTop/);
  assert.match(emptyZones, /hasContent[\s\S]*setAttribute\(ACTIVE_AUTHORING_POPULATED_ATTR, "true"\)/);
  assert.match(emptyZones, /configureActiveDocumentAuthoringSurfaces[\s\S]*refreshActiveDocumentRoots\(\)[\s\S]*refreshEmptyHtmlAffordances\(\)/);
  assert.doesNotMatch(emptyZones, /Math\.ceil\(naturalHeight \+ residualViewportHeight\)/);
  assert.match(emptyZones, /window\.addEventListener\("resize", invalidateActiveDocumentAuthoringLayout\)/);
  assert.match(emptyZones, /window\.addEventListener\("scroll", scheduleActiveDocumentAuthoringRefresh, true\)/);
  assert.match(emptyZones, /removeDuplicateActiveDocumentRoots/);
  assert.match(emptyZones, /authoringRootForSurface[\s\S]*ACTIVE_DOCUMENT_ROOT_ATTR[\s\S]*surface\.sourceNodeId/);
  assert.match(agent, /canvasAgentProjectionElements[\s\S]*activeDocumentAuthoringElementForBoundary/);
  assert.match(emptyZones, /data-pana-canvas-agent-overlay", "authoring"/);
  assert.match(dragDrop, /previewDragTargetFromPoint[\s\S]*activeDocumentAuthoringTargetAtPoint/);
  assert.match(dragDrop, /targetBoundaryInstanceId:\s*closestPreviewSourceAttribute\(drop\.target, ACTIVE_AUTHORING_ATTR\)/);
  assert.match(dragDrop, /activeDocumentAuthoringRectForElement\(target\) \|\| target\.getBoundingClientRect\(\)/);
  assert.match(geometry, /activeDocumentAuthoringRectForElement\(element\) \|\| element\.getBoundingClientRect\(\)/);
});
