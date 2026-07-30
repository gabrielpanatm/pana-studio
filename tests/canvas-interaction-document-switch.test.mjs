import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  handleCanvasAgentMessage,
  synchronizeCanvasInteractionBinding,
} from "$lib/state/canvas-interaction-controller";
import {
  CANVAS_AGENT_MESSAGE_SOURCE,
} from "$lib/preview/canvas-interaction";
import { CANVAS_INTERACTION_SCHEMA_VERSION } from "$lib/types";

if (!globalThis.window) globalThis.window = globalThis;

afterEach(() => {
  clearMocks();
});

async function nextTurn() {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

function focusedSnapshot(identity, route, activeDocumentPath) {
  return {
    schemaVersion: 3,
    identity,
    modelRevision: "model-1",
    route,
    surface: "templateWorkbench",
    rootNodeIds: [],
    nodes: [],
    focusedView: {
      activeDocumentPath,
      activeTemplateName: activeDocumentPath,
      activeSourceNodeId: "template-active",
      breadcrumbs: [],
      rootNodeIds: [],
      nodes: [],
      previewContextRenderInstanceId: null,
    },
    diagnostics: [],
  };
}

test("Canvas Interaction waits for the navigation snapshot of the new Workbench document", async () => {
  const identity = {
    projectRoot: "/project-a",
    runtimeSessionId: "session-a:runtime-1",
    workspaceRevision: 13,
    transactionId: "canvas-13",
    previewRevision: "preview-13",
  };
  const route = "/__pana_workbench/template-page/";
  const contentWindow = {};
  const messages = [];
  const statuses = [];
  const bindRequests = [];
  const app = {
    activeCanvasIdentity: identity,
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
    editorNavigationSnapshot: focusedSnapshot(
      identity,
      route,
      "templates/index.html",
    ),
    coordinatedElementSelection: null,
    previewFrame: { contentWindow },
    postPreviewMessage(message) {
      messages.push(message);
    },
    setGlobalStatus(text, kind) {
      statuses.push({ text, kind });
    },
  };

  mockIPC((command, payload) => {
    assert.equal(command, "bind_canvas_interaction_agent");
    bindRequests.push(payload.input);
    return {
      schemaVersion: CANVAS_INTERACTION_SCHEMA_VERSION,
      identity: payload.input.identity,
      lastAcceptedSequence: 0,
      activeDocumentPath: payload.input.activeDocumentPath,
    };
  });

  handleCanvasAgentMessage(app, {
    source: contentWindow,
    data: {
      source: CANVAS_AGENT_MESSAGE_SOURCE,
      schemaVersion: CANVAS_INTERACTION_SCHEMA_VERSION,
      type: "agentReady",
      agentInstanceId: "agent-1",
    },
  });
  await nextTurn();

  assert.equal(bindRequests.length, 1);
  assert.equal(bindRequests[0].activeDocumentPath, "templates/index.html");
  assert.deepEqual(statuses, []);

  handleCanvasAgentMessage(app, {
    source: contentWindow,
    data: {
      source: CANVAS_AGENT_MESSAGE_SOURCE,
      schemaVersion: CANVAS_INTERACTION_SCHEMA_VERSION,
      type: "agentActivated",
      agentInstanceId: "agent-1",
      documentEpoch: bindRequests[0].identity.documentEpoch,
    },
  });

  app.activeScannedPath = "templates/page.html";
  synchronizeCanvasInteractionBinding(app);
  await nextTurn();

  assert.equal(bindRequests.length, 1);
  assert.deepEqual(statuses, []);
  assert.equal(messages.at(-1).type, "deactivate-canvas-interaction-agent");

  app.editorNavigationSnapshot = focusedSnapshot(
    identity,
    route,
    "templates/page.html",
  );
  synchronizeCanvasInteractionBinding(app);
  await nextTurn();

  assert.equal(bindRequests.length, 2);
  assert.equal(bindRequests[1].activeDocumentPath, "templates/page.html");
  assert.deepEqual(statuses, []);
  assert.equal(messages.at(-1).type, "activate-canvas-interaction-agent");

  handleCanvasAgentMessage(app, {
    source: contentWindow,
    data: {
      source: CANVAS_AGENT_MESSAGE_SOURCE,
      schemaVersion: CANVAS_INTERACTION_SCHEMA_VERSION,
      type: "agentActivated",
      agentInstanceId: "agent-1",
      documentEpoch: bindRequests[1].identity.documentEpoch,
    },
  });
});
