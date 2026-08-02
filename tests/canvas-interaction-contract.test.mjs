import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import {
  CANVAS_AGENT_MESSAGE_SOURCE,
  createCanvasInteractionIdentity,
  createCanvasInteractionRequest,
  parseCanvasAgentMessage,
} from "$lib/preview/canvas-interaction";
import { CANVAS_INTERACTION_SCHEMA_VERSION } from "$lib/types";

const canvas = {
  projectRoot: "/project",
  runtimeSessionId: "runtime-1",
  workspaceRevision: 17,
  transactionId: "canvas-17",
  previewRevision: "preview-17",
};

function pointer() {
  return {
    clientX: 120.5,
    clientY: 84,
    button: "primary",
    buttons: 1,
    modifiers: {
      alt: false,
      control: false,
      meta: false,
      shift: true,
    },
  };
}

function gestureData(overrides = {}) {
  return {
    source: CANVAS_AGENT_MESSAGE_SOURCE,
    schemaVersion: CANVAS_INTERACTION_SCHEMA_VERSION,
    type: "gesture",
    agentInstanceId: "agent-4",
    documentEpoch: 4,
    emittedAtMs: 1_785_460_000_000,
    gestureSequence: 9,
    gesture: "click",
    pointer: pointer(),
    hitPath: [
      { kind: "renderInstance", id: "render-card-title" },
      { kind: "renderInstance", id: "render-card" },
    ],
    drag: null,
    ...overrides,
  };
}

function physicalSelection(overrides = {}) {
  return {
    selector: "h1.hero-title",
    cssSelector: "h1.hero-title",
    domPath: "main > h1:nth-child(1)",
    tag: "h1",
    id: "",
    href: "",
    title: "",
    alt: "",
    classes: ["hero-title"],
    text: "Titlu",
    rawText: "Titlu",
    hasChildElements: false,
    rect: { width: "200px", height: "50px", top: "20px", left: "40px" },
    styles: [{ label: "color", value: "rgb(0, 0, 0)" }],
    variables: [{ name: "--accent", value: "#123456" }],
    matchedRules: [],
    imageSrc: null,
    zolaImage: null,
    attributes: {
      class: "hero-title",
      "data-pana-source-id": "source-spoof",
    },
    parentNode: null,
    childNodes: [],
    sourceLocation: { file: "spoof.html", line: 1 },
    sourceId: "source-spoof",
    templateSourceId: "template-spoof",
    sessionId: "session-physical",
    renderInstanceId: "render-card-title",
    blockContext: null,
    ...overrides,
  };
}

test("CanvasAgent ingress accepts only the exact frame and bounded physical facts", () => {
  const contentWindow = {};
  const frame = { contentWindow };
  const event = { source: contentWindow, data: gestureData() };

  const message = parseCanvasAgentMessage(frame, event, "agent-4");
  assert.equal(message?.type, "gesture");
  assert.equal(message?.gestureSequence, 9);
  assert.deepEqual(message?.hitPath, [
    { kind: "renderInstance", id: "render-card-title" },
    { kind: "renderInstance", id: "render-card" },
  ]);
  assert.equal(parseCanvasAgentMessage({ contentWindow: {} }, event, "agent-4"), null);
  assert.equal(parseCanvasAgentMessage(frame, event, "agent-old"), null);
});

test("CanvasAgent activation acknowledgement is exact and epoch-bound", () => {
  const contentWindow = {};
  const frame = { contentWindow };
  const activated = {
    source: CANVAS_AGENT_MESSAGE_SOURCE,
    schemaVersion: CANVAS_INTERACTION_SCHEMA_VERSION,
    type: "agentActivated",
    agentInstanceId: "agent-4",
    documentEpoch: 4,
  };
  assert.deepEqual(
    parseCanvasAgentMessage(
      frame,
      { source: contentWindow, data: activated },
      "agent-4",
    ),
    activated,
  );
  assert.equal(parseCanvasAgentMessage(
    frame,
    { source: contentWindow, data: { ...activated, documentEpoch: 0 } },
    "agent-4",
  ), null);
  assert.equal(parseCanvasAgentMessage(
    frame,
    { source: contentWindow, data: activated },
    "agent-old",
  ), null);
});

