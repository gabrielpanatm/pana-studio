import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { createMotionStepTimingQueue } from "$lib/js/motion-step-timing-queue";
import { normalizePageJsTemplatePath } from "$lib/js/page-path";
import { createLatestWinsAsyncQueue } from "$lib/session/latest-wins-async-queue";
import { createPageJsDraftSyncQueue } from "$lib/session/page-js-draft-sync";
import { flushRegisteredEditDrafts, registerEditFlushHandler } from "$lib/session/edit-flush-registry";
import {
  createInspectorPendingSourceRegistry,
  updateInspectorPendingSource,
} from "$lib/state/inspector-pending";
import {
  createPreviewRuntime,
  PreviewRuntimeTransportError,
} from "$lib/editor-runtime/preview-runtime";

function deferred() {
  let resolve;
  const promise = new Promise((resolvePromise) => { resolve = resolvePromise; });
  return { promise, resolve };
}

function identity(session = "session:runtime", generation = 1) {
  return { expectedProjectRoot: "/project", expectedSessionId: session, generation };
}

function stageTask(revision, templatePath = "templates/index.html", taskIdentity = identity()) {
  return {
    kind: "stage",
    templatePath,
    identity: taskIdentity,
    input: {
      templatePath,
      baseConfig: { version: 1, revision: 0 },
      currentConfig: { version: 1, revision },
      cachebustAssets: false,
      source: "test.page_js",
      coalesceKey: "test.page_js",
    },
  };
}

function stageReceipt(taskIdentity, revision = 1) {
  return {
    schemaVersion: 2,
    status: "staged",
    changed: true,
    dirty: true,
    templatePath: "templates/index.html",
    revision,
    entryRevision: revision,
    dirtyCount: 1,
    retainedConfigBytes: 100,
    projectRoot: taskIdentity.expectedProjectRoot,
    runtimeSessionId: taskIdentity.expectedSessionId,
  };
}

