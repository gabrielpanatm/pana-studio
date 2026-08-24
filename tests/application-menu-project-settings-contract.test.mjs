import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("meniul aplicației trăiește în toolbar, delegă Command Center și nu montează un meniu nativ secundar", () => {
  const app = source("../src-tauri/src/lib.rs");
  const topbar = source("../src/lib/components/Topbar.svelte");
  const menu = source("../src/lib/components/topbar/ApplicationMenuBar.svelte");

  assert.equal(existsSync(new URL("../src-tauri/src/native_menu.rs", import.meta.url)), false);
  assert.equal(existsSync(new URL("../src/lib/application/native-menu-service.ts", import.meta.url)), false);
  assert.doesNotMatch(app, /on_menu_event|native_menu::install/);
  assert.match(topbar, /ApplicationMenuBar/);
  assert.match(menu, /executeAction\(\{ kind: "app_command", command \}\)/);
  assert.match(menu, /document\.execCommand/);
  assert.equal(menu.match(/t\("application-menu-about"\)/g)?.length, 1);
});

test("titlul ferestrei este proiectat de Rust din rădăcina sesiunii", () => {
  const app = source("../src-tauri/src/lib.rs");
  const lifecycle = source("../src-tauri/src/commands/project/lifecycle.rs");

  assert.match(app, /fn synchronize_main_window_title/);
  assert.match(app, /window\.set_title\(title\)/);
  assert.match(lifecycle, /synchronize_main_window_title\(&app, None\)/);
  assert.match(lifecycle, /synchronize_main_window_title[\s\S]*opened_session_for_event\.project_root/);
});

test("setările proiectului sunt activitate tehnică separată și DeployPane legacy a dispărut", () => {
  const rustModel = source("../src-tauri/src/kernel/workbench/model.rs");
  const contracts = source("../src/lib/workbench/contracts.ts");
  const rail = source("../src/lib/components/workbench/ActivityRail.svelte");
  const center = source("../src/lib/components/workspace/WorkspaceCenterArea.svelte");
  const projectSettings = source("../src/lib/components/project-settings/ProjectSettingsWorkspace.svelte");
  const publish = source("../src/lib/components/publish/PublishWorkspace.svelte");
  const designSystem = source("../src/routes/design-system.css");

  assert.match(rustModel, /ProjectSettings/);
  assert.match(contracts, /"project_settings"/);
  assert.match(rail, /technicalActivities[\s\S]*project_settings/);
  assert.doesNotMatch(rail, /IconTerminal2|toggleTerminal/);
  assert.match(center, /project_settings[\s\S]*ProjectSettingsWorkspace/);
  assert.match(projectSettings, /activity-workspace-header-content/);
  assert.match(
    designSystem,
    /\.activity-workspace\.activity-workspace-header-content\s*\{[\s\S]*grid-template-rows:\s*auto minmax\(0, 1fr\)/,
  );
  assert.match(projectSettings, /saveProjectConfiguration/);
  assert.match(projectSettings, /registerEditFlushHandler/);
  assert.doesNotMatch(publish, /saveProjectConfiguration|ZolaProjectSettings|cachebustAssets=/);
  assert.equal(existsSync(new URL("../src/lib/components/DeployPane.svelte", import.meta.url)), false);
});
