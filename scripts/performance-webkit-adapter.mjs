#!/usr/bin/env node

import { fileURLToPath } from "node:url";

const wait = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds));

function parseArguments(argv) {
  const options = {
    endpoint: "http://127.0.0.1:9222",
    profile: "unknown",
    samples: 30,
    warmups: 5,
    frameSamples: 180,
    timeoutMs: 30_000,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--endpoint") options.endpoint = argv[++index];
    else if (argument === "--profile") options.profile = argv[++index];
    else if (argument === "--samples") options.samples = Number(argv[++index]);
    else if (argument === "--warmups") options.warmups = Number(argv[++index]);
    else if (argument === "--frame-samples") options.frameSamples = Number(argv[++index]);
    else if (argument === "--timeout-ms") options.timeoutMs = Number(argv[++index]);
    else throw new Error(`Unknown argument: ${argument}`);
  }
  for (const [name, value] of Object.entries({
    samples: options.samples,
    warmups: options.warmups,
    frameSamples: options.frameSamples,
    timeoutMs: options.timeoutMs,
  })) {
    if (!Number.isSafeInteger(value) || value < (name === "warmups" ? 0 : 1)) {
      throw new Error(`${name} must be a valid integer`);
    }
  }
  return options;
}

function websocketUrlFrom(value) {
  if (typeof value === "string" && value.startsWith("ws")) return value;
  if (Array.isArray(value)) {
    for (const item of value) {
      const match = websocketUrlFrom(item);
      if (match) return match;
    }
  } else if (value && typeof value === "object") {
    for (const item of Object.values(value)) {
      const match = websocketUrlFrom(item);
      if (match) return match;
    }
  }
  return null;
}

function sameRuntimeLocation(actual, expected) {
  try {
    return new URL(actual).pathname === new URL(expected).pathname;
  } catch {
    return actual === expected;
  }
}

