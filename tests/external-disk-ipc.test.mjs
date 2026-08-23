import assert from "node:assert/strict";
import { test } from "node:test";
import {
  acceptExternalDiskSaveBaseline,
  createExternalDiskSnapshot,
  establishExternalDiskBaseline,
  invalidateExternalDiskForTransition,
  markExternalDiskProjectionRecovery,
  resetExternalDiskSnapshot,
} from "$lib/session/external-disk/state";
import {
  suspendAndDrainExternalDiskMonitoring,
} from "$lib/session/external-disk/monitor";
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
  const state = {
    kernelProjectSessionId: "session:runtime",
    projectSessionEpoch: 2,
    projectTransitionFrontendLeaseActive: false,
    kernelUndoRedoFrontendLeaseActive: false,
    externalDiskState: createExternalDiskSnapshot(),
    externalDiskAuditTimer: null,
    externalDiskWatchUnlisten: null,
    externalDiskWatchGeneration: null,
    externalDiskWatchStopIdentity: null,
    externalDiskWatchRevision: 0,
    externalDiskWatchSubscriptionGeneration: 0,
    externalDiskPendingWatchNotice: null,
    externalDiskWatchEventPending: false,
    externalDiskWatchEventDrainInFlight: false,
    externalDiskSuspended: false,
    externalDiskCheckInFlight: null,
    externalDiskCheckGeneration: 0,
    externalDiskReconcileGeneration: 0,
    scannedProject: {
      root: "/project",
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
    scssVariables: [],
    globalDirtyState: { dirty: false },
    projectStatus: "",
    escalateGlobalStatus(notification) { notifications.push(notification); },
    clearNotification(id) { cleared.push(id); },
    quiesceExternalReconcileInteractions() {},
    async waitForExternalReconcileInteractionLock() {},
    async resetHistoryAfterExternalReconcile() {},
    setGlobalStatus() {},
    notifications,
    cleared,
  };
  Object.assign(state, overrides);
  state.commands = {
    setStatus: (...args) => state.setGlobalStatus(...args),
    escalateStatus: (notification) => state.escalateGlobalStatus(notification),
    clearStatus: (id) => state.clearNotification(id),
    refreshSourceGraph: (...args) => state.refreshSourceGraph?.(...args),
    quiesceInteractions: () => state.quiesceExternalReconcileInteractions(),
    waitForInteractionLock: () => state.waitForExternalReconcileInteractionLock(),
    resetHistory: () => state.resetHistoryAfterExternalReconcile(),
    projectLatestPreview: async () => ({ status: "projected" }),
  };
  state.runtime = {
    get snapshot() { return state.externalDiskState; },
    set snapshot(value) { state.externalDiskState = value; },
    get auditTimer() { return state.externalDiskAuditTimer; },
    set auditTimer(value) { state.externalDiskAuditTimer = value; },
    get watchUnlisten() { return state.externalDiskWatchUnlisten; },
    set watchUnlisten(value) { state.externalDiskWatchUnlisten = value; },
    get watchGeneration() { return state.externalDiskWatchGeneration; },
    set watchGeneration(value) { state.externalDiskWatchGeneration = value; },
    get watchStopIdentity() { return state.externalDiskWatchStopIdentity; },
    set watchStopIdentity(value) { state.externalDiskWatchStopIdentity = value; },
    get watchRevision() { return state.externalDiskWatchRevision; },
    set watchRevision(value) { state.externalDiskWatchRevision = value; },
    get watchSubscriptionGeneration() { return state.externalDiskWatchSubscriptionGeneration; },
    set watchSubscriptionGeneration(value) { state.externalDiskWatchSubscriptionGeneration = value; },
    get pendingWatchNotice() { return state.externalDiskPendingWatchNotice; },
    set pendingWatchNotice(value) { state.externalDiskPendingWatchNotice = value; },
    get watchEventPending() { return state.externalDiskWatchEventPending; },
    set watchEventPending(value) { state.externalDiskWatchEventPending = value; },
    get watchEventDrainInFlight() { return state.externalDiskWatchEventDrainInFlight; },
    set watchEventDrainInFlight(value) { state.externalDiskWatchEventDrainInFlight = value; },
    get suspended() { return state.externalDiskSuspended; },
    set suspended(value) { state.externalDiskSuspended = value; },
    get checkInFlight() { return state.externalDiskCheckInFlight; },
    set checkInFlight(value) { state.externalDiskCheckInFlight = value; },
    get checkGeneration() { return state.externalDiskCheckGeneration; },
    set checkGeneration(value) { state.externalDiskCheckGeneration = value; },
    get reconcileGeneration() { return state.externalDiskReconcileGeneration; },
    set reconcileGeneration(value) { state.externalDiskReconcileGeneration = value; },
  };
  state.environment = {
    session: {
      get runtimeSessionId() { return state.kernelProjectSessionId; },
      get epoch() { return state.projectSessionEpoch; },
      get project() { return state.scannedProject; },
      get transitionLocked() { return state.projectTransitionFrontendLeaseActive; },
      get historyLocked() { return state.kernelUndoRedoFrontendLeaseActive; },
      get aiLocked() { return state.aiEditLeaseFrontendLockActive ?? false; },
    },
    editor: {
      get activeScannedPath() { return state.activeScannedPath; },
      get sourceCache() { return state.sourceCache; },
      get mutationEpoch() { return state.editorMutationEpoch; },
      get selectionEpoch() { return state.selectionEpoch; },
      get dirty() { return state.globalDirtyState.dirty; },
    },
    projections: {
      invalidateProjectSession: () => { state.projectSessionEpoch += 1; },
      acceptProject: (project) => { state.scannedProject = project; },
      acceptWorkspace: (workspace) => { state.projectWorkspaceSnapshot = workspace; },
      setProjectStatus: (status) => { state.projectStatus = status; },
      acceptSources: (cache, activeSource) => {
        state.sourceCache = cache;
        if (activeSource !== null) state.source = activeSource;
      },
      acceptScssVariables: (variables) => { state.scssVariables = variables; },
      invalidateDerived: () => { state.refreshToken += 1; },
      invalidatePageJs: () => { state.jsRefreshToken += 1; },
    },
    commands: state.commands,
  };
  state.context = { runtime: state.runtime, environment: state.environment };
  return state;
}

