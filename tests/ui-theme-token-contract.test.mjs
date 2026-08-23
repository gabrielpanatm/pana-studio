import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { test } from "node:test";

function styleSourceUrls(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const entryUrl = new URL(`${entry.name}${entry.isDirectory() ? "/" : ""}`, directory);
    if (entry.isDirectory()) return styleSourceUrls(entryUrl);
    return /\.(?:css|svelte)$/.test(entry.name) ? [entryUrl] : [];
  });
}

test("interfața folosește tokenul canonic --brand pentru accente", () => {
  const designSystemCss = readFileSync(new URL("../src/routes/design-system.css", import.meta.url), "utf8");
  const preferences = readFileSync(
    new URL("../src/lib/application/preferences.svelte.ts", import.meta.url),
    "utf8",
  );
  const settingsWorkspace = readFileSync(
    new URL("../src/lib/components/settings/SettingsWorkspace.svelte", import.meta.url),
    "utf8",
  );
  assert.match(designSystemCss, /--brand\s*:/);
  assert.match(preferences, /setProperty\("--brand", this\.accent\)/);
  assert.match(settingsWorkspace, /applicationPreferences\.snapshot\?\.brandAccent/);
  assert.doesNotMatch(settingsWorkspace, /\.accent-swatch\.brand/);

  for (const token of [
    "brand-strong",
    "brand-soft",
    "focus-ring",
    "control-hover",
    "control-selected",
    "selector-bg",
    "terminal-shell-ring",
  ]) {
    const declarations = [
      ...designSystemCss.matchAll(new RegExp(`--${token}:\\s*([^;]+);`, "g")),
    ];
    assert.ok(declarations.length > 0, `lipsește tokenul --${token}`);
    for (const declaration of declarations) {
      assert.match(
        declaration[1],
        /var\(--brand\)/,
        `--${token} trebuie derivat din accentul autoritativ --brand`,
      );
    }
  }

  for (const declaration of [
    ...designSystemCss.matchAll(/--code-text:\s*([^;]+);/g),
  ]) {
    assert.match(declaration[1], /var\(--brand-strong\)/);
  }
  assert.match(designSystemCss, /--wb-control-hover:\s*var\(--control-hover\)/);
  assert.doesNotMatch(
    designSystemCss,
    /#173a32|#e2f3ee|#eef3f1|#54c0a5|#63cdb2|#116b5b|#bfe5d9|#155444|rgb\(22 131 111|rgb\(67 185 155|rgb\(41 104 89/,
    "familia accentului nu trebuie să păstreze nuanțele teal hardcodate",
  );

  const sourceUrls = [
    ...styleSourceUrls(new URL("../src/", import.meta.url)),
    ...styleSourceUrls(new URL("../static/", import.meta.url)),
  ];
  const undefinedAccentUsers = sourceUrls
    .filter((sourceUrl) => readFileSync(sourceUrl, "utf8").includes("var(--accent)"))
    .map((sourceUrl) => sourceUrl.pathname);

  assert.deepEqual(
    undefinedAccentUsers,
    [],
    "Folosește var(--brand); shell-ul aplicației nu definește --accent.",
  );
});

