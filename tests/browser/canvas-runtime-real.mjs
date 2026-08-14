import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const bridgeParts = [
  "00_bootstrap.js",
  "01_dom_structure.js",
  "02_css_inspection.js",
  "03_overlay_geometry.js",
  "03_canvas_agent.js",
  "06_empty_zones.js",
  "07_drag_drop.js",
  "08_inspector_shell.js",
  "09_design_safe_surface.js",
  "10_canvas_patch.js",
  "11_document_sync.js",
  "12_messages_events.js",
  "13_boot.js",
];

const bridgeSources = await Promise.all(
  bridgeParts.map((part) => readFile(
    resolve(repoRoot, "src-tauri/src/preview/bridge", part),
    "utf8",
  )),
);
const htmlEditorSchema = await readFile(
  resolve(repoRoot, "src/lib/html/editor-schema.json"),
  "utf8",
);
const bridge = [
  bridgeSources[0],
  `var HTML_EDITOR_SCHEMA = ${htmlEditorSchema};\n`,
  ...bridgeSources.slice(1),
].join("");
const interactiveRuntime = await readFile(
  resolve(repoRoot, "src-tauri/src/preview/interactive_runtime.js"),
  "utf8",
);
const blockRuntimeCore = await readFile(
  resolve(repoRoot, "src-tauri/src/blocks/runtime.js"),
  "utf8",
);
const blockRuntimeProviders = await Promise.all(
  ["accordion.js", "slider.js"].map((provider) => readFile(
    resolve(repoRoot, "src-tauri/src/blocks/runtime", provider),
    "utf8",
  )),
);
const blockRuntime = [
  blockRuntimeCore,
  ...blockRuntimeProviders,
  'window.PanaBlockRuntime.installPageConfig({blocks:[{id:"accordion"},{id:"slider"}]});',
].join("\n");
assert.equal(blockRuntime.match(/PANA BLOCK RUNTIME CORE/g)?.length, 1);
assert.equal(blockRuntime.match(/PANA BLOCK PROVIDER: accordion/g)?.length, 1);
assert.equal(blockRuntime.match(/PANA BLOCK PROVIDER: slider/g)?.length, 1);
assert.doesNotMatch(blockRuntime, /PANA BLOCK PROVIDER: (?:counter|tabs|dialog|offcanvas|nav-menu)/);
assert.doesNotMatch(blockRuntime, /__panaMotionV2Config/);
const fontFixture = await readFile(
  resolve(
    repoRoot,
    "src-tauri/resources/theme-packs/radacini/theme/static/fonturi/inter-400-700-latin-ext.woff2",
  ),
);

const identity = {
  projectRoot: "/project",
  runtimeSessionId: "runtime-browser-real",
  workspaceRevision: 107,
  transactionId: "canvas_next_browser_real",
  previewRevision: "preview-next-browser-real",
};
const oldCss = "/old.css";
const nextCss = "/next.css";
const fontCss = `@font-face{font-family:"Pana Runtime Probe";src:url("/font-probe.woff2") format("woff2");font-style:normal;font-weight:400 700;font-display:swap}`;

function escapeInlineScript(source) {
  return source.replaceAll("</script", "<\\/script");
}

function htmlJson(value) {
  return JSON.stringify(value).replaceAll("<", "\\u003c");
}

const initialDocument = `<!doctype html>
<html data-pana-preview-revision="preview-active-browser-real"
      data-pana-canvas-project-root="/project"
      data-pana-canvas-runtime-session-id="runtime-browser-real"
      data-pana-canvas-workspace-revision="1"
      data-pana-canvas-transaction-id="canvas_active_browser_real">
  <head><meta name="description" content="Before"><link rel="preload" href="/font-probe.woff2" as="font" type="font/woff2" crossorigin><link rel="stylesheet" href="${oldCss}"><style>html,body,h1{margin:0}header{height:72px}main{min-height:calc(100vh - 136px)}footer{height:64px}</style></head>
  <body><header><h1 id="probe" data-pana-source-id="source-title" data-pana-render-instance-id="render-title">Before</h1><a id="nav-probe" data-pana-source-id="source-nav" data-pana-render-instance-id="render-nav" href="/servicii">Servicii</a><svg id="icon-probe" class="icon custom-icon" style="color: rgb(194, 65, 12)" data-pana-block="icon" data-pana-instance="icon-browser-real" data-pana-icon="tabler-outline:home" data-pana-source-id="source-icon" data-pana-render-instance-id="render-icon" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false"><path fill="currentColor" d="M 3 3 H 21 V 21 H 3 Z"></path></svg></header><main><!-- pana-template-source-start:source-empty-content --><div hidden class="pana-studio-empty-editable pana-studio-empty-tera-slot" data-pana-empty-tera-slot="source-empty-content" data-pana-empty-tera-slot-static="true" data-pana-source-id="source-empty-content" data-pana-template-source-id="source-empty-content"></div><!-- pana-template-source-end:source-empty-content --></main><footer id="flow-footer">Footer extern</footer><!-- pana-template-source-start:source-external-scripts --><!-- pana-template-source-end:source-external-scripts -->
  <script>
    window.addEventListener("error", function (event) {
      window.parent.postMessage({source:"pana-browser-harness",type:"child-error",message:String(event.message || "error"),line:event.lineno,column:event.colno}, "*");
    });
    window.addEventListener("unhandledrejection", function (event) {
      window.parent.postMessage({source:"pana-browser-harness",type:"child-rejection",message:String(event.reason && event.reason.stack || event.reason || "rejection")}, "*");
    });
    window.parent.postMessage({source:"pana-browser-harness",type:"child-script-started"}, "*");
  </script>
  <script>${escapeInlineScript(bridge)}</script></body>
</html>`;

const canonicalDocument = `<!doctype html>
<html data-pana-preview-revision="${identity.previewRevision}"
      data-pana-canvas-project-root="${identity.projectRoot}"
      data-pana-canvas-runtime-session-id="${identity.runtimeSessionId}"
      data-pana-canvas-workspace-revision="${identity.workspaceRevision}"
      data-pana-canvas-workspace-transaction-id="workspace-browser-real-107"
      data-pana-canvas-transaction-id="${identity.transactionId}">
  <head><!-- pana-template-source-start:sg_head_description --><meta name="description" content="After"><!-- pana-template-source-end:sg_head_description --><link rel="preload" href="/font-probe.woff2" as="font" type="font/woff2" crossorigin><link rel="stylesheet" href="${nextCss}"></head>
  <body><main><h1 id="probe" data-pana-source-id="source-title" data-pana-render-instance-id="render-title">After</h1><a id="nav-probe" data-pana-source-id="source-nav" data-pana-render-instance-id="render-nav" href="/despre">Servicii</a><svg id="icon-probe" class="icon custom-icon" style="color: rgb(194, 65, 12)" data-pana-block="icon" data-pana-instance="icon-browser-real" data-pana-icon="tabler-outline:home" data-pana-source-id="source-icon" data-pana-render-instance-id="render-icon" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false"><path fill="currentColor" d="M 3 3 H 21 V 21 H 3 Z"></path></svg></main></body>
</html>`;

const reorderedResourceDocument = `<!doctype html>
<html data-pana-preview-revision="${identity.previewRevision}"
      data-pana-canvas-project-root="${identity.projectRoot}"
      data-pana-canvas-runtime-session-id="${identity.runtimeSessionId}"
      data-pana-canvas-workspace-revision="${identity.workspaceRevision}"
      data-pana-canvas-workspace-transaction-id="workspace-browser-real-107"
      data-pana-canvas-transaction-id="${identity.transactionId}">
  <head><!-- pana-template-source-start:sg_head_description --><meta name="description" content="After"><!-- pana-template-source-end:sg_head_description --><link rel="stylesheet" href="${nextCss}"><link rel="preload" href="/font-probe.woff2" as="font" type="font/woff2" crossorigin></head>
  <body><main><h1 id="probe" data-pana-source-id="source-title" data-pana-render-instance-id="render-title">After</h1><a id="nav-probe" data-pana-source-id="source-nav" data-pana-render-instance-id="render-nav" href="/despre">Servicii</a></main></body>
</html>`;

const brokenStylesheetDocument = `<!doctype html>
<html data-pana-preview-revision="${identity.previewRevision}"
      data-pana-canvas-project-root="${identity.projectRoot}"
      data-pana-canvas-runtime-session-id="${identity.runtimeSessionId}"
      data-pana-canvas-workspace-revision="${identity.workspaceRevision}"
      data-pana-canvas-transaction-id="${identity.transactionId}">
  <head><link rel="stylesheet" href="/broken.css"></head>
  <body><main><h1 id="probe" data-pana-source-id="source-title" data-pana-render-instance-id="render-title">Broken candidate</h1></main></body>
</html>`;

const missingFontDocument = `<!doctype html>
<html data-pana-preview-revision="${identity.previewRevision}"
      data-pana-canvas-project-root="${identity.projectRoot}"
      data-pana-canvas-runtime-session-id="${identity.runtimeSessionId}"
      data-pana-canvas-workspace-revision="${identity.workspaceRevision}"
      data-pana-canvas-transaction-id="${identity.transactionId}">
  <head><meta name="description" content="Font fallback"><link rel="stylesheet" href="/missing-font.css"></head>
  <body><main><h1 id="probe" data-pana-source-id="source-title" data-pana-render-instance-id="render-title">Fallback candidate</h1></main></body>
</html>`;

const interactiveDocument = `<!doctype html>
<html data-pana-preview-revision="interactive-browser-real">
  <body>
    <section data-pana-block="accordion" data-pana-source-id="source-accordion">
      <div data-pana-accordion-item>
        <button data-pana-accordion-trigger aria-expanded="false">Toggle</button>
        <div data-pana-accordion-panel hidden>Panel</div>
      </div>
      <div data-pana-accordion-item>
        <button data-pana-accordion-trigger aria-expanded="false">Toggle 2</button>
        <div data-pana-accordion-panel hidden>Panel 2</div>
      </div>
    </section>
    <div data-pana-block="slider" data-pana-source-id="source-slider" aria-label="Slider test">
      <div data-pana-slider-viewport><div data-pana-slider-track data-pana-slot="slides">
        <div data-pana-slider-slide data-pana-source-id="source-slide-1">Slide 1</div>
        <div data-pana-slider-slide data-pana-source-id="source-slide-2">Slide 2</div>
      </div></div>
      <div data-pana-slider-controls>
        <button data-pana-slider-previous>Previous</button>
        <div data-pana-slider-indicators></div>
        <button data-pana-slider-next>Next</button>
        <button data-pana-slider-autoplay hidden>Stop</button>
      </div>
    </div>
    <div data-pana-block="slider" data-pana-source-id="source-slider-autoplay" data-autoplay="true" data-interval="1000" aria-label="Slider autoplay test">
      <div data-pana-slider-viewport><div data-pana-slider-track data-pana-slot="slides">
        <div data-pana-slider-slide data-pana-source-id="source-auto-slide-1">Auto 1</div>
        <div data-pana-slider-slide data-pana-source-id="source-auto-slide-2">Auto 2</div>
      </div></div>
      <div data-pana-slider-controls>
        <button data-pana-slider-previous>Previous</button>
        <div data-pana-slider-indicators></div>
        <button data-pana-slider-next>Next</button>
        <button data-pana-slider-autoplay hidden>Stop</button>
      </div>
    </div>
    <script>
      window.__panaReducedMotion = false;
      window.__panaMotionListeners = [];
      window.matchMedia = function () {
        return {
          get matches() { return window.__panaReducedMotion; },
          addEventListener: function (type, listener) { if (type === "change") window.__panaMotionListeners.push(listener); },
          removeEventListener: function (type, listener) { window.__panaMotionListeners = window.__panaMotionListeners.filter(function (candidate) { return candidate !== listener; }); }
        };
      };
    </script>
    <script>${escapeInlineScript(blockRuntime)}</script>
    <script>${escapeInlineScript(interactiveRuntime)}</script>
  </body>
</html>`;

