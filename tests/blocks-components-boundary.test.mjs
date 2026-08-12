import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("ComponentGraph rămâne exclusiv semantic Zola/Tera", () => {
  const model = source("../src-tauri/src/source_graph/model.rs");
  const graph = source("../src-tauri/src/source_graph/component_graph.rs");
  const workspace = source("../src/lib/components/creation/ComponentsWorkspace.svelte");
  const types = source("../src/lib/types.ts");

  assert.match(model, /pub struct ComponentGraph/);
  assert.match(graph, /ComponentDefinitionKind::Shortcode/);
  assert.match(graph, /ComponentDefinitionKind::InlineRepeat/);
  assert.doesNotMatch(model, /Blueprint|RuntimeProvider/);
  assert.doesNotMatch(graph, /Blueprint|RuntimeProvider|data-pana-block/);
  assert.doesNotMatch(types, /"blueprint"|"runtimeProvider"/);
  assert.doesNotMatch(workspace, /NativeBlock|readNativeBlockRegistry|blockGraph/);
});

test("BlockGraph deține sursa, iar UiBlockGraph unește explicit Canvas-ul", () => {
  const model = source("../src-tauri/src/source_graph/model.rs");
  const graph = source("../src-tauri/src/blocks/graph.rs");
  const canvas = source("../src-tauri/src/preview/canvas.rs");
  const workspace = source("../src/lib/components/creation/BlocksWorkspace.svelte");

  assert.match(model, /pub struct BlockDefinition/);
  assert.match(model, /pub struct BlockSourceInstance/);
  assert.match(model, /pub struct BlockGraph/);
  const blockGraphModel = model.slice(
    model.indexOf("pub struct BlockGraph"),
    model.indexOf("pub enum BlockOrigin"),
  );
  assert.doesNotMatch(blockGraphModel, /rendered_instances/);
  assert.match(graph, /native_block_provider_definitions/);
  assert.match(graph, /SourceNodeKind::BlockMarker/);
  assert.match(canvas, /derive_block_instances/);
  assert.match(workspace, /readUiBlockGraph/);
  assert.match(workspace, /uiBlockGraph\?\.renderedInstances/);
});

