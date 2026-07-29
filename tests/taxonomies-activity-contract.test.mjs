import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("Taxonomii este o activitate Workbench sincronizată între Rust și frontend", () => {
  const rustWorkbench = source("../src-tauri/src/kernel/workbench/model.rs");
  const rustCommands = source("../src-tauri/src/kernel/command_center/search.rs");
  const types = source("../src/lib/types.ts");
  const terms = source("../src/lib/i18n/ui-terms.ts");
  const rail = source("../src/lib/components/workbench/ActivityRail.svelte");
  const center = source("../src/lib/components/workspace/WorkspaceCenterArea.svelte");

  assert.match(
    rustWorkbench.slice(
      rustWorkbench.indexOf("pub enum WorkbenchActivity"),
      rustWorkbench.indexOf("pub enum WorkbenchSurface"),
    ),
    /Content,\s+Taxonomies,\s+Data,/,
  );
  assert.match(types, /\|\s*"content"\s+\|\s*"taxonomies"\s+\|\s*"data"/);
  assert.match(terms, /taxonomies:\s*"workbench-taxonomies"/);
  assert.match(rail, /id:\s*"taxonomies"[\s\S]*UI_TERM_IDS\.taxonomies/);
  assert.match(rail, /activity\.id === "taxonomies"[\s\S]*<IconTags/);
  assert.match(center, /activeWorkbenchActivity === "taxonomies"[\s\S]*<TaxonomiesWorkspace/);
  assert.match(rustCommands, /WorkbenchActivity::Taxonomies[\s\S]*"Taxonomii"/);
});

test("catalogul și toate mutațiile taxonomice rămân autoritate Rust", () => {
  const registry = source("../src-tauri/src/tauri_command_registry.rs");
  const command = source("../src-tauri/src/commands/taxonomies.rs");
  const catalog = source("../src-tauri/src/source_graph/taxonomy_catalog.rs");
  const mutation = source("../src-tauri/src/kernel/taxonomy_mutation.rs");
  const io = source("../src/lib/project/io.ts");
  const types = source("../src/lib/types.ts");

  for (const commandName of [
    "read_taxonomy_catalog",
    "plan_taxonomy_mutation",
    "apply_taxonomy_mutation",
  ]) assert.match(registry, new RegExp(commandName));
  assert.match(catalog, /Config::parse/);
  assert.match(catalog, /slugify_paths/);
  assert.match(catalog, /taxonomy_root/);
  assert.match(catalog, /taxonomy_list\.html/);
  assert.match(catalog, /taxonomy_single\.html/);
  assert.match(mutation, /stage_composite_changes/);
  assert.match(mutation, /WorkspaceMutationMetadata/);
  assert.match(command, /commit_project_workspace_session_mutation/);
  assert.match(command, /expected_plan_id/);
  assert.match(io, /snapshot\.schemaVersion !== TAXONOMY_CATALOG_SCHEMA_VERSION/);
  assert.match(io, /plan\.schemaVersion !== TAXONOMY_MUTATION_SCHEMA_VERSION/);
  assert.match(types, /export type TaxonomyCatalogSnapshot/);
  assert.match(types, /kind:\s*"set_page_terms"/);
});

test("activitatea are stare goală, impact, template-uri și confirmare distructivă", () => {
  const workspace = source("../src/lib/components/taxonomies/TaxonomiesWorkspace.svelte");

  assert.match(workspace, /t\("taxonomies-empty-title"\)/);
  assert.match(workspace, /t\("taxonomies-add-first"\)/);
  assert.match(workspace, /t\("taxonomies-effective-templates"\)/);
  assert.match(workspace, /t\("taxonomies-open-templates"\)/);
  assert.match(workspace, /t\("taxonomies-affected-pages"\)/);
  assert.match(workspace, /t\("taxonomies-rust-diagnostics"\)/);
  assert.match(workspace, /expectedUsageCount:\s*entry\.pages\.length/);
  assert.match(workspace, /removeAssignments/);
  assert.match(workspace, /planTaxonomyMutation[\s\S]*applyTaxonomyMutation/);
  assert.match(workspace, /loadedKey === key/);
  assert.match(workspace, /if \(busy\) return false/);
});

test("Conținut atribuie dinamic taxonomiile din catalog, fără Tags/Categories hardcodate", () => {
  const content = source("../src/lib/components/content/ContentWorkspace.svelte");
  const assignments = source("../src/lib/components/content/PageTaxonomyAssignments.svelte");
  const legacyPanel = source("../src/lib/components/project/ProjectPageSettingsTab.svelte");
  const frontmatter = source("../src/lib/markdown/frontmatter.ts");

  assert.match(content, /<PageTaxonomyAssignments \{app\} page=\{selectedPage\}/);
  assert.match(assignments, /readTaxonomyCatalog/);
  assert.match(assignments, /entry\.declared && entry\.language === pageLanguage/);
  assert.match(assignments, /kind:\s*"set_page_terms"/);
  assert.match(assignments, /pageFile:\s*page\.file/);
  assert.match(assignments, /planTaxonomyMutation[\s\S]*applyTaxonomyMutation/);
  assert.doesNotMatch(legacyPanel, />Tags<|>Categories<|taxonomy-note/);
  assert.doesNotMatch(frontmatter, /taxonomies\.tags|taxonomies\.categories/);
});