const harness = `<!doctype html>
<html><head><meta charset="utf-8"><title>RUNNING</title><style>#canvas{width:800px;height:600px}</style></head>
<body><pre id="result">running</pre>
<iframe id="canvas" sandbox="allow-scripts allow-same-origin"></iframe>
<iframe id="interactive" sandbox="allow-scripts allow-same-origin"></iframe>
<script>
(() => {
  const frame = document.getElementById("canvas");
  const interactiveFrame = document.getElementById("interactive");
  const result = document.getElementById("result");
  const initialDocument = ${htmlJson(initialDocument)};
  const canonicalDocument = ${htmlJson(canonicalDocument)};
  const reorderedResourceDocument = ${htmlJson(reorderedResourceDocument)};
  const brokenStylesheetDocument = ${htmlJson(brokenStylesheetDocument)};
  const missingFontDocument = ${htmlJson(missingFontDocument)};
  const interactiveDocument = ${htmlJson(interactiveDocument)};
  const identity = ${JSON.stringify(identity)};
  const messages = [];
  const canvasAgentMessages = [];
  const interactiveMessages = [];
  const childDiagnostics = [];
  const colors = [];
  const patchRoundTrips = [];
  const patchBridgeDurations = [];
  const historyPatchRoundTrips = [];
  const historyPatchBridgeDurations = [];
  let sample = true;

  function finish(ok, details) {
    sample = false;
    result.textContent = JSON.stringify({ ok, ...details });
    document.title = ok ? "PASS" : "FAIL";
  }

  function waitForMessage(predicate, timeoutMs = 12000) {
    return new Promise((resolve, reject) => {
      const existing = messages.find(predicate);
      if (existing) return resolve(existing);
      const timeout = setTimeout(() => {
        window.removeEventListener("message", listener);
        reject(new Error("browser bridge message timeout"));
      }, timeoutMs);
      function listener(event) {
        if (event.source !== frame.contentWindow || !predicate(event.data)) return;
        clearTimeout(timeout);
        window.removeEventListener("message", listener);
        resolve(event.data);
      }
      window.addEventListener("message", listener);
    });
  }

  function waitForInteractiveMessage(predicate, timeoutMs = 12000) {
    return new Promise((resolve, reject) => {
      const existing = interactiveMessages.find(predicate);
      if (existing) return resolve(existing);
      const timeout = setTimeout(() => {
        window.removeEventListener("message", listener);
        reject(new Error("interactive runtime message timeout"));
      }, timeoutMs);
      function listener(event) {
        if (event.source !== interactiveFrame.contentWindow || !predicate(event.data)) return;
        clearTimeout(timeout);
        window.removeEventListener("message", listener);
        resolve(event.data);
      }
      window.addEventListener("message", listener);
    });
  }

  function waitForCanvasAgentMessage(predicate, timeoutMs = 12000) {
    return new Promise((resolve, reject) => {
      const existing = canvasAgentMessages.find(predicate);
      if (existing) return resolve(existing);
      const timeout = setTimeout(() => {
        window.removeEventListener("message", listener);
        reject(new Error("canvas agent message timeout"));
      }, timeoutMs);
      function listener(event) {
        if (event.source !== frame.contentWindow || !predicate(event.data)) return;
        clearTimeout(timeout);
        window.removeEventListener("message", listener);
        resolve(event.data);
      }
      window.addEventListener("message", listener);
    });
  }

  window.addEventListener("message", (event) => {
    if (event.source === frame.contentWindow && event.data?.source === "pana-studio-preview") {
      messages.push(event.data);
    }
    if (event.source === frame.contentWindow && event.data?.source === "pana-studio-canvas-agent") {
      canvasAgentMessages.push(event.data);
    }
    if (event.source === interactiveFrame.contentWindow && event.data?.source === "pana-studio-interactive") {
      interactiveMessages.push(event.data);
    }
    if (event.source === frame.contentWindow && event.data?.source === "pana-browser-harness") {
      childDiagnostics.push(event.data);
    }
  });

  function sampleColor() {
    if (!sample) return;
    const probe = frame.contentDocument?.getElementById("probe");
    if (probe) colors.push(frame.contentWindow.getComputedStyle(probe).color);
    frame.contentWindow?.requestAnimationFrame(sampleColor);
  }

  async function run() {
    frame.srcdoc = initialDocument;
    const ready = await waitForMessage((data) => data?.type === "ready");
    if (ready.canvasPhaseReceipts?.map((entry) => entry.phase).join(",") !== "resourcesReady,committed,styledReady") {
      throw new Error("boot phase sequence mismatch");
    }
    const agentReady = await waitForCanvasAgentMessage((data) => data?.type === "agentReady");
    const preActivationSlot = frame.contentDocument.querySelector(
      '[data-pana-empty-tera-slot="source-empty-content"]'
    );
    if (
      !preActivationSlot?.hidden
      || preActivationSlot.getClientRects().length !== 0
      || preActivationSlot.hasAttribute("data-pana-empty-label")
      || frame.contentDocument.querySelector("main")?.hasAttribute("data-pana-empty-html")
    ) {
      throw new Error("empty document leaked a transient placeholder before Rust activation");
    }
    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "activate-canvas-interaction-agent",
      schemaVersion: 2,
      agentInstanceId: agentReady.agentInstanceId,
      documentEpoch: 1,
      lastAcceptedSequence: 0,
      selection: true,
      authoringSurfaces: [{
        sourceNodeId: "source-empty-content",
        boundaryInstanceId: "boundary-empty-content",
        renderInstanceId: null
      }]
    }, "*");
    const agentActivated = await waitForCanvasAgentMessage((data) =>
      data?.type === "agentActivated"
    );
    if (
      agentActivated.agentInstanceId !== agentReady.agentInstanceId
      || agentActivated.documentEpoch !== 1
    ) {
      throw new Error("CanvasAgent activation acknowledgement mismatch");
    }
    await new Promise((resolve) => frame.contentWindow.requestAnimationFrame(() =>
      frame.contentWindow.requestAnimationFrame(resolve)
    ));
    const dynamicAuthoringSlot = frame.contentDocument.querySelector(
      '[data-pana-active-document-root="source-empty-content"]'
    );
    const authoringOverlay = frame.contentDocument.querySelector(
      '[data-pana-active-authoring-overlay="0"]'
    );
    if (!(dynamicAuthoringSlot instanceof frame.contentWindow.Element)) {
      throw new Error("persistent active document root was not materialized");
    }
    if (
      frame.contentDocument.querySelectorAll('[data-pana-active-document-root="source-empty-content"]').length !== 1
      || frame.contentDocument.querySelector('[data-pana-empty-tera-slot="source-empty-content"]')
    ) {
      throw new Error("active document root did not replace the bootstrap placeholder exactly once");
    }
    if (dynamicAuthoringSlot.getAttribute("data-pana-empty-label") !== "Document gol") {
      throw new Error("active document slot leaked its internal Tera implementation label");
    }
    if (
      dynamicAuthoringSlot.hidden
      || dynamicAuthoringSlot.hasAttribute("data-pana-empty-tera-slot-static")
      || dynamicAuthoringSlot.getAttribute("data-pana-active-authoring-surface")
        !== "boundary-empty-content"
    ) {
      throw new Error("active document root did not adopt the exact Rust boundary instance");
    }
    if (frame.contentDocument.querySelector('[data-pana-empty-tera-slot="source-external-scripts"]')) {
      throw new Error("external empty Tera boundary leaked into the visual authoring surface");
    }
    const authoringSlotRect = dynamicAuthoringSlot.getBoundingClientRect();
    const externalFooterRect = frame.contentDocument.getElementById("flow-footer")?.getBoundingClientRect();
    if (
      authoringSlotRect.height < 400
      || !externalFooterRect
      || Math.abs(externalFooterRect.bottom - frame.contentWindow.innerHeight) > 2
      || frame.contentDocument.querySelector("main")?.hasAttribute("data-pana-empty-html")
      || frame.contentDocument.documentElement.scrollHeight > frame.contentWindow.innerHeight + 1
    ) {
      throw new Error("active empty document did not occupy the flow between external header and footer");
    }
    if (
      authoringOverlay?.style.display !== "block"
      || authoringOverlay.getBoundingClientRect().height
        < dynamicAuthoringSlot.getBoundingClientRect().height - 2
    ) {
      throw new Error("active document root did not receive the expanded authoring surface");
    }
    const authoringCenter = {
      x: authoringSlotRect.left + authoringSlotRect.width / 2,
      y: authoringSlotRect.top + authoringSlotRect.height / 2
    };
    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "preview-insert-drag-drop",
      x: authoringCenter.x,
      y: authoringCenter.y,
      element: {
        id: "html:div",
        kind: "html",
        tag: "div",
        label: "Container",
        text: "",
        className: "",
        html: ""
      }
    }, "*");
    const authoringDrop = await waitForMessage((data) => data?.type === "preview-insert-drop");
    if (
      authoringDrop.targetKind !== "active-document-root"
      || authoringDrop.targetTemplateSourceId !== "source-empty-content"
      || authoringDrop.targetBoundaryInstanceId !== "boundary-empty-content"
    ) {
      throw new Error("empty document drop lost its exact Rust boundary identity");
    }
    const firstSyntheticChild = frame.contentDocument.createElement("div");
    firstSyntheticChild.textContent = "Primul copil";
    dynamicAuthoringSlot.parentNode.insertBefore(firstSyntheticChild, dynamicAuthoringSlot);
    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "activate-canvas-interaction-agent",
      schemaVersion: 2,
      agentInstanceId: agentReady.agentInstanceId,
      documentEpoch: 1,
      lastAcceptedSequence: 0,
      selection: true,
      authoringSurfaces: [{
        sourceNodeId: "source-empty-content",
        boundaryInstanceId: "boundary-empty-content",
        renderInstanceId: null
      }]
    }, "*");
    await waitForCanvasAgentMessage((data) => data?.type === "agentActivated");
    await new Promise((resolve) => frame.contentWindow.requestAnimationFrame(resolve));
    const persistentAuthoringRoot = frame.contentDocument.querySelector(
      '[data-pana-active-document-root="source-empty-content"]'
    );
    if (
      !(persistentAuthoringRoot instanceof frame.contentWindow.Element)
      || persistentAuthoringRoot.hasAttribute("data-pana-empty-label")
      || !persistentAuthoringRoot.hasAttribute("data-pana-active-document-populated")
    ) {
      throw new Error("active document root disappeared after the first authored child");
    }
    const persistentRootRect = persistentAuthoringRoot.getBoundingClientRect();
    const populatedParentRect = persistentAuthoringRoot.parentElement.getBoundingClientRect();
    const persistentDropRect = {
      left: persistentRootRect.left,
      right: persistentRootRect.right,
      top: persistentRootRect.top,
      bottom: populatedParentRect.bottom,
      width: persistentRootRect.width,
      height: Math.max(0, populatedParentRect.bottom - persistentRootRect.top),
    };
    const authoredChildRect = firstSyntheticChild.getBoundingClientRect();
    const populatedFooterRect = frame.contentDocument.getElementById("flow-footer")?.getBoundingClientRect();
    if (
      persistentRootRect.height > 1
      || !persistentDropRect
      || persistentDropRect.height < 300
      || !populatedFooterRect
      || Math.abs(populatedFooterRect.top - populatedParentRect.bottom) > 1
      || persistentDropRect.top < authoredChildRect.bottom - 1
    ) {
      throw new Error("populated active document root still consumes permanent layout space");
    }
    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "preview-insert-drag-drop",
      x: persistentDropRect.left + persistentDropRect.width / 2,
      y: persistentDropRect.top + persistentDropRect.height / 2,
      element: {
        id: "html:section",
        kind: "html",
        tag: "section",
        label: "Secțiune",
        text: "",
        className: "",
        html: ""
      }
    }, "*");
    const secondAuthoringDrop = await waitForMessage((data) =>
      data?.type === "preview-insert-drop" && data?.element?.tag === "section"
    );
    if (
      secondAuthoringDrop.targetKind !== "active-document-root"
      || secondAuthoringDrop.targetTemplateSourceId !== "source-empty-content"
      || secondAuthoringDrop.targetBoundaryInstanceId !== "boundary-empty-content"
    ) {
      throw new Error("second drop fell back to the Tera gate after the first child");
    }
    result.textContent = "canvas-agent-dynamic-authoring";
    document.title = "AGENT_AUTHORING_WAIT";
    const authoringClick = await waitForCanvasAgentMessage((data) =>
      data?.type === "gesture" && data.gesture === "click"
    );
    if (
      authoringClick.documentEpoch !== 1
      || authoringClick.hitPath?.[0]?.kind !== "boundaryInstance"
      || authoringClick.hitPath?.[0]?.id !== "boundary-empty-content"
    ) {
      throw new Error("dynamic authoring surface did not prioritize its Rust boundary");
    }
    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "render-canvas-interaction-overlay",
      agentInstanceId: agentReady.agentInstanceId,
      documentEpoch: 1,
      channel: "selection",
      targetKind: "teraBoundary",
      editorNodeId: "editor_boundary:boundary-empty-content",
      gestureSequence: authoringClick.gestureSequence,
      selectionRevision: 40,
      actions: { canEnterBoundary: false },
      projection: {
        primaryRenderInstanceId: null,
        renderInstanceIds: [],
        boundaryInstanceId: "boundary-empty-content"
      }
    }, "*");
    await new Promise((resolve) => frame.contentWindow.requestAnimationFrame(() =>
      frame.contentWindow.requestAnimationFrame(resolve)
    ));
    const dynamicSelectionOverlay = frame.contentDocument.getElementById(
      "pana-studio-canvas-agent-selection"
    );
    if (
      dynamicSelectionOverlay?.style.display !== "block"
      || dynamicSelectionOverlay.getBoundingClientRect().height
        < dynamicAuthoringSlot.getBoundingClientRect().height - 2
    ) {
      throw new Error("empty Rust boundary did not reuse its dynamic authoring geometry");
    }
    result.textContent = "canvas-agent-native-hover";
    document.title = "AGENT_HOVER_WAIT";
    const agentHover = await waitForCanvasAgentMessage((data) =>
      data?.type === "gesture" && data.gesture === "pointerMove"
    );
    if (
      agentHover.documentEpoch !== 1
      || agentHover.hitPath?.[0]?.kind !== "renderInstance"
      || agentHover.hitPath?.[0]?.id !== "render-title"
    ) {
      throw new Error("trusted CanvasAgent hover hit path mismatch");
    }
    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "render-canvas-interaction-overlay",
      agentInstanceId: agentReady.agentInstanceId,
      documentEpoch: 1,
      channel: "hover",
      targetKind: "htmlElement",
      editorNodeId: "editor_render:render-title",
      gestureSequence: agentHover.gestureSequence,
      actions: { canEnterBoundary: false },
      projection: {
        primaryRenderInstanceId: "render-title",
        renderInstanceIds: ["render-title"],
        boundaryInstanceId: null
      }
    }, "*");
    await new Promise((resolve) => frame.contentWindow.requestAnimationFrame(() =>
      frame.contentWindow.requestAnimationFrame(resolve)
    ));
    const hoverTarget = frame.contentDocument.querySelector(
      '[data-pana-render-instance-id="render-title"]'
    );
    if (hoverTarget?.getAttribute("data-pana-canvas-agent-hover") !== "html") {
      throw new Error("CanvasAgent did not project the Rust-owned hover overlay target");
    }
    const overlayBenchmarkHost = frame.contentDocument.createElement("div");
    overlayBenchmarkHost.style.cssText = "position:fixed;left:-10000px;top:0;width:1000px;display:grid;grid-template-columns:repeat(10,10px)";
    const overlayBenchmarkMembers = [];
    for (let index = 0; index < 100; index += 1) {
      const target = frame.contentDocument.createElement("span");
      const renderInstanceId = "render-overlay-benchmark-" + index;
      target.setAttribute("data-pana-render-instance-id", renderInstanceId);
      target.style.cssText = "display:block;width:10px;height:10px";
      overlayBenchmarkHost.appendChild(target);
      overlayBenchmarkMembers.push({
        memberId: "editor-overlay-benchmark-" + index,
        targetKind: "htmlElement",
        editorNodeId: "editor-overlay-benchmark-" + index,
        actions: { canEnterBoundary: false },
        selectionRevision: 40,
        projection: {
          primaryRenderInstanceId: renderInstanceId,
          renderInstanceIds: [renderInstanceId],
          boundaryInstanceId: null
        }
      });
    }
    frame.contentDocument.body.appendChild(overlayBenchmarkHost);
    const overlayDurations = [];
    for (let sample = 0; sample < 108; sample += 1) {
      const measurementId = "selection-overlay-" + sample;
      frame.contentWindow.postMessage({
        source: "pana-studio-app",
        type: "render-canvas-interaction-overlay",
        agentInstanceId: agentReady.agentInstanceId,
        documentEpoch: 1,
        channel: "selection",
        selectionRevision: 40,
        primaryMemberId: overlayBenchmarkMembers[0].memberId,
        members: overlayBenchmarkMembers,
        measurementId
      }, "*");
      const measured = await waitForCanvasAgentMessage((data) =>
        data?.type === "selectionOverlayRendered" && data.measurementId === measurementId
      );
      if (measured.memberCount !== 100 || !Number.isFinite(measured.renderDurationMs)) {
        throw new Error("CanvasAgent did not confirm the 100-member overlay projection");
      }
      if (sample >= 8) overlayDurations.push(measured.renderDurationMs);
    }
    overlayDurations.sort((left, right) => left - right);
    const overlayP95Ms = overlayDurations[Math.ceil(overlayDurations.length * 0.95) - 1];
    if (!Number.isFinite(overlayP95Ms) || overlayP95Ms >= 16) {
      throw new Error("100-member selection overlay p95 exceeded 16 ms: " + overlayP95Ms);
    }
    overlayBenchmarkHost.remove();
    result.textContent = "canvas-agent-native-click";
    document.title = "AGENT_WAIT";
    const agentClick = await waitForCanvasAgentMessage((data) =>
      data?.type === "gesture"
        && data.gesture === "click"
        && data.gestureSequence > authoringClick.gestureSequence
    );
    if (
      agentClick.documentEpoch !== 1
      || agentClick.hitPath?.[0]?.kind !== "renderInstance"
      || agentClick.hitPath?.[0]?.id !== "render-title"
    ) {
      throw new Error("trusted CanvasAgent hit path mismatch");
    }
    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "render-canvas-interaction-overlay",
      agentInstanceId: agentReady.agentInstanceId,
      documentEpoch: 1,
      channel: "selection",
      targetKind: "htmlElement",
      editorNodeId: "editor_render:render-title",
      gestureSequence: agentClick.gestureSequence,
      selectionRevision: 41,
      actions: { canEnterBoundary: false },
      projection: {
        primaryRenderInstanceId: "render-title",
        renderInstanceIds: ["render-title"],
        boundaryInstanceId: null
      }
    }, "*");
    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "inspect-canvas-interaction-target",
      schemaVersion: 2,
      agentInstanceId: agentReady.agentInstanceId,
      documentEpoch: 1,
      inspectionRequestId: "inspection:1:click",
      renderInstanceId: "render-title"
    }, "*");
    const inspection = await waitForCanvasAgentMessage((data) =>
      data?.type === "domInspection"
        && data.inspectionRequestId === "inspection:1:click"
    );
    if (
      inspection.renderInstanceId !== "render-title"
      || inspection.observation?.tag !== "h1"
      || "sourceId" in inspection.observation
      || "templateSourceId" in inspection.observation
      || inspection.observation?.attributes?.["data-pana-source-id"]
    ) {
      throw new Error("CanvasAgent leaked semantic authority into DOM inspection");
    }
    const agentOverlay = frame.contentDocument.getElementById(
      "pana-studio-canvas-agent-selection"
    );
    if (!agentOverlay || agentOverlay.style.display !== "block") {
      throw new Error("CanvasAgent did not project the Rust-owned overlay target");
    }
    result.textContent = "canvas-agent-icon-descendant-click";
    document.title = "AGENT_ICON_WAIT";
    const iconClick = await waitForCanvasAgentMessage((data) =>
      data?.type === "gesture"
        && data.gesture === "click"
        && data.gestureSequence > agentClick.gestureSequence
    );
    if (
      iconClick.hitPath?.[0]?.kind !== "renderInstance"
      || iconClick.hitPath?.[0]?.id !== "render-icon"
      || iconClick.hitPath?.some((entry) => entry.id !== "render-icon")
    ) {
      throw new Error("SVG descendant click did not resolve the atomic Icon root");
    }
    result.textContent = "canvas-agent-native-drag";
    document.title = "AGENT_DRAG_WAIT";
    const dragStart = await waitForCanvasAgentMessage((data) =>
      data?.type === "gesture" && data.gesture === "dragStart"
    );
    const dragOver = await waitForCanvasAgentMessage((data) =>
      data?.type === "gesture"
        && data.gesture === "dragOver"
        && data.gestureSequence > dragStart.gestureSequence
        && data.hitPath?.[0]?.id === "render-nav"
    );
    if (
      dragStart.hitPath?.[0]?.id !== "render-title"
      || !dragStart.drag?.sessionId
      || dragStart.drag.position !== null
      || dragOver.drag?.sessionId !== dragStart.drag.sessionId
      || !["before", "after", "inside"].includes(dragOver.drag?.position)
      || dragOver.hitPath?.[0]?.id !== "render-nav"
    ) {
      throw new Error("trusted CanvasAgent drag contract mismatch");
    }
    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "render-canvas-interaction-overlay",
      agentInstanceId: agentReady.agentInstanceId,
      documentEpoch: 1,
      channel: "drag",
      targetKind: "htmlElement",
      editorNodeId: "editor_render:render-nav",
      gestureSequence: dragOver.gestureSequence,
      dragSessionId: dragOver.drag.sessionId,
      dragPosition: dragOver.drag.position,
      dragPermission: {
        state: "pending"
      },
      projection: {
        primaryRenderInstanceId: "render-nav",
        renderInstanceIds: ["render-nav"],
        boundaryInstanceId: null
      }
    }, "*");
    await new Promise((resolve) => frame.contentWindow.requestAnimationFrame(() =>
      frame.contentWindow.requestAnimationFrame(resolve)
    ));
    const dragIndicator = frame.contentDocument.getElementById(
      "pana-studio-canvas-agent-drag"
    );
    const dragAxis = dragIndicator?.getAttribute("data-pana-drag-axis");
    const navRect = frame.contentDocument.getElementById("nav-probe")?.getBoundingClientRect();
    const indicatorRect = dragIndicator?.getBoundingClientRect();
    if (
      !dragIndicator
      || dragIndicator.style.display !== "block"
      || dragIndicator.getAttribute("data-pana-drag-position") !== dragOver.drag.position
      || dragIndicator.getAttribute("data-pana-drag-permission") !== "pending"
      || !navRect
      || !indicatorRect
    ) {
      throw new Error("CanvasAgent did not render the Rust-projected drag indicator");
    }
    if (dragOver.drag.position === "inside") {
      if (
        Math.abs(indicatorRect.left - navRect.left) > 2
        || Math.abs(indicatorRect.top - navRect.top) > 2
        || Math.abs(indicatorRect.width - navRect.width) > 2
        || Math.abs(indicatorRect.height - navRect.height) > 2
      ) {
        throw new Error("inside drag indicator geometry mismatch");
      }
    } else if (dragAxis === "horizontal") {
      const expectedX = dragOver.drag.position === "before" ? navRect.left : navRect.right;
      if (Math.abs(indicatorRect.left + 1 - expectedX) > 2 || indicatorRect.width !== 3) {
        throw new Error("horizontal drag edge indicator geometry mismatch");
      }
    } else {
      const expectedY = dragOver.drag.position === "before" ? navRect.top : navRect.bottom;
      if (Math.abs(indicatorRect.top + 1 - expectedY) > 2 || indicatorRect.height !== 3) {
        throw new Error("vertical drag edge indicator geometry mismatch");
      }
    }
    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "render-canvas-interaction-overlay",
      agentInstanceId: agentReady.agentInstanceId,
      documentEpoch: 1,
      channel: "drag",
      targetKind: "htmlElement",
      editorNodeId: "editor_render:render-nav",
      gestureSequence: dragOver.gestureSequence,
      dragSessionId: dragOver.drag.sessionId,
      dragPosition: dragOver.drag.position,
      dragPermission: {
        state: "blocked"
      },
      projection: {
        primaryRenderInstanceId: "render-nav",
        renderInstanceIds: ["render-nav"],
        boundaryInstanceId: null
      }
    }, "*");
    await new Promise((resolve) => frame.contentWindow.requestAnimationFrame(() =>
      frame.contentWindow.requestAnimationFrame(resolve)
    ));
    const blockedIndicatorColor = dragOver.drag.position === "inside"
      ? dragIndicator.style.borderColor
      : dragIndicator.style.background;
    if (
      dragIndicator.getAttribute("data-pana-drag-permission") !== "blocked"
      || blockedIndicatorColor !== "rgb(220, 38, 38)"
    ) {
      throw new Error("CanvasAgent did not project the blocked Rust move verdict");
    }
    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "render-canvas-interaction-overlay",
      agentInstanceId: agentReady.agentInstanceId,
      documentEpoch: 1,
      channel: "drag",
      targetKind: "htmlElement",
      editorNodeId: "editor_render:render-nav",
      gestureSequence: dragOver.gestureSequence,
      dragSessionId: dragOver.drag.sessionId,
      dragPosition: dragOver.drag.position,
      dragPermission: {
        state: "allowed"
      },
      projection: {
        primaryRenderInstanceId: "render-nav",
        renderInstanceIds: ["render-nav"],
        boundaryInstanceId: null
      }
    }, "*");
    await new Promise((resolve) => frame.contentWindow.requestAnimationFrame(() =>
      frame.contentWindow.requestAnimationFrame(resolve)
    ));
    const allowedIndicatorColor = dragOver.drag.position === "inside"
      ? dragIndicator.style.borderColor
      : dragIndicator.style.background;
    if (
      dragIndicator.getAttribute("data-pana-drag-permission") !== "allowed"
      || allowedIndicatorColor !== "rgb(21, 128, 61)"
    ) {
      throw new Error("CanvasAgent did not project the allowed Rust move verdict");
    }
    const projectDragPreview = (gestureSequence, position) => frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "project-canvas-drag-preview",
      agentInstanceId: agentReady.agentInstanceId,
      documentEpoch: 1,
      dragSessionId: dragOver.drag.sessionId,
      gestureSequence,
      inputEmittedAtMs: Date.now(),
      projection: {
        schemaVersion: 1,
        operation: "move",
        scope: "selectedInstance",
        planToken: "rust-plan-browser-real",
        identity: {
          projectRoot: "/project",
          runtimeSessionId: "runtime-browser-real",
          workspaceRevision: 1,
          transactionId: "canvas-browser-real",
          previewRevision: "preview-browser-real"
        },
        sourceRenderInstanceId: "render-title",
        targetRenderInstanceId: "render-nav",
        position,
        rollback: {
          sourceParentRenderInstanceId: null,
          sourceNextSiblingRenderInstanceId: "render-nav"
        }
      }
    }, "*");
    const assertDragPreviewPosition = (position) => {
      const source = frame.contentDocument.getElementById("probe");
      const target = frame.contentDocument.getElementById("nav-probe");
      if (!source || !target || source.style.pointerEvents !== "none") {
        throw new Error("CanvasAgent did not install the reversible drag projection");
      }
      if (position === "inside" && source.parentElement !== target) {
        throw new Error("inside drag projection did not move the source into the target");
      }
      if (
        position === "before"
        && source.nextElementSibling !== target
      ) {
        throw new Error("before drag projection has the wrong DOM order");
      }
      if (
        position === "after"
        && target.nextElementSibling !== source
      ) {
        throw new Error("after drag projection has the wrong DOM order");
      }
    };
    projectDragPreview(dragOver.gestureSequence, dragOver.drag.position);
    await new Promise((resolve) => frame.contentWindow.requestAnimationFrame(resolve));
    const unchangedDuringDrag = frame.contentDocument.getElementById("probe")?.parentElement;
    if (
      unchangedDuringDrag?.children[0]?.id !== "probe"
      || unchangedDuringDrag?.children[1]?.id !== "nav-probe"
      || frame.contentDocument.getElementById("probe")?.style.pointerEvents
    ) {
      throw new Error("CanvasAgent moved the DOM before the trusted Drop");
    }
    result.textContent = "canvas-agent-native-drop";
    document.title = "AGENT_DROP_WAIT";
    const drop = await waitForCanvasAgentMessage((data) =>
      data?.type === "gesture"
        && data.gesture === "drop"
        && data.gestureSequence > dragOver.gestureSequence
    );
    if (
      drop.drag?.sessionId !== dragStart.drag.sessionId
      || !["before", "after", "inside"].includes(drop.drag?.position)
      || drop.hitPath?.[0]?.id !== "render-nav"
    ) {
      throw new Error("trusted CanvasAgent drop contract mismatch");
    }
    await new Promise((resolve) => frame.contentWindow.requestAnimationFrame(resolve));
    if (dragIndicator.style.display !== "none") {
      throw new Error("CanvasAgent drag indicator survived pointer release");
    }
    const unchangedAtDrop = frame.contentDocument.getElementById("probe")?.parentElement;
    if (
      unchangedAtDrop?.children[0]?.id !== "probe"
      || unchangedAtDrop?.children[1]?.id !== "nav-probe"
      || frame.contentDocument.getElementById("probe")?.style.pointerEvents
    ) {
      throw new Error("CanvasAgent moved the DOM before the Drop projection arrived");
    }
    const dragPreviewStartedAt = performance.now();
    const dragPreviewAppliedPromise = waitForCanvasAgentMessage((data) =>
      data?.type === "dragPreviewApplied"
        && data.planToken === "rust-plan-browser-real"
        && data.gestureSequence === drop.gestureSequence
    );
    projectDragPreview(drop.gestureSequence, drop.drag.position);
    const dragPreviewApplied = await dragPreviewAppliedPromise;
    await new Promise((resolve) => frame.contentWindow.requestAnimationFrame(resolve));
    const dragPreviewRoundTripMs = Math.max(0, performance.now() - dragPreviewStartedAt);
    assertDragPreviewPosition(drop.drag.position);
    if (
      !Number.isSafeInteger(dragPreviewApplied.dragPreviewAppliedMs)
      || dragPreviewApplied.dragPreviewAppliedMs < 0
      || dragPreviewApplied.dragPreviewAppliedMs > 50
    ) {
      throw new Error("typed Drop projection missed the 50 ms DOM budget");
    }
    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "cancel-canvas-drag-preview",
      agentInstanceId: agentReady.agentInstanceId,
      documentEpoch: 1,
      dragSessionId: dragOver.drag.sessionId
    }, "*");
    await new Promise((resolve) => frame.contentWindow.requestAnimationFrame(resolve));
    const rolledBackParent = frame.contentDocument.getElementById("probe")?.parentElement;
    if (
      rolledBackParent?.children[0]?.id !== "probe"
      || rolledBackParent?.children[1]?.id !== "nav-probe"
    ) {
      throw new Error("CanvasAgent did not rollback the failed Drop projection exactly");
    }
    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "render-canvas-interaction-overlay",
      agentInstanceId: agentReady.agentInstanceId,
      documentEpoch: 1,
      channel: "selection",
      targetKind: "teraBoundary",
      editorNodeId: "editor_boundary:hero",
      gestureSequence: drop.gestureSequence,
      selectionRevision: 42,
      actions: { canEnterBoundary: true },
      projection: {
        primaryRenderInstanceId: "render-nav",
        renderInstanceIds: ["render-nav"],
        boundaryInstanceId: "boundary-hero"
      }
    }, "*");
    result.textContent = "canvas-agent-native-action";
    document.title = "AGENT_ACTION_WAIT";
    const boundaryAction = await waitForCanvasAgentMessage((data) =>
      data?.type === "action" && data.action === "enterBoundary"
    );
    if (
      boundaryAction.selectionRevision !== 42
      || boundaryAction.editorNodeId !== "editor_boundary:hero"
      || boundaryAction.actionSequence <= drop.gestureSequence
    ) {
      throw new Error("CanvasAgent boundary action was not tied to the Rust projection");
    }
    document.title = "RUNNING";
    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "set-application-appearance",
      accent: "#c2410c",
      textOnAccent: "#ffffff"
    }, "*");
    for (let attempt = 0; attempt < 10; attempt += 1) {
      await new Promise((resolve) => frame.contentWindow.requestAnimationFrame(resolve));
      const style = frame.contentDocument.documentElement.style;
      if (
        style.getPropertyValue("--pana-studio-accent") === "#c2410c"
        && style.getPropertyValue("--pana-studio-text-on-accent") === "#ffffff"
      ) break;
    }
    const previewRootStyle = frame.contentDocument.documentElement.style;
    if (
      previewRootStyle.getPropertyValue("--pana-studio-accent") !== "#c2410c"
      || previewRootStyle.getPropertyValue("--pana-studio-text-on-accent") !== "#ffffff"
    ) {
      throw new Error("application appearance was not propagated to the preview bridge");
    }
    const persistentDocument = frame.contentDocument;
    frame.contentWindow.requestAnimationFrame(sampleColor);

    async function applyMeasuredPatch(baseRevision, operationRevision, measured) {
      const workspaceRevision = baseRevision + 1;
      const patch = {
        schemaVersion: 1,
        patchId: "canvas_patch_" + workspaceRevision.toString(16).padStart(64, "0"),
        projectRoot: "/project",
        runtimeSessionId: "runtime-browser-real",
        baseWorkspaceRevision: baseRevision,
        workspaceRevision,
        workspaceTransactionId: "workspace-browser-real-" + workspaceRevision,
        issuedAtMs: Date.now(),
        operation: {
          kind: "setText",
          target: {
            sourceId: "source-title",
            renderInstanceId: null,
            expectedTag: "h1"
          },
          text: workspaceRevision === 106 ? "After" : "After " + workspaceRevision
        }
      };
      const startedAt = performance.now();
      frame.contentWindow.postMessage({
        source: "pana-studio-app",
        type: "apply-canvas-patch",
        previewRevision: operationRevision,
        patch
      }, "*");
      const patchAck = await waitForMessage((data) =>
        data?.type === "preview-operation-complete"
          && data.operation === "apply-canvas-patch"
          && data.previewRevision === operationRevision
      );
      if (!patchAck.ok || patchAck.canvasPatchReceipt?.patchId !== patch.patchId) {
        throw new Error(patchAck.error || "CanvasPatch receipt mismatch");
      }
      if (measured) {
        patchRoundTrips.push(Math.max(0, performance.now() - startedAt));
        patchBridgeDurations.push(patchAck.canvasPatchReceipt.bridgeCommitDurationMs);
      }
      return patchAck;
    }

    let patchAck = null;
    for (let index = 0; index < 105; index += 1) {
      if (index % 10 === 0) result.textContent = "canvas-patch-" + index;
      patchAck = await applyMeasuredPatch(index + 1, index + 1, index >= 5);
    }
    result.textContent = "canvas-patch-complete";
    if (frame.contentDocument.getElementById("probe")?.textContent !== "After") {
      throw new Error("CanvasPatch series did not update the real DOM");
    }

    const hrefPatch = {
      schemaVersion: 1,
      patchId: "canvas_patch_" + "f".repeat(64),
      projectRoot: "/project",
      runtimeSessionId: "runtime-browser-real",
      baseWorkspaceRevision: 106,
      workspaceRevision: 107,
      workspaceTransactionId: "workspace-browser-real-107",
      issuedAtMs: Date.now(),
      operation: {
        kind: "setAttributes",
        target: {
          sourceId: "source-nav",
          renderInstanceId: null,
          expectedTag: "a"
        },
        attributes: { href: "/despre" }
      }
    };
    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "apply-canvas-patch",
      previewRevision: 850,
      patch: hrefPatch
    }, "*");
    const hrefPatchAck = await waitForMessage((data) =>
      data?.type === "preview-operation-complete"
        && data.operation === "apply-canvas-patch"
        && data.previewRevision === 850
    );
    if (!hrefPatchAck.ok || frame.contentDocument.getElementById("nav-probe")?.getAttribute("href") !== "/despre") {
      throw new Error(hrefPatchAck.error || "safe relative href CanvasPatch was refused");
    }

    async function applyHistoryPatch(previewRevision, patch) {
      const startedAt = performance.now();
      frame.contentWindow.postMessage({
        source: "pana-studio-app",
        type: "apply-canvas-patch",
        previewRevision,
        patch
      }, "*");
      const ack = await waitForMessage((data) =>
        data?.type === "preview-operation-complete"
          && data.operation === "apply-canvas-patch"
          && data.previewRevision === previewRevision
      );
      if (!ack.ok) throw new Error(ack.error || "History CanvasPatch was refused");
      historyPatchRoundTrips.push(Math.max(0, performance.now() - startedAt));
      historyPatchBridgeDurations.push(
        ack.canvasPatchReceipt?.bridgeCommitDurationMs ?? Number.POSITIVE_INFINITY
      );
    }
    const iconTarget = {
      sourceId: "source-icon",
      renderInstanceId: "render-icon",
      expectedTag: "svg"
    };
    const iconManagedBase = {
      xmlns: "http://www.w3.org/2000/svg",
      viewBox: "0 0 24 24",
      fill: "none",
      stroke: "currentColor",
      "stroke-linecap": "round",
      "stroke-linejoin": "round",
      focusable: "false"
    };
    const iconPatchBase = {
      schemaVersion: 1,
      projectRoot: "/project",
      runtimeSessionId: "runtime-browser-real",
      issuedAtMs: Date.now(),
      beforeModelRevision: "icon-before",
      afterModelRevision: "icon-after"
    };
    const starChildren = '<path d="M 12 2 L 15 8 L 22 9 L 17 14 L 18 21 L 12 18 L 6 21 L 7 14 L 2 9 L 9 8 Z"></path>';
    const homeChildren = '<path d="M 3 3 H 21 V 21 H 3 Z" fill="currentColor"></path>';
    await applyHistoryPatch(855, {
      ...iconPatchBase,
      patchId: "canvas_patch_" + "d".repeat(64),
      baseWorkspaceRevision: 107,
      workspaceRevision: 108,
      workspaceTransactionId: "icon-forward-108",
      operation: {
        kind: "setIcon",
        target: iconTarget,
        providerId: "icon",
        iconIdentity: "tabler-outline:star",
        attributes: {
          ...iconManagedBase,
          "data-pana-icon": "tabler-outline:star",
          width: "32",
          height: "32",
          "stroke-width": "1.5",
          "aria-hidden": null,
          role: "img",
          "aria-label": "Favorite & More"
        },
        childrenHtml: starChildren
      }
    });
    let iconProbe = frame.contentDocument.getElementById("icon-probe");
    if (
      iconProbe?.getAttribute("data-pana-icon") !== "tabler-outline:star"
      || iconProbe.getAttribute("width") !== "32"
      || iconProbe.getAttribute("role") !== "img"
      || iconProbe.getAttribute("aria-label") !== "Favorite & More"
      || iconProbe.children.length !== 1
      || iconProbe.firstElementChild?.getAttribute("d") !== "M 12 2 L 15 8 L 22 9 L 17 14 L 18 21 L 12 18 L 6 21 L 7 14 L 2 9 L 9 8 Z"
      || !iconProbe.classList.contains("custom-icon")
      || iconProbe.style.color !== "rgb(194, 65, 12)"
      || iconProbe.getAttribute("data-pana-instance") !== "icon-browser-real"
    ) {
      throw new Error("atomic Icon CanvasPatch did not preserve the user-owned root contract");
    }
    await applyHistoryPatch(856, {
      ...iconPatchBase,
      patchId: "canvas_patch_" + "e".repeat(64),
      baseWorkspaceRevision: 108,
      workspaceRevision: 109,
      workspaceTransactionId: "icon-undo-109",
      operation: {
        kind: "setIcon",
        target: iconTarget,
        providerId: "icon",
        iconIdentity: "tabler-outline:home",
        attributes: {
          ...iconManagedBase,
          "data-pana-icon": "tabler-outline:home",
          width: "24",
          height: "24",
          "stroke-width": "2",
          "aria-hidden": "true",
          role: null,
          "aria-label": null
        },
        childrenHtml: homeChildren
      }
    });
    iconProbe = frame.contentDocument.getElementById("icon-probe");
    if (
      iconProbe?.getAttribute("data-pana-icon") !== "tabler-outline:home"
      || iconProbe.getAttribute("aria-hidden") !== "true"
      || iconProbe.hasAttribute("role")
      || iconProbe.hasAttribute("aria-label")
      || iconProbe.firstElementChild?.getAttribute("d") !== "M 3 3 H 21 V 21 H 3 Z"
    ) {
      throw new Error("inverse Icon CanvasPatch did not restore the exact managed state");
    }
    await applyHistoryPatch(857, {
      ...iconPatchBase,
      patchId: "canvas_patch_" + "1".repeat(64),
      baseWorkspaceRevision: 109,
      workspaceRevision: 110,
      workspaceTransactionId: "icon-redo-110",
      operation: {
        kind: "setIcon",
        target: iconTarget,
        providerId: "icon",
        iconIdentity: "tabler-outline:star",
        attributes: {
          ...iconManagedBase,
          "data-pana-icon": "tabler-outline:star",
          width: "32",
          height: "32",
          "stroke-width": "1.5",
          "aria-hidden": null,
          role: "img",
          "aria-label": "Favorite & More"
        },
        childrenHtml: starChildren
      }
    });
    const unsafeIconPatch = {
      ...iconPatchBase,
      patchId: "canvas_patch_" + "2".repeat(64),
      baseWorkspaceRevision: 110,
      workspaceRevision: 111,
      workspaceTransactionId: "icon-unsafe-111",
      operation: {
        kind: "setIcon",
        target: iconTarget,
        providerId: "icon",
        iconIdentity: "tabler-outline:star",
        attributes: {
          ...iconManagedBase,
          "data-pana-icon": "tabler-outline:star",
          width: "32",
          height: "32",
          "stroke-width": "1.5",
          "aria-hidden": "true",
          role: null,
          "aria-label": null
        },
        childrenHtml: '<script>alert(1)<\\/script>'
      }
    };
    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "apply-canvas-patch",
      previewRevision: 858,
      patch: unsafeIconPatch
    }, "*");
    const unsafeIconAck = await waitForMessage((data) =>
      data?.type === "preview-operation-complete"
        && data.operation === "apply-canvas-patch"
        && data.previewRevision === 858
    );
    iconProbe = frame.contentDocument.getElementById("icon-probe");
    if (
      unsafeIconAck.ok
      || iconProbe?.getAttribute("data-pana-icon") !== "tabler-outline:star"
      || iconProbe.querySelector("script")
    ) {
      throw new Error("arbitrary Icon SVG geometry was not rejected fail-closed");
    }
    const historyTarget = {
      sourceId: "source-nav",
      renderInstanceId: null,
      expectedTag: "a"
    };
    const historyInserted = {
      sourceId: "source-history-icon",
      renderInstanceId: "render-history-icon",
      expectedTag: "svg"
    };
    const insertedIconHtml = '<svg class="icon ps-icon-history" data-anim="ps-icon-history" data-pana-block="icon" data-pana-instance="icon-ps-icon-history" data-pana-icon="tabler-outline:home" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false"><path d="M 3 3 H 21 V 21 H 3 Z" fill="currentColor"></path></svg>';
    const historyPatchBase = {
      schemaVersion: 1,
      projectRoot: "/project",
      runtimeSessionId: "runtime-browser-real",
      issuedAtMs: Date.now(),
      beforeModelRevision: "history-before",
      afterModelRevision: "history-after"
    };
    await applyHistoryPatch(860, {
      ...historyPatchBase,
      patchId: "canvas_patch_" + "a".repeat(64),
      baseWorkspaceRevision: 110,
      workspaceRevision: 111,
      workspaceTransactionId: "history-forward-111",
      operation: {
        kind: "insert",
        target: historyTarget,
        position: "after",
        html: insertedIconHtml,
        inserted: historyInserted
      }
    });
    const insertedHistoryIcon = frame.contentDocument.querySelector('[data-pana-source-id="source-history-icon"]');
    if (
      !insertedHistoryIcon
      || insertedHistoryIcon.getAttribute("data-pana-icon") !== "tabler-outline:home"
      || insertedHistoryIcon.getAttribute("data-pana-render-instance-id") !== "render-history-icon"
      || insertedHistoryIcon.children.length !== 1
    ) {
      throw new Error("forward History patch did not publish the inserted atomic Icon identity");
    }
    await applyHistoryPatch(861, {
      ...historyPatchBase,
      patchId: "canvas_patch_" + "b".repeat(64),
      baseWorkspaceRevision: 111,
      workspaceRevision: 112,
      workspaceTransactionId: "history-undo-112",
      operation: { kind: "delete", target: historyInserted }
    });
    if (frame.contentDocument.querySelector('[data-pana-source-id="source-history-icon"]')) {
      throw new Error("inverse History patch did not remove the inserted Icon");
    }
    await applyHistoryPatch(862, {
      ...historyPatchBase,
      patchId: "canvas_patch_" + "c".repeat(64),
      baseWorkspaceRevision: 112,
      workspaceRevision: 113,
      workspaceTransactionId: "history-redo-113",
      operation: {
        kind: "insert",
        target: historyTarget,
        position: "after",
        html: insertedIconHtml,
        inserted: historyInserted
      }
    });
    if (frame.contentDocument.querySelectorAll('[data-pana-source-id="source-history-icon"]').length !== 1) {
      throw new Error("rapid History redo lost the inserted Rust Icon identity");
    }

    const batchPatchBase = {
      schemaVersion: 1,
      projectRoot: "/project",
      runtimeSessionId: "runtime-browser-real",
      issuedAtMs: Date.now(),
      beforeModelRevision: "batch-before",
      afterModelRevision: "batch-after"
    };
    await applyHistoryPatch(863, {
      ...batchPatchBase,
      patchId: "canvas_patch_" + "3".repeat(64),
      baseWorkspaceRevision: 113,
      workspaceRevision: 114,
      workspaceTransactionId: "batch-forward-114",
      operation: {
        kind: "batch",
        operations: [
          {
            kind: "setAttributes",
            target: { sourceId: "source-title", renderInstanceId: null, expectedTag: "h1" },
            attributes: { "data-batch-proof": "title" }
          },
          {
            kind: "setAttributes",
            target: { sourceId: "source-nav", renderInstanceId: null, expectedTag: "a" },
            attributes: { "data-batch-proof": "nav" }
          }
        ]
      }
    });
    if (
      frame.contentDocument.getElementById("probe")?.getAttribute("data-batch-proof") !== "title"
      || frame.contentDocument.getElementById("nav-probe")?.getAttribute("data-batch-proof") !== "nav"
    ) {
      throw new Error("CanvasPatch batch did not commit every operation atomically");
    }

    const rejectedBatchPatch = {
      ...batchPatchBase,
      patchId: "canvas_patch_" + "4".repeat(64),
      baseWorkspaceRevision: 114,
      workspaceRevision: 115,
      workspaceTransactionId: "batch-rejected-115",
      operation: {
        kind: "batch",
        operations: [
          {
            kind: "setAttributes",
            target: { sourceId: "source-title", renderInstanceId: null, expectedTag: "h1" },
            attributes: { "data-batch-rollback": "must-disappear" }
          },
          {
            kind: "setAttributes",
            target: { sourceId: "source-does-not-exist", renderInstanceId: null, expectedTag: "div" },
            attributes: { "data-batch-rollback": "unreachable" }
          }
        ]
      }
    };
    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "apply-canvas-patch",
      previewRevision: 864,
      patch: rejectedBatchPatch
    }, "*");
    const rejectedBatchAck = await waitForMessage((data) =>
      data?.type === "preview-operation-complete"
        && data.operation === "apply-canvas-patch"
        && data.previewRevision === 864
    );
    if (
      rejectedBatchAck.ok
      || frame.contentDocument.getElementById("probe")?.hasAttribute("data-batch-rollback")
      || frame.contentDocument.documentElement.getAttribute("data-pana-canvas-workspace-revision") !== "114"
    ) {
      throw new Error("CanvasPatch batch failure did not rollback the complete transaction");
    }

    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "apply-live-attribute-draft",
      previewRevision: 851,
      editSessionId: "unsafe_href_browser_real",
      draftEpoch: 1,
      target: {
        sourceId: "source-nav",
        renderInstanceId: "render-nav",
        expectedTag: "a"
      },
      attributes: { href: "javascript:alert(1)" },
      baselineNames: ["href"]
    }, "*");
    const unsafeHrefAck = await waitForMessage((data) =>
      data?.type === "preview-operation-complete"
        && data.operation === "apply-live-attribute-draft"
        && data.previewRevision === 851
    );
    if (unsafeHrefAck.ok || frame.contentDocument.getElementById("nav-probe")?.getAttribute("href") !== "/despre") {
      throw new Error("active script href was not rejected fail-closed");
    }

    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "apply-live-text-draft",
      previewRevision: 900,
      editSessionId: "text_browser_real_1",
      target: {
        sourceId: "source-title",
        renderInstanceId: "render-title",
        expectedTag: "h1"
      },
      text: "Live draft"
    }, "*");
    const liveDraftAck = await waitForMessage((data) =>
      data?.type === "preview-operation-complete"
        && data.operation === "apply-live-text-draft"
        && data.previewRevision === 900
    );
    if (!liveDraftAck.ok || frame.contentDocument.getElementById("probe")?.textContent !== "Live draft") {
      throw new Error(liveDraftAck.error || "live text draft did not update the real DOM");
    }

    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "apply-live-attribute-draft",
      previewRevision: 901,
      editSessionId: "attr_browser_real_1",
      draftEpoch: 1,
      target: {
        sourceId: "source-title",
        renderInstanceId: "render-title",
        expectedTag: "h1"
      },
      attributes: { title: "Draft title" },
      baselineNames: ["title"]
    }, "*");
    const liveAttributeAck = await waitForMessage((data) =>
      data?.type === "preview-operation-complete"
        && data.operation === "apply-live-attribute-draft"
        && data.previewRevision === 901
    );
    if (!liveAttributeAck.ok || frame.contentDocument.getElementById("probe")?.title !== "Draft title") {
      throw new Error(liveAttributeAck.error || "live attribute draft did not update the real DOM");
    }

    const stablePreload = frame.contentDocument.querySelector("link[rel~='preload']");
    const stableDescription = frame.contentDocument.querySelector("meta[name='description']");
    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "replace-document",
      previewRevision: 1000,
      html: canonicalDocument,
      liveCss: "",
      canvasIdentity: identity
    }, "*");
    const canonicalAck = await waitForMessage((data) =>
      data?.type === "preview-operation-complete"
        && data.operation === "replace-document"
        && data.previewRevision === 1000
    );
    if (!canonicalAck.ok) throw new Error(canonicalAck.error || "canonical reconcile failed");
    if (
      !stablePreload
      || frame.contentDocument.querySelector("link[rel~='preload']") !== stablePreload
      || !stableDescription
      || frame.contentDocument.querySelector("meta[name='description']") !== stableDescription
      || stableDescription.getAttribute("content") !== "After"
    ) {
      throw new Error("semantic head reconciliation replaced a stable preload/meta node");
    }
    const phases = canonicalAck.canvasPhaseReceipts?.map((entry) => entry.phase) ?? [];
    if (phases.join(",") !== "resourcesReady,committed,styledReady") {
      throw new Error("canonical phase sequence mismatch: " + phases.join(","));
    }
    if (
      canonicalAck.stylesheetPromotion?.mode !== "in_place"
      || canonicalAck.stylesheetPromotion.staged !== 1
      || canonicalAck.stylesheetPromotion.retired !== 1
      || canonicalAck.stylesheetPromotion.reused !== 0
      || canonicalAck.stylesheetPromotion.preloadsReused !== 1
      || canonicalAck.stylesheetPromotion.preloadsStaged !== 0
      || canonicalAck.stylesheetPromotion.preloadAttributeMutations !== 0
      || canonicalAck.stylesheetPromotion.fontFallbackFrames !== 0
    ) {
      throw new Error(
        "changed stylesheet was not promoted atomically: "
          + JSON.stringify(canonicalAck.stylesheetPromotion)
      );
    }
    const reconciledRootStyle = frame.contentDocument.documentElement.style;
    if (
      reconciledRootStyle.getPropertyValue("--pana-studio-accent") !== "#c2410c"
      || reconciledRootStyle.getPropertyValue("--pana-studio-text-on-accent") !== "#ffffff"
    ) {
      throw new Error("canonical reconcile lost the cached application appearance");
    }
    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "render-canvas-interaction-overlay",
      agentInstanceId: agentReady.agentInstanceId,
      documentEpoch: 1,
      channel: "selection",
      targetKind: "htmlElement",
      editorNodeId: "editor_render:render-title",
      gestureSequence: agentClick.gestureSequence,
      selectionRevision: 43,
      actions: { canEnterBoundary: false },
      projection: {
        primaryRenderInstanceId: "render-title",
        renderInstanceIds: ["render-title"],
        boundaryInstanceId: null
      }
    }, "*");
    await new Promise((resolve) => frame.contentWindow.requestAnimationFrame(() =>
      frame.contentWindow.requestAnimationFrame(resolve)
    ));
    const reconciledSelectionOverlay = frame.contentDocument.getElementById(
      "pana-studio-canvas-agent-selection"
    );
    const reconciledSelectionBorder = reconciledSelectionOverlay
      ? frame.contentWindow.getComputedStyle(reconciledSelectionOverlay).borderColor
      : "";
    if (
      !reconciledSelectionOverlay
      || reconciledSelectionOverlay.style.display !== "block"
      || reconciledSelectionBorder !== "rgb(194, 65, 12)"
    ) {
      throw new Error(
        "canonical reconcile did not preserve the application accent on the selection outline: "
          + reconciledSelectionBorder
      );
    }
    if (frame.contentDocument.getElementById("probe")?.textContent !== "Live draft") {
      throw new Error("canonical reconcile clobbered the active live text draft");
    }
    if (frame.contentDocument.getElementById("probe")?.title !== "Draft title") {
      throw new Error("canonical reconcile clobbered the active live attribute draft");
    }
    const fontResourceUrl = new URL("/font-probe.woff2", frame.contentDocument.baseURI).href;
    const fontEntriesBeforeStableReconcile = frame.contentWindow.performance
      .getEntriesByName(fontResourceUrl)
      .length;

    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "clear-live-text-draft",
      previewRevision: 1001,
      editSessionId: "text_browser_real_1"
    }, "*");
    const clearDraftAck = await waitForMessage((data) =>
      data?.type === "preview-operation-complete"
        && data.operation === "clear-live-text-draft"
        && data.previewRevision === 1001
    );
    if (!clearDraftAck.ok) throw new Error(clearDraftAck.error || "live text draft did not close");

    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "clear-live-attribute-draft",
      previewRevision: 10015,
      editSessionId: "attr_browser_real_1",
      draftEpoch: 1
    }, "*");
    const clearAttributeDraftAck = await waitForMessage((data) =>
      data?.type === "preview-operation-complete"
        && data.operation === "clear-live-attribute-draft"
        && data.previewRevision === 10015
    );
    if (!clearAttributeDraftAck.ok) {
      throw new Error(clearAttributeDraftAck.error || "live attribute draft did not close");
    }

    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "replace-document",
      previewRevision: 1002,
      html: canonicalDocument,
      liveCss: "",
      canvasIdentity: identity
    }, "*");
    const settledCanonicalAck = await waitForMessage((data) =>
      data?.type === "preview-operation-complete"
        && data.operation === "replace-document"
        && data.previewRevision === 1002
    );
    if (!settledCanonicalAck.ok) {
      throw new Error(settledCanonicalAck.error || "settled canonical reconcile failed");
    }
    if (
      frame.contentDocument.querySelector("link[rel~='preload']") !== stablePreload
      || frame.contentDocument.querySelector("meta[name='description']") !== stableDescription
    ) {
      throw new Error("stable head node identity changed on an identical reconcile");
    }
    if (
      frame.contentWindow.performance.getEntriesByName(fontResourceUrl).length
      !== fontEntriesBeforeStableReconcile
    ) {
      throw new Error("unchanged reconcile reloaded the custom font resource");
    }
    if (
      settledCanonicalAck.stylesheetPromotion?.reused !== 1
      || settledCanonicalAck.stylesheetPromotion.staged !== 0
      || settledCanonicalAck.stylesheetPromotion.retired !== 0
      || settledCanonicalAck.stylesheetPromotion.preloadsReused !== 1
      || settledCanonicalAck.stylesheetPromotion.stylesheetAttributeMutations !== 0
      || settledCanonicalAck.stylesheetPromotion.preloadAttributeMutations !== 0
      || settledCanonicalAck.stylesheetPromotion.headNodesCreated !== 0
      || settledCanonicalAck.stylesheetPromotion.headNodesRetired !== 0
      || settledCanonicalAck.stylesheetPromotion.headNodesReordered !== 0
      || settledCanonicalAck.stylesheetPromotion.fontInvalidationCount !== 0
      || settledCanonicalAck.stylesheetPromotion.fontFallbackFrames !== 0
      || settledCanonicalAck.stylesheetPromotion.maxTextMetricDelta !== 0
    ) {
      throw new Error(
        "unchanged stylesheet was not reused: "
          + JSON.stringify(settledCanonicalAck.stylesheetPromotion)
      );
    }
    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "replace-document",
      previewRevision: 10025,
      html: reorderedResourceDocument,
      liveCss: "",
      canvasIdentity: identity
    }, "*");
    const reorderedResourceAck = await waitForMessage((data) =>
      data?.type === "preview-operation-complete"
        && data.operation === "replace-document"
        && data.previewRevision === 10025
    );
    if (!reorderedResourceAck.ok) {
      throw new Error(reorderedResourceAck.error || "semantic resource reorder failed");
    }
    const reorderedResources = [...frame.contentDocument.head.querySelectorAll(
      "link[rel~='stylesheet'], link[rel~='preload']"
    )];
    if (
      reorderedResources.length !== 2
      || reorderedResources[0].rel !== "stylesheet"
      || reorderedResources[1].rel !== "preload"
      || reorderedResources[1] !== stablePreload
      || reorderedResourceAck.stylesheetPromotion?.headNodesReordered < 1
    ) {
      throw new Error(
        "a real canonical resource-order change was suppressed: "
          + JSON.stringify(reorderedResourceAck.stylesheetPromotion)
      );
    }
    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "replace-document",
      previewRevision: 10026,
      html: canonicalDocument,
      liveCss: "",
      canvasIdentity: identity
    }, "*");
    const restoredResourceOrderAck = await waitForMessage((data) =>
      data?.type === "preview-operation-complete"
        && data.operation === "replace-document"
        && data.previewRevision === 10026
    );
    if (
      !restoredResourceOrderAck.ok
      || frame.contentDocument.head.querySelector("link[rel~='preload']") !== stablePreload
      || frame.contentDocument.head.querySelector("link[rel~='stylesheet']")
        ?.previousElementSibling !== stablePreload
      || restoredResourceOrderAck.stylesheetPromotion?.headNodesReordered < 1
    ) {
      throw new Error(
        "canonical resource order was not restored: "
          + JSON.stringify(restoredResourceOrderAck.stylesheetPromotion)
      );
    }
    if (frame.contentDocument.getElementById("probe")?.textContent !== "After") {
      throw new Error("closed live text draft leaked into a later canonical reconcile");
    }
    if (frame.contentDocument.getElementById("probe")?.hasAttribute("title")) {
      throw new Error("closed live attribute draft leaked into a later canonical reconcile");
    }
    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "replace-document",
      previewRevision: 1003,
      html: missingFontDocument,
      liveCss: "",
      canvasIdentity: identity
    }, "*");
    const missingFontAck = await waitForMessage((data) =>
      data?.type === "preview-operation-complete"
        && data.operation === "replace-document"
        && data.previewRevision === 1003
    );
    const missingFontDiagnostic = missingFontAck.canvasPhaseReceipts?.at(-1)?.diagnostic || "";
    if (
      !missingFontAck.ok
      || missingFontAck.canvasPhaseReceipts?.at(-1)?.phase !== "styledReady"
      || missingFontAck.stylesheetPromotion?.fontActivationErrorCount < 1
      || !missingFontDiagnostic.includes("Pana Missing Probe")
      || frame.contentDocument.getElementById("probe")?.textContent !== "Fallback candidate"
    ) {
      throw new Error(
        "missing font did not settle as a usable fallback: " + JSON.stringify(missingFontAck)
      );
    }

    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "replace-document",
      previewRevision: 1004,
      html: canonicalDocument,
      liveCss: "",
      canvasIdentity: identity
    }, "*");
    const restoredAfterMissingFontAck = await waitForMessage((data) =>
      data?.type === "preview-operation-complete"
        && data.operation === "replace-document"
        && data.previewRevision === 1004
    );
    if (!restoredAfterMissingFontAck.ok) {
      throw new Error(restoredAfterMissingFontAck.error || "font fallback document did not restore");
    }

    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "replace-document",
      previewRevision: 1005,
      html: brokenStylesheetDocument,
      liveCss: "",
      canvasIdentity: identity
    }, "*");
    const brokenStylesheetAck = await waitForMessage((data) =>
        data?.type === "preview-operation-complete"
        && data.operation === "replace-document"
        && data.previewRevision === 1005
    );
    if (brokenStylesheetAck.ok) {
      throw new Error("broken stylesheet candidate was promoted");
    }
    if (frame.contentDocument.getElementById("probe")?.textContent !== "After") {
      throw new Error("failed stylesheet candidate changed the mounted document");
    }
    if (
      frame.contentDocument.querySelector("link[href*='broken.css']")
      || frame.contentDocument.querySelectorAll("link[rel~='stylesheet']").length !== 1
    ) {
      throw new Error("failed staged stylesheet was not cleaned up");
    }
    await new Promise((resolve) => frame.contentWindow.requestAnimationFrame(() =>
      frame.contentWindow.requestAnimationFrame(resolve)
    ));

    const finalColor = frame.contentWindow.getComputedStyle(
      frame.contentDocument.getElementById("probe")
    ).color;
    const allowed = new Set(["rgb(220, 20, 60)", "rgb(30, 100, 220)"]);
    const unstyledFrames = colors.filter((color) => !allowed.has(color));
    const sortedPatchDurations = [...patchRoundTrips].sort((left, right) => left - right);
    const sortedBridgeDurations = [...patchBridgeDurations].sort((left, right) => left - right);
    const p95Index = Math.max(0, Math.ceil(sortedPatchDurations.length * 0.95) - 1);
    const patchP95Ms = sortedPatchDurations[p95Index];
    const bridgeP95Ms = sortedBridgeDurations[p95Index];
    const historyPatchMaxMs = Math.max(...historyPatchRoundTrips);
    const historyBridgeMaxMs = Math.max(...historyPatchBridgeDurations);
    if (persistentDocument !== frame.contentDocument) throw new Error("same-route document navigated");
    if (finalColor !== "rgb(30, 100, 220)") throw new Error("final stylesheet mismatch: " + finalColor);
    if (unstyledFrames.length > 0) throw new Error("unstyled frame observed: " + unstyledFrames.join("|"));
    if (frame.contentDocument.querySelectorAll("link[rel~='stylesheet']").length !== 1) {
      throw new Error("obsolete stylesheet was not retired after styledReady");
    }
    if (!Number.isFinite(patchP95Ms) || patchP95Ms >= 50) {
      throw new Error("warmed CanvasPatch p95 exceeded 50 ms: " + patchP95Ms);
    }
    if (!Number.isFinite(historyPatchMaxMs) || historyPatchMaxMs >= 100) {
      throw new Error("History CanvasPatch exceeded 100 ms: " + historyPatchMaxMs);
    }
    if (!Number.isFinite(dragPreviewRoundTripMs) || dragPreviewRoundTripMs >= 50) {
      throw new Error("Drop projection exceeded 50 ms: " + dragPreviewRoundTripMs);
    }
    if (frame.contentDocument.querySelectorAll("script").length !== 1) {
      throw new Error("privileged bridge was replaced or duplicated");
    }

    result.textContent = "interactive-runtime";
    interactiveFrame.srcdoc = interactiveDocument;
    const interactiveReady = await waitForInteractiveMessage((data) => data?.type === "ready");
    const configReceipt = await waitForInteractiveMessage((data) => data?.type === "page-config-installed");
    const domSnapshot = await waitForInteractiveMessage((data) => data?.type === "dom-snapshot");
    if (interactiveReady.previewRevision !== "interactive-browser-real") {
      throw new Error("interactive ready revision mismatch");
    }
    if (
      configReceipt.blockCount !== 2
      || Object.keys(configReceipt).some((key) => key.startsWith("motion"))
    ) {
      throw new Error("block-only runtime receipt mismatch");
    }
    if (!domSnapshot.nodes?.some((node) => node.sourceId === "source-accordion")) {
      throw new Error("interactive read-only DOM snapshot lost source provenance");
    }
    const interactiveWindow = interactiveFrame.contentWindow;
    const interactiveDoc = interactiveFrame.contentDocument;
    const accordion = interactiveDoc.querySelector("[data-pana-block='accordion']");
    const triggers = [...interactiveDoc.querySelectorAll("[data-pana-accordion-trigger]")];
    const panels = [...interactiveDoc.querySelectorAll("[data-pana-accordion-panel]")];
    triggers[0].click();
    if (triggers[0].getAttribute("aria-expanded") !== "true" || panels[0].hidden) {
      throw new Error("interactive lifecycle mount did not handle the component");
    }
    triggers[1].click();
    if (triggers[0].getAttribute("aria-expanded") !== "false" || !panels[0].hidden
        || triggers[1].getAttribute("aria-expanded") !== "true" || panels[1].hidden) {
      throw new Error("accordion default contract did not enforce a single open item");
    }
    triggers[1].click();
    interactiveWindow.PanaBlockRuntime.reconcile(interactiveDoc);
    interactiveWindow.PanaBlockRuntime.reconcile(interactiveDoc);
    triggers[0].click();
    triggers[0].click();
    if (triggers[0].getAttribute("aria-expanded") !== "false" || !panels[0].hidden) {
      throw new Error("interactive lifecycle reconcile duplicated listeners");
    }
    accordion.setAttribute("data-multiple", "true");
    await new Promise((resolve) => setTimeout(resolve, 20));
    triggers[0].click();
    triggers[1].click();
    if (triggers.some((trigger) => trigger.getAttribute("aria-expanded") !== "true")
        || panels.some((panel) => panel.hidden)) {
      throw new Error("runtime option update did not remount the accordion contract");
    }

    const slider = interactiveDoc.querySelector("[data-pana-source-id='source-slider']");
    const sliderTrack = slider.querySelector("[data-pana-slider-track]");
    const sliderNext = slider.querySelector("[data-pana-slider-next]");
    const activeSlideIndex = (root) => [...root.querySelectorAll("[data-pana-slider-slide]")]
      .findIndex((slide) => !slide.hidden);
    if (sliderTrack.getAttribute("aria-live") !== "polite"
        || slider.querySelectorAll("[data-pana-slider-indicators] button").length !== 2) {
      throw new Error("slider default accessibility contract was not mounted");
    }
    sliderNext.click();
    if (activeSlideIndex(slider) !== 1) {
      throw new Error("slider next navigation did not activate exactly one slide");
    }
    slider.querySelectorAll("[data-pana-slider-slide]")[1].dispatchEvent(
      new interactiveWindow.KeyboardEvent("keydown", { key: "Home", bubbles: true }),
    );
    if (activeSlideIndex(slider) !== 1) {
      throw new Error("slider intercepted keyboard navigation from editable slide content");
    }
    slider.dispatchEvent(new interactiveWindow.KeyboardEvent("keydown", { key: "Home", bubbles: true }));
    if (activeSlideIndex(slider) !== 0) {
      throw new Error("slider keyboard navigation did not return to the first slide");
    }
    const insertedSlide = interactiveDoc.createElement("div");
    insertedSlide.setAttribute("data-pana-slider-slide", "");
    insertedSlide.setAttribute("data-pana-source-id", "source-slide-3");
    insertedSlide.textContent = "Slide 3";
    sliderTrack.appendChild(insertedSlide);
    await new Promise((resolve) => setTimeout(resolve, 30));
    if (slider.querySelectorAll("[data-pana-slider-indicators] button").length !== 3
        || !interactiveMessages.some((message) => message.type === "lifecycle"
          && message.blockId === "slider" && message.phase === "remount-structure")) {
      throw new Error("slider structural change did not remount the runtime instance");
    }
    sliderNext.click();
    if (activeSlideIndex(slider) !== 1) {
      throw new Error("slider structural remount duplicated navigation listeners");
    }

    const autoplaySlider = interactiveDoc.querySelector("[data-pana-source-id='source-slider-autoplay']");
    const autoplayTrack = autoplaySlider.querySelector("[data-pana-slider-track]");
    const autoplayNext = autoplaySlider.querySelector("[data-pana-slider-next]");
    const autoplayControl = autoplaySlider.querySelector("[data-pana-slider-autoplay]");
    if (autoplayControl.hidden || autoplayTrack.getAttribute("aria-live") !== "off") {
      throw new Error("configured autoplay did not expose Start/Stop with aria-live off");
    }
    autoplayNext.focus();
    await new Promise((resolve) => setTimeout(resolve, 1100));
    if (activeSlideIndex(autoplaySlider) !== 0
        || autoplayTrack.getAttribute("aria-live") !== "polite") {
      throw new Error("slider autoplay did not pause while focus remained inside");
    }
    interactiveDoc.body.tabIndex = -1;
    interactiveDoc.body.focus();
    await new Promise((resolve) => setTimeout(resolve, 30));
    autoplayControl.click();
    if (autoplayTrack.getAttribute("aria-live") !== "polite"
        || autoplayControl.textContent !== "Porneste") {
      throw new Error("explicit autoplay Stop did not pause rotation");
    }
    autoplayControl.click();
    interactiveWindow.__panaReducedMotion = true;
    interactiveWindow.__panaMotionListeners.forEach((listener) => listener({ matches: true }));
    if (autoplayTrack.getAttribute("aria-live") !== "polite") {
      throw new Error("prefers-reduced-motion did not suppress slider autoplay");
    }
    interactiveDoc.dispatchEvent(new interactiveWindow.CustomEvent("pana:blocks:dispose", {
      detail: { root: interactiveDoc }
    }));
    triggers[0].click();
    if (triggers[0].getAttribute("aria-expanded") !== "true") {
      throw new Error("interactive lifecycle dispose leaked a listener");
    }
    if (interactiveWindow.__panaMotionV2Config !== undefined) {
      throw new Error("block runtime leaked a private Motion configuration bridge");
    }
    if (interactiveMessages.some((message) => message.type === "lifecycle-error")) {
      throw new Error("interactive lifecycle emitted an error");
    }
    finish(true, {
      samples: colors.length,
      colors: [...new Set(colors)],
      phases,
      stylesheetPromotion: canonicalAck.stylesheetPromotion,
      stylesheetReuse: settledCanonicalAck.stylesheetPromotion,
      stylesheetRollback: "last-styled-document-preserved",
      missingFontFallback: "styledReady-with-diagnostic",
      patchSamples: patchRoundTrips.length,
      patchP95Ms,
      selectionOverlayMembers: 100,
      selectionOverlayP95Ms: overlayP95Ms,
      bridgeP95Ms,
      historyPatchMaxMs,
      historyBridgeMaxMs,
      dragPreviewRoundTripMs,
      lastPatchBridgeMs: patchAck.canvasPatchReceipt.bridgeCommitDurationMs,
      sameDocument: true,
      interactiveNodes: domSnapshot.nodes.length,
      interactiveLifecycle: "mount/reconcile/dispose",
      blockProviders: configReceipt.blockCount,
      canvasAgentHover: "trusted-hover",
      canvasAgentGesture: "trusted-click",
      canvasAgentIconDescendant: "atomic-root",
      canvasAgentDrag: "trusted-drag",
      canvasAgentDragIndicator: "rust-projected",
      canvasAgentBoundaryAction: "rust-projected",
      canvasAgentInspection: "physical-only",
      historyCanvasPatch: "forward/inverse/redo",
      iconCanvasPatch: "forward/inverse/redo/fail-closed",
    });
  }

  run().catch((error) => finish(false, {
    error: String(error?.message || error) + "\\n" + String(error?.stack || ""),
    stage: result.textContent,
    childDiagnostics,
    previewMessageTypes: messages.map((message) => message.type),
    canvasAgentMessageTypes: canvasAgentMessages.map((message) => message.type)
  }));
})();
<\/script></body></html>`;

