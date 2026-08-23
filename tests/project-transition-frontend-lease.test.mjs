import assert from "node:assert/strict";
import { test } from "node:test";
import {
  ProjectTransitionFrontendLeaseBusyError,
  acquireProjectTransitionFrontendLease,
  releaseProjectTransitionFrontendLease,
  runWithProjectTransitionFrontendLease,
} from "$lib/state/project-transition-frontend-lease";
import {
  capturePreviewStructuralSessionLease,
  PreviewStructuralCancellationError,
} from "$lib/kernel/preview-structural-lane";
import {
  resumeExternalDiskMonitoringAfterTransition,
} from "$lib/session/external-disk/monitor";
import { rescanCurrentProject } from "$lib/state/project-derived-state-controller";
import { sourceWorkspaceReadOnly } from "$lib/editor/source-workspace.svelte";

function deferred() {
  let resolve;
  const promise = new Promise((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

function authority() {
  return {
    projectTransitionFrontendLease: null,
    projectTransitionFrontendLeaseGeneration: 0,
  };
}

test("open și rescan suprapuse au un singur proprietar și rescan nu ajunge la efecte Rust", async () => {
  const state = authority();
  const openGate = deferred();
  const events = [];
  let rescanRustEffects = 0;

  const opening = runWithProjectTransitionFrontendLease(
    state,
    { kind: "open", owner: "project-transition-controller" },
    {
      onAcquire(lease) {
        events.push(`acquire:${lease.kind}:${lease.generation}`);
      },
      async quiesce() {
        events.push("quiesced:open");
      },
      onRelease(lease) {
        events.push(`release:${lease.kind}:${lease.generation}`);
      },
    },
    async () => {
      events.push("rust:open");
      await openGate.promise;
      return "opened";
    },
  );

  await assert.rejects(
    runWithProjectTransitionFrontendLease(
      state,
      { kind: "rescan", owner: "project-transition-controller" },
      { async quiesce() {} },
      async () => {
        rescanRustEffects += 1;
      },
    ),
    (error) => {
      assert.ok(error instanceof ProjectTransitionFrontendLeaseBusyError);
      assert.equal(error.code, "PROJECT_TRANSITION_FRONTEND_BUSY");
      assert.equal(error.active.kind, "open");
      assert.equal(error.requested.kind, "rescan");
      return true;
    },
  );

  assert.equal(rescanRustEffects, 0);
  assert.equal(state.projectTransitionFrontendLease !== null, true);
  openGate.resolve();
  assert.equal(await opening, "opened");
  assert.equal(state.projectTransitionFrontendLease !== null, false);
  assert.deepEqual(events, [
    "acquire:open:1",
    "quiesced:open",
    "rust:open",
    "release:open:1",
  ]);
});

test("rescanCurrentProject refuză lease-ul open înainte de orice efect de reconciliere", async () => {
  const state = authority();
  let rescanSetupEffects = 0;
  const host = {
    ...state,
    scannedProject: { root: "/project" },
    activeScannedPath: "content/_index.md",
    get projectTransitionFrontendLeaseActive() {
      return this.projectTransitionFrontendLease !== null;
    },
    runWithProjectTransitionFrontendLease(request, operation) {
      return runWithProjectTransitionFrontendLease(
        this,
        request,
        {
          async quiesce() {
            rescanSetupEffects += 1;
          },
        },
        operation,
      );
    },
    requireProjectTransitionFrontendLease() {
      throw new Error("rescan nu trebuie să intre în callback");
    },
  };
  const openLease = acquireProjectTransitionFrontendLease(host, {
    kind: "open",
    owner: "project-transition-controller",
  });

  await assert.rejects(
    rescanCurrentProject(host),
    ProjectTransitionFrontendLeaseBusyError,
  );
  assert.equal(rescanSetupEffects, 0);
  assert.equal(host.projectTransitionFrontendLease, openLease);
  assert.equal(releaseProjectTransitionFrontendLease(host, openLease), true);
});

test("release-ul străin sau stale nu coboară barierele editor, structural și disk", async () => {
  const state = authority();
  const foreignAuthority = authority();
  const ownerLease = acquireProjectTransitionFrontendLease(state, {
    kind: "reload",
    owner: "project-transition-controller",
  });
  const foreignLease = acquireProjectTransitionFrontendLease(foreignAuthority, {
    kind: "close",
    owner: "native-window-close",
  });

  assert.equal(releaseProjectTransitionFrontendLease(state, foreignLease), false);
  assert.equal(state.projectTransitionFrontendLease !== null, true);

  const barrierHost = {
    projectTransitionFrontendLeaseActive: state.projectTransitionFrontendLease !== null,
    externalDiskSuspended: true,
    externalDiskAuditTimer: null,
    externalDiskState: {
      baseline: null,
      workspaceProjectionRecoveryRequired: false,
    },
    kernelUndoRedoFrontendLeaseActive: false,
    scannedProject: { root: "/project" },
    sessionProjectRoot: "/project",
    kernelProjectSessionId: "session-a",
    projectSessionEpoch: 1,
    editorSelection: { selectionSnapshot: null },
    async beginPreviewStructuralWriteBoundary() {},
    endPreviewStructuralWriteBoundary() {},
  };
  barrierHost.session = {
    get transitionLocked() { return barrierHost.projectTransitionFrontendLeaseActive; },
    get historyLocked() { return barrierHost.kernelUndoRedoFrontendLeaseActive; },
    get project() { return barrierHost.scannedProject; },
  };
  barrierHost.disk = {
    get suspended() { return barrierHost.externalDiskSuspended; },
    set suspended(value) { barrierHost.externalDiskSuspended = value; },
    get auditTimer() { return barrierHost.externalDiskAuditTimer; },
    set auditTimer(value) { barrierHost.externalDiskAuditTimer = value; },
    get state() { return barrierHost.externalDiskState; },
  };
  const externalDiskContext = {
    runtime: {
      get suspended() { return barrierHost.externalDiskSuspended; },
      set suspended(value) { barrierHost.externalDiskSuspended = value; },
      get auditTimer() { return barrierHost.externalDiskAuditTimer; },
      set auditTimer(value) { barrierHost.externalDiskAuditTimer = value; },
      get snapshot() { return barrierHost.externalDiskState; },
    },
    environment: {
      session: {
        get transitionLocked() { return barrierHost.projectTransitionFrontendLeaseActive; },
        get historyLocked() { return barrierHost.kernelUndoRedoFrontendLeaseActive; },
        get project() { return barrierHost.scannedProject; },
      },
    },
  };

  assert.equal(sourceWorkspaceReadOnly({
    projectTransitionLocked:
      barrierHost.projectTransitionFrontendLeaseActive,
    historyLocked: false,
    aiLocked: false,
  }), true);
  assert.throws(
    () => capturePreviewStructuralSessionLease(barrierHost),
    PreviewStructuralCancellationError,
  );
  resumeExternalDiskMonitoringAfterTransition(externalDiskContext);
  assert.equal(barrierHost.externalDiskSuspended, true);

  assert.equal(releaseProjectTransitionFrontendLease(state, ownerLease), true);
  assert.equal(releaseProjectTransitionFrontendLease(state, ownerLease), false);
  assert.equal(state.projectTransitionFrontendLease !== null, false);
  barrierHost.projectTransitionFrontendLeaseActive = false;
  resumeExternalDiskMonitoringAfterTransition(externalDiskContext);
  assert.equal(barrierHost.externalDiskSuspended, false);
});

test("setup-ul eșuat eliberează numai lease-ul propriu prin finally", async () => {
  const state = authority();
  let released = 0;

  await assert.rejects(
    runWithProjectTransitionFrontendLease(
      state,
      { kind: "reattach", owner: "project-transition-controller" },
      {
        async quiesce() {
          throw new Error("drain failed");
        },
        onRelease() {
          released += 1;
        },
      },
      async () => {
        throw new Error("operația nu trebuie pornită");
      },
    ),
    /drain failed/,
  );

  assert.equal(released, 1);
  assert.equal(state.projectTransitionFrontendLease !== null, false);
});
