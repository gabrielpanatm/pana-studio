import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  exitTemplateWorkbench,
  updateTemplateWorkbenchContext,
} from "$lib/state/project-template-workbench-controller";
import { TemplateWorkbenchService } from "$lib/project/template-workbench-service";
import { loadScannedProjectFile } from "$lib/state/project-document-controller";

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
  return { relativePath, role: "page", kind: "MD", previewPath: "/" };
}

function styleFile(relativePath) {
  return { relativePath, role: "style", kind: "SCSS", previewPath: null };
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
      schemaVersion: 5,
      projectModelRevision: `model-${input.expectedWorkspaceRevision}`,
      activeTemplate: {
        sourceId: "template-active",
        name: input.templatePath.replace(/^templates\//, ""),
        file: input.templatePath,
        origin: "local",
        themeName: null,
        isPartial: false,
        definesComponents: Boolean(input.preferredComponentName),
      },
      activeComponentName: input.preferredComponentName ?? null,
      directParent: null,
      selectedContext: null,
      selectedRoute: null,
      navigator: [],
      consumers: [],
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
    reuseToken: `sha256:workbench-${input.expectedWorkspaceRevision}`,
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
    publicationStatus: "materialized",
    performance: {
      totalUs: 1,
      operationLockWaitUs: 0,
      projectModelUs: 0,
      planUs: 0,
      engineLockWaitUs: 0,
      publishUs: 1,
      renderUs: 0,
      graphUs: 0,
      prepareUs: 0,
      modelCacheHit: true,
    },
    ...overrides,
  };
}

function reuseConfirmation(input, overrides = {}) {
  return {
    status: "confirmed",
    route: "/__pana_workbench/template-active/",
    previewUrl: "http://127.0.0.1:41000/__pana_workbench/template-active/",
    reuseToken: input.reuseToken,
    workspaceRevision: input.expectedWorkspaceRevision,
    previewRevision: input.expectedPreviewRevision,
    canvasTransactionId: input.expectedCanvasTransactionId,
    performance: {
      totalUs: 1,
      operationLockWaitUs: 0,
      engineLockWaitUs: 0,
    },
    ...overrides,
  };
}

function fileBufferTextReceipt(relativePath, text) {
  return {
    projectRoot: "/project-a",
    runtimeSessionId: "session-a:runtime-1",
    workspaceRevision: 0,
    payload: {
      relativePath,
      text,
      dirty: false,
      hash: `fixture-${relativePath}-${text.length}`,
      bytes: new TextEncoder().encode(text).byteLength,
      revision: 1,
    },
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
    projectLifecycle: {
      schemaVersion: 1,
      revision: 1,
      activeSession: null,
      transition: "idle",
      operationId: null,
      transitionStartedAtMs: null,
      reason: "test",
    },
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
    templateWorkbenchReuseToken: null,
    editorSelection: {
      selectionSnapshot: null,
      reset() {},
      async refreshNavigationSnapshot(identity, previewUrl) {
        navigationRefreshes.push({ identity, previewUrl });
      },
    },
    async refreshRenderedPreviewDocument() {
      refreshes.push(this.previewSrc);
      return true;
    },
    templateWorkbenchCanvas: {
      async reconcile(previewUrl, canvasProjection) {
        reconciliations.push({ previewUrl, canvasProjection });
        host.activeCanvasIdentity = canvasProjection.identity;
        return true;
      },
      canReuse() {
        return false;
      },
      getReuseToken() {
        return host.templateWorkbenchReuseToken;
      },
      setReuseToken(token) {
        host.templateWorkbenchReuseToken = token;
      },
      setPublicationStatus(status) {
        host.templatePublicationStatus = status;
      },
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
    preferredComponentName: null,
  });
  assert.equal(host.templateWorkbenchActive, true);
  assert.equal(host.templateWorkbenchTarget, template.relativePath);
  assert.equal(host.templateWorkbenchReturnPreviewPath, "content/_index.md");
  assert.match(host.previewSrc, /__pana_workbench\/template-active/);
  assert.match(statuses.at(-1).text, /Active template context/);
});

test("Component Workspace requests the exact Tera 2 symbol for Preview", async () => {
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
  const { host, template } = workbenchHost();

  await updateTemplateWorkbenchContext(host, host.scannedProject, template, null, {
    preferredComponentName: "ui.secondary",
    strict: true,
  });

  assert.equal(workbenchRequest.preferredComponentName, "ui.secondary");
  assert.equal(host.templateWorkbenchPlan.activeComponentName, "ui.secondary");
});

