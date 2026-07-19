import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

function source(path) {
  return readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
}

test("CSS panel has one Rust/ProjectWorkspace write authority", () => {
  const inspector = source("src/lib/components/InspectorPane.svelte");
  const app = source("src/lib/state/app.svelte.ts");
  const sourceSync = source("src/lib/css/source-sync.ts");

  assert.match(inspector, /setCssRuleAtViewport/);
  assert.match(inspector, /setPageCssRuleAtViewport/);
  assert.match(inspector, /setScssVariable/);
  assert.match(inspector, /stageCssRuleMutation/);
  assert.match(inspector, /flushStagedCssPanelMutations/);
  assert.match(inspector, /registerEditFlushHandler\("inspector-css-workspace"/);
  assert.doesNotMatch(inspector, /applyCssPanelEditToOpenSource/);
  assert.doesNotMatch(app, /applyCssPanelEditToOpenSource|projectOpenSourceInspectorCssMutation/);
  assert.doesNotMatch(sourceSync, /upsertCssPropertyInSource|upsertDeclarationInBlock/);
});

test("CSS receipt projects exact FileBuffer snapshots into CodeMirror state", () => {
  const types = source("src/lib/types.ts");
  const io = source("src/lib/project/io.ts");
  const app = source("src/lib/state/app.svelte.ts");

  assert.match(types, /CssMutationAuthorityReceipt[\s\S]*documents: WorkspaceDocumentProjection\[\]/);
  assert.match(io, /authority\.schemaVersion !== 2/);
  assert.match(io, /written\.contents !== snapshot\.text/);
  assert.match(app, /rebaseFileBufferDraftSyncProjection\(projection\.relativePath, projection\.snapshot\)/);
  assert.match(app, /this\.source = projection\.snapshot\.text/);
});

test("toggle-ul activ emite delete intent, iar un no-op recitește adevărul canonic", () => {
  const segmented = source("src/lib/components/inspector/controls/SegmentedControl.svelte");
  const app = source("src/lib/state/app.svelte.ts");

  assert.match(segmented, /toggleable && value === nextValue \? "" : nextValue/);
  assert.match(
    app,
    /authority\.status === "noop"[\s\S]*this\.notifyCssSourceChanged\(\)/,
  );
});

test("proiecția workspace cere o origine Preview navigabilă, nu doar un iframe", () => {
  const app = source("src/lib/state/app.svelte.ts");
  const canProject = app.slice(
    app.indexOf("canProjectWorkspacePreview()"),
    app.indexOf("markPreviewLive", app.indexOf("canProjectWorkspacePreview()")),
  );

  assert.match(canProject, /previewFrame\?\.contentWindow/);
  assert.match(canProject, /scannedProject\?\.previewBaseUrl/);
  assert.match(canProject, /previewSrc !== "about:blank"/);
});

test("all ten CSS sections share the explicit property edit contract", () => {
  const editor = source("src/lib/components/inspector/ClassEditor.svelte");
  const sections = [
    "Typography", "Colors", "Spacing", "Layout", "Position",
    "Size", "Border", "Shadow", "Transform", "Effects",
  ];
  for (const section of sections) {
    assert.match(editor, new RegExp(`<${section}Section\\s`), section);
  }
  assert.equal((editor.match(/edit=\{cssPropertyEdit\}/g) ?? []).length, sections.length);
  assert.doesNotMatch(editor, /setValue=\{updateProperty\}/);
});

test("CSS edits use explicit draft, commit and cancel boundaries", () => {
  const inspector = source("src/lib/components/InspectorPane.svelte");
  const contract = source("src/lib/inspector/css-property-edit.ts");
  const propInput = source("src/lib/components/inspector/controls/PropInput.svelte");
  const typography = source("src/lib/components/inspector/sections/TypographySection.svelte");

  assert.match(contract, /draft: \(property: string, value: string\)/);
  assert.match(contract, /commit: \(property: string, value\?: string\)/);
  assert.match(contract, /cancel: \(property: string\)/);
  assert.match(inspector, /function draftCssProperty/);
  assert.match(inspector, /function commitCssProperty/);
  assert.match(inspector, /function cancelCssProperty/);
  assert.match(inspector, /restoreCssPendingValueBaseline/);
  assert.doesNotMatch(inspector, /addEventListener\("click", scheduleStagedCssPanelFlush\)/);
  assert.doesNotMatch(inspector, /addEventListener\("change", scheduleStagedCssPanelFlush\)/);
  assert.doesNotMatch(inspector, /<svelte:window onpointerup=\{scheduleStagedCssPanelFlush\}/);

  assert.match(propInput, /oncommit\?\.\(draftValue\)/);
  assert.match(propInput, /oncancel\?\.\(\)/);
  assert.match(typography, /edit\.continuous\("font-size"\)/);
  assert.match(typography, /edit\.commit\("text-align", v\)/);
});

test("structured compound editors preserve unsupported values in raw mode", () => {
  const colors = source("src/lib/components/inspector/sections/ColorsSection.svelte");
  const shadows = source("src/lib/components/inspector/sections/ShadowSection.svelte");

  assert.match(colors, /isBackgroundGradientStructurallyEditable/);
  assert.match(colors, /Valoarea brută este păstrată/);
  assert.match(shadows, /boxStructured/);
  assert.match(shadows, /textStructured/);
  assert.match(shadows, /Valoare complexă păstrată integral/);
});
