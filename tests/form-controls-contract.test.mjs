import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

function recursiveSvelteSources(directory = fileURLToPath(new URL("../src", import.meta.url))) {
  const entries = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = `${directory}/${entry.name}`;
    if (entry.isDirectory()) entries.push(...recursiveSvelteSources(path));
    else if (entry.isFile() && entry.name.endsWith(".svelte")) {
      entries.push({ path, content: readFileSync(path, "utf8") });
    }
  }
  return entries;
}

function styleBlocks(content) {
  return [...content.matchAll(/<style(?:\s[^>]*)?>([\s\S]*?)<\/style>/g)]
    .map((match) => match[1])
    .join("\n");
}

const canonicalFormWorkspaces = {
  templates: "../src/lib/components/templates/TemplatesWorkspace.svelte",
  content: "../src/lib/components/content/ContentWorkspace.svelte",
  components: "../src/lib/components/creation/ComponentsWorkspace.svelte",
  data: "../src/lib/components/data/DataWorkspace.svelte",
  taxonomies: "../src/lib/components/taxonomies/TaxonomiesWorkspace.svelte",
  assets: "../src/lib/components/creation/AssetsWorkspace.svelte",
  contentModels: "../src/lib/components/content-models/ContentModelsWorkspace.svelte",
  versions: "../src/lib/components/VersionsPanel.svelte",
  deploy: "../src/lib/components/deploy/DeployTargetsPanel.svelte",
  settings: "../src/lib/components/settings/SettingsWorkspace.svelte",
  projectSettings: "../src/lib/components/project-settings/ProjectSettingsWorkspace.svelte",
};

const specializedControlExceptions = [
  ["../src/lib/components/ui/PanaColorPicker.svelte", "suprafață bidimensională și canale de culoare"],
  ["../src/lib/components/workspace/MotionTimelinePanel.svelte", "timeline cu riglă și keyframe-uri"],
  ["../src/lib/components/inspector/js/MotionStudioPanel.svelte", "editor vizual de interacțiuni"],
  ["../src/lib/components/markdown/TipTapMarkdownEditor.svelte", "suprafață ProseMirror"],
  ["../src/lib/components/TerminalPane.svelte", "terminal xterm"],
  ["../src/lib/components/workbench/ResponsiveCanvasToolbar.svelte", "control dimensional pentru Canvas"],
  ["../src/lib/components/workspace/WorkspaceResizeHandle.svelte", "mâner de redimensionare"],
];

test("design system-ul deține contractul global al formularelor", () => {
  const designSystem = source("../src/routes/design-system.css");

  for (const primitive of [
    "ui-form-section",
    "ui-form-field",
    "ui-form-label",
    "ui-input",
    "ui-textarea",
    "ui-control-group",
    "ui-control-input",
    "ui-segmented",
    "ui-segmented-option",
    "ui-popover",
    "ui-option",
    "ui-select-trigger",
    "ui-checkbox",
    "ui-empty-state",
    "ui-message",
    "ui-switch",
  ]) {
    assert.match(designSystem, new RegExp(`\\.${primitive}\\b`), primitive);
  }
  assert.match(designSystem, /\.ui-input[\s\S]*background:\s*var\(--material-inset\)/);
  assert.match(designSystem, /\.ui-switch\.checked \.ui-switch-track/);
  assert.match(designSystem, /\.ui-switch:focus-within/);
  for (const component of [
    "CheckboxControl.svelte",
    "EmptyState.svelte",
    "InlineMessage.svelte",
    "TextAreaControl.svelte",
    "TextFieldControl.svelte",
    "SegmentedControl.svelte",
  ]) {
    assert.doesNotThrow(() => source(`../src/lib/components/ui/${component}`), component);
  }
});

