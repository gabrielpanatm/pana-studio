import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const rustLifecycle = await readFile(
  new URL("../src-tauri/src/project/lifecycle.rs", import.meta.url),
  "utf8",
);
const rustProjectCommands = await readFile(
  new URL("../src-tauri/src/commands/project.rs", import.meta.url),
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
const projectController = await readFile(
  new URL("../src/lib/state/project-controller.ts", import.meta.url),
  "utf8",
);
const previewController = await readFile(
  new URL("../src/lib/state/preview-controller.ts", import.meta.url),
  "utf8",
);
const page = await readFile(new URL("../src/routes/+page.svelte", import.meta.url), "utf8");
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
  const openStart = rustProjectCommands.indexOf("pub fn open_project(");
  const openEnd = rustProjectCommands.indexOf("pub fn read_project_file", openStart);
  const body = rustProjectCommands.slice(openStart, openEnd);
  assert.match(body, /operation_id: String/);
  assert.match(body, /candidate_token: String/);
  assert.match(body, /begin_preparing/);
  assert.match(body, /begin_commit/);
  assert.match(body, /commit_session/);
  assert.match(body, /ProjectOpenBootstrapReceipt/);
  assert.doesNotMatch(body, /require_valid_zola_candidate/);
  assert.equal((body.match(/read_project_disk_manifest\(&root\)/g) ?? []).length, 1);
  assert.match(body, /scan_project_disk_manifest\(&root, &inspection\.manifest\)/);
  assert.doesNotMatch(body, /scan_project_root\(&root\)/);
  assert.match(body, /render_candidate_with_pending_project_authority[\s\S]*begin_commit/);
  const inspectCandidateBody = rustStartup.slice(
    rustStartup.indexOf("fn inspect_candidate_root"),
    rustStartup.indexOf("struct CandidateInventory"),
  );
  assert.doesNotMatch(inspectCandidateBody, /run_zola_editor_check/);
  assert.match(rustStartup, /inspection_manifest/);
  assert.match(rustStartup, /inspect_project_disk/);
  assert.doesNotMatch(rustStartupCommands, /read_project_disk_manifest/);
  assert.match(rustPreviewEngine, /remove_persistent_preview_session\(app, &zola_root, &session_root\)/);
  assert.doesNotMatch(rustPreviewEngine, /reset_persistent_preview_editor_cache/);
  assert.match(rustPreviewCommands, /generation_for_workspace_revision\(projection\.revision\)/);
});

test("frontend hydration consumes the Rust bootstrap without redundant bootstrap reads", () => {
  const hydrateStart = projectController.indexOf("async function projectPublishedSessionIntoFrontend");
  const hydrateEnd = projectController.indexOf("export async function reattachCurrentProjectSession", hydrateStart);
  const body = projectController.slice(hydrateStart, hydrateEnd);
  assert.match(body, /options\.bootstrap\.fileBuffers/);
  assert.match(body, /options\.bootstrap\.workspace/);
  assert.match(body, /options\.bootstrap\.workbench/);
  assert.match(body, /options\.bootstrap\.activeDocument/);
  assert.doesNotMatch(body, /readFileBufferStore\(/);
  assert.doesNotMatch(body, /readProjectAppConfig\(/);
  assert.doesNotMatch(body, /resolveZolaIndexTemplateFile\(/);
  assert.doesNotMatch(body, /preferredFile|openPlan\.fileToOpen/);
  assert.match(projectController, /"source_graph"[\s\S]*errorMessage\(error\)/);
  assert.match(projectController, /"frontend"[\s\S]*diagnostic/);
  const previewStart = projectController.indexOf("export async function startPreviewAfterOpen");
  const previewEnd = projectController.indexOf("export function resetProjectScopedState", previewStart);
  const previewBody = projectController.slice(previewStart, previewEnd);
  assert.doesNotMatch(previewBody, /loadScannedProjectFile\(/);
  assert.match(previewBody, /updateTemplateWorkbenchContext\(/);
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
  assert.match(rustEditorNavigation, /route\.starts_with\("\/__pana_workbench\/"\)/);
  assert.match(page, /activeLifecycleReadiness\.state !== "ready"/);
  assert.match(page, /class="project-lifecycle-overlay"/);
  assert.match(page, /inert=.*lifecycleBlocksEditing/);
  const overlayStart = workspaceCss.indexOf(".project-lifecycle-overlay");
  const overlayEnd = workspaceCss.indexOf("}", overlayStart);
  assert.match(workspaceCss.slice(overlayStart, overlayEnd), /background:\s*var\(--app-bg\)/);
});

test("cold open stages the index Workbench surface before commit and mounts it first", () => {
  const openStart = rustProjectCommands.indexOf("pub fn open_project(");
  const openEnd = rustProjectCommands.indexOf("pub fn read_project_file", openStart);
  const openBody = rustProjectCommands.slice(openStart, openEnd);
  assert.match(rustProjectCommands, /struct ProjectBootstrapInitialSurface/);
  assert.match(openBody, /initial_project_file\(&authoritative_scan/);
  assert.match(openBody, /publish_template_workbench_view/);
  assert.ok(openBody.indexOf("stage_candidate") < openBody.indexOf("publish_template_workbench_view"));
  assert.ok(openBody.indexOf("publish_template_workbench_view") < openBody.indexOf("begin_commit"));
  assert.match(openBody, /initial_surface/);

  const workbenchStart = rustProjectCommands.indexOf("fn prepare_bootstrap_workbench");
  const workbenchEnd = rustProjectCommands.indexOf("pub fn current_project_root", workbenchStart);
  const workbenchBody = rustProjectCommands.slice(workbenchStart, workbenchEnd);
  assert.match(workbenchBody, /project_index_file\(scan\)/);
  assert.match(workbenchBody, /WorkbenchIntent::OpenDocument/);
  assert.match(workbenchBody, /WorkbenchSurface::Visual/);

  const previewStart = projectController.indexOf("export async function startPreviewAfterOpen");
  const previewEnd = projectController.indexOf("export function resetProjectScopedState", previewStart);
  const previewBody = projectController.slice(previewStart, previewEnd);
  assert.match(previewBody, /mountBootstrapInitialSurface/);
  assert.match(previewBody, /identity\.initialSurface/);
  assert.match(previewBody, /bootstrapSurfaceMounted/);
  assert.match(previewBody, /synchronizeActiveCanvasSurfaceRoute/);
  assert.match(previewController, /readiness === "ready" \|\| readiness === "degraded"/);
});

test("initial SourceGraph consumes the ProjectModel already published by Preview", () => {
  const start = rustSourceGraphCommands.indexOf("pub(crate) fn read_source_graph_from_accepted_project");
  const end = rustSourceGraphCommands.indexOf("#[tauri::command", start);
  const body = rustSourceGraphCommands.slice(start, end);
  assert.match(body, /project_model_source_revision/);
  assert.match(body, /ProjectModel-ul publicat de Preview/);
  assert.doesNotMatch(body, /build_project_model_from_workspace_projection/);
});
