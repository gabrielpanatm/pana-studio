import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  committedAction,
  failedAction,
} from "$lib/editor-runtime/action-outcome";
import { createEditorRuntime } from "$lib/editor-runtime/runtime";
import { drainPreviewStructuralLanes } from "$lib/kernel/preview-structural-lane";
import { registerEditFlushHandler } from "$lib/session/edit-flush-registry";
import { committedDraftCanSettle } from "$lib/session/committed-draft-settlement";
import { resetPageJsDraftSyncState } from "$lib/session/page-js-draft-sync";
import { saveActiveFile } from "$lib/state/save-controller";
import {
  applyAttributesToHtml,
  attributeMutationsFromRecord,
  generateDataAnimForSelectedHtml,
} from "$lib/state/html-actions-controller";
import { deleteSelectedTeraNode } from "$lib/state/tera-actions-controller";

if (!globalThis.window) globalThis.window = globalThis;

const unregister = [];

afterEach(async () => {
  clearMocks();
  while (unregister.length > 0) unregister.pop()?.();
  resetPageJsDraftSyncState();
  await drainPreviewStructuralLanes();
});

function emptyHtmlPending() {
  return {
    tag: false,
    attributes: false,
    text: false,
    image: false,
    classes: false,
    structure: false,
  };
}

test("a generated attribute settles the untouched baseline but preserves a concurrent edit", () => {
  const baseline = JSON.stringify({ title: "Titlu" });
  const generated = JSON.stringify({ "data-anim": "ps-h1-generated", title: "Titlu" });
  const concurrent = JSON.stringify({ "aria-label": "Titlu nou", title: "Titlu" });

  assert.equal(committedDraftCanSettle(baseline, generated, baseline), true);
  assert.equal(committedDraftCanSettle(generated, generated, baseline), true);
  assert.equal(committedDraftCanSettle(concurrent, generated, baseline), false);
});

test("contractul frontend distinge SetAttribute gol de RemoveAttribute", () => {
  assert.deepEqual(attributeMutationsFromRecord({
    alt: "",
    disabled: "",
    title: null,
  }), [
    { kind: "setAttribute", name: "alt", value: "" },
    { kind: "setAttribute", name: "disabled", value: "" },
    { kind: "removeAttribute", name: "title" },
  ]);
});

test("controllerul păstrează aria pending când kernelul blochează commit-ul HTML", async () => {
  mockIPC(async (command) => {
    assert.equal(command, "execute_preview_html_attributes_intent");
    return {
      status: "blocked",
      message: "HTML Attribute Engine a refuzat commit-ul.",
      diagnostics: [{ blocking: true, message: "baseline HTML s-a schimbat" }],
    };
  });

  const htmlPending = emptyHtmlPending();
  const host = {
    sessionProjectRoot: "/project",
    kernelProjectSessionId: "session:runtime",
    projectSessionEpoch: 3,
    projectTransitionFrontendLeaseActive: false,
    async beginPreviewStructuralWriteBoundary() {},
    endPreviewStructuralWriteBoundary() {},
    selectedElement: {
      domPath: "main > h1:nth-of-type(1)",
      cssSelector: "main > h1:nth-of-type(1)",
      tag: "h1",
      sourceId: "source-h1",
      sourceLocation: { file: "templates/index.html", line: 3, column: 3 },
      sessionId: "preview-h1",
      attributes: { title: "vechi" },
      classes: [],
      hasChildElements: false,
      rawText: "Titlu",
      parentNode: null,
    },
    attributeValues: { title: "nou" },
    attributeStatus: "",
    htmlPending,
    pageSections: [],
    isActivePreviewHtmlSource: false,
    currentHtmlRelativePath: "",
    resolveSourceEditTargetForSourceId() {
      return null;
    },
    setHtmlPending(area, pending) {
      this.htmlPending[area] = pending;
    },
    setGlobalStatus() {},
  };

  const result = await applyAttributesToHtml(host);
  assert.equal(result.status, "blocked");
  assert.match(result.reason, /baseline HTML/);
  assert.equal(host.htmlPending.attributes, true);
  assert.deepEqual(host.attributeValues, { title: "nou" });
});

