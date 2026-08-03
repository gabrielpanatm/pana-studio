import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("Straturi păstrează Canvas global și proiectează separat documentul activ", () => {
  const rust = source("../src-tauri/src/kernel/editor_navigation.rs");
  const tree = source("../src/lib/components/project/EditorNavigationTree.svelte");
  const pane = source("../src/lib/components/ProjectPane.svelte");

  assert.match(rust, /EDITOR_NAVIGATION_SCHEMA_VERSION:\s*u32\s*=\s*3/);
  assert.match(rust, /pub struct EditorNavigationView/);
  assert.match(rust, /pub focused_view:\s*Option<EditorNavigationView>/);
  assert.match(rust, /#\[serde\(skip\)\]\s*planning_nodes/);
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

test("click-dreapta în Straturi deschide meniul aplicației pentru nodul Rust curent", () => {
  const tree = source("../src/lib/components/project/EditorNavigationTree.svelte");
  const pane = source("../src/lib/components/ProjectPane.svelte");
  const projectArea = source(
    "../src/lib/components/workspace/WorkspaceProjectArea.svelte",
  );
  const app = source("../src/lib/state/app.svelte.ts");

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
    /app\.openEditorNavigationContextMenu\(node, x, y\)/,
  );
  assert.match(app, /openEditorNavigationContextMenu\(/);
  assert.match(app, /candidate\.id === requestedNode\.id/);
  assert.match(app, /htmlElementContextMenuItems\(/);
  assert.match(app, /teraContextMenuItems\(/);
  assert.match(app, /source:\s*"layers"/);
});

test("ștergerea din Straturi păstrează identitatea și locația aceleiași proiecții Rust", () => {
  const app = source("../src/lib/state/app.svelte.ts");
  const actions = source("../src/lib/state/html-actions-controller.ts");
  const deleteMethod = app.slice(
    app.indexOf("async deleteEditorNavigationNode"),
    app.indexOf("async deleteHtmlElement"),
  );

  assert.match(deleteMethod, /renderInstanceId:\s*node\.renderInstanceId/);
  assert.match(deleteMethod, /sourceLocation:\s*node\.file && node\.range/);
  assert.match(deleteMethod, /line:\s*node\.range\.line/);
  assert.match(deleteMethod, /column:\s*node\.range\.column/);
  assert.match(deleteMethod, /sessionId:\s*this\.activeCanvasIdentity\?\.runtimeSessionId/);
  const resolver = actions.slice(
    actions.indexOf("function sourceLocationForSourceReference"),
    actions.indexOf("function sourceLocationForSessionReference"),
  );
  assert.ok(
    resolver.indexOf("if (fallbackSourceLocation)")
      < resolver.indexOf("host.resolveSourceEditTargetForSourceId"),
  );
});

test("documentul activ și mutațiile sunt validate de Workbench-ul Rust", () => {
  const command = source("../src-tauri/src/commands/editor_navigation.rs");
  const kernel = source("../src-tauri/src/kernel/editor_navigation.rs");
  const io = source("../src/lib/project/io.ts");
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
