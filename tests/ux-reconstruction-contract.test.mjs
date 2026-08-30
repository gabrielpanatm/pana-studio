import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { test } from "node:test";
import { UI_TERM_IDS } from "$lib/i18n/ui-terms";

function filesBelow(directory, extensionPattern) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const url = new URL(`${entry.name}${entry.isDirectory() ? "/" : ""}`, directory);
    if (entry.isDirectory()) return filesBelow(url, extensionPattern);
    return extensionPattern.test(entry.name) ? [url] : [];
  });
}

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("bara separă statusul global din stânga de sursa selecției din dreapta", () => {
  const application = source("../src/lib/components/application/ApplicationWorkspace.svelte");
  const status = source("../src/lib/components/StatusBar.svelte");
  const editor = source("../src/lib/components/EditorShell.svelte");
  const session = source("../src/lib/state/app-session-controller.ts");

  assert.doesNotMatch(application + status, /AiEditAuthorityIndicator/);
  assert.doesNotMatch(
    status,
    /status-right|preview-chip|canvasPatchPerformance|controlledPreview/,
  );
  assert.match(status, /globalStatus\?:\s*GlobalStatusEvent/);
  assert.match(status, /sourceStatus\?:\s*WorkbenchSourceStatus/);
  assert.match(status, /class="selection-source"/);
  assert.match(status, /role="status"[\s\S]*aria-live="polite"/);
  assert.match(status, /\.status-bar\s*\{[\s\S]*height:\s*26px;/);
  assert.match(status, /\.selection-source\s*\{[\s\S]*height:\s*20px;/);
  assert.equal((editor.match(/class="editor-context-bar"/g) ?? []).length, 0);
  assert.doesNotMatch(editor, /workbench-banner|design-safe-banner/);

  const setter = session.slice(
    session.indexOf("export function setGlobalStatus"),
    session.indexOf("export function escalateGlobalStatus"),
  );
  assert.doesNotMatch(
    setter,
    /escalateGlobalStatus\(/,
    "o eroare pasivă nu devine automat notificare persistentă",
  );
});

test("glosarul românesc elimină etichetele legacy din suprafețele vizibile", () => {
  assert.equal(UI_TERM_IDS.projectSettings, "workbench-project-settings");
  assert.equal(UI_TERM_IDS.designSystem, "workbench-design-system");
  assert.equal(UI_TERM_IDS.problemsAudit, "workbench-audit");
  assert.equal(UI_TERM_IDS.safeEditing, "workbench-safe-editing");

  const svelteFiles = filesBelow(new URL("../src/lib/components/", import.meta.url), /\.svelte$/);
  const visibleSources = svelteFiles.map((url) => readFileSync(url, "utf8")
    .replace(/<script[\s\S]*?<\/script>/g, "")
    .replace(/<style[\s\S]*?<\/style>/g, ""));
  for (const phrase of [
    "History snapshots",
    "Template Workbench",
    "Design Safe",
    "Interactive Preview",
    "Run extern",
    "Website Builder",
    "Problems & Audit",
    "New Tab",
    "Release workspace",
    "Build & deploy",
    "Open Project",
    "Disk Conflict Snapshot",
    "Timeline step inspector",
    "Design workspace",
    "Preflight release",
    "Light UI",
    "Dark UI",
  ]) {
    assert.equal(
      visibleSources.some((markup) => markup.includes(phrase)),
      false,
      `eticheta vizibilă legacy trebuie eliminată: ${phrase}`,
    );
  }
  for (const word of ["Settings", "Save"] ) {
    const exactVisible = new RegExp(`(?:>\\s*${word}\\s*<|["']${word}["'])`);
    assert.equal(visibleSources.some((markup) => exactVisible.test(markup)), false, `eticheta ${word} nu este localizată`);
  }

  const rustCommands = source("../src-tauri/src/kernel/command_center/search.rs");
  assert.doesNotMatch(rustCommands, /"(?:Design System|Asset-uri|Problems & Audit|Arată Problems|Arată Output|Arată Timeline)"/);

  const htmlPalette = source("../src/lib/html/palette.ts");
  const teraPalette = source("../src/lib/tera/palette.ts");
  assert.doesNotMatch(htmlPalette, /label: "(?:Section|Article|Paragraph|Quote|Image|Source|Picture|Button|Form|Option|Table|Caption)"/);
  assert.doesNotMatch(teraPalette, /label: "(?:Extends|Block content|Include partial|Import macros|If|With|Set|Variable|Comment|Raw)"/);
});

test("panoul inferior este exclusiv Terminal, cu un singur toolbar compact", () => {
  const bottomPanel = source("../src/lib/components/workbench/WorkbenchBottomPanel.svelte");
  const terminal = source("../src/lib/components/TerminalPane.svelte");
  const commandCenter = source("../src/lib/application/command-center-service.svelte.ts");
  const workbenchState = source("../src/lib/workbench/workspace-state.svelte.ts");
  const projectBootstrap = source("../src-tauri/src/commands/project/bootstrap.rs");
  const audit = source("../src/lib/components/audit/AuditWorkspace.svelte");
  const kernel = source("../src/lib/components/kernel/KernelWorkspace.svelte");
  const observability = source("../src/lib/components/kernel/ObservabilityLogControl.svelte");
  const controller = source("../src/lib/terminal/controller.ts");
  const smoothWheel = source("../src/lib/ui/smooth-wheel.ts");
  const documentBar = source("../src/lib/components/workbench/DocumentBar.svelte");

  assert.match(bottomPanel, /<TerminalPaneComponent/);
  assert.doesNotMatch(bottomPanel, /Probleme|Jurnal|WorkbenchBottomPanelView|readKernelObservabilityLog/);
  assert.doesNotMatch(terminal, /terminal-task-button[\s\S]*<span>\{task\.label\}<\/span>/);
  assert.match(terminal, /grid-template-rows:\s*38px minmax\(0, 1fr\)/);
  assert.match(terminal, /\.terminal-body \{[^}]*padding:\s*0;/);
  assert.match(terminal, /\.terminal-host \{[^}]*width:\s*100%;[^}]*height:\s*100%;[^}]*border:\s*0;[^}]*border-radius:\s*0;/);
  assert.match(terminal, /:global\(\.terminal-host > \.xterm\) \{[^}]*width:\s*100%;[^}]*height:\s*100%;/);
  assert.match(terminal, /:global\(\.terminal-host \.xterm-viewport\) \{[^}]*background-color:\s*var\(--terminal-shell-bg/);
  assert.match(terminal, /\.terminal-scroll-proxy\) \{[^}]*top:\s*0;[^}]*bottom:\s*0;/);
  assert.doesNotMatch(terminal, /--terminal-shell-background/);

  assert.match(commandCenter, /case "show_problems": await d\.actions\.openAudit\("overview"\)/);
  assert.match(commandCenter, /case "show_output": await d\.actions\.openAudit\("runtime", true\)/);
  assert.doesNotMatch(commandCenter, /setWorkbenchBottomPanel\(true, "(?:problems|output)"\)/);
  assert.match(workbenchState, /activeView: WorkbenchBottomPanelView = "terminal"/);
  assert.match(
    projectBootstrap,
    /WorkbenchIntent::SetBottomPanel\s*\{[\s\S]*?open:\s*false,[\s\S]*?active_view:\s*WorkbenchBottomPanelView::Terminal/,
  );
  assert.match(audit, /observabilityFocusSerial/);
  assert.match(kernel, /focusToken=\{observabilityFocusSerial\}/);
  assert.match(observability, /scrollIntoView\(\{ block: "start", behavior: "smooth" \}\)/);
  assert.match(controller, /new ResizeObserver\(\(\) => \{[\s\S]*fitAddon\.fit\(\)/);
  assert.match(controller, /terminal\.onScroll\(\(\) => this\.scheduleScrollProxySync\(\)\)/);
  assert.match(controller, /terminal\.scrollToLine\(targetLine\)/);
  assert.match(controller, /smoothScrollDuration:/);
  assert.match(smoothWheel, /prefers-reduced-motion: reduce/);
  assert.match(smoothWheel, /requestAnimationFrame/);
  assert.match(smoothWheel, /event\.ctrlKey[\s\S]*event\.metaKey/);
  assert.doesNotMatch(documentBar, /handleDocumentTabsWheel|wheelAnimationFrame|onwheel=/);
  assert.match(documentBar, /overflow-x:\s*auto;[\s\S]*overflow-y:\s*hidden;/);
});

test("design-system.css este singura sursă de tokeni și expune primitivele comune", () => {
  const appHtml = source("../src/app.html");
  const shell = source("../src/routes/workspace-shell.css");
  const design = source("../src/routes/design-system.css");
  const projectPane = source("../src/lib/components/ProjectPane.svelte");
  const projectFiles = source("../src/lib/components/project/ProjectFilesTab.svelte");
  const navigationTree = source("../src/lib/components/project/EditorNavigationTree.svelte");
  const htmlPane = source("../src/lib/components/inspector/HtmlPane.svelte");
  const gridBuilder = source("../src/lib/components/inspector/controls/GridBuilder.svelte");
  const blockProperties = source("../src/lib/components/inspector/BlockPropertiesPane.svelte");
  const motionStudio = source("../src/lib/components/inspector/js/MotionStudioPanel.svelte");
  const aiSettings = source("../src/lib/components/settings/AiIntegrationPane.svelte");
  const storageSettings = source("../src/lib/components/settings/StoragePane.svelte");
  const colorPicker = source("../src/lib/components/ui/PanaColorPicker.svelte");
  const terminal = source("../src/lib/components/TerminalPane.svelte");
  const components = source("../src/lib/components/creation/ComponentsWorkspace.svelte");
  const dataWorkspace = source("../src/lib/components/data/DataWorkspace.svelte");

  assert.equal(existsSync(new URL("../static/app-shell.css", import.meta.url)), false);
  assert.doesNotMatch(appHtml, /app-shell\.css/);
  assert.match(shell, /@import "\.\/design-system\.css"/);
  assert.doesNotMatch(shell, /--(?:surface-base|brand|text|border-subtle)\s*:/);

  for (const primitive of ["ui-button", "ui-icon-button", "ui-close-button", "ui-tabs", "ui-tab", "ui-field", "ui-panel", "ui-card", "ui-badge", "ui-message"]) {
    assert.match(design, new RegExp(`\\.${primitive}(?:[\\s,{.:])`), `lipsește primitiva ${primitive}`);
  }
  assert.match(
    design,
    /button\.ui-icon-button\.ui-close-button\s*\{[^}]*width:\s*var\(--control-height-compact\);[^}]*height:\s*var\(--control-height-compact\);[^}]*min-height:\s*var\(--control-height-compact\);[^}]*border-radius:\s*var\(--radius-control\);/,
  );
  assert.match(design, /button\.ui-icon-button\.mini\s*\{[^}]*width:\s*20px;[^}]*height:\s*20px;[^}]*background:\s*transparent;/);
  assert.match(design, /button\.ui-icon-button\.mini :where\(svg\)\s*\{[^}]*width:\s*var\(--ui-mini-icon-size\);[^}]*height:\s*var\(--ui-mini-icon-size\);/);
  assert.match(design, /button\.ui-icon-button\.mini:is\(\.active, \[aria-pressed="true"\]\)/);
  assert.match(design, /button\.ui-icon-button\.mini\.danger:hover:not\(:disabled\)\s*\{[^}]*var\(--danger\)/);
  assert.match(projectPane, /ui-button/);
  assert.match(projectFiles, /class="ui-icon-button mini"/);
  assert.match(projectFiles, /class="ui-icon-button mini danger"/);
  assert.match(projectFiles, /class="file-row-btn ui-entity-trigger"/);
  assert.match(projectFiles, /t\("project-files-heading"\)/);
  assert.doesNotMatch(projectFiles, /project-files-explorer/);
  assert.doesNotMatch(projectFiles, /\.icon-action\s*\{/);
  assert.match(navigationTree, /class="delete-action ui-icon-button mini danger"/);
  assert.match(htmlPane, /class="ui-icon-button mini danger"/);
  assert.doesNotMatch(htmlPane, /\.cls-chip-del\s*\{/);
  assert.match(gridBuilder, /class="ui-icon-button mini danger"/);
  assert.match(blockProperties, /class="ui-icon-button mini"/);
  assert.match(motionStudio, /class="ui-icon-button mini danger"/);
  assert.match(aiSettings, /class="ui-icon-button mini"/);
  assert.match(storageSettings, /class="ui-icon-button mini"/);
  assert.match(colorPicker, /class="ui-icon-button compact"/);
  assert.match(terminal, /class="ui-icon-button compact quiet"/);
  assert.doesNotMatch(htmlPane, /\.(?:hf-add-btn|hf-del-btn)\s*\{/);
  assert.doesNotMatch(blockProperties, /\.panel-actions button\s*\{/);
  assert.doesNotMatch(aiSettings, /^\s*button\s*\{/m);
  assert.doesNotMatch(storageSettings, /\.icon-action\s*\{/);
  assert.doesNotMatch(colorPicker, /\.icon-button\s*\{/);
  assert.doesNotMatch(terminal, /\.terminal-(?:add|icon)-button/);
  assert.match(dataWorkspace, /class="ui-icon-button ui-close-button"/);
  for (const usage of ["SelectControl", "ui-field", "ui-message"]) assert.match(components, new RegExp(usage));
  assert.doesNotMatch(components, /ui-tabs|ui-tab/);

  const uniqueDarkSurfaces = [...design.matchAll(/--surface-(?:base|panel|raised):\s*([^;]+);/g)].map((match) => match[1]);
  assert.equal(new Set(uniqueDarkSurfaces.slice(0, 3)).size, 3);
  assert.match(design, /--surface-[1-9]:\s*var\(--surface-(?:base|panel|raised)\)/);
});

test("panourile Editor ocupă permanent coloana și au o singură umbră", () => {
  const shell = source("../src/routes/workspace-shell.css");
  const projectPane = source("../src/lib/components/ProjectPane.svelte");
  const inspectorPane = source("../src/lib/components/InspectorPane.svelte");
  const editorShell = source("../src/lib/components/EditorShell.svelte");
  const statusBar = source("../src/lib/components/StatusBar.svelte");

  assert.match(shell, /\.workspace\s*\{[^}]*align-items:\s*stretch/s);
  assert.match(shell, /\.project-pane-shell,[\s\S]*?\.inspector-pane-shell\s*\{[^}]*height:\s*100%[^}]*align-self:\s*stretch/s);
  assert.doesNotMatch(shell, /box-shadow:\s*var\(--shadow-workspace-panel\)/);
  assert.match(projectPane, /\.project-pane\s*\{[^}]*flex:\s*1\s+1\s+auto[^}]*height:\s*100%[^}]*box-shadow:\s*var\(--shadow-panel\)/s);
  assert.match(inspectorPane, /\.inspector-pane\s*\{[^}]*flex:\s*1\s+1\s+auto[^}]*height:\s*100%[^}]*box-shadow:\s*var\(--shadow-panel\)/s);
  assert.match(editorShell, /\.editor-shell\s*\{[^}]*height:\s*100%[^}]*box-shadow:\s*var\(--shadow-panel\)/s);
  assert.doesNotMatch(statusBar, /box-shadow:\s*0\s+-1px/);
});

