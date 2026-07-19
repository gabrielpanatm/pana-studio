import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { drainPreviewStructuralLanes } from "$lib/kernel/preview-structural-lane";
import { resetProjectWorkspacePreviewCoordinator } from "$lib/kernel/project-workspace-preview-coordinator";
import { moveLayerElement } from "$lib/state/layers-drag-controller";

if (!globalThis.window) globalThis.window = globalThis;

afterEach(async () => {
  clearMocks();
  resetProjectWorkspacePreviewCoordinator();
  await drainPreviewStructuralLanes();
});

function section(selector, sourceId, line) {
  return {
    selector,
    label: "Section",
    tag: "section",
    depth: 0,
    sourceId,
    templateSourceId: "template:index",
    sessionId: `preview:${sourceId}`,
    sourceLocation: { file: "sursa/templates/index.html", line, column: 1 },
  };
}

function moveRequest() {
  return {
    sourceSelector: "main > section:nth-of-type(1)",
    targetSelector: "main > section:nth-of-type(2)",
    sourceSourceId: "sg_aaaaaaaaaaaaaaaa",
    targetSourceId: "sg_bbbbbbbbbbbbbbbb",
    sourceSessionId: "preview:source-a",
    targetSessionId: "preview:target-b",
    sourceTemplateSourceId: "template:index",
    targetTemplateSourceId: "template:index",
    targetKind: "html",
    position: "after",
  };
}

function workspaceMutation() {
  return {
    schemaVersion: 1,
    changed: true,
    revisionBefore: 4,
    revisionAfter: 5,
    dirty: true,
    transactionId: "workspace-move-5",
    touchedFiles: ["sursa/templates/index.html"],
  };
}

function canvasPatch() {
  return {
    schemaVersion: 1,
    patchId: `canvas_patch_${"a".repeat(64)}`,
    issuedAtMs: Date.now(),
    projectRoot: "/project",
    runtimeSessionId: "session:runtime",
    baseWorkspaceRevision: 4,
    workspaceRevision: 5,
    workspaceTransactionId: "workspace-move-5",
    beforeModelRevision: "before",
    afterModelRevision: "after",
    operation: {
      kind: "move",
      source: { sourceId: "sg_aaaaaaaaaaaaaaaa", renderInstanceId: null, selectorFallback: null, expectedTag: "section" },
      target: { sourceId: "sg_bbbbbbbbbbbbbbbb", renderInstanceId: null, selectorFallback: null, expectedTag: "section" },
      position: "after",
    },
  };
}

function committedReceipt(overrides = {}) {
  return {
    schemaVersion: 2,
    intent: {
      projectRoot: "/project",
      runtimeSessionId: "session:runtime",
      kind: "layer_drop",
    },
    status: "committed",
    message: "committed",
    modelRevision: "model:after",
    projectedSourceId: "sg_2222222222222222",
    touchedFiles: ["sursa/templates/index.html"],
    diagnostics: [],
    workspaceMutation: workspaceMutation(),
    canvasPatch: canvasPatch(),
    patch: {
      file: "sursa/templates/index.html",
      resolvedSourceId: "sg_aaaaaaaaaaaaaaaa",
      resolvedTargetId: "sg_bbbbbbbbbbbbbbbb",
      sourceLabel: "<section>",
      beforeRevision: "before",
      afterRevision: "after",
      contents: "<main><section>B</section><section>A</section></main>",
      sourceLocation: { file: "sursa/templates/index.html", line: 2, column: 1 },
      targetLocation: { file: "sursa/templates/index.html", line: 5, column: 1 },
      sourceStartLine: 2,
      sourceEndLine: 4,
      newStartLine: 7,
    },
    ...overrides,
  };
}

