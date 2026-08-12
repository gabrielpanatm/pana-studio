import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { afterEach, test } from "node:test";
import { fileURLToPath } from "node:url";
import {
  drainPreviewStructuralLanes,
  PreviewStructuralCancellationError,
  previewStructuralCommandIdentity,
  previewStructuralLaneSnapshot,
  requirePreviewStructuralReceiptIdentity,
  runInPreviewStructuralLane,
} from "$lib/kernel/preview-structural-lane";
import { projectCommittedPreviewStructuralMutation } from "$lib/kernel/preview-projection-control";
import {
  htmlElementContextMenuItems,
  teraContextMenuItems,
} from "$lib/editor-runtime/context-menu";
import {
  captureEditorCommand,
  htmlTargetFromCoordinatedSelection,
} from "$lib/editor-runtime/commands";
import { captureHtmlActionTarget } from "$lib/state/html-actions-controller";

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function host(overrides = {}) {
  return {
    sessionProjectRoot: "/project",
    kernelProjectSessionId: "session:runtime-a",
    projectSessionEpoch: 4,
    projectTransitionFrontendLeaseActive: false,
    kernelUndoRedoFrontendLeaseActive: false,
    async beginPreviewStructuralWriteBoundary() {},
    endPreviewStructuralWriteBoundary() {},
    ...overrides,
  };
}

async function nextTurn() {
  await new Promise((resolve) => setImmediate(resolve));
}

function semanticSelectionSnapshot(
  selectionRevision,
  workspaceRevision,
  editorNodeId,
  sourceNodeId,
  renderInstanceId,
) {
  return {
    schemaVersion: 2,
    projectRoot: "/project",
    runtimeSessionId: "session:runtime-a",
    selectionRevision,
    canvasIdentity: { workspaceRevision },
    primaryMemberId: editorNodeId,
    rangeOriginMemberId: editorNodeId,
    members: [{
      memberId: editorNodeId,
      resolution: "resolved",
      subject: { kind: "htmlElement", tag: "div", label: "<div>" },
      anchor: {
        editorNodeId,
        sourceNodeId,
        renderInstanceId,
        renderInstanceIds: [renderInstanceId],
      },
    }],
    focus: { kind: "element" },
  };
}

function selection(selector, sourceId, tag = "section") {
  return {
    snapshot: semanticSelectionSnapshot(
      1,
      1,
      `editor:${sourceId}`,
      sourceId,
      `render:${sourceId}`,
    ),
    documentEpoch: 1,
    renderInstanceId: `render:${sourceId}`,
    sourceNodeId: sourceId,
    sourceLocation: { file: "templates/index.html", line: sourceId === "source-a" ? 4 : 8, column: 3 },
    observation: {
      selector: `<${tag}>`,
      cssSelector: `[data-test="${sourceId}"]`,
      domPath: selector,
      tag,
      id: "",
      href: "",
      title: "",
      alt: "",
      classes: [sourceId],
      text: sourceId,
      rawText: sourceId,
      hasChildElements: false,
      rect: { width: "1px", height: "1px", top: "0px", left: "0px" },
      styles: [],
      variables: [],
      matchedRules: [],
      imageSrc: null,
      zolaImage: null,
      attributes: { "data-test": sourceId },
      parentNode: { selector: "main", label: "main", tag: "main" },
      childNodes: [],
      blockContext: null,
    },
  };
}

afterEach(async () => {
  await drainPreviewStructuralLanes();
  assert.deepEqual(previewStructuralLaneSnapshot(), {
    sessionCount: 0,
    pendingCount: 0,
  });
});

test("structural lane serializes the complete commit-to-projection lifecycle", async () => {
  const activeHost = host();
  const releaseFirst = deferred();
  const order = [];

  const first = runInPreviewStructuralLane(activeHost, async () => {
    order.push("first:commit");
    await releaseFirst.promise;
    order.push("first:projection");
  });
  const second = runInPreviewStructuralLane(activeHost, async () => {
    order.push("second:commit");
    order.push("second:projection");
  });

  await nextTurn();
  assert.deepEqual(order, ["first:commit"]);
  assert.deepEqual(previewStructuralLaneSnapshot(), {
    sessionCount: 1,
    pendingCount: 2,
  });

  releaseFirst.resolve();
  await Promise.all([first, second]);
  assert.deepEqual(order, [
    "first:commit",
    "first:projection",
    "second:commit",
    "second:projection",
  ]);
});

