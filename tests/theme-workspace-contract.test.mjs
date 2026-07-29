import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const read = (path) => readFile(new URL(`../${path}`, import.meta.url), "utf8");

test("themes workspace projects Rust catalog and explicit read-plan-apply commands", async () => {
  const [workspace, io, types] = await Promise.all([
    read("src/lib/components/themes/ThemesWorkspace.svelte"),
    read("src/lib/project/io.ts"),
    read("src/lib/types.ts"),
  ]);

  assert.match(io, /invoke<ThemeCatalogSnapshot>\("read_theme_catalog"/);
  assert.match(io, /invoke<ThemePlan>\("plan_theme_change"/);
  assert.match(io, /invoke<ThemeApplyReceipt>\("apply_theme_change"/);
  assert.match(types, /type ThemeStatus = "available" \| "installed" \| "active"/);
  assert.match(workspace, /pendingPlan\.localOverrides/);
  assert.match(workspace, /pendingPlan\.blocking/);
  assert.match(workspace, /pendingPlan\.planToken/);
  assert.doesNotMatch(workspace, /@tauri-apps\/plugin-fs|readTextFile|readDir/);
});

test("project creation is a detached Rust-first startup flow outside the editor shell", async () => {
  const [startup, route, controller, io, rust, commands, projectCommands] = await Promise.all([
    read("src/lib/components/startup/StartupView.svelte"),
    read("src/routes/+page.svelte"),
    read("src/lib/state/project-controller.ts"),
    read("src/lib/project/io.ts"),
    read("src-tauri/src/project/startup.rs"),
    read("src-tauri/src/commands/startup.rs"),
    read("src-tauri/src/commands/project.rs"),
  ]);

  assert.match(route, /\{#if app\.scannedProject \|\| app\.applicationSurface === "settings"\}[\s\S]*<AppChrome[\s\S]*\{:else\}[\s\S]*<StartupView/);
  assert.match(startup, /startupCreationCatalog\?\.options/);
  assert.match(startup, /startupCreationPlan\.affectedFiles/);
  assert.match(controller, /inspectStartupFolder\(selected\)/);
  assert.match(controller, /candidate\.kind === "valid_project"/);
  assert.match(controller, /candidate\.kind === "empty_directory"/);
  assert.match(io, /const defaultPath = await homeDir\(\)/);
  assert.match(io, /invoke<StartupCreationPlan>\("plan_startup_creation"/);
  assert.match(io, /invoke<StartupCreationReceipt>\("apply_startup_creation"/);
  assert.match(rust, /ProjectCreationAuthority/);
  assert.match(rust, /require_empty_root/);
  assert.match(rust, /rollback_publication/);
  assert.match(commands, /spawn_blocking/);
  const openProjectBody = projectCommands.slice(
    projectCommands.indexOf("pub fn open_project"),
    projectCommands.indexOf("pub fn read_project_file"),
  );
  assert.ok(
    openProjectBody.indexOf("require_valid_zola_candidate(&root)?")
      < openProjectBody.indexOf("prepare_project_session(&app, &root, &scan)?"),
    "ProjectSession nu poate fi pregătită înaintea clasificării Rust valide",
  );
  assert.doesNotMatch(`${startup}\n${controller}\n${io}\n${rust}`, /zola_init|initZolaProject|ProjectBootstrapLease/);
});