async function inspectorSocket(endpoint, timeoutMs) {
  const deadline = performance.now() + timeoutMs;
  let lastError = null;
  while (performance.now() < deadline) {
    try {
      const response = await fetch(endpoint);
      if (!response.ok) throw new Error(`Inspector HTTP ${response.status}`);
      const body = await response.text();
      let socket = null;
      try {
        socket = websocketUrlFrom(JSON.parse(body));
      } catch {
        const path = body.match(/['"](\/socket\/\d+\/\d+\/WebPage)['"]/)?.[1]
          ?? body.match(/(\/socket\/\d+\/\d+\/WebPage)/)?.[1];
        if (path) {
          const inspector = new URL(endpoint);
          socket = `${inspector.protocol === "https:" ? "wss:" : "ws:"}//${inspector.host}${path}`;
        }
      }
      if (socket) return socket;
      lastError = new Error("Inspector target does not expose a WebSocket URL");
    } catch (error) {
      lastError = error;
    }
    await wait(50);
  }
  throw lastError ?? new Error("WebKit inspector unavailable");
}

class InspectorConnection {
  constructor(socket) {
    this.socket = socket;
    this.nextId = 1;
    this.nextTargetId = 1;
    this.pending = new Map();
    this.targetPending = new Map();
    this.targets = new Map();
    this.contexts = new Map();
    this.targetWaiters = [];
    socket.addEventListener("message", (event) => {
      const message = JSON.parse(String(event.data));
      if (message.method === "Target.targetCreated") {
        const info = message.params?.targetInfo;
        if (info?.targetId) {
          this.targets.set(info.targetId, info);
          this.targetWaiters.splice(0).forEach((resolve) => resolve());
        }
        return;
      }
      if (message.method === "Target.targetDestroyed") {
        this.targets.delete(message.params?.targetId);
        return;
      }
      if (message.method === "Target.dispatchMessageFromTarget") {
        const nested = JSON.parse(message.params?.message ?? "{}");
        if (nested.method === "Runtime.executionContextCreated") {
          const context = nested.params?.context;
          if (context?.id !== undefined) this.contexts.set(context.id, context);
          return;
        }
        if (nested.method === "Runtime.executionContextDestroyed") {
          this.contexts.delete(nested.params?.executionContextId);
          return;
        }
        if (nested.method === "Runtime.executionContextsCleared") {
          this.contexts.clear();
          return;
        }
        const pending = this.targetPending.get(nested.id);
        if (!pending) return;
        this.targetPending.delete(nested.id);
        clearTimeout(pending.timeout);
        if (nested.error) pending.reject(new Error(JSON.stringify(nested.error)));
        else pending.resolve(nested.result);
        return;
      }
      if (!message.id) return;
      const pending = this.pending.get(message.id);
      if (!pending) return;
      this.pending.delete(message.id);
      clearTimeout(pending.timeout);
      if (message.error) pending.reject(new Error(JSON.stringify(message.error)));
      else pending.resolve(message.result);
    });
  }

  send(method, params = {}, timeoutMs = 10_000) {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`WebKit command timed out: ${method}`));
      }, timeoutMs);
      this.pending.set(id, { resolve, reject, timeout });
      this.socket.send(JSON.stringify({ id, method, params }));
    });
  }

  async waitForPageTarget(timeoutMs) {
    const deadline = performance.now() + timeoutMs;
    while (performance.now() < deadline) {
      const page = [...this.targets.values()].find((target) => target.type === "page");
      if (page) return page.targetId;
      await Promise.race([
        new Promise((resolve) => this.targetWaiters.push(resolve)),
        wait(50),
      ]);
    }
    throw new Error("WebKit page target unavailable");
  }

  async sendTarget(targetId, method, params = {}, timeoutMs = 30_000) {
    const id = this.nextTargetId++;
    const response = new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.targetPending.delete(id);
        reject(new Error(`WebKit target command timed out: ${method}`));
      }, timeoutMs);
      this.targetPending.set(id, { resolve, reject, timeout });
    });
    try {
      await this.send("Target.sendMessageToTarget", {
        targetId,
        message: JSON.stringify({ id, method, params }),
      }, Math.min(timeoutMs, 10_000));
    } catch (error) {
      const pending = this.targetPending.get(id);
      if (pending) {
        clearTimeout(pending.timeout);
        this.targetPending.delete(id);
      }
      throw error;
    }
    return response;
  }

  async evaluate(
    expression,
    targetId = this.pageTargetId,
    timeoutMs = 60_000,
    contextId = undefined,
  ) {
    let result = await this.sendTarget(targetId, "Runtime.evaluate", {
      expression,
      returnByValue: false,
      doNotPauseOnExceptionsAndMuteConsole: true,
      ...(contextId === undefined ? {} : { contextId }),
    }, timeoutMs);
    let remote = result?.result ?? result;
    if (remote?.wasThrown || result?.wasThrown) {
      throw new Error(remote?.description ?? result?.description ?? "WebKit evaluation failed");
    }
    if (remote?.objectId && (remote.subtype === "promise" || remote.className === "Promise")) {
      result = await this.sendTarget(targetId, "Runtime.awaitPromise", {
        promiseObjectId: remote.objectId,
        returnByValue: true,
      }, timeoutMs);
      remote = result?.result ?? result;
    } else if (remote?.objectId && remote.value === undefined) {
      result = await this.sendTarget(targetId, "Runtime.callFunctionOn", {
        objectId: remote.objectId,
        functionDeclaration: "function() { return this; }",
        returnByValue: true,
        doNotPauseOnExceptionsAndMuteConsole: true,
      }, timeoutMs);
      remote = result?.result ?? result;
    }
    if (remote?.wasThrown || result?.wasThrown) {
      throw new Error(remote?.description ?? result?.description ?? "WebKit evaluation failed");
    }
    return remote?.value;
  }

  async findRuntimeTarget(url, timeoutMs = 10_000) {
    const deadline = performance.now() + timeoutMs;
    while (performance.now() < deadline) {
      for (const targetId of this.targets.keys()) {
        if (targetId === this.pageTargetId) continue;
        try {
          const location = await this.evaluate("location.href", targetId, 1_000);
          if (sameRuntimeLocation(location, url)) return targetId;
        } catch {
          // A provisional target may disappear while the route commits.
        }
      }
      await wait(50);
    }
    return null;
  }

  async findRuntimeContext(url, timeoutMs = 10_000) {
    const deadline = performance.now() + timeoutMs;
    while (performance.now() < deadline) {
      for (const contextId of this.contexts.keys()) {
        try {
          const location = await this.evaluate(
            "location.href",
            this.pageTargetId,
            1_000,
            contextId,
          );
          if (sameRuntimeLocation(location, url)) return contextId;
        } catch {
          // Navigation replaces execution contexts while the route commits.
        }
      }
      await wait(50);
    }
    return null;
  }
}

