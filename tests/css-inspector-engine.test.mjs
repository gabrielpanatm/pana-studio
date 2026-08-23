import assert from "node:assert/strict";
import { test } from "node:test";
import { CssInspectorState } from "$lib/inspector/css-inspector-state.svelte";
import { CssInspectorReader } from "$lib/inspector/css-inspector-reader";
import { CssInspectorMutationQueue } from "$lib/inspector/css-inspector-mutation-queue";

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((accept, rejectPromise) => {
    resolve = accept;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function selection(revision, {
  projectRoot = "/project",
  runtimeSessionId = "session-a",
  memberId = "member:hero",
  editorNodeId = "editor:hero",
  sourceNodeId = "source:hero",
  renderInstanceId = "render:hero",
  selector = ".hero",
} = {}) {
  return {
    selectionRevision: revision,
    projectRoot,
    runtimeSessionId,
    canvasIdentity: { workspaceRevision: 7 },
    primaryMemberId: memberId,
    members: [{
      memberId,
      resolution: "resolved",
      subject: { kind: "htmlElement" },
      anchor: { editorNodeId, sourceNodeId, renderInstanceId },
      provenance: { definition: null, composition: null },
    }],
    focus: { kind: "cssRule", selector, file: "sass/site.scss" },
  };
}

function identityFrom(snapshot) {
  const member = snapshot.members[0];
  return {
    selectionRevision: snapshot.selectionRevision,
    workspaceRevision: snapshot.canvasIdentity.workspaceRevision,
    primaryMemberId: snapshot.primaryMemberId,
    members: [{
      memberId: member.memberId,
      editorNodeId: member.anchor.editorNodeId,
      sourceNodeId: member.anchor.sourceNodeId,
      renderInstanceId: member.anchor.renderInstanceId,
    }],
  };
}

function ruleContext(value = "red") {
  return {
    file: "sass/site.scss",
    selector: ".hero",
    viewport: "desktop",
    baseRules: [{ property: "color", value }],
    viewportRules: [{ property: "color", value }],
    hasViewportRule: true,
    background: {},
    grid: {},
  };
}

function resolution(revision, value = "red") {
  return {
    selectionRevision: revision,
    selector: ".hero",
    viewport: "desktop",
    state: "existing",
    target: {
      file: "sass/site.scss",
      selector: ".hero",
      targetKind: "existing",
      templatePath: "templates/index.html",
      pageOwned: false,
    },
    ruleContext: ruleContext(value),
  };
}

function readerInput(snapshot, overrides = {}) {
  return {
    projectRoot: snapshot.projectRoot,
    runtimeSessionId: snapshot.runtimeSessionId,
    targetCssFile: "sass/site.scss",
    cssSourceRevision: 0,
    activeRenderedTemplatePath: "templates/index.html",
    previewDevice: "desktop",
    refreshToken: 0,
    historyProjectionQuiesced: false,
    workspaceRevision: 7,
    htmlProjectionPending: false,
    selectionSnapshot: snapshot,
    selectionSummary: { classes: ["hero"] },
    presentedFocusSelector: ".hero",
    ...overrides,
  };
}

test("CSS reader is latest-wins and stale completions remain silent", async () => {
  const requests = [];
  const statuses = [];
  const state = new CssInspectorState();
  const reader = new CssInspectorReader(state, {
    resolve: () => {
      const request = deferred();
      requests.push(request);
      return request.promise;
    },
    reportStatus: (status) => statuses.push(status),
  });

  const firstSelection = selection(1);
  const first = reader.reconcile(readerInput(firstSelection));
  const focusedSelection = selection(2);
  const second = reader.reconcile(readerInput(focusedSelection));
  assert.equal(requests.length, 2);

  requests[0].resolve(resolution(1, "stale"));
  await first;
  assert.equal(state.resolution, null);
  requests[1].resolve(resolution(2, "current"));
  await second;

  assert.equal(state.resolution.selectionRevision, 2);
  assert.deepEqual(state.classRules, [{ property: "color", value: "current" }]);
  assert.deepEqual(statuses, []);
});

test("focus-only rebase retains the atomic projection, subject changes clear it", async () => {
  const requests = [];
  const state = new CssInspectorState();
  const reader = new CssInspectorReader(state, {
    resolve: () => {
      const request = deferred();
      requests.push(request);
      return request.promise;
    },
  });

  const firstSelection = selection(1);
  const first = reader.reconcile(readerInput(firstSelection));
  requests[0].resolve(resolution(1, "stable"));
  await first;

  const rebasedSelection = selection(2, {
    editorNodeId: "editor:hero:next",
    renderInstanceId: "render:hero:next",
  });
  const rebased = reader.reconcile(readerInput(rebasedSelection));
  assert.equal(state.loading, false);
  assert.deepEqual(state.classRules, [{ property: "color", value: "stable" }]);
  assert.equal(state.selectionIdentity.selectionRevision, 2);
  requests[1].resolve(resolution(2, "rebased"));
  await rebased;

  const otherSelection = selection(3, {
    memberId: "member:other",
    editorNodeId: "editor:other",
    sourceNodeId: "source:other",
    renderInstanceId: "render:other",
  });
  const other = reader.reconcile(readerInput(otherSelection));
  assert.equal(state.loading, true);
  assert.deepEqual(state.classRules, []);
  requests[2].resolve(resolution(3, "other"));
  await other;
});

test("session reset and disposal invalidate in-flight CSS reads", async () => {
  const requests = [];
  const state = new CssInspectorState();
  const reader = new CssInspectorReader(state, {
    resolve: () => {
      const request = deferred();
      requests.push(request);
      return request.promise;
    },
  });

  const oldRead = reader.reconcile(readerInput(selection(1)));
  const nextSelection = selection(1, { runtimeSessionId: "session-b" });
  const nextRead = reader.reconcile(readerInput(nextSelection));
  requests[0].resolve(resolution(1, "old-session"));
  await oldRead;
  assert.equal(state.resolution, null);

  reader.dispose();
  requests[1].resolve(resolution(1, "disposed"));
  await nextRead;
  assert.equal(state.resolution, null);
  assert.deepEqual(state.classRules, []);
});

test("history quiescence blocks reads and target switching stays selection-bound", async () => {
  const targetChanges = [];
  let resolveCalls = 0;
  const state = new CssInspectorState();
  const reader = new CssInspectorReader(state, {
    resolve: async () => {
      resolveCalls += 1;
      return {
        ...resolution(1),
        target: {
          ...resolution(1).target,
          file: "sass/other.scss",
        },
        ruleContext: {
          ...ruleContext(),
          file: "sass/other.scss",
        },
      };
    },
    changeCodeTarget: async (target) => {
      targetChanges.push(target);
      return true;
    },
  });
  const snapshot = selection(1);

  assert.equal(reader.reconcile(readerInput(snapshot, {
    historyProjectionQuiesced: true,
  })), null);
  assert.equal(resolveCalls, 0);
  await reader.reconcile(readerInput(snapshot));

  assert.equal(resolveCalls, 1);
  assert.deepEqual(targetChanges, [{
    selector: ".hero",
    file: "sass/other.scss",
    expectedSelectionRevision: 1,
  }]);
  assert.equal(state.resolution, null);
});

function queueHarness(overrides = {}) {
  const snapshot = selection(1);
  const expectedSelection = identityFrom(snapshot);
  const state = new CssInspectorState();
  state.syncPresentation(".hero", "desktop", "sass/site.scss");
  state.applyResolution(resolution(1), expectedSelection);
  const context = {
    projectRoot: "/project",
    runtimeSessionId: "session-a",
    targetCssFile: "sass/site.scss",
    previewDevice: "desktop",
  };
  const events = [];
  let liveEpoch = 0;
  const queue = new CssInspectorMutationQueue({
    state,
    context: () => context,
    captureSelection: () => expectedSelection,
    flushDraftSync: async () => { events.push("flush-draft"); },
    mutateExisting: async (request) => {
      events.push({ kind: "mutate", request });
      return { authority: { operationId: "css-1" } };
    },
    applyLiveProperties: (_selector, properties) => {
      liveEpoch += 1;
      events.push({ kind: "live", epoch: liveEpoch, properties });
      return liveEpoch;
    },
    projectCommittedMutation: async (_authority, epoch) => {
      events.push({ kind: "committed", epoch });
    },
    rejectLiveProperties: (epoch) => events.push({ kind: "rejected", epoch }),
    reportStatus: (status) => events.push({ kind: "status", status }),
    setPending: (pending) => events.push({ kind: "pending", pending }),
    ...overrides,
  });
  queue.syncSession(context.projectRoot, context.runtimeSessionId);
  return { queue, state, context, events, expectedSelection };
}

test("CSS queue batches multiple properties and serializes concurrent flushes", async () => {
  const gate = deferred();
  const started = deferred();
  const mutations = [];
  const { queue, events } = queueHarness({
    mutateExisting: async (request) => {
      mutations.push(request);
      started.resolve();
      await gate.promise;
      return { authority: { operationId: "css-batch" } };
    },
  });

  queue.edit.draft("color", "red");
  queue.edit.draft("background-color", "black");
  assert.equal(queue.stagedCount, 1);
  const firstFlush = queue.flush();
  const concurrentFlush = queue.flush();
  await started.promise;
  assert.equal(mutations.length, 1);
  assert.deepEqual(mutations[0].properties, {
    color: "red",
    "background-color": "black",
  });

  gate.resolve();
  await Promise.all([firstFlush, concurrentFlush]);
  assert.equal(events.filter((event) => event === "flush-draft").length, 1);
  assert.equal(events.filter((event) => event.kind === "committed").length, 1);
  assert.equal(queue.stagedCount, 0);
  assert.equal(queue.queuedCount, 0);
});

test("CSS cancellation restores the baseline and rejects only its live epoch", async () => {
  const { queue, state, events } = queueHarness();
  state.replacePendingValues({ color: "baseline" });

  queue.edit.draft("color", "draft");
  queue.edit.cancel("color");
  await Promise.resolve();
  await Promise.resolve();

  assert.deepEqual(state.pendingValues, { color: "baseline" });
  assert.equal(queue.stagedCount, 0);
  assert.deepEqual(
    events.filter((event) => event.kind === "rejected").map((event) => event.epoch),
    [2],
  );
  assert.equal(events.some((event) => event.kind === "mutate"), false);
});

test("CSS queue reports a failure, rejects its epoch and recovers on the next write", async () => {
  let attempts = 0;
  const rejected = [];
  const { queue } = queueHarness({
    mutateExisting: async () => {
      attempts += 1;
      if (attempts === 1) throw new Error("write failed");
      return { authority: { operationId: "css-recovered" } };
    },
    rejectLiveProperties: (epoch) => rejected.push(epoch),
  });

  queue.edit.draft("color", "red");
  await queue.flush();
  assert.equal(queue.failure, "write failed");
  await assert.rejects(queue.flushForRegistry(), /write failed/);
  assert.deepEqual(rejected, [1]);

  queue.edit.draft("color", "blue");
  await queue.flushForRegistry();
  assert.equal(queue.failure, "");
  assert.equal(attempts, 2);
});

test("session reset and disposal suppress stale epochs and scheduled writes", async () => {
  const mutation = deferred();
  const started = deferred();
  const rejected = [];
  let mutationCalls = 0;
  const { queue, context } = queueHarness({
    mutateExisting: async () => {
      mutationCalls += 1;
      started.resolve();
      return await mutation.promise;
    },
    rejectLiveProperties: (epoch) => rejected.push(epoch),
  });

  queue.edit.draft("color", "red");
  const flushing = queue.flush();
  await started.promise;
  context.runtimeSessionId = "session-b";
  queue.syncSession(context.projectRoot, context.runtimeSessionId);
  mutation.reject(new Error("old session failed"));
  await flushing;
  assert.deepEqual(rejected, []);
  assert.equal(queue.queuedCount, 0);

  queue.edit.draft("color", "blue");
  queue.edit.commit("color");
  queue.dispose();
  await Promise.resolve();
  await Promise.resolve();
  assert.equal(mutationCalls, 1);
});
