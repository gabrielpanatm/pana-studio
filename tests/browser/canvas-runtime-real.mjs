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
const blockRuntime = await readFile(
  resolve(repoRoot, "src-tauri/src/blocks/runtime.js"),
  "utf8",
);

const identity = {
  projectRoot: "/project",
  runtimeSessionId: "runtime-browser-real",
  workspaceRevision: 107,
  transactionId: "canvas_next_browser_real",
  previewRevision: "preview-next-browser-real",
};
const oldCss = `data:text/css,${encodeURIComponent("#probe{color:rgb(220,20,60);text-align:center}")}`;
const nextCss = `data:text/css,${encodeURIComponent("#probe{color:rgb(30,100,220);text-align:left}")}`;

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
  <head><link rel="stylesheet" href="${oldCss}"></head>
  <body><main><h1 id="probe" data-pana-source-id="source-title" data-pana-render-instance-id="render-title">Before</h1><a id="nav-probe" data-pana-source-id="source-nav" data-pana-render-instance-id="render-nav" href="/servicii">Servicii</a></main>
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
  <head><link rel="stylesheet" href="${nextCss}"></head>
  <body><main><h1 id="probe" data-pana-source-id="source-title" data-pana-render-instance-id="render-title">After</h1><a id="nav-probe" data-pana-source-id="source-nav" data-pana-render-instance-id="render-nav" href="/despre">Servicii</a></main></body>
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
    <script>${escapeInlineScript(blockRuntime)}</script>
    <script>${escapeInlineScript(interactiveRuntime)}</script>
    <script>window.PanaBlockRuntime.installPageConfig({
      version:2,
      blocks:[{id:"accordion"}],
      motion:{
        schemaVersion:2,
        animeVersion:"4.4.1",
        interactions:[{id:"motion-1"}],
        behaviors:[],
        customCode:[]
      }
    });</script>
  </body>
</html>`;

const harness = `<!doctype html>
<html><head><meta charset="utf-8"><title>RUNNING</title></head>
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
  const interactiveDocument = ${htmlJson(interactiveDocument)};
  const identity = ${JSON.stringify(identity)};
  const messages = [];
  const canvasAgentMessages = [];
  const interactiveMessages = [];
  const childDiagnostics = [];
  const colors = [];
  const patchRoundTrips = [];
  const patchBridgeDurations = [];
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
    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "activate-canvas-interaction-agent",
      schemaVersion: 2,
      agentInstanceId: agentReady.agentInstanceId,
      documentEpoch: 1,
      lastAcceptedSequence: 0,
      selection: true
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
    result.textContent = "canvas-agent-native-click";
    document.title = "AGENT_WAIT";
    const agentClick = await waitForCanvasAgentMessage((data) =>
      data?.type === "gesture" && data.gesture === "click"
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
            selectorFallback: "#probe",
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
          selectorFallback: "#nav-probe",
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

    frame.contentWindow.postMessage({
      source: "pana-studio-app",
      type: "apply-live-attribute-draft",
      previewRevision: 851,
      editSessionId: "unsafe_href_browser_real",
      draftEpoch: 1,
      target: {
        selector: "#nav-probe",
        sourceId: "source-nav",
        sessionId: null,
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
        selector: "#probe",
        sourceId: "source-title",
        sessionId: null,
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
        selector: "#probe",
        sourceId: "source-title",
        sessionId: null,
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
    const phases = canonicalAck.canvasPhaseReceipts?.map((entry) => entry.phase) ?? [];
    if (phases.join(",") !== "resourcesReady,committed,styledReady") {
      throw new Error("canonical phase sequence mismatch: " + phases.join(","));
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
    if (frame.contentDocument.getElementById("probe")?.textContent !== "After") {
      throw new Error("closed live text draft leaked into a later canonical reconcile");
    }
    if (frame.contentDocument.getElementById("probe")?.hasAttribute("title")) {
      throw new Error("closed live attribute draft leaked into a later canonical reconcile");
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
    if (persistentDocument !== frame.contentDocument) throw new Error("same-route document navigated");
    if (finalColor !== "rgb(30, 100, 220)") throw new Error("final stylesheet mismatch: " + finalColor);
    if (unstyledFrames.length > 0) throw new Error("unstyled frame observed: " + unstyledFrames.join("|"));
    if (!Number.isFinite(patchP95Ms) || patchP95Ms >= 50) {
      throw new Error("warmed CanvasPatch p95 exceeded 50 ms: " + patchP95Ms);
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
      configReceipt.blockCount !== 1
      || configReceipt.motionInteractionCount !== 1
      || configReceipt.motionBehaviorCount !== 0
      || configReceipt.motionCustomCodeCount !== 0
    ) {
      throw new Error("PageJsConfig lifecycle receipt mismatch");
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
    interactiveDoc.dispatchEvent(new interactiveWindow.CustomEvent("pana:blocks:dispose", {
      detail: { root: interactiveDoc }
    }));
    triggers[0].click();
    if (triggers[0].getAttribute("aria-expanded") !== "true") {
      throw new Error("interactive lifecycle dispose leaked a listener");
    }
    if (interactiveWindow.__panaMotionV2Config?.interactions?.length !== 1) {
      throw new Error("Motion v2 was not derived from PageJsConfig in the interactive realm");
    }
    if (interactiveMessages.some((message) => message.type === "lifecycle-error")) {
      throw new Error("interactive lifecycle emitted an error");
    }
    finish(true, {
      samples: colors.length,
      colors: [...new Set(colors)],
      phases,
      patchSamples: patchRoundTrips.length,
      patchP95Ms,
      bridgeP95Ms,
      lastPatchBridgeMs: patchAck.canvasPatchReceipt.bridgeCommitDurationMs,
      sameDocument: true,
      interactiveNodes: domSnapshot.nodes.length,
      interactiveLifecycle: "mount/reconcile/dispose",
      motionInteractions: interactiveWindow.__panaMotionV2Config.interactions.length,
      canvasAgentHover: "trusted-hover",
      canvasAgentGesture: "trusted-click",
      canvasAgentDrag: "trusted-drag",
      canvasAgentDragIndicator: "rust-projected",
      canvasAgentBoundaryAction: "rust-projected",
      canvasAgentInspection: "physical-only",
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
const geckodriverBinary = process.env.GECKODRIVER_BIN
  || (existsSync(snapGeckodriver) ? snapGeckodriver : "geckodriver");
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
          "moz:firefoxOptions": { args: ["-headless"] },
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
  let canvasAgentHovered = false;
  let canvasAgentClicked = false;
  let canvasAgentDragged = false;
  let canvasAgentDropReleased = false;
  let canvasAgentActionClicked = false;
  for (let attempt = 0; attempt < 400; attempt += 1) {
    title = await webdriver(`/session/${sessionId}/execute/sync`, {
      method: "POST",
      body: JSON.stringify({ script: "return document.title", args: [] }),
    });
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
