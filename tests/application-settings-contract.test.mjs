import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("Setările sunt o suprafață globală, nu o activitate a proiectului", () => {
  const types = source("../src/lib/types.ts");
  const rustWorkbench = source("../src-tauri/src/kernel/workbench/model.rs");
  const center = source("../src/lib/components/workspace/WorkspaceCenterArea.svelte");
  const rail = source("../src/lib/components/workbench/ActivityRail.svelte");

  assert.match(types, /ApplicationSurface = "workbench" \| "settings"/);
  assert.doesNotMatch(
    types.slice(types.indexOf("export type WorkbenchActivity"), types.indexOf("export type WorkbenchSurface")),
    /settings/,
  );
  assert.doesNotMatch(
    rustWorkbench.slice(rustWorkbench.indexOf("pub enum WorkbenchActivity"), rustWorkbench.indexOf("pub enum WorkbenchSurface")),
    /Settings/,
  );
  assert.ok(
    center.indexOf('app.applicationSurface === "settings"') < center.indexOf("!app.scannedProject"),
    "pagina Setări trebuie să fie disponibilă înaintea condiției de proiect deschis",
  );
  assert.match(rail, /settingsActive/);
  assert.match(rail, /aria-current=\{settingsActive \? "page"/);
  assert.doesNotMatch(rail, /settingsOpen|toggleSettings/);
});

test("vechiul panou suprapus este eliminat, iar pagina nu conține configurări ale site-ului", () => {
  const legacyPanel = new URL("../src/lib/components/SettingsPanel.svelte", import.meta.url);
  const workspace = source("../src/lib/components/settings/SettingsWorkspace.svelte");
  const chrome = source("../src/lib/components/workspace/AppChrome.svelte");

  assert.equal(existsSync(legacyPanel), false);
  assert.doesNotMatch(chrome, /SettingsPanel/);
  assert.match(workspace, /Setări Pană Studio/);
  assert.match(workspace, /Nicio opțiune de aici nu modifică site-ul deschis/);
  assert.doesNotMatch(workspace, /PublishWorkspace|openPublishCenter|Configurație Zola|Construire și publicare/);
});

test("preferințele aplicației au contract Rust cu revizie și CAS", () => {
  const model = source("../src-tauri/src/commands/config/model.rs");
  const implementation = source("../src-tauri/src/commands/config/app_config.rs");
  const registry = source("../src-tauri/src/tauri_command_registry.rs");
  const appState = source("../src/lib/state/app.svelte.ts");

  assert.match(model, /pub struct ApplicationSettingsSnapshot/);
  assert.match(model, /pub expected_revision: u64/);
  assert.match(model, /pub block_properties_height: u16/);
  assert.match(model, /pub block_properties_collapsed: bool/);
  assert.match(implementation, /input\.expected_revision != config\.revision/);
  assert.match(implementation, /WriteCategory::InternalAppWrite/);
  assert.match(registry, /read_application_settings/);
  assert.match(registry, /save_application_settings/);
  assert.match(appState, /applicationSettingsSaveTail/);
  assert.match(appState, /persistBlockPropertiesLayout/);
});