async function connect(url, timeoutMs) {
  const socket = new WebSocket(url);
  const connection = new InspectorConnection(socket);
  await Promise.race([
    new Promise((resolve, reject) => {
      socket.addEventListener("open", resolve, { once: true });
      socket.addEventListener("error", () => reject(new Error("WebSocket connection failed")), { once: true });
    }),
    wait(timeoutMs).then(() => { throw new Error("WebSocket connection timeout"); }),
  ]);
  connection.pageTargetId = await connection.waitForPageTarget(timeoutMs);
  await connection.sendTarget(connection.pageTargetId, "Runtime.enable", {}, timeoutMs);
  return connection;
}

export function probeExpression(options, phase, batchStart, batchEnd) {
  return `
(async () => {
  let stage = "bootstrap";
  const phase = ${JSON.stringify(phase)};
  const batchStart = ${batchStart};
  const batchEnd = ${batchEnd};
  const coldSetupTimeoutMs = ${Math.max(
    120_000,
    Number.isSafeInteger(options.timeoutMs) ? options.timeoutMs : 30_000,
  )};
  const frame = () => Promise.race([
    new Promise((resolve) => requestAnimationFrame(resolve)),
    new Promise((resolve) => setTimeout(() => resolve(performance.now()), 250)),
  ]);
  const twice = async () => { await frame(); await frame(); };
  const waitFor = async (predicate, timeoutMs = 30000) => {
    const deadline = performance.now() + timeoutMs;
    while (performance.now() < deadline) {
      const value = predicate();
      if (value) return value;
      await new Promise((resolve) => setTimeout(resolve, 5));
    }
    throw new Error("runtime probe timeout: " + stage);
  };
  const activitySamples = [];
  const activityLabels = ["Editor", "Șabloane", "Componente"];
  const activityButton = (label) => [...document.querySelectorAll("button")]
    .find((button) => button.getAttribute("aria-label") === label);
  for (let iteration = phase === "activity" ? batchStart : batchEnd;
    iteration < batchEnd; iteration += 1) {
    stage = "activity:" + iteration;
    const label = activityLabels[iteration % activityLabels.length];
    const button = activityButton(label);
    if (!button) throw new Error("activity button missing: " + label);
    const started = performance.now();
    button.click();
    await waitFor(() => button.getAttribute("aria-current") === "page");
    await twice();
    if (iteration >= ${options.warmups}) {
      activitySamples.push({ label, elapsedMs: performance.now() - started });
    }
  }

  const warmReloadSamples = [];
  for (let iteration = phase === "reload" ? batchStart : batchEnd;
    iteration < batchEnd; iteration += 1) {
    stage = "reload-trigger:" + iteration;
    const trigger = document.querySelector(".command-center-trigger");
    if (!trigger) throw new Error("command center trigger missing");
    trigger.click();
    stage = "reload-input:" + iteration;
    const input = await waitFor(() => document.querySelector(".command-center input[role=combobox]"));
    input.value = "Reîncarcă sesiunea";
    input.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText", data: "Reîncarcă sesiunea" }));
    stage = "reload-option:" + iteration;
    const option = await waitFor(() => [...document.querySelectorAll(".command-center [role=option]")]
      .find((item) => item.querySelector("strong")?.textContent?.trim() === "Reîncarcă sesiunea"));
    const started = performance.now();
    option.click();
    stage = "reload-complete:" + iteration;
    await waitFor(() => !document.querySelector(".command-center") && document.querySelector("iframe")?.src);
    await twice();
    if (iteration >= ${options.warmups}) warmReloadSamples.push(performance.now() - started);
  }

  const paneTabSamples = [];
  for (let iteration = phase === "pane" ? batchStart : batchEnd;
    iteration < batchEnd; iteration += 1) {
    stage = "pane-tab:" + iteration;
    const id = iteration % 2 === 0 ? "#project-pane-tab-files" : "#project-pane-tab-layers";
    const tab = await waitFor(() => document.querySelector(id));
    const started = performance.now();
    tab.click();
    await waitFor(() => tab.getAttribute("aria-selected") === "true");
    await twice();
    if (iteration >= ${options.warmups}) paneTabSamples.push(performance.now() - started);
  }

  const openResource = async (path) => {
    stage = "resource-trigger:" + path;
    document.querySelector(".command-center-trigger")?.click();
    const input = await waitFor(() => document.querySelector(".command-center input[role=combobox]"));
    input.value = path;
    input.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText", data: path }));
    stage = "resource-result:" + path;
    const option = await waitFor(() => [...document.querySelectorAll(".command-center [role=option]")]
      .find((item) => item.title?.includes(path) || item.textContent?.includes(path)));
    option.click();
    stage = "resource-open:" + path;
    await waitFor(() => !document.querySelector(".command-center")
      && [...document.querySelectorAll(".document-tab .document-select")]
        .some((tab) => tab.title?.includes(path)), coldSetupTimeoutMs);
    await waitFor(() => {
      const editor = document.querySelector(".editor-shell");
      return editor?.dataset.activeDocumentPath === path
        && editor.dataset.sourceLoading === "false"
        && document.querySelector("iframe")?.getAttribute("aria-busy") !== "true";
    }, coldSetupTimeoutMs);
  };

  const documentPhase = phase.startsWith("document-");
  const documentPaths = {
    codeA: "config.toml",
    codeB: "static/js/site.js",
    template: "templates/index.html",
  };
  const editorState = () => document.querySelector(".editor-shell");
  const tabForPath = (path) => [...document.querySelectorAll(".document-tab .document-select")]
    .find((candidate) => candidate.title === path);
  const measureTabSelection = async (path, action, startedAt = performance.now()) => {
    const target = tabForPath(path);
    if (!target) throw new Error("document tab missing: " + path);
    if (target.getAttribute("aria-selected") === "true") {
      throw new Error("document tab already selected before input: " + path);
    }
    const tabList = target.closest('[role="tablist"]');
    if (!tabList) throw new Error("document tablist missing: " + path);
    const selectedAt = await new Promise((resolve, reject) => {
      let settled = false;
      let timeout = 0;
      const observer = new MutationObserver(() => {
        if (tabForPath(path)?.getAttribute("aria-selected") !== "true") return;
        if (settled) return;
        settled = true;
        clearTimeout(timeout);
        observer.disconnect();
        resolve(performance.now());
      });
      observer.observe(tabList, {
        subtree: true,
        attributes: true,
        attributeFilter: ["aria-selected"],
      });
      timeout = setTimeout(() => {
        if (settled) return;
        settled = true;
        observer.disconnect();
        reject(new Error("runtime probe timeout: " + stage + ":tab-selected"));
      }, 30000);
      try {
        action();
        if (tabForPath(path)?.getAttribute("aria-selected") === "true") {
          settled = true;
          clearTimeout(timeout);
          observer.disconnect();
          resolve(performance.now());
        }
      } catch (error) {
        settled = true;
        clearTimeout(timeout);
        observer.disconnect();
        reject(error);
      }
    });
    return { startedAt, selectedAt, elapsedMs: selectedAt - startedAt };
  };
  const waitForDocumentReady = async (path, minimumSerial) => await waitFor(() => {
    const editor = editorState();
    const serial = Number(editor?.dataset.documentActivationSerial ?? -1);
    return editor?.dataset.activeDocumentPath === path
      && editor.dataset.documentActivationPath === path
      && editor.dataset.documentActivationPhase === "ready"
      && editor.dataset.sourceLoading === "false"
      && serial > minimumSerial
      ? editor
      : null;
  });
  const activationMetrics = (editor) => ({
    reportedIntentMs: Number(editor.dataset.documentActivationIntentMs ?? NaN),
    reportedResolveMs: Number(editor.dataset.documentActivationResolveMs ?? NaN),
    reportedLoadMs: Number(editor.dataset.documentActivationLoadMs ?? NaN),
    reportedSurfaceMs: Number(editor.dataset.documentActivationSurfaceMs ?? NaN),
    reportedTotalMs: Number(editor.dataset.documentActivationTotalMs ?? NaN),
  });
  const activateDocument = async (path, measured = true) => {
    stage = "document-activate:" + path;
    const tab = await waitFor(() => tabForPath(path));
    const editor = editorState();
    const previousPath = editor?.dataset.activeDocumentPath ?? "";
    const previousSurface = editor?.dataset.documentActivationSurface
      ?? editor?.dataset.centerView
      ?? "unknown";
    const minimumSerial = Number(editor?.dataset.documentActivationSerial ?? -1);
    const selection = await measureTabSelection(path, () => tab.click());
    const started = selection.startedAt;
    const tabActivationMs = selection.elapsedMs;
    const settled = await waitForDocumentReady(path, minimumSerial);
    const readyMs = performance.now() - started;
    return measured ? {
      path,
      previousPath,
      direction: previousPath + "→" + path,
      previousSurface,
      surface: settled.dataset.documentActivationSurface ?? "unknown",
      cacheOutcome: settled.dataset.documentActivationCacheOutcome ?? "unknown",
      tabActivationMs,
      readyMs,
      ...activationMetrics(settled),
    } : null;
  };

  if (phase === "document-code" && batchStart === 0) {
    document.querySelector("#project-pane-tab-files")?.click();
    await openResource(documentPaths.codeA);
    await openResource(documentPaths.codeB);
    await openResource(documentPaths.template);
    await activateDocument(documentPaths.codeB, false);
  }
  const documentSamples = [];
  for (let iteration = documentPhase ? batchStart : batchEnd;
    iteration < batchEnd; iteration += 1) {
    if (phase === "document-code") {
      stage = "document-code-switch:" + iteration;
      const target = iteration % 2 === 0 ? documentPaths.codeA : documentPaths.codeB;
      const opposite = target === documentPaths.codeA ? documentPaths.codeB : documentPaths.codeA;
      if (editorState()?.dataset.activeDocumentPath === target) {
        await activateDocument(opposite, false);
      }
      const sample = await activateDocument(target);
      if (iteration >= ${options.warmups}) {
        documentSamples.push({ ...sample, scenario: "code_to_code" });
      }
    } else if (phase === "document-template") {
      stage = "document-template-reactivation:" + iteration;
      if (editorState()?.dataset.activeDocumentPath !== documentPaths.codeA) {
        await activateDocument(documentPaths.codeA, false);
      }
      const sample = await activateDocument(documentPaths.template);
      if (iteration >= ${options.warmups}) {
        documentSamples.push({ ...sample, scenario: "canonical_template_reactivation" });
      }
    }
  }

  const rapidDocumentSamples = [];
  for (let iteration = phase === "document-rapid" ? batchStart : batchEnd;
    iteration < batchEnd; iteration += 1) {
    stage = "document-rapid-switch:" + iteration;
    if (editorState()?.dataset.activeDocumentPath !== documentPaths.codeA) {
      await activateDocument(documentPaths.codeA, false);
    }
    const editor = editorState();
    const minimumSerial = Number(editor?.dataset.documentActivationSerial ?? -1);
    const started = performance.now();
    const burst = [
      documentPaths.template,
      documentPaths.codeB,
      documentPaths.template,
      documentPaths.codeB,
      documentPaths.template,
    ];
    const selection = await measureTabSelection(documentPaths.template, () => {
      for (const path of burst) tabForPath(path)?.click();
    }, started);
    const tabActivationMs = selection.elapsedMs;
    const settled = await waitForDocumentReady(documentPaths.template, minimumSerial);
    if (iteration >= ${options.warmups}) rapidDocumentSamples.push({
      path: documentPaths.template,
      direction: "rapid_alternation→" + documentPaths.template,
      surface: settled.dataset.documentActivationSurface ?? "unknown",
      cacheOutcome: settled.dataset.documentActivationCacheOutcome ?? "unknown",
      burstSize: burst.length,
      tabActivationMs,
      readyMs: performance.now() - started,
      ...activationMetrics(settled),
    });
  }

  const inspectorSamples = [];
  for (let iteration = phase === "inspector" ? batchStart : batchEnd;
    iteration < batchEnd; iteration += 1) {
    stage = "inspector:" + iteration;
    const control = await waitFor(() => [...document.querySelectorAll("button")]
      .find((button) => button.title?.includes("Inspector")));
    const started = performance.now();
    control.click();
    await twice();
    if (iteration >= ${options.warmups}) inspectorSamples.push(performance.now() - started);
  }

  const frameDeltas = [];
  stage = "frame-sampling";
  let previous = null;
  for (let index = phase === "frames" ? 0 : ${options.frameSamples + 1};
    index <= ${options.frameSamples}; index += 1) {
    const current = await new Promise((resolve) => requestAnimationFrame(resolve));
    if (previous !== null) frameDeltas.push(current - previous);
    previous = current;
  }

  const compositionProbe = phase === "frames"
    ? window.__PANA_APPLICATION_COMPOSITION_RUNTIME__
    : null;
  return {
    schemaVersion: 1,
    activitySamples,
    warmReloadSamples,
    paneTabSamples,
    documentSamples,
    rapidDocumentSamples,
    inspectorSamples,
    frameDeltas,
    visibilityState: document.visibilityState,
    composition: compositionProbe?.read?.() ?? null,
    reactiveLayout: compositionProbe?.runReactiveUpdates?.(${options.samples}) ?? null,
    routes: [],
  };
})()`;
}

