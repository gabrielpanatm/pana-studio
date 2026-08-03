import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("registry-ul Rust definește DynamicValue v2, migrare și limite sursă versionate", () => {
  const registry = source("../src-tauri/src/kernel/dynamic_widgets.rs");
  assert.match(registry, /DYNAMIC_WIDGET_SCHEMA_VERSION:\s*u32\s*=\s*2/);
  assert.match(registry, /LEGACY_DYNAMIC_WIDGET_SCHEMA_VERSION:\s*u32\s*=\s*1/);
  assert.match(registry, /"dynamic-field"/);
  assert.match(registry, /"listing"/);
  assert.match(registry, /pub value_catalog:/);
  assert.match(registry, /DynamicValueSource/);
  assert.match(registry, /migrate_legacy_properties/);
  assert.match(registry, /TrustedContent/);
  assert.match(registry, /ListObject/);
  assert.match(registry, /tera_access/);
  assert.match(registry, /START_MARKER_PREFIX/);
  assert.match(registry, /replace_dynamic_widget_source/);
  assert.match(registry, /source_revision/);
  assert.match(registry, /canonical_binding_expression/);
  assert.match(registry, /get_section\(path=/);
  assert.match(registry, /\{% for item in/);
  assert.match(registry, /\{% include/);
});

test("snapshotul inspectorului refuză workspace, model, preview și sursă stale", () => {
  const commands = source("../src-tauri/src/commands/dynamic_widgets.rs");
  for (const field of [
    "expected_workspace_revision",
    "expected_model_revision",
    "preview_revision",
    "source_instance_id",
    "expected_source_revision",
  ]) assert.match(commands, new RegExp(field));
  assert.match(commands, /dynamic_widget_stale_model/);
  assert.match(commands, /dynamic_widget_stale_preview/);
  assert.match(commands, /replace_dynamic_widget_source/);
  assert.match(commands, /finish_mutation/);
});

test("BlockPropertiesPane este gazda comună pentru bloc nativ și widget dinamic", () => {
  const host = source("../src/lib/components/inspector/BlockPropertiesPane.svelte");
  const editor = source("../src/lib/components/inspector/DynamicWidgetPropertiesEditor.svelte");
  const area = source("../src/lib/components/workspace/WorkspaceInspectorArea.svelte");
  const app = source("../src/lib/state/app.svelte.ts");

  assert.match(host, /dynamicSelectionContext/);
  assert.match(host, /readDynamicWidgetSnapshot/);
  assert.match(host, /<DynamicWidgetPropertiesEditor/);
  assert.match(editor, /inspector-dynamic-model/);
  assert.match(editor, /valueCatalog/);
  assert.match(editor, /Source|Sursă|context/);
  assert.match(editor, /compatiblePresentations/);
  assert.match(editor, /type="search"/);
  assert.match(editor, /sourceGroups/);
  assert.match(editor, /chooseSourceGroup/);
  assert.match(editor, /canonicalBindingExpression/);
  assert.match(editor, /inspector-dynamic-advanced/);
  assert.match(editor, /inspector-dynamic-listing-item/);
  assert.match(editor, /inspector-dynamic-include-subsections/);
  assert.match(area, /inspectorDynamicWidgetSelectionContext=\{app\.inspectorDynamicWidgetSelectionContext\}/);
  assert.match(app, /dynamicWidgetSourceInstanceIds/);
  assert.match(app, /selectDynamicWidgetSourceInstance/);
  assert.match(app, /updateDynamicWidgetFromInspector/);
  assert.match(app, /settleProjectWorkspaceMutation\(this, receipt/);
});

test("Straturi primește eticheta semantică a rădăcinii Dynamic Field din catalogul Rust", () => {
  const navigation = source("../src-tauri/src/kernel/editor_navigation.rs");
  assert.match(navigation, /dynamic_widget_navigation_label/);
  assert.match(navigation, /root_source_node_ids/);
  assert.match(navigation, /Câmp dinamic · \{label\}/);
});

test("mutațiile structurale tratează widgetul dinamic ca envelope atomic", () => {
  const envelope = source("../src-tauri/src/project_model/structural_envelope.rs");
  const move = source("../src-tauri/src/project_model/move_engine.rs");
  const remove = source("../src-tauri/src/project_model/delete_engine.rs");
  const duplicate = source("../src-tauri/src/project_model/duplicate_engine.rs");
  const insert = source("../src-tauri/src/project_model/insert_engine.rs");
  const teraInsert = source("../src-tauri/src/project_model/tera_insert_engine.rs");

  assert.match(envelope, /start marker \+ generated body \+ end marker/);
  assert.match(envelope, /dynamic_widget_for_node/);
  for (const engine of [move, remove, duplicate, insert, teraInsert]) {
    assert.match(engine, /structural_envelope_for_html_node/);
  }
  assert.match(move, /preserves_internal_indentation/);
  assert.match(duplicate, /generate_dynamic_widget_instance_id/);
  assert.match(duplicate, /dynamic_widget_contract/);
  assert.match(teraInsert, /html_inside_insert_index/);
});

test("Șabloane administrează Listing Item, iar Adaugă element inițiază widgeturile", () => {
  const templates = source("../src/lib/components/templates/TemplatesWorkspace.svelte");
  const catalog = source("../src-tauri/src/kernel/insert_catalog.rs");
  const adapter = source("../src/lib/state/insert-catalog-drag-controller.ts");

  assert.match(templates, /id:\s*"listing_item"/);
  assert.match(templates, /createListingItem/);
  assert.match(templates, /deleteListingItem/);
  assert.match(catalog, /DynamicWidget/);
  assert.match(catalog, /Câmp dinamic/);
  assert.match(catalog, /Listing/);
  assert.match(catalog, /Câmpuri directe/);
  assert.doesNotMatch(source("../src/lib/components/project/InsertCatalogPanel.svelte"), /id:\s*"directField"/);
  assert.match(adapter, /dynamicWidget/);
});
