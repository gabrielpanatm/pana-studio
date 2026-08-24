import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { noopAction } from "$lib/editor-runtime/action-outcome";
import { ProjectSaveService } from "$lib/project/save-service";
import { createDiskState } from "$lib/session/disk-state";

if (!globalThis.window) globalThis.window = globalThis;

afterEach(() => clearMocks());

function workspace() {
  return {
    projectRoot: "/project",
    runtimeSessionId: "session:runtime",
    revision: 4,
    diskGeneration: 2,
    dirty: false,
    createdDocumentCount: 0,
    deletedDocumentCount: 0,
  };
}

function fixture(overrides = {}) {
  const statuses = [];
  const resolvedStatuses = [];
  let suspendCalls = 0;
  let resumeCalls = 0;
  let releaseSuspend = () => {};
  const suspendGate = new Promise((resolve) => { releaseSuspend = resolve; });
  const project = {
    root: "/project",
    runtimeSessionId: "session:runtime",
    editorMutationEpoch: 0,
    workspace: workspace(),
    saveRequest: 0,
    refreshToken: 0,
    jsRefreshToken: 0,
  };
  const html = {
    inspectorPending: { html: false, css: false, js: false },
    htmlPending: {
      tag: false,
      attributes: false,
      text: false,
      image: false,
      classes: false,
      structure: false,
    },
    pendingTag: null,
    setInspectorPending(area, pending) { this.inspectorPending[area] = pending; },
  };
  const externalDisk = {
    snapshot: {
      checking: false,
      reconciling: false,
      changed: false,
      blockedByDirtySession: false,
      workspaceProjectionRecoveryRequired: false,
    },
    async suspendAndDrain() { suspendCalls += 1; await suspendGate; },
    resumeAfterSave() { resumeCalls += 1; },
    acceptSaveBaseline() {},
  };
  const dependencies = {
    project,
    documents: { activeScannedPath: "templates/index.html" },
    disk: { snapshot: createDiskState() },
    externalDisk,
    transition: { isActive: false },
    history: { quiesceActive: false, leaseActive: false },
    ai: { frontendLockActive: false },
    html,
    editor: {
      htmlDraft: {
        applyAttributes: async () => noopAction(),
        applyText: async () => noopAction(),
      },
    },
    status: {
      set(text, kind) { statuses.push({ text, kind }); },
      resolve(key) { resolvedStatuses.push(key); },
    },
    commands: {
      applyTagChange: async () => noopAction(),
      applyClasses: async () => noopAction(),
      applyImageSource: async () => noopAction(),
      reconcileWorkspaceDerivedState: async () => ({ warnings: [] }),
      projectLatestPreview: async () => ({ status: "projected" }),
      markPreviewSavedToDisk() {},
      scheduleZolaValidation() {},
    },
    ...overrides,
  };
  return {
    service: new ProjectSaveService(dependencies),
    dependencies,
    statuses,
    resolvedStatuses,
    releaseSuspend,
    suspendCalls: () => suspendCalls,
    resumeCalls: () => resumeCalls,
  };
}

test("Save suprapus este serializat, iar drain așteaptă aceeași operație", async () => {
  let reads = 0;
  mockIPC((command) => {
    if (command === "read_project_workspace_state") {
      reads += 1;
      return workspace();
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });
  const state = fixture();
  const first = state.service.saveActiveFile();
  const second = state.service.saveActiveFile();
  const drained = state.service.drain();
  await Promise.resolve();
  assert.equal(state.suspendCalls(), 1);
  state.releaseSuspend();
  assert.deepEqual(await Promise.all([first, second]), [false, false]);
  await drained;
  assert.equal(reads, 1);
  assert.equal(state.resumeCalls(), 1);
  assert.equal(state.resolvedStatuses.length, 1);
});

test("lease-ul de tranziție blochează Save înainte de disk-watch", async () => {
  const state = fixture({ transition: { isActive: true } });
  assert.equal(await state.service.saveActiveFile(), false);
  assert.equal(state.suspendCalls(), 0);
  assert.equal(state.statuses.at(-1)?.kind, "error");
});
