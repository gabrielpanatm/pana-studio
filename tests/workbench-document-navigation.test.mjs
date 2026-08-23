import assert from "node:assert/strict";
import { test } from "node:test";
import { WorkbenchDocumentNavigationService } from "$lib/workbench/document-navigation";

function document() {
  return {
    documentId: "document:index",
    relativePath: "templates/index.html",
    title: "index.html",
    surface: "visual",
    pinned: true,
  };
}

function otherDocument(relativePath, surface = "code") {
  return {
    documentId: `document:${relativePath}`,
    relativePath,
    title: relativePath.split("/").at(-1),
    surface,
    pinned: true,
  };
}

function deferred() {
  let resolve;
  const promise = new Promise((accept) => { resolve = accept; });
  return { promise, resolve };
}

async function nextTurn() {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

function snapshot(activeGroupId = "primary", activeDocumentId = "document:index") {
  return {
    activeGroupId,
    groups: [
      {
        groupId: "primary",
        activeDocumentId,
        documents: [document()],
      },
      {
        groupId: "secondary",
        activeDocumentId: null,
        documents: [],
      },
    ],
  };
}

test("reselectarea documentului activ nu reproiectează și nu reîncarcă Preview", async () => {
  const calls = [];
  const service = new WorkbenchDocumentNavigationService({
    currentSnapshot: () => snapshot(),
    async resolveProjectFile() {
      calls.push("resolve");
      return { relativePath: "templates/index.html", role: "template" };
    },
    async loadProjectFile() { calls.push("load"); },
    async applyIntent() { calls.push("intent"); },
    async setCenterView() { calls.push("surface"); },
  }, {
    set(text, kind) { calls.push(`status:${kind}:${text}`); },
  });

  await service.activate("primary", document());

  assert.deepEqual(calls, []);
});

test("activarea din alt grup păstrează fluxul canonic de document", async () => {
  const calls = [];
  const loadOptions = [];
  const activeDocument = document();
  const service = new WorkbenchDocumentNavigationService({
    currentSnapshot: () => snapshot("secondary"),
    async resolveProjectFile(relativePath) {
      calls.push(`resolve:${relativePath}`);
      return { relativePath, role: "template" };
    },
    async loadProjectFile(file, options) {
      calls.push(`load:${file.relativePath}`);
      loadOptions.push(options);
    },
    async applyIntent(intent) {
      calls.push(`intent:${intent.kind}`);
      return { snapshot: snapshot() };
    },
    async setCenterView(view) { calls.push(`surface:${view}`); },
  }, {
    set(text, kind) { calls.push(`status:${kind}:${text}`); },
  });

  await service.activate("primary", activeDocument);

  assert.deepEqual(calls, [
    "intent:activate_document",
    "resolve:templates/index.html",
    "load:templates/index.html",
    "surface:preview",
  ]);
  assert.deepEqual(loadOptions, [{ syncWorkbench: false }]);
});

test("activările sincrone sunt coalescate și numai ultimul document ajunge în Rust", async () => {
  const calls = [];
  const first = otherDocument("sass/first.scss");
  const latest = otherDocument("sass/latest.scss");
  let activation = null;
  const service = new WorkbenchDocumentNavigationService({
    currentSnapshot: () => snapshot("secondary"),
    async resolveProjectFile(relativePath) {
      calls.push(`resolve:${relativePath}`);
      return { relativePath, role: "style" };
    },
    async loadProjectFile(file) { calls.push(`load:${file.relativePath}`); },
    async applyIntent(intent) { calls.push(`intent:${intent.documentId}`); return {}; },
    async setCenterView(view) { calls.push(`surface:${view}`); },
    beginDocumentActivation(serial, selected) {
      activation = { serial, path: selected.relativePath, phase: "applying" };
    },
    updateDocumentActivation(serial, patch) {
      if (activation?.serial === serial) activation = { ...activation, ...patch };
    },
  }, { set() {} });

  const superseded = service.activate("primary", first);
  const selected = service.activate("primary", latest);
  await Promise.all([superseded, selected]);

  assert.deepEqual(calls, [
    `intent:${latest.documentId}`,
    `resolve:${latest.relativePath}`,
    `load:${latest.relativePath}`,
    "surface:code",
  ]);
  assert.equal(activation.path, latest.relativePath);
  assert.equal(activation.phase, "ready");
  assert.equal(activation.cacheOutcome, "not_applicable");
});

test("un receipt de activare întârziat nu mai pornește încărcarea documentului stale", async () => {
  const calls = [];
  const firstIntent = deferred();
  const first = otherDocument("sass/first.scss");
  const latest = otherDocument("sass/latest.scss");
  let intents = 0;
  const service = new WorkbenchDocumentNavigationService({
    currentSnapshot: () => snapshot("secondary"),
    async resolveProjectFile(relativePath) {
      calls.push(`resolve:${relativePath}`);
      return { relativePath, role: "style" };
    },
    async loadProjectFile(file) { calls.push(`load:${file.relativePath}`); },
    async applyIntent(intent) {
      calls.push(`intent:${intent.documentId}`);
      intents += 1;
      return intents === 1 ? firstIntent.promise : {};
    },
    async setCenterView(view) { calls.push(`surface:${view}`); },
  }, { set() {} });

  const stale = service.activate("primary", first);
  await nextTurn();
  const current = service.activate("primary", latest);
  firstIntent.resolve({});
  await Promise.all([stale, current]);

  assert.deepEqual(calls, [
    `intent:${first.documentId}`,
    `intent:${latest.documentId}`,
    `resolve:${latest.relativePath}`,
    `load:${latest.relativePath}`,
    "surface:code",
  ]);
});

test("o încărcare stale deja pornită nu mai poate schimba suprafața finală", async () => {
  const calls = [];
  const firstLoad = deferred();
  const first = otherDocument("templates/first.html", "visual");
  const latest = otherDocument("sass/latest.scss", "code");
  const service = new WorkbenchDocumentNavigationService({
    currentSnapshot: () => snapshot("secondary"),
    async resolveProjectFile(relativePath) {
      calls.push(`resolve:${relativePath}`);
      return {
        relativePath,
        role: relativePath.endsWith(".html") ? "template" : "style",
      };
    },
    async loadProjectFile(file) {
      calls.push(`load:${file.relativePath}`);
      if (file.relativePath === first.relativePath) await firstLoad.promise;
    },
    async applyIntent(intent) { calls.push(`intent:${intent.documentId}`); return {}; },
    async setCenterView(view) { calls.push(`surface:${view}`); },
    currentTemplateCacheOutcome: () => "reused",
  }, { set() {} });

  const stale = service.activate("primary", first);
  await nextTurn();
  const current = service.activate("primary", latest);
  await current;
  firstLoad.resolve();
  await stale;

  assert.equal(calls.filter((call) => call === "surface:preview").length, 0);
  assert.equal(calls.at(-1), "surface:code");
});
