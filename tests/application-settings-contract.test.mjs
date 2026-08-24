import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("Setările sunt o suprafață globală, nu o activitate a proiectului", () => {
  const types = source("../src/lib/application/contracts.ts")
    + source("../src/lib/workbench/contracts.ts");
  const rustWorkbench = source("../src-tauri/src/kernel/workbench/model.rs");
  const center = source("../src/lib/components/workspace/WorkspaceCenterArea.svelte");
  const rail = source("../src/lib/components/workbench/ActivityRail.svelte");
  const application = source("../src/lib/components/application/ApplicationWorkspace.svelte");
  const startup = source("../src/lib/components/startup/StartupView.svelte");

  assert.match(types, /ApplicationSurface = "workbench" \| "settings"/);
  assert.doesNotMatch(
    types.slice(types.indexOf("export type WorkbenchActivity"), types.indexOf("export type WorkbenchSurface")),
    /\| "settings"/,
  );
  assert.match(
    rustWorkbench.slice(rustWorkbench.indexOf("pub enum WorkbenchActivity"), rustWorkbench.indexOf("pub enum WorkbenchSurface")),
    /ProjectSettings/,
  );
  assert.match(center, /retainedAuxiliarySurface === "settings"[\s\S]*<SettingsWorkspace/);
  assert.match(
    application,
    /\{#if \(projectSession\.lifecycle\.activeSession && projectSession\.project\) \|\| shell\.surface === "settings"\}/,
  );
  assert.match(startup, /onclick=\{openApplicationSettings\}/);
  assert.doesNotMatch(startup, /AppState|app\./);
  assert.match(rail, /applicationSettingsActive/);
  assert.match(rail, /technicalActivities[\s\S]*project_settings/);
  assert.doesNotMatch(rail, /openSettings|toggleSettings/);
});

test("vechiul panou suprapus este eliminat, iar pagina nu conține configurări ale site-ului", () => {
  const legacyPanel = new URL("../src/lib/components/SettingsPanel.svelte", import.meta.url);
  const workspace = source("../src/lib/components/settings/SettingsWorkspace.svelte");
  const chrome = source("../src/lib/components/workspace/AppChrome.svelte");

  assert.equal(existsSync(legacyPanel), false);
  assert.doesNotMatch(chrome, /SettingsPanel/);
  assert.match(workspace, /t\("settings-title"\)/);
  assert.match(workspace, /t\("settings-description"\)/);
  assert.match(workspace, /class="ui-tabs settings-navigation"[\s\S]*role="tablist"/);
  assert.match(workspace, /class="ui-tab"[\s\S]*role="tab"[\s\S]*aria-selected/);
  assert.match(workspace, /id="settings-tab-panel"[\s\S]*role="tabpanel"/);
  assert.match(workspace, /handleSettingsTabKeydown/);
  assert.doesNotMatch(workspace, /\.settings-navigation button\.active::after/);
  assert.doesNotMatch(workspace, /PublishWorkspace|openPublishCenter|Configurație Zola|Construire și publicare/);
});

test("preferințele aplicației au contract Rust cu revizie și CAS", () => {
  const model = source("../src-tauri/src/commands/config/model.rs");
  const implementation = source("../src-tauri/src/commands/config/app_config.rs");
  const registry = source("../src-tauri/src/tauri_command_registry.rs");
  const preferences = source("../src/lib/application/preferences.svelte.ts");

  assert.match(model, /pub struct ApplicationSettingsSnapshot/);
  assert.match(model, /pub brand_accent: String/);
  assert.match(model, /enum ApplicationLanguagePreference[\s\S]*System[\s\S]*Fixed/);
  assert.match(model, /enum ApplicationThemePreference[\s\S]*System[\s\S]*Fixed/);
  assert.match(model, /enum ApplicationAccentPreference[\s\S]*System[\s\S]*Brand[\s\S]*Fixed/);
  assert.match(model, /pub expected_revision: u64/);
  assert.match(model, /pub patch: ApplicationSettingsPatch/);
  assert.match(model, /pub block_properties_height: u16/);
  assert.match(model, /pub block_properties_collapsed: bool/);
  assert.match(implementation, /input\.expected_revision != config\.revision/);
  assert.match(implementation, /LocalizedDiagnostic::new\("diagnostic-application-settings-stale"\)/);
  assert.match(implementation, /WriteCategory::InternalAppWrite/);
  assert.match(
    implementation,
    /brand_accent: DEFAULT_APPLICATION_ACCENT\.to_string\(\)/,
  );
  assert.match(registry, /read_application_settings/);
  assert.match(registry, /save_application_settings/);
  assert.match(preferences, /private saveTail: Promise<void>/);
  assert.match(preferences, /persistBlockPropertiesLayout/);
  assert.doesNotMatch(
    source("../src/lib/components/application/ApplicationWorkspace.svelte"),
    /ApplicationSettingsSnapshot/,
  );
});
