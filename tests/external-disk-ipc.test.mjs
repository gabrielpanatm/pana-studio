import assert from "node:assert/strict";
import { test } from "node:test";
import {
  acceptProjectWorkspaceSaveBaseline,
  createExternalDiskState,
  establishExternalDiskBaseline,
  invalidateExternalReconcileForProjectTransition,
  markWorkspaceProjectionRecoveryRequired,
  resetExternalDiskState,
  suspendAndDrainExternalDiskMonitoring,
} from "$lib/state/external-disk-controller";
import { diffDiskManifests } from "$lib/project/disk-manifest";
import {
  acceptedExternalReconcileManifest,
  externalReconcileUiLeaseMatches,
  projectExternalReconcileSources,
} from "$lib/project/external-reconcile-projection";

if (!globalThis.window) globalThis.window = globalThis;

function deferred() {
  let resolve;
  const promise = new Promise((resolvePromise) => { resolve = resolvePromise; });
  return { promise, resolve };
}

function manifest(versionToken = "v1", truncated = false) {
  return {
    root: "/project",
    files: [{
      relativePath: "templates/index.html",
      modifiedMs: 1,
      size: 20,
      versionToken,
    }],
    truncated,
    maxFiles: 1_000,
  };
}

function host(overrides = {}) {
  const notifications = [];
  const cleared = [];
  return {
    sessionProjectRoot: "/project",
    kernelProjectSessionId: "session:runtime",
    projectSessionEpoch: 2,
    projectTransitionFrontendLeaseActive: false,
    kernelUndoRedoFrontendLeaseActive: false,
    externalDiskState: createExternalDiskState(),
    externalDiskTimer: null,
    externalDiskSuspended: false,
    externalDiskCheckInFlight: null,
    externalDiskCheckGeneration: 0,
    scannedProject: {
      root: "/project",
      isZola: true,
      kernelSessionId: "session:runtime",
      acceptedDiskGeneration: 3,
      acceptedDiskManifest: manifest(),
    },
    activeScannedPath: "templates/index.html",
    source: "old",
    sourceCache: {},
    editorMutationEpoch: 0,
    selectionEpoch: 0,
    refreshToken: 0,
    jsRefreshToken: 0,
    previewWorkspaceRevision: null,
    scssVariables: [],
    globalDirtyState: { dirty: false },
    projectStatus: "",
    notify(notification) { notifications.push(notification); },
    clearNotification(id) { cleared.push(id); },
    quiesceExternalReconcileInteractions() {},
    async waitForExternalReconcileInteractionLock() {},
    async resetHistoryAfterExternalReconcile() {},
    async requestPreviewRefresh() { return true; },
    setGlobalStatus() {},
    notifications,
    cleared,
    ...overrides,
  };
}

test("external monitor baseline comes only from the Rust-accepted session manifest", async () => {
  const activeHost = host();
  await establishExternalDiskBaseline(activeHost);
  assert.equal(activeHost.externalDiskState.baseline, activeHost.scannedProject.acceptedDiskManifest);
  assert.equal(activeHost.externalDiskState.truncated, false);

  const staleHost = host({ kernelProjectSessionId: "session:replacement" });
  await establishExternalDiskBaseline(staleHost);
  assert.equal(staleHost.externalDiskState.baseline, null);
});

test("Save advances the external monitor baseline before polling resumes", () => {
  const activeHost = host({ externalDiskSuspended: true });
  activeHost.externalDiskState = {
    ...activeHost.externalDiskState,
    baseline: manifest("v1"),
    changed: true,
    changedFiles: ["templates/index.html"],
    blockedByDirtySession: true,
  };
  const accepted = manifest("v2");

  acceptProjectWorkspaceSaveBaseline(activeHost, accepted, 4);

  assert.equal(activeHost.scannedProject.acceptedDiskGeneration, 4);
  assert.equal(activeHost.scannedProject.acceptedDiskManifest, accepted);
  assert.equal(activeHost.externalDiskState.baseline, accepted);
  assert.equal(activeHost.externalDiskState.changed, false);
  assert.equal(activeHost.externalDiskState.blockedByDirtySession, false);
  assert.equal(activeHost.externalDiskState.workspaceProjectionRecoveryRequired, false);
  assert.deepEqual(
    diffDiskManifests(activeHost.externalDiskState.baseline, accepted).changedFiles,
    [],
  );
  assert.equal(activeHost.externalDiskSuspended, true);
});

