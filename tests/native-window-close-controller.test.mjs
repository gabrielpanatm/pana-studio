import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { handleNativeWindowCloseRequest } from "$lib/state/native-window-close-controller";
import { closeCurrentProject } from "$lib/state/project-controller";
import { resetFileBufferDraftSyncState } from "$lib/session/file-buffer-draft-sync";
import { resetPageJsDraftSyncState } from "$lib/session/page-js-draft-sync";

if (!globalThis.window) globalThis.window = globalThis;

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
    scannedProject: { root: "/project" },
    projectTransitionDecisionRequest: null,
    async closeCurrentProject() {
      throw new Error("draft drain failed");
    },
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
    scannedProject: null,
    projectTransitionDecisionRequest: null,
    async closeCurrentProject(projectRoot) {
      closeRoots.push(projectRoot);
      this.projectTransitionDecisionRequest = {
        continuation: { kind: "close_project" },
      };
    },
    setGlobalStatus() {},
  };

  await handleNativeWindowCloseRequest(app, "/project-a");

  assert.deepEqual(closeRoots, ["/project-a"]);
  assert.equal(app.scannedProject, null);
  assert.equal(app.nativeWindowClosePending, true);
  assert.equal(app.nativeWindowCloseInProgress, false);
  assert.equal(app.projectTransitionDecisionRequest.continuation.kind, "close_project");
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
  const host = {
    scannedProject: null,
    projectTransitionDecisionRequest: null,
    projectStatus: "",
    async beginProjectTransitionFrontendLease() {},
    endProjectTransitionFrontendLease() {},
    setGlobalStatus(text, kind) {
      statuses.push({ text, kind });
    },
    notify(notification) {
      notifications.push(notification);
    },
  };

  const closed = await closeCurrentProject(host, { detachedProjectRoot: "/project-a" });

  assert.equal(closed, false);
  assert.equal(calls.length, 1);
  assert.equal(host.scannedProject, null);
  assert.equal(host.projectTransitionDecisionRequest.targetRoot, "/project-a");
  assert.equal(host.projectTransitionDecisionRequest.action, "close_project");
  assert.equal(host.projectTransitionDecisionRequest.continuation.kind, "close_project");
  assert.equal(statuses.at(-1).kind, "idle");
  assert.equal(notifications.at(-1).level, "warning");
});