test("generarea data-anim blocată nu inventează un draft pending și expune cauza", async () => {
  mockIPC(async (command) => {
    assert.equal(command, "execute_preview_html_attributes_intent");
    return {
      status: "blocked",
      message: "HTML Attribute Engine a refuzat commit-ul.",
      diagnostics: [{ blocking: true, message: "Source ID expirat" }],
    };
  });

  const statuses = [];
  const htmlPending = emptyHtmlPending();
  const host = {
    sessionProjectRoot: "/project",
    kernelProjectSessionId: "session:runtime",
    projectSessionEpoch: 4,
    projectTransitionFrontendLeaseActive: false,
    async beginPreviewStructuralWriteBoundary() {},
    endPreviewStructuralWriteBoundary() {},
    selectedElement: {
      domPath: "main > h1:nth-of-type(1)",
      cssSelector: "main > h1:nth-of-type(1)",
      tag: "h1",
      sourceId: "source-h1-stale",
      sourceLocation: { file: "templates/index.html", line: 3, column: 3 },
      sessionId: "preview-h1",
      attributes: { title: "Titlu" },
      classes: ["hero-title"],
      hasChildElements: false,
      rawText: "Titlu",
      parentNode: null,
    },
    attributeValues: { title: "Titlu" },
    attributeStatus: "",
    classEditorValue: "hero-title",
    htmlPending,
    pageSections: [],
    sourceCache: {
      "scanned:templates/index.html": "<main><h1 class=\"hero-title\">Titlu</h1></main>",
    },
    cssRuleEdits: {},
    scssVariableEdits: {},
    pageJsEdits: {},
    scannedProject: null,
    isActivePreviewHtmlSource: false,
    currentHtmlRelativePath: "",
    resolveSourceEditTargetForSourceId() {
      return null;
    },
    setHtmlPending(area, pending) {
      this.htmlPending[area] = pending;
    },
    setGlobalStatus(text, kind) {
      statuses.push({ text, kind });
    },
  };

  const result = await generateDataAnimForSelectedHtml(host);
  assert.equal(result.status, "blocked");
  assert.equal(htmlPending.attributes, false);
  assert.deepEqual(host.attributeValues, { title: "Titlu" });
  assert.match(host.attributeStatus, /Source ID expirat/);
  assert.deepEqual(statuses.at(-1), {
    text: host.attributeStatus,
    kind: "error",
  });
});

test("Save rămâne eșuat și păstrează HTML pending după flush-uri CSS/JS reușite", async () => {
  const flushed = [];
  unregister.push(registerEditFlushHandler("test-css-success", async (reason) => {
    flushed.push(`css:${reason}`);
  }));
  unregister.push(registerEditFlushHandler("test-js-success", async (reason) => {
    flushed.push(`js:${reason}`);
  }));

  const statuses = [];
  const htmlPending = { ...emptyHtmlPending(), attributes: true };
  const host = {
    sessionProjectRoot: "/project",
    kernelProjectSessionId: "session:runtime",
    projectSessionEpoch: 1,
    projectTransitionFrontendLeaseActive: false,
    async beginPreviewStructuralWriteBoundary() {},
    endPreviewStructuralWriteBoundary() {},
    saveRequest: 0,
    inspectorPending: { html: true, css: false, js: false },
    htmlPending,
    pendingTag: null,
    globalDirtyState: { dirty: true, canSave: true },
    kernelSourceDirtyPaths: ["sursa/templates/index.html"],
    cssRuleEdits: { ".hero": { dirty: true } },
    scssVariableEdits: {},
    pageJsEdits: { "templates/index.html": { dirty: true } },
    centerView: "preview",
    currentSourceRelativePath: "",
    async applyTagChange() {
      return committedAction();
    },
    async applyClassesToHtml() {
      return committedAction();
    },
    async applyAttributesToHtml() {
      return failedAction("commit HTML refuzat de kernel");
    },
    async applyImageSourceToHtml() {
      return committedAction();
    },
    async applyTextContentToHtml() {
      return committedAction();
    },
    setGlobalStatus(text, kind) {
      statuses.push({ text, kind });
    },
  };

  assert.equal(await saveActiveFile(host), false);
  assert.deepEqual(flushed, ["css:save", "js:save"]);
  assert.equal(host.htmlPending.attributes, true);
  assert.deepEqual(host.kernelSourceDirtyPaths, ["sursa/templates/index.html"]);
  assert.equal(host.saveRequest, 1);
  assert.deepEqual(statuses.at(-1), {
    text: "Salvarea sesiunii proiectului a eșuat: commit HTML refuzat de kernel",
    kind: "error",
  });
});

