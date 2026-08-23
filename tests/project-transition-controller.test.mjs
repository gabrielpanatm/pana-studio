import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  cancelProjectOpenRecoveryDecision,
  closeCurrentProject,
  discardSessionAndReloadFromDisk,
  openProjectRoot,
  reattachCurrentProjectSession,
} from "$lib/state/project-transition-controller";
import {
  ProjectTransitionFrontendLeaseBusyError,
  requireCurrentProjectTransitionFrontendLease,
  runWithProjectTransitionFrontendLease,
} from "$lib/state/project-transition-frontend-lease";
import {
  publishProjectSessionIntoFrontend,
} from "$lib/state/project-attachment-controller";
import {
  refreshSourceGraphAfterCommit,
  startPreviewAfterOpen,
} from "$lib/state/project-preview-bootstrap-controller";
import { resetProjectScopedState } from "$lib/state/project-session-reset";
import { resetFileBufferDraftSyncState } from "$lib/session/file-buffer-draft-sync";
import { resetPageJsDraftSyncState } from "$lib/session/page-js-draft-sync";

if (!globalThis.window) globalThis.window = globalThis;

function deferred() {
  let resolve;
  const promise = new Promise((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

function lifecycle({
  runtimeSessionId = "runtime-a",
  operationId = null,
  readiness = "initializing_frontend",
  revision = 1,
  active = true,
} = {}) {
  return {
    schemaVersion: 1,
    revision,
    activeSession: active ? {
      projectRoot: "/project-a",
      runtimeSessionId,
      readiness: readiness === "degraded"
        ? { state: "degraded", capability: "preview", diagnostic: "preview unavailable" }
        : { state: readiness },
      committedAtMs: 1,
      readinessChangedAtMs: 1,
    } : null,
    transition: operationId ? "preparing" : "idle",
    operationId,
    transitionStartedAtMs: operationId ? 1 : null,
    reason: "test",
  };
}

function project(runtimeSessionId = "runtime-a", previewWarning = "preview unavailable") {
  return {
    root: "/project-a",
    previewBaseUrl: null,
    previewWarning,
    activeTheme: null,
    files: [],
    kernelSessionId: runtimeSessionId,
    workspaceRevision: 1,
    acceptedDiskGeneration: 1,
    acceptedDiskManifest: {
      root: "/project-a",
      files: [],
      truncated: false,
      maxFiles: 100,
    },
  };
}

function bootstrap(runtimeSessionId = "runtime-a") {
  const documents = {
    schemaVersion: 1,
    sessionId: "session-a",
    runtimeSessionId,
    projectRoot: "/project-a",
    loadedAtMs: 1,
    fileCount: 0,
    loadedFileCount: 0,
    skippedFileCount: 0,
    dirtyFileCount: 0,
    totalLoadedBytes: 0,
    limits: { maxFiles: 100, maxFileBytes: 1_000_000, maxTotalBytes: 2_000_000 },
    files: [],
    diagnostics: [],
  };
  return {
    schemaVersion: 5,
    project: project(runtimeSessionId),
    lifecycle: lifecycle({ runtimeSessionId }),
    workspace: {
      schemaVersion: 3,
      projectRoot: "/project-a",
      runtimeSessionId,
      revision: 1,
      diskGeneration: 1,
      dirty: false,
      dirtyDocumentCount: 0,
      createdDocumentCount: 0,
      createdDocuments: [],
      deletedDocumentCount: 0,
      deletedDocuments: [],
      stagedBinaryResourceCount: 0,
      stagedBinaryResourceBytes: 0,
      stagedBinaryResources: [],
      deletedBinaryResourceCount: 0,
      deletedBinaryResources: [],
      dirtyPageJsCount: 0,
      projectModelRevision: null,
      projectModelSourceRevision: null,
      lastProjectionTransactionId: null,
      documents,
    },
    projectSettings: { schemaVersion: 1, workspaceRevision: 1, cachebustAssets: false },
    deploySettings: { schemaVersion: 1, revision: 0, activeTargetId: null, targets: [] },
    workbench: {
      schemaVersion: 1,
      projectRoot: "/project-a",
      projectSessionId: "session-a",
      runtimeSessionId,
      revision: 1,
    },
    activeDocument: null,
    targetCssFile: null,
    initialSurface: null,
  };
}

function validCandidate() {
  return {
    root: "/project-a",
    displayName: "Project A",
    kind: "valid_project",
    snapshotToken: "snapshot-a",
    entryCount: 1,
    truncated: false,
    diagnostics: [],
  };
}

function inspection() {
  return {
    schemaVersion: 1,
    operationId: "operation-a",
    operationStartedAtMs: 1,
    candidateToken: "candidate-a",
    recovery: {
      schemaVersion: 1,
      status: "missing",
      projectRoot: "/project-a",
      assessmentToken: null,
      conflictReason: null,
      rootIdentityChanged: null,
      recoveryRevision: null,
      dirtyDocumentCount: 0,
      stagedBinaryResourceCount: 0,
      deletedBinaryResourceCount: 0,
      pageJsDraftCount: 0,
      undoCount: 0,
      redoCount: 0,
      acceptedFileCount: 0,
      currentFileCount: 0,
      diagnostic: null,
    },
    lifecycle: lifecycle({ operationId: "operation-a", active: false }),
  };
}

function host(overrides = {}) {
  const events = [];
  const current = {
    events,
    projectTransitionFrontendLease: null,
    projectTransitionFrontendLeaseGeneration: 0,
    get projectTransitionFrontendLeaseActive() {
      return this.projectTransitionFrontendLease !== null;
    },
    runWithProjectTransitionFrontendLease(request, operation) {
      return runWithProjectTransitionFrontendLease(
        this,
        request,
        { async quiesce() {} },
        operation,
      );
    },
    requireProjectTransitionFrontendLease(lease) {
      requireCurrentProjectTransitionFrontendLease(this, lease);
    },
    attachPublishedProjectSession(project, mode, receipt, lease) {
      return publishProjectSessionIntoFrontend(this, project, mode, receipt, lease);
    },
    startAttachedProjectPreview(attachment) {
      return startPreviewAfterOpen(this, attachment);
    },
    refreshAttachedProjectSourceGraph(attachment) {
      return refreshSourceGraphAfterCommit(this, attachment);
    },
    resetProjectSessionProjection(options) {
      resetProjectScopedState(this, options);
    },
    scannedProject: null,
    projectLifecycle: lifecycle({ active: false }),
    startupFlow: {
      schemaVersion: 1,
      revision: 0,
      stage: "idle",
      candidate: null,
      diagnostics: [],
    },
    projectOpenRecoveryDecisionRequest: null,
    projectTransitionDecisionRequest: null,
    projectStatus: "",
    sessionProjectRoot: "",
    kernelProjectSessionId: "",
    activeScannedPath: null,
    applicationSurface: "startup",
    source: "old",
    sourceCache: { old: "old" },
    sourceGraph: null,
    sourceGraphProjectionStatus: "deferred",
    sourceGraphWorkspaceRevision: null,
    scssVariables: [],
    targetCssFile: "styles.css",
    previewSrc: "about:blank",
    activePreviewPath: "about:blank",
    browserPreviewRoute: "/",
    previewDocumentMarkup: null,
    previewWorkspaceRevision: null,
    pendingCanvasProjection: null,
    activeCanvasIdentity: null,
    activeCanvasUrl: "",
    activeVersionPreview: null,
    templateWorkbenchPlan: null,
    templateWorkbenchPreferredPagePath: null,
    templateWorkbenchPreferredRoute: null,
    templateWorkbenchActive: false,
    templateWorkbenchTarget: null,
    templateWorkbenchReturnPreviewPath: null,
    templateWorkbenchRequestSerial: 0,
    projectWorkspaceMutationEpoch: 0,
    projectSessionEpoch: 0,
    overrideRules: {},
    variableOverrides: {},
    htmlPending: {
      tag: false,
      attributes: false,
      text: false,
      image: false,
      classes: false,
      structure: false,
    },
    inspectorPending: { html: false, css: false, js: false },
    pendingTag: null,
    pendingTagOriginal: null,
    pendingTagSourceLocation: null,
    tagStatus: "",
    projectWorkspaceSnapshot: null,
    workbenchSnapshot: null,
    fileExplorerSnapshot: null,
    fileExplorerLoading: false,
    fileExplorerError: "",
    publishWorkspace: {
      cachebustAssets: false,
      invalidate() {},
    },
    diskState: {
      projectRoot: "",
      revision: 0,
      scannedAt: null,
      fileCount: 0,
      directoryCount: 0,
      lastMutation: null,
    },
    refreshToken: 0,
    editorSelection: {
      selectionSnapshot: null,
      reset() { events.push("editor-reset"); },
      async refreshNavigationSnapshot() {},
    },
    async invalidateExternalReconcileForProjectTransition() {
      events.push("external-invalidate");
    },
    markWorkspaceProjectionRecoveryRequired(message) {
      events.push(["recovery-required", message]);
    },
    resetExternalDiskState() { events.push("external-reset"); },
    async establishExternalDiskBaseline() { events.push("baseline"); },
    startExternalDiskMonitoring() { events.push("monitor-start"); },
    resetControlledPreviewState() { events.push("preview-reset"); },
    resetPageSections() { events.push("sections-reset"); },
    clearPreviewSelection() { events.push("selection-reset"); },
    resetInspectorPendingSources() { events.push("inspector-reset"); },
    cancelPendingHtmlMutations() { events.push("html-cancel"); },
    setSessionProjectRoot(root = "") { this.sessionProjectRoot = root; },
    setGlobalStatus(text, kind) { events.push(["status", text, kind]); },
    escalateGlobalStatus(notification) { events.push(["notification", notification]); },
    clearNotification(id) { events.push(["clear", id]); },
    hydrateWorkbenchBootstrap(snapshot) { events.push(["workbench", snapshot.revision]); },
    async loadScannedProjectFile() {},
    async prepareCanvasProjectionNavigation() {},
    async reconcileTemplateWorkbenchPreviewDocument() { return true; },
    async refreshRenderedPreviewDocument() { return true; },
    async updateTemplateWorkbenchContext() { return null; },
    async refreshSourceGraph() {},
    scheduleZolaValidation() {},
    ...overrides,
  };
  return current;
}

function installBootstrapIpc(nextBootstrap, extra = {}) {
  const calls = [];
  mockIPC((command, payload) => {
    calls.push({ command, payload });
    if (command === "reattach_project_session") return nextBootstrap;
    if (command === "open_project") return nextBootstrap;
    if (command === "acknowledge_project_frontend_hydrated") {
      return lifecycle({ runtimeSessionId: nextBootstrap.project.kernelSessionId, revision: 2 });
    }
    if (command === "report_project_capability_degraded") {
      return lifecycle({
        runtimeSessionId: nextBootstrap.project.kernelSessionId,
        readiness: "degraded",
        revision: 3,
      });
    }
    if (command === "read_project_lifecycle") {
      return lifecycle({
        runtimeSessionId: nextBootstrap.project.kernelSessionId,
        readiness: "degraded",
        revision: 3,
      });
    }
    if (command === "get_scss_variables") {
      return {
        projectRoot: "/project-a",
        runtimeSessionId: nextBootstrap.project.kernelSessionId,
        workspaceRevision: 1,
        payload: [],
      };
    }
    if (command in extra) return extra[command](payload);
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });
  return calls;
}

afterEach(() => {
  clearMocks();
  resetFileBufferDraftSyncState();
  resetPageJsDraftSyncState();
});

test("reattach fără sesiune Rust rămâne un noop terminal", async () => {
  mockIPC((command) => {
    assert.equal(command, "reattach_project_session");
    return null;
  });
  const current = host();

  assert.equal(await reattachCurrentProjectSession(current), false);
  assert.equal(current.scannedProject, null);
  assert.equal(current.projectTransitionFrontendLeaseActive, false);
});

test("reattach raportează și eșecul produs înainte de callback-ul Rust", async () => {
  const current = host({
    async runWithProjectTransitionFrontendLease() {
      throw new Error("frontend quiesce failed");
    },
  });

  await assert.rejects(
    reattachCurrentProjectSession(current),
    /frontend quiesce failed/,
  );

  const notifications = current.events.filter(([kind]) => kind === "notification");
  assert.equal(notifications.length, 1);
  assert.equal(notifications[0][1].id, "project.reattach.error");
  assert.equal(notifications[0][1].level, "error");
  assert.match(notifications[0][1].message, /frontend quiesce failed/);
  assert.match(current.projectStatus, /frontend quiesce failed/);
});

test("reattach publică receipt-ul Rust și degradează numai Preview-ul indisponibil", async () => {
  const receipt = bootstrap();
  const calls = installBootstrapIpc(receipt);
  const current = host();

  assert.equal(await reattachCurrentProjectSession(current), true);
  assert.equal(current.scannedProject.root, "/project-a");
  assert.equal(current.kernelProjectSessionId, "runtime-a");
  assert.equal(current.sessionProjectRoot, "/project-a");
  assert.equal(current.projectWorkspaceSnapshot.revision, 1);
  assert.equal(current.projectLifecycle.activeSession.readiness.state, "degraded");
  assert.equal(current.projectTransitionFrontendLeaseActive, false);
  assert.ok(calls.some(({ command }) => command === "acknowledge_project_frontend_hydrated"));
  assert.ok(calls.some(({ command }) => command === "report_project_capability_degraded"));
});

test("open folosește operationId-ul inspectat și publică o singură sesiune", async () => {
  const receipt = bootstrap();
  const calls = installBootstrapIpc(receipt);
  const current = host();

  await openProjectRoot(current, "/project-a", { inspection: inspection() });

  const openCall = calls.find(({ command }) => command === "open_project");
  assert.equal(openCall.payload.operationId, "operation-a");
  assert.equal(openCall.payload.candidateToken, "candidate-a");
  assert.equal(current.scannedProject.kernelSessionId, "runtime-a");
  assert.equal(current.projectTransitionFrontendLeaseActive, false);
});

test("open și reload suprapuse trec prin controller cu un singur commit Rust", async () => {
  const openStarted = deferred();
  const allowOpen = deferred();
  const calls = [];
  mockIPC(async (command) => {
    calls.push(command);
    if (command === "open_project") {
      openStarted.resolve();
      await allowOpen.promise;
      return bootstrap();
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });
  const current = host({
    scannedProject: project("runtime-old", null),
    sessionProjectRoot: "/project-a",
    kernelProjectSessionId: "runtime-old",
    projectLifecycle: lifecycle({ runtimeSessionId: "runtime-old" }),
    async attachPublishedProjectSession(project) {
      this.scannedProject = project;
      this.kernelProjectSessionId = project.kernelSessionId;
      return {
        projectRoot: project.root,
        runtimeSessionId: project.kernelSessionId,
        workspaceRevision: project.workspaceRevision,
      };
    },
    async startAttachedProjectPreview(attachment) {
      return {
        status: "canonical",
        projectSessionId: attachment.runtimeSessionId,
        message: null,
      };
    },
    async refreshAttachedProjectSourceGraph() {},
  });

  const firstOpen = openProjectRoot(current, "/project-a", {
    inspection: inspection(),
    operatorDecisionId: "decision-a",
  });
  await openStarted.promise;

  await assert.rejects(
    discardSessionAndReloadFromDisk(current, null),
    (error) => {
      assert.ok(error instanceof ProjectTransitionFrontendLeaseBusyError);
      assert.equal(error.active.kind, "open");
      assert.equal(error.requested.kind, "reload");
      return true;
    },
  );
  assert.equal(calls.filter((command) => command === "open_project").length, 1);
  assert.equal(current.projectTransitionFrontendLeaseActive, true);

  allowOpen.resolve();
  await firstOpen;
  assert.equal(current.kernelProjectSessionId, "runtime-a");
  assert.equal(current.projectTransitionFrontendLeaseActive, false);
});

test("un eșec Rust la open păstrează sesiunea nepublicată și eliberează lease-ul", async () => {
  const current = host();
  const calls = [];
  mockIPC((command) => {
    calls.push(command);
    if (command === "open_project") throw new Error("Rust open failed");
    if (command === "read_project_lifecycle") {
      return lifecycle({ active: false, revision: 2 });
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  await assert.rejects(
    openProjectRoot(current, "/project-a", { inspection: inspection() }),
    /Rust open failed/,
  );

  assert.deepEqual(calls, ["open_project", "read_project_lifecycle"]);
  assert.equal(current.scannedProject, null);
  assert.equal(current.kernelProjectSessionId, "");
  assert.equal(current.projectTransitionFrontendLeaseActive, false);
});

test("open marchează recovery când attachment-ul eșuează după commit-ul Rust", async () => {
  const calls = installBootstrapIpc(bootstrap());
  const current = host({
    async attachPublishedProjectSession() {
      throw new Error("frontend attachment failed");
    },
  });

  await assert.rejects(
    openProjectRoot(current, "/project-a", { inspection: inspection() }),
    /frontend attachment failed/,
  );

  assert.equal(calls.filter(({ command }) => command === "open_project").length, 1);
  assert.ok(calls.some(({ command }) => command === "report_project_capability_degraded"));
  assert.ok(current.events.some((event) => (
    Array.isArray(event)
    && event[0] === "recovery-required"
  )));
  assert.equal(current.projectLifecycle.activeSession.readiness.state, "degraded");
  assert.equal(current.scannedProject, null);
  assert.equal(current.projectTransitionFrontendLeaseActive, false);
});

test("open cu recovery incompatibilă așteaptă decizia fără commit Rust", async () => {
  const pendingInspection = inspection();
  pendingInspection.recovery.status = "decision_required";
  pendingInspection.recovery.assessmentToken = "assessment-a";
  const calls = [];
  mockIPC((command) => {
    calls.push(command);
    throw new Error(`Comandă neașteptată: ${command}`);
  });
  const current = host();

  await openProjectRoot(current, "/project-a", { inspection: pendingInspection });

  assert.equal(calls.includes("open_project"), false);
  assert.equal(current.projectOpenRecoveryDecisionRequest.targetRoot, "/project-a");
  assert.equal(current.projectOpenRecoveryDecisionRequest.operationId, "operation-a");
  assert.equal(current.scannedProject, null);
  assert.equal(current.projectTransitionFrontendLeaseActive, false);
});

test("anularea recovery acceptă numai request-ul curent", async () => {
  const current = host({
    projectOpenRecoveryDecisionRequest: {
      id: "request-a",
      operationId: "operation-a",
    },
  });
  let cancellations = 0;
  mockIPC((command, payload) => {
    assert.equal(command, "cancel_project_open");
    assert.equal(payload.operationId, "operation-a");
    cancellations += 1;
    return lifecycle({ active: false, revision: 2 });
  });

  await cancelProjectOpenRecoveryDecision(current, "stale-request");
  assert.equal(cancellations, 0);
  await cancelProjectOpenRecoveryDecision(current, "request-a");
  assert.equal(cancellations, 1);
  assert.equal(current.projectOpenRecoveryDecisionRequest, null);
});

test("reload discard reconstruiește sesiunea prin pipeline-ul unic de attachment", async () => {
  const receipt = bootstrap("runtime-b");
  const nextInspection = inspection();
  const calls = installBootstrapIpc(receipt, {
    read_kernel_project_transition_policy() {
      return {
        decision: "allow",
        reason: "clean",
        title: "Allowed",
        message: "Allowed",
        recommendedAction: "Continue",
        sessionId: "runtime-a",
      };
    },
    inspect_startup_folder() {
      return {
        schemaVersion: 1,
        revision: 2,
        stage: "ready",
        candidate: validCandidate(),
        diagnostics: [],
      };
    },
    inspect_project_open() { return nextInspection; },
  });
  const current = host({
    scannedProject: project("runtime-a", null),
    sessionProjectRoot: "/project-a",
    kernelProjectSessionId: "runtime-a",
    projectLifecycle: lifecycle({ runtimeSessionId: "runtime-a" }),
  });

  const outcome = await discardSessionAndReloadFromDisk(current, null);

  assert.equal(outcome.status, "completed");
  assert.equal(outcome.projectSessionId, "runtime-b");
  assert.equal(outcome.previewStatus, "degraded");
  assert.equal(current.kernelProjectSessionId, "runtime-b");
  assert.equal(current.diskState.lastMutation.kind, "discard");
  assert.equal(calls.filter(({ command }) => command === "open_project").length, 1);
});

test("reload raportează sesiunea Rust nouă fără a publica un attachment parțial", async () => {
  const receipt = bootstrap("runtime-b");
  const calls = installBootstrapIpc(receipt, {
    read_kernel_project_transition_policy() {
      return {
        decision: "allow",
        reason: "clean",
        title: "Allowed",
        message: "Allowed",
        recommendedAction: "Continue",
        sessionId: "runtime-a",
      };
    },
    inspect_startup_folder() {
      return {
        schemaVersion: 1,
        revision: 2,
        stage: "ready",
        candidate: validCandidate(),
        diagnostics: [],
      };
    },
    inspect_project_open() { return inspection(); },
  });
  const current = host({
    scannedProject: project("runtime-a", null),
    sessionProjectRoot: "/project-a",
    kernelProjectSessionId: "runtime-a",
    projectLifecycle: lifecycle({ runtimeSessionId: "runtime-a" }),
    async attachPublishedProjectSession() {
      throw new Error("reload attachment failed");
    },
  });

  const outcome = await discardSessionAndReloadFromDisk(current, null);

  assert.equal(outcome.status, "failed");
  assert.equal(outcome.projectSessionId, "runtime-b");
  assert.match(outcome.message, /reload attachment failed/);
  assert.equal(current.kernelProjectSessionId, "runtime-a");
  assert.equal(current.diskState.lastMutation, null);
  assert.ok(calls.some(({ command }) => command === "report_project_capability_degraded"));
  assert.ok(current.events.some((event) => (
    Array.isArray(event)
    && event[0] === "recovery-required"
  )));
  assert.equal(current.projectLifecycle.activeSession.readiness.state, "degraded");
  assert.equal(current.projectTransitionFrontendLeaseActive, false);
});

test("close resetează o singură dată și elimină complet proiecția frontend", async () => {
  const current = host({
    scannedProject: project("runtime-a", null),
    sessionProjectRoot: "/project-a",
    kernelProjectSessionId: "runtime-a",
    projectLifecycle: lifecycle({ runtimeSessionId: "runtime-a" }),
  });
  mockIPC((command) => {
    if (command === "read_kernel_project_transition_policy") {
      return {
        decision: "allow",
        reason: "clean",
        title: "Allowed",
        message: "Allowed",
        recommendedAction: "Continue",
        sessionId: "runtime-a",
      };
    }
    if (command === "close_project") return null;
    if (command === "read_project_lifecycle") return lifecycle({ active: false, revision: 2 });
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  assert.equal(await closeCurrentProject(current), true);
  assert.equal(current.scannedProject, null);
  assert.equal(current.kernelProjectSessionId, "");
  assert.equal(current.sessionProjectRoot, "");
  assert.equal(current.previewSrc, "about:blank");
  assert.equal(current.events.filter((event) => event === "html-cancel").length, 1);
  assert.equal(current.events.filter((event) => event === "selection-reset").length, 1);
  assert.equal(current.projectTransitionFrontendLeaseActive, false);
});