test("registrul Rust este autoritatea unică pentru cei opt provideri nativi", () => {
  const native = source("../src-tauri/src/blocks/native.rs");
  const commands = source("../src-tauri/src/commands/blocks.rs");
  const registry = source("../src-tauri/src/tauri_command_registry.rs");
  const io = source("../src/lib/project/io.ts");

  for (const blockId of ["icon", "counter", "accordion", "tabs", "slider", "dialog", "offcanvas", "nav-menu"]) {
    assert.match(native, new RegExp(`id: "${blockId}"`));
  }
  assert.match(native, /data-pana-block=/);
  assert.doesNotMatch(native, /#[0-9a-fA-F]{3,8}|rgba?\(/);
  for (const command of [
    "read_native_block_registry",
    "plan_native_block_contract",
    "apply_native_block_contract",
    "read_block_runtime_snapshot",
    "read_ui_block_graph",
    "read_icon_catalog",
    "search_icon_catalog",
  ]) {
    assert.match(commands, new RegExp(command));
    assert.match(registry, new RegExp(command));
    assert.match(io, new RegExp(`"${command}"`));
  }
});

test("taxonomia separă elementele, compozițiile și secțiunile page-level", () => {
  const native = source("../src-tauri/src/blocks/native.rs");
  const model = source("../src-tauri/src/source_graph/model.rs");
  const workspace = source("../src/lib/components/creation/BlocksWorkspace.svelte");

  for (const blockId of ["accordion", "tabs", "slider", "dialog", "offcanvas", "nav-menu"]) {
    const definitionStart = native.indexOf(`id: "${blockId}"`);
    const definitionKind = native.indexOf("kind:", definitionStart);
    assert.notEqual(definitionStart, -1);
    assert.notEqual(definitionKind, -1);
    assert.match(native.slice(definitionStart, definitionKind), /scale: BlockScale::Composition/);
  }
  assert.match(model, /zonă completă de pagină\.\n\s+Section,/);
  assert.doesNotMatch(native, /scale: BlockScale::Section/);
  assert.match(workspace, /availableNativeBlockScales\(definitions\)/);
  assert.match(workspace, /dynamicFields\.length > 0/);
});

test("Slider este Composition Rust-first și se editează exclusiv în panoul Blocuri", () => {
  const native = source("../src-tauri/src/blocks/native.rs");
  const slots = source("../src-tauri/src/blocks/slots.rs");
  const runtime = source("../src-tauri/src/blocks/runtime.js");
  const blockPane = source("../src/lib/components/inspector/BlockPropertiesPane.svelte");
  const sliderEditor = source("../src/lib/components/inspector/SliderBlockPropertiesEditor.svelte");
  const htmlPane = source("../src/lib/components/inspector/HtmlPane.svelte");

  assert.match(native, /id: "slider"[\s\S]*scale: BlockScale::Composition[\s\S]*tag: "div"/);
  assert.match(native, /id: "slides"[\s\S]*minimum_items: 1[\s\S]*maximum_items: Some\(32\)/);
  assert.match(native, /data-pana-slider-slide/);
  assert.match(slots, /NativeBlockSlotMutationContext/);
  assert.match(slots, /render_native_block_slot_item_html/);
  assert.match(slots, /Slider în slider este blocat/);
  assert.match(runtime, /structureSignature/);
  assert.match(runtime, /register\("slider"/);
  assert.match(blockPane, /<SliderBlockPropertiesEditor/);
  assert.match(sliderEditor, /operation:\s*"insert" \| "duplicate" \| "move" \| "delete"|request\("insert"\)/);
  assert.doesNotMatch(htmlPane, /SliderBlockPropertiesEditor|data-pana-slider|inspector-slider/);
});

test("Icon este bloc static Rust-first, iar editorul lui există exclusiv în panoul Blocuri", () => {
  const native = source("../src-tauri/src/blocks/native.rs");
  const icons = source("../src-tauri/src/blocks/icons.rs");
  const insert = source("../src-tauri/src/project_model/insert_engine.rs");
  const sourceScan = source("../src-tauri/src/source_graph/scan/template.rs");
  const blockPane = source("../src/lib/components/inspector/BlockPropertiesPane.svelte");
  const iconEditor = source("../src/lib/components/inspector/IconBlockPropertiesEditor.svelte");
  const htmlPane = source("../src/lib/components/inspector/HtmlPane.svelte");
  const io = source("../src/lib/project/io.ts");

  assert.match(native, /enum NativeBlockKind[\s\S]*Static/);
  assert.match(native, /id: "icon"[\s\S]*kind: NativeBlockKind::Static/);
  assert.match(native, /scale: BlockScale::Element/);
  assert.match(native, /render_icon_block_html/);
  assert.match(icons, /include_str!\("\.\.\/\.\.\/resources\/icon-packs\/tabler-outline-3\.41\.1\.json"\)/);
  assert.match(icons, /MAX_PAGE_LIMIT:\s*usize\s*=\s*96/);
  assert.match(icons, /normalize_icon_identity/);
  assert.match(icons, /validate_node/);
  assert.match(insert, /render_native_block_html/);
  assert.match(sourceScan, /is_managed_icon_descendant/);
  assert.match(blockPane, /<IconBlockPropertiesEditor/);
  assert.match(iconEditor, /searchIconCatalog\(\{/);
  assert.match(iconEditor, /offset:\s*currentOffset/);
  assert.match(iconEditor, /limit:\s*48/);
  assert.match(iconEditor, /window\.setTimeout[\s\S]*140/);
  assert.match(io, /invoke<IconCatalogSummary>\("read_icon_catalog"\)/);
  assert.match(io, /invoke<IconCatalogPage>\("search_icon_catalog"/);
  assert.doesNotMatch(htmlPane, /IconBlockPropertiesEditor|searchIconCatalog|data-pana-icon/);
});

test("proprietățile blocurilor sunt definite și validate exclusiv în Rust", () => {
  const native = source("../src-tauri/src/blocks/native.rs");
  const options = source("../src-tauri/src/blocks/options.rs");
  const attributes = source("../src-tauri/src/project_model/attribute_engine.rs");
  const inspector = source("../src/lib/components/inspector/BlockPropertiesPane.svelte");
  const canvasPatch = source("../src-tauri/src/preview/bridge/10_canvas_patch.js");

  assert.match(native, /NativeBlockOptionDefinition/);
  assert.match(native, /COUNTER_OPTIONS/);
  assert.match(native, /OFFCANVAS_OPTIONS/);
  assert.match(options, /plan_native_block_option_attribute/);
  assert.match(options, /Marcajul data-pana-component.*read-only/s);
  assert.match(attributes, /native_block_option/);
  assert.match(inspector, /readUiBlockGraph/);
  assert.match(inspector, /onblur=\{\(\) => \{ void commit\(option\); \}\}/);
  assert.doesNotMatch(inspector, /data-tinta|data-multiple|data-close-outside/);
  assert.match(canvasPatch, /operation\.kind === "setBlockOption"/);
});

test("selecția unui descendent alege rădăcina celui mai apropiat bloc imbricat", () => {
  const embeddedBridge = source("../src-tauri/src/preview/bridge/02_css_inspection.js");
  const canvasAgent = source("../src-tauri/src/preview/bridge/03_canvas_agent.js");
  const app = source("../src/lib/state/app.svelte.ts");
  const inspector = source("../src/lib/components/inspector/BlockPropertiesPane.svelte");
  const registry = source("../src/lib/blocks/registry.ts");
  const types = source("../src/lib/types.ts");
  const navigation = source("../src-tauri/src/kernel/editor_navigation.rs");
  const navigationNodeType = types.slice(
    types.indexOf("export type EditorNavigationNode ="),
    types.indexOf("export type EditorNavigationRelation ="),
  );
  const navigationNodeStruct = navigation.slice(
    navigation.indexOf("pub struct EditorNavigationNode"),
    navigation.indexOf("pub struct EditorNavigationViewNode"),
  );

  assert.match(embeddedBridge, /element\.closest\("\[data-pana-block\],\[data-pana-component\]"\)/);
  assert.match(embeddedBridge, /markerKind:\s*canonical \? "canonical" : "legacy"/);
  assert.match(canvasAgent, /physicalBlockContext/);
  assert.doesNotMatch(canvasAgent, /rootSourceId|rootTemplateSourceId|rootSessionId/);
  assert.match(app, /bounded\.providerId !== physical\.providerId/);
  assert.match(app, /navigationNode\?\.renderInstanceId === coordinated\.renderInstanceId/);
  assert.match(app, /rootSourceId: navigationOwnsSelection/);
  assert.match(app, /\? \[\.\.\.navigationNode\.blockSourceInstanceIds\]/);
  assert.match(navigationNodeType, /renderInstanceId: string \| null/);
  assert.doesNotMatch(navigationNodeType, /renderInstanceIds/);
  assert.match(navigationNodeStruct, /pub render_instance_id: Option<String>/);
  assert.doesNotMatch(navigationNodeStruct, /render_instance_ids/);
  assert.match(app, /rootSessionId: coordinated\.snapshot\.runtimeSessionId/);
  assert.match(inspector, /resolveUiBlockSourceInstanceForSelection\(graph, blockContext\)/);
  assert.match(registry, /Array\.isArray\(selection\.sourceInstanceIds\)/);
  assert.match(registry, /sourceInstanceIds\.length - 1/);
  assert.match(registry, /sourceInstanceMatchesSelection\(instance, selection\)/);
  assert.doesNotMatch(inspector, /querySelector\(|getAttribute\(/);
});

test("preview și site folosesc același runtime canonic de blocuri", () => {
  const runtime = source("../src-tauri/src/blocks/runtime.js");
  const generator = source("../src-tauri/src/js/generator.rs");
  const interactive = source("../src-tauri/src/preview/interactive_runtime.js");
  const inject = source("../src-tauri/src/preview/inject.rs");

  assert.match(generator, /install_native_block_runtime/);
  assert.match(inject, /NATIVE_BLOCK_RUNTIME_SCRIPT/);
  assert.match(interactive, /window\.PanaBlockRuntime/);
  assert.doesNotMatch(interactive, /function (?:counter|accordion|tabs|overlay|navMenu)Definition/);
  assert.doesNotMatch(generator, /generate_(?:counter|accordion|tabs|dialog|offcanvas|nav_menu)_component/);
  assert.match(runtime, /cancelAnimationFrame/);
  assert.match(runtime, /IntersectionObserver/);
  assert.match(runtime, /removeEventListener/);
  assert.match(runtime, /media\.removeListener/);
  assert.match(runtime, /document\.body\.style\.overflow/);
  assert.match(runtime, /aria-expanded/);
});

test("scrierea structurală reconciliază markup, SCSS și Page JS într-o singură tranzacție", () => {
  const structural = source("../src-tauri/src/kernel/preview_projection/structural_write.rs");
  const frontend = source("../src/lib/state/html-actions-controller.ts");

  assert.match(structural, /stage_structural_write_with_native_block_contract/);
  assert.match(structural, /plan_native_block_contract/);
  assert.match(structural, /stage_composite_changes/);
  assert.match(structural, /native_block_insert_and_last_delete_are_atomic_and_noop_safe/);
  assert.doesNotMatch(frontend, /applyNativeBlockContract|reconcileNativeBlock/);
});

test("activitatea Blocuri părăsește inserarea numai după confirmarea commitului", () => {
  const controller = source("../src/lib/state/html-actions-controller.ts");
  const app = source("../src/lib/state/app.svelte.ts");
  const workspace = source("../src/lib/components/creation/BlocksWorkspace.svelte");

  assert.match(
    controller,
    /insertPaletteElementAtTarget\([\s\S]*?Promise<EditorActionOutcome>/,
  );
  assert.match(controller, /return committedAction\(\);/);
  assert.match(
    app,
    /return await insertPaletteElementAtTargetFromController\(/,
  );
  assert.match(
    workspace,
    /const outcome = await app\.insertPaletteElementAtTarget\([\s\S]*?if \(outcome\.status !== "committed"\)/,
  );
  assert.ok(
    workspace.indexOf('if (outcome.status !== "committed")')
      < workspace.indexOf('await app.setWorkbenchActivity("editor")'),
  );
});

test("compatibilitatea legacy este citire controlată, nu un al doilea model", () => {
  const contract = source("../src-tauri/src/blocks/contract.rs");
  const runtime = source("../src-tauri/src/blocks/runtime.js");
  const scanner = source("../src-tauri/src/source_graph/scan/template.rs");
  const jsTypes = source("../src-tauri/src/js/types.rs");
  const generator = source("../src-tauri/src/js/generator.rs");

  for (const file of [contract, runtime, scanner]) {
    assert.match(file, /data-pana-component/);
    assert.match(file, /data-pana-block/);
  }
  assert.match(jsTypes, /alias = "components"/);
  assert.doesNotMatch(jsTypes, /rename = "components"/);
  assert.doesNotMatch(generator, /output\.push_str\("\/\/ @pana-component/);
  assert.equal(
    existsSync(new URL("../src-tauri/src/page_components/mod.rs", import.meta.url)),
    false,
  );
  assert.equal(
    existsSync(new URL("../src/lib/page-components/registry.ts", import.meta.url)),
    false,
  );
});