test("important rămâne izolat la suprascrierile din documentul preview", () => {
  const frontendFiles = filesBelow(new URL("../src/", import.meta.url), /\.(?:css|svelte|ts)$/);
  const forcedCascadeFiles = frontendFiles.flatMap((url) => {
    const matches = readFileSync(url, "utf8").match(/!important/g) ?? [];
    const relativePath = url.pathname.split("/src/").at(-1);
    return matches.map(() => relativePath);
  }).sort();

  assert.deepEqual(forcedCascadeFiles, [
    "lib/preview/bridge.ts",
    "lib/state/preview-live-controller.ts",
  ]);
});

test("pictogramele UI folosesc componente Tabler, nu simboluri tipografice", () => {
  const componentFiles = filesBelow(new URL("../src/lib/components/", import.meta.url), /\.svelte$/);
  const forbiddenIconGlyph = /(?:>\s*(?:×|\+|−|⌾|⧉)\s*<|["'](?:▴|▾|▸|▶|⏸)["']|<span class="menu-code">↵<\/span>)/;

  for (const url of componentFiles) {
    const markup = readFileSync(url, "utf8").replace(/<style[\s\S]*?<\/style>/g, "");
    assert.doesNotMatch(markup, forbiddenIconGlyph, `pictogramă tipografică găsită în ${url.pathname}`);
  }
});

test("densitatea și navigarea au praguri verificabile", () => {
  const styleFiles = [
    ...filesBelow(new URL("../src/", import.meta.url), /\.(?:css|svelte)$/),
  ];
  const tooSmall = [];
  for (const url of styleFiles) {
    const css = readFileSync(url, "utf8");
    assert.doesNotMatch(css, /!important/, `cascada frontend nu poate fi forțată în ${url.pathname}`);
    for (const match of css.matchAll(/font-size:\s*([0-9.]+)px/g)) {
      if (Number(match[1]) < 11) tooSmall.push(`${url.pathname}:${match[0]}`);
    }
    for (const match of css.matchAll(/font:\s*(?:\d+\s+)?([0-9.]+)px\//g)) {
      if (Number(match[1]) < 11) tooSmall.push(`${url.pathname}:${match[0]}`);
    }
  }
  assert.deepEqual(tooSmall, [], "textul vizibil nu poate coborî sub 11px");

  const design = source("../src/routes/design-system.css");
  const projectPane = source("../src/lib/components/ProjectPane.svelte");
  assert.match(design, /--control-height:\s*32px/);
  assert.match(design, /min-height:\s*var\(--control-height\)/);
  assert.doesNotMatch(
    design,
    /:where\(\s*button,\s*input:not/,
    "elementele HTML brute nu trebuie forțate global la înălțimea controalelor principale",
  );
  assert.match(design, /small\s*\{[\s\S]*font-size:\s*var\(--font-meta\)/);
  assert.match(design, /:focus-visible[\s\S]*outline:\s*2px solid var\(--focus-ring\)/);
  assert.match(projectPane, /role="tablist"/);
  assert.match(projectPane, /role="tabpanel"/);
  assert.match(projectPane, /event\.key === "ArrowRight"/);
  assert.match(projectPane, /event\.key === "Escape"/);

  const navigation = source("../src/lib/components/project/EditorNavigationTree.svelte");
  assert.match(navigation, /class="tree-viewport"/);
  assert.match(navigation, /role="tree"/);
  assert.match(navigation, /role="treeitem"/);
  assert.match(navigation, /event\.key === "ArrowRight"/);
  assert.doesNotMatch(navigation, /<span[\s\S]{0,120}role="button"/);
});

test("navigatorul păstrează comenzile fixe și derulează conținutul tabului", () => {
  const projectPane = source("../src/lib/components/ProjectPane.svelte");

  assert.match(
    projectPane,
    /\.project-pane\s*\{[\s\S]*--project-pane-padding:\s*10px;[\s\S]*padding:\s*var\(--project-pane-padding\);[\s\S]*overflow:\s*hidden;/,
  );
  assert.match(
    projectPane,
    /\.pane-tab-panel\s*\{[\s\S]*flex:\s*1 1 auto;[\s\S]*min-height:\s*0;[\s\S]*margin-right:\s*calc\(-1 \* var\(--project-pane-padding\)\);[\s\S]*padding-right:\s*var\(--project-pane-padding\);[\s\S]*overflow:\s*auto;[\s\S]*overscroll-behavior:\s*contain;/,
  );
});

test("toate familiile de taburi folosesc segmented controlul tactil aprobat", () => {
  const designSystem = source("../src/routes/design-system.css");
  const projectPane = source("../src/lib/components/ProjectPane.svelte");
  const inspector = source("../src/lib/components/InspectorPane.svelte");
  const designWorkspace = source("../src/lib/components/creation/design-system/font-manager/FontInstaller.svelte");
  const documentBar = source("../src/lib/components/workbench/DocumentBar.svelte");
  const terminal = source("../src/lib/components/TerminalPane.svelte");
  const audit = source("../src/lib/components/audit/AuditWorkspace.svelte");
  const publish = source("../src/lib/components/publish/PublishWorkspace.svelte");
  const settings = source("../src/lib/components/settings/SettingsWorkspace.svelte");

  assert.match(
    designSystem,
    /\.ui-tabs\s*\{[\s\S]*gap:\s*3px;[\s\S]*padding:\s*3px;[\s\S]*border-radius:\s*calc\(var\(--radius-control\) \+ 2px\);[\s\S]*background:\s*var\(--material-inset\);[\s\S]*var\(--shadow-inset\)/,
  );
  assert.match(
    designSystem,
    /\.ui-tab:is\(\.active, \[aria-selected="true"\]\)\s*\{[\s\S]*border-color:\s*color-mix\([\s\S]*background:\s*linear-gradient\([\s\S]*var\(--shadow-control\)/,
  );
  assert.match(projectPane, /class="ui-tabs pane-tabs"/);
  assert.match(projectPane, /IconStack2 size=\{15\}[\s\S]*project-pane-layers/);
  assert.match(inspector, /class="ui-tabs inspector-tabs" role="tablist"/);
  assert.match(inspector, /class="ui-tab"[\s\S]*role="tab"[\s\S]*aria-selected/);
  assert.match(inspector, /handleInspectorTabKeydown/);
  assert.match(inspector, /IconHierarchy3 size=\{15\}/);
  assert.match(inspector, /IconPalette size=\{15\}/);
  assert.match(inspector, /IconPointerBolt size=\{15\}/);
  assert.match(inspector, /\.inspector-tabs\s*\{[^}]*margin:\s*10px 10px 0;/);
  assert.match(designWorkspace, /class="ui-tabs font-source-switch" role="tablist"/);
  assert.match(documentBar, /class="ui-document-tabs document-tabs"/);
  assert.match(documentBar, /class="ui-document-tab document-tab"/);
  assert.match(documentBar, /class="ui-tabs compact surface-switcher"/);
  assert.match(
    designSystem,
    /\.ui-document-tab\.active\s*\{[^}]*border-color:[^}]*background:\s*var\(--surface-raised\);[^}]*box-shadow:/,
  );
  assert.doesNotMatch(
    designSystem,
    /\.ui-document-tab\.active\s*\{[^}]*background:\s*linear-gradient\(/,
  );
  assert.match(terminal, /class="ui-tabs compact terminal-tab-strip"/);
  assert.match(terminal, /class="ui-tab terminal-tab"/);
  assert.match(audit, /class="workspace-toolbar"[\s\S]*class="ui-tabs view-tabs"/);
  assert.match(publish, /class="workspace-toolbar"[\s\S]*class="ui-tabs view-tabs"/);
  assert.match(settings, /class="ui-tabs settings-navigation"[\s\S]*role="tablist"/);
  assert.match(settings, /handleSettingsTabKeydown/);
  assert.doesNotMatch(
    designSystem,
    /\.ui-tab:is\(\.active, \[aria-selected="true"\]\) svg\s*\{[^}]*background:/,
  );
  assert.doesNotMatch(designSystem, /\.ui-tab:is\(\.active, \[aria-selected="true"\]\)\s*\{[^}]*border-bottom-color/);
  assert.doesNotMatch(inspector, /\.inspector-tabs \.active\s*\{/);
  assert.doesNotMatch(documentBar, /\.document-tab\.active::after/);
  assert.doesNotMatch(terminal, /\.terminal-tab\.active\s*\{/);
});

test("inspectorul HTML CSS și JS folosește o singură suprafață vizuală", () => {
  const inspector = source("../src/lib/components/InspectorPane.svelte");
  const sections = source("../src/lib/components/inspector/InspectorSection.svelte");
  const html = source("../src/lib/components/inspector/HtmlPane.svelte");
  const css = source("../src/lib/components/inspector/panes/CssPane.svelte");
  const js = source("../src/lib/components/inspector/JsPane.svelte");
  const motion = source("../src/lib/components/inspector/js/MotionStudioPanel.svelte");
  const layout = source("../src/lib/components/inspector/sections/LayoutSection.svelte");

  assert.match(inspector, /class="inspector-scroll inspector-editor-scroll"/);
  assert.match(inspector, /\.inspector-editor-scroll\s*\{[^}]*padding:\s*0;/);
  assert.match(inspector, /\.inspector-route\s*\{[^}]*flex-direction:\s*column;/);
  assert.match(inspector, /\.inspector-route\s*\{[^}]*min-width:\s*0;/);
  assert.match(sections, /\.section\s*\{[^}]*border-bottom:\s*1px solid var\(--border-subtle\);/);
  assert.match(sections, /IconChevronDown/);

  assert.doesNotMatch(html, /hf-delete-element|inspector-delete-element|inspector-delete-selected/);
  assert.doesNotMatch(html, /IconTrash/);
  assert.match(layout, /IconAlignBoxCenterMiddle/);
  assert.doesNotMatch(layout, /IconAlignBoxCenterMiddleFilled/);
  assert.doesNotMatch(html, />\+\s*\{t\("inspector-/);
  assert.doesNotMatch(motion, />\+\s*\{t\("motion-/);
  assert.doesNotMatch(motion, /<span>→<\/span>/);
  assert.match(motion, /IconArrowRight class="value-arrow"/);

  assert.match(css, /<section\s+[\s\S]*?class="css-pane"/);
  assert.doesNotMatch(css, /inspector-group/);
  assert.match(css, /\.css-context\s*\{[^}]*border-bottom:\s*1px solid var\(--border-subtle\);/);
  assert.doesNotMatch(css, /\.css-pane\s*\{[^}]*border(?:-radius)?:/);

  assert.match(js, /\.jp-target\s*\{[^}]*background:\s*transparent;/);
  assert.match(
    motion,
    /\.create-card, \.interaction-card, \.secondary-section\s*\{[^}]*border:\s*0;[^}]*border-bottom:\s*1px solid var\(--border-subtle\);[^}]*background:\s*transparent;/,
  );
});

test("rail-ul de activități începe direct cu navigarea, fără monogramă decorativă", () => {
  const rail = source("../src/lib/components/workbench/ActivityRail.svelte");

  assert.doesNotMatch(rail, /product-mark/);
  assert.doesNotMatch(rail, /aria-label="Pană Studio">P</);
  assert.match(rail, /<nav class="activity-rail"[\s\S]*?<div class="activity-list primary-activities">/);
});

test("capul preview-ului nu dublează documentul și nu păstrează contextul legacy", () => {
  const documentBar = source("../src/lib/components/workbench/DocumentBar.svelte");
  const editor = source("../src/lib/components/EditorShell.svelte");
  const status = source("../src/lib/components/StatusBar.svelte");
  const toolbar = source("../src/lib/components/workbench/ResponsiveCanvasToolbar.svelte");
  const zoom = source("../src/lib/components/workbench/PreviewZoomControl.svelte");
  const previewStageIndex = editor.indexOf('class="preview-stage"');
  const toolbarIndex = editor.indexOf("<ResponsiveCanvasToolbar");

  assert.doesNotMatch(toolbar, /surface-copy/);
  assert.doesNotMatch(toolbar, /documentPath/);
  assert.doesNotMatch(editor, /Context de template/);
  assert.doesNotMatch(editor, />Înapoi la site</);
  assert.match(toolbar, /t\("workbench-preview-interactive-title"\)/);
  assert.ok(toolbarIndex > previewStageIndex, "bara de control trebuie să fie sub canvas");
  assert.match(toolbar, /border-top:/);
  assert.match(toolbar, /class="ui-button compact"/);
  assert.match(toolbar, /\.segmented button\s*\{[\s\S]*?border-radius:\s*0;/);
  assert.match(toolbar, /container-type:\s*inline-size/);
  assert.doesNotMatch(toolbar, /IconMinus|IconPlus|changeZoom/);
  assert.match(toolbar, /<PreviewZoomControl/);
  assert.doesNotMatch(status, /zoom-slider|Zoom previzualizare|previewZoom/);
  assert.match(zoom, /type="range"/);
  assert.match(zoom, /oninput=\{\(event\) => setPreviewZoom/);
  assert.match(zoom, /onchange=\{\(event\) => \{ void commitPreviewZoom/);
  assert.match(documentBar, /IconLayoutColumns size=\{15\} stroke=\{1\.8\}/);
  assert.match(documentBar, /IconLayoutRows size=\{15\} stroke=\{1\.8\}/);
  assert.doesNotMatch(documentBar, /IconColumns2/);
});

test("taburile documentelor derulează exclusiv orizontal", () => {
  const documentBar = source("../src/lib/components/workbench/DocumentBar.svelte");
  const designSystem = source("../src/routes/design-system.css");
  const smoothWheel = source("../src/lib/ui/smooth-wheel.ts");
  const lifecycle = source("../src/lib/application/workspace-page-lifecycle.ts");

  assert.match(documentBar, /\.document-tabs\s*\{[\s\S]*overflow-x:\s*auto;/);
  assert.match(documentBar, /\.document-tabs\s*\{[\s\S]*overflow-y:\s*hidden;/);
  assert.match(documentBar, /new ResizeObserver\(\(\) => scheduleDocumentLayout\(\)\)/);
  assert.match(documentBar, /onscroll=\{\(\) => scheduleDocumentLayout\(\)\}/);
  assert.match(documentBar, /class:can-scroll-left=\{canScrollDocumentsLeft\}/);
  assert.match(documentBar, /class:can-scroll-right=\{canScrollDocumentsRight\}/);
  assert.match(documentBar, /\.document-tabs-shell\.can-scroll-left::before,[\s\S]*\.document-tabs-shell\.can-scroll-right::after/);
  assert.match(documentBar, /class="ui-icon-button mini danger document-close"/);
  assert.match(designSystem, /button\.ui-icon-button\.mini\.danger:hover:not\(:disabled\)\s*\{[^}]*var\(--danger\)/);
  assert.match(designSystem, /\.ui-button\.primary:disabled\s*\{[^}]*var\(--material-control-disabled\)[^}]*opacity:\s*1/);
  assert.doesNotMatch(
    documentBar,
    /\.document-select,\s*\.document-close|\.document-close:hover:not\(:disabled\)/,
  );
  assert.doesNotMatch(documentBar, /onwheel=|handleDocumentTabsWheel|wheelScrollTarget|animateWheelScroll/);
  assert.match(smoothWheel, /const fallbackAxis = preferredAxis === "x" \? "y" : "x"/);
  assert.match(smoothWheel, /Math\.exp\(-elapsed \/ EASING_TIME_CONSTANT_MS\)/);
  assert.match(smoothWheel, /prefers-reduced-motion: reduce/);
  assert.match(lifecycle, /installSmoothScrolling:\s*installSmoothWheelScrolling/);
  assert.match(lifecycle, /this\.platform\.installSmoothScrolling\(window\)/);
  assert.match(documentBar, /revealActiveDocumentTab/);
  assert.match(documentBar, /requestAnimationFrame/);
  assert.match(documentBar, /behavior:\s*window\.matchMedia\("\(prefers-reduced-motion: reduce\)"\)\.matches/);
  assert.doesNotMatch(documentBar, /scrollIntoView/);
});
