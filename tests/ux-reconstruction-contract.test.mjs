import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { test } from "node:test";
import { FEEDBACK_CHANNELS } from "$lib/feedback/policy";
import { UI_TERMS } from "$lib/i18n/ui-terms";

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

test("feedbackul are canale exclusive și autoritatea AI rămâne în bara de stare", () => {
  assert.deepEqual(FEEDBACK_CHANNELS.statusBar.owns, ["save", "validation", "preview", "ai-authority", "current-source"]);
  assert.deepEqual(FEEDBACK_CHANNELS.notification.owns, ["conflict", "recovery", "operator-decision"]);

  const route = source("../src/routes/+page.svelte");
  const status = source("../src/lib/components/StatusBar.svelte");
  const ai = source("../src/lib/components/ai/AiEditAuthorityIndicator.svelte");
  const editor = source("../src/lib/components/EditorShell.svelte");
  const session = source("../src/lib/state/app-session-controller.ts");

  assert.doesNotMatch(route, /<AiEditAuthorityIndicator/);
  assert.match(status, /<AiEditAuthorityIndicator/);
  assert.doesNotMatch(ai, /position:\s*fixed/);
  assert.equal((editor.match(/class="editor-context-bar"/g) ?? []).length, 1);
  assert.doesNotMatch(editor, /workbench-banner|design-safe-banner/);

  const setter = session.slice(session.indexOf("export function setGlobalStatus"), session.indexOf("export function notify"));
  assert.doesNotMatch(setter, /notify\(/, "o eroare pasivă nu devine automat notificare persistentă");
});

test("glosarul românesc elimină etichetele legacy din suprafețele vizibile", () => {
  assert.equal(UI_TERMS.settings, "Setări");
  assert.equal(UI_TERMS.designSystem, "Sistem de design");
  assert.equal(UI_TERMS.problemsAudit, "Probleme și audit");
  assert.equal(UI_TERMS.safeEditing, "Editare sigură");

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

  const htmlPalette = source("../src/lib/project/html-palette.ts");
  const teraPalette = source("../src/lib/tera/palette.ts");
  assert.doesNotMatch(htmlPalette, /label: "(?:Section|Article|Paragraph|Quote|Image|Source|Picture|Button|Form|Option|Table|Caption)"/);
  assert.doesNotMatch(teraPalette, /label: "(?:Extends|Block content|Include partial|Import macros|If|With|Set|Variable|Comment|Raw)"/);
});

test("design-system.css este singura sursă de tokeni și expune primitivele comune", () => {
  const appHtml = source("../src/app.html");
  const shell = source("../src/routes/workspace-shell.css");
  const design = source("../src/routes/design-system.css");
  const site = source("../src/lib/components/site/SiteOverviewWorkspace.svelte");
  const components = source("../src/lib/components/creation/ComponentsWorkspace.svelte");

  assert.equal(existsSync(new URL("../static/app-shell.css", import.meta.url)), false);
  assert.doesNotMatch(appHtml, /app-shell\.css/);
  assert.match(shell, /@import "\.\/design-system\.css"/);
  assert.doesNotMatch(shell, /--(?:surface-base|brand|text|border-subtle)\s*:/);

  for (const primitive of ["ui-button", "ui-icon-button", "ui-tabs", "ui-tab", "ui-field", "ui-panel", "ui-card", "ui-badge", "ui-message"]) {
    assert.match(design, new RegExp(`\\.${primitive}(?:[\\s,{.:])`), `lipsește primitiva ${primitive}`);
  }
  for (const usage of ["ui-button", "ui-panel", "ui-card", "ui-badge"]) assert.match(site, new RegExp(usage));
  for (const usage of ["ui-tabs", "ui-tab", "ui-field", "ui-message"]) assert.match(components, new RegExp(usage));

  const uniqueDarkSurfaces = [...design.matchAll(/--surface-(?:base|panel|raised):\s*([^;]+);/g)].map((match) => match[1]);
  assert.equal(new Set(uniqueDarkSurfaces.slice(0, 3)).size, 3);
  assert.match(design, /--surface-[1-9]:\s*var\(--surface-(?:base|panel|raised)\)/);
});

test("densitatea și navigarea au praguri verificabile", () => {
  const styleFiles = [
    ...filesBelow(new URL("../src/", import.meta.url), /\.(?:css|svelte)$/),
  ];
  const tooSmall = [];
  for (const url of styleFiles) {
    const css = readFileSync(url, "utf8");
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
  assert.match(design, /min-height:\s*var\(--control-height\)\s*!important/);
  assert.match(design, /small\s*\{[\s\S]*font-size:\s*var\(--font-meta\)\s*!important/);
  assert.match(design, /:focus-visible[\s\S]*outline:\s*2px solid var\(--focus-ring\)\s*!important/);
  assert.match(projectPane, /role="tablist"/);
  assert.match(projectPane, /role="tabpanel"/);
  assert.match(projectPane, /event\.key === "ArrowRight"/);
  assert.match(projectPane, /event\.key === "Escape"/);

  const layers = source("../src/lib/components/project/ProjectLayersTab.svelte");
  assert.match(layers, /class="layers-tree" role="tree"/);
  assert.match(layers, /role="treeitem"/);
  assert.match(layers, /event\.key === "ArrowRight"/);
  assert.doesNotMatch(layers, /<span[\s\S]{0,120}role="button"/);
});
