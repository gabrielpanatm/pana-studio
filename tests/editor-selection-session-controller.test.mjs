import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { EditorSelectionSessionController } from "$lib/state/editor-selection-session.svelte";

if (!globalThis.window) globalThis.window = globalThis;

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

const identity = Object.freeze({
  projectRoot: "/project",
  runtimeSessionId: "session:runtime",
  workspaceRevision: 7,
  transactionId: "canvas:7",
  previewRevision: "preview:7",
});

function aggregate(memberCount) {
  return {
    memberCount,
    allResolved: true,
    allSourceBacked: memberCount > 0,
    sameFile: true,
    sameParent: true,
    hasAncestorDescendant: false,
    hasDuplicateSourceTargets: false,
    canBatchAttributes: false,
    canBatchDuplicate: false,
    canBatchDelete: false,
    canBatchMove: false,
    primaryOnlyEditsAllowed: true,
    primaryOnlyReasonCode: null,
    reasons: [],
  };
}

function htmlFacts() {
  return {
    complete: false,
    commonClasses: [],
    mixedClasses: [],
    commonAttributes: {},
    mixedAttributeNames: [],
  };
}

function inspectorSummary(selectionRevision, state = "resolving", renderInstanceId = null) {
  const resolved = state === "resolved";
  const empty = state === "empty";
  const reason = resolved ? null : empty ? "noSelection" : "awaitingPhysicalFacts";
  return {
    schemaVersion: 3,
    projectRoot: identity.projectRoot,
    runtimeSessionId: identity.runtimeSessionId,
    selectionRevision,
    canvasIdentity: identity,
    documentEpoch: resolved ? 1 : null,
    renderInstanceId,
    state,
    subjectKind: empty ? null : "htmlElement",
    boundaryKind: null,
    componentKind: null,
    tag: resolved ? "h2" : null,
    label: resolved ? "<h2>" : null,
    selector: resolved ? "h2" : null,
    elementId: resolved ? "heading" : null,
    classes: resolved ? ["heading"] : [],
    blockContext: null,
    activeCssClass: resolved ? "heading" : null,
    canInspect: !empty,
    reason,
    diagnostics: reason ? [{ code: reason, message: reason }] : [],
  };
}

function selection(selectionRevision, editorNodeId = null, renderInstanceId = null) {
  const members = editorNodeId
    ? [{
        memberId: editorNodeId,
        resolution: "resolved",
        subject: {
          kind: "htmlElement",
          boundaryKind: null,
          componentKind: null,
          tag: "h2",
          label: "<h2>",
        },
        anchor: {
          editorNodeId,
          sourceNodeId: `source:${editorNodeId}`,
          renderInstanceId,
          renderInstanceIds: renderInstanceId ? [renderInstanceId] : [],
        },
        provenance: { definition: null, composition: null, resolution: "direct" },
        capabilities: {},
        diagnostics: [],
      }]
    : [];
  return {
    schemaVersion: 3,
    selectionRevision,
    projectRoot: identity.projectRoot,
    runtimeSessionId: identity.runtimeSessionId,
    canvasIdentity: identity,
    route: "/workbench/",
    activeDocumentPath: "templates/index.html",
    primaryMemberId: editorNodeId,
    rangeOriginMemberId: editorNodeId,
    members,
    aggregateCapabilities: aggregate(members.length),
    aggregateHtmlFacts: htmlFacts(),
    focus: { kind: "element" },
    diagnostics: [],
  };
}

function coordinatorReceipt(selectionRevision, editorNodeId = null, renderInstanceId = null) {
  return {
    schemaVersion: 3,
    selection: selection(selectionRevision, editorNodeId, renderInstanceId),
    hover: null,
    inspectorSummary: inspectorSummary(
      selectionRevision,
      editorNodeId ? "resolving" : "empty",
    ),
  };
}

function navigationSnapshot() {
  return {
    schemaVersion: 4,
    identity,
    modelRevision: "model:7",
    route: "/workbench/",
    surface: "templateWorkbench",
    rootNodeIds: [],
    nodes: [],
    focusedView: {
      activeDocumentPath: "templates/index.html",
      activeTemplateName: "index.html",
      activeSourceNodeId: "source:template",
      breadcrumbs: [],
      rootNodeIds: [],
      nodes: [],
      previewContextRenderInstanceId: null,
    },
    diagnostics: [],
  };
}