test("a superseded Template Workbench revision never requests a generation that is not materialized", async () => {
  let previewRequests = 0;
  mockIPC((command) => {
    if (command === "read_project_workspace_state") {
      return {
        projectRoot: "/project-a",
        runtimeSessionId: "session-a:runtime-1",
        revision: 13,
      };
    }
    if (command === "project_template_workbench_preview") previewRequests += 1;
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });
  const { host, statuses, template } = workbenchHost();

  const selected = await updateTemplateWorkbenchContext(
    host,
    host.scannedProject,
    template,
    null,
    {
      expectedWorkspaceRevision: 12,
      minimumWorkspaceRevision: 12,
      strict: true,
    },
  );

  assert.equal(selected, null);
  assert.equal(previewRequests, 0);
  assert.deepEqual(statuses, []);
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

test("an exact Rust cache hit reuses the mounted canonical Workbench surface without navigation", async () => {
  const { host, refreshes, navigationRefreshes, template } = workbenchHost();
  const input = {
    expectedProjectRoot: host.sessionProjectRoot,
    expectedSessionId: host.kernelProjectSessionId,
    expectedWorkspaceRevision: 12,
    templatePath: template.relativePath,
  };
  const cached = receipt(input, { publicationStatus: "reused" });
  host.projectWorkspaceSnapshot = {
    projectRoot: input.expectedProjectRoot,
    runtimeSessionId: input.expectedSessionId,
    revision: input.expectedWorkspaceRevision,
  };
  host.templateWorkbenchActive = true;
  host.templateWorkbenchTarget = template.relativePath;
  host.templateWorkbenchPlan = cached.plan;
  host.templateWorkbenchPreferredPagePath = null;
  host.templateWorkbenchPreferredRoute = null;
  host.templateWorkbenchReuseToken = cached.reuseToken;
  host.activePreviewPath = template.relativePath;
  host.activeCanvasIdentity = cached.canvasProjection.identity;
  host.activeCanvasUrl = `${cached.previewUrl}?mounted=1`;
  host.previewSrc = `${cached.previewUrl}?__pana_reload=9`;
  host.previewDocumentMarkup = null;
  host.templateWorkbenchCanvas.canReuse = (identity, previewUrl) => (
    identity === cached.canvasProjection.identity && previewUrl === cached.previewUrl
  );

  let fullPreviewRequests = 0;
  mockIPC((command, payload) => {
    if (command === "project_template_workbench_preview") {
      fullPreviewRequests += 1;
      return cached;
    }
    assert.equal(command, "confirm_template_workbench_reuse");
    assert.deepEqual(payload.input, {
      ...input,
      preferredPagePath: null,
      preferredRoute: null,
      preferredComponentName: null,
      reuseToken: cached.reuseToken,
      expectedPreviewRevision: cached.previewRevision,
      expectedCanvasTransactionId: cached.canvasProjection.identity.transactionId,
    });
    return reuseConfirmation(payload.input);
  });

  await updateTemplateWorkbenchContext(host, host.scannedProject, template, null, {
    strict: true,
  });

  assert.deepEqual(refreshes, []);
  assert.equal(fullPreviewRequests, 0);
  assert.equal(host.previewSrc, `${cached.previewUrl}?__pana_reload=9`);
  assert.equal(host.activeCanvasUrl, cached.previewUrl);
  assert.equal(navigationRefreshes.length, 0);
});

test("a compact Rust reuse miss falls back to full authoritative publication", async () => {
  const { host, template } = workbenchHost();
  const input = {
    expectedProjectRoot: host.sessionProjectRoot,
    expectedSessionId: host.kernelProjectSessionId,
    expectedWorkspaceRevision: 12,
    templatePath: template.relativePath,
  };
  const cached = receipt(input, { publicationStatus: "reused" });
  const replacement = receipt(input, {
    publicationStatus: "reused",
    reuseToken: "sha256:replacement",
  });
  host.projectWorkspaceSnapshot = {
    projectRoot: input.expectedProjectRoot,
    runtimeSessionId: input.expectedSessionId,
    revision: input.expectedWorkspaceRevision,
  };
  host.templateWorkbenchActive = true;
  host.templateWorkbenchTarget = template.relativePath;
  host.templateWorkbenchPlan = cached.plan;
  host.templateWorkbenchPreferredPagePath = null;
  host.templateWorkbenchPreferredRoute = null;
  host.templateWorkbenchReuseToken = cached.reuseToken;
  host.activePreviewPath = template.relativePath;
  host.activeCanvasIdentity = cached.canvasProjection.identity;
  host.activeCanvasUrl = cached.previewUrl;
  host.previewSrc = cached.previewUrl;
  host.previewDocumentMarkup = null;
  host.templateWorkbenchCanvas.canReuse = () => true;

  const calls = [];
  mockIPC((command, payload) => {
    calls.push(command);
    if (command === "confirm_template_workbench_reuse") {
      return reuseConfirmation(payload.input, {
        status: "miss",
        route: null,
        previewUrl: null,
        reuseToken: null,
        previewRevision: null,
        canvasTransactionId: null,
      });
    }
    assert.equal(command, "project_template_workbench_preview");
    return replacement;
  });

  await updateTemplateWorkbenchContext(host, host.scannedProject, template, null, {
    strict: true,
  });

  assert.deepEqual(calls, [
    "confirm_template_workbench_reuse",
    "project_template_workbench_preview",
  ]);
  assert.equal(host.templateWorkbenchReuseToken, "sha256:replacement");
});

test("a late compact reuse confirmation has zero UI effects after rapid switching", async () => {
  const gate = deferred();
  const { host, statuses, template } = workbenchHost();
  const input = {
    expectedProjectRoot: host.sessionProjectRoot,
    expectedSessionId: host.kernelProjectSessionId,
    expectedWorkspaceRevision: 12,
    templatePath: template.relativePath,
  };
  const cached = receipt(input, { publicationStatus: "reused" });
  host.projectWorkspaceSnapshot = {
    projectRoot: input.expectedProjectRoot,
    runtimeSessionId: input.expectedSessionId,
    revision: input.expectedWorkspaceRevision,
  };
  host.templateWorkbenchActive = true;
  host.templateWorkbenchTarget = template.relativePath;
  host.templateWorkbenchPlan = cached.plan;
  host.templateWorkbenchPreferredPagePath = null;
  host.templateWorkbenchPreferredRoute = null;
  host.templateWorkbenchReuseToken = cached.reuseToken;
  host.activePreviewPath = template.relativePath;
  host.activeCanvasIdentity = cached.canvasProjection.identity;
  host.activeCanvasUrl = cached.previewUrl;
  host.previewSrc = cached.previewUrl;
  host.previewDocumentMarkup = null;
  host.templateWorkbenchCanvas.canReuse = () => true;

  let fullPreviewRequests = 0;
  let reuseRequest = null;
  mockIPC((command, payload) => {
    if (command === "project_template_workbench_preview") {
      fullPreviewRequests += 1;
      return cached;
    }
    assert.equal(command, "confirm_template_workbench_reuse");
    reuseRequest = payload.input;
    return gate.promise;
  });

  const opening = updateTemplateWorkbenchContext(host, host.scannedProject, template, null, {
    strict: true,
  });
  await nextTurn();
  host.activeScannedPath = "sass/site.scss";
  gate.resolve(reuseConfirmation(reuseRequest));

  assert.equal(await opening, null);
  assert.equal(fullPreviewRequests, 0);
  assert.deepEqual(statuses, []);
});

test("a Rust cache hit still refreshes when the mounted Canvas generation is not reusable", async () => {
  const { host, refreshes, template } = workbenchHost();
  const input = {
    expectedProjectRoot: host.sessionProjectRoot,
    expectedSessionId: host.kernelProjectSessionId,
    expectedWorkspaceRevision: 12,
    templatePath: template.relativePath,
  };
  const cached = receipt(input, { publicationStatus: "reused" });
  host.projectWorkspaceSnapshot = {
    projectRoot: input.expectedProjectRoot,
    runtimeSessionId: input.expectedSessionId,
    revision: input.expectedWorkspaceRevision,
  };
  host.templateWorkbenchActive = true;
  host.templateWorkbenchTarget = template.relativePath;
  host.templateWorkbenchPlan = cached.plan;
  host.activePreviewPath = template.relativePath;
  host.previewDocumentMarkup = null;
  host.templateWorkbenchCanvas.canReuse = () => false;

  mockIPC((command) => command === "read_project_workspace_state"
    ? {
        projectRoot: input.expectedProjectRoot,
        runtimeSessionId: input.expectedSessionId,
        revision: input.expectedWorkspaceRevision,
      }
    : cached);

  await updateTemplateWorkbenchContext(host, host.scannedProject, template, null, {
    strict: true,
  });

  assert.deepEqual(refreshes, [cached.previewUrl]);
});

test("a Rust cache hit is invalidated when the confirmed template route context changes", async () => {
  const { host, refreshes, template } = workbenchHost();
  const input = {
    expectedProjectRoot: host.sessionProjectRoot,
    expectedSessionId: host.kernelProjectSessionId,
    expectedWorkspaceRevision: 12,
    templatePath: template.relativePath,
  };
  const previous = receipt(input, { publicationStatus: "reused" });
  const changed = {
    ...previous,
    plan: {
      ...previous.plan,
      renderMode: "canonicalRoute",
      selectedRoute: { kind: "taxonomy_list", label: "Tags", url: "/tags/" },
    },
  };
  host.projectWorkspaceSnapshot = {
    projectRoot: input.expectedProjectRoot,
    runtimeSessionId: input.expectedSessionId,
    revision: input.expectedWorkspaceRevision,
  };
  host.templateWorkbenchActive = true;
  host.templateWorkbenchTarget = template.relativePath;
  host.templateWorkbenchPlan = previous.plan;
  host.activePreviewPath = template.relativePath;
  host.previewDocumentMarkup = null;
  host.templateWorkbenchCanvas.canReuse = () => true;

  mockIPC((command) => command === "read_project_workspace_state"
    ? {
        projectRoot: input.expectedProjectRoot,
        runtimeSessionId: input.expectedSessionId,
        revision: input.expectedWorkspaceRevision,
      }
    : changed);

  await updateTemplateWorkbenchContext(host, host.scannedProject, template, null, {
    strict: true,
  });

  assert.deepEqual(refreshes, [changed.previewUrl]);
  assert.equal(host.templateWorkbenchPreferredRoute, "/tags/");
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

test("switching templates does not carry the previous template page context", async () => {
  let workbenchRequest = null;
  const { host } = workbenchHost("templates/index.html");
  const contactTemplate = templateFile("templates/contact.html");
  const contactPage = pageFile("content/contact.md");
  const contactContext = consumer(contactPage.relativePath, "Contact", "/contact/");
  host.scannedProject.files.push(contactTemplate, contactPage);
  host.templateWorkbenchActive = true;
  host.templateWorkbenchTarget = "templates/index.html";
  host.templateWorkbenchPreferredPagePath = "content/_index.md";
  host.templateWorkbenchPreferredRoute = "/";
  host.source = "index";
  host.sourceCache = {
    "scanned:templates/contact.html": "contact",
  };
  host.updateTemplateWorkbenchContext = async (...args) => {
    return await updateTemplateWorkbenchContext(host, ...args);
  };

  mockIPC((command, payload) => {
    if (command === "read_project_workspace_state") {
      return {
        projectRoot: "/project-a",
        runtimeSessionId: "session-a:runtime-1",
        revision: 13,
      };
    }
    assert.equal(command, "project_template_workbench_preview");
    workbenchRequest = payload.input;
    const base = receipt(payload.input);
    return {
      ...base,
      plan: {
        ...base.plan,
        consumers: [contactContext],
        selectedContext: contactContext,
        renderMode: "page",
        renderContext: {
          kind: "realZolaPage",
          canonicalTruth: true,
          label: "Contact",
          explanation: "Pagină Zola reală",
        },
      },
    };
  });

  await loadScannedProjectFile(host, contactTemplate, {
    skipDraftFlush: true,
    strict: true,
  });

  assert.equal(workbenchRequest.preferredPagePath, null);
  assert.equal(workbenchRequest.preferredRoute, null);
  assert.equal(host.templateWorkbenchTarget, contactTemplate.relativePath);
  assert.equal(host.templateWorkbenchPreferredPagePath, contactPage.relativePath);
  assert.equal(host.source, "contact");
});

test("reopening the active template preserves its confirmed context", async () => {
  const { host, template } = workbenchHost("templates/contact.html");
  const calls = [];
  host.templateWorkbenchActive = true;
  host.templateWorkbenchTarget = template.relativePath;
  host.templateWorkbenchPreferredPagePath = "content/contact.md";
  host.templateWorkbenchPreferredRoute = "/contact/";
  host.source = "contact";
  host.sourceCache = {
    [`scanned:${template.relativePath}`]: "contact",
  };
  host.updateTemplateWorkbenchContext = async (...args) => {
    calls.push(args);
    return null;
  };

  await loadScannedProjectFile(host, template, {
    skipDraftFlush: true,
    strict: true,
  });

  assert.equal(calls.length, 1);
  assert.equal(calls[0][2], "content/contact.md");
  assert.equal(calls[0][3].preferredRoute, "/contact/");
});

test("opening a style source preserves the mounted Template Workbench preview", async () => {
  const { host } = workbenchHost("templates/index.html");
  const style = styleFile("sass/css-framework/_baza.scss");
  const initialPreview = host.previewSrc;
  let exitCalls = 0;
  host.templateWorkbenchActive = true;
  host.source = "";
  host.sourceCache = {};
  host.exitTemplateWorkbench = async () => {
    exitCalls += 1;
    throw new Error("Preview navigation must not run for source-only files");
  };
  mockIPC((command, payload) => {
    assert.equal(command, "read_file_buffer_text");
    assert.equal(payload.relativePath, style.relativePath);
    return fileBufferTextReceipt(style.relativePath, "$culoare: red;\n");
  });

  await loadScannedProjectFile(host, style, { skipDraftFlush: true, strict: true });

  assert.equal(host.source, "$culoare: red;\n");
  assert.equal(host.activeScannedPath, style.relativePath);
  assert.equal(host.previewSrc, initialPreview);
  assert.equal(host.templateWorkbenchActive, true);
  assert.equal(exitCalls, 0);
});

test("page source is committed before a failing Preview transition", async () => {
  const { host, page } = workbenchHost("templates/index.html");
  host.templateWorkbenchActive = true;
  host.source = "";
  host.sourceCache = {};
  host.exitTemplateWorkbench = async () => {
    throw new Error("Canvas route unavailable");
  };
  mockIPC((command, payload) => {
    assert.equal(command, "read_file_buffer_text");
    return fileBufferTextReceipt(
      payload.relativePath,
      "+++\ntitle = 'Acasă'\n+++\n",
    );
  });

  await assert.rejects(
    loadScannedProjectFile(host, page, { skipDraftFlush: true, strict: true }),
    /Canvas route unavailable/,
  );

  assert.equal(host.source, "+++\ntitle = 'Acasă'\n+++\n");
  assert.equal(host.activeScannedPath, page.relativePath);
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

test("a background Workbench projection remains valid while the active document is SCSS", async () => {
  const { host, template } = workbenchHost();
  host.templateWorkbenchActive = true;
  host.templateWorkbenchTarget = template.relativePath;
  host.activePreviewPath = template.relativePath;
  host.activeScannedPath = "sass/css-framework/_componente.scss";
  host.projectWorkspaceSnapshot = {
    projectRoot: "/project-a",
    runtimeSessionId: "session-a:runtime-1",
    revision: 9,
  };
  host.scannedProject.files.push(styleFile(host.activeScannedPath));
  mockIPC((command, payload) => {
    if (command === "read_project_workspace_state") {
      return {
        projectRoot: "/project-a",
        runtimeSessionId: "session-a:runtime-1",
        revision: 9,
      };
    }
    assert.equal(command, "project_template_workbench_preview");
    const projected = receipt(payload.input);
    return {
      ...projected,
      canvasProjection: { ...projected.canvasProjection, phase: "prepared" },
    };
  });

  await updateTemplateWorkbenchContext(host, host.scannedProject, template, null, {
    bindToActiveDocument: false,
    expectedWorkspaceRevision: 9,
    strict: true,
  });

  assert.equal(host.activeScannedPath, "sass/css-framework/_componente.scss");
  assert.equal(host.templateWorkbenchActive, true);
  assert.equal(host.templateWorkbenchTarget, template.relativePath);
  assert.equal(host.templateWorkbenchPlan.activeTemplate.file, template.relativePath);
  assert.equal(host.activeCanvasIdentity.workspaceRevision, 9);
});

test("reactivating an exact canonical template refreshes navigation without reprojecting it", async () => {
  const template = templateFile("templates/index.html");
  const project = {
    root: "/project-a",
    files: [template],
  };
  const canonical = receipt({
    expectedProjectRoot: "/project-a",
    expectedSessionId: "session-a:runtime-1",
    expectedWorkspaceRevision: 11,
    templatePath: template.relativePath,
  });
  let joinCalls = 0;
  let navigationRefreshes = 0;
  mockIPC((command) => {
    throw new Error(`Nu trebuia emis IPC Template pentru contextul canonic: ${command}`);
  });
  const service = new TemplateWorkbenchService({
    project: {
      root: "/project-a",
      runtimeSessionId: "session-a:runtime-1",
      workspace: { revision: 11 },
      project,
    },
    documents: {
      templateActive: true,
      templateTarget: template.relativePath,
      templatePlan: canonical.plan,
      templatePreferredPagePath: null,
      templatePreferredRoute: null,
    },
    preview: {
      activeIdentity: canonical.canvasProjection.identity,
      activeUrl: canonical.previewUrl,
      src: canonical.previewUrl,
      canReuseCanonicalWorkbenchSurface(identity, url) {
        return identity === canonical.canvasProjection.identity && url === canonical.previewUrl;
      },
    },
    selection: {
      session: {
        async refreshNavigationSnapshot(identity, url, options) {
          navigationRefreshes += 1;
          assert.equal(identity, canonical.canvasProjection.identity);
          assert.equal(url, canonical.previewUrl);
          assert.equal(options.strict, true);
        },
      },
    },
    status: {},
    joinProjection() {
      joinCalls += 1;
      return null;
    },
  });

  const selected = await service.update(project, template);

  assert.equal(selected, null);
  assert.equal(joinCalls, 0);
  assert.equal(navigationRefreshes, 1);
});

test("canonical metadata without a reusable Canvas surface falls back to Workbench reconciliation", async () => {
  const template = templateFile("templates/index.html");
  const projectScan = { root: "/project-a", files: [template] };
  const canonical = receipt({
    expectedProjectRoot: "/project-a",
    expectedSessionId: "session-a:runtime-1",
    expectedWorkspaceRevision: 12,
    templatePath: template.relativePath,
  });
  let projections = 0;
  let refreshes = 0;
  let navigationRefreshes = 0;
  mockIPC((command, payload) => {
    assert.equal(command, "project_template_workbench_preview");
    projections += 1;
    return receipt(payload.input);
  });
  const project = {
    root: "/project-a",
    runtimeSessionId: "session-a:runtime-1",
    epoch: 1,
    workspaceMutationEpoch: 0,
    workspace: {
      projectRoot: "/project-a",
      runtimeSessionId: "session-a:runtime-1",
      revision: 12,
    },
    project: projectScan,
    lifecycle: undefined,
  };
  const documents = {
    activeScannedPath: template.relativePath,
    activePreviewPath: template.relativePath,
    browserPreviewRoute: canonical.route,
    templateActive: true,
    templateTarget: template.relativePath,
    templatePlan: canonical.plan,
    templatePreferredPagePath: null,
    templatePreferredRoute: null,
    templateRequestSerial: 0,
    templateReturnPreviewPath: null,
    templateReuseToken: null,
    templatePublicationStatus: "materialized",
  };
  const preview = {
    activeIdentity: canonical.canvasProjection.identity,
    activeUrl: canonical.previewUrl,
    src: canonical.previewUrl,
    documentMarkup: null,
    canReuseCanonicalWorkbenchSurface() { return false; },
    async reconcileWorkbenchDocument() { return true; },
    async refreshDocument() { refreshes += 1; return true; },
  };
  const service = new TemplateWorkbenchService({
    project,
    documents,
    preview,
    selection: {
      session: {
        async refreshNavigationSnapshot() { navigationRefreshes += 1; },
      },
    },
    status: { set() {} },
  });

  await service.update(projectScan, template);

  assert.equal(projections, 1);
  assert.equal(refreshes, 1);
  assert.equal(navigationRefreshes, 1);
  assert.equal(documents.templateRequestSerial, 1);
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
  assert.equal(host.previewSrc, "http://127.0.0.1:41000/");
  assert.equal(host.browserPreviewRoute, "/");
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
