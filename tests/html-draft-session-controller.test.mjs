import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { flushRegisteredEditDrafts } from "$lib/session/edit-flush-registry";
import { HtmlDraftSessionController } from "$lib/state/html-draft-session.svelte";

if (!globalThis.window) globalThis.window = globalThis;

const controllers = [];

afterEach(() => {
  while (controllers.length > 0) controllers.pop()?.destroy();
  clearMocks();
});

function coordinatedSelection({
  attributes = { title: "original" },
  rawText = "Original",
} = {}) {
  return {
    snapshot: {
      runtimeSessionId: "session:runtime",
      selectionRevision: 4,
    },
    renderInstanceId: "render:heading",
    sourceNodeId: "source:heading",
    sourceLocation: { file: "templates/index.html", line: 3, column: 3 },
    observation: {
      tag: "h1",
      attributes,
      classes: [],
      hasChildElements: false,
      rawText,
      zolaImage: null,
    },
  };
}

function createController(overrides = {}) {
  const previewOperations = [];
  const previewMessages = [];
  const statuses = [];
  const host = {
    sessionProjectRoot: "/project",
    kernelProjectSessionId: "session:runtime",
    projectSessionEpoch: 2,
    projectWorkspaceSnapshot: { revision: 7 },
    coordinatedElementSelection: coordinatedSelection(),
    htmlPending: {
      tag: false,
      attributes: false,
      text: false,
      image: false,
      classes: false,
      structure: false,
    },
    context() {
      return {
        projectRoot: this.sessionProjectRoot,
        runtimeSessionId: this.kernelProjectSessionId,
        projectSessionEpoch: this.projectSessionEpoch,
        htmlPending: this.htmlPending,
        workspace: this.projectWorkspaceSnapshot,
        coordinatedSelection: this.coordinatedElementSelection,
      };
    },
    previewRuntime: {
      async sendAndWait(payload) {
        previewOperations.push(payload);
        return { ok: true };
      },
    },
    postPreviewMessage(payload) {
      previewMessages.push(payload);
    },
    setHtmlPending(area, pending) {
      this.htmlPending[area] = pending;
    },
    setGlobalStatus(text, kind) {
      statuses.push({ text, kind });
    },
    ...overrides,
  };
  const controller = new HtmlDraftSessionController(() => host);
  host.htmlDraft = controller;
  controllers.push(controller);
  return { controller, host, previewMessages, previewOperations, statuses };
}

test("attribute cancellation restores the captured Canvas baseline and clears the live layer", async () => {
  const { controller, host, previewOperations } = createController();

  controller.attributeValues = { title: "original" };
  controller.updateAttribute("title", "draft");
  await Promise.resolve();

  assert.deepEqual(controller.attributeValues, { title: "draft" });
  assert.equal(host.htmlPending.attributes, true);
  assert.equal(previewOperations[0]?.type, "apply-live-attribute-draft");

  controller.cancelAttributes();
  await Promise.resolve();

  assert.deepEqual(controller.attributeValues, { title: "original" });
  assert.equal(host.htmlPending.attributes, false);
  assert.equal(previewOperations.at(-1)?.type, "clear-live-attribute-draft");
});

test("a rejected speculative attribute projection reports the error without mutating authority", async () => {
  const { controller, host } = createController({
    previewRuntime: {
      async sendAndWait(payload) {
        if (payload.type === "apply-live-attribute-draft") {
          return { ok: false, error: "preview rejected" };
        }
        return { ok: true };
      },
    },
  });

  controller.attributeValues = { title: "original" };
  controller.updateAttribute("title", "draft");
  await Promise.resolve();
  await Promise.resolve();

  assert.match(controller.attributeStatus, /preview rejected/i);
  assert.equal(host.htmlPending.attributes, true);
});

test("cancel invalidates queued text recovery and the registered idle flush remains safe", async () => {
  let ipcCalls = 0;
  mockIPC(() => {
    ipcCalls += 1;
    throw new Error("the cancelled queue must not reach Rust");
  });
  const { controller, host, previewMessages } = createController();

  controller.updateText("Draft", false);
  assert.equal(host.htmlPending.text, true);
  assert.equal(previewMessages[0]?.type, "apply-live-text-draft");
  controller.cancel();

  await new Promise((resolve) => setTimeout(resolve, 240));
  await flushRegisteredEditDrafts("manual");

  assert.equal(ipcCalls, 0);
  assert.equal(previewMessages.at(-1)?.type, "clear-live-text-draft");
  assert.equal(controller.activeTextEditKey, null);
  assert.equal(controller.activeTextEditValue, null);
});
