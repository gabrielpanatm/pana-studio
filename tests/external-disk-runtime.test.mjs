import assert from "node:assert/strict";
import { test } from "node:test";

import {
  ensureNativeExternalDiskMonitoring,
  suspendAndDrainExternalDiskMonitoring,
} from "$lib/session/external-disk/monitor";
import {
  currentExternalDiskCheckLease,
  runExternalDiskCheck,
} from "$lib/session/external-disk/reconcile";
import { createExternalDiskSnapshot } from "$lib/session/external-disk/state";

function deferred() {
  let resolve;
  const promise = new Promise((resolvePromise) => { resolve = resolvePromise; });
  return { promise, resolve };
}

function manifest(versionToken = "v1") {
  return {
    root: "/project",
    files: [{
      relativePath: "templates/index.html",
      modifiedMs: versionToken === "v1" ? 1 : 2,
      size: 20,
      versionToken,
    }],
    truncated: false,
    maxFiles: 1_000,
  };
}

function project(baseline = manifest()) {
  return {
    root: "/project",
    kernelSessionId: "session:runtime",
    workspaceRevision: 4,
    acceptedDiskGeneration: 3,
    acceptedDiskManifest: baseline,
    previewBaseUrl: "http://127.0.0.1:1111",
    files: [],
  };
}

function createContext(options = {}) {
  const notifications = [];
  const events = [];
  const holder = {
    epoch: 2,
    project: project(),
    workspace: null,
    projectStatus: "",
    source: "old",
    sourceCache: { "scanned:templates/index.html": "old" },
    mutationEpoch: 4,
    selectionEpoch: 5,
    dirty: false,
    scssVariables: [],
    refreshToken: 0,
    jsRefreshToken: 0,
    transitionLocked: false,
    historyLocked: false,
    aiLocked: false,
    ...options,
  };
  const runtime = {
    snapshot: { ...createExternalDiskSnapshot(), baseline: holder.project.acceptedDiskManifest },
    auditTimer: null,
    watchUnlisten: null,
    watchGeneration: null,
    watchStopIdentity: null,
    watchRevision: 0,
    watchSubscriptionGeneration: 0,
    pendingWatchNotice: null,
    watchEventPending: false,
    watchEventDrainInFlight: false,
    suspended: false,
    checkInFlight: null,
    checkGeneration: 0,
    reconcileGeneration: 0,
  };
  const environment = {
    session: {
      get runtimeSessionId() { return "session:runtime"; },
      get epoch() { return holder.epoch; },
      get project() { return holder.project; },
      get transitionLocked() { return holder.transitionLocked; },
      get historyLocked() { return holder.historyLocked; },
      get aiLocked() { return holder.aiLocked; },
    },
    editor: {
      get activeScannedPath() { return "templates/index.html"; },
      get sourceCache() { return holder.sourceCache; },
      get mutationEpoch() { return holder.mutationEpoch; },
      get selectionEpoch() { return holder.selectionEpoch; },
      get dirty() { return holder.dirty; },
    },
    projections: {
      invalidateProjectSession() { holder.epoch += 1; },
      acceptProject(next) { holder.project = next; events.push("project"); },
      acceptWorkspace(next) { holder.workspace = next; events.push("workspace"); },
      setProjectStatus(status) { holder.projectStatus = status; },
      acceptSources(cache, activeSource) {
        holder.sourceCache = cache;
        if (activeSource !== null) holder.source = activeSource;
        events.push("sources");
      },
      acceptScssVariables(variables) {
        holder.scssVariables = variables;
        events.push("scss");
      },
      invalidateDerived() { holder.refreshToken += 1; events.push("derived"); },
      invalidatePageJs() { holder.jsRefreshToken += 1; events.push("page-js"); },
    },
    commands: {
      setStatus(text, kind) { events.push(`status:${kind}:${text}`); },
      escalateStatus(notification) { notifications.push(notification); },
      clearStatus(id) { events.push(`clear:${id}`); },
      async refreshSourceGraph() { events.push("source-graph"); },
      quiesceInteractions() { events.push("quiesce"); },
      async waitForInteractionLock() { events.push("interaction-lock"); },
      async resetHistory() { events.push("history"); },
      async projectLatestPreview() { events.push("preview"); return { status: "projected" }; },
    },
  };
  return {
    context: { runtime, environment },
    runtime,
    environment,
    holder,
    events,
    notifications,
  };
}

