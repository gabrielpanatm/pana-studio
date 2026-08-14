import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("Workbench deschide documentele materializate din File Explorer, nu din scanarea veche", () => {
  const app = source("../src/lib/state/app.svelte.ts");
  const center = source("../src/lib/components/workspace/WorkspaceCenterArea.svelte");
  const selection = app.match(
    /private async commitFileExplorerSelection[\s\S]*?\n  async planFileExplorerOperation/,
  )?.[0] ?? "";
  const opening = center.match(
    /async function showWorkbenchDocument[\s\S]*?\n  async function activateWorkbenchDocument/,
  )?.[0] ?? "";

  assert.match(selection, /receipt\.snapshot\.entries\.find/);
  assert.match(selection, /projectFileFromExplorerEntry\(entry\)/);
  assert.doesNotMatch(selection, /scannedProject\?*\.files\.find/);
  assert.match(opening, /resolveWorkspaceProjectFile\(document\.relativePath\)/);
  assert.doesNotMatch(opening, /scannedProject\?*\.files\.find/);
});

test("citirea și editarea folosesc namespace-ul materializat Rust", () => {
  const projectCommands = source("../src-tauri/src/commands/project.rs");
  const workspace = source("../src-tauri/src/kernel/project_workspace/workspace.rs");

  assert.match(projectCommands, /projected_text_snapshot\(&relative_path\)/);
  assert.match(projectCommands, /apply_projected_document_changeset/);
  assert.match(workspace, /The first real edit adopts a materialized document/);
  assert.match(workspace, /stage_resource_texts/);
});
