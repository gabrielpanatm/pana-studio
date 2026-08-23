import assert from "node:assert/strict";
import { test } from "node:test";
import { ProjectTransitionLeaseState } from "$lib/project/transition-lease-state.svelte";
import { ProjectTransitionFrontendLeaseBusyError } from "$lib/state/project-transition-frontend-lease";

function deferred() {
  let resolve;
  const promise = new Promise((resolvePromise) => { resolve = resolvePromise; });
  return { promise, resolve };
}

function harness(overrides = {}) {
  const events = [];
  const state = new ProjectTransitionLeaseState({
    guards: () => ({
      aiEditLocked: false,
      aiRecoveryReloadAuthorized: false,
      historyLocked: false,
    }),
    cancelEditorDrafts: () => events.push("cancel-drafts"),
    invalidatePreview: () => events.push("invalidate-preview"),
    invalidateSourceGraph: () => events.push("invalidate-source-graph"),
    quiesceInteractions: () => events.push("quiesce-interactions"),
    drainActiveSave: async () => { events.push("drain-save"); },
    suspendExternalDisk: async () => { events.push("suspend-disk"); },
    recoverExternalDiskAfterFailure: () => events.push("recover-disk"),
    resumeExternalDisk: () => events.push("resume-disk"),
    ...overrides,
  });
  return { state, events };
}

test("ownerul de tranziție refuză suprapunerea și eliberează toți waiterii", async () => {
  const { state, events } = harness();
  const operationStarted = deferred();
  const operationGate = deferred();

  const opening = state.run(
    { kind: "open", owner: "project-transition-controller" },
    async () => {
      events.push("open-operation");
      operationStarted.resolve();
      await operationGate.promise;
      return "opened";
    },
  );
  await operationStarted.promise;
  assert.equal(state.isActive, true);

  let reloadEffects = 0;
  await assert.rejects(
    state.run(
      { kind: "reload", owner: "project-transition-controller" },
      async () => { reloadEffects += 1; },
    ),
    ProjectTransitionFrontendLeaseBusyError,
  );
  assert.equal(reloadEffects, 0);

  let idleResolved = false;
  const idle = state.waitForIdle().then(() => { idleResolved = true; });
  await Promise.resolve();
  assert.equal(idleResolved, false);
  operationGate.resolve();

  assert.equal(await opening, "opened");
  await idle;
  assert.equal(state.isActive, false);
  assert.equal(events.at(-2), "recover-disk");
  assert.equal(events.at(-1), "resume-disk");
});

test("eșecul quiesce eliberează lease-ul și reia monitorizarea", async () => {
  const { state, events } = harness({
    suspendExternalDisk: async () => {
      events.push("suspend-failed");
      throw new Error("disk drain failed");
    },
  });

  await assert.rejects(
    state.run(
      { kind: "close", owner: "project-transition-controller" },
      async () => assert.fail("operația nu trebuie pornită"),
    ),
    /disk drain failed/,
  );

  assert.equal(state.isActive, false);
  assert.deepEqual(events.slice(-3), [
    "suspend-failed",
    "recover-disk",
    "resume-disk",
  ]);
});

test("guard-urile AI și history blochează înainte de acquire", async () => {
  for (const guards of [
    { aiEditLocked: true, aiRecoveryReloadAuthorized: false, historyLocked: false },
    { aiEditLocked: false, aiRecoveryReloadAuthorized: false, historyLocked: true },
  ]) {
    const { state, events } = harness({ guards: () => guards });
    await assert.rejects(
      state.run(
        { kind: "reload", owner: "project-transition-controller" },
        async () => assert.fail("operația nu trebuie pornită"),
      ),
    );
    assert.equal(state.isActive, false);
    assert.deepEqual(events, []);
  }
});
