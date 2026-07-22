import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { afterEach, test } from "node:test";
import { fileURLToPath } from "node:url";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { createControlledPreviewState } from "$lib/preview/controlled";
import { PreviewRuntimeTransportError } from "$lib/editor-runtime/preview-runtime";
import {
  confirmMountedCanvasProjection,
  fetchDomTreeFromPreview,
  prepareCanvasProjectionNavigation,
  refreshRenderedPreviewDocument,
  reloadPreview,
} from "$lib/state/preview-controller";
import { requestControlledPreviewRefresh } from "$lib/state/controlled-preview-controller";
import {
  refreshSourceGraph,
  isPreviewControlPlaneMessage,
} from "$lib/state/app-preview-runtime-controller";
import {
  openCurrentProjectInBrowser,
  startPreviewAfterOpen,
} from "$lib/state/project-controller";

if (!globalThis.window) globalThis.window = globalThis;
if (!globalThis.location) globalThis.location = new URL("http://app.local/");

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

async function nextTurn() {
  await new Promise((resolve) => setImmediate(resolve));
}

function previewFile(relativePath = "templates/index.html") {
  return {
    name: relativePath.split("/").at(-1),
    relativePath,
    role: "page",
  };
}

function canvasPhaseReceipts(identity) {
  return [
    {
      schemaVersion: 1,
      identity,
      phase: "resourcesReady",
      phaseTimingsMs: { resourcesReady: 1 },
      diagnostic: null,
    },
    {
      schemaVersion: 1,
      identity,
      phase: "committed",
      phaseTimingsMs: { resourcesReady: 1, committed: 2 },
      diagnostic: null,
    },
    {
      schemaVersion: 1,
      identity,
      phase: "styledReady",
      phaseTimingsMs: { resourcesReady: 1, committed: 2, styledReady: 3 },
      diagnostic: null,
    },
  ];
}

function host(overrides = {}) {
  return {
    sessionProjectRoot: "/project-a",
    kernelProjectSessionId: "session-a:runtime-1",
    projectSessionEpoch: 7,
    previewRefreshSerial: 0,
    previewDomTreeSerial: 0,
    previewFrame: undefined,
    previewSrc: "http://127.0.0.1:1111/old",
    previewReloadSerial: 0,
    previewDiskRevision: null,
    previewWorkspaceRevision: null,
    pendingCanvasProjection: null,
    canvasProjectionConfirmation: null,
    previewSyncTimer: null,
    domTreeFetchTimer: null,
    previewDocumentMarkup: "old markup",
    activeRenderedPreviewPageFile: previewFile(),
    isActiveRenderedPreviewPage: true,
    selectedPreviewElement: null,
    projectStatus: "Project A",
    overrideRules: {},
    variableOverrides: {},
    pageSections: [],
    previewUrlForScannedFile(file) {
      return `http://127.0.0.1:1111/${file.relativePath}`;
    },
    ...overrides,
  };
}

function controlledHost(overrides = {}) {
  return host({
    controlledPreview: createControlledPreviewState(),
    zolaValidationTimer: null,
    zolaValidationSerial: 0,
    scannedProject: { root: "/project-a", isZola: true },
    statuses: [],
    async reloadPreview(lease) {
      return await reloadPreview(this, lease);
    },
    setGlobalStatus(text, kind) {
      this.statuses.push({ text, kind });
    },
    ...overrides,
  });
}

afterEach(() => {
  clearMocks();
});

test("lock-ul de editare păstrează ACK-urile Canvas și refuză mesajele de intenție", () => {
  assert.equal(isPreviewControlPlaneMessage({ source: "pana-studio-preview", type: "ready" }), true);
  assert.equal(isPreviewControlPlaneMessage({ source: "pana-studio-preview", type: "preview-operation-complete" }), true);
  assert.equal(isPreviewControlPlaneMessage({ source: "pana-studio-preview", type: "selection" }), false);
  assert.equal(isPreviewControlPlaneMessage({ source: "pana-studio-preview", type: "preview-shortcut" }), false);
  assert.equal(isPreviewControlPlaneMessage({ source: "foreign", type: "ready" }), false);
});

