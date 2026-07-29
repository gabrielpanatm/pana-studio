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

function consumer(pageFile, pageTitle, pageUrl) {
  return {
    pageId: `page:${pageFile}`,
    pageFile,
    pageTitle,
    pageUrl,
    rootTemplateSourceId: "template-active",
    rootTemplateFile: "templates/blog/single.html",
    dependencyPath: [],
  };
}

function receipt(input, overrides = {}) {
  const previewRevision = `workbench-${input.expectedWorkspaceRevision}`;
  return {
    plan: {
      schemaVersion: 3,
      activeTemplate: {
        sourceId: "template-active",
        name: input.templatePath.replace(/^templates\//, ""),
        file: input.templatePath,
        origin: "local",
        themeName: null,
      },
      selectedContext: null,
      selectedRoute: null,
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
  const navigationRefreshes = [];
  const template = templateFile(activePath);
  const page = pageFile("content/_index.md");
  const host = {
    scannedProject: {
      root: "/project-a",
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
    activeCanvasUrl: "http://127.0.0.1:41000/",
    previewDocumentMarkup: "site",
    activeCanvasIdentity: null,
    templateWorkbenchPlan: null,
    templateWorkbenchPreferredPagePath: null,
    templateWorkbenchPreferredRoute: null,
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
    async refreshEditorNavigationSnapshot(identity, previewUrl) {
      navigationRefreshes.push({ identity, previewUrl });
    },
    previewUrlForScannedFile(file) {
      return `http://127.0.0.1:41000/${file.relativePath}`;
    },
    setGlobalStatus(text, kind) {
      statuses.push({ text, kind });
    },
  };
  return {
    host,
    page,
    statuses,
    refreshes,
    reconciliations,
    navigationRefreshes,
    template,
  };
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
    preferredRoute: null,
  });
  assert.equal(host.templateWorkbenchActive, true);
  assert.equal(host.templateWorkbenchTarget, template.relativePath);
  assert.equal(host.templateWorkbenchReturnPreviewPath, "content/_index.md");
  assert.match(host.previewSrc, /__pana_workbench\/template-active/);
  assert.match(statuses.at(-1).text, /Active template context/);
});

test("Template Workbench binds Canvas interaction to the mounted Workbench graph route", async () => {
  const {
    host,
    navigationRefreshes,
    template,
  } = workbenchHost();
  let expectedReceipt = null;
  mockIPC((command, payload) => {
    if (command === "read_project_workspace_state") {
      return {
        projectRoot: "/project-a",
        runtimeSessionId: "session-a:runtime-1",
        revision: 12,
      };
    }
    assert.equal(command, "project_template_workbench_preview");
    expectedReceipt = receipt(payload.input);
    host.activeCanvasIdentity = expectedReceipt.canvasProjection.identity;
    return expectedReceipt;
  });

  await updateTemplateWorkbenchContext(host, host.scannedProject, template, null, {
    strict: true,
  });

  assert.equal(host.activeCanvasUrl, expectedReceipt.previewUrl);
  assert.deepEqual(navigationRefreshes, [{
    identity: expectedReceipt.canvasProjection.identity,
    previewUrl: expectedReceipt.previewUrl,
  }]);
});

test("Template Workbench opens a collection template with the exact Rust-confirmed page context", async () => {
  let workbenchRequest = null;
  const { host, statuses } = workbenchHost("templates/blog/single.html");
  const article = pageFile("content/blog/articol.md");
  host.scannedProject.files.push(article);
  const context = consumer(article.relativePath, "Articol demonstrativ", "/blog/articol/");

  mockIPC((command, payload) => {
    if (command === "read_project_workspace_state") {
      return {
        projectRoot: "/project-a",
        runtimeSessionId: "session-a:runtime-1",
        revision: 9,
      };
    }
    assert.equal(command, "project_template_workbench_preview");
    workbenchRequest = payload.input;
    const base = receipt(payload.input);
    return {
      ...base,
      plan: {
        ...base.plan,
        consumers: [context],
        selectedContext: context,
        renderMode: "page",
        renderContext: {
          kind: "realZolaPage",
          canonicalTruth: true,
          label: "Articol demonstrativ",
          explanation: "Pagină Zola reală",
        },
      },
    };
  });

  const selected = await updateTemplateWorkbenchContext(
    host,
    host.scannedProject,
    host.scannedProject.files[0],
    article.relativePath,
  );

  assert.equal(workbenchRequest.preferredPagePath, article.relativePath);
  assert.equal(selected, article);
  assert.equal(host.templateWorkbenchPreferredPagePath, article.relativePath);
  assert.match(statuses.at(-1).text, /Articol demonstrativ/);
  assert.match(statuses.at(-1).text, /\/blog\/articol\//);
});

test("Template Workbench opens a taxonomy resource only with the exact Rust-confirmed route", async () => {
  let workbenchRequest = null;
  const { host, statuses } = workbenchHost("templates/tags/list.html");
  const route = {
    kind: "taxonomy_list",
    label: "Listă tags",
    url: "/tags/",
  };

  mockIPC((command, payload) => {
    if (command === "read_project_workspace_state") {
      return {
        projectRoot: "/project-a",
        runtimeSessionId: "session-a:runtime-1",
        revision: 11,
      };
    }
    assert.equal(command, "project_template_workbench_preview");
    workbenchRequest = payload.input;
    const base = receipt(payload.input);
    return {
      ...base,
      plan: {
        ...base.plan,
        selectedRoute: route,
        renderMode: "canonicalRoute",
        renderContext: {
          kind: "realZolaRoute",
          canonicalTruth: true,
          label: "Rută Zola reală",
          explanation: "Rută taxonomie verificată",
        },
      },
    };
  });

  await updateTemplateWorkbenchContext(
    host,
    host.scannedProject,
    host.scannedProject.files[0],
    null,
    { preferredRoute: route.url, strict: true },
  );

  assert.equal(workbenchRequest.preferredPagePath, null);
  assert.equal(workbenchRequest.preferredRoute, route.url);
  assert.equal(host.templateWorkbenchPreferredPagePath, null);
  assert.equal(host.templateWorkbenchPreferredRoute, route.url);
  assert.match(statuses.at(-1).text, /Listă tags/);
  assert.match(statuses.at(-1).text, /\/tags\//);
});

test("Template Workbench refuses a fallback page when an exact collection context was requested", async () => {
  const { host, statuses } = workbenchHost("templates/blog/single.html");
  const requestedArticle = pageFile("content/blog/cerut.md");
  host.scannedProject.files.push(requestedArticle);
  const wrongContext = consumer("content/blog/altul.md", "Alt articol", "/blog/altul/");

  mockIPC((command, payload) => {
    if (command === "read_project_workspace_state") {
      return {
        projectRoot: "/project-a",
        runtimeSessionId: "session-a:runtime-1",
        revision: 10,
      };
    }
    const base = receipt(payload.input);
    return {
      ...base,
      plan: {
        ...base.plan,
        consumers: [wrongContext],
        selectedContext: wrongContext,
      },
    };
  });

  await updateTemplateWorkbenchContext(
    host,
    host.scannedProject,
    host.scannedProject.files[0],
    requestedArticle.relativePath,
  );

  assert.equal(host.templateWorkbenchActive, false);
  assert.equal(host.templateWorkbenchPlan, null);
  assert.match(statuses.at(-1).text, /did not confirm requested page/);
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
  assert.match(statuses.at(-1).text, /receipt for another revision/);
});
