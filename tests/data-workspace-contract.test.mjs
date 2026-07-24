import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("Date este o activitate distinctă cu modelul catalog și panou contextual", () => {
  const rail = source("../src/lib/components/workbench/ActivityRail.svelte");
  const center = source("../src/lib/components/workspace/WorkspaceCenterArea.svelte");
  const workspace = source("../src/lib/components/data/DataWorkspace.svelte");
  const types = source("../src/lib/types.ts");
  const workbench = source("../src-tauri/src/kernel/workbench/model.rs");
  const commandCenter = source("../src-tauri/src/kernel/command_center/search.rs");

  assert.match(rail, /id:\s*"data"/);
  assert.match(center, /activeWorkbenchActivity === "data"[\s\S]*<DataWorkspace/);
  assert.match(types, /export type WorkbenchActivity[\s\S]*\|\s*"data"/);
  assert.match(workbench, /\bData,\s*\n\s*Versioning/);
  assert.match(commandCenter, /WorkbenchActivity::Data[\s\S]*"Fișiere TOML reutilizabile/);
  assert.match(workspace, /type DetailMode = "info" \| "create" \| "edit"/);
  assert.match(workspace, /class="workspace-header"/);
  assert.match(workspace, /class="workspace-toolbar"/);
  assert.match(workspace, /role="tablist"/);
  assert.match(workspace, /type="search"/);
  assert.match(workspace, /detailMode === "create"/);
  assert.match(workspace, /detailMode === "edit"/);
  assert.doesNotMatch(workspace, /window\.(?:prompt|confirm)/);
});

test("editorul vizual TOML păstrează Rust și ProjectWorkspace ca autoritate unică", () => {
  const workspace = source("../src/lib/components/data/DataWorkspace.svelte");
  const kernel = source("../src-tauri/src/kernel/data_mutation.rs");
  const commands = source("../src-tauri/src/commands/data.rs");
  const registry = source("../src-tauri/src/tauri_command_registry.rs");
  const io = source("../src/lib/project/io.ts");

  assert.match(workspace, /applyDataMutation/);
  assert.match(workspace, /readDataNodeEditor/);
  assert.doesNotMatch(workspace, /DocumentMut|toml_edit|setFileBufferDraft/);
  assert.match(kernel, /stage_validated_data_mutation/);
  assert.match(kernel, /validate_semantic_workspace_candidate/);
  assert.match(kernel, /stage_resource_texts/);
  assert.match(kernel, /apply_exact_replacements/);
  assert.match(kernel, /DataMutationOperation::UpdateNode/);
  assert.match(kernel, /DataMutationOperation::InsertChild/);
  assert.match(kernel, /DataMutationOperation::DeleteNode/);
  assert.match(commands, /commit_project_workspace_session_mutation/);
  assert.match(registry, /read_data_node_editor/);
  assert.match(registry, /apply_data_mutation/);
  assert.match(io, /"read_data_node_editor"/);
  assert.match(io, /"apply_data_mutation"/);
  assert.doesNotMatch(commands, /std::fs::(?:write|remove_file|rename)/);
});

test("proiecția TOML păstrează span-urile înainte de conversia mutabilă", () => {
  const structured = source("../src-tauri/src/source_graph/structured_data.rs");

  assert.match(structured, /Document::parse\(source\.to_string\(\)\)/);
  assert.match(structured, /project_table\(parsed\.as_table\(\)/);
  assert.match(structured, /_document:\s*parsed\.into_mut\(\)/);
  assert.doesNotMatch(
    structured.slice(
      structured.indexOf("pub(crate) fn parse_lossless_toml"),
      structured.indexOf("pub(crate) fn parse_zola_data_adapter"),
    ),
    /source\s*\.parse::<DocumentMut>/,
  );
});