test("bootstrap Canvas promovează numai identitatea completă a documentului montat", async () => {
  const identity = {
    projectRoot: "/project-a",
    runtimeSessionId: "session-a:runtime-1",
    workspaceRevision: 8,
    transactionId: "canvas-workspace-8",
    previewRevision: "workspace-8",
  };
  const plan = {
    schemaVersion: 1,
    identity,
    workspaceTransactionId: "workspace-edit-8",
    phase: "prepared",
    impact: { kinds: ["htmlStructure"], paths: ["templates/index.html"], requiresFullDocument: false },
    resources: { schemaVersion: 1, previewRevision: "workspace-8", totalBytes: 0, entries: [] },
  };
  mockIPC(async (command, payload) => {
    assert.equal(command, "acknowledge_canvas_projection_phase");
    assert.deepEqual(payload.input.identity, identity);
    return {
      ...plan,
      phase: payload.input.phase === "styledReady"
        ? "canonicalVerified"
        : payload.input.phase,
    };
  });
  const activeHost = host();
  const mounted = prepareCanvasProjectionNavigation(activeHost, plan);

  assert.equal(await confirmMountedCanvasProjection(
    activeHost,
    { ...identity, transactionId: "stale-transaction" },
    canvasPhaseReceipts({ ...identity, transactionId: "stale-transaction" }),
  ), false);
  assert.equal(activeHost.pendingCanvasProjection, plan);

  assert.equal(await confirmMountedCanvasProjection(
    activeHost,
    identity,
    canvasPhaseReceipts(identity),
  ), true);
  await mounted;
  assert.equal(activeHost.pendingCanvasProjection, null);
  assert.equal(activeHost.canvasProjectionConfirmation, null);
});

test("browser preview nu deschide URL-ul vechi după redeschiderea aceluiași root", async () => {
  const startGate = deferred();
  let capturedIdentity = null;
  let openedUrl = null;
  const activeHost = {
    scannedProject: { root: "/project-a", isZola: true, acceptedDiskGeneration: 4 },
    sessionProjectRoot: "/project-a",
    kernelProjectSessionId: "session-a:runtime-1",
    currentStatus: "",
    notifications: [],
    setGlobalStatus(text, kind) {
      this.currentStatus = `${kind}:${text}`;
    },
    clearNotification() {
      throw new Error("continuarea stale nu poate curăța notificări");
    },
    notify(notification) {
      this.notifications.push(notification);
    },
  };

  const opening = openCurrentProjectInBrowser(activeHost, {
    start(identity) {
      capturedIdentity = identity;
      return startGate.promise;
    },
    async openUrl(url) {
      openedUrl = url;
    },
  });
  await nextTurn();
  assert.deepEqual(capturedIdentity, {
    expectedProjectRoot: "/project-a",
    expectedSessionId: "session-a:runtime-1",
    expectedDiskGeneration: 4,
  });

  activeHost.kernelProjectSessionId = "session-a:runtime-2";
  activeHost.currentStatus = "restored:Runtime 2 activ";
  startGate.resolve({
    url: "http://127.0.0.1:43101",
    projectRoot: "/project-a",
    runtimeSessionId: "session-a:runtime-1",
    acceptedDiskGeneration: 4,
  });

  await opening;
  assert.equal(openedUrl, null);
  assert.equal(activeHost.currentStatus, "restored:Runtime 2 activ");
  assert.deepEqual(activeHost.notifications, []);
});

test("browser preview deschide numai receipt-ul legat de runtime-ul capturat", async () => {
  const opened = [];
  const statuses = [];
  const activeHost = {
    scannedProject: { root: "/project-a", isZola: true, acceptedDiskGeneration: 5 },
    sessionProjectRoot: "/project-a",
    kernelProjectSessionId: "session-a:runtime-1",
    setGlobalStatus(text, kind) {
      statuses.push({ text, kind });
    },
    clearNotification(id) {
      assert.equal(id, "project.browser-preview.warning");
    },
    notify() {
      throw new Error("un receipt valid nu trebuie să emită warning");
    },
  };

  await openCurrentProjectInBrowser(activeHost, {
    async start(identity) {
      return {
        url: "http://127.0.0.1:43102",
        projectRoot: identity.expectedProjectRoot,
        runtimeSessionId: identity.expectedSessionId,
        acceptedDiskGeneration: identity.expectedDiskGeneration,
      };
    },
    async openUrl(url) {
      opened.push(url);
    },
  });

  assert.deepEqual(opened, ["http://127.0.0.1:43102"]);
  assert.deepEqual(statuses.at(-1), {
    text: "Source Browser deschis din generația salvată: http://127.0.0.1:43102",
    kind: "restored",
  });
});

