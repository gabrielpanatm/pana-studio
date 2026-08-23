import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("Workbench deschide documentele materializate din File Explorer, nu din scanarea veche", () => {
  const explorer = source("../src/lib/workbench/file-explorer-state.svelte.ts");
  const navigation = source("../src/lib/workbench/document-navigation.ts");
  const selection = explorer.match(
    /private async commitSelection[\s\S]*?\n  async plan\(/,
  )?.[0] ?? "";
  const opening = navigation.match(
    /async show\([\s\S]*?\n  async activate\(/,
  )?.[0] ?? "";

  assert.match(selection, /receipt\.snapshot\.entries\.find/);
  assert.match(selection, /projectFileFromExplorerEntry\(entry\)/);
  assert.doesNotMatch(selection, /scannedProject\?*\.files\.find/);
  assert.match(opening, /resolveProjectFile\(document\.relativePath\)/);
  assert.doesNotMatch(opening, /scannedProject\?*\.files\.find/);
});

test("citirea și editarea folosesc namespace-ul materializat Rust", () => {
  const lifecycle = source("../src-tauri/src/commands/project/lifecycle.rs");
  const fileBuffers = source("../src-tauri/src/commands/project/file_buffers.rs");
  const workspace = source("../src-tauri/src/kernel/project_workspace/workspace.rs");

  assert.match(lifecycle, /projected_text_snapshot\(&relative_path\)/);
  assert.match(fileBuffers, /apply_projected_document_changeset/);
  assert.match(workspace, /The first real edit adopts a materialized document/);
  assert.match(workspace, /stage_resource_texts/);
});