test("selection-driven structural commands capture the Rust selection revision before queueing", async () => {
  const activeHost = host({
    selectionSnapshot: semanticSelectionSnapshot(
      37,
      23,
      "editor_render:title",
      "source:title",
      "render-title",
    ),
  });
  const gate = deferred();
  const first = runInPreviewStructuralLane(activeHost, async () => {
    await gate.promise;
  });
  let capturedIdentity;
  const queued = runInPreviewStructuralLane(activeHost, async (lease) => {
    capturedIdentity = previewStructuralCommandIdentity(lease, true);
  });

  await nextTurn();
  activeHost.selectionSnapshot = {
    ...semanticSelectionSnapshot(
      38,
      23,
      "editor_render:subtitle",
      "source:subtitle",
      "render-subtitle",
    ),
  };
  gate.resolve();
  await Promise.all([first, queued]);

  assert.deepEqual(capturedIdentity.expectedSelection, {
    selectionRevision: 37,
    workspaceRevision: 23,
    primaryMemberId: "editor_render:title",
    members: [{
      memberId: "editor_render:title",
      editorNodeId: "editor_render:title",
      sourceNodeId: "source:title",
      renderInstanceId: "render-title",
    }],
  });
});

test("structural lane drains the disk monitor before commit and resumes it after projection", async () => {
  const boundaryReady = deferred();
  const releaseBoundary = deferred();
  const releaseProjection = deferred();
  const order = [];
  const activeHost = host({
    async beginPreviewStructuralWriteBoundary() {
      order.push("boundary:begin");
      boundaryReady.resolve();
      await releaseBoundary.promise;
      order.push("boundary:acquired");
    },
    endPreviewStructuralWriteBoundary() {
      order.push("boundary:end");
    },
  });

  const running = runInPreviewStructuralLane(activeHost, async () => {
    order.push("commit");
    await releaseProjection.promise;
    order.push("projection");
  });
  await boundaryReady.promise;
  assert.deepEqual(order, ["boundary:begin"]);

  releaseBoundary.resolve();
  await nextTurn();
  assert.deepEqual(order, ["boundary:begin", "boundary:acquired", "commit"]);

  releaseProjection.resolve();
  await running;
  assert.deepEqual(order, [
    "boundary:begin",
    "boundary:acquired",
    "commit",
    "projection",
    "boundary:end",
  ]);
});

test("structural lane releases the disk boundary when projection fails", async () => {
  const order = [];
  const activeHost = host({
    async beginPreviewStructuralWriteBoundary() {
      order.push("boundary:begin");
    },
    endPreviewStructuralWriteBoundary() {
      order.push("boundary:end");
    },
  });

  await assert.rejects(
    runInPreviewStructuralLane(activeHost, async () => {
      order.push("commit");
      throw new Error("projection failed");
    }),
    /projection failed/,
  );
  assert.deepEqual(order, ["boundary:begin", "commit", "boundary:end"]);
});

test("a failed structural operation does not poison the next queued operation", async () => {
  const activeHost = host();
  const order = [];
  const first = runInPreviewStructuralLane(activeHost, async () => {
    order.push("first");
    throw new Error("projection failed");
  });
  const second = runInPreviewStructuralLane(activeHost, async () => {
    order.push("second");
    return "committed";
  });

  await assert.rejects(first, /projection failed/);
  assert.equal(await second, "committed");
  assert.deepEqual(order, ["first", "second"]);
});

test("a queued request from a replaced runtime session performs no callback", async () => {
  const activeHost = host();
  const releaseFirst = deferred();
  let staleCallbackCount = 0;
  const first = runInPreviewStructuralLane(activeHost, async () => {
    await releaseFirst.promise;
  });
  const stale = runInPreviewStructuralLane(activeHost, async () => {
    staleCallbackCount += 1;
  });

  await nextTurn();
  activeHost.kernelProjectSessionId = "session:runtime-b";
  activeHost.projectSessionEpoch += 1;
  releaseFirst.resolve();
  await first;
  assert.equal(await stale, undefined);
  assert.equal(staleCallbackCount, 0);
});