export function probeBatches(totalIterations, batchSize = 10) {
  if (!Number.isSafeInteger(totalIterations) || totalIterations < 1) {
    throw new Error("totalIterations must be a positive safe integer");
  }
  if (!Number.isSafeInteger(batchSize) || batchSize < 1) {
    throw new Error("batchSize must be a positive safe integer");
  }
  const batches = [];
  for (let start = 0; start < totalIterations; start += batchSize) {
    batches.push({ start, end: Math.min(totalIterations, start + batchSize) });
  }
  return batches;
}

export function probeBatchTimeoutMs(batchIterations, interactionTimeoutMs = 30_000) {
  if (!Number.isSafeInteger(batchIterations) || batchIterations < 1) {
    throw new Error("batchIterations must be a positive safe integer");
  }
  if (!Number.isSafeInteger(interactionTimeoutMs) || interactionTimeoutMs < 1) {
    throw new Error("interactionTimeoutMs must be a positive safe integer");
  }
  return Math.max(120_000, batchIterations * interactionTimeoutMs + 60_000);
}

function routeExpression(route) {
  return `
(async () => {
  const frame = document.querySelector("iframe");
  if (!frame) throw new Error("preview iframe missing");
  const target = new URL(${JSON.stringify(route)}, frame.src);
  const started = performance.now();
  await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("route timeout: " + target.pathname)), 15000);
    frame.addEventListener("load", () => { clearTimeout(timeout); resolve(); }, { once: true });
    frame.src = target.href;
  });
  await Promise.race([
    new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))),
    new Promise((resolve) => setTimeout(resolve, 500)),
  ]);
  return { route: target.pathname, url: target.href, elapsedMs: performance.now() - started };
})()`;
}