function watchNotice(revision) {
  return {
    schemaVersion: 1,
    projectRoot: "/project",
    runtimeSessionId: "session:runtime",
    watchGeneration: 7,
    watchRevision: revision,
    changedPaths: ["templates/index.html"],
    overflowed: false,
  };
}

test("watcher lifecycle coalesces revisions and releases its exact listener and generation", async () => {
  const fixture = createContext();
  const checkGate = deferred();
  const scheduled = [];
  const stopped = [];
  let listener = null;
  let unlistenCount = 0;
  let checks = 0;
  const port = {
    async subscribe(next) {
      listener = next;
      return () => { unlistenCount += 1; };
    },
    async startWatch() {
      return {
        projectRoot: "/project",
        runtimeSessionId: "session:runtime",
        watchGeneration: 7,
      };
    },
    async stopWatch(identity) { stopped.push(identity); },
    async runCheck() {
      checks += 1;
      if (checks === 1) await checkGate.promise;
    },
    schedule(operation, delayMs) {
      scheduled.push({ operation, delayMs });
      return scheduled.length;
    },
    clearSchedule() {},
  };

  await ensureNativeExternalDiskMonitoring(fixture.context, port);
  assert.equal(fixture.runtime.watchGeneration, 7);
  assert.equal(scheduled[0].delayMs, 5 * 60_000);

  listener(watchNotice(1));
  await Promise.resolve();
  listener(watchNotice(2));
  listener(watchNotice(3));
  checkGate.resolve();
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(checks, 2);
  assert.equal(fixture.runtime.watchRevision, 3);

  listener(watchNotice(2));
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(checks, 2);

  await suspendAndDrainExternalDiskMonitoring(fixture.context, port);
  assert.equal(unlistenCount, 1);
  assert.deepEqual(stopped, [{
    expectedProjectRoot: "/project",
    expectedSessionId: "session:runtime",
    expectedWatchGeneration: 7,
  }]);
  assert.equal(fixture.runtime.suspended, true);
});

test("watcher start failure releases the listener and schedules a bounded retry", async () => {
  const fixture = createContext();
  const scheduled = [];
  let unlistenCount = 0;
  const port = {
    async subscribe() { return () => { unlistenCount += 1; }; },
    async startWatch() { throw new Error("watch unavailable"); },
    async stopWatch() {},
    async runCheck() {},
    schedule(operation, delayMs) {
      scheduled.push({ operation, delayMs });
      return 41;
    },
    clearSchedule() {},
  };

  await ensureNativeExternalDiskMonitoring(fixture.context, port);
  assert.equal(unlistenCount, 1);
  assert.equal(fixture.runtime.watchUnlisten, null);
  assert.equal(fixture.runtime.watchGeneration, null);
  assert.equal(fixture.runtime.auditTimer, 41);
  assert.equal(scheduled[0].delayMs, 5 * 60_000);
  assert.match(fixture.holder.projectStatus, /watch unavailable/);
});

test("failed native stop retains the exact identity for the bounded retry", async () => {
  const fixture = createContext();
  const identity = {
    expectedProjectRoot: "/project",
    expectedSessionId: "session:runtime",
    expectedWatchGeneration: 7,
  };
  fixture.runtime.watchGeneration = 7;
  fixture.runtime.watchStopIdentity = identity;
  let unlistenCount = 0;
  fixture.runtime.watchUnlisten = () => { unlistenCount += 1; };
  let subscribed = 0;
  const port = {
    async subscribe() { subscribed += 1; return () => {}; },
    async startWatch() { throw new Error("must not start"); },
    async stopWatch() { throw new Error("stop unavailable"); },
    async runCheck() {},
    schedule(_operation, delayMs) {
      assert.equal(delayMs, 5 * 60_000);
      return 42;
    },
    clearSchedule() {},
  };

  await ensureNativeExternalDiskMonitoring(fixture.context, port);
  assert.equal(unlistenCount, 1);
  assert.equal(subscribed, 0);
  assert.deepEqual(fixture.runtime.watchStopIdentity, identity);
  assert.equal(fixture.runtime.watchGeneration, 7);
  assert.equal(fixture.runtime.auditTimer, 42);
  assert.match(fixture.holder.projectStatus, /stop unavailable/);
});

