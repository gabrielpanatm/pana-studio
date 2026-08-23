import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const rustLifecycle = await readFile(
  new URL("../src-tauri/src/project/lifecycle.rs", import.meta.url),
  "utf8",
);
const rustProjectCommands = await readFile(
  new URL("../src-tauri/src/commands/project/lifecycle.rs", import.meta.url),
  "utf8",
);
const rustProjectBootstrap = await readFile(
  new URL("../src-tauri/src/commands/project/bootstrap.rs", import.meta.url),
  "utf8",
);
const rustProjectContracts = await readFile(
  new URL("../src-tauri/src/commands/project/contracts.rs", import.meta.url),
  "utf8",
);
const rustPreviewCommands = await readFile(
  new URL("../src-tauri/src/commands/preview.rs", import.meta.url),
  "utf8",
);
const rustEditorNavigation = await readFile(
  new URL("../src-tauri/src/commands/editor_navigation.rs", import.meta.url),
  "utf8",
);
const rustStartup = await readFile(
  new URL("../src-tauri/src/project/startup.rs", import.meta.url),
  "utf8",
);
const rustStartupCommands = await readFile(
  new URL("../src-tauri/src/commands/startup.rs", import.meta.url),
  "utf8",
);
const rustPreviewEngine = await readFile(
  new URL("../src-tauri/src/preview/engine.rs", import.meta.url),
  "utf8",
);
const rustSourceGraphCommands = await readFile(
  new URL("../src-tauri/src/commands/source_graph.rs", import.meta.url),
  "utf8",
);
const previewController = await readFile(
  new URL("../src/lib/state/preview-controller.ts", import.meta.url),
  "utf8",
);
const page = await readFile(
  new URL("../src/lib/components/application/ApplicationWorkspace.svelte", import.meta.url),
  "utf8",
);
const workspaceCss = await readFile(
  new URL("../src/routes/workspace-shell.css", import.meta.url),
  "utf8",
);

test("Rust owns the complete project transition and readiness state machines", () => {
  for (const state of [
    "Idle",
    "Inspecting",
    "AwaitingRecoveryDecision",
    "Preparing",
    "Committing",
  ]) {
    assert.match(rustLifecycle, new RegExp(`\\b${state}\\b`));
  }
  for (const readiness of [
    "InitializingFrontend",
    "PreparingPreview",
    "AwaitingCanvas",
    "FinalizingFrontend",
    "Ready",
    "Degraded",
  ]) {
    assert.match(rustLifecycle, new RegExp(`\\b${readiness}\\b`));
  }
  assert.match(rustLifecycle, /require_current_operation/);
  assert.match(rustLifecycle, /stale_operation_cannot_consume_newer_inspection/);
  assert.match(rustLifecycle, /precommit_failure_preserves_the_previous_active_session/);
});

test("open_project cannot bypass the inspected operation and returns one bootstrap receipt", () => {
  assert.match(rustProjectCommands, /pub fn open_project\(/);
  assert.match(rustProjectCommands, /operation_id: String/);
  assert.match(rustProjectCommands, /candidate_token: String/);
  assert.match(rustProjectCommands, /begin_preparing/);
  assert.match(rustProjectCommands, /begin_commit/);
  assert.match(rustProjectCommands, /commit_session/);
  assert.match(rustProjectContracts, /pub struct ProjectOpenBootstrapReceipt/);
  assert.doesNotMatch(rustProjectCommands, /require_valid_zola_candidate/);
  assert.equal(
    (rustProjectCommands.match(/read_project_disk_manifest\(&root\)/g) ?? []).length,
    1,
  );
  assert.match(rustProjectCommands, /scan_project_disk_manifest\(&root, &inspection\.manifest\)/);
  assert.doesNotMatch(rustProjectCommands, /scan_project_root\(&root\)/);
  assert.match(rustProjectCommands, /render_candidate_with_pending_project_authority/);
  assert.match(rustStartup, /inspection_manifest/);
  assert.match(rustStartup, /inspect_project_disk/);
  assert.doesNotMatch(rustStartupCommands, /read_project_disk_manifest/);
  assert.match(rustPreviewEngine, /remove_persistent_preview_session\(app, &zola_root, &session_root\)/);
  assert.doesNotMatch(rustPreviewEngine, /reset_persistent_preview_editor_cache/);
  assert.match(rustPreviewCommands, /generation_for_workspace_revision\(projection\.revision\)/);
});

test("Ready requires both canonical Canvas and the final frontend surface", () => {
  assert.match(
    rustPreviewCommands,
    /CanvasProjectionPhase::CanonicalVerified[\s\S]*ActiveProjectReadiness::FinalizingFrontend/,
  );
  assert.match(rustPreviewCommands, /ActiveProjectReadiness::AwaitingCanvas/);
  assert.match(rustPreviewCommands, /ActiveProjectReadiness::Degraded/);
  assert.match(rustEditorNavigation, /ActiveProjectReadiness::FinalizingFrontend/);
  assert.match(rustEditorNavigation, /initial_frontend_surface_ready/);
  assert.match(rustEditorNavigation, /ActiveProjectReadiness::Ready/);
  assert.match(rustEditorNavigation, /app\.emit\("project-lifecycle-changed", lifecycle\)/);
  assert.match(rustEditorNavigation, /route\.starts_with\("\/__pana_workbench\/"\)/);
  assert.match(page, /activeLifecycleReadiness\.state !== "ready"/);
  assert.match(page, /class="project-lifecycle-overlay"/);
  assert.match(page, /inert=.*lifecycleBlocksEditing/);
  const overlayStart = workspaceCss.indexOf(".project-lifecycle-overlay");
  const overlayEnd = workspaceCss.indexOf("}", overlayStart);
  assert.match(workspaceCss.slice(overlayStart, overlayEnd), /background:\s*var\(--app-bg\)/);
});

test("cold open stages the index Workbench surface before commit and mounts it first", () => {
  assert.match(rustProjectContracts, /struct ProjectBootstrapInitialSurface/);
  assert.match(rustProjectCommands, /initial_project_file\(&bootstrap\.project/);
  assert.match(rustProjectCommands, /stage_candidate/);
  assert.match(rustProjectCommands, /publish_template_workbench_view/);
  assert.match(rustProjectCommands, /initial_surface/);
  assert.match(rustProjectBootstrap, /project_index_file\(scan\)/);
  assert.match(rustProjectBootstrap, /WorkbenchIntent::OpenDocument/);
  assert.match(rustProjectBootstrap, /WorkbenchSurface::Visual/);
  assert.equal(
    (rustProjectCommands.match(/ProjectBootstrapAssembler::prepare\(/g) ?? []).length,
    2,
  );

  assert.match(previewController, /readiness === "ready" \|\| readiness === "degraded"/);
});

test("SourceGraph consumes the current ProjectModel and rebuilds it after invalidation without Canvas", () => {
  assert.match(rustSourceGraphCommands, /pub\(crate\) fn read_source_graph_from_accepted_project/);
  assert.match(rustSourceGraphCommands, /project_model_source_revision/);
  assert.match(rustSourceGraphCommands, /capture_project_model_build_context/);
  assert.match(rustSourceGraphCommands, /build_project_model_from_context/);
  assert.match(rustSourceGraphCommands, /publish_project_model_if_current/);
});
