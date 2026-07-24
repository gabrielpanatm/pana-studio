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
  const types = source("../src/lib/types.ts");
  const workbench = source("../src-tauri/src/kernel/workbench/model.rs");

  assert.match(rail, /id:\s*"templates"/);
  assert.match(center, /activeWorkbenchActivity === "templates"[\s\S]*<TemplatesWorkspace/);
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
  assert.match(catalog, /fn affected_pages/);
  assert.match(catalog, /local_template_names/);
  assert.match(command, /read_template_catalog[\s\S]*build_template_catalog/);
  assert.match(templates, /readTemplateCatalog/);
  assert.match(source("../src/lib/project/io.ts"), /TEMPLATE_CATALOG_SCHEMA_VERSION/);
  assert.doesNotMatch(templates, /CodeMirror|Monaco|contenteditable|<textarea/);
});

test("operațiile șabloanelor trec prin ProjectWorkspace și păstrează o redenumire atomică", () => {
  const commands = source("../src-tauri/src/commands/templates.rs");
  const registry = source("../src-tauri/src/tauri_command_registry.rs");
  const frontend = source("../src/lib/project/io.ts");

  for (const command of [
    "workspace_create_template",
    "workspace_duplicate_template",
    "workspace_override_theme_template",
    "workspace_rename_template",
    "workspace_delete_template",
  ]) {
    assert.match(registry, new RegExp(command));
    assert.match(frontend, new RegExp(`"${command}"`));
  }

  assert.match(commands, /require_bound_workspace/);
  assert.match(commands, /finish_mutation/);
  assert.match(commands, /build_source_graph_from_workspace_projection/);
  assert.match(
    commands,
    /workspace_override_theme_template[\s\S]*build_template_catalog[\s\S]*entry\.effective[\s\S]*!entry\.editable/,
  );
  assert.match(commands, /plan_template_reference_workspace_mutation_from_graph/);
  assert.match(commands, /stage_composite_changes/);
  assert.match(commands, /delete_blocked_reason/);
  assert.doesNotMatch(commands, /std::fs::(?:write|remove_file|rename)/);
});

test("Deschide în Editor folosește editorul existent, nu creează o suprafață duplicată", () => {
  const templates = source("../src/lib/components/templates/TemplatesWorkspace.svelte");
  const open = templates.slice(
    templates.indexOf("async function openPathInEditor"),
    templates.indexOf("function roleLabel"),
  );

  assert.match(open, /await openWorkspaceSource\(path\)/);
  assert.match(open, /await app\.setWorkbenchActivity\("editor"\)/);
  assert.match(templates, /openPathInEditor\(usage\.file\)/);
  assert.match(templates, /openPathInEditor\(page\.file\)/);
  assert.match(templates, />\s*Deschide în Editor\s*</);
});

test("panoul contextual al șabloanelor separă informarea, crearea și editarea", () => {
  const templates = source("../src/lib/components/templates/TemplatesWorkspace.svelte");

  assert.match(templates, /type DetailMode = "info" \| "create" \| "edit"/);
  assert.match(templates, /function beginCreate\(\)[\s\S]*detailMode = "create"/);
  assert.match(templates, /function beginEdit\(entry: TemplateCatalogEntry\)[\s\S]*detailMode = "edit"/);
  assert.match(templates, /detailMode === "create"[\s\S]*onsubmit=\{submitCreate\}/);
  assert.match(templates, /detailMode === "edit"[\s\S]*submitEdit\(event, selected\)/);
  assert.match(templates, />\s*Editează\s*</);
  assert.match(templates, />\s*Adaugă șablon\s*</);
  assert.match(templates, /deleteConfirmationOpen/);
  assert.doesNotMatch(templates, /window\.(?:prompt|confirm)/);
});

test("formularele șabloanelor păstrează comenzile Rust drept autoritate de mutație", () => {
  const templates = source("../src/lib/components/templates/TemplatesWorkspace.svelte");
  const createFlow = templates.slice(
    templates.indexOf("async function submitCreate"),
    templates.indexOf("async function submitEdit"),
  );
  const editFlow = templates.slice(
    templates.indexOf("async function submitEdit"),
    templates.indexOf("async function overrideSelected"),
  );

  assert.match(createFlow, /duplicateTemplate/);
  assert.match(createFlow, /createTemplate/);
  assert.match(editFlow, /renameTemplate/);
  assert.match(templates, /app\.rescanCurrentProject\(receipt\.relativePath, \{ strict: true \}\)/);
  assert.match(templates, /Ctrl\+S persistă pe disc/);
});
