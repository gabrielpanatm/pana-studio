import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const read = (path) => readFile(new URL(`../${path}`, import.meta.url), "utf8");

test("bundled starters are normalized local Zola projects without theme management", async () => {
  const [startup, registry, rail, center, tauriConfig] = await Promise.all([
    read("src-tauri/src/project/startup.rs"),
    read("src-tauri/src/project/starters/registry.rs"),
    read("src/lib/components/workbench/ActivityRail.svelte"),
    read("src/lib/components/workspace/WorkspaceCenterArea.svelte"),
    read("src-tauri/tauri.conf.json"),
  ]);

  assert.match(startup, /ProjectStarterRegistry::load/);
  assert.match(startup, /materialize_starter/);
  assert.doesNotMatch(startup, /ThemeRegistry|set_active_theme_in_source|theme_files|recipe_files/);
  assert.match(registry, /resources\/project-starters/);
  assert.match(registry, /document\.get\("theme"\)\.is_some\(\)/);
  assert.match(registry, /path\.starts_with\("themes\/"\)/);
  assert.match(tauriConfig, /resources\/project-starters/);
  assert.doesNotMatch(tauriConfig, /resources\/theme-packs/);
  assert.doesNotMatch(rail, /id:\s*"themes"|UI_TERM_IDS\.themes/);
  assert.doesNotMatch(center, /ThemesWorkspace|components\/themes/);

  for (const id of ["minimal", "pana-studio", "nord", "cadru", "radacini"]) {
    const base = new URL(`../src-tauri/resources/project-starters/${id}/`, import.meta.url);
    assert.equal(existsSync(new URL("starter.toml", base)), true, id);
    assert.equal(existsSync(new URL("project/zola.toml", base)), true, id);
    assert.equal(existsSync(new URL("project/content/_index.md", base)), true, id);
    assert.equal(existsSync(new URL("project/templates/index.html", base)), true, id);
    const config = await readFile(new URL("project/zola.toml", base), "utf8");
    const manifest = await readFile(new URL("starter.toml", base), "utf8");
    assert.doesNotMatch(config, /^theme\s*=/m, id);
    assert.match(manifest, /^tested = "0\.23\.4"$/m, id);
    assert.equal(existsSync(new URL("project/themes", base)), false, id);
  }
});

test("bundled Zola sources are native Tera 2 and Markdown templating is intentional", async () => {
  const roots = [
    ...["minimal", "pana-studio", "nord", "cadru", "radacini"].map(
      (id) => new URL(`../src-tauri/resources/project-starters/${id}/project/`, import.meta.url),
    ),
    new URL("../tests/fixtures/projects/index-zero/sursa/", import.meta.url),
  ];

  for (const root of roots) {
    const relativePaths = await readdir(root, { recursive: true });
    assert.equal(relativePaths.some((path) => path.split("/").includes("shortcodes")), false);

    for (const relativePath of relativePaths.filter((path) => /\.(?:html|md)$/.test(path))) {
      const contents = await readFile(new URL(relativePath, root), "utf8");
      assert.doesNotMatch(contents, /\{%\s*(?:macro|import)\b/, relativePath);
      assert.doesNotMatch(contents, /\|\s*slice\s*\(/, relativePath);
      assert.doesNotMatch(
        contents,
        /get_taxonomy_url\s*\([^)]*\bname\s*=/,
        relativePath,
      );
      if (relativePath.endsWith(".md")) {
        assert.doesNotMatch(contents, /\{\{|\{%/, relativePath);
      }
    }
  }
});

test("project creation is a detached Rust-first startup flow outside the editor shell", async () => {
  const [startup, application, io, rust, commands, lifecycle] = await Promise.all([
    read("src/lib/components/startup/StartupView.svelte"),
    read("src/lib/components/application/ApplicationWorkspace.svelte"),
    read("src/lib/project/io/startup.ts"),
    read("src-tauri/src/project/startup.rs"),
    read("src-tauri/src/commands/startup.rs"),
    read("src-tauri/src/project/lifecycle.rs"),
  ]);

  assert.match(
    application,
    /\{#if \(projectSession\.lifecycle\.activeSession && projectSession\.project\) \|\| shell\.surface === "settings"\}[\s\S]*<AppChrome[\s\S]*\{:else\}[\s\S]*<StartupView/,
  );
  assert.match(startup, /startupCreationCatalog\?\.options/);
  assert.match(startup, /startupCreationPlan\.affectedFiles/);
  assert.match(io, /const defaultPath = await homeDir\(\)/);
  assert.match(io, /invoke<StartupCreationPlan>\("plan_startup_creation"/);
  assert.match(io, /invoke<StartupCreationReceipt>\("apply_startup_creation"/);
  assert.match(rust, /ProjectCreationAuthority/);
  assert.match(rust, /require_empty_root/);
  assert.match(rust, /rollback_publication/);
  assert.match(commands, /spawn_blocking/);
  assert.match(lifecycle, /stale_operation_cannot_consume_newer_inspection/);
  assert.match(lifecycle, /precommit_failure_preserves_the_previous_active_session/);
  assert.doesNotMatch(`${startup}\n${io}\n${rust}`, /zola_init|initZolaProject|ProjectBootstrapLease/);
});
