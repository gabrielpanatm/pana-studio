import assert from "node:assert/strict";
import { test } from "node:test";
import { CssAuthoringState } from "$lib/css/authoring-state.svelte";
import { PageSectionsState } from "$lib/preview/page-sections.svelte";
import { PreviewSurfaceState } from "$lib/preview/surface-state.svelte";
import { PreviewWorkspaceState } from "$lib/preview/workspace-state.svelte";
import { ProjectSessionState } from "$lib/project/session-state.svelte";
import {
  beginPreviewRefreshLease,
  previewRefreshLeaseMatches,
} from "$lib/state/preview-controller";

if (!globalThis.window) globalThis.window = globalThis;
if (!globalThis.location) globalThis.location = new URL("http://app.local/");

function workspace(options = {}) {
  const session = new ProjectSessionState();
  session.root = "/project";
  session.runtimeSessionId = "session:runtime-1";
  session.epoch = 3;
  session.project = {
    root: "/project",
    files: options.files ?? [],
    previewBaseUrl: "http://127.0.0.1:4000",
  };
  const surface = new PreviewSurfaceState();
  const css = new CssAuthoringState();
  const sections = new PageSectionsState(() => null);
  let resetCount = 0;
  const controlled = { reset() { resetCount += 1; } };
  const state = new PreviewWorkspaceState({
    session,
    surface,
    css,
    sections,
    selection: {
      refreshNavigationSnapshot: options.refreshNavigationSnapshot ?? (async () => {}),
      reset() { resetCount += 1; },
    },
    controlled: () => controlled,
    motion: { previewMode: "design", previewStatus: null },
    context: () => ({
      activePage: null,
      isActivePage: false,
      templateWorkbenchActive: options.templateWorkbenchActive ?? false,
      project: session.project,
      activeScannedPath: options.activeScannedPath ?? null,
      activeVersionPreview: null,
    }),
    setStatus() {},
    clearStatus() {},
    reportCanvasDegraded: options.reportCanvasDegraded ?? (async () => {}),
    projectLatest: options.projectLatest ?? (async () => ({ status: "deferred", workspaceRevision: null })),
    loadProjectFile: options.loadProjectFile ?? (async () => {}),
    invalidateSourceGraph() { resetCount += 1; },
  });
  return { state, session, surface, resetCount: () => resetCount };
}

test("PreviewWorkspace deține suprafața și invalidează lease-ul la schimbarea sesiunii", () => {
  const { state, session, surface } = workspace();
  const frame = { contentWindow: { postMessage() {} } };
  assert.equal(state.mountAndTrackSurface(frame), 1);
  assert.equal(surface.canvasElement, frame);
  assert.equal(surface.frame, frame);

  const lease = beginPreviewRefreshLease(state.commands().session);
  assert.ok(lease);
  assert.equal(previewRefreshLeaseMatches(state.commands().session, lease), true);
  session.runtimeSessionId = "session:runtime-2";
  assert.equal(previewRefreshLeaseMatches(state.commands().session, lease), false);
});

test("resetarea Preview curăță runtime-ul, selecția și generațiile domeniului", () => {
  const { state, resetCount } = workspace();
  state.src = "http://127.0.0.1:4000/";
  state.interactiveEnabled = true;
  state.workspaceRevision = "preview:4";
  const refreshBefore = state.refreshSerial;

  state.resetControlled();

  assert.equal(state.src, "about:blank");
  assert.equal(state.interactiveEnabled, false);
  assert.equal(state.workspaceRevision, null);
  assert.ok(state.refreshSerial > refreshBefore);
  assert.equal(resetCount(), 3);
});