function host(overrides = {}) {
  const events = [];
  return {
    sessionProjectRoot: "/project",
    kernelProjectSessionId: "session:runtime",
    projectSessionEpoch: 4,
    projectTransitionFrontendLeaseActive: false,
    kernelUndoRedoFrontendLeaseActive: false,
    async beginPreviewStructuralWriteBoundary() { events.push("boundary:begin"); },
    endPreviewStructuralWriteBoundary() { events.push("boundary:end"); },
    scannedProject: { isZola: true },
    previewWorkspaceRevision: null,
    pendingCanvasProjection: null,
    structureStatus: "",
    isActivePreviewHtmlSource: false,
    htmlSourceMutationBlockedReason: "",
    htmlPending: { tag: false, attributes: false, text: false, image: false, classes: false, structure: false },
    selectedElement: null,
    textEditOriginalKey: null,
    pageSections: [
      section("main > section:nth-of-type(1)", "sg_aaaaaaaaaaaaaaaa", 2),
      section("main > section:nth-of-type(2)", "sg_bbbbbbbbbbbbbbbb", 5),
    ],
    activeScannedPath: "sursa/templates/index.html",
    source: "before",
    sourceCache: {},
    pendingSelectionSelector: null,
    getPreviewDocument() { return undefined; },
    async applyCanvasPatchToPreview(patch) {
      assert.equal(patch.workspaceTransactionId, "workspace-move-5");
      events.push("canvas-patch");
    },
    async rollbackCanvasPatchInPreview(patch) {
      assert.equal(patch.workspaceTransactionId, "workspace-move-5");
      events.push("canvas-rollback");
    },
    async refreshSourceGraph() { events.push("graph"); },
    async requestPreviewRefresh(reason) { events.push(`refresh:${reason}`); return true; },
    async applyTextContentToHtml() { return { status: "noop" }; },
    setHtmlPending(area, pending) { this.htmlPending[area] = pending; },
    setGlobalStatus() {},
    events,
    ...overrides,
  };
}

function installCommittedIpc(receipt = committedReceipt(), options = {}) {
  const commands = [];
  mockIPC((command, args) => {
    commands.push(command);
    if (command === "execute_preview_layer_drop_intent") return receipt;
    if (command === "read_project_workspace_state") {
      return {
        projectRoot: "/project",
        runtimeSessionId: "session:runtime",
        revision: 5,
        history: { nextUndo: { transactionId: "workspace-move-5" } },
      };
    }
    if (command === "project_project_workspace_preview") {
      if (options.projectionError) throw new Error(options.projectionError);
      const identity = {
        projectRoot: "/project",
        runtimeSessionId: "session:runtime",
        workspaceRevision: 5,
        transactionId: "canvas:workspace-5",
        previewRevision: "preview:workspace-5",
      };
      return {
        operation: "workspace_projection",
        projectRoot: "/project",
        runtimeSessionId: "session:runtime",
        requestedPaths: args.input.requestedPaths,
        previewRevision: "preview:workspace-5",
        canvasProjection: {
          schemaVersion: 1,
          identity,
          workspaceTransactionId: "workspace-move-5",
          phase: "prepared",
          impact: { kinds: ["htmlStructure"], paths: args.input.requestedPaths, requiresFullDocument: false },
          resources: { schemaVersion: 1, previewRevision: "preview:workspace-5", totalBytes: 0, entries: [] },
        },
        workspaceRevision: 5,
      };
    }
    throw new Error(`Comandă IPC neașteptată: ${command}`);
  });
  return commands;
}

test("committed move applies one Rust CanvasPatch before the canonical projection", async () => {
  const commands = installCommittedIpc();
  const activeHost = host();

  const result = await moveLayerElement(activeHost, moveRequest());

  assert.equal(result.status, "committed", result.reason);
  assert.equal(activeHost.source, "<main><section>B</section><section>A</section></main>");
  assert.equal(activeHost.previewWorkspaceRevision, "preview:workspace-5");
  assert.equal(activeHost.pendingSelectionSelector, '[data-pana-source-id="sg_2222222222222222"]');
  assert.deepEqual(commands, [
    "execute_preview_layer_drop_intent",
    "read_project_workspace_state",
    "read_project_workspace_state",
    "project_project_workspace_preview",
  ]);
  assert.deepEqual(activeHost.events, [
    "boundary:begin",
    "canvas-patch",
    "graph",
    "refresh:html-structural",
    "boundary:end",
  ]);
});