test("panoul CSS reutilizează controalele compacte și suprafața globală de popover", () => {
  const designSystem = source("../src/routes/design-system.css");
  const coreInputs = [
    "PropInput.svelte",
    "TextWithOptions.svelte",
    "ColorInput.svelte",
    "AssetPicker.svelte",
  ].map((component) => [component, source(`../src/lib/components/inspector/controls/${component}`)]);
  const popoverOwners = [
    ["SelectControl", source("../src/lib/components/ui/SelectControl.svelte")],
    ["OptionsPopover", source("../src/lib/components/inspector/controls/OptionsPopover.svelte")],
    ["VariablePopover", source("../src/lib/components/inspector/controls/VariablePopover.svelte")],
    ["AssetPicker", source("../src/lib/components/inspector/controls/AssetPicker.svelte")],
  ];
  const segmented = source("../src/lib/components/ui/SegmentedControl.svelte");
  const cssPane = source("../src/lib/components/inspector/panes/CssPane.svelte");
  const anchoredPopover = source("../src/lib/ui/anchored-popover.ts");
  const forbiddenLocalSurface = /var\(--surface-[248]\)|var\(--border-[34]\)|box-shadow:\s*0 12px 30px/;

  assert.match(designSystem, /\.ui-control-group\s*\{[\s\S]*background:\s*var\(--material-inset\)/);
  assert.match(designSystem, /\.ui-segmented\s*\{[\s\S]*background:\s*var\(--material-inset\)/);
  assert.match(designSystem, /\.ui-popover\s*\{[\s\S]*background:\s*var\(--material-panel\)/);
  assert.match(designSystem, /\.ui-option:is\([^)]*\[aria-selected="true"\][^)]*\)/);
  assert.match(anchoredPopover, /export function anchoredPopoverScrollParents/);
  assert.match(anchoredPopover, /window\.getComputedStyle\(parent\)/);
  assert.match(anchoredPopover, /export function observeAnchoredPopoverPosition/);

  for (const [name, component] of coreInputs) {
    assert.match(component, /ui-control-group compact/, `${name}: grup compact`);
    assert.match(component, /ui-control-input/, `${name}: input global`);
    assert.doesNotMatch(styleBlocks(component), forbiddenLocalSurface, `${name}: suprafață locală paralelă`);
  }

  for (const [name, component] of popoverOwners) {
    assert.match(component, /calculateAnchoredPopoverPlacement/, `${name}: poziționare comună`);
    assert.match(component, /observeAnchoredPopoverPosition/, `${name}: urmărire comună a scrollului`);
    assert.match(component, /ui-popover/, `${name}: suprafață comună`);
    assert.match(component, /ui-option/, `${name}: opțiune comună`);
    assert.doesNotMatch(styleBlocks(component), forbiddenLocalSurface, `${name}: popover local paralel`);
  }

  assert.match(segmented, /class="ui-segmented"/);
  assert.match(segmented, /class="ui-segmented-option"/);
  assert.match(segmented, /role="group"/);
  assert.match(cssPane, /class="pseudo-bar ui-segmented compact"/);
  assert.match(cssPane, /class="pseudo-btn ui-segmented-option"/);

  for (const component of ["TransformSection.svelte", "EffectsSection.svelte"]) {
    const section = source(`../src/lib/components/inspector/sections/${component}`);
    assert.match(section, /calculateAnchoredPopoverPlacement/, `${component}: poziționare comună`);
    assert.match(section, /observeAnchoredPopoverPosition/, `${component}: urmărire comună a scrollului`);
    assert.match(section, /ui-popover/, `${component}: suprafață comună`);
    assert.match(section, /ui-option/, `${component}: opțiune comună`);
  }
});

test("select-ul este un trigger ridicat și marchează opțiunea activă", () => {
  const designSystem = source("../src/routes/design-system.css");
  const selectControl = source("../src/lib/components/ui/SelectControl.svelte");

  assert.match(designSystem, /\.ui-select-trigger\s*\{[\s\S]*background:\s*var\(--material-control\)/);
  assert.match(designSystem, /\.ui-select-trigger\[aria-expanded="true"\]/);
  assert.match(selectControl, /class="select-control ui-select-trigger"/);
  assert.match(selectControl, /class="select-option-check"/);
  assert.match(selectControl, /class:visible=\{option\.value === value\}/);
  assert.doesNotMatch(selectControl, /class="select-control ui-input"/);
  assert.match(selectControl, /role="combobox"/);
  assert.match(selectControl, /aria-controls=\{listboxId\}/);
  assert.match(selectControl, /aria-activedescendant=\{activeOptionId\}/);
  assert.match(selectControl, /role="listbox"/);
  assert.match(selectControl, /role="option"/);
  for (const key of ["Escape", "Tab", "ArrowDown", "ArrowUp", "Home", "End", "Enter"]) {
    assert.match(selectControl, new RegExp(`event\\.key === "${key}"`), key);
  }
  assert.match(selectControl, /event\.key === " "/);
  assert.match(selectControl, /<input type="hidden" \{name\} \{value\}/);
  assert.match(selectControl, /aria-required=\{required\}/);
});

test("switch-ul reutilizabil expune starea și semantică accesibilă", () => {
  const switchControl = source("../src/lib/components/ui/SwitchControl.svelte");

  assert.match(switchControl, /role="switch"/);
  assert.match(switchControl, /common-enabled/);
  assert.match(switchControl, /common-disabled/);
  assert.match(switchControl, /onchange\?\.\(event\.currentTarget\.checked\)/);
});

test("setările proiectului folosesc primitivele globale fără controale locale paralele", () => {
  const workspace = source("../src/lib/components/project-settings/ProjectSettingsWorkspace.svelte");

  assert.match(workspace, /import SwitchControl/);
  assert.match(workspace, /class="ui-input"/);
  assert.match(workspace, /class="ui-textarea"/);
  assert.match(workspace, /class="ui-form-field"/);
  assert.match(workspace, /<SwitchControl/);
  assert.match(workspace, /<SelectControl size="default"/);
  assert.doesNotMatch(workspace, /class="(?:config-field|switch-field)/);
  assert.doesNotMatch(workspace, /\.switch-field|\.config-field/);
});

test("controalele native generice nu reapar în afara primitivelor globale", () => {
  const files = recursiveSvelteSources();
  const checkboxOwners = new Set([
    "src/lib/components/ui/CheckboxControl.svelte",
    "src/lib/components/ui/SwitchControl.svelte",
  ]);

  for (const file of files) {
    const relativePath = file.path.slice(file.path.indexOf("/src/") + 1);
    assert.doesNotMatch(file.content, /<select\b/i, `${relativePath} folosește un select nativ`);
    if (/type\s*=\s*["']checkbox["']/.test(file.content)) {
      assert.ok(checkboxOwners.has(relativePath), `${relativePath} deține un checkbox nativ în afara primitivelor globale`);
    }
  }
});

test("activitățile canonice nu redeclară local sistemul vizual al formularelor", () => {
  for (const [name, path] of Object.entries(canonicalFormWorkspaces)) {
    const workspace = source(path);
    const styles = styleBlocks(workspace);
    assert.doesNotMatch(
      styles,
      /^\s*(?:input|select|textarea)(?=[\s,:.#\[])/m,
      `${name} stilizează direct un control generic`,
    );
    assert.doesNotMatch(
      styles,
      /^\s*\.(?:form-error|workspace-state|empty-state)\b/m,
      `${name} menține local o stare sau un mesaj standard`,
    );
    assert.doesNotMatch(
      styles,
      /^\s*\.ui-(?:button|icon-button|input|textarea|message|empty-state|form-section)\b/m,
      `${name} suprascrie local sursa de adevăr globală`,
    );
    assert.doesNotMatch(
      workspace,
      /class="(?![^"]*\bui-(?:button|icon-button)\b)[^"]*\b(?:primary-action|secondary-action|danger-action)\b[^"]*"/,
      `${name} expune o acțiune semantică fără primitiva globală`,
    );
  }
});

test("panourile dense adoptă primitivele compacte, iar excepțiile rămân explicite", () => {
  const densePanels = [
    ["../src/lib/components/inspector/DynamicWidgetPropertiesEditor.svelte", ["SelectControl", "CheckboxControl"]],
    ["../src/lib/components/inspector/js/MotionStudioPanel.svelte", ["SelectControl", "CheckboxControl"]],
    ["../src/lib/components/kernel/ObservabilityLogControl.svelte", ["SelectControl", "CheckboxControl"]],
    ["../src/lib/components/project/ProjectPageSettingsTab.svelte", ["SelectControl", "CheckboxControl"]],
  ];
  for (const [path, primitives] of densePanels) {
    const panel = source(path);
    for (const primitive of primitives) assert.match(panel, new RegExp(`import ${primitive}`), `${path}: ${primitive}`);
    assert.doesNotMatch(panel, /<select\b|type\s*=\s*["']checkbox["']/i, path);
  }

  assert.ok(specializedControlExceptions.length > 0);
  for (const [path, reason] of specializedControlExceptions) {
    assert.ok(reason.length >= 12, `${path} nu are o justificare explicită`);
    const component = source(path);
    assert.doesNotMatch(component, /<select\b|type\s*=\s*["']checkbox["']/i, path);
  }
});