test("browser preview compune pagina activă peste originea Source Browser", async () => {
  const opened = [];
  const statuses = [];
  const activeHost = {
    scannedProject: { root: "/project-a", isZola: true, acceptedDiskGeneration: 6 },
    sessionProjectRoot: "/project-a",
    kernelProjectSessionId: "session-a:runtime-1",
    setGlobalStatus(text, kind) {
      statuses.push({ text, kind });
    },
    clearNotification() {},
    notify() {
      throw new Error("ruta activă validă nu trebuie să emită warning");
    },
  };

  await openCurrentProjectInBrowser(activeHost, {
    async start(identity) {
      return {
        url: "http://127.0.0.1:43104",
        projectRoot: identity.expectedProjectRoot,
        runtimeSessionId: identity.expectedSessionId,
        acceptedDiskGeneration: identity.expectedDiskGeneration,
      };
    },
    async openUrl(url) {
      opened.push(url);
    },
  }, { route: "/servicii/" });

  assert.deepEqual(opened, ["http://127.0.0.1:43104/servicii/"]);
  assert.deepEqual(statuses.at(-1), {
    text: "Source Browser deschis din generația salvată: http://127.0.0.1:43104/servicii/",
    kind: "restored",
  });
});

test("browser preview nu publică URL-ul unei generații AcceptedDisk depășite", async () => {
  const startGate = deferred();
  let openedUrl = null;
  const activeHost = {
    scannedProject: { root: "/project-a", isZola: true, acceptedDiskGeneration: 8 },
    sessionProjectRoot: "/project-a",
    kernelProjectSessionId: "session-a:runtime-1",
    currentStatus: "",
    notifications: [],
    setGlobalStatus(text, kind) {
      this.currentStatus = `${kind}:${text}`;
    },
    clearNotification() {
      throw new Error("continuarea stale nu poate curăța notificări");
    },
    notify(notification) {
      this.notifications.push(notification);
    },
  };

  const opening = openCurrentProjectInBrowser(activeHost, {
    start() {
      return startGate.promise;
    },
    async openUrl(url) {
      openedUrl = url;
    },
  });
  await nextTurn();

  activeHost.scannedProject = {
    ...activeHost.scannedProject,
    acceptedDiskGeneration: 9,
  };
  activeHost.currentStatus = "saved:generația 9 activă";
  startGate.resolve({
    url: "http://127.0.0.1:43103",
    projectRoot: "/project-a",
    runtimeSessionId: "session-a:runtime-1",
    acceptedDiskGeneration: 8,
  });

  await opening;
  assert.equal(openedUrl, null);
  assert.equal(activeHost.currentStatus, "saved:generația 9 activă");
  assert.deepEqual(activeHost.notifications, []);
});

test("preview embedded întârziat nu se publică după redeschiderea aceluiași root", async () => {
  const startGate = deferred();
  let capturedIdentity = null;
  let loadCount = 0;
  const activeHost = {
    scannedProject: {
      root: "/project-a",
      isZola: true,
      files: [{ relativePath: "templates/index.html" }],
      previewBaseUrl: null,
      previewWarning: null,
    },
    sessionProjectRoot: "/project-a",
    kernelProjectSessionId: "session-a:runtime-1",
    projectTransitionFrontendLeaseActive: false,
    activeScannedPath: "templates/index.html",
    async loadScannedProjectFile() {
      loadCount += 1;
    },
    clearNotification() {
      throw new Error("continuarea stale nu poate curăța notificări");
    },
    setGlobalStatus() {
      throw new Error("continuarea stale nu poate publica status");
    },
    scheduleZolaValidation() {
      throw new Error("continuarea stale nu poate porni validarea");
    },
    notify() {
      throw new Error("continuarea stale nu poate publica warning");
    },
  };
  const identity = {
    expectedProjectRoot: "/project-a",
    expectedSessionId: "session-a:runtime-1",
  };

  const starting = startPreviewAfterOpen(activeHost, identity, {
    start(requestIdentity) {
      capturedIdentity = requestIdentity;
      return startGate.promise;
    },
  });
  await nextTurn();
  assert.deepEqual(capturedIdentity, identity);

  activeHost.kernelProjectSessionId = "session-a:runtime-2";
  activeHost.scannedProject = {
    ...activeHost.scannedProject,
    previewBaseUrl: "http://127.0.0.1:43200",
  };
  startGate.resolve({
    url: "http://127.0.0.1:43199",
    projectRoot: "/project-a",
    runtimeSessionId: "session-a:runtime-1",
  });

  await starting;
  assert.equal(activeHost.scannedProject.previewBaseUrl, "http://127.0.0.1:43200");
  assert.equal(loadCount, 0);
});

