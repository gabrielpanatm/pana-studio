import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("EditorShell rămâne un owner unic și stabil pentru ProjectSession", () => {
  const center = source("../src/lib/components/workspace/WorkspaceCenterArea.svelte");
  const shell = source("../src/lib/components/EditorShell.svelte");
  const effects = source("../src/lib/state/app-effects.svelte.ts");
  const workspaceCss = source("../src/routes/workspace-shell.css");

  assert.equal(center.match(/<EditorShell\b/g)?.length, 1);
  assert.match(
    center,
    /editorSurfaceActive\s*=\s*\$derived\([\s\S]*app\.applicationSurface === "workbench"[\s\S]*activeWorkbenchActivity === "editor"[\s\S]*app\.centerView !== "kernel"/,
  );
  assert.match(
    center,
    /\{#if app\.scannedProject && app\.kernelProjectSessionId\}[\s\S]*\{#key app\.kernelProjectSessionId\}[\s\S]*class="stable-editor-surface"[\s\S]*class:surface-inactive=\{!editorSurfaceActive\}[\s\S]*inert=\{!editorSurfaceActive \? true : undefined\}[\s\S]*aria-hidden=\{!editorSurfaceActive\}[\s\S]*surfaceActive=\{editorSurfaceActive\}/,
  );
  assert.ok(
    center.indexOf("<EditorShell") < center.indexOf("{#if retainedAuxiliarySurface}"),
    "EditorShell trebuie să fie în afara owner-ului workspace-ului auxiliar",
  );
  assert.doesNotMatch(center, /\{:else\}\s*<EditorShell/);
  assert.match(
    shell,
    /function registerPreviewSurface[\s\S]*mountPreviewSurface\(frame\)[\s\S]*destroy\(\)[\s\S]*unmountPreviewSurface\(frame\)/,
  );
  assert.match(shell, /<DocumentBar[\s\S]*active=\{surfaceActive\}/);
  assert.match(shell, /sourceIsLoading[\s\S]*class="code-loading-stage"/);
  assert.match(
    effects,
    /app\.source === SOURCE_LOADING_SENTINEL[\s\S]*app\.codeEditorController\.setDoc\(app\.source\)/,
  );
  assert.match(
    workspaceCss,
    /\.stable-editor-surface\s*\{[\s\S]*position:\s*absolute[\s\S]*inset:\s*0/,
  );
  assert.match(
    workspaceCss,
    /\.stable-editor-surface\.surface-inactive,\s*[\s\S]*\.workspace-auxiliary-overlay\.surface-inactive\s*\{[\s\S]*visibility:\s*hidden[\s\S]*pointer-events:\s*none/,
  );
});

test("ultimul workspace auxiliar rămâne montat numai cât Editorul este activ", () => {
  const center = source("../src/lib/components/workspace/WorkspaceCenterArea.svelte");

  assert.match(
    center,
    /type RetainedAuxiliarySurface[\s\S]*Exclude<WorkbenchActivity, "editor">[\s\S]*"settings"[\s\S]*"kernel"/,
  );
  assert.match(
    center,
    /if \(retainedAuxiliarySessionId !== sessionId\)[\s\S]*retainedAuxiliarySurface = null/,
  );
  assert.match(
    center,
    /activeWorkbenchActivity !== "editor"[\s\S]*retainedAuxiliarySurface = activeWorkbenchActivity/,
  );
  assert.match(
    center,
    /<\/section>\s*\{#if retainedAuxiliarySurface\}[\s\S]*class="workspace-auxiliary-overlay"[\s\S]*class:surface-inactive=\{editorSurfaceActive\}[\s\S]*inert=\{editorSurfaceActive \? true : undefined\}[\s\S]*aria-hidden=\{editorSurfaceActive\}/,
  );
  assert.equal(center.match(/<ThemesWorkspace\b/g)?.length, 1);
});

test("workspace-urile non-Editor sunt chunk-uri lazy încărcate la prima activare", () => {
  const center = source("../src/lib/components/workspace/WorkspaceCenterArea.svelte");
  const lazyWorkspaces = [
    ["settings", "$lib/components/settings/SettingsWorkspace.svelte"],
    ["themes", "$lib/components/themes/ThemesWorkspace.svelte"],
    ["templates", "$lib/components/templates/TemplatesWorkspace.svelte"],
    ["components", "$lib/components/creation/ComponentsWorkspace.svelte"],
    ["blocks", "$lib/components/creation/BlocksWorkspace.svelte"],
    ["design_system", "$lib/components/creation/DesignSystemWorkspace.svelte"],
    ["assets", "$lib/components/creation/AssetsWorkspace.svelte"],
    ["content", "$lib/components/content/ContentWorkspace.svelte"],
    ["taxonomies", "$lib/components/taxonomies/TaxonomiesWorkspace.svelte"],
    ["data", "$lib/components/data/DataWorkspace.svelte"],
    ["versioning", "$lib/components/versioning/VersionControlWorkspace.svelte"],
    ["publish", "$lib/components/publish/PublishWorkspace.svelte"],
    ["audit", "$lib/components/audit/AuditWorkspace.svelte"],
    ["kernel", "$lib/components/kernel/KernelWorkspace.svelte"],
  ];

  assert.match(center, /import EditorShell from "\$lib\/components\/EditorShell\.svelte"/);
  assert.match(center, /auxiliaryWorkspaceLoads = new Map/);
  assert.match(
    center,
    /if \(retainedAuxiliarySurface\)[\s\S]*ensureAuxiliaryWorkspaceLoaded\(retainedAuxiliarySurface\)/,
  );
  for (const [surface, modulePath] of lazyWorkspaces) {
    const escapedPath = modulePath.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    assert.doesNotMatch(
      center,
      new RegExp(`import [A-Z][A-Za-z]+ from "${escapedPath}"`),
      `${surface} nu trebuie inclus static în chunk-ul Editorului`,
    );
    assert.match(
      center,
      new RegExp(`import\\(\\s*"${escapedPath}"\\s*\\)`),
      `${surface} trebuie să aibă un import dinamic dedicat`,
    );
  }
});

test("Sistemul de design încarcă numai catalogul cerut de subvederea activă", () => {
  const design = source("../src/lib/components/creation/DesignSystemWorkspace.svelte");

  assert.match(design, /let activeView = \$state<DesignView>\("global-styles"\)/);
  assert.match(
    design,
    /const view = activeView;[\s\S]*view === "global-styles"[\s\S]*reloadThemeStyleCatalog\(\)[\s\S]*view === "tokens"[\s\S]*reloadDesignTokenCatalog\(\)[\s\S]*view === "classes"[\s\S]*refreshDesignClassInventory\(\)[\s\S]*view === "fonts"[\s\S]*reloadFontManager\(\)/,
  );
  assert.equal(
    design.match(/const view = activeView;/g)?.length,
    1,
  );
  assert.doesNotMatch(
    design,
    /\$effect\(\(\) => \{\s*void (?:reloadThemeStyleCatalog|reloadDesignTokenCatalog|reloadFontManager)\(\)/,
  );
});

test("suprafața ascunsă suspendă observerul și canalul Canvas fără a distruge iframe-ul", () => {
  const documentBar = source("../src/lib/components/workbench/DocumentBar.svelte");
  const effects = source("../src/lib/state/app-effects.svelte.ts");
  const codeEditor = source("../src/lib/editor/controller.ts");
  const interaction = source("../src/lib/state/canvas-interaction-controller.ts");
  const rustCommand = source("../src-tauri/src/commands/editor_navigation.rs");

  assert.match(
    documentBar,
    /\$effect\(\(\) => \{[\s\S]*if \(!active \|\| !documentTabsElement[\s\S]*new ResizeObserver\(updateDocumentScrollCues\)[\s\S]*resizeObserver\.disconnect\(\)/,
  );
  assert.match(
    effects,
    /app\.applicationSurface;[\s\S]*app\.workbenchSnapshot\?\.activeActivity;[\s\S]*app\.centerView;[\s\S]*synchronizeCanvasInteractionBinding\(app\)/,
  );
  assert.match(
    effects,
    /app\.activeScannedPath;[\s\S]*app\.editorNavigationSnapshot\?\.focusedView\?\.activeDocumentPath;[\s\S]*synchronizeCanvasInteractionBinding\(app\)/,
  );
  assert.match(
    effects,
    /!app\.codeEditorController\.ownsHost\(codeEditorHost\)[\s\S]*codeEditorController\?\.destroy\(\)[\s\S]*activeActivity !== "editor"[\s\S]*return;[\s\S]*codeEditorController\.requestMeasure\(\)/,
  );
  assert.match(
    codeEditor,
    /ownsHost:\s*\(host: HTMLDivElement\)[\s\S]*requestMeasure:\s*\(\)[\s\S]*host === options\.host[\s\S]*view\.requestMeasure\(\)/,
  );
  assert.match(
    interaction,
    /function canvasInteractionSurfaceActive\(app: AppState\)[\s\S]*app\.applicationSurface === "workbench"[\s\S]*activeActivity \?\? "editor"\) === "editor"[\s\S]*app\.centerView !== "kernel"/,
  );
  assert.match(
    interaction,
    /if \(!canvasInteractionSurfaceActive\(app\)\)[\s\S]*const retainedBinding = runtime\.binding \?\? runtime\.pendingBinding[\s\S]*runtime\.phase = "suspended"[\s\S]*deactivateCanvasAgent\(app, runtime\)/,
  );
  assert.match(
    interaction,
    /runtime\.phase === "suspended"[\s\S]*reactivateRetainedCanvasAgent\(app, runtime, runtime\.binding\)/,
  );
  assert.match(
    interaction,
    /function reactivateRetainedCanvasAgent[\s\S]*activate-canvas-interaction-agent[\s\S]*lastAcceptedSequence/,
  );
  assert.match(
    interaction,
    /interactionGeneration \+= 1[\s\S]*generation !== runtime\.interactionGeneration/,
  );
  assert.match(
    rustCommand,
    /pub async fn bind_canvas_interaction_agent[\s\S]*spawn_blocking[\s\S]*resolve_editor_navigation_context/,
  );
});

test("activitatea Workbench primește receipt Rust înaintea proiecției persistente", () => {
  const command = source("../src-tauri/src/commands/workbench.rs");
  const storage = source("../src-tauri/src/kernel/workbench/storage.rs");

  assert.match(
    command,
    /matches!\(&intent, WorkbenchIntent::SetActivity[\s\S]*workbench\.apply\(&session, &identity, intent\)[\s\S]*spawn_blocking[\s\S]*persist_latest_workbench/,
  );
  assert.match(
    storage,
    /static WORKBENCH_PERSISTENCE_LOCK:\s*Mutex<\(\)>/,
  );
  assert.match(
    storage,
    /persist_latest_workbench[\s\S]*persisted\.revision > snapshot\.revision[\s\S]*persist_workbench_unlocked/,
  );
});

test("sidebarele Editor păstrează un owner stabil pe durata ProjectSession", () => {
  const projectArea = source("../src/lib/components/workspace/WorkspaceProjectArea.svelte");
  const inspectorArea = source("../src/lib/components/workspace/WorkspaceInspectorArea.svelte");
  const workspaceCss = source("../src/routes/workspace-shell.css");

  for (const area of [projectArea, inspectorArea]) {
    assert.match(
      area,
      /editorSidebarActive\s*=\s*\$derived\([\s\S]*applicationSurface === "workbench"[\s\S]*activeActivity \?\? "editor"\) === "editor"/,
    );
    assert.match(
      area,
      /\{#if app\.scannedProject && app\.kernelProjectSessionId\}[\s\S]*\{#key app\.kernelProjectSessionId\}/,
    );
  }
  assert.match(
    projectArea,
    /class="project-pane-shell"[\s\S]*hidden=\{app\.leftPaneCollapsed\}[\s\S]*inert=\{!editorSidebarActive[\s\S]*app\.leftPaneCollapsed[\s\S]*app\.kernelUndoRedoFrontendQuiesceActive[\s\S]*app\.kernelUndoRedoFrontendLeaseActive[\s\S]*<ProjectPane/,
  );
  assert.match(
    inspectorArea,
    /class="inspector-pane-shell"[\s\S]*hidden=\{app\.rightPaneCollapsed\}[\s\S]*inert=\{!editorSidebarActive[\s\S]*app\.rightPaneCollapsed[\s\S]*app\.kernelUndoRedoFrontendQuiesceActive[\s\S]*app\.kernelUndoRedoFrontendLeaseActive[\s\S]*<InspectorPane/,
  );
  assert.match(
    workspaceCss,
    /\.project-pane-shell\[hidden\],\s*[\s\S]*\.inspector-pane-shell\[hidden\]\s*\{[\s\S]*display:\s*none/,
  );
  assert.match(
    workspaceCss,
    /\.workspace-auxiliary-overlay\s*\{[\s\S]*position:\s*absolute[\s\S]*z-index:\s*4[\s\S]*inset:\s*6px 8px 7px/,
  );
});