test("superseded watcher receipt cleans up only its own generation", async () => {
  const fixture = createContext();
  const stopped = [];
  let unlistenCount = 0;
  const port = {
    async subscribe() { return () => { unlistenCount += 1; }; },
    async startWatch() {
      fixture.runtime.suspended = true;
      return {
        projectRoot: "/project",
        runtimeSessionId: "session:runtime",
        watchGeneration: 9,
      };
    },
    async stopWatch(identity) { stopped.push(identity); },
    async runCheck() {},
    schedule() { return 1; },
    clearSchedule() {},
  };

  await ensureNativeExternalDiskMonitoring(fixture.context, port);
  assert.equal(unlistenCount, 1);
  assert.deepEqual(stopped, [{
    expectedProjectRoot: "/project",
    expectedSessionId: "session:runtime",
    expectedWatchGeneration: 9,
  }]);
  assert.equal(fixture.runtime.watchGeneration, null);
});

function reconcileReceipt(status, acceptedManifest, overrides = {}) {
  return {
    schemaVersion: 2,
    operationId: "external:1",
    sessionId: "session:runtime",
    projectRoot: "/project",
    status,
    verdictReason: status,
    startedAtMs: 1,
    completedAtMs: 2,
    requestedCount: 1,
    targetCount: 1,
    reconciledCount: status === "applied" ? 1 : 0,
    metadataRefreshedCount: 0,
    unchangedCount: status === "noop" ? 1 : 0,
    totalBytesRead: 20,
    requestedPaths: ["templates/index.html"],
    effectivePaths: ["templates/index.html"],
    invalidatedPaths: ["templates/index.html"],
    blockedPaths: [],
    reloadRequiredPaths: [],
    historyInvalidated: true,
    sourceGraphInvalidated: true,
    activeFile: {
      relativePath: "templates/index.html",
      text: "new",
    },
    acceptedDiskGeneration: 4,
    workspaceRevision: 5,
    acceptedManifest,
    projectionHints: {
      projectRescan: true,
      sourceGraph: true,
      preview: true,
      pageJs: true,
      scss: true,
      history: true,
      selection: true,
    },
    items: [],
    diagnostics: [],
    ...overrides,
  };
}

function reconcilePort(fixture, receipt) {
  return {
    async readManifest() { return receipt.acceptedManifest; },
    async reconcile() { fixture.events.push("rust-reconcile"); return receipt; },
    async readWorkspace() {
      return {
        projectRoot: "/project",
        runtimeSessionId: "session:runtime",
        revision: 5,
        dirty: false,
      };
    },
    async scan() {
      return {
        ...fixture.holder.project,
        workspaceRevision: 5,
        acceptedDiskGeneration: 4,
        acceptedDiskManifest: receipt.acceptedManifest,
      };
    },
    async readScssVariables() { return [{ name: "$accent", value: "red" }]; },
    async flushInputs() { fixture.events.push("flush"); },
    projectionDeadlineMs: 100,
  };
}

test("applied reconcile projects the exact Rust receipt and advances AcceptedDisk once", async () => {
  const fixture = createContext();
  const current = manifest("v2");
  const receipt = reconcileReceipt("applied", current);
  const lease = currentExternalDiskCheckLease(fixture.context);
  await runExternalDiskCheck(fixture.context, lease, reconcilePort(fixture, receipt));

  assert.equal(fixture.runtime.snapshot.changed, false);
  assert.equal(fixture.runtime.snapshot.baseline, current);
  assert.equal(fixture.holder.project.acceptedDiskGeneration, 4);
  assert.equal(fixture.holder.workspace.revision, 5);
  assert.equal(fixture.holder.source, "new");
  assert.equal(fixture.holder.sourceCache["scanned:templates/index.html"], "new");
  assert.equal(fixture.holder.scssVariables[0].name, "$accent");
  assert.equal(fixture.holder.refreshToken, 1);
  assert.equal(fixture.holder.jsRefreshToken, 1);
  for (const event of [
    "flush",
    "rust-reconcile",
    "workspace",
    "sources",
    "history",
    "source-graph",
    "scss",
    "derived",
    "page-js",
    "preview",
  ]) assert.ok(fixture.events.includes(event), event);
});

