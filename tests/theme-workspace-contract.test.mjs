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

test("project creation requires a selected bundled theme and has no frontend default id", async () => {
  const [startup, controller, io] = await Promise.all([
    read("src/lib/components/workspace/StartupState.svelte"),
    read("src/lib/state/project-controller.ts"),
    read("src/lib/project/io.ts"),
  ]);

  assert.match(startup, /readThemeCatalog\(catalogIdentity\(\)\)/);
  assert.match(startup, /initZolaProject\(selectedThemeId\)/);
  assert.match(startup, /disabled=\{!selectedThemeId/);
  assert.match(controller, /zolaInit\(host\.scannedProject\.root, themeId\)/);
  assert.match(io, /invoke<string>\("zola_init", \{ path, themeId \}\)/);
  assert.doesNotMatch(`${startup}\n${controller}\n${io}`, /pana-studio.*(?:themeId|selectedThemeId)/);
});
