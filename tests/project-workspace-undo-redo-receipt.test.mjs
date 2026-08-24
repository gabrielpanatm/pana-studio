import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { resolve } from "node:path";
import {
  requireProjectWorkspaceUndoRedoCommandReceipt,
} from "$lib/kernel/project-workspace-undo-redo-receipt";
import {
  projectWorkspaceHistoryChangesTopology,
  reconcileProjectWorkspaceTopologyAfterHistory,
} from "$lib/kernel/project-workspace-history-topology";
import {
  PROJECT_WORKSPACE_SCHEMA_VERSION,
  PROJECT_WORKSPACE_UNDO_REDO_COMMAND_SCHEMA_VERSION,
} from "$lib/project/workspace-contract";
import { WORKBENCH_SCHEMA_VERSION } from "$lib/workbench/contracts";

function receipt(overrides = {}) {
  return {
    schemaVersion: PROJECT_WORKSPACE_UNDO_REDO_COMMAND_SCHEMA_VERSION,
    projectRoot: "/project-a",
    runtimeSessionId: "session-a:runtime-1",
    result: {
      schemaVersion: PROJECT_WORKSPACE_SCHEMA_VERSION,
      direction: "undo",
      revisionBefore: 7,
      revisionAfter: 8,
      dirty: false,
      entry: {
        transactionId: "tx-undo-1",
        documentPaths: [],
        topologyPaths: [],
        pageJsPaths: [],
      },
      documents: [],
      history: {},
      applicationTransactionId: "history-application-8",
    },
    workspace: {
      schemaVersion: PROJECT_WORKSPACE_SCHEMA_VERSION,
      projectRoot: "/project-a",
      runtimeSessionId: "session-a:runtime-1",
      revision: 8,
      lastProjectionTransactionId: "history-application-8",
    },
    workbench: null,
    canvasPatch: null,
    ...overrides,
  };
}

const expected = {
  projectRoot: "/project-a",
  runtimeSessionId: "session-a:runtime-1",
  direction: "undo",
  revisionBefore: 7,
  transactionId: "tx-undo-1",
};

test("receipt-ul comenzii v4 acceptă snapshot-ul ProjectWorkspace curent", () => {
  const value = receipt();
  assert.equal(
    requireProjectWorkspaceUndoRedoCommandReceipt(value, expected),
    value,
  );
});

test("schema comenzii Undo/Redo este validată separat și are diagnostic explicit", () => {
  assert.throws(
    () => requireProjectWorkspaceUndoRedoCommandReceipt(
      receipt({ schemaVersion: 1 }),
      expected,
    ),
    new RegExp(`command schema .*1.*schema .*${PROJECT_WORKSPACE_UNDO_REDO_COMMAND_SCHEMA_VERSION}.*required`),
  );
});

test("receipt-ul este legat de tranzacția rezervată și de proiecția exactă a documentului", () => {
  const projected = receipt();
  projected.result.entry.documentPaths = ["content/despre.md"];
  projected.result.documents = [{
    relativePath: "content/despre.md",
    snapshot: {
      relativePath: "content/despre.md",
      text: "Despre noi",
      dirty: true,
      hash: "0000000000000000",
      bytes: 10,
      revision: 4,
    },
  }];
  assert.equal(
    requireProjectWorkspaceUndoRedoCommandReceipt(projected, expected),
    projected,
  );

  assert.throws(
    () => requireProjectWorkspaceUndoRedoCommandReceipt(
      projected,
      { ...expected, transactionId: "altă-tranzacție" },
    ),
    /not the reserved target/,
  );

  const mismatchedProjection = structuredClone(projected);
  mismatchedProjection.result.documents[0].snapshot.relativePath = "content/alta.md";
  assert.throws(
    () => requireProjectWorkspaceUndoRedoCommandReceipt(mismatchedProjection, expected),
    /invalid FileBuffer snapshot/,
  );
});

test("manifestul de topologie este obligatoriu și rămâne în resursele tranzacției", () => {
  const withoutTopology = receipt();
  delete withoutTopology.result.entry.topologyPaths;
  assert.throws(
    () => requireProjectWorkspaceUndoRedoCommandReceipt(withoutTopology, expected),
    /valid transaction-topology manifest/,
  );

  const outsideTransaction = receipt();
  outsideTransaction.result.entry = {
    ...outsideTransaction.result.entry,
    documentPaths: ["content/despre.md"],
    topologyPaths: ["templates/despre.html"],
  };
  assert.throws(
    () => requireProjectWorkspaceUndoRedoCommandReceipt(outsideTransaction, expected),
    /outside the transaction resources/,
  );
});

