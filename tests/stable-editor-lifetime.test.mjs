import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("EditorShell rămâne un owner unic și stabil pentru ProjectSession", () => {
  const center = source("../src/lib/components/workspace/WorkspaceCenterArea.svelte");
  const workspace = source("../src/lib/components/application/ApplicationWorkspace.svelte");
  const shell = source("../src/lib/components/EditorShell.svelte");
  const documentBar = source("../src/lib/components/workbench/DocumentBar.svelte");
  const effects = source("../src/lib/editor/lifecycle.svelte.ts");
  const workspaceCss = source("../src/routes/workspace-shell.css");

  assert.equal(center.match(/<EditorShell\b/g)?.length, 1);
  assert.match(
    center,
    /editorSurfaceActive\s*=\s*\$derived\([\s\S]*session\.applicationSurface === "workbench"[\s\S]*activeWorkbenchActivity === "editor"[\s\S]*session\.centerView !== "kernel"/,
  );
  assert.match(
    center,
    /\{#if session\.project && session\.sessionId\}[\s\S]*\{#key session\.sessionId\}[\s\S]*class="stable-editor-surface"[\s\S]*class:surface-inactive=\{!editorSurfaceActive\}[\s\S]*inert=\{!editorSurfaceActive \? true : undefined\}[\s\S]*aria-hidden=\{!editorSurfaceActive\}[\s\S]*surfaceActive=\{editorSurfaceActive\}/,
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
  assert.match(shell, /<DocumentBar[\s\S]*active=\{surfaceActive\}[\s\S]*\{documentActivation\}/);
  assert.match(documentBar, /requestDocumentActivation/);
  assert.match(workspace, /documentActivation=\{workbench\.documentActivation\}/);
  assert.doesNotMatch(workspace, /get documentActivation\(\)/);
  assert.match(center, /<EditorShell[\s\S]*\{documentActivation\}/);
  assert.match(shell, /sourceIsLoading[\s\S]*class="code-loading-stage"/);
  assert.match(
    effects,
    /source\.source === SOURCE_LOADING_SENTINEL[\s\S]*source\.controller\.setDoc\(source\.source\)/,
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
  assert.doesNotMatch(center, /ThemesWorkspace|components\/themes/);
});

test("workspace-urile non-Editor sunt chunk-uri lazy încărcate la prima activare", () => {
  const center = source("../src/lib/components/workspace/WorkspaceCenterArea.svelte");
  const lazyWorkspaces = [
    ["settings", "$lib/components/settings/SettingsWorkspace.svelte"],
    ["templates", "$lib/components/templates/TemplatesWorkspace.svelte"],
    ["components", "$lib/components/creation/ComponentsWorkspace.svelte"],
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
    /const view = activeView;[\s\S]*view === "global-styles"[\s\S]*themeStyleCatalog\.refresh\(\)[\s\S]*view === "tokens"[\s\S]*tokenCatalog\.refresh\(\)[\s\S]*view === "classes"[\s\S]*commands\.refreshClassInventory\(\)[\s\S]*view === "fonts"[\s\S]*fontManager\.refresh\(\)/,
  );
  assert.equal(
    design.match(/const view = activeView;/g)?.length,
    1,
  );
  assert.doesNotMatch(
    design,
    /\$effect\(\(\) => \{\s*void (?:themeStyleCatalog|tokenCatalog|fontManager)\.refresh\(\)/,
  );
});

test("suprafața ascunsă suspendă observerul și canalul Canvas fără a distruge iframe-ul", () => {
  const documentBar = source("../src/lib/components/workbench/DocumentBar.svelte");
  const effects = `${source("../src/lib/canvas/interaction-lifecycle.svelte.ts")}\n${source("../src/lib/editor/lifecycle.svelte.ts")}`;
  const codeEditor = source("../src/lib/editor/controller.ts");
  const interaction = `${source("../src/lib/state/canvas-interaction-session.ts")}\n${source("../src/lib/state/canvas-interaction-runtime.ts")}`;
  const rustCommand = source("../src-tauri/src/commands/editor_navigation.rs");

  assert.match(
    documentBar,
    /\$effect\(\(\) => \{[\s\S]*if \(!active \|\| !documentTabsElement[\s\S]*new ResizeObserver\(\(\) => scheduleDocumentLayout\(\)\)[\s\S]*resizeObserver\.disconnect\(\)/,
  );
  assert.match(
    effects,
    /app\.session\.applicationSurface;[\s\S]*app\.session\.workbenchSnapshot\?\.activeActivity;[\s\S]*app\.session\.centerView;[\s\S]*synchronizeCanvasInteractionBinding\(app\)/,
  );
  assert.match(
    effects,
    /app\.session\.activeScannedPath;[\s\S]*app\.selection\.editorSelection\.navigationSnapshot\?\.focusedView\?\.activeDocumentPath;[\s\S]*synchronizeCanvasInteractionBinding\(app\)/,
  );
  assert.match(
    effects,
    /!source\.controller\.ownsHost\(codeEditorHost\)[\s\S]*source\.controller\.destroy\(\)[\s\S]*activeActivity !== "editor"[\s\S]*return;[\s\S]*source\.controller\.requestMeasure\(\)/,
  );
  assert.match(
    codeEditor,
    /ownsHost:\s*\(host: HTMLDivElement\)[\s\S]*requestMeasure:\s*\(\)[\s\S]*host === options\.host[\s\S]*view\.requestMeasure\(\)/,
  );
  assert.match(
    interaction,
    /function canvasInteractionSurfaceActive\(app: CanvasInteractionControllerHost\)[\s\S]*app\.session\.applicationSurface === "workbench"[\s\S]*activeActivity \?\? "editor"\) === "editor"[\s\S]*app\.session\.centerView !== "kernel"/,
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
    rustCommand,
    /pub async fn bind_canvas_interaction_agent[\s\S]*spawn_blocking[\s\S]*resolve_editor_navigation_context/,
  );
});

test("navigarea Workbench primește receipt Rust înaintea proiecției persistente", () => {
  const command = source("../src-tauri/src/commands/workbench.rs");
  const persistence = source(
    "../src-tauri/src/kernel/workbench/projection_persistence.rs",
  );
  const storage = source("../src-tauri/src/kernel/workbench/storage.rs");

  assert.match(
    command,
    /intent_uses_projection_write_behind\(&intent\)[\s\S]*workbench\s*\.\s*apply\(&session, &identity, intent\)[\s\S]*workbench_projection_persistence\s*\.\s*schedule/,
  );
  assert.match(
    command,
    /fn intent_uses_projection_write_behind[\s\S]*WorkbenchIntent::SetActivity[\s\S]*WorkbenchIntent::ActivateDocument/,
  );
  assert.match(
    persistence,
    /PROJECTION_PERSISTENCE_QUIET_PERIOD[\s\S]*Duration::from_millis\(250\)/,
  );
  assert.match(
    persistence,
    /DebouncedLatest[\s\S]*pending = Some\(value\)[\s\S]*worker_running[\s\S]*spawn_projection_persistence_worker/,
  );
  assert.match(
    storage,
    /static WORKBENCH_PERSISTENCE_LOCK:\s*Mutex<\(\)>/,
  );
  assert.match(
    storage,
    /persist_latest_workbench[\s\S]*persisted\.revision >= snapshot\.revision[\s\S]*persist_workbench_unlocked/,
  );
});

test("sidebarele Editor păstrează un owner stabil pe durata ProjectSession", () => {
  const projectArea = source("../src/lib/components/workspace/WorkspaceProjectArea.svelte");
  const inspectorArea = source("../src/lib/components/workspace/WorkspaceInspectorArea.svelte");
  const workspaceCss = source("../src/routes/workspace-shell.css");

  for (const area of [projectArea, inspectorArea]) {
    assert.match(
      area,
      /editorSidebarActive\s*=\s*\$derived\(visible\)/,
    );
    assert.match(
      area,
      /\{#if (?:pane\.projectRoot|workspaceMutations\.snapshot) && sessionId\}[\s\S]*\{#key sessionId\}/,
    );
  }
  assert.match(
    projectArea,
    /class="project-pane-shell"[\s\S]*hidden=\{workspaceLayout\.leftPaneCollapsed\}[\s\S]*inert=\{!editorSidebarActive[\s\S]*workspaceLayout\.leftPaneCollapsed[\s\S]*interactionLocked[\s\S]*<ProjectPane/,
  );
  assert.match(
    inspectorArea,
    /class="inspector-pane-shell"[\s\S]*hidden=\{workspaceLayout\.rightPaneCollapsed\}[\s\S]*inert=\{!editorSidebarActive[\s\S]*workspaceLayout\.rightPaneCollapsed[\s\S]*interactionLocked[\s\S]*<InspectorPane/,
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
