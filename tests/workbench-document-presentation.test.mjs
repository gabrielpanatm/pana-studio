import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import {
  activeWorkbenchDocument,
  workbenchPresentationForProjectFile,
} from "$lib/workbench/document-presentation";
import {
  availableProjectPaneTabs,
  reconcileProjectPaneTab,
} from "$lib/workbench/project-pane-policy";
import { WorkbenchWorkspaceState } from "$lib/workbench/workspace-state.svelte";

function snapshot({ presentation, surface, split = "none" }) {
  return {
    schemaVersion: 2,
    projectRoot: "/project",
    projectSessionId: "project-session",
    runtimeSessionId: "runtime-session",
    revision: 1,
    activeActivity: "editor",
    activeGroupId: "primary",
    split,
    splitRatioBasisPoints: 5_000,
    canvasViewport: {
      mode: "fit",
      preset: "desktop",
      widthPx: 1_440,
      zoomPercent: 100,
      showRulers: true,
    },
    groups: [{
      groupId: "primary",
      activeDocumentId: "project:document",
      documents: [{
        documentId: "project:document",
        relativePath: presentation === "html" ? "templates/index.html" : "sass/site.scss",
        title: presentation === "html" ? "index.html" : "site.scss",
        presentation,
        surface,
        pinned: false,
      }],
    }],
    bottomPanel: { open: false, activeView: "terminal" },
    contentWorkspace: { mode: "list", pagePath: null },
    selectedProjectEntry: null,
  };
}

test("ProjectFile.kind proiectează capabilitatea fără euristică de extensie", () => {
  assert.equal(workbenchPresentationForProjectFile({ kind: "HTML" }), "html");
  assert.equal(workbenchPresentationForProjectFile({ kind: "SCSS" }), "code_only");
  assert.equal(workbenchPresentationForProjectFile({ kind: "OTHER" }), "code_only");
});

test("fila Fișiere este forțată pentru code-only și rămâne aleasă la revenirea în HTML", () => {
  assert.deepEqual(availableProjectPaneTabs(false), ["files"]);
  assert.equal(reconcileProjectPaneTab("layers", false), "files");

  assert.deepEqual(availableProjectPaneTabs(true), ["layers", "files"]);
  assert.equal(reconcileProjectPaneTab("files", true), "files");
  assert.equal(reconcileProjectPaneTab("layers", true), "layers");
});

test("publicarea snapshotului code-only proiectează codul în aceeași tranzacție frontend", () => {
  const projections = [];
  const centerViews = [];
  const state = new WorkbenchWorkspaceState({
    authority: () => ({
      projectRoot: "/project",
      runtimeSessionId: "runtime-session",
      project: null,
      activeRelativePath: "sass/site.scss",
      centerView: "preview",
    }),
    flushDrafts: async () => {},
    loadProjectFile: async () => {},
    setCenterView: (view) => centerViews.push(view),
    projectActiveDocument: (document, previous) => projections.push({ document, previous }),
    synchronizeTerminalPane: () => {},
    clearStatus: () => {},
    escalateStatus: () => {},
  });

  state.hydrateBootstrap(snapshot({ presentation: "code_only", surface: "code" }));

  assert.equal(state.activeDocumentPresentation, "code_only");
  assert.equal(activeWorkbenchDocument(state.snapshot)?.surface, "code");
  assert.equal(projections.length, 1);
  assert.equal(projections[0].document.presentation, "code_only");
  assert.deepEqual(centerViews, ["code"]);
});

test("controalele incompatibile și fallback-ul legacy lipsesc din componente", async () => {
  const [bar, pane, shell, lifecycle] = await Promise.all([
    readFile(new URL("../src/lib/components/workbench/DocumentBar.svelte", import.meta.url), "utf8"),
    readFile(new URL("../src/lib/components/ProjectPane.svelte", import.meta.url), "utf8"),
    readFile(new URL("../src/lib/components/EditorShell.svelte", import.meta.url), "utf8"),
    readFile(new URL("../src/lib/editor/lifecycle.svelte.ts", import.meta.url), "utf8"),
  ]);

  assert.match(bar, /\{#if visualPresentationAvailable\}[\s\S]*class="layout-switcher"/);
  assert.match(pane, /availableProjectPaneTabs\(layersAvailable\)/);
  assert.match(pane, /reconcileProjectPaneTab\(projectPaneTab, layersAvailable\)/);
  assert.match(pane, /class="ui-button primary pane-add-element-btn"[\s\S]*?disabled=\{!layersAvailable\}/);
  assert.match(pane, /id="project-pane-tab-layers"[\s\S]*?disabled=\{!layersAvailable\}/);
  assert.doesNotMatch(pane, /\{#if layersAvailable\}\s*<button[\s\S]{0,300}pane-add-element-btn/);
  assert.doesNotMatch(pane, /\{#if layersAvailable\}\s*<button[\s\S]{0,300}project-pane-tab-layers/);
  assert.match(pane, /\{#if layersAvailable && projectPaneTab === "layers"\}/);
  assert.match(shell, /showPreview = visualPresentationAvailable &&/);
  assert.match(shell, /showSource = !visualPresentationAvailable \|\|/);
  assert.doesNotMatch(lifecycle, /canPreviewCurrentSource/);
});