test("numai istoricul structural rescanează catalogul înainte de Preview", async () => {
  const calls = [];
  const host = {
    activeScannedPath: "content/despre.md",
    async rescanCurrentProjectForCommittedHistory(...args) {
      calls.push(args);
    },
  };
  const context = {
    projectRoot: "/project-a",
    runtimeSessionId: "session-a:runtime-1",
    projectSessionEpoch: 4,
    workspaceRevision: 8,
  };

  const contentOnly = receipt();
  assert.equal(projectWorkspaceHistoryChangesTopology(contentOnly), false);
  assert.equal(
    await reconcileProjectWorkspaceTopologyAfterHistory(host, contentOnly, context),
    false,
  );
  assert.deepEqual(calls, []);

  const structural = receipt();
  structural.result.entry = {
    ...structural.result.entry,
    documentPaths: ["content/despre.md", "templates/despre.html"],
    topologyPaths: ["content/despre.md", "templates/despre.html"],
  };
  assert.equal(projectWorkspaceHistoryChangesTopology(structural), true);
  assert.equal(
    await reconcileProjectWorkspaceTopologyAfterHistory(host, structural, context),
    true,
  );
  assert.deepEqual(calls, [[
    context,
    "content/despre.md",
    { strict: true, deferPreviewRefresh: true },
  ]]);
});

test("reconcilierea topologiei precedă publicarea generației Preview", () => {
  const service = readFileSync(resolve(process.cwd(), "src/lib/versioning/workspace-history-service.svelte.ts"), "utf8");
  const syncStart = service.indexOf("private async settleCanonicalProjection");
  const topology = service.indexOf(
    "await reconcileProjectWorkspaceTopologyAfterHistory",
    syncStart,
  );
  const preview = service.indexOf("await this.dependencies.authority.projectLatest", syncStart);
  assert.ok(syncStart >= 0 && topology > syncStart && preview > topology);
});

test("proiecția canonică UI a Undo/Redo nu depinde de succesul Preview", () => {
  const service = readFileSync(resolve(process.cwd(), "src/lib/versioning/workspace-history-service.svelte.ts"), "utf8");
  const localStart = service.indexOf("private applyLocalProjection");
  const syncStart = service.indexOf("private async settleCanonicalProjection");
  const refresh = service.indexOf("project.refreshToken += 1", localStart);
  const preview = service.indexOf("await this.dependencies.authority.projectLatest", syncStart);
  const previewCatch = service.indexOf("let warning = errorMessage(error)", preview);
  const rollback = service.indexOf("await this.dependencies.preview.rollbackCanvasPatch", previewCatch);
  const cssRefresh = service.indexOf("source.notifyCssSourceChanged()", localStart);

  assert.ok(localStart >= 0 && syncStart > localStart);
  assert.ok(cssRefresh > localStart && cssRefresh < syncStart);
  assert.ok(refresh > localStart && refresh < syncStart);
  assert.ok(previewCatch > preview);
  assert.ok(rollback > previewCatch);
});

test("Undo/Redo aplică patch-ul înaintea reconcilierii canonice și nu rezervă frontend-ul", () => {
  const service = readFileSync(resolve(process.cwd(), "src/lib/versioning/workspace-history-service.svelte.ts"), "utf8");
  const start = service.indexOf("private async runKernel");
  const end = service.indexOf("private contextIsCurrent", start);
  const operation = service.slice(start, end);
  const patch = operation.indexOf("await this.dependencies.preview.applyCanvasPatch");
  const settle = operation.indexOf("void this.settleCanonicalProjection");
  assert.ok(start >= 0 && end > start);
  assert.ok(patch >= 0 && settle > patch);
  assert.doesNotMatch(operation, /beginKernelUndoRedoFrontendLease|drainPreviewStructuralLanes/);
});

test("mutarea semantică finalizează editarea HTML și comite direct prin autoritatea Rust", () => {
  const controller = readFileSync(
    resolve(process.cwd(), "src/lib/state/editor-navigation-controller.ts"),
    "utf8",
  );
  const start = controller.indexOf("export async function moveEditorNavigationNode");
  const end = controller.indexOf("function requireFocusedActiveDocument", start);
  const operation = controller.slice(start, end);
  const flush = operation.indexOf(
    'await host.flushInteractiveEditorDrafts("snapshot")',
  );
  const capture = operation.indexOf("captureEditorMoveNodeAnchor");
  const rebase = operation.indexOf("resolveEditorMoveNodeAnchor");
  const commit = operation.indexOf("await commitEditorMove");
  const projection = operation.indexOf("const settlement = await host.projectCommittedMove");
  const settledStatus = operation.indexOf("if (settlement.warnings.length === 0)", projection);

  assert.ok(start >= 0 && end > start);
  assert.ok(capture >= 0 && capture < flush);
  assert.ok(flush >= 0);
  assert.ok(rebase > flush);
  assert.ok(commit > rebase);
  assert.ok(projection > commit);
  assert.ok(settledStatus > projection);
  assert.doesNotMatch(operation, /runInPreviewStructuralLane/);
});

