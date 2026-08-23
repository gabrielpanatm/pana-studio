import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("TipTap belongs only to Content page editing and shares one canonical source with settings", () => {
  const content = source("../src/lib/components/content/ContentWorkspace.svelte");
  const editorShell = source("../src/lib/components/EditorShell.svelte");

  assert.match(content, /import MarkdownEditor from "\$lib\/components\/markdown\/MarkdownEditor\.svelte"/);
  assert.match(content, /contentWorkspace\?\.mode === "edit"/);
  assert.match(content, /await commands\.openPageEditor\(page\.file\)/);
  assert.match(content, /<MarkdownEditor[\s\S]*source=\{metadataSource\}[\s\S]*onChange=/);
  assert.match(content, /<ProjectPageSettingsTab[\s\S]*pageSource=\{metadataSource\}/);
  assert.match(content, /<ProjectPageSettingsTab[\s\S]*pageKind=\{editingPage\.pageKind\}/);
  assert.match(content, /class="content-page-workspace"/);
  assert.match(content, /grid-template-columns: minmax\(0, 1fr\) minmax\(290px, 360px\)/);
  assert.doesNotMatch(content, /DetailMode = "info" \| "create" \| "edit"/);
  assert.doesNotMatch(content, /content-finish-editing|content-controlled-change/);
  assert.doesNotMatch(editorShell, /MarkdownEditor|surface === "markdown"/);
});

test("section settings expose mandatory pagination and hide page-only fields", () => {
  const settings = source("../src/lib/components/project/ProjectPageSettingsTab.svelte");

  assert.match(settings, /isSection = pageKind === "section"/);
  assert.match(settings, /isSection[\s\S]*content-settings-field-paginate-by/);
  assert.match(settings, /min="1"[\s\S]*setField\("paginateBy"/);
  assert.match(settings, /\{#if !isSection\}<label class="field">[\s\S]*content-settings-field-date/);
  assert.match(settings, /\{#if !isSection\}<label class="field">[\s\S]*content-settings-field-slug/);
});

test("the technical editor has Visual and Code only, while raw Markdown opens in Code", () => {
  const model = source("../src-tauri/src/kernel/workbench/model.rs");
  const explorer = source("../src-tauri/src/kernel/file_explorer.rs");
  const commandCenter = source("../src-tauri/src/kernel/command_center/search.rs");
  const documentBar = source("../src/lib/components/workbench/DocumentBar.svelte");
  const route = source("../src/routes/+page.svelte");
  const workflow = source("../src/lib/project/workflow.ts");

  assert.match(model, /enum WorkbenchSurface\s*\{[\s\S]*Visual,[\s\S]*#\[serde\(alias = "markdown"\)\][\s\S]*Code,/);
  assert.match(explorer, /name\.ends_with\("\.md"\)[\s\S]*Some\(WorkbenchSurface::Code\)/);
  assert.match(commandCenter, /page\.file[\s\S]*WorkbenchSurface::Code/);
  assert.doesNotMatch(commandCenter, /command\.show_markdown|ShowMarkdown/);
  assert.doesNotMatch(documentBar, /setSurface\("markdown"\)|workbench-markdown/);
  assert.doesNotMatch(route, /setCenterView\("markdown"\)|surface === "markdown"/);
  assert.match(workflow, /file\.kind === "MD"[\s\S]*return "code"/);
});

test("Markdown boundaries expose distinct semantic and raw-source actions", () => {
  const card = source("../src/lib/components/inspector/MarkdownSourceCard.svelte");
  const inspector = source("../src/lib/components/InspectorPane.svelte");
  const selection = source("../src/lib/state/app-selection-controller.ts");

  assert.match(card, /editSelectedContent/);
  assert.match(card, /openSelectedSource/);
  assert.match(card, /markdown-boundary-edit-content/);
  assert.match(card, /markdown-boundary-open-source/);
  assert.match(inspector, /editSelectedContent=\{openSelectedMarkdownContent\}/);
  assert.match(selection, /navigationNode\?\.kind !== "boundary"/);
  assert.match(selection, /navigationNode\.boundary\?\.kind !== "markdown"/);
  assert.match(selection, /await app\.openContentPageEditor\(relativePath\)/);
});

test("invalid front matter blocks semantic mutation without hiding raw Code", () => {
  const content = source("../src/lib/components/content/ContentWorkspace.svelte");

  assert.match(content, /editingPage\?\.frontmatterParseError/);
  assert.match(content, /content-frontmatter-settings-blocked/);
  assert.match(content, /openWorkspaceSource\(editingPagePath\)/);
  assert.match(content, /\{:else if editingPage\}[\s\S]*<MarkdownEditor/);
});
