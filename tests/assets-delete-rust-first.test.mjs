import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

const assets = source("../src/lib/components/creation/AssetsWorkspace.svelte");
const explorerCommand = source("../src-tauri/src/commands/file_explorer.rs");
const explorerKernel = source("../src-tauri/src/kernel/file_explorer.rs");
const workspace = source("../src-tauri/src/kernel/project_workspace/workspace.rs");

test("Media deletion uses the revision-bound Rust File Explorer plan", () => {
  assert.match(assets, /app\.planFileExplorerOperation\(\{ kind: "delete", entryId: entry\.id \}\)/);
  assert.match(assets, /app\.commitFileExplorerOperation\(plan\)/);
  assert.match(assets, /role="dialog" aria-modal="true"/);
  assert.match(assets, /assets-delete-references-preserved/);
  assert.doesNotMatch(assets, /window\.(?:confirm|prompt)/);
});

test("Media hides both text and binary deletion tombstones immediately", () => {
  assert.match(assets, /deletedDocuments/);
  assert.match(assets, /deletedBinaryResources/);
  assert.match(assets, /visibleGraphAssets = graphAssets\.filter/);
});

test("an unsaved binary import can be withdrawn through ProjectWorkspace history", () => {
  assert.match(explorerKernel, /staged_size\.or\(accepted_size\)/);
  assert.match(explorerCommand, /accepted_before\.or_else\(\|\| Some\(current\.clone\(\)\)\)/);
  assert.match(workspace, /staged_before_matches/);
  assert.match(workspace, /next_resources\.remove\(&normalized\)/);
});
