import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const cargo = spawnSync("cargo", [
  "test",
  "--manifest-path",
  "src-tauri/Cargo.toml",
  "js::motion::tests::browser_fixture_emits_exact_runtime",
  "--lib",
  "--",
  "--exact",
  "--nocapture",
], {
  cwd: repoRoot,
  encoding: "utf8",
  env: {
    ...process.env,
    CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR || "/tmp/pana-motion-v2-target",
  },
  maxBuffer: 4 * 1024 * 1024,
});
assert.equal(cargo.status, 0, cargo.stderr || cargo.stdout);
const runtimeLine = cargo.stdout
  .split(/\r?\n/)
  .find((line) => line.startsWith("PANA_MOTION_FIXTURE_JSON="));
assert(runtimeLine, "Rust did not emit the exact Motion runtime fixture");
const motionFixture = JSON.parse(runtimeLine.slice("PANA_MOTION_FIXTURE_JSON=".length));
const animeVersion = motionFixture.animeVersion;
assert.match(animeVersion, /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/);
const previewRuntime = motionFixture.previewRuntime;
const productionRuntime = motionFixture.productionRuntime;
const previewPayload = Buffer.from(motionFixture.previewPayload, "utf8").toString("base64");
const animeRuntime = await readFile(
  resolve(repoRoot, "src-tauri/resources/anime.umd.min.js"),
  "utf8",
);

function runtimePage(preview) {
  const reducedMotionOverride = preview
    ? ""
    : `<script>
      (() => {
        const nativeMatchMedia = window.matchMedia.bind(window);
        window.matchMedia = (query) => {
          if (query !== "(prefers-reduced-motion: reduce)") return nativeMatchMedia(query);
          const listeners = new Set();
          return {
            media: query,
            matches: true,
            onchange: null,
            addListener(listener) { listeners.add(listener); },
            removeListener(listener) { listeners.delete(listener); },
            addEventListener(_type, listener) { listeners.add(listener); },
            removeEventListener(_type, listener) { listeners.delete(listener); },
            dispatchEvent(event) {
              listeners.forEach((listener) => listener.call(this, event));
              return true;
            }
          };
        };
      })();
    <\/script>`;
  const runtimeScripts = preview
    ? `<meta name="pana-motion-preview-config" content="${previewPayload}">
       <script src="/anime.js"><\/script>
       <script src="/motion-preview.js"><\/script>`
    : `<script src="/motion-production.js"><\/script>`;
  return `<!doctype html>
<html>
  <head><meta charset="utf-8"><title>Motion realm</title></head>
  <body>
    <div id="hero" data-anim="hero">Hero</div>
    <button id="button" data-anim="button">Run</button>
    <div id="hidden" data-anim="hidden">Hidden condition</div>
    <div id="reduced-skip" data-anim="reduced-skip">Reduced skip</div>
    <div id="reduced-duration" data-anim="reduced-duration">Reduced duration</div>
    <div id="pointer" data-anim="pointer" style="width: 400px; height: 20px">Pointer</div>
    <div id="scroll" data-anim="scroll" style="margin-top: 1200px">Scroll</div>
    <div id="drag" data-anim="drag">Drag</div>
    <div id="layout" data-anim="layout"><span class="layout-item">Layout</span></div>
    ${reducedMotionOverride}
    ${runtimeScripts}
    <script>window.__motionRuntimeReady = true;<\/script>
  </body>
</html>`;
}

