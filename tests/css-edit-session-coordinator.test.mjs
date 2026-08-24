import assert from "node:assert/strict";
import { test } from "node:test";
import { CssEditSessionCoordinator } from "$lib/inspector/css-edit-session-coordinator";
import { projectWorkspaceDirtyStatusKey } from "$lib/status/global-status";

function selection(selectionRevision, workspaceRevision = 7, overrides = {}) {
  return {
    selectionRevision,
    workspaceRevision,
    primaryMemberId: "member:hero",
    members: [{
      memberId: "member:hero",
      editorNodeId: "editor:hero",
      sourceNodeId: "source:hero",
      renderInstanceId: "render:hero",
    }],
    ...overrides,
  };
}

function target(overrides = {}) {
  return {
    projectRoot: "/project",
    runtimeSessionId: "session:a",
    selector: ".hero",
    file: "sass/site.scss",
    viewport: "desktop",
    expectedSelection: selection(5),
    ...overrides,
  };
}

function authority(revisionAfter, operationId = `workspace:${revisionAfter}`) {
  return {
    operationId,
    projectRoot: "/project",
    sessionId: "session:a",
    revisionBefore: revisionAfter - 1,
    revisionAfter,
  };
}

test("a CSS edit session retains the live projection until Workspace and Canvas are canonical", () => {
  const coordinator = new CssEditSessionCoordinator();
  const gesture = coordinator.beginGesture(target(), "font-weight");

  assert.equal(gesture.started, true);
  assert.match(gesture.interactionId, /^css-edit:/);
  assert.equal(coordinator.acceptAuthority(gesture.interactionId, authority(8)).kind, "applied");
  assert.deepEqual(coordinator.readDecision({
    ...target(),
    workspaceRevision: 8,
    selectionWorkspaceRevision: 7,
  }), {
    kind: "retain",
    reason: "awaitingCanonicalPreview",
    expectedWorkspaceRevision: 8,
  });

  assert.deepEqual(coordinator.readDecision({
    ...target({ expectedSelection: selection(6, 8) }),
    workspaceRevision: 8,
    selectionWorkspaceRevision: 8,
  }), { kind: "read", canonicalWorkspaceRevision: 8 });
  assert.equal(coordinator.snapshot.pendingWorkspaceRevision, null);
  assert.equal(coordinator.snapshot.canonicalWorkspaceRevision, 8);
});

test("rapid CSS commits supersede intermediate canonical reads without losing the latest revision", () => {
  const coordinator = new CssEditSessionCoordinator();
  const first = coordinator.beginGesture(target(), "font-weight");
  coordinator.finishGesture();
  const second = coordinator.beginGesture(target(), "font-weight");
  coordinator.finishGesture();
  coordinator.acceptAuthority(first.interactionId, authority(8, "css:first"));
  assert.equal(coordinator.readDecision({
    ...target({ expectedSelection: selection(6, 8) }),
    workspaceRevision: 8,
    selectionWorkspaceRevision: 8,
  }).kind, "retain");
  coordinator.acceptAuthority(second.interactionId, authority(9, "css:second"));

  assert.notEqual(first.interactionId, second.interactionId);
  assert.equal(coordinator.snapshot.pendingWorkspaceRevision, 9);
  assert.equal(coordinator.snapshot.pendingTransactionId, "css:second");
  assert.equal(coordinator.readDecision({
    ...target({ expectedSelection: selection(6, 8) }),
    workspaceRevision: 9,
    selectionWorkspaceRevision: 8,
  }).kind, "retain");
  assert.deepEqual(coordinator.readDecision({
    ...target({ expectedSelection: selection(7, 9) }),
    workspaceRevision: 9,
    selectionWorkspaceRevision: 9,
  }), { kind: "read", canonicalWorkspaceRevision: 9 });
});

test("continuous picker input owns one interaction and a later gesture receives a new identity", () => {
  const coordinator = new CssEditSessionCoordinator();
  const first = coordinator.beginGesture(target(), "color");
  const second = coordinator.beginGesture(target(), "color");

  assert.equal(first.started, true);
  assert.equal(second.started, false);
  assert.equal(second.interactionId, first.interactionId);

  coordinator.finishGesture();
  const next = coordinator.beginGesture(target(), "color");
  assert.equal(next.started, true);
  assert.notEqual(next.interactionId, first.interactionId);
});

test("project or semantic target changes invalidate the previous CSS edit session", () => {
  const coordinator = new CssEditSessionCoordinator();
  const first = coordinator.beginGesture(target(), "color");
  coordinator.acceptAuthority(first.interactionId, authority(8));

  const other = coordinator.beginGesture(target({
    selector: ".subtitle",
    expectedSelection: selection(6, 8, {
      primaryMemberId: "member:subtitle",
      members: [{
        memberId: "member:subtitle",
        editorNodeId: "editor:subtitle",
        sourceNodeId: "source:subtitle",
        renderInstanceId: "render:subtitle",
      }],
    }),
  }), "color");

  assert.notEqual(other.interactionId, first.interactionId);
  assert.equal(coordinator.snapshot.pendingWorkspaceRevision, null);
  assert.equal(coordinator.acceptAuthority(first.interactionId, authority(9)).kind, "superseded");

  coordinator.syncRuntime("/other-project", "session:b");
  assert.equal(coordinator.snapshot, null);
});

test("CSS operation status lanes correlate only events from the same interaction", () => {
  const coordinator = new CssEditSessionCoordinator();
  const first = coordinator.beginGesture(target(), "color");
  coordinator.finishGesture();
  const second = coordinator.beginGesture(target(), "font-weight");

  const firstLane = coordinator.statusOptions(first.interactionId, "preview");
  const secondLane = coordinator.statusOptions(second.interactionId, "preview");
  const errorLane = coordinator.statusOptions(second.interactionId, "error");
  assert.notEqual(firstLane.dedupeKey, secondLane.dedupeKey);
  assert.equal(firstLane.source, "css-inspector");
  assert.equal(firstLane.resolutionKey, secondLane.resolutionKey);
  assert.equal(
    firstLane.resolutionKey,
    projectWorkspaceDirtyStatusKey("/project", "session:a"),
  );
  assert.equal(errorLane.resolutionKey, errorLane.dedupeKey);
});