test("Canvas reia automat proiecția dacă load sosește înainte de curățarea pending", async () => {
  const template = {
    name: "index.html",
    relativePath: "templates/index.html",
    role: "template",
  };
  const events = [];
  let degradedCount = 0;
  const { state, session, surface } = workspace({
    files: [template],
    activeScannedPath: template.relativePath,
    templateWorkbenchActive: true,
    async projectLatest() {
      throw new Error("planul pregătit nu trebuie înlocuit cu o proiecție generică");
    },
    async reportCanvasDegraded() {
      degradedCount += 1;
    },
    async refreshNavigationSnapshot(identity, url, options) {
      assert.equal(options.strict, true);
      events.push(`navigation:${identity.transactionId}:${url}`);
    },
  });
  const plan = {
    schemaVersion: 1,
    phase: "prepared",
    identity: {
      projectRoot: "/project",
      runtimeSessionId: "session:runtime-1",
      workspaceRevision: 7,
      transactionId: "canvas:7",
      previewRevision: "preview:7",
    },
    workspaceTransactionId: null,
    impact: { kinds: ["fullDocument"], paths: [], requiresFullDocument: true },
    resources: { schemaVersion: 1, previewRevision: "preview:7", totalBytes: 0, entries: [] },
  };
  const frame = { contentWindow: { postMessage() {} } };
  session.workspace = { revision: 7 };
  state.reconcileWorkbenchDocument = async (url, candidate) => {
    events.push(`reconcile:${candidate.identity.transactionId}:${url}`);
    state.activeIdentity = { ...candidate.identity };
    state.activeUrl = url;
    return true;
  };

  state.src = "http://127.0.0.1:4000/?__pana_preview_revision=preview%3A7";
  state.setPendingProjection(plan);
  state.deferSurfaceProjection();
  state.mountAndTrackSurface(frame);
  state.onSurfaceLoaded(frame);

  assert.equal(surface.loadedGeneration, surface.generation);
  assert.equal(surface.resumeRequired, true);
  assert.equal(state.deferredProjection, plan);
  assert.deepEqual(events, []);

  state.setPendingProjection(null);
  await Promise.resolve();
  assert.ok(surface.resumePromise);
  await surface.resumePromise;

  assert.deepEqual(events, [
    "reconcile:canvas:7:http://127.0.0.1:4000/?__pana_preview_revision=preview%3A7",
    "navigation:canvas:7:http://127.0.0.1:4000/?__pana_preview_revision=preview%3A7",
  ]);
  assert.deepEqual(state.activeIdentity, plan.identity);
  assert.equal(state.deferredProjection, null);
  assert.equal(surface.resumeRequired, false);
  assert.equal(degradedCount, 0);

  state.setPendingProjection(null);
  state.onSurfaceLoaded(frame);
  await Promise.resolve();
  assert.equal(events.length, 2);
});

test("reutilizarea Workbench cere identitate, rută și generație Canvas exact curente", async () => {
  const { state, surface } = workspace({ templateWorkbenchActive: true });
  const identity = {
    projectRoot: "/project",
    runtimeSessionId: "session:runtime-1",
    workspaceRevision: 7,
    transactionId: "canvas:7",
    previewRevision: "preview:7",
  };
  const previewUrl = "http://127.0.0.1:4000/__pana_workbench/template-active/?revision=7";
  const frame = { contentWindow: { postMessage() {} } };
  state.mountAndTrackSurface(frame);
  state.onSurfaceLoaded(frame);
  state.activeIdentity = identity;
  state.src = `${previewUrl}&__pana_reload=3`;
  state.activeUrl = previewUrl;
  state.documentMarkup = null;
  await Promise.resolve();

  assert.equal(state.canReuseCanonicalWorkbenchSurface(identity, previewUrl), true);

  state.pendingProjection = { phase: "prepared", identity };
  assert.equal(state.canReuseCanonicalWorkbenchSurface(identity, previewUrl), false);
  state.pendingProjection = null;

  surface.loadedGeneration -= 1;
  assert.equal(state.canReuseCanonicalWorkbenchSurface(identity, previewUrl), false);
  surface.loadedGeneration = surface.generation;

  assert.equal(
    state.canReuseCanonicalWorkbenchSurface(
      { ...identity, workspaceRevision: identity.workspaceRevision + 1 },
      previewUrl,
    ),
    false,
  );
  assert.equal(
    state.canReuseCanonicalWorkbenchSurface(identity, previewUrl.replace("template-active", "other")),
    false,
  );
});