const harness = `<!doctype html>
<html>
  <head><meta charset="utf-8"><title>RUNNING</title></head>
  <body>
    <pre id="result">running</pre>
    <iframe id="preview" src="/realm?preview=1"></iframe>
    <iframe id="published" src="/realm"></iframe>
    <script>
      (() => {
        const result = document.getElementById("result");
        const previewFrame = document.getElementById("preview");
        const publishedFrame = document.getElementById("published");
        const previewMessages = [];
        window.addEventListener("message", (event) => {
          if (
            event.source === previewFrame.contentWindow
            && event.data?.source === "pana-studio-motion-runtime"
          ) previewMessages.push(event.data);
        });

        const wait = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds));
        const frameReady = (frame) => new Promise((resolve, reject) => {
          const timeout = setTimeout(() => reject(new Error("Motion realm timeout")), 12000);
          const finish = () => {
            if (!frame.contentWindow?.__motionRuntimeReady) return;
            clearTimeout(timeout);
            resolve();
          };
          frame.addEventListener("load", finish, { once: true });
          finish();
        });
        const assertNear = (value, expected, tolerance, label) => {
          if (!Number.isFinite(value) || Math.abs(value - expected) > tolerance) {
            throw new Error(label + ": expected " + expected + ", received " + value);
          }
        };
        const translateX = (frame, node) => {
          const transform = frame.getComputedStyle(node).transform;
          return transform === "none" ? 0 : new frame.DOMMatrix(transform).m41;
        };
        const finish = (ok, details) => {
          result.textContent = JSON.stringify({ ok, ...details });
          document.title = ok ? "PASS" : "FAIL";
        };

        async function run() {
          await Promise.all([frameReady(previewFrame), frameReady(publishedFrame)]);

          const previewWindow = previewFrame.contentWindow;
          const previewDocument = previewFrame.contentDocument;
          const previewRegistry = previewWindow.__panaMotionV2;
          if (!previewRegistry || previewRegistry.schemaVersion !== 2) {
            throw new Error("Motion v2 preview registry is missing");
          }
          if (Object.keys(previewRegistry.scopes).length !== 0) {
            throw new Error("Motion Preview installed published trigger scopes");
          }
          previewRegistry.preview.seek("preview-sequence", 101);
          await wait(30);
          const previewHero = previewDocument.getElementById("hero");
          assertNear(translateX(previewWindow, previewHero), 50.5, 3, "preview translateX");
          assertNear(Number(previewWindow.getComputedStyle(previewHero).opacity), 0.505, 0.04, "preview opacity");
          if (previewHero.style.getPropertyValue("--motion-probe").trim() !== "ready") {
            throw new Error("reversible Set property was not projected in Motion Preview");
          }
          if (
            previewHero.classList.contains("published-motion")
            || previewHero.hasAttribute("data-motion-state")
            || previewWindow.__motionCallCount
          ) {
            throw new Error("Motion Preview executed a non-reversible side effect");
          }
          if (!previewMessages.some((message) =>
            message.type === "state"
              && message.interactionId === "preview-sequence"
              && Math.abs(message.value - 101) < 2
          )) {
            throw new Error("preview playhead state was not bridged to the parent");
          }
          previewRegistry.destroy();
          if (previewHero.getAttribute("style")) {
            throw new Error("Motion Preview did not restore the element inline state");
          }

          const publishedWindow = publishedFrame.contentWindow;
          const publishedDocument = publishedFrame.contentDocument;
          await wait(500);
          const publishedHero = publishedDocument.getElementById("hero");
          if (publishedWindow.__motionCallCount !== 1) {
            throw new Error("published load Call did not execute exactly once");
          }
          if (publishedWindow.__customActive !== 1) {
            throw new Error("custom lifecycle did not install exactly once");
          }
          if (publishedWindow.__hiddenMotionRan) {
            throw new Error("false responsive media condition executed");
          }
          if (
            !publishedHero.classList.contains("published-motion")
            || publishedHero.getAttribute("data-motion-state") !== "ready"
          ) {
            throw new Error("published Set side effects did not execute");
          }
          assertNear(
            Number(publishedWindow.getComputedStyle(
              publishedDocument.getElementById("reduced-skip"),
            ).opacity),
            1,
            0.01,
            "skip-to-end reduced motion",
          );
          if (publishedWindow.PanaMotionRuntime || publishedWindow.__panaMotionV2) {
            throw new Error("production exposed the editor Motion registry");
          }
          const pointer = publishedDocument.getElementById("pointer");
          const pointerRect = pointer.getBoundingClientRect();
          pointer.dispatchEvent(new publishedWindow.PointerEvent("pointermove", {
            bubbles: true,
            clientX: pointerRect.left + pointerRect.width * 0.75,
            clientY: pointerRect.top + pointerRect.height / 2,
          }));
          await wait(40);
          assertNear(
            translateX(publishedWindow, pointer),
            75,
            4,
            "pointer progress scrub",
          );

          publishedDocument.getElementById("button").click();
          await wait(120);
          assertNear(
            Number(publishedWindow.getComputedStyle(publishedDocument.getElementById("button")).opacity),
            1,
            0.03,
            "published click trigger",
          );

          publishedWindow.dispatchEvent(new publishedWindow.Event("pagehide"));
          await wait(40);
          if (
            publishedWindow.__customActive !== 0
            || publishedWindow.__motionCallCleanup !== 1
            || publishedHero.classList.contains("published-motion")
            || publishedHero.hasAttribute("data-motion-state")
            || publishedHero.getAttribute("style")
            || pointer.style.transform
          ) {
            throw new Error("destroy did not restore DOM and effect lifecycles");
          }

          finish(true, {
            animeVersion: "${animeVersion} modular",
            previewStateMessages: previewMessages.filter((message) => message.type === "state").length,
            reducedDurationMs: 40,
            productionCallCount: publishedWindow.__motionCallCount,
            lifecycle: "preview/seek/production-triggers/reduced/pagehide-cleanup"
          });
        }

        run().catch((error) => finish(false, {
          error: String(error?.message || error) + "\\n" + String(error?.stack || "")
        }));
      })();
    <\/script>
  </body>
</html>`;