test("design system-ul oferă o variantă compactă reutilizabilă pentru micro-acțiuni", () => {
  const designSystemCss = readFileSync(new URL("../src/routes/design-system.css", import.meta.url), "utf8");
  const navigationSource = readFileSync(
    new URL("../src/lib/components/project/EditorNavigationTree.svelte", import.meta.url),
    "utf8",
  );

  assert.match(designSystemCss, /--control-height-compact:\s*24px/);
  assert.match(designSystemCss, /\.ui-button\.compact,\s*\n\.ui-icon-button\.compact/);
  assert.match(designSystemCss, /\.ui-icon-button\.compact\s*\{/);
  assert.match(navigationSource, /class="scope-action"/);
  assert.match(navigationSource, /class="delete-action"/);
  assert.match(navigationSource, /\.delete-action\s*\{[\s\S]*width:\s*20px;[\s\S]*height:\s*20px;/);
});

test("profunzimea skeuomorphic este tokenizată și păstrează fallbackul high contrast", () => {
  const designSystemCss = readFileSync(
    new URL("../src/routes/design-system.css", import.meta.url),
    "utf8",
  );
  const workspaceShell = readFileSync(
    new URL("../src/routes/workspace-shell.css", import.meta.url),
    "utf8",
  );
  const componentSources = {
    topbar: readFileSync(new URL("../src/lib/components/Topbar.svelte", import.meta.url), "utf8"),
    toolbarButton: readFileSync(new URL("../src/lib/components/topbar/ToolbarButton.svelte", import.meta.url), "utf8"),
    activityRail: readFileSync(new URL("../src/lib/components/workbench/ActivityRail.svelte", import.meta.url), "utf8"),
    projectPane: readFileSync(new URL("../src/lib/components/ProjectPane.svelte", import.meta.url), "utf8"),
    editorShell: readFileSync(new URL("../src/lib/components/EditorShell.svelte", import.meta.url), "utf8"),
    inspectorPane: readFileSync(new URL("../src/lib/components/InspectorPane.svelte", import.meta.url), "utf8"),
    terminal: readFileSync(new URL("../src/lib/components/TerminalPane.svelte", import.meta.url), "utf8"),
    status: readFileSync(new URL("../src/lib/components/StatusBar.svelte", import.meta.url), "utf8"),
  };

  for (const token of [
    "surface-inset",
    "surface-control",
    "material-panel",
    "material-inset",
    "material-control",
    "material-control-hover",
    "material-control-selected",
    "material-accent",
    "material-accent-hover",
    "material-accent-pressed",
    "shadow-panel",
    "shadow-control",
    "shadow-control-hover",
    "shadow-accent",
    "shadow-accent-hover",
    "shadow-inset",
    "shadow-pressed",
  ]) {
    assert.match(designSystemCss, new RegExp(`--${token}:`), `lipsește --${token}`);
  }

  assert.match(designSystemCss, /--material-control-selected:[\s\S]*var\(--control-selected\)/);
  assert.match(designSystemCss, /\.ui-button,\s*\n\.ui-icon-button\s*\{[\s\S]*box-shadow:\s*var\(--shadow-control\)/);
  assert.match(designSystemCss, /\.ui-field\s*\{[\s\S]*background:\s*var\(--material-inset\);[\s\S]*box-shadow:\s*var\(--shadow-inset\)/);
  assert.match(designSystemCss, /\.ui-panel\s*\{[\s\S]*border:\s*1px solid var\(--border-subtle\);[\s\S]*box-shadow:\s*var\(--shadow-panel\)/);

  const highContrast = designSystemCss.slice(
    designSystemCss.indexOf('html[data-pana-contrast="high"]'),
    designSystemCss.indexOf('html[data-pana-reduced-motion="true"]'),
  );
  for (const material of [
    "panel",
    "inset",
    "control",
    "control-hover",
    "control-selected",
    "accent",
    "accent-hover",
    "accent-pressed",
  ]) {
    assert.match(highContrast, new RegExp(`--material-${material}:`));
  }
  for (const shadow of ["panel", "control", "control-hover", "accent", "accent-hover"]) {
    assert.match(highContrast, new RegExp(`--shadow-${shadow}: none;`));
  }
  assert.match(highContrast, /--shadow-inset:\s*inset 0 0 0 1px var\(--border-strong\)/);
  assert.match(highContrast, /--shadow-pressed:\s*inset 0 0 0 1px var\(--border-strong\)/);
  assert.match(designSystemCss, /--radius-control:\s*8px/);
  assert.match(designSystemCss, /--radius-panel:\s*12px/);
  assert.match(designSystemCss, /\.ui-button\.primary[\s\S]*var\(--material-accent\)[\s\S]*var\(--shadow-accent\)/);

  assert.match(workspaceShell, /\.workbench-frame[\s\S]*background:\s*var\(--material-inset\);[\s\S]*box-shadow:\s*var\(--shadow-inset\)/);
  assert.match(componentSources.topbar, /\.topbar[\s\S]*background:\s*var\(--material-panel\);[\s\S]*box-shadow:\s*var\(--shadow-panel\)/);
  assert.match(componentSources.topbar, /\.segmented-group[\s\S]*overflow:\s*hidden;[\s\S]*var\(--skeuo-shade-soft\)/);
  assert.match(componentSources.topbar, /toolbar-icon-button\.segmented \+ \.toolbar-icon-button\.segmented[\s\S]*::before/);
  assert.match(componentSources.toolbarButton, /aria-pressed=\{segmented \? active : undefined\}/);
  assert.match(componentSources.toolbarButton, /\.toolbar-icon-button\.segmented\.active[\s\S]*border-color:\s*transparent;[\s\S]*var\(--material-control-selected\)/);
  assert.match(componentSources.activityRail, /\.activity-rail[\s\S]*background:\s*var\(--material-panel\)/);
  for (const source of [
    componentSources.projectPane,
    componentSources.editorShell,
    componentSources.inspectorPane,
  ]) {
    assert.match(source, /background:\s*var\(--material-panel\);[\s\S]*box-shadow:\s*var\(--shadow-panel\)/);
  }
  assert.match(designSystemCss, /--entity-selection-outline:\s*var\(--brand\)/);
  assert.match(componentSources.terminal, /\.terminal-body[\s\S]*background:\s*var\(--material-inset\);[\s\S]*box-shadow:\s*var\(--shadow-inset\)/);
  assert.match(componentSources.status, /\.status-bar[\s\S]*background:\s*var\(--material-panel\)/);

  assert.match(
    designSystemCss,
    /body \.app-shell \.activity-workspace\s*\{[\s\S]*background:\s*var\(--material-panel\);[\s\S]*box-shadow:\s*var\(--shadow-panel\)/,
    "shell-ul activităților trebuie să primească profunzimea din contractul vizual central",
  );

  const canonicalActivityWorkspaceUrls = [
    "../src/lib/components/audit/AuditWorkspace.svelte",
    "../src/lib/components/content/ContentWorkspace.svelte",
    "../src/lib/components/creation/AssetsWorkspace.svelte",
    "../src/lib/components/creation/BlocksWorkspace.svelte",
    "../src/lib/components/creation/ComponentsWorkspace.svelte",
    "../src/lib/components/creation/DesignSystemWorkspace.svelte",
    "../src/lib/components/data/DataWorkspace.svelte",
    "../src/lib/components/publish/PublishWorkspace.svelte",
    "../src/lib/components/taxonomies/TaxonomiesWorkspace.svelte",
    "../src/lib/components/templates/TemplatesWorkspace.svelte",
    "../src/lib/components/VersionsPanel.svelte",
  ];
  for (const relativeUrl of canonicalActivityWorkspaceUrls) {
    const source = readFileSync(new URL(relativeUrl, import.meta.url), "utf8");
    assert.match(
      source,
      /class="activity-workspace [^"]+-workspace"/,
      `${relativeUrl} trebuie să adopte shell-ul vizual central`,
    );
  }

  const bespokePanelUrls = [
    "../src/lib/components/kernel/KernelWorkspace.svelte",
    "../src/lib/components/settings/SettingsWorkspace.svelte",
  ];
  for (const relativeUrl of bespokePanelUrls) {
    const source = readFileSync(new URL(relativeUrl, import.meta.url), "utf8");
    assert.match(
      source,
      /background:\s*var\(--material-panel\);[\s\S]*box-shadow:\s*var\(--shadow-panel\)/,
      `${relativeUrl} trebuie să folosească suprafața tactilă structurală`,
    );
  }
});