test("Project Transition invalidează preview-ul embedded înainte de schimbarea proiecției UI", async () => {
  const startGate = deferred();
  let loadCount = 0;
  const activeHost = {
    scannedProject: {
      root: "/project-a",
      isZola: true,
      files: [{ relativePath: "templates/index.html" }],
      previewBaseUrl: null,
      previewWarning: null,
    },
    sessionProjectRoot: "/project-a",
    kernelProjectSessionId: "session-a:runtime-1",
    projectTransitionFrontendLeaseActive: false,
    activeScannedPath: "templates/index.html",
    async loadScannedProjectFile() {
      loadCount += 1;
    },
    clearNotification() {},
    setGlobalStatus() {},
    scheduleZolaValidation() {},
    notify() {},
  };
  const identity = {
    expectedProjectRoot: "/project-a",
    expectedSessionId: "session-a:runtime-1",
  };
  const starting = startPreviewAfterOpen(activeHost, identity, {
    start() {
      return startGate.promise;
    },
  });
  await nextTurn();
  activeHost.projectTransitionFrontendLeaseActive = true;
  startGate.resolve({
    url: "http://127.0.0.1:43201",
    projectRoot: "/project-a",
    runtimeSessionId: "session-a:runtime-1",
  });

  await starting;
  assert.equal(activeHost.scannedProject.previewBaseUrl, null);
  assert.equal(loadCount, 0);
});

test("restaurarea unui template așteaptă publicarea Canvas principal înainte de Workbench", async () => {
  const mainCanvasGate = deferred();
  const events = [];
  const template = {
    name: "index.html",
    relativePath: "templates/index.html",
    role: "template",
  };
  const page = {
    name: "_index.md",
    relativePath: "content/_index.md",
    role: "page",
    previewPath: "/",
  };
  const canvasIdentity = {
    projectRoot: "/project-a",
    runtimeSessionId: "session-a:runtime-1",
    workspaceRevision: 4,
    transactionId: "canvas-main-4",
    previewRevision: "preview-main-4",
  };
  const canvasProjection = {
    schemaVersion: 1,
    identity: canvasIdentity,
    workspaceTransactionId: null,
    phase: "prepared",
    impact: { kinds: ["fullDocument"], paths: [], requiresFullDocument: true },
    resources: { schemaVersion: 1, previewRevision: "preview-main-4", totalBytes: 0, entries: [] },
  };
  const activeHost = {
    scannedProject: {
      root: "/project-a",
      isZola: true,
      files: [template, page],
      previewBaseUrl: null,
      previewWarning: null,
    },
    sessionProjectRoot: "/project-a",
    kernelProjectSessionId: "session-a:runtime-1",
    projectTransitionFrontendLeaseActive: false,
    activeScannedPath: template.relativePath,
    activePreviewPath: "about:blank",
    previewSrc: "about:blank",
    previewDocumentMarkup: null,
    pendingCanvasProjection: null,
    previewWorkspaceRevision: null,
    activeCanvasIdentity: null,
    activeCanvasUrl: "",
    prepareCanvasProjectionNavigation(plan) {
      events.push("main-prepared");
      this.pendingCanvasProjection = plan;
      return mainCanvasGate.promise.then(() => {
        this.pendingCanvasProjection = null;
        this.activeCanvasIdentity = plan.identity;
        events.push("main-canonical");
      });
    },
    previewUrlForScannedFile(file) {
      return `http://127.0.0.1:43220${file.previewPath ?? "/"}?__pana_preview_revision=preview-main-4`;
    },
    async loadScannedProjectFile(file, options) {
      events.push(`load:${file.role}`);
      assert.equal(events.includes("main-canonical"), true);
      assert.equal(file.relativePath, template.relativePath);
      assert.equal(options.activateTemplateWorkbench, true);
      assert.equal(options.strict, true);
    },
    clearNotification() {},
    setGlobalStatus() {},
    scheduleZolaValidation() {},
    notify() {},
  };

  const starting = startPreviewAfterOpen(activeHost, {
    expectedProjectRoot: "/project-a",
    expectedSessionId: "session-a:runtime-1",
  }, {
    async start() {
      return {
        url: "http://127.0.0.1:43220",
        projectRoot: "/project-a",
        runtimeSessionId: "session-a:runtime-1",
        workspaceRevision: 4,
        previewRevision: "preview-main-4",
        canvasProjection,
      };
    },
  });
  await nextTurn();
  assert.deepEqual(events, ["main-prepared"]);

  mainCanvasGate.resolve();
  const outcome = await starting;
  assert.equal(outcome.status, "canonical");
  assert.deepEqual(events, ["main-prepared", "main-canonical", "load:template"]);
});

