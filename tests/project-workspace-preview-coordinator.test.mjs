import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  markProjectWorkspacePreviewPublished,
  projectLatestProjectWorkspacePreview,
  resetProjectWorkspacePreviewCoordinator,
  scheduleProjectWorkspaceDerivedPreviewProjection,
} from "$lib/kernel/project-workspace-preview-coordinator";
import {
  projectTemplateWorkbenchPreview,
} from "$lib/project/io";

if (!globalThis.window) globalThis.window = globalThis;

afterEach(() => {
  clearMocks();
  resetProjectWorkspacePreviewCoordinator();
});

function deferred() {
  let resolve;
  const promise = new Promise((resolvePromise) => { resolve = resolvePromise; });
  return { promise, resolve };
}

function snapshot(revision, projectRoot = "/project", runtimeSessionId = "session:a") {
  return { projectRoot, runtimeSessionId, revision, dirty: true };
}

function host(overrides = {}) {
  return {
    sessionProjectRoot: "/project",
    kernelProjectSessionId: "session:a",
    scannedProject: { isZola: true },
    previewWorkspaceRevision: null,
    pendingCanvasProjection: null,
    canvasSurfaceAvailable: true,
    canProjectWorkspacePreview() {
      return this.canvasSurfaceAvailable;
    },
    refreshes: [],
    async requestPreviewRefresh(reason) {
      this.refreshes.push(reason);
      return true;
    },
    ...overrides,
  };
}

function projectionReceipt(input, previewRevision = `preview-${input.expectedWorkspaceRevision}`) {
  const identity = {
    projectRoot: input.expectedProjectRoot,
    runtimeSessionId: input.expectedSessionId,
    workspaceRevision: input.expectedWorkspaceRevision,
    transactionId: `canvas-${input.expectedWorkspaceRevision}`,
    previewRevision,
  };
  return {
    operation: "workspace_projection",
    projectRoot: input.expectedProjectRoot,
    runtimeSessionId: input.expectedSessionId,
    requestedPaths: input.requestedPaths,
    previewRevision,
    canvasProjection: {
      schemaVersion: 1,
      identity,
      workspaceTransactionId: `workspace-${input.expectedWorkspaceRevision}`,
      phase: "prepared",
      impact: { kinds: ["htmlStructure"], paths: input.requestedPaths, requiresFullDocument: false },
      resources: { schemaVersion: 1, previewRevision, totalBytes: 0, entries: [] },
    },
    workspaceRevision: input.expectedWorkspaceRevision,
  };
}

