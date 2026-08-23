import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { handleNativeWindowCloseRequest } from "$lib/state/native-window-close-controller";
import { closeCurrentProject } from "$lib/state/project-transition-controller";
import { resetFileBufferDraftSyncState } from "$lib/session/file-buffer-draft-sync";
import { resetPageJsDraftSyncState } from "$lib/session/page-js-draft-sync";
import {
  requireCurrentProjectTransitionFrontendLease,
  runWithProjectTransitionFrontendLease,
} from "$lib/state/project-transition-frontend-lease";

if (!globalThis.window) globalThis.window = globalThis;

function transitionHost(overrides = {}) {
  return {
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
    ...overrides,
  };
}

function deferred() {
  let resolve;
  const promise = new Promise((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

afterEach(() => {
  clearMocks();
  resetFileBufferDraftSyncState();
  resetPageJsDraftSyncState();
});

test("native close remains pending-safe when the project pre-close drain rejects", async () => {
  const statuses = [];
  const app = {
    nativeWindowClosePending: false,
    nativeWindowCloseInProgress: false,
    projectTransitionFrontendLeaseActive: false,
    scannedProject: { root: "/project" },
    projectTransitionDecisionRequest: null,
    async closeCurrentProject() {
      throw new Error("draft drain failed");
    },
    async waitForProjectTransitionFrontendLeaseIdle() {},
    setGlobalStatus(text, kind) {
      statuses.push({ text, kind });
    },
  };

  await handleNativeWindowCloseRequest(app);
  assert.deepEqual(app.scannedProject, { root: "/project" });
  assert.equal(app.nativeWindowClosePending, false);
  assert.equal(app.nativeWindowCloseInProgress, false);
  assert.match(statuses.at(-1).text, /draft drain failed/);
  assert.equal(statuses.at(-1).kind, "error");
});

test("native close routes a detached Rust session directly to close policy", async () => {
  const closeRoots = [];
  const app = {
    nativeWindowClosePending: false,
    nativeWindowCloseInProgress: false,
    projectTransitionFrontendLeaseActive: false,
    scannedProject: null,
    projectTransitionDecisionRequest: null,
    async closeCurrentProject(projectRoot, owner) {
      closeRoots.push({ projectRoot, owner });
      this.projectTransitionDecisionRequest = {
        continuation: { kind: "close_project" },
      };
    },
    async waitForProjectTransitionFrontendLeaseIdle() {},
    setGlobalStatus() {},
  };

  await handleNativeWindowCloseRequest(app, "/project-a");

  assert.deepEqual(closeRoots, [{
    projectRoot: "/project-a",
    owner: "native-window-close",
  }]);
  assert.equal(app.scannedProject, null);
  assert.equal(app.nativeWindowClosePending, true);
  assert.equal(app.nativeWindowCloseInProgress, false);
  assert.equal(app.projectTransitionDecisionRequest.continuation.kind, "close_project");
});

test("native close așteaptă event-driven proprietarul activ și reevaluează înainte de close", async () => {
  const idle = deferred();
  const calls = [];
  const app = {
    nativeWindowClosePending: false,
    nativeWindowCloseInProgress: false,
    projectTransitionFrontendLeaseActive: true,
    projectTransitionFrontendLease: { kind: "open" },
    scannedProject: { root: "/project-a" },
    projectTransitionDecisionRequest: null,
    waitForProjectTransitionFrontendLeaseIdle() {
      return idle.promise;
    },
    async closeCurrentProject(projectRoot, owner) {
      calls.push({ projectRoot, owner });
      this.scannedProject = null;
      return true;
    },
    setGlobalStatus() {},
  };

  const closing = handleNativeWindowCloseRequest(app, "/project-a");
  await Promise.resolve();
  assert.deepEqual(calls, []);
  assert.equal(app.nativeWindowClosePending, true);
  assert.equal(app.nativeWindowCloseInProgress, true);

  app.projectTransitionFrontendLeaseActive = false;
  idle.resolve();
  await closing;

  assert.deepEqual(calls, [{
    projectRoot: null,
    owner: "native-window-close",
  }]);
  assert.equal(app.nativeWindowClosePending, true);
  assert.equal(app.nativeWindowCloseInProgress, false);
});

test("native close nu pornește un al doilea close după ce tranziția activă a închis proiectul", async () => {
  const idle = deferred();
  let closeCalls = 0;
  const app = {
    nativeWindowClosePending: false,
    nativeWindowCloseInProgress: false,
    projectTransitionFrontendLeaseActive: true,
    projectTransitionFrontendLease: { kind: "close" },
    scannedProject: { root: "/project-a" },
    projectTransitionDecisionRequest: null,
    waitForProjectTransitionFrontendLeaseIdle() {
      return idle.promise;
    },
    async closeCurrentProject() {
      closeCalls += 1;
      return true;
    },
    setGlobalStatus() {},
  };

  const closing = handleNativeWindowCloseRequest(app, "/project-a");
  await Promise.resolve();
  app.scannedProject = null;
  app.projectTransitionFrontendLeaseActive = false;
  idle.resolve();
  await closing;

  assert.equal(closeCalls, 0);
  assert.equal(app.nativeWindowClosePending, true);
  assert.equal(app.nativeWindowCloseInProgress, false);
});

test("detached close evaluates kernel policy without rebuilding the frontend project", async () => {
  const calls = [];
  mockIPC((command, payload) => {
    calls.push({ command, payload });
    assert.equal(command, "read_kernel_project_transition_policy");
    assert.equal(payload.action, "close_project");
    return {
      decision: "confirm",
      reason: "undo_redo_dirty",
      title: "Confirmă închiderea",
      message: "Istoricul sesiunii este departe de saved point.",
      recommendedAction: "Confirmă sau anulează.",
      sessionId: "runtime-session-a",
    };
  });
  const statuses = [];
  const notifications = [];
  const host = transitionHost({
    scannedProject: null,
    projectTransitionDecisionRequest: null,
    projectStatus: "",
    setGlobalStatus(text, kind) {
      statuses.push({ text, kind });
    },
    escalateGlobalStatus(notification) {
      notifications.push(notification);
    },
  });

  const closed = await closeCurrentProject(host, { detachedProjectRoot: "/project-a" });

  assert.equal(closed, false);
  assert.equal(calls.length, 1);
  assert.equal(host.scannedProject, null);
  assert.equal(host.projectTransitionDecisionRequest.targetRoot, "/project-a");
  assert.equal(host.projectTransitionDecisionRequest.action, "close_project");
  assert.equal(host.projectTransitionDecisionRequest.continuation.kind, "close_project");
  assert.equal(statuses.length, 0);
  assert.equal(notifications.at(-1).level, "warning");
});