test("mutarea semantică păstrează patch-ul rapid, dar așteaptă reconcilierea canonică", () => {
  const control = readFileSync(
    resolve(process.cwd(), "src/lib/kernel/preview-projection-control.ts"),
    "utf8",
  );
  const start = control.indexOf("export async function projectCommittedEditorMoveMutation");
  const end = control.indexOf("function degradedCommittedStructuralSettlement", start);
  const projection = control.slice(start, end);
  const patch = projection.indexOf("await applyCommittedCanvasPatch");
  const settlement = projection.indexOf("await settleProjectWorkspaceMutation");

  assert.ok(start >= 0 && end > start);
  assert.ok(patch >= 0 && settlement > patch);
  assert.doesNotMatch(projection, /projectionMode|deferredCommittedStructuralSettlement/);
});

test("receipt-ul Workbench este validat și proiectat înaintea topologiei", () => {
  const withWorkbench = receipt({
    workbench: {
      schemaVersion: 1,
      changed: true,
      projectRoot: "/project-a",
      runtimeSessionId: "session-a:runtime-1",
      revisionBefore: 4,
      revisionAfter: 5,
      snapshot: {
        schemaVersion: WORKBENCH_SCHEMA_VERSION,
        projectRoot: "/project-a",
        projectSessionId: "session-a",
        runtimeSessionId: "session-a:runtime-1",
        revision: 5,
        activeActivity: "editor",
        activeGroupId: "primary",
        split: "none",
        splitRatioBasisPoints: 5000,
        canvasViewport: {
          mode: "fit",
          preset: "desktop",
          widthPx: 1440,
          zoomPercent: 100,
          showRulers: true,
        },
        groups: [{
          groupId: "primary",
          documents: [],
          activeDocumentId: null,
        }],
        bottomPanel: { open: false, activeView: "problems" },
        contentWorkspace: { mode: "list", pagePath: null },
        selectedProjectEntry: null,
      },
    },
  });
  assert.equal(
    requireProjectWorkspaceUndoRedoCommandReceipt(withWorkbench, expected),
    withWorkbench,
  );

  const service = readFileSync(resolve(process.cwd(), "src/lib/versioning/workspace-history-service.svelte.ts"), "utf8");
  const assignment = service.indexOf("this.dependencies.workbench.acceptSnapshot(receipt.workbench.snapshot)");
  const topology = service.indexOf("await reconcileProjectWorkspaceTopologyAfterHistory");
  assert.ok(assignment >= 0 && topology > assignment);
});

test("snapshot-ul și lanțul reviziilor Undo/Redo sunt validate independent", () => {
  assert.throws(
    () => requireProjectWorkspaceUndoRedoCommandReceipt(
      receipt({ workspace: { ...receipt().workspace, schemaVersion: 1 } }),
      expected,
    ),
    new RegExp(`ProjectWorkspace snapshot.*schema .*1.*schema .*${PROJECT_WORKSPACE_SCHEMA_VERSION}.*required`),
  );
  assert.throws(
    () => requireProjectWorkspaceUndoRedoCommandReceipt(
      receipt({ workspace: { ...receipt().workspace, revision: 9 } }),
      expected,
    ),
    /Undo\/Redo snapshot is at revision .*9.*result confirms revision .*8/,
  );
});

test("versiunea frontend a comenzii este identică versiunii publicate de Rust", () => {
  const rust = readFileSync(
    resolve(process.cwd(), "src-tauri/src/commands/project/contracts.rs"),
    "utf8",
  );
  const match = rust.match(
    /PROJECT_WORKSPACE_UNDO_REDO_COMMAND_SCHEMA_VERSION:\s*u32\s*=\s*(\d+)/,
  );
  assert.ok(match, "constanta Rust a contractului Undo/Redo trebuie să existe");
  assert.equal(
    Number(match[1]),
    PROJECT_WORKSPACE_UNDO_REDO_COMMAND_SCHEMA_VERSION,
  );
});

test("versiunea frontend ProjectWorkspace este identică versiunii publicate de Rust", () => {
  const rust = readFileSync(
    resolve(process.cwd(), "src-tauri/src/kernel/project_workspace/model.rs"),
    "utf8",
  );
  const match = rust.match(
    /PROJECT_WORKSPACE_SCHEMA_VERSION:\s*u32\s*=\s*(\d+)/,
  );
  assert.ok(match, "constanta Rust a contractului ProjectWorkspace trebuie să existe");
  assert.equal(Number(match[1]), PROJECT_WORKSPACE_SCHEMA_VERSION);
});
