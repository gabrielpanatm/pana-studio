import assert from "node:assert/strict";
import { test } from "node:test";
import { CssInspectorState } from "$lib/inspector/css-inspector-state.svelte";
import { CssInspectorReader } from "$lib/inspector/css-inspector-reader";
import { CssInspectorMutationQueue } from "$lib/inspector/css-inspector-mutation-queue";
import { CssEditSessionCoordinator } from "$lib/inspector/css-edit-session-coordinator";
import {
  CssInspectorTransientReadError,
  CssMutationTransientError,
} from "$lib/css/io";

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

function authority(operationId, revisionBefore = 7, revisionAfter = revisionBefore + 1) {
  return {
    schemaVersion: 2,
    operationId,
    status: revisionAfter === revisionBefore ? "noop" : "staged",
    projectRoot: "/project",
    sessionId: "session-a",
    revisionBefore,
    revisionAfter,
    dirty: true,
    touchedFiles: [],
    writtenFiles: [],
    removedFiles: [],
    documents: [],
    workspaceMutation: null,
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

test("same-target reads keep the latest optimistic value until its canonical revision arrives", async () => {
  let calls = 0;
  const state = new CssInspectorState();
  const editSession = new CssEditSessionCoordinator();
  const reader = new CssInspectorReader(state, {
    editSession,
    resolve: async () => {
      calls += 1;
      if (calls === 1) return resolution(1, "red");
      return resolution(1, "blue");
    },
    getOpenContext: () => null,
  });
  const snapshot = selection(1);

  await reader.reconcile(readerInput(snapshot));
  state.replacePendingValues({ color: "green" });
  const first = editSession.beginGesture({
    projectRoot: "/project",
    runtimeSessionId: "session-a",
    selector: ".hero",
    file: "sass/site.scss",
    viewport: "desktop",
    expectedSelection: identityFrom(snapshot),
  }, "color");
  editSession.finishGesture();
  editSession.acceptAuthority(first.interactionId, authority("css:first", 7, 8));
  assert.equal(reader.reconcile(readerInput(snapshot, {
    workspaceRevision: 8,
  })), null);
  assert.deepEqual(state.pendingValues, { color: "green" });

  const second = editSession.beginGesture({
    projectRoot: "/project",
    runtimeSessionId: "session-a",
    selector: ".hero",
    file: "sass/site.scss",
    viewport: "desktop",
    expectedSelection: identityFrom(snapshot),
  }, "color");
  editSession.finishGesture();
  editSession.acceptAuthority(second.interactionId, authority("css:second", 8, 9));
  state.replacePendingValues({ color: "blue" });
  const canonicalEight = selection(2);
  canonicalEight.canvasIdentity.workspaceRevision = 8;
  assert.equal(reader.reconcile(readerInput(canonicalEight, {
    workspaceRevision: 9,
  })), null);
  assert.deepEqual(state.pendingValues, { color: "blue" });

  const canonicalNine = selection(3);
  canonicalNine.canvasIdentity.workspaceRevision = 9;
  await reader.reconcile(readerInput(canonicalNine, { workspaceRevision: 9 }));
  assert.deepEqual(state.classRules, [{ property: "color", value: "blue" }]);
  assert.deepEqual(state.pendingValues, {});
});

test("a retryable stale CSS read remains silent and rearms the canonical projection", async () => {
  let calls = 0;
  const statuses = [];
  const state = new CssInspectorState();
  const reader = new CssInspectorReader(state, {
    resolve: async () => {
      calls += 1;
      if (calls === 1) throw new CssInspectorTransientReadError({
        kind: "retryableStale",
        interactionId: "css-edit:test",
        reason: "canonical_authority_pending",
        message: "stale selection revision",
      });
      return resolution(1, "recovered");
    },
    reportStatus: (status) => statuses.push(status),
  });
  const input = readerInput(selection(1));

  await reader.reconcile(input);
  assert.equal(state.resolution, null);
  assert.equal(calls, 1);
  assert.deepEqual(statuses, []);

  await reader.reconcile(input);
  assert.equal(calls, 2);
  assert.equal(state.resolution.selectionRevision, 1);
  assert.deepEqual(state.classRules, [{ property: "color", value: "recovered" }]);
  assert.equal(state.hasEditableTarget, true);
});

test("class selection publishes its Rust result while the focus confirmation read is in flight", async () => {
  const confirmationRead = deferred();
  let resolveCalls = 0;
  let pendingConfirmation = null;
  const state = new CssInspectorState();
  let reader;
  reader = new CssInspectorReader(state, {
    resolve: async () => {
      resolveCalls += 1;
      if (resolveCalls === 1) return resolution(1, "selected");
      return confirmationRead.promise;
    },
    changeCodeTarget: async (target) => {
      const focusedSelection = selection(2, { selector: target.selector });
      pendingConfirmation = reader.reconcile(readerInput(focusedSelection, {
        presentedFocusSelector: target.selector,
      }));
      return true;
    },
  });
  const initialSelection = selection(1);

  assert.equal(reader.reconcile(readerInput(initialSelection, {
    presentedFocusSelector: null,
  })), null);
  assert.equal(await reader.selectClass("hero"), "allowed");
  assert.equal(resolveCalls, 2);
  assert.equal(state.hasEditableTarget, true);
  assert.equal(state.selectionIdentity.selectionRevision, 2);
  assert.deepEqual(state.classRules, [{ property: "color", value: "selected" }]);

  confirmationRead.resolve(resolution(2, "confirmed"));
  await pendingConfirmation;
  assert.equal(state.hasEditableTarget, true);
  assert.deepEqual(state.classRules, [{ property: "color", value: "confirmed" }]);
});

test("class selection recovers when the canonical CSS focus was already exact", async () => {
  let resolveCalls = 0;
  const state = new CssInspectorState();
  const reader = new CssInspectorReader(state, {
    resolve: async () => {
      resolveCalls += 1;
      if (resolveCalls === 1) throw new Error("stale selection revision");
      return resolution(1, "recovered");
    },
    changeCodeTarget: async () => true,
  });
  const input = readerInput(selection(1));

  await reader.reconcile(input);
  assert.equal(state.resolution, null);
  assert.equal(await reader.selectClass("hero"), "allowed");
  assert.equal(resolveCalls, 2);
  assert.equal(state.hasEditableTarget, true);
  assert.deepEqual(state.classRules, [{ property: "color", value: "recovered" }]);
});

test("class selection requested during CSS canonicalization retries without a second click", async () => {
  const editSession = new CssEditSessionCoordinator();
  const state = new CssInspectorState();
  let resolveCalls = 0;
  const reader = new CssInspectorReader(state, {
    editSession,
    resolve: async () => {
      resolveCalls += 1;
      return {
        ...resolution(2, "blue"),
        selector: ".other",
        target: { ...resolution(2).target, selector: ".other" },
        ruleContext: { ...ruleContext("blue"), selector: ".other" },
      };
    },
    changeCodeTarget: async () => true,
  });
  const staleCanvas = selection(1);
  state.syncPresentation(".hero", "desktop", "sass/site.scss");
  state.applyResolution(resolution(1), identityFrom(staleCanvas));
  reader.reconcile(readerInput(staleCanvas, { workspaceRevision: 8 }));

  assert.equal(await reader.selectClass("other"), "blocked");
  assert.equal(resolveCalls, 0);

  const canonicalCanvas = selection(2);
  canonicalCanvas.canvasIdentity.workspaceRevision = 8;
  await reader.reconcile(readerInput(canonicalCanvas, { workspaceRevision: 8 }));

  assert.equal(resolveCalls, 1);
  assert.equal(state.resolution.selector, ".other");
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
    expectedSelection: identityFrom(snapshot),
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
      return { authority: authority("css-1") };
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
      return { authority: authority("css-batch") };
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

test("two CSS commits 150 ms apart keep distinct interactions and the latest value", async () => {
  const mutations = [];
  const { queue, state, events } = queueHarness({
    mutateExisting: async (request) => {
      mutations.push(request);
      const sequence = mutations.length;
      return { authority: authority(`css:${sequence}`, 6 + sequence, 7 + sequence) };
    },
  });
  const color = queue.edit.continuous("color");

  color.oninput("#110000");
  color.oncommit();
  await queue.flush();
  await new Promise((resolve) => setTimeout(resolve, 150));
  color.oninput("#220000");
  color.oncommit();
  await queue.flush();

  assert.equal(mutations.length, 2);
  assert.notEqual(mutations[0].interactionId, mutations[1].interactionId);
  assert.deepEqual(mutations.map((request) => request.properties.color), [
    "#110000",
    "#220000",
  ]);
  assert.equal(state.pendingValues.color, "#220000");
  assert.equal(events.filter((event) => event.kind === "committed").length, 2);
});

test("a superseded Inspector target still projects its already committed workspace authority", async () => {
  const gate = deferred();
  const started = deferred();
  const editSession = new CssEditSessionCoordinator();
  const { queue, events, expectedSelection } = queueHarness({
    editSession,
    mutateExisting: async () => {
      started.resolve();
      await gate.promise;
      return { authority: authority("css:superseded") };
    },
  });

  queue.edit.draft("color", "red");
  const flushing = queue.flush();
  await started.promise;
  editSession.beginGesture({
    projectRoot: "/project",
    runtimeSessionId: "session-a",
    selector: ".other",
    file: "sass/site.scss",
    viewport: "desktop",
    expectedSelection,
  }, "color");
  gate.resolve();
  await flushing;

  assert.equal(events.filter((event) => event.kind === "committed").length, 1);
  assert.equal(events.some((event) => (
    event.kind === "status" && event.status.kind === "saved"
  )), false);
});

test("a superseded CSS mutation settles silently and removes only its live projection", async () => {
  const { queue, events } = queueHarness({
    mutateExisting: async (request) => {
      throw new CssMutationTransientError({
        kind: "superseded",
        interactionId: request.interactionId,
        reason: "selection_anchor_superseded",
        message: "selection changed",
      });
    },
  });

  queue.edit.draft("color", "red");
  queue.edit.commit("color");
  await queue.flush();

  assert.equal(queue.failure, "");
  assert.equal(events.filter((event) => event.kind === "rejected").length, 1);
  assert.equal(events.some((event) => (
    event.kind === "status" && event.status.kind === "mutationFailed"
  )), false);
});

test("CSS queue reports an unavailable canonical target instead of dropping the draft silently", () => {
  const { queue, state, events } = queueHarness();
  state.resetProjection(true);
  events.length = 0;

  queue.edit.draft("color", "#af0b0b");

  assert.equal(queue.stagedCount, 0);
  assert.deepEqual(events, [{
    kind: "status",
    status: { kind: "targetUnavailable", property: "color", interactionId: null },
  }]);
});

test("continuous CSS drafts keep one edit interaction without mutating selection focus", async () => {
  const focusTargets = [];
  const { queue, events } = queueHarness({
    changeCodeTarget: (target) => {
      focusTargets.push(target);
      return true;
    },
  });
  const color = queue.edit.continuous("color");

  color.oninput("#110000");
  color.oninput("#220000");
  color.oninput("#330000");

  assert.equal(focusTargets.length, 0);
  assert.equal(
    events.filter((event) => (
      event.kind === "status" && event.status.kind === "previewChanged"
    )).length,
    1,
  );
  assert.deepEqual(
    events.filter((event) => event.kind === "live").map((event) => event.properties.color),
    ["#110000", "#220000", "#330000"],
  );

  color.oncommit("#330000");
  await queue.flush();
  color.oninput("#440000");

  assert.equal(focusTargets.length, 0);
  assert.equal(
    events.filter((event) => (
      event.kind === "status" && event.status.kind === "previewChanged"
    )).length,
    2,
  );
});

test("an unavailable CSS target reports once during a continuous gesture", () => {
  const { queue, state, events } = queueHarness();
  state.resetProjection(true);
  events.length = 0;
  const color = queue.edit.continuous("color");

  color.oninput("#110000");
  color.oninput("#220000");
  color.oninput("#330000");

  assert.equal(
    events.filter((event) => (
      event.kind === "status" && event.status.kind === "targetUnavailable"
    )).length,
    1,
  );
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
      return { authority: authority("css-recovered", 8, 9) };
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