function canvasObservation(id) {
  return {
    selector: `#${id}`,
    cssSelector: `#${id}`,
    domPath: `body > #${id}`,
    tag: "h2",
    id,
    href: "",
    title: "",
    alt: "",
    classes: ["heading"],
    text: id,
    rawText: id,
    hasChildElements: false,
    rect: { width: "100px", height: "20px", top: "0px", left: "0px" },
    styles: [],
    variables: [],
    matchedRules: [],
    imageSrc: null,
    zolaImage: null,
    attributes: { id },
    parentNode: null,
    childNodes: [],
    blockContext: null,
  };
}

function createController() {
  const projected = [];
  const appliedObservations = [];
  const host = {
    activeCanvasIdentity: identity,
    activeCanvasUrl: "http://127.0.0.1:4000/workbench/",
    activeScannedPath: "templates/index.html",
    browserPreviewRoute: "/",
    coordinatedElementSelection: null,
    previewSrc: "http://127.0.0.1:4000/workbench/",
    projectWorkspaceSnapshot: { revision: 7 },
    targetCssFile: "styles.css",
    applySelectionState(observation) {
      appliedObservations.push(observation);
    },
    projectSelectionSnapshotOnCanvas(snapshot) {
      projected.push(snapshot);
    },
  };
  const controller = new EditorSelectionSessionController(() => host);
  return { controller, host, projected, appliedObservations };
}

afterEach(() => clearMocks());

test("identical navigation refreshes share one Rust request", async () => {
  const navigation = deferred();
  const commands = [];
  mockIPC(async (command) => {
    commands.push(command);
    if (command === "read_editor_navigation_snapshot") return await navigation.promise;
    if (command === "read_selection_snapshot") return coordinatorReceipt(1);
    throw new Error(`unexpected command: ${command}`);
  });
  const { controller } = createController();

  const first = controller.refreshNavigationSnapshot(identity, "/workbench/");
  const second = controller.refreshNavigationSnapshot(identity, "/workbench/");
  assert.deepEqual(commands, ["read_editor_navigation_snapshot"]);

  navigation.resolve(navigationSnapshot());
  await Promise.all([first, second]);

  assert.deepEqual(commands, [
    "read_editor_navigation_snapshot",
    "read_selection_snapshot",
  ]);
  assert.equal(controller.navigationSnapshot?.modelRevision, "model:7");
  assert.equal(controller.diagnostics.navigationRequests, 1);
  assert.equal(controller.diagnostics.navigationRefreshesDeduplicated, 1);
  assert.equal(controller.diagnostics.staleNavigationResponses, 0);
});

test("a superseded selection receipt cannot overwrite the latest selection", async () => {
  const pending = [deferred(), deferred()];
  let requestIndex = 0;
  mockIPC(async (command) => {
    assert.equal(command, "apply_selection_intent");
    return await pending[requestIndex++].promise;
  });
  const { controller } = createController();

  const first = controller.applySelectionIntent({
    kind: "selectEditorNode",
    editorNodeId: "editor:a",
  });
  const second = controller.applySelectionIntent({
    kind: "selectEditorNode",
    editorNodeId: "editor:b",
  });
  pending[1].resolve(coordinatorReceipt(3, "editor:b", "render:b"));
  assert.equal((await second)?.primaryMemberId, "editor:b");
  pending[0].resolve(coordinatorReceipt(2, "editor:a", "render:a"));

  assert.equal(await first, null);
  assert.equal(controller.selectionSnapshot?.primaryMemberId, "editor:b");
  assert.equal(controller.diagnostics.selectionRequests, 2);
  assert.equal(controller.diagnostics.staleSelectionResponses, 1);
});

