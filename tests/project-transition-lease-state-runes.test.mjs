import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import ts from "typescript";
import { compileModule } from "svelte/compiler";

const sourceUrl = new URL(
  "../src/lib/project/transition-lease-state.svelte.ts",
  import.meta.url,
);

async function loadCompiledLeaseState() {
  const source = await readFile(sourceUrl, "utf8");
  const stripped = ts.transpileModule(source, {
    compilerOptions: {
      target: ts.ScriptTarget.ESNext,
      module: ts.ModuleKind.ESNext,
    },
  }).outputText;
  let compiled = compileModule(stripped, {
    filename: "project-transition-lease-state.svelte.js",
    generate: "client",
    dev: true,
  }).js.code;

  for (const specifier of ["svelte/internal/client", "svelte"]) {
    const resolved = JSON.stringify(import.meta.resolve(specifier));
    compiled = compiled
      .replaceAll(`"${specifier}"`, resolved)
      .replaceAll(`'${specifier}'`, resolved);
  }

  const moduleUrl = `data:text/javascript;base64,${Buffer.from(compiled).toString("base64")}`;
  return await import(moduleUrl);
}

const compiledLeaseState = loadCompiledLeaseState();

function harness(ProjectTransitionLeaseState, overrides = {}) {
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

test("lease-ul compilat cu runes păstrează identitatea și pornește reattach", async () => {
  const { ProjectTransitionLeaseState } = await compiledLeaseState;
  const { state, events } = harness(ProjectTransitionLeaseState);

  const result = await state.run(
    { kind: "reattach", owner: "project-transition-controller" },
    async (lease) => {
      assert.equal(state.active, lease);
      events.push("reattach-operation");
      return "reattached";
    },
  );

  assert.equal(result, "reattached");
  assert.equal(state.active, null);
  assert.equal(state.isActive, false);
  assert.deepEqual(events, [
    "cancel-drafts",
    "invalidate-preview",
    "invalidate-source-graph",
    "quiesce-interactions",
    "drain-save",
    "suspend-disk",
    "reattach-operation",
    "recover-disk",
    "resume-disk",
  ]);
});

test("lease-ul compilat cu runes se eliberează după eșecul operației", async () => {
  const { ProjectTransitionLeaseState } = await compiledLeaseState;
  const { state, events } = harness(ProjectTransitionLeaseState);

  await assert.rejects(
    state.run(
      { kind: "reattach", owner: "project-transition-controller" },
      async (lease) => {
        assert.equal(state.active, lease);
        events.push("reattach-operation");
        throw new Error("reattach failed");
      },
    ),
    /reattach failed/,
  );

  assert.equal(state.active, null);
  assert.equal(state.isActive, false);
  assert.deepEqual(events.slice(-3), [
    "reattach-operation",
    "recover-disk",
    "resume-disk",
  ]);
});
