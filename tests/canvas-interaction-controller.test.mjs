import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  handleCanvasAgentMessage,
  projectSelectionSnapshotOnCanvas,
  synchronizeCanvasInteractionBinding,
} from "$lib/state/canvas-interaction-controller";
import {
  CANVAS_AGENT_MESSAGE_SCHEMA_VERSION,
  CANVAS_AGENT_MESSAGE_SOURCE,
} from "$lib/preview/canvas-interaction";
import { contextMenu } from "$lib/context-menu/store.svelte";
import { canvasInteractionRuntimeFor } from "$lib/state/canvas-interaction-runtime";
import { CANVAS_INTERACTION_SCHEMA_VERSION } from "$lib/canvas/contracts";

if (!globalThis.window) globalThis.window = globalThis;

const canvas = {
  projectRoot: "/project",
  runtimeSessionId: "runtime-1",
  workspaceRevision: 7,
  transactionId: "canvas-7",
  previewRevision: "preview-7",
};
const route = "/__pana_workbench/page/";

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

async function nextTurn() {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

function pointer() {
  return {
    clientX: 20,
    clientY: 30,
    button: "none",
    buttons: 0,
    modifiers: { alt: false, control: false, meta: false, shift: false },
  };
}

function navigationNode(id = "editor:heading", renderInstanceId = "render:heading") {
  return {
    id,
    kind: "htmlElement",
    label: "Heading",
    tag: "h2",
    sourceNodeId: "source:heading",
    sourceKind: "htmlElement",
    file: "templates/index.html",
    range: null,
    renderInstanceId,
    boundary: null,
    origin: "project",
    themeName: null,
    sourceProvenance: "canonical",
    capabilities: {
      requiresEditScopeId: null,
      canSelect: true,
      canInspect: true,
      canOpenInCode: true,
      canEnterBoundary: false,
      canMoveAtomic: false,
      canMove: true,
      canEditText: true,
      canEditAttributes: true,
      readOnly: false,
      reasonCode: null,
    },
  };
}

function focusedSnapshot(activeDocumentPath, nodes = []) {
  return {
    schemaVersion: 4,
    identity: canvas,
    modelRevision: "model-7",
    route,
    surface: "templateWorkbench",
    rootNodeIds: nodes.map((node) => node.id),
    nodes,
    focusedView: {
      activeDocumentPath,
      activeTemplateName: activeDocumentPath,
      activeSourceNodeId: "source:template",
      breadcrumbs: [],
      rootNodeIds: nodes.map((node) => node.id),
      nodes: [],
      previewContextRenderInstanceId: null,
    },
    diagnostics: [],
  };
}

function selectionSnapshot(revision = 5) {
  return {
    schemaVersion: 3,
    selectionRevision: revision,
    canvasIdentity: canvas,
    resolution: "resolved",
    primaryMemberId: "editor:heading",
    rangeOriginMemberId: "editor:heading",
    members: [{
      memberId: "editor:heading",
      resolution: "resolved",
      anchor: { editorNodeId: "editor:heading" },
    }],
    aggregateCapabilities: { memberCount: 1 },
  };
}

function physicalObservation() {
  return {
    selector: "h2.heading",
    cssSelector: "h2.heading",
    domPath: "main > h2",
    tag: "h2",
    id: "heading",
    href: "",
    title: "",
    alt: "",
    classes: ["heading"],
    text: "Heading",
    rawText: "Heading",
    hasChildElements: false,
    rect: { width: "100px", height: "30px", top: "0px", left: "0px" },
    styles: [],
    variables: [],
    matchedRules: [],
    imageSrc: null,
    zolaImage: null,
    attributes: { class: "heading" },
    parentNode: null,
    childNodes: [],
    renderInstanceId: "render:heading",
    blockContext: null,
  };
}

function createHost() {
  const contentWindow = {};
  const messages = [];
  const statuses = [];
  const projectedHovers = [];
  const selectionIntents = [];
  const observations = [];
  const host = {
    activeCanvasIdentity: canvas,
    activeCanvasUrl: `http://127.0.0.1:41000${route}`,
    previewSrc: `http://127.0.0.1:41000${route}`,
    browserPreviewRoute: "/",
    applicationSurface: "workbench",
    workbenchSnapshot: { activeActivity: "editor" },
    centerView: "preview",
    activeScannedPath: "templates/index.html",
    scannedProject: {
      files: [
        { relativePath: "templates/index.html", role: "template" },
        { relativePath: "templates/page.html", role: "template" },
      ],
    },
    editorSelection: {
      navigationSnapshot: focusedSnapshot("templates/index.html", [navigationNode()]),
      selectionSnapshot: null,
      editScopeGrant: null,
      editScopeId: null,
      beginCanvasHoverProjection() {
        return projectedHovers.length + 1;
      },
      projectCanvasHoverReceipt(serial, identity, hover) {
        projectedHovers.push({ serial, identity, hover });
        return true;
      },
      async applySelectionIntent(intent) {
        selectionIntents.push(intent);
        return this.selectionSnapshot;
      },
      async applyHoverIntent() {},
      async acceptObservation(input, observation) {
        observations.push({ input, observation });
        return true;
      },
      clearSelectionProjection() {},
      async refreshNavigationSnapshot() {},
    },
    coordinatedElementSelection: null,
    editorRuntime: { async dispatch() {} },
    gridOverlayEnabled: false,
    previewFrame: {
      contentWindow,
      getBoundingClientRect: () => ({ left: 100, top: 50 }),
    },
    sourceGraph: null,
    closeContextMenu() {},
    async moveEditorNavigationNode() {
      return { status: "committed" };
    },
    postPreviewMessage(message) {
      messages.push(message);
    },
    async previewEditorNavigationMove() {
      throw new Error("unexpected move plan");
    },
    async recordCanvasProjectionRuntimeEvent() {},
    setGlobalStatus(text, kind) {
      statuses.push({ text, kind });
    },
    syncCodeSelectionHighlight() {},
  };
  host.session = {
    get activeCanvasIdentity() { return host.activeCanvasIdentity; },
    get activeCanvasUrl() { return host.activeCanvasUrl; },
    get activeScannedPath() { return host.activeScannedPath; },
    get applicationSurface() { return host.applicationSurface; },
    get browserPreviewRoute() { return host.browserPreviewRoute; },
    get centerView() { return host.centerView; },
    get previewFrame() { return host.previewFrame; },
    get previewSrc() { return host.previewSrc; },
    get scannedProject() { return host.scannedProject; },
    get workbenchSnapshot() { return host.workbenchSnapshot; },
  };
  host.selection = {
    get coordinatedElementSelection() { return host.coordinatedElementSelection; },
    editorSelection: host.editorSelection,
    get sourceGraph() { return host.sourceGraph; },
  };
  host.runtime = {
    editorRuntime: host.editorRuntime,
    get gridOverlayEnabled() { return host.gridOverlayEnabled; },
  };
  host.commands = {
    closeContextMenu: (...args) => host.closeContextMenu(...args),
    moveEditorNavigationNode: (...args) => host.moveEditorNavigationNode(...args),
    postPreviewMessage: (...args) => host.postPreviewMessage(...args),
    previewEditorNavigationMove: (...args) => host.previewEditorNavigationMove(...args),
    recordCanvasProjectionRuntimeEvent: (...args) => host.recordCanvasProjectionRuntimeEvent(...args),
    setGlobalStatus: (...args) => host.setGlobalStatus(...args),
    syncCodeSelectionHighlight: (...args) => host.syncCodeSelectionHighlight(...args),
  };
  return {
    host,
    contentWindow,
    messages,
    observations,
    projectedHovers,
    selectionIntents,
    statuses,
  };
}

function agentEvent(contentWindow, data) {
  return {
    source: contentWindow,
    data: {
      source: CANVAS_AGENT_MESSAGE_SOURCE,
      schemaVersion: CANVAS_AGENT_MESSAGE_SCHEMA_VERSION,
      agentInstanceId: "agent-1",
      ...data,
    },
  };
}

function gestureEvent(contentWindow, documentEpoch, gestureSequence, gesture, overrides = {}) {
  return agentEvent(contentWindow, {
    type: "gesture",
    documentEpoch,
    emittedAtMs: Date.now(),
    gestureSequence,
    gesture,
    pointer: pointer(),
    hitPath: [],
    drag: null,
    ...overrides,
  });
}

function bindingReceipt(input) {
  return {
    schemaVersion: CANVAS_INTERACTION_SCHEMA_VERSION,
    identity: input.identity,
    lastAcceptedSequence: 0,
    activeDocumentPath: input.activeDocumentPath,
    authoringSurfaces: [],
  };
}

function interactionReceipt(request, status = "noTarget") {
  return {
    schemaVersion: CANVAS_INTERACTION_SCHEMA_VERSION,
    identity: request.identity,
    gestureSequence: request.gestureSequence,
    gesture: request.gesture,
    status,
    target: null,
    overlay: null,
    dragPosition: status === "resolved" && request.gesture === "dragOver"
      ? request.drag?.position ?? null
      : null,
    diagnostics: [],
  };
}

function interactionTarget(
  editorNodeId,
  renderInstanceId,
  overrides = {},
) {
  return {
    editorNodeId,
    kind: "htmlElement",
    boundaryKind: null,
    componentKind: null,
    label: "Heading",
    tag: "h2",
    sourceNodeId: `source:${editorNodeId}`,
    file: "templates/index.html",
    range: null,
    renderInstanceId,
    boundaryInstanceId: null,
    origin: "project",
    themeName: null,
    sourceProvenance: "canonical",
    requiredEditScopeId: null,
    scopeState: "unscoped",
    effectScope: "singleSource",
    renderedInstanceCount: 1,
    actions: {
      canSelect: true,
      canInspect: true,
      canOpenInCode: true,
      canEnterBoundary: false,
      canMoveAtomic: false,
      canMove: true,
      canEditText: true,
      canEditAttributes: true,
      readOnly: false,
      reasonCode: null,
    },
    ...overrides,
  };
}

function resolvedInteractionReceipt(request, target) {
  return {
    ...interactionReceipt(request, "resolved"),
    target,
    overlay: {
      primaryRenderInstanceId: target.renderInstanceId,
      renderInstanceIds: target.renderInstanceId ? [target.renderInstanceId] : [],
      boundaryInstanceId: target.boundaryInstanceId,
    },
  };
}

function hoverReceipt(request, projection) {
  return {
    schemaVersion: CANVAS_INTERACTION_SCHEMA_VERSION,
    interaction: interactionReceipt(request),
    projection,
    timings: {
      emittedAtMs: request.emittedAtMs,
      rustReceivedAtMs: request.emittedAtMs,
      rustCompletedAtMs: request.emittedAtMs,
      inputToProjectionDurationMs: 0,
      rustDurationMs: 0,
    },
  };
}

function allowedMovePlan(sourceNodeId, targetNodeId, position) {
  const token = "move-plan-1";
  return {
    schemaVersion: 3,
    token,
    allowed: true,
    reasonCode: null,
    reason: null,
    operation: "htmlSourceMove",
    identity: canvas,
    modelRevision: "model-7",
    route,
    activeDocumentPath: "templates/index.html",
    sourceNodeId,
    targetNodeId,
    position,
    impact: {
      files: ["templates/index.html"],
      editScopeId: null,
      effectScope: "singleSource",
      renderedInstanceCount: 1,
      affectsAllRenderedInstances: false,
      requiresPreviewReprojection: false,
    },
    liveProjection: {
      schemaVersion: 1,
      operation: "move",
      scope: "selectedInstance",
      planToken: token,
      identity: canvas,
      sourceRenderInstanceId: "render:source",
      targetRenderInstanceId: "render:target",
      position,
      rollback: {
        sourceParentRenderInstanceId: "render:parent",
        sourceNextSiblingRenderInstanceId: null,
      },
    },
    liveProjectionReason: "ready",
    issuedAtMs: Date.now(),
  };
}

async function activate(host, contentWindow, bindInputs) {
  handleCanvasAgentMessage(host, agentEvent(contentWindow, {
    type: "agentReady",
  }));
  await nextTurn();
  const binding = bindInputs.at(-1);
  handleCanvasAgentMessage(host, agentEvent(contentWindow, {
    type: "agentActivated",
    documentEpoch: binding.identity.documentEpoch,
  }));
  return binding.identity.documentEpoch;
}

afterEach(() => {
  contextMenu.close();
  clearMocks();
});

test("suspending and returning to Editor reactivates the retained exact binding", async () => {
  const { host, contentWindow, messages, statuses } = createHost();
  const bindInputs = [];
  mockIPC((command, payload) => {
    assert.equal(command, "bind_canvas_interaction_agent");
    bindInputs.push(payload.input);
    return bindingReceipt(payload.input);
  });
  const documentEpoch = await activate(host, contentWindow, bindInputs);

  host.workbenchSnapshot.activeActivity = "templates";
  synchronizeCanvasInteractionBinding(host);
  assert.equal(messages.at(-1).type, "deactivate-canvas-interaction-agent");

  host.workbenchSnapshot.activeActivity = "editor";
  synchronizeCanvasInteractionBinding(host);
  const activation = messages.at(-1);
  assert.equal(activation.type, "activate-canvas-interaction-agent");
  assert.equal(activation.documentEpoch, documentEpoch);
  assert.equal(bindInputs.length, 1);

  handleCanvasAgentMessage(host, agentEvent(contentWindow, {
    type: "agentActivated",
    documentEpoch,
  }));
  assert.deepEqual(statuses, []);
});

test("Canvas hover retains only the latest pending Rust receipt", async () => {
  const { host, contentWindow, projectedHovers, statuses } = createHost();
  const bindInputs = [];
  const firstHover = deferred();
  const hoverRequests = [];
  mockIPC(async (command, payload) => {
    if (command === "bind_canvas_interaction_agent") {
      bindInputs.push(payload.input);
      return bindingReceipt(payload.input);
    }
    assert.equal(command, "resolve_canvas_hover_intent");
    hoverRequests.push(payload.input.request);
    if (hoverRequests.length === 1) return await firstHover.promise;
    return hoverReceipt(
      payload.input.request,
      { changed: true, hover: null },
    );
  });
  const documentEpoch = await activate(host, contentWindow, bindInputs);

  handleCanvasAgentMessage(host, gestureEvent(
    contentWindow,
    documentEpoch,
    1,
    "pointerMove",
  ));
  await nextTurn();
  handleCanvasAgentMessage(host, gestureEvent(
    contentWindow,
    documentEpoch,
    2,
    "pointerMove",
  ));
  firstHover.resolve(hoverReceipt(
    hoverRequests[0],
    { changed: true, hover: null },
  ));
  await nextTurn();
  await nextTurn();

  assert.deepEqual(hoverRequests.map((request) => request.gestureSequence), [1, 2]);
  assert.equal(projectedHovers.length, 1);
  assert.deepEqual(projectedHovers[0].hover, null);
  assert.deepEqual(statuses, []);
});

test("click applies the Rust target and requests DOM inspection for the selected element", async () => {
  const { host, contentWindow, messages, selectionIntents } = createHost();
  const bindInputs = [];
  const selected = selectionSnapshot(8);
  host.editorSelection.selectionSnapshot = selected;
  mockIPC((command, payload) => {
    if (command === "bind_canvas_interaction_agent") {
      bindInputs.push(payload.input);
      return bindingReceipt(payload.input);
    }
    assert.equal(command, "resolve_canvas_interaction_intent");
    const target = interactionTarget("editor:heading", "render:heading");
    return resolvedInteractionReceipt(payload.input.request, target);
  });
  const documentEpoch = await activate(host, contentWindow, bindInputs);

  handleCanvasAgentMessage(host, gestureEvent(
    contentWindow,
    documentEpoch,
    1,
    "click",
  ));
  await nextTurn();
  await nextTurn();

  assert.deepEqual(selectionIntents, [{
    kind: "selectEditorNode",
    editorNodeId: "editor:heading",
  }]);
  assert.ok(messages.some((message) => (
    message.type === "render-canvas-interaction-overlay"
    && message.channel === "selection"
    && message.selectionRevision === selected.selectionRevision
  )));
  assert.ok(messages.some((message) => (
    message.type === "inspect-canvas-interaction-target"
    && message.renderInstanceId === "render:heading"
  )));
});

test("context menu keeps an already selected Tera target primary and opens its menu", async () => {
  const { host, contentWindow, selectionIntents, statuses } = createHost();
  const teraNode = {
    ...navigationNode("editor:tera", null),
    kind: "boundary",
    label: "content",
    tag: null,
    sourceNodeId: "source:tera",
    boundary: {
      kind: "component",
      componentKind: "repeat",
      boundaryInstanceId: "boundary:tera",
      sourceNodeId: "source:tera",
      rootRenderInstanceIds: ["render:tera"],
      atomicWhenClosed: true,
      effectScope: "singleSource",
      renderedInstanceCount: 1,
      target: null,
      empty: false,
    },
  };
  host.editorSelection.navigationSnapshot = focusedSnapshot(
    "templates/index.html",
    [teraNode],
  );
  host.editorSelection.selectionSnapshot = {
    ...selectionSnapshot(9),
    primaryMemberId: "editor:tera",
    rangeOriginMemberId: "editor:tera",
    members: [{
      memberId: "editor:tera",
      resolution: "resolved",
      anchor: { editorNodeId: "editor:tera" },
    }],
  };
  const bindInputs = [];
  const openedMenus = [];
  const originalOpen = contextMenu.open;
  contextMenu.open = (request) => openedMenus.push(request);
  mockIPC((command, payload) => {
    if (command === "bind_canvas_interaction_agent") {
      bindInputs.push(payload.input);
      return bindingReceipt(payload.input);
    }
    assert.equal(command, "resolve_canvas_interaction_intent");
    return resolvedInteractionReceipt(payload.input.request, interactionTarget(
      "editor:tera",
      "render:tera",
      {
        kind: "boundary",
        boundaryKind: "component",
        componentKind: "repeat",
        label: "content",
        tag: null,
        sourceNodeId: "source:tera",
        boundaryInstanceId: "boundary:tera",
        actions: {
          ...interactionTarget("unused", null).actions,
          canInspect: false,
          canEnterBoundary: true,
          canMoveAtomic: true,
        },
      },
    ));
  });
  const documentEpoch = await activate(host, contentWindow, bindInputs);

  handleCanvasAgentMessage(host, gestureEvent(
    contentWindow,
    documentEpoch,
    1,
    "contextMenu",
  ));
  await nextTurn();
  await nextTurn();
  contextMenu.open = originalOpen;

  assert.deepEqual(selectionIntents, [{
    kind: "setPrimaryEditorNode",
    editorNodeId: "editor:tera",
  }]);
  assert.equal(openedMenus[0]?.source, "preview", JSON.stringify(statuses));
  assert.equal(openedMenus[0]?.title, "content");
  assert.ok(openedMenus[0]?.items.some((item) => item.id === "preview-delete-tera"));
});

test("drag start, latest DragOver plan and Drop commit one Rust-authorized move", async () => {
  const { host, contentWindow, messages, statuses } = createHost();
  const bindInputs = [];
  const previewCalls = [];
  const moveCalls = [];
  const source = interactionTarget("editor:source", "render:source");
  const target = interactionTarget("editor:target", "render:target");
  const plan = allowedMovePlan(source.editorNodeId, target.editorNodeId, "before");
  host.previewEditorNavigationMove = async (...args) => {
    previewCalls.push(args);
    return plan;
  };
  host.moveEditorNavigationNode = async (...args) => {
    moveCalls.push(args);
    return { status: "committed" };
  };
  mockIPC((command, payload) => {
    if (command === "bind_canvas_interaction_agent") {
      bindInputs.push(payload.input);
      return bindingReceipt(payload.input);
    }
    if (command === "resolve_canvas_drag_over_intent") {
      return {
        schemaVersion: CANVAS_INTERACTION_SCHEMA_VERSION,
        interaction: resolvedInteractionReceipt(payload.input.request, target),
        plan,
        timings: {
          emittedAtMs: payload.input.request.emittedAtMs,
          rustReceivedAtMs: payload.input.request.emittedAtMs,
          rustCompletedAtMs: payload.input.request.emittedAtMs,
          inputToPlanDurationMs: 0,
          inputToFirstAllowedPlanMs: 0,
          rustDurationMs: 0,
        },
      };
    }
    assert.equal(command, "resolve_canvas_interaction_intent");
    const receiptTarget = payload.input.request.gesture === "dragStart" ? source : target;
    return resolvedInteractionReceipt(payload.input.request, receiptTarget);
  });
  const documentEpoch = await activate(host, contentWindow, bindInputs);

  handleCanvasAgentMessage(host, gestureEvent(
    contentWindow,
    documentEpoch,
    1,
    "dragStart",
    { drag: { sessionId: "drag-1", position: null } },
  ));
  await nextTurn();
  await nextTurn();
  assert.equal(
    canvasInteractionRuntimeFor(host).dragSource?.target.editorNodeId,
    "editor:source",
  );
  handleCanvasAgentMessage(host, gestureEvent(
    contentWindow,
    documentEpoch,
    2,
    "dragOver",
    { drag: { sessionId: "drag-1", position: "before" } },
  ));
  await nextTurn();
  await nextTurn();

  assert.deepEqual(previewCalls, []);
  assert.equal(
    canvasInteractionRuntimeFor(host).dragMovePreview?.plan,
    plan,
  );
  assert.ok(messages.some((message) => (
    message.type === "render-canvas-interaction-overlay"
    && message.channel === "drag"
    && message.dragPermission?.state === "allowed"
  )));

  handleCanvasAgentMessage(host, gestureEvent(
    contentWindow,
    documentEpoch,
    3,
    "drop",
    { drag: { sessionId: "drag-1", position: "before" } },
  ));
  await nextTurn();
  await nextTurn();
  await nextTurn();

  assert.equal(moveCalls.length, 1);
  assert.deepEqual(moveCalls[0].slice(0, 4), [
    "editor:source",
    "editor:target",
    "before",
    plan,
  ]);
  assert.ok(messages.some((message) => (
    message.type === "project-canvas-drag-preview"
    && message.dragSessionId === "drag-1"
    && message.projection.planToken === plan.token
  )));
  assert.deepEqual(statuses, []);
});

test("a fallback Drop plan resolved after document switch cannot mutate the new binding", async () => {
  const { host, contentWindow, messages, statuses } = createHost();
  const bindInputs = [];
  const fallbackPlan = deferred();
  const previewCalls = [];
  const moveCalls = [];
  const source = interactionTarget("editor:source", "render:source");
  const target = interactionTarget("editor:target", "render:target");
  const plan = allowedMovePlan(source.editorNodeId, target.editorNodeId, "before");
  host.previewEditorNavigationMove = (...args) => {
    previewCalls.push(args);
    return fallbackPlan.promise;
  };
  host.moveEditorNavigationNode = async (...args) => {
    moveCalls.push(args);
    return { status: "committed" };
  };
  mockIPC((command, payload) => {
    if (command === "bind_canvas_interaction_agent") {
      bindInputs.push(payload.input);
      return bindingReceipt(payload.input);
    }
    assert.equal(command, "resolve_canvas_interaction_intent");
    const receiptTarget = payload.input.request.gesture === "dragStart" ? source : target;
    return resolvedInteractionReceipt(payload.input.request, receiptTarget);
  });
  const firstEpoch = await activate(host, contentWindow, bindInputs);

  handleCanvasAgentMessage(host, gestureEvent(
    contentWindow,
    firstEpoch,
    1,
    "dragStart",
    { drag: { sessionId: "drag-stale", position: null } },
  ));
  await nextTurn();
  await nextTurn();
  handleCanvasAgentMessage(host, gestureEvent(
    contentWindow,
    firstEpoch,
    2,
    "drop",
    { drag: { sessionId: "drag-stale", position: "before" } },
  ));
  await nextTurn();
  await nextTurn();
  assert.deepEqual(previewCalls, [["editor:source", "editor:target", "before"]]);

  host.activeScannedPath = "templates/page.html";
  host.editorSelection.navigationSnapshot = focusedSnapshot("templates/page.html");
  synchronizeCanvasInteractionBinding(host);
  await nextTurn();
  const secondBinding = bindInputs.at(-1);
  handleCanvasAgentMessage(host, agentEvent(contentWindow, {
    type: "agentActivated",
    documentEpoch: secondBinding.identity.documentEpoch,
  }));
  const projectedBeforeStalePlan = messages.filter(
    (message) => message.type === "project-canvas-drag-preview",
  ).length;

  fallbackPlan.resolve(plan);
  await nextTurn();
  await nextTurn();

  assert.deepEqual(moveCalls, []);
  assert.equal(
    messages.filter((message) => message.type === "project-canvas-drag-preview").length,
    projectedBeforeStalePlan,
  );
  assert.deepEqual(statuses, []);
});

test("active binding validation does not rescan the project file list per hover", async () => {
  const { host, contentWindow } = createHost();
  const files = host.scannedProject.files;
  let fileScans = 0;
  host.scannedProject.files = new Proxy(files, {
    get(target, property, receiver) {
      if (property === "find") {
        return (predicate) => {
          fileScans += 1;
          return target.find(predicate);
        };
      }
      return Reflect.get(target, property, receiver);
    },
  });
  const bindInputs = [];
  mockIPC((command, payload) => {
    if (command === "bind_canvas_interaction_agent") {
      bindInputs.push(payload.input);
      return bindingReceipt(payload.input);
    }
    assert.equal(command, "resolve_canvas_hover_intent");
    return hoverReceipt(
      payload.input.request,
      { changed: false, hover: null },
    );
  });
  const documentEpoch = await activate(host, contentWindow, bindInputs);
  assert.equal(fileScans, 1);

  for (let sequence = 1; sequence <= 3; sequence += 1) {
    handleCanvasAgentMessage(host, gestureEvent(
      contentWindow,
      documentEpoch,
      sequence,
      "pointerMove",
    ));
    await nextTurn();
  }

  assert.equal(fileScans, 1);
});

test("an ordered gesture failure from an old document cannot stop the new binding", async () => {
  const { host, contentWindow, messages, statuses } = createHost();
  const bindInputs = [];
  const oldGesture = deferred();
  mockIPC(async (command, payload) => {
    if (command === "bind_canvas_interaction_agent") {
      bindInputs.push(payload.input);
      return bindingReceipt(payload.input);
    }
    assert.equal(command, "resolve_canvas_interaction_intent");
    return await oldGesture.promise;
  });
  const firstEpoch = await activate(host, contentWindow, bindInputs);
  handleCanvasAgentMessage(host, gestureEvent(contentWindow, firstEpoch, 1, "click"));
  await nextTurn();

  host.activeScannedPath = "templates/page.html";
  host.editorSelection.navigationSnapshot = focusedSnapshot("templates/page.html");
  synchronizeCanvasInteractionBinding(host);
  await nextTurn();
  const secondBinding = bindInputs.at(-1);
  handleCanvasAgentMessage(host, agentEvent(contentWindow, {
    type: "agentActivated",
    documentEpoch: secondBinding.identity.documentEpoch,
  }));
  const deactivationsBeforeFailure = messages.filter(
    (message) => message.type === "deactivate-canvas-interaction-agent",
  ).length;

  oldGesture.reject(new Error("old document failed late"));
  await nextTurn();

  assert.equal(bindInputs.length, 2);
  assert.equal(
    messages.filter((message) => message.type === "deactivate-canvas-interaction-agent").length,
    deactivationsBeforeFailure,
  );
  assert.deepEqual(statuses, []);
});

test("a DOM inspection accepted after a document switch cannot project into the new document", async () => {
  const runtime = createHost();
  const { host, contentWindow, messages } = runtime;
  const bindInputs = [];
  const observation = deferred();
  let highlightCount = 0;
  host.editorSelection.selectionSnapshot = selectionSnapshot();
  host.editorSelection.acceptObservation = async (input, physical) => {
    runtime.observations.push({ input, physical });
    return await observation.promise;
  };
  host.syncCodeSelectionHighlight = () => {
    highlightCount += 1;
  };
  mockIPC((command, payload) => {
    assert.equal(command, "bind_canvas_interaction_agent");
    bindInputs.push(payload.input);
    return bindingReceipt(payload.input);
  });
  await activate(host, contentWindow, bindInputs);

  assert.equal(projectSelectionSnapshotOnCanvas(host, host.editorSelection.selectionSnapshot), true);
  const inspection = messages.filter(
    (message) => message.type === "inspect-canvas-interaction-target",
  ).at(-1);
  assert.ok(inspection);
  handleCanvasAgentMessage(host, agentEvent(contentWindow, {
    type: "domInspection",
    documentEpoch: inspection.documentEpoch,
    inspectionRequestId: inspection.inspectionRequestId,
    renderInstanceId: "render:heading",
    observation: physicalObservation(),
  }));
  await nextTurn();
  assert.equal(runtime.observations.length, 1);

  host.activeScannedPath = "templates/page.html";
  host.editorSelection.navigationSnapshot = focusedSnapshot("templates/page.html");
  synchronizeCanvasInteractionBinding(host);
  await nextTurn();
  const secondBinding = bindInputs.at(-1);
  handleCanvasAgentMessage(host, agentEvent(contentWindow, {
    type: "agentActivated",
    documentEpoch: secondBinding.identity.documentEpoch,
  }));
  observation.resolve(true);
  await nextTurn();

  assert.equal(highlightCount, 0);
});
