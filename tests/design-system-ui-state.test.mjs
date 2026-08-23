import assert from "node:assert/strict";
import test from "node:test";
import { DesignTokenCatalogState } from "../src/lib/components/creation/design-system/catalog-state.svelte.ts";
import { FontManagerState } from "../src/lib/fonts/manager-state.svelte.ts";

function deferred() {
  let resolve;
  const promise = new Promise((accept) => { resolve = accept; });
  return { promise, resolve };
}

test("catalogul deduplică requestul și păstrează numai revizia curentă", async () => {
  const authority = { projectRoot: "/project", runtimeSessionId: "session-1", workspaceRevision: 1 };
  const requests = [];
  const state = new DesignTokenCatalogState(() => authority, (...args) => {
    const request = deferred();
    requests.push({ args, request });
    return request.promise;
  });

  const first = state.refresh();
  const duplicate = state.refresh();
  assert.equal(requests.length, 1);
  requests[0].request.resolve({ projectRoot: "/project", runtimeSessionId: "session-1", workspaceRevision: 1, categories: [], tokens: [] });
  assert.equal(await first, await duplicate);
  assert.equal(state.snapshot.workspaceRevision, 1);

  authority.workspaceRevision = 2;
  const stale = state.refresh();
  authority.workspaceRevision = 3;
  requests[1].request.resolve({ projectRoot: "/project", runtimeSessionId: "session-1", workspaceRevision: 2, categories: [], tokens: [] });
  assert.equal(await stale, null);
  assert.equal(state.snapshot.workspaceRevision, 1);

  const current = state.refresh();
  requests[2].request.resolve({ projectRoot: "/project", runtimeSessionId: "session-1", workspaceRevision: 3, categories: [], tokens: [] });
  await current;
  assert.equal(state.snapshot.workspaceRevision, 3);
});

test("FontManager deduplică și respinge răspunsul unei identități înlocuite", async () => {
  const identity = { expectedProjectRoot: "/project", expectedSessionId: "session-1", expectedRevision: 1 };
  const requests = [];
  const state = new FontManagerState(() => identity, () => {
    const request = deferred();
    requests.push(request);
    return request.promise;
  });

  const first = state.refresh();
  const duplicate = state.refresh();
  assert.equal(requests.length, 1);
  const snapshot = { graph: { families: [] }, roles: [], diagnostics: [] };
  requests[0].resolve(snapshot);
  assert.equal(await first, snapshot);
  assert.equal(await duplicate, snapshot);

  identity.expectedRevision = 2;
  const stale = state.refresh();
  identity.expectedSessionId = "session-2";
  requests[1].resolve({ graph: { families: [{ id: "stale" }] }, roles: [], diagnostics: [] });
  assert.equal(await stale, null);
  assert.equal(state.snapshot, snapshot);
});