test("CanvasAgent ingress rejects malformed, duplicate and unbounded paths", () => {
  const contentWindow = {};
  const frame = { contentWindow };
  const parse = (data) => parseCanvasAgentMessage(
    frame,
    { source: contentWindow, data },
    "agent-4",
  );

  assert.equal(parse(gestureData({
    pointer: { ...pointer(), clientX: Number.NaN },
  })), null);
  assert.equal(parse(gestureData({
    hitPath: [
      { kind: "renderInstance", id: "render-card" },
      { kind: "renderInstance", id: "render-card" },
    ],
  })), null);
  assert.equal(parse(gestureData({
    hitPath: Array.from(
      { length: 65 },
      (_, index) => ({ kind: "renderInstance", id: `render-${index}` }),
    ),
  })), null);
  assert.equal(parse(gestureData({ gestureSequence: 0 })), null);
  assert.equal(parse(gestureData({ gesture: "semanticTeraSelection" })), null);
});

test("drag facts are physical, bounded and phase-specific", () => {
  const contentWindow = {};
  const frame = { contentWindow };
  const parse = (data) => parseCanvasAgentMessage(
    frame,
    { source: contentWindow, data },
    "agent-4",
  );
  const start = parse(gestureData({
    gesture: "dragStart",
    drag: { sessionId: "agent-4-drag-1", position: null },
  }));
  assert.equal(start?.type, "gesture");
  assert.equal(start?.drag?.sessionId, "agent-4-drag-1");
  assert.equal(parse(gestureData({
    gesture: "dragOver",
    drag: { sessionId: "agent-4-drag-1", position: "before" },
  }))?.drag?.position, "before");
  assert.equal(parse(gestureData({
    gesture: "drop",
    drag: { sessionId: "agent-4-drag-1", position: "inside" },
  }))?.type, "gesture");
  assert.equal(parse(gestureData({
    gesture: "drop",
    drag: { sessionId: "agent-4-drag-1", position: null },
  })), null);
  assert.equal(parse(gestureData({
    drag: { sessionId: "agent-4-drag-1", position: "inside" },
  })), null);
});

test("frontend creates the Rust request only for the active document binding", () => {
  const identity = createCanvasInteractionIdentity(canvas, "/", 4, "agent-4");
  const message = parseCanvasAgentMessage(
    { contentWindow: globalThis },
    { source: globalThis, data: gestureData() },
    "agent-4",
  );
  assert.ok(message && message.type === "gesture");

  const request = createCanvasInteractionRequest(identity, message);
  assert.equal(request.schemaVersion, CANVAS_INTERACTION_SCHEMA_VERSION);
  assert.deepEqual(request.identity.canvas, canvas);
  assert.equal(request.gesture, "click");
  assert.throws(
    () => createCanvasInteractionRequest(
      { ...identity, documentEpoch: 5 },
      message,
    ),
    /binding-ului CanvasAgent activ/,
  );
});

test("DOM inspection is accepted only for an exact render instance and strips semantics", () => {
  const contentWindow = {};
  const frame = { contentWindow };
  const data = {
    source: CANVAS_AGENT_MESSAGE_SOURCE,
    schemaVersion: CANVAS_INTERACTION_SCHEMA_VERSION,
    type: "domInspection",
    agentInstanceId: "agent-4",
    documentEpoch: 4,
    inspectionRequestId: "inspection:4:9",
    renderInstanceId: "render-card-title",
    observation: physicalSelection(),
  };
  const message = parseCanvasAgentMessage(
    frame,
    { source: contentWindow, data },
    "agent-4",
  );
  assert.equal(message?.type, "domInspection");
  assert.equal(message?.inspectionRequestId, "inspection:4:9");
  assert.equal(message?.renderInstanceId, "render-card-title");
  assert.equal("sourceId" in message.observation, false);
  assert.equal("templateSourceId" in message.observation, false);
  assert.equal("sourceLocation" in message.observation, false);
  assert.equal(message?.observation.attributes["data-pana-source-id"], undefined);

  assert.equal(parseCanvasAgentMessage(
    frame,
    {
      source: contentWindow,
      data: {
        ...data,
        observation: physicalSelection({ renderInstanceId: "render-other" }),
      },
    },
    "agent-4",
  ), null);
});