const server = createServer((request, response) => {
  if (request.url === "/old.css" || request.url === "/next.css") {
    const changed = request.url === "/next.css";
    response.writeHead(200, {
      "content-type": "text/css; charset=utf-8",
      "cache-control": "public, max-age=3600",
    });
    response.end(
      `${fontCss}#probe{font-family:"Pana Runtime Probe",sans-serif;color:${
        changed ? "rgb(30,100,220)" : "rgb(220,20,60)"
      };text-align:${changed ? "left" : "center"}}`,
    );
    return;
  }
  if (request.url === "/font-probe.woff2") {
    response.writeHead(200, {
      "content-type": "font/woff2",
      "cache-control": "public, max-age=3600",
      "access-control-allow-origin": "*",
    });
    response.end(fontFixture);
    return;
  }
  if (request.url === "/missing-font.css") {
    response.writeHead(200, {
      "content-type": "text/css; charset=utf-8",
      "cache-control": "no-store",
    });
    response.end(
      '@font-face{font-family:"Pana Missing Probe";src:url("/missing-font.woff2") format("woff2");font-style:normal;font-weight:700;font-display:swap}'
        + '#probe{font-family:"Pana Missing Probe",sans-serif;color:rgb(30,100,220)}',
    );
    return;
  }
  if (request.url !== "/") {
    response.writeHead(404).end("not found");
    return;
  }
  response.writeHead(200, {
    "content-type": "text/html; charset=utf-8",
    "cache-control": "no-store",
  });
  response.end(harness);
});

