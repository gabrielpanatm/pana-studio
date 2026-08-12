import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

function productionSources(relativeDirectory, extensions) {
  const root = new URL(relativeDirectory, import.meta.url);
  const sources = [];
  const visit = (directory) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const location = new URL(entry.name, directory.href.endsWith("/") ? directory : new URL(`${directory.href}/`));
      if (entry.isDirectory()) {
        visit(new URL(`${location.href}/`));
      } else if (extensions.some((extension) => entry.name.endsWith(extension))) {
        sources.push([location.pathname, readFileSync(location, "utf8")]);
      }
    }
  };
  visit(root);
  return sources;
}

const inspectorFiles = {
  shell: source("../src/lib/components/InspectorPane.svelte"),
  html: source("../src/lib/components/inspector/HtmlPane.svelte"),
  css: source("../src/lib/components/inspector/panes/CssPane.svelte"),
  js: source("../src/lib/components/inspector/JsPane.svelte"),
  block: source("../src/lib/components/inspector/BlockPropertiesPane.svelte"),
  motion: source("../src/lib/components/inspector/js/MotionStudioPanel.svelte"),
  workspace: source("../src/lib/components/workspace/WorkspaceInspectorArea.svelte"),
};

test("inspector editors do not consume the legacy selection presentation", () => {
  for (const [name, contents] of Object.entries(inspectorFiles)) {
    assert.doesNotMatch(
      contents,
      /\bselectionPresentation\b|\bSelectionPresentation(?:Input|Receipt)?\b|\bSelectionInfo\b|\bpreviewSelection\b/,
      `${name} reintroduced the legacy selection route`,
    );
  }

  assert.match(inspectorFiles.shell, /selectionSummary=\{presentedInspectorSelectionSummary\}/);
  assert.match(inspectorFiles.shell, /physicalFacts=\{presentedHtmlPhysicalFacts\}/);
  assert.match(inspectorFiles.shell, /advanceStableHtmlInspectorProjection/);
  assert.match(
    inspectorFiles.shell,
    /selectionContext=\{presentedSelectionSnapshot\?\.aggregateCapabilities\.primaryOnlyEditsAllowed/,
  );
  assert.match(
    inspectorFiles.shell,
    /dynamicSelectionContext=\{presentedSelectionSnapshot\?\.aggregateCapabilities\.primaryOnlyEditsAllowed/,
  );
  assert.match(inspectorFiles.workspace, /inspectorSelectionSummary=\{app\.inspectorSelectionSummary\}/);
  assert.match(inspectorFiles.workspace, /inspectorHtmlPhysicalFacts=\{app\.inspectorHtmlPhysicalFacts\}/);
  assert.match(inspectorFiles.workspace, /inspectorBlockSelectionContext=\{app\.inspectorBlockSelectionContext\}/);
});

test("editor selection availability comes from the Rust summary", () => {
  for (const contents of [
    inspectorFiles.html,
    inspectorFiles.css,
    inspectorFiles.js,
  ]) {
    assert.match(contents, /InspectorSelectionSummarySnapshot/);
    assert.match(contents, /selectionSummary\?\.state === "resolved"/);
    assert.match(contents, /selectionSummary\.subjectKind === "htmlElement"/);
    assert.match(contents, /selectionSummary\.subjectKind === "runtimeElement"/);
  }

  assert.match(inspectorFiles.html, /selectionSummary\?\.elementId/);
  assert.match(inspectorFiles.html, /selectionSummary\?\.classes/);
  assert.match(inspectorFiles.css, /\{#if hasElementSelection && selectedClass\}/);
  assert.match(inspectorFiles.css, /selectionSummary\?\.classes\.length/);
  assert.match(inspectorFiles.js, /dataAnim\?: string \| null/);
});

test("HTML physical facts remain narrow and cannot become selection authority", () => {
  const types = source("../src/lib/types.ts");
  const start = types.indexOf("export type InspectorHtmlPhysicalFacts");
  const end = types.indexOf("export type ZolaImageOperation", start);
  const contract = types.slice(start, end);

  assert.match(contract, /selectionRevision: number/);
  assert.match(contract, /renderInstanceId: string/);
  assert.match(contract, /rect:/);
  assert.match(contract, /hasChildElements: boolean/);
  assert.match(contract, /childElementCount: number/);
  assert.match(contract, /zolaImage: ZolaImagePresentation \| null/);
  assert.doesNotMatch(contract, /\btag:|\bid:|\bclasses:|\battributes:|\bselector:|\btext:/);

  assert.match(inspectorFiles.html, /physicalFacts\?: InspectorHtmlPhysicalFacts \| null/);
  assert.match(inspectorFiles.html, /selectionSnapshot\?: SelectionSnapshot \| null/);
  assert.match(inspectorFiles.html, /primarySelectionEntry\(selectionSnapshot\)\?\.provenance\.definition/);
  assert.doesNotMatch(
    inspectorFiles.html,
    /physicalFacts\?\.(?:tag|id|classes|attributes|selector|text)/,
  );
});

test("AppState exposes only exact Rust-accepted inspector projections", () => {
  const app = source("../src/lib/state/app.svelte.ts");
  const htmlStart = app.indexOf("coordinatedElementSelection = $derived.by");
  const blockEnd = app.indexOf("selectionEpoch = $derived", htmlStart);
  const adapters = app.slice(htmlStart, blockEnd);

  assert.match(adapters, /this\.acceptedSelectionObservation/);
  assert.match(adapters, /accepted\.selectionRevision !== semantic\.selectionRevision/);
  assert.match(adapters, /accepted\.renderInstanceId !== primary\?\.anchor\.renderInstanceId/);
  assert.match(adapters, /summary\.selectionRevision !== coordinated\.snapshot\.selectionRevision/);
  assert.match(adapters, /summary\.renderInstanceId !== coordinated\.renderInstanceId/);
  assert.match(adapters, /bounded\.providerId !== physical\.providerId/);
  assert.match(adapters, /bounded\.markerKind !== physical\.markerKind/);
  assert.match(adapters, /bounded\.rootTag !== physical\.rootTag/);
  assert.doesNotMatch(
    Object.values(inspectorFiles).join("\n"),
    /\bselectionPresentation\b|\bSelectionInfo\b/,
  );

  const acceptanceStart = app.indexOf("async acceptSelectionObservation(");
  const acceptanceEnd = app.indexOf(
    "private projectSelectionCoordinatorSnapshot(",
    acceptanceStart,
  );
  const acceptance = app.slice(acceptanceStart, acceptanceEnd);
  const summary = acceptance.indexOf(
    "this.inspectorSelectionSummary = receipt.inspectorSummary",
  );
  const editorFields = acceptance.indexOf(
    "this.applySelectionState(accepted.observation)",
  );
  const returned = acceptance.indexOf("return accepted");
  assert.ok(summary >= 0 && editorFields > summary && returned > editorFields);
});

test("the legacy opaque presentation protocol is absent repo-wide", () => {
  const app = source("../src/lib/state/app.svelte.ts");
  const io = source("../src/lib/project/io.ts");
  const rust = source("../src-tauri/src/kernel/selection_coordinator.rs");
  const commands = source("../src-tauri/src/commands/editor_navigation.rs");

  for (const contents of [app, io, rust, commands]) {
    assert.doesNotMatch(
      contents,
      /\bSelectionPresentation(?:Input|Receipt)?\b|\bselectionPresentation\b|physicalSelectionPresentation/,
    );
  }
  assert.match(rust, /pub struct SelectionObservationReceipt/);
  assert.doesNotMatch(rust, /serde_json::Value|\bpayload:\s*Value\b/);
  assert.match(io, /"accept_selection_observation"/);
  assert.doesNotMatch(io, /read_selection_(?:presentation|observation)/);
});

test("all production selection consumers stay on the coordinated Rust-first route", () => {
  const sources = [
    ...productionSources("../src/", [".ts", ".svelte"]),
    ...productionSources("../src-tauri/src/", [".rs", ".js"]),
    ...productionSources("../src-tauri/permissions/", [".toml"]),
  ];
  const forbidden =
    /\bSelectionPresentation(?:Input|Receipt)?\b|\bselectionPresentation\b|\bphysicalSelectionPresentation\b|\bSelectionInfo\b|\bPreviewSelectionState\b|\baccept_selection_presentation\b|\bread_selection_presentation\b|\bacceptSelectionPresentation\b|\breadSelectionPresentation\b|\bcreateSelectionInfo\b|\bhtmlTargetFromSelection\b/;

  for (const [path, contents] of sources) {
    assert.doesNotMatch(contents, forbidden, `${path} reintroduced a legacy selection consumer`);
  }
});
