import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

function editorNavigationSource() {
  return ["contracts", "provenance", "snapshot"]
    .map((module) => source(`../src-tauri/src/kernel/editor_navigation/${module}.rs`))
    .join("\n");
}

test("proveniența sursei este proiectată tipizat de Rust până în Canvas", () => {
  const navigation = editorNavigationSource();
  const interaction = source("../src-tauri/src/kernel/canvas_interaction.rs");
  const types = source("../src/lib/editor/contracts.ts")
    + source("../src/lib/canvas/contracts.ts");

  assert.match(navigation, /EDITOR_NAVIGATION_SCHEMA_VERSION:\s*u32\s*=\s*4/);
  assert.match(navigation, /pub struct EditorSourceProvenance/);
  assert.match(navigation, /pub definition:\s*Option<EditorSourceReference>/);
  assert.match(navigation, /pub composition:\s*Option<EditorSourceReference>/);
  assert.match(navigation, /component_graph\.invocations/);
  assert.match(navigation, /resolved_definition_ids/);
  assert.match(interaction, /CANVAS_INTERACTION_SCHEMA_VERSION:\s*u32\s*=\s*3/);
  assert.match(interaction, /pub source_provenance:\s*EditorSourceProvenance/);
  assert.match(interaction, /source_provenance:\s*node\.source_provenance\.clone\(\)/);
  assert.match(types, /export type EditorSourceProvenance/);
  assert.match(types, /sourceProvenance:\s*EditorSourceProvenance/);
});

test("proveniența selecției alimentează Inspectorul și indicatorul din dreapta", () => {
  const route = source("../src/lib/components/application/ApplicationWorkspace.svelte");
  const chrome = source("../src/lib/components/workspace/AppChrome.svelte");
  const derived = source("../src/lib/editor/selection-workspace.svelte.ts");
  const provenance = source("../src/lib/source-provenance.ts");
  const inspectorArea = source("../src/lib/components/workspace/WorkspaceInspectorArea.svelte");
  const inspector = source("../src/lib/components/InspectorPane.svelte");
  const status = source("../src/lib/components/StatusBar.svelte");
  const card = source("../src/lib/components/inspector/TeraSourceCard.svelte");

  for (const contents of [route, chrome, inspectorArea, inspector]) {
    assert.doesNotMatch(contents, /statusSourceLabel|statusSourceValue|statusSourceOpenable/);
    assert.doesNotMatch(contents, /onSourceContextChange|setStatusSourceContext/);
  }
  assert.doesNotMatch(route + chrome, /openStatusSource/);
  assert.doesNotMatch(route, /startsWith\("SCSS"\)|includes\(":"\)/);
  assert.match(status, /globalStatus\?:\s*GlobalStatusEvent/);
  assert.match(status, /sourceStatus\?:\s*WorkbenchSourceStatus/);
  assert.match(status, /class="selection-source"/);
  assert.match(route, /sourceStatus:\s*selectionWorkspace\.workbenchSourceStatus/);
  assert.match(chrome, /sourceStatus=\{surface\.sourceStatus\}/);
  assert.match(chrome, /openSource=\{openWorkbenchSource\}/);
  assert.match(derived, /workbenchSourceStatusFromSelection\(this\.session\.selectionSnapshot\)/);
  assert.match(provenance, /primarySelectionEntry\(selection\)\?\.provenance/);
  assert.match(provenance, /selectionResolution\(selection\) === "cleared"/);
  assert.match(provenance, /const source = definition \?\? composition/);
  assert.match(provenance, /location:\s*editorSourceReferenceLocation\(source\)/);
  assert.match(provenance, /selectionMemberIds:\s*selection\.members\.map/);
  assert.match(provenance, /primaryMemberId:\s*selection\.primaryMemberId/);
  assert.match(card, /sourceProvenance\.definition/);
  assert.match(card, /sourceProvenance\.composition/);
  assert.equal(
    existsSync(new URL("../src/lib/source-graph/context.ts", import.meta.url)),
    false,
  );
});

test("Code, Status și AI publică primary plus setul opac bounded, fără fapte DOM per membru", () => {
  const readModel = source("../src/lib/kernel/selection-read-model.ts");
  const ai = source("../src/lib/ai/context-state.svelte.ts");
  const rustContext = source("../src-tauri/src/commands/mcp.rs");

  assert.match(readModel, /memberIds:\s*selection\?\.members\.map/);
  assert.match(readModel, /primaryMemberId:\s*selection\?\.primaryMemberId/);
  assert.match(ai, /memberIds:\s*coordinated\?\.members\.map/);
  assert.match(rustContext, /current_opaque_selection\(project_session_id\.as_deref\(\)\)/);
  assert.match(rustContext, /projection\.selection\.member_ids\s*!=\s*rust_member_ids/);
  assert.match(ai, /primaryMemberId:\s*coordinated\?\.primaryMemberId/);
  assert.doesNotMatch(ai, /members\.map\(\(member\)\s*=>\s*\(\{/);
  assert.doesNotMatch(ai, /member\.(?:rect|selector|renderInstanceId)/);
  assert.match(rustContext, /MAX_UI_SELECTION_MEMBERS:\s*usize\s*=\s*256/);
  assert.match(rustContext, /validate_ui_selection_context/);
});

test("Cod deschide definiția, iar ștergerea păstrează call-site-ul selecției Tera", () => {
  const selection = source("../src/lib/state/app-selection-controller.ts");
  const teraActions = source("../src/lib/state/tera-actions-controller.ts");

  assert.match(
    selection,
    /sourceProvenance;\s*const source = provenance\?\.definition \?\? provenance\?\.composition/,
  );
  assert.match(selection, /editorSourceReferenceLocation\(source\)/);
  assert.doesNotMatch(selection, /selectedTemplateSourceNode;\s*if \(!node\)/);
  assert.match(teraActions, /host\.context\(\)\.selectedTemplateSourceNode/);
  assert.doesNotMatch(teraActions, /sourceProvenance\.definition/);
});

test("comutatorul Vizual-Cod păstrează documentul activ", () => {
  const center = source("../src/lib/components/workspace/WorkspaceCenterArea.svelte");
  const navigation = source("../src/lib/workbench/document-navigation.ts");
  const selection = source("../src/lib/state/app-selection-controller.ts");

  assert.match(
    center,
    /const setWorkbenchSurface:[\s\S]*?\(\s*surface,[\s\S]*?\) => workbenchDocuments\.setSurface\(surface\)/,
  );
  assert.match(center, /\{setWorkbenchSurface\}/);
  assert.match(navigation, /setCenterView\(surface === "code" \? "code" : "preview"\)/);
  const surfaceSwitch = navigation.slice(
    navigation.indexOf("async setSurface("),
    navigation.indexOf("\n  }", navigation.indexOf("async setSurface(")) + 4,
  );
  assert.doesNotMatch(surfaceSwitch, /loadProjectFile|OpenDocument|requestSelectionReveal/);
  assert.match(surfaceSwitch, /setCenterView\(surface === "code" \? "code" : "preview"\)/);
  assert.match(
    selection,
    /openSelectedTeraSource[\s\S]*openSourceLocation\(editorSourceReferenceLocation\(source\)\)/,
  );
});
