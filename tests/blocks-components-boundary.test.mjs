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

test("registrul Rust este autoritatea unică pentru cei șase provideri nativi", () => {
  const native = source("../src-tauri/src/blocks/native.rs");
  const commands = source("../src-tauri/src/commands/blocks.rs");
  const registry = source("../src-tauri/src/tauri_command_registry.rs");
  const io = source("../src/lib/project/io.ts");

  for (const blockId of ["counter", "accordion", "tabs", "dialog", "offcanvas", "nav-menu"]) {
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
  ]) {
    assert.match(commands, new RegExp(command));
    assert.match(registry, new RegExp(command));
    assert.match(io, new RegExp(`"${command}"`));
  }
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
  const selection = source("../src/lib/preview/selection.ts");
  const embeddedBridge = source("../src-tauri/src/preview/bridge/02_css_inspection.js");
  const inspector = source("../src/lib/components/inspector/BlockPropertiesPane.svelte");

  assert.match(selection, /element\.closest\("\[data-pana-block\],\[data-pana-component\]"\)/);
  assert.match(embeddedBridge, /element\.closest\("\[data-pana-block\],\[data-pana-component\]"\)/);
  assert.match(selection, /markerKind:\s*canonical \? "canonical" : "legacy"/);
  assert.match(inspector, /instance\.rootSourceNodeId === blockContext\.rootSourceId/);
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