test("Save păstrează terminală eroarea saveSessionDrafts și nu o rescrie cu succes fals", async () => {
  let flushCount = 0;
  unregister.push(registerEditFlushHandler("test-second-flush-fails", async () => {
    flushCount += 1;
    if (flushCount === 2) throw new Error("al doilea flush a eșuat");
  }));

  const statuses = [];
  const host = {
    sessionProjectRoot: "/project",
    kernelProjectSessionId: "session:runtime",
    projectSessionEpoch: 1,
    projectTransitionFrontendLeaseActive: false,
    async beginPreviewStructuralWriteBoundary() {},
    endPreviewStructuralWriteBoundary() {},
    saveRequest: 0,
    saveState: "unsaved",
    saveStatus: "",
    inspectorPending: { html: false },
    htmlPending: emptyHtmlPending(),
    pendingTag: null,
    globalDirtyState: { dirty: true, canSave: true },
    kernelSourceDirtyPaths: ["sursa/templates/index.html"],
    cssRuleEdits: {},
    scssVariableEdits: {},
    pageJsEdits: {},
    centerView: "preview",
    currentSourceRelativePath: "",
    setGlobalStatus(text, kind) {
      this.saveStatus = text;
      this.saveState = kind;
      statuses.push({ text, kind });
    },
  };

  assert.equal(await saveActiveFile(host), false);
  assert.equal(flushCount, 2);
  assert.equal(host.saveState, "error");
  assert.match(host.saveStatus, /al doilea flush a eșuat/);
  assert.equal(statuses.some(({ text, kind }) => kind === "saved" || /Nicio modificare/.test(text)), false);
});

test("EditorRuntime nu raportează ok când controllerul contextual blochează mutația", async () => {
  const statuses = [];
  const htmlHost = {
    sessionProjectRoot: "/project",
    kernelProjectSessionId: "session:runtime",
    projectSessionEpoch: 7,
    projectTransitionFrontendLeaseActive: false,
    async beginPreviewStructuralWriteBoundary() {},
    endPreviewStructuralWriteBoundary() {},
    selectedElement: null,
    pageSections: [],
    structureStatus: "",
    isActivePreviewHtmlSource: false,
    currentHtmlRelativePath: "",
    htmlSourceMutationBlockedReason: "Controllerul HTML a blocat ținta fără sursă canonică.",
    getPreviewDocument() {
      return undefined;
    },
    resolveSourceEditTargetForSourceId() {
      return null;
    },
    setGlobalStatus(text, kind) {
      statuses.push({ text, kind });
    },
  };
  const runtime = createEditorRuntime({
    centerView: "preview",
    async setCenterView() {
      return true;
    },
    templateHtmlEditSourceId: null,
    htmlActionsControllerHost() {
      return htmlHost;
    },
    selectionControllerHost() {
      return {};
    },
    selectDomNode() {},
    selectTeraLayerSource() {},
    setPreviewTeraSelection() {},
    allowTemplateHtmlEdit() {},
    async openSelectedTeraSource() {},
    async deleteSelectedTeraNode() {},
    setGlobalStatus(text, kind) {
      statuses.push({ text, kind });
    },
  });

  const result = await runtime.dispatch({
    type: "delete-html",
    surface: "preview",
    target: {
      kind: "html",
      selector: "main > section:nth-of-type(1)",
      tag: "section",
    },
  });

  assert.equal(result.ok, false);
  assert.equal(result.status, "blocked");
  assert.match(result.reason, /fără sursă canonică/);
  assert.equal(runtime.lastTransaction?.ok, false);
  assert.equal(runtime.lastTransaction?.status, "blocked");
  assert.match(statuses.at(-1)?.text ?? "", /fără sursă canonică/);
});