test("suspension drains the exact in-flight monitor operation", async () => {
  const gate = deferred();
  const activeHost = host();
  activeHost.externalDiskState = { ...activeHost.externalDiskState, checking: true };
  const tracked = {
    projectRoot: "/project",
    runtimeSessionId: "session:runtime",
    projectSessionEpoch: 2,
    generation: 0,
    promise: gate.promise.finally(() => {
      if (activeHost.externalDiskCheckInFlight === tracked) {
        activeHost.externalDiskCheckInFlight = null;
      }
    }),
  };
  activeHost.externalDiskCheckInFlight = tracked;
  let completed = false;
  const draining = suspendAndDrainExternalDiskMonitoring(activeHost).then(() => { completed = true; });
  await Promise.resolve();
  assert.equal(completed, false);
  gate.resolve();
  await draining;
  assert.equal(activeHost.externalDiskSuspended, true);
  assert.equal(activeHost.externalDiskState.checking, false);
});

test("project transition invalidates monitor continuations before UI replacement", () => {
  const activeHost = host();
  activeHost.externalDiskCheckInFlight = {
    projectRoot: "/project",
    runtimeSessionId: "session:runtime",
    projectSessionEpoch: 2,
    generation: 0,
    promise: Promise.resolve(),
  };
  invalidateExternalReconcileForProjectTransition(activeHost);
  assert.equal(activeHost.projectSessionEpoch, 3);
  assert.equal(activeHost.externalDiskCheckInFlight, null);
  assert.equal(activeHost.externalDiskState.reconciling, true);
});

test("projection recovery blocks monitoring and exposes only destructive disk reload", () => {
  const activeHost = host();
  markWorkspaceProjectionRecoveryRequired(activeHost, "proiecția trebuie refăcută");
  assert.equal(activeHost.externalDiskState.workspaceProjectionRecoveryRequired, true);
  assert.equal(activeHost.externalDiskState.blockedByDirtySession, true);
  assert.equal(activeHost.notifications.length, 1);
  assert.equal(activeHost.notifications[0].actionId, "external-disk.reload");
  assert.equal(activeHost.notifications[0].secondaryActionId, undefined);
});

test("external reconcile manifest advances only for applied or noop Rust receipts", () => {
  const accepted = manifest("v2");
  const base = {
    status: "applied",
    acceptedManifest: accepted,
    acceptedDiskGeneration: 4,
  };
  assert.equal(acceptedExternalReconcileManifest(base, "/project"), accepted);
  assert.equal(
    acceptedExternalReconcileManifest({ ...base, status: "noop" }, "/project"),
    accepted,
  );
  assert.throws(
    () => acceptedExternalReconcileManifest({ ...base, status: "blocked" }, "/project"),
    /nu poate avansa baseline-ul/,
  );
  assert.throws(
    () => acceptedExternalReconcileManifest({ ...base, acceptedDiskGeneration: null }, "/project"),
    /generația AcceptedDisk terminală/,
  );
});

test("external source projection applies exact Rust text and preserves unrelated cache", () => {
  const receipt = {
    invalidatedPaths: ["templates/index.html"],
    activeFile: { relativePath: "templates/index.html", text: "new" },
  };
  const projection = projectExternalReconcileSources(
    {
      "scanned:templates/index.html": "old",
      "scanned:templates/other.html": "keep",
    },
    receipt,
    "templates/index.html",
    true,
  );
  assert.equal(projection.activeSource, "new");
  assert.equal(projection.sourceCache["scanned:templates/index.html"], "new");
  assert.equal(projection.sourceCache["scanned:templates/other.html"], "keep");
});

test("external UI lease detects project, runtime, edit and selection races", () => {
  const lease = {
    projectRoot: "/project",
    kernelSessionId: "session:runtime",
    projectSessionEpoch: 2,
    activeRelativePath: "templates/index.html",
    editorMutationEpoch: 4,
    selectionEpoch: 5,
  };
  assert.equal(externalReconcileUiLeaseMatches(lease, { ...lease }), true);
  for (const changed of [
    { kernelSessionId: "session:replacement" },
    { projectSessionEpoch: 3 },
    { editorMutationEpoch: 5 },
    { selectionEpoch: 6 },
  ]) {
    assert.equal(externalReconcileUiLeaseMatches(lease, { ...lease, ...changed }), false);
  }
});

test("reset detaches all monitor state from the old ProjectWorkspace session", () => {
  const activeHost = host();
  activeHost.externalDiskState = { ...activeHost.externalDiskState, baseline: manifest() };
  resetExternalDiskState(activeHost);
  assert.equal(activeHost.projectSessionEpoch, 3);
  assert.equal(activeHost.externalDiskState.baseline, null);
  assert.equal(activeHost.externalDiskCheckInFlight, null);
});
