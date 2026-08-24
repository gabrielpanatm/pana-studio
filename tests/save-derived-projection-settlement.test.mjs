import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { noopAction } from "$lib/editor-runtime/action-outcome";
import { resetProjectWorkspacePreviewCoordinator } from "$lib/kernel/project-workspace-preview-coordinator";
import { createDiskState } from "$lib/session/disk-state";
import { resetPageJsDraftSyncState } from "$lib/session/page-js-draft-sync";
import { saveSessionDrafts } from "$lib/state/save-controller";
import { projectWorkspaceDirtyStatusKey } from "$lib/status/global-status";

if (!globalThis.window) globalThis.window = globalThis;

afterEach(() => {
  clearMocks();
  resetPageJsDraftSyncState();
  resetProjectWorkspacePreviewCoordinator();
});

function workspace(dirty) {
  return {
    projectRoot: "/project",
    runtimeSessionId: "session:runtime",
    revision: 4,
    diskGeneration: dirty ? 2 : 3,
    dirty,
    createdDocumentCount: 0,
    deletedDocumentCount: 0,
  };
}

test("Save rezolvă statusul dirty, dar păstrează o editare pornită în timpul scrierii", async () => {
  const statuses = [];
  const resolvedStatuses = [];
  let saveCalls = 0;
  let previewCalls = 0;
  let workspaceDirty = true;
  let injectConcurrentMutation = false;
  const savedWorkspace = workspace(false);
  mockIPC((command) => {
    if (command === "read_project_workspace_state") {
      return workspaceDirty ? workspace(true) : savedWorkspace;
    }
    if (command === "save_project_workspace") {
      saveCalls += 1;
      workspaceDirty = false;
      if (injectConcurrentMutation) state.editorMutationEpoch += 1;
      return {
        schemaVersion: 1,
        transactionId: "save-4",
        status: "saved",
        projectRoot: "/project",
        runtimeSessionId: "session:runtime",
        revisionBefore: 4,
        revisionAfter: 4,
        diskGenerationBefore: 2,
        diskGenerationAfter: 3,
        writtenFiles: ["templates/index.html"],
        removedFiles: [],
        writeReceipts: [],
        acceptedManifest: {
          root: "/project",
          files: [],
          totalBytes: 0,
          scannedAtMs: 1,
          truncated: false,
        },
        workspace: savedWorkspace,
      };
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });
  const state = {
    sessionProjectRoot: "/project",
    kernelProjectSessionId: "session:runtime",
    editorMutationEpoch: 0,
    projectWorkspaceSnapshot: workspace(true),
    saveRequest: 0,
    projectStatus: "",
    scannedProject: { root: "/project" },
    diskState: createDiskState(),
    activeScannedPath: "templates/index.html",
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
    refreshToken: 0,
    jsRefreshToken: 0,
  };
  const host = {
    context: () => ({
      projectRoot: state.sessionProjectRoot,
      runtimeSessionId: state.kernelProjectSessionId,
      editorMutationEpoch: state.editorMutationEpoch,
      workspace: state.projectWorkspaceSnapshot,
      diskState: state.diskState,
      activeScannedPath: state.activeScannedPath,
    }),
    incrementSaveRequest() { state.saveRequest += 1; },
    acceptWorkspace(workspace) { state.projectWorkspaceSnapshot = workspace; },
    markDiskSaved() {},
    bumpRefreshTokens() {
      state.refreshToken += 1;
      state.jsRefreshToken += 1;
    },
    setGlobalStatus(text, kind) {
      statuses.push({ text, kind });
    },
    resolveGlobalStatus(key) {
      resolvedStatuses.push(key);
    },
    html: {
      get inspectorPending() { return state.inspectorPending; },
      get pending() { return state.htmlPending; },
      get pendingTag() { return state.pendingTag; },
      setInspectorPending(area, pending) { state.inspectorPending[area] = pending; },
      applyTagChange: async () => noopAction(),
      applyClasses: async () => noopAction(),
      draft: {
        applyAttributes: async () => noopAction(),
        applyText: async () => noopAction(),
      },
      applyImageSource: async () => noopAction(),
    },
    async reconcileWorkspaceDerivedState() {
      throw new Error("Source Graph indisponibil");
    },
    async projectLatestPreview() {
      previewCalls += 1;
      throw new Error("HTTP/1.1 404 Not Found");
    },
    acceptProjectWorkspaceSaveBaseline() {},
  };

  assert.equal(await saveSessionDrafts(host), true);
  assert.equal(saveCalls, 1);
  assert.equal(previewCalls, 1);
  assert.equal(state.projectWorkspaceSnapshot, savedWorkspace);
  assert.equal(statuses.at(-1)?.kind, "saved");
  assert.match(statuses.at(-1)?.text ?? "", /Atomically saved/);
  assert.match(statuses.at(-1)?.text ?? "", /interface must resynchronize/i);
  assert.equal(statuses.some(({ kind }) => kind === "error"), false);
  assert.deepEqual(resolvedStatuses, [
    projectWorkspaceDirtyStatusKey("/project", "session:runtime"),
  ]);

  // A second edit committed while Rust is saving must keep both the pending
  // projection and its unsaved status alive after the older receipt settles.
  workspaceDirty = true;
  injectConcurrentMutation = true;
  state.projectWorkspaceSnapshot = workspace(true);
  state.inspectorPending.css = true;

  assert.equal(await saveSessionDrafts(host), true);
  assert.equal(saveCalls, 2);
  assert.equal(state.inspectorPending.css, true);
  assert.equal(resolvedStatuses.length, 1);
});
