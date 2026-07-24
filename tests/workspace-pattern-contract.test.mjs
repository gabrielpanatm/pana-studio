import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

const workspaces = {
  design: "../src/lib/components/creation/DesignSystemWorkspace.svelte",
  components: "../src/lib/components/creation/ComponentsWorkspace.svelte",
  content: "../src/lib/components/content/ContentWorkspace.svelte",
  assets: "../src/lib/components/creation/AssetsWorkspace.svelte",
  data: "../src/lib/components/data/DataWorkspace.svelte",
};

test("workspaces-urile folosesc același model catalog plus panou contextual", () => {
  for (const [name, path] of Object.entries(workspaces)) {
    const workspace = source(path);
    assert.match(workspace, /type DetailMode = "info" \| "create" \| "edit"/, name);
    assert.match(workspace, /class="workspace-header"/, name);
    assert.match(workspace, /class="workspace-toolbar"/, name);
    assert.match(workspace, /role="tablist"/, name);
    assert.match(workspace, /type="search"/, name);
    assert.match(workspace, /class="[^"]*\btoolbar-action\b[^"]*"/, name);
    assert.match(workspace, /detailMode === "create"/, name);
    assert.match(workspace, /detailMode === "edit"/, name);
    assert.match(workspace, /Adaugă/, name);
    assert.doesNotMatch(workspace, /window\.(?:prompt|confirm)/, name);
  }
});

test("Sistem de design creează și editează prin comenzile ProjectWorkspace", () => {
  const workspace = source(workspaces.design);
  const css = source("../src-tauri/src/commands/css.rs");
  const design = source("../src-tauri/src/commands/design_system.rs");
  const io = source("../src/lib/project/io.ts");

  assert.match(workspace, /app\.createDesignSystemVariable/);
  assert.match(workspace, /app\.createDesignSystemClass/);
  assert.match(workspace, /createProjectTextFile/);
  assert.match(workspace, /downloadGoogleFontFamily/);
  assert.match(workspace, />\s*Editează\s*</);
  assert.match(css, /create_scss_variable[\s\S]*execute_css_workspace_mutation/);
  assert.match(design, /create_design_class[\s\S]*finish_mutation/);
  assert.match(design, /stage_resource_texts/);
  assert.match(io, /"create_scss_variable"/);
  assert.match(io, /"create_design_class"/);
  assert.doesNotMatch(design, /std::fs::write/);
});

test("listele Tera sunt surse reale în ComponentGraph, fără catalog paralel", () => {
  const workspace = source(workspaces.components);
  const palette = source("../src/lib/tera/palette.ts");
  const graph = source("../src-tauri/src/source_graph/component_graph.rs");
  const commands = source("../src-tauri/src/commands/components.rs");
  const app = source("../src/lib/state/app.svelte.ts");
  const route = source("../src/routes/+page.svelte");

  assert.equal(existsSync(new URL("../src/lib/loops/storage.ts", import.meta.url)), false);
  assert.equal(existsSync(new URL("../src/lib/loops/model.ts", import.meta.url)), false);
  assert.equal(
    existsSync(new URL("../src/lib/components/creation/LoopBuilderPanel.svelte", import.meta.url)),
    false,
  );
  assert.doesNotMatch(app, /loadLoopDefinitionsForProject|saveLoopDefinitionsForProject/);
  assert.doesNotMatch(route, /data\/pana-studio\/loops\.json|loadProjectLoopDefinitions/);
  assert.match(palette, /id:\s*"for:items"[\s\S]*kind:\s*"for"/);
  assert.match(palette, /\{% for \$\{item\.expression/);
  assert.match(graph, /ComponentDefinitionKind::InlineRepeat/);
  assert.match(workspace, /app\.sourceGraph\?\.componentGraph/);
  assert.match(workspace, /applyComponentMutation/);
  assert.match(commands, /commit_project_workspace_session_mutation[\s\S]*stage_validated_component_mutation/);
  assert.doesNotMatch(workspace, /registerLoopDefinition|removeLoopDefinition/);
});

test("Conținut are două panouri și elimină fluxul legacy cu prompt", () => {
  const workspace = source(workspaces.content);
  const controller = source("../src/lib/state/project-controller.ts");

  assert.match(workspace, /type ContentView = "all" \| "pages" \| "sections"/);
  assert.match(workspace, /class="section-field"/);
  assert.match(workspace, /class="content-list"/);
  assert.match(workspace, /class="detail-panel"/);
  assert.match(workspace, /app\.createContentPageFromInput/);
  assert.match(workspace, /app\.readPageSettingsDocument/);
  assert.match(workspace, /app\.updatePageFrontmatterSource/);
  assert.doesNotMatch(workspace, /class="collections"/);
  assert.doesNotMatch(controller, /export async function createContentPage\(/);
  assert.doesNotMatch(controller, /window\.prompt/);
});

test("Resurse importă binar create-only prin Rust și expune resursele staged", () => {
  const workspace = source(workspaces.assets);
  const commands = source("../src-tauri/src/commands/page_assets.rs");
  const registry = source("../src-tauri/src/tauri_command_registry.rs");
  const io = source("../src/lib/project/io.ts");

  assert.match(workspace, /type AssetView = "all" \| "images" \| "fonts" \| "other"/);
  assert.match(workspace, /stagedBinaryResources/);
  assert.match(workspace, /chooseAssetFile/);
  assert.match(workspace, /importProjectAsset/);
  assert.match(commands, /import_project_asset/);
  assert.match(commands, /stage_binary_resource_creates/);
  assert.match(commands, /WorkspaceBinaryResource::new/);
  assert.match(commands, /destination_directory != "static"/);
  assert.doesNotMatch(commands, /fs::(?:write|copy|rename)/);
  assert.match(registry, /import_project_asset/);
  assert.match(io, /"import_project_asset"/);
});