test("external monitor baseline comes only from the Rust-accepted session manifest", async () => {
  const activeHost = host();
  establishExternalDiskBaseline(activeHost.context);
  assert.equal(activeHost.externalDiskState.baseline, activeHost.scannedProject.acceptedDiskManifest);
  assert.equal(activeHost.externalDiskState.truncated, false);

  const staleHost = host({ kernelProjectSessionId: "session:replacement" });
  establishExternalDiskBaseline(staleHost.context);
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

  acceptExternalDiskSaveBaseline(activeHost.context, accepted, 4);

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
  const draining = suspendAndDrainExternalDiskMonitoring(activeHost.context).then(() => { completed = true; });
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
  invalidateExternalDiskForTransition(activeHost.context);
  assert.equal(activeHost.projectSessionEpoch, 3);
  assert.equal(activeHost.externalDiskCheckInFlight, null);
  assert.equal(activeHost.externalDiskState.reconciling, true);
});

test("projection recovery blocks monitoring and exposes only destructive disk reload", () => {
  const activeHost = host();
  markExternalDiskProjectionRecovery(activeHost.context, "proiecția trebuie refăcută");
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
    /cannot advance the external baseline/,
  );
  assert.throws(
    () => acceptedExternalReconcileManifest({ ...base, acceptedDiskGeneration: null }, "/project"),
    /terminal AcceptedDisk generation/,
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
  resetExternalDiskSnapshot(activeHost.context);
  assert.equal(activeHost.projectSessionEpoch, 3);
  assert.equal(activeHost.externalDiskState.baseline, null);
  assert.equal(activeHost.externalDiskCheckInFlight, null);
});
