import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("proveniența sursei este proiectată tipizat de Rust până în Canvas", () => {
  const navigation = source("../src-tauri/src/kernel/editor_navigation.rs");
  const interaction = source("../src-tauri/src/kernel/canvas_interaction.rs");
  const types = source("../src/lib/types.ts");

  assert.match(navigation, /EDITOR_NAVIGATION_SCHEMA_VERSION:\s*u32\s*=\s*3/);
  assert.match(navigation, /pub struct EditorSourceProvenance/);
  assert.match(navigation, /pub definition:\s*Option<EditorSourceReference>/);
  assert.match(navigation, /pub composition:\s*Option<EditorSourceReference>/);
  assert.match(navigation, /component_graph\.invocations/);
  assert.match(navigation, /resolved_definition_ids/);
  assert.match(interaction, /CANVAS_INTERACTION_SCHEMA_VERSION:\s*u32\s*=\s*2/);
  assert.match(interaction, /pub source_provenance:\s*EditorSourceProvenance/);
  assert.match(interaction, /source_provenance:\s*node\.source_provenance\.clone\(\)/);
  assert.match(types, /export type EditorSourceProvenance/);
  assert.match(types, /sourceProvenance:\s*EditorSourceProvenance/);
});

test("proveniența selecției alimentează Inspectorul și indicatorul din dreapta", () => {
  const route = source("../src/routes/+page.svelte");
  const chrome = source("../src/lib/components/workspace/AppChrome.svelte");
  const derived = source("../src/lib/state/app-derived.ts");
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
  assert.match(chrome, /sourceStatus=\{app\.workbenchSourceStatus\}/);
  assert.match(chrome, /openSource=\{openWorkbenchSource\}/);
  assert.match(derived, /workbenchSourceStatusFromSelection\(app\.selectionSnapshot\)/);
  assert.match(provenance, /selection\.projections\.status/);
  assert.match(provenance, /const source = definition \?\? composition/);
  assert.match(provenance, /location:\s*editorSourceReferenceLocation\(source\)/);
  assert.match(card, /sourceProvenance\.definition/);
  assert.match(card, /sourceProvenance\.composition/);
  assert.equal(
    existsSync(new URL("../src/lib/source-graph/context.ts", import.meta.url)),
    false,
  );
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
  assert.match(teraActions, /host\.selectedTemplateSourceNode/);
  assert.doesNotMatch(teraActions, /sourceProvenance\.definition/);
});

test("comutatorul Vizual-Cod păstrează documentul activ", () => {
  const app = source("../src/lib/state/app.svelte.ts");
  const center = source("../src/lib/components/workspace/WorkspaceCenterArea.svelte");
  const selection = source("../src/lib/state/app-selection-controller.ts");

  assert.match(
    center,
    /async function setWorkbenchSurface\(surface: WorkbenchSurface\)\s*\{\s*await app\.setCenterView\(centerViewForSurface\(surface\)\);\s*\}/,
  );
  assert.doesNotMatch(app, /prepareHtmlCodeRevealTargetForCodeEntry/);
  assert.match(
    app,
    /if \(enteringCode\) \{[\s\S]*this\.requestCodeSelectionReveal\(\);[\s\S]*\}/,
  );
  assert.match(
    app,
    /setActiveDocumentSurface\(this\.activeScannedPath, view\)/,
  );
  assert.match(
    selection,
    /openSelectedTeraSource[\s\S]*openSourceLocation\(editorSourceReferenceLocation\(source\)\)/,
  );
});