test("un server Preview pornit rămâne reatașabil după un eșec frontend de montare", async () => {
  const canvasIdentity = {
    projectRoot: "/project-a",
    runtimeSessionId: "session-a:runtime-1",
    workspaceRevision: 3,
    transactionId: "canvas-start-3",
    previewRevision: "workspace-3",
  };
  const canvasProjection = {
    schemaVersion: 1,
    identity: canvasIdentity,
    workspaceTransactionId: null,
    phase: "canonicalVerified",
    impact: { kinds: ["htmlStructure"], paths: ["content/_index.md"], requiresFullDocument: true },
    resources: { schemaVersion: 1, previewRevision: "workspace-3", totalBytes: 0, entries: [] },
  };
  const warnings = [];
  let resetCount = 0;
  const activeHost = {
    scannedProject: {
      root: "/project-a",
      isZola: true,
      files: [{
        name: "_index.md",
        relativePath: "content/_index.md",
        role: "page",
        previewPath: "/",
      }],
      previewBaseUrl: null,
      previewWarning: null,
    },
    sessionProjectRoot: "/project-a",
    kernelProjectSessionId: "session-a:runtime-1",
    projectTransitionFrontendLeaseActive: false,
    activeScannedPath: "content/_index.md",
    pendingCanvasProjection: null,
    previewWorkspaceRevision: null,
    activeCanvasIdentity: null,
    async loadScannedProjectFile() {
      throw new Error("iframe mount failed");
    },
    resetControlledPreviewState() { resetCount += 1; },
    notify(notification) { warnings.push(notification); },
    setGlobalStatus() {},
  };

  await startPreviewAfterOpen(activeHost, {
    expectedProjectRoot: "/project-a",
    expectedSessionId: "session-a:runtime-1",
  }, {
    async start() {
      return {
        url: "http://127.0.0.1:43211",
        projectRoot: "/project-a",
        runtimeSessionId: "session-a:runtime-1",
        workspaceRevision: 3,
        previewRevision: "workspace-3",
        canvasProjection,
      };
    },
  });

  assert.equal(resetCount, 1);
  assert.equal(activeHost.scannedProject.previewBaseUrl, "http://127.0.0.1:43211");
  assert.match(activeHost.scannedProject.previewWarning, /iframe mount failed/);
  assert.equal(warnings.at(-1).id, "project.preview.warning");
});

test("refresh-ul întârziat din proiectul A nu poate proiecta peste proiectul B", async () => {
  const readStarted = deferred();
  const readGate = deferred();
  mockIPC(async (command, payload) => {
    assert.equal(command, "read_preview_document");
    readStarted.resolve();
    return await readGate.promise;
  });

  const activeHost = host();
  const refresh = refreshRenderedPreviewDocument(activeHost);
  await readStarted.promise;

  activeHost.sessionProjectRoot = "/project-b";
  activeHost.kernelProjectSessionId = "session-b:runtime-1";
  activeHost.projectSessionEpoch += 1;
  activeHost.previewSrc = "http://127.0.0.1:2222/project-b";
  activeHost.previewDocumentMarkup = "project B markup";
  activeHost.projectStatus = "Project B activ";
  readGate.resolve("<html><body>Project A</body></html>");

  assert.equal(await refresh, false);
  assert.equal(activeHost.previewSrc, "http://127.0.0.1:2222/project-b");
  assert.equal(activeHost.previewDocumentMarkup, "project B markup");
  assert.equal(activeHost.projectStatus, "Project B activ");
});

test("redeschiderea aceleiași rădăcini invalidează refresh-ul runtime-ului vechi", async () => {
  const readStarted = deferred();
  const readGate = deferred();
  mockIPC(async (command, payload) => {
    assert.equal(command, "read_preview_document");
    readStarted.resolve();
    return await readGate.promise;
  });

  const activeHost = host();
  const refresh = refreshRenderedPreviewDocument(activeHost);
  await readStarted.promise;

  activeHost.kernelProjectSessionId = "session-a:runtime-2";
  activeHost.projectSessionEpoch += 1;
  activeHost.previewSrc = "http://127.0.0.1:1111/runtime-2";
  activeHost.previewDocumentMarkup = "runtime 2 markup";
  activeHost.projectStatus = "Runtime 2 activ";
  readGate.resolve("<html><body>Runtime 1</body></html>");

  assert.equal(await refresh, false);
  assert.equal(activeHost.previewSrc, "http://127.0.0.1:1111/runtime-2");
  assert.equal(activeHost.previewDocumentMarkup, "runtime 2 markup");
  assert.equal(activeHost.projectStatus, "Runtime 2 activ");
});