const routeMetricsExpression = `(() => {
  const cssRuleCount = [...document.styleSheets].reduce((total, sheet) => {
    try { return total + sheet.cssRules.length; } catch { return total; }
  }, 0);
  const images = [...document.images];
  return {
    elementCount: document.querySelectorAll("*").length,
    cssRuleCount,
    imageCount: images.length,
    loadedImageCount: images.filter((image) => image.complete && image.naturalWidth > 0).length,
  };
})()`;

function emit(sample) {
  process.stdout.write(`${JSON.stringify({
    schemaVersion: 1,
    kind: "sample",
    status: "ok",
    attributes: {},
    ...sample,
  })}\n`);
}

function emitResults(profile, result, offsets = {}) {
  result.activitySamples.forEach((item, iteration) => emit({
    layer: "rendering",
    scenario: "activity_switch",
    profile,
    mode: "warm",
    metric: "input_to_two_raf",
    value: item.elapsedMs,
    unit: "ms",
    iteration: (offsets.activitySamples ?? 0) + iteration,
    attributes: { activity: item.label },
  }));
  result.frameDeltas.forEach((value, iteration) => emit({
    layer: "rendering",
    scenario: "workspace_frames",
    profile,
    mode: "sustained",
    metric: "frame_delta",
    value,
    unit: "ms",
    iteration: (offsets.frameDeltas ?? 0) + iteration,
    status: value > 50 ? "long_frame" : "ok",
    attributes: { visibilityState: result.visibilityState },
  }));
  result.warmReloadSamples.forEach((value, iteration) => emit({
    layer: "ui",
    scenario: "project_open",
    profile,
    mode: "warm_process",
    metric: "canvas_accessible",
    value,
    unit: "ms",
    iteration: (offsets.warmReloadSamples ?? 0) + iteration,
    attributes: { operation: "reload_session" },
  }));
  for (const [scenario, values, offsetKey] of [
    ["pane_tab_switch", result.paneTabSamples, "paneTabSamples"],
    ["inspector_toggle", result.inspectorSamples, "inspectorSamples"],
  ]) values.forEach((value, iteration) => emit({
    layer: "interactions",
    scenario,
    profile,
    mode: "warm",
    metric: "input_to_two_raf",
    value,
    unit: "ms",
    iteration: (offsets[offsetKey] ?? 0) + iteration,
  }));
  result.documentSamples.forEach((sample, iteration) => {
    const attributes = {
      direction: sample.direction,
      previousPath: sample.previousPath,
      path: sample.path,
      previousSurface: sample.previousSurface,
      surface: sample.surface,
      cacheOutcome: sample.cacheOutcome,
      reportedIntentMs: sample.reportedIntentMs,
      reportedResolveMs: sample.reportedResolveMs,
      reportedLoadMs: sample.reportedLoadMs,
      reportedSurfaceMs: sample.reportedSurfaceMs,
      reportedTotalMs: sample.reportedTotalMs,
    };
    for (const [metric, value] of [
      ["input_to_tab_selected", sample.tabActivationMs],
      ["input_to_document_ready", sample.readyMs],
    ]) emit({
      layer: "interactions",
      scenario: sample.scenario,
      profile,
      mode: "warm",
      metric,
      value,
      unit: "ms",
      iteration: (offsets.documentSamples ?? 0) + iteration,
      attributes,
    });
  });
  result.rapidDocumentSamples.forEach((sample, iteration) => {
    const attributes = {
      direction: sample.direction,
      path: sample.path,
      surface: sample.surface,
      cacheOutcome: sample.cacheOutcome,
      burstSize: sample.burstSize,
      reportedIntentMs: sample.reportedIntentMs,
      reportedResolveMs: sample.reportedResolveMs,
      reportedLoadMs: sample.reportedLoadMs,
      reportedSurfaceMs: sample.reportedSurfaceMs,
      reportedTotalMs: sample.reportedTotalMs,
    };
    for (const [metric, value] of [
      ["input_to_tab_selected", sample.tabActivationMs],
      ["input_to_document_ready", sample.readyMs],
    ]) emit({
      layer: "interactions",
      scenario: "rapid_document_alternation",
      profile,
      mode: "warm",
      metric,
      value,
      unit: "ms",
      iteration: (offsets.rapidDocumentSamples ?? 0) + iteration,
      attributes,
    });
  });
  result.routes.forEach((route, routeIteration) => {
    const iteration = (offsets.routes ?? 0) + routeIteration;
    emit({
      layer: "rendering",
      scenario: "preview_route",
      profile,
      mode: "warm",
      metric: "load_to_two_raf",
      value: route.elapsedMs,
      unit: "ms",
      iteration,
      attributes: { route: route.route },
    });
    for (const [metric, value] of [
      ["element_count", route.elementCount],
      ["css_rule_count", route.cssRuleCount],
      ["image_count", route.imageCount],
      ["loaded_image_count", route.loadedImageCount],
    ]) emit({
      layer: "rendering",
      scenario: "preview_route",
      profile,
      mode: "snapshot",
      metric,
      value,
      unit: "count",
      iteration,
      status: route.metricsAvailable ? "ok" : "unavailable",
      attributes: {
        route: route.route,
        metricsAvailable: route.metricsAvailable,
        measurementReason: route.metricsAvailable
          ? "webkit_runtime_context"
          : "isolated_cross_origin_preview_context_unavailable",
        discoveredTargetCount: route.discoveredTargetCount,
        discoveredContextCount: route.discoveredContextCount,
      },
    });
  });
  if (result.composition) {
    for (const [metric, value] of [
      ["composition_construction", result.composition.constructionMs],
      ["composition_effect_settle", result.composition.effectSettleFromConstructionStartMs],
    ]) {
      if (Number.isFinite(value)) emit({
        layer: "frontend",
        scenario: "application_composition",
        profile,
        mode: "cold_process",
        metric,
        value,
        unit: "ms",
        iteration: 0,
      });
    }
  }
  (result.reactiveLayout?.samplesMs ?? []).forEach((value, iteration) => emit({
    layer: "frontend",
    scenario: "reactive_workspace_layout",
    profile,
    mode: "warm",
    metric: "flush_sync",
    value,
    unit: "ms",
    iteration,
  }));
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  const socketUrl = await inspectorSocket(options.endpoint, options.timeoutMs);
  const connection = await connect(socketUrl, options.timeoutMs);
  try {
    const totalIterations = options.samples + options.warmups;
    const batchSize = 10;
    const sampleKeyByPhase = {
      activity: "activitySamples",
      reload: "warmReloadSamples",
      pane: "paneTabSamples",
      "document-code": "documentSamples",
      "document-template": "documentSamples",
      "document-rapid": "rapidDocumentSamples",
      inspector: "inspectorSamples",
    };
    for (const phase of Object.keys(sampleKeyByPhase)) {
      for (const { start: batchStart, end: batchEnd } of probeBatches(
        totalIterations,
        batchSize,
      )) {
        const batchTimeoutMs = probeBatchTimeoutMs(batchEnd - batchStart);
        const result = await connection.evaluate(
          probeExpression(options, phase, batchStart, batchEnd),
          undefined,
          batchTimeoutMs,
        );
        if (!result) {
          throw new Error(`WebKit runtime probe returned no value: ${phase}:${batchStart}`);
        }
        emitResults(options.profile, result, {
          [sampleKeyByPhase[phase]]: Math.max(0, batchStart - options.warmups),
        });
      }
    }
    const frameResult = await connection.evaluate(
      probeExpression(options, "frames", 0, 0),
      undefined,
      120_000,
    );
    if (!frameResult) throw new Error("WebKit frame probe returned no value");
    emitResults(options.profile, frameResult);
    for (const [routeIteration, route] of [
      "/laboratoare/densitate/",
      "/laboratoare/motion/",
      "/laboratoare/media/",
    ].entries()) {
      const navigation = await connection.evaluate(routeExpression(route), undefined, 20_000);
      const targetId = await connection.findRuntimeTarget(navigation.url);
      const contextId = targetId ? null : await connection.findRuntimeContext(navigation.url);
      const metrics = targetId
        ? await connection.evaluate(routeMetricsExpression, targetId, 5_000)
        : contextId !== null
          ? await connection.evaluate(
              routeMetricsExpression,
              connection.pageTargetId,
              5_000,
              contextId,
            )
          : null;
      emitResults(options.profile, {
        activitySamples: [],
        warmReloadSamples: [],
        paneTabSamples: [],
        documentSamples: [],
        rapidDocumentSamples: [],
        inspectorSamples: [],
        frameDeltas: [],
        composition: null,
        reactiveLayout: null,
        routes: [{
          ...navigation,
          elementCount: metrics?.elementCount ?? 0,
          cssRuleCount: metrics?.cssRuleCount ?? 0,
          imageCount: metrics?.imageCount ?? 0,
          loadedImageCount: metrics?.loadedImageCount ?? 0,
          metricsAvailable: metrics !== null,
          discoveredTargetCount: connection.targets.size,
          discoveredContextCount: connection.contexts.size,
        }],
      }, { routes: routeIteration });
    }
  } finally {
    connection.socket.close();
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().then(
    () => process.exit(0),
    (error) => {
      console.error(`[pana-performance-webkit] ${error.message}`);
      process.exit(1);
    },
  );
}
