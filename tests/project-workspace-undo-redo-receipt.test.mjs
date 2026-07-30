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
} from "$lib/types";

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
    },
    workspace: {
      schemaVersion: PROJECT_WORKSPACE_SCHEMA_VERSION,
      projectRoot: "/project-a",
      runtimeSessionId: "session-a:runtime-1",
      revision: 8,
    },
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

test("receipt-ul comenzii v3 acceptă snapshot-ul ProjectWorkspace curent", () => {
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
    async rescanCurrentProjectWithinKernelUndoRedoLease(...args) {
      calls.push(args);
    },
  };
  const lease = {
    expectedProjectRoot: "/project-a",
    expectedSessionId: "session-a:runtime-1",
    expectedSessionEpoch: 4,
  };

  const contentOnly = receipt();
  assert.equal(projectWorkspaceHistoryChangesTopology(contentOnly), false);
  assert.equal(
    await reconcileProjectWorkspaceTopologyAfterHistory(host, contentOnly, lease),
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
    await reconcileProjectWorkspaceTopologyAfterHistory(host, structural, lease),
    true,
  );
  assert.deepEqual(calls, [[
    lease,
    "content/despre.md",
    { strict: true, deferPreviewRefresh: true },
  ]]);
});

test("reconcilierea topologiei precedă publicarea generației Preview", () => {
  const route = readFileSync(resolve(process.cwd(), "src/routes/+page.svelte"), "utf8");
  const syncStart = route.indexOf("async function syncAfterKernelUndoRedo");
  const topology = route.indexOf(
    "await reconcileProjectWorkspaceTopologyAfterHistory",
    syncStart,
  );
  const preview = route.indexOf("await projectLatestProjectWorkspacePreview", syncStart);
  assert.ok(syncStart >= 0 && topology > syncStart && preview > topology);
});

test("proiecția canonică UI a Undo/Redo nu depinde de succesul Preview", () => {
  const route = readFileSync(resolve(process.cwd(), "src/routes/+page.svelte"), "utf8");
  const syncStart = route.indexOf("async function syncAfterKernelUndoRedo");
  const refresh = route.indexOf("app.refreshToken += 1", syncStart);
  const preview = route.indexOf("await projectLatestProjectWorkspacePreview", syncStart);
  const previewCatch = route.indexOf("return errorMessage(error)", preview);
  const cssRefresh = route.indexOf("app.notifyCssSourceChanged()", syncStart);

  assert.ok(syncStart >= 0);
  assert.ok(cssRefresh > syncStart && cssRefresh < preview);
  assert.ok(refresh > syncStart && refresh < preview);
  assert.ok(previewCatch > preview);
});

test("bariera Undo închide drafturile interactive înainte să blocheze structural lane", () => {
  const app = readFileSync(
    resolve(process.cwd(), "src/lib/state/app.svelte.ts"),
    "utf8",
  );
  const start = app.indexOf("async beginKernelUndoRedoFrontendLease()");
  const end = app.indexOf("endKernelUndoRedoFrontendLease()", start);
  const boundary = app.slice(start, end);
  const quiesce = boundary.indexOf(
    "this.kernelUndoRedoFrontendQuiesceActive = true",
  );
  const flush = boundary.indexOf(
    'await this.flushInteractiveEditorDrafts("history")',
  );
  const exclusiveLease = boundary.indexOf(
    "this.kernelUndoRedoFrontendLeaseActive = true",
  );
  const drain = boundary.indexOf("await drainPreviewStructuralLanes()");

  assert.ok(start >= 0 && end > start);
  assert.ok(quiesce >= 0);
  assert.ok(flush > quiesce);
  assert.ok(exclusiveLease > flush);
  assert.ok(drain > exclusiveLease);
});

test("mutarea semantică finalizează editarea HTML înainte să intre în structural lane", () => {
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
  const lane = operation.indexOf("runInPreviewStructuralLane");

  assert.ok(start >= 0 && end > start);
  assert.ok(capture >= 0 && capture < flush);
  assert.ok(flush >= 0);
  assert.ok(rebase > flush);
  assert.ok(lane > flush);
  assert.ok(lane > rebase);
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
        schemaVersion: 1,
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
        selectedProjectEntry: null,
      },
    },
  });
  assert.equal(
    requireProjectWorkspaceUndoRedoCommandReceipt(withWorkbench, expected),
    withWorkbench,
  );

  const route = readFileSync(resolve(process.cwd(), "src/routes/+page.svelte"), "utf8");
  const assignment = route.indexOf("app.workbenchSnapshot = receipt.workbench.snapshot");
  const topology = route.indexOf("await reconcileProjectWorkspaceTopologyAfterHistory");
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
    resolve(process.cwd(), "src-tauri/src/commands/project.rs"),
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
