import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { extname, resolve } from "node:path";
import { test } from "node:test";

const repoRoot = resolve(new URL("..", import.meta.url).pathname);

function source(path) {
  return readFileSync(resolve(repoRoot, path), "utf8");
}

function filesUnder(relativeRoot, extensions = new Set([".ts", ".svelte", ".rs"])) {
  const root = resolve(repoRoot, relativeRoot);
  const result = [];
  const visit = (directory) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      if (["node_modules", "target", ".svelte-kit"].includes(entry.name)) continue;
      const path = resolve(directory, entry.name);
      if (entry.isDirectory()) visit(path);
      else if (extensions.has(extname(entry.name))) result.push(path);
    }
  };
  visit(root);
  return result;
}

test("Save is the single ProjectWorkspace disk boundary", () => {
  const controller = source("src/lib/state/save-controller.ts");
  const coordinator = source("src/lib/session/workspace-mutation-coordinator.ts");
  const projectIo = source("src/lib/project/io.ts");
  const saveCommand = source("src-tauri/src/commands/project.rs");
  const diskEngine = source("src-tauri/src/kernel/project_workspace/disk_boundary/engine.rs");

  assert.match(controller, /await saveProjectWorkspace\(/);
  assert.match(controller, /await flushWorkspaceMutationInputs\("save"/);
  assert.match(coordinator, /await flushRegisteredEditDrafts\(reason\)/);
  assert.match(coordinator, /await flushPageJsDraftSync/);
  assert.match(coordinator, /await flushFileBufferDraftSync/);
  assert.match(controller, /await readProjectWorkspaceState\(\)/);
  const acceptedBaselineIndex = controller.indexOf("host.acceptProjectWorkspaceSaveBaseline(");
  assert.ok(
    acceptedBaselineIndex >= 0
      && controller.indexOf("await settleFrontendProjection(", acceptedBaselineIndex)
        > acceptedBaselineIndex,
    "Save must publish the Rust-accepted monitor baseline before fallible UI projection",
  );
  assert.match(projectIo, /"save_project_workspace"/);
  assert.match(saveCommand, /pub fn save_project_workspace\(/);
  assert.match(saveCommand, /project_workspace::save_project_workspace/);
  assert.match(diskEngine, /WriteOwner::ProjectWorkspace/);
  assert.match(diskEngine, /WriteAuthority::new/);
});

test("all editor mutation command families stage through ProjectWorkspace", () => {
  const commandFiles = [
    "src-tauri/src/commands/css.rs",
    "src-tauri/src/commands/js.rs",
    "src-tauri/src/commands/page_contracts.rs",
    "src-tauri/src/commands/config/workspace.rs",
    "src-tauri/src/commands/workspace_entries.rs",
    "src-tauri/src/commands/kernel_preview_pipeline.rs",
  ];
  for (const path of commandFiles) {
    const rust = source(path);
    assert.match(rust, /ProjectWorkspace|project_workspace/, path);
    assert.match(rust, /commit_project_workspace_session_mutation/, path);
    assert.doesNotMatch(rust, /persist_project_workspace_recovery\(/, path);
    assert.doesNotMatch(rust, /WriteAuthority::new\([^)]*\)\s*\.write_text/, path);
  }
});

test("session commands publish only after recovery persistence succeeds", () => {
  const commandSources = filesUnder("src-tauri/src/commands", new Set([".rs"]));
  const directRecoveryWriters = commandSources
    .filter((path) => /persist_project_workspace_recovery\(/.test(readFileSync(path, "utf8")))
    .map((path) => path.slice(repoRoot.length + 1));

  assert.deepEqual(directRecoveryWriters, ["src-tauri/src/commands/project.rs"]);

  const projectCommands = source("src-tauri/src/commands/project.rs");
  assert.equal(projectCommands.match(/persist_project_workspace_recovery\(/g)?.length, 1);
  assert.match(
    projectCommands,
    /save_project_workspace[\s\S]*persist_project_workspace_recovery\(&app, workspace\)/,
  );
});

test("HTML structural mutations publish one workspace revision and never write project files directly", () => {
  const pipeline = source("src-tauri/src/commands/kernel_preview_pipeline.rs");
  const context = source("src-tauri/src/commands/kernel_preview_context.rs");
  assert.match(pipeline, /candidate\.publish_project_model/);
  assert.match(pipeline, /commit_project_workspace_session_mutation_with_projection\(/);
  assert.match(context, /workspace_revision/);
  assert.match(context, /require_accepted_disk_unchanged/);
  assert.doesNotMatch(pipeline, /acceptedDiskGeneration|InternalWriteEvidence/);
});

test("Preview stages exact ProjectWorkspace revisions and promotes only an exact styledReady ACK", () => {
  const materializer = source("src-tauri/src/preview/preprocess/workspace.rs");
  const engine = source("src-tauri/src/preview/engine.rs");
  const server = source("src-tauri/src/preview/server.rs");
  const commands = source("src-tauri/src/commands/preview.rs");
  const coordinator = source("src/lib/kernel/project-workspace-preview-coordinator.ts");

  assert.match(materializer, /sync_persistent_project_workspace/);
  assert.match(materializer, /lease\.source_texts/);
  assert.match(materializer, /lease\.deleted_sources/);
  assert.match(materializer, /require_accepted_disk_baseline/);
  assert.match(engine, /struct PersistentZolaPreviewEngine/);
  assert.match(engine, /site: Option<Site>/);
  assert.match(engine, /render_candidate/);
  assert.match(engine, /stage_candidate/);
  assert.match(engine, /acknowledge_candidate_phase/);
  assert.match(engine, /BuildMode::Memory/);
  assert.match(engine, /site\.reload_templates\(\)/);
  assert.match(engine, /materialize_official_zola_assets/);
  assert.match(server, /RwLock<PreviewGenerationRegistry>/);
  assert.match(server, /staged: BTreeMap<String, Arc<ActivePreviewGeneration>>/);
  assert.match(server, /retired: VecDeque<Arc<ActivePreviewGeneration>>/);
  assert.match(server, /active\.write\(\)/);
  assert.match(server, /fn acknowledge_phase\(/);
  assert.match(server, /requested_preview_revision/);
  assert.match(commands, /workspace\.require_current_projection\(lease\)/);
  assert.match(commands, /engine\.stage_candidate/);
  assert.match(commands, /pub fn acknowledge_canvas_projection_phase/);
  assert.match(commands, /engine\.acknowledge_candidate_phase/);
  assert.doesNotMatch(commands, /engine\.publish_candidate/);
  assert.doesNotMatch(materializer, /materialize_project_workspace_generation/);
  assert.doesNotMatch(engine.split("#[cfg(test)]")[0], /Command::new|zola serve/);
  assert.match(engine, /publish_template_workbench_view/);
  assert.match(engine, /render_template_workbench_document/);
  assert.match(server, /workbench_content/);
  assert.match(commands, /project_template_workbench_preview/);
  assert.match(commands, /resolve_template_workbench_plan/);
  assert.match(coordinator, /projectLatestProjectWorkspacePreview/);
  assert.match(coordinator, /projectLatestActiveTemplateWorkbench/);
  assert.match(coordinator, /projectProjectWorkspacePreview\(input\)/);
  assert.match(coordinator, /projectedEvidence\?\.workspaceRevision === snapshot\.revision/);
  assert.match(coordinator, /cachedPlan\?\.workspaceTransactionId !== options\.expectedWorkspaceTransactionId/);

  const consumers = filesUnder("src/lib", new Set([".ts", ".svelte"]))
    .filter((path) => readFileSync(path, "utf8").includes("projectProjectWorkspacePreview"));
  assert.deepEqual(
    consumers.map((path) => path.slice(repoRoot.length + 1)),
    ["src/lib/kernel/project-workspace-preview-coordinator.ts", "src/lib/project/io.ts"],
  );
});

test("Source Browser renders AcceptedDisk with embedded Zola on one stable Rust server", () => {
  const manager = source("src-tauri/src/preview/source_browser/mod.rs");
  const server = source("src-tauri/src/preview/source_browser/server.rs");
  const process = source("src-tauri/src/preview/process.rs");
  const state = source("src-tauri/src/state.rs");
  const previewCommands = source("src-tauri/src/commands/preview.rs");
  const projectCommands = source("src-tauri/src/commands/project.rs");
  const externalDisk = source("src-tauri/src/commands/external_disk.rs");
  const materializer = source("src-tauri/src/preview/preprocess/workspace.rs");

  assert.match(manager, /render_official_zola_disk_generation/);
  assert.match(manager, /require_browser_preview_session/);
  assert.match(manager, /active_matches_generation/);
  assert.match(manager, /schedule_source_browser_refresh/);
  assert.match(manager, /prepare_source_browser_session/);
  assert.match(materializer, /pub\(crate\) fn prepare_source_browser_session/);
  assert.match(
    materializer,
    /create_directory\(app, &container, &container, "preview\/root"\)[\s\S]*create_directory\([\s\S]*&browser_root,[\s\S]*"preview\/source-browser"[\s\S]*create_directory\([\s\S]*session_root,[\s\S]*"preview\/source-browser-session"/,
  );
  assert.match(server, /struct SourceBrowserServer/);
  assert.match(server, /TcpListener::bind\("127\.0\.0\.1:0"\)/);
  assert.match(server, /SourceBrowserPublicationStatus::Building/);
  assert.match(server, /SourceBrowserPublicationStatus::Ready/);
  assert.match(server, /SourceBrowserPublicationStatus::Failed/);
  assert.match(server, /text\/event-stream/);
  assert.match(server, /new EventSource/);
  assert.match(server, /X-Pana-Disk-Generation/);
  assert.match(previewCommands, /start_or_refresh_source_browser/);
  assert.match(projectCommands, /schedule_source_browser_refresh/);
  assert.match(externalDisk, /schedule_source_browser_refresh/);
  assert.match(state, /source_browser_engine: Mutex<Option<SourceBrowserEngine>>/);

  for (const legacy of [
    "BrowserProcessCandidate",
    "browser_zola_process",
    "browser_zola_disk_generation",
    "start_browser_zola_serve",
    "stop_browser_zola_process",
  ]) {
    assert.doesNotMatch(process, new RegExp(legacy));
    assert.doesNotMatch(state, new RegExp(legacy));
  }
  assert.doesNotMatch(process, /zola serve|Command::new\([^)]*zola/);
});

test("Files panel is projected from one exact ProjectWorkspace revision", () => {
  const scan = source("src-tauri/src/project/scan.rs");
  const projectCommands = source("src-tauri/src/commands/project.rs");

  assert.match(scan, /pub fn scan_project_workspace_projection/);
  assert.match(scan, /projection\s*\.accepted_disk\s*\.manifest\s*\.files/);
  assert.match(scan, /projection\.source_texts/);
  assert.match(scan, /projection\.deleted_sources/);
  assert.match(projectCommands, /scan_project_workspace_projection\(&projection\)/);
  assert.match(projectCommands, /require_current_projection\(&projection\)/);
  assert.doesNotMatch(scan, /overlay_workspace_text_paths/);
  assert.doesNotMatch(projectCommands, /overlay_workspace_text_paths/);
});

test("Rust mutation events drive a separate authority epoch, not UI concurrency", () => {
  const effects = source("src/lib/state/app-effects.svelte.ts");
  const app = source("src/lib/state/app.svelte.ts");
  const recovery = source("src-tauri/src/kernel/project_workspace/recovery.rs");

  assert.match(recovery, /pana-project-workspace-mutated/);
  assert.match(effects, /app\.markProjectWorkspaceMutation\(\)/);
  assert.doesNotMatch(effects, /notice[\s\S]{0,300}markEditorMutation\(\)/);
  assert.match(effects, /app\.projectWorkspaceMutationEpoch;/);
  assert.match(app, /projectWorkspaceMutationEpoch = \$state\(0\)/);
});

test("Undo and Redo operate only on ProjectWorkspace history", () => {
  const projectCommands = source("src-tauri/src/commands/project.rs");
  const frontend = source("src/routes/+page.svelte");
  const codeEditor = source("src/lib/editor/controller.ts");
  const shortcuts = source("src/lib/ui/app-shortcuts.ts");
  assert.match(projectCommands, /candidate\.undo\(/);
  assert.match(projectCommands, /candidate\.redo\(/);
  assert.match(projectCommands, /require_history_target/);
  assert.match(projectCommands, /commit_project_workspace_session_mutation/);
  assert.match(frontend, /undoProjectWorkspace/);
  assert.match(frontend, /redoProjectWorkspace/);
  assert.match(frontend, /expectedTransactionId: target\.transactionId/);
  assert.match(frontend, /rebaseFileBufferDraftSyncProjection/);
  assert.doesNotMatch(frontend, /readFileBufferText/);
  assert.doesNotMatch(codeEditor, /historyKeymap|\bhistory\(\)/);
  assert.match(codeEditor, /Transaction\.addToHistory\.of\(false\)/);
  assert.match(shortcuts, /isManagedWorkspaceEditorTarget/);
  assert.match(frontend, /const nextKey = \[[\s\S]*app\.projectWorkspaceMutationEpoch,[\s\S]*\]\.join\(":"\)/);
  assert.doesNotMatch(frontend, /const nextKey = \[[\s\S]{0,180}app\.editorMutationEpoch/);
  assert.doesNotMatch(frontend, /InternalWriteEvidence|acceptedDiskGeneration/);
});

test("CSS and Page JS receipts expose workspace revisions, not disk acknowledgements", () => {
  const types = source("src/lib/types.ts");
  const projectIo = source("src/lib/project/io.ts");
  const pageJsSync = source("src/lib/session/page-js-draft-sync.ts");

  assert.match(types, /export type CssMutationAuthorityReceipt[\s\S]*revisionBefore[\s\S]*workspaceMutation/);
  assert.match(projectIo, /authority\.workspaceMutation\.revisionAfter/);
  assert.match(pageJsSync, /stagePageJsDraft/);
  assert.doesNotMatch(pageJsSync, /save_page_js|acceptedManifest|InternalWriteEvidence/);
});

test("Markdown flush remains on the canonical FileBuffer and ProjectWorkspace authority path", () => {
  const tipTapEditor = source("src/lib/components/markdown/TipTapMarkdownEditor.svelte");
  const markdownEditor = source("src/lib/components/markdown/MarkdownEditor.svelte");
  const fileBufferSync = source("src/lib/session/file-buffer-draft-sync.ts");

  assert.match(tipTapEditor, /registerEditFlushHandler/);
  assert.match(tipTapEditor, /editor\?\.getMarkdown\(\)/);
  assert.match(tipTapEditor, /localEditVersion === flushedLocalEditVersion/);
  assert.match(tipTapEditor, /undoRedo: false/);
  assert.match(markdownEditor, /queueFileBufferDraftFlushSnapshotForPath/);
  assert.match(fileBufferSync, /queueFileBufferDraftFlushSnapshotForPath/);
  assert.match(fileBufferSync, /applyFileBufferChangeSet/);
  assert.match(fileBufferSync, /setFileBufferDraft/);
  assert.doesNotMatch(`${tipTapEditor}\n${markdownEditor}`, /writeFile|writeTextFile|save_project_file|invoke\(/);
});

test("legacy parallel-authority modules and symbols are absent", () => {
  const removedFiles = [
    "src/lib/session/file-buffer-save-settlement.ts",
    "src/lib/session/save-domain-settlement.ts",
    "src/lib/kernel/undo-redo-settlement.ts",
    "src/lib/session/page-js-edits.ts",
    "src/lib/session/change-log/index.ts",
    "src-tauri/src/preview/proxy.rs",
    "src-tauri/src/preview/bridge/09_live_mutations.js",
    "src-tauri/permissions/autogenerated/confirm_canvas_projection.toml",
  ];
  for (const path of removedFiles) assert.equal(existsSync(resolve(repoRoot, path)), false, path);

  const forbidden = [
    "InternalWriteEvidence",
    "record_file_buffer_draft_recovery_journal",
    "record_file_buffer_draft_sync_recovery",
    "projectCommittedPreviewStructuralDiskWrite",
    "resetPreviewTemplatesFromDisk",
    "ArchiveLegacyExternalConfig",
    "archive_legacy_record",
    "confirm_canvas_projection",
    "replayLiveCssToPreview",
    "WriteOwner::Css",
    "WriteOwner::Js",
    "WriteOwner::Deploy",
    "WriteOwner::FontManager",
  ];
  const currentSources = [
    ...filesUnder("src", new Set([".ts", ".svelte"])),
    ...filesUnder("src-tauri/src", new Set([".rs"])),
  ];
  for (const symbol of forbidden) {
    const matches = currentSources.filter((path) => readFileSync(path, "utf8").includes(symbol));
    assert.deepEqual(matches, [], `simbol legacy rămas: ${symbol}`);
  }
});

test("Canvas observability covers cache, stale, fallback, rollback, FOUC and JS restart", () => {
  const observability = source("src-tauri/src/kernel/observability/mod.rs");
  const previewCommands = source("src-tauri/src/commands/preview.rs");
  const previewController = source("src/lib/state/preview-controller.ts");
  const appState = source("src/lib/state/app.svelte.ts");

  for (const event of [
    "PreviewCanvasStaleDiscarded",
    "PreviewCanvasPatchRolledBack",
    "PreviewCanvasFallback",
    "PreviewCanvasCacheHit",
    "PreviewCanvasCacheMiss",
    "PreviewCanvasFoucGuardSatisfied",
    "PreviewInteractiveJsRestarted",
    "PreviewInteractiveJsFailed",
  ]) {
    assert.match(observability, new RegExp(event));
    assert.match(previewCommands, new RegExp(event));
  }
  assert.match(previewController, /recordCanvasProjectionRuntimeEvent[\s\S]*canvas_fallback/);
  assert.match(appState, /rollbackCanvasPatchInPreview[\s\S]*canvas_patch_rolled_back/);
});

test("project entry create, delete and rename are session mutations", () => {
  const entries = source("src-tauri/src/commands/workspace_entries.rs");
  const filesController = source("src/lib/state/files-controller.ts");
  assert.match(entries, /stage_resource_texts/);
  assert.match(entries, /stage_resource_changes/);
  assert.match(entries, /stage_composite_changes/);
  assert.match(filesController, /Ctrl\+S persistă pe disc/);
  assert.doesNotMatch(filesController, /acceptedManifest|InternalWriteEvidence/);
});

test("external disk reconciliation remains a conflict gate, never an internal edit authority", () => {
  const external = source("src/lib/state/external-disk-controller.ts");
  const projectState = source("src-tauri/src/kernel/project_state/assessment/evaluator.rs");
  assert.match(external, /workspaceProjectionRecoveryRequired/);
  assert.match(external, /reconcileCleanExternalProjectFiles/);
  assert.doesNotMatch(external, /projectAcceptedInternalDiskManifest|completeInternalDiskProjection/);
  assert.match(projectState, /ProjectWorkspace/);
  assert.match(projectState, /DiskConflict/);
});