test("agent action is bounded and tied to the selected Rust node", () => {
  const contentWindow = {};
  const frame = { contentWindow };
  const action = {
    source: CANVAS_AGENT_MESSAGE_SOURCE,
    schemaVersion: CANVAS_INTERACTION_SCHEMA_VERSION,
    type: "action",
    agentInstanceId: "agent-4",
    documentEpoch: 4,
    actionSequence: 10,
    selectionRevision: 9,
    editorNodeId: "editor_boundary:hero",
    action: "enterBoundary",
  };
  assert.equal(parseCanvasAgentMessage(
    frame,
    { source: contentWindow, data: action },
    "agent-4",
  )?.type, "action");
  assert.equal(parseCanvasAgentMessage(
    frame,
    { source: contentWindow, data: { ...action, action: "unlockHtml" } },
    "agent-4",
  ), null);
});

test("Rust-first authority is mandatory and the legacy ingress is absent", () => {
  const io = readFileSync(
    new URL("../src/lib/project/io.ts", import.meta.url),
    "utf8",
  );
  const handler = readFileSync(
    new URL("../src/lib/state/app-preview-runtime-controller.ts", import.meta.url),
    "utf8",
  );
  const agent = readFileSync(
    new URL("../src-tauri/src/preview/bridge/03_canvas_agent.js", import.meta.url),
    "utf8",
  );
  const controller = readFileSync(
    new URL("../src/lib/state/canvas-interaction-controller.ts", import.meta.url),
    "utf8",
  );
  assert.match(io, /bind_canvas_interaction_agent/);
  assert.match(io, /resolve_canvas_interaction_intent/);
  assert.match(io, /resolve_canvas_drag_over_intent/);
  assert.match(io, /resolve_canvas_hover_intent/);
  assert.match(agent, /canvasAgentSelectionEnabled = false/);
  assert.match(agent, /postCanvasAgent\("agentActivated"/);
  assert.match(agent, /data-pana-render-instance-id/);
  assert.match(agent, /inspectCanvasAgentTarget/);
  assert.match(agent, /canEnterBoundary/);
  assert.match(agent, /CANVAS_AGENT_DRAG_ID/);
  assert.match(agent, /hitKey === canvasAgentLastPointerHitKey/);
  assert.match(agent, /document\.addEventListener\("pointerover", handleCanvasAgentPointerOver, true\)/);
  assert.match(
    agent,
    /document\.addEventListener\("pointermove", handleCanvasAgentHoverPointerMove,[\s\S]*passive: true/,
  );
  assert.match(
    agent,
    /canvasAgentDragCandidate = \{[\s\S]*document\.addEventListener\("pointermove", handleCanvasAgentPointerMove, true\)/,
  );
  assert.match(
    agent,
    /CANVAS_AGENT_HOVER_DWELL_MS = 120[\s\S]*clearTimeout\(canvasAgentHoverTimer\)[\s\S]*emitCanvasAgentGesture\(pending, "pointerMove"/,
  );
  assert.match(agent, /data-pana-drag-position/);
  assert.match(agent, /data-pana-drag-permission/);
  assert.match(agent, /permissionState === "allowed"/);
  assert.match(agent, /permissionState === "blocked"/);
  assert.match(agent, /canvasAgentDropAxis/);
  assert.match(agent, /projectCanvasAgentDragPreview/);
  assert.match(agent, /restoreCanvasAgentDragPreview/);
  assert.doesNotMatch(agent, /SOURCE_ID_ATTR|TEMPLATE_SOURCE_ID_ATTR|sourceId:/);
  assert.match(controller, /bindCanvasInteractionAgent/);
  assert.match(controller, /runtime\.phase = "activating"/);
  assert.match(controller, /message\.type === "agentActivated"/);
  assert.match(controller, /runtime\.phase = "active"/);
  assert.match(controller, /nextDocumentEpoch/);
  assert.match(controller, /resolveCanvasInteractionIntent/);
  assert.match(controller, /resolveCanvasDragOverIntent/);
  assert.match(controller, /resolveCanvasHoverIntent/);
  assert.match(controller, /requestEditorEditScope/);
  assert.match(controller, /app\.moveEditorNavigationNode/);
  assert.match(controller, /app\.previewEditorNavigationMove/);
  assert.match(controller, /canvasDragPermission/);
  assert.match(controller, /dragMovePreview/);
  assert.match(controller, /receipt\.dragPosition/);
  assert.match(controller, /latestDragOverSequence/);
  assert.doesNotMatch(controller, /VITE_PANA_RUST_FIRST_CANVAS_INTERACTION|phase = "fallback"/);
  assert.doesNotMatch(controller, /templateGateForSelection|templateHtmlEditSourceId/);
  assert.match(handler, /handleCanvasAgentMessage/);
  assert.doesNotMatch(
    handler,
    /data\.type !== "selection"|preview-hover-clear|preview-context-menu|preview-pointerdown|preview-layer-drop|preview-tera-move-drop/,
  );
});

test("Canvas DragOver plans latest-wins and mutates DOM only after Drop", () => {
  const controller = readFileSync(
    new URL("../src/lib/state/canvas-interaction-controller.ts", import.meta.url),
    "utf8",
  );
  const command = readFileSync(
    new URL("../src-tauri/src/commands/editor_navigation.rs", import.meta.url),
    "utf8",
  );
  const runtime = readFileSync(
    new URL("../src-tauri/src/kernel/canvas_interaction.rs", import.meta.url),
    "utf8",
  );
  const agent = readFileSync(
    new URL("../src-tauri/src/preview/bridge/03_canvas_agent.js", import.meta.url),
    "utf8",
  );
  assert.match(
    controller,
    /message\.gesture === "dragOver"[\s\S]*pendingDragOver = message[\s\S]*drainLatestCanvasDragOver\(app, runtime\)[\s\S]*return true/,
  );
  assert.match(
    controller,
    /runtime\.pendingDragOver[\s\S]*message\.gestureSequence !== runtime\.latestDragOverSequence[\s\S]*resolveCanvasDragOverIntent[\s\S]*currentBinding\(app, runtime\) !== binding[\s\S]*message\.gestureSequence !== runtime\.latestDragOverSequence[\s\S]*projectResolvedCanvasDragOver/,
  );
  assert.match(
    command,
    /fn resolve_canvas_drag_over_intent[\s\S]*resolve_drag_over[\s\S]*build_editor_move_plan[\s\S]*issue_editor_move_decision/,
  );
  assert.match(
    runtime,
    /fn resolve_drag_over[\s\S]*CanvasInteractionGesture::DragOver[\s\S]*last_accepted_ordered_sequence = request\.gesture_sequence[\s\S]*let projection = project/,
  );
  assert.match(controller, /type: "project-canvas-drag-preview"/);
  assert.match(controller, /const liveProjection = plan\.liveProjection/);
  assert.match(controller, /projection: liveProjection/);
  const dragOverProjectionBlock = controller.slice(
    controller.indexOf("function projectResolvedCanvasDragOver"),
    controller.indexOf("function projectCanvasDropDomPreview"),
  );
  assert.doesNotMatch(dragOverProjectionBlock, /project-canvas-drag-preview/);
  assert.match(
    controller,
    /projectCanvasDropDomPreview\([\s\S]*message\.gestureSequence,[\s\S]*message\.emittedAtMs,[\s\S]*plan,[\s\S]*app\.moveEditorNavigationNode/,
  );
  assert.match(
    agent,
    /dropGestureSequence = emitCanvasAgentGesture[\s\S]*canvasAgentPendingDrop = \{[\s\S]*gestureSequence: dropGestureSequence/,
  );
  assert.match(
    agent,
    /function projectCanvasAgentDragPreview[\s\S]*pendingDropMatches[\s\S]*!pendingDropMatches/,
  );
  assert.doesNotMatch(controller, /plan\.operation === "htmlSourceMove"/);
  assert.match(
    controller,
    /dragOverTail = runtime\.dragOverTail[\s\S]*await dragOverTail[\s\S]*runtime\.dragOverTail !== dragOverTail/,
  );
  assert.match(
    controller,
    /const settledPreview = movePreview[\s\S]*targetNodeId = settledPreview\?\.targetNodeId[\s\S]*position = settledPreview\?\.position \?\? drag\.position/,
  );
  assert.match(
    controller,
    /app\.moveEditorNavigationNode\([\s\S]*targetNodeId,[\s\S]*position,[\s\S]*plan/,
  );
  assert.match(controller, /type: "cancel-canvas-drag-preview"/);
});

test("Canvas pointer hover has a dedicated latest-wins Rust lane", () => {
  const controller = readFileSync(
    new URL("../src/lib/state/canvas-interaction-controller.ts", import.meta.url),
    "utf8",
  );
  const command = readFileSync(
    new URL("../src-tauri/src/commands/editor_navigation.rs", import.meta.url),
    "utf8",
  );
  const runtime = readFileSync(
    new URL("../src-tauri/src/kernel/canvas_interaction.rs", import.meta.url),
    "utf8",
  );

  assert.match(
    controller,
    /message\.gesture === "pointerMove"[\s\S]*pendingPointerMove = message[\s\S]*drainLatestPointerHover\(app, runtime\)[\s\S]*return true/,
  );
  assert.match(
    controller,
    /async \(\) => \{[\s\S]*runtime\.pendingPointerMove[\s\S]*message\.gestureSequence !== runtime\.latestPointerMoveSequence[\s\S]*resolveCanvasHoverIntent[\s\S]*currentBinding\(app, runtime\) !== binding[\s\S]*message\.gestureSequence !== runtime\.latestPointerMoveSequence[\s\S]*projectCanvasHoverReceipt/,
  );
  assert.match(
    command,
    /fn resolve_canvas_hover_intent[\s\S]*resolve_pointer_hover[\s\S]*selection_coordinator\.apply_hover/,
  );
  const hoverCommandBlock = command.slice(
    command.indexOf("pub fn resolve_canvas_hover_intent"),
    command.indexOf("pub fn apply_selection_intent"),
  );
  assert.match(hoverCommandBlock, /CanvasHoverProjection/);
  assert.match(hoverCommandBlock, /changed/);
  assert.doesNotMatch(hoverCommandBlock, /inspector_summary|SelectionCoordinatorSnapshot/);
  assert.match(
    runtime,
    /fn resolve_pointer_hover[\s\S]*PointerMove[\s\S]*last_accepted_hover_sequence = request\.gesture_sequence[\s\S]*let projection = project/,
  );
  assert.match(runtime, /last_accepted_ordered_sequence:\s*u64/);
  assert.match(runtime, /last_accepted_hover_sequence:\s*u64/);
  const orderedGestureBlock = controller.slice(
    controller.indexOf("runtime.gestureTail = runtime.gestureTail", controller.indexOf("message.gestureSequence")),
    controller.indexOf("function drainLatestPointerHover"),
  );
  assert.doesNotMatch(orderedGestureBlock, /resolveCanvasHoverIntent/);
});
