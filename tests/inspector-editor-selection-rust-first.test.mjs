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
  cssCoordinator: source("../src/lib/components/inspector/CssInspectorCoordinator.svelte"),
  html: source("../src/lib/components/inspector/HtmlPane.svelte"),
  htmlCoordinator: source("../src/lib/components/inspector/HtmlInspectorCoordinator.svelte"),
  css: source("../src/lib/components/inspector/panes/CssPane.svelte"),
  js: source("../src/lib/components/inspector/JsPane.svelte"),
  block: source("../src/lib/components/inspector/BlockPropertiesPane.svelte"),
  motion: source("../src/lib/components/inspector/js/MotionStudioPanel.svelte"),
  workspace: source("../src/lib/components/workspace/WorkspaceInspectorArea.svelte"),
  application: source("../src/lib/components/application/ApplicationWorkspace.svelte"),
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
  assert.match(inspectorFiles.htmlCoordinator, /advanceStableHtmlInspectorProjection/);
  assert.match(
    inspectorFiles.shell,
    /const blockSelectionContext = \$derived\([\s\S]*presentedSelectionSnapshot\?\.aggregateCapabilities\.primaryOnlyEditsAllowed/,
  );
  assert.match(
    inspectorFiles.shell,
    /const dynamicBlockSelectionContext = \$derived\([\s\S]*presentedSelectionSnapshot\?\.aggregateCapabilities\.primaryOnlyEditsAllowed/,
  );
  assert.match(inspectorFiles.shell, /selectionContext=\{blockSelectionContext\}/);
  assert.match(inspectorFiles.shell, /dynamicSelectionContext=\{dynamicBlockSelectionContext\}/);
  assert.match(inspectorFiles.application, /inspectorSelectionSummary:\s*selectionWorkspace\.session\.inspectorSummary/);
  assert.match(inspectorFiles.application, /inspectorHtmlPhysicalFacts:\s*selectionWorkspace\.htmlPhysicalFacts/);
  assert.match(inspectorFiles.application, /inspectorBlockSelectionContext:\s*selectionWorkspace\.blockContext/);
});

test("inspector shell owns routing while domain coordinators own their state machines", () => {
  assert.match(
    inspectorFiles.shell,
    /import\("\$lib\/components\/inspector\/CssInspectorCoordinator\.svelte"\)/,
  );
  assert.match(
    inspectorFiles.shell,
    /import\("\$lib\/components\/inspector\/JsPane\.svelte"\)/,
  );
  assert.match(
    inspectorFiles.shell,
    /import\("\$lib\/components\/inspector\/BlockPropertiesPane\.svelte"\)/,
  );
  assert.match(inspectorFiles.shell, /hidden=\{inspectorTab !== "css"\}/);
  assert.match(inspectorFiles.shell, /hidden=\{inspectorTab !== "js"\}/);
  assert.match(
    inspectorFiles.shell,
    /selectionSnapshot\?\.focus\.kind === "cssRule"[\s\S]*untrack\(\(\) => \{ void changeInspectorTab\("css"\); \}\)/,
  );
  assert.match(inspectorFiles.cssCoordinator, /registerEditFlushHandler\(\s*"inspector-css-workspace"/);
  assert.match(inspectorFiles.htmlCoordinator, /advanceStableHtmlInspectorProjection/);
  for (const coordinator of [inspectorFiles.cssCoordinator, inspectorFiles.htmlCoordinator]) {
    assert.doesNotMatch(coordinator, /\bAppState\b|as unknown as|as AppState/);
  }
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
  assert.match(inspectorFiles.css, /\{#if hasCssSubject && selectedClass\}/);
  assert.match(inspectorFiles.css, /selectionSummary\.subjectKind === "cssRule"/);
  assert.match(inspectorFiles.css, /selectionSummary\?\.classes\.length/);
  assert.match(inspectorFiles.js, /dataAnim\?: string \| null/);
});

test("a CSS rule selected in Code is a first-class Rust source subject", () => {
  const types = source("../src/lib/editor/contracts.ts");
  const rust = source("../src-tauri/src/kernel/selection_coordinator.rs");
  const navigation = source("../src-tauri/src/commands/editor_navigation.rs");
  const pane = inspectorFiles.css;

  assert.match(types, /SelectionSubjectKind[\s\S]*\| "cssRule"/);
  assert.match(rust, /SelectionSubjectKind::CssRule/);
  assert.match(rust, /selection_entry_from_css_focus\(source_graph, &focus\)/);
  assert.match(rust, /SourceNodeKind::Style/);
  assert.match(rust, /css_rule_member_id/);
  assert.match(rust, /SelectCssSourceRule/);
  assert.match(navigation, /ProjectModelFileKind::Style[\s\S]*SelectionIntent::SelectCssSourceRule/);
  assert.match(navigation, /selector_source_target_at_offset/);
  assert.match(
    rust,
    /subject\.kind == SelectionSubjectKind::CssRule[\s\S]*InspectorSelectionSummaryState::Resolved/,
  );
  assert.match(pane, /selectionSummary\.subjectKind === "cssRule"/);
  assert.doesNotMatch(rust, /Focusul CSS\/JS necesită mai întâi un subiect semantic selectat/);
});

test("HTML physical facts remain narrow and cannot become selection authority", () => {
  const types = source("../src/lib/canvas/contracts.ts");
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

test("selection session exposes only exact Rust-accepted inspector projections", () => {
  const workspace = source("../src/lib/editor/selection-workspace.svelte.ts");
  const session = source("../src/lib/state/editor-selection-session.svelte.ts");
  const htmlStart = workspace.indexOf("get coordinatedElement()");
  const blockEnd = workspace.indexOf("get selectionEpoch()", htmlStart);
  const adapters = workspace.slice(htmlStart, blockEnd);

  assert.match(adapters, /this\.session\.acceptedObservation/);
  assert.match(adapters, /accepted\.selectionRevision !== semantic\.selectionRevision/);
  assert.match(adapters, /accepted\.renderInstanceId !== primary\?\.anchor\.renderInstanceId/);
  assert.match(adapters, /summary\.selectionRevision !== coordinated\.snapshot\.selectionRevision/);
  assert.match(adapters, /summary\.renderInstanceId !== coordinated\.renderInstanceId/);
  assert.match(adapters, /bounded\.providerId !== physical\.providerId/);
  assert.match(adapters, /bounded\.rootTag !== physical\.rootTag/);
  assert.doesNotMatch(
    Object.values(inspectorFiles).join("\n"),
    /\bselectionPresentation\b|\bSelectionInfo\b/,
  );

  const acceptanceStart = session.indexOf("async acceptObservation(");
  const acceptanceEnd = session.indexOf(
    "private projectCoordinatorSnapshot(",
    acceptanceStart,
  );
  const acceptance = session.slice(acceptanceStart, acceptanceEnd);
  const summary = acceptance.indexOf(
    "this.inspectorSummary = receipt.inspectorSummary",
  );
  const editorFields = acceptance.indexOf(
    "this.host().applySelectionState(accepted.observation)",
  );
  const returned = acceptance.indexOf("return accepted");
  assert.ok(summary >= 0 && editorFields > summary && returned > editorFields);
});

test("the legacy opaque presentation protocol is absent repo-wide", () => {
  const selectionWorkspace = source("../src/lib/editor/selection-workspace.svelte.ts");
  const io = source("../src/lib/editor/selection-io.ts");
  const rust = source("../src-tauri/src/kernel/selection_coordinator.rs");
  const commands = source("../src-tauri/src/commands/editor_navigation.rs");

  for (const contents of [selectionWorkspace, io, rust, commands]) {
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
