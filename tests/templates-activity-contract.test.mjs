import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("Șabloane este distinctă de catalogul semantic al componentelor Tera", () => {
  const rail = source("../src/lib/components/workbench/ActivityRail.svelte");
  const center = source("../src/lib/components/workspace/WorkspaceCenterArea.svelte");
  const components = source("../src/lib/components/creation/ComponentsWorkspace.svelte");
  const types = source("../src/lib/workbench/contracts.ts");
  const workbench = source("../src-tauri/src/kernel/workbench/model.rs");

  assert.match(rail, /id:\s*"templates"/);
  assert.match(center, /retainedAuxiliarySurface === "templates"[\s\S]*<TemplatesWorkspace/);
  assert.match(types, /export type WorkbenchActivity[\s\S]*\|\s*"templates"/);
  assert.match(workbench, /#\[serde\(alias = "site"\)\]\s*Templates/);

  assert.match(components, /ComponentView[\s\S]*"partials"/);
  assert.match(components, /ComponentView[\s\S]*"macros"/);
  assert.match(components, /ComponentView[\s\S]*"shortcodes"/);
  assert.match(components, /ComponentView[\s\S]*"repeats"/);
  assert.doesNotMatch(components, /sourceGraph\?\.templates|SourceGraphTemplate/);
  assert.match(components, /sourceGraph\?\.componentGraph/);
  assert.doesNotMatch(components, /readNativeBlockRegistry|readBlockRuntimeSnapshot|blockGraph/);
});

test("catalogul semantic și impactul șabloanelor au autoritate Rust unică", () => {
  const catalog = source("../src-tauri/src/source_graph/template_catalog.rs");
  const command = source("../src-tauri/src/commands/source_graph.rs");
  const templates = source("../src/lib/components/templates/TemplatesWorkspace.svelte");

  assert.equal(
    existsSync(new URL("../src/lib/source-graph/architecture.ts", import.meta.url)),
    false,
  );
  assert.equal(
    existsSync(new URL("../src/lib/source-graph/workspace-selection.ts", import.meta.url)),
    false,
  );
  assert.match(catalog, /pub enum TemplateCatalogRole/);
  assert.match(catalog, /pub struct TemplateResource/);
  assert.match(catalog, /pub struct TemplateSemanticEntry/);
  assert.match(catalog, /pub enum TemplateSemanticCategory/);
  assert.match(catalog, /pub enum TemplateSemanticRole/);
  assert.match(catalog, /pub enum TemplateAssignmentSource/);
  assert.match(catalog, /fn affected_pages/);
  assert.match(catalog, /fn semantic_template_assignments/);
  assert.match(catalog, /fn build_semantic_entries/);
  assert.doesNotMatch(catalog, /TemplateCatalogCollection|fn template_collections/);
  assert.match(catalog, /local_template_names/);
  assert.match(command, /read_template_catalog[\s\S]*build_template_catalog_with_taxonomies/);
  assert.match(templates, /readTemplateCatalog/);
  assert.match(templates, /catalog\?\.semanticEntries/);
  assert.doesNotMatch(templates, /catalog\?\.collections|selectedCollection/);
  assert.match(source("../src/lib/templates/io.ts"), /TEMPLATE_CATALOG_SCHEMA_VERSION/);
  assert.doesNotMatch(templates, /CodeMirror|Monaco|contenteditable|<textarea/);
});

test("operațiile șabloanelor trec prin ProjectWorkspace și păstrează o redenumire atomică", () => {
  const commands = source("../src-tauri/src/commands/templates.rs");
  const registry = source("../src-tauri/src/tauri_command_registry.rs");
  const frontend = source("../src/lib/templates/io.ts");

  for (const command of [
    "workspace_create_semantic_template",
    "workspace_duplicate_template",
    "workspace_override_theme_template",
    "workspace_rename_template",
    "workspace_set_template_parent",
    "workspace_set_template_assignment",
    "workspace_delete_template",
  ]) {
    assert.match(registry, new RegExp(command));
  }
  for (const activeFrontendCommand of [
    "workspace_create_semantic_template",
    "workspace_duplicate_template",
    "workspace_override_theme_template",
    "workspace_rename_template",
    "workspace_set_template_parent",
    "workspace_set_template_assignment",
    "workspace_delete_template",
  ]) assert.match(frontend, new RegExp(`"${activeFrontendCommand}"`));
  assert.doesNotMatch(
    `${commands}\n${registry}\n${frontend}`,
    /\bworkspace_create_template(?:_collection)?\b/,
  );

  assert.match(commands, /require_bound_workspace/);
  assert.match(commands, /finish_mutation/);
  assert.match(commands, /build_source_graph_from_workspace_projection/);
  assert.match(
    commands,
    /workspace_override_theme_template[\s\S]*build_template_catalog[\s\S]*entry\.effective[\s\S]*!entry\.editable/,
  );
  assert.match(commands, /plan_template_reference_workspace_mutation_from_graph/);
  assert.match(commands, /stage_composite_changes/);
  assert.match(commands, /delete_blocked_diagnostic/);
  assert.doesNotMatch(commands, /std::fs::(?:write|remove_file|rename)/);
});

test("Deschide în Editor folosește editorul existent, nu creează o suprafață duplicată", () => {
  const templates = source("../src/lib/components/templates/TemplatesWorkspace.svelte");
  assert.match(
    templates,
    /async function openResource[\s\S]*await openWorkspaceSource\(resource\.file/,
  );
  assert.match(templates, /await openEditor\(\)/);
  assert.match(templates, /t\("templates-edit-visual"\)/);
});

test("rolurile semantice se deschid vizual numai în contextul exact proiectat de Rust", () => {
  const templates = source("../src/lib/components/templates/TemplatesWorkspace.svelte");

  assert.match(
    templates,
    /const context = entry\.previewContext/,
  );
  assert.match(
    templates,
    /context\?\.available && context\.pageFile[\s\S]*surface:\s*"visual"[\s\S]*templateContextPagePath:\s*context\.pageFile/,
  );
  assert.match(
    templates,
    /context\?\.available && context\.url[\s\S]*surface:\s*"visual"[\s\S]*templateContextUrl:\s*context\.url/,
  );
  assert.match(templates, /previewContext\?\.unavailableDiagnostic/);
});

test("activitatea prezintă ierarhia Mosaic adaptată la Zola fără tabul Toate", () => {
  const templates = source("../src/lib/components/templates/TemplatesWorkspace.svelte");

  assert.match(templates, /type DetailMode = "info" \| "create" \| "rename"/);
  assert.match(templates, /\{ id: "layout" as const, label: t\("templates-view-layouts"\) \}/);
  assert.match(templates, /\{ id: "page" as const, label: t\("templates-view-pages"\) \}/);
  assert.match(templates, /\{ id: "archive" as const, label: t\("templates-view-archives"\) \}/);
  assert.match(templates, /\{ id: "element" as const, label: t\("templates-view-elements"\) \}/);
  assert.match(templates, /\{ id: "taxonomy" as const, label: t\("templates-view-taxonomies"\) \}/);
  assert.match(templates, /\{ id: "system" as const, label: t\("templates-view-system"\) \}/);
  assert.doesNotMatch(templates, /\{ id: "all"/);
  assert.doesNotMatch(templates, /templates-view-(?:partials|macros)/);
  assert.match(templates, /function beginCreate\(\)[\s\S]*detailMode = "create"/);
  assert.match(templates, /NEW_SECTION_TARGET/);
  assert.match(templates, /templates-create-new-section/);
  assert.match(templates, /content\/\{createSectionSlug \|\| "…"\}\/_index\.md/);
  assert.match(templates, /function beginRename\(resource: TemplateResource\)[\s\S]*detailMode = "rename"/);
  assert.match(templates, /detailMode === "create"[\s\S]*onsubmit=\{submitCreate\}/);
  assert.match(templates, /detailMode === "rename"[\s\S]*submitRename\(event, selectedResource\)/);
  assert.doesNotMatch(templates, /Colecție \(listă \+ element\)|createTemplateCollection/);
  assert.match(templates, /t\("templates-assignment-source-explicit"\)/);
  assert.match(templates, /t\("templates-assignment-source-inherited"\)/);
  assert.match(templates, /t\("templates-assignment-source-default"\)/);
  assert.match(templates, /t\("templates-assignment-source-convention"\)/);
  assert.match(templates, /deleteConfirmationOpen/);
  assert.doesNotMatch(templates, /window\.(?:prompt|confirm)/);
});

test("formularele șabloanelor păstrează comenzile Rust drept autoritate de mutație", () => {
  const templates = source("../src/lib/components/templates/TemplatesWorkspace.svelte");
  const createFlow = templates.slice(
    templates.indexOf("async function submitCreate"),
    templates.indexOf("async function submitRename"),
  );
  const editFlow = templates.slice(
    templates.indexOf("async function submitRename"),
    templates.indexOf("async function saveAssignment"),
  );

  assert.match(createFlow, /duplicateTemplate/);
  assert.match(createFlow, /createSemanticTemplate/);
  assert.match(createFlow, /newSection:\s*creatingNewArchiveSection/);
  assert.match(createFlow, /sortBy:\s*createSectionSort/);
  assert.doesNotMatch(createFlow, /createTemplateCollection/);
  assert.match(editFlow, /renameTemplate/);
  assert.match(templates, /setTemplateParent/);
  assert.match(templates, /setTemplateAssignment/);
  assert.match(templates, /workspaceMutations\.settle\(receipt,/);
  assert.match(templates, /preferredRelativePath: receipt\.relativePath/);
  assert.match(templates, /warningLabel: t\("templates-operation-label"\)/);
  assert.doesNotMatch(templates, /rescanCurrentProject\(receipt\.relativePath/);
  assert.doesNotMatch(templates, /\bwriteProjectFile\b/);
});