await new Promise((resolvePromise, rejectPromise) => {
  server.once("error", rejectPromise);
  server.listen(0, "127.0.0.1", resolvePromise);
});
const address = server.address();
assert(address && typeof address === "object");

const driverPort = 45000 + (process.pid % 1000);
const snapGeckodriver = "/snap/firefox/current/usr/lib/firefox/geckodriver";
const snapFirefox = "/snap/firefox/current/usr/lib/firefox/firefox";
const geckodriverBinary = process.env.GECKODRIVER_BIN
  || (existsSync(snapGeckodriver) ? snapGeckodriver : "geckodriver");
const firefoxBinary = process.env.FIREFOX_BIN
  || (existsSync(snapFirefox) ? snapFirefox : null);
const driver = spawn(geckodriverBinary, ["--port", String(driverPort)], {
  stdio: ["ignore", "pipe", "pipe"],
});
let driverDiagnostics = "";
driver.on("error", (error) => {
  driverDiagnostics += `geckodriver process error: ${error.message}\n`;
});
driver.stdout.on("data", (chunk) => { driverDiagnostics += chunk.toString(); });
driver.stderr.on("data", (chunk) => { driverDiagnostics += chunk.toString(); });

async function webdriver(path, init = {}) {
  const response = await fetch(`http://127.0.0.1:${driverPort}${path}`, {
    ...init,
    headers: { "content-type": "application/json", ...(init.headers || {}) },
  });
  const payload = await response.json();
  if (!response.ok || payload.value?.error) {
    throw new Error(`WebDriver ${path}: ${JSON.stringify(payload)}`);
  }
  return payload.value;
}