test("one coordinator projects, publishes and deduplicates a workspace revision", async () => {
  const target = host();
  const calls = [];
  mockIPC((command, args) => {
    if (command === "read_project_workspace_state") return snapshot(7);
    if (command === "project_project_workspace_preview") {
      calls.push(args.input);
      return projectionReceipt(args.input);
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  await projectLatestProjectWorkspacePreview(target, {
    reason: "workspace-mutation",
    requestedPaths: ["sursa/templates/index.html", "sursa/templates/index.html", ""],
  });
  await projectLatestProjectWorkspacePreview(target, { reason: "workspace-mutation" });

  assert.deepEqual(calls, [{
    expectedProjectRoot: "/project",
    expectedSessionId: "session:a",
    expectedWorkspaceRevision: 7,
    requestedPaths: ["sursa/templates/index.html"],
  }]);
  assert.equal(target.previewWorkspaceRevision, "preview-7");
  assert.deepEqual(target.refreshes, ["workspace-mutation"]);
});

test("cache-ul Preview deduplică numai cu dovada exactă a tranzacției Canvas", async () => {
  const target = host();
  let projections = 0;
  mockIPC((command, args) => {
    if (command === "read_project_workspace_state") return snapshot(7);
    if (command === "project_project_workspace_preview") {
      projections += 1;
      return projectionReceipt(args.input);
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  await projectLatestProjectWorkspacePreview(target, { reason: "workspace-mutation" });
  let cachedPlan = null;
  await projectLatestProjectWorkspacePreview(target, {
    reason: "workspace-mutation",
    expectedWorkspaceRevision: 7,
    expectedWorkspaceTransactionId: "workspace-7",
    onCanvasPlanPrepared(plan) {
      cachedPlan = plan;
    },
  });
  assert.equal(cachedPlan?.workspaceTransactionId, "workspace-7");
  assert.equal(projections, 1);

  await assert.rejects(
    projectLatestProjectWorkspacePreview(target, {
      reason: "workspace-mutation",
      expectedWorkspaceRevision: 7,
      expectedWorkspaceTransactionId: "workspace-străin",
    }),
    /nu dovedește tranzacția așteptată a sesiunii proiectului/,
  );
  assert.equal(projections, 1);
});

test("a superseded materialization retries only the latest Rust workspace revision", async () => {
  const target = host();
  let workspaceRevision = 3;
  const projected = [];
  mockIPC((command, args) => {
    if (command === "read_project_workspace_state") return snapshot(workspaceRevision);
    if (command === "project_project_workspace_preview") {
      projected.push(args.input.expectedWorkspaceRevision);
      if (projected.length === 1) {
        workspaceRevision = 4;
        throw new Error("revizia candidată a fost depășită");
      }
      return projectionReceipt(args.input);
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  await projectLatestProjectWorkspacePreview(target, {
    reason: "workspace-mutation",
    minimumWorkspaceRevision: 3,
  });

  assert.deepEqual(projected, [3, 4]);
  assert.equal(target.previewWorkspaceRevision, "preview-4");
  assert.deepEqual(target.refreshes, ["workspace-mutation"]);
});

test("a superseded route failure is retried without publishing the stale revision error", async () => {
  let workspaceRevision = 20;
  const projected = [];
  const refreshes = [];
  const statuses = [];
  const target = host({
    async requestWorkspaceProjectionPreviewRefresh(reason) {
      refreshes.push({ reason, revision: workspaceRevision });
      if (workspaceRevision === 20) {
        workspaceRevision = 21;
        throw new Error("HTTP/1.1 404 Not Found pentru draftul reviziei 20");
      }
      return true;
    },
    setGlobalStatus(text, kind) {
      statuses.push({ text, kind });
    },
  });
  mockIPC((command, args) => {
    if (command === "read_project_workspace_state") return snapshot(workspaceRevision);
    if (command === "project_project_workspace_preview") {
      projected.push(args.input.expectedWorkspaceRevision);
      return projectionReceipt(args.input);
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  await projectLatestProjectWorkspacePreview(target, {
    reason: "workspace-mutation",
    minimumWorkspaceRevision: 20,
  });

  assert.deepEqual(projected, [20, 21]);
  assert.deepEqual(refreshes, [
    { reason: "workspace-mutation", revision: 20 },
    { reason: "workspace-mutation", revision: 21 },
  ]);
  assert.deepEqual(statuses, []);
  assert.deepEqual(target.refreshes, []);
  assert.equal(target.previewWorkspaceRevision, "preview-21");
});

test("a late projection from a replaced same-root runtime has zero UI effects", async () => {
  const gate = deferred();
  const target = host();
  mockIPC(async (command, args) => {
    if (command === "read_project_workspace_state") return snapshot(2);
    if (command === "project_project_workspace_preview") {
      await gate.promise;
      return projectionReceipt(args.input);
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  const projection = projectLatestProjectWorkspacePreview(target, {
    reason: "workspace-mutation",
  });
  await Promise.resolve();
  target.kernelProjectSessionId = "session:b";
  gate.resolve();
  await projection;

  assert.equal(target.previewWorkspaceRevision, null);
  assert.deepEqual(target.refreshes, []);
});

test("minimum revision and receipt identity are fail-closed", async () => {
  const target = host();
  mockIPC((command, args) => {
    if (command === "read_project_workspace_state") return snapshot(5);
    if (command === "project_project_workspace_preview") {
      return { ...projectionReceipt(args.input), workspaceRevision: 4 };
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  await assert.rejects(
    projectLatestProjectWorkspacePreview(target, {
      reason: "workspace-mutation",
      minimumWorkspaceRevision: 6,
    }),
    /sub revizia minimă cerută/,
  );
  await assert.rejects(
    projectLatestProjectWorkspacePreview(target, { reason: "workspace-mutation" }),
    /altă revizie ProjectWorkspace/,
  );
  assert.equal(target.previewWorkspaceRevision, null);
  assert.deepEqual(target.refreshes, []);
});

test("a preview revision published during session start is already settled", async () => {
  const target = host();
  let projections = 0;
  mockIPC((command) => {
    if (command === "read_project_workspace_state") return snapshot(9);
    if (command === "project_project_workspace_preview") {
      projections += 1;
      throw new Error("proiecția nu trebuie repetată");
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });
  markProjectWorkspacePreviewPublished("/project", "session:a", 9);

  await projectLatestProjectWorkspacePreview(target, { reason: "workspace-mutation" });
  assert.equal(projections, 0);
});

test("one mutation event projects the same workspace revision into main Preview and active Template Workbench", async () => {
  const workbenchRevisions = [];
  const projectedRevisions = [];
  const target = host({
    templateWorkbenchActive: true,
    async reprojectActiveTemplateWorkbench(minimumWorkspaceRevision) {
      workbenchRevisions.push(minimumWorkspaceRevision);
      return true;
    },
  });
  mockIPC((command, args) => {
    if (command === "read_project_workspace_state") return snapshot(12);
    if (command === "project_project_workspace_preview") {
      projectedRevisions.push(args.input.expectedWorkspaceRevision);
      return projectionReceipt(args.input);
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  scheduleProjectWorkspaceDerivedPreviewProjection(
    target,
    "workspace-mutation",
    11,
  );
  scheduleProjectWorkspaceDerivedPreviewProjection(
    target,
    "workspace-mutation",
    12,
  );
  await new Promise((resolve) => setTimeout(resolve, 220));

  assert.deepEqual(projectedRevisions, [12]);
  assert.deepEqual(workbenchRevisions, [12]);
  assert.deepEqual(target.refreshes, []);
  assert.equal(target.previewWorkspaceRevision, "preview-12");
});

test("an exact receipt-bound projection consumes the derived mutation timer", async () => {
  const projectedRevisions = [];
  const target = host();
  mockIPC((command, args) => {
    if (command === "read_project_workspace_state") return snapshot(18);
    if (command === "project_project_workspace_preview") {
      projectedRevisions.push(args.input.expectedWorkspaceRevision);
      return projectionReceipt(args.input);
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  scheduleProjectWorkspaceDerivedPreviewProjection(target, "workspace-mutation", 18);
  await projectLatestProjectWorkspacePreview(target, {
    reason: "workspace-mutation",
    minimumWorkspaceRevision: 18,
    expectedWorkspaceRevision: 18,
    expectedWorkspaceTransactionId: "workspace-18",
  });
  await new Promise((resolve) => setTimeout(resolve, 180));

  assert.deepEqual(projectedRevisions, [18]);
  assert.deepEqual(target.refreshes, ["workspace-mutation"]);
});

test("a direct structural projection restores the active Template Workbench at the canonical revision", async () => {
  const workbenchRevisions = [];
  const target = host({
    templateWorkbenchActive: true,
    async reprojectActiveTemplateWorkbench(minimumWorkspaceRevision) {
      workbenchRevisions.push(minimumWorkspaceRevision);
      return true;
    },
  });
  mockIPC((command, args) => {
    if (command === "read_project_workspace_state") return snapshot(14);
    if (command === "project_project_workspace_preview") return projectionReceipt(args.input);
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  await projectLatestProjectWorkspacePreview(target, {
    reason: "html-structural",
    minimumWorkspaceRevision: 14,
    requestedPaths: ["sursa/templates/partials/header.html"],
  });

  assert.deepEqual(target.refreshes, []);
  assert.deepEqual(workbenchRevisions, [14]);
});

test("an unmounted Canvas defers the workspace revision and projects it once a surface is mounted", async () => {
  const workbenchRevisions = [];
  const projectedRevisions = [];
  let workspaceReads = 0;
  const target = host({
    canvasSurfaceAvailable: false,
    templateWorkbenchActive: true,
    async reprojectActiveTemplateWorkbench(minimumWorkspaceRevision) {
      workbenchRevisions.push(minimumWorkspaceRevision);
      return true;
    },
  });
  mockIPC((command, args) => {
    if (command === "read_project_workspace_state") {
      workspaceReads += 1;
      return snapshot(16);
    }
    if (command === "project_project_workspace_preview") {
      projectedRevisions.push(args.input.expectedWorkspaceRevision);
      return projectionReceipt(args.input);
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });

  await projectLatestProjectWorkspacePreview(target, {
    reason: "workspace-mutation",
    minimumWorkspaceRevision: 16,
    requestedPaths: ["sursa/content/despre.md", "sursa/templates/despre.html"],
  });

  assert.equal(workspaceReads, 0);
  assert.deepEqual(projectedRevisions, []);
  assert.deepEqual(workbenchRevisions, []);
  assert.deepEqual(target.refreshes, []);
  assert.equal(target.previewWorkspaceRevision, null);

  target.canvasSurfaceAvailable = true;
  await projectLatestProjectWorkspacePreview(target, {
    reason: "manual",
    minimumWorkspaceRevision: 16,
  });

  assert.equal(workspaceReads, 1);
  assert.deepEqual(projectedRevisions, [16]);
  assert.deepEqual(workbenchRevisions, [16]);
  assert.deepEqual(target.refreshes, []);
  assert.equal(target.previewWorkspaceRevision, "preview-16");
});

test("Template Workbench uses the canonical preview command with one exact workspace revision", async () => {
  const request = {
    expectedProjectRoot: "/project",
    expectedSessionId: "session:a",
    expectedWorkspaceRevision: 9,
    templatePath: "sursa/templates/index.html",
    preferredPagePath: null,
  };
  const receipt = {
    previewUrl: "http://127.0.0.1:4100/__pana_workbench/index/",
    route: "/__pana_workbench/index/",
    workspaceRevision: 9,
    previewRevision: "preview-9",
    plan: { activeTemplate: { file: request.templatePath } },
    canvasProjection: { phase: "canonicalVerified" },
  };
  let invocation = null;
  mockIPC((command, payload) => {
    invocation = { command, payload };
    return receipt;
  });

  assert.equal(await projectTemplateWorkbenchPreview(request), receipt);
  assert.deepEqual(invocation, {
    command: "project_template_workbench_preview",
    payload: { input: request },
  });
});