const server = createServer((request, response) => {
  const url = new URL(request.url || "/", "http://127.0.0.1");
  if (url.pathname === "/") {
    response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
    response.end(harness);
    return;
  }
  if (url.pathname === "/realm") {
    response.writeHead(200, {
      "content-type": "text/html; charset=utf-8",
      "cache-control": "no-store",
    });
    response.end(runtimePage(url.searchParams.get("preview") === "1"));
    return;
  }
  if (url.pathname === "/anime.js") {
    response.writeHead(200, { "content-type": "text/javascript; charset=utf-8" });
    response.end(animeRuntime);
    return;
  }
  if (url.pathname === "/motion-preview.js") {
    response.writeHead(200, {
      "content-type": "text/javascript; charset=utf-8",
      "cache-control": "no-store",
    });
    response.end(previewRuntime);
    return;
  }
  if (url.pathname === "/motion-production.js") {
    response.writeHead(200, {
      "content-type": "text/javascript; charset=utf-8",
      "cache-control": "no-store",
    });
    response.end(productionRuntime);
    return;
  }
  const animePublicRoot = `/js/vendor/animejs-${animeVersion}/`;
  if (url.pathname.startsWith(animePublicRoot)) {
    const modulePath = url.pathname.slice(animePublicRoot.length);
    const animeModuleRoot = resolve(
      repoRoot,
      `src-tauri/resources/animejs-${animeVersion}/modules`,
    );
    const absolutePath = resolve(
      animeModuleRoot,
      modulePath,
    );
    if (!absolutePath.startsWith(`${animeModuleRoot}/`)) {
      response.writeHead(400).end("invalid module path");
      return;
    }
    response.writeHead(200, { "content-type": "text/javascript; charset=utf-8" });
    readFile(absolutePath).then(
      (source) => response.end(source),
      () => response.writeHead(404).end("module not found"),
    );
    return;
  }
  response.writeHead(404).end("not found");
});

await new Promise((resolvePromise, rejectPromise) => {
  server.once("error", rejectPromise);
  server.listen(0, "127.0.0.1", resolvePromise);
});
const address = server.address();
assert(address && typeof address === "object");

const driverPort = 46000 + (process.pid % 1000);
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
  for (let attempt = 0; attempt < 240; attempt += 1) {
    title = await webdriver(`/session/${sessionId}/execute/sync`, {
      method: "POST",
      body: JSON.stringify({ script: "return document.title", args: [] }),
    });
    if (title === "PASS" || title === "FAIL") break;
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 100));
  }
  const result = await webdriver(`/session/${sessionId}/execute/sync`, {
    method: "POST",
    body: JSON.stringify({
      script: "return document.getElementById('result').textContent",
      args: [],
    }),
  });
  assert.equal(title, "PASS", result);
  const evidence = JSON.parse(result);
  assert.equal(evidence.ok, true);
  assert.equal(evidence.animeVersion, `${animeVersion} modular`);
  process.stdout.write(`${JSON.stringify(evidence)}\n`);
} finally {
  if (sessionId) {
    await webdriver(`/session/${sessionId}`, {
      method: "DELETE",
      body: "{}",
    }).catch(() => {});
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