test("noop receipt advances the Rust baseline without running unrequested heavy projections", async () => {
  const fixture = createContext();
  const current = manifest("v2");
  const receipt = reconcileReceipt("noop", current, {
    historyInvalidated: false,
    sourceGraphInvalidated: false,
    projectionHints: {
      projectRescan: false,
      sourceGraph: false,
      preview: false,
      pageJs: false,
      scss: false,
      history: false,
      selection: false,
    },
  });
  await runExternalDiskCheck(
    fixture.context,
    currentExternalDiskCheckLease(fixture.context),
    reconcilePort(fixture, receipt),
  );

  assert.equal(fixture.runtime.snapshot.baseline, current);
  assert.equal(fixture.holder.project.acceptedDiskGeneration, 4);
  for (const event of ["history", "source-graph", "scss", "page-js", "preview"]) {
    assert.equal(fixture.events.includes(event), false, event);
  }
});

test("dirty, blocked and reload-required receipts remain conflict gates", async (t) => {
  await t.test("dirty session never reaches Rust reconcile", async () => {
    const fixture = createContext({ dirty: true });
    const current = manifest("v2");
    let reconciles = 0;
    const port = reconcilePort(fixture, reconcileReceipt("applied", current));
    port.reconcile = async () => { reconciles += 1; return reconcileReceipt("applied", current); };
    await runExternalDiskCheck(
      fixture.context,
      currentExternalDiskCheckLease(fixture.context),
      port,
    );
    assert.equal(reconciles, 0);
    assert.equal(fixture.runtime.snapshot.blockedByDirtySession, true);
    assert.equal(fixture.notifications[0].actionId, "external-disk.reload");
  });

  for (const status of ["blocked", "stale_evidence", "reload_required"]) {
    await t.test(status, async () => {
      const fixture = createContext();
      const current = manifest("v2");
      const receipt = reconcileReceipt(status, current, {
        acceptedDiskGeneration: null,
        workspaceRevision: null,
        acceptedManifest: null,
        historyInvalidated: false,
        sourceGraphInvalidated: false,
        projectionHints: {
          projectRescan: false,
          sourceGraph: false,
          preview: false,
          pageJs: false,
          scss: false,
          history: false,
          selection: false,
        },
      });
      const port = reconcilePort(fixture, { ...receipt, acceptedManifest: current });
      port.readManifest = async () => current;
      port.reconcile = async () => receipt;
      await runExternalDiskCheck(
        fixture.context,
        currentExternalDiskCheckLease(fixture.context),
        port,
      );
      assert.equal(fixture.runtime.snapshot.changed, true);
      assert.equal(
        fixture.runtime.snapshot.blockedByDirtySession,
        status !== "reload_required",
      );
      assert.equal(fixture.runtime.snapshot.baseline.files[0].versionToken, "v1");
      assert.equal(fixture.notifications[0].actionId, "external-disk.reload");
    });
  }
});

test("a projection failure after Rust commit enters recovery without advancing baseline", async () => {
  const fixture = createContext();
  const current = manifest("v2");
  fixture.environment.commands.resetHistory = async () => {
    throw new Error("history projection failed");
  };
  await runExternalDiskCheck(
    fixture.context,
    currentExternalDiskCheckLease(fixture.context),
    reconcilePort(fixture, reconcileReceipt("applied", current)),
  );

  assert.equal(fixture.runtime.snapshot.workspaceProjectionRecoveryRequired, true);
  assert.equal(fixture.runtime.snapshot.baseline.files[0].versionToken, "v1");
  assert.equal(fixture.notifications[0].actionId, "external-disk.reload");
  assert.match(fixture.notifications[0].message, /history projection failed/);
});

test("a concurrent UI lease after Rust commit blocks stale frontend projection", async () => {
  const fixture = createContext();
  const current = manifest("v2");
  const port = reconcilePort(fixture, reconcileReceipt("applied", current));
  port.reconcile = async () => {
    fixture.holder.selectionEpoch += 1;
    return reconcileReceipt("applied", current);
  };
  await runExternalDiskCheck(
    fixture.context,
    currentExternalDiskCheckLease(fixture.context),
    port,
  );

  assert.equal(fixture.runtime.snapshot.workspaceProjectionRecoveryRequired, true);
  assert.equal(fixture.holder.source, "old");
  assert.equal(fixture.notifications[0].actionId, "external-disk.reload");
});
