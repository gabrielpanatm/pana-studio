import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";

import {
  applyImageSourceToHtml,
  applyNativeBlockOptionToHtml,
} from "$lib/editor/html-actions/media";
import {
  moveSelectedHtmlElements,
  mutateNativeBlockSlotStructure,
} from "$lib/editor/html-actions/structure";
import { insertPaletteElementAtTarget } from "$lib/editor/html-actions/insertion";

if (!globalThis.window) globalThis.window = globalThis;

afterEach(() => clearMocks());

function selectionIdentity(count = 1) {
  const members = Array.from({ length: count }, (_, index) => ({
    memberId: `member-${index}`,
    editorNodeId: `editor-${index}`,
    sourceNodeId: `source-${index}`,
    renderInstanceId: `render-${index}`,
  }));
  return {
    selectionRevision: 4,
    workspaceRevision: 8,
    primaryMemberId: members[0].memberId,
    members,
  };
}

function committedReceipt(tag = "div") {
  return {
    status: "committed",
    messageDiagnostic: { schemaVersion: 1, code: "committed" },
    patch: {
      file: "templates/index.html",
      contents: `<${tag}></${tag}>`,
      tag,
      targetLocation: { file: "templates/index.html", line: 1, column: 1 },
    },
    workspaceMutation: null,
    diagnostics: [],
  };
}

function actionHost({ coordinatedSelection = null, selectedCount = 1 } = {}) {
  const pending = {};
  const statuses = [];
  const lease = {
    projectRoot: "/project",
    sessionId: "session:runtime",
    projectSessionEpoch: 3,
    selection: selectionIdentity(selectedCount),
  };
  const host = {
    context: () => ({
      coordinatedSelection,
      canEditStructure: true,
      activeScannedPath: null,
      project: null,
    }),
    html: {
      structureStatus: "",
      imageStatus: "",
      imageSourceValue: "",
      classStatus: "",
      classEditorValue: "",
    },
    draft: {},
    source: { source: "", sourceCache: {} },
    editorSelection: {
      selectionSnapshot: { members: Array.from({ length: selectedCount }, () => ({})) },
    },
    structural: {
      run: (operation) => operation(lease),
      leaseMatches: () => true,
      async projectCommitted(_lease, _receipt, _patch, projectLocalState) {
        await projectLocalState();
        return {};
      },
      async projectCommittedBatch() { return {}; },
      async settleMutation() { return {}; },
    },
    commands: {
      setPending: (area, value) => { pending[area] = value; },
      setStatus: (text, kind) => statuses.push({ text, kind }),
      async loadProjectFile() {},
      async reconcilePageAssets() {},
    },
  };
  return { host, pending, statuses };
}

test("media trimite opțiunea blocului nativ prin boundary-ul canonic de attributes", async () => {
  let submitted = null;
  mockIPC(async (command, payload) => {
    assert.equal(command, "execute_preview_html_attributes_intent");
    submitted = payload.input.attributeIntent;
    return committedReceipt("section");
  });
  const { host } = actionHost();
  const result = await applyNativeBlockOptionToHtml(host, {
    providerId: "slider",
    optionId: "autoplay",
    value: { kind: "boolean", value: true },
    rootTag: "section",
    rootSourceId: "source-slider",
    rootLocation: { file: "templates/index.html", line: 2, column: 1 },
    rootSessionId: "session:runtime",
  });
  assert.equal(result.status, "committed");
  assert.deepEqual(submitted.nativeBlockOption, {
    providerId: "slider",
    optionId: "autoplay",
    value: { kind: "boolean", value: true },
  });
  assert.deepEqual(submitted.attributes, []);
});

test("media aplică sursa imaginii pe ținta capturată, fără invoke alternativ", async () => {
  let submitted = null;
  mockIPC(async (command, payload) => {
    assert.equal(command, "execute_preview_html_attributes_intent");
    submitted = payload.input.attributeIntent;
    return committedReceipt("img");
  });
  const coordinatedSelection = {
    snapshot: { selectionRevision: 4, runtimeSessionId: "session:runtime" },
    renderInstanceId: "render-image",
    sourceNodeId: "source-image",
    sourceLocation: { file: "templates/index.html", line: 3, column: 1 },
    observation: {
      tag: "img",
      attributes: { src: "/old.webp" },
      classes: [],
      hasChildElements: false,
      rawText: "",
      zolaImage: null,
    },
  };
  const { host } = actionHost({ coordinatedSelection });
  const result = await applyImageSourceToHtml(host, "/new.webp");
  assert.equal(result.status, "committed");
  assert.deepEqual(submitted.attributes, [
    { kind: "setAttribute", name: "src", value: "/new.webp" },
  ]);
});

test("slotul nativ inserează atomic prin boundary-ul structural existent", async () => {
  let submitted = null;
  mockIPC(async (command, payload) => {
    assert.equal(command, "execute_preview_html_insert_drop_intent");
    submitted = payload.input.insertIntent;
    return committedReceipt("div");
  });
  const { host } = actionHost();
  const result = await mutateNativeBlockSlotStructure(host, {
    operation: "insert",
    context: {
      providerId: "slider",
      slotId: "slides",
      rootSourceId: "source-slider",
      expectedModelRevision: "model:8",
    },
    slot: {
      id: "slides",
      itemKind: "slide",
      containerSourceNodeId: "source-slides",
      minimumItems: 1,
      maximumItems: null,
      editable: true,
      diagnostic: null,
      items: [],
    },
  });
  assert.equal(result.status, "committed");
  assert.equal(submitted.targetSourceId, "source-slides");
  assert.equal(submitted.nativeBlockSlot.providerId, "slider");
});

test("insertion/palette confirmă rezultatul numai după callback-ul de settlement", async () => {
  mockIPC(async (command) => {
    assert.equal(command, "execute_preview_html_insert_drop_intent");
    return committedReceipt("p");
  });
  const { host } = actionHost();
  const result = await insertPaletteElementAtTarget(host, {
    targetRenderInstanceId: "render-main",
    targetSessionId: "session:runtime",
    targetSourceId: "source-main",
    targetTemplateSourceId: null,
    targetBoundaryInstanceId: null,
    targetTag: "main",
    targetKind: "html",
    position: "inside",
    element: {
      id: "paragraph",
      kind: "html",
      tag: "p",
      label: "Paragraph",
      description: "",
      text: "Text",
      className: "",
      html: "",
    },
  });
  assert.equal(result.status, "committed");
  assert.match(host.html.structureStatus, /saved|salvat/i);
});

test("multi-move păstrează selection batch ca unică autoritate", async () => {
  let submitted = null;
  mockIPC(async (command, payload) => {
    assert.equal(command, "execute_preview_selection_batch_intent");
    submitted = payload.input.action;
    return { status: "blocked", diagnostics: ["batch refused"] };
  });
  const { host } = actionHost({ selectedCount: 2 });
  const result = await moveSelectedHtmlElements(host, "source-target", "section", "before");
  assert.equal(result.status, "blocked");
  assert.deepEqual(submitted, {
    kind: "move",
    targetSourceId: "source-target",
    targetTag: "section",
    position: "before",
  });
});