function teraRuntimeHost(teraHost) {
  return {
    centerView: "preview",
    async setCenterView() {
      return true;
    },
    templateHtmlEditSourceId: null,
    htmlActionsControllerHost() {
      return {};
    },
    selectionControllerHost() {
      return {};
    },
    selectDomNode() {},
    selectTeraLayerSource() {},
    setPreviewTeraSelection() {},
    allowTemplateHtmlEdit() {},
    async openSelectedTeraSource() {},
    async deleteSelectedTeraNode(target) {
      return await deleteSelectedTeraNode(
        teraHost,
        target === undefined ? undefined : target?.sourceNode ?? null,
      );
    },
    setGlobalStatus(text, kind) {
      teraHost.setGlobalStatus(text, kind);
    },
  };
}

function minimalTeraControllerHost(statuses) {
  return {
    sessionProjectRoot: "/project",
    kernelProjectSessionId: "session:runtime",
    projectSessionEpoch: 9,
    projectTransitionFrontendLeaseActive: false,
    async beginPreviewStructuralWriteBoundary() {},
    endPreviewStructuralWriteBoundary() {},
    selectedTemplateSourceNode: null,
    setGlobalStatus(text, kind) {
      statuses.push({ text, kind });
    },
  };
}

test("EditorRuntime propagă blocked din controllerul Tera delete", async () => {
  const statuses = [];
  const runtime = createEditorRuntime(teraRuntimeHost(minimalTeraControllerHost(statuses)));
  const result = await runtime.dispatch({
    type: "delete-tera",
    surface: "layers",
    target: {
      kind: "tera",
      sourceId: "tera:missing",
      selector: null,
      sourceNode: null,
    },
  });

  assert.equal(result.ok, false);
  assert.equal(result.status, "blocked");
  assert.match(result.reason, /Selectează un nod Tera/);
  assert.equal(runtime.lastTransaction?.status, "blocked");
});

test("EditorRuntime propagă failed din controllerul Tera delete", async () => {
  mockIPC(async (command) => {
    assert.equal(command, "execute_preview_tera_delete_intent");
    throw new Error("Tera kernel indisponibil");
  });
  const statuses = [];
  const runtime = createEditorRuntime(teraRuntimeHost(minimalTeraControllerHost(statuses)));
  const sourceNode = {
    id: "tera:include:1",
    kind: "include",
    label: "Include hero",
    file: "templates/index.html",
    range: { line: 4, column: 3, endLine: 4, endColumn: 28 },
    children: [],
    capabilities: {},
  };
  const result = await runtime.dispatch({
    type: "delete-tera",
    surface: "layers",
    target: {
      kind: "tera",
      sourceId: sourceNode.id,
      selector: "main",
      sourceNode,
    },
  });

  assert.equal(result.ok, false);
  assert.equal(result.status, "failed");
  assert.match(result.reason, /Tera kernel indisponibil/);
  assert.equal(runtime.lastTransaction?.status, "failed");
});
