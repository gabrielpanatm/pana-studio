import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { noopAction } from "$lib/editor-runtime/action-outcome";
import { resetProjectWorkspacePreviewCoordinator } from "$lib/kernel/project-workspace-preview-coordinator";
import { createDiskState } from "$lib/session/disk-state";
import { resetPageJsDraftSyncState } from "$lib/session/page-js-draft-sync";
import { saveSessionDrafts } from "$lib/state/save-controller";

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

test("Save rămâne saved când Source Graph și Preview 404 eșuează după receipt-ul atomic", async () => {
  const statuses = [];
  let saveCalls = 0;
  let previewCalls = 0;
  const savedWorkspace = workspace(false);
  mockIPC((command) => {
    if (command === "read_project_workspace_state") {
      return saveCalls === 0 ? workspace(true) : savedWorkspace;
    }
    if (command === "save_project_workspace") {
      saveCalls += 1;
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
    if (command === "project_project_workspace_preview") {
      previewCalls += 1;
      throw new Error("HTTP/1.1 404 Not Found");
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });
  const host = {
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
    scssVariables: [],
    refreshToken: 0,
    jsRefreshToken: 0,
    previewWorkspaceRevision: null,
    pendingCanvasProjection: null,
    canProjectWorkspacePreview: () => true,
    setGlobalStatus(text, kind) {
      statuses.push({ text, kind });
    },
    setInspectorPending(area, pending) {
      this.inspectorPending[area] = pending;
    },
    applyTagChange: async () => noopAction(),
    applyClassesToHtml: async () => noopAction(),
    applyAttributesToHtml: async () => noopAction(),
    applyImageSourceToHtml: async () => noopAction(),
    applyTextContentToHtml: async () => noopAction(),
    async reconcileWorkspaceDerivedState() {
      throw new Error("Source Graph indisponibil");
    },
    requestPreviewRefresh: async () => true,
    acceptProjectWorkspaceSaveBaseline() {},
  };

  assert.equal(await saveSessionDrafts(host), true);
  assert.equal(saveCalls, 1);
  assert.equal(previewCalls, 1);
  assert.equal(host.projectWorkspaceSnapshot, savedWorkspace);
  assert.equal(statuses.at(-1)?.kind, "saved");
  assert.match(statuses.at(-1)?.text ?? "", /Atomically saved/);
  assert.match(statuses.at(-1)?.text ?? "", /interface must resynchronize/i);
  assert.equal(statuses.some(({ kind }) => kind === "error"), false);
});
