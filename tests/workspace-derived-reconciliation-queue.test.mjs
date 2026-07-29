import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { reconcileWorkspaceDerivedState } from "$lib/state/project-controller";

if (!globalThis.window) globalThis.window = globalThis;

afterEach(() => clearMocks());

function deferred() {
  let resolve;
  const promise = new Promise((resolvePromise) => { resolve = resolvePromise; });
  return { promise, resolve };
}

function scan(revision) {
  return {
    root: "/project",
    previewBaseUrl: null,
    previewWarning: null,
    activeTheme: null,
    files: [],
    kernelSessionId: "session:runtime",
    workspaceRevision: revision,
  };
}

function options(revision, topologyChanged) {
  return {
    expectedProjectRoot: "/project",
    expectedSessionId: "session:runtime",
    expectedWorkspaceRevision: revision,
    topologyChanged,
    refreshSourceGraph: false,
    refreshScss: false,
  };
}

test("coada per sesiune păstrează o singură cerere pending și propagă topologia spre ultima revizie", async () => {
  const firstScanStarted = deferred();
  const firstScanGate = deferred();
  let scanCalls = 0;
  mockIPC(async (command) => {
    assert.equal(command, "scan_project");
    scanCalls += 1;
    if (scanCalls === 1) {
      firstScanStarted.resolve();
      await firstScanGate.promise;
      return scan(2);
    }
    return scan(4);
  });
  const host = {
    sessionProjectRoot: "/project",
    kernelProjectSessionId: "session:runtime",
    projectWorkspaceSnapshot: { revision: 2 },
    scannedProject: scan(1),
    diskState: {},
    activeScannedPath: null,
    scssVariables: [],
    sourceGraphProjectionStatus: "current",
    sourceGraphWorkspaceRevision: 1,
    loadScannedProjectFile: async () => {},
  };

  const first = reconcileWorkspaceDerivedState(host, options(2, true));
  await firstScanStarted.promise;
  host.projectWorkspaceSnapshot = { revision: 4 };
  const replaced = reconcileWorkspaceDerivedState(host, options(3, false));
  const latest = reconcileWorkspaceDerivedState(host, options(4, false));

  assert.equal((await replaced).topology, "superseded");
  firstScanGate.resolve();
  assert.equal((await first).topology, "superseded");
  const latestOutcome = await latest;

  assert.equal(latestOutcome.topology, "current");
  assert.equal(host.scannedProject.workspaceRevision, 4);
  assert.equal(scanCalls, 2);
});
