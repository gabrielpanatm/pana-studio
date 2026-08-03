import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
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
  const kernel = source("../src-tauri/src/kernel/content_models.rs");
  const commands = source("../src-tauri/src/commands/content_models.rs");

  assert.doesNotMatch(scope, /const DERIVED_OR_INTERNAL_DIRS[\s\S]*"\.panastudio",/);
  assert.doesNotMatch(files, /const SKIP_DIRS[\s\S]*"\.panastudio",/);
  assert.match(kernel, /\.panastudio\/project\.toml/);
  assert.match(kernel, /\.panastudio\/content-models/);
  assert.match(kernel, /stage_composite_changes/);
  assert.match(commands, /require_bound_workspace/);
  assert.match(commands, /expected_plan_id/);
  assert.match(commands, /commit_project_workspace_session_mutation/);
  assert.doesNotMatch(commands, /std::fs::(?:write|remove_file|rename)/);
});

test("contractul acoperă tipurile, nesting, attach/detach și replace/migrate", () => {
  const kernel = source("../src-tauri/src/kernel/content_models.rs");
  for (const kind of [
    "Text", "Textarea", "Markdown", "Number", "Boolean", "Date",
    "Select", "Url", "Color", "Image", "Group", "Repeater",
  ]) assert.match(kernel, new RegExp(`\\b${kind},`));
  assert.match(kernel, /parent_field_id/);
  assert.match(kernel, /ReplaceModel/);
  assert.match(kernel, /RenameModel/);
  assert.match(kernel, /stage_rename_dynamic_marker_model/);
  assert.match(kernel, /stage_replace_model_values/);
  assert.match(kernel, /stage_remove_model_values/);
  assert.match(kernel, /stage_rename_template_references/);
  assert.match(kernel, /affected_keys/);
  assert.match(kernel, /remove_nested_value/);
  assert.match(kernel, /blocked:\s*!blockers\.is_empty\(\)/);
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
  assert.match(custom, /settleProjectWorkspaceMutation/);
  assert.match(recursive, /field\.kind === "group"/);
  assert.match(recursive, /field\.kind === "repeater"/);
  assert.match(recursive, /Adaugă element/);
  assert.doesNotMatch(custom, /JSON\.parse|textarea\.json/);
});

test("Blocuri oferă binding-uri dinamice tipizate și context single real", () => {
  const blocks = source("../src/lib/components/creation/BlocksWorkspace.svelte");
  const controller = source("../src/lib/state/tera-actions-controller.ts");
  const engine = source("../src-tauri/src/project_model/tera_insert_engine.rs");

  assert.match(blocks, /"dynamic_fields"/);
  for (const presentation of ["text", "image", "link", "button", "list", "condition"]) {
    assert.match(blocks, new RegExp(`"${presentation}"`));
  }
  assert.match(blocks, /dynamicBinding:\s*dynamicBinding/);
  assert.match(blocks, /Conținut de previzualizare/);
  assert.match(blocks, /updateTemplateWorkbenchContext/);
  assert.match(controller, /dynamicBinding:\s*request\.item\.dynamicBinding/);
  assert.match(engine, /validate_dynamic_field_binding/);
  assert.match(engine, /pana:dynamic model=/);
  assert.match(engine, /page\.extra/);
});