test("a superseded Canvas hover projection cannot overwrite the latest hover", () => {
  const { controller } = createController();
  const first = controller.beginCanvasHoverProjection();
  const second = controller.beginCanvasHoverProjection();

  assert.equal(controller.projectCanvasHoverReceipt(first, identity, {
    schemaVersion: 3,
    hoverRevision: 1,
    canvasIdentity: identity,
    memberId: "editor:a",
    renderInstanceId: "render:a",
  }), false);
  assert.equal(controller.projectCanvasHoverReceipt(second, identity, {
    schemaVersion: 3,
    hoverRevision: 2,
    canvasIdentity: identity,
    memberId: "editor:b",
    renderInstanceId: "render:b",
  }), true);

  assert.equal(controller.hoverSnapshot?.renderInstanceId, "render:b");
  assert.equal(controller.diagnostics.hoverRequests, 2);
  assert.equal(controller.diagnostics.hoverIntentRequests, 0);
  assert.equal(controller.diagnostics.hoverProjectionRequests, 2);
  assert.equal(controller.diagnostics.staleHoverResponses, 1);
});

test("a physical observation is discarded when selection changes in flight", async () => {
  const observation = deferred();
  mockIPC(async (command) => {
    assert.equal(command, "accept_selection_observation");
    return await observation.promise;
  });
  const { controller, appliedObservations } = createController();
  controller.selectionSnapshot = selection(4, "editor:a", "render:a");
  const input = {
    schemaVersion: 3,
    selectionRevision: 4,
    canvasIdentity: identity,
    documentEpoch: 1,
    renderInstanceId: "render:a",
    inspectorFacts: {
      observedTag: "h2",
      elementId: "heading",
      classes: ["heading"],
      blockContext: null,
    },
  };
  const pending = controller.acceptObservation(input, {
    tag: "h2",
    id: "heading",
    classes: ["heading"],
  });
  controller.selectionSnapshot = selection(5, "editor:b", "render:b");
  observation.resolve({
    schemaVersion: 3,
    selectionRevision: 4,
    canvasIdentity: identity,
    documentEpoch: 1,
    renderInstanceId: "render:a",
    inspectorSummary: inspectorSummary(4, "resolved", "render:a"),
  });

  assert.equal(await pending, null);
  assert.equal(controller.acceptedObservation, null);
  assert.deepEqual(appliedObservations, []);
  assert.equal(controller.diagnostics.staleObservationResponses, 1);
});

test("an older physical observation cannot overwrite a newer one for the same selection", async () => {
  const observations = [deferred(), deferred()];
  let requestIndex = 0;
  mockIPC(async (command) => {
    assert.equal(command, "accept_selection_observation");
    return await observations[requestIndex++].promise;
  });
  const { controller, appliedObservations } = createController();
  controller.selectionSnapshot = selection(4, "editor:a", "render:a");
  const input = {
    schemaVersion: 3,
    selectionRevision: 4,
    canvasIdentity: identity,
    documentEpoch: 1,
    renderInstanceId: "render:a",
    inspectorFacts: {
      observedTag: "h2",
      elementId: "new",
      classes: ["heading"],
      blockContext: null,
    },
  };
  const oldObservation = controller.acceptObservation(input, canvasObservation("old"));
  const newObservation = controller.acceptObservation(input, canvasObservation("new"));
  const receipt = {
    schemaVersion: 3,
    selectionRevision: 4,
    canvasIdentity: identity,
    documentEpoch: 1,
    renderInstanceId: "render:a",
    inspectorSummary: inspectorSummary(4, "resolved", "render:a"),
  };
  observations[1].resolve(receipt);
  assert.equal((await newObservation)?.observation.id, "new");
  observations[0].resolve(receipt);

  assert.equal(await oldObservation, null);
  assert.equal(controller.acceptedObservation?.observation.id, "new");
  assert.equal(appliedObservations.length, 1);
  assert.equal(appliedObservations[0].id, "new");
  assert.equal(controller.diagnostics.observationRequests, 2);
  assert.equal(controller.diagnostics.staleObservationResponses, 1);
});

test("reset invalidates an in-flight navigation response", async () => {
  const navigation = deferred();
  mockIPC(async (command) => {
    assert.equal(command, "read_editor_navigation_snapshot");
    return await navigation.promise;
  });
  const { controller } = createController();
  const pending = controller.refreshNavigationSnapshot(identity, "/workbench/");
  controller.reset();
  navigation.resolve(navigationSnapshot());
  await pending;

  assert.equal(controller.navigationSnapshot, null);
  assert.equal(controller.selectionSnapshot, null);
  assert.equal(controller.diagnostics.staleNavigationResponses, 1);
});
