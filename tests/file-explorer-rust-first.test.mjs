import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("File Explorer has one Rust tree, capability and operation authority", () => {
  const kernel = source("../src-tauri/src/kernel/file_explorer.rs");
  const commands = source("../src-tauri/src/commands/file_explorer.rs");
  const frontend = source("../src/lib/components/project/ProjectFilesTab.svelte");

  for (const contract of [
    "FileExplorerSnapshot",
    "FileExplorerEntry",
    "FileExplorerCapabilities",
    "FileExplorerOperationPlan",
    "FileExplorerOperationRequest",
  ]) {
    assert.match(kernel, new RegExp(`(?:struct|enum) ${contract}\\b`));
  }
  assert.match(kernel, /hierarchy_order\(entries\)/);
  assert.match(kernel, /PROJECT_SCAN_MAX_ENTRIES/);
  assert.match(kernel, /EditAuthorityUnavailable/);
  assert.match(commands, /plan_file_explorer_operation/);
  assert.match(commands, /commit_file_explorer_operation/);
  assert.match(commands, /consume_plan/);
  assert.match(commands, /stage_project_bundle_changes/);
  assert.match(commands, /require_user_source_mutation/);
  assert.match(frontend, /snapshot:\s*FileExplorerSnapshot \| null/);
  assert.match(frontend, /planOperation\(/);
  assert.match(frontend, /commitOperation\(plan\)/);
  assert.doesNotMatch(frontend, /ProjectFile\[\]|allProjectFiles|semanticMoveProjectEntry|semanticRenameProjectEntry/);
});

test("legacy TypeScript tree and mutation authorities are removed", () => {
  for (const relativePath of [
    "../src/lib/state/files-controller.ts",
    "../src/lib/state/files-drag-controller.ts",
    "../src/lib/project/files-drag.ts",
    "../src/lib/project/files-rename.ts",
    "../src/lib/project/pane-tree.ts",
  ]) {
    assert.equal(existsSync(new URL(relativePath, import.meta.url)), false, relativePath);
  }
});

test("Explorer selection, reveal, drag plan and accessibility stay frontend projections", () => {
  const frontend = source("../src/lib/components/project/ProjectFilesTab.svelte");
  const app = source("../src/lib/state/app.svelte.ts");

  assert.match(frontend, /role="tree"/);
  assert.match(frontend, /role="treeitem"/);
  assert.match(frontend, /aria-level=\{node\.depth \+ 1\}/);
  assert.match(frontend, /aria-expanded=/);
  assert.match(frontend, /projectFileExplorerRows/);
  assert.match(frontend, /node\.hasChildren/);
  assert.match(frontend, /node\.expanded/);
  assert.match(frontend, /aria-selected=/);
  assert.match(frontend, /event\.key === "ArrowDown"/);
  assert.match(frontend, /event\.key === "ArrowUp"/);
  assert.match(frontend, /event\.key === "ArrowRight"/);
  assert.match(frontend, /event\.key === "ArrowLeft"/);
  assert.match(frontend, /scrollIntoView\(\{ block: "nearest" \}\)/);
  assert.match(
    frontend,
    /snapshot\?\.activeDocumentPath[\s\S]*snapshot\?\.selectedEntry\?\.relativePath/,
  );
  assert.match(frontend, /data-active-document=/);
  assert.match(frontend, /aria-current=/);
  assert.match(frontend, /distance < 6/);
  assert.match(frontend, /dragPlanSerial/);
  assert.match(frontend, /resolvedPlan\?\.allowed \|\| !resolvedPlan\.commitToken/);
  assert.match(app, /selectFileExplorerEntryInRust/);
  assert.match(app, /this\.workbenchSnapshot = receipt\.workbench\.snapshot/);
  assert.match(app, /this\.fileExplorerSnapshot = receipt\.snapshot/);
});

test("hover and active rows share the requested outline-only visual contract", () => {
  const frontend = source("../src/lib/components/project/ProjectFilesTab.svelte");
  const designSystem = source("../src/routes/design-system.css");
  assert.match(
    designSystem,
    /\.ui-entity-selectable:hover:not\(:disabled\)[\s\S]*outline-style:\s*dashed;/,
  );
  assert.match(
    designSystem,
    /\.ui-entity-selectable\[data-ui-selected="true"\]\s*\{[\s\S]*outline-style:\s*solid;/,
  );
  assert.match(frontend, /class="file-row ui-entity-selectable"/);
  assert.match(frontend, /data-ui-selected=/);
  assert.match(frontend, /hoveredPath === node\.path \|\| node\.entry\?\.id === snapshot\?\.selectedEntry\?\.entryId/);
  assert.match(frontend, /<IconTrash/);
});

test("empty-directory markers stay internal and external reconcile cannot leave stale selection", () => {
  const scan = source("../src-tauri/src/project/scan.rs");
  const commands = source("../src-tauri/src/commands/file_explorer.rs");
  const app = source("../src/lib/state/app.svelte.ts");
  assert.match(scan, /Some\("\.gitkeep"\)[\s\S]*continue/);
  assert.match(commands, /missing_workbench_paths/);
  assert.match(commands, /WorkbenchIntent::ReconcileProjectEntries/);
  assert.match(app, /snapshot\.workbenchRevision/);
  assert.match(app, /await this\.refreshWorkbenchState\(\)/);
});
