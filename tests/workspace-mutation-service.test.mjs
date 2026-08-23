import assert from "node:assert/strict";
import { test } from "node:test";
import { ProjectWorkspaceMutationService } from "$lib/session/workspace-mutation-service";

function snapshot(projectRoot, runtimeSessionId, revision) {
  return { projectRoot, runtimeSessionId, revision };
}

test("Workspace Mutation Service publică numai snapshoturi curente și monotone", () => {
  const host = {
    sessionProjectRoot: "/project",
    kernelProjectSessionId: "session-a",
    projectWorkspaceSnapshot: snapshot("/project", "session-a", 4),
  };
  const service = new ProjectWorkspaceMutationService(host);

  assert.equal(service.publishSnapshot(snapshot("/other", "session-a", 5)), false);
  assert.equal(service.publishSnapshot(snapshot("/project", "session-b", 5)), false);
  assert.equal(service.publishSnapshot(snapshot("/project", "session-a", 3)), false);
  assert.equal(host.projectWorkspaceSnapshot.revision, 4);

  const next = snapshot("/project", "session-a", 5);
  assert.equal(service.publishSnapshot(next), true);
  assert.equal(service.snapshot, next);
  assert.deepEqual(service.identity, {
    expectedProjectRoot: "/project",
    expectedSessionId: "session-a",
    expectedRevision: 5,
  });
});