async function waitForDriver() {
  let lastError;
  for (let attempt = 0; attempt < 80; attempt += 1) {
    try {
      await webdriver("/status", { method: "GET" });
      return;
    } catch (error) {
      lastError = error;
      await new Promise((resolvePromise) => setTimeout(resolvePromise, 100));
    }
  }
  throw lastError;
}

let sessionId = "";
try {
  await waitForDriver();
  const session = await webdriver("/session", {
    method: "POST",
    body: JSON.stringify({
      capabilities: {
        alwaysMatch: {
          browserName: "firefox",
          "moz:firefoxOptions": {
            args: ["-headless"],
            ...(firefoxBinary ? { binary: firefoxBinary } : {}),
          },
        },
      },
    }),
  });
  sessionId = session.sessionId;
  await webdriver(`/session/${sessionId}/url`, {
    method: "POST",
    body: JSON.stringify({ url: `http://127.0.0.1:${address.port}/` }),
  });

  let title = "";
  let canvasAgentAuthoringClicked = false;
  let canvasAgentHovered = false;
  let canvasAgentClicked = false;
  let canvasAgentIconClicked = false;
  let canvasAgentDragged = false;
  let canvasAgentDropReleased = false;
  let canvasAgentActionClicked = false;
  for (let attempt = 0; attempt < 400; attempt += 1) {
    title = await webdriver(`/session/${sessionId}/execute/sync`, {
      method: "POST",
      body: JSON.stringify({ script: "return document.title", args: [] }),
    });
    if (title === "AGENT_AUTHORING_WAIT" && !canvasAgentAuthoringClicked) {
      canvasAgentAuthoringClicked = true;
      const frameElement = await webdriver(`/session/${sessionId}/element`, {
        method: "POST",
        body: JSON.stringify({ using: "css selector", value: "#canvas" }),
      });
      await webdriver(`/session/${sessionId}/frame`, {
        method: "POST",
        body: JSON.stringify({ id: frameElement }),
      });
      const authoringSlotElement = await webdriver(`/session/${sessionId}/element`, {
        method: "POST",
        body: JSON.stringify({
          using: "css selector",
          value: '[data-pana-active-document-root="source-empty-content"]',
        }),
      });
      const authoringSlotRect = await webdriver(`/session/${sessionId}/execute/sync`, {
        method: "POST",
        body: JSON.stringify({
          script: "const rect = arguments[0].getBoundingClientRect(); const parent = arguments[0].parentElement.getBoundingClientRect(); return { left: rect.left, right: rect.right, top: rect.top, bottom: parent.bottom };",
          args: [authoringSlotElement],
        }),
      });
      await webdriver(`/session/${sessionId}/actions`, {
        method: "POST",
        body: JSON.stringify({
          actions: [{
            type: "pointer",
            id: "canvas-authoring-mouse",
            parameters: { pointerType: "mouse" },
            actions: [
              {
                type: "pointerMove",
                duration: 0,
                origin: "viewport",
                x: Math.round((authoringSlotRect.left + authoringSlotRect.right) / 2),
                y: Math.round((authoringSlotRect.top + authoringSlotRect.bottom) / 2),
              },
              { type: "pointerDown", button: 0 },
              { type: "pointerUp", button: 0 },
            ],
          }],
        }),
      });
      await webdriver(`/session/${sessionId}/frame`, {
        method: "POST",
        body: JSON.stringify({ id: null }),
      });
    }
    if (title === "AGENT_HOVER_WAIT" && !canvasAgentHovered) {
      canvasAgentHovered = true;
      const frameElement = await webdriver(`/session/${sessionId}/element`, {
        method: "POST",
        body: JSON.stringify({ using: "css selector", value: "#canvas" }),
      });
      await webdriver(`/session/${sessionId}/frame`, {
        method: "POST",
        body: JSON.stringify({ id: frameElement }),
      });
      const probeElement = await webdriver(`/session/${sessionId}/element`, {
        method: "POST",
        body: JSON.stringify({ using: "css selector", value: "#probe" }),
      });
      await webdriver(`/session/${sessionId}/actions`, {
        method: "POST",
        body: JSON.stringify({
          actions: [{
            type: "pointer",
            id: "canvas-hover-mouse",
            parameters: { pointerType: "mouse" },
            actions: [
              { type: "pointerMove", duration: 0, origin: probeElement, x: 0, y: 0 },
            ],
          }],
        }),
      });
      await webdriver(`/session/${sessionId}/frame`, {
        method: "POST",
        body: JSON.stringify({ id: null }),
      });
    }
    if (title === "AGENT_WAIT" && !canvasAgentClicked) {
      canvasAgentClicked = true;
      const frameElement = await webdriver(`/session/${sessionId}/element`, {
        method: "POST",
        body: JSON.stringify({ using: "css selector", value: "#canvas" }),
      });
      await webdriver(`/session/${sessionId}/frame`, {
        method: "POST",
        body: JSON.stringify({ id: frameElement }),
      });
      const probeElement = await webdriver(`/session/${sessionId}/element`, {
        method: "POST",
        body: JSON.stringify({ using: "css selector", value: "#probe" }),
      });
      await webdriver(`/session/${sessionId}/actions`, {
        method: "POST",
        body: JSON.stringify({
          actions: [{
            type: "pointer",
            id: "canvas-mouse",
            parameters: { pointerType: "mouse" },
            actions: [
              { type: "pointerMove", duration: 0, origin: probeElement, x: 0, y: 0 },
              { type: "pointerDown", button: 0 },
              { type: "pointerUp", button: 0 },
            ],
          }],
        }),
      });
      await webdriver(`/session/${sessionId}/frame`, {
        method: "POST",
        body: JSON.stringify({ id: null }),
      });
    }
    if (title === "AGENT_ICON_WAIT" && !canvasAgentIconClicked) {
      canvasAgentIconClicked = true;
      const frameElement = await webdriver(`/session/${sessionId}/element`, {
        method: "POST",
        body: JSON.stringify({ using: "css selector", value: "#canvas" }),
      });
      await webdriver(`/session/${sessionId}/frame`, {
        method: "POST",
        body: JSON.stringify({ id: frameElement }),
      });
      const iconPathElement = await webdriver(`/session/${sessionId}/element`, {
        method: "POST",
        body: JSON.stringify({ using: "css selector", value: "#icon-probe > path" }),
      });
      await webdriver(`/session/${sessionId}/actions`, {
        method: "POST",
        body: JSON.stringify({
          actions: [{
            type: "pointer",
            id: "canvas-icon-mouse",
            parameters: { pointerType: "mouse" },
            actions: [
              { type: "pointerMove", duration: 0, origin: iconPathElement, x: 0, y: 0 },
              { type: "pointerDown", button: 0 },
              { type: "pointerUp", button: 0 },
            ],
          }],
        }),
      });
      await webdriver(`/session/${sessionId}/frame`, {
        method: "POST",
        body: JSON.stringify({ id: null }),
      });
    }
    if (title === "AGENT_DRAG_WAIT" && !canvasAgentDragged) {
      canvasAgentDragged = true;
      const frameElement = await webdriver(`/session/${sessionId}/element`, {
        method: "POST",
        body: JSON.stringify({ using: "css selector", value: "#canvas" }),
      });
      await webdriver(`/session/${sessionId}/frame`, {
        method: "POST",
        body: JSON.stringify({ id: frameElement }),
      });
      const probeElement = await webdriver(`/session/${sessionId}/element`, {
        method: "POST",
        body: JSON.stringify({ using: "css selector", value: "#probe" }),
      });
      const navElement = await webdriver(`/session/${sessionId}/element`, {
        method: "POST",
        body: JSON.stringify({ using: "css selector", value: "#nav-probe" }),
      });
      await webdriver(`/session/${sessionId}/actions`, {
        method: "POST",
        body: JSON.stringify({
          actions: [{
            type: "pointer",
            id: "canvas-drag-mouse",
            parameters: { pointerType: "mouse" },
            actions: [
              { type: "pointerMove", duration: 0, origin: probeElement, x: 0, y: 0 },
              { type: "pointerDown", button: 0 },
              { type: "pointerMove", duration: 250, origin: navElement, x: 0, y: 0 },
            ],
          }],
        }),
      });
      await webdriver(`/session/${sessionId}/frame`, {
        method: "POST",
        body: JSON.stringify({ id: null }),
      });
    }
    if (title === "AGENT_DROP_WAIT" && !canvasAgentDropReleased) {
      canvasAgentDropReleased = true;
      const frameElement = await webdriver(`/session/${sessionId}/element`, {
        method: "POST",
        body: JSON.stringify({ using: "css selector", value: "#canvas" }),
      });
      await webdriver(`/session/${sessionId}/frame`, {
        method: "POST",
        body: JSON.stringify({ id: frameElement }),
      });
      await webdriver(`/session/${sessionId}/actions`, {
        method: "POST",
        body: JSON.stringify({
          actions: [{
            type: "pointer",
            id: "canvas-drag-mouse",
            parameters: { pointerType: "mouse" },
            actions: [
              { type: "pointerUp", button: 0 },
            ],
          }],
        }),
      });
      await webdriver(`/session/${sessionId}/frame`, {
        method: "POST",
        body: JSON.stringify({ id: null }),
      });
    }
    if (title === "AGENT_ACTION_WAIT" && !canvasAgentActionClicked) {
      canvasAgentActionClicked = true;
      const frameElement = await webdriver(`/session/${sessionId}/element`, {
        method: "POST",
        body: JSON.stringify({ using: "css selector", value: "#canvas" }),
      });
      await webdriver(`/session/${sessionId}/frame`, {
        method: "POST",
        body: JSON.stringify({ id: frameElement }),
      });
      const actionElement = await webdriver(`/session/${sessionId}/element`, {
        method: "POST",
        body: JSON.stringify({
          using: "css selector",
          value: "[data-pana-canvas-agent-action='enterBoundary']",
        }),
      });
      await webdriver(`/session/${sessionId}/element/${actionElement["element-6066-11e4-a52e-4f735466cecf"]}/click`, {
        method: "POST",
        body: "{}",
      });
      await webdriver(`/session/${sessionId}/frame`, {
        method: "POST",
        body: JSON.stringify({ id: null }),
      });
    }
    if (title === "PASS" || title === "FAIL") break;
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 100));
  }
  const result = await webdriver(`/session/${sessionId}/execute/sync`, {
    method: "POST",
    body: JSON.stringify({ script: "return document.getElementById('result').textContent", args: [] }),
  });
  assert.equal(title, "PASS", result);
  const evidence = JSON.parse(result);
  assert.equal(evidence.ok, true);
  assert.equal(evidence.sameDocument, true);
  assert(evidence.samples > 0);
  process.stdout.write(`${JSON.stringify(evidence)}\n`);
} finally {
  if (sessionId) {
    await webdriver(`/session/${sessionId}`, { method: "DELETE", body: "{}" }).catch(() => {});
  }
  try {
    driver.kill("SIGTERM");
  } catch (error) {
    driverDiagnostics += `geckodriver cleanup warning: ${error.message}\n`;
  }
  driver.stdout.destroy();
  driver.stderr.destroy();
  driver.unref();
  await new Promise((resolvePromise) => server.close(resolvePromise));
  if (driverDiagnostics && process.env.PANA_BROWSER_TEST_VERBOSE === "1") {
    process.stderr.write(driverDiagnostics);
  }
}
