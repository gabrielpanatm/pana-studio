import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  exitTemplateWorkbench,
  updateTemplateWorkbenchContext,
} from "$lib/state/project-controller";

if (!globalThis.window) globalThis.window = globalThis;

afterEach(() => {
  clearMocks();
});

function deferred() {
  let resolve;
  const promise = new Promise((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

async function nextTurn() {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

function templateFile(relativePath) {
  return { relativePath, role: "template" };
}

function pageFile(relativePath) {
  return { relativePath, role: "page" };
}

function receipt(input, overrides = {}) {
  const previewRevision = `workbench-${input.expectedWorkspaceRevision}`;
  return {
    plan: {
      schemaVersion: 2,
      activeTemplate: {
        sourceId: "template-active",
        name: input.templatePath.replace(/^templates\//, ""),
        file: input.templatePath,
        origin: "local",
        themeName: null,
      },
      selectedContext: null,
      navigator: [],
      renderMode: "orphan",
      renderContext: {
        kind: "controlledTemplateFixture",
        canonicalTruth: false,
        label: "Scenariu controlat",
        explanation: "Context de test",
      },
      diagnostics: [],
    },
    route: "/__pana_workbench/template-active/",
    previewUrl: "http://127.0.0.1:41000/__pana_workbench/template-active/",
    workspaceRevision: input.expectedWorkspaceRevision,
    previewRevision,
    canvasProjection: {
      schemaVersion: 1,
      identity: {
        projectRoot: input.expectedProjectRoot,
        runtimeSessionId: input.expectedSessionId,
        workspaceRevision: input.expectedWorkspaceRevision,
        transactionId: `canvas-${input.expectedWorkspaceRevision}`,
        previewRevision,
      },
      workspaceTransactionId: `workspace-${input.expectedWorkspaceRevision}`,
      phase: "canonicalVerified",
      impact: { kinds: ["htmlStructure"], paths: [input.templatePath], requiresFullDocument: false },
      resources: { schemaVersion: 1, previewRevision, totalBytes: 0, entries: [] },
    },
    ...overrides,
  };
}

function workbenchHost(activePath = "templates/partials/header.html") {
  const statuses = [];
  const refreshes = [];
  const reconciliations = [];
  const template = templateFile(activePath);
  const page = pageFile("content/_index.md");
  const host = {
    scannedProject: {
      root: "/project-a",
      isZola: true,
      previewBaseUrl: "http://127.0.0.1:41000",
      files: [template, page],
    },
    sessionProjectRoot: "/project-a",
    kernelProjectSessionId: "session-a:runtime-1",
    projectSessionEpoch: 1,
    projectWorkspaceMutationEpoch: 0,
    activeScannedPath: activePath,
    activePreviewPath: page.relativePath,
    previewSrc: "http://127.0.0.1:41000/",
    previewDocumentMarkup: "site",
    activeCanvasIdentity: null,
    templateWorkbenchPlan: null,
    templateWorkbenchPreferredPagePath: null,
    templateWorkbenchActive: false,
    templateWorkbenchTarget: null,
    templateWorkbenchReturnPreviewPath: null,
    templateWorkbenchRequestSerial: 0,
    async refreshRenderedPreviewDocument() {
      refreshes.push(this.previewSrc);
      return true;
    },
    async reconcileTemplateWorkbenchPreviewDocument(previewUrl, canvasProjection) {
      reconciliations.push({ previewUrl, canvasProjection });
      this.activeCanvasIdentity = canvasProjection.identity;
      return true;
    },
    previewUrlForScannedFile(file) {
      return `http://127.0.0.1:41000/${file.relativePath}`;
    },
    setGlobalStatus(text, kind) {
      statuses.push({ text, kind });
    },
  };
  return { host, page, statuses, refreshes, reconciliations, template };
}

test("Template Workbench binds activation to the exact ProjectWorkspace revision and source", async () => {
  let workbenchRequest = null;
  mockIPC((command, payload) => {
    if (command === "read_project_workspace_state") {
      return {
        projectRoot: "/project-a",
        runtimeSessionId: "session-a:runtime-1",
        revision: 7,
      };
    }
    assert.equal(command, "project_template_workbench_preview");
    workbenchRequest = payload.input;
    return receipt(payload.input);
  });
  const { host, statuses, template } = workbenchHost();

  await updateTemplateWorkbenchContext(host, host.scannedProject, template);

  assert.deepEqual(workbenchRequest, {
    expectedProjectRoot: "/project-a",
    expectedSessionId: "session-a:runtime-1",
    expectedWorkspaceRevision: 7,
    templatePath: template.relativePath,
    preferredPagePath: null,
  });
  assert.equal(host.templateWorkbenchActive, true);
  assert.equal(host.templateWorkbenchTarget, template.relativePath);
  assert.equal(host.templateWorkbenchReturnPreviewPath, "content/_index.md");
  assert.match(host.previewSrc, /__pana_workbench\/template-active/);
  assert.match(statuses.at(-1).text, /Context de template activ/);
});

test("a late Workbench result has zero UI effects after the selected source changes", async () => {
  const gate = deferred();
  let request = null;
  mockIPC((command, payload) => {
    if (command === "read_project_workspace_state") {
      return {
        projectRoot: "/project-a",
        runtimeSessionId: "session-a:runtime-1",
        revision: 3,
      };
    }
    request = payload.input;
    return gate.promise;
  });
  const { host, statuses, template } = workbenchHost();

  const opening = updateTemplateWorkbenchContext(host, host.scannedProject, template);
  await nextTurn();
  assert.ok(request, "Workbench request should be in flight before changing the source");
  host.activeScannedPath = "templates/partials/footer.html";
  gate.resolve(receipt(request));
  await opening;

  assert.equal(host.templateWorkbenchActive, false);
  assert.equal(host.templateWorkbenchTarget, null);
  assert.equal(host.previewSrc, "http://127.0.0.1:41000/");
  assert.deepEqual(statuses, []);
});

test("a staged Workbench revision is reconciled in place before it becomes canonical", async () => {
  mockIPC((command, payload) => {
    if (command === "read_project_workspace_state") {
      return {
        projectRoot: "/project-a",
        runtimeSessionId: "session-a:runtime-1",
        revision: 8,
      };
    }
    return receipt(payload.input, {
      canvasProjection: {
        ...receipt(payload.input).canvasProjection,
        phase: "prepared",
      },
    });
  });
  const { host, reconciliations, refreshes, template } = workbenchHost();

  await updateTemplateWorkbenchContext(host, host.scannedProject, template, null, {
    strict: true,
  });

  assert.equal(reconciliations.length, 1);
  assert.match(reconciliations[0].previewUrl, /__pana_workbench\/template-active/);
  assert.equal(reconciliations[0].canvasProjection.phase, "prepared");
  assert.equal(host.activeCanvasIdentity.workspaceRevision, 8);
  assert.deepEqual(refreshes, []);
});

test("exiting Workbench returns to the real page preview without a second server lifecycle", async () => {
  const { host, page, refreshes } = workbenchHost();
  host.templateWorkbenchActive = true;
  host.templateWorkbenchTarget = host.activeScannedPath;
  host.templateWorkbenchReturnPreviewPath = page.relativePath;
  host.templateWorkbenchPlan = receipt({
    expectedProjectRoot: "/project-a",
    expectedSessionId: "session-a:runtime-1",
    expectedWorkspaceRevision: 1,
    templatePath: host.activeScannedPath,
  }).plan;
  host.previewSrc = "http://127.0.0.1:41000/__pana_workbench/template-active/";

  await exitTemplateWorkbench(host);

  assert.equal(host.templateWorkbenchActive, false);
  assert.equal(host.templateWorkbenchPlan, null);
  assert.equal(host.activePreviewPath, page.relativePath);
  assert.equal(host.previewSrc, `http://127.0.0.1:41000/${page.relativePath}`);
  assert.deepEqual(refreshes, [host.previewSrc]);
});

test("an invalid Workbench receipt is fail-closed and preserves the current preview", async () => {
  mockIPC((command, payload) => {
    if (command === "read_project_workspace_state") {
      return {
        projectRoot: "/project-a",
        runtimeSessionId: "session-a:runtime-1",
        revision: 5,
      };
    }
    return receipt(payload.input, { workspaceRevision: 4 });
  });
  const { host, statuses, template } = workbenchHost();

  await updateTemplateWorkbenchContext(host, host.scannedProject, template);

  assert.equal(host.templateWorkbenchActive, false);
  assert.equal(host.templateWorkbenchPlan, null);
  assert.equal(host.previewSrc, "http://127.0.0.1:41000/");
  assert.match(statuses.at(-1).text, /receipt pentru altă revizie/);
});
