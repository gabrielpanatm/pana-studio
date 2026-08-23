import assert from "node:assert/strict";
import { test } from "node:test";
import { resetProjectScopedState } from "$lib/state/project-session-reset";

function host() {
  const events = [];
  return {
    events,
    source: "old source",
    sourceCache: { "scanned:old.md": "old source" },
    activeScannedPath: "old.md",
    sourceGraph: { nodes: [] },
    sourceGraphProjectionStatus: "current",
    sourceGraphWorkspaceRevision: 7,
    scssVariables: [{ name: "$color", value: "red" }],
    targetCssFile: "sass/main.scss",
    previewSrc: "http://preview/",
    activePreviewPath: "content/_index.md",
    browserPreviewRoute: "/old/",
    previewDocumentMarkup: "<main>old</main>",
    previewWorkspaceRevision: "preview-7",
    activeVersionPreview: { id: "version" },
    templateWorkbenchPlan: { activeTemplate: { file: "templates/index.html" } },
    templateWorkbenchPreferredPagePath: "content/_index.md",
    templateWorkbenchPreferredRoute: "/",
    templateWorkbenchActive: true,
    templateWorkbenchTarget: "templates/index.html",
    templateWorkbenchReturnPreviewPath: "content/_index.md",
    templateWorkbenchRequestSerial: 4,
    overrideRules: { old: true },
    variableOverrides: { old: "value" },
    htmlPending: {
      tag: true,
      attributes: true,
      text: true,
      image: true,
      classes: true,
      structure: true,
    },
    inspectorPending: { html: true, css: true, js: true },
    pendingTag: "section",
    pendingTagOriginal: "div",
    pendingTagSourceLocation: { path: "old.html", line: 1, column: 1 },
    tagStatus: "pending",
    projectWorkspaceSnapshot: { revision: 7 },
    workbenchSnapshot: { revision: 7 },
    fileExplorerSnapshot: { revision: 7 },
    fileExplorerLoading: true,
    fileExplorerError: "old error",
    publishWorkspace: {
      cachebustAssets: true,
      invalidate() { events.push("publish-invalidate"); },
    },
    diskState: { baseline: { root: "/project", files: [] } },
    kernelProjectSessionId: "session:runtime",
    refreshToken: 9,
    editorSelection: {
      reset() { events.push("editor-reset"); },
    },
    clearPreviewSelection(options) { events.push(["selection-clear", options]); },
    resetControlledPreviewState() { events.push("preview-reset"); },
    resetPageSections() { events.push("sections-reset"); },
    resetInspectorPendingSources() { events.push("inspector-sources-reset"); },
    cancelPendingHtmlMutations() { events.push("html-cancel"); },
    resetExternalDiskState() { events.push("external-reset"); },
    setGlobalStatus(text, kind) { events.push(["status", text, kind]); },
    setSessionProjectRoot(root = "") {
      events.push(["root", root]);
      this.sessionProjectRoot = root;
    },
    sessionProjectRoot: "/project",
  };
}

test("resetarea proiectului golește determinist toate proiecțiile sesiunii", () => {
  const current = host();
  resetProjectScopedState(current);

  assert.equal(current.source, "");
  assert.deepEqual(current.sourceCache, {});
  assert.equal(current.activeScannedPath, null);
  assert.equal(current.sourceGraph, null);
  assert.equal(current.sourceGraphProjectionStatus, "deferred");
  assert.equal(current.sourceGraphWorkspaceRevision, null);
  assert.equal(current.previewSrc, "about:blank");
  assert.equal(current.activePreviewPath, "about:blank");
  assert.equal(current.browserPreviewRoute, "/");
  assert.equal(current.previewDocumentMarkup, null);
  assert.equal(current.previewWorkspaceRevision, null);
  assert.equal(current.projectWorkspaceSnapshot, null);
  assert.equal(current.workbenchSnapshot, null);
  assert.equal(current.fileExplorerSnapshot, null);
  assert.equal(current.fileExplorerLoading, false);
  assert.equal(current.fileExplorerError, "");
  assert.deepEqual(current.scssVariables, []);
  assert.equal(current.targetCssFile, "styles.css");
  assert.equal(current.templateWorkbenchActive, false);
  assert.equal(current.templateWorkbenchTarget, null);
  assert.equal(current.templateWorkbenchPlan, null);
  assert.equal(current.templateWorkbenchRequestSerial, 5);
  assert.deepEqual(current.overrideRules, {});
  assert.deepEqual(current.variableOverrides, {});
  assert.deepEqual(current.htmlPending, {
    tag: false,
    attributes: false,
    text: false,
    image: false,
    classes: false,
    structure: false,
  });
  assert.deepEqual(current.inspectorPending, { html: false, css: false, js: false });
  assert.equal(current.pendingTag, null);
  assert.equal(current.pendingTagOriginal, null);
  assert.equal(current.pendingTagSourceLocation, null);
  assert.equal(current.tagStatus, "");
  assert.equal(current.kernelProjectSessionId, "");
  assert.equal(current.sessionProjectRoot, "");
  assert.equal(current.publishWorkspace.cachebustAssets, false);
  assert.equal(current.events.filter((event) => event === "publish-invalidate").length, 1);
  assert.equal(current.activeVersionPreview, null);
  assert.equal(current.refreshToken, 9);
  assert.equal(current.events.filter((event) => event === "external-reset").length, 1);
  assert.equal(current.events.includes("html-cancel"), false);
});

test("resetarea de close invalidează istoricul o singură dată", () => {
  const current = host();
  resetProjectScopedState(current, { invalidateHistory: true });

  assert.equal(current.refreshToken, 10);
  assert.equal(current.events.filter((event) => event === "html-cancel").length, 1);
  assert.equal(current.events.filter((event) => event === "editor-reset").length, 1);
  assert.equal(
    current.events.filter((event) => Array.isArray(event) && event[0] === "selection-clear").length,
    1,
  );
});

test("atașarea păstrează bariera externă până când noua sesiune este publicată", () => {
  const current = host();
  resetProjectScopedState(current, { preserveExternalReconcileBarrier: true });

  assert.equal(current.events.includes("external-reset"), false);
  assert.equal(current.source, "");
  assert.equal(current.kernelProjectSessionId, "");
});