test("workspace shell CSS remains a plain Vite module", () => {
  const page = readFileSync(new URL("../src/routes/+page.svelte", import.meta.url), "utf8");
  const css = readFileSync(new URL("../src/routes/workspace-shell.css", import.meta.url), "utf8");
  assert.match(page, /import "\.\/workspace-shell\.css";/);
  assert.doesNotMatch(page, /<style(?:\s|>)/);
  assert.match(css, /\.center-stack\s*\{[\s\S]*display:\s*grid/);
});

test("JS pending aggregates independent Inspector and Timeline owners", () => {
  const registry = createInspectorPendingSourceRegistry();
  assert.equal(updateInspectorPendingSource(registry, "js", "inspector", false), false);
  assert.equal(updateInspectorPendingSource(registry, "js", "timeline", true), true);
  assert.equal(updateInspectorPendingSource(registry, "js", "inspector", true), true);
  assert.equal(updateInspectorPendingSource(registry, "js", "timeline", false), true);
  assert.equal(updateInspectorPendingSource(registry, "js", "inspector", false), false);
});

test("Save flush reason reaches every registered editor owner", async () => {
  const reasons = [];
  const unregisterA = registerEditFlushHandler("test-js-inspector", async (reason) => reasons.push(`a:${reason}`));
  const unregisterB = registerEditFlushHandler("test-js-timeline", async (reason) => reasons.push(`b:${reason}`));
  try {
    await flushRegisteredEditDrafts("save");
  } finally {
    unregisterA();
    unregisterB();
  }
  assert.deepEqual(reasons.sort(), ["a:save", "b:save"]);
});

test("latest-wins queue is bounded to the last pending task per key", async () => {
  const executed = [];
  const queue = createLatestWinsAsyncQueue({
    key: (task) => task.key,
    delayMs: 1_000,
    async run(task) { executed.push(task.value); },
  });
  for (let value = 0; value < 100; value += 1) queue.enqueue({ key: "page", value });
  await queue.flush();
  assert.deepEqual(executed, [99]);
  assert.equal(queue.snapshot().coalescedCount, 99);
  assert.equal(queue.snapshot().pendingCount, 0);
});

test("latest-wins throttle confirms the newest draft without extending the first deadline", async () => {
  const executed = [];
  const queue = createLatestWinsAsyncQueue({
    key: (task) => task.key,
    delayMs: 30,
    delayMode: "throttle",
    async run(task) { executed.push(task.value); },
  });
  queue.enqueue({ key: "text", value: "a" });
  await new Promise((resolve) => setTimeout(resolve, 20));
  queue.enqueue({ key: "text", value: "ab" });
  await new Promise((resolve) => setTimeout(resolve, 20));
  await queue.flush();
  assert.deepEqual(executed, ["ab"]);
});

test("latest-wins reset invalidates an in-flight completion", async () => {
  const gate = deferred();
  const started = deferred();
  let wasCurrentAfterGate = true;
  const queue = createLatestWinsAsyncQueue({
    key: () => "page",
    delayMs: 0,
    async run(_task, context) {
      started.resolve();
      await gate.promise;
      wasCurrentAfterGate = context.isCurrent();
    },
  });
  queue.enqueue({ value: 1 });
  await started.promise;
  queue.reset();
  gate.resolve();
  await queue.flush();
  assert.equal(wasCurrentAfterGate, false);
  assert.deepEqual(queue.snapshot(), {
    pendingCount: 0,
    inFlight: false,
    failureCount: 0,
    enqueuedCount: 0,
    executedCount: 0,
    coalescedCount: 0,
  });
});

test("Page JS queue canonicalizes aliases and stages only the latest config", async () => {
  const calls = [];
  const taskIdentity = identity();
  const queue = createPageJsDraftSyncQueue({
    async stage(input, transportIdentity) {
      calls.push({ input, transportIdentity });
      return stageReceipt(taskIdentity);
    },
    async clear() { throw new Error("clear neașteptat"); },
  }, 1_000);
  queue.enqueue(stageTask(1, "./templates\\index.html", taskIdentity));
  queue.enqueue(stageTask(2, "templates/index.html", taskIdentity));
  await queue.flush();

  assert.equal(normalizePageJsTemplatePath("./templates\\index.html"), "templates/index.html");
  assert.equal(calls.length, 1);
  assert.equal(calls[0].input.templatePath, "templates/index.html");
  assert.equal(calls[0].input.currentConfig.revision, 2);
  assert.deepEqual(calls[0].transportIdentity, {
    expectedProjectRoot: "/project",
    expectedSessionId: "session:runtime",
  });
});

test("Page JS queue rejects a receipt from a replacement runtime", async () => {
  const taskIdentity = identity("session:a");
  const queue = createPageJsDraftSyncQueue({
    async stage() { return stageReceipt(identity("session:b")); },
    async clear() { return stageReceipt(identity("session:b")); },
  }, 0);
  queue.enqueue(stageTask(1, "templates/index.html", taskIdentity));
  await assert.rejects(
    queue.flush({ retryFailures: false }),
    /receipt-ul altei sesiuni/,
  );
});

test("same-root Page JS replacement invalidates an in-flight continuation", async () => {
  const gate = deferred();
  const started = deferred();
  let active = identity("session:a", 1);
  let transportCalls = 0;
  const queue = createPageJsDraftSyncQueue({
    async stage() {
      transportCalls += 1;
      started.resolve();
      await gate.promise;
      return stageReceipt(identity("session:a", 1));
    },
    async clear() { throw new Error("clear neașteptat"); },
  }, 0, (candidate) => (
    candidate.expectedSessionId === active.expectedSessionId
    && candidate.generation === active.generation
  ));
  queue.enqueue(stageTask(1, "templates/index.html", active));
  await started.promise;
  active = identity("session:b", 2);
  gate.resolve();
  await queue.flush();
  assert.equal(transportCalls, 1);
  assert.equal(queue.snapshot().failureCount, 0);
});

test("motion timing queue composes partial patches for one step", async () => {
  const applied = [];
  const queue = createMotionStepTimingQueue(async (task) => applied.push(task), 1_000);
  const base = {
    projectRoot: "/project",
    runtimeSessionId: "session:runtime",
    templatePath: "templates/index.html",
    timelineId: "timeline-1",
    stepId: "step-1",
    stepIndex: 0,
  };
  queue.enqueue({ ...base, patch: { position: "100" } });
  queue.enqueue({ ...base, patch: { duration: 800 } });
  await queue.flush();
  assert.equal(applied.length, 1);
  assert.deepEqual(applied[0].patch, { position: "100", duration: 800 });
});

test("motion timing never coalesces two runtime sessions", async () => {
  const applied = [];
  const queue = createMotionStepTimingQueue(async (task) => applied.push(task), 1_000);
  const base = {
    projectRoot: "/project",
    templatePath: "templates/index.html",
    timelineId: "timeline-1",
    stepId: "step-1",
    stepIndex: 0,
  };
  queue.enqueue({ ...base, runtimeSessionId: "session:a", patch: { position: "100" } });
  queue.enqueue({ ...base, runtimeSessionId: "session:b", patch: { position: "200" } });
  await queue.flush();
  assert.deepEqual(applied.map((task) => task.runtimeSessionId), ["session:a", "session:b"]);
});

test("preview runtime accepts ACK only for the exact revision and operation", () => {
  const sent = [];
  const runtime = createPreviewRuntime({
    postPreviewMessage(payload) { sent.push(payload); },
    setGlobalStatus() {},
  }, { ackTimeoutMs: 60_000 });
  const operation = runtime.send({ type: "set-text", text: "Titlu" });
  assert.equal(runtime.handleAck({
    type: "preview-operation-complete",
    previewRevision: operation.revision,
    operation: "other",
    ok: true,
  }), null);
  const ack = runtime.handleAck({
    type: "preview-operation-complete",
    previewRevision: operation.revision,
    operation: "set-text",
    ok: true,
  });
  assert.equal(ack.revision, operation.revision);
  assert.equal(runtime.hasPending(), false);
  assert.equal(sent[0].previewRevision, operation.revision);
  runtime.reset();
});

test("preview runtime resolves canonical document replacement only after exact ACK", async () => {
  const sent = [];
  const runtime = createPreviewRuntime({
    postPreviewMessage(payload) { sent.push(payload); },
    setGlobalStatus() {},
  }, { ackTimeoutMs: 60_000 });
  const confirmation = runtime.sendAndWait({
    type: "replace-document",
    html: "<html><body>Nou</body></html>",
  });
  const operation = sent.at(-1);
  runtime.handleAck({
    type: "preview-operation-complete",
    previewRevision: operation.previewRevision,
    operation: "replace-document",
    ok: true,
  });
  const ack = await confirmation;
  assert.equal(ack.revision, operation.previewRevision);
  assert.equal(ack.ok, true);
  assert.equal(runtime.hasPending(), false);
  runtime.reset();
});

test("un timeout await-ed este clasificat pentru fallback fără toast prematur", async () => {
  const statuses = [];
  let triggerTimeout = null;
  const runtime = createPreviewRuntime({
    postPreviewMessage() {},
    setGlobalStatus(text, kind) { statuses.push({ text, kind }); },
  }, {
    ackTimeoutMs: 1,
    scheduleTimeout(callback) {
      triggerTimeout = callback;
      return 1;
    },
    cancelTimeout() {},
  });

  const confirmation = runtime.sendAndWait({ type: "replace-document", html: "<html></html>" });
  triggerTimeout();
  await assert.rejects(
    confirmation,
    (error) => error instanceof PreviewRuntimeTransportError && error.code === "ack_timeout",
  );
  assert.deepEqual(statuses, []);
  runtime.reset();
});

test("preview runtime requires exact receipts for CanvasPatch apply and rollback", async () => {
  const sent = [];
  const runtime = createPreviewRuntime({
    postPreviewMessage(payload) { sent.push(payload); },
    setGlobalStatus() {},
  }, { ackTimeoutMs: 60_000 });
  const patch = {
    schemaVersion: 1,
    patchId: `canvas_patch_${"a".repeat(64)}`,
    issuedAtMs: Date.now(),
    projectRoot: "/project",
    runtimeSessionId: "session:runtime",
    baseWorkspaceRevision: 4,
    workspaceRevision: 5,
    workspaceTransactionId: "workspace-5",
    beforeModelRevision: "model-4",
    afterModelRevision: "model-5",
    operation: { kind: "setText", target: { sourceId: "source-h1" }, text: "Nou" },
  };

  const applied = runtime.applyCanvasPatch(patch);
  const applyOperation = sent.at(-1);
  runtime.handleAck({
    type: "preview-operation-complete",
    previewRevision: applyOperation.previewRevision,
    operation: "apply-canvas-patch",
    ok: true,
    canvasPatchReceipt: {
      schemaVersion: 1,
      patchId: patch.patchId,
      workspaceRevision: 5,
      workspaceTransactionId: "workspace-5",
      bridgeCommitDurationMs: 1,
    },
  });
  assert.equal((await applied).workspaceRevision, 5);
  assert.equal(runtime.canvasPatchPerformance().sampleCount, 1);
  assert.equal(runtime.canvasPatchPerformance().bridgeCommitP95Ms, 1);

  const rolledBack = runtime.rollbackCanvasPatch(patch);
  const rollbackOperation = sent.at(-1);
  runtime.handleAck({
    type: "preview-operation-complete",
    previewRevision: rollbackOperation.previewRevision,
    operation: "rollback-canvas-patch",
    ok: true,
    canvasPatchRollbackReceipt: {
      schemaVersion: 1,
      patchId: patch.patchId,
      workspaceRevision: 4,
      workspaceTransactionId: "workspace-5",
    },
  });
  assert.equal((await rolledBack).workspaceRevision, 4);
  assert.equal(runtime.hasPending(), false);
  runtime.reset();
});

test("a rejected speculative CanvasPatch is left to the canonical fallback without a false error toast", async () => {
  const sent = [];
  const statuses = [];
  const runtime = createPreviewRuntime({
    postPreviewMessage(payload) { sent.push(payload); },
    setGlobalStatus(text, kind) { statuses.push({ text, kind }); },
  }, { ackTimeoutMs: 60_000 });
  const patch = {
    schemaVersion: 1,
    patchId: `canvas_patch_${"b".repeat(64)}`,
    issuedAtMs: Date.now(),
    projectRoot: "/project",
    runtimeSessionId: "session:runtime",
    baseWorkspaceRevision: 4,
    workspaceRevision: 5,
    workspaceTransactionId: "workspace-5",
    beforeModelRevision: "model-4",
    afterModelRevision: "model-5",
    operation: { kind: "setAttributes", target: { sourceId: "source-a" }, attributes: { href: "/despre" } },
  };

  const applying = runtime.applyCanvasPatch(patch);
  const operation = sent.at(-1);
  runtime.handleAck({
    type: "preview-operation-complete",
    previewRevision: operation.previewRevision,
    operation: "apply-canvas-patch",
    ok: false,
    error: "accelerator miss",
  });
  await assert.rejects(applying, /accelerator miss/);
  assert.deepEqual(statuses, []);
  runtime.reset();
});

test("preview ingress is bounded and reports overflow once per window", () => {
  let now = 10;
  const statuses = [];
  const runtime = createPreviewRuntime({
    postPreviewMessage() {},
    setGlobalStatus(text, kind) { statuses.push({ text, kind }); },
  }, {
    now: () => now,
    maxIncomingMessagesPerWindow: 2,
    incomingWindowMs: 100,
  });
  assert.equal(runtime.acceptIncomingMessage(), true);
  assert.equal(runtime.acceptIncomingMessage(), true);
  assert.equal(runtime.acceptIncomingMessage(), false);
  assert.equal(runtime.acceptIncomingMessage(), false);
  assert.equal(statuses.length, 1);
  now = 111;
  assert.equal(runtime.acceptIncomingMessage(), true);
});

test("Design Safe has no frontend execution path for raw project JS", () => {
  const jsPane = readFileSync(new URL("../src/lib/components/inspector/JsPane.svelte", import.meta.url), "utf8");
  const timeline = readFileSync(new URL("../src/lib/components/workspace/MotionTimelinePanel.svelte", import.meta.url), "utf8");
  const bridge = readFileSync(new URL("../src-tauri/src/preview/bridge/12_messages_events.js", import.meta.url), "utf8");
  for (const source of [jsPane, timeline, bridge]) {
    assert.doesNotMatch(source, /\beval\s*\(|new Function\s*\(/);
  }
  assert.match(jsPane, /stage|draft|ProjectWorkspace/i);
});

test("HTML persistence stays in ProjectWorkspace while live drafts are explicit ephemeral bridges", () => {
  const controller = readFileSync(
    new URL("../src/lib/state/app-preview-runtime-controller.ts", import.meta.url),
    "utf8",
  );
  const draft = readFileSync(
    new URL("../src/lib/state/html-draft-controller.ts", import.meta.url),
    "utf8",
  );
  const messages = readFileSync(
    new URL("../src-tauri/src/preview/bridge/12_messages_events.js", import.meta.url),
    "utf8",
  );
  const canvasPatch = readFileSync(
    new URL("../src-tauri/src/preview/bridge/10_canvas_patch.js", import.meta.url),
    "utf8",
  );
  const app = readFileSync(new URL("../src/lib/state/app.svelte.ts", import.meta.url), "utf8");
  const selection = readFileSync(
    new URL("../src/lib/state/selection-controller.ts", import.meta.url),
    "utf8",
  );
  const previewSelection = readFileSync(
    new URL("../src/lib/preview/selection.ts", import.meta.url),
    "utf8",
  );
  const forbiddenDirectMutation = /apply-(?:styles|variables|attributes|text-content|tag-name|classes|image-src)/;

  assert.match(controller, /flushFileBufferDraftSync/);
  assert.match(controller, /projectLatestProjectWorkspacePreview/);
  assert.doesNotMatch(controller, /type:\s*["']replace-document["']/);
  assert.doesNotMatch(draft, /querySelector|\.textContent\s*=|\.setAttribute\s*\(|postMessage/);
  assert.doesNotMatch(messages, forbiddenDirectMutation);
  assert.match(messages, /apply-canvas-patch/);
  assert.match(messages, /apply-live-text-draft/);
  assert.match(messages, /apply-live-attribute-draft/);
  assert.match(canvasPatch, /activeLiveTextDraft/);
  assert.match(canvasPatch, /reapplyLiveTextDraft/);
  assert.match(canvasPatch, /activeLiveAttributeDraft/);
  assert.match(canvasPatch, /reapplyLiveAttributeDraft/);
  assert.match(app, /deferCanonicalProjection:\s*true/);
  assert.match(app, /HTML_TEXT_HISTORY_IDLE_MS/);
  assert.match(app, /finishActiveHtmlAttributeEditSession/);
  assert.doesNotMatch(app, /htmlAttributeDraftCommitQueue/);
  assert.match(previewSelection, /attr\.name\.startsWith\("data-pana-"\)/);
  assert.match(selection, /activeHtmlTextEditKey === htmlTextSelectionKey\(selection\)/);
});

test("Canvas document commit waits for styledReady and never replaces head/body via innerHTML", () => {
  const bridgeRoot = new URL("../src-tauri/src/preview/bridge/", import.meta.url);
  const bootstrap = readFileSync(new URL("00_bootstrap.js", bridgeRoot), "utf8");
  const structure = readFileSync(new URL("01_dom_structure.js", bridgeRoot), "utf8");
  const canvasPatch = readFileSync(new URL("10_canvas_patch.js", bridgeRoot), "utf8");
  const sync = readFileSync(new URL("11_document_sync.js", bridgeRoot), "utf8");
  const messages = readFileSync(new URL("12_messages_events.js", bridgeRoot), "utf8");
  const combined = [bootstrap, structure, canvasPatch, sync, messages].join("\n");

  assert.doesNotMatch(combined, /document\.(?:head|body)\.innerHTML\s*=/);
  assert.doesNotMatch(combined, /html[-_ ]?replay|replayBaseline|REPLAY_INSERT_ATTR/i);
  assert.match(combined, /applyCanvasPatch/);
  assert.match(messages, /return replaceDocument\(/);
  assert.match(sync, /Promise\.all\(waits\)[\s\S]*entry\.fresh[\s\S]*entry\.link\.remove\(\)/);
  assert.match(sync, /requestAnimationFrame\(function \(\) \{[\s\S]*requestAnimationFrame\(resolve\)/);
  assert.match(sync, /canvasPhaseReceipts:\s*phaseReceipts/);
  assert.match(sync, /"resourcesReady"[\s\S]*"committed"[\s\S]*"styledReady"/);
  assert.match(messages, /phase:\s*["']failed["']/);
});

test("project startup defers Template Workbench until canonical Preview publication", () => {
  const controller = readFileSync(
    new URL("../src/lib/state/project-controller.ts", import.meta.url),
    "utf8",
  );
  const initialSelection = controller.indexOf("activateTemplateWorkbench: false");
  const previewStart = controller.indexOf("export async function startPreviewAfterOpen");
  const canonicalPublication = controller.indexOf(
    "markProjectWorkspacePreviewPublished(",
    previewStart,
  );
  const workbenchActivation = controller.indexOf(
    "activateTemplateWorkbench: true",
    canonicalPublication,
  );

  assert.ok(initialSelection > 0);
  assert.ok(previewStart > initialSelection);
  assert.ok(canonicalPublication > previewStart);
  assert.ok(workbenchActivation > canonicalPublication);
});

test("reload-ul proiectului folosește unicul pipeline de atașare și nu pornește Canvas inline", () => {
  const controller = readFileSync(
    new URL("../src/lib/state/project-controller.ts", import.meta.url),
    "utf8",
  );
  const reloadStart = controller.indexOf("async function reloadCurrentProjectFromDisk");
  const reloadEnd = controller.indexOf("function resetProjectSessionState", reloadStart);
  const reloadBody = controller.slice(reloadStart, reloadEnd);

  assert.ok(reloadStart > 0 && reloadEnd > reloadStart);
  assert.match(reloadBody, /projectPublishedSessionIntoFrontend\(/);
  assert.match(reloadBody, /startPreviewAfterOpen\(/);
  assert.doesNotMatch(reloadBody, /startProjectPreview\(/);
  assert.doesNotMatch(reloadBody, /prepareCanvasProjectionNavigation\(/);
});

test("Page JS has one session staging path and no independent Save engine", () => {
  const sync = readFileSync(new URL("../src/lib/session/page-js-draft-sync.ts", import.meta.url), "utf8");
  const io = readFileSync(new URL("../src/lib/project/io.ts", import.meta.url), "utf8");
  assert.match(sync, /stagePageJsDraft/);
  assert.match(sync, /clearPageJsDraft/);
  assert.match(io, /"stage_page_js_draft"/);
  assert.doesNotMatch(sync, /save_page_js|acceptedManifest|InternalWriteEvidence/);
  assert.doesNotMatch(io, /save_page_js_batch|save_page_js_config/);
});
