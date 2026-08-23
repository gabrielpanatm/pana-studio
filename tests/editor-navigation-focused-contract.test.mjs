import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

function editorNavigationSource() {
  return ["mod", "contracts", "runtime", "move_planner", "provenance", "snapshot", "view"]
    .map((module) => source(`../src-tauri/src/kernel/editor_navigation/${module}.rs`))
    .join("\n");
}

test("Straturi păstrează Canvas global și proiectează separat documentul activ", () => {
  const rust = editorNavigationSource();
  const tree = source("../src/lib/components/project/EditorNavigationTree.svelte");
  const pane = source("../src/lib/components/ProjectPane.svelte");

  assert.match(rust, /EDITOR_NAVIGATION_SCHEMA_VERSION:\s*u32\s*=\s*4/);
  assert.match(rust, /pub struct EditorNavigationView/);
  assert.match(rust, /pub focused_view:\s*Option<EditorNavigationView>/);
  assert.match(rust, /#\[serde\(skip\)\]\s*pub\(super\) planning_nodes/);
  assert.doesNotMatch(rust, /nodes\.extend\(source_editor_nodes\)/);
  assert.match(tree, /const focusedView = \$derived\(snapshot\?\.focusedView/);
  assert.match(tree, /focusedView\?\.rootNodeIds/);
  assert.match(rust, /source_kind_is_visual_layer/);
  assert.match(rust, /document_wrapper_block/);
  assert.match(rust, /rebuild_visual_hierarchy/);
  assert.match(tree, /openScope\?\.children \?\? focusedView\?\.rootNodeIds/);
  assert.match(tree, /if \(node\.kind === "boundary"\) return/);
  assert.doesNotMatch(tree, /nodeDetail/);
  assert.doesNotMatch(tree, /boundaryClosed/);
  assert.match(tree, /class="delete-action"/);
  assert.match(tree, /deleteNode\(resolved\)/);
  assert.doesNotMatch(tree, /import AppScrollArea/);
  assert.doesNotMatch(tree, /<AppScrollArea/);
  assert.match(tree, /class="tree-viewport"/);
  assert.match(tree, /overflow-y:\s*auto/);
  assert.match(tree, /overflow-x:\s*hidden/);
  assert.doesNotMatch(tree, /scrollbar-width:\s*none/);
  assert.doesNotMatch(tree, /tree-viewport::\-webkit-scrollbar/);
  assert.doesNotMatch(tree, /class="tree-scrollbar"/);
  assert.match(pane, /<EditorNavigationTree/);
  assert.match(pane, /deleteNode=\{deleteEditorNavigationNode\}/);
  assert.doesNotMatch(pane, /<ProjectLayersTab/);
});

test("Shift-range rămâne structural Rust chiar când ramuri din Straturi sunt colapsate", () => {
  const tree = source("../src/lib/components/project/EditorNavigationTree.svelte");
  const selectStart = tree.indexOf("function selectViewNode(");
  const selectEnd = tree.indexOf("function openViewNodeContextMenu(", selectStart);
  const selectionGesture = tree.slice(selectStart, selectEnd);

  assert.match(tree, /let collapsed = \$state\(new Set<string>\(\)\)/);
  assert.match(selectionGesture, /extendRange:\s*Boolean\(event\?\.shiftKey\)/);
  assert.doesNotMatch(selectionGesture, /collapsed|rows|selectedNodeIds|\.map\(/);
});

test("click-dreapta în Straturi deschide meniul aplicației pentru nodul Rust curent", () => {
  const tree = source("../src/lib/components/project/EditorNavigationTree.svelte");
  const pane = source("../src/lib/components/ProjectPane.svelte");
  const projectArea = source(
    "../src/lib/components/workspace/WorkspaceProjectArea.svelte",
  );
  const navigation = source("../src/lib/editor/navigation-service.ts");

  assert.match(tree, /event\.preventDefault\(\)/);
  assert.match(tree, /event\.stopPropagation\(\)/);
  assert.match(
    tree,
    /openContextMenu\(resolved, event\.clientX, event\.clientY\)/,
  );
  assert.match(
    tree,
    /oncontextmenu=\{\(event\) => openViewNodeContextMenu\(row\.node, event\)\}/,
  );
  assert.match(pane, /openContextMenu=\{openEditorNavigationContextMenu\}/);
  assert.match(
    projectArea,
    /openEditorNavigationContextMenu=\{commands\.openEditorNavigationContextMenu\}/,
  );
  assert.match(navigation, /async openContextMenu\(/);
  assert.match(navigation, /candidate\.id === requestedNode\.id/);
  assert.match(navigation, /htmlElementContextMenuItems\(/);
  assert.match(navigation, /teraContextMenuItems\(/);
  assert.match(navigation, /source:\s*"layers"/);
});

test("Preview și Straturi proiectează același boundary închis prin resolverul Rust", () => {
  const access = source("../src-tauri/src/kernel/editor_navigation/access.rs");
  const canvas = source("../src-tauri/src/kernel/canvas_interaction.rs");
  const commands = source("../src-tauri/src/commands/editor_navigation.rs");
  const selectionIo = source("../src/lib/editor/selection-io.ts");
  const session = source("../src/lib/state/editor-selection-session.svelte.ts");
  const tree = source("../src/lib/components/project/EditorNavigationTree.svelte");

  assert.match(access, /pub\(crate\) fn editor_navigation_access_node/);
  assert.match(canvas, /editor_navigation_access_node\(/);
  assert.doesNotMatch(canvas, /fn closed_boundary_or_node/);
  assert.match(commands, /selection_intent_with_access\(/);
  assert.match(commands, /authorize_selection_edit_scope\(/);
  assert.match(selectionIo, /editScopeGrant/);
  assert.match(session, /intent,\s*this\.editScopeGrant/);
  assert.match(tree, /revealPrimaryNode/);
});

test("ștergerea din Straturi autorizează mutația numai prin identitatea Rust", () => {
  const navigation = source("../src/lib/editor/navigation-service.ts");
  const actions = source("../src/lib/editor/html-actions/structure.ts");
  const deleteMethod = navigation.slice(
    navigation.indexOf("async deleteNode"),
    navigation.indexOf("async applyNativeBlockSlotMutation"),
  );

  assert.match(deleteMethod, /renderInstanceId:\s*node\.renderInstanceId/);
  assert.match(deleteMethod, /sourceId:\s*node\.sourceNodeId/);
  assert.match(deleteMethod, /sessionId:\s*this\.dependencies\.canvas\.session\.activeCanvasIdentity\?\.runtimeSessionId/);
  const deleteAction = actions.slice(
    actions.indexOf("export async function deleteSelectedHtmlElement"),
    actions.indexOf("export async function duplicateSelectedHtmlElement"),
  );
  assert.match(deleteAction, /if \(!target\.sourceId\)/);
  assert.match(deleteAction, /targetSourceId:\s*target\.sourceId/);
  assert.doesNotMatch(deleteAction, /targetLocation|sourceLocationForSourceReference/);
  assert.doesNotMatch(actions, /function sourceLocationForSourceReference/);
});

test("documentul activ și mutațiile sunt validate de Workbench-ul Rust", () => {
  const command = source("../src-tauri/src/commands/editor_navigation.rs");
  const kernel = editorNavigationSource();
  const io = source("../src/lib/editor/navigation-io.ts");
  const controller = source("../src/lib/state/editor-navigation-controller.ts");

  assert.match(command, /state\.workbench\.read\(session\)/);
  assert.match(command, /authoritative_active_document_path/);
  assert.match(command, /Workbench deține/);
  assert.match(command, /editor_source:/);
  assert.match(kernel, /pub active_document_path:\s*String/);
  assert.match(kernel, /stored\.active_document_path != active_document_path/);
  assert.match(io, /activeDocumentPath/);
  assert.match(controller, /requireFocusedActiveDocument/);
});

test("EditorNavigation Rust are fațadă stabilă, module focalizate și indexuri sub-cuadratice", () => {
  const directory = "../src-tauri/src/kernel/editor_navigation";
  const facade = source(`${directory}/mod.rs`);
  const contracts = source(`${directory}/contracts.rs`);
  const planner = source(`${directory}/move_planner.rs`);
  const view = source(`${directory}/view.rs`);

  assert.equal(
    existsSync(new URL("../src-tauri/src/kernel/editor_navigation.rs", import.meta.url)),
    false,
  );
  for (const module of ["access", "contracts", "runtime", "move_planner", "provenance", "snapshot", "view"]) {
    assert.match(facade, new RegExp(`mod ${module};`));
    assert.ok(source(`${directory}/${module}.rs`).split("\n").length < 1_100);
  }
  assert.doesNotMatch(facade, /\bfn\s+[a-zA-Z_]/);
  assert.match(contracts, /#\[serde\(skip\)\]\s*pub\(super\) node_index:/);
  assert.match(planner, /snapshot\.node_index\.get\(node_id\)/);
  assert.match(view, /ranged_nodes\.sort_by/);
  assert.match(view, /ancestor_stack/);
  assert.doesNotMatch(
    view,
    /self\s*\.view_nodes\s*\.iter\(\)[\s\S]{0,900}self\s*\.view_nodes\s*\.iter\(\)/,
  );
});

test("relațiile Tera navighează, iar scope-ul editabil rămâne separat de explorare", () => {
  const tree = source("../src/lib/components/project/EditorNavigationTree.svelte");
  const projectPane = source("../src/lib/components/ProjectPane.svelte");

  assert.match(tree, /node\.relation\?\.kind === "include"/);
  const includeNavigation = tree.indexOf(
    "await openDocument(node.relation.targetDocumentPath, true)",
  );
  const localScopeEntry = tree.indexOf("await enterScope(scopeId)", includeNavigation);
  assert.ok(includeNavigation >= 0);
  assert.ok(localScopeEntry > includeNavigation);
  assert.match(tree, /const enteredContext = \$derived\.by/);
  assert.match(tree, /callerTargetDocumentPath/);
  assert.doesNotMatch(tree, /IconLogout2/);
  assert.match(tree, /focusedView\.breadcrumbs/);
  assert.match(projectPane, /editorNavigationCallers/);
  assert.match(projectPane, /callerTargetDocumentPath=/);
  assert.match(projectPane, /returnFromEditorNavigationDocument/);
});