test("File Explorer uses exact Rust plan/commit while Save owns the single disk barrier", () => {
  const read = (relativePath) => readFileSync(fileURLToPath(new URL(
    `../${relativePath}`,
    import.meta.url,
  )), "utf8");
  const explorer = read("src-tauri/src/kernel/file_explorer.rs");
  const commands = read("src-tauri/src/commands/file_explorer.rs");
  const productionCommands = commands.slice(0, commands.indexOf("#[cfg(test)]"));
  const files = read("src/lib/components/project/ProjectFilesTab.svelte");
  const save = read("src/lib/state/save-controller.ts");
  const app = read("src/lib/state/app.svelte.ts");

  assert.match(explorer, /workspace_revision/);
  assert.match(explorer, /accepted_disk_generation/);
  assert.match(explorer, /commit_token/);
  assert.match(commands, /consume_plan/);
  assert.match(commands, /stage_project_bundle_changes/);
  assert.match(files, /planOperation/);
  assert.match(files, /commitOperation\(resolvedPlan\)/);
  assert.match(save, /await saveProjectWorkspace\(/);
  assert.match(app, /await suspendAndDrainExternalDiskMonitoringFromController/);
  assert.match(app, /return await saveActiveDocument\(this\.saveControllerHost\(\)\)/);
  assert.doesNotMatch(productionCommands, /fs::write|fs::remove|rename\(/);
  assert.doesNotMatch(save, /runInPreviewStructuralLane|acceptedDiskGeneration|InternalWriteEvidence/);
});

test("transition reservation rejects new work while drain waits for active work", async () => {
  const activeHost = host();
  const release = deferred();
  const running = runInPreviewStructuralLane(activeHost, async () => {
    await release.promise;
  });
  await nextTurn();

  activeHost.projectTransitionFrontendLeaseActive = true;
  let drained = false;
  const draining = drainPreviewStructuralLanes().then(() => {
    drained = true;
  });
  assert.equal(
    await runInPreviewStructuralLane(activeHost, async () => "must-not-run"),
    undefined,
  );
  await nextTurn();
  assert.equal(drained, false);

  release.resolve();
  await running;
  await draining;
  assert.equal(drained, true);
});

test("Undo/Redo reservation rejects new structural work while the existing lane drains", async () => {
  const activeHost = host();
  const release = deferred();
  const running = runInPreviewStructuralLane(activeHost, async () => {
    await release.promise;
  });
  await nextTurn();

  activeHost.kernelUndoRedoFrontendLeaseActive = true;
  let drained = false;
  const draining = drainPreviewStructuralLanes().then(() => {
    drained = true;
  });
  let callbackCount = 0;
  assert.equal(
    await runInPreviewStructuralLane(activeHost, async () => {
      callbackCount += 1;
      return "must-not-run";
    }),
    undefined,
  );
  assert.equal(callbackCount, 0);
  await nextTurn();
  assert.equal(drained, false);

  release.resolve();
  await running;
  await draining;
  assert.equal(drained, true);
});

test("receipt identity mismatch is rejected before acknowledgement or projection", () => {
  const lease = {
    projectRoot: "/project",
    sessionId: "session:runtime-a",
    projectSessionEpoch: 4,
  };
  assert.throws(
    () => requirePreviewStructuralReceiptIdentity({
      projectRoot: "/project",
      runtimeSessionId: "session:runtime-b",
    }, lease),
    /another ProjectSession instance/,
  );
  assert.throws(
    () => requirePreviewStructuralReceiptIdentity({
      projectRoot: "/other",
      runtimeSessionId: "session:runtime-a",
    }, lease),
    /another ProjectSession instance/,
  );
});

test("a late receipt cannot project local or Preview state into the active session", async () => {
  const calls = { project: 0 };
  const activeHost = host({
    scannedProject: {},
    previewWorkspaceRevision: null,
    async refreshSourceGraph() {},
    async requestPreviewRefresh() { return true; },
  });
  const lease = {
    projectRoot: "/project",
    sessionId: "session:runtime-a",
    projectSessionEpoch: 4,
  };

  await assert.rejects(
    projectCommittedPreviewStructuralMutation(
      activeHost,
      lease,
      {
        intent: {
          projectRoot: "/project",
          runtimeSessionId: "session:runtime-old",
        },
        touchedFiles: ["templates/index.html"],
        workspaceMutation: {
          schemaVersion: 1,
          changed: true,
          revisionBefore: 3,
          revisionAfter: 4,
          dirty: true,
          touchedFiles: ["templates/index.html"],
        },
      },
      { file: "templates/index.html" },
      () => {
        calls.project += 1;
      },
    ),
    /another ProjectSession instance/,
  );
  assert.deepEqual(calls, { project: 0 });
});

test("Project Transition raises its reservation before draining structural work", () => {
  const source = readFileSync(fileURLToPath(new URL(
    "../src/lib/state/app.svelte.ts",
    import.meta.url,
  )), "utf8");
  const begin = source.slice(source.indexOf("async beginProjectTransitionFrontendLease()"));
  const reserveAt = begin.indexOf("this.projectTransitionFrontendLeaseActive = true");
  const closeMenuAt = begin.indexOf("contextMenu.close()", reserveAt);
  const drainAt = begin.indexOf("await drainPreviewStructuralLanes()", closeMenuAt);
  assert.ok(reserveAt >= 0, "Project Transition trebuie să ridice reservation flag");
  assert.ok(closeMenuAt > reserveAt, "meniul contextual trebuie închis după rezervare");
  assert.ok(drainAt > closeMenuAt, "mutațiile structurale trebuie drenate după rezervare");
});

test("preview Delete uses the Rust-resolved Canvas target instead of a legacy bridge selection", () => {
  const agent = readFileSync(fileURLToPath(new URL(
    "../src-tauri/src/preview/bridge/03_canvas_agent.js",
    import.meta.url,
  )), "utf8");
  const controller = readFileSync(fileURLToPath(new URL(
    "../src/lib/state/canvas-interaction-controller.ts",
    import.meta.url,
  )), "utf8");

  assert.match(agent, /emitCanvasAgentSelectionAction\("deleteSelection"\)/);
  assert.doesNotMatch(agent, /post\("preview-delete-selected"/);
  const authorityAt = controller.indexOf("function coordinatedActionTarget");
  const revisionAt = controller.indexOf(
    "selection.selectionRevision !== message.selectionRevision",
    authorityAt,
  );
  const dispatchAt = controller.indexOf("type: \"delete-html\"");
  assert.ok(authorityAt >= 0 && revisionAt > authorityAt);
  assert.ok(dispatchAt >= 0);
  assert.doesNotMatch(controller, /selectedReceipt|selectedKey/);
  assert.match(controller, /selection\.renderInstanceId !== target\.renderInstanceId/);
});

test("Layers Delete resolves the clicked node before dispatching consecutive child-parent deletes", () => {
  const app = readFileSync(fileURLToPath(new URL(
    "../src/lib/state/app.svelte.ts",
    import.meta.url,
  )), "utf8");
  const deleteFlow = app.slice(
    app.indexOf("async deleteEditorNavigationNode"),
    app.indexOf("async deleteHtmlElement", app.indexOf("async deleteEditorNavigationNode")),
  );
  const selectAt = deleteFlow.indexOf("await selectEditorNavigationNodeFromController");
  const verifyAt = deleteFlow.indexOf("primarySelectionEditorNodeId(selection) !== node.id");
  const dispatchAt = deleteFlow.indexOf("type: \"delete-html\"");
  assert.ok(selectAt >= 0, "ținta apăsată trebuie selectată prin Rust");
  assert.ok(verifyAt > selectAt, "confirmarea Rust trebuie verificată");
  assert.ok(dispatchAt > verifyAt, "Delete trebuie emis numai după confirmarea țintei");
});

test("missing structural selection remains a visible target error, not a masked session cancellation", () => {
  const lease = {
    projectRoot: "/project",
    sessionId: "session:runtime-a",
    projectSessionEpoch: 4,
    selection: null,
  };
  assert.throws(
    () => previewStructuralCommandIdentity(lease, true),
    (error) => error instanceof Error
      && !(error instanceof PreviewStructuralCancellationError)
      && error.message.length > 0,
  );
});

test("generated class and data-anim delegate identity allocation to the captured Rust target", () => {
  const source = readFileSync(fileURLToPath(new URL(
    "../src/lib/state/html-actions-controller.ts",
    import.meta.url,
  )), "utf8");
  const classAction = source.slice(
    source.indexOf("export async function generateClassForSelectedHtml"),
    source.indexOf("export async function generateDataAnimForSelectedHtml"),
  );
  const dataAnimAction = source.slice(
    source.indexOf("export async function generateDataAnimForSelectedHtml"),
    source.indexOf("export async function insertNodeRelative"),
  );

  assert.match(
    classAction,
    /captureHtmlActionTarget\(host\.coordinatedElementSelection\)[\s\S]*generateIdentityForTarget\(host, target, "class"\)/,
  );
  assert.match(
    dataAnimAction,
    /captureHtmlActionTarget\(host\.coordinatedElementSelection\)[\s\S]*generateIdentityForTarget\(host, target, "dataAnim"\)/,
  );
  assert.match(source, /executeSelectedHtmlAttributes\([\s\S]*\{ kind \},\s*\);/);
  assert.doesNotMatch(source, /collectIdentitySourceTexts|generateUniqueHtmlIdentity|readProjectFile/);
  assert.match(source, /committedDraftCanSettle\(currentClasses, submittedClasses, baselineClasses\)/);
  assert.match(
    source,
    /committedDraftCanSettle\([\s\S]*attributeDraftToken\(host\.attributeValues\)[\s\S]*submittedAttributeDraft[\s\S]*baselineAttributeDraft/,
  );
  assert.match(
    source,
    /removedGeneratedClass\s*&&\s*isZolaTemplatePath\(patch\.file\)/,
    "adăugarea unei clase nu trebuie să lanseze reconcilierea de curățare",
  );
  assert.match(
    source,
    /removedOrReplacedDataAnim\s*&&\s*isZolaTemplatePath\(patch\.file\)/,
    "adăugarea data-anim nu trebuie să lanseze reconcilierea de curățare",
  );
});

test("A queued structural SourceNode target stays A when selection changes to B before execution", async () => {
  const activeHost = host({
    coordinatedElementSelection: selection("main > section:nth-of-type(1)", "source-a"),
  });
  const releaseFirst = deferred();
  const first = runInPreviewStructuralLane(activeHost, async () => {
    await releaseFirst.promise;
  });

  const originalSelectionA = activeHost.coordinatedElementSelection;
  const capturedTargetA = captureHtmlActionTarget(
    htmlTargetFromCoordinatedSelection(originalSelectionA),
  );
  let executedTarget = null;
  const queued = runInPreviewStructuralLane(activeHost, async () => {
    executedTarget = capturedTargetA;
  });

  await nextTurn();
  activeHost.coordinatedElementSelection = selection("main > section:nth-of-type(2)", "source-b");
  originalSelectionA.observation.domPath = "main > section:nth-of-type(99)";
  originalSelectionA.sourceNodeId = "source-mutated";
  originalSelectionA.observation.classes[0] = "class-mutated";
  originalSelectionA.observation.attributes["data-test"] = "attribute-mutated";
  releaseFirst.resolve();
  await Promise.all([first, queued]);

  assert.equal("selector" in executedTarget, false);
  assert.equal(executedTarget.sourceId, "source-a");
  assert.equal(executedTarget.sourceLocation.line, 4);
  assert.deepEqual(executedTarget.classes, ["source-a"]);
  assert.deepEqual(executedTarget.attributes, { "data-test": "source-a" });
  assert.equal(Object.isFrozen(executedTarget), true);
  assert.equal(Object.isFrozen(executedTarget.sourceLocation), true);
  assert.equal(Object.isFrozen(executedTarget.classes), true);
  assert.equal(Object.isFrozen(executedTarget.attributes), true);
});

test("editor runtime command snapshot survives its first async boundary", async () => {
  const centerGate = deferred();
  const target = {
    kind: "html",
    selector: "main > section:nth-of-type(1)",
    tag: "section",
    sourceId: "source-a",
  };
  const captured = captureEditorCommand({
    type: "open-html-code",
    surface: "preview",
    target,
  });
  const executed = centerGate.promise.then(() => captured);
  await nextTurn();
  target.selector = "main > section:nth-of-type(2)";
  target.sourceId = "source-b";
  centerGate.resolve(true);

  const commandAtExecution = await executed;
  assert.equal(commandAtExecution.target.selector, "main > section:nth-of-type(1)");
  assert.equal(commandAtExecution.target.sourceId, "source-a");
  assert.equal(Object.isFrozen(commandAtExecution), true);
  assert.equal(Object.isFrozen(commandAtExecution.target), true);
});

test("Duplicate din meniul contextual așteaptă exact comanda structurală pentru ținta capturată", async () => {
  const dispatched = [];
  const completion = deferred();
  const target = {
    kind: "html",
    selector: "main > section:nth-of-type(2)",
    tag: "section",
    sourceId: "source-section-2",
    sessionId: "preview-node-2",
  };
  const expectedTarget = {
    ...target,
    sourceLocation: null,
    observation: null,
  };
  const runtime = {
    canDispatch() {
      return { allowed: true, reason: "" };
    },
    async dispatch(command) {
      dispatched.push(command);
      await completion.promise;
      return { ok: true, revision: 1, command: command.type };
    },
  };
  const duplicate = htmlElementContextMenuItems(runtime, target, "preview")
    .find((item) => item.id === "preview-duplicate-html");
  assert.ok(duplicate);
  assert.equal(duplicate.disabled, false);

  let completed = false;
  const action = duplicate.action().then(() => {
    completed = true;
  });
  target.selector = "main > section:nth-of-type(9)";
  target.sourceId = "source-section-9";
  await nextTurn();
  assert.equal(completed, false);
  assert.deepEqual(dispatched, [{
    type: "duplicate-html",
    surface: "preview",
    target: expectedTarget,
  }]);

  completion.resolve();
  await action;
  assert.equal(completed, true);
});

test("Tera context action keeps the captured source node after the live graph object changes", async () => {
  const dispatched = [];
  const sourceNode = {
    id: "tera:a",
    kind: "include",
    file: "templates/index.html",
    origin: "local",
    themeName: null,
    label: "Include A",
    range: { start: 0, end: 10, line: 2, column: 1, endLine: 2, endColumn: 11 },
    parent: null,
    children: [],
    capabilities: {
      canOpenInCode: true,
      canEditVisual: true,
      canEditText: false,
      canEditAttributes: false,
      canMove: true,
      canExtractPartial: false,
      reason: null,
    },
  };
  const runtime = {
    canDispatch() {
      return { allowed: true, reason: "" };
    },
    async dispatch(command) {
      dispatched.push(command);
      return { ok: true, status: "committed", revision: 1, command: command.type };
    },
  };
  const remove = teraContextMenuItems(runtime, {
    kind: "tera",
    sourceId: sourceNode.id,
    selector: "main",
    sourceNode,
  }, "layers").find((item) => item.id === "layers-delete-tera");
  assert.ok(remove);

  sourceNode.id = "tera:b";
  sourceNode.range.line = 99;
  await remove.action();

  assert.equal(dispatched[0].target.sourceId, "tera:a");
  assert.equal(dispatched[0].target.sourceNode.id, "tera:a");
  assert.equal(dispatched[0].target.sourceNode.range.line, 2);
  assert.equal(Object.isFrozen(dispatched[0].target.sourceNode), true);
});

test("direct Tera delete captures its source node before entering the structural lane", () => {
  const source = readFileSync(fileURLToPath(new URL(
    "../src/lib/state/tera-actions-controller.ts",
    import.meta.url,
  )), "utf8");
  const action = source.slice(
    source.indexOf("export async function deleteSelectedTeraNode"),
    source.indexOf("async function deleteSelectedTeraNodeInLane"),
  );
  const captureAt = action.indexOf("const targetNode = captureTeraActionTarget");
  const laneAt = action.indexOf("runInPreviewStructuralLane");
  assert.ok(captureAt >= 0 && captureAt < laneAt);
  assert.match(action, /deleteSelectedTeraNodeInLane\(host, targetNode, lease\)/);
});
