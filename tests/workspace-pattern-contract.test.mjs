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

const canonicalActivityWorkspaces = {
  ...workspaces,
  blocks: "../src/lib/components/creation/BlocksWorkspace.svelte",
  templates: "../src/lib/components/templates/TemplatesWorkspace.svelte",
  taxonomies: "../src/lib/components/taxonomies/TaxonomiesWorkspace.svelte",
  themes: "../src/lib/components/themes/ThemesWorkspace.svelte",
  audit: "../src/lib/components/audit/AuditWorkspace.svelte",
  publish: "../src/lib/components/publish/PublishWorkspace.svelte",
  versioning: "../src/lib/components/VersionsPanel.svelte",
};

const tabbedActivityWorkspaces = {
  design: canonicalActivityWorkspaces.design,
  components: canonicalActivityWorkspaces.components,
  content: canonicalActivityWorkspaces.content,
  assets: canonicalActivityWorkspaces.assets,
  data: canonicalActivityWorkspaces.data,
  blocks: canonicalActivityWorkspaces.blocks,
  templates: canonicalActivityWorkspaces.templates,
  audit: canonicalActivityWorkspaces.audit,
  publish: canonicalActivityWorkspaces.publish,
};

test("workspaces-urile folosesc același model catalog plus panou contextual", () => {
  for (const [name, path] of Object.entries(workspaces)) {
    const workspace = source(path);
    if (name === "content") {
      assert.match(workspace, /type DetailMode = "info" \| "create"/, name);
      assert.match(workspace, /app\.workbenchSnapshot\?\.contentWorkspace\.mode === "edit"/, name);
      assert.match(workspace, /app\.openContentPageEditor\(page\.file\)/, name);
    } else {
      assert.match(workspace, /type DetailMode = "info" \| "create" \| "edit"/, name);
      assert.match(workspace, /detailMode === "edit"/, name);
    }
    assert.match(workspace, /class="workspace-header"/, name);
    assert.match(workspace, /class="workspace-toolbar"/, name);
    assert.match(workspace, /role="tablist"/, name);
    assert.match(workspace, /type="search"/, name);
    assert.match(workspace, /class="[^"]*\btoolbar-action\b[^"]*"/, name);
    assert.match(workspace, /detailMode === "create"/, name);
    assert.match(workspace, /t\("(?:design|components|content|assets|data)-add/, name);
    assert.doesNotMatch(workspace, /window\.(?:prompt|confirm)/, name);
  }
});

test("activitățile folosesc contractul vizual central pentru shell, taburi și toolbar", () => {
  const designSystem = source("../src/routes/design-system.css");

  assert.match(designSystem, /--control-height-toolbar:\s*30px/);
  assert.match(designSystem, /--activity-toolbar-height:\s*42px/);
  assert.match(designSystem, /--activity-search-width:\s*420px/);
  assert.match(designSystem, /--activity-filter-width:\s*164px/);
  assert.match(designSystem, /\.sr-only\s*\{[\s\S]*clip-path:\s*inset\(50%\)/);
  assert.match(designSystem, /body \.app-shell \.activity-workspace\s*\{/);
  assert.match(designSystem, /\.activity-workspace\.activity-workspace-scroll\s*\{/);
  assert.match(designSystem, /\.activity-workspace > \.workspace-toolbar/);
  assert.match(designSystem, /\.activity-workspace \.view-tabs > \.ui-tab/);
  assert.match(designSystem, /\.activity-workspace \.workspace-toolbar > \.toolbar-action/);
  assert.match(designSystem, /\.activity-workspace \.search-field\s*\{[\s\S]*flex:\s*0 1 var\(--activity-search-width\);[\s\S]*max-width:\s*var\(--activity-search-width\)/);
  assert.match(designSystem, /\.activity-workspace \.toolbar-query-group\s*\{[\s\S]*margin-left:\s*auto/);
  assert.match(designSystem, /\.activity-workspace \.toolbar-query-group\.with-filter\s*\{/);
  assert.match(designSystem, /\.activity-workspace \.ui-button:not\(\.compact\):not\(\.toolbar\)\s*\{[\s\S]*min-height:\s*var\(--control-height\)/);
  assert.match(designSystem, /\.activity-workspace :is\(\.ui-button, \.ui-icon-button\)\.compact\s*\{[\s\S]*height:\s*var\(--control-height-compact\)/);

  for (const [name, path] of Object.entries(canonicalActivityWorkspaces)) {
    const workspace = source(path);
    assert.match(
      workspace,
      /class="activity-workspace [^"]+-workspace"/,
      `${name} nu folosește shell-ul canonic`,
    );
  }

  for (const [name, path] of Object.entries(tabbedActivityWorkspaces)) {
    const workspace = source(path);
    assert.match(
      workspace,
      /class="ui-tabs view-tabs"/,
      `${name} nu folosește grupul canonic de taburi`,
    );
    assert.match(workspace, /class="ui-tab"/, `${name} nu folosește tabul canonic`);
  }

  for (const name of ["audit", "publish"]) {
    const workspace = source(canonicalActivityWorkspaces[name]);
    assert.match(
      workspace,
      /class="workspace-toolbar"[\s\S]*class="ui-tabs view-tabs"/,
      `${name} nu folosește toolbar-ul canonic al activităților`,
    );
  }

  for (const name of ["design", "components", "content", "assets", "data", "blocks", "templates"]) {
    const workspace = source(canonicalActivityWorkspaces[name]);
    assert.match(
      workspace,
      /class="ui-field toolbar"/,
      `${name} nu folosește câmpul canonic de toolbar`,
    );
    assert.match(
      workspace,
      /class="ui-button primary toolbar toolbar-action"/,
      `${name} nu folosește acțiunea canonică de toolbar`,
    );
  }

  for (const name of Object.keys(canonicalActivityWorkspaces)) {
    const workspace = source(canonicalActivityWorkspaces[name]);
    assert.doesNotMatch(
      workspace,
      /^\s*\.(?:workspace-toolbar|view-tabs|search-field|toolbar-action)\s*\{/m,
      `${name} redeclară local o primitivă vizuală canonică`,
    );
    assert.doesNotMatch(
      workspace,
      /class="(?:primary|danger|secondary-action|primary-action)"/,
      `${name} folosește o acțiune semantică fără primitiva ui-button`,
    );
  }

  for (const name of ["design", "content", "assets"]) {
    const filteredWorkspace = source(canonicalActivityWorkspaces[name]);
    assert.match(
      filteredWorkspace,
      /class="toolbar-query-group(?: with-filter)?"[\s\S]*class="toolbar-filter"[\s\S]*class="search-field"/,
      `${name} nu grupează filtrul cu bara de căutare`,
    );
    assert.doesNotMatch(
      filteredWorkspace,
      /^\s*\.workspace-toolbar select\s*\{/m,
      `${name} stilizează local filtrul din toolbar`,
    );
  }

  const designWorkspace = source(canonicalActivityWorkspaces.design);
  assert.match(designWorkspace, /class:with-filter=\{activeView === "global-styles" \|\| activeView === "tokens"\}/);
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
  assert.match(workspace, /t\("design-edit"\)/);
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
  assert.match(workspace, /class="toolbar-filter"[\s\S]*bind:value=\{sectionFilter\}/);
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