test("a canonical projection failure rolls back the provisional CanvasPatch", async () => {
  const commands = installCommittedIpc(
    committedReceipt(),
    { projectionError: "Zola candidate failed" },
  );
  const activeHost = host();

  const result = await moveLayerElement(activeHost, moveRequest());

  assert.equal(result.status, "failed");
  assert.match(result.reason, /Zola candidate failed/);
  assert.deepEqual(activeHost.events, [
    "boundary:begin",
    "canvas-patch",
    "graph",
    "canvas-rollback",
    "boundary:end",
  ]);
  assert.deepEqual(commands, [
    "execute_preview_layer_drop_intent",
    "read_project_workspace_state",
    "read_project_workspace_state",
    "project_project_workspace_preview",
    "read_project_workspace_state",
  ]);
});

test("blocked move performs no workspace projection", async () => {
  const commands = [];
  mockIPC((command) => {
    commands.push(command);
    return {
      schemaVersion: 2,
      intent: { projectRoot: "/project", runtimeSessionId: "session:runtime", kind: "layer_drop" },
      status: "blocked",
      message: "Move Engine blocked",
      modelRevision: "model:before",
      projectedSourceId: null,
      patch: null,
      workspaceMutation: null,
      touchedFiles: [],
      diagnostics: [{ blocking: true, message: "identitatea sursei este stale" }],
    };
  });
  const activeHost = host();
  const result = await moveLayerElement(activeHost, moveRequest());
  assert.equal(result.status, "blocked");
  assert.match(result.reason, /stale/);
  assert.deepEqual(commands, ["execute_preview_layer_drop_intent"]);
  assert.equal(activeHost.pendingSelectionSelector, null);
});

test("transport failure is terminal and does not poison the structural lane", async () => {
  mockIPC(() => { throw new Error("kernel move unavailable"); });
  const activeHost = host();
  const result = await moveLayerElement(activeHost, moveRequest());
  assert.equal(result.status, "failed");
  assert.match(result.reason, /kernel move unavailable/);
  assert.deepEqual(activeHost.events, ["boundary:begin", "boundary:end"]);
});

test("committed move without post-commit Source Graph identity fails closed", async () => {
  const commands = installCommittedIpc(committedReceipt({ projectedSourceId: null }));
  const activeHost = host();
  const result = await moveLayerElement(activeHost, moveRequest());
  assert.equal(result.status, "failed");
  assert.match(result.reason, /identitatea Source Graph post-commit/);
  assert.deepEqual(commands, ["execute_preview_layer_drop_intent"]);
  assert.equal(activeHost.previewWorkspaceRevision, null);
});

test("pending text on another element blocks move before Rust", async () => {
  let ipcCount = 0;
  mockIPC(() => { ipcCount += 1; });
  const activeHost = host({
    htmlPending: { tag: false, attributes: false, text: true, image: false, classes: false, structure: false },
    textEditOriginalKey: "source:other::main > p",
    selectedElement: {
      sourceId: "source:selected",
      sessionId: "selected",
      domPath: "main > h1",
      sourceLocation: { file: "sursa/templates/index.html", line: 1, column: 1 },
    },
  });
  const result = await moveLayerElement(activeHost, moveRequest());
  assert.equal(result.status, "blocked");
  assert.match(result.reason, /editare de text neaplicată/);
  assert.equal(ipcCount, 0);
});

test("Project Transition reservation cancels a new move without entering Rust", async () => {
  let ipcCount = 0;
  mockIPC(() => { ipcCount += 1; });
  const activeHost = host({ projectTransitionFrontendLeaseActive: true });
  const result = await moveLayerElement(activeHost, moveRequest());
  assert.equal(result.status, "cancelled");
  assert.equal(ipcCount, 0);
  assert.deepEqual(activeHost.events, []);
});
