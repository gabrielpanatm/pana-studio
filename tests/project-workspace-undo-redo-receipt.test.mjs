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

test("receipt-ul comenzii v3 acceptă snapshot-ul ProjectWorkspace v2", () => {
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
    new RegExp(`schema comenzii 1; era necesară ${PROJECT_WORKSPACE_UNDO_REDO_COMMAND_SCHEMA_VERSION}`),
  );
});

test("receipt-ul este legat de tranzacția rezervată și de proiecția exactă a documentului", () => {
  const projected = receipt();
  projected.result.entry.documentPaths = ["sursa/content/despre.md"];
  projected.result.documents = [{
    relativePath: "sursa/content/despre.md",
    snapshot: {
      relativePath: "sursa/content/despre.md",
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
    /nu ținta rezervată/,
  );

  const mismatchedProjection = structuredClone(projected);
  mismatchedProjection.result.documents[0].snapshot.relativePath = "sursa/content/alta.md";
  assert.throws(
    () => requireProjectWorkspaceUndoRedoCommandReceipt(mismatchedProjection, expected),
    /snapshot FileBuffer invalid/,
  );
});

test("manifestul de topologie este obligatoriu și rămâne în resursele tranzacției", () => {
  const withoutTopology = receipt();
  delete withoutTopology.result.entry.topologyPaths;
  assert.throws(
    () => requireProjectWorkspaceUndoRedoCommandReceipt(withoutTopology, expected),
    /manifest valid al topologiei/,
  );

  const outsideTransaction = receipt();
  outsideTransaction.result.entry = {
    ...outsideTransaction.result.entry,
    documentPaths: ["sursa/content/despre.md"],
    topologyPaths: ["sursa/templates/despre.html"],
  };
  assert.throws(
    () => requireProjectWorkspaceUndoRedoCommandReceipt(outsideTransaction, expected),
    /în afara resurselor tranzacției/,
  );
});

test("numai istoricul structural rescanează catalogul înainte de Preview", async () => {
  const calls = [];
  const host = {
    activeScannedPath: "sursa/content/despre.md",
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
    documentPaths: ["sursa/content/despre.md", "sursa/templates/despre.html"],
    topologyPaths: ["sursa/content/despre.md", "sursa/templates/despre.html"],
  };
  assert.equal(projectWorkspaceHistoryChangesTopology(structural), true);
  assert.equal(
    await reconcileProjectWorkspaceTopologyAfterHistory(host, structural, lease),
    true,
  );
  assert.deepEqual(calls, [[
    lease,
    "sursa/content/despre.md",
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

test("snapshot-ul și lanțul reviziilor Undo/Redo sunt validate independent", () => {
  assert.throws(
    () => requireProjectWorkspaceUndoRedoCommandReceipt(
      receipt({ workspace: { ...receipt().workspace, schemaVersion: 1 } }),
      expected,
    ),
    /Snapshot-ul ProjectWorkspace.*schema 1; era necesară 2/,
  );
  assert.throws(
    () => requireProjectWorkspaceUndoRedoCommandReceipt(
      receipt({ workspace: { ...receipt().workspace, revision: 9 } }),
      expected,
    ),
    /Snapshot-ul Undo\/Redo este la revizia 9.*confirmă revizia 8/,
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
