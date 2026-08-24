import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("Modele de conținut este o activitate separată cu autoritate Rust", () => {
  const rail = source("../src/lib/components/workbench/ActivityRail.svelte");
  const center = source("../src/lib/components/workspace/WorkspaceCenterArea.svelte");
  const workbench = source("../src-tauri/src/kernel/workbench/model.rs");
  const registry = source("../src-tauri/src/tauri_command_registry.rs");

  assert.match(rail, /id:\s*"content_models"/);
  assert.match(center, /retainedAuxiliarySurface === "content_models"[\s\S]*<ContentModelsWorkspace/);
  assert.match(workbench, /ContentModels/);
  for (const command of [
    "read_content_model_catalog",
    "plan_content_model_mutation",
    "apply_content_model_mutation",
  ]) assert.match(registry, new RegExp(command));
});

test("activitatea Modele de conținut folosește layoutul vizual comun", () => {
  const workspace = source("../src/lib/components/content-models/ContentModelsWorkspace.svelte");

  assert.match(workspace, /<section class="activity-workspace content-models-workspace"/);
  assert.match(workspace, /<header class="workspace-header">/);
  assert.match(workspace, /<div class="workspace-toolbar">/);
  assert.match(workspace, /<div class="workspace-body">/);
  assert.match(workspace, /role="tablist"/);
  for (const view of ["fields", "sections", "usages", "validation"]) {
    assert.match(workspace, new RegExp(`id: "${view}"`));
  }
  assert.match(workspace, /class="ui-button primary toolbar toolbar-action"/);
  assert.doesNotMatch(workspace, /class="models-workspace"/);
  assert.doesNotMatch(workspace, /<aside class="model-impact"/);
  assert.doesNotMatch(workspace, /\.workspace-header\s*\{/);
  assert.doesNotMatch(workspace, /\.workspace-toolbar\s*\{/);
});

test("formularele modelelor oferă exemple contextuale pentru datele cerute", () => {
  const workspace = source("../src/lib/components/content-models/ContentModelsWorkspace.svelte");

  assert.match(workspace, /const FIELD_EXAMPLES/);
  for (const example of [
    "subtitlu",
    "pret",
    "data_publicarii",
    "link_actiune",
    "culoare_accent",
    "/imagini/serviciu.webp",
  ]) assert.match(workspace, new RegExp(example.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
  assert.match(workspace, /placeholder=\{FIELD_EXAMPLES\[fieldKindDraft\]\.key\}/);
  assert.match(workspace, /placeholder=\{FIELD_EXAMPLES\[fieldKindDraft\]\.label\}/);
  assert.match(workspace, /placeholder=\{FIELD_EXAMPLES\[fieldKindDraft\]\.help\}/);
  assert.match(workspace, /placeholder=\{FIELD_EXAMPLES\[fieldKindDraft\]\.defaultValue\}/);
  assert.match(workspace, /placeholder=\{FIELD_EXAMPLES\[fieldKindDraft\]\.pattern\}/);
  assert.match(workspace, /placeholder="serviciu"/);
  assert.match(workspace, /standard\|Standard\\npremium\|Premium/);
});

test("metadatele .panastudio sunt surse de proiect și mutațiile sunt tranzacții ProjectWorkspace", () => {
  const scope = source("../src-tauri/src/project/scope.rs");
  const files = source("../src-tauri/src/project_model/files.rs");
  const facade = source("../src-tauri/src/kernel/content_models/mod.rs");
  const staging = source("../src-tauri/src/kernel/content_models/staging.rs");
  const commands = source("../src-tauri/src/commands/content_models.rs");

  assert.doesNotMatch(scope, /const DERIVED_OR_INTERNAL_DIRS[\s\S]*"\.panastudio",/);
  assert.doesNotMatch(files, /const SKIP_DIRS[\s\S]*"\.panastudio",/);
  assert.match(facade, /\.panastudio\/project\.toml/);
  assert.match(facade, /\.panastudio\/content-models/);
  assert.match(staging, /stage_composite_changes/);
  assert.match(commands, /require_bound_workspace/);
  assert.match(commands, /expected_plan_id/);
  assert.match(commands, /commit_project_workspace_session_mutation/);
  assert.doesNotMatch(commands, /std::fs::(?:write|remove_file|rename)/);
});

test("contractul acoperă tipurile, nesting, attach/detach și replace/migrate", () => {
  const schema = source("../src-tauri/src/kernel/content_schema.rs");
  const plan = source("../src-tauri/src/kernel/content_models/mutation_plan.rs");
  const frontmatter = source("../src-tauri/src/kernel/content_models/rewrite/frontmatter.rs");
  const templates = source("../src-tauri/src/kernel/content_models/rewrite/templates.rs");
  for (const kind of [
    "Text", "Textarea", "Markdown", "Number", "Boolean", "Date",
    "Select", "Url", "Color", "Image", "Group", "Repeater",
  ]) assert.match(schema, new RegExp(`\\b${kind},`));
  assert.match(plan, /parent_field_id/);
  assert.match(plan, /ReplaceModel/);
  assert.match(plan, /RenameModel/);
  assert.match(plan, /affected_keys/);
  assert.match(plan, /blocked:\s*!blockers\.is_empty\(\)/);
  assert.doesNotMatch(templates, /dynamic_marker|pana:dynamic/);
  assert.match(templates, /stage_replace_model_values/);
  assert.match(templates, /stage_remove_model_values/);
  assert.match(templates, /stage_rename_template_references/);
  assert.match(frontmatter, /remove_nested_value/);
});

test("nucleul Content Models rămâne modular și fără cuplaj circular", () => {
  const facade = source("../src-tauri/src/kernel/content_models/mod.rs");
  const plan = source("../src-tauri/src/kernel/content_models/mutation_plan.rs");
  const widgets = source("../src-tauri/src/kernel/dynamic_widgets.rs");
  const legacyMonolith = new URL("../src-tauri/src/kernel/content_models.rs", import.meta.url);

  for (const moduleName of [
    "catalog", "mutation_plan", "rewrite", "staging", "usage_index", "validation",
  ]) assert.match(facade, new RegExp(`mod ${moduleName};`));
  assert.doesNotMatch(facade, /\b(?:pub\s+)?fn\s+/);
  assert.match(facade, /pub use usage_index::refresh_content_model_template_usages;/);
  assert.equal(existsSync(legacyMonolith), false);
  assert.match(widgets, /kernel::content_schema/);
  assert.doesNotMatch(widgets, /kernel::content_models/);
  assert.match(plan, /let catalog = &graph\.content_models;/);
  assert.doesNotMatch(plan, /content_models\.clone\(\)/);
});

test("editorul de conținut separă Setări, SEO și Câmpuri personalizate", () => {
  const content = source("../src/lib/components/content/ContentWorkspace.svelte");
  const settings = source("../src/lib/components/project/ProjectPageSettingsTab.svelte");
  const custom = source("../src/lib/components/content/PageCustomFieldsPanel.svelte");
  const recursive = source("../src/lib/components/content/CustomFieldInput.svelte");

  assert.match(content, /"settings"\s*\|\s*"seo"\s*\|\s*"custom_fields"/);
  assert.match(content, /<PageCustomFieldsPanel/);
  assert.match(settings, /view:\s*"settings"\s*\|\s*"seo"/);
  assert.match(custom, /kind:\s*"set_page_values"/);
  assert.match(custom, /workspaceMutations\.settle/);
  assert.match(recursive, /field\.kind === "group"/);
  assert.match(recursive, /field\.kind === "repeater"/);
  assert.match(recursive, /Adaugă element/);
  assert.doesNotMatch(custom, /JSON\.parse|textarea\.json/);
});
