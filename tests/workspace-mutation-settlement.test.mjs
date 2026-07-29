import assert from "node:assert/strict";
import { afterEach, beforeEach, test } from "node:test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { resetProjectWorkspacePreviewCoordinator } from "$lib/kernel/project-workspace-preview-coordinator";
import {
  fileBufferDraftSyncSnapshot,
  hashFileBufferText,
  resetFileBufferDraftSyncState,
  setFileBufferDraftSyncSession,
} from "$lib/session/file-buffer-draft-sync";
import {
  settleProjectWorkspaceMutation,
  workspaceMutationAuthorityReceipt,
} from "$lib/session/workspace-mutation-coordinator";

if (!globalThis.window) globalThis.window = globalThis;

beforeEach(() => {
  setFileBufferDraftSyncSession("/project", "session:runtime");
});

afterEach(() => {
  clearMocks();
  resetProjectWorkspacePreviewCoordinator();
  resetFileBufferDraftSyncState();
});

function snapshot(revision = 2, dirty = true) {
  return {
    schemaVersion: 3,
    projectRoot: "/project",
    runtimeSessionId: "session:runtime",
    revision,
    dirty,
  };
}

function mutation(changed = true) {
  const relativePath = "templates/index.html";
  const text = "<main>Proiecție Rust</main>";
  return {
    schemaVersion: 3,
    changed,
    revisionBefore: changed ? 1 : 2,
    revisionAfter: 2,
    dirty: changed,
    transactionId: changed ? "tx-2" : null,
    touchedFiles: changed ? [relativePath] : [],
    documents: changed
      ? [{
          relativePath,
          snapshot: {
            relativePath,
            text,
            dirty: true,
            hash: hashFileBufferText(text),
            bytes: new TextEncoder().encode(text).byteLength,
            revision: 2,
          },
        }]
      : [],
    entry: changed
      ? {
          transactionId: "tx-2",
          label: "Test",
          source: "test",
          coalesceKey: null,
          createdAtMs: 1,
          updatedAtMs: 1,
          mutationCount: 1,
          documentPaths: ["templates/index.html"],
          topologyPaths: [],
          pageJsPaths: [],
          retainedBytes: 1,
        }
      : null,
  };
}

function host(overrides = {}) {
  return {
    sessionProjectRoot: "/project",
    kernelProjectSessionId: "session:runtime",
    projectWorkspaceSnapshot: snapshot(1),
    activeScannedPath: "content/_index.md",
    source: "+++\\n+++\\n",
    sourceCache: {},
    scannedProject: {},
    previewWorkspaceRevision: null,
    pendingCanvasProjection: null,
    canProjectWorkspacePreview: () => true,
    requestPreviewRefresh: async () => true,
    reconcileWorkspaceDerivedState: async ({ expectedWorkspaceRevision }) => ({
      workspaceRevision: expectedWorkspaceRevision,
      topology: "current",
      sourceGraph: "current",
      scss: "current",
      warnings: [],
    }),
    ...overrides,
  };
}

test("proiecția text Rust înlocuiește cache-ul gol al unui fișier creat înainte de deschiderea CodeMirror", async () => {
  const relativePath = "templates/index.html";
  const target = host({
    activeScannedPath: relativePath,
    source: "",
    sourceCache: { [`scanned:${relativePath}`]: "" },
  });

  const settlement = await settleProjectWorkspaceMutation(
    target,
    workspaceMutationAuthorityReceipt(mutation(), snapshot(2)),
    { projectPreview: false },
  );

  assert.equal(settlement.authority, "committed");
  assert.equal(target.source, "<main>Proiecție Rust</main>");
  assert.equal(
    target.sourceCache[`scanned:${relativePath}`],
    "<main>Proiecție Rust</main>",
  );
  assert.equal(fileBufferDraftSyncSnapshot().cursorCount, 1);
});

test("un commit Rust rămâne reușit când toate proiecțiile derivate eșuează", async () => {
  const target = host({
    async reconcileWorkspaceDerivedState() {
      throw new Error("Source Graph indisponibil");
    },
  });
  let previewAttempts = 0;
  mockIPC((command) => {
    if (command === "read_project_workspace_state") return snapshot(2);
    if (command === "project_project_workspace_preview") {
      previewAttempts += 1;
      throw new Error("HTTP/1.1 404 Not Found");
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  const settlement = await settleProjectWorkspaceMutation(
    target,
    workspaceMutationAuthorityReceipt(mutation(), snapshot(2)),
    { warningLabel: "Testul" },
  );

  assert.equal(settlement.authority, "committed");
  assert.equal(settlement.workspaceRevision, 2);
  assert.equal(target.projectWorkspaceSnapshot.revision, 2);
  assert.equal(settlement.projections.topology, "degraded");
  assert.equal(settlement.projections.sourceGraph, "degraded");
  assert.equal(settlement.projections.scss, "degraded");
  assert.equal(settlement.projections.preview, "degraded");
  assert.equal(previewAttempts, 1);
  assert.equal(settlement.warnings.length, 2);
});

test("lipsa Canvas-ului amână numai Preview-ul", async () => {
  const target = host({ canProjectWorkspacePreview: () => false });
  let ipcCalls = 0;
  mockIPC(() => {
    ipcCalls += 1;
    throw new Error("Nu trebuie apelat IPC");
  });

  const settlement = await settleProjectWorkspaceMutation(
    target,
    workspaceMutationAuthorityReceipt(mutation(), snapshot(2)),
  );

  assert.equal(settlement.authority, "committed");
  assert.equal(settlement.projections.preview, "deferred");
  assert.deepEqual(settlement.projections.previewOutcome, {
    status: "deferred",
    workspaceRevision: 2,
  });
  assert.equal(ipcCalls, 0);
});

test("no-op nu execută reconciliere sau Preview", async () => {
  let reconciliations = 0;
  const target = host({
    async reconcileWorkspaceDerivedState() {
      reconciliations += 1;
      throw new Error("Nu trebuie apelat");
    },
  });

  const settlement = await settleProjectWorkspaceMutation(
    target,
    workspaceMutationAuthorityReceipt(mutation(false), snapshot(2, false)),
  );

  assert.equal(settlement.authority, "noop");
  assert.equal(reconciliations, 0);
  assert.equal(settlement.projections.previewOutcome.status, "already_current");
});

test("un receipt din sesiunea înlocuită este superseded și nu publică snapshot-ul", async () => {
  const originalSnapshot = snapshot(7);
  const target = host({
    kernelProjectSessionId: "session:new",
    projectWorkspaceSnapshot: originalSnapshot,
  });

  const settlement = await settleProjectWorkspaceMutation(
    target,
    workspaceMutationAuthorityReceipt(mutation(), snapshot(2)),
  );

  assert.equal(settlement.authority, "committed");
  assert.equal(settlement.projections.preview, "superseded");
  assert.equal(target.projectWorkspaceSnapshot, originalSnapshot);
});
