import assert from "node:assert/strict";
import { spawn } from "node:child_process";
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
  "04_html_selection.js",
  "05_template_gate.js",
  "06_empty_zones.js",
  "06_preview_hover.js",
  "07_drag_drop.js",
  "08_inspector_shell.js",
  "09_design_safe_surface.js",
  "10_canvas_patch.js",
  "11_document_sync.js",
  "12_messages_events.js",
  "13_boot.js",
];

const bridge = (
  await Promise.all(
    bridgeParts.map((part) => readFile(
      resolve(repoRoot, "src-tauri/src/preview/bridge", part),
      "utf8",
    )),
  )
).join("");
const interactiveRuntime = await readFile(
  resolve(repoRoot, "src-tauri/src/preview/interactive_runtime.js"),
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
  <body><main><h1 id="probe" data-pana-source-id="source-title">Before</h1><a id="nav-probe" data-pana-source-id="source-nav" href="/servicii">Servicii</a></main>
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
  <body><main><h1 id="probe" data-pana-source-id="source-title">After</h1><a id="nav-probe" data-pana-source-id="source-nav" href="/despre">Servicii</a></main></body>
</html>`;

const interactiveDocument = `<!doctype html>
<html data-pana-preview-revision="interactive-browser-real">
  <body>
    <section data-pana-component="accordion" data-pana-source-id="source-accordion">
      <div data-pana-accordion-item>
        <button data-pana-accordion-trigger aria-expanded="false">Toggle</button>
        <div data-pana-accordion-panel hidden>Panel</div>
      </div>
    </section>
    <script>${escapeInlineScript(interactiveRuntime)}</script>
    <script>window.PanaInteractiveRuntime.installPageConfig({components:[{id:"accordion"}],motion:{items:[{id:"motion-1"}]}});</script>
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

  window.addEventListener("message", (event) => {
    if (event.source === frame.contentWindow && event.data?.source === "pana-studio-preview") {
      messages.push(event.data);
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
      patchAck = await applyMeasuredPatch(index + 1, index + 1, index >= 5);
    }
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
      selector: "#probe",
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
      editSessionId: "attr_browser_real_1"
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
      selector: "#probe",
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

    interactiveFrame.srcdoc = interactiveDocument;
    const interactiveReady = await waitForInteractiveMessage((data) => data?.type === "ready");
    const configReceipt = await waitForInteractiveMessage((data) => data?.type === "page-config-installed");
    const domSnapshot = await waitForInteractiveMessage((data) => data?.type === "dom-snapshot");
    if (interactiveReady.previewRevision !== "interactive-browser-real") {
      throw new Error("interactive ready revision mismatch");
    }
    if (configReceipt.componentCount !== 1 || configReceipt.motionItemCount !== 1) {
      throw new Error("PageJsConfig lifecycle receipt mismatch");
    }
    if (!domSnapshot.nodes?.some((node) => node.sourceId === "source-accordion")) {
      throw new Error("interactive read-only DOM snapshot lost source provenance");
    }
    const interactiveWindow = interactiveFrame.contentWindow;
    const interactiveDoc = interactiveFrame.contentDocument;
    const trigger = interactiveDoc.querySelector("[data-pana-accordion-trigger]");
    const panel = interactiveDoc.querySelector("[data-pana-accordion-panel]");
    trigger.click();
    if (trigger.getAttribute("aria-expanded") !== "true" || panel.hidden) {
      throw new Error("interactive lifecycle mount did not handle the component");
    }
    interactiveWindow.PanaInteractiveRuntime.reconcile(interactiveDoc);
    interactiveWindow.PanaInteractiveRuntime.reconcile(interactiveDoc);
    trigger.click();
    if (trigger.getAttribute("aria-expanded") !== "false" || !panel.hidden) {
      throw new Error("interactive lifecycle reconcile duplicated listeners");
    }
    interactiveDoc.dispatchEvent(new interactiveWindow.CustomEvent("pana:components:dispose", {
      detail: { root: interactiveDoc }
    }));
    trigger.click();
    if (trigger.getAttribute("aria-expanded") !== "false") {
      throw new Error("interactive lifecycle dispose leaked a listener");
    }
    if (interactiveWindow.__panaMotionGraphConfig?.items?.length !== 1) {
      throw new Error("MotionGraph was not derived from PageJsConfig in the interactive realm");
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
      motionItems: interactiveWindow.__panaMotionGraphConfig.items.length
    });
  }

  run().catch((error) => finish(false, {
    error: String(error?.stack || error),
    childDiagnostics,
    previewMessageTypes: messages.map((message) => message.type)
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
const driver = spawn("geckodriver", ["--port", String(driverPort)], {
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
  for (let attempt = 0; attempt < 160; attempt += 1) {
    title = await webdriver(`/session/${sessionId}/execute/sync`, {
      method: "POST",
      body: JSON.stringify({ script: "return document.title", args: [] }),
    });
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
