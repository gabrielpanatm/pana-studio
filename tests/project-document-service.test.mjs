import assert from "node:assert/strict";
import { test } from "node:test";

import { ProjectDocumentService } from "$lib/project/document-service";

function projectFile(relativePath, role, kind = "OTHER") {
  return {
    name: relativePath.split("/").at(-1),
    relativePath,
    absolutePath: `/project/${relativePath}`,
    kind,
    role,
    previewPath: null,
  };
}

function fixture(file) {
  let navigationRefreshes = 0;
  let workbenchOpens = 0;
  const project = {
    root: "/project",
    runtimeSessionId: "session:runtime",
    epoch: 1,
    status: "",
    project: {
      root: "/project",
      files: [file],
    },
  };
  const documents = {
    activeScannedPath: null,
    activePreviewPath: "templates/index.html",
    browserPreviewRoute: "/",
    templatePlan: null,
    templatePreferredPagePath: null,
    templatePreferredRoute: null,
    templateActive: false,
    templateTarget: null,
  };
  const source = {
    source: "",
    sourceCache: { [`scanned:${file.relativePath}`]: "fixture" },
  };
  const preview = {
    src: "http://127.0.0.1/preview/",
    documentMarkup: null,
    pendingProjection: null,
    activeIdentity: { transactionId: "canvas:test" },
    setPendingProjection() {},
    urlForFile() { return "about:blank"; },
    async refreshDocument() { return true; },
    cancelSync() {},
  };
  const workbench = {
    isHydrated() { return true; },
    async openDocument() { workbenchOpens += 1; },
  };
  const selection = {
    session: {
      async refreshNavigationSnapshot() { navigationRefreshes += 1; },
    },
  };
  const service = new ProjectDocumentService({
    project,
    documents,
    source,
    preview,
    shell: { centerView: "preview" },
    template: {
      async exit() {},
      async update() { return null; },
    },
    authority: {
      async runStructural(operation) { return await operation({}); },
      async settle() { return { warnings: [] }; },
    },
    workbench,
    selection,
    status: { clear() {}, escalate() {}, set() {} },
  });
  return {
    service,
    navigationRefreshes: () => navigationRefreshes,
    workbenchOpens: () => workbenchOpens,
  };
}

test("documentele fără proiecție EditorNavigation nu reconstruiesc graful Canvas", async () => {
  const file = projectFile("config.toml", "asset");
  const state = fixture(file);

  await state.service.load(file, { skipDraftFlush: true });

  assert.equal(state.workbenchOpens(), 1);
  assert.equal(state.navigationRefreshes(), 0);
});

test("template-urile păstrează refreshul semantic după sincronizarea Workbench", async () => {
  const file = projectFile("templates/index.html", "template", "HTML");
  const state = fixture(file);

  await state.service.load(file, {
    skipDraftFlush: true,
    activateTemplateWorkbench: false,
  });

  assert.equal(state.workbenchOpens(), 1);
  assert.equal(state.navigationRefreshes(), 1);
});
