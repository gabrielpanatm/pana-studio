import assert from "node:assert/strict";
import { test } from "node:test";
import { WorkspaceAuthorityService } from "$lib/session/workspace-authority-service";

function workspace(revision) {
  return {
    projectRoot: "/project",
    runtimeSessionId: "session:a",
    revision,
    dirty: true,
  };
}

function serviceFixture() {
  const project = {
    root: "/project",
    runtimeSessionId: "session:a",
    project: {},
    workspace: workspace(16),
    epoch: 3,
  };
  const documents = {
    activeScannedPath: "templates/index.html",
    templateActive: false,
  };
  const source = {
    source: "revizia 16",
    sourceCache: { "templates/index.html": "revizia 16" },
  };
  const previewWorkspace = {
    structuralWriteBoundaryActive: true,
    structuralWriteBoundaryResumesMonitoring: false,
    workspaceRevision: "preview-16",
    pendingProjection: null,
    setPendingProjection(plan) { this.pendingProjection = plan; },
    canProjectWorkspacePreview: () => true,
    deferSurfaceProjection() {},
    async requestRefresh() { return true; },
    async requestWorkspaceProjectionRefresh() { return true; },
    async applyCanvasPatch() { return true; },
    async rollbackCanvasPatch() { return true; },
  };
  const locks = {
    transition: { isActive: false },
    history: { leaseActive: false },
    ai: { frontendLockActive: false },
  };
  const reconciledRevisions = [];
  const instance = new WorkspaceAuthorityService({
    session: { project, documents, source, analysis: {} },
    preview: { surface: { generation: 4 }, workspace: previewWorkspace },
    selection: { session: { selectionSnapshot: null } },
    locks,
    disk: {
      suspended: true,
      snapshot: {},
      async suspendAndDrain() {},
      resumeAfterSave() {},
    },
    status: { set() {} },
    async reconcileDerived(options) {
      reconciledRevisions.push(project.workspace?.revision ?? null);
      return {
        workspaceRevision: options.expectedWorkspaceRevision,
        topology: "current",
        sourceGraph: "current",
        scss: "current",
        warnings: [],
      };
    },
    async reprojectTemplate() { return true; },
  });
  return {
    instance,
    project,
    documents,
    source,
    previewWorkspace,
    locks,
    reconciledRevisions,
  };
}

test("all workspace authority views preserve live getters and setters", async () => {
  const {
    instance,
    project,
    documents,
    source,
    previewWorkspace,
    locks,
    reconciledRevisions,
  } = serviceFixture();
  const preview = instance.previewHost();
  const settlement = instance.settlementHost();
  const structural = instance.structuralHost();
  const revision17 = workspace(17);
  const canvasPlan = { schemaVersion: 1, phase: "prepared" };

  structural.projectWorkspaceSnapshot = revision17;
  settlement.source = "revizia 17";
  settlement.sourceCache = { "templates/index.html": "revizia 17" };
  settlement.activeScannedPath = "templates/contact.html";
  structural.previewWorkspaceRevision = "preview-17";
  structural.pendingCanvasProjection = canvasPlan;
  await structural.reconcileWorkspaceDerivedState({
    expectedProjectRoot: "/project",
    expectedSessionId: "session:a",
    expectedWorkspaceRevision: 17,
    topologyChanged: true,
  });

  assert.equal(project.workspace, revision17);
  assert.equal(settlement.projectWorkspaceSnapshot, revision17);
  assert.equal(source.source, "revizia 17");
  assert.deepEqual(source.sourceCache, { "templates/index.html": "revizia 17" });
  assert.equal(documents.activeScannedPath, "templates/contact.html");
  assert.equal(previewWorkspace.workspaceRevision, "preview-17");
  assert.equal(previewWorkspace.pendingProjection, canvasPlan);
  assert.deepEqual(reconciledRevisions, [17]);

  project.workspace = workspace(18);
  project.root = "/replacement";
  previewWorkspace.structuralWriteBoundaryActive = false;
  locks.transition.isActive = true;

  assert.equal(structural.projectWorkspaceSnapshot.revision, 18);
  assert.equal(preview.sessionProjectRoot, "/replacement");
  assert.equal(preview.structuralWriteBoundaryActive, false);
  assert.equal(structural.projectTransitionFrontendLeaseActive, true);
});