test("aceeași rută primește documentul Zola canonic fără reload de iframe", async () => {
  const canonicalUrl = "http://127.0.0.1:1111/templates/index.html";
  const sent = [];
  const identity = {
    projectRoot: "/project-a",
    runtimeSessionId: "session-a:runtime-1",
    workspaceRevision: 8,
    transactionId: "canvas-workspace-8",
    previewRevision: "workspace-8",
  };
  const plan = {
    schemaVersion: 1,
    identity,
    workspaceTransactionId: "workspace-edit-8",
    phase: "prepared",
    impact: { kinds: ["htmlStructure"], paths: ["templates/index.html"], requiresFullDocument: false },
    resources: { schemaVersion: 1, previewRevision: "workspace-8", totalBytes: 0, entries: [] },
  };
  mockIPC(async (command, payload) => {
    if (command === "read_preview_document") {
      return '<html data-pana-preview-revision="workspace-8"><body>Titlu canonic</body></html>';
    }
    if (command === "acknowledge_canvas_projection_phase") {
      return {
        ...plan,
        phase: payload.input.phase === "styledReady"
          ? "canonicalVerified"
          : payload.input.phase,
      };
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });
  const activeHost = host({
    previewSrc: canonicalUrl,
    previewUrlForScannedFile() { return canonicalUrl; },
    previewDocumentMarkup: null,
    previewWorkspaceRevision: "workspace-8",
    pendingCanvasProjection: plan,
    previewFrame: {
      contentWindow: {},
      getAttribute(name) {
        if (name === "src") return canonicalUrl;
        return null;
      },
      hasAttribute() { return false; },
    },
    previewRuntime: {
      async sendAndWait(payload) {
        sent.push(payload);
        return {
          revision: 1,
          type: payload.type,
          ok: true,
          canvasIdentity: payload.canvasIdentity,
          canvasPhaseReceipts: canvasPhaseReceipts(payload.canvasIdentity),
        };
      },
    },
  });

  assert.equal(await refreshRenderedPreviewDocument(activeHost), true);
  assert.equal(activeHost.previewSrc, canonicalUrl);
  assert.equal(activeHost.previewWorkspaceRevision, null);
  assert.equal(activeHost.pendingCanvasProjection, null);
  assert.equal(sent.length, 1);
  assert.equal(sent[0].type, "replace-document");
  assert.match(sent[0].html, /Titlu canonic/);
});

test("eșecul reconcilerului pe aceeași rută păstrează ultimul document și nu navighează iframe-ul", async () => {
  const canonicalUrl = "http://127.0.0.1:1111/templates/index.html";
  const identity = {
    projectRoot: "/project-a",
    runtimeSessionId: "session-a:runtime-1",
    workspaceRevision: 9,
    transactionId: "canvas-workspace-9",
    previewRevision: "workspace-9",
  };
  const plan = {
    schemaVersion: 1,
    identity,
    workspaceTransactionId: "workspace-edit-9",
    phase: "prepared",
    impact: { kinds: ["stylesheet"], paths: ["sass/pagini/index.scss"], requiresFullDocument: false },
    resources: { schemaVersion: 1, previewRevision: "workspace-9", totalBytes: 0, entries: [] },
  };
  mockIPC(async (command, payload) => {
    if (command === "read_preview_document") {
      return '<html data-pana-preview-revision="workspace-9"><body>Document candidat</body></html>';
    }
    if (command === "acknowledge_canvas_projection_phase") {
      assert.equal(payload.input.phase, "failed");
      return { ...plan, phase: "failed" };
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });
  const activeHost = host({
    previewSrc: canonicalUrl,
    previewDocumentMarkup: null,
    previewWorkspaceRevision: "workspace-9",
    pendingCanvasProjection: plan,
    previewFrame: {
      contentWindow: {},
      getAttribute(name) {
        if (name === "src") return canonicalUrl;
        return null;
      },
      hasAttribute() { return false; },
    },
    previewRuntime: {
      async sendAndWait(payload) {
        return {
          revision: 1,
          type: payload.type,
          ok: false,
          error: "Resource manager a refuzat stylesheet-ul candidat.",
          canvasIdentity: payload.canvasIdentity,
          canvasPhaseReceipts: [{
            schemaVersion: 1,
            identity: payload.canvasIdentity,
            phase: "failed",
            phaseTimingsMs: { failed: 1 },
            diagnostic: "Resource manager a refuzat stylesheet-ul candidat.",
          }],
        };
      },
    },
  });

  assert.equal(await refreshRenderedPreviewDocument(activeHost), false);
  assert.equal(activeHost.previewSrc, canonicalUrl);
  assert.equal(activeHost.pendingCanvasProjection, null);
  assert.match(activeHost.projectStatus, /\[preview_reconcile_failed\]/);
});

test("un bridge fără ACK recuperează aceeași revizie prin navigarea iframe-ului", async () => {
  const canonicalUrl = "http://127.0.0.1:1111/";
  const identity = {
    projectRoot: "/project-a",
    runtimeSessionId: "session-a:runtime-1",
    workspaceRevision: 10,
    transactionId: "canvas-workspace-10",
    previewRevision: "workspace-10",
  };
  const plan = {
    schemaVersion: 1,
    identity,
    workspaceTransactionId: "workspace-edit-10",
    phase: "prepared",
    impact: { kinds: ["stylesheet"], paths: ["sass/pagini/index.scss"], requiresFullDocument: false },
    resources: { schemaVersion: 1, previewRevision: "workspace-10", totalBytes: 0, entries: [] },
  };
  mockIPC(async (command, payload) => {
    if (command === "read_preview_document") {
      return '<html data-pana-preview-revision="workspace-10"><body>Document candidat</body></html>';
    }
    if (command === "acknowledge_canvas_projection_phase") {
      return {
        ...plan,
        phase: payload.input.phase === "styledReady"
          ? "canonicalVerified"
          : payload.input.phase,
      };
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });
  const sendStarted = deferred();
  const activeHost = host({
    previewSrc: canonicalUrl,
    previewDocumentMarkup: null,
    previewWorkspaceRevision: "workspace-10",
    previewUrlForScannedFile() { return canonicalUrl; },
    pendingCanvasProjection: plan,
    previewFrame: {
      contentWindow: {},
      getAttribute(name) {
        if (name === "src") return canonicalUrl;
        return null;
      },
      hasAttribute() { return false; },
    },
    previewRuntime: {
      async sendAndWait() {
        sendStarted.resolve();
        throw new PreviewRuntimeTransportError("ack_timeout", "bridge timeout");
      },
    },
  });

  const refreshing = refreshRenderedPreviewDocument(activeHost);
  await sendStarted.promise;
  await nextTurn();
  assert.match(activeHost.previewSrc, /__pana_preview_revision=workspace-10/);
  assert.equal(await confirmMountedCanvasProjection(
    activeHost,
    identity,
    canvasPhaseReceipts(identity),
  ), true);
  assert.equal(await refreshing, true);
  assert.equal(activeHost.pendingCanvasProjection, null);
  assert.equal(activeHost.previewWorkspaceRevision, null);
});

test("latest-wins păstrează numai al doilea refresh și starea canonical aferentă", async () => {
  const firstStarted = deferred();
  const secondStarted = deferred();
  const firstGate = deferred();
  const secondGate = deferred();
  let readCount = 0;
  mockIPC(async (command) => {
    assert.equal(command, "read_preview_document");
    readCount += 1;
    if (readCount === 1) {
      firstStarted.resolve();
      return await firstGate.promise;
    }
    if (readCount === 2) {
      secondStarted.resolve();
      return await secondGate.promise;
    }
    throw new Error(`Read preview neașteptat: ${readCount}`);
  });

  const activeHost = controlledHost();
  const first = requestControlledPreviewRefresh(activeHost, "manual");
  await firstStarted.promise;
  const second = requestControlledPreviewRefresh(activeHost, "external-change");
  await secondStarted.promise;

  secondGate.resolve("<html><body>Al doilea refresh</body></html>");
  assert.equal(await second, true);
  const winningSrc = activeHost.previewSrc;
  const winningStatus = activeHost.projectStatus;
  const winningState = activeHost.controlledPreview;
  assert.match(winningSrc, /__pana_reload=2/);
  assert.equal(winningState.freshness, "canonical");
  assert.equal(winningState.refreshReason, "external-change");

  firstGate.resolve("<html><body>Primul refresh întârziat</body></html>");
  assert.equal(await first, false);
  assert.equal(activeHost.previewSrc, winningSrc);
  assert.equal(activeHost.projectStatus, winningStatus);
  assert.equal(activeHost.controlledPreview, winningState);

  if (activeHost.zolaValidationTimer !== null) {
    window.clearTimeout(activeHost.zolaValidationTimer);
    activeHost.zolaValidationTimer = null;
  }
});

test("un refresh de proiecție poate păstra eșecul intern fără notificare globală prematură", async () => {
  const activeHost = controlledHost({
    projectStatus: "Previzualizare anterioară validă",
    async reloadPreview() {
      this.projectStatus = "Randarea previzualizării a eșuat: previzualizarea locală a răspuns cu un statut invalid: HTTP/1.1 404 Not Found";
      return false;
    },
  });

  assert.equal(await requestControlledPreviewRefresh(
    activeHost,
    "workspace-mutation",
    { publishFailure: false },
  ), false);
  assert.deepEqual(activeHost.statuses, []);
  assert.equal(activeHost.controlledPreview.freshness, "error");
  assert.match(activeHost.projectStatus, /404 Not Found/);
});

test("citirea DOM întârziată nu poate repopula Layers după schimbarea proiectului", async () => {
  const readStarted = deferred();
  const readGate = deferred();
  mockIPC(async (command) => {
    assert.equal(command, "read_preview_document");
    readStarted.resolve();
    return await readGate.promise;
  });

  const activeHost = host({
    pageSections: [{ selector: "#project-a", label: "A", tag: "main", depth: 0 }],
  });
  fetchDomTreeFromPreview(activeHost);
  await readStarted.promise;

  activeHost.sessionProjectRoot = "/project-b";
  activeHost.kernelProjectSessionId = "session-b:runtime-1";
  activeHost.projectSessionEpoch += 1;
  activeHost.previewSrc = "http://127.0.0.1:2222/project-b";
  activeHost.pageSections = [{ selector: "#project-b", label: "B", tag: "main", depth: 0 }];
  readGate.resolve("<html><body><main id=\"project-a\">A</main></body></html>");
  await nextTurn();

  assert.deepEqual(activeHost.pageSections, [
    { selector: "#project-b", label: "B", tag: "main", depth: 0 },
  ]);
});

test("Source Graph întârziat nu poate fi publicat într-un runtime redeschis pe aceeași rădăcină", async () => {
  const readStarted = deferred();
  const readGate = deferred();
  mockIPC(async (command, args) => {
    assert.equal(command, "read_source_graph");
    assert.deepEqual(args.identity, {
      expectedProjectRoot: "/project-a",
      expectedSessionId: "session-a:runtime-1",
    });
    readStarted.resolve();
    return await readGate.promise;
  });
  const runtimeTwoGraph = { schemaVersion: 1, nodes: [{ id: "runtime-two" }] };
  const activeHost = {
    scannedProject: { root: "/project-a", isZola: true },
    sessionProjectRoot: "/project-a",
    kernelProjectSessionId: "session-a:runtime-1",
    projectSessionEpoch: 7,
    sourceGraphLoadSerial: 0,
    sourceGraph: runtimeTwoGraph,
    kernelSourceDirtyPaths: [],
    sourceCache: {},
    pageSections: [],
    selectedElement: null,
    hydratePageSections(sections) {
      return sections;
    },
    syncPreviewTeraGateState() {},
  };

  const refresh = refreshSourceGraph(activeHost);
  await readStarted.promise;
  activeHost.kernelProjectSessionId = "session-a:runtime-2";
  activeHost.projectSessionEpoch += 1;
  readGate.resolve({ schemaVersion: 1, nodes: [{ id: "runtime-one" }] });

  assert.equal(await refresh, false);
  assert.equal(activeHost.sourceGraph, runtimeTwoGraph);
});

test("Project Transition și reset invalidează lease-ul înaintea continuărilor asincrone", () => {
  const source = readFileSync(fileURLToPath(new URL(
    "../src/lib/state/app.svelte.ts",
    import.meta.url,
  )), "utf8");

  const transition = source.slice(source.indexOf("async beginProjectTransitionFrontendLease()"));
  const transitionInvalidation = transition.indexOf("invalidatePreviewRefreshLease(");
  const transitionFirstAwait = transition.indexOf("await ");
  assert.ok(transitionInvalidation >= 0, "Project Transition trebuie să invalideze refresh-ul activ");
  assert.ok(
    transitionInvalidation < transitionFirstAwait,
    "invalidarea Preview trebuie să se producă înainte de primul await din tranziție",
  );

  const reset = source.slice(source.indexOf("resetControlledPreviewState()"));
  const resetInvalidation = reset.indexOf("invalidatePreviewRefreshLease(");
  const nextMethod = reset.indexOf("\n  scheduleZolaValidation(");
  assert.ok(resetInvalidation >= 0 && resetInvalidation < nextMethod);
});
